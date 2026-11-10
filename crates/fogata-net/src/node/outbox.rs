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
/// Most bytes of member requests waiting for one peer; past it the oldest go. A member that
/// stops reading would otherwise keep up to [`MAX_WAITING`] banners or group messages here,
/// hundreds of MiB, for as long as the queue takes to drain at a few requests per timeout.
pub(super) const MAX_WAITING_BYTES: usize = 16 * 1024 * 1024;

/// About the bytes `request` takes while it waits: what it carries that can be large.
pub(super) fn request_bytes(request: &MemberRequest) -> usize {
    use super::members::wire_bytes;
    let list = |l: &[serde_bytes::ByteBuf]| l.iter().map(|b| b.len() + 8).sum::<usize>();
    64 + match request {
        MemberRequest::Challenge { challenge, tags } => challenge.len() + list(tags),
        MemberRequest::Deliver(message) => wire_bytes(message),
        MemberRequest::Spread { message, reached } => wire_bytes(message) + reached.len(),
        MemberRequest::Sync { have, .. } => list(have),
        MemberRequest::Push { entries, .. } => entries.iter().map(|e| e.len() + 8).sum(),
        MemberRequest::Admitted { entries, .. } => list(entries),
        MemberRequest::JoinRequest { ticket } => ticket.len(),
        MemberRequest::JoinCommit { entry, binding } => {
            entry.len() + binding.as_ref().map_or(0, |b| b.len())
        }
        MemberRequest::Profile { profile, animation } => {
            profile.len() + animation.as_ref().map_or(0, |a| a.len())
        }
        MemberRequest::Addresses { addresses } => list(addresses),
        MemberRequest::Signal { payload, .. } => payload.len(),
        MemberRequest::JoinDeclined { revoke, .. } => revoke.as_ref().map_or(0, |r| r.len()),
        MemberRequest::Direct { messages, .. } => messages.iter().map(wire_bytes).sum(),
        MemberRequest::GroupBanner { banner, .. } => banner.len(),
        MemberRequest::GroupPicture { picture, .. } => picture.len(),
        MemberRequest::RoomEvents { ids, .. } => ids.len(),
        MemberRequest::FriendAsk { sealed } => sealed.len(),
        MemberRequest::Roster { .. }
        | MemberRequest::RoomSync { .. }
        | MemberRequest::Presence { .. }
        | MemberRequest::OtherSession
        | MemberRequest::KeyProblem { .. } => 0,
    }
}

impl Node {
    /// Sends a member request, or keeps it until fewer are on their way to that peer (see
    /// [`IN_FLIGHT`]); past [`MAX_WAITING`] or [`MAX_WAITING_BYTES`] waiting, the oldest
    /// waiting go (the one just added always waits).
    pub(super) fn request(&mut self, peer: PeerId, request: MemberRequest, kind: Pending) {
        let outbox = self.outbox.entry(peer).or_default();
        if outbox.in_flight >= IN_FLIGHT {
            outbox.waiting_bytes += request_bytes(&request);
            outbox.waiting.push_back((request, kind));
            let mut dropped = Vec::new();
            while outbox.waiting.len() > 1
                && (outbox.waiting.len() > MAX_WAITING || outbox.waiting_bytes > MAX_WAITING_BYTES)
            {
                let Some((request, kind)) = outbox.waiting.pop_front() else {
                    break;
                };
                outbox.waiting_bytes = outbox.waiting_bytes.saturating_sub(request_bytes(&request));
                dropped.push(kind);
            }
            for kind in dropped {
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
            // No longer on its way: another banner or picture may go to it at once, the same
            // one only after a while (see `RecordSent`), as after an answer.
            Pending::Banner(group, digest) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.banner_answered(peer, digest);
                }
            }
            Pending::Picture(group, digest) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.picture_answered(peer, digest);
                }
            }
            // Sent again at the next roster that asks for them.
            Pending::Sync(group) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.syncing.remove(&peer);
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
            .map(|o| {
                o.waiting_bytes = 0;
                std::mem::take(&mut o.waiting)
            })
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
            outbox.waiting_bytes = outbox.waiting_bytes.saturating_sub(request_bytes(&request));
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

    /// Large requests waiting for a member that does not read are kept up to
    /// [`MAX_WAITING_BYTES`], the oldest dropped first, not up to [`MAX_WAITING`] of them.
    #[tokio::test(flavor = "multi_thread")]
    async fn requests_waiting_for_a_member_are_kept_up_to_a_bound_in_bytes() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = PeerId::random();
            add_member(n, group, bea);
            for _ in 0..IN_FLIGHT {
                n.request(bea, empty_push(group), Pending::Push);
            }
            let large = || MemberRequest::GroupBanner {
                group_id: group.as_bytes().to_vec(),
                banner: vec![0; 512 * 1024],
            };
            for _ in 0..200 {
                n.request(bea, large(), Pending::Push);
            }
            let outbox = &n.outbox[&bea];
            let counted: usize = outbox.waiting.iter().map(|(r, _)| request_bytes(r)).sum();
            (outbox.waiting.len(), outbox.waiting_bytes, counted)
        })
        .await;
        let (waiting, bytes, counted) = found;
        assert_eq!(bytes, counted);
        assert!(bytes <= MAX_WAITING_BYTES, "{bytes} bytes waiting");
        assert!(
            waiting >= MAX_WAITING_BYTES / (513 * 1024),
            "only {waiting} waiting"
        );
        node.shutdown().await.unwrap();
    }

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
