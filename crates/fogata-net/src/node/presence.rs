//! This device's profile and presence sent to members: the presence one at a time to each,
//! so an older one never arrives after a newer, and again when one was not taken.

use std::collections::{HashMap, HashSet};

use fogata_proto::MemberRequest;
use libp2p::PeerId;

use super::{Node, Pending};

/// This device's presence on its way to each member ([`Node::send_presence`]).
#[derive(Default)]
pub(super) struct PresenceWays {
    /// Members one is on its way to, one at a time to each: the number it went under, and
    /// whether a newer one waits for it.
    pub(super) on_its_way: HashMap<PeerId, (u64, bool)>,
    /// Members the last one did not get to, sent it again at the next heartbeat.
    pub(super) lost: HashSet<PeerId>,
    /// The number the next one goes under: the answer to one sent before a member went and
    /// came back is not taken for that of the one on its way now.
    next: u64,
}

impl PresenceWays {
    #[cfg(test)]
    pub(super) fn is_on_its_way(&self, peer: &PeerId) -> bool {
        self.on_its_way.contains_key(peer)
    }
}

impl Node {
    /// Sends this device's profile to every member connected now: a changed name or picture
    /// shows on the other devices without waiting for them to reconnect.
    pub(super) fn send_profile_to_members(&mut self) {
        let Some(profile) = self.own_profile().map(|p| p.encode()) else {
            return;
        };
        let animation = self.own_photo_animation().map(serde_bytes::ByteBuf::from);
        let peers: Vec<PeerId> = self.members.keys().copied().collect();
        for peer in peers {
            let request = MemberRequest::Profile {
                profile: profile.clone(),
                animation: animation.clone(),
            };
            self.request(peer, request, Pending::Profile);
        }
    }

    /// Tells every member connected now whether this device appears offline, or is away.
    pub(super) fn send_presence_to_members(&mut self) {
        let peers: Vec<PeerId> = self.members.keys().copied().collect();
        for peer in peers {
            self.send_presence(peer);
        }
    }

    /// Tells member `peer` whether this device appears offline or away. One at a time: two
    /// on their way at once may arrive in either order, and the member would keep the older,
    /// say showing online a device that appears offline. A change meanwhile goes, as it is
    /// then, once the one on its way is answered.
    pub(super) fn send_presence(&mut self, peer: PeerId) {
        if let Some((_, newer)) = self.presence.on_its_way.get_mut(&peer) {
            *newer = true;
            return;
        }
        let sent = self.presence.next;
        self.presence.next += 1;
        self.presence.on_its_way.insert(peer, (sent, false));
        self.presence.lost.remove(&peer);
        let request = MemberRequest::Presence {
            hidden: self.hidden,
            away: self.shown_away(),
        };
        self.request(peer, request, Pending::Presence(sent));
    }

    /// The presence `sent` to `peer` was answered, `taken` or not, or did not get there, or
    /// was dropped before it went: a newer one goes now, and one not taken goes again at the
    /// next heartbeat while `peer` is still a member (not at once, so that a member refusing
    /// it cannot keep this device sending). One sent before `peer` went counts no more.
    pub(super) fn presence_done(&mut self, peer: PeerId, sent: u64, taken: bool) {
        if self.presence.on_its_way.get(&peer).map(|(n, _)| *n) != Some(sent) {
            return;
        }
        let newer = self
            .presence
            .on_its_way
            .remove(&peer)
            .is_some_and(|(_, newer)| newer);
        if !taken {
            if self.members.contains_key(&peer) {
                self.presence.lost.insert(peer);
            }
        } else if newer {
            self.send_presence(peer);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::GroupLog;
    use libp2p::{
        PeerId,
        request_response::{self, OutboundRequestId},
        swarm::ConnectionId,
    };

    use crate::{
        NodeConfig, NodeEvent, Path,
        group::GroupSetup,
        node::{
            Member, Node, Pending,
            outbox::IN_FLIGHT,
            start,
            tests::{add_member, empty_push, inspect, lone_member},
        },
    };

    /// This device's presence goes to a member one at a time, so that an older one cannot
    /// arrive after a newer and leave the member showing online a device that appears
    /// offline: a change while one is on its way goes once that one is answered. One the
    /// member did not take goes again at the next heartbeat, not at once.
    #[tokio::test(flavor = "multi_thread")]
    async fn presence_goes_to_a_member_one_at_a_time_and_again_when_not_taken() {
        use fogata_proto::MemberResponse;
        let (node, _group) = lone_member();
        let found = inspect(&node, |n| {
            let bea = PeerId::random();
            n.members.insert(
                bea,
                Member {
                    device: DeviceIdentity::generate().unwrap().public_key(),
                    path: Path::Direct,
                    address: "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap(),
                    told: Path::Direct,
                    worse_since: None,
                },
            );
            let on_its_way = |n: &Node| -> Vec<OutboundRequestId> {
                n.pending
                    .iter()
                    .filter(|(_, (p, kind))| *p == bea && matches!(kind, Pending::Presence(_)))
                    .map(|(id, _)| *id)
                    .collect()
            };
            let answer = |n: &mut Node, response: MemberResponse| {
                let id = on_its_way(n)[0];
                let (to, kind) = n.pending.remove(&id).unwrap();
                n.on_response(to, kind, response);
            };
            let mut found = Vec::new();
            n.send_presence(bea);
            n.hidden = true;
            n.send_presence(bea);
            found.push(on_its_way(n).len());
            answer(n, MemberResponse::Ack);
            found.push(on_its_way(n).len());
            answer(n, MemberResponse::Refused);
            found.push(on_its_way(n).len());
            n.member_heartbeat();
            found.push(on_its_way(n).len());
            answer(n, MemberResponse::Ack);
            n.member_heartbeat();
            found.push(on_its_way(n).len());
            found
        })
        .await;
        assert_eq!(found, [1, 1, 0, 1, 0]);
        node.shutdown().await.unwrap();
    }

    /// This device's presence goes again to a member whose last connection closed once it
    /// is back, whether one was on its way to it then or waiting its turn. The failure of
    /// one that was on its way, which comes after, does not count for the one going now:
    /// two would then be on their way at once.
    #[tokio::test(flavor = "multi_thread")]
    async fn presence_goes_again_to_a_member_that_went_and_came_back() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, leo) = (PeerId::random(), PeerId::random());
            // Presences on their way to `peer`, and waiting their turn.
            let presences = |n: &Node, peer: PeerId| {
                let on_its_way = n
                    .pending
                    .values()
                    .filter(|(p, kind)| *p == peer && matches!(kind, Pending::Presence(_)))
                    .count();
                let waiting = n.outbox.get(&peer).map_or(0, |o| {
                    o.waiting
                        .iter()
                        .filter(|(_, kind)| matches!(kind, Pending::Presence(_)))
                        .count()
                });
                (on_its_way, waiting)
            };
            let both = |n: &Node| (presences(n, bea), presences(n, leo));
            let mut found = Vec::new();
            add_member(n, group, bea);
            add_member(n, group, leo);
            // To Bea it goes at once, before other requests; to Leo it waits behind them.
            n.send_presence(bea);
            for _ in 1..IN_FLIGHT {
                n.request(bea, empty_push(group), Pending::Push);
            }
            for _ in 0..IN_FLIGHT {
                n.request(leo, empty_push(group), Pending::Push);
            }
            n.send_presence(leo);
            found.push(both(n));
            // Both go, and are back before the failures of what was on its way come.
            n.on_connection_closed(bea, ConnectionId::new_unchecked(1), 0);
            n.on_connection_closed(leo, ConnectionId::new_unchecked(2), 0);
            add_member(n, group, bea);
            add_member(n, group, leo);
            n.hidden = true;
            n.send_presence(bea);
            n.send_presence(leo);
            found.push(both(n));
            let old: Vec<(OutboundRequestId, PeerId)> =
                n.pending.iter().map(|(id, (p, _))| (*id, *p)).collect();
            for (request_id, peer) in old {
                n.on_member_event(request_response::Event::OutboundFailure {
                    peer,
                    connection_id: ConnectionId::new_unchecked(1),
                    request_id,
                    error: request_response::OutboundFailure::ConnectionClosed,
                });
            }
            found.push(both(n));
            // A change now waits for the one on its way.
            n.hidden = false;
            n.send_presence(bea);
            n.send_presence(leo);
            found.push(both(n));
            found
        })
        .await;
        assert_eq!(
            found,
            [
                ((1, 0), (0, 1)),
                // Bea's old one is still counted on its way until its failure comes.
                ((1, 1), (0, 1)),
                ((1, 0), (1, 0)),
                ((1, 0), (1, 0)),
            ]
        );
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_saying_its_presence_over_and_over_loses_no_other_event() {
        use fogata_proto::MemberRequest;
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let group = log.group_id();
        let (node, mut events) = start(NodeConfig {
            identity: ana,
            groups: vec![GroupSetup::Member(log)],
            friends: None,
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
        // While the application is busy (reading nothing), a member says it is away and
        // back again twenty thousand times, a few hundred kilobytes; then the group changes.
        inspect(&node, move |n| {
            let bea = PeerId::random();
            n.members.insert(
                bea,
                Member {
                    device: DeviceIdentity::generate().unwrap().public_key(),
                    path: Path::Direct,
                    address: "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap(),
                    told: Path::Direct,
                    worse_since: None,
                },
            );
            for i in 0..20_000 {
                let away = i % 2 == 0;
                n.on_request(
                    bea,
                    MemberRequest::Presence {
                        hidden: false,
                        away,
                    },
                );
            }
            n.emit(NodeEvent::GroupChanged(group));
        })
        .await;
        let mut changed = false;
        let mut last_presence = None;
        while let Ok(event) = events.try_recv() {
            match event {
                NodeEvent::GroupChanged(g) if g == group => changed = true,
                NodeEvent::MemberPresence { away, .. } => last_presence = Some(away),
                _ => {}
            }
        }
        assert!(changed, "the group change was dropped");
        assert_eq!(
            last_presence,
            Some(false),
            "the last presence said was lost"
        );
        node.shutdown().await.unwrap();
    }
}
