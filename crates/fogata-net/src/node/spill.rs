//! The messages of rooms' shares this device keeps on disk rather than in memory, past a
//! bound on what it holds of them in memory (see `Node::fit_room_shares` in room.rs). The
//! rooms still keep every share, and answer with every message as it was sent: only where
//! the bytes wait changes.
//!
//! One file at a time, in a folder of its own that is emptied at start (rooms end with the
//! program). Each message is sealed as one box under a key made for that file and held in
//! memory only, so the file reads as nothing once the program ends, or to anyone who copies
//! it meanwhile; the box names the message's number, so one box cannot be read in place of
//! another. The folder is made where nothing was (not even a link), and only its owner opens
//! it; the file is made new in it, readable by its owner only (on Unix; on Windows both are
//! as private as the data folder). A file found damaged is followed by a new one, with a new
//! key, for what comes next (see room.rs).
//!
//! The file is cut in pages of [`PAGE`] bytes. A message takes the first run of free pages
//! that holds it whole, so it is read in one go, or else the free pages there are, the
//! lowest first, and new ones at the end only once none is left: the pages of a message no
//! longer kept are the next one's. So the file only grows when every page it has is taken,
//! and is never longer than the most pages taken at once: the messages kept, each sealed
//! (some 80 bytes more) and rounded up to a page. Free pages at its end are cut off.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use fogata_crypto::at_rest::{DataKey, Form};
use fogata_proto::WireMessage;

/// Bytes of a page of the file.
pub(super) const PAGE: usize = 1024;
/// After a write that failed (a full or read-only disk), how long before writing again.
const WRITE_AGAIN: Duration = Duration::from_secs(60);

/// What the box of message `number` is sealed for.
fn context(number: u64) -> String {
    format!("room share {number}")
}

/// Pages of the file, as runs: the first page of each and how many.
type Runs = Box<[(u32, u32)]>;

pub(super) struct Spill {
    dir: PathBuf,
    /// Which file of this run it is: the first is 0, and each one after a loss the next.
    generation: u32,
    path: PathBuf,
    file: File,
    key: DataKey,
    /// Each message kept, by its number: its sealed length and its pages, as runs (the
    /// first page and how many).
    kept: HashMap<u64, (usize, Runs)>,
    /// The free pages before [`Self::end`], as runs: the first page and how many.
    free: BTreeMap<u32, u32>,
    /// The pages the file has.
    end: u32,
    next: u64,
    /// A message could not be read back: the file is taken as lost (see room.rs).
    lost: AtomicBool,
    /// When a write last failed.
    failed_at: Option<Instant>,
    /// Bytes written, and writes tried, for tests to see.
    #[cfg(test)]
    pub(super) written: (u64, usize),
    /// Writes fail, for tests.
    #[cfg(test)]
    pub(super) fail_writes: bool,
}

impl Spill {
    /// The first file of this run in `dir`, empty: what an earlier run left there goes, and
    /// the folder is made again where nothing is. Blocking.
    pub(super) fn open(dir: PathBuf) -> io::Result<Self> {
        match std::fs::remove_dir_all(&dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                // Not a folder at all (a link, say): it goes, so that the folder can be made.
                if std::fs::symlink_metadata(&dir).is_ok_and(|m| !m.is_dir()) {
                    std::fs::remove_file(&dir)?;
                } else {
                    return Err(e);
                }
            }
            Ok(()) => {}
        }
        // Only Unix has a mode to set.
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        // Not made along the way: the data folder is there, and a folder or a link that
        // turned up there since it was emptied is an error, not one to write in.
        builder.create(&dir)?;
        Self::create(dir, 0)
    }

    /// A new file in the same folder, with a new key, for what comes after this one was
    /// found damaged. Blocking.
    pub(super) fn fresh(&self) -> io::Result<Self> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| io::Error::other("too many files of rooms' shares"))?;
        Self::create(self.dir.clone(), generation)
    }

    fn create(dir: PathBuf, generation: u32) -> io::Result<Self> {
        let path = dir.join(format!("shares-{generation}"));
        let mut options = std::fs::OpenOptions::new();
        // Made new: never a file or a link already there.
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(&path)?;
        let key = DataKey::generate().map_err(io::Error::other)?;
        Ok(Self {
            dir,
            generation,
            path,
            file,
            key,
            kept: HashMap::new(),
            free: BTreeMap::new(),
            end: 0,
            next: 0,
            lost: AtomicBool::new(false),
            failed_at: None,
            #[cfg(test)]
            written: (0, 0),
            #[cfg(test)]
            fail_writes: false,
        })
    }

    /// Which file of this run it is.
    pub(super) fn generation(&self) -> u32 {
        self.generation
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Whether to write now: not for [`WRITE_AGAIN`] after a write failed.
    pub(super) fn writable(&self) -> bool {
        self.failed_at.is_none_or(|at| at.elapsed() >= WRITE_AGAIN)
    }

    /// Writes `wire` to the file; its number, to read it back with. A failure is said once,
    /// and no write is tried again for [`WRITE_AGAIN`] (see [`Self::writable`]).
    pub(super) fn put(&mut self, wire: &WireMessage) -> io::Result<u64> {
        let written = self.write(wire);
        if let Err(e) = &written {
            tracing::warn!("cannot keep rooms' shares on disk for a minute: {e}");
            self.failed_at = Some(Instant::now());
        }
        written
    }

    fn write(&mut self, wire: &WireMessage) -> io::Result<u64> {
        let number = self.next;
        let sealed = self
            .key
            .seal(Form::Whole, &context(number), &encode(wire))
            .map_err(io::Error::other)?;
        let pages = u32::try_from(sealed.len().div_ceil(PAGE))
            .map_err(|_| io::Error::other("a message too large"))?;
        let runs = self.take(pages)?;
        if let Err(e) = self.write_runs(&runs, &sealed) {
            self.give_back(&runs);
            self.trim();
            return Err(e);
        }
        self.next += 1;
        self.kept.insert(number, (sealed.len(), runs.into()));
        Ok(number)
    }

    /// Message `number` as it was written. A read that fails is tried once more (it may be
    /// passing); one that fails again (the file gone, changed or damaged) marks the file as
    /// lost.
    pub(super) fn read(&self, number: u64) -> io::Result<WireMessage> {
        let read = self.read_kept(number).or_else(|_| self.read_kept(number));
        if let Err(e) = &read
            && !self.lost.swap(true, Ordering::Relaxed)
        {
            tracing::warn!("a room's share kept on disk cannot be read back: {e}");
        }
        read
    }

    fn read_kept(&self, number: u64) -> io::Result<WireMessage> {
        let (len, runs) = self
            .kept
            .get(&number)
            .ok_or_else(|| io::Error::other("no such message"))?;
        let mut sealed = vec![0u8; *len];
        let mut at = 0;
        let mut file = &self.file;
        for (first, count) in runs.iter() {
            // The last page of a message is only written as far as the message goes.
            let bytes = (*count as usize * PAGE).min(*len - at);
            file.seek(SeekFrom::Start(u64::from(*first) * PAGE as u64))?;
            file.read_exact(&mut sealed[at..at + bytes])?;
            at += bytes;
        }
        let plain = self
            .key
            .open(Form::Whole, &context(number), &sealed)
            .map_err(io::Error::other)?;
        decode(&plain).ok_or_else(|| io::Error::other("a message that does not read"))
    }

    /// Whether a message could not be read back.
    pub(super) fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }

    /// Marks the file as lost, for tests.
    #[cfg(test)]
    pub(super) fn lose(&self) {
        self.lost.store(true, Ordering::Relaxed);
    }

    /// Keeps the messages numbered in `live` only: the pages of the others are free again,
    /// and those at the end of the file are cut off.
    pub(super) fn keep_only(&mut self, live: &HashSet<u64>) {
        let gone: Vec<u64> = self
            .kept
            .keys()
            .filter(|n| !live.contains(n))
            .copied()
            .collect();
        for number in gone {
            if let Some((_, runs)) = self.kept.remove(&number) {
                self.give_back(&runs);
            }
        }
        self.trim();
    }

    /// The bytes the file takes, and those its messages take as sealed.
    pub(super) fn bytes(&self) -> (u64, usize) {
        let kept = self.kept.values().map(|(len, _)| len).sum();
        (u64::from(self.end) * PAGE as u64, kept)
    }

    /// The pages the messages kept take.
    #[cfg(test)]
    pub(super) fn pages_taken(&self) -> u64 {
        self.kept
            .values()
            .map(|(len, _)| len.div_ceil(PAGE) as u64)
            .sum()
    }

    /// `pages` pages for a new message: the first free run that holds them all, else the
    /// free ones there are, the lowest first, then new ones at the end of the file.
    fn take(&mut self, mut pages: u32) -> io::Result<Vec<(u32, u32)>> {
        let whole = self
            .free
            .iter()
            .find(|(_, count)| **count >= pages)
            .map(|(first, count)| (*first, *count));
        if let Some((first, count)) = whole {
            self.free.remove(&first);
            if count > pages {
                self.free.insert(first + pages, count - pages);
            }
            return Ok(vec![(first, pages)]);
        }
        let mut runs = Vec::new();
        while pages > 0 {
            let Some((first, count)) = self.free.pop_first() else {
                break;
            };
            let used = count.min(pages);
            if used < count {
                self.free.insert(first + used, count - used);
            }
            runs.push((first, used));
            pages -= used;
        }
        if pages > 0 {
            let Some(end) = self.end.checked_add(pages) else {
                self.give_back(&runs);
                return Err(io::Error::other("the file of rooms' shares is full"));
            };
            runs.push((self.end, pages));
            self.end = end;
        }
        Ok(runs)
    }

    /// Frees `runs`, joined with the free pages beside them.
    fn give_back(&mut self, runs: &[(u32, u32)]) {
        for &(mut first, mut count) in runs {
            // Each page is freed once: never one that is free already.
            debug_assert!(
                self.free
                    .range(..first + count)
                    .next_back()
                    .is_none_or(|(f, c)| f + c <= first),
                "pages freed twice"
            );
            if let Some((&before, &len)) = self.free.range(..first).next_back()
                && before + len == first
            {
                self.free.remove(&before);
                first = before;
                count += len;
            }
            if let Some(len) = self.free.remove(&(first + count)) {
                count += len;
            }
            self.free.insert(first, count);
        }
    }

    /// Cuts off the free pages at the end of the file.
    fn trim(&mut self) {
        let end = self.end;
        if let Some((&first, &count)) = self.free.last_key_value()
            && first + count == end
        {
            self.free.remove(&first);
            self.end = first;
        }
        if self.end < end
            && let Err(e) = self.file.set_len(u64::from(self.end) * PAGE as u64)
        {
            tracing::debug!("cannot shorten the file of rooms' shares: {e}");
        }
    }

    fn write_runs(&mut self, runs: &[(u32, u32)], sealed: &[u8]) -> io::Result<()> {
        #[cfg(test)]
        {
            self.written.1 += 1;
            if self.fail_writes {
                return Err(io::Error::other("no room left on the disk"));
            }
            self.written.0 += sealed.len() as u64;
        }
        let mut file = &self.file;
        let mut at = 0;
        for (first, count) in runs {
            let bytes = (*count as usize * PAGE).min(sealed.len() - at);
            file.seek(SeekFrom::Start(u64::from(*first) * PAGE as u64))?;
            file.write_all(&sealed[at..at + bytes])?;
            at += bytes;
        }
        Ok(())
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A message as written: its version, epoch and time, then each of its byte fields after
/// its length.
fn encode(wire: &WireMessage) -> Vec<u8> {
    let fields = fields(wire);
    let mut out = Vec::with_capacity(13 + fields.iter().map(|f| 4 + f.len()).sum::<usize>());
    out.push(wire.version);
    out.extend_from_slice(&wire.epoch.to_le_bytes());
    out.extend_from_slice(&wire.timestamp_ms.to_le_bytes());
    for field in fields {
        out.extend_from_slice(&(field.len() as u32).to_le_bytes());
        out.extend_from_slice(field);
    }
    out
}

fn fields(wire: &WireMessage) -> [&Vec<u8>; 6] {
    [
        &wire.group_id,
        &wire.sender,
        &wire.message_id,
        &wire.nonce,
        &wire.ciphertext,
        &wire.signature,
    ]
}

fn decode(bytes: &[u8]) -> Option<WireMessage> {
    let (version, rest) = bytes.split_first()?;
    let (epoch, rest) = rest.split_first_chunk::<4>()?;
    let (timestamp, mut rest) = rest.split_first_chunk::<8>()?;
    let mut wire = WireMessage {
        version: *version,
        epoch: u32::from_le_bytes(*epoch),
        timestamp_ms: u64::from_le_bytes(*timestamp),
        group_id: Vec::new(),
        sender: Vec::new(),
        message_id: Vec::new(),
        nonce: Vec::new(),
        ciphertext: Vec::new(),
        signature: Vec::new(),
    };
    for field in [
        &mut wire.group_id,
        &mut wire.sender,
        &mut wire.message_id,
        &mut wire.nonce,
        &mut wire.ciphertext,
        &mut wire.signature,
    ] {
        let (len, after) = rest.split_first_chunk::<4>()?;
        let len = u32::from_le_bytes(*len) as usize;
        if after.len() < len {
            return None;
        }
        let (bytes, after) = after.split_at(len);
        *field = bytes.to_vec();
        rest = after;
    }
    rest.is_empty().then_some(wire)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: u8, ciphertext: usize) -> WireMessage {
        WireMessage {
            version: 1,
            group_id: vec![1; 16],
            epoch: 3,
            sender: vec![2; 32],
            timestamp_ms: 1_000 + u64::from(id),
            message_id: vec![id; 16],
            nonce: vec![4; 24],
            ciphertext: vec![id; ciphertext],
            signature: vec![6; 64],
        }
    }

    fn same(a: &WireMessage, b: &WireMessage) -> bool {
        encode(a) == encode(b)
    }

    fn dir(name: &str) -> PathBuf {
        let mut random = [0u8; 8];
        fogata_crypto::random_bytes(&mut random).unwrap();
        std::env::temp_dir().join(format!("fogata-spill-{name}-{}", hex::encode(random)))
    }

    /// Messages read back as written; the pages of those let go are the next ones', so the
    /// file holds no more pages than were taken at once, and its free end is cut off. What
    /// is written is sealed: none of a message's bytes are in the file as they are.
    #[test]
    fn messages_read_back_and_their_pages_are_used_again() {
        let dir = dir("pages");
        let mut spill = Spill::open(dir.clone()).unwrap();
        let mut numbers = Vec::new();
        for id in 0..40u8 {
            let size = if id % 3 == 0 { 90_000 } else { 400 };
            numbers.push((spill.put(&message(id, size)).unwrap(), id, size));
        }
        for (number, id, size) in &numbers {
            assert!(same(&spill.read(*number).unwrap(), &message(*id, *size)));
        }
        let (file, kept) = spill.bytes();
        let pages = |len: usize| len.div_ceil(PAGE) * PAGE;
        let taken: usize = spill.kept.values().map(|(len, _)| pages(*len)).sum();
        assert_eq!(file as usize, taken);
        assert!(kept < taken);
        let on_disk = std::fs::read(spill.path()).unwrap();
        assert!(!on_disk.windows(16).any(|w| w == [7u8; 16]));
        assert!(!on_disk.windows(32).any(|w| w == [2u8; 32]));
        // Half let go, then as many again written: the file does not grow.
        let live: HashSet<u64> = numbers.iter().step_by(2).map(|n| n.0).collect();
        spill.keep_only(&live);
        for id in 0..20u8 {
            let size = if id % 3 == 1 { 90_000 } else { 400 };
            spill.put(&message(100 + id, size)).unwrap();
        }
        assert!(spill.bytes().0 <= file, "{} > {file}", spill.bytes().0);
        for (number, id, size) in numbers.iter().step_by(2) {
            assert!(same(&spill.read(*number).unwrap(), &message(*id, *size)));
        }
        // All let go: the file is empty again.
        spill.keep_only(&HashSet::new());
        assert_eq!(spill.bytes(), (0, 0));
        assert_eq!(std::fs::metadata(spill.path()).unwrap().len(), 0);
        assert!(!spill.lost());
        let path = spill.path().to_path_buf();
        drop(spill);
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A file changed under the node does not read as another message: the read fails, and
    /// the file is taken as lost. What an earlier run left goes at start.
    #[test]
    fn a_damaged_file_reads_as_lost() {
        let dir = dir("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shares-0"), b"left by an earlier run").unwrap();
        let mut spill = Spill::open(dir.clone()).unwrap();
        assert_eq!(std::fs::metadata(spill.path()).unwrap().len(), 0);
        let (a, b) = (
            spill.put(&message(1, 3000)).unwrap(),
            spill.put(&message(2, 3000)).unwrap(),
        );
        // The second box copied over the first: it does not read as the first.
        let mut bytes = std::fs::read(spill.path()).unwrap();
        let second = bytes[4 * PAGE..].to_vec();
        bytes[..second.len()].copy_from_slice(&second);
        std::fs::write(spill.path(), &bytes).unwrap();
        assert!(spill.read(a).is_err());
        assert!(spill.lost());
        assert!(same(&spill.read(b).unwrap(), &message(2, 3000)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode();
            assert_eq!(mode(&dir) & 0o777, 0o700);
            assert_eq!(mode(spill.path()) & 0o777, 0o600);
        }
        // What comes next goes to a new file, under a new key.
        let mut next = spill.fresh().unwrap();
        assert_eq!(next.generation(), 1);
        assert_ne!(next.path(), spill.path());
        let c = next.put(&message(3, 3000)).unwrap();
        assert!(same(&next.read(c).unwrap(), &message(3, 3000)));
        assert!(!next.lost());
        drop((spill, next));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A write that fails (a full disk) takes no pages and leaves the file as it was, and
    /// none is tried again for a while.
    #[test]
    fn a_failed_write_takes_nothing() {
        let dir = dir("failed");
        let mut spill = Spill::open(dir.clone()).unwrap();
        let kept = spill.put(&message(1, 5000)).unwrap();
        let before = spill.bytes();
        spill.fail_writes = true;
        assert!(spill.writable());
        assert!(spill.put(&message(2, 90_000)).is_err());
        assert_eq!(spill.bytes(), before);
        assert!(spill.free.is_empty());
        assert!(!spill.writable());
        spill.fail_writes = false;
        assert!(same(&spill.read(kept).unwrap(), &message(1, 5000)));
        drop(spill);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Messages of every size coming and going over many rounds: the file is never longer
    /// than the most pages taken at once, every page is either free or a message's, and once
    /// all are let go each page was freed once (a page freed twice fails `give_back`) and
    /// the file is empty.
    #[test]
    fn the_file_stays_within_its_bound_through_churn() {
        let random = |below: u64| {
            let mut bytes = [0u8; 8];
            fogata_crypto::random_bytes(&mut bytes).unwrap();
            u64::from_le_bytes(bytes) % below
        };
        let dir = dir("churn");
        let mut spill = Spill::open(dir.clone()).unwrap();
        let mut live: Vec<u64> = Vec::new();
        let mut peak = 0;
        for round in 0..2_000u32 {
            if random(3) > 0 || live.is_empty() {
                let size = 100 + random(120_000) as usize;
                let number = spill.put(&message(round as u8, size)).unwrap();
                live.push(number);
                peak = peak.max(spill.pages_taken());
            } else {
                // Some let go, as rooms let shares go between slots.
                live.retain(|_| random(4) > 0);
                spill.keep_only(&live.iter().copied().collect());
            }
            let (file, _) = spill.bytes();
            assert!(
                file / PAGE as u64 <= peak,
                "{file} bytes, {peak} pages at most"
            );
            let free: u64 = spill.free.values().map(|c| u64::from(*c)).sum();
            let taken: u64 = spill
                .kept
                .values()
                .flat_map(|(_, runs)| runs.iter())
                .map(|(_, c)| u64::from(*c))
                .sum();
            assert_eq!(free + taken, u64::from(spill.end));
        }
        let last = *live.last().unwrap();
        assert!(spill.read(last).is_ok());
        spill.keep_only(&HashSet::new());
        assert_eq!(spill.bytes(), (0, 0));
        assert!(spill.free.is_empty());
        assert_eq!(std::fs::metadata(spill.path()).unwrap().len(), 0);
        drop(spill);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
