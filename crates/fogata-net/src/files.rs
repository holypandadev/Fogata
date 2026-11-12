//! Files shared in a room, sent on request over a stream between the two devices. The
//! connection's own encryption covers the transfer end to end, also when a member carries it.
//! A file's id is the BLAKE3 hash of its contents, and the receiver checks it before keeping
//! the file. Only verified members of the file's group are served, and only files shared in
//! that group on purpose.
//!
//! A file kept in the data folder may be sealed with the folder's data key (see
//! [`fogata_crypto::at_rest`]): it is served as its plaintext, read as it goes, and a download
//! can be sealed as it arrives, its `.part` file too.

use std::{
    collections::{BTreeSet, HashMap},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures::{AsyncReadExt as _, AsyncWriteExt as _};
use libp2p::{PeerId, Stream, StreamProtocol};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    sync::{OwnedSemaphorePermit, Semaphore},
};

use fogata_crypto::{
    DevicePublicKey, GroupId,
    at_rest::{DataKey, Durable, SealReader, SealWriter, StreamFile},
};

use crate::{
    IncomingStreams, NetError, NodeHandle,
    slow::{Reader, Waiting},
};

pub const FILE_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/file/1");
const CHUNK: usize = 64 * 1024;
const FOUND: u8 = 0;
const NOT_HERE: u8 = 1;
/// Answered to a device sent all of its day: the seconds until its next day follow.
const LATER: u8 = 2;
/// The longest a fetch told to ask later waits before it asks again.
const LATER_MOST: Duration = Duration::from_secs(24 * 60 * 60);
/// How long a transfer waits for the other device to send or take the next bytes: a link
/// that died without closing would otherwise hold it forever.
const IDLE: Duration = Duration::from_secs(30);
/// Requests for files this device takes at once, from everyone and from one device (or one
/// connection not proved to be a member's); past that a request's stream is closed unread. A room's pictures are asked for all at once.
const MAX_REQUESTS: usize = 512;
const MAX_REQUESTS_PER_PEER: usize = 128;
/// Files this device sends at once, to everyone and to one device: each holds a file open
/// and a buffer, for as long as the other device takes the bytes. The others wait.
const MAX_SENDING: usize = 32;
const MAX_SENDING_PER_PEER: usize = 4;
/// Files sent again at the pace (see [`PACE`]) at once, to everyone and to one device: such
/// a transfer waits most of its time, and gives its turn among [`MAX_SENDING`] back for one
/// of these, so that members asking for what they had already cannot hold every turn for
/// hours while the others wait. One waiting for such a turn holds no file open.
const MAX_PACED: usize = 8;
const MAX_PACED_PER_PEER: usize = 1;
/// A member can ask for the same files again and again, each time with the upload of this
/// device's internet line. What it was sent of a file in the last day goes again only as
/// fast as the line takes it up to [`PACE_BURST`], and then at [`PACE`] a second, which
/// leaves most of a home's upload to calls and to the others however often one member asks;
/// what it was not sent comes back to it at that pace. A file the first time, or the rest
/// of one that stopped, goes as fast as the line takes it, whatever its size: nothing is
/// ever cut off on its way.
const PACE_BURST: u64 = 512 * 1024 * 1024;
const PACE: u64 = 1024 * 1024;
/// Askers whose pace is remembered; past it, those back to the whole burst are forgotten.
const PACE_KEPT: usize = 4096;
/// Files whose part sent to one asker is remembered, for a day: past it, every file goes
/// at the pace, so that asking for many in turn does not make them new again.
const SEEN_FILES: usize = 256;
const SEEN_FOR: Duration = Duration::from_secs(24 * 60 * 60);
/// What one device is sent in a day, at most, of everything: even at the pace, asking again
/// and again would take most of a hundred gigabytes a day of this device's upload. A file
/// that starts within the day goes whole; past it, the device is told when its next day
/// begins, and asks again then (see [`FileError::Later`]).
const DAY_BYTES: u64 = 32 << 30;
const DAY: Duration = Duration::from_secs(24 * 60 * 60);
/// A device whose files together were taken at less than [`LEAST_RATE`] over
/// [`SLOW_WINDOW`] of waiting for it loses the turn of each, while another device waits for
/// a turn (see crate::slow): a reader that takes a block every half minute would otherwise
/// hold its turns for the whole of a large file, and four such devices every turn of
/// everyone. The device goes on from what it got when it asks again. 4 KiB/s (32 kbit/s)
/// together is a third of the slowest honest lines seen, a satellite dish at 0.1 Mbit/s and
/// mobile data far from a mast: their files go on, also four at once, also through a stall
/// of most of a minute in a window. Below it, a 100 MiB file would take seven hours.
const LEAST_RATE: u64 = 4 * 1024;
const SLOW_WINDOW: Duration = Duration::from_secs(2 * 60);
/// How long a device that waited for a turn counts as waiting.
const WAITING_FOR: Duration = Duration::from_secs(2 * 60);
/// How long a request waits for a turn to send: less than the asker waits for the answer
/// ([`IDLE`]), so that it hears it should ask later rather than give up first.
const TURN_WAIT: Duration = Duration::from_secs(20);
/// What a device asking while every turn stays taken is told to wait before it asks again
/// (see [`FileError::Later`]): it is busy here, the file is still there.
const BUSY_LATER: Duration = Duration::from_secs(10);

/// `io`, or a timeout error when it does not finish within [`IDLE`].
async fn idle<T>(io: impl Future<Output = io::Result<T>>) -> io::Result<T> {
    within(IDLE, io).await
}

async fn within<T>(limit: Duration, io: impl Future<Output = io::Result<T>>) -> io::Result<T> {
    tokio::time::timeout(limit, io).await.unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the other device stopped answering",
        ))
    })
}

/// Files this device serves, by group and id: each read from a file kept in the data
/// folder, or from the copy made of one shared in a room (see [`Copies`]). Share a file by
/// adding it; stop by removing it, which stops at once every transfer of it under way.
#[derive(Clone, Debug, Default)]
pub struct SharedFiles {
    registry: Arc<Mutex<Registry>>,
    /// What each device was sent in its day.
    day: Arc<Day>,
}

/// A shared file: its group and id.
type FileKey = (GroupId, [u8; 32]);

#[derive(Debug, Default)]
struct Registry {
    files: HashMap<FileKey, Shared>,
    /// How many times each group's files, and each file, were stopped being served: a
    /// share confirmed only after its room took it is added only if nothing stopped it
    /// meanwhile (see [`SharedFiles::ticket`]).
    groups_stopped: HashMap<GroupId, u64>,
    files_stopped: HashMap<FileKey, u64>,
}

#[derive(Debug)]
struct Shared {
    source: Source,
    /// Where the person has it, shown to them: the file they shared, or the download.
    shown: PathBuf,
    /// Dropped when the file stops being served: transfers of it under way stop then.
    gone: tokio::sync::watch::Sender<()>,
}

/// What a shared file is read from.
#[derive(Clone, Debug)]
enum Source {
    /// A file of the data folder, which may be sealed with the key (see [`open_kept`]).
    Kept(PathBuf, Option<DataKey>),
    /// The copy made of a file shared in a room.
    Copy(Arc<Copy>),
}

impl Source {
    /// Opens it at `offset`, with its plaintext size. Blocking.
    fn open(&self, id: &[u8; 32], offset: u64) -> io::Result<(Kept, u64)> {
        let (mut file, size) = match self {
            Source::Kept(path, key) => open_kept(path, id, key.as_ref())?,
            Source::Copy(copy) => copy.open()?,
        };
        file.seek(SeekFrom::Start(offset))?;
        Ok((file, size))
    }
}

/// When the files a share would add were last stopped being served (see
/// [`SharedFiles::ticket`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    group: GroupId,
    id: [u8; 32],
    stopped: (u64, u64),
}

impl SharedFiles {
    pub fn insert(&self, group: GroupId, id: [u8; 32], path: PathBuf) {
        self.put((group, id), Source::Kept(path.clone(), None), path);
    }

    /// Shares a file kept in the data folder, which may be sealed with `key` (see
    /// [`open_kept`]).
    pub fn insert_kept(&self, group: GroupId, id: [u8; 32], path: PathBuf, key: Option<DataKey>) {
        self.put((group, id), Source::Kept(path.clone(), key), path);
    }

    /// Sends each device at most `most` bytes in a day `length` long, in place of
    /// [`DAY_BYTES`] in [`DAY`]: for tests, which cannot send that much.
    #[doc(hidden)]
    pub fn limit_day(&self, most: u64, length: Duration) {
        *self
            .day
            .limits
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = (most, length);
    }

    /// Takes note of what stopped `id` of `group` being served so far, before its share
    /// is told to its room: see [`Self::insert_copy`].
    pub fn ticket(&self, group: GroupId, id: [u8; 32]) -> Ticket {
        let registry = crate::locked(&self.registry);
        Ticket {
            group,
            id,
            stopped: registry.stopped(&(group, id)),
        }
    }

    /// Serves `copy` as file `ticket` names, which the person has at `shown`, once its room
    /// took it: unless it was stopped being served since the ticket was taken (the room was
    /// left, or the file stopped being shared, while the room was being told). Whether it
    /// is served. A file served already stays as it is.
    pub fn insert_copy(&self, ticket: Ticket, copy: Arc<Copy>, shown: PathBuf) -> bool {
        let key = (ticket.group, ticket.id);
        let mut registry = crate::locked(&self.registry);
        if registry.stopped(&key) != ticket.stopped {
            return false;
        }
        registry.files.entry(key).or_insert_with(|| Shared {
            source: Source::Copy(copy),
            shown,
            gone: tokio::sync::watch::Sender::new(()),
        });
        true
    }

    fn put(&self, key: FileKey, source: Source, shown: PathBuf) {
        let mut registry = crate::locked(&self.registry);
        match registry.files.get_mut(&key) {
            Some(shared) => (shared.source, shared.shown) = (source, shown),
            None => {
                registry.files.insert(
                    key,
                    Shared {
                        source,
                        shown,
                        gone: tokio::sync::watch::Sender::new(()),
                    },
                );
            }
        }
    }

    pub fn remove(&self, group: GroupId, id: [u8; 32]) {
        let mut registry = crate::locked(&self.registry);
        registry.files.remove(&(group, id));
        *registry.files_stopped.entry((group, id)).or_default() += 1;
    }

    /// Stops serving every file of `group`.
    pub fn clear_group(&self, group: GroupId) {
        let mut registry = crate::locked(&self.registry);
        registry.files.retain(|(g, _), _| *g != group);
        registry.files_stopped.retain(|(g, _), _| *g != group);
        *registry.groups_stopped.entry(group).or_default() += 1;
    }

    pub fn contains(&self, group: GroupId, id: [u8; 32]) -> bool {
        crate::locked(&self.registry)
            .files
            .contains_key(&(group, id))
    }

    /// Where the person has a file this device serves: their own, or one they downloaded.
    pub fn get(&self, group: GroupId, id: &[u8; 32]) -> Option<PathBuf> {
        crate::locked(&self.registry)
            .files
            .get(&(group, *id))
            .map(|shared| shared.shown.clone())
    }

    /// What to serve file `id` of `group` from, and what tells when it stops being served.
    fn entry(
        &self,
        group: GroupId,
        id: &[u8; 32],
    ) -> Option<(Source, tokio::sync::watch::Receiver<()>)> {
        crate::locked(&self.registry)
            .files
            .get(&(group, *id))
            .map(|shared| (shared.source.clone(), shared.gone.subscribe()))
    }
}

impl Registry {
    fn stopped(&self, key: &FileKey) -> (u64, u64) {
        (
            self.groups_stopped.get(&key.0).copied().unwrap_or(0),
            self.files_stopped.get(key).copied().unwrap_or(0),
        )
    }
}

/// `O_NONBLOCK` of the system's `open`, which the standard library does not name.
#[cfg(any(target_os = "linux", target_os = "android"))]
const O_NONBLOCK: i32 = 0o4000;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
const O_NONBLOCK: i32 = 0x0004;

/// Opens `path` to read it without waiting: a file put in its place that is a pipe would
/// wait at the open for a writer that never comes. A file's reads do not change. What is
/// opened must still be checked to be a file.
pub fn open_without_waiting(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, O_NONBLOCK);
    options.open(path)
}

/// Copies of the files this device shares in rooms, made as each is shared, in a folder of
/// the data folder of their own (sealed with its key, when it has one): what is served is
/// what was shared, whatever becomes of the file it came from afterwards (written over,
/// replaced, made a link to another), and its id is the hash of the bytes copied. A copy
/// goes once nothing serves or sends it any more, and what an earlier run left goes when
/// the next one starts: rooms do not outlast it.
#[derive(Debug)]
pub struct Copies {
    dir: PathBuf,
    key: Option<DataKey>,
    /// The copies kept now, by id: a file shared twice, or in two rooms, is kept once.
    kept: Mutex<HashMap<[u8; 32], std::sync::Weak<Copy>>>,
    /// Set once the device stops ([`Copies::stop`]): copies being made stop at their next
    /// part, and none is begun.
    stopped: AtomicBool,
}

/// A copy of a shared file (see [`Copies`]), removed once dropped.
#[derive(Debug)]
pub struct Copy {
    path: PathBuf,
    key: Option<DataKey>,
    /// What its seal is for: its own name, as its id is known only once it is made.
    context: String,
    size: u64,
}

impl Copy {
    /// Its plaintext size.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Opens it, with its plaintext size; only a copy sealed with the folder's key is read
    /// when it has one. Blocking.
    fn open(&self) -> io::Result<(Kept, u64)> {
        let file = std::fs::File::open(&self.path)?;
        let Some(key) = &self.key else {
            return Ok((Kept::Plain(file), self.size));
        };
        let len = file.metadata()?.len();
        let reader = SealReader::new(key, &self.context, file, len)?;
        Ok((Kept::Sealed(Box::new(reader)), self.size))
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path)
            && e.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!("cannot remove the copy of a file no longer shared: {e}");
        }
    }
}

impl Copies {
    /// The copies kept in `dir`, sealed with `key` when there is one; what an earlier run
    /// left there goes. What cannot go (a file another program holds open, say) is left
    /// there, as it is never served: it does not keep the device from starting. Blocking.
    pub fn open(dir: PathBuf, key: Option<DataKey>) -> io::Result<Self> {
        match std::fs::remove_dir_all(&dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                tracing::warn!(
                    "cannot remove all the copies an earlier run left in {}: {e}; what is \
                     left there stays",
                    dir.display()
                );
                // Not a folder at all: it goes, so that the folder can be made.
                if std::fs::symlink_metadata(&dir).is_ok_and(|m| !m.is_dir()) {
                    std::fs::remove_file(&dir)?;
                }
            }
            Ok(()) => {}
        }
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.recursive(true).create(&dir)?;
        Ok(Self {
            dir,
            key,
            kept: Mutex::default(),
            stopped: AtomicBool::new(false),
        })
    }

    /// Stops the copies being made, at their next part, and refuses new ones: the device
    /// stops, and what holds its data folder goes with them.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    /// Where the copies are.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Copies the file at `from`, as it is now, and hashes the bytes copied: the copy, the
    /// file's id and its size. Only a regular file is copied (a device or a pipe could be
    /// read without end); a link the person picked is followed this once. More than `most`
    /// bytes are not copied: the copy fails with [`io::ErrorKind::FileTooLarge`] (the file
    /// grew past the room held for it), and the disk keeps its room. A file already kept
    /// (the same bytes) is not kept twice. Once [`Copies::stop`] is called, the copy stops
    /// at its next part. Blocking.
    pub fn copy(&self, from: &Path, most: u64) -> io::Result<(Arc<Copy>, [u8; 32], u64)> {
        let not_a_file = || io::Error::new(io::ErrorKind::InvalidInput, "not a file");
        if self.stopped.load(Ordering::Relaxed) {
            return Err(stopped());
        }
        if !std::fs::metadata(from)?.is_file() {
            return Err(not_a_file());
        }
        let mut source = open_without_waiting(from)?;
        // It may have been put in another's place meanwhile.
        if !source.metadata()?.is_file() {
            return Err(not_a_file());
        }
        let mut name = [0u8; 12];
        fogata_crypto::random_bytes(&mut name).map_err(io::Error::other)?;
        let name = hex::encode(name);
        let path = self.dir.join(format!("{name}.copy"));
        let context = format!("shared copy {name}");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let out = options.open(&path)?;
        // Removed with it from here on, whatever fails.
        let mut copy = Copy {
            path,
            key: self.key.clone(),
            context,
            size: 0,
        };
        let mut hashing = Hashing {
            hasher: blake3::Hasher::new(),
            read: (&mut source).take(most.saturating_add(1)),
            stopped: &self.stopped,
        };
        let size = match &self.key {
            Some(key) => {
                let mut writer = SealWriter::new(key, &copy.context, out)?;
                let size = io::copy(&mut hashing, &mut writer)?;
                writer.finish()?;
                size
            }
            None => {
                let mut out = out;
                let size = io::copy(&mut hashing, &mut out)?;
                out.flush()?;
                size
            }
        };
        if size > most {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "the file grew while it was being copied",
            ));
        }
        copy.size = size;
        let id = *hashing.hasher.finalize().as_bytes();
        let mut kept = crate::locked(&self.kept);
        kept.retain(|_, copy| copy.strong_count() > 0);
        if let Some(earlier) = kept.get(&id).and_then(std::sync::Weak::upgrade) {
            return Ok((earlier, id, size));
        }
        let copy = Arc::new(copy);
        kept.insert(id, Arc::downgrade(&copy));
        Ok((copy, id, size))
    }
}

/// The error of a copy stopped by [`Copies::stop`]. Not [`io::ErrorKind::Interrupted`],
/// which a copy tries again.
fn stopped() -> io::Error {
    io::Error::other("the device stopped while the file was being copied")
}

/// A reader that hashes what is read through it, and stops once `stopped` is set.
struct Hashing<'a, R> {
    hasher: blake3::Hasher,
    read: R,
    stopped: &'a AtomicBool,
}

impl<R: Read> Read for Hashing<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.stopped.load(Ordering::Relaxed) {
            return Err(stopped());
        }
        let n = self.read.read(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }
}

/// What a kept file is sealed for: its id, so that one cannot stand for another.
pub fn kept_context(id: &[u8; 32]) -> String {
    format!("file {}", hex::encode(id))
}

/// A file read as its plaintext: as it is, or through its seal.
#[derive(Debug)]
pub enum Kept {
    Plain(std::fs::File),
    Sealed(Box<SealReader<std::fs::File>>),
}

impl Read for Kept {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Kept::Plain(file) => file.read(buf),
            Kept::Sealed(reader) => reader.read(buf),
        }
    }
}

impl Seek for Kept {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Kept::Plain(file) => file.seek(to),
            Kept::Sealed(reader) => reader.seek(to),
        }
    }
}

/// Opens file `id` kept at `path`, with its plaintext size. With `key`, a file sealed with it
/// is read through its seal; any other, one kept before its folder was sealed among them, is
/// read as it is. Blocking.
pub fn open_kept(path: &Path, id: &[u8; 32], key: Option<&DataKey>) -> io::Result<(Kept, u64)> {
    let file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    if let Some(key) = key {
        let mut start = [0u8; fogata_crypto::at_rest::HEADER_LEN];
        let n = (&file).read(&mut start)?;
        if fogata_crypto::at_rest::sealed_form(&start[..n]).is_some()
            && let Ok(reader) = SealReader::new(key, &kept_context(id), file.try_clone()?, len)
        {
            let len = reader.len();
            return Ok((Kept::Sealed(Box::new(reader)), len));
        }
    }
    let mut file = file;
    file.seek(SeekFrom::Start(0))?;
    Ok((Kept::Plain(file), len))
}

/// Runs `f` on `value` away from the async threads, and gives both back.
async fn blocking<T: Send + 'static, R: Send + 'static>(
    mut value: T,
    f: impl FnOnce(&mut T) -> R + Send + 'static,
) -> io::Result<(T, R)> {
    tokio::task::spawn_blocking(move || {
        let r = f(&mut value);
        (value, r)
    })
    .await
    .map_err(crate::task_failed)
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("the other device does not have the file")]
    NotThere,
    #[error("the file arrived damaged and was thrown away")]
    Damaged,
    #[error("cannot reach the other device: {0}")]
    Unreachable(String),
    /// The other device sent this one all it sends a device in a day, or is busy sending
    /// others their files: ask again after this long (at most [`LATER_MOST`]), and the rest
    /// comes then.
    #[error("the other device is busy or sent you all it sends in a day; the rest comes later")]
    Later(Duration),
    /// The other device stopped sending after some of the file came (a slow transfer ended
    /// for another device's, a link that went): what came is kept, and asked again, the
    /// file goes on from there.
    #[error("the other device stopped sending; the rest comes when asked again")]
    Cut,
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A file's id (BLAKE3 hash of its contents) and size. Reads the whole file; run it away from
/// the async threads. Only a regular file is read: a device can go on forever, and opening
/// a pipe waits for something to write to it.
pub fn hash_file(path: &Path) -> io::Result<([u8; 32], u64)> {
    let not_a_file = || io::Error::new(io::ErrorKind::InvalidInput, "not a file");
    if !std::fs::metadata(path)?.is_file() {
        return Err(not_a_file());
    }
    let mut hasher = blake3::Hasher::new();
    let mut file = open_without_waiting(path)?;
    // It may have been put in another's place meanwhile.
    if !file.metadata()?.is_file() {
        return Err(not_a_file());
    }
    let size = io::copy(&mut file, &mut hasher)?;
    Ok((*hasher.finalize().as_bytes(), size))
}

/// Who asks for a file: the device a member proved to be, which may connect under several
/// transport identities, and comes back under new ones; a member that has not proved which
/// device it is is not served.
type Asker = DevicePublicKey;

/// The requests for files being answered: how many, and turns to send, for everyone and
/// for each asker (see [`MAX_SENDING`]).
#[derive(Clone)]
struct Answering {
    peers: Arc<Mutex<HashMap<Asker, PeerRequests>>>,
    requests: Arc<Semaphore>,
    sending: Arc<Semaphore>,
    paced: Arc<Semaphore>,
    seen: Arc<Seen>,
    pace: Arc<Pace>,
    /// The devices that waited lately for a turn among everyone's, and among those at the
    /// pace (see [`LEAST_RATE`]).
    waiting: Arc<Waiting<Asker>>,
    waiting_paced: Arc<Waiting<Asker>>,
    /// The least rate a device's files are taken at together, and the window it is
    /// counted over ([`LEAST_RATE`], [`SLOW_WINDOW`]; others in tests).
    least: u64,
    window: Duration,
}

/// How far into each file each asker was sent it lately (see [`PACE_BURST`]).
#[derive(Default)]
struct Seen(Mutex<HashMap<Asker, SentFiles>>);

/// How far into each file an asker was sent it, and when last.
type SentFiles = HashMap<FileKey, (u64, tokio::time::Instant)>;

impl Seen {
    /// Counts the `bytes` of file `file` from `offset` sent to `asker` at `now`, and says how
    /// many of them it was sent already: they go at the pace.
    fn again(
        &self,
        asker: Asker,
        file: FileKey,
        offset: u64,
        bytes: u64,
        now: tokio::time::Instant,
    ) -> u64 {
        let mut askers = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let fresh =
            |(_, at): &(u64, tokio::time::Instant)| now.saturating_duration_since(*at) < SEEN_FOR;
        if askers.len() >= PACE_KEPT {
            askers.retain(|_, files| {
                files.retain(|_, sent| fresh(sent));
                !files.is_empty()
            });
        }
        let files = askers.entry(asker).or_default();
        let end = offset.saturating_add(bytes);
        let reached = match files.get(&file).filter(|sent| fresh(sent)) {
            Some((reached, _)) => *reached,
            None => {
                files.retain(|_, sent| fresh(sent));
                if files.len() >= SEEN_FILES {
                    return bytes;
                }
                0
            }
        };
        files.insert(file, (reached.max(end), now));
        reached.min(end).saturating_sub(offset)
    }
}

/// How many bytes each asker may still be sent at once (see [`PACE`]), and when that was
/// counted.
#[derive(Default)]
struct Pace(Mutex<HashMap<Asker, (u64, tokio::time::Instant)>>);

impl Pace {
    /// Counts `bytes` sent to `asker` at `now`, and says how long to wait before they go:
    /// nothing within its burst, else as long as it takes the pace to make up what is
    /// missing, after what others sent to it at once (its other files) wait for.
    fn take(&self, asker: Asker, bytes: u64, now: tokio::time::Instant) -> Duration {
        if bytes == 0 {
            return Duration::ZERO;
        }
        let mut askers = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let refilled = |(left, at): (u64, tokio::time::Instant)| {
            let made = now.saturating_duration_since(at).as_secs_f64() * PACE as f64;
            (left as f64 + made).min(PACE_BURST as f64) as u64
        };
        if askers.len() >= PACE_KEPT {
            askers.retain(|_, entry| refilled(*entry) < PACE_BURST);
        }
        // Counted from when it is all made up: now, or later while bytes wait for the pace.
        let (left, from) = match askers.get(&asker) {
            None => (PACE_BURST, now),
            Some(&(left, at)) if at > now => (left, at),
            Some(&entry) => (refilled(entry), now),
        };
        let missing = bytes.saturating_sub(left);
        let ready = from + Duration::from_secs_f64(missing as f64 / PACE as f64);
        askers.insert(asker, (left.saturating_sub(bytes), ready));
        ready.saturating_duration_since(now)
    }
}

/// What each asker was sent in its day: the bytes, and when its day began (see
/// [`DAY_BYTES`]); and how long a day is, and the most it is sent in one.
#[derive(Debug)]
struct Day {
    limits: Mutex<(u64, Duration)>,
    sent: Mutex<HashMap<Asker, (u64, tokio::time::Instant)>>,
}

impl Default for Day {
    fn default() -> Self {
        Self {
            limits: Mutex::new((DAY_BYTES, DAY)),
            sent: Mutex::default(),
        }
    }
}

impl Day {
    fn limits(&self) -> (u64, Duration) {
        *self.limits.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How long `asker` waits until its next day, if it was sent all of this one's at
    /// `now`.
    fn used_up(&self, asker: Asker, now: tokio::time::Instant) -> Option<Duration> {
        let (most, length) = self.limits();
        let sent = self.sent.lock().unwrap_or_else(PoisonError::into_inner);
        let &(bytes, began) = sent.get(&asker)?;
        let ends = began + length;
        (bytes >= most && ends > now).then(|| ends - now)
    }

    /// Counts `bytes` to send to `asker` from `now`, a whole file at once, unless it was
    /// sent all of its day already: then how long it waits until the next. Checked and
    /// counted together, so files that start at once cannot all pass the day's end: one
    /// that starts within it goes whole, and those after it wait. What does not go after
    /// all is given back ([`Day::give_back`]).
    fn start(
        &self,
        asker: Asker,
        bytes: u64,
        now: tokio::time::Instant,
    ) -> Result<tokio::time::Instant, Duration> {
        let (most, length) = self.limits();
        let mut sent = self.sent.lock().unwrap_or_else(PoisonError::into_inner);
        if sent.len() >= PACE_KEPT {
            sent.retain(|_, (_, began)| *began + length > now);
        }
        let day = sent.entry(asker).or_insert((0, now));
        if day.1 + length <= now {
            *day = (0, now);
        }
        if day.0 >= most {
            return Err(day.1 + length - now);
        }
        day.0 = day.0.saturating_add(bytes);
        Ok(day.1)
    }

    /// Gives back `bytes` counted for `asker` from the day that began `began` and not sent.
    fn give_back(&self, asker: Asker, bytes: u64, began: tokio::time::Instant) {
        let mut sent = self.sent.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(day) = sent.get_mut(&asker).filter(|day| day.1 == began) {
            day.0 = day.0.saturating_sub(bytes);
        }
    }
}

/// The part of a file counted against an asker's day and not sent yet: given back when the
/// transfer ends before it.
struct Counted {
    day: Arc<Day>,
    asker: Asker,
    began: tokio::time::Instant,
    left: u64,
}

impl Drop for Counted {
    fn drop(&mut self) {
        if self.left > 0 {
            self.day.give_back(self.asker, self.left, self.began);
        }
    }
}

/// An asker's requests being answered, and its turns to send, and to send at the pace.
type PeerRequests = (usize, Arc<Semaphore>, Arc<Semaphore>);

/// One request being answered; counted until dropped.
struct Request {
    answering: Answering,
    peer: Asker,
    _all: OwnedSemaphorePermit,
}

impl Answering {
    fn new() -> Self {
        Self {
            peers: Arc::default(),
            requests: Arc::new(Semaphore::new(MAX_REQUESTS)),
            sending: Arc::new(Semaphore::new(MAX_SENDING)),
            paced: Arc::new(Semaphore::new(MAX_PACED)),
            seen: Arc::default(),
            pace: Arc::default(),
            waiting: Arc::new(Waiting::new(WAITING_FOR)),
            waiting_paced: Arc::new(Waiting::new(WAITING_FOR)),
            least: LEAST_RATE,
            window: SLOW_WINDOW,
        }
    }

    fn peers(&self) -> std::sync::MutexGuard<'_, HashMap<Asker, PeerRequests>> {
        self.peers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A request from `peer`, if one more may be answered now.
    fn request(&self, peer: Asker) -> Option<Request> {
        let all = self.requests.clone().try_acquire_owned().ok()?;
        let mut peers = self.peers();
        let (count, _, _) = peers.entry(peer).or_insert_with(|| {
            (
                0,
                Arc::new(Semaphore::new(MAX_SENDING_PER_PEER)),
                Arc::new(Semaphore::new(MAX_PACED_PER_PEER)),
            )
        });
        if *count >= MAX_REQUESTS_PER_PEER {
            return None;
        }
        *count += 1;
        Some(Request {
            answering: self.clone(),
            peer,
            _all: all,
        })
    }
}

/// A turn to send: the asker's and everyone's, held until dropped.
type Turn = (OwnedSemaphorePermit, OwnedSemaphorePermit);

impl Request {
    /// Waits for a turn to send (the peer's and everyone's), at most [`TURN_WAIT`].
    async fn turn(&self) -> Option<Turn> {
        let own = self.answering.peers().get(&self.peer)?.1.clone();
        let all = &self.answering;
        self.wait(own, all.sending.clone(), &all.waiting).await
    }

    /// Waits for a turn to send at the pace (see [`MAX_PACED`]), at most [`TURN_WAIT`].
    async fn paced_turn(&self) -> Option<Turn> {
        let own = self.answering.peers().get(&self.peer)?.2.clone();
        let all = &self.answering;
        self.wait(own, all.paced.clone(), &all.waiting_paced).await
    }

    /// Waits for a turn of the asker's own, then for one of everyone's, noted in `waiting`
    /// when none is free: other devices' slow files then give theirs up (see
    /// [`LEAST_RATE`]). Waiting for one of its own, the asker waits for itself only.
    async fn wait(
        &self,
        own: Arc<Semaphore>,
        all: Arc<Semaphore>,
        waiting: &Waiting<Asker>,
    ) -> Option<Turn> {
        tokio::time::timeout(TURN_WAIT, async {
            let own = own.acquire_owned().await.ok()?;
            let all = match all.clone().try_acquire_owned() {
                Ok(all) => all,
                Err(_) => {
                    waiting.note(self.peer);
                    all.acquire_owned().await.ok()?
                }
            };
            Some((own, all))
        })
        .await
        .ok()
        .flatten()
    }

    /// How many of the asker's files are being sent now, at the pace or not.
    fn sending_now(&self, paced: bool) -> usize {
        let peers = self.answering.peers();
        let Some((_, own, own_paced)) = peers.get(&self.peer) else {
            return 1;
        };
        let (turns, most) = if paced {
            (own_paced, MAX_PACED_PER_PEER)
        } else {
            (own, MAX_SENDING_PER_PEER)
        };
        most.saturating_sub(turns.available_permits()).max(1)
    }

    /// Whether a file being sent, whose reader is `reader`, gives up its turn: it is taken
    /// at less than the asker's share of the least rate, and another device waits for a
    /// turn of the kind it holds (see [`LEAST_RATE`]).
    fn gives_way(&self, reader: &Reader, paced: bool) -> bool {
        let waiting = if paced {
            &self.answering.waiting_paced
        } else {
            &self.answering.waiting
        };
        reader.too_slow() && waiting.besides(&self.peer)
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        let mut peers = self.answering.peers();
        if let Some((count, _, _)) = peers.get_mut(&self.peer) {
            *count -= 1;
            if *count == 0 {
                peers.remove(&self.peer);
            }
        }
    }
}

/// Serves the files in `shared` to verified members of their groups, until the node stops.
/// `served` is called with the group and file id each time a transfer completes. A member
/// holds only so many files open here at once (see [`MAX_SENDING`]).
pub fn serve(
    node: NodeHandle,
    shared: SharedFiles,
    served: impl Fn(GroupId, [u8; 32]) + Send + Sync + 'static,
) -> Result<(), NetError> {
    serve_answering(node, shared, served, Answering::new())
}

fn serve_answering(
    node: NodeHandle,
    shared: SharedFiles,
    served: impl Fn(GroupId, [u8; 32]) + Send + Sync + 'static,
    answering: Answering,
) -> Result<(), NetError> {
    let incoming = node
        .streams()
        .accept(FILE_PROTOCOL)
        .map_err(|e| NetError::Setup(e.to_string()))?;
    serve_from(node, incoming, shared, served, answering);
    Ok(())
}

/// Answers the requests `incoming` takes (see [`serve`]).
fn serve_from(
    node: NodeHandle,
    mut incoming: IncomingStreams,
    shared: SharedFiles,
    served: impl Fn(GroupId, [u8; 32]) + Send + Sync + 'static,
    answering: Answering,
) {
    let served = Arc::new(served);
    // Serving ends with the node, and does not keep it running.
    let node = node.background();
    node.clone().spawn(async move {
        while let Some((peer, stream)) = node.next_stream(&mut incoming).await {
            if !node.is_member(&peer) {
                tracing::debug!(%peer, "file request from a peer that is not a member");
                continue;
            }
            let Some(asker) = node.member_device(&peer) else {
                tracing::debug!(%peer, "file request from a member not proved to be a device");
                continue;
            };
            let Some(request) = answering.request(asker) else {
                tracing::debug!(%peer, "too many file requests at once");
                continue;
            };
            let (sender, shared, served) = (node.clone(), shared.clone(), served.clone());
            node.spawn(async move {
                let node = sender;
                match send_file(stream, &node, peer, &shared, &request).await {
                    Ok(Some((group, id))) => served(group, id),
                    Ok(None) => {}
                    Err(e) => tracing::debug!(%peer, "file transfer stopped: {e}"),
                }
            });
        }
    });
}

/// How often a transfer waiting for the pace looks again whether the file is still shared
/// with the device waiting for it.
const STILL_SHARED: Duration = Duration::from_millis(500);

/// Request: group id, file id and the offset to start at. Answer: found, not here, or later
/// (and the seconds to wait); then the size and the bytes. Returns the file sent, if one was
/// sent to the end.
///
/// Each block goes only if, right before it is written, the file is still shared and the
/// device asking is still a member of the group: whatever it waited for meanwhile (a turn,
/// the pace, the other device taking the bytes). A file that stops being shared stops its
/// transfers at once, also while they wait.
async fn send_file(
    mut stream: Stream,
    node: &NodeHandle,
    peer: PeerId,
    shared: &SharedFiles,
    answering: &Request,
) -> io::Result<Option<(GroupId, [u8; 32])>> {
    let mut request = [0u8; 56];
    idle(stream.read_exact(&mut request)).await?;
    let group = GroupId::from_bytes(&request[..16])
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    let id: [u8; 32] = request[16..48].try_into().expect("32 bytes");
    let offset = u64::from_be_bytes(request[48..].try_into().expect("8 bytes"));
    let (asker, all) = (answering.peer, &answering.answering);
    let still_shared = || shared.contains(group, id) && node.is_member_of(&peer, &group);
    let served = shared
        .entry(group, &id)
        .filter(|_| node.is_member_of(&peer, &group));
    let Some((source, mut gone)) = served else {
        stream.write_all(&[NOT_HERE]).await?;
        stream.close().await?;
        return Ok(None);
    };
    if let Some(wait) = shared.day.used_up(asker, tokio::time::Instant::now()) {
        return answer_later(stream, wait).await;
    }
    // A turn to send, held until the transfer ends or waits for the pace: the file is open
    // meanwhile.
    let mut turn = until_gone(&mut gone, answering.turn()).await.flatten();
    // Every turn stayed taken for as long as a request waits: busy here, the file still
    // there. Asked again later, it comes then (or goes on from what came).
    if turn.is_none() && !is_gone(&gone) && still_shared() {
        return answer_later(stream, BUSY_LATER).await;
    }
    let open = |offset: u64| {
        let source = source.clone();
        async move {
            tokio::task::spawn_blocking(move || source.open(&id, offset).ok())
                .await
                .ok()
                .flatten()
        }
    };
    let file = match turn.is_some() && still_shared() && !is_gone(&gone) {
        true => open(offset).await,
        false => None,
    };
    let Some((file, size)) = file.filter(|(_, size)| offset <= *size) else {
        stream.write_all(&[NOT_HERE]).await?;
        stream.close().await?;
        return Ok(None);
    };
    // The whole rest of the file counts against the asker's day as it starts.
    let began = match shared
        .day
        .start(asker, size - offset, tokio::time::Instant::now())
    {
        Ok(began) => began,
        Err(wait) => return answer_later(stream, wait).await,
    };
    let mut counted = Counted {
        day: shared.day.clone(),
        asker,
        began,
        left: size - offset,
    };
    let mut file = Some(file);
    let mut paced = false;
    let mut at = offset;
    stream.write_all(&[FOUND]).await?;
    stream.write_all(&size.to_be_bytes()).await?;
    let mut buf = vec![0u8; CHUNK];
    let stopped = || io::Error::other("no longer shared with that device");
    let mut reader = Reader::new(all.window);
    loop {
        let open_file = match file.take() {
            Some(open_file) => open_file,
            None => open(at).await.ok_or_else(stopped)?.0,
        };
        let ((back, back_buf), n) =
            blocking((open_file, buf), |(file, buf)| file.read(buf)).await?;
        buf = back_buf;
        let n = n?;
        if n == 0 {
            break;
        }
        file = Some(back);
        let now = tokio::time::Instant::now();
        let again = all.seen.again(asker, (group, id), at, n as u64, now);
        let wait = all.pace.take(asker, again, now);
        if !wait.is_zero() {
            // Sent again, and waiting for the pace: the turn among everyone's goes back, and
            // the file is closed, while it waits for one of the few turns at the pace.
            if !paced {
                drop((turn.take(), file.take()));
                let paced_turn = until_gone(&mut gone, answering.paced_turn())
                    .await
                    .ok_or_else(stopped)?;
                turn =
                    Some(paced_turn.ok_or_else(|| {
                        io::Error::other("too many files being sent again at once")
                    })?);
                paced = true;
            }
            let until = tokio::time::Instant::now() + wait;
            while tokio::time::Instant::now() < until {
                if !still_shared() {
                    return Err(stopped());
                }
                let slice = until.min(tokio::time::Instant::now() + STILL_SHARED);
                until_gone(&mut gone, tokio::time::sleep_until(slice))
                    .await
                    .ok_or_else(stopped)?;
            }
        }
        // Stopped sharing, out of the room or the group, or the other device is no longer a
        // member: this block and the rest are not sent.
        if is_gone(&gone) || !still_shared() {
            return Err(stopped());
        }
        // Taken too slowly while another device waits: the turn goes to it, and the asker
        // goes on from here when it asks again.
        if answering.gives_way(&reader, paced) {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "taken too slowly while another device waits",
            ));
        }
        let writing = tokio::time::Instant::now();
        until_gone(&mut gone, idle(stream.write_all(&buf[..n])))
            .await
            .ok_or_else(stopped)??;
        // The asker's share of the least rate: its files are taken over one line.
        let least = all.least / answering.sending_now(paced) as u64;
        reader.took(n as u64, writing.elapsed(), least);
        at += n as u64;
        counted.left = counted.left.saturating_sub(n as u64);
    }
    stream.close().await?;
    Ok(Some((group, id)))
}

/// Answers that the asker asks again in `wait`, and when.
async fn answer_later(
    mut stream: Stream,
    wait: Duration,
) -> io::Result<Option<(GroupId, [u8; 32])>> {
    let seconds = u32::try_from(wait.as_secs().saturating_add(1)).unwrap_or(u32::MAX);
    stream.write_all(&[LATER]).await?;
    stream.write_all(&seconds.to_be_bytes()).await?;
    stream.close().await?;
    Ok(None)
}

/// Whether the file `gone` tells of stopped being served.
fn is_gone(gone: &tokio::sync::watch::Receiver<()>) -> bool {
    gone.has_changed().is_err()
}

/// `work`, unless the file `gone` tells of stops being served first: then `None`, and
/// `work` is dropped.
async fn until_gone<T>(
    gone: &mut tokio::sync::watch::Receiver<()>,
    work: impl Future<Output = T>,
) -> Option<T> {
    tokio::select! {
        biased;
        _ = gone.changed() => None,
        done = work => Some(done),
    }
}

/// How long an asker waits before it asks again, each time a request's stream was dropped
/// before any answer: copies of Fogata before 0.1.0-beta.9 keep only one of the streams of
/// one kind that come in together and drop the others (vendor/libp2p-stream/FOGATA.md), so
/// several previews asked of one of them at once failed on a busy computer. A request
/// changes nothing on the other device until it is answered, so asking again is safe.
pub(crate) const ASKED_AGAIN_AFTER: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(300),
    Duration::from_secs(1),
];

/// Asks `peer` for file `id` of `group` from `offset`, and reads the answer up to the file's
/// size: the stream the bytes come on once the file is found, or why not. A request whose
/// stream ends or is reset before the answer is made again (see [`ASKED_AGAIN_AFTER`]).
async fn ask(
    node: &NodeHandle,
    group: GroupId,
    peer: PeerId,
    id: [u8; 32],
    offset: u64,
) -> Result<Stream, FileError> {
    let mut again = ASKED_AGAIN_AFTER.iter();
    loop {
        let mut stream = node
            .streams()
            .open_stream(peer, FILE_PROTOCOL)
            .await
            .map_err(|e| FileError::Unreachable(e.to_string()))?;
        match request(&mut stream, group, id, offset).await {
            Ok(status) => {
                answered(&mut stream, status).await?;
                return Ok(stream);
            }
            Err(e) if dropped_unanswered(&e) => {
                let Some(wait) = again.next() else {
                    return Err(e.into());
                };
                tracing::debug!(%peer, "file request dropped unanswered, asking again: {e}");
                tokio::time::sleep(*wait).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Sends a request for file `id` of `group` from `offset` on `stream`, and reads the first
/// byte of the answer.
async fn request(stream: &mut Stream, group: GroupId, id: [u8; 32], offset: u64) -> io::Result<u8> {
    stream.write_all(group.as_bytes()).await?;
    stream.write_all(&id).await?;
    stream.write_all(&offset.to_be_bytes()).await?;
    stream.flush().await?;
    let mut status = [0u8; 1];
    idle(stream.read_exact(&mut status)).await?;
    Ok(status[0])
}

/// Whether a request's stream was ended or reset by the other end before it answered, as a
/// stream the other end dropped unread is. Not a request that went unanswered for
/// [`IDLE`], nor a connection that closed.
fn dropped_unanswered(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
    )
}

/// Reads the rest of the answer to a request for a file after its first byte, `status`, up
/// to its size: found, or why not.
async fn answered(stream: &mut Stream, status: u8) -> Result<(), FileError> {
    match status {
        FOUND => Ok(()),
        LATER => {
            let mut seconds = [0u8; 4];
            idle(stream.read_exact(&mut seconds)).await?;
            let wait = Duration::from_secs(u32::from_be_bytes(seconds).into());
            Err(FileError::Later(
                wait.clamp(Duration::from_secs(1), LATER_MOST),
            ))
        }
        _ => Err(FileError::NotThere),
    }
}

/// Fetches file `id` of `size` bytes, shared in `group`, from `peer` into memory, for a
/// preview: nothing is written to disk.
pub async fn fetch_to_memory(
    node: &NodeHandle,
    group: GroupId,
    peer: PeerId,
    id: [u8; 32],
    size: u64,
) -> Result<Vec<u8>, FileError> {
    let mut stream = ask(node, group, peer, id, 0).await?;
    let mut their_size = [0u8; 8];
    idle(stream.read_exact(&mut their_size)).await?;
    if u64::from_be_bytes(their_size) != size {
        return Err(FileError::Damaged);
    }
    let mut out = Vec::with_capacity(size as usize);
    let mut buf = vec![0u8; CHUNK];
    while (out.len() as u64) < size {
        let n = idle(stream.read(&mut buf)).await?;
        if n == 0 {
            break;
        }
        let n = n.min((size - out.len() as u64) as usize);
        out.extend_from_slice(&buf[..n]);
    }
    if (out.len() as u64) < size {
        return Err(FileError::Io(io::ErrorKind::UnexpectedEof.into()));
    }
    if blake3::hash(&out).as_bytes() != &id {
        return Err(FileError::Damaged);
    }
    Ok(out)
}

/// Downloads file `id` of `size` bytes, shared in `group`, from `peer` into `dest`, in place
/// of any file there: for folders of this application's own, under names of the file's id.
/// A `.part` file next to it, named after the file's id too, keeps what arrived, so a later
/// call for the same file resumes and one for another file of the same name never mixes
/// them. `progress` gets the bytes received so far.
pub async fn fetch(
    node: &NodeHandle,
    group: GroupId,
    peer: PeerId,
    id: [u8; 32],
    size: u64,
    dest: &Path,
    progress: impl FnMut(u64),
) -> Result<(), FileError> {
    let (part, writing) = fetch_part(node, group, peer, id, size, dest, None, progress).await?;
    rename_part(part, dest, writing).await?;
    Ok(())
}

/// As [`fetch`], into a file of the data folder sealed with `key` as it arrives (see
/// [`open_kept`]); its `.part` file is sealed too.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_sealed(
    node: &NodeHandle,
    group: GroupId,
    peer: PeerId,
    id: [u8; 32],
    size: u64,
    dest: &Path,
    key: &DataKey,
    progress: impl FnMut(u64),
) -> Result<(), FileError> {
    let (part, writing) =
        fetch_part(node, group, peer, id, size, dest, Some(key), progress).await?;
    rename_part(part, dest, writing).await?;
    Ok(())
}

/// Puts a checked `.part` file in its place, holding it meanwhile, however the download's
/// caller ends.
async fn rename_part(part: PathBuf, dest: &Path, writing: Writing) -> io::Result<()> {
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _writing = writing;
        std::fs::rename(&part, &dest)
    })
    .await
    .map_err(crate::task_failed)?
}

/// [`fetch`] into a folder people use (their downloads): a file already at `dest`, or one
/// that appeared there during the download, is never replaced; the download then goes to
/// the next free name, `name (2).ext` and so on. Returns where it went.
pub async fn fetch_new(
    node: &NodeHandle,
    group: GroupId,
    peer: PeerId,
    id: [u8; 32],
    size: u64,
    dest: &Path,
    progress: impl FnMut(u64),
) -> Result<PathBuf, FileError> {
    let (part, writing) = fetch_part(node, group, peer, id, size, dest, None, progress).await?;
    let dest = dest.to_path_buf();
    Ok(tokio::task::spawn_blocking(move || {
        let _writing = writing;
        place_new(&part, &dest)
    })
    .await
    .map_err(crate::task_failed)??)
}

/// The `.part` files downloads in this process write now, and a word each time one is let
/// go.
static WRITING: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());
static LET_GO: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// A `.part` file one download writes, until every copy is dropped: a second download into
/// the same place fails rather than add to it, as each would check only the bytes it got
/// itself, and the two together are not the file. Each piece of work on the file away from
/// the async threads holds a copy, so a download given up (its future dropped) holds the
/// file until the work it started has ended: such work cannot be stopped, and may still
/// write the file, or put a new one in its place.
#[derive(Clone)]
struct Writing {
    _held: Arc<Held>,
}

struct Held(PathBuf);

impl Writing {
    fn take(part: &Path) -> Result<Self, FileError> {
        if !crate::locked(&WRITING).insert(part.to_path_buf()) {
            return Err(FileError::Io(io::Error::other(
                "that file is being downloaded here already",
            )));
        }
        Ok(Self {
            _held: Arc::new(Held(part.to_path_buf())),
        })
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        crate::locked(&WRITING).remove(&self.0);
        LET_GO.notify_waiters();
    }
}

/// Removes the `.part` file at `part`, left by a download that was given up, once nothing
/// here works on it any more (see [`Writing`]); none can start on it meanwhile.
pub async fn remove_part(part: &Path) -> io::Result<()> {
    let writing = loop {
        let let_go = LET_GO.notified();
        if let Ok(writing) = Writing::take(part) {
            break writing;
        }
        let_go.await;
    };
    remove_held(part, writing).await
}

/// Removes the `.part` file at `part` unless a download here works on it now. Whether it
/// was free.
pub async fn remove_free_part(part: &Path) -> io::Result<bool> {
    match Writing::take(part) {
        Ok(writing) => remove_held(part, writing).await.map(|()| true),
        Err(_) => Ok(false),
    }
}

async fn remove_held(part: &Path, writing: Writing) -> io::Result<()> {
    let part = part.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _writing = writing;
        match std::fs::remove_file(&part) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            done => done,
        }
    })
    .await
    .map_err(crate::task_failed)?
}

/// Downloads into the `.part` file beside `dest`, sealed with `key` when there is one, and
/// checks it; returns that file's path, held until the caller puts it in its place.
#[allow(clippy::too_many_arguments)]
async fn fetch_part(
    node: &NodeHandle,
    group: GroupId,
    peer: PeerId,
    id: [u8; 32],
    size: u64,
    dest: &Path,
    key: Option<&DataKey>,
    mut progress: impl FnMut(u64),
) -> Result<(PathBuf, Writing), FileError> {
    let part = part_path(dest, &id);
    let writing = Writing::take(&part)?;
    let seal = key.map(|key| (key.clone(), kept_context(&id)));

    // What arrived earlier counts, after hashing it again.
    let mut hasher = blake3::Hasher::new();
    let mut have = 0u64;
    let mut earlier = match &seal {
        None => open_earlier_part(&part).await?.map(Out::Plain),
        Some((key, context)) => {
            let (writer, earlier) =
                resume_sealed_part(&part, key, context, writing.clone(), &mut progress).await?;
            if let Some(writer) = &writer {
                (hasher, have) = (earlier, writer.written());
            }
            writer.map(Out::Sealed)
        }
    };
    if let Some(Out::Plain(file)) = earlier.as_mut() {
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            have += n as u64;
        }
    }
    if have > size {
        drop(earlier.take());
        tokio::fs::remove_file(&part).await?;
        hasher = blake3::Hasher::new();
        have = 0;
    }

    let mut stream = ask(node, group, peer, id, have).await?;
    let mut their_size = [0u8; 8];
    idle(stream.read_exact(&mut their_size)).await?;
    if u64::from_be_bytes(their_size) != size {
        return Err(FileError::Damaged);
    }

    let mut out = match (earlier, &seal) {
        (Some(out), _) => out,
        (None, None) => Out::Plain(tokio::fs::File::from_std(
            create_held(&part, &writing).await?,
        )),
        (None, Some((key, context))) => {
            let file = StreamFile::new(create_held(&part, &writing).await?, part.clone());
            let (key, context, held) = (key.clone(), context.clone(), writing.clone());
            let writer = tokio::task::spawn_blocking(move || {
                let _held = held;
                SealWriter::new(&key, &context, file)
            })
            .await
            .map_err(crate::task_failed)??;
            Out::Sealed(Box::new(writer))
        }
    };
    let mut got = have;
    let mut buf = vec![0u8; CHUNK];
    while got < size {
        let n = match idle(stream.read(&mut buf)).await {
            Ok(n) => n,
            // Kept, for a later call to go on from (see below).
            Err(_) if got - have >= CHUNK as u64 => break,
            Err(e) => return Err(e.into()),
        };
        if n == 0 {
            break;
        }
        let n = n.min((size - got) as usize);
        hasher.update(&buf[..n]);
        out = match out {
            Out::Plain(mut file) => {
                file.write_all(&buf[..n]).await?;
                Out::Plain(file)
            }
            // Sealed away from the async threads, with the file held until it is written
            // even when the download is given up meanwhile; complete chunks are on the disk
            // as they go, for a later call to go on from.
            Out::Sealed(writer) => {
                let (bytes, held) = (buf[..n].to_vec(), writing.clone());
                let (writer, written) = blocking(writer, move |w| {
                    let _held = held;
                    w.write_all(&bytes)
                })
                .await?;
                written?;
                Out::Sealed(writer)
            }
        };
        got += n as u64;
        progress(got);
    }
    // What the file on the disk holds, which must be what was checked.
    let on_disk = match out {
        Out::Plain(mut file) => {
            file.flush().await?;
            file.metadata().await?.len()
        }
        Out::Sealed(writer) if got == size => {
            let held = writing.clone();
            tokio::task::spawn_blocking(move || {
                let _held = held;
                let mut file = writer.finish()?;
                file.settle()?;
                file.seek(io::SeekFrom::End(0))
            })
            .await
            .map_err(crate::task_failed)??
        }
        Out::Sealed(_) => 0,
    };
    // Stopped after a block or more came (a slow transfer given up for another device's, a
    // link that went): asked again, it goes on from there.
    if got < size && got - have >= CHUNK as u64 {
        return Err(FileError::Cut);
    }
    if got < size {
        return Err(FileError::Io(io::ErrorKind::UnexpectedEof.into()));
    }
    let whole = match seal {
        None => size,
        Some(_) => fogata_crypto::at_rest::stream_len(size),
    };
    if hasher.finalize().as_bytes() != &id || on_disk != whole {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(FileError::Damaged);
    }
    Ok((part, writing))
}

/// Where a download of file `id` into `dest` keeps what arrived until it is whole: beside
/// `dest`, named after the file's id too.
pub fn part_path(dest: &Path, id: &[u8; 32]) -> PathBuf {
    let mut part = dest.as_os_str().to_owned();
    part.push(format!(".{}.part", hex::encode(&id[..4])));
    PathBuf::from(part)
}

/// The `.part` file an interrupted download left, opened to add to it, if there is one.
/// Only a plain file is taken: a link there is removed rather than followed (whatever it
/// points to would be read and written to), and anything else fails the download.
pub(crate) async fn open_earlier_part(part: &Path) -> io::Result<Option<tokio::fs::File>> {
    open_earlier(part, true).await
}

/// [`open_earlier_part`] for writing anywhere in the file rather than only at its end, for
/// a caller that cuts it shorter first: Windows refuses to change the length of a file
/// opened only to append.
pub(crate) async fn open_earlier_part_to_cut(part: &Path) -> io::Result<Option<tokio::fs::File>> {
    open_earlier(part, false).await
}

async fn open_earlier(part: &Path, append: bool) -> io::Result<Option<tokio::fs::File>> {
    let part = part.to_path_buf();
    let file = tokio::task::spawn_blocking(move || open_earlier_now(&part, append))
        .await
        .map_err(crate::task_failed)??;
    Ok(file.map(tokio::fs::File::from_std))
}

/// [`open_earlier_part`], to be run away from the async threads; opened for writing
/// anywhere unless `append`.
fn open_earlier_now(part: &Path, append: bool) -> io::Result<Option<std::fs::File>> {
    let found = match look_at(part) {
        Ok(found) => found,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    #[cfg(test)]
    swap_part(part);
    if found.meta.file_type().is_symlink() {
        std::fs::remove_file(part)?;
        return Ok(None);
    }
    if !found.meta.is_file() {
        return Err(io::Error::other(
            "a partial download there is not a plain file",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    if append {
        options.read(true).append(true);
    } else {
        options.read(true).write(true);
    }
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut options, OPEN_REPARSE_POINT);
    let file = options.open(part)?;
    // What was opened must be the plain file looked at, not a link put there meanwhile.
    if !same_file(&found, &file)? {
        return Err(io::Error::other(
            "a partial download changed while it was opened",
        ));
    }
    Ok(Some(file))
}

/// Where a download writes: a plain file, or a sealed stream.
enum Out {
    Plain(tokio::fs::File),
    Sealed(Box<SealWriter<StreamFile>>),
}

/// The sealed `.part` file an interrupted download left, taken up from its complete chunks,
/// with the hash of what they hold; the same rules as [`open_earlier_part`]: only a plain
/// file, a link removed, anything else failing.
///
/// It runs away from the async threads, and holds `writing` until it is over, even when
/// the download is given up meanwhile: the next try could otherwise take up the same file
/// at the same time, and one cut off what the other added. What it has taken up so far is
/// told to `progress` every [`KEPT_EVERY`], so a download that waits only while bytes come
/// does not give up for a large file on a slow disk.
async fn resume_sealed_part(
    part: &Path,
    key: &DataKey,
    context: &str,
    writing: Writing,
    progress: &mut impl FnMut(u64),
) -> io::Result<(Option<Box<SealWriter<StreamFile>>>, blake3::Hasher)> {
    let (part, key, context) = (part.to_path_buf(), key.clone(), context.to_string());
    #[cfg(test)]
    let slow = slow_part(&part);
    let held = writing;
    let kept = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let counted = kept.clone();
    let mut resuming = tokio::task::spawn_blocking(move || {
        let _held = held;
        let mut earlier = blake3::Hasher::new();
        let Some(file) = open_earlier_now(&part, false)? else {
            return Ok((None, earlier));
        };
        let writer = SealWriter::resume(&key, &context, StreamFile::new(file, part), |chunk| {
            earlier.update(chunk);
            counted.fetch_add(chunk.len() as u64, std::sync::atomic::Ordering::Relaxed);
            #[cfg(test)]
            std::thread::sleep(slow);
        })?;
        Ok((Some(Box::new(writer)), earlier))
    });
    let mut told = 0;
    loop {
        tokio::select! {
            resumed = &mut resuming => return resumed.map_err(crate::task_failed)?,
            () = tokio::time::sleep(KEPT_EVERY) => {
                let now = kept.load(std::sync::atomic::Ordering::Relaxed);
                if now > told {
                    told = now;
                    progress(now);
                }
            }
        }
    }
}

/// How often a download hears how much of an earlier `.part` file was taken up, while that
/// goes on.
const KEPT_EVERY: Duration = Duration::from_secs(1);

/// `.part` files whose taking up is slowed by this much for each chunk kept: a slow disk,
/// for tests.
#[cfg(test)]
static SLOW_PARTS: Mutex<BTreeSet<(PathBuf, Duration)>> = Mutex::new(BTreeSet::new());

#[cfg(test)]
fn slow_part(part: &Path) -> Duration {
    crate::locked(&SLOW_PARTS)
        .iter()
        .find(|(p, _)| p == part)
        .map_or(Duration::ZERO, |(_, slow)| *slow)
}

/// `.part` files another file is put in place of right after they are looked at, before
/// they are opened, as another program could: (the part, the file moved there), for tests.
#[cfg(test)]
static SWAPPED_PARTS: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

#[cfg(test)]
fn swap_part(part: &Path) {
    let swap = {
        let mut swaps = crate::locked(&SWAPPED_PARTS);
        let at = swaps.iter().position(|(p, _)| p == part);
        at.map(|at| swaps.remove(at))
    };
    if let Some((part, with)) = swap {
        std::fs::rename(with, part).unwrap();
    }
}

/// [`create_part`] held by `writing` until it is made.
async fn create_held(part: &Path, writing: &Writing) -> io::Result<std::fs::File> {
    let (part, held) = (part.to_path_buf(), writing.clone());
    tokio::task::spawn_blocking(move || {
        let _held = held;
        let mut options = std::fs::OpenOptions::new();
        options.read(true).append(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(part)
    })
    .await
    .map_err(crate::task_failed)?
}

/// A new `.part` file, made where nothing is (not even a link), readable by its owner only.
pub(crate) async fn create_part(part: &Path) -> io::Result<tokio::fs::File> {
    let mut options = tokio::fs::OpenOptions::new();
    options.read(true).append(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(part).await
}

/// What a look at a `.part` file found, to compare with what is opened there next.
struct Looked {
    meta: std::fs::Metadata,
    /// The volume's serial number and the file's index on it, which Windows' metadata does
    /// not give on stable Rust.
    #[cfg(windows)]
    id: (u64, u64),
}

/// The file or link at `path`, never followed.
#[cfg(not(windows))]
fn look_at(path: &Path) -> io::Result<Looked> {
    Ok(Looked {
        meta: std::fs::symlink_metadata(path)?,
    })
}

/// The file or link at `path`, never followed: it is opened itself (as std's
/// `symlink_metadata` does), with no right to read or write it, and asked what it is.
#[cfg(windows)]
fn look_at(path: &Path) -> io::Result<Looked> {
    use std::os::windows::fs::OpenOptionsExt as _;
    /// `FILE_FLAG_BACKUP_SEMANTICS`, without which a folder cannot be opened.
    const BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let file = std::fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(OPEN_REPARSE_POINT | BACKUP_SEMANTICS)
        .open(path)?;
    let info = winapi_util::file::information(&file)?;
    Ok(Looked {
        meta: file.metadata()?,
        id: (info.volume_serial_number(), info.file_index()),
    })
}

#[cfg(unix)]
fn same_file(found: &Looked, file: &std::fs::File) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt as _;
    let (a, b) = (&found.meta, file.metadata()?);
    Ok(b.is_file() && a.dev() == b.dev() && a.ino() == b.ino())
}

/// Windows' `FILE_FLAG_OPEN_REPARSE_POINT`: a link at the path is opened itself, never what
/// it points to.
#[cfg(windows)]
const OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// On Windows the file was opened without following a link (`OPEN_REPARSE_POINT`), so a
/// link put there meanwhile is what was opened, and shows as one; any other file put there
/// (a hard link to one elsewhere among them) has another index, as unix's inode.
#[cfg(windows)]
fn same_file(found: &Looked, file: &std::fs::File) -> io::Result<bool> {
    use std::os::windows::fs::MetadataExt as _;
    /// `FILE_ATTRIBUTE_REPARSE_POINT`: a link, a junction or another reparse point.
    const REPARSE_POINT: u64 = 0x400;
    let info = winapi_util::file::information(file)?;
    Ok(file.metadata()?.is_file()
        && info.file_attributes() & REPARSE_POINT == 0
        && u64::from(found.meta.file_attributes()) & REPARSE_POINT == 0
        && (info.volume_serial_number(), info.file_index()) == found.id)
}

#[cfg(not(any(unix, windows)))]
fn same_file(_: &Looked, file: &std::fs::File) -> io::Result<bool> {
    Ok(file.metadata()?.is_file())
}

/// Gives the finished `part` the name `dest`, or the next free one (`name (2).ext`...),
/// never replacing a file or a link there: a hard link is made to the new name, which fails
/// if anything has it, and the part's name is removed. Where hard links cannot be made
/// (FAT drives, some network folders), the part is renamed once nothing is seen there.
fn place_new(part: &Path, dest: &Path) -> io::Result<PathBuf> {
    for n in 1..=10_000u32 {
        let target = if n == 1 {
            dest.to_path_buf()
        } else {
            numbered(dest, n)
        };
        match std::fs::hard_link(part, &target) {
            Ok(()) => {
                std::fs::remove_file(part)?;
                return Ok(target);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => {
                if std::fs::symlink_metadata(&target).is_ok() {
                    continue;
                }
                std::fs::rename(part, &target)?;
                return Ok(target);
            }
        }
    }
    Err(io::Error::other("no free name for the download"))
}

/// `dir/name (n).ext` for `dir/name.ext`.
fn numbered(dest: &Path, n: u32) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let numbered = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({n}).{ext}"),
        _ => format!("{name} ({n})"),
    };
    dest.with_file_name(numbered)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Work done off the async threads that panics fails the fetch with an error that says
    /// only that: what the panic said (a path, a value) goes to the log, scrubbed, by the
    /// panic hook, and never into the error, which reaches the screens as why the file did
    /// not come.
    #[tokio::test]
    async fn work_that_panics_fails_without_what_it_said() {
        let failed = blocking((), |_| -> () {
            panic!("cannot open /home/marta/holiday.mp4 from 203.0.113.7:4001")
        })
        .await
        .unwrap_err();
        let said = failed.to_string();
        for gone in ["marta", "holiday", "203.0.113.7", "panicked"] {
            assert!(!said.contains(gone), "{said}");
        }
        assert!(said.contains("internal error"), "{said}");
        let stopped = tokio::task::spawn(std::future::pending::<()>());
        stopped.abort();
        let stopped = crate::task_failed(stopped.await.unwrap_err()).to_string();
        assert_eq!(stopped, "the work was stopped");
    }

    /// A device of someone's.
    fn device() -> Asker {
        fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key()
    }

    /// [`resume_sealed_part`] by a download that holds `part` meanwhile.
    #[cfg(unix)]
    async fn resume(
        part: &Path,
        key: &DataKey,
        context: &str,
    ) -> io::Result<(Option<Box<SealWriter<StreamFile>>>, blake3::Hasher)> {
        let writing = Writing::take(part).unwrap();
        resume_sealed_part(part, key, context, writing, &mut |_| {}).await
    }

    /// A `.part` file sealed with `key`, holding `chunks` whole chunks of a download cut
    /// short.
    fn sealed_part(name: &str, key: &DataKey, chunks: usize) -> PathBuf {
        let dir = folder(name);
        let part = dir.join("party.mp4.part");
        let file = std::fs::File::create_new(&part).unwrap();
        let mut writer = SealWriter::new(key, "kept", StreamFile::new(file, part.clone())).unwrap();
        writer
            .write_all(&vec![7u8; chunks * fogata_crypto::at_rest::CHUNK])
            .unwrap();
        writer.flush().unwrap();
        part
    }

    /// A download given up while it takes up what an earlier one left (a large file on a slow
    /// disk) keeps the `.part` file to itself until that is over: the next try would
    /// otherwise take it up at the same time, and one cut what the other added. And while it
    /// goes on, the download hears of it, so one that waits only while bytes come does not
    /// give up for it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_part_being_taken_up_stays_with_its_download_until_that_is_over() {
        let key = DataKey::generate().unwrap();
        let part = sealed_part("taken-up", &key, 6);
        crate::locked(&SLOW_PARTS).insert((part.clone(), Duration::from_millis(400)));
        // Given up after a moment, as a fetch past its limit is.
        let given_up = tokio::time::timeout(Duration::from_millis(300), async {
            let writing = Writing::take(&part).unwrap();
            resume_sealed_part(&part, &key, "kept", writing, &mut |_| {}).await
        })
        .await;
        assert!(given_up.is_err());
        assert!(
            Writing::take(&part).is_err(),
            "the next try took the part up at once"
        );
        // Once the first is over, the next goes on from what it kept.
        let started = std::time::Instant::now();
        let writing = loop {
            match Writing::take(&part) {
                Ok(writing) => break writing,
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        };
        assert!(started.elapsed() > Duration::from_secs(1));
        let mut heard = Vec::new();
        let (writer, _) =
            resume_sealed_part(&part, &key, "kept", writing.clone(), &mut |n| heard.push(n))
                .await
                .unwrap();
        let kept = writer.unwrap().written();
        assert!(kept >= 5 * fogata_crypto::at_rest::CHUNK as u64, "{kept}");
        assert!(
            heard.iter().any(|n| *n > 0 && *n < kept),
            "nothing heard while it was taken up: {heard:?}"
        );
        crate::locked(&SLOW_PARTS).clear();
        let _ = std::fs::remove_dir_all(part.parent().unwrap());
    }

    /// A folder, a device or a pipe dropped as a file to share is refused at once, never
    /// read without end.
    /// A member is sent its burst of files as fast as the line takes them, and then at the
    /// pace, however many it asks for at once; what it was not sent comes back at the pace.
    /// Another member is not slowed by it.
    #[test]
    fn a_member_is_sent_its_burst_at_once_and_then_at_the_pace() {
        let pace = Pace::default();
        let (bea, leo) = (device(), device());
        let start = tokio::time::Instant::now();
        let chunk = CHUNK as u64;
        let mut waited = Duration::ZERO;
        for _ in 0..PACE_BURST / chunk {
            waited += pace.take(bea, chunk, start);
        }
        assert_eq!(waited, Duration::ZERO);
        // Past it, each chunk waits for the pace to make it up, after the ones before.
        let one = Duration::from_secs_f64(chunk as f64 / PACE as f64);
        let mut at = start;
        for _ in 0..16 {
            let wait = pace.take(bea, chunk, at);
            assert!(wait.abs_diff(one) < Duration::from_micros(10), "{wait:?}");
            at += wait;
        }
        assert_eq!(pace.take(leo, chunk, at), Duration::ZERO);
        // Two files at once wait one after the other.
        let first = pace.take(bea, chunk, at);
        let second = pace.take(bea, chunk, at);
        assert!(
            second.abs_diff(first + one) < Duration::from_micros(10),
            "{second:?}"
        );
        at += second;
        // A minute without asking gives back a minute's worth, never more than the burst.
        let later = at + Duration::from_secs(60);
        let mut waited = Duration::ZERO;
        for _ in 0..60 * PACE / chunk {
            waited += pace.take(bea, chunk, later);
        }
        assert!(waited < Duration::from_millis(1), "{waited:?}");
        assert!(!pace.take(bea, chunk, later).is_zero());
        let much_later = later + Duration::from_secs(24 * 3600);
        assert_eq!(pace.take(bea, PACE_BURST, much_later), Duration::ZERO);
        assert!(!pace.take(bea, chunk, much_later).is_zero());
    }

    /// A 5 GiB file goes whole and at once the first time, as does the rest of one that
    /// stopped: nothing is cut off on its way. Asked for again, it goes whole too, at the
    /// pace past the burst. Past so many files remembered, each goes at the pace.
    #[test]
    fn a_large_file_goes_whole_and_only_what_goes_again_waits() {
        let (seen, pace) = (Seen::default(), Pace::default());
        let bea = device();
        let group = GroupId::random().unwrap();
        let (big, size) = ((group, [1; 32]), 5u64 << 30);
        let now = tokio::time::Instant::now();
        // How long until the last byte may go, all of it asked for at once.
        let send = |from: u64, to: u64| {
            let mut last = Duration::ZERO;
            let mut at = from;
            while at < to {
                let n = (CHUNK as u64).min(to - at);
                last = last.max(pace.take(bea, seen.again(bea, big, at, n, now), now));
                at += n;
            }
            last
        };
        // Stopped at 3 GiB, then on from there: all of it at once.
        assert_eq!(send(0, 3 << 30), Duration::ZERO);
        assert_eq!(send(3 << 30, size), Duration::ZERO);
        // Again from the start: the burst at once, the rest at the pace, all of it sent.
        let again = send(0, size);
        let paced = Duration::from_secs_f64((size - PACE_BURST) as f64 / PACE as f64);
        assert!(again.abs_diff(paced) < Duration::from_secs(1), "{again:?}");
        // A day later it is new again.
        let later = now + SEEN_FOR;
        assert_eq!(seen.again(bea, big, 0, 10, later), 0);
        // Many small files, each new: past the ones remembered, each counts as sent again.
        let leo = device();
        let counted: Vec<u64> = (0..=SEEN_FILES)
            .map(|i| seen.again(leo, (group, [i as u8; 32]), 0, 10, now))
            .collect();
        assert!(counted[..SEEN_FILES].iter().all(|n| *n == 0));
        assert_eq!(counted[SEEN_FILES], 10);
    }

    #[test]
    fn only_a_regular_file_is_hashed() {
        fn soon(path: PathBuf) -> io::Result<([u8; 32], u64)> {
            let (done, result) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = done.send(hash_file(&path));
            });
            result
                .recv_timeout(Duration::from_secs(5))
                .expect("still reading")
        }
        let dir = std::env::temp_dir().join(format!("fogata-hash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo.jpg"), b"picture").unwrap();
        let (id, size) = soon(dir.join("photo.jpg")).unwrap();
        assert_eq!((id, size), (*blake3::hash(b"picture").as_bytes(), 7));
        assert!(soon(dir.clone()).is_err());
        #[cfg(unix)]
        {
            assert!(soon(PathBuf::from("/dev/zero")).is_err());
            let pipe = dir.join("pipe");
            let made = std::process::Command::new("mkfifo").arg(&pipe).status();
            if made.is_ok_and(|s| s.success()) {
                assert!(soon(pipe).is_err());
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_member_holds_only_so_many_files_open_here() {
        let answering = Answering::new();
        let (greedy, other) = (device(), device());
        let held: Vec<Request> = (0..MAX_REQUESTS_PER_PEER)
            .map(|_| answering.request(greedy).unwrap())
            .collect();
        assert!(answering.request(greedy).is_none());
        let theirs = answering.request(other).unwrap();
        // A few files sent at once to one peer; the next waits for one of them.
        let mut turns = Vec::new();
        for request in &held[..MAX_SENDING_PER_PEER] {
            turns.push(request.turn().await.unwrap());
        }
        let short = Duration::from_millis(50);
        assert!(tokio::time::timeout(short, held[0].turn()).await.is_err());
        assert!(theirs.turn().await.is_some());
        turns.pop();
        assert!(tokio::time::timeout(short, held[0].turn()).await.is_ok());
        // A device counts once, under whatever transport identities it connects with.
        let device = device();
        let theirs: Vec<Request> = (0..MAX_REQUESTS_PER_PEER)
            .map(|_| answering.request(device).unwrap())
            .collect();
        assert!(answering.request(device).is_none());
        drop(theirs);
        // Requests answered make room for others.
        drop(held);
        assert!(answering.request(greedy).is_some());
        assert_eq!(answering.peers().get(&greedy).map(|p| p.0), None);
    }

    /// The least rate leaves the slowest honest lines seen their turns: a satellite dish at
    /// 0.1 Mbit/s taking four files at once, each at a quarter of its line, through a stall
    /// of 45 seconds in every window, for an hour, never gives way. A device that takes its
    /// one file a block every 29 seconds (as slowly as the idle timeout allows) does once a
    /// window has gone by, and only while another device waits for a turn of the kind it
    /// holds; four such files of one device are taken at twice the least rate together, and
    /// keep their turns. A turn given up goes to the device that waits.
    #[tokio::test]
    async fn only_a_reader_under_the_least_rate_gives_way_and_only_to_another_device() {
        let second = Duration::from_secs(1);
        let answering = Answering::new();
        let (bea, carl, waiter) = (device(), device(), device());
        let bea_files: Vec<Request> = (0..MAX_SENDING_PER_PEER)
            .map(|_| answering.request(bea).unwrap())
            .collect();
        let carl_file = answering.request(carl).unwrap();
        let others: Vec<Request> = (0..MAX_SENDING - MAX_SENDING_PER_PEER - 1)
            .map(|_| answering.request(device()).unwrap())
            .collect();
        let mut turns = Vec::new();
        for request in bea_files.iter().chain([&carl_file]).chain(&others) {
            turns.push(request.turn().await.unwrap());
        }
        assert_eq!(bea_files[0].sending_now(false), MAX_SENDING_PER_PEER);
        assert_eq!(carl_file.sending_now(false), 1);
        let share = |request: &Request| answering.least / request.sending_now(false) as u64;
        let read = |request: &Request, every: Duration, bytes: u64, rounds: u64| {
            let mut reader = Reader::new(answering.window);
            for _ in 0..rounds {
                reader.took(bytes, every, share(request));
            }
            reader
        };
        let trickle = |request: &Request| {
            read(
                request,
                29 * second,
                CHUNK as u64,
                SLOW_WINDOW.as_secs() / 29 + 1,
            )
        };
        // Nobody waits yet: nothing gives way.
        assert!(!carl_file.gives_way(&trickle(&carl_file), false));
        let theirs = answering.request(waiter).unwrap();
        let short = Duration::from_millis(50);
        assert!(tokio::time::timeout(short, theirs.turn()).await.is_err());
        // The dish, a quarter of its line for each of four files, with a long stall.
        let dish = 100_000 / 8 / MAX_SENDING_PER_PEER as u64;
        let mut honest = Reader::new(answering.window);
        for _ in 0..30 {
            for _ in 0..75 {
                honest.took(dish, second, share(&bea_files[0]));
            }
            honest.took(0, 45 * second, share(&bea_files[0]));
            assert!(!bea_files[0].gives_way(&honest, false));
        }
        assert!(!bea_files[0].gives_way(&trickle(&bea_files[0]), false));
        assert!(carl_file.gives_way(&trickle(&carl_file), false));
        // Only another device's wait counts, and only for turns of the kind it waits for.
        assert!(!theirs.gives_way(&trickle(&theirs), false));
        assert!(!carl_file.gives_way(&trickle(&carl_file), true));
        // Carl's file given up: the device that waits gets its turn.
        drop(turns.remove(MAX_SENDING_PER_PEER));
        assert!(
            tokio::time::timeout(short, theirs.turn())
                .await
                .is_ok_and(|t| t.is_some())
        );
    }

    /// Bea takes a large file slowly while every other turn is taken and another device
    /// waits for one: under the least rate her file gives way, its turn goes to the device
    /// that waits, and what she got is a part she goes on from; above it, her file comes
    /// whole however long the other waits.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_taken_too_slowly_gives_its_turn_to_a_device_that_waits() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let dir = folder("slow-reader");
        let source = dir.join("film.mkv");
        // Past what the transport takes before the reader does.
        let bytes: Vec<u8> = (0..24u32 << 20).map(|i| (i % 253) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        let mut answering = Answering::new();
        answering.least = 512 * 1024;
        answering.window = Duration::from_secs(1);
        serve_answering(node_a.clone(), shared, |_, _| {}, answering.clone()).unwrap();
        // Bea takes a block every `every`, for up to a minute: how much came, counted in
        // `got` as it comes.
        let take = |every: Duration, got: Arc<std::sync::atomic::AtomicU64>| {
            let node_b = node_b.clone();
            tokio::spawn(async move {
                let mut stream = node_b
                    .streams()
                    .open_stream(peer_a, FILE_PROTOCOL)
                    .await
                    .unwrap();
                let status = request(&mut stream, group, id, 0).await.unwrap();
                assert_eq!(status, FOUND);
                let mut size = [0u8; 8];
                stream.read_exact(&mut size).await.unwrap();
                let mut buf = vec![0u8; CHUNK];
                let _ = tokio::time::timeout(Duration::from_secs(60), async {
                    loop {
                        match stream.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                got.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        tokio::time::sleep(every).await;
                    }
                })
                .await;
            })
        };
        let got = |count: &Arc<std::sync::atomic::AtomicU64>| {
            count.load(std::sync::atomic::Ordering::Relaxed)
        };
        // Every turn of everyone's but one is taken.
        let others: Vec<Request> = (0..MAX_SENDING - 1)
            .map(|_| answering.request(device()).unwrap())
            .collect();
        let mut held = Vec::new();
        for other in &others {
            held.push(other.turn().await.unwrap());
        }
        // Taken at a quarter of the least rate, while another device waits.
        let slow_got: Arc<std::sync::atomic::AtomicU64> = Arc::default();
        let slow = take(Duration::from_millis(500), Arc::clone(&slow_got));
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(answering.sending.available_permits(), 0);
        let waiter = answering.request(device()).unwrap();
        let turn = tokio::time::timeout(Duration::from_secs(25), waiter.turn()).await;
        assert!(
            turn.as_ref().is_ok_and(|t| t.is_some()),
            "the slow file kept its turn"
        );
        slow.abort();
        let slow_got = got(&slow_got);
        assert!(
            0 < slow_got && slow_got < size,
            "{slow_got} of {size} bytes"
        );
        drop(turn);
        // Taken at five times the least rate, while another device waits: it comes whole.
        let honest_got: Arc<std::sync::atomic::AtomicU64> = Arc::default();
        let honest = take(Duration::from_millis(25), Arc::clone(&honest_got));
        tokio::time::sleep(Duration::from_secs(1)).await;
        let waiter = answering.request(device()).unwrap();
        let waiting = tokio::spawn(async move { waiter.turn().await.is_some() });
        honest.await.unwrap();
        assert_eq!(got(&honest_got), size, "an honest reader was cut");
        assert!(
            waiting.await.unwrap(),
            "the device that waited got no turn after"
        );
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A device that asks while every turn stays taken for as long as a request waits is
    /// told to ask again later, as one sent all of its day is: the file is there, the
    /// server busy, and its download waits rather than fails.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_device_asking_while_every_turn_is_taken_is_told_to_ask_later() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let dir = folder("busy-later");
        let source = dir.join("photo.jpg");
        std::fs::write(&source, vec![7u8; 100_000]).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        let answering = Answering::new();
        serve_answering(node_a.clone(), shared, |_, _| {}, answering.clone()).unwrap();
        let others: Vec<Request> = (0..MAX_SENDING)
            .map(|_| answering.request(device()).unwrap())
            .collect();
        let mut held = Vec::new();
        for other in &others {
            held.push(other.turn().await.unwrap());
        }
        let asked = fetch_to_memory(&node_b, group, peer_a, id, size).await;
        assert!(
            matches!(asked, Err(FileError::Later(wait)) if wait > BUSY_LATER && wait < 2 * BUSY_LATER),
            "{asked:?}"
        );
        drop(held);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A download that stops after some of the file came (here: the file withdrawn while it
    /// comes at the pace) says so, and keeps what came: asked again once the file is there
    /// again, it goes on from there to the whole file.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_cut_short_goes_on_from_what_came() {
        let (node_a, node_b, peer_a, log, _ana, bea) = two_members().await;
        let group = log.group_id();
        let (shared, answering, id, dir) =
            shared_again("cut-short", group, bea.public_key(), Duration::from_secs(1));
        let source = dir.join("film.mkv");
        let size = std::fs::metadata(&source).unwrap().len();
        serve_answering(node_a.clone(), shared.clone(), |_, _| {}, answering).unwrap();
        let dest = dir.join("film-here.mkv");
        let fetching = {
            let (node_b, dest) = (node_b.clone(), dest.clone());
            tokio::spawn(
                async move { fetch(&node_b, group, peer_a, id, size, &dest, |_| {}).await },
            )
        };
        // A second at the pace, and half a second of it: about half the file came.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        shared.remove(group, id);
        let first = fetching.await.unwrap();
        assert!(matches!(first, Err(FileError::Cut)), "{first:?}");
        let kept = std::fs::metadata(part_path(&dest, &id)).unwrap().len();
        assert!(
            kept >= CHUNK as u64 && kept < size,
            "{kept} of {size} bytes kept"
        );
        shared.insert(group, id, source.clone());
        let mut from = None;
        fetch(&node_b, group, peer_a, id, size, &dest, |got| {
            from.get_or_insert(got);
        })
        .await
        .unwrap();
        assert!(
            from.is_some_and(|from| from > kept),
            "started again from {from:?}"
        );
        assert_eq!(
            std::fs::read(&dest).unwrap(),
            std::fs::read(&source).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A member that asks again and again for files it already got, past its burst, is sent
    /// them at the pace, each holding one of the few turns kept for that: it never holds
    /// the turns of everyone, and a member asking for a file the first time is answered at
    /// once, all the other turns taken.
    #[tokio::test(flavor = "multi_thread")]
    async fn files_sent_again_at_the_pace_leave_the_turns_to_others() {
        use crate::{GroupSetup, NodeConfig, NodeEvent};
        let ana = Arc::new(fogata_crypto::DeviceIdentity::generate().unwrap());
        let bea = Arc::new(fogata_crypto::DeviceIdentity::generate().unwrap());
        let mut log = fogata_state::GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let group = log.group_id();
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let address: crate::Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        let config = |identity, listen_port, peers| NodeConfig {
            identity,
            groups: vec![GroupSetup::Member(log.clone())],
            friends: None,
            listen_port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers,
            remembered: Vec::new(),
            hidden: false,
        };
        let (node_a, mut events_a) = crate::start(config(ana.clone(), port, Vec::new())).unwrap();
        let (node_b, _events_b) =
            crate::start(config(bea.clone(), 0, vec![address.clone()])).unwrap();
        let connected = tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                match tokio::time::timeout(Duration::from_secs(5), events_a.recv()).await {
                    Ok(Some(NodeEvent::MemberConnected { .. })) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => panic!("the node stopped"),
                    Err(_) => node_b.dial(vec![address.clone()], None).await.unwrap(),
                }
            }
            loop {
                if let Ok(Some(peer)) = node_b.member_peer(ana.public_key()).await {
                    return peer;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        let peer_a = connected.expect("the two never connected");

        let dir = folder("paced");
        let source = dir.join("film.mkv");
        let bytes: Vec<u8> = (0..1u32 << 20).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        // Bea had the file today, and her pace is used up for a while to come.
        let answering = Answering::new();
        let asker = bea.public_key();
        let now = tokio::time::Instant::now();
        answering.seen.again(asker, (group, id), 0, size, now);
        crate::locked(&answering.pace.0).insert(asker, (0, now + Duration::from_secs(3)));
        serve_answering(node_a.clone(), shared, |_, _| {}, answering.clone()).unwrap();
        // Others hold every turn but Bea's own.
        let others: Vec<Request> = (0..(MAX_SENDING / MAX_SENDING_PER_PEER - 1))
            .flat_map(|_| {
                let other = device();
                (0..MAX_SENDING_PER_PEER).map(move |_| other)
            })
            .map(|other| answering.request(other).unwrap())
            .collect();
        let mut held = Vec::new();
        for request in &others {
            held.push(request.turn().await.unwrap());
        }
        // Bea asks for it again, four times at once.
        let fetches: Vec<_> = (0..MAX_SENDING_PER_PEER)
            .map(|_| {
                let node_b = node_b.clone();
                tokio::spawn(async move { fetch_to_memory(&node_b, group, peer_a, id, size).await })
            })
            .collect();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(
            answering.paced.available_permits(),
            MAX_PACED - MAX_PACED_PER_PEER,
            "Bea's files are not sent at the pace"
        );
        let newcomer = answering.request(device()).unwrap();
        let turn = tokio::time::timeout(Duration::from_secs(1), newcomer.turn()).await;
        assert!(
            turn.is_ok_and(|t| t.is_some()),
            "files sent again hold the turns of everyone"
        );
        for fetch in fetches {
            fetch.abort();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two members of one group, Ana and Bea, each with a node on the loopback interface,
    /// connected: Ana's node, Bea's, the peer Bea sees Ana as, the group and its log.
    async fn two_members() -> (
        NodeHandle,
        NodeHandle,
        PeerId,
        fogata_state::GroupLog,
        Arc<fogata_crypto::DeviceIdentity>,
        Arc<fogata_crypto::DeviceIdentity>,
    ) {
        use crate::{GroupSetup, NodeConfig, NodeEvent};
        let ana = Arc::new(fogata_crypto::DeviceIdentity::generate().unwrap());
        let bea = Arc::new(fogata_crypto::DeviceIdentity::generate().unwrap());
        let mut log = fogata_state::GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        // Another group of both, which keeps them connected when one leaves the first.
        let mut other = fogata_state::GroupLog::create(&ana, "o", 1).unwrap();
        other.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let address: crate::Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        let config = |identity, listen_port, peers| NodeConfig {
            identity,
            groups: vec![
                GroupSetup::Member(log.clone()),
                GroupSetup::Member(other.clone()),
            ],
            friends: None,
            listen_port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers,
            remembered: Vec::new(),
            hidden: false,
        };
        let (node_a, mut events_a) = crate::start(config(ana.clone(), port, Vec::new())).unwrap();
        let (node_b, _events_b) =
            crate::start(config(bea.clone(), 0, vec![address.clone()])).unwrap();
        let connected = tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                match tokio::time::timeout(Duration::from_secs(5), events_a.recv()).await {
                    Ok(Some(NodeEvent::MemberConnected { .. })) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => panic!("the node stopped"),
                    Err(_) => node_b.dial(vec![address.clone()], None).await.unwrap(),
                }
            }
            loop {
                if let Ok(Some(peer)) = node_b.member_peer(ana.public_key()).await {
                    return peer;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        let peer_a = connected.expect("the two never connected");
        // Ana's events are not needed any more; her node keeps running without them.
        tokio::spawn(async move { while events_a.recv().await.is_some() {} });
        (node_a, node_b, peer_a, log, ana, bea)
    }

    /// What a member's own program gets asking `peer` for file `id` of `group` from
    /// `offset`, read as it comes and never checked, for up to `limit`: `None` when the
    /// file is not there, else the size said and the bytes that came.
    async fn ask_raw(
        node: &NodeHandle,
        group: GroupId,
        peer: PeerId,
        id: [u8; 32],
        offset: u64,
        limit: Duration,
    ) -> Option<(u64, Vec<u8>)> {
        let mut stream = node
            .streams()
            .open_stream(peer, FILE_PROTOCOL)
            .await
            .unwrap();
        stream.write_all(group.as_bytes()).await.unwrap();
        stream.write_all(&id).await.unwrap();
        stream.write_all(&offset.to_be_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        let mut status = [0u8; 1];
        stream.read_exact(&mut status).await.ok()?;
        if status[0] != FOUND {
            return None;
        }
        let mut size = [0u8; 8];
        stream.read_exact(&mut size).await.unwrap();
        let mut got = Vec::new();
        let mut buf = vec![0u8; CHUNK];
        let _ = tokio::time::timeout(limit, async {
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
        })
        .await;
        Some((u64::from_be_bytes(size), got))
    }

    /// A file shared from Ana, served with `answering`, that Bea had today and whose pace
    /// is used up for `wait` to come: the first bytes she asks for again wait that long.
    fn shared_again(
        name: &str,
        group: GroupId,
        bea: DevicePublicKey,
        wait: Duration,
    ) -> (SharedFiles, Answering, [u8; 32], PathBuf) {
        let dir = folder(name);
        let source = dir.join("film.mkv");
        let bytes: Vec<u8> = (0..1u32 << 20).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        let answering = Answering::new();
        let asker = bea;
        let now = tokio::time::Instant::now();
        answering.seen.again(asker, (group, id), 0, size, now);
        crate::locked(&answering.pace.0).insert(asker, (0, now + wait));
        (shared, answering, id, dir)
    }

    /// A file withdrawn while its next block waits for the pace: that block never goes,
    /// nor any after it, however long the wait was.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_block_waiting_for_the_pace_is_not_sent_once_the_file_is_withdrawn() {
        let (node_a, node_b, peer_a, log, _ana, bea) = two_members().await;
        let group = log.group_id();
        let (shared, answering, id, dir) =
            shared_again("withdrawn", group, bea.public_key(), Duration::from_secs(3));
        serve_answering(node_a.clone(), shared.clone(), |_, _| {}, answering).unwrap();
        let asking = {
            let node_b = node_b.clone();
            tokio::spawn(async move {
                ask_raw(&node_b, group, peer_a, id, 0, Duration::from_secs(8)).await
            })
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
        shared.remove(group, id);
        let (size, got) = asking.await.unwrap().expect("it was there when asked");
        assert_eq!(size, 1 << 20);
        assert!(
            got.is_empty(),
            "{} bytes went after the file was withdrawn",
            got.len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same when the asker stops being a member while it waits.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_block_waiting_for_the_pace_is_not_sent_to_one_no_longer_a_member() {
        let (node_a, node_b, peer_a, mut log, ana, bea) = two_members().await;
        let group = log.group_id();
        let (shared, answering, id, dir) =
            shared_again("removed", group, bea.public_key(), Duration::from_secs(3));
        serve_answering(node_a.clone(), shared.clone(), |_, _| {}, answering).unwrap();
        let asking = {
            let node_b = node_b.clone();
            tokio::spawn(async move {
                ask_raw(&node_b, group, peer_a, id, 0, Duration::from_secs(8)).await
            })
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
        log.remove(&ana, bea.public_key(), 3).unwrap();
        node_a.merge_group(log).await.unwrap();
        // Still connected, through the other group.
        assert!(node_a.is_member(&node_b.local_peer()));
        let (_, got) = asking.await.unwrap().expect("it was there when asked");
        assert!(
            got.is_empty(),
            "{} bytes went to a device no longer a member",
            got.len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A device is sent at most so much in a day: a file that starts within it goes whole,
    /// even past it; after it the device is told when its next day begins, and is sent the
    /// file again then.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_device_is_sent_its_day_and_then_told_when_to_come_back() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let dir = folder("day");
        let source = dir.join("film.mkv");
        let bytes: Vec<u8> = (0..1u32 << 20).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        let day = Duration::from_secs(6);
        shared.limit_day(3 << 19, day);
        serve(node_a.clone(), shared.clone(), |_, _| {}).unwrap();
        let started = tokio::time::Instant::now();
        // The first, and the second, which starts within the day and goes whole past it.
        for _ in 0..2 {
            let got = fetch_to_memory(&node_b, group, peer_a, id, size)
                .await
                .unwrap();
            assert_eq!(got, bytes);
        }
        let later = match fetch_to_memory(&node_b, group, peer_a, id, size).await {
            Err(FileError::Later(later)) => later,
            other => panic!("sent past its day: {:?}", other.map(|b| b.len())),
        };
        assert!(later <= day && later + started.elapsed() >= day - Duration::from_secs(1));
        // Nothing of it went meanwhile.
        let raw = ask_raw(&node_b, group, peer_a, id, 0, Duration::from_secs(1)).await;
        assert_eq!(raw, None);
        tokio::time::sleep(later).await;
        let got = fetch_to_memory(&node_b, group, peer_a, id, size)
            .await
            .unwrap();
        assert_eq!(got, bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Files that start at once are counted against the day as they start: one goes whole,
    /// and those after it are told to come back, rather than all passing the day's end.
    #[tokio::test(flavor = "multi_thread")]
    async fn files_started_at_once_count_against_the_day_together() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let dir = folder("day-at-once");
        let source = dir.join("film.mkv");
        let bytes: Vec<u8> = (0..1u32 << 20).map(|i| (i % 249) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        shared.limit_day(1 << 19, Duration::from_secs(600));
        serve(node_a.clone(), shared.clone(), |_, _| {}).unwrap();
        let fetches = (0..MAX_SENDING_PER_PEER)
            .map(|_| fetch_to_memory(&node_b, group, peer_a, id, size))
            .collect::<Vec<_>>();
        let outcomes = futures::future::join_all(fetches).await;
        let whole = outcomes.iter().filter(|o| o.is_ok()).count();
        let later = outcomes
            .iter()
            .filter(|o| matches!(o, Err(FileError::Later(_))))
            .count();
        assert_eq!(
            (whole, later),
            (1, MAX_SENDING_PER_PEER - 1),
            "whole, later"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// As above, with Ana's loop taking requests busy elsewhere while they come in, as on a
    /// loaded computer: every request reaches it and is answered, none is dropped unread.
    /// Ana's loop starts only after an asker has asked again for the last time, so this
    /// holds without asking again.
    #[tokio::test(flavor = "multi_thread")]
    async fn files_asked_at_once_of_a_busy_server_are_all_answered() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let dir = folder("day-at-once-busy");
        let source = dir.join("film.mkv");
        let bytes: Vec<u8> = (0..1u32 << 20).map(|i| (i % 249) as u8).collect();
        std::fs::write(&source, &bytes).unwrap();
        let (id, size) = hash_file(&source).unwrap();
        let shared = SharedFiles::default();
        shared.insert(group, id, source);
        shared.limit_day(1 << 19, Duration::from_secs(600));
        let incoming = node_a.streams().accept(FILE_PROTOCOL).unwrap();
        let fetches = (0..MAX_SENDING_PER_PEER)
            .map(|_| fetch_to_memory(&node_b, group, peer_a, id, size))
            .collect::<Vec<_>>();
        let late = ASKED_AGAIN_AFTER.iter().sum::<Duration>() + Duration::from_millis(500);
        let serving = async {
            tokio::time::sleep(late).await;
            serve_from(
                node_a.clone(),
                incoming,
                shared.clone(),
                |_, _| {},
                Answering::new(),
            );
        };
        let (outcomes, ()) = tokio::join!(futures::future::join_all(fetches), serving);
        let whole = outcomes.iter().filter(|o| o.is_ok()).count();
        let later = outcomes
            .iter()
            .filter(|o| matches!(o, Err(FileError::Later(_))))
            .count();
        assert_eq!(
            (whole, later),
            (1, MAX_SENDING_PER_PEER - 1),
            "whole, later; failed: {:?}",
            outcomes
                .iter()
                .filter_map(|o| o.as_ref().err())
                .collect::<Vec<_>>()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A request whose stream the other device drops unread, as copies before 0.1.0-beta.9
    /// do with all but one of a burst, is made again: the answer that then comes counts.
    /// One dropped every time is given up after [`ASKED_AGAIN_AFTER`], with the error.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_request_dropped_unanswered_is_made_again() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let mut incoming = node_a.streams().accept(FILE_PROTOCOL).unwrap();
        // Ana drops the first two requests unread and answers the third (not here); every
        // other is dropped.
        let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (counted, taker) = (seen.clone(), node_a.background());
        node_a.spawn(async move {
            while let Some((_, mut stream)) = taker.next_stream(&mut incoming).await {
                let n = counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n == 2 {
                    let mut request = [0u8; 56];
                    stream.read_exact(&mut request).await.unwrap();
                    stream.write_all(&[NOT_HERE]).await.unwrap();
                    stream.close().await.unwrap();
                }
            }
        });
        let asked = fetch_to_memory(&node_b, group, peer_a, [7; 32], 10).await;
        assert!(matches!(asked, Err(FileError::NotThere)), "{asked:?}");
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 3);

        // Counted from 4 on, every request is dropped.
        seen.store(4, std::sync::atomic::Ordering::SeqCst);
        let asked = fetch_to_memory(&node_b, group, peer_a, [7; 32], 10).await;
        assert!(
            matches!(&asked, Err(FileError::Io(e)) if dropped_unanswered(e)),
            "{asked:?}"
        );
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            4 + 1 + ASKED_AGAIN_AFTER.len()
        );
    }

    /// A file fetched into memory (a room's preview) takes no more than the size it was asked
    /// for, which is what the previews' memory is charged: a holder that says another size is
    /// refused, and bytes it sends past the size are left unread.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_fetched_into_memory_takes_only_its_size() {
        let (node_a, node_b, peer_a, log, _ana, _bea) = two_members().await;
        let group = log.group_id();
        let mut incoming = node_a.streams().accept(FILE_PROTOCOL).unwrap();
        let file = b"0123456789".to_vec();
        let id = *blake3::hash(&file).as_bytes();
        // Ana says the file is 10 bytes, then 11; either way it sends a megabyte more.
        let taker = node_a.background();
        let sent = file.clone();
        node_a.spawn(async move {
            let mut says = 10u64;
            while let Some((_, mut stream)) = taker.next_stream(&mut incoming).await {
                let mut request = [0u8; 56];
                stream.read_exact(&mut request).await.unwrap();
                stream.write_all(&[FOUND]).await.unwrap();
                stream.write_all(&says.to_be_bytes()).await.unwrap();
                stream.write_all(&sent).await.unwrap();
                let _ = stream.write_all(&vec![7u8; 1 << 20]).await;
                let _ = stream.close().await;
                says += 1;
            }
        });
        let got = fetch_to_memory(&node_b, group, peer_a, id, 10)
            .await
            .unwrap();
        assert_eq!(got, file);
        assert!(got.capacity() < 1024, "{}", got.capacity());
        let other = fetch_to_memory(&node_b, group, peer_a, id, 10).await;
        assert!(matches!(other, Err(FileError::Damaged)), "{other:?}");
    }

    /// A path made a pipe between the check and the open is refused at once, rather than
    /// waiting at the open for a writer that never comes.
    #[cfg(unix)]
    #[test]
    fn a_pipe_in_a_files_place_is_refused_without_waiting() {
        let dir = folder("pipe");
        let kept = dir.join("kept");
        std::fs::create_dir_all(&kept).unwrap();
        let pipe = dir.join("notes.txt");
        let made = std::process::Command::new("mkfifo")
            .arg(&pipe)
            .status()
            .unwrap();
        assert!(made.success());
        let copies = Copies::open(kept, None).unwrap();
        // As the open sees it: the check before it saw a file, the open meets the pipe.
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, O_NONBLOCK);
        let (done, waited) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let opened = options.open(&pipe).map(|f| f.metadata().unwrap().is_file());
            let _ = done.send((opened.ok(), copies.copy(&pipe, u64::MAX).is_err()));
        });
        let (opened, refused) = waited
            .recv_timeout(Duration::from_secs(5))
            .expect("the open waited for a writer");
        assert_eq!(opened, Some(false));
        assert!(refused);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A copy is made of the file as it is, hashed as it is copied, and read back as it
    /// was whatever becomes of the file; sealed with the key when there is one. The same
    /// bytes are kept once; a copy goes when dropped, and one larger than allowed is not
    /// made. What an earlier run left goes.
    #[test]
    fn copies_hold_the_file_as_it_was() {
        let dir = folder("copies");
        let kept = dir.join("kept");
        std::fs::create_dir_all(&kept).unwrap();
        std::fs::write(kept.join("left.copy"), b"old").unwrap();
        for key in [None, Some(DataKey::generate().unwrap())] {
            let copies = Copies::open(kept.clone(), key.clone()).unwrap();
            assert_eq!(std::fs::read_dir(&kept).unwrap().count(), 0);
            let from = dir.join("notes.txt");
            std::fs::write(&from, b"PUBLIC-V1").unwrap();
            let (copy, id, size) = copies.copy(&from, u64::MAX).unwrap();
            assert_eq!((id, size), (*blake3::hash(b"PUBLIC-V1").as_bytes(), 9));
            std::fs::write(&from, b"SECRET-V2").unwrap();
            let mut read = Vec::new();
            copy.open().unwrap().0.read_to_end(&mut read).unwrap();
            assert_eq!(read, b"PUBLIC-V1");
            let on_disk = std::fs::read(&copy.path).unwrap();
            assert_eq!(key.is_some(), on_disk != b"PUBLIC-V1");
            // The same bytes again: kept once.
            let same = dir.join("same.txt");
            std::fs::write(&same, b"PUBLIC-V1").unwrap();
            let (again, _, _) = copies.copy(&same, u64::MAX).unwrap();
            assert!(Arc::ptr_eq(&copy, &again));
            assert_eq!(std::fs::read_dir(&kept).unwrap().count(), 1);
            drop((copy, again));
            assert_eq!(std::fs::read_dir(&kept).unwrap().count(), 0);
            // Too large for the room allowed: not made, nothing left.
            let refused = copies.copy(&from, 8).unwrap_err();
            assert_eq!(refused.kind(), io::ErrorKind::FileTooLarge);
            assert!(copies.copy(&dir, u64::MAX).is_err());
            assert_eq!(std::fs::read_dir(&kept).unwrap().count(), 0);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A copy being made stops at once when the device stops, and leaves nothing; none is
    /// begun after. (A large one went on for long, holding the data folder.)
    #[test]
    fn a_copy_stops_when_the_device_stops() {
        let dir = folder("copy-stop");
        let from = dir.join("film.mkv");
        // Takes no room on the disk: it is read as zeros.
        std::fs::File::create(&from)
            .unwrap()
            .set_len(512 << 20)
            .unwrap();
        let copies = Arc::new(Copies::open(dir.join("kept"), None).unwrap());
        let (done, finished) = std::sync::mpsc::channel();
        {
            let (copies, from) = (copies.clone(), from.clone());
            std::thread::spawn(move || {
                let _ = done.send(copies.copy(&from, u64::MAX).map(|_| ()));
            });
        }
        std::thread::sleep(Duration::from_millis(200));
        copies.stop();
        let copied = finished
            .recv_timeout(Duration::from_secs(3))
            .expect("the copy went on after the device stopped");
        assert!(copied.is_err());
        assert_eq!(std::fs::read_dir(copies.dir()).unwrap().count(), 0);
        assert!(copies.copy(&from, u64::MAX).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What an earlier run left and cannot be removed does not keep the copies from being
    /// made: it is left where it is.
    #[cfg(unix)]
    #[test]
    fn what_an_earlier_run_left_and_cannot_go_is_left() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = folder("copy-stuck");
        let kept = dir.join("kept");
        let stuck = kept.join("stuck");
        std::fs::create_dir_all(&stuck).unwrap();
        std::fs::write(stuck.join("a.copy"), b"old").unwrap();
        std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o500)).unwrap();
        let opened = Copies::open(kept.clone(), None);
        let from = dir.join("notes.txt");
        std::fs::write(&from, b"PUBLIC-V1").unwrap();
        let copied = opened
            .as_ref()
            .map(|copies| copies.copy(&from, u64::MAX).map(|(_, _, size)| size));
        std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(copied.unwrap().unwrap(), 9);
        assert!(stuck.join("a.copy").exists());
        drop(opened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file that grows while it is copied, past the room held for it, is refused as one
    /// that grew, not for want of room on the disk; nothing is left.
    #[test]
    fn a_file_growing_while_copied_is_refused_as_such() {
        let dir = folder("copy-grow");
        let from = dir.join("recording.mkv");
        std::fs::write(&from, vec![7u8; 4 << 20]).unwrap();
        let size = std::fs::metadata(&from).unwrap().len();
        let copies = Copies::open(dir.join("kept"), None).unwrap();
        let writing = Arc::new(AtomicBool::new(true));
        let writer = {
            let (writing, from) = (writing.clone(), from.clone());
            std::thread::spawn(move || {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&from)
                    .unwrap();
                let chunk = vec![9u8; 64 * 1024];
                // At most 64 MiB more.
                for _ in 0..1024 {
                    if !writing.load(Ordering::Relaxed) {
                        break;
                    }
                    file.write_all(&chunk).unwrap();
                }
            })
        };
        let refused = copies.copy(&from, size + 128 * 1024).unwrap_err();
        writing.store(false, Ordering::Relaxed);
        writer.join().unwrap();
        assert_eq!(refused.kind(), io::ErrorKind::FileTooLarge, "{refused}");
        assert_eq!(std::fs::read_dir(copies.dir()).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A share confirmed once its room took it is served unless something stopped the file,
    /// or its group's files, from being served since its ticket was taken; one that failed
    /// leaves a share of the same file that went as it is.
    #[test]
    fn a_share_is_served_only_if_nothing_stopped_it_meanwhile() {
        let dir = folder("tickets");
        let copies = Copies::open(dir.join("kept"), None).unwrap();
        let from = dir.join("notes.txt");
        std::fs::write(&from, b"PUBLIC-V1").unwrap();
        let (copy, id, _) = copies.copy(&from, u64::MAX).unwrap();
        let (group, other) = (GroupId::random().unwrap(), GroupId::random().unwrap());
        let shared = SharedFiles::default();
        // Stopped, or its group's files stopped, meanwhile.
        let ticket = shared.ticket(group, id);
        shared.remove(group, id);
        assert!(!shared.insert_copy(ticket, copy.clone(), from.clone()));
        let ticket = shared.ticket(group, id);
        shared.clear_group(group);
        assert!(!shared.insert_copy(ticket, copy.clone(), from.clone()));
        assert!(!shared.contains(group, id));
        // Another group's left alone.
        let ticket = shared.ticket(group, id);
        shared.clear_group(other);
        shared.remove(other, id);
        assert!(shared.insert_copy(ticket, copy.clone(), from.clone()));
        assert!(shared.contains(group, id));
        // Two at once: one went; the other never does anything to it.
        let (one, two) = (shared.ticket(group, id), shared.ticket(group, id));
        assert!(shared.insert_copy(one, copy.clone(), from.clone()));
        let _never_confirmed = two;
        assert!(shared.contains(group, id));
        assert_eq!(shared.get(group, &id), Some(from));
        // Removed: transfers under way hear of it.
        let (_, gone) = shared.entry(group, &id).unwrap();
        assert!(!is_gone(&gone));
        shared.remove(group, id);
        assert!(is_gone(&gone));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_finished_download_never_replaces_a_file() {
        let dir = folder("place-new");
        let dest = dir.join("plan.pdf");
        std::fs::write(&dest, "mine").unwrap();
        std::fs::write(dir.join("plan (2).pdf"), "mine too").unwrap();
        let part = dir.join("plan.pdf.0a0b0c0d.part");
        std::fs::write(&part, "theirs").unwrap();
        let placed = place_new(&part, &dest).unwrap();
        assert_eq!(placed, dir.join("plan (3).pdf"));
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "mine");
        assert_eq!(std::fs::read_to_string(&placed).unwrap(), "theirs");
        assert!(!part.exists());
        // A name without an extension.
        assert_eq!(numbered(&dir.join("notes"), 2), dir.join("notes (2)"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_finished_download_never_goes_through_a_link() {
        let dir = folder("place-link");
        let elsewhere = dir.join("elsewhere");
        std::fs::write(&elsewhere, "keep").unwrap();
        let dest = dir.join("photo.png");
        std::os::unix::fs::symlink(&elsewhere, &dest).unwrap();
        let part = dir.join("photo.png.0a0b0c0d.part");
        std::fs::write(&part, "theirs").unwrap();
        let placed = place_new(&part, &dest).unwrap();
        assert_eq!(placed, dir.join("photo (2).png"));
        assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "keep");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_partial_download_is_never_a_link() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = folder("part-link");
        let secret = dir.join("secret");
        std::fs::write(&secret, "do not touch").unwrap();
        let part = dir.join("plan.pdf.0a0b0c0d.part");
        std::os::unix::fs::symlink(&secret, &part).unwrap();
        // The link is removed, not read nor written through.
        assert!(open_earlier_part(&part).await.unwrap().is_none());
        assert!(std::fs::symlink_metadata(&part).is_err());
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "do not touch");
        // A new one is the owner's only, and is never made through a link.
        let file = create_part(&part).await.unwrap();
        let mode = file.metadata().await.unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        drop(file);
        std::fs::remove_file(&part).unwrap();
        std::os::unix::fs::symlink(&secret, &part).unwrap();
        assert!(create_part(&part).await.is_err());
        // A folder there fails the download.
        std::fs::remove_file(&part).unwrap();
        std::fs::create_dir(&part).unwrap();
        assert!(open_earlier_part(&part).await.is_err());
        // A plain file is taken, to add to.
        std::fs::remove_dir(&part).unwrap();
        std::fs::write(&part, "half").unwrap();
        assert!(open_earlier_part(&part).await.unwrap().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A partial download replaced between the look at it and its opening fails the
    /// download, rather than adding to whatever was put there: here a hard link to another
    /// file that, on Windows, also has the part's creation time (as NTFS gives a file made
    /// where one was just removed). The same for a sealed one.
    #[tokio::test]
    async fn a_partial_download_replaced_while_opened_is_refused() {
        let dir = folder("part-swapped");
        let key = DataKey::generate().unwrap();
        for sealed in [false, true] {
            let part = dir.join(format!("plan.pdf.0a0b0c0d.{sealed}.part"));
            if sealed {
                let mut w =
                    SealWriter::new(&key, "file x", std::fs::File::create(&part).unwrap()).unwrap();
                w.write_all(&vec![5u8; CHUNK + 10]).unwrap();
            } else {
                std::fs::write(&part, "half").unwrap();
            }
            let secret = dir.join(format!("secret.{sealed}"));
            std::fs::write(&secret, "do not touch").unwrap();
            same_creation_time(&part, &secret);
            let link = dir.join(format!("link.{sealed}"));
            std::fs::hard_link(&secret, &link).unwrap();
            crate::locked(&SWAPPED_PARTS).push((part.clone(), link));
            let failed = if sealed {
                let writing = Writing::take(&part).unwrap();
                resume_sealed_part(&part, &key, "file x", writing, &mut |_| {})
                    .await
                    .map(|_| ())
            } else {
                open_earlier_part(&part).await.map(|_| ())
            };
            assert_eq!(
                failed.unwrap_err().to_string(),
                "a partial download changed while it was opened",
                "sealed: {sealed}"
            );
            assert_eq!(std::fs::read_to_string(&secret).unwrap(), "do not touch");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Gives `to` the creation time of `from` (Windows only: elsewhere it is not used).
    fn same_creation_time(from: &Path, to: &Path) {
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileTimesExt as _;
            let created = std::fs::metadata(from).unwrap().created().unwrap();
            let file = std::fs::OpenOptions::new().write(true).open(to).unwrap();
            file.set_times(std::fs::FileTimes::new().set_created(created))
                .unwrap();
            drop(file);
            assert_eq!(std::fs::metadata(to).unwrap().created().unwrap(), created);
        }
        #[cfg(not(windows))]
        let _ = (from, to);
    }

    /// The same for a sealed one, which is taken up from its complete chunks.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_sealed_partial_download_is_never_a_link() {
        let dir = folder("part-link-sealed");
        let key = DataKey::generate().unwrap();
        let secret = dir.join("secret");
        std::fs::write(&secret, "do not touch").unwrap();
        let part = dir.join("0a0b.mp4.0a0b0c0d.part");
        std::os::unix::fs::symlink(&secret, &part).unwrap();
        let (writer, _) = resume(&part, &key, "file x").await.unwrap();
        assert!(writer.is_none());
        assert!(std::fs::symlink_metadata(&part).is_err());
        assert_eq!(std::fs::read_to_string(&secret).unwrap(), "do not touch");
        std::fs::create_dir(&part).unwrap();
        assert!(resume(&part, &key, "file x").await.is_err());
        std::fs::remove_dir(&part).unwrap();
        // Two complete chunks and part of a third: the two are kept, the rest cut off.
        let mut w = SealWriter::new(&key, "file x", std::fs::File::create(&part).unwrap()).unwrap();
        w.write_all(&vec![5u8; 2 * CHUNK + 10]).unwrap();
        drop(w);
        let (writer, earlier) = resume(&part, &key, "file x").await.unwrap();
        assert_eq!(writer.unwrap().written(), 2 * CHUNK as u64);
        assert_eq!(earlier.finalize(), blake3::hash(&vec![5u8; 2 * CHUNK]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A download given up while its work away from the async threads still runs (taking
    /// up a large sealed part, which writes it anew and puts it in its place) holds the
    /// `.part` until that work ends: another download of it fails cleanly meanwhile, and the
    /// part removed for the one given up stays removed.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_given_up_holds_its_part_until_its_work_ends() {
        use std::io::Write as _;
        let dir = folder("given-up");
        let key = DataKey::generate().unwrap();
        let dest = dir.join("film.mkv");
        let id = [7u8; 32];
        let part = part_path(&dest, &id);
        // A sealed part as a download cut short leaves it, large enough to take a while.
        let file = std::fs::File::create(&part).unwrap();
        let mut w = SealWriter::new(&key, &kept_context(&id), file).unwrap();
        let chunk = vec![3u8; 1 << 20];
        for _ in 0..8 {
            w.write_all(&chunk).unwrap();
        }
        drop(w);
        let (node, _events) = crate::start(crate::NodeConfig {
            identity: Arc::new(fogata_crypto::DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let group = GroupId::random().unwrap();
        let fetch = |node: NodeHandle| {
            let (dest, key) = (dest.clone(), key.clone());
            async move {
                let size = 1 << 30;
                fetch_sealed(
                    &node,
                    group,
                    PeerId::random(),
                    id,
                    size,
                    &dest,
                    &key,
                    |_| {},
                )
                .await
            }
        };
        let given_up = tokio::spawn(fetch(node.clone()));
        tokio::time::sleep(Duration::from_millis(20)).await;
        given_up.abort();
        assert!(given_up.await.is_err_and(|e| e.is_cancelled()));
        assert!(
            crate::locked(&WRITING).contains(&part),
            "the part was let go while its work went on"
        );
        let again = fetch(node.clone()).await;
        assert!(again.is_err_and(|e| e.to_string().contains("being downloaded here already")));
        remove_part(&part).await.unwrap();
        // Nothing of the work given up comes back afterwards.
        tokio::time::sleep(Duration::from_secs(1)).await;
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(left.is_empty(), "{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_read_that_never_comes_times_out() {
        let never = std::future::pending::<io::Result<usize>>();
        let error = within(Duration::from_millis(20), never).await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        let ready = async { Ok(7) };
        assert_eq!(within(Duration::from_millis(20), ready).await.unwrap(), 7);
    }
}
