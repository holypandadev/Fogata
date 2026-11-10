//! Limits on what peers may make this device do: requests any member or friend can send as
//! often as it likes, and that cost this device more than they cost the sender.

use std::{
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

use fogata_crypto::DevicePublicKey;
use fogata_friends::{Animation, Banner, Bio, FriendBook, Profile};
use libp2p::PeerId;

/// Call signals (offers, answers, network candidates) one member may send at once, and how
/// many more each second. Setting up a call with one peer takes a few dozen at most.
const SIGNAL_BURST: f64 = 32.0;
const SIGNALS_PER_SECOND: f64 = 8.0;
/// Call signals from a member that is not (yet) in this device's room: as many as the
/// application keeps waiting for its join to come (four), then one every five seconds.
const EARLY_SIGNAL_BURST: f64 = 4.0;
const EARLY_SIGNALS_PER_SECOND: f64 = 0.2;
/// Longest call signal taken: a description of a call with a camera, a screen and its sound
/// takes a few kilobytes.
pub(super) const MAX_SIGNAL_BYTES: usize = 64 * 1024;

/// Steps of direct calls (ring, answer, end, the call's signals) one friend may send at
/// once, and how many more each second: setting up a call takes a few dozen at most, and
/// each one goes to the application, which shows a ringing call for a ring.
const CALL_STEP_BURST: f64 = 64.0;
const CALL_STEPS_PER_SECOND: f64 = 16.0;

/// Searches of every key a group ever had, for a peer's challenge that names none of the
/// newest: a few at once, then one every 15 seconds, per peer.
const DEEP_SEARCH_BURST: f64 = 4.0;
const DEEP_SEARCHES_PER_SECOND: f64 = 1.0 / 15.0;
/// The same from every peer that is not a member together: each new connection can come
/// with a new peer, and a search costs one check per tag for every key of every group
/// (164 µs with 50 groups of 300 epochs in a release build). Strangers at new peers can
/// drain this, so a peer vouched for by something a stranger cannot make up (see
/// [`Vouch`]) takes from a bucket of its voucher's instead: a member coming back after
/// several key changes, which only such a search can recognise, still gets in.
const DEEP_SEARCH_ALL_BURST: f64 = 64.0;
const DEEP_SEARCHES_ALL_PER_SECOND: f64 = 4.0;
const DEEP_SEARCH_VOUCHED_BURST: f64 = 16.0;
const DEEP_SEARCHES_VOUCHED_PER_SECOND: f64 = 1.0;
/// Vouchers whose deep-search budget is kept; past it, those whose bucket is full again
/// go, and while none is, a vouched peer takes from the strangers' bucket.
const DEEP_SEARCH_VOUCHERS_KEPT: usize = 1024;
/// Of those, the addresses on the local network, and the peers through one member's relay:
/// a neighbour can take several addresses, and one coming through a relay several peers
/// (each one verified by that member, see behaviour.rs), each of which gets a bucket of its
/// own; past these, the same as past [`DEEP_SEARCH_VOUCHERS_KEPT`].
const DEEP_SEARCH_LAN_KEPT: usize = 64;
const DEEP_SEARCH_PER_RELAY_KEPT: usize = 32;
/// The searches of every key strangers at one address (an IPv6 /64) may take from the
/// strangers' bucket, whatever peers they come as: one machine making up a peer for each
/// connection would otherwise drain it alone, and a member back from a new address after
/// several key changes would not get in. It takes some sixty addresses to drain it now.
const DEEP_SEARCH_ADDRESS_BURST: f64 = 8.0;
const DEEP_SEARCHES_ADDRESS_PER_SECOND: f64 = 1.0 / 15.0;
/// Addresses whose deep-search budget is kept; past it, those whose bucket is full again go,
/// and while none is, a stranger at a new address waits.
const DEEP_SEARCH_ADDRESSES_KEPT: usize = 1024;

/// What tells a peer that is not a member yet apart from any stranger, for the searches
/// of every key: the address it comes from is one a member or friend was reached at (or
/// is connected from), or it is on the local network, or it comes through a member's
/// relay, which serves members only. A stranger on the internet can bring none of these;
/// one behind a member's address gets only that address's share. On the local network each
/// address has a share, and through a relay each peer: one sharing them all would let a
/// neighbour, or anyone the relay serves, use them up for a member coming back that way.
/// So that a neighbour with many addresses, or one many peers through a relay, cannot make
/// them many, only so many are kept of each ([`DEEP_SEARCH_LAN_KEPT`],
/// [`DEEP_SEARCH_PER_RELAY_KEPT`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Vouch {
    /// An IPv6 address counts by its /64.
    Address(std::net::IpAddr),
    /// The address on the local network; an IPv6 one counts by its /64.
    Lan(std::net::IpAddr),
    /// The member whose relay the peer comes through, and the peer.
    Relay(PeerId, PeerId),
}

impl Vouch {
    /// Whether `self` and `other` are of one kind kept only so many of: both on the local
    /// network, or both through one relay.
    fn shares_room_with(&self, other: &Vouch) -> bool {
        match (self, other) {
            (Vouch::Lan(_), Vouch::Lan(_)) => true,
            (Vouch::Relay(a, _), Vouch::Relay(b, _)) => a == b,
            _ => false,
        }
    }

    fn room(&self) -> usize {
        match self {
            Vouch::Address(_) => DEEP_SEARCH_VOUCHERS_KEPT,
            Vouch::Lan(_) => DEEP_SEARCH_LAN_KEPT,
            Vouch::Relay(..) => DEEP_SEARCH_PER_RELAY_KEPT,
        }
    }
}

/// Requests from peers that are neither members nor friends here (yet), from one address
/// (or through one member's relay, see [`Source`]), answered at once and each second. A
/// stranger's `Challenge` makes this device try the newest keys of every group (15 µs in a
/// release build with 50 groups), its `Hello` the secret of every friend, and nothing else
/// holds back how many it makes; a few devices behind one address that connect at once ask
/// a few dozen each, one for each group they share.
const STRANGER_REQUEST_BURST: f64 = 256.0;
const STRANGER_REQUESTS_PER_SECOND: f64 = 32.0;
/// Addresses whose strangers' budget is kept; past it, those whose bucket is full again go,
/// and while none is, a new address waits. An IPv6 address counts by its /64, which one
/// machine has whole.
const STRANGER_SOURCES_KEPT: usize = 1024;

/// Where a peer that is neither a member nor a friend here (yet) talks to this device from,
/// for the budget of its requests: the address of its own connection, or the member whose
/// relay its connection comes through. Those through one relay share that relay's budget,
/// as those behind one address share the address's: a stranger can come through a member's
/// slot with any number of peers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Source {
    /// An IPv6 address counts by its /64.
    Address(std::net::IpAddr),
    Relay(PeerId),
    /// A relay that could not be told: all such count as one.
    SomeRelay,
}

/// Group log entries one member or friend may make this device check and keep at once, and
/// how many more each second, by count and by bytes. Each costs a signature check (about 50
/// microseconds) on the node's one task, so a message full of entries could hold every group
/// and call up for seconds, and every entry is kept for good. A group's whole log is a few
/// hundred entries and a few hundred kilobytes; honest members add a few a day, and the rest
/// of a longer log comes with later syncs.
const ENTRY_BURST: f64 = 4096.0;
const ENTRIES_PER_SECOND: f64 = 1.0;
const ENTRY_BYTES_BURST: f64 = 4.0 * 1024.0 * 1024.0;
const ENTRY_BYTES_PER_SECOND: f64 = 4.0 * 1024.0;
/// What each entry that was judged and refused costs its sender of that budget, besides its
/// own place: judging one may take working out the whole group (10 to 27 ms in a log of
/// 4096 entries), as much as hundreds of signature checks, and an honest device sends none.
/// The budget may go below nothing by a burst's worth, so a sender of nothing but refused
/// entries gets one judged a minute, not one a second.
const REFUSED_ENTRY_COST: f64 = 64.0;

/// Group messages (`Deliver`, `Spread`) one connection may hand this device at once, and how
/// many more each second, by count and by bytes: it passes on the other members' messages
/// too.
const PEER_MESSAGE_BURST: f64 = 1024.0;
const PEER_MESSAGES_PER_SECOND: f64 = 128.0;
const PEER_MESSAGE_BYTES_BURST: f64 = 16.0 * 1024.0 * 1024.0;
const PEER_MESSAGE_BYTES_PER_SECOND: f64 = 2.0 * 1024.0 * 1024.0;
/// Group messages one member may write, as counted when they open: a burst covers a room
/// joined with dozens of files shared at once (each with a picture of up to 64 KiB); past
/// it, this device neither shows nor passes on that member's messages.
const AUTHOR_MESSAGE_BURST: f64 = 256.0;
const AUTHOR_MESSAGES_PER_SECOND: f64 = 32.0;
const AUTHOR_MESSAGE_BYTES_BURST: f64 = 8.0 * 1024.0 * 1024.0;
const AUTHOR_MESSAGE_BYTES_PER_SECOND: f64 = 512.0 * 1024.0;
/// Room events (`RoomLog` answers) one member may make this device open at once, and how
/// many more each second: each is a signature check, whether or not it was seen, as asked
/// for again. A burst is a little more than a group's rooms can hold, asked for whole when
/// connecting, and a member is asked for them at most every 15 seconds.
const ROOM_LOG_BURST: f64 = 8192.0;
const ROOM_LOG_PER_SECOND: f64 = 8192.0 / 60.0;
/// What one member may make this device send it in answers to its asks for the log or a
/// room's past (`Sync`, `Roster`, `RoomSync`, `RoomEvents`), in bytes, at once and each
/// second after: each answer is built on the node's one task, and a room's past alone may
/// fill a 1 MiB answer. A burst covers a member that comes back after a long time away
/// (the entries it lacks and every room's events); after it, about a room's past a minute.
/// Every answer counts at least [`SERVED_LEAST`], so asks that bring back little are
/// bounded too. Kept per device across connections; an answer past it is refused and asked
/// for again later.
const SERVED_BYTES_BURST: f64 = 32.0 * 1024.0 * 1024.0;
const SERVED_BYTES_PER_SECOND: f64 = 256.0 * 1024.0;
pub(super) const SERVED_LEAST: usize = 4 * 1024;
/// Longest sealed group message taken: the largest a device writes is a file shared with a
/// video's first picture (64 KiB, as base64 in the payload).
pub(super) const MAX_GROUP_MESSAGE_BYTES: usize = 192 * 1024;

/// A group's banners and picture records one member may hand this device, in bytes (each
/// counts at least [`GROUP_RECORD_LEAST`]), at once and each second after. Each is decoded,
/// hashed and its signature checked on the node's one task before it is known whether it is
/// newer than the one here, and a banner is up to 550 KiB: a member could otherwise keep
/// the loop busy with them at the speed of its link. A burst covers a member that comes back
/// with a new banner and picture for each of a few dozen groups; honest ones change rarely.
const GROUP_RECORD_BYTES_BURST: f64 = 16.0 * 1024.0 * 1024.0;
const GROUP_RECORD_BYTES_PER_SECOND: f64 = 64.0 * 1024.0;
const GROUP_RECORD_LEAST: usize = 4 * 1024;

/// Hellos one friend's device may send while online, at once and each second after: an
/// honest one says hello once per connection, and each answer is sealed and signed.
const FRIEND_HELLO_BURST: f64 = 8.0;
const FRIEND_HELLOS_PER_SECOND: f64 = 0.1;
/// This device's profile (up to 64 KiB) goes in an answer to a friend's hello at most this
/// often to each friend: a friend saying it has an older one over and over would otherwise
/// pull it again with each hello.
const HELLO_PROFILE_AGAIN: Duration = Duration::from_secs(60);
/// Groups and direct chats one friend's device may offer, in parts, at once and each second
/// after: each is judged off the loop and may become an event for the application.
const FRIEND_OFFER_BURST: f64 = 64.0;
const FRIEND_OFFERS_PER_SECOND: f64 = 1.0;

/// Peers new to this device that mDNS may tell of, from one address on the local network
/// and from all of them: a burst, then a few each minute. Each is dialled and asked who it
/// is, and anyone there can make up any number; a home or an office has a few devices.
pub(super) const MDNS_PEER_BURST: f64 = 8.0;
const MDNS_PEERS_PER_SECOND: f64 = 8.0 / 60.0;
const MDNS_ALL_BURST: f64 = 32.0;
const MDNS_ALL_PER_SECOND: f64 = 32.0 / 60.0;
/// Addresses whose mDNS budget is kept; past it, those whose bucket is full again go.
const MDNS_SOURCES_KEPT: usize = 256;

/// A friend's profile, banner or bio, or a member's profile, is taken at most twice in this
/// long from each (one on connecting, and a change right after it): each one rewrites a file
/// that holds every friend's pictures.
const UPDATE_EVERY: Duration = Duration::from_secs(10);
const UPDATES_AT_ONCE: u8 = 2;

/// How many requests a peer may still make now: `burst` at once, then `per_second`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bucket {
    tokens: f64,
    at: Instant,
}

impl Bucket {
    fn full(burst: f64, now: Instant) -> Self {
        Self {
            tokens: burst,
            at: now,
        }
    }

    /// Takes one request, if one is allowed now.
    fn take(&mut self, now: Instant, burst: f64, per_second: f64) -> bool {
        self.take_cost(1.0, now, burst, per_second)
    }

    /// Takes something that costs `cost`, if that much is allowed now.
    fn take_cost(&mut self, cost: f64, now: Instant, burst: f64, per_second: f64) -> bool {
        self.refill(now, burst, per_second);
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    /// Takes as many of `wanted` as are allowed now; returns how many.
    fn take_up_to(&mut self, wanted: usize, now: Instant, burst: f64, per_second: f64) -> usize {
        self.refill(now, burst, per_second);
        let taken = (self.tokens.max(0.0).floor() as usize).min(wanted);
        self.tokens -= taken as f64;
        taken
    }

    fn refill(&mut self, now: Instant, burst: f64, per_second: f64) {
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64();
        self.tokens = (self.tokens + elapsed * per_second).min(burst);
        self.at = now;
    }
}

/// Takes one from `key`'s bucket in `buckets`, kept up to 1024 of them (those full again go
/// first).
fn take_keyed<K: Hash + Eq>(
    buckets: &mut HashMap<K, Bucket>,
    key: K,
    now: Instant,
    burst: f64,
    per_second: f64,
) -> bool {
    if buckets.len() > 1024 {
        buckets.retain(|_, bucket| {
            bucket.refill(now, burst, per_second);
            bucket.tokens < burst
        });
    }
    buckets
        .entry(key)
        .or_insert_with(|| Bucket::full(burst, now))
        .take(now, burst, per_second)
}

/// Who a budget is kept for: a device once it is known (a member or a friend), so that
/// connecting again does not fill it up, else the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Sender {
    Device(DevicePublicKey),
    Peer(PeerId),
}

/// Records each sender signs with a version of its own, taken at most [`UPDATES_AT_ONCE`]
/// times per [`UPDATE_EVERY`] from each; one that comes past that waits for the next turn,
/// and a newer one that comes meanwhile takes its place.
pub(super) struct Held<K, V> {
    slots: HashMap<K, Turn<V>>,
}

/// When a sender's current turn began, how many were taken in it, and the one waiting with
/// its version.
type Turn<V> = (Instant, u8, Option<(u64, V)>);

impl<K, V> Default for Held<K, V> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash, V> Held<K, V> {
    /// `value` (of `version`) if it may be taken now; else it waits (see [`Self::due`]).
    pub(super) fn offer(&mut self, key: K, version: u64, value: V, now: Instant) -> Option<V> {
        let Some((began, taken, waiting)) = self.slots.get_mut(&key) else {
            self.slots.insert(key, (now, 1, None));
            return Some(value);
        };
        if now.saturating_duration_since(*began) >= UPDATE_EVERY {
            (*began, *taken) = (now, 0);
        }
        if waiting.as_ref().is_none_or(|(v, _)| version > *v) {
            *waiting = Some((version, value));
        }
        if *taken >= UPDATES_AT_ONCE {
            return None;
        }
        *taken += 1;
        waiting.take().map(|(_, value)| value)
    }

    /// The values whose turn has come, to take now.
    pub(super) fn due(&mut self, now: Instant) -> Vec<V> {
        let mut out = Vec::new();
        self.slots.retain(|_, (began, taken, waiting)| {
            if now.saturating_duration_since(*began) < UPDATE_EVERY {
                return true;
            }
            match waiting.take() {
                Some((_, value)) => {
                    out.push(value);
                    (*began, *taken) = (now, 1);
                    true
                }
                None => false,
            }
        });
        out
    }
}

/// A friend's signed record that changes its look in the friend book.
pub(super) enum FriendUpdate {
    Profile(Profile),
    Banner(Banner),
    Bio(Bio),
}

impl FriendUpdate {
    fn key(&self) -> (DevicePublicKey, u8) {
        match self {
            Self::Profile(p) => (p.device, 0),
            Self::Banner(b) => (b.device, 1),
            Self::Bio(b) => (b.device, 2),
        }
    }

    fn version(&self) -> u64 {
        match self {
            Self::Profile(p) => p.version_ms,
            Self::Banner(b) => b.version_ms,
            Self::Bio(b) => b.version_ms,
        }
    }

    /// Whether it is newer than what `book` has of that friend.
    pub(super) fn is_news(&self, book: &FriendBook) -> bool {
        let (device, _) = self.key();
        let Some(friend) = book.friends.get(&device) else {
            return false;
        };
        let known = match self {
            Self::Profile(_) => Some(friend.profile.version_ms),
            Self::Banner(_) => friend.banner.as_ref().map(|b| b.version_ms),
            Self::Bio(_) => friend.bio.as_ref().map(|b| b.version_ms),
        };
        known.is_none_or(|v| self.version() > v)
    }

    /// Puts it in `book`; returns whether anything changed.
    pub(super) fn apply(self, book: &mut FriendBook) -> bool {
        match self {
            Self::Profile(p) => book.update_profile(p),
            Self::Banner(b) => book.update_banner(b),
            Self::Bio(b) => book.update_bio(b),
        }
    }
}

/// What each connected peer may still send.
#[derive(Default)]
pub(super) struct Limits {
    signals: HashMap<PeerId, Bucket>,
    early_signals: HashMap<PeerId, Bucket>,
    deep_searches: HashMap<PeerId, Bucket>,
    deep_searches_all: Option<Bucket>,
    deep_searches_vouched: HashMap<Vouch, Bucket>,
    deep_search_addresses: HashMap<std::net::IpAddr, Bucket>,
    call_steps: HashMap<DevicePublicKey, Bucket>,
    entries: HashMap<Sender, (Bucket, Bucket)>,
    peer_messages: HashMap<PeerId, (Bucket, Bucket)>,
    author_messages: HashMap<DevicePublicKey, (Bucket, Bucket)>,
    room_logs: HashMap<Sender, Bucket>,
    served: HashMap<DevicePublicKey, Bucket>,
    group_records: HashMap<DevicePublicKey, Bucket>,
    friend_hellos: HashMap<DevicePublicKey, Bucket>,
    hello_profiles: HashMap<DevicePublicKey, Instant>,
    friend_offers: HashMap<DevicePublicKey, Bucket>,
    friend_updates: Held<(DevicePublicKey, u8), FriendUpdate>,
    member_profiles: Held<DevicePublicKey, Profile>,
    /// Animations of a picture or banner, from friends and members alike.
    animations: Held<(DevicePublicKey, fogata_friends::Animated), Animation>,
    mdns_sources: HashMap<std::net::IpAddr, Bucket>,
    mdns_all: Option<Bucket>,
    stranger_sources: HashMap<Source, Bucket>,
}

impl Limits {
    /// Whether a call signal from `peer` is taken now.
    pub(super) fn take_signal(&mut self, peer: PeerId, now: Instant) -> bool {
        self.signals
            .entry(peer)
            .or_insert_with(|| Bucket::full(SIGNAL_BURST, now))
            .take(now, SIGNAL_BURST, SIGNALS_PER_SECOND)
    }

    /// How many of `entries` (encoded group log entries, in order) from `sender` may be
    /// checked now, the first ones; the rest are left for a later sync.
    pub(super) fn take_entries(
        &mut self,
        sender: Sender,
        entries: &[Vec<u8>],
        now: Instant,
    ) -> usize {
        let (count, size) = self.entries.entry(sender).or_insert_with(|| {
            (
                Bucket::full(ENTRY_BURST, now),
                Bucket::full(ENTRY_BYTES_BURST, now),
            )
        });
        size.refill(now, ENTRY_BYTES_BURST, ENTRY_BYTES_PER_SECOND);
        let mut room = size.tokens;
        let fit = entries
            .iter()
            .take_while(|e| {
                room -= e.len() as f64;
                room >= 0.0
            })
            .count();
        let taken = count.take_up_to(fit, now, ENTRY_BURST, ENTRIES_PER_SECOND);
        let bytes: usize = entries[..taken].iter().map(Vec::len).sum();
        size.tokens -= bytes as f64;
        taken
    }

    /// Charges `sender` for `refused` entries of its own that were judged and refused (see
    /// [`REFUSED_ENTRY_COST`]), if it ever sent entries here: one that never did has no
    /// budget to charge, and none is made for it, so keys made up for entries do not grow
    /// the budgets kept.
    pub(super) fn refused_entries(&mut self, sender: Sender, refused: usize, now: Instant) {
        if refused == 0 {
            return;
        }
        let Some((count, _)) = self.entries.get_mut(&sender) else {
            return;
        };
        count.refill(now, ENTRY_BURST, ENTRIES_PER_SECOND);
        count.tokens = (count.tokens - refused as f64 * REFUSED_ENTRY_COST).max(-ENTRY_BURST);
    }

    /// What is left of `sender`'s budget of entries, for tests.
    #[cfg(test)]
    pub(super) fn entry_tokens(&self, sender: Sender) -> Option<f64> {
        self.entries.get(&sender).map(|(count, _)| count.tokens)
    }

    /// Whether a group message of `bytes` handed over by `peer` is opened now.
    pub(super) fn take_peer_message(&mut self, peer: PeerId, bytes: usize, now: Instant) -> bool {
        let (count, size) = self.peer_messages.entry(peer).or_insert_with(|| {
            (
                Bucket::full(PEER_MESSAGE_BURST, now),
                Bucket::full(PEER_MESSAGE_BYTES_BURST, now),
            )
        });
        size.refill(now, PEER_MESSAGE_BYTES_BURST, PEER_MESSAGE_BYTES_PER_SECOND);
        if size.tokens < bytes as f64 {
            return false;
        }
        if !count.take(now, PEER_MESSAGE_BURST, PEER_MESSAGES_PER_SECOND) {
            return false;
        }
        size.take_cost(
            bytes as f64,
            now,
            PEER_MESSAGE_BYTES_BURST,
            PEER_MESSAGE_BYTES_PER_SECOND,
        )
    }

    /// Whether a group message of `bytes` written by member `author` is shown and passed
    /// on.
    pub(super) fn take_author_message(
        &mut self,
        author: DevicePublicKey,
        bytes: usize,
        now: Instant,
    ) -> bool {
        let (count, size) = self.author_messages.entry(author).or_insert_with(|| {
            (
                Bucket::full(AUTHOR_MESSAGE_BURST, now),
                Bucket::full(AUTHOR_MESSAGE_BYTES_BURST, now),
            )
        });
        size.refill(
            now,
            AUTHOR_MESSAGE_BYTES_BURST,
            AUTHOR_MESSAGE_BYTES_PER_SECOND,
        );
        if size.tokens < bytes as f64 {
            return false;
        }
        if !count.take(now, AUTHOR_MESSAGE_BURST, AUTHOR_MESSAGES_PER_SECOND) {
            return false;
        }
        size.take_cost(
            bytes as f64,
            now,
            AUTHOR_MESSAGE_BYTES_BURST,
            AUTHOR_MESSAGE_BYTES_PER_SECOND,
        )
    }

    /// How many of `wanted` room events from `sender`, answering an ask for a room's past,
    /// are opened now; the rest are left for a later ask.
    pub(super) fn take_room_log(&mut self, sender: Sender, wanted: usize, now: Instant) -> usize {
        self.room_logs
            .entry(sender)
            .or_insert_with(|| Bucket::full(ROOM_LOG_BURST, now))
            .take_up_to(wanted, now, ROOM_LOG_BURST, ROOM_LOG_PER_SECOND)
    }

    /// Whether an ask of member `device` for the log or a room's past is answered now: it is
    /// while anything is left of its budget, and the answer is charged once it is made
    /// ([`Self::charge_served`]), so the last one may take it below nothing.
    pub(super) fn may_serve(&mut self, device: DevicePublicKey, now: Instant) -> bool {
        if self.served.len() > 1024 {
            self.served.retain(|_, bucket| {
                bucket.refill(now, SERVED_BYTES_BURST, SERVED_BYTES_PER_SECOND);
                bucket.tokens < SERVED_BYTES_BURST
            });
        }
        let bucket = self
            .served
            .entry(device)
            .or_insert_with(|| Bucket::full(SERVED_BYTES_BURST, now));
        bucket.refill(now, SERVED_BYTES_BURST, SERVED_BYTES_PER_SECOND);
        bucket.tokens > 0.0
    }

    /// Charges member `device` for an answer of `bytes` made for it.
    pub(super) fn charge_served(&mut self, device: DevicePublicKey, bytes: usize, now: Instant) {
        let bucket = self
            .served
            .entry(device)
            .or_insert_with(|| Bucket::full(SERVED_BYTES_BURST, now));
        bucket.refill(now, SERVED_BYTES_BURST, SERVED_BYTES_PER_SECOND);
        bucket.tokens = (bucket.tokens - bytes.max(SERVED_LEAST) as f64).max(-SERVED_BYTES_BURST);
    }

    /// Whether a group's banner or picture record of `bytes` that member `device` handed
    /// over is looked at now (see [`GROUP_RECORD_BYTES_BURST`]).
    pub(super) fn take_group_record(
        &mut self,
        device: DevicePublicKey,
        bytes: usize,
        now: Instant,
    ) -> bool {
        if self.group_records.len() > 1024 {
            self.group_records.retain(|_, bucket| {
                bucket.refill(now, GROUP_RECORD_BYTES_BURST, GROUP_RECORD_BYTES_PER_SECOND);
                bucket.tokens < GROUP_RECORD_BYTES_BURST
            });
        }
        self.group_records
            .entry(device)
            .or_insert_with(|| Bucket::full(GROUP_RECORD_BYTES_BURST, now))
            .take_cost(
                bytes.max(GROUP_RECORD_LEAST) as f64,
                now,
                GROUP_RECORD_BYTES_BURST,
                GROUP_RECORD_BYTES_PER_SECOND,
            )
    }

    /// Whether a hello from the friend `device`, online already, is answered now.
    pub(super) fn take_friend_hello(&mut self, device: DevicePublicKey, now: Instant) -> bool {
        take_keyed(
            &mut self.friend_hellos,
            device,
            now,
            FRIEND_HELLO_BURST,
            FRIEND_HELLOS_PER_SECOND,
        )
    }

    /// Whether this device's profile goes in an answer to a hello of the friend `device`
    /// now (see [`HELLO_PROFILE_AGAIN`]); if so, it counts as sent.
    pub(super) fn hello_profile(&mut self, device: DevicePublicKey, now: Instant) -> bool {
        if self.hello_profiles.len() > 1024 {
            self.hello_profiles
                .retain(|_, at| now.saturating_duration_since(*at) < HELLO_PROFILE_AGAIN);
        }
        if self
            .hello_profiles
            .get(&device)
            .is_some_and(|at| now.saturating_duration_since(*at) < HELLO_PROFILE_AGAIN)
        {
            return false;
        }
        self.hello_profiles.insert(device, now);
        true
    }

    /// Whether a group or direct chat offered by the friend `device` (or a part of one) is
    /// looked at now.
    pub(super) fn take_friend_offer(&mut self, device: DevicePublicKey, now: Instant) -> bool {
        take_keyed(
            &mut self.friend_offers,
            device,
            now,
            FRIEND_OFFER_BURST,
            FRIEND_OFFERS_PER_SECOND,
        )
    }

    /// Whether a step of a direct call from the friend `device` is taken now.
    pub(super) fn take_call_step(&mut self, device: DevicePublicKey, now: Instant) -> bool {
        if self.call_steps.len() > 256 {
            self.call_steps.retain(|_, bucket| {
                bucket.refill(now, CALL_STEP_BURST, CALL_STEPS_PER_SECOND);
                bucket.tokens < CALL_STEP_BURST
            });
        }
        self.call_steps
            .entry(device)
            .or_insert_with(|| Bucket::full(CALL_STEP_BURST, now))
            .take(now, CALL_STEP_BURST, CALL_STEPS_PER_SECOND)
    }

    /// Whether a call signal from `peer`, not in this device's room, is taken now.
    pub(super) fn take_early_signal(&mut self, peer: PeerId, now: Instant) -> bool {
        self.early_signals
            .entry(peer)
            .or_insert_with(|| Bucket::full(EARLY_SIGNAL_BURST, now))
            .take(now, EARLY_SIGNAL_BURST, EARLY_SIGNALS_PER_SECOND)
    }

    /// Whether a challenge from `peer`, at `from` if its address is known and vouched for by
    /// `vouch` if by anything, may be tried against every key now (see `named_group`).
    pub(super) fn take_deep_search(
        &mut self,
        peer: PeerId,
        from: Option<std::net::IpAddr>,
        vouch: Option<Vouch>,
        now: Instant,
    ) -> bool {
        let by_64 = |ip: std::net::IpAddr| match ip {
            std::net::IpAddr::V6(ip) => {
                std::net::Ipv6Addr::from(u128::from(ip) & !u128::from(u64::MAX)).into()
            }
            v4 => v4,
        };
        let vouch = vouch.map(|v| match v {
            Vouch::Address(ip) => Vouch::Address(by_64(ip)),
            Vouch::Lan(ip) => Vouch::Lan(by_64(ip)),
            other => other,
        });
        let (shared, burst, per_second) = match vouch {
            Some(vouch) => {
                let full = |kept: &HashMap<Vouch, Bucket>| {
                    kept.len() >= DEEP_SEARCH_VOUCHERS_KEPT
                        || kept.keys().filter(|v| v.shares_room_with(&vouch)).count()
                            >= vouch.room()
                };
                let vouchers = &mut self.deep_searches_vouched;
                if !vouchers.contains_key(&vouch) && full(vouchers) {
                    vouchers.retain(|_, bucket| {
                        bucket.refill(
                            now,
                            DEEP_SEARCH_VOUCHED_BURST,
                            DEEP_SEARCHES_VOUCHED_PER_SECOND,
                        );
                        bucket.tokens < DEEP_SEARCH_VOUCHED_BURST
                    });
                }
                if vouchers.contains_key(&vouch) || !full(vouchers) {
                    let bucket = vouchers
                        .entry(vouch)
                        .or_insert_with(|| Bucket::full(DEEP_SEARCH_VOUCHED_BURST, now));
                    (
                        bucket,
                        DEEP_SEARCH_VOUCHED_BURST,
                        DEEP_SEARCHES_VOUCHED_PER_SECOND,
                    )
                } else {
                    Self::deep_searches_all(&mut self.deep_searches_all, now)
                }
            }
            None => Self::deep_searches_all(&mut self.deep_searches_all, now),
        };
        shared.refill(now, burst, per_second);
        if shared.tokens < 1.0 {
            return false;
        }
        // A stranger's address has its own share of the strangers' bucket.
        let address = match (vouch, from) {
            (None, Some(from)) => {
                let from = match from {
                    std::net::IpAddr::V6(ip) => {
                        std::net::Ipv6Addr::from(u128::from(ip) & !u128::from(u64::MAX)).into()
                    }
                    v4 => v4,
                };
                let addresses = &mut self.deep_search_addresses;
                if addresses.len() >= DEEP_SEARCH_ADDRESSES_KEPT && !addresses.contains_key(&from) {
                    addresses.retain(|_, bucket| {
                        bucket.refill(
                            now,
                            DEEP_SEARCH_ADDRESS_BURST,
                            DEEP_SEARCHES_ADDRESS_PER_SECOND,
                        );
                        bucket.tokens < DEEP_SEARCH_ADDRESS_BURST
                    });
                    if addresses.len() >= DEEP_SEARCH_ADDRESSES_KEPT {
                        return false;
                    }
                }
                let bucket = addresses
                    .entry(from)
                    .or_insert_with(|| Bucket::full(DEEP_SEARCH_ADDRESS_BURST, now));
                bucket.refill(
                    now,
                    DEEP_SEARCH_ADDRESS_BURST,
                    DEEP_SEARCHES_ADDRESS_PER_SECOND,
                );
                if bucket.tokens < 1.0 {
                    return false;
                }
                Some(bucket)
            }
            _ => None,
        };
        let taken = self
            .deep_searches
            .entry(peer)
            .or_insert_with(|| Bucket::full(DEEP_SEARCH_BURST, now))
            .take(now, DEEP_SEARCH_BURST, DEEP_SEARCHES_PER_SECOND);
        if taken {
            shared.tokens -= 1.0;
            if let Some(address) = address {
                address.tokens -= 1.0;
            }
        }
        taken
    }

    /// The bucket every stranger's deep searches come from.
    fn deep_searches_all(all: &mut Option<Bucket>, now: Instant) -> (&mut Bucket, f64, f64) {
        (
            all.get_or_insert_with(|| Bucket::full(DEEP_SEARCH_ALL_BURST, now)),
            DEEP_SEARCH_ALL_BURST,
            DEEP_SEARCHES_ALL_PER_SECOND,
        )
    }

    /// Whether a peer new to this device that mDNS found at `from` is dialled now.
    pub(super) fn take_mdns_peer(&mut self, from: std::net::IpAddr, now: Instant) -> bool {
        if self.mdns_sources.len() >= MDNS_SOURCES_KEPT {
            self.mdns_sources.retain(|_, bucket| {
                bucket.refill(now, MDNS_PEER_BURST, MDNS_PEERS_PER_SECOND);
                bucket.tokens < MDNS_PEER_BURST
            });
        }
        let all = self
            .mdns_all
            .get_or_insert_with(|| Bucket::full(MDNS_ALL_BURST, now));
        all.refill(now, MDNS_ALL_BURST, MDNS_ALL_PER_SECOND);
        if all.tokens < 1.0 {
            return false;
        }
        let taken = self
            .mdns_sources
            .entry(from)
            .or_insert_with(|| Bucket::full(MDNS_PEER_BURST, now))
            .take(now, MDNS_PEER_BURST, MDNS_PEERS_PER_SECOND);
        if taken {
            all.tokens -= 1.0;
        }
        taken
    }

    /// Whether a request from a peer that is neither a member nor a friend here, coming
    /// from `from`, is answered now.
    pub(super) fn take_stranger_request(&mut self, from: Source, now: Instant) -> bool {
        let from = match from {
            Source::Address(std::net::IpAddr::V6(ip)) => Source::Address(
                std::net::Ipv6Addr::from(u128::from(ip) & !u128::from(u64::MAX)).into(),
            ),
            other => other,
        };
        if self.stranger_sources.len() >= STRANGER_SOURCES_KEPT
            && !self.stranger_sources.contains_key(&from)
        {
            self.stranger_sources.retain(|_, bucket| {
                bucket.refill(now, STRANGER_REQUEST_BURST, STRANGER_REQUESTS_PER_SECOND);
                bucket.tokens < STRANGER_REQUEST_BURST
            });
            if self.stranger_sources.len() >= STRANGER_SOURCES_KEPT {
                return false;
            }
        }
        self.stranger_sources
            .entry(from)
            .or_insert_with(|| Bucket::full(STRANGER_REQUEST_BURST, now))
            .take(now, STRANGER_REQUEST_BURST, STRANGER_REQUESTS_PER_SECOND)
    }

    /// A friend's update, if it may be taken now; else it waits.
    pub(super) fn friend_update(
        &mut self,
        update: FriendUpdate,
        now: Instant,
    ) -> Option<FriendUpdate> {
        let (key, version) = (update.key(), update.version());
        self.friend_updates.offer(key, version, update, now)
    }

    /// A member's profile, if it may be taken now; else it waits.
    pub(super) fn member_profile(&mut self, profile: Profile, now: Instant) -> Option<Profile> {
        let (device, version) = (profile.device, profile.version_ms);
        self.member_profiles.offer(device, version, profile, now)
    }

    /// A friend's or member's animation, if it may be taken now; else it waits. Each one
    /// taken makes the application rewrite the file of every friend's pictures.
    pub(super) fn animation(&mut self, animation: Animation, now: Instant) -> Option<Animation> {
        let key = (animation.device, animation.of);
        let version = animation.version_ms;
        self.animations.offer(key, version, animation, now)
    }

    /// The friends' updates, members' profiles and animations whose turn has come.
    pub(super) fn due_updates(
        &mut self,
        now: Instant,
    ) -> (Vec<FriendUpdate>, Vec<Profile>, Vec<Animation>) {
        (
            self.friend_updates.due(now),
            self.member_profiles.due(now),
            self.animations.due(now),
        )
    }

    /// `peer` has no connection left.
    pub(super) fn peer_gone(&mut self, peer: &PeerId) {
        self.signals.remove(peer);
        self.early_signals.remove(peer);
        self.deep_searches.remove(peer);
        self.peer_messages.remove(peer);
        self.entries.remove(&Sender::Peer(*peer));
        self.room_logs.remove(&Sender::Peer(*peer));
    }
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;

    /// A member handing over banners again and again has them looked at up to its budget
    /// in bytes, then a few each second; another member still has its own.
    #[test]
    fn a_member_s_banners_are_looked_at_up_to_a_budget_in_bytes() {
        let mut limits = Limits::default();
        let start = Instant::now();
        let bea = DeviceIdentity::generate().unwrap().public_key();
        let cai = DeviceIdentity::generate().unwrap().public_key();
        let banner = 512 * 1024;
        let taken = (0..1000)
            .filter(|_| limits.take_group_record(bea, banner, start))
            .count();
        assert_eq!(taken, GROUP_RECORD_BYTES_BURST as usize / banner);
        assert!(limits.take_group_record(cai, banner, start));
        assert!(!limits.take_group_record(bea, banner, start + Duration::from_secs(1)));
        assert!(limits.take_group_record(bea, banner, start + Duration::from_secs(8)));
        // Small ones count as at least a few KiB each.
        let taken = (0..100_000)
            .filter(|_| limits.take_group_record(cai, 10, start))
            .count();
        assert!(taken <= GROUP_RECORD_BYTES_BURST as usize / GROUP_RECORD_LEAST);
    }

    /// A friend saying hello again and again is answered a few times, then one every few
    /// seconds, and gets this device's profile once a minute; its offers are bounded too.
    #[test]
    fn a_friend_saying_hello_over_and_over_is_answered_a_few_times() {
        let mut limits = Limits::default();
        let start = Instant::now();
        let bea = DeviceIdentity::generate().unwrap().public_key();
        let answered = (0..1000)
            .filter(|_| limits.take_friend_hello(bea, start))
            .count();
        assert_eq!(answered, FRIEND_HELLO_BURST as usize);
        assert!(limits.take_friend_hello(bea, start + Duration::from_secs(10)));
        let profiles = (0..100)
            .filter(|i| limits.hello_profile(bea, start + Duration::from_secs(*i)))
            .count();
        assert_eq!(profiles, 2);
        let offers = (0..1000)
            .filter(|_| limits.take_friend_offer(bea, start))
            .count();
        assert_eq!(offers, FRIEND_OFFER_BURST as usize);
    }

    #[test]
    fn mdns_tells_of_a_few_new_peers_from_each_address_and_in_all() {
        let mut limits = Limits::default();
        let start = Instant::now();
        let neighbour: std::net::IpAddr = [192, 168, 1, 66].into();
        let taken = (0..1000)
            .filter(|_| limits.take_mdns_peer(neighbour, start))
            .count();
        assert_eq!(taken, MDNS_PEER_BURST as usize);
        // Another device there still gets its turn.
        assert!(limits.take_mdns_peer([192, 168, 1, 20].into(), start));
        assert!(limits.take_mdns_peer(neighbour, start + Duration::from_secs(8)));
        // Made-up addresses, as many as one likes: bounded in all.
        let mut limits = Limits::default();
        let taken = (0..1000u32)
            .filter(|i| {
                let from = std::net::Ipv4Addr::from(0x0a00_0000 + i);
                limits.take_mdns_peer(from.into(), start)
            })
            .count();
        assert_eq!(taken, MDNS_ALL_BURST as usize);
        assert!(limits.mdns_sources.len() <= MDNS_SOURCES_KEPT);
    }

    #[test]
    fn a_friend_rings_only_so_often() {
        let mut limits = Limits::default();
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let start = Instant::now();
        let taken = (0..10_000)
            .filter(|_| limits.take_call_step(friend, start))
            .count();
        assert_eq!(taken, CALL_STEP_BURST as usize);
        assert!(limits.take_call_step(friend, start + Duration::from_millis(100)));
    }

    #[test]
    fn animations_are_taken_a_couple_at_a_time_and_the_newest_waits() {
        let ana = DeviceIdentity::generate().unwrap();
        let mut limits = Limits::default();
        let start = Instant::now();
        let animation = |version_ms| Animation {
            device: ana.public_key(),
            of: fogata_friends::Animated::Photo,
            still: [0; 32],
            picture: Vec::new(),
            version_ms,
            signature: [0; fogata_crypto::SIGNATURE_LEN],
        };
        // In a loop with a growing version: each would rewrite every friend's pictures.
        let taken = (1..=500)
            .filter(|v| limits.animation(animation(*v), start).is_some())
            .count();
        assert_eq!(taken, UPDATES_AT_ONCE as usize);
        let (_, _, due) = limits.due_updates(start + UPDATE_EVERY);
        assert_eq!(
            due.iter().map(|a| a.version_ms).collect::<Vec<_>>(),
            vec![500]
        );
    }

    #[test]
    fn a_stranger_gets_a_few_searches_of_every_key_then_one_now_and_then() {
        let mut limits = Limits::default();
        let peer = PeerId::random();
        let start = Instant::now();
        let taken = (0..20)
            .filter(|_| limits.take_deep_search(peer, None, None, start))
            .count();
        assert_eq!(taken, DEEP_SEARCH_BURST as usize);
        assert!(!limits.take_deep_search(peer, None, None, start + Duration::from_secs(10)));
        assert!(limits.take_deep_search(peer, None, None, start + Duration::from_secs(16)));
    }

    #[test]
    fn strangers_with_a_new_peer_each_time_get_few_searches_of_every_key() {
        let mut limits = Limits::default();
        let start = Instant::now();
        let taken = (0..1000)
            .filter(|_| limits.take_deep_search(PeerId::random(), None, None, start))
            .count();
        assert_eq!(taken, DEEP_SEARCH_ALL_BURST as usize);
        assert!(limits.take_deep_search(
            PeerId::random(),
            None,
            None,
            start + Duration::from_secs(1)
        ));
    }

    /// Peers through one member's relay, and neighbours on the local network, each drain
    /// only their own share: a member coming back through that relay, or from another
    /// address on the local network, still gets its searches. A neighbour taking many
    /// addresses, or one coming through the relay as many peers, gets only so many shares.
    #[test]
    fn one_through_a_relay_or_on_the_local_network_drains_only_its_own_searches() {
        let mut limits = Limits::default();
        let start = Instant::now();
        let relay = PeerId::random();
        let stranger = PeerId::random();
        let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
        for _ in 0..1000 {
            limits.take_deep_search(stranger, None, Some(Vouch::Relay(relay, stranger)), start);
            limits.take_deep_search(PeerId::random(), None, Some(Vouch::Lan(neighbour)), start);
        }
        let member = PeerId::random();
        assert!(
            limits.take_deep_search(member, None, Some(Vouch::Relay(relay, member)), start),
            "a peer through the relay drained the searches of every peer through it"
        );
        let elsewhere = Vouch::Lan([192, 168, 1, 20].into());
        assert!(
            limits.take_deep_search(PeerId::random(), None, Some(elsewhere), start),
            "a neighbour drained the searches of the whole local network"
        );
        // Many peers through one relay, and many addresses on the local network: only so
        // many shares are kept of each.
        for i in 0..1000u32 {
            let peer = PeerId::random();
            limits.take_deep_search(peer, None, Some(Vouch::Relay(relay, peer)), start);
            let from = std::net::Ipv4Addr::from(0xc0a8_0000 + i).into();
            limits.take_deep_search(PeerId::random(), None, Some(Vouch::Lan(from)), start);
        }
        let kept = |kind: fn(&Vouch) -> bool| {
            limits
                .deep_searches_vouched
                .keys()
                .filter(|v| kind(v))
                .count()
        };
        assert_eq!(
            kept(|v| matches!(v, Vouch::Relay(..))),
            DEEP_SEARCH_PER_RELAY_KEPT
        );
        assert_eq!(kept(|v| matches!(v, Vouch::Lan(_))), DEEP_SEARCH_LAN_KEPT);
    }

    #[test]
    fn a_member_coming_back_gets_its_searches_when_strangers_drained_them() {
        let mut limits = Limits::default();
        let start = Instant::now();
        // Strangers at new peers, as many as they like, use up the strangers' bucket.
        for _ in 0..1000 {
            limits.take_deep_search(PeerId::random(), None, None, start);
        }
        assert!(!limits.take_deep_search(PeerId::random(), None, None, start));
        // One at an address a member was reached at, one from the local network and one
        // through a member's relay still get theirs, each from its voucher's share, which
        // strangers behind that voucher drain only for it.
        let home = Vouch::Address([203, 0, 113, 9].into());
        let relay = Vouch::Relay(PeerId::random(), PeerId::random());
        for vouch in [home, Vouch::Lan([192, 168, 1, 66].into()), relay] {
            let taken = (0..1000)
                .filter(|_| limits.take_deep_search(PeerId::random(), None, Some(vouch), start))
                .count();
            assert_eq!(taken, DEEP_SEARCH_VOUCHED_BURST as usize);
        }
        let later = start + Duration::from_secs(1);
        assert!(limits.take_deep_search(PeerId::random(), None, Some(home), later));
        assert!(!limits.take_deep_search(PeerId::random(), None, Some(home), later));
        // The addresses of one IPv6 /64 are one voucher.
        let six = |last: u16| {
            Vouch::Address(std::net::Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, last).into())
        };
        let taken = (0..1000)
            .filter(|i| limits.take_deep_search(PeerId::random(), None, Some(six(*i)), start))
            .count();
        assert_eq!(taken, DEEP_SEARCH_VOUCHED_BURST as usize);
        // More vouchers than are kept (there cannot be, see `Vouch`): what is kept stays
        // bounded, the rest take from the strangers' bucket, and a new one gets its own
        // once the others' are full again.
        let busy = later + Duration::from_secs(60);
        for i in 0..4 * DEEP_SEARCH_VOUCHERS_KEPT as u32 {
            let from = Vouch::Address(std::net::Ipv4Addr::from(0x0a00_0000 + i).into());
            limits.take_deep_search(PeerId::random(), None, Some(from), busy);
        }
        assert_eq!(
            limits.deep_searches_vouched.len(),
            DEEP_SEARCH_VOUCHERS_KEPT
        );
        let new = Vouch::Address([192, 0, 2, 1].into());
        assert!(!limits.take_deep_search(PeerId::random(), None, Some(new), busy));
        assert!(!limits.deep_searches_vouched.contains_key(&new));
        let quiet = busy + Duration::from_secs(60);
        assert!(limits.take_deep_search(PeerId::random(), None, Some(new), quiet));
        assert!(limits.deep_searches_vouched.contains_key(&new));
    }

    #[test]
    fn one_address_making_up_peers_cannot_drain_the_strangers_searches() {
        let mut limits = Limits::default();
        let start = Instant::now();
        // A machine at one address comes as a new peer each time, as fast as it likes:
        // it gets that address's share, not the strangers' whole bucket.
        let attacker: std::net::IpAddr = [198, 51, 100, 7].into();
        let mut taken = 0;
        for second in 0..60 {
            let now = start + Duration::from_secs(second);
            taken += (0..100)
                .filter(|_| limits.take_deep_search(PeerId::random(), Some(attacker), None, now))
                .count();
        }
        // The burst, then one every 15 seconds.
        let burst = DEEP_SEARCH_ADDRESS_BURST as usize;
        assert!((burst + 2..=burst + 4).contains(&taken), "{taken}");
        // An IPv6 /64 is one address.
        let six = |last: u16| -> std::net::IpAddr {
            std::net::Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, last).into()
        };
        let taken = (0..1000)
            .filter(|i| limits.take_deep_search(PeerId::random(), Some(six(*i)), None, start))
            .count();
        assert_eq!(taken, DEEP_SEARCH_ADDRESS_BURST as usize);
        // A member back from an address nobody knows, after several key changes, still
        // gets its search.
        let later = start + Duration::from_secs(60);
        assert!(limits.take_deep_search(
            PeerId::random(),
            Some([192, 0, 2, 44].into()),
            None,
            later
        ));
        // The addresses kept are bounded; past them, a new one waits until some are full
        // again.
        for i in 0..2 * DEEP_SEARCH_ADDRESSES_KEPT as u32 {
            let from = std::net::Ipv4Addr::from(0x0a00_0000 + i).into();
            limits.take_deep_search(PeerId::random(), Some(from), None, later);
        }
        assert!(limits.deep_search_addresses.len() <= DEEP_SEARCH_ADDRESSES_KEPT);
    }

    #[test]
    fn strangers_at_one_address_get_a_burst_then_a_few_answers_a_second() {
        let mut limits = Limits::default();
        let start = Instant::now();
        let home = Source::Address([203, 0, 113, 9].into());
        let taken = (0..1000)
            .filter(|_| limits.take_stranger_request(home, start))
            .count();
        assert_eq!(taken, STRANGER_REQUEST_BURST as usize);
        // Another address has its own.
        assert!(limits.take_stranger_request(Source::Address([198, 51, 100, 9].into()), start));
        let later = start + Duration::from_secs(1);
        let taken = (0..1000)
            .filter(|_| limits.take_stranger_request(home, later))
            .count();
        assert_eq!(taken, STRANGER_REQUESTS_PER_SECOND as usize);
        // The addresses of one IPv6 /64 are one.
        let six = |last: u16| {
            Source::Address(std::net::Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, last).into())
        };
        let taken = (0..1000)
            .filter(|i| limits.take_stranger_request(six(*i), start))
            .count();
        assert_eq!(taken, STRANGER_REQUEST_BURST as usize);
        // Many addresses at once: what is kept stays bounded, and a new one waits until
        // the others' budgets are full again.
        let busy = later + Duration::from_secs(60);
        for i in 0..4 * STRANGER_SOURCES_KEPT as u32 {
            let from = Source::Address(std::net::Ipv4Addr::from(0x0a00_0000 + i).into());
            limits.take_stranger_request(from, busy);
        }
        assert_eq!(limits.stranger_sources.len(), STRANGER_SOURCES_KEPT);
        let new = Source::Address([192, 0, 2, 1].into());
        assert!(!limits.take_stranger_request(new, busy));
        assert!(limits.take_stranger_request(new, busy + Duration::from_secs(10)));
    }

    #[test]
    fn updates_are_taken_twice_per_turn_and_the_newest_waits() {
        let mut held: Held<u8, &str> = Held::default();
        let start = Instant::now();
        assert_eq!(held.offer(1, 10, "a", start), Some("a"));
        // Others have turns of their own.
        assert_eq!(held.offer(2, 10, "x", start), Some("x"));
        let soon = start + Duration::from_secs(1);
        assert_eq!(held.offer(1, 11, "b", soon), Some("b"));
        assert_eq!(held.offer(1, 13, "d", soon), None);
        // An older one does not take the place of a newer one.
        assert_eq!(held.offer(1, 12, "c", soon), None);
        assert!(held.due(soon).is_empty());
        let turn = start + UPDATE_EVERY;
        assert_eq!(held.due(turn), ["d"]);
        // That one counts in the new turn: one more goes at once, the next waits.
        assert_eq!(held.offer(1, 14, "e", turn), Some("e"));
        assert_eq!(held.offer(1, 15, "f", turn), None);
        assert_eq!(held.due(turn + UPDATE_EVERY), ["f"]);
        // Nothing waits: forgotten, and the next is taken at once.
        let much_later = turn + 3 * UPDATE_EVERY;
        assert!(held.due(much_later).is_empty());
        assert!(held.slots.is_empty());
        assert_eq!(held.offer(1, 16, "g", much_later), Some("g"));
    }

    #[test]
    fn log_entries_are_checked_a_burst_at_once_then_a_few_a_second() {
        let mut limits = Limits::default();
        let bea = Sender::Device(DeviceIdentity::generate().unwrap().public_key());
        let start = Instant::now();
        let small = vec![vec![0u8; 200]; 10_000];
        // A message's worth: the first ones are checked, the rest wait for a later sync.
        assert_eq!(
            limits.take_entries(bea, &small, start),
            ENTRY_BURST as usize
        );
        assert_eq!(limits.take_entries(bea, &small, start), 0);
        let later = start + Duration::from_secs(10);
        assert_eq!(limits.take_entries(bea, &small, later), 10);
        // Another sender has its own; a device keeps its budget across connections.
        let leo = Sender::Peer(PeerId::random());
        assert_eq!(limits.take_entries(leo, &small[..5], later), 5);
        limits.peer_gone(&PeerId::random());
        assert_eq!(limits.take_entries(bea, &small, later), 0);
        // Big entries are counted by their bytes.
        let mut fresh = Limits::default();
        let big = vec![vec![0u8; 64 * 1024]; 100];
        assert_eq!(fresh.take_entries(bea, &big, start), 64);
        let soon = start + Duration::from_secs(16);
        assert_eq!(fresh.take_entries(bea, &big, soon), 1);
    }

    /// Entries judged and refused cost their sender far more than their place: a sender of
    /// nothing else gets one judged a minute once its burst is gone.
    #[test]
    fn refused_log_entries_cost_their_sender() {
        let mut limits = Limits::default();
        let bea = Sender::Device(DeviceIdentity::generate().unwrap().public_key());
        let leo = Sender::Device(DeviceIdentity::generate().unwrap().public_key());
        let start = Instant::now();
        let small = vec![vec![0u8; 200]; 100];
        assert_eq!(limits.take_entries(bea, &small[..9], start), 9);
        limits.refused_entries(bea, 8, start);
        // 9 taken and 8 refused cost 9 + 8 * 64 of 4096.
        assert_eq!(limits.take_entries(bea, &small, start), 100);
        let mut at = start;
        let mut judged = 0;
        while at < start + Duration::from_secs(600) {
            let taken = limits.take_entries(bea, &small[..9], at);
            limits.refused_entries(bea, taken, at);
            judged += taken;
            at += Duration::from_secs(1);
        }
        // About 3,400 tokens left: some 53 more at once, then about one a minute.
        assert!((50..80).contains(&judged), "{judged} judged in ten minutes");
        // Owed at most a burst: back to taking entries after a bit over an hour.
        limits.refused_entries(bea, 1000, at);
        let soon = at + Duration::from_secs(ENTRY_BURST as u64 - 60);
        assert_eq!(limits.take_entries(bea, &small[..1], soon), 0);
        let back = at + Duration::from_secs(ENTRY_BURST as u64 + 1);
        assert_eq!(limits.take_entries(bea, &small[..1], back), 1);
        // Others keep their own.
        assert_eq!(limits.take_entries(leo, &small, at), 100);
    }

    #[test]
    fn group_messages_are_limited_per_connection_and_per_member() {
        let mut limits = Limits::default();
        let peer = PeerId::random();
        let start = Instant::now();
        // A connection hands over others' messages too: plenty at once, then by bytes.
        let small = (0..2000)
            .filter(|_| limits.take_peer_message(peer, 1000, start))
            .count();
        assert_eq!(small, PEER_MESSAGE_BURST as usize);
        let other = PeerId::random();
        let big = (0..200)
            .filter(|_| limits.take_peer_message(other, 150 * 1024, start))
            .count();
        assert_eq!(big, (PEER_MESSAGE_BYTES_BURST / (150.0 * 1024.0)) as usize);
        let second = start + Duration::from_secs(1);
        let big = (0..200)
            .filter(|_| limits.take_peer_message(other, 150 * 1024, second))
            .count();
        assert_eq!(
            big,
            (PEER_MESSAGE_BYTES_PER_SECOND / (150.0 * 1024.0)) as usize
        );
        // One member's own messages: a burst (a room joined with dozens of files shared),
        // then a steady pace.
        let ana = DeviceIdentity::generate().unwrap().public_key();
        let chat = (0..1000)
            .filter(|_| limits.take_author_message(ana, 500, start))
            .count();
        assert_eq!(chat, AUTHOR_MESSAGE_BURST as usize);
        let chat = (0..1000)
            .filter(|_| limits.take_author_message(ana, 500, second))
            .count();
        assert_eq!(chat, AUTHOR_MESSAGES_PER_SECOND as usize);
        let bea = DeviceIdentity::generate().unwrap().public_key();
        let files = (0..100)
            .filter(|_| limits.take_author_message(bea, 90 * 1024, start))
            .count();
        assert_eq!(files, 91);
    }

    #[test]
    fn a_member_outside_the_room_is_heard_only_a_few_times() {
        let mut limits = Limits::default();
        let peer = PeerId::random();
        let start = Instant::now();
        let taken = (0..20)
            .filter(|_| limits.take_early_signal(peer, start))
            .count();
        assert_eq!(taken, EARLY_SIGNAL_BURST as usize);
        assert!(!limits.take_early_signal(peer, start + Duration::from_secs(2)));
        assert!(limits.take_early_signal(peer, start + Duration::from_secs(6)));
    }

    #[test]
    fn a_member_gets_a_burst_of_signals_then_eight_a_second() {
        let mut limits = Limits::default();
        let (peer, other) = (PeerId::random(), PeerId::random());
        let start = Instant::now();
        let taken = (0..100).filter(|_| limits.take_signal(peer, start)).count();
        assert_eq!(taken, SIGNAL_BURST as usize);
        // Another member has its own.
        assert!(limits.take_signal(other, start));
        let later = start + Duration::from_secs(1);
        let taken = (0..100).filter(|_| limits.take_signal(peer, later)).count();
        assert_eq!(taken, SIGNALS_PER_SECOND as usize);
        // Gone and back, it starts afresh.
        limits.peer_gone(&peer);
        let taken = (0..100).filter(|_| limits.take_signal(peer, later)).count();
        assert_eq!(taken, SIGNAL_BURST as usize);
    }
}
