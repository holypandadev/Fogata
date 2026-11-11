//! Groups offered to friends: a group this device shares with a friend is sent to it, part
//! after part when its log is long, an offer that comes in is judged away from the loop
//! before it is taken, and an invitation this device made that was declined is told to the
//! application once.

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_proto::{FriendRequest, FriendResponse};
use fogata_state::{Body, Entry, EntryHash, GroupLog};
use libp2p::PeerId;

use super::{
    BehindTold, Node,
    friends::{FriendContext, FriendPending},
    judging,
    limits::Sender,
};
use crate::NodeEvent;

/// Most entries, and bytes of entries, of a group a friend offers this device: far more than
/// a group's log holds, and a bound on what one offer may write to the disk.
const MAX_OFFERED_ENTRIES: usize = 4096;
pub(super) const MAX_OFFERED_BYTES: usize = 4 * 1024 * 1024;
/// Most parts of a long log a friend offers (see `share_group`).
pub(super) const MAX_OFFERED_PARTS: u32 = 64;
/// Bytes of offered logs kept waiting for their other parts, from every friend together.
pub(super) const MAX_OFFERED_WAITING: usize = 16 * 1024 * 1024;
/// How long a log offered in parts waits for its next part, and for all of them: each part
/// is sent once the one before arrived, tried again three times when lost (a request's 20
/// seconds each), and the longest log takes about five minutes at 128 kbit/s. Past that it
/// is dropped, so a friend that never finishes keeps nobody else's offer out.
const OFFER_PART_WITHIN: Duration = Duration::from_secs(2 * 60);
const OFFER_WITHIN: Duration = Duration::from_secs(10 * 60);

/// Times a part of an offered log that did not arrive is sent again.
const PART_TRIES: u8 = 3;

/// A long log on its way to a friend: the parts still to send, the number of the first of
/// them, how many there are in all, and how many times the first was tried.
pub(super) struct Offering {
    parts: std::collections::VecDeque<Vec<serde_bytes::ByteBuf>>,
    next: u32,
    count: u32,
    tries: u8,
}

/// Parts of a long log a friend offers, until they are all here, with when the first came
/// and when the last new one did.
pub(super) struct OfferIn {
    group: GroupId,
    count: u32,
    bytes: usize,
    parts: BTreeMap<u32, Vec<Vec<u8>>>,
    since: Instant,
    last: Instant,
}

/// What became of a part of a log offered by a friend.
pub(super) enum OfferPart {
    /// Kept, waiting for the others.
    Kept,
    Refused,
    /// The last one: the whole log, its parts in order.
    Whole(Vec<Vec<u8>>),
}

impl FriendContext {
    /// A part of a long log the friend `by` offers. A part sent again (its answer was lost)
    /// takes the place of the one kept, and counts once. What one friend's offer holds is
    /// bounded, and so is what all of them hold: offers that stopped coming go first, then
    /// those holding more than this one would, the most first; when that is not enough the
    /// part is refused.
    pub(super) fn take_offered_part(
        &mut self,
        by: DevicePublicKey,
        group: GroupId,
        index: u32,
        count: u32,
        entries: Vec<Vec<u8>>,
        now: Instant,
    ) -> OfferPart {
        self.forget_stale_offers(now);
        if count > MAX_OFFERED_PARTS || index >= count {
            self.offers_in.remove(&by);
            return OfferPart::Refused;
        }
        let size: usize = entries.iter().map(Vec::len).sum();
        let offer = self.offers_in.entry(by).or_insert_with(|| OfferIn {
            group,
            count,
            bytes: 0,
            parts: BTreeMap::new(),
            since: now,
            last: now,
        });
        if offer.group != group || offer.count != count {
            *offer = OfferIn {
                group,
                count,
                bytes: 0,
                parts: BTreeMap::new(),
                since: now,
                last: now,
            };
        }
        let replaced = offer
            .parts
            .get(&index)
            .map_or(0, |p| p.iter().map(Vec::len).sum());
        let held = offer.bytes - replaced + size;
        if held > MAX_OFFERED_BYTES || !self.room_for_offer(by, held) {
            self.offers_in.remove(&by);
            return OfferPart::Refused;
        }
        let offer = self.offers_in.get_mut(&by).expect("kept above");
        if offer.parts.insert(index, entries).is_none() {
            offer.last = now;
        }
        offer.bytes = held;
        if offer.parts.len() as u32 != count {
            return OfferPart::Kept;
        }
        match self.offers_in.remove(&by) {
            Some(offer) => OfferPart::Whole(offer.parts.into_values().flatten().collect()),
            None => OfferPart::Refused,
        }
    }

    /// Drops offers whose next part has not come in time, or that took too long in all.
    pub(super) fn forget_stale_offers(&mut self, now: Instant) {
        self.offers_in.retain(|_, o| {
            now.saturating_duration_since(o.last) < OFFER_PART_WITHIN
                && now.saturating_duration_since(o.since) < OFFER_WITHIN
        });
    }

    /// Whether friend `by` may hold `held` bytes of its offer within what all offers may
    /// hold together, once those holding more than it would have made way, the most first.
    fn room_for_offer(&mut self, by: DevicePublicKey, held: usize) -> bool {
        let mut others: Vec<(usize, DevicePublicKey)> = self
            .offers_in
            .iter()
            .filter(|(d, _)| **d != by)
            .map(|(d, o)| (o.bytes, *d))
            .collect();
        let mut total = others.iter().map(|(b, _)| b).sum::<usize>() + held;
        others.sort_unstable_by_key(|o| std::cmp::Reverse(o.0));
        for (bytes, device) in others {
            if total <= MAX_OFFERED_WAITING || bytes <= held {
                break;
            }
            self.offers_in.remove(&device);
            total -= bytes;
        }
        total <= MAX_OFFERED_WAITING
    }

    /// What offers in parts hold, for tests and fuzzing.
    #[cfg(any(test, feature = "fuzzing"))]
    pub(super) fn offered_bytes(&self) -> usize {
        self.offers_in
            .values()
            .flat_map(|o| o.parts.values())
            .flatten()
            .map(Vec::len)
            .sum()
    }
}

impl Node {
    /// Sends a changed group to the friends it concerns right away: a friend just added to
    /// the group gets the invitation now rather than at the next heartbeat.
    pub(super) fn group_changed_for_friends(&mut self, group: GroupId) {
        if let Some(f) = self.friends.as_mut() {
            f.group_shared.retain(|(_, g)| *g != group);
        }
        self.share_group();
    }

    /// Sends a group's log to online friends who are members of the group but are not
    /// connected as its members, for example because they were just added and do not know
    /// the group yet. A long log goes in parts of at most `SYNC_BYTES`, each sent once the
    /// one before arrived, so that each arrives within a request's time also over a slow
    /// link.
    ///
    /// A friend whose copy is known not to judge at the group's level is not sent it: it
    /// would refuse the log from the entry that raised the group on (see
    /// `joins::judges_level`). Its manager is told, once, that it must update; it gets the
    /// group once it has.
    pub(super) fn share_group(&mut self) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let mut due: Vec<(PeerId, GroupId, Vec<serde_bytes::ByteBuf>)> = Vec::new();
        let mut long: Vec<(PeerId, GroupId)> = Vec::new();
        let mut behind: Vec<(GroupId, DevicePublicKey, String)> = Vec::new();
        let advertised = self.advertised.clone();
        let advertised = crate::locked(&advertised);
        for (group, slot) in &self.groups {
            if !slot.ctx.i_am_member() || slot.call || slot.direct {
                continue;
            }
            let level = slot.ctx.log().level();
            let mut peers: Vec<PeerId> = Vec::new();
            for (peer, device) in &f.online {
                if !slot.ctx.is_member_device(device)
                    || slot.members.contains(peer)
                    || f.group_shared.contains(&(*peer, *group))
                {
                    continue;
                }
                let listed = advertised
                    .get(peer)
                    .filter(|protocols| !protocols.is_empty())
                    .map(crate::caps::level_listed);
                match super::joins::judges_level(level, listed) {
                    // Not known: sent as before.
                    Some(true) | None => peers.push(*peer),
                    Some(false)
                        if slot.ctx.i_am_manager()
                            && !slot.behind_told.contains(&BehindTold::Friend(*device)) =>
                    {
                        let state = slot.ctx.log().state();
                        let name = state.members.get(device).map(|m| m.name.clone());
                        behind.push((*group, *device, name.unwrap_or_default()));
                    }
                    Some(false) => {}
                }
            }
            if peers.is_empty() {
                continue;
            }
            let parts: std::collections::VecDeque<Vec<serde_bytes::ByteBuf>> = slot
                .ctx
                .encoded_parts()
                .into_iter()
                .map(|p| p.into_iter().map(serde_bytes::ByteBuf::from).collect())
                .collect();
            let count = parts.len() as u32;
            for peer in peers {
                f.group_shared.insert((peer, *group));
                if count <= 1 {
                    due.push((peer, *group, parts.front().cloned().unwrap_or_default()));
                } else {
                    f.offering.insert(
                        (peer, *group),
                        Offering {
                            parts: parts.clone(),
                            next: 0,
                            count,
                            tries: 0,
                        },
                    );
                    long.push((peer, *group));
                }
            }
        }
        drop(advertised);
        for (peer, _, entries) in due {
            self.friend_request(
                peer,
                FriendRequest::Group {
                    entries,
                    declined: None,
                    part: None,
                },
                FriendPending::Group,
            );
        }
        for (peer, group) in long {
            self.send_offered_part(peer, group);
        }
        for (group, device, name) in behind {
            self.tell_behind(group, BehindTold::Friend(device), device, name);
        }
    }

    /// Sends the next part of a long log offered to the friend on `peer`, if one is left.
    fn send_offered_part(&mut self, peer: PeerId, group: GroupId) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let Some(offering) = f.offering.get(&(peer, group)) else {
            return;
        };
        let Some(entries) = offering.parts.front().cloned() else {
            f.offering.remove(&(peer, group));
            return;
        };
        let part = Some((offering.next, offering.count));
        self.friend_request(
            peer,
            FriendRequest::Group {
                entries,
                declined: None,
                part,
            },
            FriendPending::GroupPart(group),
        );
    }

    /// The friend took a part of a long log (`taken`) and gets the next, or refused it and
    /// gets no more; a part that did not arrive goes again a few times. A log that does not
    /// get through is offered again on the next connection, or when the group changes.
    pub(super) fn offered_part_answered(
        &mut self,
        peer: PeerId,
        group: GroupId,
        answer: Option<bool>,
    ) {
        let Some(f) = self.friends.as_mut() else {
            return;
        };
        let Some(offering) = f.offering.get_mut(&(peer, group)) else {
            return;
        };
        match answer {
            Some(true) => {
                offering.parts.pop_front();
                offering.next += 1;
                offering.tries = 0;
            }
            None if offering.tries < PART_TRIES => offering.tries += 1,
            _ => {
                f.offering.remove(&(peer, group));
                return;
            }
        }
        self.send_offered_part(peer, group);
    }

    /// Sends signed entries of a group to the online friends among `to` (see
    /// [`crate::NodeHandle::give_entries`]).
    pub(super) fn give_entries(
        &mut self,
        to: &[DevicePublicKey],
        entries: Vec<Vec<u8>>,
        declined: Option<EntryHash>,
    ) {
        let entries: Vec<serde_bytes::ByteBuf> = entries
            .into_iter()
            .map(serde_bytes::ByteBuf::from)
            .collect();
        let declined = declined.map(|hash| serde_bytes::ByteBuf::from(hash.to_vec()));
        for peer in self.friend_peers(to) {
            self.friend_request(
                peer,
                FriendRequest::Group {
                    entries: entries.clone(),
                    declined: declined.clone(),
                    part: None,
                },
                FriendPending::Group,
            );
        }
    }

    /// `device` handed back the `Leave` it signed (`entries`, that one alone, now in the
    /// log) for the invitation that `admission` made. Said to the application once per
    /// invitation, when this device made it: it added the device, or wrote the invite the
    /// device joined with (an admin's yes to a join is not in the log; the invite's writer
    /// stands for it). Returns whether the `Leave` is there.
    fn on_declined(
        &mut self,
        group: GroupId,
        device: DevicePublicKey,
        admission: EntryHash,
        entries: &[Vec<u8>],
    ) -> bool {
        let me = self.identity.public_key();
        let Some(slot) = self.groups.get_mut(&group) else {
            return false;
        };
        let log = slot.ctx.log();
        let left = !slot.ctx.is_member_device(&device)
            && matches!(entries, [leave] if Entry::decode(leave).is_ok_and(|e| {
                e.author == device
                    && matches!(e.body, Body::Leave)
                    && log.get(&e.hash()).is_some()
            }));
        let mine = log.get(&admission).is_some_and(|e| match &e.body {
            Body::Add { member, .. } => *member == device && e.author == me,
            Body::Join { ticket, .. } => e.author == device && ticket.issuer == me,
            _ => false,
        });
        if left && mine && slot.declines_told.insert(admission) {
            self.emit(NodeEvent::InvitationDeclined { group, device });
        }
        left
    }

    /// A group's log offered by the friend `by` on `peer`, whole: entries for a group that
    /// runs here, from one of its members, or a group to show the person, read whole or not
    /// at all (it is offered again later). `declined` names the invitation `entries` hand
    /// back the `Leave` of. A group to show is judged away from the loop (see judging.rs),
    /// and shown once it is ([`Self::group_offer_judged`]).
    pub(super) fn on_group_offer(
        &mut self,
        peer: PeerId,
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
        declined: Option<serde_bytes::ByteBuf>,
    ) -> FriendResponse {
        let Some(group_id) = entries
            .first()
            .and_then(|e| Entry::decode(e).ok())
            .map(|e| e.group_id)
        else {
            return FriendResponse::Refused;
        };
        let declined = declined.and_then(|h| EntryHash::try_from(h.as_slice()).ok());
        let running = self.groups.contains_key(&group_id);
        // Refused before the friend pays for it when too many logs wait to be judged.
        if !running && !self.judging.has_room(peer) {
            return FriendResponse::Refused;
        }
        // Each entry costs a signature check: a friend gets the same budget as a member
        // (see limits.rs).
        let wanted = entries.len();
        let taken = self
            .limits
            .take_entries(Sender::Device(by), &entries, Instant::now());
        if running {
            return self.offer_to_running(peer, by, group_id, &entries, taken, declined);
        }
        // A group offered is read whole, or not at all: it is offered again later.
        if entries.len() > MAX_OFFERED_ENTRIES
            || entries.iter().map(Vec::len).sum::<usize>() > MAX_OFFERED_BYTES
            || taken < wanted
        {
            return FriendResponse::Refused;
        }
        let job = judging::Job::Group {
            by,
            entries,
            declined,
        };
        if self.judging.add(peer, group_id, job) {
            FriendResponse::Ack
        } else {
            FriendResponse::Refused
        }
    }

    /// Entries a friend offered for a group that runs here: taken only from one of its own
    /// members, the first `taken` of them, as any batch a member sends. One that declined
    /// may hand its `Leave` again after another member did.
    fn offer_to_running(
        &mut self,
        peer: PeerId,
        by: DevicePublicKey,
        group_id: GroupId,
        entries: &[Vec<u8>],
        taken: usize,
        declined: Option<EntryHash>,
    ) -> FriendResponse {
        let Some(slot) = self.groups.get_mut(&group_id) else {
            return FriendResponse::Refused;
        };
        let member = slot.ctx.is_member_device(&by);
        if member {
            let inserted = slot.ctx.insert_encoded(&entries[..taken]);
            self.refused_entries(&inserted.refused);
            self.after_group_change(group_id, inserted.new, Some(peer));
        }
        let left = declined.is_some_and(|hash| self.on_declined(group_id, by, hash, entries));
        if member || left {
            FriendResponse::Ack
        } else {
            FriendResponse::Refused
        }
    }

    /// A group offered by the friend `by` on `peer`, judged: shown to the person if it has
    /// both of them as members and `by` is still a friend. A group that runs here by now
    /// takes the entries as if the offer came now; they were paid for when it came.
    pub(super) fn group_offer_judged(
        &mut self,
        peer: PeerId,
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
        declined: Option<EntryHash>,
        log: GroupLog,
    ) {
        let group_id = log.group_id();
        if self.groups.contains_key(&group_id) {
            let taken = entries.len();
            self.offer_to_running(peer, by, group_id, &entries, taken, declined);
            return;
        }
        let friend = self
            .friends
            .as_ref()
            .is_some_and(|f| f.book.friends.contains_key(&by));
        let state = log.state();
        if !friend || !state.is_member(&self.identity.public_key()) || !state.is_member(&by) {
            return;
        }
        // Offered to the person; the application runs it once they accept. Told even when
        // the application fell far behind meanwhile: the friend was told it was taken and
        // offers it again only at its next connection. The offers judged are few, and none
        // is taken while the application is far behind (see `on_friend_request`); one of
        // the same group waiting gives way (see events.rs).
        self.emit(NodeEvent::GroupOffered { log, by });
    }
}

#[cfg(test)]
mod offer_tests {
    use std::time::{Duration, Instant};

    use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
    use fogata_friends::FriendBook;

    use super::{MAX_OFFERED_WAITING, OfferPart};
    use crate::node::friends::FriendContext;

    fn context() -> FriendContext {
        let me = DeviceIdentity::generate().unwrap();
        FriendContext::new(FriendBook::new(&me, "Ana", 1).unwrap())
    }

    fn friend() -> DevicePublicKey {
        DeviceIdentity::generate().unwrap().public_key()
    }

    /// A friend added to a group that went up a level, whose copy does not judge at it, is
    /// not sent the group (it would refuse it from the raise on); its manager is told once,
    /// and the friend gets the group once it has updated.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_friend_below_the_group_s_level_is_not_sent_it_and_its_manager_is_told() {
        use std::{collections::HashSet, sync::Arc};

        use fogata_state::{GroupLog, Level};
        use libp2p::PeerId;

        use crate::{
            NodeConfig, NodeEvent, caps,
            group::GroupSetup,
            node::{start, tests::inspect},
        };
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = friend();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.raise_level(&ana, Level::Three, 2).unwrap();
        log.add(&ana, bea, "Bea", 3).unwrap();
        let group = log.group_id();
        let book = FriendBook::new(&ana, "Ana", 1).unwrap();
        let (node, mut events) = start(NodeConfig {
            identity: ana,
            groups: vec![GroupSetup::Member(log)],
            friends: Some(book),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let (before, after) = inspect(&node, move |n| {
            let peer = PeerId::random();
            n.friends.as_mut().unwrap().online.insert(peer, bea);
            let shared = |n: &mut super::Node, level: u32| {
                crate::locked(&n.advertised)
                    .insert(peer, HashSet::from([caps::caps_protocol(level)]));
                n.share_group();
                n.share_group();
                let f = n.friends.as_ref().unwrap();
                f.group_shared.contains(&(peer, group))
            };
            (shared(n, 2), shared(n, caps::CAPS_LEVEL))
        })
        .await;
        assert!(!before, "not sent to the previous version");
        assert!(after, "sent once updated");
        let mut told = 0;
        while let Ok(event) = events.try_recv() {
            if let NodeEvent::MustUpdateToJoin {
                group: g,
                device,
                name,
            } = event
            {
                assert_eq!((g, device, name.as_str()), (group, bea, "Bea"));
                told += 1;
            }
        }
        assert_eq!(told, 1);
    }

    /// The first `parts` parts, of 1 MiB each, of a log of 64 parts from `by`; how many
    /// were kept.
    fn offer(f: &mut FriendContext, by: DevicePublicKey, parts: u32, now: Instant) -> usize {
        let group = GroupId::from_bytes(&[3; 16]).unwrap();
        (0..parts)
            .filter(|index| {
                matches!(
                    f.take_offered_part(by, group, *index, 64, vec![vec![0; 1 << 20]], now),
                    OfferPart::Kept
                )
            })
            .count()
    }

    #[test]
    fn friends_that_never_finish_their_offers_keep_no_one_else_s_out() {
        let start = Instant::now();
        let mut f = context();
        // Four friends hold as much as offers may in all, and send nothing more.
        for _ in 0..4 {
            assert_eq!(offer(&mut f, friend(), 4, start), 4);
        }
        assert!(f.offered_bytes() <= MAX_OFFERED_WAITING);
        // Another friend's offer, smaller, still gets in at once.
        assert_eq!(offer(&mut f, friend(), 1, start), 1);
        assert!(f.offered_bytes() <= MAX_OFFERED_WAITING);
        // A while later the offers that stopped are gone.
        let later = start + Duration::from_secs(3 * 60);
        f.forget_stale_offers(later);
        assert_eq!(f.offered_bytes(), 0);
        // And one as large as they were gets in.
        assert_eq!(offer(&mut f, friend(), 4, later), 4);
    }

    #[test]
    fn an_offer_kept_alive_part_after_part_still_ends() {
        let start = Instant::now();
        let mut f = context();
        let slow = friend();
        let group = GroupId::from_bytes(&[3; 16]).unwrap();
        for i in 0..20u32 {
            let now = start + Duration::from_secs(60 * u64::from(i));
            f.take_offered_part(slow, group, i, 64, vec![vec![0; 1024]], now);
        }
        f.forget_stale_offers(start + Duration::from_secs(25 * 60));
        assert_eq!(f.offered_bytes(), 0);
    }

    #[test]
    fn a_part_sent_again_counts_once() {
        let now = Instant::now();
        let mut f = context();
        let by = friend();
        let group = GroupId::from_bytes(&[3; 16]).unwrap();
        for _ in 0..10 {
            let kept = f.take_offered_part(by, group, 0, 2, vec![vec![0; 1 << 20]], now);
            assert!(matches!(kept, OfferPart::Kept));
        }
        assert_eq!(f.offered_bytes(), 1 << 20);
        let whole = f.take_offered_part(by, group, 1, 2, vec![vec![1; 10]], now);
        assert!(matches!(whole, OfferPart::Whole(e) if e.len() == 2));
    }
}
