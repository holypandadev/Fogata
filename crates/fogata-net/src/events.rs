//! What the node tells the application, in the order it happened.
//!
//! The queue is bounded, so that an application that stops reading (its screens stuck, say)
//! does not make the node keep every event, and the only events it ever drops are notes and
//! typing signals, worth nothing late:
//!
//! - Past [`SOFT_LIMIT`] events waiting, a new note or typing signal is dropped, and any
//!   other event takes the place of the oldest of them still waiting.
//! - An event that only says how something is now (a member's presence, a friend's status,
//!   that a group or the friends changed, a profile, an animation, a capability level, and
//!   how a group left, this device's join of a group or another device's ask to join one
//!   ended, a group a friend offered, a friend request) takes the place of the same one still
//!   waiting instead of queueing behind it: a peer may send them as often as it likes, and
//!   there is one of each at most per device, group, join asked or friend list, which the
//!   groups, the joins waiting, the friends, the offers being judged (none is taken while
//!   the queue is full) and the requests (each needs a link or a code of this device's)
//!   bound. So none of them is ever dropped, and what the application keeps of them (a group
//!   gone, a join, an ask waiting, a group offered, a request) never needs reading anew. They count towards the bounds
//!   below all the same, in number and in bytes (an animation is up to half a megabyte), so
//!   that many of them waiting hold back the rest. One carrying what it says (a presence,
//!   a status, a profile) goes last, where the newer one was sent, so that it still comes
//!   after an event it came after (a member gone, or back); one that has the application
//!   read the state anew (a group or the friends changed) keeps its place, which is earlier
//!   and reads the same.
//! - Every other event is kept. Past [`HARD_LIMIT`] of them waiting the queue is
//!   [`full`](EventSender::full): the node then takes no request of a peer that would add
//!   one (a group or direct message, a call signal or step, a group offered), and refuses
//!   it, so that the peer keeps it and sends it again, or this device asks for it again
//!   later (room events by their ids); it opens no room events asked for either. What still
//!   comes (connections opening and closing, answers to this device's own asks, its timers)
//!   is bounded by what it is about. The same holds past [`BYTES_FULL`] of them, as near as
//!   their sizes are known (messages, signals, logs and pictures they carry), and while the
//!   application says it is behind further on ([`Backlog`]).
//! - [`CEILING`] and [`BYTES_CEILING`] are last bounds, for a fault elsewhere: past them an
//!   event is dropped, and a single [`NodeEvent::Resync`] goes after the last one dropped,
//!   for the application to read the groups, members, friends and rooms anew. A step of a call
//!   with a friend, or a taking out of a room, which reading anew does not bring back, has
//!   [`RESERVE`] more. A call signal is not brought back either: past its bound, the
//!   signals of its sender give way, and a single [`NodeEvent::SignalsLost`] goes after the
//!   last of them kept, for the application's call with it to agree anew.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::sync::{Notify, mpsc::error::TryRecvError};

use crate::{NodeEvent, locked};

/// Events waiting before notes and typing signals give way.
const SOFT_LIMIT: usize = 1024;
/// Events waiting, of those neither notes nor typing signals nor saying how something is
/// now, past which the node takes no peer request that would add one.
pub(crate) const HARD_LIMIT: usize = 16 * 1024;
/// ... and past which they are dropped: nothing reaches it while the node holds back.
const CEILING: usize = 4 * HARD_LIMIT;
/// The same in bytes, as [`weight`] counts them.
pub(crate) const BYTES_FULL: usize = 64 * 1024 * 1024;
const BYTES_CEILING: usize = 4 * BYTES_FULL;
/// More, in number and in bytes, for the steps no reading anew brings back (see
/// [`transition`]).
const RESERVE: usize = HARD_LIMIT / 16;
const BYTES_RESERVE: usize = BYTES_FULL / 4;

struct Shared {
    queue: Mutex<Queue>,
    ready: Notify,
    /// The application is behind further on (see [`Backlog`]).
    behind: AtomicBool,
}

#[derive(Default)]
struct Queue {
    /// Each event sent and not received yet, with its number in the order sent; `None` once
    /// a newer one took its place, or it gave way (see the module's text).
    events: VecDeque<(u64, Option<NodeEvent>)>,
    sent: u64,
    /// The number of the waiting event that says how each thing is now (see [`latest`]).
    latest: HashMap<Latest, u64>,
    /// The numbers of the notes and typing signals waiting, oldest first.
    expendable: VecDeque<u64>,
    /// Events waiting, and those of them of neither kind above, and their [`weight`].
    waiting: usize,
    others: usize,
    others_bytes: usize,
    /// The [`weight`] of those in `latest`, which are as many as its keys.
    latest_bytes: usize,
    closed: bool,
    dropped: u64,
}

/// What an event that only says how something is now is about.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Latest {
    Presence(fogata_crypto::DevicePublicKey),
    Status(fogata_crypto::DevicePublicKey),
    Group(fogata_crypto::GroupId),
    Friends,
    Profile(fogata_crypto::DevicePublicKey),
    Animation(fogata_crypto::DevicePublicKey, fogata_friends::Animated),
    Level(fogata_crypto::DevicePublicKey),
    Removed(fogata_crypto::GroupId),
    Join(fogata_crypto::GroupId),
    JoinAsk(fogata_crypto::GroupId, fogata_crypto::DevicePublicKey),
    Offered(fogata_crypto::GroupId),
    Request(fogata_crypto::DevicePublicKey),
    Resync,
    SignalsLost(fogata_crypto::GroupId, fogata_crypto::DevicePublicKey),
}

/// Where the newer of two events of one key goes (see the module's text).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// In the place of the one waiting.
    Kept,
    /// Last, the one waiting leaving the queue.
    Last,
}

fn latest(event: &NodeEvent) -> Option<(Latest, Place)> {
    Some(match event {
        NodeEvent::MemberPresence { device, .. } => (Latest::Presence(*device), Place::Last),
        NodeEvent::FriendStatus { device, .. } => (Latest::Status(*device), Place::Last),
        NodeEvent::GroupChanged(group) => (Latest::Group(*group), Place::Kept),
        NodeEvent::FriendsChanged => (Latest::Friends, Place::Kept),
        NodeEvent::MemberProfile { profile } => (Latest::Profile(profile.device), Place::Last),
        NodeEvent::Animation { animation } => (
            Latest::Animation(animation.device, animation.of),
            Place::Last,
        ),
        NodeEvent::LevelSeen { device, .. } => (Latest::Level(*device), Place::Last),
        NodeEvent::Removed { group, .. } => (Latest::Removed(*group), Place::Last),
        NodeEvent::JoinWaiting { group }
        | NodeEvent::Joined { group, .. }
        | NodeEvent::JoinDeclined { group }
        | NodeEvent::JoinExpired { group } => (Latest::Join(*group), Place::Last),
        NodeEvent::JoinAsked { group, device, .. } | NodeEvent::JoinAnswered { group, device } => {
            (Latest::JoinAsk(*group, *device), Place::Last)
        }
        // The application keeps the offer (its store) and the request (the friends): the
        // newer of one group or device says what the older did.
        NodeEvent::GroupOffered { log, .. } => (Latest::Offered(log.group_id()), Place::Last),
        NodeEvent::FriendRequest { profile } => (Latest::Request(profile.device), Place::Last),
        NodeEvent::Resync => (Latest::Resync, Place::Last),
        NodeEvent::SignalsLost { group, from } => (Latest::SignalsLost(*group, *from), Place::Last),
        _ => return None,
    })
}

/// Whether `new` says all `old`, of the same key, does. The application keeps the newest
/// profile and animation and the highest level, whatever their order: an older one comes
/// to nothing.
fn supersedes(new: &NodeEvent, old: &NodeEvent) -> bool {
    match (new, old) {
        (NodeEvent::MemberProfile { profile: new }, NodeEvent::MemberProfile { profile: old }) => {
            new.version_ms >= old.version_ms
        }
        (NodeEvent::Animation { animation: new }, NodeEvent::Animation { animation: old }) => {
            new.version_ms >= old.version_ms
        }
        (NodeEvent::LevelSeen { level: new, .. }, NodeEvent::LevelSeen { level: old, .. }) => {
            new >= old
        }
        // A join that ended (let in, said no to, run out) is not undone by a step of it
        // told late: an answer that it waits, sent before it was let in. In its place the
        // application would never learn the group was joined.
        (
            NodeEvent::JoinWaiting { .. },
            NodeEvent::Joined { .. }
            | NodeEvent::JoinDeclined { .. }
            | NodeEvent::JoinExpired { .. },
        ) => false,
        _ => true,
    }
}

/// About how many bytes `event` holds: what it carries that can be large, and a little for
/// the rest.
fn weight(event: &NodeEvent) -> usize {
    const LOG_ENTRY: usize = 512;
    let photo = |profile: &fogata_friends::Profile| match &profile.avatar {
        fogata_friends::Avatar::Photo(bytes) => bytes.len(),
        fogata_friends::Avatar::Preset(_) => 0,
    };
    128 + match event {
        NodeEvent::Direct { payloads, .. } => payloads.iter().map(Vec::len).sum(),
        NodeEvent::Signal { payload, .. } => payload.len(),
        NodeEvent::Room {
            change: fogata_state::RoomChange::Chat(line),
            ..
        } => line.text.len(),
        NodeEvent::Removed { log, .. }
        | NodeEvent::GroupOffered { log, .. }
        | NodeEvent::DirectOffered { log, .. } => log.len() * LOG_ENTRY,
        NodeEvent::FriendRequest { profile }
        | NodeEvent::FriendAdded { profile }
        | NodeEvent::MemberProfile { profile } => photo(profile),
        NodeEvent::Animation { animation } => animation.picture.len(),
        NodeEvent::Info(text) | NodeEvent::FriendRequestFailed(text) => text.len(),
        _ => 0,
    }
}

/// Events dropped first when the application falls behind.
fn expendable(event: &NodeEvent) -> bool {
    matches!(event, NodeEvent::Info(_) | NodeEvent::Typing { .. })
}

/// Steps no reading anew brings back, kept up to [`RESERVE`] past the last bounds: a call
/// with a friend ringing, answered or ended, one that did not reach it, and this device
/// taken out of a room.
fn transition(event: &NodeEvent) -> bool {
    matches!(
        event,
        NodeEvent::Call { .. } | NodeEvent::CallUndelivered { .. } | NodeEvent::TakenOut { .. }
    )
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

    /// Keeps `event`, which says how something is now, in the place of the one of its key
    /// waiting or last (see the module's text); false if the one waiting says more.
    fn keep_latest(&mut self, event: NodeEvent, key: Latest, place: Place) -> bool {
        if let Some(at) = self.latest.get(&key).and_then(|n| self.at(*n))
            && let Some(old) = self.events[at].1.as_ref()
        {
            if !supersedes(&event, old) {
                return false;
            }
            self.latest_bytes = self.latest_bytes - weight(old) + weight(&event);
            if place == Place::Kept {
                self.events[at].1 = Some(event);
                return true;
            }
            self.take_out(at);
        } else {
            self.latest_bytes += weight(&event);
        }
        let number = self.push(event);
        self.latest.insert(key, number);
        true
    }

    fn push(&mut self, event: NodeEvent) -> u64 {
        // Past the soft bound, the oldest note or typing signal gives way.
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
}

/// A queue of events between the node and the application.
pub(crate) fn queue() -> (EventSender, NodeEvents) {
    let shared = Arc::new(Shared {
        queue: Mutex::new(Queue::default()),
        ready: Notify::new(),
        behind: AtomicBool::new(false),
    });
    (
        EventSender {
            shared: shared.clone(),
        },
        NodeEvents { shared },
    )
}

/// The node's end of the queue; the application's end is told the node stopped when it is
/// dropped.
pub(crate) struct EventSender {
    shared: Arc<Shared>,
}

impl EventSender {
    pub(crate) fn send(&self, event: NodeEvent) {
        let mut guard = locked(&self.shared.queue);
        let queue = &mut *guard;
        let kept = if let Some((key, place)) = latest(&event) {
            if !queue.keep_latest(event, key, place) {
                return;
            }
            true
        } else if expendable(&event) {
            let kept = queue.waiting < SOFT_LIMIT;
            if kept {
                let number = queue.push(event);
                queue.expendable.push_back(number);
            }
            kept
        } else {
            let bytes = weight(&event);
            let (most, most_bytes) = if transition(&event) {
                (CEILING + RESERVE, BYTES_CEILING + BYTES_RESERVE)
            } else {
                (CEILING, BYTES_CEILING)
            };
            // Once signals of a sender gave way, none of its comes before the application
            // heard so: one coming after the call agreed anew would be mistaken for one of
            // the new agreement.
            let lost = match &event {
                NodeEvent::Signal { group, from, .. } => Some((*group, *from)),
                _ => None,
            };
            let kept = queue.others < most
                && queue.others_bytes + bytes <= most_bytes
                && !lost
                    .is_some_and(|(g, f)| queue.latest.contains_key(&Latest::SignalsLost(g, f)));
            if kept {
                queue.push(event);
                queue.others += 1;
                queue.others_bytes += bytes;
            } else if let Some((group, from)) = lost {
                queue.keep_latest(
                    NodeEvent::SignalsLost { group, from },
                    Latest::SignalsLost(group, from),
                    Place::Last,
                );
            } else {
                // The application is told to read what it knows anew, once, after the last
                // event dropped: what is kept after it then comes whole and in order, while
                // one kept before a resync waiting could undo what was dropped after.
                queue.keep_latest(NodeEvent::Resync, Latest::Resync, Place::Last);
            }
            kept
        };
        if !kept {
            queue.dropped += 1;
        }
        let dropped = queue.dropped;
        drop(guard);
        if !kept && dropped.is_power_of_two() {
            tracing::warn!("the application is not reading the node's events: {dropped} dropped");
        }
        self.shared.ready.notify_one();
    }

    /// Whether so many events wait that the node takes no peer request that would add one
    /// more (see the module's text).
    pub(crate) fn full(&self) -> bool {
        let queue = locked(&self.shared.queue);
        queue.others + queue.latest.len() >= HARD_LIMIT
            || queue.others_bytes + queue.latest_bytes >= BYTES_FULL
            || self.shared.behind.load(Ordering::Relaxed)
    }
}

/// Where the application says it is behind further on than this queue, with events of its
/// own waiting for its screens: the node then holds back as when this queue is full.
#[derive(Clone)]
pub struct Backlog {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Backlog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backlog").finish_non_exhaustive()
    }
}

impl Backlog {
    /// Says whether the application is behind further on now.
    pub fn set(&self, behind: bool) {
        self.shared.behind.store(behind, Ordering::Relaxed);
    }
}

impl Drop for EventSender {
    fn drop(&mut self) {
        locked(&self.shared.queue).closed = true;
        self.shared.ready.notify_one();
    }
}

/// Events from a running node ([`crate::start`]), received in order. Once the node stopped
/// and every event was received, [`Self::recv`] gives `None`.
pub struct NodeEvents {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for NodeEvents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeEvents").finish_non_exhaustive()
    }
}

impl NodeEvents {
    /// Where the application says it is behind further on (see [`Backlog`]).
    pub fn backlog(&self) -> Backlog {
        Backlog {
            shared: self.shared.clone(),
        }
    }

    /// The next event, waiting for one; `None` once the node stopped and none is left. Safe
    /// to cancel (in `select!`): an event is taken only when it is returned.
    pub async fn recv(&mut self) -> Option<NodeEvent> {
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
    pub fn try_recv(&mut self) -> Result<NodeEvent, TryRecvError> {
        let mut guard = locked(&self.shared.queue);
        let queue = &mut *guard;
        while let Some((number, event)) = queue.events.pop_front() {
            let Some(event) = event else {
                continue;
            };
            queue.waiting -= 1;
            if let Some((key, _)) = latest(&event) {
                if queue.latest.get(&key) == Some(&number) {
                    queue.latest.remove(&key);
                    queue.latest_bytes -= weight(&event);
                }
            } else if expendable(&event) {
                // The oldest waiting, since those before it left.
                queue.expendable.pop_front();
            } else {
                queue.others -= 1;
                queue.others_bytes -= weight(&event);
            }
            return Ok(event);
        }
        if queue.closed {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(n: usize) -> NodeEvent {
        NodeEvent::Info(n.to_string())
    }

    fn device() -> fogata_crypto::DevicePublicKey {
        fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key()
    }

    fn all(events: &mut NodeEvents) -> Vec<NodeEvent> {
        let mut got = Vec::new();
        while let Ok(event) = events.try_recv() {
            got.push(event);
        }
        got
    }

    #[tokio::test]
    async fn notes_give_way_when_the_application_falls_behind() {
        let (sender, mut events) = queue();
        for n in 0..SOFT_LIMIT {
            sender.send(info(n));
        }
        // Full of notes: another note is dropped, anything else takes the oldest's place.
        sender.send(info(SOFT_LIMIT));
        sender.send(NodeEvent::OtherSession);
        let got = all(&mut events);
        assert_eq!(got.len(), SOFT_LIMIT);
        assert!(matches!(&got[0], NodeEvent::Info(n) if n == "1"));
        assert!(matches!(got.last(), Some(NodeEvent::OtherSession)));
        // What is left is received after the node stopped; then nothing.
        sender.send(NodeEvent::OtherSession);
        drop(sender);
        assert!(matches!(events.recv().await, Some(NodeEvent::OtherSession)));
        assert!(events.recv().await.is_none());
    }

    /// Only notes and typing signals are ever dropped. Past the hard bound the queue says
    /// it is full, for the node to hold back what would add more, and keeps what still
    /// comes; events saying how something is now still take the place of their own.
    #[test]
    fn no_event_but_notes_is_dropped_when_the_application_falls_behind() {
        let (sender, mut events) = queue();
        let ana = device();
        let presence = |away| NodeEvent::MemberPresence {
            device: ana,
            hidden: false,
            away,
        };
        for _ in 0..HARD_LIMIT - 1 {
            sender.send(NodeEvent::OtherSession);
        }
        assert!(!sender.full());
        sender.send(NodeEvent::OtherSession);
        assert!(sender.full());
        sender.send(info(0));
        for _ in 0..10 {
            sender.send(NodeEvent::FriendOffline { device: ana });
        }
        for i in 0..2 * HARD_LIMIT {
            sender.send(presence(i % 2 == 0));
        }
        sender.send(NodeEvent::Joined {
            group: fogata_crypto::GroupId::random().unwrap(),
            name: "g".into(),
        });
        let got = all(&mut events);
        assert_eq!(got.len(), HARD_LIMIT + 10 + 1 + 1);
        assert!(!got.iter().any(|e| matches!(e, NodeEvent::Info(_))));
        assert!(matches!(
            got[HARD_LIMIT + 10],
            NodeEvent::MemberPresence { away: false, .. }
        ));
        assert!(matches!(got.last(), Some(NodeEvent::Joined { .. })));
        assert!(!sender.full());
        // Only a fault elsewhere could take them to the last bound, where they give way.
        for _ in 0..CEILING + 10 {
            sender.send(NodeEvent::OtherSession);
        }
        let got = all(&mut events);
        assert_eq!(got.len(), CEILING + 1);
        assert!(matches!(got.last(), Some(NodeEvent::Resync)));
    }

    /// Signals, direct messages, members coming and going and their presences, mixed, far
    /// past every bound, as a stuck application would leave them: the queue stays within its
    /// bounds in number and bytes, and an application that applies what it reads, and reads
    /// the members connected anew at a resync, ends with the node's own view.
    #[test]
    fn a_saturated_queue_ends_with_the_application_knowing_what_the_node_knows() {
        use std::collections::{HashMap as Map, HashSet as Set};
        let (sender, mut events) = queue();
        let devices: Vec<_> = (0..64).map(|_| device()).collect();
        let groups: Vec<_> = (0..4)
            .map(|_| fogata_crypto::GroupId::random().unwrap())
            .collect();
        let address: libp2p::Multiaddr = "/ip4/192.168.1.7/udp/4001/quic-v1".parse().unwrap();
        // What the node knows, changed before each event is sent, as the node does.
        let mut online: Set<(fogata_crypto::GroupId, fogata_crypto::DevicePublicKey)> = Set::new();
        let mut away: Map<fogata_crypto::DevicePublicKey, bool> = Map::new();
        let mut most = (0, 0);
        for step in 0..60_000usize {
            let device = devices[step % devices.len()];
            let group = groups[(step / devices.len()) % groups.len()];
            let event = match step % 5 {
                0 => NodeEvent::Signal {
                    group,
                    from: device,
                    payload: vec![0; 256 * 1024],
                },
                1 => NodeEvent::Direct {
                    group,
                    from: device,
                    payloads: vec![vec![1; 1024]],
                },
                2 | 3 if online.insert((group, device)) => NodeEvent::MemberConnected {
                    group,
                    device,
                    peer: libp2p::PeerId::random(),
                    path: crate::Path::Direct,
                    address: address.clone(),
                },
                2 | 3 => {
                    online.remove(&(group, device));
                    NodeEvent::MemberDisconnected {
                        group,
                        device,
                        peer: libp2p::PeerId::random(),
                    }
                }
                _ => {
                    let now = !away.get(&device).copied().unwrap_or(false);
                    away.insert(device, now);
                    NodeEvent::MemberPresence {
                        device,
                        hidden: false,
                        away: now,
                    }
                }
            };
            sender.send(event);
            let queue = locked(&sender.shared.queue);
            most = (most.0.max(queue.others), most.1.max(queue.others_bytes));
        }
        assert!(most.0 <= CEILING && most.1 <= BYTES_CEILING, "{most:?}");
        // The application, reading at last.
        let (mut seen_online, mut seen_away) = (Set::new(), Map::new());
        let mut resyncs = 0;
        while let Ok(event) = events.try_recv() {
            match event {
                NodeEvent::MemberConnected { group, device, .. } => {
                    seen_online.insert((group, device));
                }
                NodeEvent::MemberDisconnected { group, device, .. } => {
                    seen_online.remove(&(group, device));
                }
                NodeEvent::MemberPresence { device, away, .. } => {
                    seen_away.insert(device, away);
                }
                NodeEvent::Resync => {
                    resyncs += 1;
                    seen_online = online.clone();
                }
                _ => {}
            }
        }
        assert_eq!(resyncs, 1);
        assert_eq!(seen_online, online);
        assert_eq!(seen_away, away);
    }

    /// What the application keeps of a group gone, of this device's join and of another's
    /// ask to join is never dropped, however far the queue is past its last bound: a group
    /// removed and not told would come back at the next start. The newest of each says it.
    #[test]
    fn a_group_gone_a_join_and_an_ask_are_never_dropped() {
        let (sender, mut events) = queue();
        let (group, joined) = (
            fogata_crypto::GroupId::random().unwrap(),
            fogata_crypto::GroupId::random().unwrap(),
        );
        let ana = device();
        for _ in 0..CEILING + 10 {
            sender.send(NodeEvent::OtherSession);
        }
        let ana_identity = fogata_crypto::DeviceIdentity::generate().unwrap();
        let log = fogata_state::GroupLog::create(&ana_identity, "g", 1).unwrap();
        sender.send(NodeEvent::Removed {
            group,
            log: log.clone(),
        });
        sender.send(NodeEvent::JoinWaiting { group: joined });
        sender.send(NodeEvent::Joined {
            group: joined,
            name: "g".into(),
        });
        sender.send(NodeEvent::JoinAsked {
            group,
            device: ana,
            name: "Ana".into(),
        });
        sender.send(NodeEvent::JoinAnswered { group, device: ana });
        let got = all(&mut events);
        let removed = got
            .iter()
            .filter(|e| matches!(e, NodeEvent::Removed { group: g, .. } if *g == group))
            .count();
        assert_eq!(removed, 1);
        let joins: Vec<_> = got
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    NodeEvent::JoinWaiting { .. }
                        | NodeEvent::Joined { .. }
                        | NodeEvent::JoinAsked { .. }
                        | NodeEvent::JoinAnswered { .. }
                )
            })
            .collect();
        assert!(matches!(
            joins[..],
            [NodeEvent::Joined { .. }, NodeEvent::JoinAnswered { .. }]
        ));
    }

    /// A group a friend offered and a friend request are never dropped either, however far
    /// the queue is past its last bound: reading anew does not bring them back (the friend
    /// offers again only at its next connection, a request needs a new link or code). The
    /// newest of each group or device says it.
    #[test]
    fn a_group_offered_and_a_friend_request_are_never_dropped() {
        let (sender, mut events) = queue();
        for _ in 0..CEILING + 10 {
            sender.send(NodeEvent::OtherSession);
        }
        let ana = fogata_crypto::DeviceIdentity::generate().unwrap();
        let bea = fogata_crypto::DeviceIdentity::generate().unwrap();
        let log = fogata_state::GroupLog::create(&ana, "g", 1).unwrap();
        let other = fogata_state::GroupLog::create(&ana, "o", 1).unwrap();
        let profile = |name: &str| {
            fogata_friends::FriendBook::new(&bea, name, 1)
                .unwrap()
                .profile
        };
        for log in [&log, &other, &log] {
            sender.send(NodeEvent::GroupOffered {
                log: log.clone(),
                by: ana.public_key(),
            });
        }
        sender.send(NodeEvent::FriendRequest {
            profile: profile("Bea"),
        });
        sender.send(NodeEvent::FriendRequest {
            profile: profile("Bea B"),
        });
        let got = all(&mut events);
        let offered: Vec<_> = got
            .iter()
            .filter_map(|e| match e {
                NodeEvent::GroupOffered { log, .. } => Some(log.group_id()),
                _ => None,
            })
            .collect();
        assert_eq!(offered, vec![other.group_id(), log.group_id()]);
        let requests: Vec<_> = got
            .iter()
            .filter_map(|e| match e {
                NodeEvent::FriendRequest { profile } => Some(profile.name.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(requests, vec!["Bea B".to_string()]);
        assert!(got.iter().any(|e| matches!(e, NodeEvent::Resync)));
    }

    /// Past the bound in bytes the queue is full, whatever the number of events.
    #[test]
    fn large_events_make_the_queue_full_before_many_do() {
        let (sender, _events) = queue();
        let from = device();
        let group = fogata_crypto::GroupId::random().unwrap();
        while !sender.full() {
            sender.send(NodeEvent::Direct {
                group,
                from,
                payloads: vec![vec![0; 1 << 20]],
            });
        }
        let queue = locked(&sender.shared.queue);
        assert!(queue.others < HARD_LIMIT);
        assert!(queue.others_bytes >= BYTES_FULL);
    }

    /// A newer presence or status goes where it was sent, after what came between: a
    /// friend gone and back since the one waiting keeps the newer status (in a room, or
    /// away) once both are read.
    #[test]
    fn an_event_saying_how_something_is_now_comes_after_what_came_before_it() {
        let (sender, mut events) = queue();
        let bea = device();
        let status = |away| NodeEvent::FriendStatus {
            device: bea,
            in_room: false,
            group: None,
            hidden: false,
            away,
        };
        let group = fogata_crypto::GroupId::random().unwrap();
        sender.send(status(false));
        sender.send(NodeEvent::GroupChanged(group));
        sender.send(NodeEvent::FriendOffline { device: bea });
        sender.send(NodeEvent::OtherSession);
        sender.send(status(true));
        // One that has the application read the group anew keeps its earlier place.
        sender.send(NodeEvent::GroupChanged(group));
        let got = all(&mut events);
        assert_eq!(got.len(), 4);
        assert!(matches!(got[0], NodeEvent::GroupChanged(_)));
        assert!(matches!(got[1], NodeEvent::FriendOffline { .. }));
        assert!(matches!(got[2], NodeEvent::OtherSession));
        assert!(matches!(got[3], NodeEvent::FriendStatus { away: true, .. }));
        // Once received, the next one queues again.
        sender.send(status(false));
        assert!(matches!(
            events.try_recv(),
            Ok(NodeEvent::FriendStatus { away: false, .. })
        ));
    }

    /// An older profile, animation or level than the one waiting takes nothing of its
    /// place: the application would keep the newer one anyway.
    #[test]
    fn an_older_profile_or_level_leaves_the_newer_one_waiting() {
        let (sender, mut events) = queue();
        let identity = fogata_crypto::DeviceIdentity::generate().unwrap();
        let profile = |version| NodeEvent::MemberProfile {
            profile: fogata_friends::FriendBook::new(&identity, "Bea", version)
                .unwrap()
                .profile,
        };
        let level = |level| NodeEvent::LevelSeen {
            device: identity.public_key(),
            level,
        };
        sender.send(profile(9));
        sender.send(level(3));
        sender.send(profile(7));
        sender.send(level(2));
        let got = all(&mut events);
        assert_eq!(got.len(), 2);
        assert!(matches!(&got[0], NodeEvent::MemberProfile { profile } if profile.version_ms == 9));
        assert!(matches!(got[1], NodeEvent::LevelSeen { level: 3, .. }));
    }

    /// Keeping each presence last costs little: 100 000 from 512 members, with the queue
    /// past its hard bound, go through in well under a second.
    #[test]
    fn many_presences_behind_a_full_queue_cost_little() {
        let (sender, mut events) = queue();
        let devices: Vec<_> = (0..512).map(|_| device()).collect();
        for _ in 0..HARD_LIMIT {
            sender.send(NodeEvent::OtherSession);
        }
        let start = std::time::Instant::now();
        for i in 0..100_000 {
            sender.send(NodeEvent::MemberPresence {
                device: devices[i % devices.len()],
                hidden: false,
                away: i % 3 == 0,
            });
        }
        let got = all(&mut events);
        let took = start.elapsed();
        assert_eq!(got.len(), HARD_LIMIT + devices.len());
        assert!(took < std::time::Duration::from_secs(2), "{took:?}");
    }

    /// Events saying how something is now, of as many keys as members, friends and groups
    /// can make, count towards the bound: past it the queue is full, so the node holds back
    /// what peers would add, and every one of them is still received.
    #[test]
    fn many_events_saying_how_things_are_now_make_the_queue_full() {
        let (sender, mut events) = queue();
        let (mut first_full, mut sent) = (None, 0);
        for _ in 0..75_000 {
            sender.send(NodeEvent::MemberPresence {
                device: device(),
                hidden: false,
                away: true,
            });
            sent += 1;
            if first_full.is_none() && sender.full() {
                first_full = Some(sent);
            }
        }
        assert_eq!(
            first_full,
            Some(HARD_LIMIT),
            "full after {first_full:?} of {sent}"
        );
        assert_eq!(all(&mut events).len(), sent);
        assert!(!sender.full());
    }

    /// Animations, the largest there are, count in bytes: past the bound in bytes the queue
    /// is full, whatever their number (128 of them were held without it).
    #[test]
    fn animations_count_in_bytes() {
        let (sender, mut events) = queue();
        let mut held = 0;
        for _ in 0..128 {
            // The queue does not look inside: as large as one gets, from as many members.
            let picture = vec![7; fogata_friends::MAX_PHOTO_ANIMATION_BYTES];
            held += picture.len();
            let animation = fogata_friends::Animation {
                device: device(),
                of: fogata_friends::Animated::Photo,
                still: [0; 32],
                picture,
                version_ms: 1,
                signature: [0; fogata_crypto::SIGNATURE_LEN],
            };
            sender.send(NodeEvent::Animation { animation });
            if sender.full() {
                break;
            }
        }
        assert!(sender.full(), "{held} bytes waiting and not full");
        assert!(
            held < BYTES_FULL + fogata_friends::MAX_PHOTO_ANIMATION_BYTES,
            "{held}"
        );
        all(&mut events);
        assert!(!sender.full());
    }

    /// Past the last bound a call signal gives way, and the application hears once, after
    /// the last signal of that sender kept, that its call with it is to agree anew. No
    /// signal of that sender comes after it until it is received; another sender's are
    /// told apart.
    #[test]
    fn a_signal_dropped_has_the_call_agree_anew() {
        let (sender, mut events) = queue();
        let (ana, bea) = (device(), device());
        let group = fogata_crypto::GroupId::random().unwrap();
        let signal = |from, n: u8| NodeEvent::Signal {
            group,
            from,
            payload: vec![n],
        };
        sender.send(signal(ana, 0));
        for _ in 0..CEILING - 1 {
            sender.send(NodeEvent::OtherSession);
        }
        // Full up to the last bound: Ana's next signals and Bea's give way.
        sender.send(signal(ana, 1));
        sender.send(signal(bea, 1));
        sender.send(signal(ana, 2));
        let got = all(&mut events);
        assert_eq!(got.len(), CEILING + 2);
        assert!(matches!(&got[0], NodeEvent::Signal { payload, .. } if payload == &[0]));
        assert!(!got.iter().any(|e| matches!(e, NodeEvent::Resync)));
        assert!(matches!(
            got[CEILING..],
            [
                NodeEvent::SignalsLost { from: b, .. },
                NodeEvent::SignalsLost { from: a, .. },
            ] if a == ana && b == bea
        ));
        // Heard: the next ones come again.
        sender.send(signal(ana, 3));
        assert!(
            matches!(events.try_recv(), Ok(NodeEvent::Signal { payload, .. }) if payload == [3])
        );
    }

    /// While the signals of a sender are said lost, the ones it sends after are dropped as
    /// well, though there would be room for them: they belong to the agreement that failed.
    #[test]
    fn no_signal_of_a_sender_comes_after_its_loss_until_heard() {
        let (sender, mut events) = queue();
        let ana = device();
        let group = fogata_crypto::GroupId::random().unwrap();
        for _ in 0..CEILING {
            sender.send(NodeEvent::OtherSession);
        }
        let signal = |n: u8| NodeEvent::Signal {
            group,
            from: ana,
            payload: vec![n],
        };
        sender.send(signal(1));
        // Room again, but the loss is not heard yet.
        for _ in 0..10 {
            events.try_recv().unwrap();
        }
        sender.send(signal(2));
        let got = all(&mut events);
        assert!(!got.iter().any(|e| matches!(e, NodeEvent::Signal { .. })));
        assert!(matches!(got.last(), Some(NodeEvent::SignalsLost { .. })));
    }

    /// A step of a call with a friend, which reading anew does not bring back, still has
    /// room past the last bound.
    #[test]
    fn a_call_step_has_room_past_the_last_bound() {
        let (sender, mut events) = queue();
        for _ in 0..CEILING {
            sender.send(NodeEvent::OtherSession);
        }
        sender.send(NodeEvent::CallUndelivered {
            to: device(),
            call: [1; 16],
            refused: false,
        });
        sender.send(NodeEvent::OtherSession);
        let got = all(&mut events);
        assert!(matches!(
            got[CEILING..],
            [NodeEvent::CallUndelivered { .. }, NodeEvent::Resync]
        ));
    }

    /// The application, behind further on, makes the node hold back as a full queue does.
    #[test]
    fn an_application_behind_makes_the_queue_full() {
        let (sender, events) = queue();
        let backlog = events.backlog();
        assert!(!sender.full());
        backlog.set(true);
        assert!(sender.full());
        backlog.set(false);
        assert!(!sender.full());
    }

    #[tokio::test]
    async fn a_waiting_receiver_wakes_for_each_event() {
        let (sender, mut events) = queue();
        let reader = tokio::spawn(async move {
            let mut n = 0;
            while events.recv().await.is_some() {
                n += 1;
            }
            n
        });
        for n in 0..100 {
            sender.send(info(n));
            tokio::task::yield_now().await;
        }
        drop(sender);
        assert_eq!(reader.await.unwrap(), 100);
    }

    /// A join that ended is not undone by a step of it told late: the answer that it
    /// waits, which can come after the `Admitted` that let this device in, took the place
    /// of `Joined` while the application had not read it yet, and the application never
    /// learnt the group was joined.
    #[test]
    fn a_join_that_ended_is_not_undone_by_a_late_wait() {
        let (sender, mut events) = queue();
        let group = fogata_crypto::GroupId::random().unwrap();
        sender.send(NodeEvent::Joined {
            group,
            name: "g".into(),
        });
        sender.send(NodeEvent::JoinWaiting { group });
        let got = all(&mut events);
        assert!(matches!(got[..], [NodeEvent::Joined { .. }]), "{got:?}");
        // A wait before the end still gives way to it.
        sender.send(NodeEvent::JoinWaiting { group });
        sender.send(NodeEvent::JoinDeclined { group });
        let got = all(&mut events);
        assert!(
            matches!(got[..], [NodeEvent::JoinDeclined { .. }]),
            "{got:?}"
        );
    }
}
