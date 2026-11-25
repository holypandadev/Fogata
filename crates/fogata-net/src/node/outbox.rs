//! Member requests on their way: a few at a time to each peer, the rest waiting their turn,
//! a direct chat's messages one at a time, and what waits for an answer let go when a
//! request is answered, lost or dropped unsent.

use std::time::{Duration, Instant};

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

/// How long a quick request (see [`is_quick`]) may go unanswered on a connection before the
/// connection is suspect, and other member requests go on the next best one while it is: a
/// small request and its answer take about two round trips, under 1.5 s even through a
/// member's relay. A connection that died unnoticed takes far longer to be known dead: a
/// request on it fails after 10 s (the stream does not open), a dead QUIC connection closes
/// 25 to 30 s after it died, a TCP one 55 to 70 s after (see the `Ping` arm of
/// `on_behaviour_event` in swarm_events.rs).
pub(super) const SUSPECT_AFTER: Duration = Duration::from_secs(3);

/// Most bytes of a quick request.
const QUICK_BYTES: usize = 64 * 1024;

/// Whether `request` is a quick one: at most [`QUICK_BYTES`], of a kind whose answer is
/// small (every request is answered as soon as it is read). Logs, rooms' past and
/// whatever else can carry megabytes either way are not: on a slow link they may take
/// longer than [`SUSPECT_AFTER`] on a connection that works. Every kind is named, with no
/// wildcard.
pub(super) fn is_quick(request: &MemberRequest) -> bool {
    let small_answer = match request {
        MemberRequest::Challenge { .. }
        | MemberRequest::Deliver(_)
        | MemberRequest::Spread { .. }
        | MemberRequest::Roster { .. }
        | MemberRequest::Profile { .. }
        | MemberRequest::Addresses { .. }
        | MemberRequest::Signal { .. }
        | MemberRequest::Presence { .. }
        | MemberRequest::JoinDeclined { .. }
        | MemberRequest::OtherSession
        | MemberRequest::Direct { .. }
        | MemberRequest::GroupBanner { .. }
        | MemberRequest::GroupPicture { .. }
        | MemberRequest::KeyProblem { .. } => true,
        MemberRequest::Sync { .. }
        | MemberRequest::Push { .. }
        | MemberRequest::JoinRequest { .. }
        | MemberRequest::JoinCommit { .. }
        | MemberRequest::RoomSync { .. }
        | MemberRequest::Admitted { .. }
        | MemberRequest::RoomEvents { .. }
        | MemberRequest::FriendAsk { .. }
        | MemberRequest::MovedLog { .. } => false,
    };
    small_answer && request_bytes(request) <= QUICK_BYTES
}

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
        | MemberRequest::KeyProblem { .. }
        | MemberRequest::MovedLog { .. } => 0,
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
                self.request_unsent(peer, kind);
            }
            return;
        }
        outbox.in_flight += 1;
        // On the peer's best connection that is not suspect, chosen as the request leaves
        // (see `request_connection`): libp2p would take any of them, a slower one, or one
        // about to close (a relayed or silent one kept a few seconds). Without one known
        // here, as libp2p chooses.
        let quick = is_quick(&request);
        // A room event of this device's sent again avoids the connection the first failed
        // on, also after waiting its turn (see own.rs).
        let besides = match kind {
            Pending::Own(_, _, failed_on) => failed_on,
            _ => None,
        };
        let chosen = self.request_connection(&peer, besides);
        let member = &mut self.swarm.behaviour_mut().member;
        let id = match chosen {
            Some(connection) => member.send_request_on(&peer, connection, request),
            None => member.send_request(&peer, request),
        };
        if let Some(connection) = chosen.filter(|_| quick) {
            self.quick_on_its_way
                .insert(id, (connection, Instant::now()));
        }
        self.pending.insert(id, (peer, kind));
    }

    /// A member request to `peer` that will not go: the oldest of a full queue, or one
    /// waiting when `peer` went, or one that failed on its way. What waits for its answer is
    /// let go as when one fails on its way, but nothing is sent again now: it would only
    /// wait in the same queue, or dial a peer that just went. Each is asked or sent again
    /// later as after a refusal; a banner or picture record dropped before it left goes
    /// again unasked once the peer's outbox has room (see `request_unsent`).
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
            // Failed on its way: it may have got there, and only its answer be what was lost
            // (see `record_lost`). One dropped unsent, or whose dial failed, goes to
            // `request_unsent` instead.
            Pending::Banner(..) | Pending::Picture(..) => self.record_lost(peer, kind, false),
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
            // A room event of this device's counts as gone, and is not sent again: into a
            // full queue it would push out the next oldest, and a peer that went is not
            // dialled. Only one that failed on its way goes again (see own.rs).
            Pending::Own(_, id, _) => self.own_answered(id),
            // Another member is asked at the next heartbeat.
            Pending::MovedLog(group, _) => self.moved_log_lost(group, peer, false),
            _ => {}
        }
    }

    /// A member request to `peer` dropped before it went: past the outbox's bounds, or its
    /// last connection closed while it waited, or, for a banner or picture record, its dial
    /// failed. A banner or picture record never got there.
    pub(super) fn request_unsent(&mut self, peer: PeerId, kind: Pending) {
        match kind {
            Pending::Banner(..) | Pending::Picture(..) => self.record_lost(peer, kind, true),
            kind => self.request_dropped(peer, kind),
        }
    }

    /// A banner or picture record's request to `peer` was lost. One that failed on its way
    /// (no answer in time, its connection closed) may have got there, and only its answer
    /// be what was lost: the same record goes again a little later only at a roster of the
    /// member that names another. One dropped before it left (`unsent`) goes again unasked
    /// at a check of the links once the peer's outbox has room; a newer one chosen meanwhile
    /// at once either way (see `RecordSent::due_again`).
    fn record_lost(&mut self, peer: PeerId, kind: Pending, unsent: bool) {
        match kind {
            Pending::Banner(group, digest) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.banner_lost(peer, digest, unsent);
                }
            }
            Pending::Picture(group, digest) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.picture_lost(peer, digest, unsent);
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
            self.request_unsent(peer, kind);
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

    use fogata_crypto::GroupId;

    use fogata_proto::MemberResponse;
    use libp2p::{PeerId, request_response};

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

    /// What a member's budget may refill while a test charges it, at 256 bytes a millisecond:
    /// half the least a record counts, so that one record is still told from two, or none.
    const SLACK: usize = super::super::limits::SERVED_LEAST / 2;

    /// A group with a banner and a picture record, and member Bea, whose version keeps
    /// both, connected to it.
    fn with_records(n: &mut Node, group: GroupId) -> PeerId {
        let bea = PeerId::random();
        add_member(n, group, bea);
        let png = |fill: u8| {
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.resize(500, fill);
            png
        };
        let slot = n.groups.get_mut(&group).unwrap();
        slot.banner_peers.insert(bea);
        slot.picture_peers.insert(bea);
        slot.ctx.set_banner(png(1), crate::node::now_ms()).unwrap();
        slot.ctx.set_picture(png(2), crate::node::now_ms()).unwrap();
        bea
    }

    /// The banners and picture records on their way to `peer`.
    fn records_to(n: &Node, peer: PeerId) -> (usize, usize) {
        let count = |banner: bool| {
            (n.pending.values())
                .filter(|(p, kind)| {
                    *p == peer
                        && if banner {
                            matches!(kind, Pending::Banner(..))
                        } else {
                            matches!(kind, Pending::Picture(..))
                        }
                })
                .count()
        };
        (count(true), count(false))
    }

    /// The banner and the picture record on their way to `peer` fail, as a request whose
    /// connection closed or that got no answer in time does (see `on_member_event`).
    fn lose_records(n: &mut Node, peer: PeerId) {
        let lost: Vec<_> = (n.pending.iter())
            .filter(|(_, (p, kind))| {
                *p == peer && matches!(kind, Pending::Banner(..) | Pending::Picture(..))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in lost {
            let (to, kind) = n.pending.remove(&id).unwrap();
            n.request_done(to);
            n.request_dropped(to, kind);
        }
    }

    /// The banners and picture records waiting their turn to go to `peer`.
    fn records_waiting(n: &Node, peer: PeerId) -> (usize, usize) {
        let waiting = n.outbox.get(&peer).map(|o| &o.waiting);
        let count = |banner: bool| {
            (waiting.into_iter().flatten())
                .filter(|(_, kind)| {
                    if banner {
                        matches!(kind, Pending::Banner(..))
                    } else {
                        matches!(kind, Pending::Picture(..))
                    }
                })
                .count()
        };
        (count(true), count(false))
    }

    /// A roster ask of `peer`'s, naming the banner and picture record it has: the group's
    /// own when `has_them`, others when not.
    fn roster_of(n: &mut Node, group: GroupId, peer: PeerId, has_them: bool) {
        let slot = &n.groups[&group];
        let named = |digest: Vec<u8>| {
            Some(serde_bytes::ByteBuf::from(if has_them {
                digest
            } else {
                vec![0; 16]
            }))
        };
        let ask = MemberRequest::Roster {
            group_id: group.as_bytes().to_vec(),
            heads: None,
            banner: named(slot.ctx.banner_digest().to_vec()),
            picture: named(slot.ctx.picture_digest().to_vec()),
        };
        n.on_request(peer, ask);
    }

    /// Takes `ago` off when the records to `peer` last went or were lost, as if that much
    /// time had passed since.
    fn records_age(n: &mut Node, group: GroupId, peer: PeerId, ago: Duration) {
        let slot = n.groups.get_mut(&group).unwrap();
        for sent in [&mut slot.banner_sent, &mut slot.picture_sent] {
            let sent = sent.get_mut(&peer).unwrap();
            sent.at = sent.at.checked_sub(ago).unwrap();
        }
    }

    /// A banner or picture record whose request to a member was lost on its way goes to it
    /// again at its next roster, not only once [`super::super::RECORD_AGAIN`] passed, as
    /// one it took does.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_lost_on_its_way_goes_again_at_the_next_roster() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            let first = records_to(n, bea);
            lose_records(n, bea);
            // Its next roster, a heartbeat later, names neither.
            records_age(n, group, bea, Duration::from_secs(20));
            let ask = MemberRequest::Roster {
                group_id: group.as_bytes().to_vec(),
                heads: None,
                banner: Some(serde_bytes::ByteBuf::from(vec![0; 16])),
                picture: Some(serde_bytes::ByteBuf::from(vec![0; 16])),
            };
            n.on_request(bea, ask);
            (first, records_to(n, bea))
        })
        .await;
        assert_eq!(found, ((1, 1), (1, 1)));
        node.shutdown().await.unwrap();
    }

    /// A record sent again after a loss that is neither answered nor lost a minute later,
    /// as one still waiting in the outbox behind other requests is, does not go once more at
    /// each roster: it goes again only once [`super::super::RECORD_AGAIN`] passed, as before.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_sent_again_and_not_yet_answered_is_not_sent_once_more() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            lose_records(n, bea);
            records_age(n, group, bea, Duration::from_secs(2));
            roster_of(n, group, bea, false);
            let again = records_to(n, bea);
            let mut at_rosters = Vec::new();
            // 61 s after it went again, then 301 s.
            for ago in [61, 60 * 4] {
                records_age(n, group, bea, Duration::from_secs(ago));
                roster_of(n, group, bea, false);
                n.send_lost_records();
                at_rosters.push(records_to(n, bea));
            }
            (again, at_rosters)
        })
        .await;
        assert_eq!(found, ((1, 1), vec![(1, 1), (2, 2)]));
        node.shutdown().await.unwrap();
    }

    /// A record lost on its way goes again at a roster of the member that names another
    /// one, once it is due: 2, 4, 8 and 16 s after each loss in a row, not before; past four
    /// losses in a row only as a record that got there does. One that gets there, answered,
    /// starts the count afresh.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_lost_on_its_way_goes_again_soon_a_few_times() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            let mut tries = Vec::new();
            for wait in [2, 4, 8, 16, 60] {
                lose_records(n, bea);
                // Not before its time.
                records_age(n, group, bea, Duration::from_secs(wait - 1));
                roster_of(n, group, bea, false);
                let early = records_to(n, bea);
                records_age(n, group, bea, Duration::from_secs(1));
                roster_of(n, group, bea, false);
                tries.push((early, records_to(n, bea)));
            }
            // Answered at last: the next loss counts as the first again.
            records_age(n, group, bea, super::super::RECORD_AGAIN);
            n.send_banner_to(group, &[bea]);
            let id = (n.pending.iter())
                .find(|(_, (p, kind))| *p == bea && matches!(kind, Pending::Banner(..)))
                .map(|(id, _)| *id)
                .unwrap();
            let (to, kind) = n.pending.remove(&id).unwrap();
            n.request_done(to);
            n.on_response(to, kind, fogata_proto::MemberResponse::Ack);
            let after_answer = records_to(n, bea).0;
            records_age(n, group, bea, super::super::RECORD_AGAIN);
            n.send_banner_to(group, &[bea]);
            lose_records(n, bea);
            records_age(n, group, bea, Duration::from_secs(2));
            roster_of(n, group, bea, false);
            (tries, after_answer, records_to(n, bea).0)
        })
        .await;
        let (tries, after_answer, again) = found;
        let went = ((0, 0), (1, 1));
        assert_eq!(tries, vec![went, went, went, went, ((0, 0), (0, 0))]);
        assert_eq!((after_answer, again), (0, 1));
        node.shutdown().await.unwrap();
    }

    /// Past its quick tries, a record lost again and again goes once each
    /// [`super::super::RECORD_AGAIN`], at the member's roster, for as long as it goes on:
    /// what is kept of its losses outlasts that while, and its quick tries do not start
    /// again. A newer record chosen after a loss goes at the next check of the links.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_lost_over_and_over_goes_once_each_while() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            for wait in [2, 4, 8, 16] {
                lose_records(n, bea);
                records_age(n, group, bea, Duration::from_secs(wait));
                roster_of(n, group, bea, false);
            }
            // The fifth try is lost once the request timed out, 20 s after it went: the
            // five minutes run from when it went.
            let on_its_way = Duration::from_secs(20);
            records_age(n, group, bea, on_its_way);
            lose_records(n, bea);
            let roster = |n: &mut Node| {
                n.on_request(
                    bea,
                    MemberRequest::Roster {
                        group_id: group.as_bytes().to_vec(),
                        heads: None,
                        banner: Some(serde_bytes::ByteBuf::from(vec![0; 16])),
                        picture: Some(serde_bytes::ByteBuf::from(vec![0; 16])),
                    },
                );
                records_to(n, bea)
            };
            let second = Duration::from_secs(1);
            let mut seen = Vec::new();
            // Since the last record went: the time it was on its way, then the 2 s of the
            // try that did not go.
            for since in [on_its_way, Duration::from_secs(2)] {
                records_age(n, group, bea, super::super::RECORD_AGAIN - second - since);
                n.send_lost_records();
                seen.push(roster(n));
                records_age(n, group, bea, second);
                n.groups.get_mut(&group).unwrap().forget_sent();
                n.send_lost_records();
                seen.push(roster(n));
                lose_records(n, bea);
                records_age(n, group, bea, Duration::from_secs(2));
                n.send_lost_records();
                seen.push(records_to(n, bea));
            }
            let mut newer = b"\x89PNG\r\n\x1a\n".to_vec();
            newer.resize(500, 3);
            let slot = n.groups.get_mut(&group).unwrap();
            slot.ctx.set_banner(newer, crate::node::now_ms()).unwrap();
            n.send_lost_records();
            (seen, records_to(n, bea))
        })
        .await;
        let (none, both) = ((0, 0), (1, 1));
        assert_eq!(found, (vec![none, both, none, none, both, none], (1, 0)));
        node.shutdown().await.unwrap();
    }

    /// A record that failed on its way to a member (its connection closed, no answer in
    /// time) goes again at the next check of the links, unasked, 2 s after the failure and
    /// 4 s after the next, and not a third time: past those, only a roster that names
    /// another record sends it. It does not go to a member that is gone, nor once answered.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_that_failed_outright_goes_again_soon_unasked_twice() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            let second = Duration::from_secs(1);
            let mut seen = Vec::new();
            // The wait after each failure: 2 s, then 4 s.
            for wait in [2, 4] {
                lose_records(n, bea);
                // Not before its time.
                records_age(n, group, bea, second * (wait - 1));
                n.send_lost_records();
                seen.push(records_to(n, bea));
                records_age(n, group, bea, second);
                n.send_lost_records();
                seen.push(records_to(n, bea));
            }
            // The third failure in a row: not again unasked, however long it waits.
            lose_records(n, bea);
            records_age(n, group, bea, Duration::from_secs(60));
            n.send_lost_records();
            seen.push(records_to(n, bea));
            // At the member's roster, as before.
            roster_of(n, group, bea, false);
            seen.push(records_to(n, bea));
            // A member that is gone is not sent it.
            lose_records(n, bea);
            n.groups.get_mut(&group).unwrap().members.remove(&bea);
            records_age(n, group, bea, Duration::from_secs(60));
            n.send_lost_records();
            seen.push(records_to(n, bea));
            seen
        })
        .await;
        let (none, both) = ((0, 0), (1, 1));
        assert_eq!(found, vec![none, both, none, both, none, both, none]);
        node.shutdown().await.unwrap();
    }

    /// A record that failed on its way may have got there, only its answer lost: it is
    /// sent again unasked only a couple of times (`RECORD_LOST_UNASKED`), however often
    /// the copy it would send fails too, and after that goes again only to a member whose
    /// roster names another record, never to one whose roster names it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_that_may_have_got_there_goes_again_unasked_only_twice() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, cai) = (with_records(n, group), with_records(n, group));
            n.send_banner_to(group, &[bea, cai]);
            n.send_picture_to(group, &[bea, cai]);
            // Bea took both, and each answer was lost; the links are checked every 2 s. The
            // two unasked tries each is allowed are all that go.
            let mut copies = (0, 0);
            for _ in 0..30 {
                lose_records(n, bea);
                records_age(n, group, bea, Duration::from_secs(2));
                n.send_lost_records();
                let (banners, pictures) = records_to(n, bea);
                copies = (copies.0 + banners, copies.1 + pictures);
            }
            // Its roster names both: nothing goes.
            roster_of(n, group, bea, true);
            let bea_roster = records_to(n, bea);
            // Cai lacks both: they go unasked at the next check, and its roster adds no
            // copy while they are on their way.
            lose_records(n, cai);
            records_age(n, group, cai, Duration::from_secs(20));
            n.send_lost_records();
            let unasked = records_to(n, cai);
            roster_of(n, group, cai, false);
            (copies, bea_roster, unasked, records_to(n, cai))
        })
        .await;
        assert_eq!(found, ((2, 2), (0, 0), (1, 1), (1, 1)));
        node.shutdown().await.unwrap();
    }

    /// A record dropped before it left, from a member's full outbox, is counted apart from
    /// one that failed on its way: it goes again unasked 2 s later once nothing waits for
    /// that member, so that it pushes out no other request, however often it was dropped,
    /// and its quick tries stay as they were.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_dropped_from_a_full_outbox_goes_again_once_there_is_room() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.outbox.entry(bea).or_default().in_flight = IN_FLIGHT;
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            let fill = |n: &mut Node| {
                for _ in 0..MAX_WAITING {
                    n.request(bea, empty_push(group), Pending::Push);
                }
            };
            let drain = |n: &mut Node| {
                let outbox = n.outbox.get_mut(&bea).unwrap();
                outbox.waiting.clear();
                outbox.waiting_bytes = 0;
            };
            let second = Duration::from_secs(1);
            let mut rounds = Vec::new();
            for _ in 0..6 {
                // Requests that fill the outbox push both out.
                fill(n);
                let pushed_out = records_waiting(n, bea);
                // A second after: not yet, though nothing waits.
                records_age(n, group, bea, second);
                drain(n);
                n.send_lost_records();
                let early = records_waiting(n, bea);
                // Two seconds after: not while the outbox is full, which would push out
                // another request.
                fill(n);
                records_age(n, group, bea, second);
                n.send_lost_records();
                let full = records_waiting(n, bea);
                let others = n.outbox[&bea].waiting.len();
                // Once it drained, both go, waiting their turn.
                drain(n);
                n.send_lost_records();
                rounds.push((pushed_out, early, full, others, records_waiting(n, bea)));
            }
            let slot = &n.groups[&group];
            let sent = (slot.banner_sent[&bea], slot.picture_sent[&bea]);
            (
                rounds,
                (sent.0.lost, sent.0.unsent),
                (sent.1.lost, sent.1.unsent),
            )
        })
        .await;
        let (rounds, banner, picture) = found;
        let none = (0, 0);
        let round = (none, none, none, MAX_WAITING, (1, 1));
        assert_eq!(rounds, vec![round; 6]);
        assert_eq!((banner, picture), ((0, 6), (0, 6)));
        node.shutdown().await.unwrap();
    }

    /// A record dropped before it left, after it failed on its way three times in a row,
    /// goes again as late as those failures ask, 8 s after, not 2 s, unasked or at a
    /// roster: a drop neither raises nor cuts their wait.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_dropped_after_failures_waits_as_long_as_they_ask() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = with_records(n, group);
            n.send_banner_to(group, &[bea]);
            n.send_picture_to(group, &[bea]);
            for wait in [2, 4] {
                lose_records(n, bea);
                records_age(n, group, bea, Duration::from_secs(wait));
                roster_of(n, group, bea, false);
            }
            lose_records(n, bea);
            records_age(n, group, bea, Duration::from_secs(8));
            roster_of(n, group, bea, false);
            // The fourth try is dropped before it left.
            let dropped: Vec<_> = (n.pending.iter())
                .filter(|(_, (p, _))| *p == bea)
                .map(|(id, _)| *id)
                .collect();
            for id in dropped {
                let (to, kind) = n.pending.remove(&id).unwrap();
                n.request_done(to);
                n.request_unsent(to, kind);
            }
            let mut seen = Vec::new();
            for wait in [2, 5, 1] {
                records_age(n, group, bea, Duration::from_secs(wait));
                roster_of(n, group, bea, false);
                n.send_lost_records();
                seen.push(records_to(n, bea));
            }
            let sent = n.groups[&group].banner_sent[&bea];
            (seen, (sent.lost, sent.unsent))
        })
        .await;
        let none = (0, 0);
        assert_eq!(found, (vec![none, none, (1, 1)], (3, 1)));
        node.shutdown().await.unwrap();
    }

    /// A record sent again unasked is charged to the member it goes to, as one its roster
    /// asks for is: one dropped before it left, and a newer one chosen after a loss. One
    /// sent at a change to the log is not charged, as before.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_sent_again_unasked_is_charged_to_its_member() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, cai) = (with_records(n, group), with_records(n, group));
            let left = |n: &Node, peer: PeerId| n.limits.served_left(n.members[&peer].device);
            n.send_banner_to(group, &[bea, cai]);
            n.send_picture_to(group, &[bea, cai]);
            let before = (left(n, bea), left(n, cai));
            // Bea's both dropped before they left.
            let dropped: Vec<_> = (n.pending.iter())
                .filter(|(_, (p, _))| *p == bea)
                .map(|(id, _)| *id)
                .collect();
            for id in dropped {
                let (to, kind) = n.pending.remove(&id).unwrap();
                n.request_done(to);
                n.request_unsent(to, kind);
            }
            records_age(n, group, bea, Duration::from_secs(2));
            // Cai's failed on their way, and a newer banner was chosen since.
            lose_records(n, cai);
            let mut newer = b"\x89PNG\r\n\x1a\n".to_vec();
            newer.resize(500, 3);
            let slot = n.groups.get_mut(&group).unwrap();
            slot.ctx.set_banner(newer, crate::node::now_ms()).unwrap();
            n.send_lost_records();
            let charged = |n: &Node, peer, before: f64| (before - left(n, peer)).round() as usize;
            let charged = (charged(n, bea, before.0), charged(n, cai, before.1));
            let went = (records_to(n, bea), records_to(n, cai));
            (before.0 == before.1, charged, went)
        })
        .await;
        // Each record of 500 bytes counts as the least an answer counts, less what the
        // budget refills meanwhile (see `SLACK`).
        let least = super::super::limits::SERVED_LEAST;
        let (same, (bea, cai), went) = found;
        assert!(same);
        assert!((2 * least - SLACK..=2 * least).contains(&bea), "{bea}");
        assert!((least - SLACK..=least).contains(&cai), "{cai}");
        assert_eq!(went, ((1, 1), (1, 0)));
        node.shutdown().await.unwrap();
    }

    /// A newer record chosen while one was on its way goes once that one is answered, and
    /// is charged to the member it goes to, as one sent again after a loss is. An answer
    /// with nothing newer to send charges nothing.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_newer_record_sent_at_an_answer_is_charged_to_its_member() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, cai) = (with_records(n, group), with_records(n, group));
            let left = |n: &Node, peer: PeerId| n.limits.served_left(n.members[&peer].device);
            let key = |n: &Node, banner: bool| {
                let ctx = &n.groups[&group].ctx;
                super::super::group_changes::banner_key(&if banner {
                    ctx.banner_digest()
                } else {
                    ctx.picture_digest()
                })
            };
            n.send_banner_to(group, &[bea, cai]);
            n.send_picture_to(group, &[bea, cai]);
            let (banner, picture) = (key(n, true), key(n, false));
            // Cai's are answered with nothing newer chosen.
            let cai_before = left(n, cai);
            n.on_response(cai, Pending::Banner(group, banner), MemberResponse::Ack);
            n.on_response(cai, Pending::Picture(group, picture), MemberResponse::Ack);
            let cai_after = left(n, cai);
            // Newer ones are chosen while Bea's are on their way, and go at their answers.
            let png = |fill: u8| {
                let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
                png.resize(500, fill);
                png
            };
            let slot = n.groups.get_mut(&group).unwrap();
            slot.ctx.set_banner(png(5), crate::node::now_ms()).unwrap();
            slot.ctx.set_picture(png(6), crate::node::now_ms()).unwrap();
            let bea_before = left(n, bea);
            // Answered: no longer on their way.
            let answered: Vec<_> = (n.pending.iter())
                .filter(|(_, (p, _))| *p == bea)
                .map(|(id, _)| *id)
                .collect();
            for id in answered {
                n.pending.remove(&id);
                n.request_done(bea);
            }
            n.on_response(bea, Pending::Banner(group, banner), MemberResponse::Ack);
            n.on_response(bea, Pending::Picture(group, picture), MemberResponse::Ack);
            let charged = |before: f64, after: f64| (before - after).round() as usize;
            let charged = (
                charged(bea_before, left(n, bea)),
                charged(cai_before, cai_after),
            );
            (charged, records_to(n, bea))
        })
        .await;
        let least = super::super::limits::SERVED_LEAST;
        // As in the test above.
        let ((bea, cai), went) = found;
        assert!((2 * least - SLACK..=2 * least).contains(&bea), "{bea}");
        assert_eq!(cai, 0);
        assert_eq!(went, (1, 1));
        node.shutdown().await.unwrap();
    }

    /// A record whose dial failed never left: it goes again unasked as one dropped before
    /// it left, a little later, not held back as one that may have got there. One whose
    /// connection closed on its way goes again unasked too, after the same wait (at most
    /// twice; see `a_record_that_failed_outright_goes_again_soon_unasked_twice`).
    #[tokio::test(flavor = "multi_thread")]
    async fn a_record_whose_dial_failed_goes_again_as_one_that_never_left() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let (bea, cai) = (with_records(n, group), with_records(n, group));
            n.send_banner_to(group, &[bea, cai]);
            n.send_picture_to(group, &[bea, cai]);
            let failed: Vec<_> = (n.pending.iter())
                .filter(|(_, (p, _))| *p == bea || *p == cai)
                .map(|(id, (p, _))| (*id, *p))
                .collect();
            for (request_id, peer) in failed {
                let error = if peer == bea {
                    request_response::OutboundFailure::DialFailure
                } else {
                    request_response::OutboundFailure::ConnectionClosed
                };
                n.on_member_event(request_response::Event::OutboundFailure {
                    peer,
                    connection_id: libp2p::swarm::ConnectionId::new_unchecked(1),
                    request_id,
                    error,
                });
            }
            let counts = |n: &Node, peer: PeerId| {
                let slot = &n.groups[&group];
                let (b, p) = (slot.banner_sent[&peer], slot.picture_sent[&peer]);
                ((b.lost, b.unsent), (p.lost, p.unsent))
            };
            let kept = (counts(n, bea), counts(n, cai));
            records_age(n, group, bea, Duration::from_secs(2));
            records_age(n, group, cai, Duration::from_secs(2));
            n.send_lost_records();
            (kept, records_to(n, bea), records_to(n, cai))
        })
        .await;
        let (unsent, lost) = (((0, 1), (0, 1)), ((1, 0), (1, 0)));
        assert_eq!(found, ((unsent, lost), (1, 1), (1, 1)));
        node.shutdown().await.unwrap();
    }
}
