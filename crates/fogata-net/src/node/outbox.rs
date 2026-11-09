//! Member requests on their way: a few at a time to each peer, the rest waiting their turn,
//! a direct chat's messages one at a time, and what waits for an answer let go when a
//! request is answered, lost or dropped unsent.

use fogata_proto::MemberRequest;
use libp2p::PeerId;

use super::{Node, Pending};

/// Member requests on their way to one peer at once. A connection takes 16 streams of the
/// member protocol at once, both ways together, and drops any stream past that with what it
/// carried: a burst of messages (pasted lines, many files shared at once) lost most of them.
pub(super) const IN_FLIGHT: usize = 6;

/// Most member requests waiting for one peer; past it the oldest goes.
pub(super) const MAX_WAITING: usize = 1024;

impl Node {
    /// Sends a member request, or keeps it until fewer are on their way to that peer (see
    /// [`IN_FLIGHT`]).
    pub(super) fn request(&mut self, peer: PeerId, request: MemberRequest, kind: Pending) {
        let outbox = self.outbox.entry(peer).or_default();
        if outbox.in_flight >= IN_FLIGHT {
            let dropped = if outbox.waiting.len() >= MAX_WAITING {
                outbox.waiting.pop_front()
            } else {
                None
            };
            outbox.waiting.push_back((request, kind));
            if let Some((_, kind)) = dropped {
                self.request_dropped(peer, kind);
            }
            return;
        }
        outbox.in_flight += 1;
        let id = self
            .swarm
            .behaviour_mut()
            .member
            .send_request(&peer, request);
        self.pending.insert(id, (peer, kind));
    }

    /// A member request to `peer` that will not go: the oldest of a full queue, or one
    /// waiting when `peer` went. What waits for its answer is let go as when one fails on
    /// its way, but nothing is sent again now: it would only wait in the same queue, or
    /// dial a peer that just went. Each is asked or sent again later as after a refusal.
    pub(super) fn request_dropped(&mut self, peer: PeerId, kind: Pending) {
        match kind {
            Pending::Challenge(group, challenge) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.forget_challenge(&peer, &challenge);
                }
                self.drop_early_unless_asked(peer);
            }
            Pending::OtherSession(device, check) => {
                self.on_other_session_answer(peer, device, check, false)
            }
            // The device gets the log, or hears of the refusal, when it shows its invite
            // again.
            Pending::AdmittedPart(group) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.admitting.remove(&peer);
                }
            }
            Pending::Refusal(group) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.refusing.remove(&peer);
                }
            }
            Pending::Presence(sent) => self.presence_done(peer, sent, false),
            Pending::FriendAsk(device, fresh) => self.member_ask_answered(device, fresh, false),
            Pending::Join(group) | Pending::JoinCommit(group) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.join_sent.remove(&peer);
                }
            }
            Pending::RoomSync(group, _) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.room_asked.remove(&peer);
                }
            }
            // Those after it wait for the application to send again, in its order,
            // everything not confirmed, as when one fails on its way.
            Pending::Direct => {
                if let Some(outbox) = self.outbox.get_mut(&peer) {
                    outbox.direct.clear();
                }
            }
            _ => {}
        }
    }

    /// `peer`'s last connection closed: the member requests waiting for it are dropped,
    /// and this device's presence is sent anew when it is a member again.
    pub(super) fn requests_peer_gone(&mut self, peer: PeerId) {
        let waiting = self
            .outbox
            .get_mut(&peer)
            .map(|o| std::mem::take(&mut o.waiting))
            .unwrap_or_default();
        for (_, kind) in waiting {
            self.request_dropped(peer, kind);
        }
        if let Some(outbox) = self.outbox.get_mut(&peer) {
            outbox.direct.clear();
        }
        self.presence.on_its_way.remove(&peer);
        self.presence.lost.remove(&peer);
    }

    /// Sends a direct chat's messages to `peer` once those sent before them got there (see
    /// `Outbox::direct`).
    pub(super) fn request_direct(&mut self, peer: PeerId, request: MemberRequest) {
        let busy = self.direct_on_its_way(peer);
        let outbox = self.outbox.entry(peer).or_default();
        if busy || !outbox.direct.is_empty() {
            // Past the limit they are dropped, the newest first, so what does arrive keeps
            // its order; the application sends again what was not confirmed.
            if outbox.direct.len() < MAX_WAITING {
                outbox.direct.push_back(request);
            }
            return;
        }
        self.request(peer, request, Pending::Direct);
    }

    /// Whether a direct chat's request to `peer` is on its way or waiting its turn.
    fn direct_on_its_way(&self, peer: PeerId) -> bool {
        self.pending
            .values()
            .any(|(p, kind)| *p == peer && matches!(kind, Pending::Direct))
            || self.outbox.get(&peer).is_some_and(|o| {
                o.waiting
                    .iter()
                    .any(|(_, kind)| matches!(kind, Pending::Direct))
            })
    }

    /// A member request to `peer` got its answer or failed: the next one waiting goes, and
    /// the next of a direct chat's once none of its own is on its way.
    pub(super) fn request_done(&mut self, peer: PeerId) {
        let Some(outbox) = self.outbox.get_mut(&peer) else {
            return;
        };
        outbox.in_flight = outbox.in_flight.saturating_sub(1);
        if let Some((request, kind)) = outbox.waiting.pop_front() {
            self.request(peer, request, kind);
        }
        if !self.direct_on_its_way(peer)
            && let Some(request) = self
                .outbox
                .get_mut(&peer)
                .and_then(|o| o.direct.pop_front())
        {
            self.request(peer, request, Pending::Direct);
        }
        let idle = self
            .outbox
            .get(&peer)
            .is_some_and(|o| o.in_flight == 0 && o.waiting.is_empty() && o.direct.is_empty());
        if idle {
            self.outbox.remove(&peer);
            if self.farewell.remove(&peer) {
                self.let_go(peer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Instant};

    use libp2p::PeerId;

    use super::*;
    use crate::node::{
        Admitting, Pending,
        tests::{add_member, empty_push, inspect, lone_member},
    };

    /// A member request dropped before it went, the oldest of a full queue to a member
    /// that answers nothing, lets go what waits for its answer as one lost on the way does:
    /// the presence goes again at the next heartbeat, the invite is shown and the room
    /// asked for again, the challenge sent again, a joining device's log and refusal are
    /// no longer kept, and the direct chat's messages after it do not go before it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_request_dropped_unsent_lets_go_what_waits_for_its_answer() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = PeerId::random();
            add_member(n, group, bea);
            for _ in 0..IN_FLIGHT {
                n.request(bea, empty_push(group), Pending::Push);
            }
            n.send_presence(bea);
            n.request_direct(bea, empty_push(group));
            n.request_direct(bea, empty_push(group));
            let challenge = [7; 32];
            let slot = n.groups.get_mut(&group).unwrap();
            slot.join_sent.insert(bea);
            slot.room_asked.insert(bea, Instant::now());
            slot.refusing.insert(bea, (None, 0));
            slot.admitting.insert(
                bea,
                Admitting {
                    parts: VecDeque::from([Vec::new()]),
                    next: 0,
                    count: 1,
                    tries: 0,
                },
            );
            slot.challenges.insert(bea, challenge);
            for kind in [
                Pending::Join(group),
                Pending::RoomSync(group, 0),
                Pending::Refusal(group),
                Pending::AdmittedPart(group),
                Pending::Challenge(group, challenge),
            ] {
                n.request(bea, empty_push(group), kind);
            }
            for _ in 0..MAX_WAITING {
                n.request(bea, empty_push(group), Pending::Push);
            }
            let slot = &n.groups[&group];
            let kept = [
                n.presence.is_on_its_way(&bea),
                !n.outbox[&bea].direct.is_empty(),
                slot.join_sent.contains(&bea),
                slot.room_asked.contains_key(&bea),
                slot.refusing.contains_key(&bea),
                slot.admitting.contains_key(&bea),
                slot.challenges.contains_key(&bea),
            ];
            n.member_heartbeat();
            (kept, n.presence.is_on_its_way(&bea))
        })
        .await;
        assert_eq!(found, ([false; 7], true));
        node.shutdown().await.unwrap();
    }
}
