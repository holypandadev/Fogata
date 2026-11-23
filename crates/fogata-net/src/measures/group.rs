//! Measurements of group removals and of a group's room memory, run by hand (`--ignored`).

use super::tests::{ctx, fill_rooms};
use super::*;
use fogata_state::Entry;

/// What a removal shown with as many entries as it may bring costs the node's loop, in a
/// group of 4096 entries written apart to cost the most: the checks and the copy of the
/// log before it is judged elsewhere, and taking the judged log after; and what judging
/// it costs. For a release build: `cargo test --release -p fogata-net --lib --
/// --ignored measure_removal_on_the_loop --nocapture`.
#[test]
#[ignore]
fn measure_removal_on_the_loop() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let leo = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, leo.public_key(), "leo", 2).unwrap();
    let group = log.group_id();
    let write_apart = |log: &mut GroupLog, n: usize, from: u64| {
        let mut last = [log.heads(), log.heads()];
        let mut written = Vec::new();
        for i in 0..n {
            let by: &DeviceIdentity = [&ana, &leo][i % 2];
            let body = fogata_state::Body::Rename {
                name: format!("n{i}"),
            };
            let entry = Entry::sign(by, group, last[i % 2].clone(), from + i as u64, body);
            last[i % 2] = vec![entry.hash()];
            written.push(entry);
        }
        log.insert_all(written);
    };
    write_apart(&mut log, 4096 - 2 - MAX_REMOVAL_ENTRIES, 10);
    let at_leo = ctx(&leo, &log);
    write_apart(&mut log, MAX_REMOVAL_ENTRIES - 1, 10_000);
    log.remove(&ana, leo.public_key(), 20_000).unwrap();
    let shown: Vec<Vec<u8>> = log
        .entries_in_order()
        .into_iter()
        .filter(|e| at_leo.log().get(&e.hash()).is_none())
        .map(Entry::encode)
        .collect();
    let started = std::time::Instant::now();
    let trial = at_leo.removal_trial(&shown).unwrap();
    let checked = started.elapsed();
    let started = std::time::Instant::now();
    let (_, judged) = trial.judge().unwrap();
    let judging = started.elapsed();
    let mut at_leo = at_leo;
    let started = std::time::Instant::now();
    let new = at_leo.take_judged(judged).unwrap();
    let taken = started.elapsed();
    assert!(!at_leo.i_am_member());
    println!(
        "{} entries shown in a log of {}: checked and copied on the loop in {checked:?}, \
         judged elsewhere in {judging:?}, taken on the loop in {taken:?}",
        new.len(),
        at_leo.log().len() - new.len(),
    );
}

/// How much memory a member sending the heaviest events a room takes makes a device keep
/// in the ten rooms of a group, measured (Linux).
#[test]
#[ignore]
fn measure_room_memory() {
    let rss = || {
        let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
        let pages: u64 = statm.split_whitespace().nth(1).unwrap().parse().unwrap();
        pages * 4096
    };
    let before = rss();
    let trackers = fill_rooms(10, false);
    let after = rss();
    let records: usize = trackers.iter().map(|t| t.snapshot().len()).sum();
    println!("{records} records kept; {} MiB", (after - before) >> 20);
}
