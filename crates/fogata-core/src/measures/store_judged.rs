//! Measurements of what logs costly to judge cost the store, and of a burst of changes to a large log, run by hand (`--ignored`).

use super::tests::{assert_same, costly_log, folder, timed};
use super::*;
use crate::{
    store::Store,
    vault::{Keychain, MemoryKeyStore},
};
use fogata_crypto::DeviceIdentity;
use fogata_state::Body;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// What a group of 4096 entries costs the store, written to cost the most to judge and
/// honest (two members taking turns), in a plain folder and a sealed one: read back judged
/// whole (as a folder written before these verdicts were kept), read back again, and
/// offered again with one entry more. For a release build: `cargo test --release -p
/// fogata-core --lib -- --ignored measure_costly_logs_in_the_store --nocapture`.
#[test]
#[ignore]
fn measure_costly_logs_in_the_store() {
    let ana = DeviceIdentity::generate().unwrap();
    let (costly, _) = costly_log(&ana, 4093);
    let mut honest = GroupLog::create(&ana, "Home", 1).unwrap();
    let leo = DeviceIdentity::generate().unwrap();
    honest.add(&ana, leo.public_key(), "Leo", 2).unwrap();
    for i in 0..4093 {
        let by = [&ana, &leo][i % 2];
        let body = Body::Rename {
            name: format!("n{i}"),
        };
        honest.append(by, body, 10 + i as u64).unwrap();
    }
    for (what, log) in [("costly", costly), ("honest", honest)] {
        for sealed in [false, true] {
            let dir = folder(&format!("measure-{what}-{sealed}"));
            let keychain = if sealed {
                Keychain::Seal(Arc::new(MemoryKeyStore::default()))
            } else {
                Keychain::Off
            };
            let store = Store::open_with(dir.clone(), keychain).unwrap();
            let (me, _) = store.identity().unwrap();
            let mut log = log.clone();
            log.add(&ana, me.public_key(), "Dani", 950_000).unwrap();
            let id = log.group_id();
            store.save_offered_group(&log, &ana.public_key()).unwrap();
            fs::remove_file(store.judged_path(&id)).unwrap();
            let read = || timed(|| drop(store.load_log_of(&id).unwrap().state())).1;
            let (whole, again) = (read(), read());
            log.append(&ana, Body::SetIcon { icon: 3 }, 950_001)
                .unwrap();
            let (_, offered) = timed(|| store.save_offered_group(&log, &ana.public_key()));
            println!(
                "{what}, {}: {} entries read judged whole in {whole:?}, then in {again:?}; \
                 offered again in {offered:?}",
                if sealed { "sealed" } else { "plain" },
                log.len(),
            );
            fs::remove_dir_all(&dir).unwrap();
        }
    }
}

/// A group of two members taking turns, `n` entries after its start, each about
/// `bytes` long: a group picture set as versions before picture records did.
pub(crate) fn large_log(ana: &DeviceIdentity, n: usize, bytes: usize) -> GroupLog {
    let leo = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(ana, "Home", 1).unwrap();
    log.add(ana, leo.public_key(), "Leo", 2).unwrap();
    for i in 0..n {
        let by = [ana, &leo][i % 2];
        let mut picture = vec![(i % 251) as u8; bytes];
        picture[..8].copy_from_slice(&(i as u64).to_be_bytes());
        let body = Body::SetPicture {
            picture: Some(picture),
        };
        log.append(by, body, 10 + i as u64).unwrap();
    }
    log
}

/// What a burst of 200 changes to a group of about 3 MB costs, as the core goes through
/// them: each change made on the node's log, which hands the core a copy of it
/// (`Node::group`), saved in turn by the writer, which writes only the newest of the
/// saves waiting; in a plain folder and a sealed one. For a release build: `cargo test
/// --release -p fogata-core --lib -- --ignored measure_a_burst_of_changes --nocapture`.
#[test]
#[ignore]
fn measure_a_burst_of_changes_to_a_large_log() {
    let ana = DeviceIdentity::generate().unwrap();
    let large = large_log(&ana, 4094, 600);
    for sealed in [false, true] {
        let dir = folder(&format!("burst-{sealed}"));
        let keychain = if sealed {
            Keychain::Seal(Arc::new(MemoryKeyStore::default()))
        } else {
            Keychain::Off
        };
        let store = Arc::new(Store::open_with(dir.clone(), keychain).unwrap());
        store.identity().unwrap();
        let mut log = large.clone();
        let id = log.group_id();
        let (_, whole) = timed(|| store.save_group_log(&log, "Home").unwrap());
        let size = fs::metadata(store.log_path(&id)).unwrap().len();
        let writer = crate::writer::Writer::start().unwrap();
        let saves = Arc::new(std::sync::Mutex::new((0, Duration::ZERO)));
        let started = Instant::now();
        let (mut node, mut copying) = (Duration::ZERO, Duration::ZERO);
        for i in 0..200 {
            let ((), took) = timed(|| {
                let body = Body::Rename {
                    name: format!("b{i}"),
                };
                log.append(&ana, body, 1_000_000 + i).unwrap();
            });
            let (snapshot, copied) = timed(|| log.clone());
            node += took + copied;
            copying += copied;
            let (store, saves) = (store.clone(), saves.clone());
            writer.soon(crate::writer::File::Group(id), move || {
                let (_, took) = timed(|| store.save_group_log(&snapshot, "Home").unwrap());
                let mut saves = saves.lock().unwrap();
                saves.0 += 1;
                saves.1 += took;
            });
        }
        // Dropped, the writer writes whatever waits before it goes.
        drop(writer);
        let took = started.elapsed();
        let (saves, saving) = *saves.lock().unwrap();
        let back = store.load_log_of(&id).unwrap();
        assert_same(&back, &log);
        let after = fs::metadata(store.log_path(&id)).unwrap().len();
        // One change at a time: the log file added to, or written whole, and what was
        // judged of it; the middle of 21 of each.
        let key = store.known_key().unwrap();
        let (judged, context) = (store.judged_path(&id), judged_context(&id));
        let mut each = |whole: bool| {
            let mut times = (Vec::new(), Vec::new());
            for i in 0..21 {
                let body = Body::Rename {
                    name: format!("o{i}"),
                };
                log.append(&ana, body, 2_000_000 + i).unwrap();
                if whole {
                    store.forget_files();
                }
                times.0.push(timed(|| store.store_log(&log).unwrap()).1);
                times
                    .1
                    .push(timed(|| write_judged(key, &judged, &context, &log)).1);
            }
            times.0.sort();
            times.1.sort();
            (times.0[10], times.1[10])
        };
        let (added, verdicts) = each(false);
        let (written, _) = each(true);
        println!(
            "{}: one change, the log file added to in {added:?} or written whole in \
             {written:?}; what was judged of it written in {verdicts:?}",
            if sealed { "sealed" } else { "plain" },
        );
        println!(
            "{}: a log of {} entries, {size} bytes on disk, saved whole in {whole:?}; a \
             burst of 200 changes in {took:?}, {node:?} of it on the node's side \
             ({copying:?} copying the log); {saves} saves made, in {saving:?}; the file \
             grew by {} bytes",
            if sealed { "sealed" } else { "plain" },
            log.len(),
            after - size,
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
