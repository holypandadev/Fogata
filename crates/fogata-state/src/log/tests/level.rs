//! The levels of judgment (see [`Level`]): the judgment of the level before, fogata-state
//! as it was at level one (`crates/fogata-state/before`), run beside this one; and the
//! judgment of level two pinned on a corpus of entries, so that a change to what it keeps
//! or applies fails here unless it comes as a new level.

use std::collections::HashMap;

use fogata_state_before as before;

use super::{hostile::Hostile, *};
use crate::{
    budget::RUN_ROOM,
    state::{LEVEL_NOT_HIGHER, LEVEL_UNKNOWN},
};

/// The entries of `log` in the order they came to it.
fn as_they_came(log: &GroupLog) -> Vec<EntryHash> {
    let mut came: Vec<(u64, EntryHash)> = log.arrived.iter().map(|(h, (n, _))| (*n, *h)).collect();
    came.sort_unstable();
    came.into_iter().map(|(_, h)| h).collect()
}

/// `entry` as the level before reads it.
fn before_level(entry: &Entry) -> before::Entry {
    before::Entry::decode(&entry.encode()).expect("one encoding of entries")
}

/// The log as the level before judges it (see [`Level`]): the entries of `log`, in the
/// order they came to it, taken by the fogata-state of level one
/// (`crates/fogata-state/before`), that level's judgment as it ran. One it refuses is
/// not kept there, and what is signed on it waits, as on any copy of that level.
fn at_the_level_before(log: &GroupLog) -> before::GroupLog {
    let mut old = before::GroupLog::empty(log.group_id());
    for hash in as_they_came(log) {
        let _ = old.insert(before_level(&log.entries[&hash]));
    }
    old
}

/// Takes into `old`, the level before's copy of `log`, the entries that came to `log`
/// from the `from`th on (counted as [`GroupLog::arrived`] counts them); returns the
/// count the next one will have.
fn take_at_the_level_before(old: &mut before::GroupLog, log: &GroupLog, from: u64) -> u64 {
    let mut came: Vec<(u64, EntryHash)> = log
        .arrived
        .iter()
        .filter(|(_, (n, _))| *n >= from)
        .map(|(h, (n, _))| (*n, *h))
        .collect();
    came.sort_unstable();
    let mut next = from;
    for (n, hash) in came {
        let _ = old.insert(before_level(&log.entries[&hash]));
        next = n + 1;
    }
    next
}

/// What the level before and this one make of `log` differently, each with the entry
/// it is about when it is about one: an entry one keeps and the other does not, one
/// that applies at one level and not the other (`applied before`, `applied now`), and
/// each part of the group they disagree on, the order of the entries both keep among
/// them. The room rules are compared only when every rule applies alike: one applying
/// differently changes them, as it should.
fn levels_disagree_on(log: &GroupLog, old: &before::GroupLog) -> Vec<(Option<EntryHash>, String)> {
    let mut out = Vec::new();
    let kept_now: BTreeSet<EntryHash> = log.hashes().copied().collect();
    let kept_before: BTreeSet<EntryHash> = old.hashes().copied().collect();
    for hash in kept_now.difference(&kept_before) {
        let what = format!("kept now, not before: {:?}", log.entries[hash].body);
        out.push((Some(*hash), what));
    }
    for hash in kept_before.difference(&kept_now) {
        let what = format!("kept before, not now: {:?}", old.get(hash).unwrap().body);
        out.push((Some(*hash), what));
    }
    let (now, then) = (log.state(), old.state());
    let applied_now: BTreeSet<EntryHash> = now.applied.iter().copied().collect();
    let applied_before: BTreeSet<EntryHash> = then.applied.iter().copied().collect();
    let mut rules_alike = true;
    for hash in applied_now.difference(&applied_before) {
        let body = &log.entries[hash].body;
        rules_alike &= !body.is_room_rule();
        out.push((Some(*hash), format!("applied now: {body:?}")));
    }
    for hash in applied_before.difference(&applied_now) {
        let body = &old.get(hash).unwrap().body;
        rules_alike &= !body.is_room_rule();
        out.push((Some(*hash), format!("applied before: {body:?}")));
    }
    let both = |order: Vec<EntryHash>| -> Vec<EntryHash> {
        order
            .into_iter()
            .filter(|h| kept_now.contains(h) && kept_before.contains(h))
            .collect()
    };
    let limits_now = (now.max_members, now.max_in_room);
    let limits_then = (then.max_members, then.max_in_room);
    let parts = [
        (
            "name",
            format!("{:?}", now.name),
            format!("{:?}", then.name),
        ),
        (
            "icon",
            format!("{:?}", now.icon),
            format!("{:?}", then.icon),
        ),
        (
            "members",
            format!("{:?}", now.members),
            format!("{:?}", then.members),
        ),
        (
            "ladder",
            format!("{:?}", now.ladder),
            format!("{:?}", then.ladder),
        ),
        (
            "managers",
            format!("{:?}", now.managers),
            format!("{:?}", then.managers),
        ),
        (
            "epochs",
            format!("{:?}", now.epochs),
            format!("{:?}", then.epochs),
        ),
        (
            "rooms",
            format!("{:?}", now.rooms),
            format!("{:?}", then.rooms),
        ),
        (
            "limits",
            format!("{limits_now:?}"),
            format!("{limits_then:?}"),
        ),
        (
            "tickets",
            format!("{:?}", now.used_tickets),
            format!("{:?}", then.used_tickets),
        ),
        (
            "order",
            format!("{:?}", both(log.ordered())),
            format!("{:?}", both(old.ordered())),
        ),
    ];
    for (what, a, b) in parts {
        if a != b {
            out.push((None, format!("{what} now {a}, before {b}")));
        }
    }
    if rules_alike && format!("{:?}", now.rules) != format!("{:?}", then.rules) {
        let what = format!("rules now {:?}, before {:?}", now.rules, then.rules);
        out.push((None, what));
    }
    out
}

/// Whether a disagreement of [`levels_disagree_on`] is one honest devices with clocks
/// that agree bring about: a room rule signed apart from a loss of its author (each on
/// its own copy, not knowing of the other), which the level before judged by their
/// times and this one by their places (it is outranked). Nothing else is.
fn honest_levels_disagree(log: &GroupLog, hash: Option<EntryHash>, what: &str) -> bool {
    match hash {
        Some(hash) if what.starts_with("applied before: ") => {
            log.entries[&hash].body.is_room_rule() && log.stands_cache().of(&hash).apart
        }
        _ => false,
    }
}

/// What honest devices write is judged alike at the level before (see
/// [`at_the_level_before`]): in random histories of members that rotate, demote, write
/// room rules and let devices in with invites, two managers at once, with clocks that
/// only go forward, each copy and its copy at the level before keep the same entries,
/// apply the same (a room rule signed apart from a loss of its author aside, see
/// [`honest_levels_disagree`]) and make the same group. `FOGATA_LADDER_SEEDS` sets how
/// many histories run (4 by default).
#[test]
fn what_honest_devices_write_is_judged_alike_at_the_level_before() {
    let seeds = std::env::var("FOGATA_LADDER_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4u64);
    // One history per seed, compared every few steps and at the end.
    let history = |seed: u64| {
        let mut kinds = BTreeMap::new();
        let mut apart = 0;
        let mut world = LadderWorld::new(seed, 6);
        let mut olds: Vec<before::GroupLog> = world
            .logs
            .iter()
            .map(|log| before::GroupLog::empty(log.group_id()))
            .collect();
        let mut taken = vec![0; world.logs.len()];
        for i in 0..=120 {
            match i {
                0 => {}
                i if i % 4 == 0 => world.race_step(),
                _ => world.step(),
            }
            for (k, log) in world.logs.iter().enumerate() {
                let was = taken[k];
                taken[k] = take_at_the_level_before(&mut olds[k], log, was);
                for (hash, (n, _)) in &log.arrived {
                    if *n < was {
                        continue;
                    }
                    let kind = format!("{:?}", log.entries[hash].body);
                    let kind = kind.split([' ', '{', '(']).next().unwrap_or("").to_string();
                    *kinds.entry(kind).or_insert(0) += 1;
                }
                if i % 5 != 0 && i != 120 {
                    continue;
                }
                for (hash, what) in levels_disagree_on(log, &olds[k]) {
                    assert!(
                        honest_levels_disagree(log, hash, &what),
                        "seed {seed}, step {i}, copy {k}: {what}"
                    );
                    apart += 1;
                }
            }
        }
        (kinds, apart)
    };
    let (mut kinds, mut apart) = (BTreeMap::new(), 0);
    std::thread::scope(|threads| {
        let runs: Vec<_> = (1..=seeds)
            .map(|seed| threads.spawn(move || history(seed)))
            .collect();
        for run in runs {
            let (of_seed, apart_in_seed) = run.join().expect("a history that passed");
            for (kind, n) in of_seed {
                *kinds.entry(kind).or_insert(0) += n;
            }
            apart += apart_in_seed;
        }
    });
    println!("{kinds:?}; {apart} rules signed apart from a loss of their author");
    for kind in [
        "Join",
        "Rotate",
        "KeepOut",
        "RoomAccess",
        "Demote",
        "Welcome",
    ] {
        assert!(kinds.contains_key(kind) || seeds < 4, "no {kind}");
    }
}

/// Entries the two levels judge apart, which honest devices with clocks that agree
/// never write (see [`at_the_level_before`]).
#[test]
fn the_level_before_tells_apart_what_honest_devices_never_write() {
    let refused = |e: &'static str| Err(before::StateError::NotAllowed(e));
    // A join dated after its ticket expired, by a device whose clock is ahead: kept now
    // (the manager letting it in checks the ticket), not before.
    let (log, ana, bea, carlos) = two_managers();
    let ticket = log.issue_ticket(&ana.id, 1_000).unwrap();
    let mut joined = log.clone();
    let mut old = at_the_level_before(&joined);
    let late = join(&mut joined, &Device::new(), ticket, 2_000);
    assert!(joined.get(&late.hash()).is_some());
    assert_eq!(old.insert(before_level(&late)), refused("ticket expired"));
    // A rotation of a member that does not manage, for a key sent in an add that did
    // not apply: kept now, not before.
    let (mut at_ana, mut at_bea) = (log.clone(), log.clone());
    at_ana
        .append(&ana.id, Body::Demote { member: bea.key() }, 5)
        .unwrap();
    at_bea.add(&bea.id, Device::new().key(), "x", 5).unwrap();
    met(&mut at_ana, &mut at_bea);
    let mut old = at_the_level_before(&at_ana);
    assert!(levels_disagree_on(&at_ana, &old).is_empty());
    let rotate = at_ana.rotate(&bea.id, 6).unwrap();
    assert_eq!(
        old.insert(before_level(&rotate)),
        refused(before::ROTATE_NOT_NEEDED)
    );
    // A rule signed on a loss of its author but dated before it: refused now (it is
    // judged on the place its author has), kept and applied before (judged on the
    // place it had by the times).
    let mut log = log;
    log.append(&ana.id, Body::Demote { member: bea.key() }, 3_000)
        .unwrap();
    let mut old = at_the_level_before(&log);
    let rule = Entry::sign(
        &bea.id,
        log.group_id(),
        log.heads(),
        2_500,
        Body::RoomAccess {
            room: FIRST_ROOM,
            members: Some(vec![carlos.key()]),
        },
    );
    assert!(log.insert(rule.clone()).is_err());
    assert_eq!(old.insert(before_level(&rule)), Ok(true));
    assert!(old.state().applied.contains(&rule.hash()));
    assert_eq!(
        levels_disagree_on(&log, &old),
        vec![
            (
                Some(rule.hash()),
                format!("kept before, not now: {:?}", rule.body)
            ),
            (
                Some(rule.hash()),
                format!("applied before: {:?}", rule.body)
            ),
        ]
    );
}

/// A group judges at its level (see [`Level`]): a `Level` entry raises it only to a
/// level this version judges at, above the group's, by a manager; one naming a level
/// this version does not judge at is refused, and what is signed on it waits, as on a
/// copy that stopped at that entry. A level entry with more in it than a level is a
/// kind this version does not know: kept, changing nothing.
#[test]
fn the_group_judges_at_its_level_and_stops_at_one_it_does_not_know() {
    let (mut log, ana, _bea, carlos) = two_managers();
    let group = log.group_id();
    assert_eq!(log.level(), Level::Two);
    assert_eq!(
        Level::numbered(Level::CURRENT.number()),
        Some(Level::CURRENT)
    );
    assert_eq!(
        log.raise_level(&ana.id, Level::Two, 5),
        Err(StateError::NotAllowed(LEVEL_NOT_HIGHER))
    );
    let three = Entry::sign(&ana.id, group, log.heads(), 5, Body::Level { level: 3 });
    assert_eq!(
        log.insert(three.clone()),
        Err(StateError::NotAllowed(LEVEL_UNKNOWN))
    );
    let on_it = Entry::sign(
        &ana.id,
        group,
        vec![three.hash()],
        6,
        Body::Rename { name: "h".into() },
    );
    assert_eq!(log.insert(on_it.clone()), Ok(true));
    assert!(log.waiting().contains(&on_it.hash()));
    assert!(!log.ordered().contains(&on_it.hash()));
    let by_carlos = Entry::sign(&carlos.id, group, log.heads(), 7, Body::Level { level: 3 });
    assert_eq!(
        log.insert(by_carlos),
        Err(StateError::NotAllowed("author does not manage the group"))
    );
    let junk = Entry::sign(
        &ana.id,
        group,
        log.heads(),
        8,
        Body::Unknown {
            kind: 25,
            bytes: vec![0, 0, 0, 3, 9],
        },
    );
    assert_eq!(Entry::decode(&junk.encode()).unwrap().body, junk.body);
    assert_eq!(log.insert(junk.clone()), Ok(true));
    assert!(log.ordered().contains(&junk.hash()));
    assert_eq!(log.level(), Level::Two);
    assert_eq!(log.state().name, "g");
}

/// The entries every copy of a world of random histories was given, honest and hostile,
/// kept or not, in the order they were written; and the room for a run its copies had.
struct World {
    run: u32,
    entries: Vec<Entry>,
    /// The devices of the world, which wrote most of them.
    devices: Vec<DeviceIdentity>,
}

impl World {
    /// The entries of the world, each followed by others like it: the same body on the
    /// same parents signed by two other devices of the world, and by its own author an
    /// hour later. Signatures are the same each time, so these are too.
    fn with_variants(&self) -> Vec<Entry> {
        let n = self.devices.len();
        let mut out = Vec::new();
        for (i, entry) in self.entries.iter().enumerate() {
            out.push(entry.clone());
            if matches!(entry.body, Body::Create { .. }) || entry.encode().len() > 4_096 {
                continue;
            }
            let sign = |by: &DeviceIdentity, time_ms| {
                Entry::sign(
                    by,
                    entry.group_id,
                    entry.parents.clone(),
                    time_ms,
                    entry.body.clone(),
                )
            };
            for by in [&self.devices[(i + 1) % n], &self.devices[(i * 7 + 3) % n]] {
                if by.public_key() != entry.author {
                    out.push(sign(by, entry.time_ms));
                }
            }
            if let Some(by) = self.devices.iter().find(|d| d.public_key() == entry.author) {
                out.push(sign(by, entry.time_ms + 3_600_000));
            }
        }
        out
    }
}

/// The corpus the judgment of level two is pinned on: worlds made by
/// [`write_the_level_two_corpus`] once, never made again (see [`JUDGED`]).
const CORPUS: &[u8] = include_bytes!("level-two.bin");

/// What level two makes of [`CORPUS`], as [`judged`] writes it: written with the corpus,
/// and never since. A change to what level two keeps or applies changes it, and forks the
/// log between copies that judge before and after the change: it comes as a new level
/// instead, which leaves this as it is.
const JUDGED: &str = include_str!("level-two.txt");

/// [`JUDGED`] with its lines ended as [`judged`] ends them, whatever a checkout made of them
/// (Git on Windows may end them with `\r\n`).
fn judged_then() -> String {
    JUDGED.replace("\r\n", "\n")
}

fn corpus_bytes(worlds: &[World]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(worlds.len() as u32).to_be_bytes());
    for world in worlds {
        out.extend_from_slice(&world.run.to_be_bytes());
        out.extend_from_slice(&(world.devices.len() as u32).to_be_bytes());
        for device in &world.devices {
            out.extend_from_slice(&*device.secret_bytes());
        }
        out.extend_from_slice(&(world.entries.len() as u32).to_be_bytes());
        for entry in &world.entries {
            let bytes = entry.encode();
            out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            out.extend_from_slice(&bytes);
        }
    }
    out
}

fn corpus_of(mut bytes: &[u8]) -> Vec<World> {
    let u32_of = |bytes: &mut &[u8]| {
        let (n, rest) = bytes.split_at(4);
        *bytes = rest;
        u32::from_be_bytes(n.try_into().unwrap())
    };
    let mut worlds = Vec::new();
    for _ in 0..u32_of(&mut bytes) {
        let run = u32_of(&mut bytes);
        let mut devices = Vec::new();
        for _ in 0..u32_of(&mut bytes) {
            let (secret, rest) = bytes.split_at(32);
            bytes = rest;
            devices.push(DeviceIdentity::from_secret_bytes(
                secret.try_into().unwrap(),
            ));
        }
        let mut entries = Vec::new();
        for _ in 0..u32_of(&mut bytes) {
            let len = u32_of(&mut bytes) as usize;
            let (entry, rest) = bytes.split_at(len);
            bytes = rest;
            entries.push(Entry::decode(entry).expect("an entry of the corpus"));
        }
        worlds.push(World {
            run,
            entries,
            devices,
        });
    }
    assert!(bytes.is_empty());
    worlds
}

/// A world of random histories: honest and hostile steps, two managers answering one
/// invite at once, devices past their run (odd seeds), and `Level` entries, every entry
/// written noted, kept or not.
fn corpus_world(seed: u64) -> World {
    let mut h = Hostile::new(seed);
    let noted = |h: &mut Hostile, step: &dyn Fn(&mut LadderWorld)| {
        let had: Vec<BTreeSet<EntryHash>> = h
            .world
            .logs
            .iter()
            .map(|l| l.hashes().copied().collect())
            .collect();
        step(&mut h.world);
        for (log, had) in h.world.logs.iter().zip(&had) {
            for entry in log.entries() {
                if !had.contains(&entry.hash()) && !h.written.contains(entry) {
                    h.written.push(entry.clone());
                }
            }
        }
    };
    for i in 0..150 {
        match i % 6 {
            0 | 3 => h.hostile_step(1 + i % 2),
            4 => noted(&mut h, &LadderWorld::race_step),
            _ => h.honest_step(),
        }
        if i == 75 {
            levels_written(&mut h);
        }
    }
    for (to, entry) in std::mem::take(&mut h.late) {
        let _ = h.world.logs[to].insert(entry);
    }
    if seed % 2 == 1 {
        h.go_past_a_run();
    }
    World {
        run: h.world.logs[0].run_room.entries,
        entries: h.written,
        devices: (h.world.devices.iter())
            .map(|d| DeviceIdentity::from_secret_bytes(&d.id.secret_bytes()))
            .collect(),
    }
}

/// A world of what the levels of judgment were changed for, each written as it happens:
/// room rules signed before a loss of their author and apart from one, by an author given
/// its place back later; a join dated after its ticket expired; and a member that does not
/// manage rotating for a key sent in an add that did not apply.
fn crafted_world() -> World {
    let (ana, bea, carlos, dani) = (Device::new(), Device::new(), Device::new(), Device::new());
    let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
    let mut written: Vec<Entry> = log.entries_in_order().into_iter().cloned().collect();
    let mut w = |entry: Result<Entry, StateError>| written.push(entry.unwrap());
    w(log.add(&ana.id, bea.key(), "bea", 2));
    w(log.add(&ana.id, carlos.key(), "carlos", 3));
    w(log.add(&ana.id, dani.key(), "dani", 4));
    w(log.append(&ana.id, Body::SetManagers { first: Some(2) }, 5));
    // A rule before a loss of its author.
    w(log.keep_out(&bea.id, FIRST_ROOM, dani.key(), Some(60_000), 10));
    w(log.append(&ana.id, Body::Demote { member: bea.key() }, 11));
    w(log.append(&ana.id, Body::Promote { member: bea.key() }, 12));
    // A rule apart from a loss of its author, who has its place back later.
    let (mut a, mut b) = (log.clone(), log);
    w(a.append(&ana.id, Body::Demote { member: bea.key() }, 20));
    w(b.set_room_access(&bea.id, FIRST_ROOM, Some(vec![carlos.key()]), 21));
    met(&mut a, &mut b);
    w(a.append(&ana.id, Body::Promote { member: bea.key() }, 22));
    // A join dated after its ticket expired, by a device whose clock is ahead, let in by a
    // manager before it did.
    let ticket = a.issue_ticket(&ana.id, 30).unwrap();
    let late = join(&mut a, &Device::new(), ticket, 40);
    written.push(late.clone());
    let mut w = |entry: Result<Entry, StateError>| written.push(entry.unwrap());
    w(a.let_in(&ana.id, &late.hash(), 29));
    // A rotation of a member that does not manage, for a key sent in an add that did not
    // apply.
    let (mut c, mut d) = (a.clone(), a);
    w(c.append(&ana.id, Body::Demote { member: bea.key() }, 50));
    w(d.add(&bea.id, Device::new().key(), "x", 50));
    met(&mut c, &mut d);
    w(c.rotate(&bea.id, 51));
    World {
        run: RUN_ROOM.entries,
        entries: written,
        devices: [ana, bea, carlos, dani]
            .iter()
            .map(|d| DeviceIdentity::from_secret_bytes(&d.id.secret_bytes()))
            .collect(),
    }
}

/// `Level` entries on what copy 0 has: one naming the group's level, by a manager; one
/// naming a level this version does not judge at, by a manager and by a member that does
/// not manage, with an entry signed on the first; and one of that kind with more in it.
fn levels_written(h: &mut Hostile) {
    let log = &h.world.logs[0];
    let (state, group, heads, time) = (log.state(), log.group_id(), log.heads(), h.world.time);
    let device = |manages: bool| {
        h.world
            .devices
            .iter()
            .find(|d| state.is_member(&d.key()) && state.is_manager(&d.key()) == manages)
            .map(|d| &d.id)
    };
    let mut written = Vec::new();
    if let Some(manager) = device(true) {
        let at = |level| Entry::sign(manager, group, heads.clone(), time, Body::Level { level });
        let (two, three) = (at(2), at(3));
        let on_three = Entry::sign(
            manager,
            group,
            vec![three.hash()],
            time + 1,
            Body::Rename { name: "l".into() },
        );
        let more = Body::Unknown {
            kind: 25,
            bytes: vec![0, 0, 0, 3, 1],
        };
        let more = Entry::sign(manager, group, heads.clone(), time, more);
        written.extend([two, three, on_three, more]);
    }
    if let Some(member) = device(false) {
        written.push(Entry::sign(
            member,
            group,
            heads,
            time,
            Body::Level { level: 3 },
        ));
    }
    for entry in written {
        let _ = h.world.logs[0].insert(entry.clone());
        h.written.push(entry);
    }
}

/// Short names for the devices of a world in what [`judged`] writes, in the order they
/// first show.
#[derive(Default)]
struct Names(Vec<String>);

impl Names {
    /// `text` with each device key in it written as its short name.
    fn of(&mut self, text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find("DevicePublicKey(") {
            out.push_str(&rest[..at]);
            let key = &rest[at + "DevicePublicKey(".len()..];
            let end = key.find(')').expect("a key's end");
            let n = match self.0.iter().position(|k| k == &key[..end]) {
                Some(n) => n,
                None => {
                    self.0.push(key[..end].to_string());
                    self.0.len() - 1
                }
            };
            out.push_str(&format!("d{n}"));
            rest = &key[end + 1..];
        }
        out.push_str(rest);
        out
    }
}

/// The places in `entries` of `hashes`, sorted, as runs (`3-7 9`).
fn places(
    index: &HashMap<EntryHash, usize>,
    hashes: impl IntoIterator<Item = EntryHash>,
) -> String {
    let mut at: Vec<usize> = hashes.into_iter().map(|h| index[&h]).collect();
    at.sort_unstable();
    in_runs(&at)
}

fn in_runs(at: &[usize]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < at.len() {
        let mut j = i;
        while j + 1 < at.len() && at[j + 1] == at[j] + 1 {
            j += 1;
        }
        out.push(if j == i {
            at[i].to_string()
        } else {
            format!("{}-{}", at[i], at[j])
        });
        i = j + 1;
    }
    out.join(" ")
}

/// Where each entry of `corpus` is in it.
fn index_of(corpus: &[Entry]) -> HashMap<EntryHash, usize> {
    corpus
        .iter()
        .enumerate()
        .map(|(i, e)| (e.hash(), i))
        .collect()
}

/// A copy of this level given `order`, entries of `corpus`, one at a time, with room `run`
/// for a run.
fn given(corpus: &[Entry], order: &[&Entry], run: u32) -> GroupLog {
    let mut log = GroupLog::empty(corpus[0].group_id);
    log.set_run_room(run, RUN_ROOM.bytes);
    for entry in order {
        let _ = log.insert((*entry).clone());
    }
    log
}

/// The limits level two judges by never change: a change to any of them changes what a log
/// keeps or applies, so it comes as a new level (see [`Level::caps`]). The common order is
/// pinned the same way, by [`the_judgment_of_level_two_never_changes`].
#[test]
fn the_limits_of_level_two_never_change() {
    use crate::state::Caps;
    assert_eq!(
        Level::Two.caps(),
        Caps {
            members: 20,
            in_room: 8,
            rooms: 10,
            room_name_chars: 40,
            group_name_chars: 60,
            member_name_chars: 40,
            picture_bytes: 48 * 1024,
            entry_bytes: 64 * 1024,
            key_package_bytes: 2048,
            parents: 64,
            run_entries: 4096,
            run_bytes: 4 << 20,
            update_gap_ms: 24 * 60 * 60 * 1000,
        },
        "a limit of level two changed: it comes as a new level instead"
    );
}

/// A `Level` entry is written only by [`GroupLog::raise_level`], the one place a group's
/// level goes up.
#[test]
fn only_raise_level_writes_a_level_entry() {
    let (mut log, ana, _bea, _carlos) = two_managers();
    assert_eq!(
        log.append(&ana.id, Body::Level { level: 2 }, 5),
        Err(StateError::NotAllowed(crate::log::RAISE_LEVEL_ONLY))
    );
}

/// A manager raising the group to a level this copy does not judge at yet is a copy ahead
/// of this one, not an author writing what it could not: its `Level` entry is refused here,
/// and what is signed on it waits, but its author is not counted as having written one it
/// could not. One naming a level below this version's, or by a member that does not manage
/// the group, is.
#[test]
fn a_level_ahead_of_this_copy_is_not_counted_against_its_author() {
    let (log, ana, bea, carlos) = two_managers();
    let group = log.group_id();
    let next = Level::CURRENT.number() + 1;
    let ahead = Entry::sign(&ana.id, group, log.heads(), 5, Body::Level { level: next });
    let below = Entry::sign(&bea.id, group, log.heads(), 5, Body::Level { level: 1 });
    let not_a_manager = Entry::sign(
        &carlos.id,
        group,
        log.heads(),
        5,
        Body::Level { level: next },
    );
    let on_it = Entry::sign(
        &ana.id,
        group,
        vec![ahead.hash()],
        6,
        Body::Rename { name: "h".into() },
    );
    let mut copy = log.clone();
    let taken = copy.insert_some([ahead, below, not_a_manager, on_it.clone()]);
    assert_eq!(
        taken.refused,
        BTreeMap::from([(bea.key(), 1), (carlos.key(), 1)])
    );
    assert!(copy.waiting().contains(&on_it.hash()));
    // The same, the entry waiting for its parent first.
    let mut copy = log.clone();
    let parent = Entry::sign(
        &ana.id,
        group,
        log.heads(),
        7,
        Body::Rename { name: "i".into() },
    );
    let ahead = Entry::sign(
        &ana.id,
        group,
        vec![parent.hash()],
        8,
        Body::Level { level: next },
    );
    assert!(copy.insert_some([ahead]).refused.is_empty());
    assert!(copy.insert_some([parent]).refused.is_empty());
}

/// What a copy given `order`, entries of `corpus`, with room `run` for a run makes of
/// them: which it keeps, which wait, their common order, which apply, and the group they
/// make; entries named by their places in `corpus`.
fn judged_in(corpus: &[Entry], order: &[&Entry], run: u32, names: &mut Names) -> String {
    let index = index_of(corpus);
    let log = given(corpus, order, run);
    let state = log.state();
    let order: Vec<usize> = log.ordered().iter().map(|h| index[h]).collect();
    let applied: Vec<usize> = state.applied.iter().map(|h| index[h]).collect();
    let epochs: Vec<String> = state
        .epochs
        .iter()
        .map(|e| format!("{:02x?}@{}", &e.id[..4], e.since_ms))
        .collect();
    let out = [
        format!("kept {}", places(&index, log.hashes().copied())),
        format!("waiting {}", places(&index, log.waiting())),
        format!("order {order:?}"),
        format!("applied {applied:?}"),
        format!(
            "not applied {}",
            places(&index, state.rejected.iter().map(|(h, _)| *h))
        ),
        format!(
            "name {:?} icon {} picture {} limits {} {} level {:?} creator {:?}",
            state.name,
            state.icon,
            state.picture.as_ref().map_or(0, Vec::len),
            state.max_members,
            state.max_in_room,
            state.level,
            state.creator
        ),
        format!("members {:?}", state.members),
        format!("ladder {:?} managers {:?}", state.ladder, state.managers),
        format!("epochs {}", epochs.join(" ")),
        format!("rooms {:?}", state.rooms),
        format!("rules {:?}", state.rules),
        format!(
            "tickets used {} invites open {}",
            state.used_tickets.len(),
            state.open_invites.len()
        ),
        format!(
            "secret leaked to {:?} lacking {:?}",
            state.current_secret_leaked_to(),
            state.lacking_current_secret()
        ),
    ];
    out.map(|line| names.of(&line)).join("\n")
}

/// What the level before, the fogata-state of level one, makes of `corpus` given in that
/// order otherwise than a copy of this level: the entries one keeps and the other does
/// not, and those that apply on one only, by their places in `corpus`.
fn judged_apart_at_level_one(corpus: &[Entry]) -> [(&'static str, Vec<usize>); 4] {
    let index = index_of(corpus);
    let order: Vec<&Entry> = corpus.iter().collect();
    let log = given(corpus, &order, RUN_ROOM.entries);
    let mut old = before::GroupLog::empty(corpus[0].group_id);
    for entry in corpus {
        let _ = old.insert(before_level(entry));
    }
    let at =
        |hashes: &[EntryHash]| -> BTreeSet<usize> { hashes.iter().map(|h| index[h]).collect() };
    let (now, then) = (log.state(), old.state());
    let kept_now = at(&log.hashes().copied().collect::<Vec<_>>());
    let kept_then = at(&old.hashes().copied().collect::<Vec<_>>());
    let (applied_now, applied_then) = (at(&now.applied), at(&then.applied));
    let apart = |a: &BTreeSet<usize>, b: &BTreeSet<usize>| a.difference(b).copied().collect();
    [
        ("kept here only", apart(&kept_now, &kept_then)),
        ("kept at level one only", apart(&kept_then, &kept_now)),
        ("applied here only", apart(&applied_now, &applied_then)),
        (
            "applied at level one only",
            apart(&applied_then, &applied_now),
        ),
    ]
}

/// Why the level before and this one may judge entry `i` of `corpus` apart, as decided when
/// this level came (`None` if not): `log` is a copy of this level given the corpus. A
/// `Level` entry, which the level before keeps and applies as a kind it does not know, and
/// what is signed on one; a room rule signed apart from a loss of its author, which the
/// level before judged by their times; a `Join` dated after its ticket expired and a
/// `Rotate` of a member that does not manage, which the level before refused, and what is
/// signed on them.
fn apart_as_decided(log: &GroupLog, corpus: &[Entry], i: usize) -> Option<&'static str> {
    let index = index_of(corpus);
    let level_kind =
        |e: &Entry| matches!(e.body, Body::Level { .. } | Body::Unknown { kind: 25, .. });
    let refused_before = |e: &Entry| match &e.body {
        Body::Join { ticket, .. } | Body::MlsJoin { ticket, .. } => e.time_ms > ticket.expires_ms,
        Body::Rotate { .. } => !log.state().is_manager(&e.author),
        _ => false,
    };
    let entry = &corpus[i];
    if level_kind(entry) {
        return Some("level");
    }
    if refused_before(entry) {
        return Some("refused before");
    }
    if entry.body.is_room_rule()
        && log.get(&entry.hash()).is_some()
        && log.stands_cache().of(&entry.hash()).apart
    {
        return Some("rule apart from a loss");
    }
    // Signed on one of those, directly or not.
    let mut todo = entry.parents.clone();
    let mut seen = BTreeSet::new();
    while let Some(parent) = todo.pop() {
        let Some(&p) = index.get(&parent) else {
            continue;
        };
        if !seen.insert(p) {
            continue;
        }
        if level_kind(&corpus[p]) {
            return Some("on a level");
        }
        if refused_before(&corpus[p]) {
            return Some("on one refused before");
        }
        todo.extend(corpus[p].parents.iter().copied());
    }
    None
}

/// What level two makes of the worlds of the corpus, as [`JUDGED`] has it: for each, what a
/// copy given its entries one at a time as they were written makes of them (one given them
/// all at once makes the same, checked here), and where the level before judges them
/// apart.
fn judged(worlds: &[World]) -> String {
    let mut out = String::new();
    for (w, world) in worlds.iter().enumerate() {
        let corpus = &world.with_variants();
        let mut names = Names::default();
        for entry in corpus {
            names.of(&format!("{:?}", entry.author));
        }
        let as_written: Vec<&Entry> = corpus.iter().collect();
        let judgment = judged_in(corpus, &as_written, world.run, &mut names);
        // A copy given them all at once keeps and applies the same.
        let mut at_once = GroupLog::empty(corpus[0].group_id);
        at_once.set_run_room(world.run, RUN_ROOM.bytes);
        at_once.insert_all(corpus.iter().cloned());
        let one_at_a_time = given(corpus, &as_written, world.run);
        assert_eq!(at_once.ordered(), one_at_a_time.ordered(), "world {w}");
        assert_eq!(
            at_once.state().applied,
            one_at_a_time.state().applied,
            "world {w}"
        );
        out.push_str(&format!(
            "world {w}: {} entries, {} in a run\n{judgment}\n",
            corpus.len(),
            world.run
        ));
        // The level before knows no other room for a run.
        if world.run == RUN_ROOM.entries {
            let log = given(corpus, &as_written, world.run);
            for (what, at) in judged_apart_at_level_one(corpus) {
                let mut line = what.to_string();
                for i in at {
                    let why = apart_as_decided(&log, corpus, i)
                        .unwrap_or_else(|| panic!("world {w}: {what}: {:?}", corpus[i].body));
                    line.push_str(&format!(" {i} ({why})"));
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    out
}

/// What level two keeps and applies of the corpus is what it kept and applied when the
/// corpus was made (see [`JUDGED`]). A change to it fails here: it forks the log between
/// copies before and after it, so it comes as a new level (a `Level` variant, with an arm
/// in each judgment), which leaves level two as it is.
#[test]
fn the_judgment_of_level_two_never_changes() {
    let worlds = corpus_of(CORPUS);
    assert!(!worlds.is_empty());
    let now = judged(&worlds);
    let then = judged_then();
    if now != then {
        let path = std::env::temp_dir().join("level-two-now.txt");
        let _ = std::fs::write(&path, &now);
        let line = now
            .lines()
            .zip(then.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(now.lines().count().min(then.lines().count()));
        panic!(
            "level two judges its corpus otherwise than when it was made, from line {} of \
             level-two.txt on (what it makes of it now is in {}). A change to what is kept \
             or applied comes as a new level, never as a change to level two.",
            line + 1,
            path.display()
        );
    }
}

/// Makes the corpus of level two and what it judges of it, into the folder
/// `FOGATA_LEVEL_TWO_CORPUS` names; only when the corpus is made, and never while level two
/// judges the corpus there otherwise than [`JUDGED`] says.
#[test]
#[ignore]
fn write_the_level_two_corpus() {
    let dir = std::env::var_os("FOGATA_LEVEL_TWO_CORPUS").expect("FOGATA_LEVEL_TWO_CORPUS");
    if !CORPUS.is_empty() {
        assert_eq!(judged(&corpus_of(CORPUS)), judged_then());
    }
    let mut worlds: Vec<World> = [1, 2, 4].map(corpus_world).into();
    worlds.push(crafted_world());
    let bytes = corpus_bytes(&worlds);
    assert_eq!(corpus_of(&bytes).len(), worlds.len());
    let dir = std::path::Path::new(&dir);
    std::fs::write(dir.join("level-two.bin"), bytes).unwrap();
    std::fs::write(dir.join("level-two.txt"), judged(&worlds)).unwrap();
}
