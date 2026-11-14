//! Which peers are kept and which are strangers: members and friends online stay wanted,
//! the addresses they come from keep places in the connection limits, and a stranger asks
//! only so much, counted by where it talks to this device from.

use std::time::Instant;

use libp2p::PeerId;

use super::{Node, limits};
use crate::{Path, addr};

impl Node {
    /// `peer` spoke a member or friend protocol: its connections stay open a while. Only
    /// members and friends online are spared when incoming connections run out; anyone can
    /// send a request, and one that is refused would otherwise keep its sender's
    /// connections safe (see strangers.rs). That includes a peer this device is checking
    /// (one on the local network, one a DHT node named): it is kept while being dialled, and
    /// then only once it proves itself.
    pub(super) fn spoke(&self, peer: PeerId) {
        if self.members.contains_key(&peer) || self.is_friend_peer(&peer) {
            self.wanted.want(peer);
        } else {
            self.wanted.hear(peer);
        }
    }

    /// Tells the connection limits the addresses members and friends are connected from
    /// (not through a relay), came from in this run or were reached at (see
    /// [`Wanted::set_known`](crate::strangers::Wanted::set_known)), so that their
    /// connections being set up get the places kept for them. Nothing a peer can say or
    /// announce puts an address here: it takes a proof of membership or a friend's session
    /// over a connection from it.
    pub(super) fn publish_known_addresses(&self) {
        let connected = self
            .connections
            .values()
            .filter(|c| {
                !matches!(c.path, Path::ThroughMember { .. })
                    && (self.members.contains_key(&c.peer) || self.is_friend_peer(&c.peer))
            })
            .map(|c| &c.address);
        let remembered = self
            .remembered
            .iter()
            .filter(|(_, device)| self.still_known(device))
            .map(|(address, _)| address);
        let came_from = self
            .came_from
            .iter()
            .filter(|(_, device)| self.still_known(device))
            .map(|(ip, _)| *ip);
        self.wanted.set_known(
            connected
                .chain(remembered)
                .filter_map(addr::ip)
                .chain(came_from),
        );
    }

    /// Members and friends online stay wanted while they are, whether or not they said
    /// anything lately.
    pub(super) fn want_proven(&self) {
        for peer in self.members.keys().chain(self.online_friend_peers()) {
            self.wanted.want(*peer);
        }
    }

    /// Whether `peer`, neither a member nor a friend here, asks more than strangers at its
    /// address, or through its member's relay, may now (see limits.rs). One not connected is
    /// not counted.
    pub(super) fn stranger_over_budget(&mut self, peer: &PeerId) -> bool {
        if self.members.contains_key(peer) || self.is_friend_peer(peer) {
            return false;
        }
        let Some(source) = self.peer_source(peer) else {
            return false;
        };
        !self.limits.take_stranger_request(source, Instant::now())
    }

    /// Where `peer` talks to us from, for the limits on strangers: the address of a
    /// connection of its own, or else the member whose relay one comes through. One whose
    /// relay is not known counts with every other such one.
    pub(super) fn peer_source(&self, peer: &PeerId) -> Option<limits::Source> {
        self.peer_ip(peer).map(limits::Source::Address).or_else(|| {
            self.connections
                .values()
                .find_map(|c| match c.path {
                    Path::ThroughMember { relay, .. } if c.peer == *peer => Some(relay),
                    _ => None,
                })
                .map(|relay| {
                    // A circuit whose relay was not found is told as coming through itself.
                    if relay == *peer {
                        limits::Source::SomeRelay
                    } else {
                        limits::Source::Relay(relay)
                    }
                })
        })
    }

    /// The address a connected peer talks to us from. Not that of a member's relay a
    /// connection comes through: whoever comes through one would share the places of that
    /// address (see `keep_early`).
    pub(super) fn peer_ip(&self, peer: &PeerId) -> Option<std::net::IpAddr> {
        self.connections
            .values()
            .filter(|c| c.peer == *peer && !addr::is_relayed(&c.address))
            .find_map(|c| addr::ip(&c.address))
    }

    /// What vouches for `peer`, not a member yet, when it asks for a search of every key
    /// (see limits.rs): a connection through a member's relay, or the address it comes
    /// from when a member or friend was reached there or is connected from there, or its
    /// address on the local network (this computer included) when it is on it.
    pub(super) fn search_vouch(&self, peer: &PeerId) -> Option<limits::Vouch> {
        let mut vouch = None;
        for c in self.connections.values().filter(|c| c.peer == *peer) {
            if let Path::ThroughMember { relay, .. } = c.path {
                return Some(limits::Vouch::Relay(relay, *peer));
            }
            let Some(ip) = addr::ip(&c.address) else {
                continue;
            };
            if self.wanted.is_known(ip) {
                vouch = Some(limits::Vouch::Address(ip));
            } else if vouch.is_none() && addr::is_local(&c.address) {
                vouch = Some(limits::Vouch::Lan(ip));
            }
        }
        vouch
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::GroupLog;
    use libp2p::{core::ConnectedPoint, swarm::ConnectionId};

    use super::*;
    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{
            ConnectionInfo, start,
            tests::{inspect, lone_member, through_relay},
        },
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn peers_that_prove_nothing_are_not_kept_for_speaking() {
        let (node, _group) = lone_member();
        let wanted = inspect(&node, |n| {
            // One that came in from the local network, or that a DHT node named: checked,
            // and heard while it answers, but not spared when connections run out.
            let neighbour = PeerId::random();
            n.on_connection_established(
                neighbour,
                ConnectionId::new_unchecked(7001),
                &ConnectedPoint::Listener {
                    local_addr: "/ip4/192.168.1.20/udp/4001/quic-v1".parse().unwrap(),
                    send_back_addr: "/ip4/192.168.1.66/udp/40000/quic-v1".parse().unwrap(),
                },
            );
            let after_connecting = n.wanted.is_wanted(&neighbour);
            n.spoke(neighbour);
            let after_speaking = n.wanted.is_wanted(&neighbour);
            let interesting = n.interesting.contains(&neighbour);
            (after_connecting, after_speaking, interesting)
        })
        .await;
        assert_eq!(wanted, (false, false, true));
        node.shutdown().await.unwrap();
    }

    /// A member on a network that blocks UDP is only ever connected over TCP, from an
    /// address that cannot be dialled back (so never remembered): the connection limits
    /// still keep places for that address once it has gone, for when it comes back.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_address_a_member_came_from_over_tcp_stays_known() {
        let (node, _group) = lone_member();
        let (known, stranger) = inspect(&node, |n| {
            // The device itself stands for a member here: it is one of its group.
            let device = n.identity.public_key();
            let info = ConnectionInfo {
                peer: PeerId::random(),
                path: Path::Direct,
                address: "/ip4/198.51.100.4/tcp/51234".parse().unwrap(),
            };
            n.note_reached(&info, device);
            (
                n.wanted.is_known([198, 51, 100, 4].into()),
                n.wanted.is_known([198, 51, 100, 5].into()),
            )
        })
        .await;
        assert!(known);
        assert!(!stranger);
        node.shutdown().await.unwrap();
    }

    /// Requests of strangers through one member's relay come from that relay's budget, as
    /// those behind one address come from the address's: a stranger making up a peer for
    /// each circuit asks no more than one address may.
    #[tokio::test(flavor = "multi_thread")]
    async fn strangers_through_one_relay_share_its_budget() {
        let (node, _group) = lone_member();
        let (answered, other) = inspect(&node, |n| {
            let relay = PeerId::random();
            let peers: Vec<PeerId> = (0..16).map(|_| PeerId::random()).collect();
            for (i, peer) in peers.iter().enumerate() {
                through_relay(n, *peer, relay, 8400 + i);
            }
            let answered = (0..1000)
                .filter(|i| !n.stranger_over_budget(&peers[i % peers.len()]))
                .count();
            let elsewhere = PeerId::random();
            through_relay(n, elsewhere, PeerId::random(), 8500);
            (answered, !n.stranger_over_budget(&elsewhere))
        })
        .await;
        assert!(
            (256..=260).contains(&answered),
            "strangers through one relay were answered {answered} times at once"
        );
        assert!(other, "a stranger through another relay was refused");
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn what_vouches_for_a_peer_asking_a_search_of_every_key() {
        use limits::Vouch;
        let (node, _group) = lone_member();
        let relay = PeerId::random();
        let (vouches, through) = inspect(&node, move |n| {
            let connect = |n: &mut Node, i: usize, local: &str, from: &str| {
                let peer = PeerId::random();
                n.on_connection_established(
                    peer,
                    ConnectionId::new_unchecked(8200 + i),
                    &ConnectedPoint::Listener {
                        local_addr: local.parse().unwrap(),
                        send_back_addr: from.parse().unwrap(),
                    },
                );
                peer
            };
            let own = "/ip4/203.0.113.50/udp/4001/quic-v1";
            let stranger = connect(n, 1, own, "/ip4/198.51.100.7/udp/4001/quic-v1");
            let neighbour = connect(n, 2, own, "/ip4/192.168.1.66/udp/4001/quic-v1");
            n.wanted
                .set_known([std::net::IpAddr::from([203, 0, 113, 9])]);
            let known = connect(n, 3, own, "/ip4/203.0.113.9/udp/4001/quic-v1");
            let through = connect(
                n,
                4,
                &format!("/ip4/203.0.113.9/udp/4001/quic-v1/p2p/{relay}/p2p-circuit"),
                &format!("/p2p/{}", PeerId::random()),
            );
            (
                [stranger, neighbour, known, through].map(|p| n.search_vouch(&p)),
                through,
            )
        })
        .await;
        assert_eq!(
            vouches,
            [
                None,
                Some(Vouch::Lan([192, 168, 1, 66].into())),
                Some(Vouch::Address([203, 0, 113, 9].into())),
                Some(Vouch::Relay(relay, through)),
            ]
        );
        node.shutdown().await.unwrap();
    }

    /// Strangers at one address get so many answers at once, then a few each second, of
    /// either protocol; a challenge past them gets a decoy, as one that names no group.
    /// Strangers at another address have their own.
    #[tokio::test(flavor = "multi_thread")]
    async fn strangers_at_one_address_get_so_many_answers() {
        use fogata_friends::FriendBook;
        use fogata_proto::{FriendRequest, FriendResponse, MemberRequest, MemberResponse};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let (node, _events) = start(NodeConfig {
            identity: ana.clone(),
            groups: vec![GroupSetup::Member(log)],
            friends: Some(FriendBook::new(&ana, "Ana", 1).unwrap()),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap();
        let (home, challenge, other) = inspect(&node, |n| {
            let mut next = 9300;
            let mut stranger = |n: &mut Node, from: &str| {
                let peer = PeerId::random();
                next += 1;
                n.on_connection_established(
                    peer,
                    ConnectionId::new_unchecked(next),
                    &ConnectedPoint::Listener {
                        local_addr: "/ip4/0.0.0.0/udp/4001/quic-v1".parse().unwrap(),
                        send_back_addr: format!("/ip4/{from}/udp/4001/quic-v1").parse().unwrap(),
                    },
                );
                peer
            };
            let probe = || FriendRequest::Probe { nonce: vec![1; 16] };
            let answered = |n: &mut Node, peer: PeerId, times: usize| {
                (0..times)
                    .filter(|_| {
                        matches!(
                            n.on_friend_request(peer, probe()),
                            FriendResponse::ProbeReply { .. }
                        )
                    })
                    .count()
            };
            // Two of them at one address share its budget (and what came back meanwhile).
            let (one, two) = (stranger(n, "203.0.113.9"), stranger(n, "203.0.113.9"));
            let at = Instant::now();
            let home = answered(n, one, 200) + answered(n, two, 200);
            let home = (home, at.elapsed());
            let tags = vec![serde_bytes::ByteBuf::from(vec![7u8; 32])];
            let challenge = n.on_request(
                one,
                MemberRequest::Challenge {
                    challenge: vec![1; 32],
                    tags,
                },
            );
            let elsewhere = stranger(n, "198.51.100.9");
            (home, challenge, answered(n, elsewhere, 10))
        })
        .await;
        let (home, took) = home;
        let came_back = (took.as_secs_f64() * 32.0).ceil() as usize;
        assert!(
            (256..=256 + came_back).contains(&home),
            "{home} in {took:?}"
        );
        assert!(matches!(challenge, MemberResponse::Proof(_)));
        assert_eq!(other, 10);
        node.shutdown().await.unwrap();
    }
}
