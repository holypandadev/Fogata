//! Typing indicators: "Ana is typing…" under a chat. A signal says that someone is writing
//! in a chat now, or stopped; it is never kept, never part of a group's log or a room's
//! events, and goes straight to the people of that chat who are connected now, each over a
//! stream of its own. Nothing forwards it.
//!
//! - In a direct chat it goes to the other person, connected as a member of the chat's
//!   group; in a room's chat, to the members connected now who are in the same voice room.
//!   A device that appears offline sends none.
//! - Only devices that take [`TYPING_PROTOCOL`] get one: they say so when they connect
//!   (identify), so older devices are never sent anything they would not know.
//! - What arrives is taken only from a peer that proved membership of the chat's group and
//!   is still a member of it, about a chat this device is in (the same voice room), at most
//!   a few signals at once and one a second after that from each peer, and at most
//!   [`MAX_SIGNAL_BYTES`] of each stream. A signal older than the last one heard from that
//!   peer in that chat (they may overtake each other) is dropped.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use fogata_crypto::GroupId;
use fogata_state::VoiceRoomId;
use futures::{AsyncReadExt as _, AsyncWriteExt as _};
use libp2p::{PeerId, StreamProtocol};
use tokio::sync::mpsc;

use super::Node;
use crate::{NodeEvent, behaviour::VerifiedPeers};

/// The stream protocol of typing signals.
pub const TYPING_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/typing/1");

/// Bytes of a signal of this version.
pub const SIGNAL_BYTES: usize = 30;
/// Most bytes read from one stream; a later version may add to the end of a signal.
pub const MAX_SIGNAL_BYTES: usize = 64;
const VERSION: u8 = 1;

/// How long a stream may take to open, send or arrive.
const STREAM_TIME: Duration = Duration::from_secs(5);
/// Streams being read at once, in all and from one peer; more are closed unread.
const MAX_READING: usize = 32;
const MAX_READING_PER_PEER: usize = 2;
/// Signals taken from one peer at once, and how often one more is allowed after that.
const BURST: f64 = 4.0;
const REFILL_PER_SECOND: f64 = 1.0;
/// Signals read and waiting for the node; more are dropped.
const QUEUE: usize = 64;
/// "Still typing" goes at most this often for one chat, whatever the application asks.
const SEND_GAP: Duration = Duration::from_secs(2);
/// A chat this device said it was typing in gets "stopped" only within this long; later,
/// the others have stopped showing it already.
const STOP_WITHIN: Duration = Duration::from_secs(10);

/// One signal: the chat (its group and, for a room's chat, the voice room), whether the
/// sender is typing, and the sender's count of signals, which only grows in a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypingSignal {
    pub group: GroupId,
    /// The first voice room, or nothing, in a direct chat.
    pub voice: VoiceRoomId,
    pub typing: bool,
    pub seq: u32,
}

impl TypingSignal {
    /// Version, typing (1) or stopped (0), group id, voice room, count, in that order.
    pub fn encode(&self) -> [u8; SIGNAL_BYTES] {
        let mut out = [0u8; SIGNAL_BYTES];
        out[0] = VERSION;
        out[1] = u8::from(self.typing);
        out[2..18].copy_from_slice(self.group.as_bytes());
        out[18..26].copy_from_slice(&self.voice);
        out[26..30].copy_from_slice(&self.seq.to_be_bytes());
        out
    }

    /// A signal of this version; anything shorter, of another version or with an unknown
    /// state is none. Bytes after the signal are ignored.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < SIGNAL_BYTES || bytes.len() > MAX_SIGNAL_BYTES || bytes[0] != VERSION {
            return None;
        }
        let typing = match bytes[1] {
            0 => false,
            1 => true,
            _ => return None,
        };
        Some(Self {
            group: GroupId::from_bytes(&bytes[2..18]).ok()?,
            voice: bytes[18..26].try_into().ok()?,
            typing,
            seq: u32::from_be_bytes(bytes[26..30].try_into().ok()?),
        })
    }
}

/// How many signals a peer may still send now: a few at once, then one a second.
#[derive(Clone, Copy, Debug)]
pub(super) struct Allowance {
    tokens: f64,
    at: Instant,
}

impl Allowance {
    fn new(now: Instant) -> Self {
        Self {
            tokens: BURST,
            at: now,
        }
    }

    /// Takes one signal, if one is allowed now.
    fn take(&mut self, now: Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * REFILL_PER_SECOND).min(BURST);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// What a peer sent: its allowance and the last count heard in each chat.
struct Heard {
    allowance: Allowance,
    last: HashMap<GroupId, u32>,
}

/// The node's part of typing indicators.
pub(super) struct Typing {
    streams: libp2p_stream::Control,
    incoming: mpsc::Receiver<(PeerId, TypingSignal)>,
    /// Per connected peer that sent signals; dropped when it disconnects.
    heard: HashMap<PeerId, Heard>,
    /// Chats this device said it was typing in, and when it last said so.
    said: HashMap<GroupId, Instant>,
    seq: u32,
}

impl Typing {
    /// Takes signals from verified members over [`TYPING_PROTOCOL`] and passes them to the
    /// node, which checks them further.
    pub(super) fn start(
        mut streams: libp2p_stream::Control,
        verified: VerifiedPeers,
        stop: crate::Stopped,
        tasks: crate::Tasks,
    ) -> Result<Self, crate::NetError> {
        let mut accepted = streams
            .accept(TYPING_PROTOCOL)
            .map_err(|e| crate::NetError::Setup(e.to_string()))?;
        let (to_node, incoming) = mpsc::channel(QUEUE);
        let reading: Arc<Mutex<HashMap<PeerId, usize>>> = Default::default();
        let readers = tasks.clone();
        tasks.spawn(async move {
            while let Some((peer, mut stream)) =
                crate::next_until_stopped(&stop, &mut accepted).await
            {
                let member = verified.lock().map(|v| v.contains(&peer)).unwrap_or(false);
                if !member || !Reading::start(&reading, peer) {
                    continue;
                }
                let guard = Reading(reading.clone(), peer);
                let to_node = to_node.clone();
                readers.spawn(async move {
                    let _guard = guard;
                    let mut bytes = Vec::with_capacity(SIGNAL_BYTES);
                    let mut limited = (&mut stream).take(MAX_SIGNAL_BYTES as u64 + 1);
                    let read = limited.read_to_end(&mut bytes);
                    if !matches!(tokio::time::timeout(STREAM_TIME, read).await, Ok(Ok(_))) {
                        return;
                    }
                    match TypingSignal::decode(&bytes) {
                        Some(signal) => {
                            let _ = to_node.try_send((peer, signal));
                        }
                        None => tracing::debug!(%peer, "unreadable typing signal"),
                    }
                });
            }
        });
        Ok(Self {
            streams,
            incoming,
            heard: HashMap::new(),
            said: HashMap::new(),
            seq: 0,
        })
    }

    /// The next signal read from a stream.
    pub(super) async fn next(&mut self) -> Option<(PeerId, TypingSignal)> {
        self.incoming.recv().await
    }
}

/// Streams being read from a peer; one less when dropped.
struct Reading(Arc<Mutex<HashMap<PeerId, usize>>>, PeerId);

impl Reading {
    /// Counts one more stream being read from `peer`, if there is room for it.
    fn start(reading: &Mutex<HashMap<PeerId, usize>>, peer: PeerId) -> bool {
        let Ok(mut map) = reading.lock() else {
            return false;
        };
        let total: usize = map.values().sum();
        let mine = map.entry(peer).or_default();
        if total >= MAX_READING || *mine >= MAX_READING_PER_PEER {
            if *mine == 0 {
                map.remove(&peer);
            }
            return false;
        }
        *mine += 1;
        true
    }
}

impl Drop for Reading {
    fn drop(&mut self) {
        if let Ok(mut map) = self.0.lock()
            && let Some(n) = map.get_mut(&self.1)
        {
            *n -= 1;
            if *n == 0 {
                map.remove(&self.1);
            }
        }
    }
}

impl Node {
    /// This device is typing in the chat of `group`, or stopped (`typing` false): tells the
    /// people of that chat who are connected now and take the signal.
    pub(super) fn send_typing(&mut self, group: GroupId, typing: bool) {
        if self.hidden {
            return;
        }
        let now = Instant::now();
        if typing {
            if self
                .typing
                .said
                .get(&group)
                .is_some_and(|t| now.duration_since(*t) < SEND_GAP)
            {
                return;
            }
        } else if !self
            .typing
            .said
            .remove(&group)
            .is_some_and(|t| now.duration_since(t) < STOP_WITHIN)
        {
            return;
        }
        let Some((voice, peers)) = self.typing_audience(group) else {
            return;
        };
        if typing {
            self.typing
                .said
                .retain(|_, t| now.duration_since(*t) < STOP_WITHIN);
            self.typing.said.insert(group, now);
        }
        let (takers, others): (Vec<PeerId>, Vec<PeerId>) = peers.into_iter().partition(|p| {
            self.advertised
                .lock()
                .map(|map| map.get(p).is_some_and(|set| set.contains(&TYPING_PROTOCOL)))
                .unwrap_or(false)
        });
        if !others.is_empty() {
            tracing::debug!(
                count = others.len(),
                "typing not sent to devices that do not take it"
            );
        }
        if takers.is_empty() {
            return;
        }
        self.typing.seq = self.typing.seq.wrapping_add(1);
        let frame = TypingSignal {
            group,
            voice,
            typing,
            seq: self.typing.seq,
        }
        .encode();
        tracing::debug!(typing, count = takers.len(), "typing signal sent");
        for peer in takers {
            let mut streams = self.typing.streams.clone();
            self.tasks.spawn(async move {
                let send = async {
                    let mut stream = streams
                        .open_stream(peer, TYPING_PROTOCOL)
                        .await
                        .map_err(std::io::Error::other)?;
                    stream.write_all(&frame).await?;
                    stream.close().await
                };
                match tokio::time::timeout(STREAM_TIME, send).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::debug!(%peer, "typing signal not sent: {e}"),
                    Err(_) => tracing::debug!(%peer, "typing signal not sent in time"),
                }
            });
        }
    }

    /// "Stopped" for every chat this device said it was typing in, while it still may send:
    /// it is about to appear offline, and would otherwise show as typing until that runs
    /// out on the others' screens.
    pub(super) fn stop_typing_everywhere(&mut self) {
        let chats: Vec<GroupId> = self.typing.said.keys().copied().collect();
        for group in chats {
            self.send_typing(group, false);
        }
    }

    /// Who a typing signal about the chat of `group` goes to: the other person of a direct
    /// chat, or the members in the voice room this device is in; with that voice room.
    fn typing_audience(&self, group: GroupId) -> Option<(VoiceRoomId, Vec<PeerId>)> {
        let slot = self.groups.get(&group)?;
        let me = self.identity.public_key();
        let device_of = |peer: &PeerId| {
            self.members
                .get(peer)
                .map(|m| m.device)
                .filter(|d| *d != me && slot.ctx.is_member_device(d))
        };
        if slot.direct {
            let peers = slot
                .members
                .iter()
                .filter(|p| device_of(p).is_some())
                .copied()
                .collect();
            return Some(([0; 8], peers));
        }
        let (voice, tracker) = slot.joined()?;
        let peers = slot
            .members
            .iter()
            .filter(|p| device_of(p).is_some_and(|d| tracker.is_participant(&d)))
            .copied()
            .collect();
        Some((voice, peers))
    }

    /// A signal read from `peer`: shown only if it comes from someone in that chat, within
    /// the peer's allowance, and newer than the last one from it there.
    pub(super) fn on_typing(&mut self, peer: PeerId, signal: TypingSignal) {
        let Some(device) = self.members.get(&peer).map(|m| m.device) else {
            return;
        };
        let now = Instant::now();
        let heard = self.typing.heard.entry(peer).or_insert_with(|| Heard {
            allowance: Allowance::new(now),
            last: HashMap::new(),
        });
        if !heard.allowance.take(now) {
            tracing::debug!(%peer, "typing signals too often; dropped");
            return;
        }
        let me = self.identity.public_key();
        let Some(slot) = self.groups.get(&signal.group) else {
            return;
        };
        if device == me || !slot.members.contains(&peer) || !slot.ctx.is_member_device(&device) {
            tracing::debug!(%peer, "typing signal from someone not in that chat");
            return;
        }
        let voice = if slot.direct {
            None
        } else {
            match slot.joined() {
                Some((voice, tracker))
                    if voice == signal.voice && tracker.is_participant(&device) =>
                {
                    Some(voice)
                }
                _ => {
                    tracing::debug!(%peer, "typing signal about a room this device is not in");
                    return;
                }
            }
        };
        let heard = self.typing.heard.get_mut(&peer).expect("added above");
        if heard
            .last
            .get(&signal.group)
            .is_some_and(|n| *n >= signal.seq)
        {
            return;
        }
        heard.last.insert(signal.group, signal.seq);
        self.emit(NodeEvent::Typing {
            group: signal.group,
            voice,
            from: device,
            typing: signal.typing,
        });
    }

    /// `peer` has no connection left.
    pub(super) fn typing_peer_gone(&mut self, peer: &PeerId) {
        self.typing.heard.remove(peer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal() -> TypingSignal {
        TypingSignal {
            group: GroupId::from_bytes(&[7; 16]).unwrap(),
            voice: [3; 8],
            typing: true,
            seq: 0x0102_0304,
        }
    }

    #[test]
    fn a_signal_reads_back() {
        let bytes = signal().encode();
        assert_eq!(bytes.len(), SIGNAL_BYTES);
        assert_eq!(TypingSignal::decode(&bytes), Some(signal()));
        let stopped = TypingSignal {
            typing: false,
            ..signal()
        };
        assert_eq!(TypingSignal::decode(&stopped.encode()), Some(stopped));
    }

    #[test]
    fn what_does_not_fit_is_no_signal() {
        let bytes = signal().encode();
        // Short, of another version, an unknown state, too long.
        assert_eq!(TypingSignal::decode(&bytes[..SIGNAL_BYTES - 1]), None);
        assert_eq!(TypingSignal::decode(&[]), None);
        let mut other = bytes;
        other[0] = 2;
        assert_eq!(TypingSignal::decode(&other), None);
        let mut state = bytes;
        state[1] = 2;
        assert_eq!(TypingSignal::decode(&state), None);
        let mut long = bytes.to_vec();
        long.resize(MAX_SIGNAL_BYTES + 1, 0);
        assert_eq!(TypingSignal::decode(&long), None);
        // A later version's additions at the end are ignored.
        long.truncate(MAX_SIGNAL_BYTES);
        assert_eq!(TypingSignal::decode(&long), Some(signal()));
    }

    #[test]
    fn a_peer_gets_a_few_signals_at_once_then_one_a_second() {
        let start = Instant::now();
        let mut allowance = Allowance::new(start);
        let taken = (0..10).filter(|_| allowance.take(start)).count();
        assert_eq!(taken, BURST as usize);
        assert!(!allowance.take(start + Duration::from_millis(500)));
        assert!(allowance.take(start + Duration::from_millis(1000)));
        assert!(!allowance.take(start + Duration::from_millis(1100)));
        // A long quiet time gives back the burst, no more.
        let later = start + Duration::from_secs(60);
        let taken = (0..10).filter(|_| allowance.take(later)).count();
        assert_eq!(taken, BURST as usize);
    }

    #[test]
    fn streams_read_at_once_are_bounded() {
        let reading = Mutex::new(HashMap::new());
        let ana = PeerId::random();
        assert!(Reading::start(&reading, ana));
        assert!(Reading::start(&reading, ana));
        assert!(!Reading::start(&reading, ana), "two at once from one peer");
        let mut others = 0;
        while Reading::start(&reading, PeerId::random()) {
            others += 1;
        }
        assert_eq!(others, MAX_READING - MAX_READING_PER_PEER);
        // Refused peers leave nothing behind.
        assert_eq!(reading.lock().unwrap().len(), 1 + others);
    }
}
