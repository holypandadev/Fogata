//! A group's log file grown by adding what is new at its end, rather than written whole at
//! every change.
//!
//! The file is what it always was: entries one after another, each after its length, signed
//! and not sealed (what is secret in them is sealed to the identity), read whatever their
//! order. Written whole, they go in the common order with the `Level` entry a version too old
//! for the group refused last ([`GroupLog::entries_to_store`]); added to, the new ones follow
//! in the same order among themselves. Earlier versions read either the same way.
//!
//! - **When it is added to.** Only when this store knows what the file holds ([`OnDisk`]),
//!   having read it whole or written it itself, and finds it as it left it (its length, when
//!   it was last changed and, on Unix, which file it is): a file changed by anything else is
//!   written whole. And only with entries: when the log let go of one the file holds (spare
//!   ones trimmed, waiting ones past their room, a raise past this version replaced by a
//!   higher one), the file is written whole, so it never holds what the log does not, and
//!   what was judged of it (judged.rs) says the same of the file as of the log.
//! - **Written whole now and then.** Once what was added since the file was last written whole
//!   is larger than it was then, it is written whole again: each byte is written twice on
//!   average, and the file is back in the common order. Not when the application quits: a
//!   file added to reads as well as one written whole.
//! - **Cut short.** What is added is flushed to the disk before what was judged of the log is
//!   written; a write that fails is cut off again. A crash while adding leaves part of an
//!   entry at the end: reading stops there (the entries before it are kept, and that part is
//!   not a damage to put aside), and the file is written whole at the next change, so nothing
//!   lands behind it. What was judged is bound to the very entries read (`Verdicts`): when
//!   the entries added reached the disk and the verdicts on them did not, or only some of
//!   them did, the log is judged again from its file, once.
//!
//! The banner and picture records beside the log are written only when they change.

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::PoisonError,
    time::SystemTime,
};

use anyhow::{Context, Result};
use fogata_crypto::GroupId;
use fogata_state::{Entry, EntryHash, GroupLog};

#[cfg(test)]
use super::faults;
use super::{Store, decode_entries, encode_entries, encode_log, entry_at, write_private};

/// A file as a store left it: one found otherwise was changed by something else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    /// Its device and number: a file put in its place is another, whatever its length.
    #[cfg(unix)]
    file: (u64, u64),
}

impl Stamp {
    fn of(meta: &fs::Metadata) -> Self {
        Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            #[cfg(unix)]
            file: {
                use std::os::unix::fs::MetadataExt;
                (meta.dev(), meta.ino())
            },
        }
    }

    /// The file at `path` as it is now, when it is one (not a link).
    pub(super) fn at(path: &Path) -> Option<Self> {
        let meta = fs::symlink_metadata(path).ok()?;
        meta.is_file().then(|| Self::of(&meta))
    }
}

/// What a group's log file holds, as this store last read or wrote it.
#[derive(Debug)]
struct OnDisk {
    stamp: Stamp,
    /// The entries in it, each once.
    hashes: HashSet<EntryHash>,
    /// Its length when it was last written whole (or first read), and the bytes added since.
    whole: u64,
    added: u64,
}

/// What this store knows of the files it adds to or leaves alone.
#[derive(Debug, Default)]
pub(super) struct Kept {
    logs: HashMap<GroupId, OnDisk>,
    /// The banner or picture record each file holds, by its signature.
    records: HashMap<PathBuf, (Stamp, [u8; 64])>,
}

/// The entries of a log file, read from its start.
#[derive(Debug)]
pub(crate) struct LogFile {
    pub(crate) entries: Vec<Entry>,
    /// Some of it did not read: what reads past that is kept too ([`decode_entries`]).
    pub(crate) damaged: bool,
    /// It ends with part of an entry, as an entry being added leaves it when the computer
    /// stops: what came before is whole.
    pub(crate) cut_short: bool,
}

/// The entries of the log file `bytes`, telling an end cut short as an entry was added from
/// damage: the bytes after the last entry that reads are the start of one (its length says
/// it goes past the end, or there is not even a length) and hold none that reads.
pub(crate) fn read_log_file(bytes: &[u8]) -> LogFile {
    let mut entries = Vec::new();
    let mut at = 0;
    while let Some((entry, used)) = entry_at(&bytes[at..]) {
        entries.push(entry);
        at += used;
    }
    let rest = &bytes[at..];
    if rest.is_empty() {
        return LogFile {
            entries,
            damaged: false,
            cut_short: false,
        };
    }
    let (more, _) = decode_entries(rest);
    let started = rest.len() < 4
        || 4 + u32::from_be_bytes(rest[..4].try_into().expect("4 bytes")) as usize > rest.len();
    let cut_short = more.is_empty() && started;
    entries.extend(more);
    LogFile {
        entries,
        damaged: !cut_short,
        cut_short,
    }
}

impl Store {
    fn kept(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes what group `id`'s log file holds, read whole as `log`: `read` entries, found as
    /// `stamp` before it was read and still so. Only when the log holds every one of them
    /// and nothing else (as entries come only from the file, it does when it holds as many):
    /// otherwise the file is written whole at the next change.
    pub(super) fn note_read(&self, id: GroupId, stamp: Option<Stamp>, read: usize, log: &GroupLog) {
        let Some(stamp) = stamp.filter(|s| Some(*s) == Stamp::at(&self.log_path(&id))) else {
            return;
        };
        let hashes: HashSet<EntryHash> = log.hashes_to_store().collect();
        if hashes.len() != read {
            return;
        }
        let mut kept = self.kept();
        // Noted already as it is: what was added since it was last written whole counts on.
        if kept.logs.get(&id).is_some_and(|known| known.stamp == stamp) {
            return;
        }
        kept.logs.insert(
            id,
            OnDisk {
                stamp,
                hashes,
                whole: stamp.len,
                added: 0,
            },
        );
    }

    /// Stores `log` in its file: the entries it does not hold yet added at its end when it
    /// may be (see the module), else written whole.
    pub(super) fn store_log(&self, log: &GroupLog) -> Result<()> {
        let id = log.group_id();
        let path = self.log_path(&id);
        // Taken out while the file changes: put back only once it is as it says.
        let known = self.kept().logs.remove(&id);
        if let Some(known) = known
            && let Some(known) = self.add_to_log(&path, log, known)?
        {
            self.kept().logs.insert(id, known);
            return Ok(());
        }
        write_private(&path, &encode_log(log))?;
        if let Some(stamp) = Stamp::at(&path) {
            let known = OnDisk {
                stamp,
                hashes: log.hashes_to_store().collect(),
                whole: stamp.len,
                added: 0,
            };
            self.kept().logs.insert(id, known);
        }
        Ok(())
    }

    /// Adds to the file at `path`, known to hold what `known` says, the entries of `log` it
    /// does not hold; returns what it holds then, or `None` when it is to be written whole.
    fn add_to_log(&self, path: &Path, log: &GroupLog, known: OnDisk) -> Result<Option<OnDisk>> {
        let ahead = log.ahead_entry().map(|e| (e.hash(), e));
        let in_log = |h: &EntryHash| log.get(h).is_some() || ahead.is_some_and(|(a, _)| a == *h);
        if !known.hashes.iter().all(in_log) {
            return Ok(None);
        }
        let mut new: Vec<(EntryHash, &Entry)> = log
            .hashes_in_order()
            .into_iter()
            .filter(|h| !known.hashes.contains(h))
            .filter_map(|h| Some((h, log.get(&h)?)))
            .collect();
        new.extend(ahead.filter(|(h, _)| !known.hashes.contains(h)));
        if new.is_empty() {
            return Ok(Some(known));
        }
        let bytes = encode_entries(new.iter().map(|(_, e)| *e).collect());
        if known.added + bytes.len() as u64 > known.whole {
            return Ok(None);
        }
        if Stamp::at(path) != Some(known.stamp) {
            return Ok(None);
        }
        let Ok(mut file) = fs::OpenOptions::new().write(true).open(path) else {
            return Ok(None);
        };
        // Checked again on what was opened: the file could have been replaced meanwhile.
        if file.metadata().ok().map(|m| Stamp::of(&m)) != Some(known.stamp) {
            return Ok(None);
        }
        #[cfg(test)]
        faults::check().with_context(|| format!("cannot write {}", path.display()))?;
        let written = file
            .seek(SeekFrom::Start(known.stamp.len))
            .and_then(|_| file.write_all(&bytes))
            .and_then(|()| file.sync_data());
        if written.is_err() {
            // A full disk can take part of the entries: cut it off, or an entry added behind
            // it would not read. Should that fail too, the file is written whole at the next
            // change (it is no longer known), and read up to the part until then.
            if let Err(e) = file.set_len(known.stamp.len) {
                tracing::warn!("cannot cut {} back after a failed write: {e}", path.display());
            }
        }
        written.with_context(|| format!("cannot write {}", path.display()))?;
        drop(file);
        // As the file is once closed: some systems date a change only then.
        let Some(stamp) = Stamp::at(path) else {
            return Ok(None);
        };
        let mut known = known;
        known.hashes.extend(new.iter().map(|(h, _)| *h));
        known.added += bytes.len() as u64;
        known.stamp = stamp;
        Ok(Some(known))
    }

    /// Writes the group's banner or picture record `signature`, `encoded`, to `path`, unless
    /// the file holds it as this store left it.
    pub(super) fn write_record(
        &self,
        path: &Path,
        signature: [u8; 64],
        encoded: impl FnOnce() -> Vec<u8>,
    ) -> Result<()> {
        let mut kept = self.kept();
        if let Some((stamp, held)) = kept.records.get(path)
            && *held == signature
            && Stamp::at(path) == Some(*stamp)
        {
            return Ok(());
        }
        kept.records.remove(path);
        write_private(path, &encoded())?;
        if let Some(stamp) = Stamp::at(path) {
            kept.records.insert(path.to_path_buf(), (stamp, signature));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::{Body, Verdicts};

    use super::*;
    use crate::{
        store::judged::tests::{assert_same, cheaper, costly_log, counted, judging_from_nothing},
        vault::{Keychain, MemoryKeyStore},
    };

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "fogata-appended-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A folder's keychain: one that seals, the same for every store of the folder, or none.
    fn keychain(keys: &Option<Arc<MemoryKeyStore>>) -> Keychain {
        match keys {
            Some(keys) => Keychain::Seal(keys.clone()),
            None => Keychain::Off,
        }
    }

    /// The folder opened again, as at a start.
    fn reopen(dir: &Path, keys: &Option<Arc<MemoryKeyStore>>) -> Store {
        let store = Store::open_with(dir.to_path_buf(), keychain(keys)).unwrap();
        store.identity().unwrap();
        store
    }

    /// The log file at `path` reads whole: no part cut short nor damaged.
    fn reads_whole(path: &Path) -> LogFile {
        let file = read_log_file(&fs::read(path).unwrap());
        assert!(!file.damaged && !file.cut_short, "{}", path.display());
        file
    }

    /// A change of the group, made by one of its two members in turn.
    fn change(log: &mut GroupLog, by: &DeviceIdentity, i: u64) {
        let body = Body::Rename {
            name: format!("r{i}"),
        };
        log.append(by, body, 1_000_000 + i).unwrap();
    }

    /// A change adds the new entries to the end of the file, which reads as the log; one
    /// that adds nothing writes nothing.
    #[test]
    fn a_change_adds_the_new_entries_to_the_file() {
        let dir = folder("adds");
        let keys = Some(Arc::new(MemoryKeyStore::default()));
        let store = reopen(&dir, &keys);
        let (me, _) = store.identity().unwrap();
        let (mut log, _) = costly_log(&me, 40);
        let id = log.group_id();
        store.save_log("Home", &log).unwrap();
        let path = store.log_path(&id);
        let before = fs::read(&path).unwrap();
        change(&mut log, &me, 1);
        change(&mut log, &me, 2);
        store.save_log("Home", &log).unwrap();
        let after = fs::read(&path).unwrap();
        assert!(after.len() > before.len());
        assert_eq!(after[..before.len()], before[..], "added at the end");
        assert_eq!(reads_whole(&path).entries.len(), log.len());
        let stamp = Stamp::at(&path);
        store.save_log("Home", &log).unwrap();
        assert_eq!(Stamp::at(&path), stamp, "nothing new, nothing written");
        let again = reopen(&dir, &keys);
        assert_same(&again.load_log_of(&id).unwrap(), &log);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// An entry added to the file cut short at any byte, as a computer that stops while it is
    /// written leaves it, with what was judged of the log before or after it: read back, the
    /// log is the one before the change or after it, nothing else, judged once (what was
    /// judged is right, or judged anew and kept); and the next change leaves a file that
    /// reads whole. Entries added together and cut short between them read as the log with
    /// those before the cut.
    #[test]
    fn an_entry_cut_short_at_any_byte_reads_as_the_log_before_or_after() {
        for sealed in [false, true] {
            let dir = folder(&format!("cut-{sealed}"));
            let keys = sealed.then(|| Arc::new(MemoryKeyStore::default()));
            let store = reopen(&dir, &keys);
            let (me, _) = store.identity().unwrap();
            let (log, _) = costly_log(&me, 24);
            let id = log.group_id();
            store.save_log("Home", &log).unwrap();
            let (path, judged) = (store.log_path(&id), store.judged_path(&id));
            let file_before = fs::read(&path).unwrap();
            let judged_before = fs::read(&judged).unwrap();
            let mut one = log.clone();
            change(&mut one, &me, 1);
            let mut two = one.clone();
            change(&mut two, &me, 2);
            store.save_log("Home", &two).unwrap();
            let file_after = fs::read(&path).unwrap();
            let judged_after = fs::read(&judged).unwrap();
            let first_end = file_before.len() + 4 + {
                let at = file_before.len();
                u32::from_be_bytes(file_after[at..at + 4].try_into().unwrap()) as usize
            };
            drop(store);
            for cut in file_before.len()..=file_after.len() {
                let expected = if cut == file_after.len() {
                    &two
                } else if cut >= first_end {
                    &one
                } else {
                    &log
                };
                // What was judged before the change, or after it, in turn.
                {
                    let judged_bytes = [&judged_before, &judged_after][cut % 2];
                    fs::write(&path, &file_after[..cut]).unwrap();
                    fs::write(&judged, judged_bytes).unwrap();
                    let store = reopen(&dir, &keys);
                    let back = store.load_log_of(&id).unwrap();
                    assert_same(&back, expected);
                    let judging = judging_from_nothing(&path);
                    let (again, took) = counted(|| store.load_log_of(&id).unwrap());
                    assert_same(&again, expected);
                    assert!(cheaper(took, judging), "cut at {cut}: {took:?}, {judging:?}");
                    assert!(Verdicts::decode(&fs::read(&judged).unwrap()).is_ok() || sealed);
                    // Never a part an entry is to follow.
                    let mut next = back.clone();
                    change(&mut next, &me, 3);
                    store.save_log("Home", &next).unwrap();
                    assert_eq!(reads_whole(&path).entries.len(), next.len());
                    assert_same(&reopen(&dir, &keys).load_log_of(&id).unwrap(), &next);
                    // Nothing left aside for an entry cut short as it was added.
                    let aside = super::super::with_suffix(&path, ".damaged");
                    assert!(!aside.exists(), "cut at {cut}");
                }
            }
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// Once more was added than the file held when written whole, it is written whole again,
    /// in the common order. Stopped as entries are added or as it is written whole (a full
    /// disk, or a crash before the new file replaces the old one), the file is as it was,
    /// and the next change writes it whole.
    #[test]
    fn the_file_is_written_whole_now_and_then_and_a_stop_meanwhile_keeps_it() {
        let dir = folder("whole");
        let keys = None;
        let store = reopen(&dir, &keys);
        let (me, _) = store.identity().unwrap();
        let mut log = GroupLog::create(&me, "Home", 1).unwrap();
        let id = log.group_id();
        store.save_log("Home", &log).unwrap();
        let path = store.log_path(&id);
        // What was added since the file was last written whole.
        let added = |store: &Store| store.kept().logs.get(&id).map(|k| k.added);
        let mut i = 0;
        // Added to until it is written whole again.
        loop {
            i += 1;
            let before = fs::read(&path).unwrap();
            change(&mut log, &me, i);
            store.save_log("Home", &log).unwrap();
            let after = fs::read(&path).unwrap();
            match added(&store) {
                Some(0) => {
                    assert_eq!(after, encode_log(&log), "written whole in the common order");
                    break;
                }
                Some(_) => assert!(after.len() > before.len() && after.starts_with(&before)),
                None => panic!("not known after a change"),
            }
            assert!(i < 100);
        }
        assert!(i > 1, "{i}");
        // Stopped as entries are added (a full disk, say), and then as the file is written
        // whole, as it is after an addition that failed: the file stays as it was.
        let before = log.clone();
        let file = fs::read(&path).unwrap();
        for k in 1..=2 {
            change(&mut log, &me, i + k);
            faults::fail_after(Some(0));
            assert!(store.save_log("Home", &log).is_err());
            faults::fail_after(None);
            assert_eq!(fs::read(&path).unwrap(), file, "the file as it was");
        }
        // A new file written whole but never put in place reads as nothing.
        let tmp = super::super::with_suffix(&path, &format!(".{}.999.tmp", std::process::id()));
        fs::write(&tmp, encode_log(&log)).unwrap();
        assert_same(&reopen(&dir, &keys).load_log_of(&id).unwrap(), &before);
        store.save_log("Home", &log).unwrap();
        assert_eq!(fs::read(&path).unwrap(), encode_log(&log));
        assert_same(&reopen(&dir, &keys).load_log_of(&id).unwrap(), &log);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A file the log no longer holds every entry of is written whole: here the raise past
    /// this version's levels kept with the log, replaced by a higher one. One changed by
    /// something else than this store is written whole too.
    #[test]
    fn a_file_with_what_the_log_let_go_of_or_changed_elsewhere_is_written_whole() {
        let dir = folder("let-go");
        let keys = None;
        let store = reopen(&dir, &keys);
        let (me, _) = store.identity().unwrap();
        let mut log = GroupLog::create(&me, "Home", 1).unwrap();
        let id = log.group_id();
        let next = fogata_state::Level::CURRENT.number() + 1;
        let raise = Entry::sign(&me, id, log.heads(), 2, Body::Level { level: next });
        assert!(log.insert(raise).is_err());
        store.save_log("Home", &log).unwrap();
        let path = store.log_path(&id);
        let higher = Entry::sign(&me, id, log.heads(), 3, Body::Level { level: next + 1 });
        assert!(log.insert(higher).is_err());
        assert_eq!(log.level_ahead(), Some(next + 1));
        store.save_log("Home", &log).unwrap();
        assert_eq!(fs::read(&path).unwrap(), encode_log(&log));
        let back = reopen(&dir, &keys).load_log_of(&id).unwrap();
        assert_eq!(back.level_ahead(), Some(next + 1));
        assert_same(&back, &log);
        // Changed elsewhere (a copy put back, say): written whole.
        let older = encode_log(&back);
        change(&mut log, &me, 1);
        store.save_log("Home", &log).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, &older).unwrap();
        change(&mut log, &me, 2);
        store.save_log("Home", &log).unwrap();
        assert_eq!(fs::read(&path).unwrap(), encode_log(&log));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A data folder as 0.1.0-beta.9 or yesterday's build left it (a log written whole in the
    /// common order, and what beta.9 judged of it) opens, and its first change is added to
    /// the file, which reads as the log with it.
    #[test]
    fn a_folder_of_the_version_before_opens_and_its_first_change_is_added() {
        let dir = folder("before");
        let keys = None;
        let store = reopen(&dir, &keys);
        let (me, _) = store.identity().unwrap();
        let (mut log, _) = costly_log(&me, 40);
        let id = log.group_id();
        let path = store.log_path(&id);
        store.save_log("Home", &log).unwrap();
        // As beta.9 left it: the file in the common order, its verdicts in its own form.
        fs::write(&path, encode_log(&log)).unwrap();
        let mut old = b"fgjudged".to_vec();
        old.push(1);
        old.extend(3u32.to_be_bytes());
        old.extend(1u32.to_be_bytes());
        old.extend(id.as_bytes());
        old.extend([0u8; 32]);
        old.extend(0u32.to_be_bytes());
        old.extend(0u32.to_be_bytes());
        fs::write(store.judged_path(&id), &old).unwrap();
        drop(store);
        let file = fs::read(&path).unwrap();
        let store = reopen(&dir, &keys);
        let back = store.load_log_of(&id).unwrap();
        assert_same(&back, &log);
        change(&mut log, &me, 1);
        store.save_log("Home", &log).unwrap();
        let after = fs::read(&path).unwrap();
        assert!(after.len() > file.len() && after[..file.len()] == file[..]);
        let judging = judging_from_nothing(&path);
        let (back, took) = counted(|| reopen(&dir, &keys).load_log_of(&id).unwrap());
        assert_same(&back, &log);
        assert!(cheaper(took, judging), "{took:?} against {judging:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A small random generator, the same from the same seed.
    struct Random(u64);

    impl Random {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }
    }

    /// Random changes (one entry or a few, entries that wait for a parent that comes later,
    /// raises past this version), saves, restarts and stops while entries are added (the
    /// file cut anywhere in what the last save added, what was judged of it kept or not)
    /// always read back as the log in memory: at a stop, as before the save or after it,
    /// with the entries added before the cut.
    #[test]
    fn random_changes_restarts_and_stops_read_back_as_the_log() {
        for seed in 1..=24u64 {
            let dir = folder(&format!("random-{seed}"));
            let keys = (seed % 2 == 0).then(|| Arc::new(MemoryKeyStore::default()));
            let mut random = Random(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let mut store = reopen(&dir, &keys);
            let (me, _) = store.identity().unwrap();
            let leo = DeviceIdentity::generate().unwrap();
            let mut log = GroupLog::create(&me, "Home", 1).unwrap();
            log.add(&me, leo.public_key(), "Leo", 2).unwrap();
            let id = log.group_id();
            let path = store.log_path(&id);
            let judged = store.judged_path(&id);
            store.save_log("Home", &log).unwrap();
            let mut saved = log.clone();
            // Entries signed but not yet given to the log: their children wait for them.
            let mut held: Vec<Entry> = Vec::new();
            let mut time = 10;
            let mut level = fogata_state::Level::CURRENT.number();
            for step in 0..60 {
                time += 1;
                match random.below(10) {
                    0..=3 => {
                        for _ in 0..=random.below(3) {
                            time += 1;
                            let by = [&me, &leo][random.below(2) as usize];
                            change(&mut log, by, time);
                        }
                    }
                    4 => {
                        // An entry, and one signed on it that comes first and waits.
                        let parent = Entry::sign(
                            &leo,
                            id,
                            log.heads(),
                            time,
                            Body::Rename {
                                name: format!("p{step}"),
                            },
                        );
                        let child = Entry::sign(
                            &me,
                            id,
                            vec![parent.hash()],
                            time + 1,
                            Body::Rename {
                                name: format!("c{step}"),
                            },
                        );
                        log.insert(child).unwrap();
                        held.push(parent);
                    }
                    5 => {
                        if let Some(parent) = held.pop() {
                            log.insert(parent).unwrap();
                        }
                    }
                    6 => {
                        level += 1;
                        let raise =
                            Entry::sign(&me, id, log.heads(), time, Body::Level { level });
                        assert!(log.insert(raise).is_err());
                    }
                    7 => {
                        drop(store);
                        store = reopen(&dir, &keys);
                        let back = store.load_log_of(&id).unwrap();
                        assert_same(&back, &saved);
                        assert_eq!(back.level_ahead(), saved.level_ahead());
                        reads_whole(&path);
                    }
                    _ => {
                        let file_before = fs::read(&path).unwrap();
                        let judged_before = fs::read(&judged).unwrap();
                        store.save_log("Home", &log).unwrap();
                        let file_after = fs::read(&path).unwrap();
                        let added = file_after.starts_with(&file_before)
                            && file_after.len() > file_before.len();
                        if added && random.below(3) == 0 {
                            // Stopped as the entries were added.
                            let cut = file_before.len()
                                + random.below((file_after.len() - file_before.len()) as u64)
                                    as usize;
                            fs::write(&path, &file_after[..cut]).unwrap();
                            if random.below(2) == 0 {
                                fs::write(&judged, &judged_before).unwrap();
                            }
                            drop(store);
                            store = reopen(&dir, &keys);
                            let back = store.load_log_of(&id).unwrap();
                            // The log before, with the entries added before the cut.
                            let mut model = saved.clone();
                            let file = read_log_file(&file_after[..cut]);
                            let read: HashSet<EntryHash> =
                                file.entries.iter().map(Entry::hash).collect();
                            model.insert_all(
                                log.entries_to_store()
                                    .into_iter()
                                    .filter(|e| read.contains(&e.hash()))
                                    .cloned(),
                            );
                            assert_same(&back, &model);
                            // The next change written after it, never behind a part.
                            log = back;
                            held.clear();
                            change(&mut log, &me, time);
                            store.save_log("Home", &log).unwrap();
                            reads_whole(&path);
                        }
                        saved = log.clone();
                    }
                }
            }
            store.save_log("Home", &log).unwrap();
            let back = reopen(&dir, &keys).load_log_of(&id).unwrap();
            assert_same(&back, &log);
            assert_eq!(back.level_ahead(), log.level_ahead());
            fs::remove_dir_all(&dir).unwrap();
        }
    }
}
