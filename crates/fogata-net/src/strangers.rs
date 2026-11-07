//! Connections to strangers: the DHT nodes that lookups and announcements pass through.
//!
//! One DHT lookup talks to a hundred nodes or more, and a device looks up its groups and
//! friends every half minute while it waits for them. Those connections are only worth
//! keeping while a request runs on them, and they should not each take a socket: every TCP
//! connection is an open file of its own (macOS apps may open 256 by default), while QUIC
//! connections share the node's UDP socket.
//!
//! - Peers this device is trying to reach, and those that proved something (members, friends
//!   online), are *wanted*: their connections stay open for [`KEEP`] after that, even with
//!   nothing to send, and are never closed to make room for others. Peers that only spoke a
//!   Fogata protocol to it are *heard*: kept open as long, but closed first when incoming
//!   connections run out, since anyone can send a request. Every other connection closes
//!   [`IDLE`] after its last request.
//! - Once QUIC works on this network, strangers with QUIC addresses are dialled there only.
//!   Those known only by TCP addresses, and those whose QUIC dial just failed (some DHT
//!   nodes answer only over TCP), are dialled over TCP while fewer than [`TCP_STRANGERS`]
//!   TCP connections to strangers are open; past that they are skipped, and the lookup asks
//!   another node. A stranger known only by TCP is also tried over QUIC on the same port at
//!   the same time, as many DHT nodes listen on both. Local network addresses of strangers
//!   are skipped when they have public ones. Networks that block UDP keep using TCP.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use libp2p::{
    Multiaddr, PeerId,
    core::{Endpoint, transport::PortUse, upgrade::DeniedUpgrade},
    multiaddr::Protocol,
    swarm::{
        CloseConnection, ConnectionDenied, ConnectionHandler, ConnectionHandlerEvent, ConnectionId,
        DialError, FromSwarm, NetworkBehaviour, SubstreamProtocol, THandler, THandlerInEvent,
        THandlerOutEvent, ToSwarm, handler::ConnectionEvent,
    },
};

use crate::{
    addr,
    lan::{self, LanKeys},
};

/// Incoming connections open at once, in all and from one public address (an IPv6 /64, which
/// one machine often has to itself). Past the total, the oldest from a peer this device does
/// not want is closed for the new one, so a flood cannot keep friends and members out.
const MAX_INBOUND: usize = 512;
const MAX_INBOUND_PER_ADDRESS: usize = 32;
/// The same from one address on the local network (this computer itself not counted), where
/// several devices may share a computer: past it, the oldest from there of a peer this device
/// does not want is closed for the new one.
const MAX_INBOUND_PER_LAN_ADDRESS: usize = 64;
/// Incoming connections from one peer: QUIC, TCP and a punch's second one are a handful. A
/// peer this device dials (one a DHT node named, say) is kept, so it could otherwise come in
/// from many addresses with all of them.
const MAX_INBOUND_PER_PEER: usize = 8;
/// The same from one IPv6 /48, as much as one site (a home, an office) is commonly given: a
/// machine there may use thousands of /64s.
const MAX_INBOUND_PER_SITE: usize = 64;
/// Incoming connections still being set up at once from one public address, from one IPv6
/// /48 and from one block of addresses (an IPv4 /16, an IPv6 /32: what one provider is
/// given, from which one machine may be lent a few addresses). A handshake may take up to
/// [`crate::node::CONNECTION_TIMEOUT`] (a TCP one from a stranger [`STRANGER_SETUP`]), and a
/// TCP one holds a socket all that time, before the limits above apply.
const MAX_PENDING_PER_ADDRESS: usize = 8;
const MAX_PENDING_PER_SITE: usize = 16;
const MAX_PENDING_PER_BLOCK: usize = 32;
/// The same from one address on the local network, where several devices may share a
/// computer (or be one, behind a virtual machine's NAT); this computer itself is not
/// counted.
const MAX_PENDING_PER_LAN_ADDRESS: usize = 32;
/// Connections being set up at once over each transport, in all. TCP ones each hold a
/// socket, and a neighbour can use any number of addresses, the internet many; QUIC ones
/// share the node's one socket, but each holds a handshake's state, and one whose address
/// is not proven to be the sender's is answered with a retry first (see
/// vendor/libp2p-quic) so that the state is only ever for a real address. Of each share,
/// only so much goes to peers from public addresses this device knows nothing of, and to
/// the local network: the rest is kept for addresses where a member or friend was reached
/// (see [`Wanted::set_known`]) and for this computer itself, so that machines opening
/// silent connections from a few addresses, as fast as they are closed, keep no member or
/// friend out on a network that blocks UDP. One behind the same address as a member gets
/// only that address's few places. The strangers' TCP share is what all TCP ones together
/// had before places were kept: it still takes 16 addresses, in 4 blocks, to fill, so a
/// member coming from an address this device never reached it at (a new one from its
/// provider, a first visit) is kept out no more easily than before.
const PENDING_TCP: Share = Share {
    all: 192,
    strangers: 128,
    lan: 48,
};
const PENDING_QUIC: Share = Share {
    all: 1024,
    strangers: 768,
    lan: 128,
};
/// How long an incoming TCP connection from an address where no member or friend was reached
/// (see [`Wanted::set_known`]) may take to be set up, rather than
/// [`crate::node::CONNECTION_TIMEOUT`], once the places of its kind among [`PENDING_TCP`]
/// are half taken: a machine opening connections and leaving them silent holds a place and
/// a socket only this long. While they are not, it gets the longer time, as dials, QUIC,
/// relayed connections and punches always do: a member over a slow and lossy link (a
/// satellite, a crowded mobile network) that loses a packet of its handshake needs more
/// than this, though its own address is new here, or on the local network.
pub(crate) const STRANGER_SETUP: Duration = Duration::from_secs(5);

/// How long connections to a peer stay open, idle, after it was last wanted.
const KEEP: Duration = Duration::from_secs(60);
/// How long any other connection stays open once no request runs on it.
pub(crate) const IDLE: Duration = Duration::from_secs(10);
/// How long an established QUIC connection counts as proof that UDP gets through here.
const QUIC_PROOF: Duration = Duration::from_secs(2 * 60);
/// Expired entries are dropped once this many peers have been wanted.
const PRUNE_AT: usize = 1024;
/// ... at most this often for the peers wanted or heard, which every request and every peer
/// found adds to: going through all of them at each one, under the lock every connection
/// takes, would cost more the more there are.
const PRUNE_EVERY: Duration = Duration::from_secs(10);
/// Most addresses dialled for a peer known only from a DHT announcement, which a DHT node
/// may have made up.
const MAX_DHT_ADDRESSES: usize = 8;
/// TCP connections to strangers allowed at once while QUIC works; past it, strangers that
/// need TCP are not dialled.
const TCP_STRANGERS: usize = 32;
/// How long a stranger whose QUIC dial failed is dialled over TCP instead. Lookups of the
/// same key reach the same nodes every half minute.
const QUIC_FAILED_FOR: Duration = Duration::from_secs(30 * 60);

/// The peers this device wants connections to, shared with every connection.
#[derive(Clone, Debug, Default)]
pub(crate) struct Wanted(Arc<Mutex<State>>);

#[derive(Debug, Default)]
struct State {
    until: HashMap<PeerId, Instant>,
    /// Peers that only spoke to this device, and until when they are kept.
    heard: HashMap<PeerId, Instant>,
    /// When expired peers were last dropped from `until` and `heard`.
    pruned: Option<Instant>,
    /// Peers known only from DHT announcements, since when.
    from_dht: HashMap<PeerId, Instant>,
    quic_at: Option<Instant>,
    /// Addresses (as [`network_of`] counts them) where a member or friend was reached, or is
    /// connected from now.
    known: HashSet<std::net::IpAddr>,
    /// Incoming TCP connections being set up from strangers on the internet, and from the
    /// local network (see [`Pending::stranger`] and [`Pending::lan`]).
    tcp_setting_up: (usize, usize),
}

/// Connections being set up at once over one transport: in all, from public addresses
/// this device knows nothing of, and from the local network.
struct Share {
    all: usize,
    strangers: usize,
    lan: usize,
}

impl Wanted {
    /// Keeps connections to `peer` open for [`KEEP`] from now, and never closes them to make
    /// room: for peers this device chose to reach, or that proved something.
    pub(crate) fn want(&self, peer: PeerId) {
        let mut state = crate::locked(&self.0);
        let now = Instant::now();
        state.prune(now);
        state.until.insert(peer, now + KEEP);
    }

    /// Keeps connections to `peer`, which spoke a Fogata protocol to this device, open for
    /// [`KEEP`] from now; they are still the first closed when incoming connections run out.
    pub(crate) fn hear(&self, peer: PeerId) {
        let mut state = crate::locked(&self.0);
        let now = Instant::now();
        state.prune(now);
        state.heard.insert(peer, now + KEEP);
    }

    /// `peer` was found in a DHT announcement and nowhere else: its local network addresses
    /// are not dialled, since any DHT node can announce anything (a member's own come
    /// sealed), and only a few of its others are.
    pub(crate) fn found_on_dht(&self, peer: PeerId) {
        let mut state = crate::locked(&self.0);
        if state.from_dht.len() >= PRUNE_AT {
            state
                .from_dht
                .retain(|_, at| at.elapsed() < QUIC_FAILED_FOR);
        }
        state.from_dht.entry(peer).or_insert_with(Instant::now);
    }

    /// `peer` was found on the local network: its local addresses are dialled.
    pub(crate) fn found_nearby(&self, peer: &PeerId) {
        crate::locked(&self.0).from_dht.remove(peer);
    }

    pub(crate) fn only_from_dht(&self, peer: &PeerId) -> bool {
        crate::locked(&self.0).from_dht.contains_key(peer)
    }

    pub(crate) fn is_wanted(&self, peer: &PeerId) -> bool {
        crate::locked(&self.0)
            .until
            .get(peer)
            .is_some_and(|until| *until > Instant::now())
    }

    /// Whether connections to `peer` stay open while idle: it is wanted or was heard.
    fn keeps(&self, peer: &PeerId) -> bool {
        let state = crate::locked(&self.0);
        let now = Instant::now();
        [&state.until, &state.heard]
            .iter()
            .any(|map| map.get(peer).is_some_and(|until| *until > now))
    }

    /// Records that a QUIC connection was just established, so UDP gets through.
    pub(crate) fn quic_works(&self) {
        crate::locked(&self.0).quic_at = Some(Instant::now());
    }

    fn quic_proven(&self) -> bool {
        crate::locked(&self.0)
            .quic_at
            .is_some_and(|at| at.elapsed() < QUIC_PROOF)
    }

    /// The addresses members and friends were reached at, or are connected from: incoming
    /// connections from them being set up get the places kept for them (see [`PENDING_TCP`]).
    pub(crate) fn set_known(&self, addresses: impl IntoIterator<Item = std::net::IpAddr>) {
        let known = addresses.into_iter().map(network_of).collect();
        crate::locked(&self.0).known = known;
    }

    pub(crate) fn is_known(&self, ip: std::net::IpAddr) -> bool {
        crate::locked(&self.0).known.contains(&network_of(ip))
    }

    /// Whether half the places for incoming TCP connections being set up from the local
    /// network (`lan`), or from strangers on the internet, are taken (see
    /// [`STRANGER_SETUP`]).
    fn tcp_crowded(&self, lan: bool) -> bool {
        let (strangers, local) = crate::locked(&self.0).tcp_setting_up;
        if lan {
            2 * local >= PENDING_TCP.lan
        } else {
            2 * strangers >= PENDING_TCP.strangers
        }
    }
}

impl State {
    /// Drops expired peers once there are many, at most every [`PRUNE_EVERY`]: between two
    /// prunes, the maps hold only the peers wanted or heard since the last one.
    fn prune(&mut self, now: Instant) {
        if self.until.len() + self.heard.len() < PRUNE_AT
            || self
                .pruned
                .is_some_and(|at| now.saturating_duration_since(at) < PRUNE_EVERY)
        {
            return;
        }
        self.pruned = Some(now);
        self.until.retain(|_, until| *until > now);
        self.heard.retain(|_, until| *until > now);
    }
}

/// What decides how a stranger is dialled.
#[derive(Clone, Copy, Debug)]
struct Situation {
    /// A QUIC connection to a public address succeeded a moment ago: UDP gets through.
    quic_works: bool,
    /// A QUIC dial to this stranger failed a moment ago.
    quic_failed: bool,
    /// Fewer than [`TCP_STRANGERS`] TCP connections to strangers are open.
    tcp_room: bool,
}

/// Of the addresses known for a stranger, those worth dialling. Only its public ones if it
/// has any (its local network addresses mean nothing from here). Once QUIC works here,
/// only its QUIC ones; if it has none, or its QUIC just failed, its TCP ones while there is
/// room for another TCP connection, preceded by their QUIC twins unless its QUIC failed.
fn for_stranger(addresses: Vec<Multiaddr>, now: Situation) -> Vec<Multiaddr> {
    let (public, local): (Vec<_>, Vec<_>) = addresses.into_iter().partition(|a| !addr::is_local(a));
    let addresses = if public.is_empty() { local } else { public };
    if !now.quic_works {
        return addresses;
    }
    let (quic, other): (Vec<_>, Vec<_>) = addresses.into_iter().partition(is_plain_quic);
    if !quic.is_empty() && !now.quic_failed {
        return quic;
    }
    if !now.tcp_room {
        // A lookup does better asking another node at once than waiting for a dial that
        // is likely to fail. Names (such as the bootstrap peers' /dnsaddr) are kept.
        return other
            .into_iter()
            .filter(|a| addr::ip(a).is_none())
            .collect();
    }
    let mut out: Vec<Multiaddr> = Vec::new();
    if !now.quic_failed {
        for twin in other.iter().filter_map(quic_twin) {
            if !out.contains(&twin) {
                out.push(twin);
            }
        }
    }
    out.extend(other);
    out
}

/// Of the addresses DHT records, identify and the other behaviours give for a peer, those
/// worth a dial that may need DNS: none this node's transports cannot dial (DHT nodes often
/// list WebSocket addresses under names of their own), no `/dns`, `/dns4` or `/dns6` name
/// (whoever announces it picks what this device asks its DNS server, and the server, often
/// the internet provider's, sees which peers it reaches), and a `/dnsaddr` (the public
/// bootstrap peers') only while the peer has no plain IP address.
fn without_lookups(addresses: Vec<Multiaddr>) -> Vec<Multiaddr> {
    let addresses: Vec<Multiaddr> = addresses
        .into_iter()
        .filter(|a| {
            addr::is_dialable(a)
                && !a
                    .iter()
                    .any(|p| matches!(p, Protocol::Dns(_) | Protocol::Dns4(_) | Protocol::Dns6(_)))
        })
        .collect();
    if addresses.iter().any(|a| !addr::is_named(a)) {
        addresses
            .into_iter()
            .filter(|a| !addr::is_named(a))
            .collect()
    } else {
        addresses
    }
}

/// Of the addresses found for a peer known only from a DHT announcement, those dialled: no
/// local network address, which would have this device probe its own network for whoever
/// made up the announcement, and only a few others.
fn for_dht_peer(found: Vec<Multiaddr>) -> Vec<Multiaddr> {
    found
        .into_iter()
        .filter(|a| !addr::is_local(a))
        .take(MAX_DHT_ADDRESSES)
        .collect()
}

/// The QUIC address on the same IP and port as a plain TCP address.
fn quic_twin(address: &Multiaddr) -> Option<Multiaddr> {
    let mut parts = address.iter();
    let ip = parts
        .next()
        .filter(|p| matches!(p, Protocol::Ip4(_) | Protocol::Ip6(_)))?;
    let Some(Protocol::Tcp(port)) = parts.next() else {
        return None;
    };
    let mut twin = Multiaddr::empty()
        .with(ip)
        .with(Protocol::Udp(port))
        .with(Protocol::QuicV1);
    match parts.next() {
        None => {}
        Some(peer @ Protocol::P2p(_)) if parts.next().is_none() => twin.push(peer),
        // WebSocket and the like on top of TCP: a different listener.
        Some(_) => return None,
    }
    Some(twin)
}

/// QUIC with nothing on top (not WebTransport), which is what this node can dial.
fn is_plain_quic(address: &Multiaddr) -> bool {
    address
        .iter()
        .filter(|p| !matches!(p, Protocol::P2p(_)))
        .last()
        .is_some_and(|p| matches!(p, Protocol::QuicV1))
}

/// An incoming connection being set up.
#[derive(Clone, Copy, Debug)]
struct Pending {
    /// Its address, an IPv6 one cut to its /64.
    from: std::net::IpAddr,
    public: bool,
    /// From this computer itself.
    loopback: bool,
    /// From an address where a member or friend was reached (see [`Wanted::set_known`]).
    known: bool,
    tcp: bool,
}

impl Pending {
    /// Whether it takes one of the places for strangers from public addresses, or one of
    /// those for the local network (see [`PENDING_TCP`]); a known address, and this
    /// computer, take neither.
    fn stranger(&self) -> bool {
        self.public && !self.known
    }

    fn lan(&self) -> bool {
        !self.public && !self.loopback && !self.known
    }
}

/// What is being set up now, as the limits count it, gathered in one pass.
#[derive(Default)]
struct PendingCounts {
    all: usize,
    strangers: usize,
    lan: usize,
    /// From the same address, the same IPv6 /48 and the same block as the new one.
    same: usize,
    same_site: usize,
    same_block: usize,
}

impl PendingCounts {
    fn of<'a>(pending: impl Iterator<Item = &'a Pending>, new: &Pending) -> Self {
        let (site, block) = (site_of(new.from), block_of(new.from));
        let mut counts = Self::default();
        for p in pending {
            if p.tcp == new.tcp {
                counts.all += 1;
                counts.strangers += usize::from(p.stranger());
                counts.lan += usize::from(p.lan());
            }
            counts.same += usize::from(p.from == new.from);
            if p.public && new.public {
                counts.same_site += usize::from(site_of(p.from) == site);
                counts.same_block += usize::from(block_of(p.from) == block);
            }
        }
        counts
    }
}

/// Wraps the node's behaviours and chooses the addresses they supply for dialling
/// strangers. Addresses the node passes to a dial itself are never changed.
pub(crate) struct StrangerDials<B> {
    inner: B,
    wanted: Wanted,
    /// Open TCP connections to peers that were strangers when they were established.
    tcp_strangers: HashSet<ConnectionId>,
    /// Strangers whose last dial (over QUIC only) failed, and when.
    quic_failed: HashMap<PeerId, Instant>,
    /// Keys that open the sealed local addresses of friends and members.
    lan: LanKeys,
    /// Incoming connections open now: the peer, its public address (an IPv6 one cut to its
    /// /64) and when it came.
    inbound: HashMap<ConnectionId, (PeerId, Option<std::net::IpAddr>, Instant)>,
    /// Those of them from a local network address other than this computer's, and it.
    inbound_lan: HashMap<ConnectionId, std::net::IpAddr>,
    /// Incoming connections being set up, other than through a member's relay (see
    /// [`MAX_PENDING_PER_ADDRESS`] and [`MAX_PENDING`]).
    pending_inbound: HashMap<ConnectionId, Pending>,
    /// Connections to close to make room (see [`MAX_INBOUND`]).
    to_close: VecDeque<(PeerId, ConnectionId)>,
    /// Local addresses opened for each peer, so that a later dial (a scheduled punch, or a
    /// lookup whose first answer came from a DHT node that had not heard the name yet)
    /// still tries them.
    lan_opened: HashMap<PeerId, Vec<Multiaddr>>,
}

impl<B> StrangerDials<B> {
    pub(crate) fn new(inner: B, wanted: Wanted, lan: LanKeys) -> Self {
        Self {
            inner,
            wanted,
            tcp_strangers: HashSet::new(),
            quic_failed: HashMap::new(),
            lan,
            inbound: HashMap::new(),
            inbound_lan: HashMap::new(),
            pending_inbound: HashMap::new(),
            to_close: VecDeque::new(),
            lan_opened: HashMap::new(),
        }
    }
}

impl<B> StrangerDials<B> {
    /// Tells the transport how many incoming TCP connections are being set up from
    /// strangers and from the local network (see [`STRANGER_SETUP`]).
    fn publish_setting_up(&self) {
        let tcp = self.pending_inbound.values().filter(|p| p.tcp);
        let counts = tcp.fold((0, 0), |(strangers, lan), p| {
            (
                strangers + usize::from(p.stranger()),
                lan + usize::from(p.lan()),
            )
        });
        crate::locked(&self.wanted.0).tcp_setting_up = counts;
    }

    /// Closes the oldest incoming connection, among those `among` takes, of a peer this
    /// device does not want, to make room for a new one; refuses the new one if there is
    /// none.
    fn close_oldest_stranger(
        &mut self,
        among: impl Fn(&ConnectionId) -> bool,
    ) -> Result<(), ConnectionDenied> {
        let closing: HashSet<ConnectionId> = self.to_close.iter().map(|(_, c)| *c).collect();
        let oldest = self
            .inbound
            .iter()
            .filter(|(id, (peer, _, _))| {
                among(id) && !closing.contains(id) && !self.wanted.is_wanted(peer)
            })
            .min_by_key(|(_, (_, _, since))| *since)
            .map(|(id, (peer, _, _))| (*peer, *id));
        match oldest {
            Some(oldest) => {
                self.to_close.push_back(oldest);
                Ok(())
            }
            None => Err(ConnectionDenied::new("too many incoming connections")),
        }
    }
}

impl<B> Deref for StrangerDials<B> {
    type Target = B;

    fn deref(&self) -> &B {
        &self.inner
    }
}

impl<B> DerefMut for StrangerDials<B> {
    fn deref_mut(&mut self) -> &mut B {
        &mut self.inner
    }
}

/// A TCP transport whose incoming connections from strangers are set up within
/// [`STRANGER_SETUP`] or closed; the swarm bounds all the others.
pub(crate) struct StrangerSetup<T> {
    inner: T,
    wanted: Wanted,
}

impl<T> StrangerSetup<T> {
    pub(crate) fn new(inner: T, wanted: Wanted) -> Self {
        Self { inner, wanted }
    }

    /// How long a connection coming from `from` may take to be set up, if less than the
    /// swarm's bound when its kind's places are crowded then, and whether it comes from the
    /// local network.
    fn limit(&self, from: &Multiaddr) -> Option<(Duration, bool)> {
        match addr::ip(from) {
            Some(ip) if ip.is_loopback() || self.wanted.is_known(ip) => None,
            _ => Some((STRANGER_SETUP, addr::is_local(from))),
        }
    }
}

type SetupError<E> = libp2p::core::transport::timeout::TransportTimeoutError<E>;

impl<T> libp2p::core::Transport for StrangerSetup<T>
where
    T: libp2p::core::Transport + Unpin,
    T::Output: Send + 'static,
    T::Error: Send + 'static,
    T::ListenerUpgrade: Send + 'static,
{
    type Output = T::Output;
    type Error = SetupError<T::Error>;
    type ListenerUpgrade = futures::future::BoxFuture<'static, Result<T::Output, Self::Error>>;
    type Dial = futures::future::MapErr<T::Dial, fn(T::Error) -> Self::Error>;

    fn listen_on(
        &mut self,
        id: libp2p::core::transport::ListenerId,
        addr: Multiaddr,
    ) -> Result<(), libp2p::core::transport::TransportError<Self::Error>> {
        self.inner
            .listen_on(id, addr)
            .map_err(|e| e.map(SetupError::Other))
    }

    fn remove_listener(&mut self, id: libp2p::core::transport::ListenerId) -> bool {
        self.inner.remove_listener(id)
    }

    fn dial(
        &mut self,
        addr: Multiaddr,
        opts: libp2p::core::transport::DialOpts,
    ) -> Result<Self::Dial, libp2p::core::transport::TransportError<Self::Error>> {
        use futures::TryFutureExt as _;
        let dial = self
            .inner
            .dial(addr, opts)
            .map_err(|e| e.map(SetupError::Other))?;
        Ok(dial.map_err(SetupError::Other as fn(T::Error) -> Self::Error))
    }

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<libp2p::core::transport::TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        use futures::FutureExt as _;
        let this = self.get_mut();
        let event = std::task::ready!(std::pin::Pin::new(&mut this.inner).poll(cx));
        let limit = match &event {
            libp2p::core::transport::TransportEvent::Incoming { send_back_addr, .. } => {
                this.limit(send_back_addr)
            }
            _ => None,
        };
        let wanted = this.wanted.clone();
        Poll::Ready(
            event
                .map_upgrade(move |upgrade| {
                    async move {
                        let Some((limit, lan)) = limit else {
                            return upgrade.await.map_err(SetupError::Other);
                        };
                        let mut upgrade = std::pin::pin!(upgrade);
                        if let Ok(done) = tokio::time::timeout(limit, &mut upgrade).await {
                            return done.map_err(SetupError::Other);
                        }
                        // Past it, only while others wait for places; else the swarm's
                        // bound applies.
                        if wanted.tcp_crowded(lan) {
                            return Err(SetupError::Timeout);
                        }
                        upgrade.await.map_err(SetupError::Other)
                    }
                    .boxed()
                })
                .map_err(SetupError::Other),
        )
    }
}

impl<B: NetworkBehaviour> NetworkBehaviour for StrangerDials<B> {
    type ConnectionHandler = B::ConnectionHandler;
    type ToSwarm = B::ToSwarm;

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        self.inner
            .handle_pending_inbound_connection(connection_id, local_addr, remote_addr)?;
        let Some(ip) = addr::ip(remote_addr).filter(|_| !addr::is_relayed(remote_addr)) else {
            return Ok(());
        };
        let pending = Pending {
            from: network_of(ip),
            public: !addr::is_local(remote_addr),
            loopback: ip.is_loopback(),
            known: self.wanted.is_known(ip),
            tcp: !addr::is_quic(remote_addr),
        };
        let denied = |why| Err(ConnectionDenied::new(why));
        let counts = PendingCounts::of(self.pending_inbound.values(), &pending);
        let share = if pending.tcp {
            &PENDING_TCP
        } else {
            &PENDING_QUIC
        };
        // A known address is held only to its own few places: strangers filling its /48
        // or its block do not keep the member or friend there out.
        let too_many_from_one = if pending.public {
            counts.same >= MAX_PENDING_PER_ADDRESS
                || (!pending.known
                    && (counts.same_site >= MAX_PENDING_PER_SITE
                        || counts.same_block >= MAX_PENDING_PER_BLOCK))
        } else {
            counts.same >= MAX_PENDING_PER_LAN_ADDRESS && !pending.loopback
        };
        if too_many_from_one {
            return denied("too many connections being set up from one address");
        }
        if counts.all >= share.all
            || (pending.stranger() && counts.strangers >= share.strangers)
            || (pending.lan() && counts.lan >= share.lan)
        {
            return denied("too many connections being set up");
        }
        self.pending_inbound.insert(connection_id, pending);
        self.publish_setting_up();
        Ok(())
    }

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // One machine opening connection after connection would use up this device's
        // sockets and memory: a bounded number in all, and from any one public address.
        let ip = public_network(remote_addr);
        if let Some(ip) = ip {
            let site = site_of(ip);
            let (mut same, mut same_site) = (0, 0);
            for (_, from, _) in self.inbound.values() {
                same += usize::from(*from == Some(ip));
                same_site += usize::from(from.is_some_and(|f| site_of(f) == site));
            }
            if same >= MAX_INBOUND_PER_ADDRESS || same_site >= MAX_INBOUND_PER_SITE {
                return Err(ConnectionDenied::new(
                    "too many connections from one address",
                ));
            }
        }
        let from_peer = self.inbound.values().filter(|(p, ..)| *p == peer).count();
        if from_peer >= MAX_INBOUND_PER_PEER {
            return Err(ConnectionDenied::new("too many connections from one peer"));
        }
        // A local network address as the limits count it: an IPv6 /64, which one neighbour
        // has whole (link-local addresses, all in one, are rare here: the local addresses
        // announced and found over mDNS are IPv4).
        let lan = addr::ip(remote_addr)
            .filter(|ip| {
                !ip.is_loopback() && !addr::is_relayed(remote_addr) && addr::is_local(remote_addr)
            })
            .map(network_of);
        if let Some(lan) = lan {
            let same: HashSet<ConnectionId> = self
                .inbound_lan
                .iter()
                .filter(|(_, from)| **from == lan)
                .map(|(id, _)| *id)
                .collect();
            if same.len() >= MAX_INBOUND_PER_LAN_ADDRESS {
                self.close_oldest_stranger(|id| same.contains(id))?;
            }
        }
        let open = self.inbound.len().saturating_sub(self.to_close.len());
        if open >= MAX_INBOUND {
            self.close_oldest_stranger(|_| true)?;
        }
        let handler = self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )?;
        self.inbound
            .insert(connection_id, (peer, ip, Instant::now()));
        if let Some(lan) = lan {
            self.inbound_lan.insert(connection_id, lan);
        }
        Ok(handler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        let found = self.inner.handle_pending_outbound_connection(
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )?;
        // Sealed local addresses are opened for peers this device wants to reach, first,
        // since they are the ones that work when both are at the same place. A name is never
        // left for DNS to look up.
        let (names, found): (Vec<_>, Vec<_>) = found.into_iter().partition(lan::is_name);
        let found = without_lookups(found);
        Ok(match maybe_peer {
            Some(peer) if !self.wanted.is_wanted(&peer) => {
                let now = Situation {
                    quic_works: self.wanted.quic_proven(),
                    quic_failed: self
                        .quic_failed
                        .get(&peer)
                        .is_some_and(|at| at.elapsed() < QUIC_FAILED_FOR),
                    tcp_room: self.tcp_strangers.len() < TCP_STRANGERS,
                };
                for_stranger(found, now)
            }
            _ => {
                let opened = self.lan.open_all(&names);
                if !names.is_empty() {
                    tracing::debug!(
                        peer = ?maybe_peer,
                        names = names.len(),
                        opened = opened.len(),
                        "sealed local addresses in a dial"
                    );
                }
                let found = match maybe_peer {
                    Some(peer) if self.wanted.only_from_dht(&peer) => for_dht_peer(found),
                    _ => found,
                };
                let mut out = match maybe_peer {
                    Some(peer) if !opened.is_empty() => {
                        if self.lan_opened.len() >= PRUNE_AT {
                            self.lan_opened.clear();
                        }
                        self.lan_opened.insert(peer, opened.clone());
                        opened
                    }
                    Some(peer) => self.lan_opened.get(&peer).cloned().unwrap_or_default(),
                    None => opened,
                };
                out.extend(found);
                out
            }
        })
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        if let FromSwarm::ConnectionEstablished(e) = &event
            && self.pending_inbound.remove(&e.connection_id).is_some()
        {
            self.publish_setting_up();
        }
        match &event {
            FromSwarm::ConnectionEstablished(e)
                if !addr::is_quic(e.endpoint.get_remote_address())
                    && !addr::is_relayed(e.endpoint.get_remote_address())
                    && !self.wanted.is_wanted(&e.peer_id) =>
            {
                self.tcp_strangers.insert(e.connection_id);
            }
            FromSwarm::ConnectionClosed(e) => {
                self.tcp_strangers.remove(&e.connection_id);
                self.inbound.remove(&e.connection_id);
                self.inbound_lan.remove(&e.connection_id);
            }
            FromSwarm::ListenFailure(e) => {
                if self.pending_inbound.remove(&e.connection_id).is_some() {
                    self.publish_setting_up();
                }
            }
            FromSwarm::DialFailure(e) => {
                if let (Some(peer), DialError::Transport(attempts)) = (e.peer_id, e.error)
                    && !attempts.is_empty()
                    && attempts.iter().all(|(a, _)| is_plain_quic(a))
                    && !self.wanted.is_wanted(&peer)
                {
                    if self.quic_failed.len() >= PRUNE_AT {
                        self.quic_failed
                            .retain(|_, at| at.elapsed() < QUIC_FAILED_FOR);
                    }
                    self.quic_failed.insert(peer, Instant::now());
                }
            }
            _ => {}
        }
        self.inner.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<ToSwarm<B::ToSwarm, THandlerInEvent<Self>>> {
        if let Some((peer_id, connection)) = self.to_close.pop_front() {
            return Poll::Ready(ToSwarm::CloseConnection {
                peer_id,
                connection: CloseConnection::One(connection),
            });
        }
        self.inner.poll(cx)
    }
}

/// The public address a connection comes from, as the limits per address count it; none for
/// one from the local network or through a member's relay.
fn public_network(remote_addr: &Multiaddr) -> Option<std::net::IpAddr> {
    addr::ip(remote_addr)
        .filter(|_| !addr::is_relayed(remote_addr) && !addr::is_local(remote_addr))
        .map(network_of)
}

/// The address a limit per address counts by: an IPv4 address, or an IPv6 address's /64.
fn network_of(ip: std::net::IpAddr) -> std::net::IpAddr {
    match ip {
        std::net::IpAddr::V6(v6) => {
            let s = v6.segments();
            std::net::Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0).into()
        }
        v4 => v4,
    }
}

/// The site a limit per site counts by: an IPv6 address's /48; an IPv4 address itself (the
/// limit per address is the smaller one there).
fn site_of(ip: std::net::IpAddr) -> std::net::IpAddr {
    match ip {
        std::net::IpAddr::V6(v6) => {
            let s = v6.segments();
            std::net::Ipv6Addr::new(s[0], s[1], s[2], 0, 0, 0, 0, 0).into()
        }
        v4 => v4,
    }
}

/// The block a limit per block counts by: an IPv4 address's /16, an IPv6 address's /32.
fn block_of(ip: std::net::IpAddr) -> std::net::IpAddr {
    match ip {
        std::net::IpAddr::V6(v6) => {
            let s = v6.segments();
            std::net::Ipv6Addr::new(s[0], s[1], 0, 0, 0, 0, 0, 0).into()
        }
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            std::net::Ipv4Addr::new(o[0], o[1], 0, 0).into()
        }
    }
}

/// Keeps connections to wanted peers open while they are idle.
#[derive(Debug)]
pub(crate) struct KeepWanted {
    wanted: Wanted,
}

impl KeepWanted {
    pub(crate) fn new(wanted: Wanted) -> Self {
        Self { wanted }
    }

    fn handler(&self, peer: PeerId) -> KeepHandler {
        KeepHandler {
            peer,
            wanted: self.wanted.clone(),
        }
    }
}

impl NetworkBehaviour for KeepWanted {
    type ConnectionHandler = KeepHandler;
    type ToSwarm = Infallible;

    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(self.handler(peer))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
        _: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(self.handler(peer))
    }

    fn on_swarm_event(&mut self, _: FromSwarm) {}

    fn on_connection_handler_event(&mut self, _: PeerId, _: ConnectionId, event: Infallible) {
        match event {}
    }

    fn poll(&mut self, _: &mut Context<'_>) -> Poll<ToSwarm<Infallible, Infallible>> {
        Poll::Pending
    }
}

/// Speaks no protocol; it only answers whether its connection should stay open. The swarm
/// asks again whenever the idle connection wakes up (a ping every 15 seconds does that), so
/// a peer that stops being wanted is noticed without help.
#[derive(Debug)]
pub(crate) struct KeepHandler {
    peer: PeerId,
    wanted: Wanted,
}

impl ConnectionHandler for KeepHandler {
    type FromBehaviour = Infallible;
    type ToBehaviour = Infallible;
    type InboundProtocol = DeniedUpgrade;
    type OutboundProtocol = DeniedUpgrade;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<DeniedUpgrade> {
        SubstreamProtocol::new(DeniedUpgrade, ())
    }

    fn connection_keep_alive(&self) -> bool {
        self.wanted.keeps(&self.peer)
    }

    fn poll(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<ConnectionHandlerEvent<DeniedUpgrade, (), Infallible>> {
        Poll::Pending
    }

    fn on_behaviour_event(&mut self, event: Infallible) {
        match event {}
    }

    fn on_connection_event(&mut self, _: ConnectionEvent<DeniedUpgrade, DeniedUpgrade>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only an incoming connection from an address where no member or friend was reached
    /// gets the shorter time to be set up; this computer and known addresses get the
    /// swarm's, as slow links need.
    #[test]
    fn only_strangers_get_less_time_to_set_a_connection_up() {
        let wanted = Wanted::default();
        wanted.set_known(["203.0.113.7".parse::<std::net::IpAddr>().unwrap()]);
        let setup = StrangerSetup::new((), wanted);
        let from = |a: &str| a.parse::<Multiaddr>().unwrap();
        assert_eq!(setup.limit(&from("/ip4/127.0.0.1/tcp/4001")), None);
        assert_eq!(setup.limit(&from("/ip4/203.0.113.7/tcp/4001")), None);
        assert_eq!(
            setup.limit(&from("/ip4/203.0.113.8/tcp/4001")),
            Some((STRANGER_SETUP, false))
        );
        assert_eq!(
            setup.limit(&from("/ip4/192.168.1.20/tcp/4001")),
            Some((STRANGER_SETUP, true))
        );
        assert!(STRANGER_SETUP < crate::node::CONNECTION_TIMEOUT);
    }

    /// A transport whose one incoming connection, from `from`, takes `takes` to be set up.
    struct Slow {
        from: Multiaddr,
        takes: Duration,
        given: bool,
    }

    impl libp2p::core::Transport for Slow {
        type Output = ();
        type Error = std::io::Error;
        type ListenerUpgrade = futures::future::BoxFuture<'static, Result<(), std::io::Error>>;
        type Dial = futures::future::Ready<Result<(), std::io::Error>>;

        fn listen_on(
            &mut self,
            _: libp2p::core::transport::ListenerId,
            addr: Multiaddr,
        ) -> Result<(), libp2p::core::transport::TransportError<Self::Error>> {
            Err(libp2p::core::transport::TransportError::MultiaddrNotSupported(addr))
        }

        fn remove_listener(&mut self, _: libp2p::core::transport::ListenerId) -> bool {
            false
        }

        fn dial(
            &mut self,
            addr: Multiaddr,
            _: libp2p::core::transport::DialOpts,
        ) -> Result<Self::Dial, libp2p::core::transport::TransportError<Self::Error>> {
            Err(libp2p::core::transport::TransportError::MultiaddrNotSupported(addr))
        }

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<libp2p::core::transport::TransportEvent<Self::ListenerUpgrade, Self::Error>>
        {
            use futures::FutureExt as _;
            if self.given {
                return Poll::Pending;
            }
            self.given = true;
            let takes = self.takes;
            Poll::Ready(libp2p::core::transport::TransportEvent::Incoming {
                listener_id: libp2p::core::transport::ListenerId::next(),
                upgrade: async move {
                    tokio::time::sleep(takes).await;
                    Ok(())
                }
                .boxed(),
                local_addr: "/ip4/192.168.1.2/tcp/4001".parse().unwrap(),
                send_back_addr: self.from.clone(),
            })
        }
    }

    /// Whether an incoming connection from `from` that takes `takes` to be set up is, past
    /// [`STRANGER_SETUP`], with the places for strangers and for the local network being
    /// set up as `setting_up` says.
    async fn set_up(from: &str, takes: Duration, setting_up: (usize, usize)) -> bool {
        let wanted = Wanted::default();
        crate::locked(&wanted.0).tcp_setting_up = setting_up;
        let mut setup = StrangerSetup::new(
            Slow {
                from: from.parse().unwrap(),
                takes,
                given: false,
            },
            wanted,
        );
        let event = futures::future::poll_fn(|cx| {
            libp2p::core::Transport::poll(std::pin::Pin::new(&mut setup), cx)
        })
        .await;
        let libp2p::core::transport::TransportEvent::Incoming { upgrade, .. } = event else {
            panic!("no incoming connection");
        };
        upgrade.await.is_ok()
    }

    /// A member on a slow, lossy link takes longer than [`STRANGER_SETUP`] to set up a TCP
    /// connection from an address new here, on the internet or the local network: it gets
    /// the swarm's time while others are not waiting for places, and only then the shorter.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_slow_connection_from_a_new_address_gets_the_time_it_needs_unless_crowded() {
        let slow = STRANGER_SETUP + Duration::from_secs(3);
        assert!(slow < crate::node::CONNECTION_TIMEOUT);
        let (internet, lan) = ("/ip4/203.0.113.8/tcp/4001", "/ip4/192.168.1.20/tcp/4001");
        let crowded = (PENDING_TCP.strangers / 2, PENDING_TCP.lan / 2);
        // All at once, on the clock: each takes its seconds.
        let cases = [
            (internet, slow, (0, 0), true),
            (lan, slow, (0, 0), true),
            (internet, STRANGER_SETUP / 2, (128, 48), true),
            (internet, slow, crowded, false),
            (lan, slow, crowded, false),
            // Each kind counts its own places.
            (internet, slow, (0, PENDING_TCP.lan), true),
            (lan, slow, (PENDING_TCP.strangers, 0), true),
        ];
        let done = futures::future::join_all(
            cases
                .iter()
                .map(|(from, takes, setting_up, _)| set_up(from, *takes, *setting_up)),
        )
        .await;
        for ((from, takes, setting_up, expected), done) in cases.iter().zip(done) {
            assert_eq!(
                done, *expected,
                "{from} taking {takes:?} with {setting_up:?} being set up"
            );
        }
    }

    #[test]
    fn nothing_is_looked_up_in_dns_that_need_not_be() {
        let parse = |a: &str| a.parse::<Multiaddr>().unwrap();
        let quic = parse("/ip4/203.0.113.9/udp/4001/quic-v1");
        let websocket = parse(
            "/dns4/203-0-113-9.k51qzi5uqu5d.libp2p.direct/tcp/4001/tls/sni/203-0-113-9.k51qzi5uqu5d.libp2p.direct/ws",
        );
        let named = parse("/dns4/node.example.org/udp/4001/quic-v1");
        let bootstrap = parse("/dnsaddr/bootstrap.libp2p.io");
        let relayed = parse(
            "/ip4/203.0.113.9/udp/4001/quic-v1/p2p/12D3KooWNHgcaTZmBFySL7DfWJyuGybDV7RXu9CsTxrQzvqnGeVK/p2p-circuit",
        );
        assert_eq!(
            without_lookups(vec![
                websocket.clone(),
                named.clone(),
                quic.clone(),
                bootstrap.clone(),
                relayed.clone()
            ]),
            vec![quic.clone(), relayed]
        );
        // A bootstrap peer known only by its /dnsaddr is still reached.
        assert_eq!(
            without_lookups(vec![bootstrap.clone(), named, websocket]),
            vec![bootstrap]
        );
        assert!(without_lookups(vec![parse("/ip4/203.0.113.9/udp/1/webrtc-direct")]).is_empty());
    }

    #[test]
    fn one_address_sets_up_only_a_few_connections_at_once() {
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            Wanted::default(),
            LanKeys::default(),
        );
        let local = a("/ip4/0.0.0.0/tcp/30000");
        let from = |port: u16| a(&format!("/ip4/203.0.113.9/tcp/{port}"));
        let mut pending = Vec::new();
        for i in 0..MAX_PENDING_PER_ADDRESS {
            let id = ConnectionId::new_unchecked(i);
            assert!(
                dials
                    .handle_pending_inbound_connection(id, &local, &from(40000 + i as u16))
                    .is_ok()
            );
            pending.push(id);
        }
        let next = ConnectionId::new_unchecked(100);
        assert!(
            dials
                .handle_pending_inbound_connection(next, &local, &from(41000))
                .is_err()
        );
        // Another address, and the local network, are not held back.
        let other = a("/ip4/198.51.100.4/tcp/40000");
        let lan = a("/ip4/192.168.1.20/tcp/40000");
        for (i, address) in [(101, &other), (102, &lan)] {
            let id = ConnectionId::new_unchecked(i);
            assert!(
                dials
                    .handle_pending_inbound_connection(id, &local, address)
                    .is_ok()
            );
        }
        // One that failed makes room for the next.
        let failed = libp2p::swarm::ListenError::Aborted;
        dials.on_swarm_event(FromSwarm::ListenFailure(libp2p::swarm::ListenFailure {
            local_addr: &local,
            send_back_addr: &from(40000),
            error: &failed,
            connection_id: pending[0],
            peer_id: None,
        }));
        assert!(
            dials
                .handle_pending_inbound_connection(next, &local, &from(41000))
                .is_ok()
        );
    }

    #[test]
    fn connections_being_set_up_are_bounded_from_the_local_network_and_in_all() {
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            Wanted::default(),
            LanKeys::default(),
        );
        let local = a("/ip4/0.0.0.0/tcp/30000");
        let mut pend = |id: usize, from: &str| {
            dials
                .handle_pending_inbound_connection(
                    ConnectionId::new_unchecked(id),
                    &local,
                    &a(from),
                )
                .is_ok()
        };
        // One machine on the local network: more than from a public address, still bounded.
        let lan = |port: usize| format!("/ip4/192.168.1.20/tcp/{port}");
        let taken = (0..100).filter(|i| pend(*i, &lan(40000 + i))).count();
        assert_eq!(taken, MAX_PENDING_PER_LAN_ADDRESS);
        // The local network together is, many addresses or not...
        let taken = (0..100)
            .filter(|i| pend(200 + i, &format!("/ip4/192.168.1.{}/tcp/40000", 21 + i)))
            .count();
        assert_eq!(taken, PENDING_TCP.lan - MAX_PENDING_PER_LAN_ADDRESS);
        // ...and so are strangers from public addresses, however many addresses, /48s and
        // blocks they use.
        let taken = (0..2 * PENDING_TCP.strangers)
            .filter(|i| pend(1000 + i, &stranger_at(*i)))
            .count();
        assert_eq!(taken, PENDING_TCP.strangers);
        // This computer itself is not bounded by address, only with everything together.
        let taken = (0..100)
            .filter(|i| pend(2000 + i, &format!("/ip4/127.0.0.1/tcp/{}", 40000 + i)))
            .count();
        assert_eq!(
            taken,
            PENDING_TCP.all - PENDING_TCP.lan - PENDING_TCP.strangers
        );
        // A connection through a member's relay is not one this device holds a socket for.
        let relayed = "/ip4/203.0.113.9/udp/4001/quic-v1/p2p/12D3KooWNHgcaTZmBFySL7DfWJyuGybDV7RXu9CsTxrQzvqnGeVK/p2p-circuit";
        assert!(pend(5000, relayed));
    }

    /// A public TCP address for stranger `i`, one each, spread over 16 blocks (/16).
    fn stranger_at(i: usize) -> String {
        format!(
            "/ip4/{}.{}.{}.{}/tcp/40000",
            11 + i % 16,
            i / 16,
            7,
            2 + i % 200
        )
    }

    #[test]
    fn silent_strangers_leave_room_for_known_addresses_and_the_local_network() {
        let wanted = Wanted::default();
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            wanted.clone(),
            LanKeys::default(),
        );
        let local = a("/ip4/0.0.0.0/tcp/30000");
        let mut pend = |id: usize, from: &str| {
            dials
                .handle_pending_inbound_connection(
                    ConnectionId::new_unchecked(id),
                    &local,
                    &a(from),
                )
                .is_ok()
        };
        // Sixteen public addresses in eight blocks open as many connections as each may at
        // once and never say anything on them: together they get only the strangers'
        // share.
        let mute =
            |i: usize, port: usize| format!("/ip4/{}.0.113.{}/tcp/{port}", 11 + i % 8, 50 + i);
        let mut taken = 0;
        for i in 0..16 {
            for k in 0..MAX_PENDING_PER_ADDRESS {
                taken += usize::from(pend(i * 100 + k, &mute(i, 40000 + k)));
            }
        }
        assert_eq!(taken, PENDING_TCP.strangers);
        assert!(!pend(5000, "/ip4/203.0.113.9/tcp/40000"));
        // A friend whose address this device reached before, from a network that blocks
        // UDP, still gets in over TCP, as does the local network; one behind the friend's
        // address gets only that address's few places.
        wanted.set_known([
            std::net::IpAddr::from([198, 51, 100, 4]),
            std::net::IpAddr::from([198, 51, 100, 5]),
        ]);
        let friend = |port: usize| format!("/ip4/198.51.100.4/tcp/{port}");
        assert!(pend(6000, &friend(40000)));
        assert!(pend(6001, "/ip4/192.168.1.20/tcp/40000"));
        let taken = (1..100)
            .filter(|i| pend(6100 + i, &friend(40000 + i)))
            .count();
        assert_eq!(taken, MAX_PENDING_PER_ADDRESS - 1);
        // A flood over QUIC (from addresses proven to be the senders') is held to its own
        // shares, and leaves the same room.
        let quic = |i: usize| format!("/ip4/{}.{}.113.7/udp/40000/quic-v1", 30 + i % 64, i / 64);
        let taken = (0..2 * PENDING_QUIC.strangers)
            .filter(|i| pend(10000 + i, &quic(*i)))
            .count();
        assert_eq!(taken, PENDING_QUIC.strangers);
        assert!(pend(20000, "/ip4/198.51.100.5/udp/45000/quic-v1"));
        assert!(pend(20001, "/ip4/192.168.1.21/udp/40000/quic-v1"));
        assert!(!pend(20002, "/ip4/203.0.113.9/udp/40000/quic-v1"));
    }

    #[test]
    fn a_known_address_gets_in_when_its_block_and_site_are_full() {
        let wanted = Wanted::default();
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            wanted.clone(),
            LanKeys::default(),
        );
        let mut pend = |id: usize, from: &str| {
            let local = a(if from.starts_with("/ip6") {
                "/ip6/::/tcp/30000"
            } else {
                "/ip4/0.0.0.0/tcp/30000"
            });
            dials
                .handle_pending_inbound_connection(
                    ConnectionId::new_unchecked(id),
                    &local,
                    &a(from),
                )
                .is_ok()
        };
        // Silent strangers at the same provider as a member fill its block (/16) and its
        // IPv6 /48 (and so its /32).
        let taken = (0..100)
            .filter(|i| pend(*i, &format!("/ip4/203.0.113.{}/tcp/40000", 100 + i % 50)))
            .count();
        assert_eq!(taken, MAX_PENDING_PER_BLOCK);
        let taken = (0..100)
            .filter(|i| {
                pend(
                    200 + i,
                    &format!("/ip6/2001:db8:0:{:x}::1/tcp/40000", i % 50),
                )
            })
            .count();
        assert_eq!(taken, MAX_PENDING_PER_SITE);
        assert!(!pend(400, "/ip4/203.0.113.22/tcp/40000"));
        assert!(!pend(401, "/ip6/2001:db8:0:77::1/tcp/40000"));
        // The member, reached at its address before, still gets that address's places.
        wanted.set_known([
            std::net::IpAddr::from([203, 0, 113, 22]),
            "2001:db8:0:77::1".parse().unwrap(),
        ]);
        let taken = (0..20)
            .filter(|i| pend(500 + i, "/ip4/203.0.113.22/tcp/40000"))
            .count();
        assert_eq!(taken, MAX_PENDING_PER_ADDRESS);
        let taken = (0..20)
            .filter(|i| pend(600 + i, "/ip6/2001:db8:0:77::2/tcp/40000"))
            .count();
        assert_eq!(taken, MAX_PENDING_PER_ADDRESS);
    }

    #[test]
    fn one_block_of_addresses_sets_up_only_so_many_connections() {
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            Wanted::default(),
            LanKeys::default(),
        );
        let local = a("/ip4/0.0.0.0/tcp/30000");
        let mut pend = |id: usize, from: &str| {
            dials
                .handle_pending_inbound_connection(
                    ConnectionId::new_unchecked(id),
                    &local,
                    &a(from),
                )
                .is_ok()
        };
        // A hundred addresses lent by one provider, all in one /16.
        let taken = (0..100)
            .filter(|i| pend(*i, &format!("/ip4/203.0.{}.{}/tcp/40000", i / 50, 2 + i)))
            .count();
        assert_eq!(taken, MAX_PENDING_PER_BLOCK);
        // Another block is not held back.
        assert!(pend(200, "/ip4/203.1.0.2/tcp/40000"));
        // An IPv6 /32 is a block too, whatever /48s and /64s it is used from.
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            Wanted::default(),
            LanKeys::default(),
        );
        let local = a("/ip6/::/tcp/30000");
        let mut pend = |id: usize, from: &str| {
            dials
                .handle_pending_inbound_connection(
                    ConnectionId::new_unchecked(id),
                    &local,
                    &a(from),
                )
                .is_ok()
        };
        let taken = (0..100)
            .filter(|i| pend(300 + i, &format!("/ip6/2001:db8:{i:x}::1/tcp/40000")))
            .count();
        assert_eq!(taken, MAX_PENDING_PER_BLOCK);
        assert!(pend(500, "/ip6/2001:db9::1/tcp/40000"));
    }

    /// Opens an incoming connection from `peer` at `from`; whether it was taken.
    fn come_in(
        dials: &mut StrangerDials<libp2p::swarm::dummy::Behaviour>,
        id: usize,
        peer: PeerId,
        from: &str,
    ) -> bool {
        dials
            .handle_established_inbound_connection(
                ConnectionId::new_unchecked(id),
                peer,
                &a("/ip4/0.0.0.0/udp/30000/quic-v1"),
                &a(from),
            )
            .is_ok()
    }

    #[test]
    fn peers_that_only_spoke_make_room_for_others() {
        let wanted = Wanted::default();
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            wanted.clone(),
            LanKeys::default(),
        );
        // 16 addresses, as many connections from each as one address may open, each peer
        // having sent a request.
        let mut first = None;
        for i in 0..MAX_INBOUND {
            let peer = PeerId::random();
            wanted.hear(peer);
            first.get_or_insert(peer);
            let from = format!("/ip4/203.0.113.{}/udp/{}/quic-v1", i / 32, 40000 + i);
            assert!(come_in(&mut dials, i, peer, &from));
        }
        assert!(dials.to_close.is_empty());
        // A friend coming in now (not recognised before its handshake) gets in, and the
        // oldest of the others is closed for it.
        assert!(come_in(
            &mut dials,
            1000,
            PeerId::random(),
            "/ip4/198.51.100.4/udp/40000/quic-v1"
        ));
        assert_eq!(dials.to_close.front().map(|(p, _)| *p), first);
        // Connections of wanted peers (members, friends online, peers this device dials)
        // are never closed for another.
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            wanted.clone(),
            LanKeys::default(),
        );
        for i in 0..MAX_INBOUND {
            let peer = PeerId::random();
            wanted.want(peer);
            let from = format!("/ip4/203.0.113.{}/udp/{}/quic-v1", i / 32, 40000 + i);
            assert!(come_in(&mut dials, i, peer, &from));
        }
        assert!(!come_in(
            &mut dials,
            1000,
            PeerId::random(),
            "/ip4/198.51.100.4/udp/40000/quic-v1"
        ));
    }

    #[test]
    fn one_machine_on_the_local_network_or_one_peer_keeps_only_so_many_open() {
        let wanted = Wanted::default();
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            wanted.clone(),
            LanKeys::default(),
        );
        // A neighbour opens connection after connection, with a new peer each time: past
        // its share, its oldest one is closed for each new one.
        let from = |i: usize| format!("/ip4/192.168.1.66/udp/{}/quic-v1", 40000 + i);
        let mut first = None;
        for i in 0..MAX_INBOUND_PER_LAN_ADDRESS {
            let peer = PeerId::random();
            first.get_or_insert(peer);
            assert!(come_in(&mut dials, i, peer, &from(i)));
        }
        assert!(dials.to_close.is_empty());
        assert!(come_in(&mut dials, 100, PeerId::random(), &from(100)));
        assert_eq!(dials.to_close.front().map(|(p, _)| *p), first);
        // Another device there, and this computer, are not held back.
        assert!(come_in(
            &mut dials,
            101,
            PeerId::random(),
            "/ip4/192.168.1.20/udp/4001/quic-v1"
        ));
        for i in 0..2 * MAX_INBOUND_PER_LAN_ADDRESS {
            let at = format!("/ip4/127.0.0.1/udp/{}/quic-v1", 40000 + i);
            assert!(come_in(&mut dials, 200 + i, PeerId::random(), &at));
        }
        // One peer, even a wanted one, from many addresses: a handful.
        let named = PeerId::random();
        wanted.want(named);
        let taken = (0..64)
            .filter(|i| {
                let at = format!("/ip4/203.0.113.{}/udp/4001/quic-v1", i);
                come_in(&mut dials, 1000 + i, named, &at)
            })
            .count();
        assert_eq!(taken, MAX_INBOUND_PER_PEER);
    }

    #[test]
    fn one_ipv6_site_opens_only_so_many_connections() {
        let mut dials = StrangerDials::new(
            libp2p::swarm::dummy::Behaviour,
            Wanted::default(),
            LanKeys::default(),
        );
        // Each from a /64 of its own, all in one /48.
        let from = |i: usize| format!("/ip6/2001:db8:7:{i:x}::1/udp/40000/quic-v1");
        for i in 0..MAX_INBOUND_PER_SITE {
            assert!(come_in(&mut dials, i, PeerId::random(), &from(i)));
        }
        assert!(!come_in(&mut dials, 100, PeerId::random(), &from(100)));
        // Another /48 is not held back.
        assert!(come_in(
            &mut dials,
            101,
            PeerId::random(),
            "/ip6/2001:db8:8::1/udp/40000/quic-v1"
        ));
        // Nor are connections being set up from it, beyond their own bound.
        let local = a("/ip6/::/udp/30000/quic-v1");
        let mut taken = 0;
        for i in 0..2 * MAX_PENDING_PER_SITE {
            let id = ConnectionId::new_unchecked(200 + i);
            let pending = a(&format!("/ip6/2001:db8:9:{i:x}::1/udp/40000/quic-v1"));
            taken += usize::from(
                dials
                    .handle_pending_inbound_connection(id, &local, &pending)
                    .is_ok(),
            );
        }
        assert_eq!(taken, MAX_PENDING_PER_SITE);
    }

    #[test]
    fn one_ipv6_network_counts_as_one_address() {
        let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
        assert_eq!(
            network_of(ip("2001:db8:1:2:aaaa::1")),
            network_of(ip("2001:db8:1:2:bbbb::9"))
        );
        assert_ne!(
            network_of(ip("2001:db8:1:2::1")),
            network_of(ip("2001:db8:1:3::1"))
        );
        assert_eq!(network_of(ip("192.0.2.7")), ip("192.0.2.7"));
    }

    fn a(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    const QUIC_WORKS: Situation = Situation {
        quic_works: true,
        quic_failed: false,
        tcp_room: true,
    };

    fn stranger() -> Vec<Multiaddr> {
        vec![
            a("/ip4/192.168.1.7/tcp/4001"),
            a("/ip4/192.168.1.7/udp/4001/quic-v1"),
            a("/ip4/8.8.8.8/tcp/4001"),
            a("/ip4/8.8.8.8/udp/4001/quic-v1/webtransport"),
            a("/ip4/8.8.8.8/udp/4001/quic-v1"),
            a("/ip6/2001:db8::7/udp/4001/quic-v1"),
        ]
    }

    fn public_non_quic() -> Vec<Multiaddr> {
        vec![
            a("/ip4/8.8.8.8/tcp/4001"),
            a("/ip4/8.8.8.8/udp/4001/quic-v1/webtransport"),
        ]
    }

    #[test]
    fn strangers_are_dialled_at_public_quic_addresses_once_quic_works() {
        let quic = vec![
            a("/ip4/8.8.8.8/udp/4001/quic-v1"),
            a("/ip6/2001:db8::7/udp/4001/quic-v1"),
        ];
        assert_eq!(for_stranger(stranger(), QUIC_WORKS), quic);
        let full = Situation {
            tcp_room: false,
            ..QUIC_WORKS
        };
        assert_eq!(for_stranger(stranger(), full), quic);
        // Until a QUIC connection succeeded here, UDP may be blocked: keep TCP.
        let unknown = Situation {
            quic_works: false,
            quic_failed: false,
            tcp_room: false,
        };
        let public: Vec<Multiaddr> = stranger().into_iter().skip(2).collect();
        assert_eq!(for_stranger(stranger(), unknown), public);
    }

    #[test]
    fn strangers_whose_quic_failed_get_tcp_while_there_is_room() {
        let failed = Situation {
            quic_failed: true,
            ..QUIC_WORKS
        };
        assert_eq!(for_stranger(stranger(), failed), public_non_quic());
        let full = Situation {
            tcp_room: false,
            ..failed
        };
        assert!(for_stranger(stranger(), full).is_empty());
    }

    #[test]
    fn strangers_known_only_by_tcp_are_also_tried_over_quic() {
        let peer = PeerId::random();
        let tcp_only = vec![
            a("/ip4/8.8.8.8/tcp/4001").with(Protocol::P2p(peer)),
            a("/ip6/2001:db8::7/tcp/4002"),
            a("/dns4/example.net/tcp/4001/tls/ws"),
            a("/ip4/8.8.8.8/tcp/4001/tls/ws"),
        ];
        let twins = [
            a("/ip4/8.8.8.8/udp/4001/quic-v1").with(Protocol::P2p(peer)),
            a("/ip6/2001:db8::7/udp/4002/quic-v1"),
        ];
        let with_tcp: Vec<Multiaddr> = twins.iter().chain(&tcp_only).cloned().collect();
        assert_eq!(for_stranger(tcp_only.clone(), QUIC_WORKS), with_tcp);
        // No room for another TCP connection: only names are left.
        let full = Situation {
            tcp_room: false,
            ..QUIC_WORKS
        };
        assert_eq!(
            for_stranger(tcp_only.clone(), full),
            vec![a("/dns4/example.net/tcp/4001/tls/ws")]
        );
    }

    #[test]
    fn strangers_without_public_addresses_keep_their_local_ones() {
        let lan_only = vec![
            a("/ip4/10.0.0.2/tcp/4001"),
            a("/ip4/10.0.0.2/udp/4001/quic-v1"),
        ];
        assert_eq!(
            for_stranger(lan_only, QUIC_WORKS),
            vec![a("/ip4/10.0.0.2/udp/4001/quic-v1")]
        );
        let dns = vec![a("/dnsaddr/bootstrap.libp2p.io")];
        assert_eq!(for_stranger(dns.clone(), QUIC_WORKS), dns);
    }

    #[test]
    fn peers_known_only_from_the_dht_get_a_few_public_addresses() {
        let mut found = vec![
            a("/ip4/192.168.1.7/udp/4001/quic-v1"),
            a("/ip4/10.0.0.1/udp/4001/quic-v1"),
            a("/ip4/127.0.0.1/udp/4001/quic-v1"),
            a("/ip6/fe80::1/udp/4001/quic-v1"),
        ];
        found.extend((0..20).map(|port| a(&format!("/ip4/203.0.113.9/udp/{port}/quic-v1"))));
        let dialled = for_dht_peer(found);
        assert_eq!(dialled.len(), MAX_DHT_ADDRESSES);
        assert!(dialled.iter().all(|d| !addr::is_local(d)));
        let wanted = Wanted::default();
        let peer = PeerId::random();
        wanted.found_on_dht(peer);
        assert!(wanted.only_from_dht(&peer));
        wanted.found_nearby(&peer);
        assert!(!wanted.only_from_dht(&peer));
    }

    #[test]
    fn many_wanted_peers_are_pruned_now_and_then_not_at_every_one() {
        let wanted = Wanted::default();
        let peers: Vec<PeerId> = (0..PRUNE_AT).map(|_| PeerId::random()).collect();
        for peer in &peers {
            wanted.want(*peer);
        }
        // All of them expired: the next one prunes them, the ones after that do not look.
        if let Ok(mut state) = wanted.0.lock() {
            let past = Instant::now() - Duration::from_secs(1);
            state.until.values_mut().for_each(|until| *until = past);
        }
        wanted.want(PeerId::random());
        assert_eq!(crate::locked(&wanted.0).until.len(), 1);
        let start = Instant::now();
        for _ in 0..50 * PRUNE_AT {
            wanted.hear(PeerId::random());
        }
        assert_eq!(crate::locked(&wanted.0).heard.len(), 50 * PRUNE_AT);
        // Fifty thousand peers heard a moment apart, in a bounded time (each used to go
        // through all the others once past the threshold).
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn wanted_peers_expire() {
        let wanted = Wanted::default();
        let peer = PeerId::random();
        assert!(!wanted.is_wanted(&peer));
        wanted.want(peer);
        assert!(wanted.is_wanted(&peer));
        if let Ok(mut state) = wanted.0.lock() {
            state.until.insert(peer, Instant::now());
        }
        assert!(!wanted.is_wanted(&peer));
        assert!(!wanted.quic_proven());
        wanted.quic_works();
        assert!(wanted.quic_proven());
    }
}
