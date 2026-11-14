//! The messages of rooms' shares this device keeps on disk rather than in memory, past a
//! bound on what it holds of them in memory (see `Node::fit_room_shares` in room.rs). The
//! rooms still keep every share, and answer with every message as it was sent: only where
//! the bytes wait changes.
//!
//! One file, in a folder of its own that is emptied at start (rooms end with the program).
//! Each message is sealed as one box under a key made for this run and held in memory only,
//! so the file reads as nothing once the program ends, or to anyone who copies it meanwhile;
//! the box names the message's number, so one box cannot be read in place of another. The
//! file is made in a folder only its owner opens, readable by its owner only, where nothing
//! was (not even a link).
//!
//! The file is cut in pages of [`PAGE`] bytes, and a message takes as many as it needs,
//! wherever they are free, the lowest first: the pages of a message no longer kept are the
//! next one's. So the file only grows when every page it has is taken, and is never longer
//! than the most pages taken at once: the messages kept, each sealed (some 80 bytes more)
//! and rounded up to a page. Free pages at its end are cut off.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use fogata_crypto::at_rest::{DataKey, Form};
use fogata_proto::WireMessage;

/// Bytes of a page of the file.
pub(super) const PAGE: usize = 1024;
/// The file's name in its folder.
const FILE: &str = "shares";

/// What the box of message `number` is sealed for.
fn context(number: u64) -> String {
    format!("room share {number}")
}

/// Pages of the file, as runs: the first page of each and how many.
type Runs = Box<[(u32, u32)]>;

pub(super) struct Spill {
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
}

impl Spill {
    /// The file in `dir`, empty: what an earlier run left there goes. Blocking.
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
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.recursive(true).create(&dir)?;
        let path = dir.join(FILE);
        let mut options = std::fs::OpenOptions::new();
        // Made new: never a file or a link already there.
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let file = options.open(&path)?;
        let key = DataKey::generate().map_err(io::Error::other)?;
        Ok(Self {
            path,
            file,
            key,
            kept: HashMap::new(),
            free: BTreeMap::new(),
            end: 0,
            next: 0,
            lost: AtomicBool::new(false),
        })
    }

    /// Writes `wire` to the file; its number, to read it back with.
    pub(super) fn put(&mut self, wire: &WireMessage) -> io::Result<u64> {
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
            return Err(e);
        }
        self.next += 1;
        self.kept.insert(number, (sealed.len(), runs.into()));
        Ok(number)
    }

    /// Message `number` as it was written. A failure (the file gone, changed or damaged)
    /// marks the file as lost.
    pub(super) fn read(&self, number: u64) -> io::Result<WireMessage> {
        let read = self.read_kept(number);
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

    /// Keeps the messages numbered in `live` only: the pages of the others are free again,
    /// and those at the end of the file are cut off.
    pub(super) fn keep_only(&mut self, live: &HashSet<u64>) {
        let gone: Vec<u64> = self
            .kept
            .keys()
            .filter(|n| !live.contains(n))
            .copied()
            .collect();
        if gone.is_empty() {
            return;
        }
        for number in gone {
            if let Some((_, runs)) = self.kept.remove(&number) {
                self.give_back(&runs);
            }
        }
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

    /// The bytes the file takes, and those its messages take as sealed.
    pub(super) fn bytes(&self) -> (u64, usize) {
        let kept = self.kept.values().map(|(len, _)| len).sum();
        (u64::from(self.end) * PAGE as u64, kept)
    }

    /// `pages` pages for a new message: free ones, the lowest first, then new ones at the
    /// end of the file.
    fn take(&mut self, mut pages: u32) -> io::Result<Vec<(u32, u32)>> {
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

    fn write_runs(&self, runs: &[(u32, u32)], sealed: &[u8]) -> io::Result<()> {
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
        let on_disk = std::fs::read(dir.join(FILE)).unwrap();
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
        assert_eq!(std::fs::metadata(dir.join(FILE)).unwrap().len(), 0);
        assert!(!spill.lost());
        drop(spill);
        assert!(!dir.join(FILE).exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A file changed under the node does not read as another message: the read fails, and
    /// the file is taken as lost. What an earlier run left goes at start.
    #[test]
    fn a_damaged_file_reads_as_lost() {
        let dir = dir("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), b"left by an earlier run").unwrap();
        let mut spill = Spill::open(dir.clone()).unwrap();
        assert_eq!(std::fs::metadata(dir.join(FILE)).unwrap().len(), 0);
        let (a, b) = (
            spill.put(&message(1, 3000)).unwrap(),
            spill.put(&message(2, 3000)).unwrap(),
        );
        // The second box copied over the first: it does not read as the first.
        let mut bytes = std::fs::read(dir.join(FILE)).unwrap();
        let second = bytes[4 * PAGE..].to_vec();
        bytes[..second.len()].copy_from_slice(&second);
        std::fs::write(dir.join(FILE), &bytes).unwrap();
        assert!(spill.read(a).is_err());
        assert!(spill.lost());
        assert!(same(&spill.read(b).unwrap(), &message(2, 3000)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode();
            assert_eq!(mode(&dir) & 0o777, 0o700);
            assert_eq!(mode(&dir.join(FILE)) & 0o777, 0o600);
        }
        drop(spill);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
