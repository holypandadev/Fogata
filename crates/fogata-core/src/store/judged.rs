//! What was judged of each group log, kept beside it and sealed: a log judged once is not
//! judged again at every start, and a verdict that does not match the log counts for
//! nothing.

use std::{fs, path::Path};

use anyhow::Result;
use fogata_crypto::{
    GroupId,
    at_rest::{DataKey, Form, sealed_form},
};
use fogata_state::{Entry, GroupLog, Verdicts};
use zeroize::Zeroizing;

use super::{to_disk, write_private};

/// What is kept beside a log file, after its name: what was judged of it.
pub(crate) const JUDGED_SUFFIX: &str = ".judged";

/// What the verdicts on group `id`'s log are sealed for.
pub(super) fn judged_context(id: &GroupId) -> String {
    format!("judged/{}", hex::encode(id.as_bytes()))
}

/// The log of `group_id` made of `entries`, read from its file, taking as judged what the
/// verdicts kept at `path` say was judged of the very same entries (see [`Verdicts`]), so
/// that a group a member made costly to judge costs that once, not at every start. In a
/// sealed folder they count only sealed with its key, for `context`: whoever could write
/// them could make this device keep what it refused. Verdicts that do not open, or on other
/// entries (a log changed, cut short, or written by a version that judged otherwise), count
/// for nothing: the entries are judged as they come, and the verdicts on them are kept for
/// the next time.
pub(crate) fn read_judged(
    key: Option<&DataKey>,
    path: &Path,
    context: &str,
    group_id: GroupId,
    entries: Vec<Entry>,
) -> GroupLog {
    let verdicts = fs::read(path)
        .ok()
        .and_then(|bytes| match (key, sealed_form(&bytes)) {
            (Some(key), Some(Form::Whole | Form::Named)) => key.open_whole(context, &bytes).ok(),
            (None, None) => Some(Zeroizing::new(bytes)),
            _ => None,
        })
        .and_then(|plain| Verdicts::decode(&plain).ok());
    let (log, anew) = GroupLog::read_judged(group_id, entries, verdicts.as_ref(), key.is_some());
    if let Some(verdicts) = anew
        && let Err(e) = keep_verdicts(key, path, context, &verdicts)
    {
        tracing::debug!("cannot keep what was judged of {}: {e:#}", path.display());
    }
    log
}

/// Keeps what was judged of `log`, about to be written whole, at `path` (see
/// [`read_judged`]). One not kept only costs judging the log again when it is read.
pub(crate) fn write_judged(key: Option<&DataKey>, path: &Path, context: &str, log: &GroupLog) {
    let verdicts = log.verdicts(log.hashes_to_store());
    if let Err(e) = keep_verdicts(key, path, context, &verdicts) {
        tracing::debug!("cannot keep what was judged of {}: {e:#}", path.display());
    }
}

fn keep_verdicts(
    key: Option<&DataKey>,
    path: &Path,
    context: &str,
    verdicts: &Verdicts,
) -> Result<()> {
    write_private(path, &to_disk(key, context, &verdicts.encode())?)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };

    use fogata_crypto::DeviceIdentity;
    use fogata_state::{Body, work::Work};

    use super::*;
    use crate::{
        store::{Offered, Store, decode_entries, encode_entries, encode_log},
        vault::{Keychain, MemoryKeyStore},
    };

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-judged-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    pub(crate) fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
        let started = Instant::now();
        let out = f();
        (out, started.elapsed())
    }

    /// A group `by` made, of `n` entries written to cost the most to judge: `by` and Leo
    /// write apart, each signing only on its own last entry. With a promotion Leo signed of
    /// himself, which he may not make.
    pub(crate) fn costly_log(by: &DeviceIdentity, n: usize) -> (GroupLog, Entry) {
        let leo = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(by, "Home", 1).unwrap();
        log.add(by, leo.public_key(), "Leo", 2).unwrap();
        let group = log.group_id();
        let mut last = [log.heads(), log.heads()];
        let mut written = Vec::new();
        for i in 0..n {
            let author = [by, &leo][i % 2];
            let body = Body::Rename {
                name: format!("n{i}"),
            };
            let entry = Entry::sign(author, group, last[i % 2].clone(), 10 + i as u64, body);
            last[i % 2] = vec![entry.hash()];
            written.push(entry);
        }
        log.insert_all(written);
        let promote = Body::Promote {
            member: leo.public_key(),
        };
        let refused = Entry::sign(&leo, group, log.heads(), 900_000, promote);
        (log, refused)
    }

    /// What judging from nothing the entries a log file holds costs, and makes of them.
    pub(crate) fn judged_from_nothing(path: &Path) -> (GroupLog, Duration) {
        let (entries, _) = decode_entries(&fs::read(path).unwrap());
        let mut log = GroupLog::empty(entries[0].group_id);
        let ((), took) = timed(|| {
            log.insert_all(entries);
            drop(log.state());
        });
        (log, took)
    }

    /// The work `f` does: groups worked out, entries judged, signatures checked (see
    /// [`fogata_state::work`]). Counted rather than timed, which a busy computer changes.
    pub(crate) fn counted<T>(f: impl FnOnce() -> T) -> (T, Work) {
        Work::start();
        let out = f();
        (out, Work::done())
    }

    /// The work of judging from nothing the entries a log file holds.
    pub(crate) fn judging_from_nothing(path: &Path) -> Work {
        counted(|| drop(judged_from_nothing(path))).1
    }

    /// Whether `took` is a small part of the work of `judging` from nothing: no group worked
    /// out to judge an entry, the whole group worked out once at most, and at most a fifth
    /// of the entries tried (signatures are checked either way, as a file is read).
    pub(crate) fn cheaper(took: Work, judging: Work) -> bool {
        took.viewed == 0 && took.settled <= 1 && took.applied * 5 <= judging.applied
    }

    pub(crate) fn assert_same(log: &GroupLog, other: &GroupLog) {
        assert_eq!(log.hashes_in_order(), other.hashes_in_order());
        assert_eq!(log.waiting(), other.waiting());
        let (a, b) = (log.state(), other.state());
        assert_eq!((a.applied, a.ladder, a.name), (b.applied, b.ladder, b.name));
    }

    /// A group a member made costly to judge is judged once, not at every start: read back,
    /// it is the log judging its file from nothing makes, in a fraction of the time. A folder
    /// written before what was judged was kept opens as it did, judged whole once more.
    #[test]
    fn a_costly_log_is_judged_once_not_at_every_start() {
        let dir = folder("once");
        let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let (me, _) = store.identity().unwrap();
        let (log, _) = costly_log(&me, 600);
        let id = log.group_id();
        store.save_log("Home", &log).unwrap();
        let (from_nothing, _) = judged_from_nothing(&store.log_path(&id));
        assert_same(&from_nothing, &log);
        let judging = judging_from_nothing(&store.log_path(&id));
        let read = |store: &Store| {
            counted(|| {
                let log = store.load_log_of(&id).unwrap();
                drop(log.state());
                log
            })
        };
        let (back, took) = read(&store);
        assert_same(&back, &log);
        println!("read back {took:?}, judged whole {judging:?}");
        assert!(
            cheaper(took, judging),
            "read back {took:?}, judged whole {judging:?}"
        );
        // At the next start the same.
        let again = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        again.identity().unwrap();
        let (back, took) = read(&again);
        assert_same(&back, &log);
        assert!(cheaper(took, judging), "{took:?} against {judging:?}");
        // As written before: judged whole, the same log, and judged once.
        fs::remove_file(store.judged_path(&id)).unwrap();
        let (back, rejudged) = read(&again);
        assert_same(&back, &log);
        assert!(!cheaper(rejudged, judging), "judged again in {rejudged:?}");
        assert!(store.judged_path(&id).exists());
        let (back, took) = read(&again);
        assert_same(&back, &log);
        assert!(cheaper(took, judging), "{took:?} against {judging:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A group a manager raised past the levels this version judges at says so after a
    /// restart too: the raise this version refused is stored with the log and refused again
    /// when it is read, whether what was judged of the log is kept or not.
    #[test]
    fn a_group_gone_past_this_version_says_so_after_a_restart() {
        let dir = folder("ahead");
        let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let (me, _) = store.identity().unwrap();
        let mut log = GroupLog::create(&me, "Home", 1).unwrap();
        let id = log.group_id();
        let next = fogata_state::Level::CURRENT.number() + 1;
        let raise = Entry::sign(&me, id, log.heads(), 2, Body::Level { level: next });
        assert!(log.insert(raise).is_err());
        assert_eq!(log.level_ahead(), Some(next));
        store.save_log("Home", &log).unwrap();
        for _ in 0..2 {
            let again = Store::open_with(dir.clone(), Keychain::Off).unwrap();
            again.identity().unwrap();
            let back = again.load_log_of(&id).unwrap();
            assert_eq!(back.level_ahead(), Some(next));
            assert_same(&back, &log);
            again.save_log("Home", &back).unwrap();
        }
        // Without what was judged of it, as a folder of an earlier version.
        fs::remove_file(store.judged_path(&id)).unwrap();
        assert_eq!(store.load_log_of(&id).unwrap().level_ahead(), Some(next));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A data folder as 0.1.0-beta.9 left it opens: its groups list and screen times without
    /// what this version adds to them, and what it judged of a log in its own form (format 1,
    /// level three), which counts for nothing here: the log is judged once from its file,
    /// which is only read (a stop half way leaves it as it was), and what this version
    /// judged is kept in its place for the next start.
    #[test]
    fn a_folder_of_the_version_before_opens_and_its_logs_are_judged_once() {
        let dir = folder("beta9");
        let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let (me, _) = store.identity().unwrap();
        let (log, _) = costly_log(&me, 40);
        let id = log.group_id();
        store.save_log("Home", &log).unwrap();
        let file = fs::read(store.log_path(&id)).unwrap();
        // What beta.9 kept beside it: its magic, format 1, level three, way of judging 1.
        let mut old = b"fgjudged".to_vec();
        old.push(1);
        old.extend(3u32.to_be_bytes());
        old.extend(1u32.to_be_bytes());
        old.extend(id.as_bytes());
        old.extend([0u8; 32]);
        old.extend(0u32.to_be_bytes());
        old.extend(0u32.to_be_bytes());
        fs::write(store.judged_path(&id), &old).unwrap();
        // Its screen times, without the waits this version notes.
        fs::write(
            dir.join("meta.json"),
            br#"{"last_seen":{},"last_room":{},"port":39756,"reached":{}}"#,
        )
        .unwrap();
        drop(store);
        let again = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        again.identity().unwrap();
        assert_eq!(again.meta().port, Some(39756));
        assert!(again.meta().level_wait.is_empty());
        let (back, judging) = counted(|| again.load_log_of(&id).unwrap());
        assert_same(&back, &log);
        assert!(judging.viewed > 0, "judged from its entries: {judging:?}");
        assert_eq!(
            fs::read(again.log_path(&id)).unwrap(),
            file,
            "the log is only read"
        );
        let kept = fs::read(again.judged_path(&id)).unwrap();
        assert_ne!(kept, old);
        assert!(Verdicts::decode(&kept).is_ok());
        let (_, took) = counted(|| drop(again.load_log_of(&id).unwrap().state()));
        assert_eq!(took.viewed, 0, "judged once: {took:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What was judged of a log counts only for the very same entries, and in a sealed folder
    /// only sealed with its key for that log: a log changed or cut short since, verdicts
    /// left plain, sealed with another key or for another group, or damaged, count for
    /// nothing, and the log is what judging its file from nothing makes.
    #[test]
    fn stale_or_forged_verdicts_count_for_nothing() {
        let dir = folder("forged");
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys)).unwrap();
        let (me, _) = store.identity().unwrap();
        assert!(store.sealed());
        let key = store.data_key().unwrap().unwrap();
        let (mut log, refused) = costly_log(&me, 200);
        let id = log.group_id();
        store.save_log("Home", &log).unwrap();
        let (log_path, judged) = (store.log_path(&id), store.judged_path(&id));
        let read_back = || {
            let (from_nothing, _) = judged_from_nothing(&log_path);
            let back = store.load_log_of(&id).unwrap();
            assert_same(&back, &from_nothing);
            back
        };
        // Changed since: an entry more, and one an earlier version kept that its author could
        // not make.
        log.append(&me, Body::SetIcon { icon: 3 }, 950_000).unwrap();
        let mut bytes = encode_log(&log);
        bytes.extend(encode_entries(vec![&refused]));
        write_private(&log_path, &bytes).unwrap();
        let before = fs::read(&judged).unwrap();
        let back = read_back();
        assert!(back.get(&refused.hash()).is_none());
        assert_same(&back, &log);
        // Kept anew for these entries: the promotion refused.
        assert_ne!(fs::read(&judged).unwrap(), before);
        // The same verdicts saying it was kept: the refused set (after the level, the way of
        // judging, the group and the digest) emptied.
        let plain = key.open_whole(&judged_context(&id), &fs::read(&judged).unwrap());
        let plain = plain.unwrap().to_vec();
        let at = 8 + 1 + 4 + 4 + 16 + 32;
        assert_eq!(plain[at..at + 4], 1u32.to_be_bytes());
        let mut forged = plain[..at].to_vec();
        forged.extend(0u32.to_be_bytes());
        forged.extend(&plain[at + 4 + 32..]);
        assert!(Verdicts::decode(&forged).is_ok());
        let other = fogata_crypto::at_rest::DataKey::generate().unwrap();
        let elsewhere = judged_context(&GroupId::random().unwrap());
        for (what, bytes) in [
            ("plain", forged.clone()),
            (
                "sealed with another key",
                other.seal_whole(&judged_context(&id), &forged).unwrap(),
            ),
            (
                "sealed for another group",
                key.seal_whole(&elsewhere, &forged).unwrap(),
            ),
            ("damaged", b"fgjudged-not-really".to_vec()),
        ] {
            write_private(&judged, &bytes).unwrap();
            let back = read_back();
            assert!(back.get(&refused.hash()).is_none(), "{what}");
        }
        // Cut short: the entries that still read.
        let bytes = fs::read(&log_path).unwrap();
        write_private(&log_path, &bytes[..bytes.len() / 2]).unwrap();
        let back = read_back();
        assert!(back.len() < log.len());
        fs::remove_dir_all(&dir).unwrap();
    }

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

    /// A group a friend offers again, costly to judge, is not judged again on the core's
    /// events: neither the copy kept here, nor what the node judged of the offer, here the
    /// half of the log this copy lacks.
    #[test]
    fn a_costly_offer_is_taken_without_judging_it_again() {
        let dir = folder("offer");
        let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let (mut log, _) = costly_log(&ana, 600);
        log.add(&ana, me.public_key(), "Dani", 950_000).unwrap();
        let id = log.group_id();
        let mut half = GroupLog::empty(id);
        let entries = log.entries_in_order();
        half.insert_all(entries[..entries.len() / 2].iter().map(|e| (*e).clone()));
        assert!(matches!(
            store.save_offered_group(&half, &ana.public_key()).unwrap(),
            Offered::Pending(_)
        ));
        let (offered, took) = counted(|| store.save_offered_group(&log, &ana.public_key()));
        assert!(matches!(offered.unwrap(), Offered::Pending(_)));
        let judging = judging_from_nothing(&store.log_path(&id));
        println!("taken {took:?}, judged {judging:?}");
        assert!(cheaper(took, judging), "taken {took:?}, judged {judging:?}");
        assert_same(&store.load_log_of(&id).unwrap(), &log);
        fs::remove_dir_all(&dir).unwrap();
    }
}
