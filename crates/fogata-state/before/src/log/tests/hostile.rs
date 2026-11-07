//! Members that write every kind of entry on whatever parents they like, to make other
//! devices keep what they could never make.

use fogata_crypto::Sealed;

use super::*;
use crate::{
    budget::{MAX_RUN_BYTES, MAX_RUN_ENTRIES},
    entry::{Changes, EpochLink, SEALED_SECRET_LEN, payload_digest},
};

/// Every entry written in a [`LadderWorld`], by honest and hostile members, kept or not.
struct Hostile {
    world: LadderWorld,
    written: Vec<Entry>,
    /// Entries handed to a copy only later, so that some come before their parents.
    late: Vec<(usize, Entry)>,
}

impl Hostile {
    fn new(seed: u64) -> Self {
        let mut world = LadderWorld::new(seed, 6);
        if seed % 2 == 1 {
            world = world.with_run_room(SMALL_RUN);
        }
        let written = world.logs[0]
            .entries_in_order()
            .into_iter()
            .cloned()
            .collect();
        Self {
            world,
            written,
            late: Vec::new(),
        }
    }

    /// What one copy or another has, as an honest member would sign on it, or older
    /// entries, or a few at random.
    fn parents(&mut self, k: usize) -> Vec<EntryHash> {
        let all: Vec<EntryHash> = self.world.logs[k].hashes().copied().collect();
        let mut parents = match self.world.below(4) {
            0 => self.world.logs[k].heads(),
            1 => vec![all[self.world.below(all.len())]],
            2 => (0..1 + self.world.below(3))
                .map(|_| all[self.world.below(all.len())])
                .collect(),
            _ => {
                let mut p = self.world.logs[k].heads();
                p.push(all[self.world.below(all.len())]);
                p
            }
        };
        parents.sort_unstable();
        parents.dedup();
        parents
    }

    /// Device `bad` writes an entry of any kind, on parents of its choosing, and hands it
    /// to a few copies, now or later.
    fn hostile_step(&mut self, bad: usize) {
        self.world.time += 1 + self.world.below(1000) as u64;
        let time = self.world.time;
        let n = self.world.devices.len();
        let k = self.world.below(n);
        let mut parents = self.parents(k);
        // Now and then on some of its own entries that are part of no run (past it, or of a
        // kind that never applies), which makes every copy keep them if it is kept.
        if self.world.below(4) == 0 {
            let log = &self.world.logs[k];
            let me = self.world.devices[bad].key();
            let live = &log.full().budget.live;
            let mut own: Vec<EntryHash> = log
                .ordered()
                .into_iter()
                .filter(|h| !live.contains(h) && log.entries[h].author == me)
                .collect();
            own.reverse();
            own.truncate(1 + self.world.below(3));
            if !own.is_empty() {
                parents = own;
                parents.sort_unstable();
            }
        }
        // Every choice made first, then what the copy has.
        let mut roll: Vec<usize> = (0..12).map(|_| self.world.below(1 << 20)).collect();
        let mut pick = |n: usize| roll.pop().unwrap() % n;
        let other = self.world.devices[pick(n)].key();
        let room = if pick(2) == 0 { FIRST_ROOM } else { [1; 8] };
        let box_len = if pick(6) == 0 {
            4_000
        } else {
            SEALED_SECRET_LEN
        };
        let on_commit = pick(2) == 0;
        let kind = pick(25);
        let (picture_len, managers, limits, room_id, ticket_id, unknown_len) = (
            1 + pick(MAX_PICTURE_BYTES),
            [None, Some(1), Some(3)][pick(3)],
            (2 + pick(19) as u8, 2 + pick(7) as u8),
            [2 + pick(3) as u8; 8],
            [pick(4) as u8; 16],
            1 + pick(2_000),
        );
        let log = &self.world.logs[k];
        let group = log.group_id();
        let state = log.state();
        let members: Vec<DevicePublicKey> = state.members.keys().copied().collect();
        let epoch = state.current_epoch().map_or([0; 16], |e| e.id);
        let sealed_to = |to: &DevicePublicKey, len: usize| {
            let mut s = Sealed::seal(to, &secret_context(&group, &epoch), &[7; 32]).unwrap();
            s.ciphertext.resize(len, 0);
            s
        };
        let rotation = || Rotation::new(&group, members.iter().copied()).unwrap().0;
        let first = &self.world.devices[0];
        // Now and then a ticket of an invite in the log, used under a key of its own: a
        // join nobody let in, which must take nobody's place nor ticket.
        let invited = log.entries().find_map(|e| match &e.body {
            Body::Invite { ticket, .. } if on_commit => Some(ticket.clone()),
            _ => None,
        });
        let throwaway = invited.as_ref().map(|_| Device::new());
        let ticket =
            invited.unwrap_or_else(|| Ticket::issue(&first.id, group, time + 50_000).unwrap());
        let commits: Vec<EntryHash> = log
            .entries()
            .filter(|e| matches!(e.body, Body::Commit { .. } | Body::Resync { .. }))
            .map(Entry::hash)
            .collect();
        let link = match commits.first() {
            Some(prev) if on_commit => EpochLink {
                epoch: 1,
                prev: Some(*prev),
            },
            _ => EpochLink {
                epoch: 0,
                prev: None,
            },
        };
        let body = match kind {
            0 => Body::Rename {
                name: format!("h{time}"),
            },
            1 => Body::SetIcon { icon: 3 },
            2 => Body::SetPicture {
                picture: Some(vec![1; picture_len]),
            },
            3 => Body::Promote { member: other },
            4 => Body::Demote { member: other },
            5 => Body::SetManagers { first: managers },
            6 => Body::Add {
                member: Device::new().key(),
                name: "x".into(),
                epoch,
                secret: sealed_to(&other, box_len),
            },
            7 => Body::Join {
                ticket,
                name: "j".into(),
            },
            8 => Body::Welcome {
                member: other,
                epoch,
                secret: sealed_to(&other, box_len),
            },
            9 => Body::Remove {
                member: other,
                rotation: rotation(),
            },
            10 => Body::Leave,
            11 => Body::Rotate {
                rotation: rotation(),
            },
            12 => Body::Invite {
                ticket: Ticket::issue(&self.world.devices[bad].id, group, time).unwrap(),
                token: [9; 16],
            },
            13 => Body::SetLimits {
                members: limits.0,
                room: limits.1,
            },
            14 => Body::SetRoom {
                id: room_id,
                name: "r".into(),
            },
            15 => Body::RemoveRoom { id: [1; 8] },
            16 => Body::Revoke { ticket_id },
            17 => Body::RoomAccess {
                room,
                members: Some(vec![other]),
            },
            18 => Body::KeepOut {
                room,
                member: other,
                until_ms: Some(time + 10_000),
            },
            19 => Body::LetBack {
                room,
                member: other,
            },
            20 => Body::MlsKeyPackage {
                key_package: vec![5; 370],
            },
            21 => Body::Commit {
                link,
                changes: Changes::new(members.iter().copied().take(2), []),
                payload: payload_digest(&time.to_be_bytes()),
                welcome: Some(payload_digest(b"w")),
            },
            22 => Body::Resync {
                link,
                payload: payload_digest(&time.to_be_bytes()),
            },
            23 => Body::MlsJoin {
                ticket: Ticket::issue(&first.id, group, time + 50_000).unwrap(),
                name: "m".into(),
                key_package: vec![6; 370],
            },
            _ => Body::Unknown {
                kind: 200,
                bytes: vec![8; unknown_len],
            },
        };
        let signer = match (&body, &throwaway) {
            (Body::Join { .. }, Some(throwaway)) => &throwaway.id,
            _ => &self.world.devices[bad].id,
        };
        let entry = Entry::sign(signer, group, parents, time, body);
        self.written.push(entry.clone());
        if self.world.below(4) == 0 {
            let to = self.world.below(n);
            self.late.push((to, entry));
        } else {
            for _ in 0..1 + self.world.below(3) {
                let to = self.world.below(n);
                let _ = self.world.logs[to].insert(entry.clone());
            }
        }
        // Some of those held back arrive now, the newest first.
        while !self.late.is_empty() && self.world.below(3) == 0 {
            let (to, entry) = self.late.pop().unwrap();
            let _ = self.world.logs[to].insert(entry);
        }
    }

    /// Whatever the random steps gave: a manager of copy 0 with room in its run writes
    /// more renames than a run holds, each on the same entries, so that past its run some
    /// apply to nothing in the common order; and then a rename signed on all of them, which
    /// is refused. For a world where a device writes little in a run.
    fn go_past_a_run(&mut self) {
        let state = self.world.logs[0].state();
        let group = self.world.logs[0].group_id();
        let run = u64::from(self.world.logs[0].run_room.entries);
        let managers: Vec<usize> = (0..self.world.devices.len())
            .filter(|i| state.is_manager(&self.world.devices[*i].key()))
            .collect();
        for m in managers {
            let time = self.world.time + 1;
            let parents = self.world.logs[0].heads();
            let mut kept = Vec::new();
            for i in 0..=run + 1 {
                let entry = Entry::sign(
                    &self.world.devices[m].id,
                    group,
                    parents.clone(),
                    time + i,
                    Body::Rename {
                        name: format!("p{i}"),
                    },
                );
                self.written.push(entry.clone());
                match self.world.logs[0].insert(entry.clone()) {
                    Ok(true) => kept.push(entry.hash()),
                    // Its run is full here: another manager may have room.
                    Err(StateError::NotAllowed(PAST_RUN)) => break,
                    other => panic!("{other:?}"),
                }
            }
            if kept.is_empty() {
                continue;
            }
            kept.sort_unstable();
            let entry = Entry::sign(
                &self.world.devices[m].id,
                group,
                kept,
                time + run + 2,
                Body::Rename {
                    name: "past".into(),
                },
            );
            self.written.push(entry.clone());
            let refused = self.world.logs[0].insert(entry);
            assert_eq!(refused, Err(StateError::NotAllowed(PAST_RUN)));
            return;
        }
    }

    /// An honest step of the world, noting what it wrote.
    fn honest_step(&mut self) {
        let before: Vec<BTreeSet<EntryHash>> = self
            .world
            .logs
            .iter()
            .map(|l| l.hashes().copied().collect())
            .collect();
        self.world.step();
        for (log, had) in self.world.logs.iter().zip(&before) {
            for entry in log.entries() {
                if !had.contains(&entry.hash()) && !self.written.contains(entry) {
                    self.written.push(entry.clone());
                }
            }
        }
    }
}

/// Every MLS message checks, as far as the log can tell.
fn valid(_: &EntryHash) -> crate::chain::Verdict {
    crate::chain::Verdict::Valid
}

/// Whether `entry`, whose parents are in `log`, is one its author could make in its own
/// view, worked out from nothing: `log` is a copy that has worked out nothing yet.
fn could_make(log: &GroupLog, entry: &Entry) -> bool {
    log.judge_in(entry, &log.view_at(entry)).is_ok()
}

/// A copy of `log` that has worked out nothing.
fn from_nothing(log: &GroupLog) -> GroupLog {
    let mut fresh = log.clone();
    *fresh.cache = Cache::default();
    fresh
}

/// Hostile members write every kind of entry on parents of their choosing, handed to a few
/// copies at a time, some before their parents. Every copy ends with the same entries and
/// group and MLS chain; it keeps exactly the entries their authors could make in their own
/// view and the entries waiting for one that was refused; and copies that take every entry
/// ever written, in any order, one at a time or all at once, keep the same. Hostile members
/// also sign on their own entries that are part of no run. In odd seeds a device writes
/// little in a run, so entries go past their author's run, refused where they are judged or
/// applying to nothing in the common order, as the random steps make them and, so that
/// every run has some, as one member makes them at the end.
/// `FOGATA_LADDER_SEEDS` sets how many histories run (4 by default).
#[test]
fn every_copy_keeps_only_what_authors_could_make_whatever_came_first() {
    let seeds = std::env::var("FOGATA_LADDER_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4u64);
    let (mut refused_past_run, mut past_run_in_order) = (0, 0);
    for seed in 1..=seeds {
        let mut h = Hostile::new(seed);
        for i in 0..160 {
            if i % 3 == 0 {
                h.hostile_step(1 + i % 2);
            } else {
                h.honest_step();
            }
        }
        for (to, entry) in std::mem::take(&mut h.late) {
            let _ = h.world.logs[to].insert(entry);
        }
        if seed % 2 == 1 {
            h.go_past_a_run();
        }
        let n = h.world.logs.len();
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (h.world.logs[i].clone(), h.world.logs[j].clone());
                h.world.logs[i].insert_all(b.entries().cloned());
                h.world.logs[j].insert_all(a.entries().cloned());
            }
        }
        h.world.sync_all();
        let mut shuffled = h.written.clone();
        for k in 0..shuffled.len() {
            let at = h.world.below(shuffled.len());
            shuffled.swap(k, at);
        }
        let log = &h.world.logs[0];
        let kept: BTreeSet<EntryHash> = log.hashes().copied().collect();
        let state = log.state();
        let chain = log.mls_chain(valid);
        for (i, other) in h.world.logs.iter().enumerate() {
            let theirs: BTreeSet<EntryHash> = other.hashes().copied().collect();
            assert_eq!(theirs, kept, "seed {seed}: device {i} keeps others");
            let s = other.state();
            assert_eq!(s.applied, state.applied, "seed {seed}: device {i}");
            assert_eq!(s.ladder, state.ladder, "seed {seed}: device {i}");
            assert_eq!(s.rules, state.rules, "seed {seed}: device {i}");
            assert_eq!(other.ordered(), log.ordered(), "seed {seed}: device {i}");
            assert_eq!(other.mls_chain(valid), chain, "seed {seed}: device {i}");
        }
        // What is placed, its author could make; what is not kept, it could not, or it
        // follows an entry that was not kept.
        let placed: BTreeSet<EntryHash> = log.ordered().into_iter().collect();
        let fresh = from_nothing(log);
        for entry in log.entries() {
            if placed.contains(&entry.hash()) {
                assert!(could_make(&fresh, entry), "seed {seed}: {:?}", entry.body);
            } else {
                assert!(
                    entry.parents.iter().any(|p| !placed.contains(p)),
                    "seed {seed}: waits for nothing"
                );
            }
        }
        let mut refused = 0;
        for entry in &h.written {
            if kept.contains(&entry.hash()) {
                continue;
            }
            refused += 1;
            if entry.parents.iter().all(|p| placed.contains(p)) {
                assert!(
                    !could_make(&fresh, entry),
                    "seed {seed}: lost {:?}",
                    entry.body
                );
                if fresh.judge_in(entry, &fresh.view_at(entry)) == Err(PAST_RUN) {
                    refused_past_run += 1;
                }
            }
        }
        assert!(refused > 0, "seed {seed}: nothing was refused");
        past_run_in_order += state
            .rejected
            .iter()
            .filter(|(_, r)| *r == PAST_RUN)
            .count();
        // Every entry ever written, taken in other orders: the same entries and group.
        let mut orders: Vec<Vec<Entry>> = vec![h.written.clone()];
        orders.push(h.written.iter().rev().cloned().collect());
        orders.push(shuffled);
        for (o, order) in orders.into_iter().enumerate() {
            let mut one_by_one = h.world.empty_log();
            // One at a time, each sent again until nothing new comes, as syncs do: one from
            // a device not named yet is taken once the entry naming it is.
            loop {
                let mut more = false;
                for entry in &order {
                    more |= one_by_one.insert(entry.clone()).unwrap_or(false);
                }
                if !more {
                    break;
                }
            }
            let mut at_once = h.world.empty_log();
            at_once.insert_all(order);
            for again in [one_by_one, at_once] {
                let theirs: BTreeSet<EntryHash> = again.hashes().copied().collect();
                assert!(theirs == kept, "seed {seed}: order {o} keeps other entries");
                assert_eq!(again.state().applied, state.applied, "seed {seed}: {o}");
                assert_eq!(again.ordered(), log.ordered(), "seed {seed}: {o}");
                assert_eq!(again.mls_chain(valid), chain, "seed {seed}: {o}");
            }
        }
        check_every_step(log, &state);
        same_as_worked_out_again(log, &h.world.devices[0].id, &format!("seed {seed}"));
    }
    eprintln!(
        "{refused_past_run} refused past their run, {past_run_in_order} past it in the order"
    );
    if seeds >= 4 {
        assert!(refused_past_run > 0 && past_run_in_order > 0);
    }
}

/// Bytes `log` keeps of entries by `author`, as encoded.
fn bytes_of(log: &GroupLog, author: &DevicePublicKey) -> (usize, usize) {
    let spare: BTreeSet<EntryHash> = log.spare().into_iter().collect();
    let mut out = (0, 0);
    for (hash, entry) in &log.entries {
        if entry.author != *author {
            continue;
        }
        let len = entry.encode().len();
        if spare.contains(hash) {
            out.0 += len;
        } else {
            out.1 += len;
        }
    }
    out
}

/// Marta, last on the ladder and managing nothing, joined with an invite and writes as
/// much as she can of every kind of entry, on what everyone signs on and on older entries,
/// boxes padded where an entry has boxes. Another member keeps of hers only spare entries,
/// within their room, and the joins and leaves she could make where she signed them:
/// a few hundred bytes each, never an entry made long.
#[test]
fn a_member_writing_every_kind_fills_nobody_s_disk() {
    let (ana, bea, leo, marta) = (Device::new(), Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
    log.append(&ana.id, Body::SetManagers { first: Some(2) }, 4)
        .unwrap();
    let (ticket, _) = log.invite(&ana.id, 1_000_000, 5).unwrap();
    join(&mut log, &marta, ticket.clone(), 6);
    log.welcome(&ana.id, marta.key(), 7).unwrap();
    log.append(&bea.id, Body::Rename { name: "h".into() }, 8)
        .unwrap();
    let group = log.group_id();
    let honest = log.clone();
    let others = [ana.key(), bea.key(), leo.key()];
    let old: Vec<EntryHash> = honest.ordered();
    let state = honest.state();
    let epoch = state.current_epoch().unwrap().id;
    let mut written: Vec<Entry> = Vec::new();
    let mut rng = 7u64;
    let mut below = |n: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n as u64) as usize
    };
    let padded = |to: &DevicePublicKey| {
        let mut s = Sealed::seal(to, &secret_context(&group, &epoch), &[7; 32]).unwrap();
        s.ciphertext.resize(60_000, 0);
        s
    };
    let fat_rotation = || {
        let mut r = Rotation::new(&group, state.members.keys().copied())
            .unwrap()
            .0;
        for _ in 0..400 {
            let d = Device::new().key();
            r.sealed
                .push((d, Sealed::seal(&d, b"x", &[0; 32]).unwrap()));
        }
        r.sealed.sort_by_key(|(k, _)| *k);
        r
    };
    const EACH: u64 = 40;
    for i in 0..EACH {
        let time = 100 + i;
        let other = others[below(3)];
        let bodies = vec![
            Body::Rename {
                name: format!("m{i}"),
            },
            Body::SetIcon { icon: 2 },
            Body::SetPicture {
                picture: Some(vec![1; MAX_PICTURE_BYTES]),
            },
            Body::Promote { member: other },
            Body::Demote { member: other },
            Body::SetManagers { first: None },
            Body::Add {
                member: Device::new().key(),
                name: "x".into(),
                epoch,
                secret: padded(&other),
            },
            Body::Join {
                ticket: ticket.clone(),
                name: "Marta".into(),
            },
            Body::Welcome {
                member: other,
                epoch,
                secret: padded(&other),
            },
            Body::Remove {
                member: other,
                rotation: fat_rotation(),
            },
            Body::Rotate {
                rotation: fat_rotation(),
            },
            Body::Invite {
                ticket: Ticket::issue(&marta.id, group, time).unwrap(),
                token: [1; 16],
            },
            Body::SetLimits {
                members: 20,
                room: 8,
            },
            Body::SetRoom {
                id: [3; 8],
                name: "r".repeat(40),
            },
            Body::RemoveRoom { id: FIRST_ROOM },
            Body::Revoke { ticket_id: [2; 16] },
            Body::RoomAccess {
                room: FIRST_ROOM,
                members: Some(vec![marta.key(); 1_500]),
            },
            Body::KeepOut {
                room: FIRST_ROOM,
                member: other,
                until_ms: None,
            },
            Body::LetBack {
                room: FIRST_ROOM,
                member: other,
            },
            Body::MlsKeyPackage {
                key_package: vec![5; crate::entry::MAX_KEY_PACKAGE_BYTES],
            },
            Body::MlsJoin {
                ticket: ticket.clone(),
                name: "Marta".into(),
                key_package: vec![5; crate::entry::MAX_KEY_PACKAGE_BYTES],
            },
            Body::Commit {
                link: EpochLink {
                    epoch: 0,
                    prev: None,
                },
                changes: Changes::new(others, []),
                payload: payload_digest(&time.to_be_bytes()),
                welcome: Some(payload_digest(b"w")),
            },
            Body::Resync {
                link: EpochLink {
                    epoch: 0,
                    prev: None,
                },
                payload: payload_digest(&time.to_be_bytes()),
            },
            Body::Unknown {
                kind: 200,
                bytes: vec![9; 60_000],
            },
        ];
        for body in bodies {
            let parents = if below(2) == 0 {
                honest.heads()
            } else {
                vec![old[below(old.len())]]
            };
            let entry = Entry::sign(&marta.id, group, parents, time, body);
            assert!(entry.encode().len() <= MAX_ENTRY_BYTES, "{:?}", entry.body);
            written.push(entry);
        }
    }
    let written_bytes: usize = written.iter().map(|e| e.encode().len()).sum();
    // Everything she wrote reaches another member, which trims as the node does.
    let mut other = honest.clone();
    other.insert_all(written.clone());
    other.trim();
    let (spare, rest) = bytes_of(&other, &marta.key());
    eprintln!("{written_bytes} bytes written, {spare} kept spare, {rest} kept otherwise");
    assert!(spare <= MAX_SPARE_BYTES_PER_AUTHOR, "{spare} spare bytes");
    let kept: Vec<&Entry> = other
        .entries()
        .filter(|e| e.author == marta.key() && honest.get(&e.hash()).is_none())
        .collect();
    let spare_now: BTreeSet<EntryHash> = other.spare().into_iter().collect();
    for entry in &kept {
        if spare_now.contains(&entry.hash()) {
            continue;
        }
        // What is not spare: a join she could sign where she was not in yet, a leave where
        // she was. Neither carries anything long.
        assert!(
            matches!(entry.body, Body::Join { .. } | Body::Leave),
            "{:?}",
            entry.body
        );
        assert!(entry.encode().len() < 512);
    }
    assert!(rest < EACH as usize * 512 * 2, "{rest} bytes kept");
    // Before, every one of them was kept: tens of megabytes.
    assert!(
        written_bytes > 10 * (spare + rest),
        "{written_bytes} written, {spare} + {rest} kept"
    );
    // The others are all still in, and their entries all there.
    let keys = |log: &GroupLog| log.state().members.into_keys().collect::<Vec<_>>();
    assert_eq!(keys(&other), keys(&honest));
    assert!(honest.hashes().all(|h| other.get(h).is_some()));
    // A copy that takes them the other way round, one at a time and again until nothing
    // new comes (those past the room for entries waiting for parents go meanwhile), keeps
    // the same.
    let mut again = honest.clone();
    loop {
        let mut more = false;
        for entry in written.iter().rev() {
            more |= again.insert(entry.clone()).unwrap_or(false);
        }
        if !more {
            break;
        }
    }
    again.trim();
    let needed = |log: &GroupLog| {
        let spare: BTreeSet<EntryHash> = log.spare().into_iter().collect();
        log.hashes()
            .copied()
            .filter(|h| !spare.contains(h))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(needed(&again), needed(&other));
}

/// A log an earlier version stored, with entries their authors could not make where they
/// signed them (it kept them all): read back, those go, on every device alike, whatever
/// part of it each had and in whatever order it reads it, and a copy that never had them
/// ends with the same log. An entry signed on one of them waits for good on every device:
/// the group then differs from the one the earlier version worked out, the same way on all.
#[test]
fn a_stored_log_reads_back_without_what_its_authors_could_not_make() {
    let (ana, bea, leo, sofia) = (Device::new(), Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
    log.append(&ana.id, Body::SetManagers { first: Some(1) }, 4)
        .unwrap();
    let clean = log.clone();
    let group = log.group_id();
    let epoch = log.state().current_epoch().unwrap().id;
    // What the earlier version kept: Bea, who does not manage, renames, invites, adds Sofía
    // with a box made long, keeps Ana out of a room, moves Ana down, and leaves on top of her
    // rename; Ana goes on, signing on what applied, as that version did.
    let mut honest = log.clone();
    honest
        .append(&ana.id, Body::Rename { name: "h".into() }, 20)
        .unwrap();
    honest.rotate(&ana.id, 21).unwrap();
    let mut stored = honest.clone();
    let mut junk = Vec::new();
    let mut sealed = Sealed::seal(&leo.key(), &secret_context(&group, &epoch), &[1; 32]).unwrap();
    sealed.ciphertext.resize(30_000, 0);
    for (i, body) in [
        Body::Rename { name: "x".into() },
        Body::Invite {
            ticket: Ticket::issue(&bea.id, group, 99).unwrap(),
            token: [3; 16],
        },
        Body::Add {
            member: sofia.key(),
            name: "Sofía".into(),
            epoch,
            secret: sealed,
        },
        Body::KeepOut {
            room: FIRST_ROOM,
            member: ana.key(),
            until_ms: None,
        },
        Body::Demote { member: ana.key() },
    ]
    .into_iter()
    .enumerate()
    {
        let entry = Entry::sign(&bea.id, group, log.heads(), 10 + i as u64, body);
        junk.push(entry.hash());
        stored.keep_unjudged(entry);
    }
    assert!(junk.iter().all(|h| stored.get(h).is_some()));
    // Her leave on her rename: every copy keeps it waiting for the rename, which none keeps.
    let leave = Entry::sign(&bea.id, group, vec![junk[0]], 30, Body::Leave);
    stored.keep_unjudged(leave.clone());
    // There, her leave applied, and Ana's rotation, sealed to her too, did not.
    assert!(!stored.state().is_member(&bea.key()));
    let entries: Vec<Entry> = stored.entries_in_order().into_iter().cloned().collect();
    // Read back in the order stored, the other way round, one at a time, and half of it
    // first with the rest from another copy later.
    let mut copies = Vec::new();
    let mut a = GroupLog::empty(group);
    a.insert_all(entries.clone());
    copies.push(a);
    let mut b = GroupLog::empty(group);
    b.insert_all(entries.iter().rev().cloned());
    copies.push(b);
    let mut c = GroupLog::empty(group);
    loop {
        let mut more = false;
        for entry in entries.iter().rev() {
            more |= c.insert(entry.clone()).unwrap_or(false);
        }
        if !more {
            break;
        }
    }
    copies.push(c);
    let mut d = GroupLog::empty(group);
    d.insert_all(entries.iter().take(entries.len() / 2).cloned());
    let first = copies[0].clone();
    d.insert_all(first.entries().cloned());
    copies.push(d);
    // A copy that never had them, written on by the same members.
    let mut e = clean.clone();
    e.insert_all(
        entries
            .iter()
            .filter(|x| !junk.contains(&x.hash()))
            .cloned(),
    );
    copies.push(e);
    let kept: BTreeSet<EntryHash> = copies[0].hashes().copied().collect();
    let state = copies[0].state();
    assert!(junk.iter().all(|h| !kept.contains(h)));
    assert_eq!(kept.len(), entries.len() - junk.len());
    assert_eq!(copies[0].waiting(), vec![leave.hash()]);
    for (i, copy) in copies.iter().enumerate() {
        let theirs: BTreeSet<EntryHash> = copy.hashes().copied().collect();
        assert_eq!(theirs, kept, "copy {i}");
        let s = copy.state();
        assert_eq!(s.applied, state.applied, "copy {i}");
        assert_eq!(s.ladder, state.ladder, "copy {i}");
        assert_eq!(s.name, "h");
        assert_eq!(s.epochs.len(), 2, "copy {i}");
        assert_eq!(copy.needed_heads(), copies[0].needed_heads(), "copy {i}");
        assert!(s.is_member(&bea.key()), "copy {i}");
    }
}

/// Entries from another device that each need the group worked out for their own parents
/// are judged at most so many a batch; the rest come again and are taken then, and the
/// log ends as if it had taken them all at once.
#[test]
fn a_batch_from_another_device_is_judged_a_part_at_a_time() {
    let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
    for i in 0..20 {
        log.append(
            &ana.id,
            Body::Rename {
                name: format!("n{i}"),
            },
            4 + i,
        )
        .unwrap();
    }
    let old = log.ordered();
    // Leo leaves again and again on entries from while he was in, each on another one.
    let batch: Vec<Entry> = (0..200u64)
        .map(|i| {
            let parent = old[3 + (i as usize) % (old.len() - 3)];
            Entry::sign(&leo.id, log.group_id(), vec![parent], 100 + i, Body::Leave)
        })
        .collect();
    let mut whole = log.clone();
    whole.insert_all(batch.clone());
    let mut parts = log.clone();
    let mut rounds = 0;
    loop {
        let new = parts.insert_some(batch.clone());
        if new.is_empty() {
            break;
        }
        assert!(new.len() <= MAX_VIEWS_PER_BATCH);
        rounds += 1;
    }
    assert!(rounds > 1);
    let hashes = |l: &GroupLog| l.hashes().copied().collect::<BTreeSet<_>>();
    assert_eq!(hashes(&parts), hashes(&whole));
    assert_eq!(parts.state().applied, whole.state().applied);
}

/// Ana first, Bea second and Leo last, all managing, with Bea's writing room cut to
/// `run` entries that count, on every copy: Bea has written nothing yet.
fn three_with_run(run: u32) -> (GroupLog, Device, Device, Device) {
    let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.set_run_room(run, RUN_ROOM.bytes);
    log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
    (log, ana, bea, leo)
}

/// Entries of `author` that `log` keeps placed and that count in a run, those past it
/// left out.
fn counted_of(log: &GroupLog, author: &DevicePublicKey) -> usize {
    let over = &log.full().budget.over;
    log.ordered()
        .iter()
        .filter(|h| !over.contains(*h) && log.entries[*h].author == *author)
        .filter(|h| {
            matches!(
                log.entries[*h].body,
                Body::Rename { .. } | Body::SetRoom { .. }
            )
        })
        .count()
}

/// Bea, a manager, renames as fast as a client that ignores every refusal can, each on
/// her last: every copy keeps the first that fit in her run and refuses the next, those
/// after it wait for good within their room, as after any entry refused. Her own copy
/// refuses to write past it. Once another member signs on what she wrote, she has a run
/// again; Ana, past her own run, still removes her, and Bea past hers still leaves.
#[test]
fn a_manager_writing_without_end_is_held_to_its_run() {
    const RUN: u32 = 16;
    let (log, ana, bea, leo) = three_with_run(RUN);
    let group = log.group_id();
    let mut written = Vec::new();
    let mut last = log.heads();
    for i in 0..200u64 {
        let entry = Entry::sign(
            &bea.id,
            group,
            last,
            10 + i,
            Body::Rename {
                name: format!("b{i}"),
            },
        );
        last = vec![entry.hash()];
        written.push(entry);
    }
    let mut leos = log.clone();
    leos.insert_all(written.clone());
    assert_eq!(counted_of(&leos, &bea.key()), RUN as usize);
    assert!(leos.get(&written[RUN as usize - 1].hash()).is_some());
    assert!(leos.get(&written[RUN as usize].hash()).is_none());
    assert!(leos.waiting().len() <= MAX_WAITING_PER_AUTHOR);
    assert_eq!(leos.state().name, format!("b{}", RUN - 1));
    // Her own copy refuses to write past her run.
    let mut beas = log.clone();
    for i in 0..RUN as u64 {
        beas.append(
            &bea.id,
            Body::Rename {
                name: format!("c{i}"),
            },
            300 + i,
        )
        .unwrap();
    }
    let past = beas.append(
        &bea.id,
        Body::Rename {
            name: "more".into(),
        },
        400,
    );
    assert_eq!(past.unwrap_err(), StateError::NotAllowed(PAST_RUN));
    // Leo signs on what she wrote: she writes a whole run again on top, and no more.
    leos.append(
        &leo.id,
        Body::SetRoom {
            id: [1; 8],
            name: "r".into(),
        },
        500,
    )
    .unwrap();
    let mut again = leos.clone();
    for i in 0..RUN as u64 {
        again
            .append(
                &bea.id,
                Body::Rename {
                    name: format!("d{i}"),
                },
                600 + i,
            )
            .unwrap();
    }
    assert!(
        again
            .append(&bea.id, Body::Rename { name: "x".into() }, 700)
            .is_err()
    );
    // Past her run she still leaves, and Ana, past hers, removes her.
    let mut left = again.clone();
    left.append(&bea.id, Body::Leave, 800).unwrap();
    assert!(!left.state().is_member(&bea.key()));
    let mut anas = again.clone();
    let mut i = 0;
    while anas
        .append(
            &ana.id,
            Body::Rename {
                name: format!("a{i}"),
            },
            900 + i,
        )
        .is_ok()
    {
        i += 1;
    }
    // Bea signed on her adds: Leo's add, Bea in before it, left Ana's run; Bea's own add
    // stays, since a device does not take out of a run the entry that brought it in.
    assert_eq!(i, u64::from(RUN) - 1);
    anas.remove(&ana.id, bea.key(), 2000).unwrap();
    assert!(!anas.state().is_member(&bea.key()));
    // Every copy that takes it all, in any order, has the same group.
    let mut back = GroupLog::empty(group);
    back.set_run_room(RUN, RUN_ROOM.bytes);
    let mut all: Vec<Entry> = anas.entries().cloned().collect();
    all.extend(written);
    all.reverse();
    back.insert_all(all);
    assert_eq!(back.state().applied, anas.state().applied);
    assert!(!back.state().is_member(&bea.key()));
}

/// The bytes of a run hold as its entries do: a manager rotating the keys of a group of
/// 20 without end writes about 3 KB each time, and goes past its run on the bytes.
#[test]
fn a_run_holds_so_many_bytes() {
    let ana = Device::new();
    let others: Vec<Device> = (0..19).map(|_| Device::new()).collect();
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    for (i, d) in others.iter().enumerate() {
        log.add(&ana.id, d.key(), "m", 2 + i as u64).unwrap();
    }
    log.set_run_room(1_000, 64 << 10);
    let mut bytes = 0;
    let mut written = 0;
    while let Ok(entry) = log.rotate(&ana.id, 100 + written) {
        bytes += entry.encode().len();
        written += 1;
    }
    // The adds count too, under 300 bytes each.
    assert!(
        bytes <= 64 << 10 && bytes + 19 * 300 > (64 << 10) - 3_200,
        "{bytes}"
    );
    assert!(written > 15, "{written}");
}

/// Bea writes entries signed on older ones, none seeing the others, so that every one fits
/// in her run where it is judged. In the common order only a run of them applies, and the
/// others are let go like any spare entry; an entry another member wrote long before on an
/// entry of hers ends none of them, and each entry of another member signed on some of them
/// ends only those. Copies that take them in any order and let them go keep the same.
#[test]
fn entries_signed_apart_past_a_run_are_let_go() {
    const RUN: u32 = 8;
    let (mut log, ana, bea, leo) = three_with_run(RUN);
    let group = log.group_id();
    // Before: Bea renamed once and Ana rotated on it, and Leo wrote a room.
    let renamed = log
        .append(&bea.id, Body::Rename { name: "b".into() }, 5)
        .unwrap();
    log.rotate(&ana.id, 6).unwrap();
    log.append(
        &leo.id,
        Body::SetRoom {
            id: [1; 8],
            name: "r".into(),
        },
        7,
    )
    .unwrap();
    let old: Vec<EntryHash> = log.ordered().into_iter().skip(3).collect();
    assert!(old.contains(&renamed.hash()));
    let mut rng = 3u64;
    let mut below = |n: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n as u64) as usize
    };
    let written: Vec<Entry> = (0..120u64)
        .map(|i| {
            let parent = old[below(old.len())];
            let body = if i % 3 == 0 {
                Body::SetRoom {
                    id: [2; 8],
                    name: format!("s{i}"),
                }
            } else {
                Body::Rename {
                    name: format!("o{i}"),
                }
            };
            Entry::sign(&bea.id, group, vec![parent], 100 + i, body)
        })
        .collect();
    let mut leos = log.clone();
    assert_eq!(leos.insert_all(written.clone()).len(), written.len());
    // Her rename from before, which Ana signed on, and a run of these apply; Ana's rotation
    // on that rename ends no run of these, though some go between them in the order. The
    // rest apply to nothing, and nothing new is signed on them.
    assert_eq!(counted_of(&leos, &bea.key()), 1 + RUN as usize);
    let heads: BTreeSet<EntryHash> = leos.heads().into_iter().collect();
    let over = leos.full().budget.over.clone();
    assert!(over.len() >= written.len() - RUN as usize);
    assert!(heads.iter().all(|h| !over.contains(h)));
    leos.set_spare_room(4, 100);
    assert!(leos.trim());
    let of_bea = leos.entries().filter(|e| e.author == bea.key()).count();
    assert_eq!(of_bea, 1 + RUN as usize + 4);
    // Leo signs on what he has of hers: those end their run, and she has a run again, no
    // more.
    let mut more = Vec::new();
    for round in 0..3u64 {
        leos.append(
            &leo.id,
            Body::SetRoom {
                id: [1; 8],
                name: format!("l{round}"),
            },
            300 + round,
        )
        .unwrap();
        let heads = leos.heads();
        for i in 0..40u64 {
            let entry = Entry::sign(
                &bea.id,
                group,
                heads.clone(),
                400 + 100 * round + i,
                Body::Rename {
                    name: format!("n{round}-{i}"),
                },
            );
            more.push(entry.clone());
            let _ = leos.insert(entry);
        }
        leos.trim();
    }
    // Her first run, and one for each of Leo's three.
    assert_eq!(counted_of(&leos, &bea.key()), 1 + 4 * RUN as usize);
    // Another copy that takes all of it, the other way round, and lets go of what it may:
    // once the two have sent each other what the other lacks of what they need, they keep
    // the same besides spare entries and work out the same group. (Each of Leo's entries,
    // of a member that came in after Bea, goes before those of hers it is not signed on
    // that wait with it, and ends the run before them: some Leo let go of come to be in a
    // run on a copy that kept them, which sends them back.)
    let mut back = GroupLog::empty(group);
    back.set_run_room(RUN, RUN_ROOM.bytes);
    back.set_spare_room(4, 100);
    let mut all: Vec<Entry> = leos.entries().cloned().collect();
    all.extend(written);
    all.extend(more);
    all.reverse();
    loop {
        if back.insert_all(all.clone()).is_empty() {
            break;
        }
    }
    back.trim();
    let send = |from: &GroupLog, to: &mut GroupLog| {
        let lacking: Vec<Entry> = from
            .hashes_to_send()
            .into_iter()
            .filter(|h| to.get(h).is_none())
            .map(|h| from.entries[&h].clone())
            .collect();
        to.insert_all(lacking);
        to.trim();
    };
    for _ in 0..4 {
        send(&back, &mut leos);
        send(&leos, &mut back);
    }
    assert_eq!(back.needed_heads(), leos.needed_heads());
    assert_eq!(back.state().applied, leos.state().applied);
    assert_eq!(back.state().name, leos.state().name);
    same_as_worked_out_again(&leos, &leo.id, "Leo");
    check_every_step(&leos, &leos.state());
}

/// A removal of Bea, signed by Ana on an entry of Bea's that fits in her run on Ana's copy
/// but not in the common order, where Bea wrote more on older entries: it still removes
/// her everywhere, and the entry it is signed on is kept.
#[test]
fn a_removal_signed_on_an_entry_past_a_run_still_removes() {
    const RUN: u32 = 4;
    let (log, ana, bea, _leo) = three_with_run(RUN);
    let group = log.group_id();
    let old = log.ordered();
    // Signed on the newest entry, so that it goes after those on older ones.
    let late = Entry::sign(
        &bea.id,
        group,
        log.heads(),
        50,
        Body::Rename {
            name: "late".into(),
        },
    );
    let mut anas = log.clone();
    anas.insert(late.clone()).unwrap();
    let removal = anas.remove(&ana.id, bea.key(), 60).unwrap();
    assert!(removal.parents.contains(&late.hash()));
    let early: Vec<Entry> = (0..10u64)
        .map(|i| {
            Entry::sign(
                &bea.id,
                group,
                vec![old[1]],
                10 + i,
                Body::Rename {
                    name: format!("e{i}"),
                },
            )
        })
        .collect();
    let mut other = log.clone();
    other.insert_all(early.iter().cloned().chain([late.clone(), removal.clone()]));
    assert!(other.full().budget.over.contains(&late.hash()));
    let state = other.state();
    assert!(!state.is_member(&bea.key()));
    assert!(state.applied.contains(&removal.hash()));
    other.set_spare_room(0, 0);
    other.trim();
    assert!(other.get(&late.hash()).is_some());
    assert!(!other.state().is_member(&bea.key()));
}

/// What honest devices write stays within a run: a manager setting a group up at once,
/// and a device that alone writes in a direct chat, rotating its key every week and
/// renaming now and then for twenty years, while the other side writes nothing. A device
/// alone rotating every week in a group of 20 would reach the bytes of a run after more
/// than 25 years.
#[test]
fn what_honest_devices_write_fits_in_a_run() {
    let ana = Device::new();
    let others: Vec<Device> = (0..19).map(|_| Device::new()).collect();
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    for (i, d) in others.iter().enumerate() {
        log.add(&ana.id, d.key(), "m", 2 + i as u64).unwrap();
    }
    log.append(&ana.id, Body::SetManagers { first: Some(3) }, 30)
        .unwrap();
    log.append(
        &ana.id,
        Body::SetLimits {
            members: 20,
            room: 6,
        },
        31,
    )
    .unwrap();
    for r in 0..9u8 {
        log.append(
            &ana.id,
            Body::SetRoom {
                id: [r + 1; 8],
                name: format!("r{r}"),
            },
            40,
        )
        .unwrap();
        log.set_room_access(&ana.id, [r + 1; 8], Some(vec![others[0].key()]), 41)
            .unwrap();
    }
    for i in 0..5 {
        log.invite(&ana.id, 100_000, 50 + i).unwrap();
    }
    let rotation = log.rotate(&ana.id, 60).unwrap().encode().len();
    assert!(
        52 * 25 * rotation < MAX_RUN_BYTES,
        "{rotation} bytes a week"
    );
    // A direct chat: Ana alone writes for twenty years.
    let bea = Device::new();
    let mut chat = GroupLog::create(&ana.id, "chat", 1).unwrap();
    chat.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    let week = 7 * 24 * 3600 * 1000;
    for w in 0..52 * 20u64 {
        chat.rotate(&ana.id, 3 + w * week).unwrap();
        if w % 8 == 0 {
            chat.append(
                &ana.id,
                Body::Rename {
                    name: format!("c{w}"),
                },
                4 + w * week,
            )
            .unwrap();
        }
    }
    assert!(chat.full().budget.over.is_empty());
}

/// Bytes of all the entries `log` keeps by `author`, waiting ones included.
fn all_bytes_of(log: &GroupLog, author: &DevicePublicKey) -> usize {
    log.entries()
        .filter(|e| e.author == *author)
        .map(|e| e.encode().len())
        .sum()
}

/// Marta, who manages nothing, writes a chain of entries of a kind this version does not
/// know, each signed on the one before, 12 MB in all, and signs a leave on the last; Ana, a
/// manager, writes such chains on what everyone signs on, each under the room for spare
/// entries, and signs a rename on the last of each. An entry signed on entries that are
/// part of no run counts them in its own author's run: Marta's leave, past it, is refused,
/// and of Ana's chains only a run's worth stays, the rest going as spare entries.
#[test]
fn entries_of_no_run_signed_on_count_in_the_run_of_the_one_signing() {
    let (ana, marta) = (Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.add(&ana.id, marta.key(), "Marta", 2).unwrap();
    let group = log.group_id();
    let unknown = |author: &Device, parents: Vec<EntryHash>, time: u64| {
        Entry::sign(
            &author.id,
            group,
            parents,
            time,
            Body::Unknown {
                kind: 200,
                bytes: vec![9; 60_000],
            },
        )
    };
    let mut written = Vec::new();
    let mut parents = log.heads();
    for i in 0..200 {
        let entry = unknown(&marta, parents, 10 + i);
        parents = vec![entry.hash()];
        written.push(entry);
    }
    let leave = Entry::sign(&marta.id, group, parents, 300, Body::Leave);
    let mut other = log.clone();
    other.insert_all(written.into_iter().chain([leave.clone()]));
    assert!(other.get(&leave.hash()).is_none());
    other.trim();
    let kept = all_bytes_of(&other, &marta.key());
    assert!(
        kept <= MAX_SPARE_BYTES_PER_AUTHOR,
        "{kept} bytes of Marta's kept"
    );
    // A leave signed on what everyone signs on still takes her out.
    other.append(&marta.id, Body::Leave, 400).unwrap();
    assert!(!other.state().is_member(&marta.key()));

    let mut anas = log.clone();
    let before = all_bytes_of(&anas, &ana.key());
    for c in 0..30u64 {
        let mut parents = anas.heads();
        let mut chain = Vec::new();
        for i in 0..15 {
            let entry = unknown(&ana, parents, 1_000 + 100 * c + i);
            parents = vec![entry.hash()];
            chain.push(entry);
        }
        chain.push(Entry::sign(
            &ana.id,
            group,
            parents,
            1_050 + 100 * c,
            Body::Rename {
                name: format!("a{c}"),
            },
        ));
        anas.insert_all(chain);
        anas.trim();
    }
    // Entries of a chain whose first went with the spare ones wait for it, within their
    // room.
    let waiting: usize = anas
        .waiting()
        .iter()
        .map(|h| anas.entries[h].encode().len())
        .sum();
    let (spare, rest) = bytes_of(&anas, &ana.key());
    assert!(
        rest - waiting <= before + MAX_RUN_BYTES,
        "{} bytes of Ana's needed",
        rest - waiting
    );
    assert!(
        spare <= MAX_SPARE_BYTES_PER_AUTHOR,
        "{spare} bytes of Ana's spare"
    );
    assert!(
        waiting <= MAX_WAITING_BYTES_PER_AUTHOR,
        "{waiting} bytes waiting"
    );
    assert_eq!(anas.state().name, "a3");
    // Copies that take the same entries the other way round keep the same.
    let mut back = GroupLog::empty(group);
    let mut all: Vec<Entry> = anas.entries().cloned().collect();
    all.reverse();
    back.insert_all(all);
    back.trim();
    assert_eq!(back.needed_heads(), anas.needed_heads());
    assert_eq!(back.state().applied, anas.state().applied);
}

/// The first Commit, by Ana, adding Bea.
fn first_commit(log: &mut GroupLog, ana: &Device, bea: &Device) -> EntryHash {
    let entry = Entry::sign(
        &ana.id,
        log.group_id(),
        log.heads(),
        10,
        Body::Commit {
            link: EpochLink {
                epoch: 0,
                prev: None,
            },
            changes: Changes::new([bea.key()], []),
            payload: payload_digest(b"first"),
            welcome: Some(payload_digest(b"welcome")),
        },
    );
    log.insert(entry.clone()).unwrap();
    entry.hash()
}

/// Ana, a manager, starts the MLS chain and then writes Commits on it without end, each
/// carrying out made-up removals (58 KB): they count in her run like any other entry, so
/// every copy keeps a run's worth and refuses the rest. Bea, who manages nothing, writes
/// Resyncs on the chain without end, and is held to her run the same way.
#[test]
fn commits_and_resyncs_count_in_their_author_s_run() {
    let (ana, bea) = (Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    let first = first_commit(&mut log, &ana, &bea);
    let start = log.clone();
    let group = log.group_id();
    let made_up: Vec<DevicePublicKey> = (0..1_800).map(|_| Device::new().key()).collect();
    let mut refused = 0;
    for i in 0..150u64 {
        let entry = Entry::sign(
            &ana.id,
            group,
            log.heads(),
            20 + i,
            Body::Commit {
                link: EpochLink {
                    epoch: 1,
                    prev: Some(first),
                },
                changes: Changes::new([], made_up.iter().copied()),
                payload: payload_digest(&i.to_be_bytes()),
                welcome: None,
            },
        );
        if log.insert(entry) == Err(StateError::NotAllowed(PAST_RUN)) {
            refused += 1;
        }
    }
    log.trim();
    let kept = all_bytes_of(&log, &ana.key());
    assert!(refused > 70, "{refused} refused");
    assert!(kept <= MAX_RUN_BYTES, "{kept} bytes of Ana's kept");

    let mut log = start;
    log.set_run_room(16, RUN_ROOM.bytes);
    let mut kept = 0;
    for i in 0..100u64 {
        let entry = Entry::sign(
            &bea.id,
            group,
            log.heads(),
            20 + i,
            Body::Resync {
                link: EpochLink {
                    epoch: 1,
                    prev: Some(first),
                },
                payload: payload_digest(&i.to_be_bytes()),
            },
        );
        match log.insert(entry) {
            Ok(true) => kept += 1,
            Err(StateError::NotAllowed(PAST_RUN)) => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(kept, 16);
}

/// Commits signed apart, each in its author's run where it is judged: in the common order
/// those past the run count for nothing in the MLS chain, and copies that let them go as
/// spare entries work out the same chain.
#[test]
fn commits_past_their_run_count_for_nothing_in_the_chain() {
    let (mut log, ana, bea, _leo) = three_with_run(4);
    let first = first_commit(&mut log, &ana, &bea);
    let group = log.group_id();
    let written: Vec<Entry> = (0..5u64)
        .map(|i| {
            Entry::sign(
                &ana.id,
                group,
                vec![first],
                20 + i,
                Body::Commit {
                    link: EpochLink {
                        epoch: 1,
                        prev: Some(first),
                    },
                    changes: Changes::default(),
                    payload: payload_digest(&i.to_be_bytes()),
                    welcome: None,
                },
            )
        })
        .collect();
    assert_eq!(log.insert_all(written.clone()).len(), written.len());
    let chain = log.mls_chain(valid);
    let over: Vec<EntryHash> = written
        .iter()
        .map(Entry::hash)
        .filter(|h| chain.refused.get(h) == Some(&PAST_RUN))
        .collect();
    assert_eq!(over.len(), 4, "{chain:?}");
    assert_eq!(chain.links.len(), 2);
    log.set_spare_room(0, 0);
    assert!(log.trim());
    assert!(over.iter().all(|h| log.get(h).is_none()));
    let trimmed = log.mls_chain(valid);
    assert_eq!(trimmed.links, chain.links);
    assert_eq!(trimmed.counted, chain.counted);
}

/// What honest devices write for MLS stays within a run: a manager that adds a member with
/// a Commit, and then alone updates its keys every day, while the other side writes
/// nothing, fills a tenth of a run in a year; a member that leaves, and the Commit that
/// carries it out, change nothing of that.
#[test]
fn honest_commits_fit_in_a_run() {
    let (ana, bea) = (Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
    let mut prev = first_commit(&mut log, &ana, &bea);
    let day = 24 * 3600 * 1000;
    for d in 1..=365u64 {
        let entry = Entry::sign(
            &ana.id,
            log.group_id(),
            log.heads(),
            10 + d * day,
            Body::Commit {
                link: EpochLink {
                    epoch: d,
                    prev: Some(prev),
                },
                changes: Changes::default(),
                payload: payload_digest(&d.to_be_bytes()),
                welcome: None,
            },
        );
        prev = entry.hash();
        log.insert(entry).unwrap();
    }
    // Bea signed on nothing of Ana's: everything Ana wrote is in her run.
    let (entries, bytes) = log
        .entries()
        .filter(|e| e.author == ana.key())
        .fold((0, 0), |(n, b), e| (n + 1, b + e.encode().len()));
    assert!(10 * entries < MAX_RUN_ENTRIES as usize, "{entries} entries");
    assert!(10 * bytes < MAX_RUN_BYTES, "{bytes} bytes");
    log.append(&bea.id, Body::Leave, 400 * day).unwrap();
    let entry = Entry::sign(
        &ana.id,
        log.group_id(),
        log.heads(),
        401 * day,
        Body::Commit {
            link: EpochLink {
                epoch: 366,
                prev: Some(prev),
            },
            changes: Changes::new([], [bea.key()]),
            payload: payload_digest(b"leave"),
            welcome: None,
        },
    );
    log.insert(entry).unwrap();
    assert!(log.full().budget.over.is_empty());
    let chain = log.mls_chain(valid);
    assert_eq!(chain.links.len(), 367);
    assert!(!chain.roster().contains(&bea.key()));
}

/// A manager that brings devices of its own in and takes them out again without end, with
/// tickets it signs outside the log or with adds, the devices leaving or removed: each
/// coming in and going out counts in the manager's run, and none of those devices takes
/// it out, so the manager is held to its run. Bea, in before them, signs on what Ana
/// wrote, and Ana has a run again, no more.
#[test]
fn bringing_devices_in_and_out_without_end_is_held_to_a_run() {
    const RUN: u32 = 12;
    let (mut log, ana, bea, _leo) = three_with_run(RUN);
    let group = log.group_id();
    let mut comings = 0;
    for i in 0..60u64 {
        let device = Device::new();
        let time = 100 + 10 * i;
        let came_in = if i % 2 == 0 {
            let ticket = log.issue_ticket(&ana.id, time + 5).unwrap();
            let join = Entry::sign(
                &device.id,
                group,
                log.heads(),
                time,
                Body::Join {
                    ticket,
                    name: "d".into(),
                },
            );
            let hash = join.hash();
            log.insert(join).is_ok_and(|kept| kept) && log.let_in(&ana.id, &hash, time).is_ok()
        } else {
            log.add(&ana.id, device.key(), "d", time).is_ok()
        };
        if !came_in {
            continue;
        }
        assert!(log.state().is_member(&device.key()));
        comings += 1;
        if i % 4 < 2 {
            log.append(&device.id, Body::Leave, time + 1).unwrap();
        } else {
            log.remove(&ana.id, device.key(), time + 1).unwrap();
        }
        assert!(!log.state().is_member(&device.key()));
    }
    // Each coming in and going out counts once; the adds of Bea and Leo stay too.
    assert!(comings < RUN as usize / 2, "{comings} came in");
    assert!(log.full().budget.run_of(&ana.key()).entries <= RUN);
    // Bea signs on all of it: Ana brings one in and out again a few more times, no more.
    log.append(&bea.id, Body::Rename { name: "b".into() }, 2_000)
        .unwrap();
    let mut again = 0;
    for i in 0..60u64 {
        let device = Device::new();
        if log.add(&ana.id, device.key(), "d", 3_000 + i).is_err() {
            break;
        }
        log.append(&device.id, Body::Leave, 3_000 + i).unwrap();
        again += 1;
    }
    assert!(again > 0 && again <= RUN as usize / 2, "{again} more");
    // The same on a copy that takes it all at once, the other way round.
    let mut back = GroupLog::empty(group);
    back.set_run_room(RUN, RUN_ROOM.bytes);
    let mut all: Vec<Entry> = log.entries().cloned().collect();
    all.reverse();
    loop {
        if back.insert_all(all.clone()).is_empty() {
            break;
        }
    }
    assert_eq!(back.ordered(), log.ordered());
    assert_eq!(back.state().applied, log.state().applied);
}

/// A group of 20 that a manager keeps for ten years while nobody else writes a thing:
/// she rotates the key every week, and every two years each member replaces its device,
/// the old one leaving or removed and the new one joining with her invite, welcomed. Her
/// run holds less than a fifth of what it may, entries and bytes.
#[test]
fn people_joining_and_replacing_devices_for_years_fit_in_a_run() {
    let ana = Device::new();
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    let mut members: Vec<Device> = (0..19).map(|_| Device::new()).collect();
    for (i, d) in members.iter().enumerate() {
        log.add(&ana.id, d.key(), "m", 2 + i as u64).unwrap();
    }
    let week = 7 * 24 * 3600 * 1000;
    let mut now = 100;
    for w in 0..52 * 10u64 {
        now = 100 + w * week;
        log.rotate(&ana.id, now).unwrap();
        // Every two years, about one member a month and a half replaces its device.
        if w % 5 == 0 {
            let at = (w / 5) as usize % members.len();
            let new = Device::new();
            let old = std::mem::replace(&mut members[at], Device::new());
            if w % 2 == 0 {
                log.append(&old.id, Body::Leave, now + 1).unwrap();
            } else {
                log.remove(&ana.id, old.key(), now + 1).unwrap();
            }
            let (ticket, _) = log.invite(&ana.id, now + week, now + 2).unwrap();
            join(&mut log, &new, ticket, now + 3);
            log.welcome(&ana.id, new.key(), now + 4).unwrap();
            assert!(log.state().is_member(&new.key()), "week {w}");
            members[at] = new;
        }
    }
    assert!(now > 9 * 52 * week);
    assert!(log.full().budget.over.is_empty());
    let run = log.full().budget.run_of(&ana.key());
    eprintln!("{run:?}");
    assert!(run.entries < MAX_RUN_ENTRIES / 5, "{run:?}");
    assert!(run.bytes < MAX_RUN_BYTES / 5, "{run:?}");
}

/// Carl joins with Ana's invite and she lets him in; the key rotates. Bea, a member that
/// does not manage, signs a `Join` with Ana's ticket (every member sees it in the `Invite`)
/// under a key of her own, dated before the ticket expires, on the `Invite` itself: in the
/// common order it comes before Carl's. It lets nobody in and uses no ticket: Carl stays in
/// with his keys, and so does the epoch sealed to him, on every copy, in any order.
#[test]
fn a_join_nobody_let_in_takes_nobody_s_place_nor_ticket() {
    let (ana, bea, carl, sock) = (Device::new(), Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "friends", 1_000).unwrap();
    log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1_001)
        .unwrap();
    log.add(&ana.id, bea.key(), "Bea", 1_002).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000_000, 2_000).unwrap();
    let invite = log
        .entries()
        .find(|e| matches!(e.body, Body::Invite { .. }))
        .unwrap()
        .hash();
    let join = Entry::sign(
        &carl.id,
        log.group_id(),
        log.heads(),
        3_000,
        Body::Join {
            ticket: ticket.clone(),
            name: "Carl".into(),
        },
    );
    assert_eq!(log.insert(join.clone()), Ok(true));
    log.let_in(&ana.id, &join.hash(), 3_100).unwrap();
    log.rotate(&ana.id, 4_000).unwrap();
    log.append(
        &ana.id,
        Body::Rename {
            name: "friends!".into(),
        },
        4_500,
    )
    .unwrap();
    let before = log.state();
    assert!(before.keyed(&carl.key()));
    assert_eq!(before.epochs.len(), 2);

    // Dated as the attacker likes, so that it sorts before Carl's join, as it would grind.
    let forged = (2_000u64..)
        .map(|time| {
            Entry::sign(
                &sock.id,
                log.group_id(),
                vec![invite],
                time,
                Body::Join {
                    ticket: ticket.clone(),
                    name: "x".into(),
                },
            )
        })
        .find(|e| e.hash() < join.hash())
        .unwrap();
    // Kept (its author could sign it there), but it waits for a `Welcome` nobody writes.
    assert_eq!(log.insert(forged.clone()), Ok(true));
    let place = |log: &GroupLog, h: &EntryHash| log.ordered().iter().position(|o| o == h);
    assert!(place(&log, &forged.hash()) < place(&log, &join.hash()));
    let check = |log: &GroupLog| {
        let state = log.state();
        assert!(state.keyed(&carl.key()));
        assert!(!state.is_member(&sock.key()));
        assert_eq!(state.members.len(), 3);
        assert_eq!(state.epochs.len(), 2);
        assert_eq!(state.name, "friends!");
        assert_eq!(state.applied, before.applied);
        assert_eq!(log.join_ticket_of(&carl.key()), Some(&ticket));
        assert!(log.current_keys(&carl.id).is_some());
        assert!(log.spare().contains(&forged.hash()));
    };
    check(&log);
    // Nor does Ana sign anything on it: it stays spare, and goes when the log is trimmed.
    let rename = log
        .append(
            &ana.id,
            Body::Rename {
                name: "friends".into(),
            },
            5_000,
        )
        .unwrap();
    assert!(!rename.parents.contains(&forged.hash()));
    // A copy fed every entry the other way round agrees.
    let mut copy = GroupLog::empty(log.group_id());
    let mut entries: Vec<Entry> = log.entries().cloned().collect();
    entries.reverse();
    copy.insert_all(entries);
    assert_eq!(copy.state().applied, log.state().applied);
    assert_eq!(copy.ordered(), log.ordered());
}

/// The same forged join, there before the invited device comes: the invite still works
/// for the device the manager lets in, and stays open until then.
#[test]
fn a_join_nobody_let_in_burns_no_invite() {
    let (ana, bea, carl, sock) = (Device::new(), Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "friends", 1_000).unwrap();
    log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1_001)
        .unwrap();
    log.add(&ana.id, bea.key(), "Bea", 1_002).unwrap();
    let (ticket, token) = log.invite(&ana.id, 10_000_000, 2_000).unwrap();
    let invite = log.heads();
    for i in 0..3 {
        let forged = Entry::sign(
            &Device::new().id,
            log.group_id(),
            invite.clone(),
            2_100 + i,
            Body::Join {
                ticket: ticket.clone(),
                name: "x".into(),
            },
        );
        assert_eq!(log.insert(forged), Ok(true));
    }
    let own = Entry::sign(
        &sock.id,
        log.group_id(),
        log.heads(),
        2_200,
        Body::Join {
            ticket: ticket.clone(),
            name: "x".into(),
        },
    );
    assert_eq!(log.insert(own), Ok(true));
    let state = log.state();
    assert_eq!(state.members.len(), 2);
    assert_eq!(state.open_invite_tokens(3_000), vec![token]);
    assert!(!state.used_tickets.contains(&ticket.ticket_id));
    // Bea cannot let her own device in.
    assert!(log.welcome(&bea.id, sock.key(), 2_300).is_err());
    let join = Entry::sign(
        &carl.id,
        log.group_id(),
        log.heads(),
        3_000,
        Body::Join {
            ticket: ticket.clone(),
            name: "Carl".into(),
        },
    );
    assert_eq!(log.insert(join.clone()), Ok(true));
    log.let_in(&ana.id, &join.hash(), 3_100).unwrap();
    let state = log.state();
    assert!(state.keyed(&carl.key()));
    assert!(!state.is_member(&sock.key()));
    assert!(state.used_tickets.contains(&ticket.ticket_id));
    assert!(state.open_invite_tokens(3_000).is_empty());
    // Used now: none of the others can be let in with it.
    assert!(log.welcome(&ana.id, sock.key(), 3_200).is_err());
}

/// Bea signs thousands of joins with Ana's one ticket under keys of her own, on the
/// `Invite`: none counts in Ana's run (nor in anyone's), so Ana still writes, and they are
/// spare entries a device lets go of past their room.
#[test]
fn joins_nobody_let_in_count_in_nobody_s_run() {
    const RUN: u32 = 16;
    let (ana, bea) = (Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "friends", 1_000).unwrap();
    log.set_run_room(RUN, RUN_ROOM.bytes);
    log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1_001)
        .unwrap();
    log.add(&ana.id, bea.key(), "Bea", 1_002).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000_000, 2_000).unwrap();
    let invite = log.heads();
    // Bea signs on all that Ana wrote: Ana starts afresh.
    log.append(&bea.id, Body::Leave, 2_001).unwrap();
    let run_before = log.full().budget.run_of(&ana.key());
    let phantoms: Vec<Entry> = (0..RUN as u64 * 8)
        .map(|i| {
            Entry::sign(
                &Device::new().id,
                log.group_id(),
                invite.clone(),
                2_100 + i,
                Body::Join {
                    ticket: ticket.clone(),
                    name: "p".into(),
                },
            )
        })
        .collect();
    let kept = log.insert_all(phantoms.clone()).len();
    assert_eq!(kept, phantoms.len());
    assert_eq!(log.full().budget.run_of(&ana.key()), run_before);
    assert_eq!(log.state().members.len(), 1);
    assert_eq!(log.spare().len(), phantoms.len());
    // Ana writes as before.
    for i in 0..4 {
        log.append(
            &ana.id,
            Body::Rename {
                name: format!("a{i}"),
            },
            3_000 + i,
        )
        .unwrap();
    }
    // A device keeps no more of them than its room for spare entries.
    log.set_spare_room(8, 32);
    assert!(log.trim());
    assert!(log.spare().len() <= 32, "{} spare", log.spare().len());
    assert_eq!(log.state().name, "a3");
}

/// Bea, added after Carl, signs a join with an invite of Ana's on the group's first entry,
/// before either was added: in the common order it would come before both adds. It moves
/// her nowhere: she stays below Carl, who stays out of her reach.
#[test]
fn a_member_s_join_on_old_entries_moves_it_nowhere() {
    let (ana, carl, bea) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1_000).unwrap();
    let root = log.heads();
    log.add(&ana.id, carl.key(), "Carl", 1_001).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 1_002).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000_000, 2_000).unwrap();
    let backdated = Entry::sign(
        &bea.id,
        log.group_id(),
        root,
        1_000,
        Body::Join {
            ticket,
            name: "Bea".into(),
        },
    );
    let _ = log.insert(backdated);
    let state = log.state();
    assert_eq!(state.ladder, [ana.key(), carl.key(), bea.key()]);
    assert!(log.remove(&bea.id, carl.key(), 3_000).is_err());
}

/// Bea writes a chain of entries of a kind nobody knows (spare: devices let them go) and a
/// device that wants in signs its join on the last: Ana's `Welcome` would make every member
/// keep the chain, in Ana's run. Ana does not let it in on that join.
#[test]
fn a_join_signed_on_spare_entries_is_not_let_in() {
    let (ana, bea, dee) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1_000).unwrap();
    log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1_001)
        .unwrap();
    log.add(&ana.id, bea.key(), "Bea", 1_002).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000_000, 2_000).unwrap();
    let mut last = log.heads();
    for i in 0..8 {
        let junk = Entry::sign(
            &bea.id,
            log.group_id(),
            last,
            2_100 + i,
            Body::Unknown {
                kind: 200,
                bytes: vec![7; 60_000],
            },
        );
        assert_eq!(log.insert(junk.clone()), Ok(true));
        last = vec![junk.hash()];
    }
    let spare = log.spare().len();
    let mut parents = log.heads();
    parents.extend(last);
    let join = Entry::sign(
        &dee.id,
        log.group_id(),
        parents,
        3_000,
        Body::Join {
            ticket,
            name: "Dee".into(),
        },
    );
    assert_eq!(log.insert(join.clone()), Ok(true));
    assert_eq!(
        log.let_in(&ana.id, &join.hash(), 3_100),
        Err(StateError::NotAllowed(JOIN_ON_SPARE))
    );
    assert_eq!(
        log.welcome(&ana.id, dee.key(), 3_100).map(drop),
        Err(StateError::NotAllowed(JOIN_ON_SPARE))
    );
    assert!(!log.state().is_member(&dee.key()));
    assert_eq!(log.spare().len(), spare + 1);
}

/// Dee signs a join with Bob's ticket and, later, one with Ana's, after an entry of Bob's
/// signed on the first. Ana lets Dee in on the join she saw: that one applies, under its
/// name, and uses her ticket; Bob's stays open.
#[test]
fn a_welcome_lets_in_with_the_join_it_is_signed_on() {
    let (ana, bob, dee) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1_000).unwrap();
    log.add(&ana.id, bob.key(), "Bob", 1_001).unwrap();
    let (by_bob, bob_token) = log.invite(&bob.id, 10_000_000, 2_000).unwrap();
    let (by_ana, _) = log.invite(&ana.id, 10_000_000, 2_001).unwrap();
    let join = |log: &GroupLog, parents: Vec<EntryHash>, ticket: &Ticket, name: &str| {
        Entry::sign(
            &dee.id,
            log.group_id(),
            parents,
            3_000,
            Body::Join {
                ticket: ticket.clone(),
                name: name.into(),
            },
        )
    };
    let first = join(&log, log.heads(), &by_bob, "with Bob's");
    assert_eq!(log.insert(first.clone()), Ok(true));
    let mut parents = log.heads();
    parents.push(first.hash());
    let on_it = Entry::sign(
        &bob.id,
        log.group_id(),
        parents,
        3_001,
        Body::Rename { name: "h".into() },
    );
    assert_eq!(log.insert(on_it), Ok(true));
    let second = join(&log, log.heads(), &by_ana, "with Ana's");
    assert_eq!(log.insert(second.clone()), Ok(true));
    log.let_in(&ana.id, &second.hash(), 3_100).unwrap();
    let state = log.state();
    assert_eq!(state.members[&dee.key()].name, "with Ana's");
    assert!(state.used_tickets.contains(&by_ana.ticket_id));
    assert!(!state.used_tickets.contains(&by_bob.ticket_id));
    assert_eq!(state.open_invite_tokens(3_200), vec![bob_token]);
    // A copy that takes it all the other way round agrees.
    let mut copy = GroupLog::empty(log.group_id());
    let mut all: Vec<Entry> = log.entries().cloned().collect();
    all.reverse();
    copy.insert_all(all);
    assert_eq!(copy.state().applied, state.applied);
}

/// Bea signs an entry of her own on a join nobody let in: she pays for keeping it, but the
/// order does not count its device as come in.
#[test]
fn an_entry_signed_on_a_join_lets_nobody_in() {
    let (ana, bea, sock) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1_000).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 1_001).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000_000, 2_000).unwrap();
    let join = Entry::sign(
        &sock.id,
        log.group_id(),
        log.heads(),
        3_000,
        Body::Join {
            ticket,
            name: "sock".into(),
        },
    );
    assert_eq!(log.insert(join.clone()), Ok(true));
    let mut parents = log.heads();
    parents.push(join.hash());
    let rename = Entry::sign(
        &bea.id,
        log.group_id(),
        parents,
        3_001,
        Body::Rename { name: "h".into() },
    );
    assert_eq!(log.insert(rename), Ok(true));
    assert!(!log.spare().contains(&join.hash()));
    assert!(!log.full().budget.counts_in(&sock.key()));
    assert!(!log.state().is_member(&sock.key()));
}
