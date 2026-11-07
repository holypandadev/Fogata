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
    let verdicts = log.verdicts(log.hashes().copied());
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
