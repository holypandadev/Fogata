//! Moving a group to a new log, from level five (see [`Moved`]): how full a log is, the new
//! log the first member writes for a move, and what a member checks of it before following.
//!
//! A log never loses an entry, and every release takes a group from a friend or a joining
//! device only up to a limit, so a log grown past it stays out of reach for good. Level five
//! holds a log under a ceiling ([`crate::budget`]); a full one starts small again on a new
//! log, which the first member writes with entries of the kinds every release knows: the
//! group as it is where the move is signed, with new keys and no history.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use fogata_crypto::{DeviceIdentity, DevicePublicKey};

use super::{GroupLog, StateError, Work};
use crate::{
    entry::{Body, Entry, EntryHash, Moved, VoiceRoomId},
    state::{GroupState, Level},
};

/// How full a log is: what its entries that count hold, all authors together, and the most
/// they may from level five (see [`crate::budget`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogFill {
    pub entries: u32,
    pub bytes: usize,
    pub max_entries: u32,
    pub max_bytes: usize,
}

impl LogFill {
    /// How much of the ceiling the log holds, from 0 to 1 (or past it, for a log that grew
    /// before its group was raised to level five): the larger of entries and bytes.
    pub fn share(&self) -> f64 {
        let entries = f64::from(self.entries) / f64::from(self.max_entries.max(1));
        let bytes = self.bytes as f64 / self.max_bytes.max(1) as f64;
        entries.max(bytes)
    }

    /// Whether the log holds three quarters of its ceiling or more: the first member is
    /// offered to move it then, before it is full.
    pub fn nearly_full(&self) -> bool {
        self.share() >= 0.75
    }

    /// Whether the log is full: nothing but removals and leaves, `Level` entries and a move
    /// still applies there from level five.
    pub fn full(&self) -> bool {
        self.share() >= 1.0
    }
}

/// Who entries written apart from a move take out of the group it saw, or bring in (see
/// [`GroupLog::apart_from_move`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApartFromMove {
    pub removed: Vec<DevicePublicKey>,
    pub added: Vec<DevicePublicKey>,
}

/// Why a new log is not the group a move says it is.
pub const NOT_THE_SAME_GROUP: &str = "the new log is not the group as it was";

impl GroupLog {
    /// How full this log is (see [`LogFill`]).
    pub fn fill(&self) -> LogFill {
        let (total, ceiling) = self.full().budget.fill();
        LogFill {
            entries: total.entries,
            bytes: total.bytes,
            max_entries: ceiling.entries,
            max_bytes: ceiling.bytes,
        }
    }

    /// What each member's entries that the log keeps hold, most first: who filled it.
    pub fn writers(&self) -> Vec<(DevicePublicKey, u32, usize)> {
        let mut by: BTreeMap<DevicePublicKey, (u32, usize)> = BTreeMap::new();
        for (hash, entry) in self.entries.iter() {
            let had = by.entry(entry.author).or_default();
            had.0 += 1;
            had.1 += self.encoded_len(hash);
        }
        let mut out: Vec<_> = by.into_iter().map(|(k, (n, b))| (k, n, b)).collect();
        out.sort_by_key(|(k, n, b)| (std::cmp::Reverse(*b), std::cmp::Reverse(*n), *k));
        out
    }

    /// The move that applies here, and its entry: the group lives on in the log it names,
    /// and nothing applies here after it.
    pub fn moved(&self) -> Option<(EntryHash, Moved)> {
        self.settled().0.moved
    }

    /// Makes the ceiling of this log smaller, for tests of what happens at it.
    #[cfg(test)]
    pub(crate) fn set_ceiling(&mut self, entries: u32, bytes: usize) {
        self.ceiling = crate::budget::RunRoom { entries, bytes };
        self.work_out_again();
        self.cache
            .views
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// The log made of the entries of this one in `keep`, taken as this log judged them
    /// (`keep` holds every entry before each of its own, all judged): what a log of just
    /// those settles, room rules too.
    fn made_of(&self, keep: &BTreeSet<EntryHash>) -> GroupLog {
        let mut log = GroupLog::empty(self.group_id);
        log.run_room = self.run_room;
        log.ceiling = self.ceiling;
        log.cache
            .judged
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend(
                self.cache
                    .judged
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .iter()
                    .filter(|(h, _)| keep.contains(*h))
                    .map(|(h, s)| (*h, *s)),
            );
        let judged: HashSet<EntryHash> = keep.iter().copied().collect();
        let entries: Vec<Entry> = (self.entries_in_order().into_iter())
            .filter(|e| keep.contains(&e.hash()))
            .cloned()
            .collect();
        log.insert_batch(
            entries,
            Work {
                judged,
                signed: true,
                ..Work::unbounded()
            },
        );
        log
    }

    /// The new log the first member writes to move this group (see [`Moved`]): a new group
    /// that starts as this one is where the move is signed (on [`Self::heads`]), with entries of the kinds every release knows. The
    /// first member creates it, raises it to this version's level, sets who manages, the
    /// icon, picture and rooms, adds every other member in ladder order with its name (which
    /// gives each the new key), sets the limits, and writes again the room rules in force:
    /// who may enter each room, and the time outs not run out at `now_ms`. Open invites do
    /// not move: managers make new ones.
    pub fn new_log_for_move(
        &self,
        identity: &DeviceIdentity,
        now_ms: u64,
    ) -> Result<GroupLog, StateError> {
        let was = self.group_on(&self.heads());
        let me = identity.public_key();
        if was.log_level < Level::Five {
            return Err(StateError::NotAllowed("a group moves from level five"));
        }
        if was.ladder.first() != Some(&me) {
            return Err(StateError::NotAllowed(crate::state::MOVED_NOT_FIRST));
        }
        if was.moved.is_some() {
            return Err(StateError::NotAllowed(crate::state::MOVED_AWAY));
        }
        let mut log = GroupLog::create(identity, &was.name, now_ms)?;
        log.raise_level(identity, Level::CURRENT, now_ms)?;
        if let Some(first) = was.managers.first() {
            log.append(identity, Body::SetManagers { first: Some(first) }, now_ms)?;
        }
        if was.icon != log.state().icon {
            log.append(identity, Body::SetIcon { icon: was.icon }, now_ms)?;
        }
        if let Some(picture) = &was.picture {
            let body = Body::SetPicture {
                picture: Some(picture.clone()),
            };
            log.append(identity, body, now_ms)?;
        }
        for room in &was.rooms {
            if !room.name.is_empty() || log.state().rooms.iter().all(|r| r.id != room.id) {
                let body = Body::SetRoom {
                    id: room.id,
                    name: room.name.clone(),
                };
                log.append(identity, body, now_ms)?;
            }
        }
        for device in was.ladder.iter().skip(1) {
            log.add(identity, *device, &was.members[device].name, now_ms)?;
        }
        if (was.max_members, was.max_in_room) != (log.state().max_members, log.state().max_in_room)
        {
            let limits = Body::SetLimits {
                members: was.max_members,
                room: was.max_in_room,
            };
            log.append(identity, limits, now_ms)?;
        }
        // A rule written by a member lower down lets in itself and those above it too: the
        // first member, above everyone, names them.
        for (room, only) in &was.rules.only {
            let mut members = only.clone();
            members.extend(was.rules.always_in.get(room).into_iter().flatten().copied());
            members.remove(&me);
            log.set_room_access(identity, *room, Some(members.into_iter().collect()), now_ms)?;
        }
        for ((room, member), out) in &was.rules.kept_out {
            if out.until_ms.is_none_or(|until| now_ms < until) && was.is_member(member) {
                log.keep_out(identity, *room, *member, out.until_ms, now_ms)?;
            }
        }
        same_group(&was, &log.state(), now_ms)
            .map_err(|_| StateError::NotAllowed(NOT_THE_SAME_GROUP))?;
        Ok(log)
    }

    /// Moves the group to `new`, the log [`Self::new_log_for_move`] wrote: writes the
    /// [`Moved`] naming it, signed on every entry here, so on the group `new` starts as.
    /// The first member only, at level five; nothing applies here after it.
    pub fn move_to(
        &mut self,
        identity: &DeviceIdentity,
        new: &GroupLog,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let [setup] = new.heads()[..] else {
            return Err(StateError::NotAllowed("a new log starts with one head"));
        };
        let start = new.made_of(&new.before_and(&setup));
        let at = start.entries[&setup].time_ms;
        let heads = self.heads();
        same_group(&self.group_on(&heads), &start.state(), at)
            .map_err(|_| StateError::NotAllowed(NOT_THE_SAME_GROUP))?;
        let body = Moved {
            to: new.group_id(),
            setup,
        }
        .body();
        self.append_on(identity, heads, body, now_ms)
    }

    /// Whether `new` is the group this one moved to: the log the move names, whose start
    /// (the entries before its `setup` and that one), written by the author of the move
    /// alone, gives the group as the move saw it (see [`same_group`]): what was written
    /// apart from the move changes nothing of it, wherever the order puts it, so a member
    /// that followed never has to go back. Time outs as at the start's last entry. Nothing
    /// but keys and history may differ: a member follows only to such a log.
    pub fn follows(&self, new: &GroupLog) -> Result<(), &'static str> {
        let (hash, moved) = self.moved().ok_or("the group did not move")?;
        if new.group_id() != moved.to {
            return Err("not the group the move names");
        }
        if !new.entries.contains_key(&moved.setup) || new.unjudged.contains(&moved.setup) {
            return Err("the new log lacks its start");
        }
        let start = new.made_of(&new.before_and(&moved.setup));
        let at = start.entries[&moved.setup].time_ms;
        let mover = self.entries[&hash].author;
        if start.entries.values().any(|e| e.author != mover) {
            return Err(NOT_THE_SAME_GROUP);
        }
        same_group(&self.group_on(&self.entries[&hash].parents), &start.state(), at)
    }

    /// What entries written apart from the move, kept and part of a run (so judged on what
    /// their authors saw), change of who is in the group the move saw: the new group starts
    /// as that group, and the first member writes these again there (nothing where the
    /// group did not move). Room rules and other changes written apart from a move are lost
    /// with the old log, as after any leave.
    pub fn apart_from_move(&self) -> ApartFromMove {
        let mut apart = ApartFromMove::default();
        let Some((hash, _)) = self.moved() else {
            return apart;
        };
        let saw = self.group_on(&self.entries[&hash].parents);
        let before = self.ancestors(&hash);
        let full = self.full();
        let (mut removed, mut added) = (BTreeSet::new(), BTreeSet::new());
        for h in &full.order.hashes {
            if *h == hash || before.contains(h) || full.budget.over.contains(h) {
                continue;
            }
            let entry = &self.entries[h];
            match &entry.body {
                Body::Remove { member, .. } if saw.is_member(member) => {
                    removed.insert(*member);
                }
                Body::Leave if saw.is_member(&entry.author) => {
                    removed.insert(entry.author);
                }
                Body::Add { member, .. } if !saw.is_member(member) => {
                    added.insert(*member);
                }
                _ => {}
            }
        }
        added.retain(|m| !removed.contains(m));
        apart.removed = removed.into_iter().collect();
        apart.added = added.into_iter().collect();
        apart
    }

    /// The group, room rules too, as the entries `parents` and every entry before them
    /// make it: what an entry signed on them saw.
    fn group_on(&self, parents: &[EntryHash]) -> GroupState {
        let mut keep = self.ancestors_of(parents);
        keep.extend(parents.iter().copied());
        self.made_of(&keep).state()
    }

    /// Entry `hash` and every entry before it.
    fn before_and(&self, hash: &EntryHash) -> BTreeSet<EntryHash> {
        let mut all = self.ancestors(hash);
        all.insert(*hash);
        all
    }
}

/// Whether `new` is the group `old` is, for a member: the same ladder, members and names,
/// who manages, name, icon, picture, limits, rooms and level, and who may enter each room at
/// `now_ms`, time outs and their ends included. Keys and history differ.
pub fn same_group(old: &GroupState, new: &GroupState, now_ms: u64) -> Result<(), &'static str> {
    let same = old.ladder == new.ladder
        && old.members == new.members
        && old.managers == new.managers
        && old.name == new.name
        && old.icon == new.icon
        && old.picture == new.picture
        && (old.max_members, old.max_in_room) == (new.max_members, new.max_in_room)
        && old.rooms == new.rooms
        && old.level <= new.level
        && entry_to_rooms(old, now_ms) == entry_to_rooms(new, now_ms);
    if same {
        Ok(())
    } else {
        Err(NOT_THE_SAME_GROUP)
    }
}

/// Who may enter each room at `now_ms`: for each room and member, whether the room is
/// one it may enter, and until when a time out in force keeps it out.
fn entry_to_rooms(
    state: &GroupState,
    now_ms: u64,
) -> Vec<(VoiceRoomId, DevicePublicKey, bool, Option<Option<u64>>)> {
    let mut out = Vec::new();
    for room in &state.rooms {
        let only = state.rules.only.get(&room.id);
        let always_in = state.rules.always_in.get(&room.id);
        for member in &state.ladder {
            let chosen = only.is_none_or(|o| o.contains(member))
                || always_in.is_some_and(|a| a.contains(member));
            let timed_out = (state.rules.kept_out.get(&(room.id, *member)))
                .filter(|k| k.until_ms.is_none_or(|until| now_ms < until))
                .map(|k| k.until_ms);
            out.push((room.id, *member, chosen, timed_out));
        }
    }
    out
}
