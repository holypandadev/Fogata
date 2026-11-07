//! What a peer says before it proves membership (a profile, a presence): kept until it
//! does, a few places for each address and each member's relay, and let go when it does
//! not.

use fogata_proto::MemberResponse;
use libp2p::PeerId;

use super::Node;

/// Most peers whose profile and presence are kept until they prove themselves, and most
/// bytes of profiles and animations kept for them in all.
const MAX_EARLY: usize = 64;
const MAX_EARLY_BYTES: usize = 4 * 1024 * 1024;
/// ... and from peers at one address: a neighbour with many made-up identities, each asked
/// to prove itself, would otherwise take every place, presences weighing no bytes. Peers
/// through one member's relay get a few more together, as several members of a group may
/// come through it at once (see `Node::peer_source`).
const MAX_EARLY_PER_ADDRESS: usize = 4;
const MAX_EARLY_PER_RELAY: usize = 8;
/// Largest profile and animation kept from a peer that has not proved itself yet: a signed
/// profile with the largest picture, and an animation of the largest size, with room for
/// what goes around them.
const MAX_EARLY_PROFILE: usize = fogata_friends::MAX_PHOTO_BYTES + 1024;
const MAX_EARLY_ANIMATION: usize = fogata_friends::MAX_PHOTO_ANIMATION_BYTES + 1024;

/// What a peer said before it proved itself ([`Node::keep_early`]).
#[derive(Default)]
pub(super) struct Early {
    pub(super) profile: Option<(Vec<u8>, Option<serde_bytes::ByteBuf>)>,
    pub(super) presence: Option<(bool, bool)>,
}

impl Early {
    pub(super) fn bytes(&self) -> usize {
        self.profile.as_ref().map_or(0, |(profile, animation)| {
            profile.len() + animation.as_ref().map_or(0, |a| a.len())
        })
    }
}

/// Whether a profile from a peer that has not proved itself yet may wait for its proof,
/// `held` bytes being kept for others already: it must decode, be signed, and fit. Its
/// animation is kept only if it is the same device's picture's; a bad one leaves the
/// still picture, as from a member.
pub(super) fn early_profile(
    profile: Vec<u8>,
    animation: Option<serde_bytes::ByteBuf>,
    held: usize,
) -> Option<(Vec<u8>, Option<serde_bytes::ByteBuf>)> {
    if profile.len() > MAX_EARLY_PROFILE {
        return None;
    }
    let device = fogata_friends::Profile::decode(&profile).ok()?.device;
    let animation = animation.filter(|a| {
        a.len() <= MAX_EARLY_ANIMATION
            && fogata_friends::Animation::decode(a)
                .is_ok_and(|a| a.device == device && a.of == fogata_friends::Animated::Photo)
    });
    let size = profile.len() + animation.as_ref().map_or(0, |a| a.len());
    (held + size <= MAX_EARLY_BYTES).then_some((profile, animation))
}

impl Node {
    /// What `peer` said before it proved itself goes once no challenge to it is pending:
    /// it proved itself (and what it said counted), or it did not.
    pub(super) fn drop_early_unless_asked(&mut self, peer: PeerId) {
        if !self
            .groups
            .values()
            .any(|s| s.challenges.contains_key(&peer))
        {
            self.early.remove(&peer);
        }
    }

    /// Keeps a profile or presence from a peer this device is still checking: both ends
    /// verify each other at once, and the other may finish first and send them before this
    /// device has its proof. Only from a peer asked for one, and from a bounded number.
    pub(super) fn keep_early(
        &mut self,
        peer: PeerId,
        keep: impl FnOnce(&mut Early),
    ) -> MemberResponse {
        let asked = self
            .groups
            .values()
            .any(|s| s.challenges.contains_key(&peer));
        if !asked {
            return MemberResponse::Refused;
        }
        if !self.early.contains_key(&peer) {
            let source = self.peer_source(&peer);
            let same = self
                .early
                .keys()
                .filter(|p| source.is_some() && self.peer_source(p) == source)
                .count();
            let most = match source {
                Some(super::limits::Source::Relay(_) | super::limits::Source::SomeRelay) => {
                    MAX_EARLY_PER_RELAY
                }
                _ => MAX_EARLY_PER_ADDRESS,
            };
            if self.early.len() >= MAX_EARLY || same >= most {
                return MemberResponse::Refused;
            }
        }
        keep(self.early.entry(peer).or_default());
        MemberResponse::Ack
    }
}

#[cfg(test)]
mod tests {
    use libp2p::{Multiaddr, PeerId, core::ConnectedPoint, swarm::ConnectionId};

    use super::*;
    use crate::node::{
        Node, Pending,
        tests::{inspect, lone_member, through_relay},
    };

    #[test]
    fn profiles_kept_before_a_proof_must_decode_and_fit() {
        use fogata_friends::{Animated, Animation, FriendBook};
        let bea = fogata_crypto::DeviceIdentity::generate().unwrap();
        let leo = fogata_crypto::DeviceIdentity::generate().unwrap();
        let profile = FriendBook::new(&bea, "Bea", 7).unwrap().profile.encode();
        let animation = |who| {
            let a = Animation::sign(who, Animated::Photo, b"still", Vec::new(), 7).unwrap();
            Some(serde_bytes::ByteBuf::from(a.encode()))
        };
        // What is no profile, or too large to be one, is not kept.
        assert!(early_profile(vec![1; 1000], None, 0).is_none());
        assert!(early_profile(vec![1; 4 << 20], None, 0).is_none());
        // A real one is, with its own device's animation; another's is left out.
        let (kept, own) = early_profile(profile.clone(), animation(&bea), 0).unwrap();
        assert_eq!(kept, profile);
        assert!(own.is_some());
        let (_, other) = early_profile(profile.clone(), animation(&leo), 0).unwrap();
        assert!(other.is_none());
        // Within a bound of bytes for every peer together.
        let room = MAX_EARLY_BYTES - profile.len();
        assert!(early_profile(profile.clone(), None, room).is_some());
        assert!(early_profile(profile.clone(), None, room + 1).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn one_neighbour_gets_only_a_few_places_before_proving_itself() {
        let (node, _group) = lone_member();
        let (from_neighbour, from_another) = inspect(&node, |n| {
            let come_in = |n: &mut Node, i: usize, from: &str| {
                let peer = PeerId::random();
                n.on_connection_established(
                    peer,
                    ConnectionId::new_unchecked(8000 + i),
                    &ConnectedPoint::Listener {
                        local_addr: "/ip4/192.168.1.20/udp/4001/quic-v1".parse().unwrap(),
                        send_back_addr: from.parse().unwrap(),
                    },
                );
                // Asked to prove itself, it says it is away before doing so.
                let kept = n.keep_early(peer, |e| e.presence = Some((false, true)));
                matches!(kept, fogata_proto::MemberResponse::Ack)
            };
            let from_neighbour = (0..100)
                .filter(|i| {
                    come_in(
                        n,
                        *i,
                        &format!("/ip4/192.168.1.66/udp/{}/quic-v1", 40000 + i),
                    )
                })
                .count();
            let from_another = come_in(n, 500, "/ip4/192.168.1.30/udp/40000/quic-v1");
            (from_neighbour, from_another)
        })
        .await;
        assert_eq!(from_neighbour, 4);
        assert!(from_another);
        node.shutdown().await.unwrap();
    }

    /// Members coming in through one member's relay are not counted as one address (the
    /// relay's) when they say something before proving themselves.
    #[tokio::test(flavor = "multi_thread")]
    async fn peers_through_one_relay_do_not_share_the_relays_places() {
        let (node, _group) = lone_member();
        let taken = inspect(&node, |n| {
            let relay = PeerId::random();
            (0..8)
                .filter(|i| {
                    let peer = PeerId::random();
                    let address: Multiaddr = format!(
                        "/ip4/203.0.113.9/udp/4001/quic-v1/p2p/{relay}/p2p-circuit/p2p/{peer}"
                    )
                    .parse()
                    .unwrap();
                    n.on_connection_established(
                        peer,
                        ConnectionId::new_unchecked(8100 + i),
                        &ConnectedPoint::Dialer {
                            address,
                            role_override: libp2p::core::Endpoint::Dialer,
                            port_use: libp2p::core::transport::PortUse::Reuse,
                        },
                    );
                    let kept = n.keep_early(peer, |e| e.presence = Some((false, true)));
                    matches!(kept, fogata_proto::MemberResponse::Ack)
                })
                .count()
        })
        .await;
        assert_eq!(taken, 8);
        node.shutdown().await.unwrap();
    }

    /// A stranger coming through a member's relay slot with peer after peer, each asked to
    /// prove itself, takes only the relay's few places among those kept until then; peers
    /// through another member's relay still get theirs.
    #[tokio::test(flavor = "multi_thread")]
    async fn peers_through_one_relay_take_only_its_places_until_they_prove_themselves() {
        let (node, _group) = lone_member();
        let (one, other) = inspect(&node, move |n| {
            let kept = |n: &mut Node, relay: PeerId, from: usize, count: usize| {
                (from..from + count)
                    .filter(|i| {
                        let peer = PeerId::random();
                        through_relay(n, peer, relay, *i);
                        let kept = n.keep_early(peer, |e| e.presence = Some((false, true)));
                        matches!(kept, fogata_proto::MemberResponse::Ack)
                    })
                    .count()
            };
            let one = kept(n, PeerId::random(), 8200, 32);
            let other = kept(n, PeerId::random(), 8300, 2);
            (one, other)
        })
        .await;
        assert_eq!(one, MAX_EARLY_PER_RELAY);
        assert_eq!(other, 2);
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn what_a_peer_said_before_proving_itself_goes_when_it_does_not() {
        let (node, group) = lone_member();
        let left = inspect(&node, move |n| {
            let mut left = Vec::new();
            for (i, ending) in ["refused", "closed"].iter().enumerate() {
                let peer = PeerId::random();
                let connection = ConnectionId::new_unchecked(9500 + i);
                n.on_connection_established(
                    peer,
                    connection,
                    &ConnectedPoint::Listener {
                        local_addr: "/ip4/192.168.1.20/udp/4001/quic-v1".parse().unwrap(),
                        send_back_addr: format!("/ip4/192.168.1.{}/udp/4001/quic-v1", 30 + i)
                            .parse()
                            .unwrap(),
                    },
                );
                n.keep_early(peer, |e| e.presence = Some((false, true)));
                assert!(n.early.contains_key(&peer));
                match *ending {
                    // Its proof does not come (refused, or the request failed or timed out).
                    "refused" => {
                        let challenge = n.groups[&group].challenges[&peer];
                        n.on_response(
                            peer,
                            Pending::Challenge(group, challenge),
                            fogata_proto::MemberResponse::Refused,
                        );
                    }
                    // It went before proving anything.
                    _ => n.on_connection_closed(peer, connection, 0),
                }
                left.push(n.early.contains_key(&peer));
            }
            left
        })
        .await;
        assert_eq!(left, vec![false, false]);
        node.shutdown().await.unwrap();
    }
}
