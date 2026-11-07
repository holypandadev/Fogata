//! What the core tells the screens ([`CoreEvent`]), in the order it happened, within bounds
//! in number and in bytes however long the screens stop reading.
//!
//! - An event that only says something changed, or how something is now (the groups, the
//!   friends, the calls, a member connected or gone, a friend online, a chat changed, a
//!   download's progress, someone typing...) takes the place of the same one still waiting:
//!   there is one of each at most per group, member, friend, file or chat, which those
//!   bound. One that has the screens read anew keeps its place, which is earlier and reads
//!   the same; one carrying what it says goes last, where the newer one was sent. None of
//!   them is ever dropped.
//! - Notes ([`CoreEvent::Info`]) give way past [`SOFT_LIMIT`] events waiting.
//! - Every other event is kept. Past [`BEHIND`] of them waiting (or [`BYTES_BEHIND`]), the
//!   node is told the screens are behind ([`fogata_net::Backlog`]), and holds back what
//!   peers would add, as it does when its own queue is full. Past [`CEILING`] (or
//!   [`BYTES_CEILING`]), for a fault elsewhere, an event is dropped, and the screens are
//!   told the groups, the friends, the calls and the asks to join changed, which have them
//!   read all that anew. A call ringing, answered or ended, a group joined or left, and this
//!   device taken out of a room, which reading anew does not bring back, have [`RESERVE`]
//!   more.
//! - A call signal is not brought back either. Past [`SIGNALS_EACH`] of one sender waiting
//!   (or [`SIGNAL_BYTES`] of all), or the last bound, the signal is dropped, and a single
//!   [`CoreEvent::SignalsLost`] goes after the last one of that sender kept, for the call
//!   with it to agree anew. No signal of that sender is kept after it until it is received:
//!   it would belong to the agreement that failed.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_state::{RoomChange, VoiceRoomId};
use tokio::sync::{Notify, mpsc::error::TryRecvError};

use crate::CoreEvent;

/// Events waiting before notes give way.
const SOFT_LIMIT: usize = 256;
/// Events waiting, of those that are neither notes nor say how something is now, past
/// which the node is told the screens are behind ...
const BEHIND: usize = 1024;
/// ... and past which they are dropped.
const CEILING: usize = 4 * BEHIND;
/// The same in bytes, as [`weight`] counts them.
const BYTES_BEHIND: usize = 16 * 1024 * 1024;
const BYTES_CEILING: usize = 4 * BYTES_BEHIND;
/// More, in number and in bytes, for the steps no reading anew brings back (see
/// [`transition`]).
const RESERVE: usize = BEHIND / 4;
const BYTES_RESERVE: usize = BYTES_BEHIND / 4;
/// Call signals of one sender waiting, and of all in bytes, past which they give way.
const SIGNALS_EACH: usize = 64;
const SIGNAL_BYTES: usize = 8 * 1024 * 1024;

struct Shared {
    queue: Mutex<Queue>,
    ready: Notify,
    /// Senders not dropped yet: the screens' end is told the core stopped when none is left.
    senders: AtomicUsize,
    /// Told when the screens are behind.
    backlog: Option<fogata_net::Backlog>,
}

#[derive(Default)]
struct Queue {
    /// Each event sent and not received yet, with its number in the order sent; `None` once
    /// a newer one took its place or it gave way.
    events: VecDeque<(u64, Option<CoreEvent>)>,
    sent: u64,
    /// The number of the waiting event of each [`Key`].
    latest: HashMap<Key, u64>,
    /// The numbers of the notes waiting, oldest first.
    expendable: VecDeque<u64>,
    /// Events waiting; those of neither kind above, and their [`weight`].
    waiting: usize,
    others: usize,
    others_bytes: usize,
    /// The [`weight`] of those in `latest`.
    latest_bytes: usize,
    /// Signals waiting from each sender, and the bytes of all.
    signals: HashMap<(GroupId, DevicePublicKey), usize>,
    signal_bytes: usize,
    /// Whether the node was last told the screens are behind.
    behind: bool,
    /// The screens' end was dropped: nothing is kept any more.
    gone: bool,
    closed: bool,
    dropped: u64,
}

/// What an event that says something changed, or how something is now, is about.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Groups,
    Friends,
    Calls,
    JoinAsks,
    Update,
    HardNat,
    OtherSession,
    Stopped,
    Away,
    Address,
    Animation(DevicePublicKey),
    Direct(DevicePublicKey),
    DirectTyping(DevicePublicKey),
    Typing(GroupId, VoiceRoomId, DevicePublicKey),
    Download(GroupId, [u8; 32]),
    DirectDownload(DevicePublicKey, [u8; 32]),
    Member(GroupId, DevicePublicKey),
    Friend(DevicePublicKey),
    SignalsLost(GroupId, DevicePublicKey),
}

/// Where the newer of two events of one key goes (see the module's text).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// In the place of the one waiting.
    Kept,
    /// Last, the one waiting leaving the queue.
    Last,
}

fn key(event: &CoreEvent) -> Option<(Key, Place)> {
    use Place::{Kept, Last};
    Some(match event {
        CoreEvent::GroupsChanged => (Key::Groups, Kept),
        CoreEvent::FriendsChanged => (Key::Friends, Kept),
        CoreEvent::CallsChanged => (Key::Calls, Kept),
        CoreEvent::JoinAsksChanged => (Key::JoinAsks, Kept),
        CoreEvent::UpdateChanged => (Key::Update, Kept),
        CoreEvent::HardNat => (Key::HardNat, Kept),
        CoreEvent::OtherSession => (Key::OtherSession, Kept),
        CoreEvent::Animation { device } => (Key::Animation(*device), Kept),
        CoreEvent::DirectChanged { friend } => (Key::Direct(*friend), Kept),
        CoreEvent::Stopped => (Key::Stopped, Last),
        CoreEvent::Away { .. } => (Key::Away, Last),
        CoreEvent::Address(_) => (Key::Address, Last),
        CoreEvent::DirectTyping { friend, .. } => (Key::DirectTyping(*friend), Last),
        CoreEvent::Typing {
            group, voice, from, ..
        } => (Key::Typing(*group, *voice, *from), Last),
        CoreEvent::Download { group, file, .. } => (Key::Download(*group, *file), Last),
        CoreEvent::DirectDownload { friend, file, .. } => {
            (Key::DirectDownload(*friend, *file), Last)
        }
        CoreEvent::MemberConnected { group, device, .. }
        | CoreEvent::MemberDisconnected { group, device } => (Key::Member(*group, *device), Last),
        CoreEvent::FriendOnline { device } | CoreEvent::FriendOffline { device } => {
            (Key::Friend(*device), Last)
        }
        CoreEvent::SignalsLost { group, from } => (Key::SignalsLost(*group, *from), Last),
        _ => return None,
    })
}

/// About how many bytes `event` holds: what it carries that can be large, and a little for
/// the rest.
fn weight(event: &CoreEvent) -> usize {
    let photo = |profile: &fogata_friends::Profile| match &profile.avatar {
        fogata_friends::Avatar::Photo(bytes) => bytes.len(),
        fogata_friends::Avatar::Preset(_) => 0,
    };
    128 + match event {
        CoreEvent::Signal { payload, .. } => payload.len(),
        CoreEvent::Room {
            change: RoomChange::Chat(line),
            ..
        } => line.text.len(),
        CoreEvent::FriendRequest { profile } | CoreEvent::FriendAdded { profile } => photo(profile),
        CoreEvent::DirectMessage { text, .. }
        | CoreEvent::Info(text)
        | CoreEvent::Notice(text)
        | CoreEvent::FriendRequestFailed(text)
        | CoreEvent::DownloadFailed { reason: text, .. }
        | CoreEvent::DirectDownloadFailed { reason: text, .. } => text.len(),
        CoreEvent::DownloadDone { path, .. } => path.as_os_str().len(),
        CoreEvent::FriendRequestSent { name, .. }
        | CoreEvent::Joined { name, .. }
        | CoreEvent::Removed { name, .. }
        | CoreEvent::JoinAsked { name, .. }
        | CoreEvent::JoinWaiting { name, .. }
        | CoreEvent::JoinDeclined { name, .. }
        | CoreEvent::GroupInvited { name, .. } => name.len(),
        CoreEvent::InvitationDeclined { name, who, .. } => name.len() + who.len(),
        _ => 0,
    }
}

/// Steps no reading anew brings back, kept up to [`RESERVE`] past the last bound.
fn transition(event: &CoreEvent) -> bool {
    matches!(
        event,
        CoreEvent::CallIncoming { .. }
            | CoreEvent::CallAccepted { .. }
            | CoreEvent::CallEnded { .. }
            | CoreEvent::CallAddEnded { .. }
            | CoreEvent::Joined { .. }
            | CoreEvent::Removed { .. }
            | CoreEvent::TakenOut { .. }
    )
}

/// The sender of a call signal.
fn signal_of(event: &CoreEvent) -> Option<(GroupId, DevicePublicKey)> {
    match event {
        CoreEvent::Signal { group, from, .. } => Some((*group, *from)),
        _ => None,
    }
}

impl Queue {
    /// Where the event numbered `number` is, if not received yet.
    fn at(&self, number: u64) -> Option<usize> {
        self.events.binary_search_by_key(&number, |(n, _)| *n).ok()
    }

    /// Takes the event at `at` out of the queue, leaving its place empty.
    fn take_out(&mut self, at: usize) {
        if self.events[at].1.take().is_some() {
            self.waiting -= 1;
        }
    }

    fn push(&mut self, event: CoreEvent) -> u64 {
        // Past the soft bound, the oldest note gives way.
        if self.waiting >= SOFT_LIMIT
            && let Some(oldest) = self.expendable.pop_front()
            && let Some(at) = self.at(oldest)
        {
            self.take_out(at);
            self.dropped += 1;
        }
        self.sent += 1;
        self.waiting += 1;
        self.events.push_back((self.sent, Some(event)));
        // Empty places go once they are as many as the events waiting, each walked over
        // once for every one that left.
        if self.events.len() - self.waiting > self.waiting.max(SOFT_LIMIT) {
            self.events.retain(|(_, e)| e.is_some());
        }
        self.sent
    }

    /// Keeps `event`, of `key`, in the place of the one of its key waiting or last.
    fn keep_latest(&mut self, event: CoreEvent, key: Key, place: Place) {
        if let Some(at) = self.latest.get(&key).and_then(|n| self.at(*n))
            && let Some(old) = self.events[at].1.as_ref()
        {
            self.latest_bytes = self.latest_bytes - weight(old) + weight(&event);
            if place == Place::Kept {
                self.events[at].1 = Some(event);
                return;
            }
            self.take_out(at);
        } else {
            self.latest_bytes += weight(&event);
        }
        let number = self.push(event);
        self.latest.insert(key, number);
    }

    /// Keeps `event`, neither a note nor saying how something is now, if within the bounds
    /// (see the module's text); otherwise has the screens told what replaces it.
    fn keep_other(&mut self, event: CoreEvent) -> bool {
        let bytes = weight(&event);
        let (most, most_bytes) = if transition(&event) {
            (CEILING + RESERVE, BYTES_CEILING + BYTES_RESERVE)
        } else {
            (CEILING, BYTES_CEILING)
        };
        let fits = self.others < most && self.others_bytes + bytes <= most_bytes;
        if let Some(sender) = signal_of(&event) {
            let lost = Key::SignalsLost(sender.0, sender.1);
            let kept = fits
                && !self.latest.contains_key(&lost)
                && self.signals.get(&sender).copied().unwrap_or(0) < SIGNALS_EACH
                && self.signal_bytes + bytes <= SIGNAL_BYTES;
            if !kept {
                self.keep_latest(
                    CoreEvent::SignalsLost {
                        group: sender.0,
                        from: sender.1,
                    },
                    lost,
                    Place::Last,
                );
                return false;
            }
            *self.signals.entry(sender).or_default() += 1;
            self.signal_bytes += bytes;
        } else if !fits {
            // What the screens show is read anew: the core keeps it already.
            for event in [
                CoreEvent::GroupsChanged,
                CoreEvent::FriendsChanged,
                CoreEvent::CallsChanged,
                CoreEvent::JoinAsksChanged,
            ] {
                let (key, place) = key(&event).expect("each says something changed");
                self.keep_latest(event, key, place);
            }
            return false;
        }
        self.push(event);
        self.others += 1;
        self.others_bytes += bytes;
        true
    }

    /// Whether the screens are behind, for the node to hold back (see the module's text).
    fn behind(&self) -> bool {
        self.others >= BEHIND || self.others_bytes >= BYTES_BEHIND
    }
}

/// A queue of events between the core and the screens; `backlog` is told when they fall
/// behind.
pub fn queue(backlog: Option<fogata_net::Backlog>) -> (Sender, CoreEvents) {
    let shared = Arc::new(Shared {
        queue: Mutex::new(Queue::default()),
        ready: Notify::new(),
        senders: AtomicUsize::new(1),
        backlog,
    });
    (
        Sender {
            shared: shared.clone(),
        },
        CoreEvents { shared },
    )
}

fn locked(queue: &Mutex<Queue>) -> MutexGuard<'_, Queue> {
    queue.lock().unwrap_or_else(|e| e.into_inner())
}

/// Tells the node whether the screens are behind, when that changed.
fn tell_backlog(shared: &Shared, queue: &mut Queue) {
    let behind = queue.behind();
    if behind != queue.behind {
        queue.behind = behind;
        if let Some(backlog) = &shared.backlog {
            backlog.set(behind);
        }
    }
}

/// The screens' end was dropped: the event was not kept.
#[derive(Debug)]
pub struct Closed;

/// The core's end of the queue. Cheap to clone; the screens' end is told the core stopped
/// once every clone is dropped.
pub struct Sender {
    shared: Arc<Shared>,
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        self.shared.senders.fetch_add(1, Ordering::Relaxed);
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        if self.shared.senders.fetch_sub(1, Ordering::AcqRel) == 1 {
            locked(&self.shared.queue).closed = true;
            self.shared.ready.notify_one();
        }
    }
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

impl Sender {
    pub fn send(&self, event: CoreEvent) -> Result<(), Closed> {
        let mut guard = locked(&self.shared.queue);
        let queue = &mut *guard;
        if queue.gone {
            return Err(Closed);
        }
        let kept = if let Some((key, place)) = key(&event) {
            queue.keep_latest(event, key, place);
            true
        } else if matches!(event, CoreEvent::Info(_)) {
            let kept = queue.waiting < SOFT_LIMIT;
            if kept {
                let number = queue.push(event);
                queue.expendable.push_back(number);
            }
            kept
        } else {
            queue.keep_other(event)
        };
        tell_backlog(&self.shared, queue);
        if !kept {
            queue.dropped += 1;
        }
        let dropped = queue.dropped;
        drop(guard);
        if !kept && dropped.is_power_of_two() {
            tracing::warn!("the screens are not reading the core's events: {dropped} dropped");
        }
        self.shared.ready.notify_one();
        Ok(())
    }
}

/// The core's events for the screens ([`crate::Core::start`]), received in order. Once the
/// core stopped and every event was received, [`Self::recv`] gives `None`.
pub struct CoreEvents {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for CoreEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreEvents").finish_non_exhaustive()
    }
}

impl Drop for CoreEvents {
    fn drop(&mut self) {
        let mut queue = locked(&self.shared.queue);
        queue.gone = true;
        queue.events.clear();
        queue.latest.clear();
        queue.expendable.clear();
        queue.signals.clear();
        (queue.waiting, queue.others, queue.others_bytes) = (0, 0, 0);
        (queue.latest_bytes, queue.signal_bytes) = (0, 0);
        tell_backlog(&self.shared, &mut queue);
    }
}

impl CoreEvents {
    /// The next event, waiting for one; `None` once the core stopped and none is left. Safe
    /// to cancel (in `select!`): an event is taken only when it is returned.
    pub async fn recv(&mut self) -> Option<CoreEvent> {
        let shared = self.shared.clone();
        loop {
            let ready = shared.ready.notified();
            match self.try_recv() {
                Ok(event) => return Some(event),
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => ready.await,
            }
        }
    }

    /// The next event if one is waiting.
    pub fn try_recv(&mut self) -> Result<CoreEvent, TryRecvError> {
        let mut guard = locked(&self.shared.queue);
        let queue = &mut *guard;
        while let Some((number, event)) = queue.events.pop_front() {
            let Some(event) = event else {
                continue;
            };
            queue.waiting -= 1;
            if let Some((key, _)) = key(&event) {
                if queue.latest.get(&key) == Some(&number) {
                    queue.latest.remove(&key);
                    queue.latest_bytes -= weight(&event);
                }
            } else if matches!(event, CoreEvent::Info(_)) {
                // The oldest waiting, since those before it left.
                queue.expendable.pop_front();
            } else {
                let bytes = weight(&event);
                queue.others -= 1;
                queue.others_bytes -= bytes;
                if let Some(sender) = signal_of(&event) {
                    queue.signal_bytes -= bytes;
                    if let Some(count) = queue.signals.get_mut(&sender) {
                        *count -= 1;
                        if *count == 0 {
                            queue.signals.remove(&sender);
                        }
                    }
                }
                tell_backlog(&self.shared, queue);
            }
            return Ok(event);
        }
        if queue.closed {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    /// How many events wait, and about how many bytes they hold.
    pub fn waiting(&self) -> (usize, usize) {
        let queue = locked(&self.shared.queue);
        (
            queue.waiting,
            queue.others_bytes + queue.latest_bytes + 128 * queue.expendable.len(),
        )
    }
}
