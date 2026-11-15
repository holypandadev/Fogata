//! Fogata addition: a bound on what dials to the device's own networks can leave waiting in
//! the kernel, on Linux.
//!
//! The device sends everything from one UDP socket: requests, the DHT and a call's media.
//! On Linux a packet to an address on one of the device's own networks waits in the kernel
//! while the neighbour is looked for (ARP, or neighbour discovery for IPv6), and counts
//! against the socket's send buffer until it leaves or is dropped. Where no host answers
//! for the address, it waits until the kernel gives up, and once the socket's buffer is
//! full it sends nothing to anyone: a call's sound and picture stop. Anyone who answers a
//! DHT lookup can name made-up peers at such addresses, as many as it likes, so the dials to
//! addresses of the device's own networks that have not answered lately take turns here:
//! each holds a share of the room (and of [`PER_ADDRESS`] for its address) while it runs
//! and for [`LINGER`] after, unless a reply came from its address, and the others wait for
//! room. An address that answered lately (see [`ANSWERED_FOR`]) needs no turn, nor does the
//! loopback or any address off those networks: a router answers for those. Only the
//! transport's dials take turns: its replies to incoming handshakes do not. The transport
//! uses the turns on Linux and Android only, whose figures these are (see `TURNS` in
//! `transport.rs`); elsewhere every dial goes at once.
//!
//! The room is in two parts. The node vouches ([`vouch`]) for an address, with the peer it
//! expects there, just before it dials it, when it has seen that a host is there now (mDNS
//! answered from that very address) or the person gave the address; for [`VOUCHED_FOR`]
//! after, dials to that peer at that address take their turns in [`VOUCHED_IN_ALL`], and
//! every other dial takes its turn in [`IN_ALL`]. A dial in one part never takes the other
//! part's room, but dials in the vouched part still wait for each other: someone on the
//! local network who makes mDNS tell of made-up peers from its own address fills it as
//! made-up DHT answers fill the other. The transport knows nothing of where an address came
//! from: a dial no one vouched for is counted with the others, never refused.
//!
//! The figures come from how Linux holds such packets:
//! - A neighbour that does not answer is asked three times a second apart, and then every
//!   packet waiting for it is dropped; the next packet starts over. So a packet waits three
//!   seconds at most, which [`LINGER`] covers with room for the kernel's timers.
//! - The kernel counts a packet by the memory it takes, not by its length: room for the
//!   data and headers, rounded up to the next power of two, and the packet's own
//!   bookkeeping. A QUIC packet of 1200 bytes (every first packet of a handshake is padded
//!   to that) takes 2304 bytes; one of 64 bytes takes 768.
//! - A dial's handshake offers a post-quantum key, which takes two first packets, and QUIC
//!   sends two again each time it hears nothing, after 1 s and then twice as long each time
//!   (at 0, 1, 3 and 7 s), and one more to close it when the dial gives up. Within three
//!   seconds a dial sends six packets at most: 14 KiB ([`DIAL_HOLDS`]).
//! - A hole punch sends a packet of 64 random bytes every 10 to 200 ms, about 29 in three
//!   seconds and very rarely more than 40: 32 KiB ([`PUNCH_HOLDS`]).
//! - A socket's default buffer is 212992 bytes (`net.core.wmem_default`), and what it may
//!   ask for is capped by `net.core.wmem_max`, the same by default; the transport asks for
//!   more on Linux, so with that cap at its default the kernel gives twice it, 425984 bytes.
//!   The dials that take turns leave [`IN_ALL`] + [`VOUCHED_IN_ALL`] = 114688 + 65536 =
//!   180224 bytes waiting at most, 42% of that (85% of the default buffer itself, where the
//!   larger one is not given). Packets to addresses that answered lately, and replies to
//!   incoming handshakes, are not counted: they go to hosts that are there.
//! - A neighbour that answered stays known (reachable, then stale) and packets to it leave at
//!   once. Once the host has gone, the next packet starts the kernel's checks: 5 s before it
//!   asks, then three asks a second apart, and only then does it forget the neighbour and
//!   hold packets again. [`ANSWERED_FOR`] (10 s) after the last connection ended or the last
//!   reply, a dial needs a turn again; one started before that sends its last packets 7 s
//!   after its first, before the kernel holds any.
//!
//! The turns cost a wait for room when more than eight new addresses of the local network
//! are dialled at once and some never answer, and a dial that waited its whole time fails
//! without sending anything (`Error::NoTurn`).

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    net::IpAddr,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::{FutureExt, channel::oneshot};
use futures_timer::Delay;
use libp2p_identity::PeerId;

/// The packets a QUIC dial can leave waiting for a neighbour at one time.
pub(crate) const DIAL_HOLDS: usize = 14 * 1024;
/// The packets a hole punch can leave waiting for a neighbour at one time.
pub(crate) const PUNCH_HOLDS: usize = 32 * 1024;
/// What the dials to one address may hold at once in each part of the room: two dials, or
/// one punch.
pub(crate) const PER_ADDRESS: usize = 32 * 1024;
/// What the dials no one vouched for may hold at once: eight dials, 114688 bytes.
pub(crate) const IN_ALL: usize = 8 * DIAL_HOLDS;
/// What the dials to addresses the node vouched for may hold at once: two punches, or four
/// dials, 65536 bytes.
pub(crate) const VOUCHED_IN_ALL: usize = 2 * PUNCH_HOLDS;
/// A socket's default send buffer on Linux (`net.core.wmem_default`).
const DEFAULT_BUFFER: usize = 212992;
const _: () = assert!(IN_ALL + VOUCHED_IN_ALL < DEFAULT_BUFFER);
/// How long a dial whose address did not reply still holds its room after it ends: its
/// last packets may still wait for the neighbour that long.
pub(crate) const LINGER: Duration = Duration::from_millis(3500);
/// How long after its last connection ended, or its last reply to a dial, an address still
/// needs no turn (see the module's notes).
pub(crate) const ANSWERED_FOR: Duration = Duration::from_secs(10);
/// Addresses that answered, kept at most; the one that answered longest ago goes first.
const MAX_ANSWERED: usize = 256;
/// How long the node's vouching for a peer at an address lasts: the node vouches again
/// before each dial, and a dial waits for its turn 7 s at most, so this covers a dial and
/// the tries mDNS makes again soon after.
pub const VOUCHED_FOR: Duration = Duration::from_secs(60);
/// Peers at addresses vouched for, kept at most; past it, the one vouched for longest ago
/// goes.
pub const MAX_VOUCHED: usize = 256;

/// Notes that a host is at `ip` now, or that the person gave it: for [`VOUCHED_FOR`] from
/// now, dials to `peer` there (to any peer only when `peer` is `None`, a dial that names
/// none) take their turns apart from those no one vouched for (see the module's notes).
/// Never for an address known only from the DHT.
pub fn vouch(ip: IpAddr, peer: Option<PeerId>) {
    vouches().vouch((ip, peer), Instant::now());
}

/// Whether the node vouched for `peer` at `ip` within [`VOUCHED_FOR`].
fn vouched(ip: IpAddr, peer: Option<PeerId>) -> bool {
    vouches().holds(&(ip, peer), Instant::now())
}

/// The addresses the node vouched for, in this process.
fn vouches() -> MutexGuard<'static, Vouches> {
    static VOUCHES: OnceLock<Mutex<Vouches>> = OnceLock::new();
    VOUCHES
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

type Vouched = (IpAddr, Option<PeerId>);

/// Peers at addresses the node vouched for, with when it last did.
#[derive(Debug, Default)]
struct Vouches {
    at: HashMap<Vouched, Instant>,
}

impl Vouches {
    fn vouch(&mut self, key: Vouched, now: Instant) {
        self.at.insert(key, now);
        if self.at.len() > MAX_VOUCHED {
            self.at
                .retain(|_, at| now.saturating_duration_since(*at) < VOUCHED_FOR);
        }
        while self.at.len() > MAX_VOUCHED {
            let oldest = self.at.iter().min_by_key(|(_, at)| **at).map(|(k, _)| *k);
            match oldest {
                Some(oldest) => self.at.remove(&oldest),
                None => break,
            };
        }
    }

    fn holds(&self, key: &Vouched, now: Instant) -> bool {
        self.at
            .get(key)
            .is_some_and(|at| now.saturating_duration_since(*at) < VOUCHED_FOR)
    }
}

/// Which part of the room a dial takes its turn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// Dials to peers at addresses the node vouched for: [`VOUCHED_IN_ALL`].
    Vouched,
    /// Every other dial: [`IN_ALL`].
    Other,
}

impl Part {
    fn of(ip: IpAddr, peer: Option<PeerId>) -> Self {
        if vouched(ip, peer) {
            Part::Vouched
        } else {
            Part::Other
        }
    }

    fn room(self) -> usize {
        match self {
            Part::Vouched => VOUCHED_IN_ALL,
            Part::Other => IN_ALL,
        }
    }
}

/// The room dials to the device's own networks share (see the module's notes).
#[derive(Clone, Debug, Default)]
pub(crate) struct DialRoom(Arc<Mutex<Room>>);

#[derive(Debug, Default)]
struct Room {
    vouched: Share,
    other: Share,
    /// Room of dials that ended, given back at the time noted.
    lingering: Vec<(Instant, Part, IpAddr, usize)>,
    next_waiter: u64,
    /// Addresses that answered: how many connections from them are open, and when the last
    /// one ended or began, or the last reply to a dial came.
    answered: HashMap<IpAddr, (usize, Instant)>,
}

/// One part of the room.
#[derive(Debug, Default)]
struct Share {
    held: usize,
    by_address: HashMap<IpAddr, usize>,
    /// Dials waiting for room in this part, in the order they came.
    waiting: VecDeque<Waiter>,
}

impl Share {
    fn give_back(&mut self, ip: IpAddr, cost: usize) {
        self.held = self.held.saturating_sub(cost);
        if let Some(at) = self.by_address.get_mut(&ip) {
            *at = at.saturating_sub(cost);
            if *at == 0 {
                self.by_address.remove(&ip);
            }
        }
    }

    /// Takes the next waiter that fits in `room` out of the queue, with its room counted:
    /// among those whose address has room, the one whose address holds least (the earliest
    /// of those), and none past it.
    fn next(&mut self, room: usize) -> Option<Waiter> {
        let best = self
            .waiting
            .iter()
            .enumerate()
            .filter_map(|(i, w)| {
                let at = self.by_address.get(&w.ip).copied().unwrap_or(0);
                (at + w.cost <= PER_ADDRESS).then_some((at, i))
            })
            .min()
            .map(|(_, i)| i)
            .filter(|i| self.held + self.waiting[*i].cost <= room)?;
        let waiter = self.waiting.remove(best)?;
        self.held += waiter.cost;
        *self.by_address.entry(waiter.ip).or_default() += waiter.cost;
        Some(waiter)
    }
}

#[derive(Debug)]
struct Waiter {
    id: u64,
    ip: IpAddr,
    cost: usize,
    turn: oneshot::Sender<Held>,
}

/// Turns to hand to their dials once the room's lock is let go.
type Granted = Vec<(oneshot::Sender<Held>, Held)>;

impl Room {
    fn answers(&self, ip: &IpAddr) -> bool {
        self.answered
            .get(ip)
            .is_some_and(|(open, since)| *open > 0 || since.elapsed() < ANSWERED_FOR)
    }

    fn share(&mut self, part: Part) -> &mut Share {
        match part {
            Part::Vouched => &mut self.vouched,
            Part::Other => &mut self.other,
        }
    }

    /// Gives back the room of the dials whose time to linger is over.
    fn expire(&mut self, now: Instant) {
        let mut i = 0;
        while i < self.lingering.len() {
            if self.lingering[i].0 <= now {
                let (_, part, ip, cost) = self.lingering.swap_remove(i);
                self.share(part).give_back(ip, cost);
            } else {
                i += 1;
            }
        }
    }

    /// Notes that `ip` answered now; `opened` when a connection from it is set up.
    fn note_answer(&mut self, ip: IpAddr, opened: bool) {
        let now = Instant::now();
        let entry = self.answered.entry(ip).or_insert((0, now));
        if opened {
            entry.0 += 1;
        }
        entry.1 = now;
        if self.answered.len() > MAX_ANSWERED {
            let oldest = self
                .answered
                .iter()
                .filter(|(_, (open, _))| *open == 0)
                .min_by_key(|(_, (_, since))| *since)
                .map(|(ip, _)| *ip);
            if let Some(oldest) = oldest {
                self.answered.remove(&oldest);
            }
        }
    }
}

impl DialRoom {
    fn lock(&self) -> MutexGuard<'_, Room> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether `ip` has a connection open, or answered within [`ANSWERED_FOR`].
    pub(crate) fn answers(&self, ip: &IpAddr) -> bool {
        self.lock().answers(ip)
    }

    /// Notes that `ip` set up a connection, until the returned guard is dropped with it.
    pub(crate) fn answered(&self, ip: IpAddr) -> Answered {
        self.settle(|room| room.note_answer(ip, true));
        Answered {
            room: self.clone(),
            ip,
        }
    }

    /// Notes that a dial to `ip` got a reply, whatever came of it: a host is there.
    pub(crate) fn replied(&self, ip: IpAddr) {
        self.settle(|room| room.note_answer(ip, false));
    }

    /// Forgets what answered, gives back what lingers and forgets the node's vouching: the
    /// device's networks changed, and the same numbers may now name other hosts, or none.
    pub(crate) fn forget_network(&self) {
        vouches().at.clear();
        self.settle(|room| {
            room.answered.clear();
            for (_, part, ip, cost) in std::mem::take(&mut room.lingering) {
                room.share(part).give_back(ip, cost);
            }
        });
    }

    /// Changes the room with `change`, then hands out the turns that fit.
    fn settle(&self, change: impl FnOnce(&mut Room)) {
        let granted = {
            let mut room = self.lock();
            change(&mut room);
            room.expire(Instant::now());
            self.grant(&mut room)
        };
        hand_out(granted);
    }

    /// A turn to dial `peer` (when the dial names one) at `ip` with `cost` bytes of packets
    /// that may wait, in the part of the room the node's vouching gives it (see [`vouch`]):
    /// at once when there is room there, else when dials before it there give theirs back,
    /// or its address answers. Among those waiting in a part whose address has room, the one
    /// whose address holds least goes first (the earliest of those), so made-up peers at a
    /// few addresses do not keep a real one at another address waiting behind all of
    /// theirs; and none goes past it, so a punch is not kept waiting by dials.
    pub(crate) fn take(&self, ip: IpAddr, peer: Option<PeerId>, cost: usize) -> Turn {
        self.take_in(Part::of(ip, peer), ip, cost)
    }

    fn take_in(&self, part: Part, ip: IpAddr, cost: usize) -> Turn {
        let (sender, receiver) = oneshot::channel();
        let mut id = 0;
        self.settle(|room| {
            id = room.next_waiter;
            room.next_waiter += 1;
            room.share(part).waiting.push_back(Waiter {
                id,
                ip,
                cost,
                turn: sender,
            });
        });
        Turn {
            room: self.clone(),
            part,
            id,
            receiver,
            timer: None,
        }
    }

    /// Takes the waiters that may go now out of the queues: those whose address answers
    /// lately, with no room counted, then those that fit, with their room counted.
    fn grant(&self, room: &mut Room) -> Granted {
        let mut granted = Vec::new();
        for part in [Part::Vouched, Part::Other] {
            let waiting = match part {
                Part::Vouched => &room.vouched.waiting,
                Part::Other => &room.other.waiting,
            };
            let answering: Vec<u64> = waiting
                .iter()
                .filter(|w| room_answers(&room.answered, &w.ip))
                .map(|w| w.id)
                .collect();
            let share = room.share(part);
            for id in answering {
                if let Some(at) = share.waiting.iter().position(|w| w.id == id)
                    && let Some(waiter) = share.waiting.remove(at)
                {
                    granted.push((waiter.turn, self.held(part, waiter.ip, 0)));
                }
            }
            while let Some(waiter) = room.share(part).next(part.room()) {
                granted.push((waiter.turn, self.held(part, waiter.ip, waiter.cost)));
            }
        }
        granted
    }

    fn held(&self, part: Part, ip: IpAddr, cost: usize) -> Held {
        Held {
            room: self.clone(),
            part,
            ip,
            cost,
            sent: true,
        }
    }

    /// The room of a dial that ended: given back now when its address answered or it sent
    /// nothing, else once its packets can no longer be waiting.
    fn release(&self, part: Part, ip: IpAddr, cost: usize, sent: bool) {
        self.settle(|room| {
            if !sent || room.answers(&ip) {
                room.share(part).give_back(ip, cost);
            } else {
                room.lingering
                    .push((Instant::now() + LINGER, part, ip, cost));
            }
        });
    }

    /// What dials not vouched for hold, and how many of them wait.
    #[cfg(test)]
    fn held_now(&self) -> (usize, usize) {
        self.held_in(Part::Other)
    }

    #[cfg(test)]
    fn held_in(&self, part: Part) -> (usize, usize) {
        let mut room = self.lock();
        let share = room.share(part);
        (share.held, share.waiting.len())
    }
}

/// [`Room::answers`] on the map alone, while a share of the room is borrowed.
fn room_answers(answered: &HashMap<IpAddr, (usize, Instant)>, ip: &IpAddr) -> bool {
    answered
        .get(ip)
        .is_some_and(|(open, since)| *open > 0 || since.elapsed() < ANSWERED_FOR)
}

/// Hands each turn to its dial, with the room's lock let go: a dial that went away drops
/// its turn, which gives the room back.
fn hand_out(granted: Granted) {
    for (sender, held) in granted {
        if let Err(held) = sender.send(held) {
            held.unsent();
        }
    }
}

/// A dial's room, given back when it is dropped (see [`DialRoom::release`]).
#[derive(Debug)]
pub(crate) struct Held {
    room: DialRoom,
    part: Part,
    ip: IpAddr,
    cost: usize,
    /// Whether the dial may have sent anything.
    sent: bool,
}

impl Held {
    /// Gives the room back at once: the dial sent nothing.
    pub(crate) fn unsent(mut self) {
        self.sent = false;
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.room.release(self.part, self.ip, self.cost, self.sent);
    }
}

/// A dial waiting for room; dropped, it leaves the queue, and the dials behind it that now
/// fit get their turns.
#[derive(Debug)]
pub(crate) struct Turn {
    room: DialRoom,
    part: Part,
    id: u64,
    receiver: oneshot::Receiver<Held>,
    /// Wakes the dial when the next lingering room is due, which nothing else would.
    timer: Option<(Instant, Delay)>,
}

impl Future for Turn {
    type Output = Held;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Held> {
        loop {
            if let Poll::Ready(Ok(held)) = self.receiver.poll_unpin(cx) {
                return Poll::Ready(held);
            }
            let (granted, due) = {
                let mut room = self.room.lock();
                room.expire(Instant::now());
                let granted = self.room.grant(&mut room);
                (granted, room.lingering.iter().map(|(at, ..)| *at).min())
            };
            if !granted.is_empty() {
                hand_out(granted);
                continue;
            }
            let Some(due) = due else {
                self.timer = None;
                return Poll::Pending;
            };
            if self.timer.as_ref().is_none_or(|(at, _)| *at != due) {
                let wait = due.saturating_duration_since(Instant::now());
                self.timer = Some((due, Delay::new(wait)));
            }
            match self.timer.as_mut().map(|(_, delay)| delay.poll_unpin(cx)) {
                Some(Poll::Ready(())) => self.timer = None,
                _ => return Poll::Pending,
            }
        }
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (id, part) = (self.id, self.part);
        self.room
            .settle(|room| room.share(part).waiting.retain(|w| w.id != id));
        // A turn handed to it and not taken: the dial sent nothing.
        self.receiver.close();
        if let Ok(Some(held)) = self.receiver.try_recv() {
            held.unsent();
        }
    }
}

/// An open connection from an address (see [`DialRoom::answered`]).
#[derive(Debug)]
pub(crate) struct Answered {
    room: DialRoom,
    ip: IpAddr,
}

impl Drop for Answered {
    fn drop(&mut self) {
        let mut room = self.room.lock();
        if let Some((open, since)) = room.answered.get_mut(&self.ip) {
            *open = open.saturating_sub(1);
            *since = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::task::Waker;

    use super::*;

    /// Held by the tests that use the process-wide vouching.
    static VOUCHING: Mutex<()> = Mutex::new(());

    /// Addresses no test vouches for in this process.
    fn ip(last: u8) -> IpAddr {
        IpAddr::from([192, 168, 1, last])
    }

    fn take(room: &DialRoom, ip: IpAddr, cost: usize) -> Turn {
        room.take(ip, None, cost)
    }

    fn ready(turn: &mut Turn) -> Option<Held> {
        match Pin::new(turn).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(held) => Some(held),
            Poll::Pending => None,
        }
    }

    /// Whether a turn was handed to `turn` already, without polling it (which would hand
    /// out turns itself).
    fn handed(turn: &mut Turn) -> Option<Held> {
        turn.receiver.try_recv().ok().flatten()
    }

    /// Makes the room of dials that ended due now.
    fn linger_over(room: &DialRoom) {
        let past = Instant::now()
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        for (at, ..) in &mut room.lock().lingering {
            *at = past;
        }
    }

    /// Fills the room of dials no one vouched for with eight dials at four addresses.
    fn fill_other(room: &DialRoom) -> Vec<Held> {
        let mut held = Vec::new();
        for last in [1, 2, 3, 4] {
            for _ in 0..2 {
                held.extend(ready(&mut take(room, ip(last), DIAL_HOLDS)));
            }
        }
        assert_eq!(room.held_now(), (IN_ALL, 0));
        held
    }

    /// Dials to one address take two places at once, dials to all of them eight, and the
    /// others wait for one to give its room back, which one whose address did not answer
    /// does only once its packets can no longer be waiting.
    #[test]
    fn dials_to_addresses_that_have_not_answered_wait_for_room() {
        let room = DialRoom::default();
        let mut turns: Vec<Turn> = (0..3).map(|_| take(&room, ip(10), DIAL_HOLDS)).collect();
        let mut held: Vec<Held> = turns.iter_mut().filter_map(ready).collect();
        assert_eq!(held.len(), 2, "one address held more than two dials");
        for last in 11..20 {
            turns.push(take(&room, ip(last), DIAL_HOLDS));
        }
        held.extend(turns.iter_mut().skip(3).filter_map(ready));
        assert_eq!(
            held.len(),
            IN_ALL / DIAL_HOLDS,
            "more dials than the room holds"
        );
        assert_eq!(room.held_now(), (IN_ALL, 4));

        // One ends: its room lingers, then the earliest waiting whose address has room
        // takes its place.
        drop(held.pop());
        assert!(
            turns.iter_mut().all(|t| ready(t).is_none()),
            "its room came back early"
        );
        linger_over(&room);
        assert!(
            ready(&mut turns[2]).is_none(),
            "an address held three dials"
        );
        let next = ready(&mut turns[3 + 6]);
        assert!(next.is_some(), "the room did not come back");
    }

    /// A dial whose address answered gives its room back as it ends.
    #[test]
    fn a_dial_whose_address_answered_gives_its_room_back_at_once() {
        let room = DialRoom::default();
        let dial = ready(&mut take(&room, ip(1), DIAL_HOLDS)).unwrap();
        let open = room.answered(ip(1));
        drop(dial);
        assert_eq!(room.held_now(), (0, 0));
        drop(open);
    }

    /// A dial that got a reply but no connection (a wrong peer, a refusal) does not linger,
    /// and its address needs no turn for a while.
    #[test]
    fn a_dial_that_got_a_reply_does_not_linger() {
        let room = DialRoom::default();
        let dial = ready(&mut take(&room, ip(30), DIAL_HOLDS)).unwrap();
        room.replied(ip(30));
        drop(dial);
        assert_eq!(room.held_now(), (0, 0));
        assert!(room.lock().lingering.is_empty());
        assert!(room.answers(&ip(30)));
    }

    /// A dial that sent nothing gives its room back at once.
    #[test]
    fn a_dial_that_sent_nothing_does_not_linger() {
        let room = DialRoom::default();
        let dial = ready(&mut take(&room, ip(31), DIAL_HOLDS)).unwrap();
        dial.unsent();
        assert_eq!(room.held_now(), (0, 0));
        assert!(room.lock().lingering.is_empty());
    }

    /// A dial waiting for room goes as soon as its address answers.
    #[test]
    fn a_waiting_dial_goes_when_its_address_answers() {
        let room = DialRoom::default();
        let _held = fill_other(&room);
        let mut waiting = take(&room, ip(32), DIAL_HOLDS);
        assert!(handed(&mut waiting).is_none());
        room.replied(ip(32));
        let held = handed(&mut waiting).expect("it kept waiting");
        assert_eq!(held.cost, 0, "it took room it does not need");
    }

    /// A punch, which takes an address's whole share, is not passed by dials that came
    /// after it.
    #[test]
    fn a_punch_is_not_passed_by_later_dials() {
        let room = DialRoom::default();
        let mut held = fill_other(&room);
        let mut punch = take(&room, ip(9), PUNCH_HOLDS);
        let mut dial = take(&room, ip(8), DIAL_HOLDS);
        drop(held.pop());
        linger_over(&room);
        assert!(ready(&mut punch).is_none());
        assert!(
            ready(&mut dial).is_none(),
            "a later dial went before the punch"
        );
        drop(held.pop());
        drop(held.pop());
        linger_over(&room);
        let punching = ready(&mut punch);
        assert!(punching.is_some(), "the punch did not get its turn");
        assert!(ready(&mut dial).is_none());
    }

    /// When a punch at the front leaves (its time ran out), the dial behind it that fits
    /// gets its turn at once, without another dial ending.
    #[test]
    fn a_dial_behind_a_punch_that_leaves_goes_at_once() {
        let room = DialRoom::default();
        let mut held = fill_other(&room);
        drop(held.pop());
        linger_over(&room);
        room.lock().expire(Instant::now());
        let punch = take(&room, ip(9), PUNCH_HOLDS);
        let mut dial = take(&room, ip(8), DIAL_HOLDS);
        assert!(handed(&mut dial).is_none(), "it went past the punch");
        drop(punch);
        assert!(handed(&mut dial).is_some(), "it was not woken");
    }

    /// A dial whose address holds nothing goes before those of addresses that hold some.
    #[test]
    fn an_address_that_holds_nothing_goes_first() {
        let room = DialRoom::default();
        let mut held = fill_other(&room);
        // Made-up peers at an address that will hold one dial, and a real one at another.
        let mut made_up: Vec<Turn> = (0..20).map(|_| take(&room, ip(4), DIAL_HOLDS)).collect();
        let mut real = take(&room, ip(6), DIAL_HOLDS);
        drop(held.pop());
        drop(held.swap_remove(0));
        linger_over(&room);
        let dialling = ready(&mut real);
        assert!(
            dialling.is_some(),
            "the new address waited behind the others"
        );
        let mut next: Vec<Held> = made_up.iter_mut().filter_map(ready).collect();
        assert_eq!(next.len(), 1, "the other place went elsewhere");
        next.clear();
    }

    /// While dials no one vouched for fill their room and many more wait, each at an address
    /// of its own (made-up peers all over the network), a dial to an address the node
    /// vouched for gets its turn at once; and those in its part wait only for each other.
    #[test]
    fn a_vouched_dial_goes_at_once_while_the_others_are_full() {
        let room = DialRoom::default();
        let mut held = fill_other(&room);
        let mut made_up: Vec<Turn> = (0..200u8)
            .map(|i| take(&room, IpAddr::from([192, 168, 2, i]), DIAL_HOLDS))
            .collect();
        assert_eq!(room.held_now(), (IN_ALL, 200));

        let real = ready(&mut room.take_in(Part::Vouched, ip(50), DIAL_HOLDS));
        assert!(real.is_some(), "a vouched dial waited behind made-up ones");
        let punch = ready(&mut room.take_in(Part::Vouched, ip(51), PUNCH_HOLDS));
        assert!(punch.is_some());
        assert_eq!(room.held_in(Part::Vouched), (DIAL_HOLDS + PUNCH_HOLDS, 0));
        // Its own part full: the next vouched dial waits for room there.
        let mut third = room.take_in(Part::Vouched, ip(52), PUNCH_HOLDS);
        assert!(ready(&mut third).is_none());
        drop(punch);
        linger_over(&room);
        assert!(
            ready(&mut third).is_some(),
            "vouched room did not come back"
        );
        // Room given back in the other part goes to the dials waiting there.
        drop(held.pop());
        linger_over(&room);
        let next: Vec<Held> = made_up.iter_mut().filter_map(ready).collect();
        assert_eq!(next.len(), 1);
        drop(real);
    }

    /// Dials no one vouched for never take the room of vouched ones, however much of it is
    /// free.
    #[test]
    fn dials_not_vouched_for_never_take_vouched_room() {
        let room = DialRoom::default();
        let _held = fill_other(&room);
        assert_eq!(room.held_in(Part::Vouched), (0, 0));
        let mut more = take(&room, ip(5), DIAL_HOLDS);
        assert!(ready(&mut more).is_none(), "it took vouched room");
        assert_eq!(room.held_in(Part::Vouched), (0, 0));
        assert_eq!(room.held_now(), (IN_ALL, 1));
    }

    /// Each part keeps its own share for one address.
    #[test]
    fn each_part_keeps_one_address_to_its_share() {
        let room = DialRoom::default();
        let mut turns: Vec<Turn> = (0..3)
            .map(|_| room.take_in(Part::Vouched, ip(7), DIAL_HOLDS))
            .collect();
        let held: Vec<Held> = turns.iter_mut().filter_map(ready).collect();
        assert_eq!(held.len(), 2, "one address held more than its share");
        assert_eq!(room.held_in(Part::Vouched), (2 * DIAL_HOLDS, 1));
    }

    /// Vouching lasts [`VOUCHED_FOR`], and the newest [`MAX_VOUCHED`] are kept.
    #[test]
    fn vouches_expire_and_are_bounded() {
        let mut vouches = Vouches::default();
        let start = Instant::now();
        let a = (IpAddr::from([10, 0, 0, 1]), None);
        vouches.vouch(a, start);
        assert!(vouches.holds(&a, start + VOUCHED_FOR - Duration::from_millis(1)));
        assert!(!vouches.holds(&a, start + VOUCHED_FOR));
        // Vouched again, it lasts again.
        vouches.vouch(a, start + VOUCHED_FOR);
        assert!(vouches.holds(&a, start + VOUCHED_FOR + Duration::from_secs(1)));

        let mut vouches = Vouches::default();
        let key = |i: usize| (IpAddr::from([10, 1, (i / 256) as u8, i as u8]), None);
        for i in 0..(MAX_VOUCHED + 10) {
            vouches.vouch(key(i), start + Duration::from_millis(i as u64));
        }
        let now = start + Duration::from_secs(1);
        assert_eq!(vouches.at.len(), MAX_VOUCHED);
        assert!(!vouches.holds(&key(0), now), "oldest kept");
        assert!(!vouches.holds(&key(9), now));
        assert!(vouches.holds(&key(10), now));
        assert!(vouches.holds(&key(MAX_VOUCHED + 9), now));
    }

    /// The node's vouching for a peer at an address puts that peer's dials there in the
    /// vouched part, and no one else's: made-up peers named at the same address stay with
    /// the others.
    #[test]
    fn vouching_is_for_one_peer_at_one_address() {
        let _vouching = VOUCHING.lock().unwrap_or_else(PoisonError::into_inner);
        let at = IpAddr::from([198, 51, 100, 77]);
        let real = PeerId::random();
        let made_up = PeerId::random();
        assert_eq!(Part::of(at, Some(real)), Part::Other);
        vouch(at, Some(real));
        assert_eq!(Part::of(at, Some(real)), Part::Vouched);
        assert_eq!(Part::of(at, Some(made_up)), Part::Other);
        assert_eq!(Part::of(at, None), Part::Other);
        assert_eq!(
            Part::of(IpAddr::from([198, 51, 100, 78]), Some(real)),
            Part::Other
        );
        let room = DialRoom::default();
        let _real = ready(&mut room.take(at, Some(real), DIAL_HOLDS)).unwrap();
        let _made_up = ready(&mut room.take(at, Some(made_up), DIAL_HOLDS)).unwrap();
        assert_eq!(room.held_in(Part::Vouched), (DIAL_HOLDS, 0));
        assert_eq!(room.held_now(), (DIAL_HOLDS, 0));
    }

    /// A change of the device's networks forgets what answered and the node's vouching, and
    /// gives back what lingers.
    #[test]
    fn a_change_of_networks_forgets_answers_vouches_and_lingering_room() {
        let _vouching = VOUCHING.lock().unwrap_or_else(PoisonError::into_inner);
        let at = IpAddr::from([198, 51, 100, 90]);
        let peer = PeerId::random();
        vouch(at, Some(peer));
        let room = DialRoom::default();
        let dial = ready(&mut take(&room, ip(40), DIAL_HOLDS)).unwrap();
        drop(dial);
        assert_eq!(room.held_now(), (DIAL_HOLDS, 0), "it did not linger");
        room.replied(ip(41));
        room.forget_network();
        assert_eq!(room.held_now(), (0, 0));
        assert!(!room.answers(&ip(41)));
        assert_eq!(Part::of(at, Some(peer)), Part::Other);
    }

    /// A dial that goes away while it waits leaves the queue, and one that goes away with
    /// its turn given gives the room back.
    #[test]
    fn a_dial_that_goes_away_leaves_no_room_taken() {
        let room = DialRoom::default();
        let first: Vec<Held> = (0..2)
            .filter_map(|_| ready(&mut take(&room, ip(1), DIAL_HOLDS)))
            .collect();
        let waiting = take(&room, ip(1), DIAL_HOLDS);
        assert_eq!(room.held_now(), (2 * DIAL_HOLDS, 1));
        drop(waiting);
        assert_eq!(room.held_now(), (2 * DIAL_HOLDS, 0));

        let given = take(&room, ip(1), DIAL_HOLDS);
        drop(first);
        linger_over(&room);
        room.settle(|_| {});
        // Its turn was handed to it; dropped unclaimed, it sent nothing: its room comes
        // back at once.
        assert_eq!(room.held_now(), (DIAL_HOLDS, 0));
        drop(given);
        assert_eq!(room.held_now(), (0, 0));
    }

    /// An address that set up a connection needs no turn while it is open and for a while
    /// after it ended.
    #[test]
    fn an_address_that_answered_needs_no_turn() {
        let room = DialRoom::default();
        assert!(!room.answers(&ip(1)));
        let open = room.answered(ip(1));
        assert!(room.answers(&ip(1)));
        drop(open);
        assert!(room.answers(&ip(1)));
        room.lock().answered.get_mut(&ip(1)).unwrap().1 =
            Instant::now().checked_sub(ANSWERED_FOR).unwrap();
        assert!(!room.answers(&ip(1)));
    }

    /// A dial waiting for room that lingers is woken when it comes back, with nothing
    /// else happening.
    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn a_waiting_dial_is_woken_when_lingering_room_comes_back() {
        let room = DialRoom::default();
        let first: Vec<Held> = (0..2)
            .filter_map(|_| ready(&mut take(&room, ip(1), DIAL_HOLDS)))
            .collect();
        let waiting = take(&room, ip(1), DIAL_HOLDS);
        drop(first);
        let started = Instant::now();
        let held = tokio::time::timeout(LINGER * 2, waiting).await;
        assert!(held.is_ok(), "the dial was never woken");
        assert!(started.elapsed() >= LINGER - Duration::from_millis(50));
    }
}
