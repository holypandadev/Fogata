//! Measurements of what hostile logs cost, run by hand (`--ignored`).

use super::*;

/// What one entry placed early costs (the group worked out again from the start), at
/// 1600, 2400 and 4096 entries: in a log of losses and room rules (the heaviest to work
/// out), and in one of renames; one at a time, each followed by the group, and 64 sent in
/// one batch, taken a part at a time. For a release build: `cargo test --release -p
/// fogata-state -- --ignored measure_early_inserts --nocapture`.
#[test]
#[ignore]
fn measure_early_inserts() {
    for n in [1600u64, 2400, 4096] {
        let renames = {
            let (a, x) = (Device::new(), Device::new());
            let mut log = GroupLog::create(&a.id, "g", 1).unwrap();
            log.add(&a.id, x.key(), "x", 2).unwrap();
            let group = log.group_id();
            let mut prev = log.heads();
            for i in 0..n - 2 {
                let by = if i % 2 == 0 { &a } else { &x };
                let body = Body::Rename {
                    name: format!("n{i}"),
                };
                let entry = Entry::sign(&by.id, group, prev, 10 + i, body);
                prev = vec![entry.hash()];
                log.insert(entry).unwrap();
            }
            (log, a)
        };
        let (log, a, _, _) = rules_and_losses((n - 2) / 2, 1, false);
        for (what, log, a) in [
            ("losses and rules", log, a),
            ("renames", renames.0, renames.1),
        ] {
            assert_eq!(log.len() as u64, n, "{what}");
            let group = log.group_id();
            let root = vec![log.root().unwrap()];
            let mut log = from_nothing(&log);
            let ((), from_scratch) = timed(|| drop(log.state()));
            let mut each = Vec::new();
            for i in 0..16u64 {
                let early =
                    Entry::sign(&a.id, group, root.clone(), 3 + i, Body::SetIcon { icon: 3 });
                let ((), took) = timed(|| {
                    log.insert(early).unwrap();
                    drop(log.state());
                });
                each.push(took);
            }
            // 64 in one batch as a device sends them: on the first entry, each on the one
            // before it, and each on an entry of its own among the last ones.
            let ordered = log.ordered();
            let mut batches: Vec<(&str, Vec<Entry>)> = Vec::new();
            let on_root = (0..64u64)
                .map(|i| {
                    let body = Body::SetIcon { icon: 4 };
                    Entry::sign(&a.id, group, root.clone(), 100 + i, body)
                })
                .collect();
            batches.push(("on the first entry", on_root));
            let mut parents = root.clone();
            let chain = (0..64u64)
                .map(|i| {
                    let body = Body::SetIcon { icon: 5 };
                    let entry = Entry::sign(&a.id, group, parents.clone(), 200 + i, body);
                    parents = vec![entry.hash()];
                    entry
                })
                .collect();
            batches.push(("each on the one before", chain));
            let late = (0..64u64)
                .map(|i| {
                    let parent = vec![ordered[ordered.len() - 2 - i as usize]];
                    let body = Body::SetIcon { icon: 6 };
                    Entry::sign(&a.id, group, parent, 10_000 + i, body)
                })
                .collect();
            batches.push(("each on one of the last", late));
            let mut took = Vec::new();
            for (how, batch) in batches {
                // As a sync takes them: a part at a time, until all are in.
                let (mut calls, mut slowest, mut taken) = (0, std::time::Duration::ZERO, 0);
                while taken < batch.len() {
                    let (new, batched) = timed(|| log.insert_some(batch.clone()).new.len());
                    assert!(new > 0);
                    (calls, taken) = (calls + 1, taken + new);
                    slowest = slowest.max(batched);
                }
                let ((), state) = timed(|| drop(log.state()));
                took.push(format!(
                    "{how} in {calls} parts, each {slowest:?} at most, and the group {state:?}"
                ));
            }
            let slowest = each.iter().max().unwrap();
            let fastest = each.iter().min().unwrap();
            println!(
                "{n} entries of {what}: from nothing {from_scratch:?}; one placed early, each \
                 with the group after it, {fastest:?} to {slowest:?}; 64 placed early in one \
                 batch: {}",
                took.join(", ")
            );
        }
    }
}

/// What one batch from a member costs that sends many entries ahead of the parent they
/// share, each also on another old entry and each refused, in a log of 4096 renames: taken
/// whole, as every waiting entry was judged before, and as a sync takes it. For a release
/// build: `cargo test --release -p fogata-state -- --ignored measure_waited_batch
/// --nocapture`.
#[test]
#[ignore]
fn measure_waited_batch() {
    let (a, x) = (Device::new(), Device::new());
    let mut log = GroupLog::create(&a.id, "g", 1).unwrap();
    log.add(&a.id, x.key(), "x", 2).unwrap();
    let group = log.group_id();
    let mut prev = log.heads();
    for i in 0..4094u64 {
        let by = if i % 2 == 0 { &a } else { &x };
        let body = Body::Rename {
            name: format!("n{i}"),
        };
        let entry = Entry::sign(&by.id, group, prev, 10 + i, body);
        prev = vec![entry.hash()];
        log.insert(entry).unwrap();
    }
    let old = log.ordered();
    let parent = Entry::sign(&x.id, group, log.heads(), 9_000, Body::SetIcon { icon: 1 });
    let nobody = Device::new().key();
    // As many as one sync answer holds (256 KiB), and fewer.
    for children in [64usize, 1000] {
        let mut batch: Vec<Entry> = (0..children)
            .map(|i| {
                let parents = vec![parent.hash(), old[2 + i]];
                let body = Body::Promote { member: nobody };
                Entry::sign(&x.id, group, parents, 10_000 + i as u64, body)
            })
            .collect();
        batch.push(parent.clone());
        let bytes: usize = batch.iter().map(|e| e.encode().len()).sum();
        // Copies that share no group worked out with the others.
        let mut whole = from_nothing(&log);
        drop(whole.state());
        whole.views_worked.set(0);
        let (new, unbounded) = timed(|| whole.insert_all(batch.clone()));
        assert_eq!(new, [parent.hash()]);
        let mut parts = from_nothing(&log);
        drop(parts.state());
        parts.views_worked.set(0);
        let (taken, bounded) = timed(|| parts.insert_some(batch.clone()));
        assert_eq!(taken.new, [parent.hash()]);
        let first = parts.views_worked.replace(0);
        let (again, next) = timed(|| parts.insert_some(batch.clone()));
        println!(
            "{children} entries ahead of their parent ({bytes} bytes): every waiting one \
             judged {unbounded:?} ({} views); a sync {bounded:?} ({first} views, {} refused), \
             the same batch again {next:?} ({} views, {} refused)",
            whole.views_worked.get(),
            taken.refused_count(),
            parts.views_worked.get(),
            again.refused_count(),
        );
    }
}

/// What a log offered whole costs to take from nothing ([`GroupLog::insert_all`], as a
/// group offered by a friend, a direct chat's group and a removal are taken), at 4096
/// entries in the order they were written, for logs made to cost the most: two members each
/// signing only on its own last entry, every entry on parents nobody else is signed on;
/// and a chain with as many entries signed on its old entries, taken or refused. For a
/// release build: `cargo test --release -p fogata-state -- --ignored measure_offered_logs
/// --nocapture`.
#[test]
#[ignore]
fn measure_offered_logs() {
    for (what, entries) in costly_offers(4096) {
        let group = entries[0].group_id;
        let mut log = GroupLog::empty(group);
        let (new, took) = timed(|| log.insert_all(entries.clone()));
        let ((), state) = timed(|| drop(log.state()));
        println!(
            "{what}: {} of {} taken in {took:?}, {} views; the group then {state:?}",
            new.len(),
            entries.len(),
            log.views_worked.get(),
        );
    }
}

/// What reading a stored log back costs, at 4096 entries in the order a log is written in:
/// judged from nothing ([`GroupLog::insert_all`]), and with what was judged of it when it
/// was written ([`GroupLog::read_judged`]), with the signatures checked again and without;
/// for the logs of [`measure_offered_logs`] and an honest one. For a release build: `cargo
/// test --release -p fogata-state -- --ignored measure_logs_read_back --nocapture`.
#[test]
#[ignore]
fn measure_logs_read_back() {
    let mut logs = costly_offers(4096);
    let (a, x) = (Device::new(), Device::new());
    let mut honest = GroupLog::create(&a.id, "g", 1).unwrap();
    honest.add(&a.id, x.key(), "x", 2).unwrap();
    for i in 0..4094 {
        let by = [&a, &x][i % 2];
        let body = Body::Rename {
            name: format!("n{i}"),
        };
        honest.append(&by.id, body, 10 + i as u64).unwrap();
    }
    let written = honest.entries_in_order().into_iter().cloned().collect();
    logs.push(("two members taking turns", written));
    for (what, entries) in logs {
        let group = entries[0].group_id;
        let mut log = GroupLog::empty(group);
        log.insert_all(entries);
        let stored: Vec<Entry> = log.entries_in_order().into_iter().cloned().collect();
        let verdicts = log.verdicts(log.hashes().copied());
        let mut judged = GroupLog::empty(group);
        let (_, judging) = timed(|| judged.insert_all(stored.clone()));
        let read = |signed| {
            timed(|| {
                let (log, anew) =
                    GroupLog::read_judged(group, stored.clone(), Some(&verdicts), signed);
                assert!(anew.is_none());
                drop(log.state());
            })
            .1
        };
        let (checked, unchecked) = (read(false), read(true));
        // A copy with all but the last 512, taking the whole log judged elsewhere.
        let mut copy =
            GroupLog::read_judged(group, stored[..stored.len() - 512].to_vec(), None, true).0;
        let ((), merged) = timed(|| {
            copy.insert_judged(&log);
            drop(copy.state());
        });
        println!(
            "{what}: {} entries read back judged in {judging:?}; with their verdicts in \
             {checked:?}, or {unchecked:?} without checking signatures; the last 512 taken \
             from a log judged elsewhere in {merged:?}",
            stored.len(),
        );
    }
}
