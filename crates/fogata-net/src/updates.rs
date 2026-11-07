//! Releases of Fogata passed from friend to friend (see the `fogata-update` crate), over a
//! stream protocol of their own. Only friends recognised on their connection and verified
//! members of a shared group are answered; anyone else's stream is closed unanswered, and
//! nothing about releases goes to the DHT. So only friends and members learn which build a
//! device has.
//!
//! One request per stream:
//! - `ASK` with the build the asker has and its platform: the answer is the newest release
//!   held here, if it is newer (its manifest's bytes, key and signature, and the platforms
//!   whose packages are complete here), or nothing.
//! - `FETCH` a package of a build from a chunk on: the chunks follow one by one, each with
//!   its length, to the end. The asker checks each chunk against the manifest's hash for it
//!   before keeping it, so a damaged one is caught at once, and an interrupted download goes
//!   on from its last whole chunk.
//! - `HAVE` a build: a device that just checked a release tells the friends and members it
//!   is connected to, which then ask it.
//!
//! Uploads are bounded: three at once in all and one per device (a friend or member may
//! connect with several sessions), at most [`UPLOAD_RATE`] bytes a second in all, and asks
//! from one device at most every few seconds. Who asks is checked
//! before anything is read; requests answered at once are bounded per peer and in all, and a
//! peer's asks, words that it has a build and fetches that found nothing are each answered
//! at most every few seconds.

use std::{
    collections::{HashMap, HashSet},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use fogata_crypto::DevicePublicKey;
use fogata_update::{Artifact, Manifest, Signed, manifest::CHUNK};
use futures::{AsyncReadExt as _, AsyncWriteExt as _};
use libp2p::{PeerId, Stream, StreamProtocol};
use tokio::{
    io::AsyncWriteExt as _,
    sync::{Semaphore, mpsc},
};

use crate::{NetError, NodeHandle, locked};

pub const UPDATE_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/update/1");

const ASK: u8 = 1;
const FETCH: u8 = 2;
const HAVE: u8 = 3;

const NOTHING: u8 = 0;
const RELEASE: u8 = 1;
const DATA: u8 = 2;
const BUSY: u8 = 3;
const NOT_HERE: u8 = 4;

/// How long a transfer waits for the other side's next bytes.
const IDLE: Duration = Duration::from_secs(30);
/// Uploads at once, in all.
const UPLOADS: usize = 3;
/// Bytes a second all uploads together send at most: a release spreads in minutes without
/// filling a home line's upload while a call goes on.
pub const UPLOAD_RATE: u64 = 4 * 1024 * 1024;
/// A peer's asks are answered at most this often; one sooner is told to ask later.
const ASK_EVERY: Duration = Duration::from_secs(3);
/// The longest platform name read.
const MAX_PLATFORM: usize = 32;
/// Requests answered at once from one peer, and in all; a stream past them is closed unread.
const REQUESTS_PER_PEER: usize = 4;
const REQUESTS: usize = 32;
/// Bytes of packages sent to one device in a day: enough to fetch a release several times
/// over, not to keep the line busy all day.
const SENT_PER_DAY: u64 = 1024 * 1024 * 1024;

/// The newest release this device checked and gives to its friends and members, with its
/// packages that are complete and checked here.
#[derive(Clone, Debug, Default)]
pub struct Shelf(Arc<Mutex<Option<Held>>>);

#[derive(Clone, Debug)]
pub struct Held {
    pub signed: Signed,
    pub manifest: Manifest,
    /// Complete packages by platform, and where they are.
    pub packages: HashMap<String, PathBuf>,
}

impl Shelf {
    pub fn set(&self, held: Option<Held>) {
        *locked(&self.0) = held;
    }

    pub fn get(&self) -> Option<Held> {
        locked(&self.0).clone()
    }

    /// Adds a complete package of the release held, if it is still that release.
    pub fn add_package(&self, build: u64, platform: &str, path: PathBuf) {
        if let Some(held) = locked(&self.0).as_mut()
            && held.manifest.build == build
        {
            held.packages.insert(platform.to_string(), path);
        }
    }

    /// Lets go of the package of `platform`: it is no longer given.
    pub fn remove_package(&self, platform: &str) {
        if let Some(held) = locked(&self.0).as_mut() {
            held.packages.remove(platform);
        }
    }

    fn build(&self) -> Option<u64> {
        locked(&self.0).as_ref().map(|h| h.manifest.build)
    }
}

/// What other devices said to this one's server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heard {
    /// A friend or member says it has `build`.
    Have { peer: PeerId, build: u64 },
    /// A friend or member asked for the package of `platform`, which is not here.
    Wanted { platform: String, build: u64 },
    /// A friend or member is given the package of `platform`.
    Given { platform: String, build: u64 },
}

/// Ways this server misbehaves, for the network lab's tests of what the other side refuses.
#[derive(Clone, Copy, Debug, Default)]
pub struct Misbehave {
    /// Every chunk sent has one byte changed.
    pub damage_chunks: bool,
    /// The release held is offered whatever build the asker has.
    pub offer_always: bool,
}

/// Serves the release on `shelf` to friends and members until the node stops. What they
/// say goes to `heard`.
pub fn serve(
    node: NodeHandle,
    shelf: Shelf,
    heard: mpsc::UnboundedSender<Heard>,
    misbehave: Misbehave,
) -> Result<(), NetError> {
    let mut incoming = node
        .streams()
        .accept(UPDATE_PROTOCOL)
        .map_err(|e| NetError::Setup(e.to_string()))?;
    let server = Arc::new(Server {
        // Serving ends with the node, and does not keep it running.
        node: node.background(),
        shelf,
        heard,
        misbehave,
        uploads: Semaphore::new(UPLOADS),
        uploading: Mutex::new(HashSet::new()),
        paced: Mutex::new(HashMap::new()),
        answering: Arc::new(Mutex::new(HashMap::new())),
        throttle: Throttle::new(UPLOAD_RATE),
        sent: crate::allowance::Allowance::new(SENT_PER_DAY),
    });
    let node = server.node.clone();
    node.spawn(async move {
        while let Some((peer, stream)) = server.node.next_stream(&mut incoming).await {
            // Strangers get nothing at all, not even a word that there is something: their
            // streams close before anything is read, and nothing waits on them.
            // Counted by device: a friend or member may connect with many sessions at once.
            let Some(device) = server.node.trusted_device(peer).await else {
                tracing::debug!(%peer, "update request from a peer that is neither friend nor member");
                continue;
            };
            let Some(turn) = Answering::start(&server.answering, device) else {
                tracing::debug!(%peer, "too many update requests at once");
                continue;
            };
            let answering = server.clone();
            server.node.spawn(async move {
                let (server, _turn) = (answering, turn);
                if let Err(e) = server.answer(peer, device, stream).await {
                    tracing::debug!(%peer, "update request ended: {e}");
                }
            });
        }
    });
    Ok(())
}

/// Requests being answered, by device; one less when dropped.
struct Answering(Arc<Mutex<HashMap<DevicePublicKey, usize>>>, DevicePublicKey);

impl Answering {
    fn start(
        answering: &Arc<Mutex<HashMap<DevicePublicKey, usize>>>,
        peer: DevicePublicKey,
    ) -> Option<Self> {
        let mut counts = locked(answering);
        let total: usize = counts.values().sum();
        let count = counts.entry(peer).or_default();
        if *count >= REQUESTS_PER_PEER || total >= REQUESTS {
            if *count == 0 {
                counts.remove(&peer);
            }
            return None;
        }
        *count += 1;
        Some(Self(answering.clone(), peer))
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        let mut counts = locked(&self.0);
        if let Some(count) = counts.get_mut(&self.1) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&self.1);
            }
        }
    }
}

struct Server {
    node: NodeHandle,
    shelf: Shelf,
    heard: mpsc::UnboundedSender<Heard>,
    misbehave: Misbehave,
    uploads: Semaphore,
    uploading: Mutex<HashSet<DevicePublicKey>>,
    /// When each device last asked, said it has a build, or fetched in vain.
    paced: Mutex<HashMap<(DevicePublicKey, u8), Instant>>,
    answering: Arc<Mutex<HashMap<DevicePublicKey, usize>>>,
    throttle: Throttle,
    sent: crate::allowance::Allowance<DevicePublicKey>,
}

impl Server {
    /// Whether `peer` did the same (`kind`) a moment ago; otherwise remembers it did now.
    fn too_soon(&self, peer: DevicePublicKey, kind: u8) -> bool {
        let now = Instant::now();
        let mut paced = locked(&self.paced);
        paced.retain(|_, at| now.duration_since(*at) < ASK_EVERY);
        paced.insert((peer, kind), now).is_some()
    }

    /// Whether `peer` fetched in vain a moment ago.
    fn failed_fetch_lately(&self, peer: DevicePublicKey) -> bool {
        locked(&self.paced)
            .get(&(peer, FETCH))
            .is_some_and(|at| at.elapsed() < ASK_EVERY)
    }

    async fn answer(
        &self,
        peer: PeerId,
        device: DevicePublicKey,
        mut stream: Stream,
    ) -> io::Result<()> {
        let kind = read_u8(&mut stream).await?;
        match kind {
            ASK => {
                let have = read_u64(&mut stream).await?;
                let _platform = read_name(&mut stream).await?;
                if self.too_soon(device, ASK) {
                    idle(stream.write_all(&[BUSY])).await?;
                    return stream.close().await;
                }
                let held = self
                    .shelf
                    .get()
                    .filter(|held| held.manifest.build > have || self.misbehave.offer_always);
                match held {
                    Some(held) => {
                        let mut out = vec![RELEASE];
                        out.extend_from_slice(&(held.signed.body.len() as u32).to_be_bytes());
                        out.extend_from_slice(&held.signed.body);
                        out.extend_from_slice(&held.signed.key);
                        out.extend_from_slice(&held.signed.signature);
                        let mut platforms: Vec<&String> = held.packages.keys().collect();
                        platforms.sort();
                        out.push(platforms.len().min(255) as u8);
                        for platform in platforms.into_iter().take(255) {
                            out.push(platform.len() as u8);
                            out.extend_from_slice(platform.as_bytes());
                        }
                        idle(stream.write_all(&out)).await?;
                    }
                    None => idle(stream.write_all(&[NOTHING])).await?,
                }
            }
            FETCH => {
                let build = read_u64(&mut stream).await?;
                let platform = read_name(&mut stream).await?;
                let first = u64::from(read_u32(&mut stream).await?);
                if self.failed_fetch_lately(device) {
                    idle(stream.write_all(&[BUSY])).await?;
                    return stream.close().await;
                }
                self.send_package(peer, device, stream, build, platform, first)
                    .await?;
                return Ok(());
            }
            HAVE => {
                let build = read_u64(&mut stream).await?;
                if !self.too_soon(device, HAVE) {
                    let _ = self.heard.send(Heard::Have { peer, build });
                }
                idle(stream.write_all(&[NOTHING])).await?;
            }
            _ => return Ok(()),
        }
        stream.close().await
    }

    async fn send_package(
        &self,
        peer: PeerId,
        device: DevicePublicKey,
        mut stream: Stream,
        build: u64,
        platform: String,
        first: u64,
    ) -> io::Result<()> {
        let Some(held) = self.shelf.get().filter(|h| h.manifest.build == build) else {
            self.too_soon(device, FETCH);
            stream.write_all(&[NOT_HERE]).await?;
            return stream.close().await;
        };
        let (Some(artifact), Some(path)) = (
            held.manifest.artifacts.get(&platform),
            held.packages.get(&platform),
        ) else {
            self.too_soon(device, FETCH);
            if held.manifest.artifacts.contains_key(&platform) {
                let _ = self.heard.send(Heard::Wanted { platform, build });
            }
            stream.write_all(&[NOT_HERE]).await?;
            return stream.close().await;
        };
        if first >= artifact.chunk_count() {
            self.too_soon(device, FETCH);
            stream.write_all(&[NOT_HERE]).await?;
            return stream.close().await;
        }
        // One upload per device, a few in all.
        let Ok(_permit) = self.uploads.try_acquire() else {
            stream.write_all(&[BUSY]).await?;
            return stream.close().await;
        };
        // What is left of the package must fit in the device's day, so that it is never cut
        // off on its way: past it, the device is told to ask later, and goes on from there.
        let left = artifact.size.saturating_sub(first * CHUNK);
        if !self.sent.fits(&device, left) || !locked(&self.uploading).insert(device) {
            stream.write_all(&[BUSY]).await?;
            return stream.close().await;
        }
        let _uploading = Uploading(&self.uploading, device);
        let _ = self.heard.send(Heard::Given {
            platform: platform.clone(),
            build,
        });
        let path = path.clone();
        let mut file = tokio::task::spawn_blocking(move || std::fs::File::open(path))
            .await
            .map_err(io::Error::other)??;
        file.seek(SeekFrom::Start(first * CHUNK))?;
        idle(stream.write_all(&[DATA])).await?;
        let mut buf = vec![0u8; CHUNK as usize];
        for index in first..artifact.chunk_count() {
            // A newer release replaced this one, or the peer is no friend or member any more:
            // the rest is not sent.
            if self.shelf.build() != Some(build) || self.node.trusted_device(peer).await.is_none() {
                return Err(io::Error::other("no longer given to that device"));
            }
            let len = artifact.chunk_len(index) as usize;
            let (back, read) = blocking((file, buf), move |(file, buf)| {
                file.read_exact(&mut buf[..len])
            })
            .await?;
            (file, buf) = back;
            read?;
            if self.misbehave.damage_chunks {
                buf[len / 2] ^= 0x55;
            }
            if !self.sent.take(device, len as u64) {
                return Err(io::Error::other(
                    "that device was sent all it may be sent today",
                ));
            }
            self.throttle.take(len as u64).await;
            idle(stream.write_all(&(len as u32).to_be_bytes())).await?;
            idle(stream.write_all(&buf[..len])).await?;
        }
        stream.close().await
    }
}

/// Takes a device off the uploads running when dropped.
struct Uploading<'a>(&'a Mutex<HashSet<DevicePublicKey>>, DevicePublicKey);

impl Drop for Uploading<'_> {
    fn drop(&mut self) {
        locked(self.0).remove(&self.1);
    }
}

/// A release another device offers: its manifest, still to check, and the platforms whose
/// packages it has.
#[derive(Clone, Debug)]
pub struct Offer {
    pub signed: Signed,
    pub platforms: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("cannot reach the other device: {0}")]
    Unreachable(String),
    #[error("the other device does not have that package")]
    NotHere,
    #[error("the other device is busy")]
    Busy,
    #[error("a chunk arrived damaged")]
    Damaged,
    #[error("the other device does not answer")]
    Refused,
    #[error(transparent)]
    Io(#[from] io::Error),
}

async fn open(node: &NodeHandle, peer: PeerId) -> Result<Stream, FetchError> {
    node.streams()
        .open_stream(peer, UPDATE_PROTOCOL)
        .await
        .map_err(|e| FetchError::Unreachable(e.to_string()))
}

/// Sends a request and reads the first byte of its answer. A stream closed or reset before
/// it is a refusal (a stranger's ask, or an older version's): a stream that is not answered
/// is closed unread.
async fn send(stream: &mut Stream, request: &[u8]) -> Result<u8, FetchError> {
    let refused = |e: io::Error| match e.kind() {
        io::ErrorKind::UnexpectedEof
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::BrokenPipe => FetchError::Refused,
        _ => e.into(),
    };
    idle(stream.write_all(request)).await.map_err(refused)?;
    idle(stream.flush()).await.map_err(refused)?;
    read_u8(stream).await.map_err(refused)
}

/// Asks `peer` for a release newer than `have`.
pub async fn ask(
    node: &NodeHandle,
    peer: PeerId,
    have: u64,
    platform: &str,
) -> Result<Option<Offer>, FetchError> {
    let mut stream = open(node, peer).await?;
    let mut out = vec![ASK];
    out.extend_from_slice(&have.to_be_bytes());
    put_name(&mut out, platform);
    match send(&mut stream, &out).await? {
        RELEASE => {}
        NOTHING => return Ok(None),
        BUSY => return Err(FetchError::Busy),
        _ => return Err(FetchError::Io(io::ErrorKind::InvalidData.into())),
    }
    let len = read_u32(&mut stream).await? as usize;
    if len > fogata_update::manifest::MAX_MANIFEST {
        return Err(FetchError::Io(io::ErrorKind::InvalidData.into()));
    }
    let mut body = vec![0u8; len];
    idle(stream.read_exact(&mut body)).await?;
    let mut key = [0u8; 32];
    idle(stream.read_exact(&mut key)).await?;
    let mut signature = [0u8; 64];
    idle(stream.read_exact(&mut signature)).await?;
    let count = read_u8(&mut stream).await?;
    let mut platforms = Vec::new();
    for _ in 0..count {
        platforms.push(read_name(&mut stream).await?);
    }
    Ok(Some(Offer {
        signed: Signed {
            body,
            key,
            signature,
        },
        platforms,
    }))
}

/// Tells `peer` that this device has `build`.
pub async fn tell(node: &NodeHandle, peer: PeerId, build: u64) -> Result<(), FetchError> {
    let mut stream = open(node, peer).await?;
    let mut out = vec![HAVE];
    out.extend_from_slice(&build.to_be_bytes());
    send(&mut stream, &out).await?;
    Ok(())
}

/// Downloads `artifact`, the package of `platform` in release `build`, from `peer` into
/// `part`, going on from the whole chunks a previous try left there (each hashed again).
/// Every chunk is checked as it arrives; a damaged one ends the download, keeping the good
/// ones before it. `progress` gets the bytes there so far. Returns once the whole package
/// is there and hashes to what the manifest says.
pub async fn fetch(
    node: &NodeHandle,
    peer: PeerId,
    build: u64,
    platform: &str,
    artifact: &Artifact,
    part: &Path,
    mut progress: impl FnMut(u64),
) -> Result<(), FetchError> {
    let (mut file, mut whole, have) = resume(part, artifact).await?;
    let count = artifact.chunk_count();
    let mut next = have / CHUNK;
    progress(have);
    if next < count {
        let mut stream = open(node, peer).await?;
        let mut out = vec![FETCH];
        out.extend_from_slice(&build.to_be_bytes());
        put_name(&mut out, platform);
        out.extend_from_slice(&(next as u32).to_be_bytes());
        match send(&mut stream, &out).await? {
            DATA => {}
            NOT_HERE => return Err(FetchError::NotHere),
            BUSY => return Err(FetchError::Busy),
            _ => return Err(FetchError::Io(io::ErrorKind::InvalidData.into())),
        }
        let mut buf = vec![0u8; CHUNK as usize];
        while next < count {
            let len = read_u32(&mut stream).await? as u64;
            if len != artifact.chunk_len(next) {
                return Err(FetchError::Damaged);
            }
            let chunk = &mut buf[..len as usize];
            idle(stream.read_exact(chunk)).await?;
            if Some(*blake3::hash(chunk).as_bytes()) != artifact.chunk_hash(next) {
                return Err(FetchError::Damaged);
            }
            whole.update(chunk);
            file.write_all(chunk).await?;
            next += 1;
            progress((next * CHUNK).min(artifact.size));
        }
    }
    file.flush().await?;
    file.sync_data().await?;
    if *whole.finalize().as_bytes() != artifact.hash() {
        // Every chunk was right and the whole is not: the manifest itself does not add up.
        drop(file);
        let _ = tokio::fs::remove_file(part).await;
        return Err(FetchError::Damaged);
    }
    Ok(())
}

/// Opens `part` to add to, keeping only its whole chunks that hash right; returns it with
/// the hash of what it holds so far and how many bytes that is.
async fn resume(
    part: &Path,
    artifact: &Artifact,
) -> io::Result<(tokio::fs::File, blake3::Hasher, u64)> {
    let earlier = crate::files::open_earlier_part_to_cut(part).await?;
    let Some(file) = earlier else {
        return Ok((
            crate::files::create_part(part).await?,
            blake3::Hasher::new(),
            0,
        ));
    };
    let artifact = artifact.clone();
    let file = file.into_std().await;
    let (file, whole, good) = tokio::task::spawn_blocking(move || -> io::Result<_> {
        let mut file = file;
        let len = file.metadata()?.len();
        file.seek(SeekFrom::Start(0))?;
        let mut whole = blake3::Hasher::new();
        let mut good = 0u64;
        let mut buf = vec![0u8; CHUNK as usize];
        let mut index = 0u64;
        while good + artifact.chunk_len(index) <= len && index < artifact.chunk_count() {
            let n = artifact.chunk_len(index) as usize;
            file.read_exact(&mut buf[..n])?;
            if Some(*blake3::hash(&buf[..n]).as_bytes()) != artifact.chunk_hash(index) {
                break;
            }
            whole.update(&buf[..n]);
            good += n as u64;
            index += 1;
        }
        file.set_len(good)?;
        file.seek(SeekFrom::End(0))?;
        Ok((file, whole, good))
    })
    .await
    .map_err(io::Error::other)??;
    Ok((tokio::fs::File::from_std(file), whole, good))
}

/// Hashes a whole package file and checks it against `artifact`: its size, BLAKE3 and
/// SHA-256. Blocking.
pub fn check_package(path: &Path, artifact: &Artifact) -> io::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    if file.metadata()?.len() != artifact.size {
        return Ok(false);
    }
    let mut builder = fogata_update::manifest::Builder::new(&artifact.file);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        builder.update(&buf[..n]);
    }
    let found = builder.finish();
    Ok(found.size == artifact.size
        && found.hash() == artifact.hash()
        && found.sha256() == artifact.sha256())
}

/// Bytes a second shared by every upload.
struct Throttle {
    rate: u64,
    /// When the bytes sent so far may all have gone at the rate.
    free_at: tokio::sync::Mutex<Instant>,
}

impl Throttle {
    fn new(rate: u64) -> Self {
        Throttle {
            rate,
            free_at: tokio::sync::Mutex::new(Instant::now()),
        }
    }

    async fn take(&self, bytes: u64) {
        let wait = {
            let mut free_at = self.free_at.lock().await;
            let now = Instant::now();
            let start = (*free_at).max(now);
            *free_at = start + Duration::from_secs_f64(bytes as f64 / self.rate as f64);
            start.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
}

async fn idle<T>(io: impl Future<Output = io::Result<T>>) -> io::Result<T> {
    tokio::time::timeout(IDLE, io).await.unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the other device stopped answering",
        ))
    })
}

async fn blocking<T: Send + 'static, R: Send + 'static>(
    mut value: T,
    f: impl FnOnce(&mut T) -> R + Send + 'static,
) -> io::Result<(T, R)> {
    tokio::task::spawn_blocking(move || {
        let r = f(&mut value);
        (value, r)
    })
    .await
    .map_err(io::Error::other)
}

fn put_name(out: &mut Vec<u8>, name: &str) {
    let name = &name.as_bytes()[..name.len().min(MAX_PLATFORM)];
    out.push(name.len() as u8);
    out.extend_from_slice(name);
}

async fn read_u8(stream: &mut Stream) -> io::Result<u8> {
    let mut b = [0u8; 1];
    idle(stream.read_exact(&mut b)).await?;
    Ok(b[0])
}

async fn read_u32(stream: &mut Stream) -> io::Result<u32> {
    let mut b = [0u8; 4];
    idle(stream.read_exact(&mut b)).await?;
    Ok(u32::from_be_bytes(b))
}

async fn read_u64(stream: &mut Stream) -> io::Result<u64> {
    let mut b = [0u8; 8];
    idle(stream.read_exact(&mut b)).await?;
    Ok(u64::from_be_bytes(b))
}

async fn read_name(stream: &mut Stream) -> io::Result<String> {
    let len = read_u8(stream).await? as usize;
    if len > MAX_PLATFORM {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut name = vec![0u8; len];
    idle(stream.read_exact(&mut name)).await?;
    String::from_utf8(name).map_err(|_| io::ErrorKind::InvalidData.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn uploads_together_keep_to_the_rate() {
        let throttle = Throttle::new(1_000_000);
        let start = Instant::now();
        for _ in 0..4 {
            throttle.take(100_000).await;
        }
        // The first goes at once; the other three wait 0.1 s each.
        let took = start.elapsed();
        assert!(took >= Duration::from_millis(290), "{took:?}");
        assert!(took < Duration::from_millis(900), "{took:?}");
    }

    #[tokio::test]
    async fn a_resumed_download_keeps_only_whole_good_chunks() {
        let dir = std::env::temp_dir().join(format!("fogata-update-part-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let data: Vec<u8> = (0..(3 * CHUNK + 10)).map(|i| (i % 253) as u8).collect();
        let artifact = Artifact::of("p", &data);
        let part = dir.join("p.part");
        // Two whole chunks and a piece of the third: two are kept.
        std::fs::write(&part, &data[..(2 * CHUNK + 7) as usize]).unwrap();
        let (_, _, have) = resume(&part, &artifact).await.unwrap();
        assert_eq!(have, 2 * CHUNK);
        assert_eq!(std::fs::metadata(&part).unwrap().len(), 2 * CHUNK);
        // A damaged second chunk: only the first is kept.
        let mut bad = data[..(2 * CHUNK) as usize].to_vec();
        bad[(CHUNK + 3) as usize] ^= 1;
        std::fs::write(&part, &bad).unwrap();
        let (_, whole, have) = resume(&part, &artifact).await.unwrap();
        assert_eq!(have, CHUNK);
        assert_eq!(whole.finalize(), blake3::hash(&data[..CHUNK as usize]));
        // Nothing there: a new one.
        std::fs::remove_file(&part).unwrap();
        let (_, _, have) = resume(&part, &artifact).await.unwrap();
        assert_eq!(have, 0);
        assert!(part.exists());
        let whole_file = dir.join("whole");
        std::fs::write(&whole_file, &data).unwrap();
        assert!(check_package(&whole_file, &artifact).unwrap());
        std::fs::write(&whole_file, &data[1..]).unwrap();
        assert!(!check_package(&whole_file, &artifact).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
