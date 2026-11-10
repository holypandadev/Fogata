//! Other ways to reach members: relay slots kept at a few of them, dials through a member
//! to one this device cannot reach directly, the local addresses members send, and members
//! on the local network dialled again for a while after their connection was lost.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_proto::MemberRequest;
use libp2p::{
    Multiaddr, PeerId,
    multiaddr::Protocol,
    swarm::{
        ConnectionId,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use super::{ConnectionInfo, Node, Pending, dial::private_block, links::THROUGH_DIAL_WAIT};
use crate::{Path, addr};

/// Most local addresses kept for a member.
const MAX_LAN_ADDRS: usize = 8;

/// Members we keep a relay slot at, so that others can reach us through them. Enough for
/// every member of a group of friends: with fewer, the slots could all go to members on our
/// own network, and a member who can only reach one of the others could not reach us.
const MAX_RESERVATIONS: usize = 8;
/// A member on the local network whose last QUIC connection closed while it was still a
/// member (an outage longer than the QUIC idle timeout, say) is dialled again there, first
/// after `REDIAL_FIRST` and then twice as long each time up to `REDIAL_MAX`, for
/// `REDIAL_FOR`. Nothing else would: mDNS tells of a device again only once it has forgotten
/// it, and the punch slots dial no local address.
const REDIAL_FIRST: Duration = Duration::from_secs(2);
const REDIAL_MAX: Duration = Duration::from_secs(16);
const REDIAL_FOR: Duration = Duration::from_secs(5 * 60);
/// Most lost members dialled again at once.
const MAX_LOST: usize = 32;

/// A member whose last QUIC connection closed, dialled again for a while
/// ([`Node::redial_lost`]).
pub(super) struct Lost {
    device: DevicePublicKey,
    addresses: Vec<Multiaddr>,
    since: Instant,
    next: Instant,
    wait: Duration,
    /// The dial on its way, if any.
    dial: Option<ConnectionId>,
}

/// The QUIC addresses on local networks among those a member sent (see
/// [`MemberRequest::Addresses`]), at most [`MAX_LAN_ADDRS`]. Public ones are left out: the
/// member's own connection already shows those, and anything else would let a member make
/// this device send packets anywhere.
pub(super) fn lan_quic(addresses: Vec<serde_bytes::ByteBuf>) -> Vec<Multiaddr> {
    addresses
        .into_iter()
        .filter_map(|a| Multiaddr::try_from(a.into_vec()).ok())
        .filter(|a| addr::is_quic(a) && !addr::is_relayed(a) && addr::is_lan(a))
        .take(MAX_LAN_ADDRS)
        .collect()
}

impl Node {
    /// Keeps a relay slot at a directly connected member, so that members who cannot reach
    /// us directly can come through it. Retried on every heartbeat, because the member may
    /// not have verified us yet when we first ask.
    pub(super) fn reserve_slot(&mut self, peer: PeerId) {
        let Some(member) = self
            .members
            .get(&peer)
            .filter(|_| self.in_regular_group(&peer))
        else {
            return;
        };
        if matches!(member.path, Path::ThroughMember { .. })
            || self.reservations.len() >= MAX_RESERVATIONS
            || self.reservations.values().any(|p| *p == peer)
        {
            return;
        }
        let circuit = addr::without_peer(&member.address)
            .with(Protocol::P2p(peer))
            .with(Protocol::P2pCircuit);
        match self.swarm.listen_on(circuit) {
            Ok(listener) => {
                self.reservations.insert(listener, peer);
            }
            Err(e) => tracing::debug!("cannot reserve a slot at {peer}: {e}"),
        }
    }

    /// Connects to `other`, a member of `group`, through `via`, a member that is directly
    /// connected to both.
    pub(super) fn reach_through(&mut self, via: PeerId, other: PeerId, group: GroupId) {
        if other == self.local_peer() {
            return;
        }
        self.interesting.insert(other);
        self.hinted.entry(other).or_default().insert(group);
        // Connected, unless only by connections that went silent: then the way through
        // `via` is opened beside them (see `check_member_links`).
        let silent = self.silent.contains_key(&other);
        let connected = self.connections.values().any(|c| c.peer == other);
        if connected && !silent {
            // Kept open while it is checked, but not spared when incoming connections run
            // out, as a peer this device dials is: any member can name any peers in its
            // roster, and peers connected with those would otherwise hold places that are
            // never freed for as long as it names them.
            self.wanted.hear(other);
            self.approach(other, Some(group));
            return;
        }
        self.wanted.want(other);
        let Some(member) = self.members.get(&via) else {
            return;
        };
        if matches!(member.path, Path::ThroughMember { .. }) || self.has_relayed(&other) {
            return;
        }
        // One way through a member at a time: several members may name it at once.
        if self
            .through_dials
            .get(&other)
            .is_some_and(|t| t.elapsed() < THROUGH_DIAL_WAIT)
        {
            return;
        }
        let circuit = addr::without_peer(&member.address)
            .with(Protocol::P2p(via))
            .with(Protocol::P2pCircuit)
            .with(Protocol::P2p(other));
        // A direct attempt may be under way (a punch slot, an address remembered) and fail
        // only after seconds: the way through the member does not wait for it.
        let opts = DialOpts::peer_id(other)
            .addresses(vec![circuit])
            .condition(if silent {
                PeerCondition::Always
            } else {
                PeerCondition::Disconnected
            })
            .build();
        match self.swarm.dial(opts) {
            Ok(()) => {
                tracing::debug!(%other, %via, "connecting through a member");
                self.through_dials.insert(other, Instant::now());
            }
            Err(e) => tracing::debug!(%other, "cannot connect through {via}: {e}"),
        }
    }

    /// Notes a member whose last QUIC connection just closed (`closed`), with where to dial
    /// it again: where it was reached on the local network, and the local addresses it sent.
    /// Public addresses are left to the punch slots, which a dial on its way would hold off.
    /// The member keeps its transport identity until it starts again, so only that session
    /// can answer.
    pub(super) fn keep_lost(&mut self, peer: PeerId, closed: Option<&ConnectionInfo>) {
        let Some(member) = self.members.get(&peer) else {
            return;
        };
        if self.lost.len() >= MAX_LOST && !self.lost.contains_key(&peer) {
            return;
        }
        let reached = [
            Some((&member.path, &member.address)),
            closed.map(|c| (&c.path, &c.address)),
        ];
        let mut addresses: Vec<Multiaddr> = Vec::new();
        let reached = reached
            .into_iter()
            .flatten()
            .filter(|(path, a)| **path == Path::Lan && addr::is_quic(a) && !addr::is_relayed(a))
            .map(|(_, a)| addr::without_peer(a));
        for address in reached.chain(self.member_lan.get(&peer).into_iter().flatten().cloned()) {
            if !addresses.contains(&address) && addresses.len() < MAX_LAN_ADDRS {
                addresses.push(address);
            }
        }
        if addresses.is_empty() {
            return;
        }
        let now = Instant::now();
        self.lost.insert(
            peer,
            Lost {
                device: member.device,
                addresses,
                since: now,
                next: now + REDIAL_FIRST,
                wait: REDIAL_FIRST,
                dial: None,
            },
        );
    }

    /// Dials the lost members (see [`Self::keep_lost`]) whose turn it is. One is let go once
    /// it has a direct or local QUIC connection again, is no longer a member or is back as
    /// another session, or after [`REDIAL_FOR`]; the punch slots and discovery go on as for
    /// anyone. A private address is dialled only while this device is on a network of the
    /// same block, since elsewhere it names some other computer.
    pub(super) fn redial_lost(&mut self) {
        if self.lost.is_empty() {
            return;
        }
        let now = Instant::now();
        let quic: HashSet<PeerId> = self
            .connections
            .values()
            .filter(|c| addr::is_quic(&c.address) && !matches!(c.path, Path::ThroughMember { .. }))
            .map(|c| c.peer)
            .collect();
        let (members, groups) = (&self.members, &self.groups);
        self.lost.retain(|peer, lost| {
            now.duration_since(lost.since) < REDIAL_FOR
                && !quic.contains(peer)
                && !members
                    .iter()
                    .any(|(p, m)| p != peer && m.device == lost.device)
                && groups
                    .values()
                    .any(|s| s.ctx.is_member_device(&lost.device))
        });
        let own_blocks: HashSet<[u8; 3]> =
            self.listen_addrs.iter().filter_map(private_block).collect();
        let due: Vec<(PeerId, Vec<Multiaddr>)> = self
            .lost
            .iter_mut()
            .filter(|(_, lost)| lost.next <= now)
            .map(|(peer, lost)| {
                lost.wait = (lost.wait * 2).min(REDIAL_MAX);
                lost.next = now + lost.wait;
                let addresses: Vec<Multiaddr> = lost
                    .addresses
                    .iter()
                    .filter(|a| private_block(a).is_none_or(|b| own_blocks.contains(&b)))
                    .cloned()
                    .collect();
                (*peer, addresses)
            })
            .filter(|(_, addresses)| !addresses.is_empty())
            .collect();
        for (peer, addresses) in due {
            self.interesting.insert(peer);
            self.wanted.want(peer);
            let opts = DialOpts::peer_id(peer)
                .addresses(addresses)
                .condition(PeerCondition::NotDialing)
                .build();
            let id = opts.connection_id();
            if self.swarm.dial(opts).is_ok() {
                tracing::debug!(%peer, "dialling a lost member again");
                if let Some(lost) = self.lost.get_mut(&peer) {
                    lost.dial = Some(id);
                }
            }
        }
    }

    /// Whether `connection` is a dial to a lost member (see [`Self::redial_lost`]).
    pub(super) fn is_redial(&self, peer: PeerId, connection: ConnectionId) -> bool {
        self.lost
            .get(&peer)
            .is_some_and(|l| l.dial == Some(connection))
    }

    /// Asks the other members of `groups` for their rosters at once, rather than at the
    /// next heartbeat: `lost` went from them, or its connections went silent, while they
    /// may still reach it, and the way through one of them is found within moments.
    pub(super) fn look_through_others(&mut self, lost: PeerId, groups: &[GroupId]) {
        for group in groups {
            let Some(slot) = self.groups.get(group).filter(|s| !s.direct) else {
                continue;
            };
            let others: Vec<PeerId> = slot
                .members
                .iter()
                .filter(|p| **p != lost)
                .filter(|p| {
                    self.members
                        .get(p)
                        .is_some_and(|m| !matches!(m.path, Path::ThroughMember { .. }))
                })
                .copied()
                .collect();
            for peer in others {
                self.request(
                    peer,
                    MemberRequest::Roster {
                        group_id: group.as_bytes().to_vec(),
                        heads: None,
                        banner: None,
                        picture: None,
                    },
                    Pending::Roster(*group),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fogata_proto::MemberResponse;
    use libp2p::{Multiaddr, core::ConnectedPoint, swarm::ConnectionId};

    use super::*;
    use crate::node::tests::{add_member, inspect, lone_member};

    /// A peer connected to this device that a member's roster names is checked, but not
    /// spared when incoming connections run out; one this device dials through the member
    /// is.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_connected_peer_a_roster_names_is_not_spared() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, connected, away) = (PeerId::random(), PeerId::random(), PeerId::random());
            add_member(n, group, bea);
            n.on_connection_established(
                connected,
                ConnectionId::new_unchecked(9700),
                &ConnectedPoint::Listener {
                    local_addr: "/ip4/0.0.0.0/udp/4001/quic-v1".parse().unwrap(),
                    send_back_addr: "/ip4/192.0.2.70/udp/4001/quic-v1".parse().unwrap(),
                },
            );
            n.on_response(
                bea,
                Pending::Roster(group),
                MemberResponse::Roster {
                    peers: [connected, away]
                        .iter()
                        .map(|p| serde_bytes::ByteBuf::from(p.to_bytes()))
                        .collect(),
                },
            );
            (n.wanted.is_wanted(&connected), n.wanted.is_wanted(&away))
        })
        .await;
        assert_eq!(found, (false, true));
        node.shutdown().await.unwrap();
    }

    #[test]
    fn only_local_quic_addresses_a_member_sends_are_kept() {
        let sent = [
            "/ip4/192.168.1.20/udp/30000/quic-v1",
            "/ip4/203.0.113.9/udp/30000/quic-v1",
            "/ip4/127.0.0.1/udp/30000/quic-v1",
            "/ip4/192.168.1.20/tcp/30000",
            "/ip6/fd00::20/udp/30000/quic-v1",
        ]
        .map(|a| serde_bytes::ByteBuf::from(a.parse::<Multiaddr>().unwrap().to_vec()));
        let kept: Vec<String> = lan_quic(sent.to_vec())
            .iter()
            .map(|a| a.to_string())
            .collect();
        assert_eq!(
            kept,
            [
                "/ip4/192.168.1.20/udp/30000/quic-v1",
                "/ip6/fd00::20/udp/30000/quic-v1"
            ]
        );
    }
}
