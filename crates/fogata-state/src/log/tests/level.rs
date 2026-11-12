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

/// `f` run as a copy of the version that brought level two judges: one that knows no level
/// after it. The corpus of level two holds `Level` entries naming later levels, which such a
/// copy refuses, and which a copy that judges at them takes.
fn as_level_two<T>(f: impl FnOnce() -> T) -> T {
    crate::state::HIGHEST.with(|h| h.set(Level::Two));
    let out = f();
    crate::state::HIGHEST.with(|h| h.set(Level::CURRENT));
    out
}

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
                for (hash, (n, _)) in log.arrived.iter() {
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
    let next = Level::CURRENT.number() + 1;
    let ahead = Entry::sign(&ana.id, group, log.heads(), 5, Body::Level { level: next });
    assert_eq!(
        log.insert(ahead.clone()),
        Err(StateError::NotAllowed(LEVEL_UNKNOWN))
    );
    let on_it = Entry::sign(
        &ana.id,
        group,
        vec![ahead.hash()],
        6,
        Body::Rename { name: "h".into() },
    );
    assert_eq!(log.insert(on_it.clone()), Ok(true));
    assert!(log.waiting().contains(&on_it.hash()));
    assert!(!log.ordered().contains(&on_it.hash()));
    let by_carlos = Entry::sign(
        &carlos.id,
        group,
        log.heads(),
        7,
        Body::Level { level: next },
    );
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
    judged_with(worlds, true)
}

/// [`judged`], and where the level before judges apart only with `apart_from_level_one`:
/// a later level's corpus holds entries level one knows nothing of, which it is not
/// compared with.
fn judged_with(worlds: &[World], apart_from_level_one: bool) -> String {
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
        if apart_from_level_one && world.run == RUN_ROOM.entries {
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
    let now = as_level_two(|| judged(&worlds));
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
        assert_eq!(as_level_two(|| judged(&corpus_of(CORPUS))), judged_then());
    }
    let mut worlds: Vec<World> = [1, 2, 4].map(corpus_world).into();
    worlds.push(crafted_world());
    let bytes = corpus_bytes(&worlds);
    assert_eq!(corpus_of(&bytes).len(), worlds.len());
    let dir = std::path::Path::new(&dir);
    std::fs::write(dir.join("level-two.bin"), bytes).unwrap();
    std::fs::write(dir.join("level-two.txt"), as_level_two(|| judged(&worlds))).unwrap();
}

/// `f` run as a copy of the version that brought level three judges: one that knows no
/// level after it.
fn as_level_three<T>(f: impl FnOnce() -> T) -> T {
    crate::state::HIGHEST.with(|h| h.set(Level::Three));
    let out = f();
    crate::state::HIGHEST.with(|h| h.set(Level::CURRENT));
    out
}

/// The corpus the judgment of level three is pinned on, made by
/// [`write_the_level_three_corpus`] with the version that brought level three, before any
/// later level, and never made again: the saved logs of the levels after two (see the
/// architecture document, 5.1). A later version judges it as level three did, as a copy of
/// level three ([`as_level_three`]).
const CORPUS_THREE: &[u8] = include_bytes!("level-three.bin");

/// What level three makes of [`CORPUS_THREE`], as [`judged_with`] writes it.
const JUDGED_THREE: &str = include_str!("level-three.txt");

/// The entries a manager of copy 0 writes to raise the group to `level`, handed to every
/// copy; none when no member manages there.
fn raise_written(h: &mut Hostile, level: Level) -> Option<Entry> {
    let time = h.world.time;
    let state = h.world.logs[0].state();
    let manager = (h.world.devices.iter()).find(|d| state.is_manager(&d.key()))?;
    let entry = h.world.logs[0].raise_level(&manager.id, level, time).ok()?;
    for log in &mut h.world.logs {
        let _ = log.insert(entry.clone());
    }
    h.written.push(entry.clone());
    Some(entry)
}

/// Rotations and removals that give their new key to their author alone, by every member
/// of copy 0, each on copy 0's heads and on `stale` (entries from before the group went up a
/// level), handed to copy 0 and another: what level three keeps of them where their
/// authors saw it, and applies at level two where they did not; and a level after it holds
/// to every member its author saw.
fn for_a_few_written(h: &mut Hostile, stale: &[EntryHash]) {
    h.world.time += 1;
    let time = h.world.time;
    let log = &h.world.logs[0];
    let (state, group, heads) = (log.state(), log.group_id(), log.heads());
    let n = h.world.devices.len();
    let mut written = Vec::new();
    for (i, d) in h.world.devices.iter().enumerate() {
        if !state.is_member(&d.key()) {
            continue;
        }
        let alone = || Rotation::new(&group, [d.key()]).unwrap().0;
        let below = state.below(&d.key()).first().copied();
        for parents in [heads.clone(), stale.to_vec()] {
            written.push((
                i,
                Entry::sign(
                    &d.id,
                    group,
                    parents.clone(),
                    time,
                    Body::Rotate { rotation: alone() },
                ),
            ));
            if let Some(member) = below {
                let rotation = alone();
                written.push((
                    i,
                    Entry::sign(
                        &d.id,
                        group,
                        parents,
                        time,
                        Body::Remove { member, rotation },
                    ),
                ));
            }
        }
    }
    for (i, entry) in written {
        for to in [0, 1 + i % (n - 1)] {
            let _ = h.world.logs[to].insert(entry.clone());
        }
        h.written.push(entry);
    }
}

/// A world of random histories raised to level three first, with invites that name who
/// lets their device in, rotations and removals for a few on entries from before the
/// raise and after it, and `Level` entries naming the level after three, by a manager and
/// by a member that does not manage, every entry written noted, kept or not.
fn level_three_world(seed: u64) -> World {
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
    // Raised before anything else is written, so that no loss of its author outranks it.
    let stale = h.world.logs[0].heads();
    raise_written(&mut h, Level::Three).expect("a manager on copy 0");
    for i in 0..150 {
        match i % 6 {
            0 | 3 => h.hostile_step(1 + i % 2),
            4 => noted(&mut h, &LadderWorld::race_step),
            _ => h.honest_step(),
        }
        if i % 30 == 29 {
            for_a_few_written(&mut h, &stale);
        }
        if i == 100 {
            let next = Level::Three.number() + 1;
            let log = &h.world.logs[0];
            let (state, group, heads) = (log.state(), log.group_id(), log.heads());
            for d in &h.world.devices {
                if state.is_member(&d.key()) {
                    let level = Body::Level { level: next };
                    h.written.push(Entry::sign(
                        &d.id,
                        group,
                        heads.clone(),
                        h.world.time,
                        level,
                    ));
                }
            }
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

/// Whether `Rotate` or `Remove` entry `hash` of `log` leaves out of its new epoch a member of
/// the group its author saw (but the one removed, and keys nothing can be sealed to).
fn leaves_out_its_view(log: &GroupLog, hash: &EntryHash) -> bool {
    let view = log.state_before(hash);
    let reached = |rotation: &Rotation, but: Option<&DevicePublicKey>| {
        view.members
            .keys()
            .filter(|k| !k.is_weak() && Some(*k) != but)
            .all(|k| rotation.sealed.iter().any(|(to, _)| to == k))
    };
    match &log.entries[hash].body {
        Body::Rotate { rotation } => !reached(rotation, None),
        Body::Remove { member, rotation } => !reached(rotation, Some(member)),
        _ => false,
    }
}

/// What level three keeps and applies of its corpus is what it kept and applied when the
/// corpus was made (see [`JUDGED_THREE`]), judged as a copy of level three: a change to it
/// comes as a new level, as for level two.
#[test]
fn the_judgment_of_level_three_never_changes() {
    let worlds = corpus_of(CORPUS_THREE);
    assert!(!worlds.is_empty());
    let now = as_level_three(|| judged_with(&worlds, false));
    let then = JUDGED_THREE.replace("\r\n", "\n");
    if now != then {
        let path = std::env::temp_dir().join("level-three-now.txt");
        let _ = std::fs::write(&path, &now);
        let line = now
            .lines()
            .zip(then.lines())
            .position(|(a, b)| a != b)
            .unwrap_or(now.lines().count().min(then.lines().count()));
        panic!(
            "level three judges its corpus otherwise than when it was made, from line {} of \
             level-three.txt on (what it makes of it now is in {}). A change to what is kept \
             or applied comes as a new level, never as a change to level three.",
            line + 1,
            path.display()
        );
    }
}

/// Makes the corpus of level three and what it judges of it, into the folder
/// `FOGATA_LEVEL_THREE_CORPUS` names: by the version that brought level three only, before
/// any later level, and never while level three judges the corpus there otherwise than
/// [`JUDGED_THREE`] says. Its worlds hold what level three was made for and what it left
/// open: rotations and removals for a few that it keeps and applies.
#[test]
#[ignore]
fn write_the_level_three_corpus() {
    let dir = std::env::var_os("FOGATA_LEVEL_THREE_CORPUS").expect("FOGATA_LEVEL_THREE_CORPUS");
    assert_eq!(
        Level::CURRENT,
        Level::Three,
        "made by the version of level three only"
    );
    if !CORPUS_THREE.is_empty() {
        let worlds = corpus_of(CORPUS_THREE);
        assert_eq!(as_level_three(|| judged_with(&worlds, false)), JUDGED_THREE);
    }
    // Even seeds: a run of the usual room, where the raise is not past its author's run
    // (the corpus of level two holds those).
    let worlds: Vec<World> = [2, 4, 6].map(level_three_world).into();
    // Each kind it holds: a group raised to three, named invites, and rotations and
    // removals for a few that apply there.
    let (mut rotations, mut removals) = (0, 0);
    for world in &worlds {
        let corpus = world.with_variants();
        let mut log = GroupLog::empty(corpus[0].group_id);
        log.set_run_room(world.run, RUN_ROOM.bytes);
        log.insert_all(corpus.iter().cloned());
        let state = log.state();
        assert_eq!(state.level, Level::Three);
        assert!(log.entries().any(|e| NamedInvite::of(&e.body).is_some()));
        for hash in &state.applied {
            if leaves_out_its_view(&log, hash) {
                match log.entries[hash].body {
                    Body::Rotate { .. } => rotations += 1,
                    Body::Remove { .. } => removals += 1,
                    _ => {}
                }
            }
        }
    }
    assert!(rotations > 0 && removals > 0, "{rotations} {removals}");
    let bytes = corpus_bytes(&worlds);
    assert_eq!(corpus_of(&bytes).len(), worlds.len());
    let dir = std::path::Path::new(&dir);
    std::fs::write(dir.join("level-three.bin"), bytes).unwrap();
    std::fs::write(
        dir.join("level-three.txt"),
        as_level_three(|| judged_with(&worlds, false)),
    )
    .unwrap();
}

/// Ana's group with Bea managing too and Carlos a member (see `two_managers`), raised to
/// level three.
fn at_level_three() -> (GroupLog, Device, Device, Device) {
    let (mut log, ana, bea, carlos) = two_managers();
    log.raise_level(&ana.id, Level::Three, 5).unwrap();
    assert_eq!(log.level(), Level::Three);
    (log, ana, bea, carlos)
}

/// An invite that names who lets its device in travels as a kind level two does not know,
/// in one encoding only; a group still at level two keeps it as such a kind, changing
/// nothing, as copies of the version before do.
#[test]
fn a_named_invite_is_a_kind_level_two_does_not_know() {
    let (mut log, ana, bea, _carlos) = two_managers();
    let ticket = log.issue_ticket(&ana.id, 1_000).unwrap();
    let invite = NamedInvite {
        ticket: ticket.clone(),
        token: [7; 16],
        // Out of order, repeated, and the issuer named too: written once, sorted, without it.
        welcomers: vec![bea.key(), ana.key(), bea.key()],
    };
    let body = invite.body();
    let read = NamedInvite::of(&body).unwrap();
    assert_eq!(read.welcomers, vec![bea.key()]);
    assert!(read.lets_in(&ana.key()) && read.lets_in(&bea.key()));
    let entry = Entry::sign(&ana.id, log.group_id(), log.heads(), 5, body.clone());
    assert_eq!(Entry::decode(&entry.encode()).unwrap().body, body);
    assert!(matches!(
        before_level(&entry).body,
        before::Body::Unknown { kind: 26, .. }
    ));
    // Another encoding of the same invite (the issuer named after Bea) is no invite.
    let Body::Unknown { kind, mut bytes } = body else {
        unreachable!("a named invite is a kind level two does not know")
    };
    let count = bytes.len() - 32 - 4;
    bytes[count..count + 4].copy_from_slice(&2u32.to_be_bytes());
    bytes.extend_from_slice(ana.key().as_bytes());
    assert_eq!(NamedInvite::of(&Body::Unknown { kind, bytes }), None);
    // At level two: kept, and no invite opens.
    assert_eq!(log.insert(entry.clone()), Ok(true));
    assert!(!log.state().applied.contains(&entry.hash()));
    assert!(log.state().open_invites.is_empty());
    assert!(!log.invited_with(&ticket));
    assert_eq!(levels_disagree_on(&log, &at_the_level_before(&log)), vec![]);
}

/// Working out the order and the group of a log reads each named invite many times, at
/// level three (its author's run, what a demotion outranks, what applies); each is read
/// from its bytes, its keys checked, only once per thread, and every copy judges alike.
#[test]
fn a_named_invite_is_read_from_its_bytes_once() {
    let (mut log, ana, bea, _carlos) = at_level_three();
    let invites = 12;
    for n in 0..invites {
        let by = if n % 2 == 0 { &ana } else { &bea };
        log.invite(&by.id, 100_000, 6 + n).unwrap();
    }
    assert_eq!(log.state().open_invites.len(), invites as usize);
    crate::work::Work::start();
    for _ in 0..3 {
        let mut copy = GroupLog::empty(log.group_id());
        copy.insert_all(log.entries().cloned());
        assert_eq!(copy.state().open_invites.len(), invites as usize);
        assert_eq!(copy.level(), Level::Three);
    }
    let read = crate::work::Work::done().named;
    assert!(
        read <= invites as usize,
        "{read} reads of {invites} invites"
    );
    // Bytes that are no invite are none each time they are asked about, kept or not.
    let entry = log
        .entries()
        .find(|e| NamedInvite::of(&e.body).is_some())
        .unwrap();
    let Body::Unknown { kind, bytes } = &entry.body else {
        unreachable!("a named invite is a kind level two does not know")
    };
    let mut damaged = bytes.clone();
    // The issuer's key, made bytes that are no point of the curve: no invite.
    let off_curve = (0..=u8::MAX)
        .map(|n| [n; 32])
        .find(|bytes| fogata_crypto::DevicePublicKey::from_bytes(bytes).is_err())
        .unwrap();
    let issuer_at = 16 + 16;
    assert_eq!(&damaged[issuer_at..issuer_at + 32], entry.author.as_bytes());
    damaged[issuer_at..issuer_at + 32].copy_from_slice(&off_curve);
    let damaged = Body::Unknown {
        kind: *kind,
        bytes: damaged,
    };
    assert_eq!(NamedInvite::of(&damaged), None);
    assert_eq!(NamedInvite::of(&damaged), None);
}

/// A copy of the version that brought level two, given a group raised to level three,
/// refuses the `Level` entry and waits on what is signed on it, as for any level it does
/// not judge at; this version takes it, and judges what follows at level three.
#[test]
fn a_copy_of_level_two_stops_where_the_group_goes_up_to_level_three() {
    let (mut log, ana, _bea, _carlos) = at_level_three();
    log.invite(&ana.id, 1_000, 6).unwrap();
    let old = as_level_two(|| {
        let mut old = GroupLog::empty(log.group_id());
        old.insert_all(log.entries().cloned());
        assert_eq!(old.level(), Level::Two);
        old
    });
    assert!(old.entries().all(|e| !matches!(e.body, Body::Level { .. })));
    assert_eq!(old.waiting().len(), 1);
    let mut new = GroupLog::empty(log.group_id());
    new.insert_all(log.entries().cloned());
    assert_eq!(new.level(), Level::Three);
    assert_eq!(new.state().open_invites.len(), 1);
}

/// From level three, an invite names who may let its device in beside its issuer: the
/// managers above the issuer on the ladder. Only they and the issuer do; the others do not
/// even answer under its token. An invite that names nobody applies no more.
#[test]
fn at_level_three_only_the_issuer_or_a_manager_its_invite_names_lets_a_device_in() {
    let (mut log, ana, bea, _carlos) = at_level_three();
    let named = |log: &GroupLog, ticket: &Ticket| {
        log.entries()
            .find_map(|e| NamedInvite::of(&e.body).filter(|i| i.ticket == *ticket))
            .expect("a named invite")
            .welcomers
    };
    // Ana is first on the ladder: nobody else lets in whom she invites.
    let (by_ana, token_ana) = log.invite(&ana.id, 1_000, 6).unwrap();
    assert_eq!(named(&log, &by_ana), vec![]);
    // Bea comes after Ana, who may let in whom Bea invites.
    let (by_bea, token_bea) = log.invite(&bea.id, 1_000, 7).unwrap();
    assert_eq!(named(&log, &by_bea), vec![ana.key()]);
    let state = log.state();
    assert_eq!(state.invite_tokens_for(&ana.key(), 8).len(), 2);
    assert_eq!(state.invite_tokens_for(&bea.key(), 8), vec![token_bea]);
    assert!(!state.invite_tokens_for(&bea.key(), 8).contains(&token_ana));
    assert!(!state.may_let_in(&bea.key(), &by_ana));

    let (xavi, yago) = (Device::new(), Device::new());
    let x = join(&mut log, &xavi, by_ana.clone(), 9);
    assert_eq!(
        log.let_in(&bea.id, &x.hash(), 10),
        Err(StateError::NotAllowed(crate::state::WELCOME_NOT_NAMED))
    );
    log.let_in(&ana.id, &x.hash(), 11).unwrap();
    assert!(log.state().is_member(&xavi.key()));
    assert!(log.admits_as_invited(&by_ana, &xavi.key()));
    let y = join(&mut log, &yago, by_bea.clone(), 12);
    log.let_in(&ana.id, &y.hash(), 13).unwrap();
    assert!(log.admits_as_invited(&by_bea, &yago.key()));

    // An invite that names nobody is one only level two writes.
    let ticket = log.issue_ticket(&ana.id, 1_000).unwrap();
    assert_eq!(
        log.append(
            &ana.id,
            Body::Invite {
                ticket,
                token: [3; 16]
            },
            14
        ),
        Err(StateError::NotAllowed(crate::state::INVITE_NAMES_NOBODY))
    );
}

/// A manager removed while an invite was open keeps the log up to the issuer's note of
/// it, and in a copy of its own still manages the group. The invited device takes no log
/// where it is let in by a manager the invite does not name, even when that copy signs the
/// `Join` and `Welcome` on the group as it was before level three, so that they apply at
/// level two there. In a group still at level two, whose invites name nobody, it cannot
/// tell, as before.
#[test]
fn an_invited_device_takes_no_welcome_from_a_manager_its_invite_does_not_name() {
    let (mut log, ana, bea, _carlos) = two_managers();
    let before_three = log.clone();
    log.raise_level(&ana.id, Level::Three, 5).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000, 6).unwrap();
    log.remove(&ana.id, bea.key(), 7).unwrap();

    // Bea's copy, as it was before level three: she still manages there.
    let xavi = Device::new();
    let mut fork = before_three.clone();
    let join_entry = join(&mut fork, &xavi, ticket.clone(), 8);
    let welcome = fork.let_in(&bea.id, &join_entry.hash(), 9).unwrap();
    let mut shown = GroupLog::empty(log.group_id());
    shown.insert_all(
        log.entries()
            .filter(|e| !matches!(e.body, Body::Remove { .. }))
            .cloned()
            .chain([join_entry, welcome]),
    );
    assert!(shown.invited_with(&ticket));
    // Signed on entries before the group's level went up, they applied at level two.
    assert!(shown.state().is_member(&xavi.key()));
    assert!(!shown.admits_as_invited(&ticket, &xavi.key()));

    // The real group, where Ana lets Xavi in, is taken.
    let x = join(&mut log, &xavi, ticket.clone(), 10);
    log.let_in(&ana.id, &x.hash(), 11).unwrap();
    assert!(log.admits_as_invited(&ticket, &xavi.key()));

    // At level two the same copy passes: its invite names nobody.
    let (mut old, ana, bea, _carlos) = two_managers();
    let before_invite = old.clone();
    let (ticket, _) = old.invite(&ana.id, 10_000, 6).unwrap();
    let mut fork = before_invite;
    let join_entry = join(&mut fork, &xavi, ticket.clone(), 8);
    let welcome = fork.let_in(&bea.id, &join_entry.hash(), 9).unwrap();
    let mut shown = GroupLog::empty(old.group_id());
    shown.insert_all(old.entries().cloned().chain([join_entry, welcome]));
    assert!(shown.state().is_member(&xavi.key()));
    assert!(shown.admits_as_invited(&ticket, &xavi.key()));
}

/// The same copy may let the invited device in without its `Welcome`: it learnt the
/// device's key from the `Join` it was sent, adds it, and seals it the copy's key. The
/// invited device takes no log where it is a member by an entry of a manager its invite
/// does not name, however it was let in; an `Add` of the issuer, signed knowing of the
/// invite, it takes.
#[test]
fn an_invited_device_takes_no_add_from_a_manager_its_invite_does_not_name() {
    let (mut log, ana, bea, _carlos) = at_level_three();
    let before_invite = log.clone();
    let (ticket, _) = log.invite(&ana.id, 10_000, 6).unwrap();
    let invited = log.clone();
    log.remove(&ana.id, bea.key(), 7).unwrap();

    let xavi = Device::new();
    // Bea's copy, from the invite on, and one signed on the group as it was before it.
    for mut fork in [invited.clone(), before_invite] {
        fork.add(&bea.id, xavi.key(), "xavi", 8).unwrap();
        let mut shown = GroupLog::empty(log.group_id());
        shown.insert_all(invited.entries().chain(fork.entries()).cloned());
        assert!(shown.invited_with(&ticket));
        assert!(shown.state().is_member(&xavi.key()));
        assert!(shown.current_keys(&xavi.id).is_some());
        assert!(!shown.admits_as_invited(&ticket, &xavi.key()));
    }

    // The issuer adding the device, knowing of the invite, is one way in it allows.
    let mut by_ana = log.clone();
    by_ana.add(&ana.id, xavi.key(), "xavi", 8).unwrap();
    assert!(by_ana.admits_as_invited(&ticket, &xavi.key()));

    // One the issuer added before the invite, and who left since, is no member by it: a
    // copy that leaves out its `Leave` shows it in still.
    let (mut log, ana, _bea, _carlos) = at_level_three();
    log.add(&ana.id, xavi.key(), "xavi", 6).unwrap();
    let mut left = log.clone();
    left.append(&xavi.id, Body::Leave, 7).unwrap();
    let (ticket, _) = log.invite(&ana.id, 10_000, 7).unwrap();
    assert!(log.state().is_member(&xavi.key()));
    assert!(!log.admits_as_invited(&ticket, &xavi.key()));
    sync(&left, &mut log);
    assert!(log.admits_as_invited(&ticket, &xavi.key()));
}

/// From level three a `Rotate` gives the new key to every member its author saw: one that
/// leaves members out is not kept, whoever wrote it (at level two, a member that does not
/// manage could start an epoch for itself alone once someone gone held the key, and leave
/// the managers without it). [`GroupLog::rotate`] gives it to all.
#[test]
fn at_level_three_a_rotation_gives_the_new_key_to_every_member() {
    let dani = Device::new();
    let (mut two, ana, bea, carlos) = two_managers();
    two.add(&ana.id, dani.key(), "dani", 5).unwrap();
    let mut three = two.clone();
    three.raise_level(&ana.id, Level::Three, 6).unwrap();
    for log in [&mut two, &mut three] {
        // Dani leaves holding the key: a member may rotate now.
        log.append(&dani.id, Body::Leave, 7).unwrap();
    }
    let alone = |log: &GroupLog, by: &Device| {
        let (rotation, _) = Rotation::new(&log.group_id(), [by.key()]).unwrap();
        Entry::sign(
            &by.id,
            log.group_id(),
            log.heads(),
            8,
            Body::Rotate { rotation },
        )
    };
    let at_two = alone(&two, &carlos);
    assert_eq!(two.insert(at_two.clone()), Ok(true));
    assert!(two.state().applied.contains(&at_two.hash()));
    assert!(two.current_keys(&ana.id).is_none());

    for by in [&carlos, &ana] {
        assert_eq!(
            three.insert(alone(&three, by)),
            Err(StateError::NotAllowed(crate::state::ROTATE_LEAVES_OUT))
        );
    }
    three.rotate(&carlos.id, 9).unwrap();
    let epoch = three.current_keys(&carlos.id).unwrap().epoch();
    for device in [&ana, &bea] {
        assert_eq!(
            three.current_keys(&device.id).map(|k| k.epoch()),
            Some(epoch)
        );
    }
}

/// A member's copy given a group that a manager raised past the levels it judges at: it
/// refuses the `Level` entry, as every copy of its version does, and waits on what is signed
/// after it, so the group stays here as it was (its members, keys and rooms) and nothing
/// later reaches it. It says so (`level_ahead`), from the log itself: told once by the batch
/// that brought it, kept with the log when it is stored and found again when it is read
/// (judged once more, verdicts or not), and gone for a version that judges at that level,
/// which reads the entry as the raise it is. Only a manager's raise counts: a member that
/// does not manage cannot have anyone told to update.
#[test]
fn a_member_s_copy_behind_its_group_s_level_says_so_until_it_judges_at_it() {
    let (mut log, ana, bea, carlos) = two_managers();
    let group = log.group_id();
    let next = Level::CURRENT.number() + 1;
    // A member that does not manage: refused for that, and nobody is told anything.
    let by_carlos = Entry::sign(
        &carlos.id,
        group,
        log.heads(),
        5,
        Body::Level { level: next },
    );
    let taken = log.insert_some([by_carlos]);
    assert_eq!(taken.refused_count(), 1);
    assert!(!taken.ahead);
    assert_eq!(log.level_ahead(), None);
    // A manager's raise: refused, not counted against Ana, and told once.
    let ahead = Entry::sign(&ana.id, group, log.heads(), 6, Body::Level { level: next });
    let on_it = Entry::sign(
        &bea.id,
        group,
        vec![ahead.hash()],
        7,
        Body::Rename {
            name: "later".into(),
        },
    );
    let taken = log.insert_some([ahead.clone(), on_it.clone()]);
    assert!(taken.new.contains(&on_it.hash()));
    assert!(taken.refused.is_empty());
    assert!(taken.ahead);
    assert_eq!(log.level_ahead(), Some(next));
    assert!(log.get(&ahead.hash()).is_none());
    assert!(log.waiting().contains(&on_it.hash()));
    assert_eq!(log.state().name, "g");
    assert!(log.state().is_member(&carlos.key()));
    // Sent again, it tells nothing new; the log sends it to nobody nor signs on it.
    assert!(!log.insert_some([ahead.clone()]).ahead);
    assert!(log.hashes_to_send().iter().all(|h| *h != ahead.hash()));
    assert!(!log.heads().contains(&ahead.hash()));
    // Stored and read back, with what was judged of it or without: still told.
    let stored: Vec<Entry> = log.entries_to_store().into_iter().cloned().collect();
    assert!(stored.contains(&ahead));
    let (read, verdicts) = GroupLog::read_judged(group, stored.clone(), None, false);
    assert_eq!(read.level_ahead(), Some(next));
    let verdicts = verdicts.unwrap();
    assert_eq!(verdicts, log.verdicts(log.hashes_to_store()));
    let (read, anew) = GroupLog::read_judged(group, stored.clone(), Some(&verdicts), true);
    assert!(anew.is_none(), "taken as judged");
    assert_eq!(read.level_ahead(), Some(next));
    assert!(read.waiting().contains(&on_it.hash()));
    // A copy put in its place keeps it.
    let mut copy = GroupLog::empty(group);
    copy.insert_all(log.entries().cloned());
    assert_eq!(copy.level_ahead(), None);
    assert!(copy.keep_ahead_of(&log));
    assert_eq!(copy.level_ahead(), Some(next));
    assert!(!copy.keep_ahead_of(&log));
}

/// What a copy of the version before stores of a group raised past it is read by a version
/// that judges at that level as the group it is: the raise applies, what waited on it
/// follows, and nobody is told to update.
#[test]
fn an_updated_copy_reads_the_raise_its_older_version_stored_and_is_told_nothing() {
    let (mut log, ana, bea, _carlos) = two_managers();
    let group = log.group_id();
    let raise = log.raise_level(&ana.id, Level::Three, 5).unwrap();
    log.append(
        &bea.id,
        Body::Rename {
            name: "later".into(),
        },
        6,
    )
    .unwrap();
    let stored: Vec<Entry> = as_level_two(|| {
        let mut old = GroupLog::empty(group);
        old.insert_all(log.entries().cloned());
        assert_eq!(old.level_ahead(), Some(3));
        assert_eq!(old.state().name, "g");
        old.entries_to_store().into_iter().cloned().collect()
    });
    assert!(stored.contains(&raise));
    let mut new = GroupLog::empty(group);
    new.insert_all(stored);
    assert_eq!(new.level_ahead(), None);
    assert_eq!(new.level(), Level::Three);
    assert_eq!(new.state().name, "later");
}

/// Ana's group with Bea managing too and Carlos a member, and Dani, who left holding the
/// current key (so a member may rotate); `then` is written on top, and the heads before it
/// are returned with the log.
fn dani_left(then: impl FnOnce(&mut GroupLog, &Device)) -> (GroupLog, Vec<EntryHash>, [Device; 3]) {
    let dani = Device::new();
    let (mut log, ana, bea, carlos) = two_managers();
    log.add(&ana.id, dani.key(), "dani", 5).unwrap();
    log.append(&dani.id, Body::Leave, 6).unwrap();
    let stale = log.heads();
    then(&mut log, &ana);
    (log, stale, [ana, bea, carlos])
}

/// A rotation by `by` on `parents` that gives the new key to `by` alone.
fn rotation_alone(log: &GroupLog, by: &Device, parents: Vec<EntryHash>, time: u64) -> Entry {
    let (rotation, _) = Rotation::new(&log.group_id(), [by.key()]).unwrap();
    Entry::sign(
        &by.id,
        log.group_id(),
        parents,
        time,
        Body::Rotate { rotation },
    )
}

/// The probe of level three's gap, kept as a check of level four: a member signs a
/// rotation for itself alone on entries from before the group went up (it saw someone gone
/// hold the key, at level two). Level three keeps it, judged at the level its author saw,
/// and applies it: the others are left without the current key. Level four keeps it too,
/// as every copy does, but it does not apply where the group is at four.
#[test]
fn stale_parent_rotate_skips_level_three_rule() {
    for level in [Level::Three, Level::Four] {
        let (mut log, stale, [ana, _bea, carlos]) = dani_left(|log, ana| {
            log.raise_level(&ana.id, level, 8).unwrap();
        });
        let rotate = rotation_alone(&log, &carlos, stale, 9);
        assert_eq!(log.insert(rotate.clone()), Ok(true), "{level:?}");
        let state = log.state();
        if level == Level::Three {
            assert!(state.applied.contains(&rotate.hash()));
            assert!(log.current_keys(&ana.id).is_none());
        } else {
            assert!(!state.applied.contains(&rotate.hash()));
            assert!(
                state
                    .rejected
                    .contains(&(rotate.hash(), crate::state::ROTATE_LEAVES_OUT))
            );
            assert!(log.current_keys(&ana.id).is_some());
            assert!(log.current_keys(&carlos.id).is_some());
        }
        // The same on what the group has now, where Dani still holds the key at level
        // four: never kept, from level three.
        if level == Level::Four {
            let fresh = rotation_alone(&log, &carlos, log.heads(), 10);
            assert_eq!(
                log.insert(fresh),
                Err(StateError::NotAllowed(crate::state::ROTATE_LEAVES_OUT))
            );
        }
    }
}

/// [`stale_parent_rotate_skips_level_three_rule`] where a manager rotated for everyone
/// between the leave and the raise: level three applies the stale rotation all the same
/// (it is not the newest epoch, so the members keep the manager's); level four does not
/// apply it at all.
#[test]
fn stale_parent_rotate_skips_level_three_rule_after_honest_rotation() {
    for level in [Level::Three, Level::Four] {
        let (mut log, stale, [ana, bea, carlos]) = dani_left(|log, ana| {
            log.rotate(&ana.id, 7).unwrap();
            log.raise_level(&ana.id, level, 8).unwrap();
        });
        let honest = log.state().current_epoch().unwrap().id;
        let rotate = rotation_alone(&log, &carlos, stale, 9);
        assert_eq!(log.insert(rotate.clone()), Ok(true), "{level:?}");
        let state = log.state();
        assert_eq!(
            state.applied.contains(&rotate.hash()),
            level == Level::Three,
            "{level:?}"
        );
        for device in [&ana, &bea, &carlos] {
            assert!(log.current_keys(&device.id).is_some(), "{level:?}");
        }
        if level == Level::Four {
            assert_eq!(state.current_epoch().unwrap().id, honest);
            assert_eq!(state.epochs.len(), log.state().epochs.len());
        }
    }
}

/// A removal whose new epoch reaches its author alone: no level checks that before four,
/// so level three keeps and applies it, and the others lack the key; at level four one its
/// author wrote seeing the group at four is not kept, and one it wrote on entries from
/// before the raise is kept but does not apply (the member is not removed by it). An honest
/// removal applies.
#[test]
fn at_level_four_a_removal_gives_the_new_key_to_every_member_left() {
    let removal_alone = |log: &GroupLog, by: &Device, member: &Device, parents, time| {
        let (rotation, _) = Rotation::new(&log.group_id(), [by.key()]).unwrap();
        let body = Body::Remove {
            member: member.key(),
            rotation,
        };
        Entry::sign(&by.id, log.group_id(), parents, time, body)
    };
    for level in [Level::Three, Level::Four] {
        let (mut log, stale, [ana, bea, carlos]) = dani_left(|log, ana| {
            log.raise_level(&ana.id, Level::Three, 7).unwrap();
        });
        let stale = if level == Level::Four {
            let stale = log.heads();
            log.raise_level(&ana.id, Level::Four, 8).unwrap();
            stale
        } else {
            stale
        };
        let remove = removal_alone(&log, &bea, &carlos, stale, 9);
        assert_eq!(log.insert(remove.clone()), Ok(true), "{level:?}");
        let state = log.state();
        if level == Level::Three {
            assert!(!state.is_member(&carlos.key()));
            assert!(log.current_keys(&ana.id).is_none());
        } else {
            assert!(state.is_member(&carlos.key()));
            assert!(
                state
                    .rejected
                    .contains(&(remove.hash(), crate::state::REMOVE_LEAVES_OUT))
            );
            assert!(log.current_keys(&ana.id).is_some());
            let now = removal_alone(&log, &bea, &carlos, log.heads(), 10);
            assert_eq!(
                log.insert(now),
                Err(StateError::NotAllowed(crate::state::REMOVE_LEAVES_OUT))
            );
            log.remove(&bea.id, carlos.key(), 11).unwrap();
            assert!(!log.state().is_member(&carlos.key()));
            for device in [&ana, &bea] {
                assert!(log.current_keys(&device.id).is_some());
            }
        }
    }
}

/// A world of random histories raised to level three first and to level four midway, with
/// rotations and removals for a few on entries from before each raise and after them.
fn level_four_world(seed: u64) -> Hostile {
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
    let mut stale = vec![h.world.logs[0].heads()];
    raise_written(&mut h, Level::Three).expect("a manager on copy 0");
    for i in 0..120 {
        match i % 6 {
            0 | 3 => h.hostile_step(1 + i % 2),
            4 => noted(&mut h, &LadderWorld::race_step),
            _ => h.honest_step(),
        }
        if i == 40 {
            stale.push(h.world.logs[0].heads());
            if raise_written(&mut h, Level::Four).is_none() {
                // Nobody manages on copy 0 any more: the world stays at three.
                stale.pop();
            }
        }
        if i % 20 == 19 {
            for parents in stale.clone() {
                for_a_few_written(&mut h, &parents);
            }
        }
    }
    for (to, entry) in std::mem::take(&mut h.late) {
        let _ = h.world.logs[to].insert(entry);
    }
    h
}

/// Level four judges the same on every copy whatever order its entries come in: copies
/// that took a world's entries in their own order, and fresh copies given every entry ever
/// written as written, the other way round and shuffled, one at a time or all at once, keep
/// the same entries, order them alike and apply the same; worked out again from nothing,
/// or entry by entry as they came, the group is the same. Every rotation and removal that
/// applies at level four reaches every member its author saw, and some that do not were
/// kept (signed on entries from before the raise) and do not apply.
/// `FOGATA_LADDER_SEEDS` sets how many histories run (4 by default).
#[test]
fn at_level_four_every_copy_applies_the_same_whatever_came_first() {
    let seeds = std::env::var("FOGATA_LADDER_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4u64);
    let (mut at_four, mut held_back) = (0, 0);
    for seed in 1..=seeds {
        let mut h = level_four_world(2 * seed);
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
        if state.level < Level::Four {
            continue;
        }
        at_four += 1;
        for (i, other) in h.world.logs.iter().enumerate() {
            let theirs: BTreeSet<EntryHash> = other.hashes().copied().collect();
            assert_eq!(theirs, kept, "seed {seed}: device {i} keeps others");
            assert_eq!(other.ordered(), log.ordered(), "seed {seed}: device {i}");
            assert_eq!(other.state().applied, state.applied, "seed {seed}: {i}");
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
        // Where it applies at level four, a rotation or removal reaches every member its
        // author saw; those that do not and were kept are held back there.
        let fresh = {
            let mut fresh = log.clone();
            *fresh.cache = Cache::default();
            fresh
        };
        let raised = (state.applied.iter())
            .position(|h| matches!(log.entries[h].body, Body::Level { level: 4 }))
            .unwrap();
        for hash in &state.applied[raised..] {
            assert!(!leaves_out_its_view(&fresh, hash), "seed {seed}");
        }
        held_back += (state.rejected.iter())
            .filter(|(h, why)| {
                [
                    crate::state::ROTATE_LEAVES_OUT,
                    crate::state::REMOVE_LEAVES_OUT,
                ]
                .contains(why)
                    && leaves_out_its_view(&fresh, h)
            })
            .count();
        check_every_step(log, &state);
        same_as_worked_out_again(log, &h.world.devices[0].id, &format!("seed {seed}"));
    }
    assert!(at_four > 0, "no world went up to level four");
    assert!(held_back > 0, "no rotation nor removal was held back");
}

/// The corpus of level three, judged by this version, which judges at level four too: the
/// same as level three judged it, but for what a raise to four in it changes. Entries that
/// follow no raise to four are kept alike, and what applies is the same until either a raise
/// to four applies or a rotation or removal that a raise to four did not see does not. (Its
/// raises to four, which copies of level three refused and nobody signed on, are each
/// outranked by a later loss of their author: kept, they apply nowhere, and so hold nothing
/// to level four: the corpus is judged as level three judged it.)
#[test]
fn the_corpus_of_level_three_is_judged_alike_until_a_raise_to_four() {
    let mut kept_raises = 0;
    for (w, world) in corpus_of(CORPUS_THREE).iter().enumerate() {
        let corpus = world.with_variants();
        let order: Vec<&Entry> = corpus.iter().collect();
        let then = as_level_three(|| given(&corpus, &order, world.run));
        let now = given(&corpus, &order, world.run);
        let fours: Vec<EntryHash> = corpus
            .iter()
            .filter(|e| matches!(e.body, Body::Level { level: 4 }))
            .map(Entry::hash)
            .collect();
        // The raises to four and every entry of the corpus after one of them.
        let mut after_four: BTreeSet<EntryHash> = fours.iter().copied().collect();
        loop {
            let more: Vec<EntryHash> = corpus
                .iter()
                .filter(|e| e.parents.iter().any(|p| after_four.contains(p)))
                .map(Entry::hash)
                .filter(|h| !after_four.contains(h))
                .collect();
            if more.is_empty() {
                break;
            }
            after_four.extend(more);
        }
        let kept = |log: &GroupLog| -> BTreeSet<EntryHash> {
            log.hashes()
                .filter(|h| !after_four.contains(*h))
                .copied()
                .collect()
        };
        assert_eq!(kept(&now), kept(&then), "world {w}");
        kept_raises += fours.iter().filter(|h| now.get(h).is_some()).count();
        let held = now.settled_cache().held.clone();
        if now.level() < Level::Four {
            assert!(
                held.is_empty(),
                "world {w}: held by a raise that does not apply"
            );
        }
        let (now, then) = (now.state(), then.state());
        let apart = (now.applied.iter())
            .zip(&then.applied)
            .position(|(a, b)| a != b)
            .unwrap_or(now.applied.len().min(then.applied.len()));
        if apart < now.applied.len().max(then.applied.len()) {
            let what = [now.applied.get(apart), then.applied.get(apart)];
            assert!(
                what.iter()
                    .flatten()
                    .any(|h| fours.contains(*h) || held.contains(*h)),
                "world {w}: apart at {apart}"
            );
        }
    }
    assert!(kept_raises > 0, "no raise to four was kept");
}

/// Whether entry `hash` of `log` is one a raise past level two judges otherwise wherever it
/// goes after the raise, as level three does since it came: a `Level` entry, and the note
/// of an invite that names nobody, which applies no more from level three (it lets nobody
/// in by itself: a device joins with its ticket and a manager's `Welcome`).
fn judged_by_level(log: &GroupLog, hash: &EntryHash) -> bool {
    matches!(
        log.entries[hash].body,
        Body::Level { .. } | Body::Invite { .. }
    )
}

/// What applied of `log` and the group it makes, its level and what [`judged_by_level`]
/// aside: what a raise to four must not change of honest history.
fn group_apart_from_level(log: &GroupLog) -> String {
    let state = log.state();
    let applied: Vec<&EntryHash> = (state.applied.iter())
        .filter(|h| !judged_by_level(log, h))
        .collect();
    let epochs: Vec<_> = state.epochs.iter().map(|e| e.id).collect();
    format!(
        "{applied:?}\n{:?}\n{:?} {:?}\n{epochs:?}\n{:?}",
        state.members, state.ladder, state.managers, state.used_tickets
    )
}

/// Raises to four on old parents: one by each device of `devices` that managed the group
/// just after entry `at`, for points `at` taken every `every` entries of the common order
/// of `log`; a former manager's as well as a current one's (as the author's view judges
/// it, so every copy keeps them).
fn stale_raises(log: &GroupLog, devices: &[DeviceIdentity], every: usize) -> Vec<Entry> {
    let order = log.ordered();
    let mut out = Vec::new();
    for at in order.iter().step_by(every.max(1)) {
        let mut upto = log.ancestors(at);
        upto.insert(*at);
        let then = log.state_of(upto.into_iter().collect());
        if then.level >= Level::Four {
            continue;
        }
        for d in devices {
            if then.is_manager(&d.public_key()) {
                let time = log.entries[at].time_ms + 1;
                let raise = Body::Level { level: 4 };
                out.push(Entry::sign(d, log.group_id(), vec![*at], time, raise));
            }
        }
    }
    out
}

/// A raise to four signed on old parents, by a manager of then that may have lost its
/// rights since (removed, moved down, left out of the managers, gone) or by one that still
/// manages, changes nothing of what honest history applied, but the notes of invites that
/// name nobody (which any raise past two judges as level three does, as a raise to three
/// already did): honest devices of every version give a new key to every member they saw,
/// so nothing it holds to four leaves anyone out. Random honest histories, with rotations
/// and removals written the way releases before this one wrote them too (sealed to the
/// members of the whole log).
#[test]
fn a_raise_to_four_on_old_parents_changes_nothing_of_honest_history() {
    let seeds = std::env::var("FOGATA_LADDER_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4u64);
    let (mut raises, mut former, mut applied) = (0, 0, 0);
    for seed in 1..=seeds {
        let mut world = LadderWorld::new(2 * seed, 6);
        for i in 0..150 {
            world.step();
            if i % 7 == 3 {
                // A rotation and a removal the way earlier releases wrote them.
                let n = world.devices.len();
                let k = world.below(n);
                let me = world.devices[k].key();
                let state = world.logs[k].state();
                let members: Vec<DevicePublicKey> = state.members.keys().copied().collect();
                let group = world.logs[k].group_id();
                let body = match state.below(&me).first() {
                    Some(below) if i % 2 == 0 => Body::Remove {
                        member: *below,
                        rotation: Rotation::new(
                            &group,
                            members.iter().filter(|m| *m != below).copied(),
                        )
                        .unwrap()
                        .0,
                    },
                    _ if state.is_manager(&me) => Body::Rotate {
                        rotation: Rotation::new(&group, members).unwrap().0,
                    },
                    _ => continue,
                };
                let time = world.time;
                let _ = world.logs[k].append(&world.devices[k].id, body, time);
            }
        }
        world.sync_all();
        let log = &world.logs[0];
        if log.level() >= Level::Four {
            continue;
        }
        let before = group_apart_from_level(log);
        let now = log.state();
        let ids: Vec<DeviceIdentity> = (world.devices.iter())
            .map(|d| DeviceIdentity::from_secret_bytes(&d.id.secret_bytes()))
            .collect();
        for raise in stale_raises(log, &ids, 9) {
            let mut copy = log.clone();
            assert_eq!(copy.insert(raise.clone()), Ok(true), "seed {seed}: kept");
            raises += 1;
            former += usize::from(!now.is_manager(&raise.author));
            applied += usize::from(copy.state().applied.contains(&raise.hash()));
            assert_eq!(group_apart_from_level(&copy), before, "seed {seed}");
            // And worked out from nothing, as a copy given the entries at once.
            let mut fresh = world.empty_log();
            fresh.insert_all(copy.entries().cloned());
            assert_eq!(group_apart_from_level(&fresh), before, "seed {seed}: fresh");
        }
    }
    eprintln!("{raises} raises, {former} by former managers, {applied} applied");
    assert!(raises > 0 && former > 0 && applied > 0);
}

/// The same over the saved corpora of levels two and three, whose worlds hold hostile
/// entries too: a raise to four on old parents, by a manager of then, changes what applies
/// (but the notes of invites that name nobody, as any raise past two) only by holding back
/// a rotation or removal that leaves out a member its author saw, which no honest device
/// writes.
#[test]
fn a_raise_to_four_on_old_parents_holds_back_only_what_leaves_members_out() {
    let mut raises = 0;
    for (name, bytes) in [("two", CORPUS), ("three", CORPUS_THREE)] {
        for (w, world) in corpus_of(bytes).iter().enumerate() {
            let corpus = world.with_variants();
            let order: Vec<&Entry> = corpus.iter().collect();
            // Worlds where a run is small aside: there any entry more of an author, a raise
            // or a rename alike, may put a later one of the same author past its run.
            if world.run != RUN_ROOM.entries {
                continue;
            }
            let log = given(&corpus, &order, world.run);
            if log.level() >= Level::Four {
                continue;
            }
            let before = log.state();
            let every = (log.ordered().len() / 6).max(1);
            for raise in stale_raises(&log, &world.devices, every) {
                let mut copy = log.clone();
                if copy.insert(raise.clone()) != Ok(true) {
                    // Past its author's run, in a world where a run is small.
                    continue;
                }
                raises += 1;
                let after = copy.state();
                let held = copy.settled_cache().held.clone();
                let (a, b): (Vec<_>, Vec<_>) = (
                    (after.applied.iter())
                        .filter(|h| !judged_by_level(&copy, h))
                        .collect(),
                    (before.applied.iter())
                        .filter(|h| !judged_by_level(&log, h))
                        .collect(),
                );
                if let Some(apart) = a.iter().zip(&b).position(|(x, y)| x != y) {
                    let what = [a[apart], b[apart]];
                    assert!(
                        what.iter()
                            .any(|h| held.contains(*h) && leaves_out_its_view(&copy, h)),
                        "corpus {name}, world {w}: apart at {apart}"
                    );
                } else {
                    assert_eq!(a.len(), b.len(), "corpus {name}, world {w}");
                }
            }
        }
    }
    assert!(raises > 0);
}
