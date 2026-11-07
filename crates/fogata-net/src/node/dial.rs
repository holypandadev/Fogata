//! Dials the node makes itself: addresses given or remembered (whose peers count as chosen
//! even when they answer later), the remembered ones on a network this device is on, and
//! the punch dials to members found on the DHT, at the same wall-clock slots at both ends.

use std::{
    collections::HashSet,
    net::IpAddr,
    time::{Duration, Instant},
};

use fogata_crypto::DevicePublicKey;
use libp2p::{
    Multiaddr, PeerId,
    multiaddr::Protocol,
    swarm::{
        ConnectionId,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use super::{ConnectionInfo, Node};
use crate::{NodeEvent, Path, addr, lan};

/// The /24 of a private or link-local IPv4 address: an address that names a computer only on
/// that network.
pub(super) fn private_block(address: &Multiaddr) -> Option<[u8; 3]> {
    match addr::ip(address) {
        Some(IpAddr::V4(ip)) if ip.is_private() || ip.is_link_local() => {
            let [a, b, c, _] = ip.octets();
            Some([a, b, c])
        }
        _ => None,
    }
}

/// Addresses dialled again at start and on a network change, at most.
pub(super) const MAX_REMEMBERED: usize = 16;
/// Addresses members and friends came from in this run that the connection limits keep
/// places for, at most (see `Node::came_from`).
const MAX_CAME_FROM: usize = 64;
/// How long a connection after a punch dial is taken as hole punched.
pub(super) const PUNCHED_WITHIN: Duration = Duration::from_secs(30);

/// How long a peer dialled at an address given or remembered, which did not answer, still
/// counts as chosen when it connects later, and how many are kept so.
const EXPLICIT_FOR: Duration = Duration::from_secs(30 * 60);
const MAX_EXPLICIT_PEERS: usize = 256;

impl Node {
    /// Dials addresses given or remembered. Those of one peer go in one dial, which tries
    /// them together and keeps the first that answers: dialled one by one, every address
    /// after the first would be turned away while the first is still being tried, and a
    /// first that does not answer (another network's, or a connection the system has not
    /// let go of yet) would leave the peer unreached.
    pub(super) fn dial_addresses(&mut self, addresses: impl IntoIterator<Item = Multiaddr>) {
        let mut by_peer: Vec<(PeerId, Vec<Multiaddr>)> = Vec::new();
        for address in addresses {
            // A sealed name is never looked up in DNS.
            if lan::is_name(&address) {
                continue;
            }
            match address.iter().last() {
                Some(Protocol::P2p(peer)) => match by_peer.iter_mut().find(|(p, _)| *p == peer) {
                    Some((_, addrs)) => addrs.push(address),
                    None => by_peer.push((peer, vec![address])),
                },
                _ => self.dial_opts(DialOpts::unknown_peer_id().address(address).build()),
            }
        }
        let now = Instant::now();
        self.explicit_peers
            .retain(|_, since| now.duration_since(*since) < EXPLICIT_FOR);
        for (peer, addrs) in by_peer {
            self.interesting.insert(peer);
            self.wanted.want(peer);
            if self.explicit_peers.len() < MAX_EXPLICIT_PEERS
                || self.explicit_peers.contains_key(&peer)
            {
                self.explicit_peers.insert(peer, now);
            }
            self.dial_opts(
                DialOpts::peer_id(peer)
                    .addresses(addrs)
                    .condition(PeerCondition::DisconnectedAndNotDialing)
                    .build(),
            );
        }
    }

    pub(super) fn dial_opts(&mut self, opts: DialOpts) {
        self.explicit_dials.insert(opts.connection_id());
        if let Err(e) = self.swarm.dial(opts) {
            tracing::debug!("dial failed: {e}");
        }
    }

    /// Dials a member found on the DHT. Both ends do this at the same wall-clock slots. In
    /// each slot one end dials normally and the other dials "as listener", which makes QUIC
    /// send packets that open its own NAT for the other end's incoming handshake. The roles
    /// swap every slot, so that a device that cannot be reached at all still gets slots in
    /// which it dials out itself. Without a slot (right after discovery) it dials normally.
    ///
    /// libp2p's QUIC transport only takes the hole-punching path for a listener-role dial
    /// that asks for a new port; it still sends from the listening socket.
    /// Returns whether a dial was started.
    pub(super) fn dial_candidate(
        &mut self,
        peer: PeerId,
        addrs: Option<Vec<Multiaddr>>,
        slot: Option<u64>,
    ) -> bool {
        let as_listener = slot.is_some_and(|slot| {
            (self.local_peer().to_bytes() > peer.to_bytes()) ^ slot.is_multiple_of(2)
        });
        // Not a stranger: dial every address the DHT gave, and keep the connection.
        self.wanted.want(peer);
        // A member reached through another member is dialled although connected.
        let condition = if self.only_relayed(&peer) {
            PeerCondition::NotDialing
        } else {
            PeerCondition::DisconnectedAndNotDialing
        };
        let opts = match addrs {
            Some(addrs) => {
                let opts = DialOpts::peer_id(peer)
                    .addresses(addrs)
                    .condition(condition);
                if as_listener {
                    opts.override_role().allocate_new_port()
                } else {
                    opts
                }
                .build()
            }
            None => {
                let opts =
                    DialOpts::peer_id(peer).condition(PeerCondition::DisconnectedAndNotDialing);
                if as_listener {
                    opts.override_role().allocate_new_port()
                } else {
                    opts
                }
                .build()
            }
        };
        self.punching.insert(peer, Instant::now());
        if slot.is_some() {
            self.punch_dials.insert(opts.connection_id());
        }
        match self.swarm.dial(opts) {
            Ok(()) => true,
            Err(e) => {
                // Already connected or dialling, or an announcement without addresses.
                tracing::trace!("dial to {peer} not started: {e}");
                false
            }
        }
    }

    /// Dials the remembered addresses (see `remembered`) of devices that are still friends or
    /// members, but those in use now; a private address only while this device is on a
    /// network of the same block, since elsewhere it names some other computer.
    pub(super) fn dial_remembered(&mut self) {
        self.dial_remembered_in(None);
    }

    /// [`Self::dial_remembered`], or only the private addresses in `block` when given.
    pub(super) fn dial_remembered_in(&mut self, block: Option<[u8; 3]>) {
        let in_use: HashSet<Multiaddr> = self
            .connections
            .values()
            .map(|c| addr::without_peer(&c.address))
            .collect();
        let own_blocks: HashSet<[u8; 3]> =
            self.listen_addrs.iter().filter_map(private_block).collect();
        let addresses: Vec<Multiaddr> = self
            .remembered
            .iter()
            .filter(|(address, device)| {
                let reachable = match (private_block(address), block) {
                    (Some(theirs), Some(only)) => theirs == only,
                    (None, Some(_)) => false,
                    (Some(theirs), None) => own_blocks.contains(&theirs),
                    (None, None) => true,
                };
                reachable
                    && !in_use.contains(&addr::without_peer(address))
                    && self.still_known(device)
            })
            .map(|(address, _)| address.clone())
            .collect();
        self.dial_addresses(addresses);
    }

    /// Whether `device` is a friend of this device or a member of one of its groups.
    pub(super) fn still_known(&self, device: &DevicePublicKey) -> bool {
        self.friends
            .as_ref()
            .is_some_and(|f| f.book.friends.contains_key(device))
            || self.groups.values().any(|s| s.ctx.is_member_device(device))
    }

    /// Notes that friend or member `device` is connected at `info`'s address: as one it came
    /// from, when the connection is not through a relay (see `came_from`), and to dial again,
    /// when it is a direct, hole-punched or local QUIC connection.
    pub(super) fn note_reached(&mut self, info: &ConnectionInfo, device: DevicePublicKey) {
        if !matches!(info.path, Path::Direct | Path::HolePunched | Path::Lan)
            || addr::is_relayed(&info.address)
        {
            return;
        }
        if let Some(ip) = addr::ip(&info.address)
            && self.came_from.front() != Some(&(ip, device))
        {
            self.came_from.retain(|(i, _)| *i != ip);
            self.came_from.push_front((ip, device));
            self.came_from.truncate(MAX_CAME_FROM);
            self.publish_known_addresses();
        }
        if !addr::is_quic(&info.address) {
            return;
        }
        let address = addr::without_peer(&info.address);
        self.remembered
            .retain(|(a, _)| addr::without_peer(a) != address);
        self.remembered.insert(0, (address.clone(), device));
        self.remembered.truncate(MAX_REMEMBERED);
        self.publish_known_addresses();
        if self.reached.insert(address.clone()) {
            self.emit(NodeEvent::Reached { address, device });
        }
    }

    /// Whether a connection is to a peer this device chose to dial: by that dial, or by a
    /// later one after it failed.
    pub(super) fn dialled_here(&mut self, peer: PeerId, connection: ConnectionId) -> bool {
        let by_this = self.explicit_dials.remove(&connection);
        let chosen = self
            .explicit_peers
            .remove(&peer)
            .is_some_and(|since| since.elapsed() < EXPLICIT_FOR);
        by_this || chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::tests::{inspect, lone_member};

    /// A peer dialled at an address given that did not answer still counts as chosen here
    /// when a later dial reaches it; the failed dial itself is not kept.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_given_peer_that_did_not_answer_counts_as_chosen_when_it_does() {
        let (node, _group) = lone_member();
        let peer = PeerId::random();
        // Nothing listens there.
        let closed = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = closed.local_addr().unwrap().port();
        drop(closed);
        let address: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1/p2p/{peer}")
            .parse()
            .unwrap();
        inspect(&node, move |n| n.dial_addresses([address])).await;
        let gone = tokio::time::timeout(Duration::from_secs(20), async {
            while inspect(&node, |n| !n.explicit_dials.is_empty()).await {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(gone.is_ok(), "the failed dial was kept");
        let chosen = inspect(&node, move |n| {
            let later = ConnectionId::new_unchecked(usize::MAX);
            let first = n.dialled_here(peer, later);
            (first, n.dialled_here(peer, later))
        })
        .await;
        assert_eq!(
            chosen,
            (true, false),
            "a later dial's connection was not its choice"
        );
        node.shutdown().await.unwrap();
    }
}
