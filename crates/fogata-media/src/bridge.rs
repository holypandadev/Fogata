//! Local TURN endpoint that carries WebRTC media over member connections.
//!
//! The WebRTC engine on this device is configured with a single TURN server at
//! `127.0.0.1` (on Windows, at the device's own address: see [`BridgeConfig::listen`]) and
//! relay-only ICE. Each of its connections is for one member, and authenticates as a TURN
//! user that names that member's device ([`member_user`]): every allocation it makes is
//! that member's, and gets a virtual relay address at this device's own address
//! (`10.a.b.c`, derived from its transport peer id). What the engine sends through an
//! allocation travels to its member's device as a QUIC datagram on the existing connection;
//! the member's bridge hands it to its WebRTC engine as data from our relay address, and
//! only to an allocation of its own that is for us.
//!
//! A packet larger than one datagram can carry at the moment goes as two or more datagrams
//! that the member's bridge puts back together (see `parts.rs`).
//!
//! A member without a datagram link (connected over TCP only, or reached through another
//! member) gets the same packets over a stream of the connection there is, `/fogata/media/1`:
//! slower under loss, since a stream retransmits and keeps order, but it works wherever the
//! two can talk at all.
//!
//! No STUN or TURN server outside the two devices is involved, and WebRTC keeps doing
//! echo cancellation, codecs, jitter buffering and bandwidth adaptation.

use std::{
    any::Any,
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use fogata_net::{NodeHandle, PeerId, Stream, StreamProtocol, datagram};
use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use turn::{
    auth::{AuthHandler, generate_auth_key},
    relay::RelayAddressGenerator,
    server::{
        Server,
        config::{ConnConfig, ServerConfig},
    },
};
use webrtc_util::Conn;

pub(crate) const TAG: u8 = 0x46;
const VERSION: u8 = 1;
const HEADER_LEN: usize = 2 + 6 + 6;
pub(crate) const REALM: &str = "fogata";

/// Media packets for members without a datagram link, each preceded by its length (two
/// bytes, big endian).
pub const MEDIA_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/media/1");
/// Says that this bridge puts packets sent in parts back together (see `parts.rs`): taken
/// and never used, it only shows in what the device tells others it takes, so a member's
/// bridge sends parts only to bridges that know them. Older ones get such packets on the
/// media stream, as before.
pub const PARTS_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/media-parts/1");
/// Packets waiting for a member's stream; more are dropped, like a full network would.
const STREAM_QUEUE: usize = 64;
/// A media stream nobody wrote to for this long is closed.
const STREAM_IDLE: Duration = Duration::from_secs(30);
/// After a stream could not be opened, the next try waits this long.
const STREAM_RETRY: Duration = Duration::from_secs(2);
/// A media packet that waited this long for a stream is dropped: the other end's jitter
/// buffer has moved on, and it would only delay the packets behind it.
const STREAM_STALE: Duration = Duration::from_millis(150);
/// No media packet is longer: WebRTC keeps them under the network's packet size.
const MAX_PACKET: usize = 4096;
/// Media packets from members waiting to be handled (one member's take at most a quarter, see
/// `datagram::PEER_SHARE`), and waiting for the local WebRTC engine at one allocation; more
/// are dropped, like a full network would. An allocation the engine no longer reads would
/// otherwise keep whatever members send it.
const RECEIVED_QUEUE: usize = 4096;
const ALLOCATION_QUEUE: usize = 1024;
/// Most packets of one member waiting at one allocation: the engine's TURN server drops
/// packets of a member the allocation has no permission for only once it reads them, so a
/// member in the group but not in the call could otherwise fill the queue and push out
/// the packets of the one the allocation talks to.
const SOURCE_SHARE: usize = ALLOCATION_QUEUE / 4;
/// How often the debug log sums up the media sent to and received from each member.
const TRAFFIC_LOG: Duration = Duration::from_secs(5);

/// The last three bytes of a device's virtual relay address, `10.a.b.c` (see
/// [`relay_address`]).
pub type Block = [u8; 3];

/// A member's device, by its public key, as the TURN user of a connection for it names it.
pub type Member = [u8; 32];

/// The TURN user the local WebRTC engine's connection for `member` authenticates as: the
/// bridge's own `user`, a dot, and the member's device key in lowercase hex.
pub fn member_user(user: &str, member: &Member) -> String {
    let mut name = String::with_capacity(user.len() + 1 + 64);
    name.push_str(user);
    name.push('.');
    for byte in member {
        name.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        name.push(char::from(b"0123456789abcdef"[usize::from(byte & 15)]));
    }
    name
}

/// The member a TURN user names, when it is `user` as [`member_user`] makes it.
fn member_named(user: &str, username: &str) -> Option<Member> {
    let hex = username.strip_prefix(user)?.strip_prefix('.')?.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut member = [0u8; 32];
    for (byte, [high, low]) in member.iter_mut().zip(hex.as_chunks::<2>().0) {
        *byte = digit(*high)? << 4 | digit(*low)?;
    }
    Some(member)
}

/// The last three bytes of a device's virtual relay address: 24 bits of a hash of its
/// transport identity, the last byte never 0 or 255. Nothing is routed by it: two members
/// that share one (by chance, or because one made up an identity for it) each still get
/// only what is meant for them, as every allocation is for one member.
pub fn block_of(peer: &PeerId) -> Block {
    let hash = Sha256::digest(peer.to_bytes());
    [hash[0], hash[1], 1 + hash[2] % 254]
}

/// A device's virtual relay address, from its transport identity.
pub fn relay_address(peer: &PeerId) -> Ipv4Addr {
    relay_ip(block_of(peer))
}

fn relay_ip(block: Block) -> Ipv4Addr {
    Ipv4Addr::new(10, block[0], block[1], block[2])
}

/// Whether `dst` is at the relay address of the device whose block is `block`.
fn to_relay(block: Block, dst: SocketAddr) -> bool {
    dst.ip() == IpAddr::V4(relay_ip(block))
}

/// Of a member's transport identities now connected, each with its block, the one to send
/// to: the one whose relay address `target` is, if any (the engine sends to the address the
/// member's own engine gave), or else the first.
fn pick(
    peers: &[(PeerId, Block)],
    target: SocketAddr,
    connected: impl Fn(&PeerId) -> bool,
) -> Option<PeerId> {
    let mut live = peers.iter().filter(|(peer, _)| connected(peer));
    let first = live.next()?;
    Some(
        std::iter::once(first)
            .chain(live)
            .find(|(_, block)| to_relay(*block, target))
            .unwrap_or(first)
            .0,
    )
}

/// Whether `src` is `peer`'s own relay address: what a member sends comes from its own
/// allocations, as its engine sends through them.
fn own_source(peer: PeerId, src: SocketAddr) -> bool {
    if src.ip() != IpAddr::V4(relay_address(&peer)) {
        tracing::debug!(%peer, %src, "media packet with a foreign source dropped");
        return false;
    }
    true
}

/// The queue of the allocation at relay `port`, when it is for the member `sender` is.
fn queue_for(
    allocations: &HashMap<u16, Allocation>,
    port: u16,
    sender: Option<Member>,
) -> Option<&mpsc::Sender<(Bytes, SocketAddr)>> {
    allocations
        .get(&port)
        .filter(|a| Some(a.member) == sender)
        .map(|a| &a.queue)
}

/// Settings of the local TURN endpoint.
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Usually on `127.0.0.1`. On Windows a socket bound to a network address cannot send to
    /// `127.0.0.1`, and WebRTC binds its sockets to the device's network addresses, so there
    /// the endpoint listens on the device's own address; it then answers only packets that
    /// come from that same address, that is from this device.
    pub listen: SocketAddr,
    /// The engine's connection for a member authenticates as [`member_user`] of this, with
    /// `password`; no other user is answered.
    pub username: String,
    pub password: String,
    /// Test only: degrade outgoing media datagrams to imitate a bad network.
    pub impairment: Option<Impairment>,
    /// Test only: send over streams even where datagrams would work.
    pub streams_only: bool,
    /// Test only: while this file exists, the answers to connectivity checks the local
    /// WebRTC engine sends are dropped, as a modified copy that stops answering them (and
    /// with them the consent to send) would do, while everything else still goes out.
    pub unanswered: Option<std::path::PathBuf>,
}

/// Loss and delay applied to outgoing media datagrams, for tests.
#[derive(Clone, Copy, Debug)]
pub struct Impairment {
    /// Probability of dropping a datagram, from 0 to 1.
    pub loss: f64,
    pub delay: Duration,
    /// Extra delay, uniformly between zero and this.
    pub jitter: Duration,
}

/// Small xorshift generator; only used to imitate packet loss in tests.
struct LabRandom(std::sync::atomic::AtomicU64);

impl LabRandom {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
            | 1;
        Self(std::sync::atomic::AtomicU64::new(seed))
    }

    /// Uniform in [0, 1).
    fn next(&self) -> f64 {
        let mut x = self.0.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0.store(x, Ordering::Relaxed);
        (x >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Running bridge. [`Bridge::close`] stops it and waits for that; dropped, it stops what
/// it runs without waiting.
pub struct Bridge {
    server: Option<Server>,
    addr: SocketAddr,
    shared: Arc<Shared>,
}

/// Longest [`Bridge::close`] waits for the bridge's tasks to end once told to.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Bridge")
    }
}

impl Bridge {
    /// Where the local WebRTC engine reaches the TURN endpoint.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stops the TURN endpoint and every task of the bridge, and waits for them to end (at
    /// most [`CLOSE_WAIT`] for the tasks).
    pub async fn close(&self) {
        if let Some(server) = &self.server {
            let _ = server.close().await;
        }
        self.shared.stop_tasks().await;
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        // A JoinSet dropped aborts its tasks, on any thread. A TURN server dropped stops on
        // any thread too: its loop ends once no command can reach it, and closes its
        // allocations and the endpoint. On a runtime it is told to close at once.
        drop(self.shared.tasks.lock().map(|mut t| t.take()));
        if let Some(server) = self.server.take()
            && let Ok(runtime) = tokio::runtime::Handle::try_current()
        {
            runtime.spawn(async move {
                let _ = server.close().await;
            });
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("cannot listen for the local WebRTC engine: {0}")]
    Listen(std::io::Error),
    #[error("TURN endpoint: {0}")]
    Turn(String),
}

/// Packets waiting at each allocation, by relay port and member relay address: at most
/// [`SOURCE_SHARE`] each.
#[derive(Default)]
struct Shares(Mutex<HashMap<(u16, IpAddr), usize>>);

impl Shares {
    /// Queues a packet from `src` for the allocation at `port` with `send`, unless `src`
    /// already has its share waiting there. Counted before it is queued, under the lock the
    /// allocation takes to count it out, so the two never miss each other.
    fn queue(&self, port: u16, src: IpAddr, send: impl FnOnce() -> bool) {
        let Ok(mut waiting) = self.0.lock() else {
            return;
        };
        let held = waiting.entry((port, src)).or_default();
        if *held >= SOURCE_SHARE {
            return;
        }
        *held += 1;
        if !send() {
            *held -= 1;
            if *held == 0 {
                waiting.remove(&(port, src));
            }
        }
    }

    /// The allocation at `port` took a packet from `src` off its queue.
    fn taken(&self, port: u16, src: IpAddr) {
        if let Ok(mut waiting) = self.0.lock()
            && let Some(held) = waiting.get_mut(&(port, src))
        {
            *held = held.saturating_sub(1);
            if *held == 0 {
                waiting.remove(&(port, src));
            }
        }
    }

    /// The allocation at `port` is gone.
    fn forget(&self, port: u16) {
        if let Ok(mut waiting) = self.0.lock() {
            waiting.retain(|(p, _), _| *p != port);
        }
    }
}

/// An allocation of the local WebRTC engine: the member its connection is for, and what
/// that member sends it, waiting for the engine.
struct Allocation {
    member: Member,
    queue: mpsc::Sender<(Bytes, SocketAddr)>,
}

/// Shared state between the TURN allocations and the datagram dispatcher.
struct Shared {
    node: NodeHandle,
    local_block: Block,
    /// Allocations of the local WebRTC engine, by relay port.
    allocations: Mutex<HashMap<u16, Allocation>>,
    /// Packets waiting at each allocation from each member.
    waiting: Shares,
    /// The transport identities connected of each member device, with their blocks.
    peers: Mutex<HashMap<Member, Vec<(PeerId, Block)>>>,
    next_port: AtomicU16,
    impairment: Option<Impairment>,
    streams_only: bool,
    unanswered: Option<std::path::PathBuf>,
    random: LabRandom,
    /// Media streams to members without a datagram link, by member.
    streams: Mutex<HashMap<PeerId, Outgoing>>,
    /// Asks for the members' identities now: a packet found none, maybe for a member that
    /// just came.
    refresh: tokio::sync::Notify,
    /// What went to each member and came from it since the last summary in the log.
    traffic: Mutex<HashMap<PeerId, Traffic>>,
    /// Packets from members that came in parts, while their parts come in.
    reassembly: Mutex<crate::parts::Reassembly>,
    /// Number of the next packet sent in parts.
    next_split: AtomicU16,
    /// Media streams taken from members now.
    streams_in: StreamsIn,
    /// Every task the bridge runs; `None` once it stopped, when no other starts.
    tasks: Mutex<Option<tokio::task::JoinSet<()>>>,
}

/// Media sent to one member and received from it, counted for the log.
#[derive(Default)]
struct Traffic {
    datagrams: u64,
    datagram_bytes: u64,
    /// Packets larger than a datagram could carry, sent in parts.
    split: u64,
    /// Packets that went on the member's media stream.
    streamed: u64,
    stream_bytes: u64,
    /// Packets dropped: the stream's queue was full, or they waited too long in it.
    dropped: u64,
    largest: usize,
    /// Received: as datagrams (parts included), parts, packets put back together from
    /// parts, and on the member's media stream.
    got_datagrams: u64,
    got_parts: u64,
    rebuilt: u64,
    got_streamed: u64,
}

/// The queue of a member's media stream, or when it may be tried again.
enum Outgoing {
    Open(mpsc::Sender<(Instant, Bytes)>),
    Failed(Instant),
}

impl Shared {
    /// Runs `task` as one of the bridge's, unless the bridge stopped. Those that ended are
    /// let go of first, so the set holds only what runs.
    fn spawn(&self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        if let Ok(mut tasks) = self.tasks.lock()
            && let Some(tasks) = tasks.as_mut()
        {
            while tasks.try_join_next().is_some() {}
            tasks.spawn(task);
        }
    }

    /// Stops every task of the bridge, and waits for them to end, at most [`CLOSE_WAIT`].
    async fn stop_tasks(&self) {
        let tasks = self.tasks.lock().ok().and_then(|mut t| t.take());
        let Some(mut tasks) = tasks else {
            return;
        };
        tasks.abort_all();
        if tokio::time::timeout(CLOSE_WAIT, async {
            while tasks.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            tracing::warn!("media bridge: {} tasks did not end in time", tasks.len());
        }
        if let Ok(mut streams) = self.streams.lock() {
            streams.clear();
        }
    }

    /// Takes again the transport identities of every member connected, by device.
    async fn refresh_peers(&self) {
        let Ok(peers) = self.node.member_peers().await else {
            return;
        };
        let mut by_member: HashMap<Member, Vec<(PeerId, Block)>> = HashMap::new();
        for peer in peers {
            if let Some(device) = self.node.member_device(&peer) {
                by_member
                    .entry(*device.as_bytes())
                    .or_default()
                    .push((peer, block_of(&peer)));
            }
        }
        if let Ok(mut known) = self.peers.lock() {
            *known = by_member;
        }
    }

    /// The transport identity to send what goes to `target` for `member` to.
    fn peer_for(&self, member: &Member, target: SocketAddr) -> Option<PeerId> {
        let known = self.peers.lock().ok()?;
        pick(known.get(member)?, target, |peer| self.node.is_member(peer))
    }

    /// Hands a packet from a member, received on its media stream or as a datagram, to the
    /// allocation its destination port belongs to; a part waits for the rest of its packet.
    fn deliver(&self, peer: PeerId, packet: &Bytes, streamed: bool) {
        if !self.in_call(&peer) {
            tracing::debug!(%peer, "media packet from a member in no call here dropped");
            return;
        }
        self.count(peer, |t| {
            if streamed {
                t.got_streamed += 1;
            } else {
                t.got_datagrams += 1;
            }
        });
        if let Some(part) = crate::parts::decode(packet) {
            if !own_source(peer, part.src) {
                return;
            }
            let whole = self
                .reassembly
                .lock()
                .ok()
                .and_then(|mut parts| parts.add(peer, part, Instant::now()));
            let rebuilt = u64::from(whole.is_some());
            self.count(peer, |t| {
                t.got_parts += 1;
                t.rebuilt += rebuilt;
            });
            if let Some((src, dst, payload)) = whole {
                self.hand_to_engine(peer, src, dst, payload);
            }
            return;
        }
        let Some((src, dst, payload)) = decode(packet) else {
            return;
        };
        if own_source(peer, src) {
            self.hand_to_engine(peer, src, dst, payload);
        }
    }

    /// Whether `peer` is a member whose device the local engine has an allocation for, that
    /// is one in a call with this device: media from anyone else is dropped before it is put
    /// together, queued or given a stream, so a member of any group cannot make this device
    /// hold its packets.
    fn in_call(&self, peer: &PeerId) -> bool {
        let Some(member) = self.node.member_device(peer).map(|d| *d.as_bytes()) else {
            return false;
        };
        self.allocations
            .lock()
            .is_ok_and(|a| a.values().any(|a| a.member == member))
    }

    /// Hands what `peer` sent to `dst` to the allocation there, when that allocation is for
    /// the member `peer` is: a member reaches only the connections meant for it.
    fn hand_to_engine(&self, peer: PeerId, src: SocketAddr, dst: SocketAddr, payload: Bytes) {
        // Allocations are told apart by their port; the address must be this device's own.
        if !to_relay(self.local_block, dst) {
            tracing::debug!(%dst, "media packet for another relay address dropped");
            return;
        }
        let sender = self.node.member_device(&peer).map(|d| *d.as_bytes());
        let target = self
            .allocations
            .lock()
            .ok()
            .and_then(|a| queue_for(&a, dst.port(), sender).cloned());
        let Some(target) = target else {
            tracing::debug!(%peer, %dst, "media packet for no allocation of its member");
            return;
        };
        self.waiting.queue(dst.port(), src.ip(), || {
            target.try_send((payload, src)).is_ok()
        });
    }

    /// Queues a packet on the member's media stream, opening it first if needed.
    fn send_stream(self: &Arc<Self>, peer: PeerId, packet: Bytes) {
        let Ok(mut streams) = self.streams.lock() else {
            return;
        };
        let queue = match streams.get(&peer) {
            Some(Outgoing::Open(queue)) if !queue.is_closed() => queue.clone(),
            Some(Outgoing::Failed(since)) if since.elapsed() < STREAM_RETRY => {
                self.count(peer, |t| t.dropped += 1);
                return;
            }
            _ => {
                let (queue, packets) = mpsc::channel(STREAM_QUEUE);
                streams.insert(peer, Outgoing::Open(queue.clone()));
                self.spawn(write_stream(self.clone(), peer, packets));
                queue
            }
        };
        drop(streams);
        let len = packet.len() as u64;
        if queue.try_send((Instant::now(), packet)).is_err() {
            tracing::debug!(%peer, "media stream full, packet dropped");
            self.count(peer, |t| t.dropped += 1);
        } else {
            self.count(peer, |t| {
                t.streamed += 1;
                t.stream_bytes += len;
            });
        }
    }

    /// Sends one datagram to a member, through the lab's loss and delay in tests. One that
    /// no longer fits, the path having shrunk just now, is lost like a packet on the way.
    fn send_datagram(&self, peer: PeerId, link: &datagram::DatagramLink, packet: Bytes) {
        let len = packet.len() as u64;
        match self.impairment {
            None => match link.send(packet) {
                Ok(()) => self.count(peer, |t| {
                    t.datagrams += 1;
                    t.datagram_bytes += len;
                }),
                Err(e) => {
                    tracing::debug!("media datagram to {peer} dropped: {e}");
                    self.count(peer, |t| t.dropped += 1);
                }
            },
            Some(lab) => {
                if self.random.next() >= lab.loss {
                    let wait = lab.delay + lab.jitter.mul_f64(self.random.next());
                    let link = link.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(wait).await;
                        let _ = link.send(packet);
                    });
                }
            }
        }
    }

    fn count(&self, peer: PeerId, add: impl FnOnce(&mut Traffic)) {
        if let Ok(mut traffic) = self.traffic.lock() {
            add(traffic.entry(peer).or_default());
        }
    }

    /// Logs what went to each member and came from it since the last call, and how the
    /// QUIC path to it is.
    fn log_traffic(&self, every: Duration) {
        let Ok(mut traffic) = self.traffic.lock() else {
            return;
        };
        let secs = every.as_secs_f64();
        for (peer, t) in traffic.drain() {
            let path = datagram::link(&peer).map(|link| link.path());
            tracing::debug!(
                %peer,
                "media out in {secs:.0} s: {} datagrams ({:.0} kbit/s, {} packets in parts), \
                 {} on the stream ({:.0} kbit/s), {} dropped, largest {} bytes; in: {} \
                 datagrams ({} parts, {} packets put together), {} from the stream; \
                 path {path:?}",
                t.datagrams,
                t.datagram_bytes as f64 * 8.0 / secs / 1000.0,
                t.split,
                t.streamed,
                t.stream_bytes as f64 * 8.0 / secs / 1000.0,
                t.dropped,
                t.largest,
                t.got_datagrams,
                t.got_parts,
                t.rebuilt,
                t.got_streamed,
            );
        }
    }
}

/// Opens a media stream to `peer` and writes queued packets to it until the queue is idle
/// or the stream breaks.
async fn write_stream(
    shared: Arc<Shared>,
    peer: PeerId,
    mut packets: mpsc::Receiver<(Instant, Bytes)>,
) {
    let mut stream = match shared
        .node
        .streams()
        .open_stream(peer, MEDIA_PROTOCOL)
        .await
    {
        Ok(stream) => stream,
        Err(e) => {
            tracing::debug!(%peer, "cannot open a media stream: {e}");
            if let Ok(mut streams) = shared.streams.lock() {
                streams.insert(peer, Outgoing::Failed(Instant::now()));
            }
            return;
        }
    };
    tracing::debug!(%peer, "media over a stream");
    let mut framed = BytesMut::new();
    while let Ok(Some((queued, packet))) = tokio::time::timeout(STREAM_IDLE, packets.recv()).await {
        let Ok(len) = u16::try_from(packet.len()) else {
            continue;
        };
        if queued.elapsed() > STREAM_STALE {
            shared.count(peer, |t| t.dropped += 1);
            continue;
        }
        framed.clear();
        framed.put_u16(len);
        framed.put_slice(&packet);
        if stream.write_all(&framed).await.is_err() || stream.flush().await.is_err() {
            break;
        }
    }
    let _ = stream.close().await;
}

/// Media streams taken from members at once, from each member device and in all; more
/// are refused. A member uses one (its bridge opens one to this device), a second while
/// its connection changes.
const STREAMS_PER_MEMBER: usize = 2;
const STREAMS_IN: usize = 16;

/// How long a member's media stream may stay without a packet before it is closed: a
/// little longer than the member's own bridge keeps one that carries nothing
/// ([`STREAM_IDLE`]).
const STREAM_QUIET: Duration = Duration::from_secs(40);
/// How long the rest of a packet may take once its first byte came.
const PACKET_WITHIN: Duration = Duration::from_secs(2);
/// How often a quiet stream looks whether its member is still one.
const MEMBER_CHECK: Duration = Duration::from_secs(2);

/// The media streams taken from members now, by member device and in all.
#[derive(Default)]
struct StreamsIn(Mutex<(HashMap<Member, usize>, usize)>);

impl StreamsIn {
    /// Takes a place for a stream from `member`; false when it has its share, or all are
    /// taken.
    fn take(&self, member: Member) -> bool {
        let Ok(mut held) = self.0.lock() else {
            return false;
        };
        let (by_member, all) = &mut *held;
        let mine = by_member.entry(member).or_default();
        if *mine >= STREAMS_PER_MEMBER || *all >= STREAMS_IN {
            if *mine == 0 {
                by_member.remove(&member);
            }
            return false;
        }
        *mine += 1;
        *all += 1;
        true
    }

    /// Gives back a place [`Self::take`] gave.
    fn give_back(&self, member: Member) {
        if let Ok(mut held) = self.0.lock() {
            let (by_member, all) = &mut *held;
            *all = all.saturating_sub(1);
            if let Some(mine) = by_member.get_mut(&member) {
                *mine -= 1;
                if *mine == 0 {
                    by_member.remove(&member);
                }
            }
        }
    }
}

/// A place taken in [`Shared::streams_in`], given back however the stream's task ends,
/// aborted too.
struct StreamPlace {
    shared: Arc<Shared>,
    member: Member,
}

impl Drop for StreamPlace {
    fn drop(&mut self) {
        self.shared.streams_in.give_back(self.member);
    }
}

/// Why reading a member's media stream ended.
#[derive(Debug, PartialEq, Eq)]
enum StreamEnd {
    /// It ended, or broke.
    Closed,
    /// Nothing came for [`STREAM_QUIET`].
    Quiet,
    /// A packet began and its rest did not come within [`PACKET_WITHIN`].
    Stalled,
    /// A packet longer than any media packet.
    TooLong,
    /// Its sender is no longer a member.
    NotMember,
}

/// The time limits of a member's media stream (see [`STREAM_QUIET`] and the others).
struct StreamTimes {
    quiet: Duration,
    packet: Duration,
    check: Duration,
}

const STREAM_TIMES: StreamTimes = StreamTimes {
    quiet: STREAM_QUIET,
    packet: PACKET_WITHIN,
    check: MEMBER_CHECK,
};

/// Reads media packets from a member's stream until it ends, and hands each to the bridge.
async fn read_stream(shared: Arc<Shared>, peer: PeerId, stream: Stream, _place: StreamPlace) {
    let end = read_packets(
        stream,
        &STREAM_TIMES,
        || shared.node.is_member(&peer),
        |packet| shared.deliver(peer, &packet, true),
    )
    .await;
    tracing::debug!(%peer, ?end, "media stream from a member ended");
}

/// Reads packets, each after its length (two bytes, big endian), from `stream` and gives
/// each to `deliver`, until it ends, stays quiet too long, stops half way through a packet,
/// or its sender stops being a member (asked as packets come, and every `times.check`
/// while none does).
async fn read_packets<R: futures::AsyncRead + Unpin>(
    mut stream: R,
    times: &StreamTimes,
    member: impl Fn() -> bool,
    mut deliver: impl FnMut(Bytes),
) -> StreamEnd {
    let mut packet = vec![0u8; MAX_PACKET];
    let mut quiet_since = Instant::now();
    loop {
        // One byte: a read cut short by the time limit takes nothing.
        let mut first = [0u8; 1];
        match tokio::time::timeout(times.check, stream.read(&mut first)).await {
            Err(_) => {
                if !member() {
                    return StreamEnd::NotMember;
                }
                if quiet_since.elapsed() >= times.quiet {
                    return StreamEnd::Quiet;
                }
                continue;
            }
            Ok(Ok(0) | Err(_)) => return StreamEnd::Closed,
            Ok(Ok(_)) => {}
        }
        if !member() {
            return StreamEnd::NotMember;
        }
        let rest = async {
            let mut second = [0u8; 1];
            stream.read_exact(&mut second).await?;
            let len = usize::from(u16::from_be_bytes([first[0], second[0]]));
            if len > MAX_PACKET {
                return Ok(None);
            }
            stream.read_exact(&mut packet[..len]).await?;
            Ok::<_, std::io::Error>(Some(len))
        };
        match tokio::time::timeout(times.packet, rest).await {
            Err(_) => return StreamEnd::Stalled,
            Ok(Err(_)) => return StreamEnd::Closed,
            Ok(Ok(None)) => return StreamEnd::TooLong,
            Ok(Ok(Some(len))) => deliver(Bytes::copy_from_slice(&packet[..len])),
        }
        quiet_since = Instant::now();
    }
}

/// Tells the devices `node` connects to, from now on, that its bridge puts packets sent in
/// parts back together ([`PARTS_PROTOCOL`]). Called as the node starts, so the first
/// exchange of what each side takes carries it, well before any call; the bridge calls it
/// too, which does nothing more once done. Streams of it are taken and dropped: none is
/// ever opened.
pub fn advertise_parts(node: &NodeHandle) {
    if let Ok(mut incoming) = node.streams().accept(PARTS_PROTOCOL) {
        let node = node.background();
        tokio::spawn(async move { while node.next_stream(&mut incoming).await.is_some() {} });
    }
}

/// Starts the local TURN endpoint for `node`.
pub async fn start(node: NodeHandle, config: BridgeConfig) -> Result<Bridge, BridgeError> {
    let shared = Arc::new(Shared {
        local_block: block_of(&node.local_peer()),
        node,
        allocations: Mutex::new(HashMap::new()),
        waiting: Shares::default(),
        peers: Mutex::new(HashMap::new()),
        refresh: tokio::sync::Notify::new(),
        next_port: AtomicU16::new(FIRST_PORT),
        impairment: config.impairment,
        streams_only: config.streams_only,
        unanswered: config.unanswered,
        random: LabRandom::new(),
        streams: Mutex::new(HashMap::new()),
        traffic: Mutex::new(HashMap::new()),
        reassembly: Mutex::new(crate::parts::Reassembly::default()),
        next_split: AtomicU16::new(0),
        streams_in: StreamsIn::default(),
        tasks: Mutex::new(Some(tokio::task::JoinSet::new())),
    });
    // What can fail comes before any task starts; the TURN server, last, stops them all
    // when it does not start.
    let socket = tokio::net::UdpSocket::bind(config.listen)
        .await
        .map_err(BridgeError::Listen)?;
    let addr = socket.local_addr().map_err(BridgeError::Listen)?;
    shared.refresh_peers().await;
    if tracing::enabled!(tracing::Level::DEBUG) {
        let shared = shared.clone();
        shared.clone().spawn(async move {
            loop {
                tokio::time::sleep(TRAFFIC_LOG).await;
                shared.log_traffic(TRAFFIC_LOG);
            }
        });
    }

    // Datagrams from members: hand each to the allocation its destination port belongs to.
    // Those of peers in no call with this device are dropped before they take a place in
    // the queue. The registry keeps `accept` until the bridge's tasks end, so it holds a
    // handle that does not keep the bridge running.
    let weak = Arc::downgrade(&shared);
    let mut datagrams = datagram::receive(RECEIVED_QUEUE, move |peer| {
        weak.upgrade().is_some_and(|shared| shared.in_call(peer))
    });
    {
        let shared = shared.clone();
        shared.clone().spawn(async move {
            while let Some((peer, packet)) = datagrams.next().await {
                shared.deliver(peer, &packet, false);
            }
        });
    }
    advertise_parts(&shared.node);
    // Media streams from members without a datagram link.
    if let Ok(mut incoming) = shared.node.streams().accept(MEDIA_PROTOCOL) {
        let shared = shared.clone();
        shared.clone().spawn(async move {
            while let Some((peer, stream)) = incoming.next().await {
                let Some(member) = shared.node.member_device(&peer).map(|d| *d.as_bytes()) else {
                    tracing::debug!(%peer, "media stream from a non-member refused");
                    continue;
                };
                if !shared.in_call(&peer) {
                    tracing::debug!(%peer, "media stream from a member in no call here refused");
                    continue;
                }
                if !shared.streams_in.take(member) {
                    tracing::debug!(%peer, "media stream refused: the member has its share");
                    continue;
                }
                let place = StreamPlace {
                    shared: shared.clone(),
                    member,
                };
                shared.spawn(read_stream(shared.clone(), peer, stream, place));
            }
        });
    }

    // Members come and go; keep their identities fresh, and fetch them at once when a
    // packet finds none (a call starting just after a member connected), at most four times
    // a second.
    {
        let shared = shared.clone();
        shared.clone().spawn(async move {
            loop {
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(2)) => {}
                    () = shared.refresh.notified() => {}
                }
                shared.refresh_peers().await;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        });
    }

    let socket = Endpoint::new(socket, (!addr.ip().is_loopback()).then_some(addr.ip()));
    let server = Server::new(ServerConfig {
        conn_configs: vec![ConnConfig {
            conn: Arc::new(socket),
            relay_addr_generator: Box::new(VirtualRelays {
                shared: shared.clone(),
                user: config.username.clone(),
            }),
        }],
        realm: REALM.to_string(),
        auth_handler: Arc::new(LocalAuth::new(
            config.username,
            config.password,
            addr.ip().is_loopback(),
        )),
        channel_bind_timeout: Duration::from_secs(0),
        alloc_close_notify: None,
    })
    .await;
    match server {
        Ok(server) => Ok(Bridge {
            server: Some(server),
            addr,
            shared,
        }),
        Err(e) => {
            shared.stop_tasks().await;
            Err(BridgeError::Turn(e.to_string()))
        }
    }
}

/// Writes a virtual address: its IPv4 address and port (there are no others).
pub(crate) fn put_addr(out: &mut BytesMut, addr: SocketAddr) -> Option<()> {
    let IpAddr::V4(ip) = addr.ip() else {
        return None;
    };
    out.put_slice(&ip.octets());
    out.put_u16(addr.port());
    Some(())
}

/// How a packet goes to a member with a datagram link.
#[derive(Debug, PartialEq, Eq)]
enum Way {
    /// In one datagram.
    Whole,
    /// Larger than what one QUIC packet carries now: the path is small (a VPN, say), or QUIC
    /// took it to be for a while after some losses. In parts, a datagram each (see
    /// parts.rs), so like any media packet it is lost rather than late and holds up nothing
    /// behind it; audio is small enough to go whole.
    Parts,
    /// As large, to a member whose bridge does not put parts back together (an older
    /// version): on the media stream, which carries it late rather than not at all.
    Stream,
}

fn way(len: usize, max: usize, takes_parts: bool) -> Way {
    if len <= max {
        Way::Whole
    } else if takes_parts {
        Way::Parts
    } else {
        Way::Stream
    }
}

pub(crate) fn encode(src: SocketAddr, dst: SocketAddr, payload: &[u8]) -> Option<Bytes> {
    let mut out = BytesMut::with_capacity(HEADER_LEN + payload.len());
    out.put_u8(TAG);
    out.put_u8(VERSION);
    put_addr(&mut out, src)?;
    put_addr(&mut out, dst)?;
    out.put_slice(payload);
    Some(out.freeze())
}

pub(crate) fn decode(packet: &Bytes) -> Option<(SocketAddr, SocketAddr, Bytes)> {
    if packet.len() < HEADER_LEN || packet[0] != TAG || packet[1] != VERSION {
        return None;
    }
    let addr = |at: usize| {
        let ip = Ipv4Addr::new(packet[at], packet[at + 1], packet[at + 2], packet[at + 3]);
        let port = u16::from_be_bytes([packet[at + 4], packet[at + 5]]);
        SocketAddr::new(IpAddr::V4(ip), port)
    };
    Some((addr(2), addr(8), packet.slice(HEADER_LEN..)))
}

/// Windows' code for a datagram longer than the buffer it was read into (`WSAEMSGSIZE`).
const MESSAGE_TOO_LONG: i32 = 10040;

/// Whether a read error of the endpoint's socket concerns one packet only. Windows reports
/// there that an earlier packet met a closed port, as when the engine closes a connection,
/// and that a packet was longer than the TURN server reads at once: anyone who can send to
/// the port could otherwise stop every call with one large packet. Neither code means
/// anything else elsewhere.
fn passing(e: &std::io::Error) -> bool {
    e.kind() == std::io::ErrorKind::ConnectionReset || e.raw_os_error() == Some(MESSAGE_TOO_LONG)
}

/// The endpoint's socket. Listening on a network address, it drops every packet that does
/// not come from that address, so other devices on the network get no answer at all. One that
/// sends with this device's address as its source (whether the system lets such a packet in
/// is up to the system) gets every answer sent here, not to itself: it never sees the nonce
/// a request needs, so it gets no allocation, and media it sends blind goes nowhere unless it
/// guesses the port of one of the engine's own sockets, whose packets never leave this device;
/// even then the other end's WebRTC drops it, unable to decrypt it.
///
/// Any program on this device can send to it, though. Only STUN messages and channel data
/// go on to the TURN server, and only so many requests: the local engine makes a few for
/// each connection, while each request without valid credentials makes the server keep a
/// nonce (for an hour at most, and a few thousand at once: see vendor/turn). A source the
/// server answered with valid credentials (one of the engine's sockets) has its own share
/// of requests, so its refreshes, permissions and channels go through whatever others
/// send; requests from every other source share one.
///
/// Media (channel data and Send indications) goes on only from an address and port the
/// server has answered a request with valid credentials from (an allocation, a refresh, a
/// permission, a channel), as only the engine knows them, and only so much from each: a
/// device on the network that fakes this device's address and guesses the port of one of
/// the engine's sockets can then make the bridge carry no more than the engine itself could
/// send, and one that guesses nothing reaches nothing.
struct Endpoint {
    socket: tokio::net::UdpSocket,
    only_from: Option<IpAddr>,
    requests: Mutex<Requests>,
    known: Mutex<HashMap<SocketAddr, Known>>,
}

/// A source the TURN server answered a request with valid credentials from.
struct Known {
    answered: Instant,
    media: Requests,
    requests: Requests,
}

/// How long a source stays known after its last answer. The engine refreshes its
/// allocations and permissions every few minutes while it uses them.
const KNOWN_FOR: Duration = Duration::from_secs(15 * 60);
/// Sources known at once: the engine has a few sockets for each call.
const KNOWN_MAX: usize = 256;
/// Media packets taken from one source at once, and each second: several times a shared
/// screen at its highest rate (6 Mbit/s, about 600 packets a second) with the voice, a
/// camera and the screen's sound.
const MEDIA_BURST: f64 = 5000.0;
const MEDIA_PER_SECOND: f64 = 5000.0;
/// Requests taken from one known source at once, and each second: the engine refreshes
/// an allocation, its permissions and channels every few minutes.
const KNOWN_REQUESTS_BURST: f64 = 32.0;
const KNOWN_REQUESTS_PER_SECOND: f64 = 2.0;

/// Whether `packet` is the TURN server's success answer to a request only valid credentials
/// get: Allocate, Refresh, CreatePermission or ChannelBind.
pub(crate) fn authenticated_answer(packet: &[u8]) -> bool {
    matches!(kind(packet), Kind::Other)
        && packet.len() >= 2
        && matches!(
            u16::from_be_bytes([packet[0], packet[1]]),
            0x0103 | 0x0104 | 0x0108 | 0x0109
        )
}

/// STUN requests taken at once from sources not known yet (enough for a full room's calls
/// to start together), and how many more each second.
const REQUESTS_BURST: f64 = 256.0;
const REQUESTS_PER_SECOND: f64 = 8.0;

/// What is left of the requests the endpoint takes.
struct Requests {
    left: f64,
    at: Instant,
    burst: f64,
    per_second: f64,
}

impl Requests {
    fn new(burst: f64, per_second: f64) -> Self {
        Self {
            left: burst,
            at: Instant::now(),
            burst,
            per_second,
        }
    }

    fn take(&mut self, now: Instant) -> bool {
        let refill = now.saturating_duration_since(self.at).as_secs_f64() * self.per_second;
        self.left = (self.left + refill).min(self.burst);
        self.at = now;
        if self.left >= 1.0 {
            self.left -= 1.0;
            true
        } else {
            false
        }
    }
}

impl Endpoint {
    fn new(socket: tokio::net::UdpSocket, only_from: Option<IpAddr>) -> Self {
        Self {
            socket,
            only_from,
            requests: Mutex::new(Requests::new(REQUESTS_BURST, REQUESTS_PER_SECOND)),
            known: Mutex::new(HashMap::new()),
        }
    }

    /// Notes that the server answered `to` a request with valid credentials.
    fn answered(&self, to: SocketAddr, now: Instant) {
        let Ok(mut known) = self.known.lock() else {
            return;
        };
        if let Some(source) = known.get_mut(&to) {
            source.answered = now;
            return;
        }
        if known.len() >= KNOWN_MAX {
            known.retain(|_, k| now.saturating_duration_since(k.answered) < KNOWN_FOR);
        }
        if known.len() >= KNOWN_MAX
            && let Some(oldest) = known
                .iter()
                .min_by_key(|(_, k)| k.answered)
                .map(|(a, _)| *a)
        {
            known.remove(&oldest);
        }
        known.insert(
            to,
            Known {
                answered: now,
                media: Requests::new(MEDIA_BURST, MEDIA_PER_SECOND),
                requests: Requests::new(KNOWN_REQUESTS_BURST, KNOWN_REQUESTS_PER_SECOND),
            },
        );
    }

    /// Whether media from `src` goes on to the TURN server.
    fn take_media(&self, src: SocketAddr, now: Instant) -> bool {
        let Ok(mut known) = self.known.lock() else {
            return false;
        };
        let taken = match known.get_mut(&src) {
            Some(k) if now.saturating_duration_since(k.answered) < KNOWN_FOR => k.media.take(now),
            Some(_) => {
                known.remove(&src);
                false
            }
            None => false,
        };
        if !taken {
            tracing::trace!(%src, "TURN media dropped");
        }
        taken
    }

    /// Whether a request from `src` goes on to the TURN server: from its own share if the
    /// source is known, else from the share of all the others.
    fn take_request(&self, src: SocketAddr, now: Instant) -> bool {
        let own = self.known.lock().ok().and_then(|mut known| {
            known
                .get_mut(&src)
                .filter(|k| now.saturating_duration_since(k.answered) < KNOWN_FOR)
                .map(|k| k.requests.take(now))
        });
        let taken = match own {
            Some(taken) => taken,
            None => self.requests.lock().is_ok_and(|mut r| r.take(now)),
        };
        if !taken {
            tracing::debug!(%src, "too many TURN requests; one dropped");
        }
        taken
    }
}

/// What a packet sent to the endpoint is.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A STUN request (an allocation, a permission, a channel, a refresh).
    Request,
    /// Another STUN message (an indication carrying media), or channel data.
    Other,
    /// Something else, which the TURN server would only log.
    Neither,
}

/// Whether a packet is the answer to a connectivity check: a STUN Binding success
/// response (RFC 8489, message type 0x0101, with the magic cookie).
fn binding_answer(packet: &[u8]) -> bool {
    packet.len() >= 20 && packet[..2] == [0x01, 0x01] && packet[4..8] == [0x21, 0x12, 0xa4, 0x42]
}

pub(crate) fn kind(packet: &[u8]) -> Kind {
    const MAGIC_COOKIE: [u8; 4] = [0x21, 0x12, 0xa4, 0x42];
    match packet.first() {
        // Channel data: a channel number from 0x4000 to 0x4fff, then the length.
        Some(0x40..=0x4f) if packet.len() >= 4 => Kind::Other,
        Some(b) if b & 0xc0 == 0 && packet.len() >= 20 && packet[4..8] == MAGIC_COOKIE => {
            // The class is in two bits of the message type: both clear for a request.
            if u16::from_be_bytes([packet[0], packet[1]]) & 0x0110 == 0 {
                Kind::Request
            } else {
                Kind::Other
            }
        }
        _ => Kind::Neither,
    }
}

#[async_trait]
impl Conn for Endpoint {
    async fn connect(&self, addr: SocketAddr) -> webrtc_util::Result<()> {
        Ok(self.socket.connect(addr).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> webrtc_util::Result<usize> {
        Ok(self.recv_from(buf).await?.0)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> webrtc_util::Result<(usize, SocketAddr)> {
        loop {
            let (len, src) = match self.socket.recv_from(buf).await {
                Ok(got) => got,
                // The socket is fine, and an error would stop the TURN server for good.
                Err(e) if passing(&e) => {
                    tracing::debug!("TURN socket: {e}");
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            match self.only_from {
                Some(ip) if src.ip() != ip => {
                    tracing::debug!(%src, "TURN packet from another device dropped");
                }
                _ => match kind(&buf[..len]) {
                    Kind::Request if !self.take_request(src, Instant::now()) => {}
                    Kind::Other if !self.take_media(src, Instant::now()) => {}
                    Kind::Request | Kind::Other => return Ok((len, src)),
                    Kind::Neither => {}
                },
            }
        }
    }

    async fn send(&self, buf: &[u8]) -> webrtc_util::Result<usize> {
        Ok(self.socket.send(buf).await?)
    }

    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> webrtc_util::Result<usize> {
        if authenticated_answer(buf) {
            self.answered(target, Instant::now());
        }
        Ok(self.socket.send_to(buf, target).await?)
    }

    fn local_addr(&self) -> webrtc_util::Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        self.socket.peer_addr().ok()
    }

    async fn close(&self) -> webrtc_util::Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
}

/// Only the local WebRTC engine knows the credentials. Nothing outside this device reaches
/// the endpoint (see [`Endpoint`]); the engine sends from the address of one of the device's
/// interfaces.
///
/// The engine asks for an allocation through every interface it has (Wi-Fi, Ethernet, VPN,
/// virtual ones), and each becomes a candidate, so a call has as many candidate pairs as
/// both sides' interfaces multiplied. They all lead to the same member over the same QUIC
/// connection, yet the engine switches between them now and then, and every switch starts
/// its bandwidth estimate over. On a loopback endpoint only the interface the device's
/// default route goes through is answered, so each side offers one candidate and a call has
/// one pair; requests from the others are refused and fail at once. (Listening on the
/// device's own address, as on Windows, the endpoint hears that one only anyway.)
///
/// Only users that name a member ([`member_user`] of the bridge's own) are answered.
pub(crate) struct LocalAuth {
    username: String,
    password: String,
    one_interface: bool,
    /// The default route's interface as last found, and when: finding it takes a socket,
    /// and requests come by the dozen when calls start.
    default_route: Mutex<Option<(Instant, Option<Ipv4Addr>)>>,
}

/// How long the default route's interface, once found, is taken as it was.
const DEFAULT_ROUTE_FOR: Duration = Duration::from_secs(2);

impl LocalAuth {
    pub(crate) fn new(username: String, password: String, one_interface: bool) -> Self {
        Self {
            username,
            password,
            one_interface,
            default_route: Mutex::new(None),
        }
    }

    /// The default route's interface (see [`default_route_ipv4`]), found again once
    /// [`DEFAULT_ROUTE_FOR`] has passed.
    fn default_route(&self, now: Instant) -> Option<Ipv4Addr> {
        let Ok(mut found) = self.default_route.lock() else {
            return default_route_ipv4();
        };
        match *found {
            Some((at, route)) if now.saturating_duration_since(at) < DEFAULT_ROUTE_FOR => route,
            _ => {
                let route = default_route_ipv4();
                *found = Some((now, route));
                route
            }
        }
    }
}

impl AuthHandler for LocalAuth {
    fn auth_handle(
        &self,
        username: &str,
        _realm: &str,
        src: SocketAddr,
    ) -> Result<Vec<u8>, turn::Error> {
        if member_named(&self.username, username).is_none() {
            tracing::debug!(%src, username, "TURN request from an unknown user refused");
            return Err(turn::Error::Other("unknown user".into()));
        }
        if self.one_interface
            && !from_default_interface(src.ip(), self.default_route(Instant::now()))
        {
            tracing::debug!(%src, "TURN request through another interface refused");
            return Err(turn::Error::Other("another interface".into()));
        }
        Ok(generate_auth_key(username, REALM, &self.password))
    }
}

/// The address of this device's interface the default route goes through, found without
/// sending anything; `None` without an IPv4 default route.
pub fn default_route_ipv4() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Whether a request from `src` comes through the default route's interface (`default`),
/// or from this device's loopback; anything goes without a default route.
fn from_default_interface(src: IpAddr, default: Option<Ipv4Addr>) -> bool {
    match default {
        Some(default) => src.is_loopback() || src == IpAddr::V4(default),
        None => true,
    }
}

struct VirtualRelays {
    shared: Arc<Shared>,
    /// The bridge's own TURN user (see [`member_user`]).
    user: String,
}

#[async_trait]
impl RelayAddressGenerator for VirtualRelays {
    fn validate(&self) -> Result<(), turn::Error> {
        Ok(())
    }

    /// Every allocation is for a member ([`Self::allocate_conn_for`]).
    async fn allocate_conn(
        &self,
        _use_ipv4: bool,
        _requested_port: u16,
    ) -> Result<(Arc<dyn Conn + Send + Sync>, SocketAddr), turn::Error> {
        Err(turn::Error::Other("an allocation is for a member".into()))
    }

    async fn allocate_conn_for(
        &self,
        _use_ipv4: bool,
        _requested_port: u16,
        username: &str,
    ) -> Result<(Arc<dyn Conn + Send + Sync>, SocketAddr), turn::Error> {
        let member = member_named(&self.user, username)
            .ok_or_else(|| turn::Error::Other("an allocation is for a member".into()))?;
        let (tx, rx) = mpsc::channel(ALLOCATION_QUEUE);
        let port = {
            let mut allocations = self
                .shared
                .allocations
                .lock()
                .map_err(|_| turn::Error::Other("allocations unavailable".into()))?;
            let port = free_port(&allocations, &self.shared.next_port)
                .ok_or_else(|| turn::Error::Other("no relay port left".into()))?;
            allocations.insert(port, Allocation { member, queue: tx });
            port
        };
        let addr = SocketAddr::new(IpAddr::V4(relay_ip(self.shared.local_block)), port);
        Ok((
            Arc::new(VirtualRelayConn {
                shared: self.shared.clone(),
                member,
                addr,
                incoming: tokio::sync::Mutex::new(rx),
            }),
            addr,
        ))
    }
}

/// First relay port of the allocations.
const FIRST_PORT: u16 = 40000;

/// The next relay port no allocation holds, from `next` on, wrapping round to
/// [`FIRST_PORT`] after the last one: the engine makes a new allocation for each connection,
/// and someone in a call can make it open connections again and again.
fn free_port<T>(allocations: &HashMap<u16, T>, next: &AtomicU16) -> Option<u16> {
    // Called with the allocations locked, so nothing moves `next` meanwhile.
    for _ in FIRST_PORT..=u16::MAX {
        let port = next.load(Ordering::Relaxed).max(FIRST_PORT);
        next.store(port.checked_add(1).unwrap_or(FIRST_PORT), Ordering::Relaxed);
        if !allocations.contains_key(&port) {
            return Some(port);
        }
    }
    None
}

/// The "socket" of one allocation: sending goes to the member it is for, receiving yields
/// what that member sent to it.
struct VirtualRelayConn {
    shared: Arc<Shared>,
    member: Member,
    addr: SocketAddr,
    incoming: tokio::sync::Mutex<mpsc::Receiver<(Bytes, SocketAddr)>>,
}

#[async_trait]
impl Conn for VirtualRelayConn {
    async fn connect(&self, _addr: SocketAddr) -> webrtc_util::Result<()> {
        Err(webrtc_util::Error::Other("not supported".into()))
    }

    async fn recv(&self, buf: &mut [u8]) -> webrtc_util::Result<usize> {
        self.recv_from(buf).await.map(|(n, _)| n)
    }

    async fn recv_from(&self, buf: &mut [u8]) -> webrtc_util::Result<(usize, SocketAddr)> {
        let (payload, from) = self
            .incoming
            .lock()
            .await
            .recv()
            .await
            .ok_or(webrtc_util::Error::ErrClosedListener)?;
        self.shared.waiting.taken(self.addr.port(), from.ip());
        let n = payload.len().min(buf.len());
        buf[..n].copy_from_slice(&payload[..n]);
        Ok((n, from))
    }

    async fn send(&self, _buf: &[u8]) -> webrtc_util::Result<usize> {
        Err(webrtc_util::Error::Other("not connected".into()))
    }

    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> webrtc_util::Result<usize> {
        if let Some(file) = &self.shared.unanswered
            && binding_answer(buf)
            && file.exists()
        {
            return Ok(buf.len());
        }
        // Like UDP: a packet without a route or a path is silently dropped.
        let Some(peer) = self.shared.peer_for(&self.member, target) else {
            tracing::debug!(%target, "the member of this allocation is not connected");
            self.shared.refresh.notify_one();
            return Ok(buf.len());
        };
        let Some(packet) = encode(self.addr, target, buf) else {
            return Ok(buf.len());
        };
        self.shared
            .count(peer, |t| t.largest = t.largest.max(packet.len()));
        let link = if self.shared.streams_only {
            None
        } else {
            datagram::link(&peer)
        };
        let Some(link) = link else {
            self.shared.send_stream(peer, packet);
            return Ok(buf.len());
        };
        let Some(max) = link.max_size() else {
            self.shared.send_stream(peer, packet);
            return Ok(buf.len());
        };
        let takes_parts = self.shared.node.advertises(&peer, &PARTS_PROTOCOL);
        match way(packet.len(), max, takes_parts) {
            Way::Whole => self.shared.send_datagram(peer, &link, packet),
            Way::Parts => {
                let id = self.shared.next_split.fetch_add(1, Ordering::Relaxed);
                match crate::parts::split(self.addr, target, buf, max, id) {
                    Some(parts) => {
                        self.shared.count(peer, |t| t.split += 1);
                        for part in parts {
                            self.shared.send_datagram(peer, &link, part);
                        }
                    }
                    None => self.shared.send_stream(peer, packet),
                }
            }
            Way::Stream => self.shared.send_stream(peer, packet),
        }
        Ok(buf.len())
    }

    fn local_addr(&self) -> webrtc_util::Result<SocketAddr> {
        Ok(self.addr)
    }

    fn remote_addr(&self) -> Option<SocketAddr> {
        None
    }

    async fn close(&self) -> webrtc_util::Result<()> {
        if let Ok(mut allocations) = self.shared.allocations.lock() {
            allocations.remove(&self.addr.port());
        }
        self.shared.waiting.forget(self.addr.port());
        Ok(())
    }

    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_member_fills_only_its_share_of_an_allocation_s_queue() {
        let shares = Shares::default();
        let (tx, mut rx) = mpsc::channel::<IpAddr>(ALLOCATION_QUEUE);
        let (other, talking): (IpAddr, IpAddr) =
            ("10.9.9.1".parse().unwrap(), "10.1.2.1".parse().unwrap());
        // Someone in the group but not in the call sends as fast as it can.
        for _ in 0..ALLOCATION_QUEUE {
            shares.queue(40000, other, || tx.try_send(other).is_ok());
        }
        // The member the allocation talks to still finds room.
        for _ in 0..100 {
            shares.queue(40000, talking, || tx.try_send(talking).is_ok());
        }
        let mut got = Vec::new();
        while let Ok(ip) = rx.try_recv() {
            shares.taken(40000, ip);
            got.push(ip);
        }
        assert_eq!(got.iter().filter(|ip| **ip == other).count(), SOURCE_SHARE);
        assert_eq!(got.iter().filter(|ip| **ip == talking).count(), 100);
        // Read, the share is free again; another allocation has its own.
        shares.queue(40000, other, || tx.try_send(other).is_ok());
        shares.queue(40001, other, || true);
        assert_eq!(rx.try_recv().ok(), Some(other));
        shares.forget(40001);
        assert!(shares.0.lock().unwrap().keys().all(|(p, _)| *p == 40000));
    }

    #[test]
    fn header_round_trips() {
        let src: SocketAddr = "10.1.2.1:40000".parse().unwrap();
        let dst: SocketAddr = "10.3.4.1:40007".parse().unwrap();
        let packet = encode(src, dst, b"rtp").unwrap();
        let (s, d, p) = decode(&packet).unwrap();
        assert_eq!((s, d, &p[..]), (src, dst, &b"rtp"[..]));
        assert!(decode(&Bytes::from_static(b"short")).is_none());
    }

    #[test]
    fn large_packets_go_in_parts_only_to_bridges_that_take_them() {
        // A full video packet, with QUIC at its smallest packet.
        assert_eq!(way(1222, 1162, true), Way::Parts);
        assert_eq!(way(1222, 1162, false), Way::Stream);
        // What fits goes whole either way.
        assert_eq!(way(1162, 1162, true), Way::Whole);
        assert_eq!(way(200, 1162, false), Way::Whole);
    }

    #[test]
    fn requests_come_through_the_default_interface_only() {
        let default = Some(Ipv4Addr::new(192, 168, 1, 11));
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(from_default_interface(ip("192.168.1.11"), default));
        assert!(from_default_interface(ip("127.0.0.1"), default));
        assert!(!from_default_interface(ip("10.201.0.11"), default));
        assert!(!from_default_interface(ip("172.17.0.1"), default));
        // Without a default route nothing tells which one, and every interface is answered.
        assert!(from_default_interface(ip("10.201.0.11"), None));
    }

    #[test]
    fn a_packet_too_long_or_a_closed_port_does_not_stop_the_endpoint() {
        use std::io::{Error, ErrorKind};
        assert!(passing(&Error::from(ErrorKind::ConnectionReset)));
        assert!(passing(&Error::from_raw_os_error(MESSAGE_TOO_LONG)));
        assert!(!passing(&Error::from(ErrorKind::NotConnected)));
    }

    /// A STUN Allocate request without credentials, as any program could send.
    fn allocate(id: u8) -> Vec<u8> {
        let mut packet = vec![0x00, 0x03, 0x00, 0x00, 0x21, 0x12, 0xa4, 0x42];
        packet.extend([id; 12]);
        packet
    }

    #[test]
    fn packets_are_told_apart() {
        assert_eq!(kind(&allocate(1)), Kind::Request);
        // A Send indication, channel data, and what is neither.
        let mut send = allocate(2);
        send[1] = 0x16;
        assert_eq!(kind(&send), Kind::Other);
        assert_eq!(kind(&[0x40, 0x00, 0x00, 0x04, 1, 2, 3, 4]), Kind::Other);
        assert_eq!(kind(b"hello, is anyone there?"), Kind::Neither);
        assert_eq!(kind(&allocate(3)[..19]), Kind::Neither);
        assert_eq!(kind(&[]), Kind::Neither);
    }

    #[test]
    fn answers_to_connectivity_checks_are_told_apart() {
        let mut answer = allocate(4);
        answer[..2].copy_from_slice(&[0x01, 0x01]);
        assert!(binding_answer(&answer));
        let mut check = allocate(5);
        check[1] = 0x01;
        assert!(!binding_answer(&check));
        assert!(!binding_answer(&answer[..19]));
        // RTP and DTLS, which still go out.
        assert!(!binding_answer(&[0x80; 40]));
        assert!(!binding_answer(&[0x17; 40]));
    }

    #[tokio::test]
    async fn a_flood_of_requests_reaches_the_turn_server_only_in_part() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        let mut endpoint = Endpoint::new(socket, None);
        endpoint.requests = Mutex::new(Requests::new(8.0, 1.0));
        let other = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        for i in 0..50u8 {
            other.send_to(&allocate(i), addr).unwrap();
            other.send_to(b"not stun at all", addr).unwrap();
        }
        let mut buf = [0u8; 1500];
        let mut passed = 0;
        while let Ok(Ok(_)) =
            tokio::time::timeout(Duration::from_millis(300), endpoint.recv_from(&mut buf)).await
        {
            passed += 1;
        }
        assert!((8..=10).contains(&passed), "{passed} of 100 packets passed");
    }

    #[test]
    fn only_answers_to_valid_credentials_make_a_source_known() {
        let mut answer = allocate(1);
        for (kind, known) in [
            (0x0103, true),  // Allocate success
            (0x0104, true),  // Refresh success
            (0x0108, true),  // CreatePermission success
            (0x0109, true),  // ChannelBind success
            (0x0101, false), // Binding success: no credentials needed
            (0x0113, false), // Allocate error
            (0x0003, false), // Allocate request
            (0x0016, false), // Send indication
        ] {
            answer[..2].copy_from_slice(&u16::to_be_bytes(kind));
            assert_eq!(authenticated_answer(&answer), known, "{kind:#06x}");
        }
        assert!(!authenticated_answer(&[0x01, 0x03]));
    }

    /// The sources of what the endpoint lets through within 200 ms of quiet.
    async fn passed(endpoint: &Endpoint) -> Vec<SocketAddr> {
        let mut buf = [0u8; 1500];
        let mut from = Vec::new();
        while let Ok(Ok((_, src))) =
            tokio::time::timeout(Duration::from_millis(200), endpoint.recv_from(&mut buf)).await
        {
            from.push(src);
        }
        from
    }

    #[tokio::test]
    async fn media_goes_on_only_from_a_known_source_and_only_so_much() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        let endpoint = Endpoint::new(socket, None);
        let engine = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let stranger = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let channel_data = [0x40, 0x00, 0x00, 0x04, 1, 2, 3, 4];
        // Before any answer, from nobody.
        engine.send_to(&channel_data, addr).unwrap();
        assert!(passed(&endpoint).await.is_empty());
        // Once the engine's source got an allocation, from it only.
        endpoint.answered(engine.local_addr().unwrap(), Instant::now());
        for _ in 0..5 {
            engine.send_to(&channel_data, addr).unwrap();
            stranger.send_to(&channel_data, addr).unwrap();
        }
        let from = passed(&endpoint).await;
        assert_eq!(from.len(), 5);
        assert!(from.iter().all(|src| *src == engine.local_addr().unwrap()));

        // Only so much from one source, and nothing once it was not answered for long.
        let src: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let start = Instant::now();
        endpoint.answered(src, start);
        let taken = (0..MEDIA_BURST as usize + 100)
            .filter(|_| endpoint.take_media(src, start))
            .count();
        assert_eq!(taken, MEDIA_BURST as usize);
        let later = start + Duration::from_secs(1);
        assert!(endpoint.take_media(src, later));
        assert!(!endpoint.take_media(src, start + KNOWN_FOR));
        assert!(!endpoint.take_media(src, start + KNOWN_FOR + Duration::from_secs(1)));
    }

    #[test]
    fn known_sources_are_bounded() {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.set_nonblocking(true).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();
        let endpoint = Endpoint::new(tokio::net::UdpSocket::from_std(socket).unwrap(), None);
        let start = Instant::now();
        for i in 0..(KNOWN_MAX as u16 + 10) {
            let src = SocketAddr::from(([127, 0, 0, 1], 10_000 + i));
            endpoint.answered(src, start + Duration::from_millis(u64::from(i)));
        }
        let known = endpoint.known.lock().unwrap();
        assert_eq!(known.len(), KNOWN_MAX);
        // The oldest went first.
        assert!(!known.contains_key(&SocketAddr::from(([127, 0, 0, 1], 10_000))));
        assert!(known.contains_key(&SocketAddr::from((
            [127, 0, 0, 1],
            10_000 + KNOWN_MAX as u16 + 9
        ))));
    }

    /// A member's stream, as its bridge writes it: chunks of bytes as they are sent.
    struct Written(mpsc::UnboundedReceiver<Vec<u8>>, Vec<u8>);

    impl futures::AsyncRead for Written {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            while self.1.is_empty() {
                match self.0.poll_recv(cx) {
                    std::task::Poll::Ready(Some(chunk)) => self.1 = chunk,
                    std::task::Poll::Ready(None) => return std::task::Poll::Ready(Ok(0)),
                    std::task::Poll::Pending => return std::task::Poll::Pending,
                }
            }
            let n = buf.len().min(self.1.len());
            buf[..n].copy_from_slice(&self.1[..n]);
            self.1.drain(..n);
            std::task::Poll::Ready(Ok(n))
        }
    }

    const QUICK: StreamTimes = StreamTimes {
        quiet: Duration::from_millis(150),
        packet: Duration::from_millis(100),
        check: Duration::from_millis(20),
    };

    fn written() -> (mpsc::UnboundedSender<Vec<u8>>, Written) {
        let (tx, rx) = mpsc::unbounded_channel();
        (tx, Written(rx, Vec::new()))
    }

    fn framed(payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u16).to_be_bytes().to_vec();
        out.extend_from_slice(payload);
        out
    }

    /// A member's stream that sends nothing, or stops half way through a length or a packet,
    /// is closed, and so is one whose sender stops being a member; meanwhile another
    /// member's stream carries its packets as ever.
    #[tokio::test]
    async fn streams_that_stall_or_whose_member_leaves_are_closed() {
        let (_silent_tx, silent) = written();
        let (half_head_tx, half_head) = written();
        half_head_tx.send(vec![0]).unwrap();
        let (half_body_tx, half_body) = written();
        half_body_tx.send(vec![0, 10, 1, 2, 3]).unwrap();
        let (_gone_tx, gone) = written();
        let (long_tx, long) = written();
        long_tx
            .send(((MAX_PACKET + 1) as u16).to_be_bytes().to_vec())
            .unwrap();
        let member = std::sync::atomic::AtomicBool::new(true);
        let (call_tx, call) = written();
        let mut got = Vec::new();
        let talking = async {
            for i in 0..20u8 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                // A packet in two writes, as a stream may carry it.
                let packet = framed(&[i; 300]);
                call_tx.send(packet[..100].to_vec()).unwrap();
                call_tx.send(packet[100..].to_vec()).unwrap();
            }
            drop(call_tx);
        };
        let leaving = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            member.store(false, Ordering::Relaxed);
        };
        let started = Instant::now();
        let (ends, ..) = tokio::join!(
            async {
                tokio::join!(
                    read_packets(silent, &QUICK, || true, |_| panic!("nothing was sent")),
                    read_packets(half_head, &QUICK, || true, |_| panic!("half a length")),
                    read_packets(half_body, &QUICK, || true, |_| panic!("half a packet")),
                    read_packets(gone, &QUICK, || member.load(Ordering::Relaxed), |_| {}),
                    read_packets(long, &QUICK, || true, |_| panic!("too long")),
                    read_packets(call, &QUICK, || true, |p| got.push(p)),
                )
            },
            talking,
            leaving,
        );
        assert_eq!(ends.0, StreamEnd::Quiet);
        assert_eq!(ends.1, StreamEnd::Stalled);
        assert_eq!(ends.2, StreamEnd::Stalled);
        assert_eq!(ends.3, StreamEnd::NotMember);
        assert_eq!(ends.4, StreamEnd::TooLong);
        assert_eq!(ends.5, StreamEnd::Closed);
        assert_eq!(got.len(), 20);
        assert!(got.iter().enumerate().all(|(i, p)| p[..] == [i as u8; 300]));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    /// Each member takes few streams at once, and all of them together a few more; one
    /// that ends gives its place back.
    #[test]
    fn streams_from_members_have_a_budget() {
        let streams = StreamsIn::default();
        let (ana, bea): (Member, Member) = ([1; 32], [2; 32]);
        for _ in 0..STREAMS_PER_MEMBER {
            assert!(streams.take(ana));
        }
        assert!(!streams.take(ana));
        assert!(streams.take(bea));
        streams.give_back(ana);
        assert!(streams.take(ana));
        let mut others = 0;
        for i in 3..=u8::MAX {
            if streams.take([i; 32]) {
                others += 1;
            }
        }
        assert_eq!(others, STREAMS_IN - STREAMS_PER_MEMBER - 1);
        // Refused members leave nothing behind: Ana, Bea and the others taken.
        assert_eq!(streams.0.lock().unwrap().0.len(), 2 + others);
    }

    #[test]
    fn turn_users_name_the_member_of_the_connection() {
        let ana: Member = std::array::from_fn(|i| i as u8 * 7);
        let user = member_user("x9", &ana);
        assert_eq!(user.len(), 2 + 1 + 64);
        assert!(user.starts_with("x9.00070e15"), "{user}");
        assert_eq!(member_named("x9", &user), Some(ana));
        // The bridge's own user alone, another's, or a key cut short or written otherwise
        // names nobody.
        for other in [
            "x9".to_string(),
            "x9.".to_string(),
            member_user("x8", &ana),
            member_user("x9x", &ana),
            user[..user.len() - 2].to_string(),
            format!("{user}00"),
            user.to_uppercase().replacen("X9", "x9", 1),
            user.replacen('.', ":", 1),
        ] {
            assert_eq!(member_named("x9", &other), None, "{other}");
        }
    }

    #[test]
    fn only_users_that_name_a_member_are_answered() {
        let auth = LocalAuth::new("x9".into(), "secret".into(), false);
        let user = member_user("x9", &[3; 32]);
        let src: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert_eq!(
            auth.auth_handle(&user, REALM, src).unwrap(),
            generate_auth_key(&user, REALM, "secret")
        );
        assert!(auth.auth_handle("x9", REALM, src).is_err());
        assert!(
            auth.auth_handle(&member_user("x8", &[3; 32]), REALM, src)
                .is_err()
        );
    }

    /// A member can take a transport identity in another's block (some 16 million tries):
    /// what goes to each still goes to it alone, and each one's packets reach only the
    /// allocations meant for it.
    #[test]
    fn two_members_in_one_block_each_get_only_their_own() {
        let (ana, mallory): (Member, Member) = ([1; 32], [2; 32]);
        let (to_ana, to_mallory) = (PeerId::random(), PeerId::random());
        let block = block_of(&to_ana);
        let at = SocketAddr::new(IpAddr::V4(relay_ip(block)), 40000);
        let connected = |_: &PeerId| true;
        assert_eq!(pick(&[(to_ana, block)], at, connected), Some(to_ana));
        assert_eq!(
            pick(&[(to_mallory, block)], at, connected),
            Some(to_mallory)
        );
        let (ana_queue, _ana_rx) = mpsc::channel(1);
        let (mallory_queue, _mallory_rx) = mpsc::channel(1);
        let allocations = HashMap::from([
            (
                40000,
                Allocation {
                    member: ana,
                    queue: ana_queue.clone(),
                },
            ),
            (
                40001,
                Allocation {
                    member: mallory,
                    queue: mallory_queue,
                },
            ),
        ]);
        assert!(
            queue_for(&allocations, 40000, Some(ana)).is_some_and(|q| q.same_channel(&ana_queue))
        );
        assert!(queue_for(&allocations, 40000, Some(mallory)).is_none());
        assert!(queue_for(&allocations, 40001, Some(ana)).is_none());
        // Nor someone who is no member, nor a port without an allocation.
        assert!(queue_for(&allocations, 40000, None).is_none());
        assert!(queue_for(&allocations, 40002, Some(ana)).is_none());
    }

    /// A member connected under several transport identities (a new one after its device
    /// started again, the old connection not timed out yet) gets packets on the one whose
    /// address its engine gave, else on one still connected.
    #[test]
    fn a_member_s_packets_go_to_the_identity_its_engine_named() {
        let (old, new) = (PeerId::random(), PeerId::random());
        let peers = [(old, block_of(&old)), (new, block_of(&new))];
        let to = |peer: &PeerId| SocketAddr::new(IpAddr::V4(relay_address(peer)), 40000);
        assert_eq!(pick(&peers, to(&new), |_| true), Some(new));
        assert_eq!(pick(&peers, to(&old), |_| true), Some(old));
        let elsewhere: SocketAddr = "10.0.0.1:40000".parse().unwrap();
        assert_eq!(pick(&peers, elsewhere, |_| true), Some(old));
        assert_eq!(pick(&peers, to(&old), |p| *p == new), Some(new));
        assert_eq!(pick(&peers, to(&old), |_| false), None);
        assert_eq!(pick(&[], to(&old), |_| true), None);
    }

    #[test]
    fn relay_addresses_take_24_bits_of_the_identity() {
        let peers: Vec<PeerId> = (0..200).map(|_| PeerId::random()).collect();
        let last: std::collections::HashSet<u8> =
            peers.iter().map(|p| relay_address(p).octets()[3]).collect();
        // The last byte is part of it, never a network's own (0) or broadcast (255) one.
        assert!(last.len() > 100, "{} last bytes", last.len());
        assert!(!last.contains(&0) && !last.contains(&255));
        for peer in &peers {
            let ip = relay_address(peer);
            assert_eq!(ip.octets()[0], 10);
            assert_eq!(&ip.octets()[1..], &block_of(peer)[..]);
        }
    }

    #[test]
    fn only_packets_for_this_device_s_relay_address_reach_the_engine() {
        let block = block_of(&PeerId::random());
        let mine = SocketAddr::new(IpAddr::V4(relay_ip(block)), 40001);
        assert!(to_relay(block, mine));
        // Another address with the port of one of its allocations: not this device's.
        let [a, b, c] = block;
        let other = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, a, b, c ^ 1)), 40001);
        assert!(!to_relay(block, other));
        assert!(!to_relay(block, "127.0.0.1:40001".parse().unwrap()));
    }

    #[test]
    fn the_default_route_is_found_again_only_after_a_while() {
        let auth = LocalAuth::new("user".into(), "pass".into(), true);
        let start = Instant::now();
        let first = auth.default_route(start);
        let found_at = |auth: &LocalAuth| auth.default_route.lock().unwrap().map(|(at, _)| at);
        assert_eq!(found_at(&auth), Some(start));
        assert_eq!(auth.default_route(start + Duration::from_secs(1)), first);
        assert_eq!(found_at(&auth), Some(start));
        let later = start + DEFAULT_ROUTE_FOR;
        auth.default_route(later);
        assert_eq!(found_at(&auth), Some(later));
    }

    #[tokio::test]
    async fn a_known_source_s_requests_go_through_a_flood_from_others() {
        let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let endpoint = Endpoint::new(socket, None);
        let engine: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let start = Instant::now();
        endpoint.answered(engine, start);
        // Some program sends request after request, from one port and from many.
        let flood = (0..2000u16)
            .filter(|i| {
                let src = SocketAddr::from(([127, 0, 0, 1], 20_000 + i % 50));
                endpoint.take_request(src, start)
            })
            .count();
        assert_eq!(flood, REQUESTS_BURST as usize);
        // The engine's socket still refreshes what it holds, up to its own share.
        let own = (0..100)
            .filter(|_| endpoint.take_request(engine, start))
            .count();
        assert_eq!(own, KNOWN_REQUESTS_BURST as usize);
        assert!(endpoint.take_request(engine, start + Duration::from_secs(1)));
    }

    #[test]
    fn relay_ports_go_round_and_skip_those_in_use() {
        let next = AtomicU16::new(u16::MAX - 1);
        let mut held: HashMap<u16, ()> = HashMap::from([(u16::MAX, ()), (FIRST_PORT, ())]);
        assert_eq!(free_port(&held, &next), Some(u16::MAX - 1));
        assert_eq!(free_port(&held, &next), Some(FIRST_PORT + 1));
        held.extend((FIRST_PORT..=u16::MAX).map(|p| (p, ())));
        assert_eq!(free_port(&held, &next), None);
    }

    #[test]
    fn blocks_are_stable_per_peer() {
        let peer = PeerId::random();
        assert_eq!(block_of(&peer), block_of(&peer));
        assert_eq!(relay_address(&peer), relay_address(&peer));
    }
}
