//! How each member is reached: the best of its connections, told to its groups when it
//! changes; a QUIC connection beside one that cannot carry media; and direct connections
//! that went silent left for a way through another member, and back.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use fogata_crypto::GroupId;
use libp2p::{
    Multiaddr, PeerId,
    swarm::{
        ConnectionId,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use super::{ConnectionInfo, Member, Node};
use crate::{NodeEvent, Path, addr};

/// How often the direct connections to members are checked for silence, and how long one
/// may hear nothing before the member is reached through another member instead. QUIC
/// keep-alives every 5 s make a live connection hear something at least that often; a
/// few more seconds leave room for a slow path.
pub(super) const LINK_CHECK: Duration = Duration::from_secs(2);
const LINK_SILENT: Duration = Duration::from_secs(10);
/// How long a connection through a member stays once the member is reached directly
/// again, so that the other end has its direct connection too before this one goes.
const RELAYED_KEPT: Duration = Duration::from_secs(3);
/// How long a dial through a member is given before another member's way is tried.
pub(super) const THROUGH_DIAL_WAIT: Duration = Duration::from_secs(10);

/// How long a path worse than the one told must last before it is told.
const STEADY: Duration = Duration::from_secs(2);

/// Most addresses one QUIC dial for a member's media goes to: a member's own device listens
/// on a few at most, and each is a handshake sent wherever the address points.
pub(super) const MAX_QUIC_ADDRESSES: usize = 4;
/// How long after a QUIC dial for a member's media another goes to it, at the soonest: a
/// member that closes its QUIC link again and again, while another stays, would otherwise
/// have this device start a new dial each time, the earlier ones still on their way.
pub(super) const QUIC_REDIAL: Duration = Duration::from_secs(60);

/// `path` as told to the members of a group, `in_group` the peers that proved membership of
/// it: the member whose relay slot carries the path is named only when it is one of them, so
/// nobody learns of a member of a group it is not in.
fn path_in_group(
    path: &Path,
    in_group: &HashSet<PeerId>,
    members: &HashMap<PeerId, Member>,
) -> Path {
    match path {
        Path::ThroughMember { relay, .. } => Path::ThroughMember {
            relay: *relay,
            member: in_group
                .contains(relay)
                .then(|| members.get(relay).map(|m| m.device))
                .flatten(),
        },
        other => other.clone(),
    }
}

/// Lower is better. Direct and hole-punched paths are equally good once open.
fn path_rank(path: &Path) -> u8 {
    match path {
        Path::Lan => 0,
        Path::Direct | Path::HolePunched => 1,
        Path::ThroughMember { .. } => 2,
    }
}

impl Node {
    /// The best open connection to `peer`.
    /// `peer`'s best open connection: the best path, QUIC first on the same path (media
    /// travels on QUIC), the oldest first after that, so the choice holds still.
    pub(super) fn best_connection(&self, peer: &PeerId) -> Option<ConnectionInfo> {
        self.connections
            .iter()
            .filter(|(id, c)| c.peer == *peer && !self.closing.contains(id))
            .min_by_key(|(id, c)| (path_rank(&c.path), !addr::is_quic(&c.address), **id))
            .map(|(_, c)| c.clone())
    }

    /// A member reached only over TCP (or through another member) cannot receive media
    /// datagrams: try once to open a QUIC connection to the addresses it listens on.
    /// Tried again at each heartbeat while one is not due yet (see [`QUIC_REDIAL`]).
    pub(super) fn want_datagrams(&mut self, peer: PeerId) {
        if !self.in_regular_group(&peer)
            || crate::datagram::link(&peer).is_some()
            || self.quic_tried.contains(&peer)
            || self.quic_dialled_lately(&peer)
        {
            return;
        }
        let quic = self.member_quic_addresses(&peer);
        if quic.is_empty() {
            return;
        }
        self.quic_tried.insert(peer);
        tracing::debug!(%peer, "also connecting over QUIC for media");
        self.dial_quic(peer, quic);
    }

    /// Whether a QUIC dial for `peer`'s media went less than [`QUIC_REDIAL`] ago.
    fn quic_dialled_lately(&self, peer: &PeerId) -> bool {
        self.quic_dialled
            .get(peer)
            .is_some_and(|at| at.elapsed() < QUIC_REDIAL)
    }

    /// The QUIC addresses a member listens on, its local ones first: the public ones fail
    /// between two devices behind the same router, which rarely sends traffic back in. Only
    /// those where the member is seen to be, at most [`MAX_QUIC_ADDRESSES`]: an address it
    /// says it listens on at the IP of one of its own connections here, and a local one it
    /// sent on a local network of this device's block (see [`Self::member_lan_for`]).
    /// Anything else a member says would let it make this device send handshakes to any
    /// address, its own network's included.
    pub(super) fn member_quic_addresses(&self, peer: &PeerId) -> Vec<Multiaddr> {
        let seen_at: HashSet<std::net::IpAddr> = self
            .connections
            .values()
            .filter(|c| c.peer == *peer && !matches!(c.path, Path::ThroughMember { .. }))
            .filter_map(|c| addr::ip(&c.address))
            .collect();
        let own_blocks = self.own_blocks();
        let lan = self.member_lan_for(peer, &own_blocks);
        let listening = self
            .peer_listening
            .get(peer)
            .into_iter()
            .flatten()
            .filter(|a| addr::ip(a).is_some_and(|ip| seen_at.contains(&ip)));
        let mut out: Vec<Multiaddr> = Vec::new();
        for address in lan.chain(listening) {
            if !addr::is_quic(address) || addr::is_relayed(address) {
                continue;
            }
            let address = addr::without_peer(address);
            if !out.contains(&address) {
                out.push(address);
            }
            if out.len() == MAX_QUIC_ADDRESSES {
                break;
            }
        }
        out
    }

    /// Dials `peer` over QUIC at `quic`, even while connected: the media link is a QUIC
    /// connection of its own.
    pub(super) fn dial_quic(&mut self, peer: PeerId, quic: Vec<Multiaddr>) {
        self.quic_dialled.insert(peer, Instant::now());
        let opts = DialOpts::peer_id(peer)
            .addresses(quic)
            .condition(PeerCondition::Always)
            .build();
        let _ = self.swarm.dial(opts);
    }

    /// Picks the best open connection to a member and reports it, for each of its groups, if
    /// it changed.
    pub(super) fn update_member_path(&mut self, peer: PeerId) {
        let Some(best) = self.best_connection(&peer) else {
            return;
        };
        let Some(member) = self.members.get_mut(&peer) else {
            return;
        };
        // Better or worse: the member's path is that of its best open connection, which
        // relaying and rosters rely on.
        if best.path == member.path && best.address == member.address {
            return;
        }
        member.path = best.path.clone();
        member.address = best.address.clone();
        let device = member.device;
        self.note_reached(&best, device);
        self.tell_path(peer);
        if !matches!(best.path, Path::ThroughMember { .. }) && self.has_relayed(&peer) {
            // Not at once: the other end may not have the new connection yet, and would
            // see the member gone in between.
            self.redundant.insert(peer, Instant::now());
        }
    }

    /// Tells the groups of member `peer` how it is reached, when that changed: a better path
    /// at once, another one once it has lasted [`STEADY`].
    fn tell_path(&mut self, peer: PeerId) {
        let Some(member) = self.members.get_mut(&peer) else {
            return;
        };
        if member.path == member.told {
            member.worse_since = None;
            return;
        }
        let better = path_rank(&member.path) < path_rank(&member.told);
        let since = *member.worse_since.get_or_insert_with(Instant::now);
        if !better && since.elapsed() < STEADY {
            return;
        }
        member.told = member.path.clone();
        member.worse_since = None;
        let (device, path, address) = (member.device, member.path.clone(), member.address.clone());
        let groups: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, s)| s.members.contains(&peer))
            .map(|(id, _)| *id)
            .collect();
        for group in groups {
            self.emit(NodeEvent::MemberConnected {
                group,
                device,
                peer,
                path: self.path_in(&group, &path),
                address: address.clone(),
            });
        }
    }

    /// `path` as told to `group` (see [`path_in_group`]).
    pub(super) fn path_in(&self, group: &GroupId, path: &Path) -> Path {
        let none = HashSet::new();
        let in_group = self.groups.get(group).map_or(&none, |s| &s.members);
        path_in_group(path, in_group, &self.members)
    }

    /// `relay` just proved membership of `group`: the group's members that come through its
    /// relay slot are told again, now naming it.
    pub(super) fn name_carrier(&mut self, group: GroupId, relay: PeerId) {
        let Some(slot) = self.groups.get(&group).filter(|s| !s.direct) else {
            return;
        };
        let carried: Vec<_> = slot
            .members
            .iter()
            .filter_map(|peer| {
                let m = self.members.get(peer)?;
                matches!(m.path, Path::ThroughMember { relay: r, .. } if r == relay)
                    .then(|| (*peer, m.device, m.path.clone(), m.address.clone()))
            })
            .collect();
        for (peer, device, path, address) in carried {
            self.emit(NodeEvent::MemberConnected {
                group,
                device,
                peer,
                path: self.path_in(&group, &path),
                address,
            });
        }
    }

    /// Whether `peer` is a member reached only through another member.
    pub(super) fn only_relayed(&self, peer: &PeerId) -> bool {
        self.members
            .get(peer)
            .is_some_and(|m| matches!(m.path, Path::ThroughMember { .. }))
    }

    /// Whether a connection to `peer` goes through a member.
    pub(super) fn has_relayed(&self, peer: &PeerId) -> bool {
        self.connections
            .values()
            .any(|c| c.peer == *peer && matches!(c.path, Path::ThroughMember { .. }))
    }

    /// Finds members whose direct connections went silent (client isolation on a Wi-Fi, a
    /// NAT that forgot its mapping), which would otherwise stay open until their idle
    /// timeout while messages and media sent on them are lost. The other members are asked
    /// for a way to them, and once one is open the silent connections are closed.
    pub(super) fn check_member_links(&mut self) {
        self.close_redundant_relayed();
        self.redial_lost();
        self.drop_cut_off();
        let waiting: Vec<PeerId> = self
            .members
            .iter()
            .filter(|(_, m)| m.worse_since.is_some())
            .map(|(p, _)| *p)
            .collect();
        for peer in waiting {
            self.tell_path(peer);
        }
        let direct: Vec<PeerId> = self
            .members
            .iter()
            .filter(|(_, m)| !matches!(m.path, Path::ThroughMember { .. }))
            .map(|(p, _)| *p)
            .collect();
        self.silent.retain(|p, _| direct.contains(p));
        for peer in direct {
            // Only QUIC connections tell; a member reached over TCP alone is left to the
            // pings.
            let silent = crate::datagram::silent_for(&peer).is_some_and(|d| d >= LINK_SILENT);
            if !silent {
                self.silent.remove(&peer);
                continue;
            }
            if self.has_relayed(&peer) {
                self.leave_silent_links(peer);
                continue;
            }
            if self
                .silent
                .get(&peer)
                .is_some_and(|t| t.elapsed() < THROUGH_DIAL_WAIT)
            {
                continue;
            }
            tracing::debug!(%peer, "a member's direct connections went silent");
            self.silent.insert(peer, Instant::now());
            let groups: Vec<GroupId> = self
                .groups
                .iter()
                .filter(|(_, s)| s.members.contains(&peer))
                .map(|(id, _)| *id)
                .collect();
            self.look_through_others(peer, &groups);
        }
    }

    /// Closes the direct connections to a member that went silent, now that a way through
    /// another member is open: its path becomes that way. Should the direct way still work
    /// after all, the hole punch through the member brings it back.
    pub(super) fn leave_silent_links(&mut self, peer: PeerId) {
        self.silent.remove(&peer);
        let direct: Vec<ConnectionId> = self
            .connections
            .iter()
            .filter(|(_, c)| c.peer == peer && !matches!(c.path, Path::ThroughMember { .. }))
            .map(|(id, _)| *id)
            .collect();
        tracing::debug!(%peer, "silent direct connections closed for the way through a member");
        // Marked first: as each one goes, the member's path must become the way through the
        // member, not another silent connection that would close that way as redundant.
        self.closing.extend(direct.iter().copied());
        for id in direct {
            self.swarm.close_connection(id);
        }
    }

    /// Once a member is reachable without a relay, the relayed connections are redundant;
    /// they close [`RELAYED_KEPT`] later, if the member is still reached directly then.
    fn close_redundant_relayed(&mut self) {
        let due: Vec<PeerId> = self
            .redundant
            .iter()
            .filter(|(_, since)| since.elapsed() >= RELAYED_KEPT)
            .map(|(peer, _)| *peer)
            .collect();
        for peer in due {
            self.redundant.remove(&peer);
            let direct = self
                .members
                .get(&peer)
                .is_some_and(|m| !matches!(m.path, Path::ThroughMember { .. }));
            if direct {
                self.close_relayed_connections(peer);
            }
        }
    }

    pub(super) fn close_relayed_connections(&mut self, peer: PeerId) {
        let relayed: Vec<ConnectionId> = self
            .connections
            .iter()
            .filter(|(_, c)| c.peer == peer && matches!(c.path, Path::ThroughMember { .. }))
            .map(|(id, _)| *id)
            .collect();
        for id in relayed {
            self.swarm.close_connection(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;
    use libp2p::core::ConnectedPoint;

    use super::*;
    use crate::node::tests::{add_member, inspect, lone_member};

    /// A member that says it listens on hundreds of QUIC addresses, anywhere, and sends
    /// local ones of a network this device is not on, is dialled for media only at the
    /// address its own connection comes from; and closing its QUIC link again and again
    /// does not start a new dial each time.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_is_dialled_over_quic_only_where_it_is_seen_and_not_over_and_over() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = PeerId::random();
            n.on_connection_established(
                bea,
                ConnectionId::new_unchecked(9500),
                &ConnectedPoint::Dialer {
                    address: "/ip4/192.0.2.50/tcp/4001".parse().unwrap(),
                    role_override: libp2p::core::Endpoint::Dialer,
                    port_use: libp2p::core::transport::PortUse::Reuse,
                },
            );
            add_member(n, group, bea);
            let quic = |ip: String| -> Multiaddr {
                format!("/ip4/{ip}/udp/4001/quic-v1").parse().unwrap()
            };
            let mut listening: Vec<Multiaddr> = (0..300)
                .map(|i| quic(format!("198.51.100.{}", i % 250)))
                .collect();
            listening.push(quic("127.0.0.1".into()));
            listening.push(quic("192.0.2.50".into()));
            n.peer_listening.insert(bea, listening);
            n.member_lan
                .insert(bea, vec![quic("10.9.9.9".into()), quic("10.9.9.10".into())]);
            let chosen = n.member_quic_addresses(&bea);
            let pending = |n: &Node| {
                n.swarm
                    .network_info()
                    .connection_counters()
                    .num_pending_outgoing()
            };
            let before = pending(n);
            n.want_datagrams(bea);
            let first = pending(n) - before;
            for _ in 0..10 {
                n.quic_tried.remove(&bea);
                n.want_datagrams(bea);
            }
            (chosen, first, pending(n) - before)
        })
        .await;
        let only: Multiaddr = "/ip4/192.0.2.50/udp/4001/quic-v1".parse().unwrap();
        assert_eq!(found, (vec![only], 1, 1));
        node.shutdown().await.unwrap();
    }

    /// The local addresses a member sends are dialled (for media, in the punch slots and
    /// when it is lost) on every network this device shares with it: the same home /24, the
    /// same unique-local IPv6 site, carrier-grade NAT or a VPN such as Tailscale when this
    /// device is on one too. Those of networks this device is not on are never dialled,
    /// whatever block they are in.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_s_local_addresses_are_dialled_only_on_networks_this_device_is_on() {
        let (node, group) = lone_member();
        let (shared, elsewhere) = inspect(&node, move |n| {
            let quic = |a: &str| -> Multiaddr {
                let ip: std::net::IpAddr = a.parse().unwrap();
                let at = match ip {
                    std::net::IpAddr::V4(_) => format!("/ip4/{a}/udp/4001/quic-v1"),
                    std::net::IpAddr::V6(_) => format!("/ip6/{a}/udp/4001/quic-v1"),
                };
                at.parse().unwrap()
            };
            let bea = PeerId::random();
            add_member(n, group, bea);
            let sent: Vec<Multiaddr> = [
                "192.168.1.30",
                "fd12:3456:789a:1::30",
                "100.70.8.9",
                "192.168.7.30",
                "10.0.0.1",
                "fd99:1:2:1::1",
            ]
            .iter()
            .map(|a| quic(a))
            .collect();
            n.member_lan.insert(bea, sent);
            let chosen = |n: &Node| {
                let own = n.own_blocks();
                let lan: Vec<String> = n
                    .member_lan_for(&bea, &own)
                    .map(|a| a.to_string())
                    .collect();
                let quic: Vec<String> = n
                    .member_quic_addresses(&bea)
                    .iter()
                    .map(|a| a.to_string())
                    .collect();
                assert_eq!(lan, quic);
                lan
            };
            // On the member's home network over IPv4 and IPv6, and on a Tailscale network.
            n.listen_addrs = ["192.168.1.20", "fd12:3456:789a:1:aaaa::20", "100.101.2.3"]
                .iter()
                .map(|a| quic(a))
                .collect();
            let shared = chosen(n);
            // Elsewhere, on a network of none of those blocks.
            n.listen_addrs = vec![quic("192.168.50.2"), quic("fd00:aaaa:bbbb:1::2")];
            (shared, chosen(n))
        })
        .await;
        assert_eq!(
            shared,
            [
                "/ip4/192.168.1.30/udp/4001/quic-v1",
                "/ip6/fd12:3456:789a:1::30/udp/4001/quic-v1",
                "/ip4/100.70.8.9/udp/4001/quic-v1",
            ]
        );
        assert!(elsewhere.is_empty(), "{elsewhere:?}");
        node.shutdown().await.unwrap();
    }

    #[test]
    fn a_relaying_member_is_named_only_to_its_own_groups() {
        let ana = DeviceIdentity::generate().unwrap().public_key();
        let relay = PeerId::random();
        let members = HashMap::from([(
            relay,
            Member {
                device: ana,
                path: Path::Direct,
                address: "/ip4/203.0.113.50/udp/4001/quic-v1".parse().unwrap(),
                told: Path::Direct,
                worse_since: None,
            },
        )]);
        let carried = Path::ThroughMember {
            relay,
            member: None,
        };
        // Bea comes through Ana: a group Ana is in hears it is Ana.
        let with_ana = HashSet::from([relay, PeerId::random()]);
        assert_eq!(
            path_in_group(&carried, &with_ana, &members),
            Path::ThroughMember {
                relay,
                member: Some(ana),
            }
        );
        // A group without Ana only hears it is a member.
        let without_ana = HashSet::from([PeerId::random()]);
        assert_eq!(path_in_group(&carried, &without_ana, &members), carried);
        // Not verified yet as anyone: unnamed as well.
        assert_eq!(path_in_group(&carried, &with_ana, &HashMap::new()), carried);
        // Other paths stay as they are.
        assert_eq!(
            path_in_group(&Path::HolePunched, &with_ana, &members),
            Path::HolePunched
        );
    }
}
