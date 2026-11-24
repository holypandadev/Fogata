//! Level five: the ceiling of a log, and moving a group to a new log (see [`Level::Five`]).

use super::*;
use crate::{
    budget::{CEILING, PAST_CEILING},
    entry::Moved,
    state::{MOVED_AWAY, MOVED_NOT_FIRST, NOT_A_MOVE},
};

/// A ceiling small enough to reach in a test: entries count, bytes never do.
const SMALL: u32 = 48;

/// `f` run as a copy of the version before level five: one that judges at four at most.
fn as_level_four<T>(f: impl FnOnce() -> T) -> T {
    crate::state::HIGHEST.with(|h| h.set(Level::Four));
    let out = f();
    crate::state::HIGHEST.with(|h| h.set(Level::CURRENT));
    out
}

fn rename(log: &mut GroupLog, who: &Device, n: u64) -> Result<Entry, StateError> {
    log.append(
        &who.id,
        Body::Rename {
            name: format!("name {n}"),
        },
        100 + n,
    )
}

/// A group of Ana, a second device of hers and Bea, everyone managing (the default), with
/// a small ceiling; raised to `level`.
fn group(level: Level) -> (GroupLog, Device, Device, Device) {
    let (ana, laptop, bea) = (Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "friends", 1).unwrap();
    log.set_ceiling(SMALL, CEILING.bytes);
    log.add(&ana.id, laptop.key(), "Ana's laptop", 2).unwrap();
    log.add(&ana.id, bea.key(), "Bea", 3).unwrap();
    if level > Level::Two {
        log.raise_level(&ana.id, level, 4).unwrap();
    }
    (log, ana, laptop, bea)
}

/// Two devices that sign on each other's entries end each other's runs: at level four
/// nothing holds them, and a log grows past any ceiling; from level five, the log stops at
/// its ceiling, and what applied stays as it was.
#[test]
fn a_flood_by_two_devices_stops_at_the_ceiling() {
    let (mut four, ana, laptop, _) = group(Level::Four);
    for n in 0..u64::from(SMALL) * 2 {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        rename(&mut four, who, n).unwrap();
    }
    assert!(four.fill().entries > SMALL, "{:?}", four.fill());

    let (mut five, ana, laptop, bea) = group(Level::Five);
    let mut refused = None;
    for n in 0..u64::from(SMALL) * 2 {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        if let Err(e) = rename(&mut five, who, n) {
            refused = Some((n, e));
            break;
        }
    }
    let (n, why) = refused.expect("the ceiling holds the flood back");
    assert_eq!(why, StateError::NotAllowed(PAST_CEILING));
    let fill = five.fill();
    assert!(fill.entries <= SMALL && fill.full(), "{fill:?}");
    assert_eq!(five.state().name, format!("name {}", n - 1));
    // Nothing else counted applies any more, by anyone...
    assert!(rename(&mut five, &bea, 1_000).is_err());
    assert!(five.rotate(&bea.id, 1_001).is_err());
    // ...but a removal does, and the group can still move.
    five.remove(&ana.id, laptop.key(), 1_002).unwrap();
    assert!(!five.state().is_member(&laptop.key()));
    let new = five.new_log_for_move(&ana.id, 1_003).unwrap();
    five.move_to(&ana.id, &new, 1_004).unwrap();
    assert_eq!(five.moved().map(|(_, m)| m.to), Some(new.group_id()));
}

/// Entries written apart from each other past the ceiling, taken in any order: every copy
/// keeps and applies the same, and holds no more than the ceiling.
#[test]
fn every_copy_keeps_the_same_at_the_ceiling() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    let mut theirs = log.clone();
    for n in 0..u64::from(SMALL) {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        let _ = rename(&mut log, who, n);
        let _ = rename(&mut theirs, &bea, 10_000 + n);
    }
    let mut one = log.clone();
    let mut other = theirs.clone();
    one.insert_all(theirs.entries_in_order().into_iter().cloned());
    other.insert_all(log.entries_in_order().into_iter().cloned());
    let mut shuffled = GroupLog::empty(log.group_id());
    shuffled.set_ceiling(SMALL, CEILING.bytes);
    let mut all: Vec<Entry> = one.entries().cloned().collect();
    all.sort_by_key(Entry::hash);
    shuffled.insert_all(all);
    for copy in [&other, &shuffled] {
        assert_eq!(copy.hashes_in_order(), one.hashes_in_order());
        assert_eq!(copy.state().applied, one.state().applied);
        assert_eq!(copy.fill(), one.fill());
    }
    assert!(one.fill().entries <= SMALL, "{:?}", one.fill());
    assert!(one.len() > SMALL as usize, "both sides wrote past it apart");
}

/// A raise to level five holds the whole log to the ceiling, as the order of a level holds
/// every entry: a log under it keeps applying what it applied; one already past it keeps
/// what fits, in the order, the raise itself too, and what does not applies to nothing.
#[test]
fn a_raise_holds_the_whole_log_to_the_ceiling() {
    let (mut under, ana, laptop, _) = group(Level::Four);
    for n in 0..u64::from(SMALL) / 2 {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        rename(&mut under, who, n).unwrap();
    }
    let before = under.state().applied;
    let raise = under.raise_level(&ana.id, Level::Five, 500).unwrap();
    let mut after = under.state().applied;
    assert_eq!(after.pop(), Some(raise.hash()));
    assert_eq!(after, before);
    assert!(!under.fill().full());

    let (mut past, ana, laptop, _) = group(Level::Four);
    for n in 0..u64::from(SMALL) * 2 {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        rename(&mut past, who, n).unwrap();
    }
    let raise = past.raise_level(&ana.id, Level::Five, 500).unwrap();
    let state = past.state();
    assert_eq!(state.level, Level::Five);
    assert!(state.applied.contains(&raise.hash()));
    let spare = (state.rejected.iter())
        .filter(|(_, why)| *why == PAST_CEILING)
        .count();
    assert!(spare >= SMALL as usize - 4, "{spare} past the ceiling");
    assert!(
        past.fill().full() && past.fill().entries <= SMALL + crate::budget::SLACK.entries,
        "{:?}",
        past.fill()
    );
    assert_eq!(
        rename(&mut past, &ana, 1_000),
        Err(StateError::NotAllowed(PAST_CEILING))
    );
    // A second raise naming no higher level is held like the rest: of two written apart,
    // the first in the order gets past the ceiling, and the other is held.
    let mut again = past.clone();
    let (group_id, heads) = (again.group_id(), raise.parents.clone());
    let twice = Entry::sign(&laptop.id, group_id, heads, 501, Body::Level { level: 5 });
    again.insert(twice.clone()).unwrap();
    let held = |h: EntryHash| (again.state().rejected).contains(&(h, PAST_CEILING));
    assert!(held(twice.hash()) != held(raise.hash()));
}

/// Only the first member moves the group, once; nothing applies after the move, and
/// nothing is kept whose author saw it.
#[test]
fn only_the_first_member_moves_the_group_once() {
    let (mut log, ana, _, bea) = group(Level::Five);
    log.set_ceiling(CEILING.entries, CEILING.bytes);
    let mut beas = log.clone();
    assert_eq!(
        beas.new_log_for_move(&bea.id, 10).err(),
        Some(StateError::NotAllowed(MOVED_NOT_FIRST))
    );
    // Written by hand, a move of Bea's is kept as hers, and does not apply.
    let forged = Moved {
        to: GroupId::random().unwrap(),
        setup: [7; 32],
    };
    assert_eq!(
        beas.append(&bea.id, forged.body(), 11).err(),
        Some(StateError::NotAllowed(MOVED_NOT_FIRST))
    );
    // Bea renames the group, not seeing the move.
    let apart = rename(&mut beas, &bea, 12).unwrap();
    let new = log.new_log_for_move(&ana.id, 13).unwrap();
    let moved = log.move_to(&ana.id, &new, 14).unwrap();
    assert_eq!(log.moved().map(|(h, _)| h), Some(moved.hash()));
    // Nothing more is kept from whoever saw the move...
    assert_eq!(
        rename(&mut log, &ana, 15).err(),
        Some(StateError::NotAllowed(MOVED_AWAY))
    );
    let again = log.new_log_for_move(&ana.id, 16);
    assert_eq!(again.err(), Some(StateError::NotAllowed(MOVED_AWAY)));
    // ...and what was written apart from it is kept, wherever the order puts it: the move
    // stays made, and the new log is the group the move saw.
    log.insert(apart.clone()).unwrap();
    beas.insert(moved.clone()).unwrap();
    for copy in [&log, &beas] {
        assert_eq!(copy.state().applied, log.state().applied);
        assert_eq!(copy.moved().map(|(h, _)| h), Some(moved.hash()));
        copy.follows(&new).unwrap();
    }
}

/// A member follows only to the group as it was where the move is signed: the same
/// members, names, ladder, managers, rooms and who may enter them, time outs included.
#[test]
fn a_member_follows_only_to_the_same_group() {
    let (ana, bea, cy, dan) = (Device::new(), Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "friends", 1).unwrap();
    log.raise_level(&ana.id, Level::Five, 2).unwrap();
    for (n, device) in [&bea, &cy, &dan].into_iter().enumerate() {
        log.add(&ana.id, device.key(), &format!("member {n}"), 3)
            .unwrap();
    }
    for r in 1..3u8 {
        let room = Body::SetRoom {
            id: [r; 8],
            name: format!("room {r}"),
        };
        log.append(&ana.id, room, 4).unwrap();
    }
    log.append(&ana.id, Body::SetIcon { icon: 3 }, 4).unwrap();
    // Bea, second, lets only Cy into a room: she and Ana are let in too.
    log.set_room_access(&bea.id, [1; 8], Some(vec![cy.key()]), 5)
        .unwrap();
    log.keep_out(&ana.id, [2; 8], dan.key(), Some(10_000), 5)
        .unwrap();
    log.keep_out(&ana.id, [2; 8], cy.key(), Some(6), 5).unwrap();
    log.append(&ana.id, Body::SetManagers { first: Some(2) }, 5)
        .unwrap();
    let mut beas = log.clone();
    let new = log.new_log_for_move(&ana.id, 20).unwrap();
    assert_eq!(new.level(), Level::CURRENT);
    assert!(new.len() < log.len() + 10);
    let moved = log.move_to(&ana.id, &new, 21).unwrap();
    beas.insert(moved).unwrap();
    beas.follows(&new).unwrap();
    // The new group goes on: later entries change nothing of its start.
    let mut later = new.clone();
    later
        .append(&ana.id, Body::Rename { name: "on".into() }, 30)
        .unwrap();
    beas.follows(&later).unwrap();
    // Any other log is refused: another group, or a new one under no move.
    let (mut other, _, _, _) = group(Level::Five);
    other.set_ceiling(CEILING.entries, CEILING.bytes);
    assert!(beas.follows(&other).is_err());
    let unnamed = GroupLog::create(&ana.id, "friends", 20).unwrap();
    assert!(beas.follows(&unnamed).is_err());
    // A new log that is not the group is never named by a move.
    let differs: [&dyn Fn(&mut GroupLog); 5] = [
        &|l| {
            l.add(&ana.id, Device::new().key(), "stranger", 20).unwrap();
        },
        &|l| {
            l.remove(&ana.id, dan.key(), 20).unwrap();
        },
        &|l| {
            l.append(&ana.id, Body::Promote { member: dan.key() }, 20)
                .unwrap();
        },
        &|l| {
            l.append(&ana.id, Body::SetManagers { first: None }, 20)
                .unwrap();
        },
        &|l| {
            l.let_back(&ana.id, [2; 8], dan.key(), 20).unwrap();
        },
    ];
    let mut unmoved = beas.clone();
    unmoved.forget(&unmoved.moved().unwrap().0);
    for change in differs {
        let mut wrong = new.clone();
        change(&mut wrong);
        let mut copy = unmoved.clone();
        assert!(copy.move_to(&ana.id, &wrong, 22).is_err());
    }
}

/// A copy of the version before level five refuses the raise to it, keeps the group as it
/// was there and tells its person that it needs a newer Fogata; one of this version takes
/// it, and the move after it.
#[test]
fn copies_at_four_refuse_the_raise_to_five() {
    let (mut log, ana, _, _) = group(Level::Four);
    let raise = log.raise_level(&ana.id, Level::Five, 10).unwrap();
    let new = log.new_log_for_move(&ana.id, 11).unwrap();
    let moved = log.move_to(&ana.id, &new, 12).unwrap();
    let entries: Vec<Entry> = log.entries_in_order().into_iter().cloned().collect();
    let old = as_level_four(|| {
        let mut old = GroupLog::empty(log.group_id());
        let taken = old.insert_some(entries.clone());
        assert!(!taken.new.contains(&raise.hash()));
        assert!(old.state().level == Level::Four && old.moved().is_none());
        assert_eq!(old.level_ahead(), Some(Level::Five.number()));
        old
    });
    // What is signed on the raise waits there, as after an entry that never came.
    assert!(!old.entries.contains_key(&moved.hash()) || old.unjudged.contains(&moved.hash()));
    let mut now = GroupLog::empty(log.group_id());
    now.insert_all(entries);
    assert_eq!(now.moved().map(|(h, _)| h), Some(moved.hash()));
}

/// Raises copy 0's group to `level`, by a member that manages there, and hands the entry to
/// every copy; false when nobody manages there any more.
fn raised(h: &mut super::hostile::Hostile, level: Level) -> bool {
    let time = h.world.time;
    let state = h.world.logs[0].state();
    let Some(manager) = (h.world.devices.iter()).find(|d| state.is_manager(&d.key())) else {
        return false;
    };
    let Ok(entry) = h.world.logs[0].raise_level(&manager.id, level, time) else {
        return false;
    };
    for log in &mut h.world.logs {
        let _ = log.insert(entry.clone());
    }
    h.written.push(entry);
    true
}

/// Two members of copy 0 sign `n` renames, each on the one before, and hand them to every
/// copy: a flood that no run holds back.
fn flooded(h: &mut super::hostile::Hostile, n: usize) {
    let state = h.world.logs[0].state();
    let two: Vec<usize> = (0..h.world.devices.len())
        .filter(|&i| state.is_member(&h.world.devices[i].key()))
        .take(2)
        .collect();
    let [a, b] = two[..] else { return };
    let group = h.world.logs[0].group_id();
    let mut parents = h.world.logs[0].heads();
    for k in 0..n {
        h.world.time += 1;
        let who = &h.world.devices[if k % 2 == 0 { a } else { b }];
        let body = Body::Rename {
            name: format!("flood {k}"),
        };
        let entry = Entry::sign(&who.id, group, parents.clone(), h.world.time, body);
        parents = vec![entry.hash()];
        for log in &mut h.world.logs {
            let _ = log.insert(entry.clone());
        }
        h.written.push(entry);
    }
}

/// A world of random histories raised to level three first, to four after a while and to
/// five after that, with a ceiling a few dozen entries above copy 0's log at the raise to
/// five, and a flood by two members at the end.
fn level_five_world(seed: u64) -> super::hostile::Hostile {
    let mut h = super::hostile::Hostile::new(seed);
    raised(&mut h, Level::Three);
    for i in 0..120 {
        if i % 3 == 0 {
            h.hostile_step(1 + i % 2);
        } else {
            h.honest_step();
        }
        if i == 30 {
            raised(&mut h, Level::Four);
        }
        if i == 60 {
            let room = h.world.logs[0].fill().entries + 30;
            for log in &mut h.world.logs {
                log.set_ceiling(room, CEILING.bytes);
            }
            raised(&mut h, Level::Five);
        }
    }
    flooded(&mut h, 80);
    for (to, entry) in std::mem::take(&mut h.late) {
        let _ = h.world.logs[to].insert(entry);
    }
    h
}

/// Level five judges the same on every copy whatever order its entries come in, at its
/// ceiling too: copies that took a world's entries in their own order, and fresh copies
/// given every entry ever written as written, the other way round and shuffled, one at a
/// time or all at once, keep the same entries, order them alike and apply the same, and
/// worked out again from nothing the group is the same. At least one world fills its log to
/// the ceiling, and what comes on top applies to nothing.
/// `FOGATA_LADDER_SEEDS` sets how many histories run (8 by default).
#[test]
fn at_level_five_every_copy_applies_the_same_whatever_came_first() {
    let seeds = std::env::var("FOGATA_LADDER_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8u64);
    let (mut at_five, mut past_ceiling) = (0, 0);
    for seed in 1..=seeds {
        let mut h = level_five_world(seed);
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
        if state.level < Level::Five {
            continue;
        }
        at_five += 1;
        for (i, other) in h.world.logs.iter().enumerate() {
            let theirs: BTreeSet<EntryHash> = other.hashes().copied().collect();
            assert_eq!(theirs, kept, "seed {seed}: device {i} keeps others");
            assert_eq!(other.ordered(), log.ordered(), "seed {seed}: device {i}");
            assert_eq!(other.state().applied, state.applied, "seed {seed}: {i}");
            assert_eq!(other.fill(), log.fill(), "seed {seed}: {i}");
        }
        let orders = [
            h.written.clone(),
            h.written.iter().rev().cloned().collect(),
            shuffled,
        ];
        for (o, order) in orders.into_iter().enumerate() {
            let mut one_by_one = h.world.empty_log();
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
                assert_eq!(again.ordered(), log.ordered(), "seed {seed}: {o}");
                assert_eq!(again.state().applied, state.applied, "seed {seed}: {o}");
            }
        }
        // The flood reached the ceiling: entries past it were refused, or the log holds all
        // it may and what came on top is spare.
        let fill = log.fill();
        let refused = (state.rejected.iter()).any(|(_, why)| *why == PAST_CEILING);
        if refused || fill.entries >= fill.max_entries {
            past_ceiling += 1;
        }
        check_every_step(log, &state);
        same_as_worked_out_again(log, &h.world.devices[0].id, &format!("seed {seed}"));
    }
    assert!(at_five > 0, "no world went up to level five");
    assert!(past_ceiling > 0, "no world reached the ceiling");
}

/// Two devices that sign on each other's entries, on the log as it was before the raise to
/// five and never on the raise, are held to the ceiling all the same: it holds every entry
/// of a log at five, wherever it goes in the order.
#[test]
fn a_flood_apart_from_the_raise_stops_at_the_ceiling() {
    let (mut log, ana, laptop, _) = group(Level::Four);
    let mut apart = log.clone();
    log.raise_level(&ana.id, Level::Five, 10).unwrap();
    for n in 0..u64::from(SMALL) * 3 {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        rename(&mut apart, who, n).unwrap();
    }
    log.insert_all(apart.entries_in_order().into_iter().cloned());
    assert_eq!(log.state().level, Level::Five);
    // The raise names a higher level: where the order puts it after the flood, it goes into
    // the reserve, one entry past the ceiling.
    let fill = log.fill();
    assert!(fill.entries <= SMALL + 1 && fill.full(), "{fill:?}");
}

/// A removal or leave is never held back for itself, but what it brings in (entries part
/// of no run it is signed on, which honest devices never sign on) goes past the ceiling only
/// so far: past that, it is held too, and one signed on the log as it is applies.
#[test]
fn what_a_leave_brings_in_goes_only_so_far_past_the_ceiling() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    for n in 0.. {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        if rename(&mut log, who, n).is_err() {
            break;
        }
    }
    let group_id = log.group_id();
    let mut parents = log.heads();
    let slack = crate::budget::SLACK.entries;
    for n in 0..slack + 10 {
        let junk = Body::Unknown {
            kind: 200,
            bytes: vec![n as u8; 1_000],
        };
        let entry = Entry::sign(&bea.id, group_id, parents, 1_000 + u64::from(n), junk);
        parents = vec![entry.hash()];
        log.insert(entry).unwrap();
    }
    let leave = Entry::sign(&bea.id, group_id, parents, 5_000, Body::Leave);
    assert_eq!(
        log.insert(leave.clone()),
        Err(StateError::NotAllowed(PAST_CEILING))
    );
    assert!(log.fill().entries <= SMALL + slack, "{:?}", log.fill());
    // Signed on the log as it is, Bea leaves.
    log.append(&bea.id, Body::Leave, 5_001).unwrap();
    assert!(!log.state().is_member(&bea.key()));
}

/// A removal or a room rule written apart from a move, by a member that did not see it:
/// every copy agrees, the move stays made wherever the order puts them, and the new log is
/// the group the move saw. A room rule the move is not signed on applies to nothing; a
/// removal the move did not see is named, for the first member to remove that member again
/// in the new group.
#[test]
fn what_is_written_apart_from_a_move_never_undoes_it() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    log.set_ceiling(CEILING.entries, CEILING.bytes);
    let mut laptops = log.clone();
    let removal = laptops.remove(&laptop.id, bea.key(), 10).unwrap();
    let mut rules = log.clone();
    let rule = rules
        .keep_out(&laptop.id, crate::entry::FIRST_ROOM, bea.key(), None, 10)
        .unwrap();
    let new = log.new_log_for_move(&ana.id, 11).unwrap();
    let moved = log.move_to(&ana.id, &new, 12).unwrap();
    assert_eq!(log.apart_from_move(), Default::default());

    let mut with_rule = log.clone();
    with_rule.insert(rule.clone()).unwrap();
    let mut other = rules.clone();
    other.insert(moved.clone()).unwrap();
    for copy in [&with_rule, &other] {
        let state = copy.state();
        assert_eq!(copy.moved().map(|(h, _)| h), Some(moved.hash()));
        assert!(state.rejected.contains(&(rule.hash(), MOVED_AWAY)));
        assert!(state.rules.kept_out.is_empty());
        copy.follows(&new).unwrap();
    }

    let mut with_removal = log.clone();
    with_removal.insert(removal.clone()).unwrap();
    let mut other = laptops.clone();
    other.insert(moved.clone()).unwrap();
    for copy in [&with_removal, &other] {
        assert_eq!(copy.state().applied, with_removal.state().applied);
        assert_eq!(copy.moved().map(|(h, _)| h), Some(moved.hash()));
        copy.follows(&new).unwrap();
        assert_eq!(copy.apart_from_move().removed, vec![bea.key()]);
    }
    assert!(new.state().is_member(&bea.key()));
}

/// Entries written apart from a move that the order puts before it, by devices that came
/// in after its author, arriving after members followed: the move stays made, and the log
/// a member followed to stays the one it follows.
#[test]
fn a_move_stays_made_whatever_comes_after() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    log.set_ceiling(CEILING.entries, CEILING.bytes);
    let mut late = log.clone();
    let mut written = Vec::new();
    for n in 0..6 {
        let who = if n % 2 == 0 { &bea } else { &laptop };
        written.push(rename(&mut late, who, n).unwrap());
    }
    written.push(late.rotate(&bea.id, 20).unwrap());
    let new = log.new_log_for_move(&ana.id, 30).unwrap();
    let moved = log.move_to(&ana.id, &new, 31).unwrap();
    log.follows(&new).unwrap();
    for entry in written {
        log.insert(entry).unwrap();
        assert_eq!(log.moved().map(|(h, _)| h), Some(moved.hash()));
        log.follows(&new).unwrap();
    }
    let mut theirs = late.clone();
    theirs.insert(moved.clone()).unwrap();
    assert_eq!(theirs.state().applied, log.state().applied);
    assert_eq!(theirs.moved().map(|(h, _)| h), Some(moved.hash()));
}

/// A raise to five that does not apply (its author lost its place apart from it) still
/// orders the log at five: the ceiling holds it, and the first member can still move the
/// group, as its group is at level four.
#[test]
fn a_log_ordered_at_five_can_move_whatever_level_its_group_is_at() {
    let (mut log, ana, laptop, bea) = group(Level::Four);
    let mut beas = log.clone();
    let raise = beas.raise_level(&bea.id, Level::Five, 10).unwrap();
    log.append(&ana.id, Body::Demote { member: bea.key() }, 10)
        .unwrap();
    log.append(&laptop.id, Body::Demote { member: bea.key() }, 11)
        .unwrap();
    log.insert(raise.clone()).unwrap();
    let state = log.state();
    assert_eq!(state.log_level, Level::Five);
    if state.level == Level::Four {
        assert!(!state.applied.contains(&raise.hash()));
    }
    for n in 0.. {
        let who = if n % 2 == 0 { &ana } else { &laptop };
        if let Err(why) = rename(&mut log, who, n) {
            assert_eq!(why, StateError::NotAllowed(PAST_CEILING));
            break;
        }
    }
    assert!(log.fill().entries <= SMALL);
    let new = log.new_log_for_move(&ana.id, 1_000).unwrap();
    log.move_to(&ana.id, &new, 1_001).unwrap();
    log.follows(&new).unwrap();
}

/// A removal written while the log filled up, on an entry of its author's that the order
/// then puts past the ceiling, still applies: entries of devices that came in later went
/// first, and its author never saw them.
#[test]
fn a_removal_on_an_entry_past_the_ceiling_still_applies() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    let mut theirs = log.clone();
    rename(&mut log, &ana, 0).unwrap();
    let removal = log.remove(&ana.id, bea.key(), 1).unwrap();
    for n in 0..u64::from(SMALL) * 2 {
        let who = if n % 2 == 0 { &bea } else { &laptop };
        let _ = rename(&mut theirs, who, 100 + n);
    }
    log.insert_all(theirs.entries_in_order().into_iter().cloned());
    let state = log.state();
    assert!(
        state.applied.contains(&removal.hash()),
        "{:?}",
        state.rejected
    );
    assert!(!state.is_member(&bea.key()));
}

/// Adds of devices nobody uses, each on the same entries so that no member limit holds
/// them, then a removal of each: the adds stop at the ceiling, the removals at the slack
/// past it, and the group can still move, past that. A log never holds more than the
/// ceiling, the slack and the reserve.
#[test]
fn adds_and_their_removals_stop_at_the_slack() {
    let (base, ana, laptop, _) = group(Level::Five);
    let mut log = base.clone();
    let room = crate::budget::SLACK.entries + 100;
    log.set_ceiling(room, CEILING.bytes);
    let mut adds = Vec::new();
    let mut removals = Vec::new();
    for n in 0..u64::from(room) {
        let mut one = base.clone();
        let add = one
            .add(&laptop.id, Device::new().key(), "nobody", 10 + n)
            .unwrap();
        let Body::Add { member, .. } = add.body else {
            unreachable!()
        };
        removals.push(one.remove(&ana.id, member, 10_000 + n).unwrap());
        adds.push(add);
    }
    log.insert_all(adds.into_iter().chain(removals));
    let (slack, reserve) = (crate::budget::SLACK, crate::budget::RESERVE);
    let fill = log.fill();
    assert!(fill.entries <= room + slack.entries, "{fill:?}");
    assert!(fill.bytes <= CEILING.bytes + slack.bytes, "{fill:?}");
    let held = (log.state().rejected.iter())
        .filter(|(_, why)| *why == PAST_CEILING)
        .count();
    assert!(held > 0, "the slack holds the removals back");
    let new = log.new_log_for_move(&ana.id, 50_000).unwrap();
    log.move_to(&ana.id, &new, 50_001).unwrap();
    log.follows(&new).unwrap();
    let fill = log.fill();
    assert!(
        fill.entries <= room + slack.entries + reserve.entries,
        "{fill:?}"
    );
}

/// A raise to five past its author's run is spare: a copy may never have it, so it orders
/// nothing. Every copy judges a move written on the log as it is at four, as at four,
/// whether the raise came first or not, and keeps and applies the same.
#[test]
fn a_raise_past_its_run_orders_nothing() {
    let (mut base, ana, _, bea) = group(Level::Four);
    // Room in Ana's run for one more entry.
    let run = base.full().budget.run_of(&ana.key()).entries + 1;
    base.set_run_room(run, CEILING.bytes);
    let (group_id, heads) = (base.group_id(), base.heads());
    // Ana writes renames and the raise apart, all on the same entries: whichever the order
    // puts after the first is past her run. Which goes first is the smallest hash: tried
    // until it is not the raise.
    let (apart, raise, mut first) = (0..64u64)
        .find_map(|t| {
            let mut apart: Vec<Entry> = (0..4)
                .map(|n| {
                    let body = Body::Rename {
                        name: format!("apart {n}"),
                    };
                    Entry::sign(&ana.id, group_id, heads.clone(), 20 + n + t, body)
                })
                .collect();
            let raise = Entry::sign(
                &ana.id,
                group_id,
                heads.clone(),
                100 + t,
                Body::Level { level: 5 },
            );
            apart.push(raise.clone());
            let mut first = base.clone();
            first.insert_all(apart.clone());
            let past_run = (crate::budget::PAST_RUN, raise.hash());
            let spare = (first.state().rejected.iter()).any(|(h, why)| (*why, *h) == past_run);
            spare.then_some((apart, raise, first))
        })
        .expect("the raise goes past Ana's run");
    assert_eq!(first.state().log_level, Level::Four);
    // Bea, not first, writes a move on the log as she has it, without the raise.
    let mut beas = base.clone();
    beas.insert_all(apart.iter().filter(|e| e.hash() != raise.hash()).cloned());
    let forged = Moved {
        to: GroupId::random().unwrap(),
        setup: [7; 32],
    };
    let moved = Entry::sign(&bea.id, group_id, beas.heads(), 40, forged.body());
    beas.insert(moved.clone()).unwrap();
    first.insert(moved.clone()).unwrap();
    beas.insert(raise.clone()).unwrap();
    let mut shuffled = GroupLog::empty(group_id);
    shuffled.set_run_room(run, CEILING.bytes);
    let mut all: Vec<Entry> = first.entries().cloned().collect();
    all.sort_by_key(Entry::hash);
    shuffled.insert_all(all);
    for copy in [&beas, &shuffled] {
        let theirs: BTreeSet<EntryHash> = copy.hashes().copied().collect();
        let ours: BTreeSet<EntryHash> = first.hashes().copied().collect();
        assert_eq!(theirs, ours);
        assert_eq!(copy.ordered(), first.ordered());
        assert_eq!(copy.state().applied, first.state().applied);
        assert_eq!(copy.state().log_level, Level::Four);
        assert_eq!(copy.moved(), None);
    }
}

/// A full log with more live entries than an entry is signed on (those written apart from
/// each other): the first member still moves it, on the heads it signs on.
#[test]
fn a_full_log_with_many_heads_still_moves() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    log.set_ceiling(100, CEILING.bytes);
    let (group_id, heads) = (log.group_id(), log.heads());
    let apart: Vec<Entry> = (0..120u64)
        .map(|n| {
            let who = [&ana, &laptop, &bea][n as usize % 3];
            let body = Body::Rename {
                name: format!("apart {n}"),
            };
            Entry::sign(&who.id, group_id, heads.clone(), 20 + n, body)
        })
        .collect();
    log.insert_all(apart);
    assert!(log.fill().full(), "{:?}", log.fill());
    assert!(log.full().tips.len() > 64, "{} tips", log.full().tips.len());
    let new = log.new_log_for_move(&ana.id, 1_000).unwrap();
    let moved = log.move_to(&ana.id, &new, 1_001).unwrap();
    assert_eq!(log.moved().map(|(h, _)| h), Some(moved.hash()));
    log.follows(&new).unwrap();
}

/// Moves forged by a member that is not first, each on the log as it was before the raise to
/// five (where its level kept them as a kind it did not know): they never count, never
/// apply and order nothing, whoever came in first, and the first member removes the forger
/// and moves the group.
#[test]
fn moves_forged_before_the_raise_count_for_nothing() {
    let (slack, reserve) = (crate::budget::SLACK, crate::budget::RESERVE);
    for forger_first in [false, true] {
        let (mut log, ana, laptop, bea) = group(Level::Four);
        // The forger came in after the raiser, or before it.
        let (raiser, forger) = if forger_first {
            (&bea, &laptop)
        } else {
            (&ana, &bea)
        };
        let (group_id, heads) = (log.group_id(), log.heads());
        let raise = Entry::sign(
            &raiser.id,
            group_id,
            heads.clone(),
            10,
            Body::Level { level: 5 },
        );
        let forged: Vec<Entry> = (0..u64::from(SMALL + slack.entries + reserve.entries) + 20)
            .map(|n| {
                let body = Moved {
                    to: GroupId::random().unwrap(),
                    setup: [n as u8; 32],
                }
                .body();
                Entry::sign(&forger.id, group_id, heads.clone(), 20 + n, body)
            })
            .collect();
        log.insert_all(std::iter::once(raise).chain(forged.iter().cloned()));
        let state = log.state();
        assert_eq!(state.log_level, Level::Five, "forger first: {forger_first}");
        assert_eq!(log.moved(), None);
        assert!(forged.iter().all(|e| !state.applied.contains(&e.hash())));
        assert!(log.fill().entries <= SMALL, "{:?}", log.fill());
        // Two devices flood after it: the ceiling holds them.
        for n in 0..u64::from(SMALL) * 2 {
            let who = if n % 2 == 0 { &ana } else { &laptop };
            if rename(&mut log, who, 1_000 + n).is_err() {
                break;
            }
        }
        assert!(log.fill().entries <= SMALL, "{:?}", log.fill());
        if forger.key() != laptop.key() {
            log.remove(&ana.id, forger.key(), 5_000).unwrap();
        }
        let new = log.new_log_for_move(&ana.id, 5_001).unwrap();
        log.move_to(&ana.id, &new, 5_002).unwrap();
        log.follows(&new).unwrap();
    }
}

/// A first member that left cannot move the group afterwards on what it saw before, even
/// where the order puts that move before its leave: what the others wrote since stays.
#[test]
fn a_first_member_that_left_moves_nothing() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    log.set_ceiling(CEILING.entries, CEILING.bytes);
    let before = log.heads();
    let new = log.new_log_for_move(&ana.id, 10).unwrap();
    // A leave whose hash is in the upper half, so that a move found below can go first.
    let leave = (0..256u64)
        .map(|t| Entry::sign(&ana.id, log.group_id(), log.heads(), 11 + t, Body::Leave))
        .find(|e| e.hash()[0] >= 0x80)
        .unwrap();
    log.insert(leave.clone()).unwrap();
    let renames = [
        rename(&mut log, &bea, 600).unwrap(),
        rename(&mut log, &laptop, 601).unwrap(),
    ];
    let group_id = log.group_id();
    let setup = new.heads()[0];
    // Signed on what Ana saw before leaving, tried until the order puts it first.
    let moved = (0..256u64)
        .map(|t| {
            let body = Moved {
                to: new.group_id(),
                setup,
            }
            .body();
            Entry::sign(&ana.id, group_id, before.clone(), 20 + t, body)
        })
        .find(|e| e.hash() < leave.hash())
        .unwrap();
    log.insert(moved.clone()).unwrap();
    let state = log.state();
    assert_eq!(log.moved(), None);
    assert!(state.rejected.contains(&(moved.hash(), NOT_A_MOVE)));
    assert!(state.applied.contains(&leave.hash()));
    assert!(renames.iter().all(|e| state.applied.contains(&e.hash())));
    assert!(!state.is_member(&ana.key()));
}

/// Before level five, a raise past its author's run orders the log as it always did: at the
/// level it names.
#[test]
fn before_five_a_raise_past_its_run_still_orders() {
    let (mut log, ana, _, _) = group(Level::Two);
    let run = log.full().budget.run_of(&ana.key()).entries + 1;
    log.set_run_room(run, CEILING.bytes);
    let (group_id, heads) = (log.group_id(), log.heads());
    let found = (0..64u64).find_map(|t| {
        let mut copy = log.clone();
        let rename = Body::Rename {
            name: "apart".into(),
        };
        // Both sign their times, so each try orders them anew.
        let first = Entry::sign(&ana.id, group_id, heads.clone(), 20 + t, rename);
        let raise = Entry::sign(
            &ana.id,
            group_id,
            heads.clone(),
            100 + t,
            Body::Level { level: 3 },
        );
        copy.insert_all([first, raise.clone()]);
        let past_run = (raise.hash(), crate::budget::PAST_RUN);
        copy.state().rejected.contains(&past_run).then_some(copy)
    });
    let copy = found.expect("the raise goes past Ana's run");
    assert_eq!(copy.full().budget.level(), Level::Three);
}

/// A copy fresh from nothing, given `from`'s entries in its order.
fn fresh(from: &GroupLog) -> GroupLog {
    let mut log = GroupLog::empty(from.group_id());
    log.insert_all(from.entries_in_order().into_iter().cloned());
    log
}

/// Whether a `Moved` is a move is judged on its author's view, whenever its entries come:
/// a copy that orders the log while the move and the raise before it still wait on an
/// entry they are signed on moves the group all the same once that entry comes.
#[test]
fn a_move_counts_the_same_whatever_came_first() {
    let (mut base, ana, _, bea) = group(Level::Four);
    base.set_ceiling(CEILING.entries, CEILING.bytes);
    // Bea raises to five; Ana, apart, writes a rename, a raise of her own and a move.
    let mut beas = base.clone();
    beas.raise_level(&bea.id, Level::Five, 10).unwrap();
    let mut anas = base.clone();
    let renamed = rename(&mut anas, &ana, 20).unwrap();
    let raise = anas.raise_level(&ana.id, Level::Five, 21).unwrap();
    let new = anas.new_log_for_move(&ana.id, 22).unwrap();
    let moved = anas.move_to(&ana.id, &new, 23).unwrap();
    // In the order written.
    let mut a = fresh(&beas);
    a.insert_all([renamed.clone(), raise.clone(), moved.clone()]);
    // The raise and the move first: they wait on the rename, and the log is ordered (at
    // five, by Bea's raise) while they wait.
    let mut b = fresh(&beas);
    b.insert_some([raise.clone(), moved.clone()]);
    assert!(b.unjudged.contains(&moved.hash()));
    let _ = b.fill();
    b.insert_some([renamed.clone()]);
    assert_eq!(a.moved().map(|(h, _)| h), Some(moved.hash()));
    assert_eq!(b.moved(), a.moved());
    assert_eq!(b.ordered(), a.ordered());
    assert_eq!(b.state().applied, a.state().applied);
    assert_eq!(b.fill(), a.fill());
    // And read again from nothing.
    assert_eq!(fresh(&b).moved(), a.moved());
}

/// A first member that writes a move apart from its own leave takes nothing from the move
/// that will apply: its move never applies, and the reserve stays for the next first
/// member's, on a log full to the ceiling.
#[test]
fn a_move_by_a_leaver_leaves_the_reserve() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    for n in 0..u64::from(SMALL) * 2 {
        let who = if n % 2 == 0 { &laptop } else { &bea };
        if rename(&mut log, who, 100 + n).is_err() {
            break;
        }
    }
    assert!(log.fill().full(), "{:?}", log.fill());
    let heads = log.heads();
    let group_id = log.group_id();
    let new = log.new_log_for_move(&ana.id, 1_000).unwrap();
    let forged = Moved {
        to: new.group_id(),
        setup: new.heads()[0],
    };
    let moved = Entry::sign(&ana.id, group_id, heads.clone(), 1_001, forged.body());
    let leave = Entry::sign(&ana.id, group_id, heads, 1_002, Body::Leave);
    log.insert_all([moved.clone(), leave.clone()]);
    let state = log.state();
    assert!(state.applied.contains(&leave.hash()));
    // Past the ceiling with no reserve, or refused for its author's leave: either way it
    // moves nothing.
    assert!(
        state.rejected.iter().any(|(h, _)| *h == moved.hash()),
        "{state:?}"
    );
    assert_eq!(log.moved(), None);
    // The next first member moves the group, whoever it is.
    let next = [&laptop, &bea]
        .into_iter()
        .find(|d| log.new_log_for_move(&d.id, 2_000).is_ok())
        .unwrap_or_else(|| {
            panic!(
                "{:?} {:?}",
                log.new_log_for_move(&laptop.id, 2_000).err(),
                log.new_log_for_move(&bea.id, 2_000).err()
            )
        });
    let new = log.new_log_for_move(&next.id, 2_000).unwrap();
    let theirs = log.move_to(&next.id, &new, 2_001).unwrap();
    assert_eq!(log.moved().map(|(h, _)| h), Some(theirs.hash()));
    assert_eq!(fresh(&log).moved(), log.moved());
}

/// What members wrote apart from a move about who is in the group, as the move went: the
/// first member writes it again in the new log, once. A removal is written again; so is
/// an add, or a join a manager let in, by a manager still in the new group; not one by a
/// member those entries took out, nor a join nobody let in. What members decide in the new
/// log later stands.
#[test]
fn what_was_written_apart_from_a_move_is_written_again_once() {
    let (mut log, ana, laptop, bea) = group(Level::Five);
    log.set_ceiling(CEILING.entries, CEILING.bytes);
    let (dan, eve, eva) = (Device::new(), Device::new(), Device::new());
    let mut laptops = log.clone();
    let removal = laptops.remove(&laptop.id, bea.key(), 10).unwrap();
    let add = laptops.add(&laptop.id, dan.key(), "Dan", 11).unwrap();
    let (ticket, _) = laptops.invite(&laptop.id, u64::MAX, 12).unwrap();
    let invite = laptops.get(&laptops.heads()[0]).unwrap().clone();
    let body = Body::Join {
        ticket,
        name: "Eva".into(),
    };
    let join = Entry::sign(&eva.id, laptops.group_id(), laptops.heads(), 13, body);
    laptops.insert(join.clone()).unwrap();
    let welcome = laptops.let_in(&laptop.id, &join.hash(), 14).unwrap();
    assert!(laptops.state().is_member(&eva.key()));
    let mut beas = log.clone();
    let beas_add = beas.add(&bea.id, eve.key(), "Eve", 10).unwrap();

    let mut new = log.new_log_for_move(&ana.id, 20).unwrap();
    log.move_to(&ana.id, &new, 21).unwrap();
    // A join nobody let in brings nobody: any member can sign one with a ticket it saw.
    let (ticket, _) = beas.invite(&bea.id, u64::MAX, 11).unwrap();
    let beas_invite = beas.get(&beas.heads()[0]).unwrap().clone();
    let body = Body::Join {
        ticket,
        name: "Ivy".into(),
    };
    let ivy = Device::new();
    let lone_join = Entry::sign(&ivy.id, beas.group_id(), beas.heads(), 12, body);
    for entry in [
        removal,
        add,
        invite,
        join,
        welcome,
        beas_add,
        beas_invite,
        lone_join,
    ] {
        log.insert(entry).unwrap();
    }
    let apart = log.apart_from_move();
    assert_eq!(apart.removed, vec![bea.key()]);
    let mut added: Vec<_> = apart.added.iter().map(|a| (a.member, a.by)).collect();
    added.sort();
    let mut expected = vec![
        (dan.key(), laptop.key()),
        (eve.key(), bea.key()),
        (eva.key(), laptop.key()),
    ];
    expected.sort();
    assert_eq!(added, expected);

    // Only the first member writes them.
    assert!(
        new.again_after_move(&laptop.id, &log, 30)
            .unwrap()
            .is_empty()
    );
    let written = new.again_after_move(&ana.id, &log, 30).unwrap();
    assert_eq!(written.len(), 3, "{written:?}");
    let state = new.state();
    assert!(!state.is_member(&bea.key()));
    assert!(state.is_member(&dan.key()));
    assert_eq!(state.members[&eva.key()].name, "Eva");
    assert!(!state.is_member(&eve.key()), "Bea's add goes with her");
    assert!(new.again_after_move(&ana.id, &log, 31).unwrap().is_empty());

    // Bea added back and Dan removed in the new log: that stands.
    new.add(&laptop.id, bea.key(), "Bea", 40).unwrap();
    new.remove(&ana.id, dan.key(), 41).unwrap();
    assert!(new.again_after_move(&ana.id, &log, 42).unwrap().is_empty());
    assert!(new.state().is_member(&bea.key()));
    assert!(!new.state().is_member(&dan.key()));

    // A log the group did not move to gets nothing.
    let (mut other, ..) = group(Level::Five);
    let mut elsewhere = GroupLog::create(&ana.id, "elsewhere", 50).unwrap();
    assert!(
        elsewhere
            .again_after_move(&ana.id, &log, 51)
            .unwrap()
            .is_empty()
    );
    assert!(
        other
            .again_after_move(&ana.id, &log, 51)
            .unwrap()
            .is_empty()
    );
}
