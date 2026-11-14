//! Friend requests between members of a group, made from a member's card instead of with a
//! link or a code.
//!
//! The request goes over the member protocol, on a connection where the receiver proved to
//! be the member asked: who it is is known already, so nothing has to be found on the DHT
//! and no secret has to travel apart. It carries what a link's request carries (the
//! requester's profile and request secret, signed and sealed to the receiver), so it becomes
//! an ordinary incoming request there, answered as any other: the acceptance comes back over
//! the friend protocol keyed by the secret the two share, on this connection or, later, by
//! looking each other up under it (see friends.rs).
//!
//! The receiver takes it only from a peer that proved to be a member of a group other than
//! a direct chat's, and only when the profile inside is that member's device. A member's
//! request is shown once in a while at most ([`MEMBER_ASK_EVERY`]), so one that was declined
//! cannot be asked again and again; the requester is not told it was let go, as with a link
//! nobody answers. Older devices cannot read the request, so it goes only to those that
//! advertise [`MEMBER_FRIEND_MARKER`].

use std::time::{Duration, Instant};

use fogata_crypto::DevicePublicKey;
use fogata_proto::{MEMBER_FRIEND_MARKER, MemberRequest, MemberResponse};
use libp2p::{PeerId, StreamProtocol};

use super::{Node, Pending};
use crate::{Command, NodeEvent};

/// See [`MEMBER_FRIEND_MARKER`].
pub(super) const MEMBER_FRIEND_PROTOCOL: StreamProtocol = StreamProtocol::new(MEMBER_FRIEND_MARKER);

/// A member's friend request is shown at most this often: a request that came within it
/// (one declined a moment ago, say) is acknowledged and let go.
pub(super) const MEMBER_ASK_EVERY: Duration = Duration::from_secs(60 * 60);

impl Node {
    /// Whether `peer` takes [`MemberRequest::FriendAsk`] (it said so when it connected).
    fn takes_member_friend(&self, peer: &PeerId) -> bool {
        crate::locked(&self.advertised)
            .get(peer)
            .is_some_and(|set| set.contains(&MEMBER_FRIEND_PROTOCOL))
    }

    fn member_ask_failed(&self, reason: &str) {
        self.emit(NodeEvent::FriendRequestFailed(reason.to_string()));
    }

    /// Sends a friend request to member `device`, listed as `name` until it is answered.
    pub(super) fn ask_member(&mut self, device: DevicePublicKey, name: String, now: u64) {
        let Some(f) = self.friends.as_ref() else {
            return;
        };
        if device == f.book.profile.device {
            return self.member_ask_failed("you cannot be friends with yourself");
        }
        if f.book.friends.contains_key(&device) {
            return self.member_ask_failed("you are already friends");
        }
        if f.book.incoming.contains_key(&device) {
            // That member asked first: asking back is saying yes.
            return self.on_friend_command(Command::AnswerFriend {
                device,
                accept: true,
            });
        }
        let peers: Vec<PeerId> = self
            .members
            .iter()
            .filter(|(peer, m)| m.device == device && self.in_regular_group(peer))
            .map(|(peer, _)| *peer)
            .collect();
        if peers.is_empty() {
            return self.member_ask_failed(
                "this member is not connected now; try again when they are online",
            );
        }
        let Some(peer) = peers.into_iter().find(|p| self.takes_member_friend(p)) else {
            return self.member_ask_failed(
                "this member's copy of Fogata is too old to take friend requests from a group; \
                 send them a friend link instead",
            );
        };
        let local = self.local_peer().to_bytes();
        let f = self.friends.as_mut().expect("checked above");
        let fresh = !f.book.outgoing.contains_key(&device);
        let secret = match f.book.request_sent(device, &name, now) {
            Ok(secret) => secret,
            Err(e) => return self.member_ask_failed(&e.to_string()),
        };
        let sealed = match fogata_friends::member_request(
            &self.identity,
            &f.book.profile,
            &secret,
            &device,
            &local,
            &peer.to_bytes(),
        ) {
            Ok(sealed) => sealed,
            Err(e) => {
                if fresh {
                    f.book.outgoing.remove(&device);
                }
                return self.member_ask_failed(&e.to_string());
            }
        };
        // Announced under the secret the two share from the next slot, and looked up under
        // it: an acceptance made while the two are apart still finds its way.
        f.recent = Some(Instant::now());
        f.last_lookup = None;
        self.provided.1 = None;
        self.changed();
        self.request(
            peer,
            MemberRequest::FriendAsk { sealed },
            Pending::FriendAsk(device, fresh),
        );
    }

    /// The answer to a friend request sent to member `device`: `taken` when it waits there
    /// for the person to answer. One that did not get there is forgotten here, unless an
    /// earlier request to that device (made with a link, say) still waits.
    pub(super) fn member_ask_answered(
        &mut self,
        device: DevicePublicKey,
        fresh: bool,
        taken: bool,
    ) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        if taken {
            if let Some(outgoing) = f.book.outgoing.get(&device) {
                let name = outgoing.name.clone();
                self.emit(NodeEvent::FriendRequestSent { device, name });
            }
            return;
        }
        if fresh && f.book.outgoing.remove(&device).is_some() {
            self.changed();
        }
        self.member_ask_failed("the friend request did not reach the member; try again");
    }

    /// A friend request from member `peer`, sealed to this device.
    pub(super) fn on_member_friend_ask(&mut self, peer: PeerId, sealed: &[u8]) -> MemberResponse {
        // Only from a peer that proved to be a member of a group shared with it; a direct
        // chat's group is left out, as its other person may be a former friend.
        let Some(device) = self.members.get(&peer).map(|m| m.device) else {
            return MemberResponse::Refused;
        };
        if !self.in_regular_group(&peer) {
            return MemberResponse::Refused;
        }
        let local = self.local_peer().to_bytes();
        let Some(f) = self.friends.as_mut() else {
            return MemberResponse::Refused;
        };
        let now = Instant::now();
        f.member_asks
            .retain(|_, at| now.duration_since(*at) < MEMBER_ASK_EVERY);
        // Already waiting for an answer here, or shown a moment ago: nothing new to show.
        if f.book.incoming.contains_key(&device) || f.member_asks.contains_key(&device) {
            return MemberResponse::Ack;
        }
        let intro = match fogata_friends::open_member_request(
            &self.identity,
            &local,
            &peer.to_bytes(),
            sealed,
        ) {
            Ok(intro) if intro.profile.device == device => intro,
            Ok(_) => {
                tracing::debug!("{peer} sent a friend request in another device's name");
                return MemberResponse::Refused;
            }
            Err(e) => {
                tracing::debug!("refused a member's friend request from {peer}: {e}");
                return MemberResponse::Refused;
            }
        };
        f.member_asks.insert(device, now);
        self.received_request(peer, intro);
        MemberResponse::Ack
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_friends::FriendBook;
    use fogata_proto::MemberResponse;
    use fogata_state::GroupLog;
    use libp2p::PeerId;

    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{
            Node, start,
            tests::{add_member, inspect},
        },
    };

    /// A request sealed to this device is taken only from the peer that proved to be the
    /// member it names, and a member declined a moment ago is not shown again.
    #[tokio::test(flavor = "multi_thread")]
    async fn only_a_member_s_own_request_is_taken_and_not_over_and_over() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let group = log.group_id();
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
        let outcome = inspect(&node, move |n| {
            let bea = DeviceIdentity::generate().unwrap();
            let book = FriendBook::new(&bea, "Bea", 1).unwrap();
            let (bea_peer, ana_peer) = (PeerId::random(), n.local_peer());
            let sealed = fogata_friends::member_request(
                &bea,
                &book.profile,
                &book.secret,
                &ana.public_key(),
                &bea_peer.to_bytes(),
                &ana_peer.to_bytes(),
            )
            .unwrap();
            let bea_key = bea.public_key();
            let shown = |n: &Node| {
                n.friends
                    .as_ref()
                    .unwrap()
                    .book
                    .incoming
                    .contains_key(&bea_key)
            };
            let taken = |n: &mut Node| {
                matches!(
                    n.on_member_friend_ask(bea_peer, &sealed),
                    MemberResponse::Ack
                )
            };
            // A peer that proved nothing.
            if taken(n) || shown(n) {
                return "taken from a peer that is no member";
            }
            // A member that sends another device's request.
            add_member(n, group, bea_peer);
            if taken(n) || shown(n) {
                return "taken in another member's name";
            }
            // The member that wrote it.
            n.members.get_mut(&bea_peer).unwrap().device = bea_key;
            if !taken(n) || !shown(n) {
                return "the member's own request was not taken";
            }
            // Declined, then asked again at once: acknowledged, not shown.
            let f = n.friends.as_mut().unwrap();
            f.book.answer(&bea_key, false, 2);
            if !taken(n) || shown(n) {
                return "a declined request was shown again at once";
            }
            "ok"
        })
        .await;
        assert_eq!(outcome, "ok");
        node.shutdown().await.unwrap();
    }
}
