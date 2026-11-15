//! Fogata addition: a bound on what dials to the device's own networks can leave waiting in
//! the kernel.
//!
//! The device sends everything from one UDP socket: requests, the DHT and a call's media.
//! On Linux a packet to an address on one of the device's own networks waits in the kernel
//! while the neighbour is looked for (ARP, or neighbour discovery for IPv6), and counts
//! against the socket's send buffer until it leaves or is dropped. Where no host answers
//! for the address, it waits until the kernel gives up, and once the socket's buffer is
//! full it sends nothing to anyone: a call's sound and picture stop. Anyone who answers a
//! DHT lookup can name made-up peers at such addresses, as many as it likes, so the dials to
//! addresses of the device's own networks that have not answered yet take turns here: each
//! holds a share of [`IN_ALL`] (and of [`PER_ADDRESS`] for its address) while it runs and
//! for [`LINGER`] after, unless its address answered, and the others wait for room. An
//! address that set up a connection lately (see [`ANSWERED_FOR`]) needs no turn, nor does
//! the loopback or any address off those networks: a router answers for those.
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
//!   ask for is capped by `net.core.wmem_max`, the same by default. [`IN_ALL`], eight dials
//!   at once, is about half of the default buffer: whatever a lookup's answers name, the
//!   other half stays for everything else, without counting on a larger buffer.
//!
//! The same turns are kept on every system, so the device behaves alike everywhere; they cost
//! nothing but a wait for room when more than eight new addresses of the local network are
//! dialled at once and some never answer.

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    net::IpAddr,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::{FutureExt, channel::oneshot};
use futures_timer::Delay;

/// The packets a QUIC dial can leave waiting for a neighbour at one time.
pub(crate) const DIAL_HOLDS: usize = 14 * 1024;
/// The packets a hole punch can leave waiting for a neighbour at one time.
pub(crate) const PUNCH_HOLDS: usize = 32 * 1024;
/// What the dials to one address may hold at once: two dials, or one punch.
pub(crate) const PER_ADDRESS: usize = 32 * 1024;
/// What the dials to every such address may hold at once.
pub(crate) const IN_ALL: usize = 8 * DIAL_HOLDS;
/// How long a dial whose address did not answer still holds its room after it ends: its
/// last packets may still wait for the neighbour that long.
pub(crate) const LINGER: Duration = Duration::from_millis(3500);
/// How long after its last connection ended an address still needs no turn.
pub(crate) const ANSWERED_FOR: Duration = Duration::from_secs(10 * 60);
/// Addresses that answered, kept at most; the one that answered longest ago goes first.
const MAX_ANSWERED: usize = 256;

/// The room dials to the device's own networks share (see the module's notes).
#[derive(Clone, Debug, Default)]
pub(crate) struct DialRoom(Arc<Mutex<Room>>);

#[derive(Debug, Default)]
struct Room {
    held: usize,
    by_address: HashMap<IpAddr, usize>,
    /// Room of dials that ended, given back at the time noted.
    lingering: Vec<(Instant, IpAddr, usize)>,
    /// Dials waiting for room, in the order they came.
    waiting: VecDeque<Waiter>,
    next_waiter: u64,
    /// Addresses that set up a connection: how many of them are open, and when the last
    /// one ended (or began, while one is open).
    answered: HashMap<IpAddr, (usize, Instant)>,
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

    fn give_back(&mut self, ip: IpAddr, cost: usize) {
        self.held = self.held.saturating_sub(cost);
        if let Some(at) = self.by_address.get_mut(&ip) {
            *at = at.saturating_sub(cost);
            if *at == 0 {
                self.by_address.remove(&ip);
            }
        }
    }

    /// Gives back the room of the dials whose time to linger is over.
    fn expire(&mut self, now: Instant) {
        let mut i = 0;
        while i < self.lingering.len() {
            if self.lingering[i].0 <= now {
                let (_, ip, cost) = self.lingering.swap_remove(i);
                self.give_back(ip, cost);
            } else {
                i += 1;
            }
        }
    }
}

impl DialRoom {
    fn lock(&self) -> MutexGuard<'_, Room> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether `ip` set up a connection that is still open, or ended within
    /// [`ANSWERED_FOR`].
    pub(crate) fn answers(&self, ip: &IpAddr) -> bool {
        self.lock().answers(ip)
    }

    /// Notes that `ip` set up a connection, until the returned guard is dropped with it.
    pub(crate) fn answered(&self, ip: IpAddr) -> Answered {
        let mut room = self.lock();
        let entry = room.answered.entry(ip).or_insert((0, Instant::now()));
        entry.0 += 1;
        entry.1 = Instant::now();
        if room.answered.len() > MAX_ANSWERED {
            let oldest = room
                .answered
                .iter()
                .filter(|(_, (open, _))| *open == 0)
                .min_by_key(|(_, (_, since))| *since)
                .map(|(ip, _)| *ip);
            if let Some(oldest) = oldest {
                room.answered.remove(&oldest);
            }
        }
        Answered {
            room: self.clone(),
            ip,
        }
    }

    /// A turn to dial `ip` with `cost` bytes of packets that may wait: at once when there
    /// is room, else when dials before it give theirs back. Among those waiting whose
    /// address has room, the one whose address holds least goes first (the earliest of
    /// those), so made-up peers at a few addresses do not keep a real one at another address
    /// waiting behind all of theirs; and none goes past it, so a punch is not kept waiting
    /// by dials.
    pub(crate) fn take(&self, ip: IpAddr, cost: usize) -> Turn {
        let (receiver, id, granted) = {
            let mut room = self.lock();
            let (sender, receiver) = oneshot::channel();
            let id = room.next_waiter;
            room.next_waiter += 1;
            room.waiting.push_back(Waiter {
                id,
                ip,
                cost,
                turn: sender,
            });
            room.expire(Instant::now());
            (receiver, id, self.grant(&mut room))
        };
        hand_out(granted);
        Turn {
            room: self.clone(),
            id,
            receiver,
            timer: None,
        }
    }

    /// Takes the waiters that fit now out of the queue, with their room counted.
    fn grant(&self, room: &mut Room) -> Granted {
        let mut granted = Vec::new();
        loop {
            let best = room
                .waiting
                .iter()
                .enumerate()
                .filter_map(|(i, w)| {
                    let at = room.by_address.get(&w.ip).copied().unwrap_or(0);
                    (at + w.cost <= PER_ADDRESS).then_some((at, i))
                })
                .min()
                .map(|(_, i)| i)
                .filter(|i| room.held + room.waiting[*i].cost <= IN_ALL);
            let Some(waiter) = best.and_then(|i| room.waiting.remove(i)) else {
                return granted;
            };
            room.held += waiter.cost;
            *room.by_address.entry(waiter.ip).or_default() += waiter.cost;
            granted.push((
                waiter.turn,
                Held {
                    room: self.clone(),
                    ip: waiter.ip,
                    cost: waiter.cost,
                },
            ));
        }
    }

    /// The room of a dial that ended: given back now when its address answered, else once
    /// its packets can no longer be waiting.
    fn release(&self, ip: IpAddr, cost: usize) {
        let granted = {
            let mut room = self.lock();
            let now = Instant::now();
            if room.answers(&ip) {
                room.give_back(ip, cost);
            } else {
                room.lingering.push((now + LINGER, ip, cost));
            }
            room.expire(now);
            self.grant(&mut room)
        };
        hand_out(granted);
    }

    #[cfg(test)]
    fn held(&self) -> (usize, usize) {
        let room = self.lock();
        (room.held, room.waiting.len())
    }
}

/// Hands each turn to its dial, with the room's lock let go: a dial that went away drops
/// its turn, which gives the room back.
fn hand_out(granted: Granted) {
    for (sender, held) in granted {
        let _ = sender.send(held);
    }
}

/// A dial's room, given back when it is dropped (see [`DialRoom::release`]).
#[derive(Debug)]
pub(crate) struct Held {
    room: DialRoom,
    ip: IpAddr,
    cost: usize,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.room.release(self.ip, self.cost);
    }
}

/// A dial waiting for room; dropped, it leaves the queue.
#[derive(Debug)]
pub(crate) struct Turn {
    room: DialRoom,
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
        let id = self.id;
        self.room.lock().waiting.retain(|w| w.id != id);
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

    fn ip(last: u8) -> IpAddr {
        IpAddr::from([192, 168, 1, last])
    }

    fn ready(turn: &mut Turn) -> Option<Held> {
        match Pin::new(turn).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(held) => Some(held),
            Poll::Pending => None,
        }
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

    /// Dials to one address take two places at once, dials to all of them eight, and the
    /// others wait for one to give its room back, which one whose address did not answer
    /// does only once its packets can no longer be waiting.
    #[test]
    fn dials_to_addresses_that_have_not_answered_wait_for_room() {
        let room = DialRoom::default();
        let mut turns: Vec<Turn> = (0..3).map(|_| room.take(ip(10), DIAL_HOLDS)).collect();
        let mut held: Vec<Held> = turns.iter_mut().filter_map(ready).collect();
        assert_eq!(held.len(), 2, "one address held more than two dials");
        for last in 11..20 {
            turns.push(room.take(ip(last), DIAL_HOLDS));
        }
        held.extend(turns.iter_mut().skip(3).filter_map(ready));
        assert_eq!(
            held.len(),
            IN_ALL / DIAL_HOLDS,
            "more dials than the room holds"
        );
        assert_eq!(room.held(), (IN_ALL, 4));

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
        let dial = ready(&mut room.take(ip(1), DIAL_HOLDS)).unwrap();
        let open = room.answered(ip(1));
        drop(dial);
        assert_eq!(room.held(), (0, 0));
        drop(open);
    }

    /// A punch, which takes an address's whole share, is not passed by dials that came
    /// after it.
    #[test]
    fn a_punch_is_not_passed_by_later_dials() {
        let room = DialRoom::default();
        let mut held = Vec::new();
        for last in [1, 2, 3, 4] {
            for _ in 0..2 {
                held.extend(ready(&mut room.take(ip(last), DIAL_HOLDS)));
            }
        }
        assert_eq!(room.held(), (IN_ALL, 0));
        let mut punch = room.take(ip(9), PUNCH_HOLDS);
        let mut dial = room.take(ip(8), DIAL_HOLDS);
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

    /// A dial whose address holds nothing goes before those of addresses that hold some.
    #[test]
    fn an_address_that_holds_nothing_goes_first() {
        let room = DialRoom::default();
        let mut held = Vec::new();
        for last in [1, 2, 3, 4] {
            for _ in 0..2 {
                held.extend(ready(&mut room.take(ip(last), DIAL_HOLDS)));
            }
        }
        assert_eq!(held.len(), 8);
        // Made-up peers at an address that will hold one dial, and a real one at another.
        let mut made_up: Vec<Turn> = (0..20).map(|_| room.take(ip(4), DIAL_HOLDS)).collect();
        let mut real = room.take(ip(6), DIAL_HOLDS);
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

    /// A dial that goes away while it waits leaves the queue, and one that goes away with
    /// its turn given gives the room back.
    #[test]
    fn a_dial_that_goes_away_leaves_no_room_taken() {
        let room = DialRoom::default();
        let settle = |room: &DialRoom| {
            linger_over(room);
            let granted = {
                let mut r = room.lock();
                r.expire(Instant::now());
                room.grant(&mut r)
            };
            hand_out(granted);
        };
        let first: Vec<Held> = (0..2)
            .filter_map(|_| ready(&mut room.take(ip(1), DIAL_HOLDS)))
            .collect();
        let waiting = room.take(ip(1), DIAL_HOLDS);
        assert_eq!(room.held(), (2 * DIAL_HOLDS, 1));
        drop(waiting);
        assert_eq!(room.held(), (2 * DIAL_HOLDS, 0));

        let given = room.take(ip(1), DIAL_HOLDS);
        drop(first);
        settle(&room);
        // Its turn was handed to it; dropped unclaimed, its room lingers and comes back.
        assert_eq!(room.held(), (DIAL_HOLDS, 0));
        drop(given);
        assert_eq!(room.held(), (DIAL_HOLDS, 0));
        settle(&room);
        assert_eq!(room.held(), (0, 0));
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
            .filter_map(|_| ready(&mut room.take(ip(1), DIAL_HOLDS)))
            .collect();
        let waiting = room.take(ip(1), DIAL_HOLDS);
        drop(first);
        let started = Instant::now();
        let held = tokio::time::timeout(LINGER * 2, waiting).await;
        assert!(held.is_ok(), "the dial was never woken");
        assert!(started.elapsed() >= LINGER - Duration::from_millis(50));
    }
}
