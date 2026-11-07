//! Fogata addition: access to QUIC datagrams (RFC 9221) of established connections.
//!
//! libp2p's muxer interface only carries reliable streams. Real-time media needs packets that
//! may be lost instead of delaying the ones behind them, so every established connection is
//! registered here by remote peer, and datagrams received on any of them are forwarded to the
//! application ([`receive`]).

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use bytes::Bytes;
use futures::{Stream, channel::mpsc};
use libp2p_identity::PeerId;
pub use quinn::SendDatagramError;

/// A connection that received nothing for this long is not taken as the live one while a
/// newer one is there: after a network change the old one lingers until its idle timeout.
const QUIET_MS: u64 = 2000;

/// Datagrams of one peer waiting in the application's queue at most: past it, that peer's
/// are dropped while the others' still go in, so one peer cannot fill the queue for all.
pub const PEER_SHARE: usize = 1024;

/// Which peers' datagrams the application takes; the others are dropped before they queue.
type Accept = Arc<dyn Fn(&PeerId) -> bool + Send + Sync>;

/// Where received datagrams go. A bounded queue: datagrams come from any connection, and
/// when the application falls behind, they are dropped as a congested network would.
struct Sink {
    sender: mpsc::Sender<(PeerId, Bytes)>,
    accept: Accept,
    /// Datagrams of each peer in the queue now.
    queued: HashMap<PeerId, usize>,
    /// Which [`receive`] this sink belongs to: a queue it replaced may still hand out
    /// datagrams, which must not count against the new one.
    generation: u64,
}

#[derive(Default)]
struct Registry {
    /// Each peer's connections, oldest first, with when each last received a datagram.
    links: HashMap<PeerId, Vec<(quinn::Connection, Arc<AtomicU64>)>>,
    /// What each connection had received when last looked at (see [`silent_for`]).
    heard_any: HashMap<usize, Heard>,
    sink: Option<Sink>,
    generations: u64,
}

/// Milliseconds since this module was first used, for comparing when links last heard.
fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}

/// Receives the datagrams of every connection, tagged with the remote peer, from the peers
/// `accept` takes: any peer that completes a handshake can send datagrams, and those of
/// strangers must not take the places of the peers the application talks to. At most
/// `capacity` wait, [`PEER_SHARE`] of them from one peer. A later call replaces the queue.
pub fn receive(
    capacity: usize,
    accept: impl Fn(&PeerId) -> bool + Send + Sync + 'static,
) -> Received {
    let (sender, receiver) = mpsc::channel(capacity);
    let mut generation = 0;
    if let Ok(mut r) = registry().lock() {
        r.generations += 1;
        generation = r.generations;
        r.sink = Some(Sink {
            sender,
            accept: Arc::new(accept),
            queued: HashMap::new(),
            generation,
        });
    }
    Received {
        receiver,
        generation,
    }
}

/// The datagrams [`receive`] takes, as `(peer, datagram)`. Dropped, it lets go of the
/// registry's sink, with the `accept` it was given, unless a later [`receive`] replaced it:
/// whatever `accept` holds is not kept for the rest of the process.
pub struct Received {
    receiver: mpsc::Receiver<(PeerId, Bytes)>,
    generation: u64,
}

impl Drop for Received {
    fn drop(&mut self) {
        let sink = registry().lock().ok().and_then(|mut r| {
            r.sink
                .take_if(|s| s.generation == self.generation)
        });
        // Dropped once the lock is let go: `accept` may hold anything.
        drop(sink);
    }
}

impl Stream for Received {
    type Item = (PeerId, Bytes);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let polled = std::pin::Pin::new(&mut self.receiver).poll_next(cx);
        if let std::task::Poll::Ready(Some((peer, _))) = &polled
            && let Ok(mut r) = registry().lock()
            && let Some(sink) = r.sink.as_mut().filter(|s| s.generation == self.generation)
            && let Some(queued) = sink.queued.get_mut(peer)
        {
            *queued -= 1;
            if *queued == 0 {
                sink.queued.remove(peer);
            }
        }
        polled
    }
}

pub(crate) fn register(peer: PeerId, connection: quinn::Connection) {
    let heard = Arc::new(AtomicU64::new(0));
    if let Ok(mut r) = registry().lock() {
        let links = r.links.entry(peer).or_default();
        links.retain(|(c, _)| c.close_reason().is_none());
        links.push((connection.clone(), heard.clone()));
    }
    #[cfg(feature = "tokio")]
    tokio::spawn(async move {
        while let Ok(datagram) = connection.read_datagram().await {
            heard.store(now_ms(), Ordering::Relaxed);
            hand_over(peer, datagram);
        }
        forget_closed(&peer);
    });
}

/// Passes a received datagram to the application, or drops it: from a peer it does not take,
/// past the peer's share, or when the queue is full. The one sender kept in the registry is
/// used, never a copy: each copy would bring a free place of its own in the queue.
fn hand_over(peer: PeerId, datagram: Bytes) {
    // Asked without the registry locked: the application answers from locks of its own.
    let Some(accept) = registry()
        .lock()
        .ok()
        .and_then(|r| r.sink.as_ref().map(|s| s.accept.clone()))
    else {
        return;
    };
    if !accept(&peer) {
        return;
    }
    if let Ok(mut r) = registry().lock()
        && let Some(sink) = r.sink.as_mut()
    {
        let queued = sink.queued.entry(peer).or_default();
        if *queued < PEER_SHARE && sink.sender.try_send((peer, datagram)).is_ok() {
            *queued += 1;
        } else if *queued == 0 {
            sink.queued.remove(&peer);
        }
    }
}

/// Lets go of `peer`'s closed connections, and of the peer once none is left, so that the
/// connections of peers never seen again (DHT nodes, strangers) do not pile up.
fn forget_closed(peer: &PeerId) {
    if let Ok(mut r) = registry().lock()
        && let Some(links) = r.links.get_mut(peer)
    {
        links.retain(|(c, _)| c.close_reason().is_none());
        if links.is_empty() {
            r.links.remove(peer);
        }
        let live: std::collections::HashSet<usize> = r
            .links
            .values()
            .flatten()
            .map(|(c, _)| c.stable_id())
            .collect();
        r.heard_any.retain(|id, _| live.contains(id));
    }
}

/// How long the open connections to `peer` have received no packet at all, for the one
/// that received last; `None` without one. Keep-alives every few seconds make a live
/// connection hear something at least that often, so a long silence means the way to the
/// peer is gone, long before the idle timeout closes the connection. Measured between
/// calls: a connection counts as heard when its count of received packets moved since the
/// previous call.
pub fn silent_for(peer: &PeerId) -> Option<std::time::Duration> {
    let mut r = registry().lock().ok()?;
    let now = now_ms();
    let connections: Vec<quinn::Connection> = r
        .links
        .get(peer)?
        .iter()
        .filter(|(c, _)| c.close_reason().is_none())
        .map(|(c, _)| c.clone())
        .collect();
    connections
        .iter()
        .map(|c| {
            let received = c.stats().udp_rx.datagrams;
            r.heard_any
                .entry(c.stable_id())
                .or_insert(Heard {
                    count: received,
                    since: now,
                })
                .silence(received, now)
        })
        .min()
        .map(std::time::Duration::from_millis)
}

/// A connection's count of received packets when last looked at, and since when it has
/// stood there.
#[derive(Clone, Copy, Debug)]
struct Heard {
    count: u64,
    since: u64,
}

impl Heard {
    /// Takes the count now; returns for how many milliseconds it has not moved.
    fn silence(&mut self, count: u64, now: u64) -> u64 {
        if count != self.count {
            self.count = count;
            self.since = now;
        }
        now.saturating_sub(self.since)
    }
}

/// A live connection to `peer` that can carry datagrams, if any: the one that received a
/// datagram last, unless that was a while ago, and otherwise the newest.
pub fn link(peer: &PeerId) -> Option<DatagramLink> {
    let mut r = registry().lock().ok()?;
    let links = r.links.get_mut(peer)?;
    links.retain(|(c, _)| c.close_reason().is_none());
    let now = now_ms();
    links
        .iter()
        .enumerate()
        .filter(|(_, (c, _))| c.max_datagram_size().is_some())
        .max_by_key(|(i, (_, heard))| {
            let heard = heard.load(Ordering::Relaxed);
            let recent = if heard > 0 && now.saturating_sub(heard) < QUIET_MS {
                heard
            } else {
                0
            };
            (recent, *i)
        })
        .map(|(_, (c, _))| DatagramLink(c.clone()))
}

/// Sends unreliable datagrams to one peer.
#[derive(Clone, Debug)]
pub struct DatagramLink(quinn::Connection);

impl DatagramLink {
    /// Largest datagram that currently fits in one packet.
    pub fn max_size(&self) -> Option<usize> {
        self.0.max_datagram_size()
    }

    /// Queues a datagram. It may be dropped if the path is congested or the packet is lost.
    pub fn send(&self, data: Bytes) -> Result<(), SendDatagramError> {
        self.0.send_datagram(data)
    }

    /// How the connection's path is doing, for logs.
    pub fn path(&self) -> PathState {
        let path = self.0.stats().path;
        PathState {
            rtt: path.rtt,
            cwnd: path.cwnd,
            mtu: path.current_mtu,
            sent_packets: path.sent_packets,
            lost_packets: path.lost_packets,
            congestion_events: path.congestion_events,
            black_holes: path.black_holes_detected,
        }
    }
}

/// A connection's path as QUIC sees it; the counts are since the connection began.
#[derive(Clone, Copy, Debug)]
pub struct PathState {
    pub rtt: std::time::Duration,
    pub cwnd: u64,
    /// Largest UDP payload the path takes now.
    pub mtu: u16,
    pub sent_packets: u64,
    pub lost_packets: u64,
    pub congestion_events: u64,
    /// Times QUIC took losses for a smaller path and fell back to its smallest packets.
    pub black_holes: u64,
}

#[cfg(test)]
mod tests {
    use futures::{FutureExt, StreamExt};

    use super::*;

    #[test]
    fn a_connection_is_silent_from_the_last_time_its_count_moved() {
        let mut heard = Heard {
            count: 10,
            since: 1_000,
        };
        assert_eq!(heard.silence(10, 3_000), 2_000);
        assert_eq!(heard.silence(11, 5_000), 0);
        assert_eq!(heard.silence(11, 9_000), 4_000);
        assert_eq!(heard.silence(11, 16_000), 11_000);
        assert_eq!(heard.silence(12, 16_500), 0);
    }

    /// The queue is one for the whole process: tests that use it take turns.
    fn queue_to_self() -> std::sync::MutexGuard<'static, ()> {
        static TURN: Mutex<()> = Mutex::new(());
        TURN.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What is in the queue now, by peer.
    fn drain(received: &mut Received) -> HashMap<PeerId, usize> {
        let mut got = HashMap::new();
        while let Some(Some((peer, _))) = received.next().now_or_never() {
            *got.entry(peer).or_default() += 1;
        }
        got
    }

    #[test]
    fn datagrams_the_application_does_not_take_are_dropped() {
        let _turn = queue_to_self();
        let mut received = receive(16, |_| true);
        let peer = PeerId::random();
        for _ in 0..1000 {
            hand_over(peer, Bytes::from_static(b"rtp"));
        }
        let count = drain(&mut received).get(&peer).copied().unwrap_or(0);
        assert!(count <= 17, "{count} datagrams queued");
        // Taken, the queue has room again.
        hand_over(peer, Bytes::from_static(b"rtp"));
        assert!(futures::executor::block_on(received.next()).is_some());
    }

    #[test]
    fn a_flood_from_a_stranger_or_one_peer_leaves_room_for_the_others() {
        let _turn = queue_to_self();
        let (member, other, stranger) = (PeerId::random(), PeerId::random(), PeerId::random());
        let mut received = receive(4096, move |p| *p != stranger);
        // A stranger floods: none of it queues, and a member's packets all go in.
        for _ in 0..10_000 {
            hand_over(stranger, Bytes::from_static(b"junk"));
        }
        for _ in 0..100 {
            hand_over(member, Bytes::from_static(b"rtp"));
        }
        let got = drain(&mut received);
        assert_eq!(got.get(&stranger), None);
        assert_eq!(got.get(&member), Some(&100));
        // A peer the application takes floods while nothing is read: it fills its share only,
        // and the next peer's packets all go in.
        for _ in 0..10_000 {
            hand_over(member, Bytes::from_static(b"rtp"));
        }
        for _ in 0..100 {
            hand_over(other, Bytes::from_static(b"rtp"));
        }
        let got = drain(&mut received);
        assert_eq!(got.get(&member), Some(&PEER_SHARE));
        assert_eq!(got.get(&other), Some(&100));
        // Read, its share is free again.
        for _ in 0..10 {
            hand_over(member, Bytes::from_static(b"rtp"));
        }
        assert_eq!(drain(&mut received).get(&member), Some(&10));
    }

    /// The queue dropped lets go of what its `accept` holds (an application's handle to
    /// its node, say), but not of a later queue's.
    #[test]
    fn a_queue_dropped_lets_go_of_its_accept() {
        let _turn = queue_to_self();
        let held = Arc::new(());
        let first = {
            let held = held.clone();
            receive(16, move |_| Arc::strong_count(&held) > 0)
        };
        assert_eq!(Arc::strong_count(&held), 2);
        drop(first);
        assert_eq!(Arc::strong_count(&held), 1);
        // A queue replaced and then dropped leaves the one that replaced it in place.
        let replaced = receive(16, |_| true);
        let later = {
            let held = held.clone();
            receive(16, move |_| Arc::strong_count(&held) > 0)
        };
        drop(replaced);
        assert_eq!(Arc::strong_count(&held), 2);
        let mut later = later;
        let peer = PeerId::random();
        hand_over(peer, Bytes::from_static(b"rtp"));
        assert_eq!(drain(&mut later).get(&peer), Some(&1));
        drop(later);
        assert_eq!(Arc::strong_count(&held), 1);
    }
}
