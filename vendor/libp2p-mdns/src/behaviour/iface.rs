// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

mod dns;
mod query;

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::{SinkExt, StreamExt, channel::mpsc};
use if_watch::IpNet;
use libp2p_core::{Multiaddr, multiaddr::Protocol};
use libp2p_identity::PeerId;
use libp2p_swarm::ListenAddresses;
use socket2::{Domain, Socket, Type};

use self::{
    dns::{build_query, build_query_response, build_service_discovery_response},
    query::MdnsPacket,
};
use crate::{
    Config,
    behaviour::{socket::AsyncSocket, timer::Builder},
};

/// Initial interval for starting probe
const INITIAL_TIMEOUT_INTERVAL: Duration = Duration::from_millis(500);

/// How long after one of its own queries a response to it is taken, and how many of its
/// latest queries are remembered. Responders answer at once, but what they answer may come
/// late: macOS's firewall holds each new sender a socket hears from until it has checked it,
/// one after the other, which on a busy computer took more than five seconds (Fogata, see
/// FOGATA.md), and an answer dropped as late then waited for the next query, a whole
/// interval away. One query interval of Fogata's: past that, a newer query asked anyway.
const ANSWER_WINDOW: Duration = Duration::from_secs(30);
const QUERIES_KEPT: usize = 8;
/// Most discovered addresses waiting to be handed to the behaviour: one 4 KiB response can
/// name dozens of made-up peers, and responses can come as fast as anyone sends them.
const MAX_WAITING: usize = 256;

/// What an interface found: a peer, one of its addresses, when that expires, and the
/// address of the response that told of it.
pub(crate) type Found = (PeerId, Multiaddr, Instant, IpAddr);

/// A packet a socket read: its bytes and its sender.
type Packet = (Vec<u8>, SocketAddr);
/// Most packets read by the other interfaces' sockets that wait for an interface's task.
const MAX_HANDED: usize = 16;
/// Shortest time between two answers of an interface to queries other interfaces' sockets
/// read: anyone who reaches one of those can send them with a made-up sender.
const HANDED_ANSWER_GAP: Duration = Duration::from_millis(250);

/// Whether a packet one interface's socket reads from another interface's network is handed
/// to that interface (Fogata, see FOGATA.md): on Linux, and on Android, whose kernel it is.
/// There the socket of every interface is bound to port 5353 of all the device's addresses,
/// and the kernel gives a packet to one of them whatever link it came over (its early
/// demultiplexing of UDP does so with multicast packets too), so the socket that read a
/// packet tells nothing of where it came from: only its sender's address does. Elsewhere a
/// packet stays with the socket that read it, as upstream: macOS gives a socket the group's
/// packets of the links it joined it on, and a socket's network keeps out a made-up sender
/// of another network sent over this one. Deciding by the interface a packet came over
/// (`IP_PKTINFO`, `IP_RECVIF`) would be the whole answer on every system; it is not done.
const HAND_OVER: bool = cfg!(any(target_os = "linux", target_os = "android"));

/// The interfaces of one behaviour, by address, each with its network and where to hand it
/// a packet another interface's socket read from its network (Fogata, see [`HAND_OVER`]):
/// an answer to its queries, or a query to answer with its addresses, on its link.
#[derive(Debug, Clone)]
pub(crate) struct Interfaces {
    tasks: Arc<Mutex<HashMap<IpAddr, (IpNet, mpsc::Sender<Packet>)>>>,
    hand_over: bool,
}

impl Default for Interfaces {
    fn default() -> Self {
        Self::new(HAND_OVER)
    }
}

impl Interfaces {
    fn new(hand_over: bool) -> Self {
        Self {
            tasks: Default::default(),
            hand_over,
        }
    }

    /// Adds the interface of `net`, and gives what will be handed to it.
    fn add(&self, net: IpNet) -> mpsc::Receiver<Packet> {
        // The channel holds one more than its buffer for each sender, and there is one.
        let (inbox, handed) = mpsc::channel(MAX_HANDED - 1);
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(net.addr(), (net, inbox));
        handed
    }

    /// Removes the interface of address `addr`.
    pub(crate) fn remove(&self, addr: &IpAddr) {
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(addr);
    }

    /// Removes the interface of address `addr` while it is the one `handed` is of: a task
    /// that ends leaves no place packets would be handed to and lost.
    fn remove_own(&self, addr: &IpAddr, handed: &mpsc::Receiver<Packet>) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if tasks
            .get(addr)
            .is_some_and(|(_, inbox)| inbox.is_connected_to(handed))
        {
            tasks.remove(addr);
        }
    }

    /// Hands the packet `bytes` from `from`, read by the socket of the interface of `own`,
    /// to the interface whose network is the narrowest that holds `from`, when that is
    /// another one, and gives that one's address; `None` keeps it with the socket that read
    /// it: no hand-over on this system, a link-local sender (it may be on any link), one of
    /// the device's own addresses (its own queries, looped back, and those of copies on this
    /// computer, which all send from there: answered as upstream, by the socket that read
    /// them), or one on no other network of the device. An interface whose task ended is
    /// passed over. A packet past what may wait for an interface is dropped, as one lost on
    /// the way.
    fn hand(&self, own: &IpNet, bytes: &[u8], from: SocketAddr) -> Option<IpAddr> {
        let ip = from.ip();
        if !self.hand_over || is_link_local(ip) {
            return None;
        }
        let (to, dropped) = {
            let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if tasks.contains_key(&ip) {
                return None;
            }
            let (addr, (net, inbox)) = tasks
                .iter_mut()
                .filter(|(addr, (net, inbox))| {
                    **addr != own.addr() && net.contains(&ip) && !inbox.is_closed()
                })
                .max_by_key(|(addr, (net, _))| (net.prefix_len(), std::cmp::Reverse(**addr)))?;
            if own.contains(&ip) && own.prefix_len() >= net.prefix_len() {
                return None;
            }
            match inbox.try_send((bytes.to_vec(), from)) {
                Ok(()) => (*addr, false),
                Err(e) if e.is_disconnected() => return None,
                Err(_) => (*addr, true),
            }
        };
        if dropped {
            tracing::debug!(
                address=%to,
                remote_address=%from,
                "dropped a packet for a busy interface"
            );
        }
        Some(to)
    }
}

/// What an interface's task read next.
enum Read {
    /// A packet its socket read, of its length in the buffer.
    Own(usize, SocketAddr),
    /// One another interface's socket read from this one's network.
    Handed(Packet),
    /// Nothing to take here: one its socket read and handed to another interface, or none
    /// after all (the socket would block).
    Skip,
}

#[derive(Debug, Clone)]
enum ProbeState {
    Probing(Duration),
    Finished(Duration),
}

impl Default for ProbeState {
    fn default() -> Self {
        ProbeState::Probing(INITIAL_TIMEOUT_INTERVAL)
    }
}

impl ProbeState {
    fn interval(&self) -> &Duration {
        match self {
            ProbeState::Probing(query_interval) => query_interval,
            ProbeState::Finished(query_interval) => query_interval,
        }
    }
}

/// An mDNS instance for a networking interface. To discover all peers when having multiple
/// interfaces an [`InterfaceState`] is required for each interface.
#[derive(Debug)]
pub(crate) struct InterfaceState<U, T> {
    /// Address this instance is bound to.
    addr: IpAddr,
    /// The network of that address: responses are taken only from on it.
    net: IpNet,
    /// Receive socket, bound to port 5353. Over IPv4 it sends too (Fogata).
    recv_socket: U,
    /// The interfaces of the behaviour, to hand them what this socket read from their
    /// networks, and what they read from this one's (Fogata).
    interfaces: Interfaces,
    handed: mpsc::Receiver<Packet>,
    /// Whether the next packet is taken from `handed` first, else from the socket: they take
    /// turns, so that neither keeps the other waiting.
    handed_turn: bool,
    /// Whether this socket sends on this interface's link (`IP_MULTICAST_IF` taken): only
    /// then are queries other interfaces' sockets read answered (Fogata).
    sends_on_own_link: bool,
    /// When the last of those was answered, and how long until the next may be (see
    /// `HANDED_ANSWER_GAP`).
    last_handed_answer: Option<Instant>,
    handed_answer_gap: Duration,
    /// Send socket, for IPv6 only; `None` over IPv4, where everything goes from the
    /// receive socket.
    send_socket: Option<U>,

    listen_addresses: Arc<RwLock<ListenAddresses>>,

    query_response_sender: mpsc::Sender<Found>,

    /// Buffer used for receiving data from the main socket.
    /// RFC6762 discourages packets larger than the interface MTU, but allows sizes of up to 9000
    /// bytes, if it can be ensured that all participating devices can handle such large packets.
    /// For computers with several interfaces and IP addresses responses can easily reach sizes in
    /// the range of 3000 bytes, so 4096 seems sensible for now. For more information see
    /// [rfc6762](https://tools.ietf.org/html/rfc6762#page-46).
    recv_buffer: [u8; 4096],
    /// Buffers pending to send on the main socket.
    send_buffer: VecDeque<Vec<u8>>,
    /// Discovery interval.
    query_interval: Duration,
    /// Discovery timer.
    timeout: T,
    /// Multicast address.
    multicast_addr: IpAddr,
    /// Discovered addresses.
    discovered: VecDeque<Found>,
    /// TTL
    ttl: Duration,
    /// Longest a discovered address is kept without being heard of again, whatever TTL its
    /// response gives: two query intervals, so that one lost response does not lose it.
    max_ttl: Duration,
    /// Ids of this instance's latest queries, and when each was sent.
    asked: VecDeque<(u16, Instant)>,
    /// The DNS-SD service asked for and answered (Fogata, see `Config::service_name`).
    service_name: String,
    probe_state: ProbeState,
    local_peer_id: PeerId,
}

impl<U, T> InterfaceState<U, T>
where
    U: AsyncSocket,
    T: Builder + futures::Stream,
{
    /// Builds a new [`InterfaceState`].
    pub(crate) fn new(
        net: IpNet,
        config: Config,
        local_peer_id: PeerId,
        listen_addresses: Arc<RwLock<ListenAddresses>>,
        query_response_sender: mpsc::Sender<Found>,
        interfaces: Interfaces,
    ) -> io::Result<Self> {
        let addr = net.addr();
        tracing::info!(address=%addr, "creating instance on iface address");
        let (recv_socket, sends_on_own_link) = match addr {
            IpAddr::V4(addr) => {
                let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(socket2::Protocol::UDP))?;
                socket.set_reuse_address(true)?;
                #[cfg(unix)]
                socket.set_reuse_port(true)?;
                socket.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 5353).into())?;
                socket.set_multicast_loop_v4(true)?;
                socket.set_multicast_ttl_v4(255)?;
                socket.join_multicast_v4(&crate::IPV4_MDNS_MULTICAST_ADDRESS, &addr)?;
                // Queries and answers go from this socket, from port 5353 on this interface
                // (Fogata, see FOGATA.md): every copy on a computer then sends from the same
                // address and port, which each other copy's socket meets once, instead of
                // one more of its own per copy and per interface. macOS's firewall checks
                // every new sender a socket hears from, one after the other. Should the
                // system refuse to pick the interface, the routes pick one, as before.
                let on_own_link = socket
                    .set_multicast_if_v4(&addr)
                    .inspect_err(|e| {
                        tracing::warn!(address=%addr, "cannot send mDNS on this interface alone: {e}");
                    })
                    .is_ok();
                // Linux hands a socket the group's packets from every interface another
                // socket joined it on: with this off, it hears fewer of other links'. Not
                // none: Linux's early demultiplexing of UDP still gives one socket of those
                // bound to the port a multicast packet from any link, which is then handed
                // to the interface of its sender's network (see `HAND_OVER`).
                #[cfg(target_os = "linux")]
                if let Err(e) = socket.set_multicast_all_v4(false) {
                    tracing::debug!(address=%addr, "mDNS hears other links too: {e}");
                }
                (U::from_std(UdpSocket::from(socket))?, on_own_link)
            }
            IpAddr::V6(_) => {
                let socket = Socket::new(Domain::IPV6, Type::DGRAM, Some(socket2::Protocol::UDP))?;
                socket.set_reuse_address(true)?;
                #[cfg(unix)]
                socket.set_reuse_port(true)?;
                socket.bind(&SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 5353).into())?;
                socket.set_multicast_loop_v6(true)?;
                // TODO: find interface matching addr.
                socket.join_multicast_v6(&crate::IPV6_MDNS_MULTICAST_ADDRESS, 0)?;
                // No interface is chosen to send on.
                (U::from_std(UdpSocket::from(socket))?, false)
            }
        };
        let bind_addr = match addr {
            IpAddr::V4(_) => SocketAddr::new(addr, 0),
            IpAddr::V6(_addr) => {
                // TODO: if-watch should return the scope_id of an address
                // as a workaround we bind to unspecified, which means that
                // this probably won't work when using multiple interfaces.
                // SocketAddr::V6(SocketAddrV6::new(addr, 0, 0, scope_id))
                SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0)
            }
        };
        let send_socket = match addr {
            IpAddr::V4(_) => None,
            IpAddr::V6(_) => Some(U::from_std(UdpSocket::bind(bind_addr)?)?),
        };
        let mut state = Self::with_sockets(
            net,
            (recv_socket, send_socket),
            config,
            local_peer_id,
            listen_addresses,
            query_response_sender,
            interfaces,
        );
        state.sends_on_own_link = sends_on_own_link;
        Ok(state)
    }

    /// Builds a new [`InterfaceState`] on sockets already made: the one bound to port 5353,
    /// and over IPv6 the one it sends from.
    fn with_sockets(
        net: IpNet,
        (recv_socket, send_socket): (U, Option<U>),
        config: Config,
        local_peer_id: PeerId,
        listen_addresses: Arc<RwLock<ListenAddresses>>,
        query_response_sender: mpsc::Sender<Found>,
        interfaces: Interfaces,
    ) -> Self {
        let addr = net.addr();
        // randomize timer to prevent all converging and firing at the same time.
        let query_interval = {
            let jitter = rand::random_range(0..100);
            config.query_interval + Duration::from_millis(jitter)
        };
        let multicast_addr = match addr {
            IpAddr::V4(_) => IpAddr::V4(crate::IPV4_MDNS_MULTICAST_ADDRESS),
            IpAddr::V6(_) => IpAddr::V6(crate::IPV6_MDNS_MULTICAST_ADDRESS),
        };
        let handed = interfaces.add(net);
        Self {
            addr,
            net,
            recv_socket,
            interfaces,
            handed,
            handed_turn: false,
            sends_on_own_link: true,
            last_handed_answer: None,
            handed_answer_gap: HANDED_ANSWER_GAP,
            send_socket,
            listen_addresses,
            query_response_sender,
            recv_buffer: [0; 4096],
            send_buffer: Default::default(),
            discovered: Default::default(),
            query_interval,
            timeout: T::interval_at(Instant::now(), INITIAL_TIMEOUT_INTERVAL),
            multicast_addr,
            ttl: config.ttl,
            max_ttl: 2 * config.query_interval,
            asked: VecDeque::new(),
            service_name: config.service_name,
            probe_state: Default::default(),
            local_peer_id,
        }
    }

    pub(crate) fn reset_timer(&mut self) {
        tracing::trace!(address=%self.addr, probe_state=?self.probe_state, "reset timer");
        let interval = *self.probe_state.interval();
        self.timeout = T::interval(interval);
    }

    fn mdns_socket(&self) -> SocketAddr {
        SocketAddr::new(self.multicast_addr, 5353)
    }

    /// Whether a query another interface's socket read (`handed`) is answered now: only by
    /// an interface whose socket sends on its own link, and at most once every
    /// [`HANDED_ANSWER_GAP`]. Those its own socket read are all answered, as upstream. (A
    /// service browse query handed over is not answered at all, see `poll`.)
    fn answers_now(&mut self, handed: bool, now: Instant) -> bool {
        if !handed {
            return true;
        }
        if !self.sends_on_own_link
            || self
                .last_handed_answer
                .is_some_and(|at| now.saturating_duration_since(at) < self.handed_answer_gap)
        {
            tracing::debug!(address=%self.addr, "left a query another socket read unanswered");
            return false;
        }
        self.last_handed_answer = Some(now);
        true
    }

    /// Notes a query about to be sent.
    fn ask(&mut self, query: &[u8], now: Instant) {
        if self.asked.len() >= QUERIES_KEPT {
            self.asked.pop_front();
        }
        self.asked
            .push_back((u16::from_be_bytes([query[0], query[1]]), now));
    }

    /// Whether a response with `id` from `from` is taken: it answers one of this instance's
    /// own queries sent a moment ago, and comes from this interface's network (RFC 6762,
    /// section 11), not from anyone on the internet who can reach port 5353.
    fn takes(&self, id: u16, from: IpAddr, now: Instant) -> bool {
        answers(&self.asked, &self.net, id, from, now)
    }
}

impl<U, T> Drop for InterfaceState<U, T> {
    fn drop(&mut self) {
        self.interfaces.remove_own(&self.addr, &self.handed);
    }
}

/// Whether a response with `id` from `from` answers one of the queries `asked` on `net`.
fn answers(
    asked: &VecDeque<(u16, Instant)>,
    net: &IpNet,
    id: u16,
    from: IpAddr,
    now: Instant,
) -> bool {
    let asked = asked
        .iter()
        .any(|(asked, at)| *asked == id && now.saturating_duration_since(*at) < ANSWER_WINDOW);
    asked && (net.contains(&from) || is_link_local(from))
}

fn is_link_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_unicast_link_local(),
    }
}

impl<U, T> Future for InterfaceState<U, T>
where
    U: AsyncSocket,
    T: Builder + futures::Stream,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            // 1st priority: Low latency: Create packet ASAP after timeout.
            if this.timeout.poll_next_unpin(cx).is_ready() {
                let query = build_query(&this.service_name);
                tracing::debug!(
                    address=%this.addr,
                    id=u16::from_be_bytes([query[0], query[1]]),
                    "sending query on iface"
                );
                this.ask(&query, Instant::now());
                this.send_buffer.push_back(query);
                tracing::trace!(address=%this.addr, probe_state=?this.probe_state, "tick");

                // Stop to probe when the initial interval reach the query interval
                if let ProbeState::Probing(interval) = this.probe_state {
                    let interval = interval * 2;
                    this.probe_state = if interval >= this.query_interval {
                        ProbeState::Finished(this.query_interval)
                    } else {
                        ProbeState::Probing(interval)
                    };
                }

                this.reset_timer();
            }

            // 2nd priority: Keep local buffers small: Send packets to remote.
            if let Some(packet) = this.send_buffer.pop_front() {
                let to = this.mdns_socket();
                let socket = this.send_socket.as_mut().unwrap_or(&mut this.recv_socket);
                match socket.poll_write(cx, &packet, to) {
                    Poll::Ready(Ok(_)) => {
                        tracing::trace!(address=%this.addr, "sent packet on iface address");
                        continue;
                    }
                    Poll::Ready(Err(err)) => {
                        tracing::error!(address=%this.addr, "error sending packet on iface address {}", err);
                        continue;
                    }
                    Poll::Pending => {
                        this.send_buffer.push_front(packet);
                    }
                }
            }

            // 3rd priority: Keep local buffers small: Return discovered addresses.
            if this.query_response_sender.poll_ready_unpin(cx).is_ready()
                && let Some(discovered) = this.discovered.pop_front()
            {
                match this.query_response_sender.try_send(discovered) {
                    Ok(()) => {}
                    Err(e) if e.is_disconnected() => {
                        return Poll::Ready(());
                    }
                    Err(e) => {
                        this.discovered.push_front(e.into_inner());
                    }
                }

                continue;
            }

            // 4th priority: Remote work: Answer incoming requests: those this socket read,
            // and those the other interfaces' sockets read from this one's network (Fogata,
            // see `HAND_OVER`), in turns.
            let mut read = None;
            for from_handed in [this.handed_turn, !this.handed_turn] {
                if from_handed {
                    if let Poll::Ready(Some(packet)) = this.handed.poll_next_unpin(cx) {
                        read = Some(Read::Handed(packet));
                        break;
                    }
                    continue;
                }
                match this.recv_socket.poll_read(cx, &mut this.recv_buffer) {
                    Poll::Ready(Ok((len, from))) => {
                        let bytes = &this.recv_buffer[..len];
                        read = Some(match this.interfaces.hand(&this.net, bytes, from) {
                            Some(to) => {
                                tracing::debug!(
                                    address=%this.addr,
                                    remote_address=%from,
                                    interface=%to,
                                    "handed a packet to the interface of its sender's network"
                                );
                                Read::Skip
                            }
                            None => Read::Own(len, from),
                        });
                        break;
                    }
                    Poll::Ready(Err(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        // No more bytes available on the socket to read
                        read = Some(Read::Skip);
                        break;
                    }
                    Poll::Ready(Err(err)) => {
                        tracing::error!("failed reading datagram: {}", err);
                        return Poll::Ready(());
                    }
                    Poll::Pending => {}
                }
            }
            let Some(read) = read else {
                return Poll::Pending;
            };
            this.handed_turn = !this.handed_turn;
            let (packet, handed) = match read {
                Read::Own(len, from) => (
                    MdnsPacket::new_from_bytes(&this.recv_buffer[..len], from, &this.service_name),
                    false,
                ),
                Read::Handed((bytes, from)) => (
                    MdnsPacket::new_from_bytes(&bytes, from, &this.service_name),
                    true,
                ),
                Read::Skip => continue,
            };
            match packet {
                Ok(Some(MdnsPacket::Query(query))) => {
                    tracing::trace!(
                        address=%this.addr,
                        remote_address=%query.remote_addr(),
                        "received query from remote address on address"
                    );
                    if !this.answers_now(handed, Instant::now()) {
                        continue;
                    }

                    // Only send addresses that belong to this interface.
                    // This prevents advertising loopback or other interface addresses
                    // to peers that can't reach them.
                    let iface_ip = this.addr;
                    let read = this
                        .listen_addresses
                        .read()
                        .unwrap_or_else(|e| e.into_inner());
                    let relevant_addrs: Vec<&Multiaddr> = read
                        .iter()
                        .filter(|multiaddr| addr_matches_interface(multiaddr, iface_ip))
                        .collect();
                    tracing::debug!(
                        address=%this.addr,
                        remote_address=%query.remote_addr(),
                        id=query.query_id(),
                        addresses=relevant_addrs.len(),
                        "answering a query"
                    );

                    this.send_buffer.extend(build_query_response(
                        query.query_id(),
                        this.local_peer_id,
                        relevant_addrs,
                        this.ttl,
                        &this.service_name,
                    ));
                    continue;
                }
                Ok(Some(MdnsPacket::Response(response))) => {
                    tracing::trace!(
                        address=%this.addr,
                        remote_address=%response.remote_addr(),
                        "received response from remote address on address"
                    );

                    let now = Instant::now();
                    let from = response.remote_addr().ip();
                    // How long ago the query it answers went, if it answers one of these.
                    let age = this
                        .asked
                        .iter()
                        .find(|(id, _)| *id == response.query_id())
                        .map(|(_, at)| now.saturating_duration_since(*at));
                    if !this.takes(response.query_id(), from, now) {
                        tracing::debug!(
                            address=%this.addr,
                            remote_address=%response.remote_addr(),
                            id=response.query_id(),
                            ?age,
                            "ignored a response to no recent query of this device"
                        );
                        continue;
                    }
                    tracing::debug!(
                        address=%this.addr,
                        remote_address=%response.remote_addr(),
                        id=response.query_id(),
                        ?age,
                        "took a response"
                    );
                    let room = MAX_WAITING.saturating_sub(this.discovered.len());
                    this.discovered.extend(
                        response
                            .extract_discovered(now, this.local_peer_id, this.max_ttl)
                            .take(room)
                            .map(|(peer, address, expires)| (peer, address, expires, from)),
                    );

                    // Probing goes on to its end whatever answers: the first to answer may be
                    // any device on the network, one that started long ago, while the one
                    // being looked for, starting at the same time, was not answering yet
                    // or its answer was lost; the next query would be a whole interval away.
                    continue;
                }
                Ok(Some(MdnsPacket::ServiceDiscovery(disc))) => {
                    tracing::trace!(
                        address=%this.addr,
                        remote_address=%disc.remote_addr(),
                        "received service discovery from remote address on address"
                    );
                    // A browse for services that another interface's socket read is left
                    // unanswered (Fogata): any device's software sends them, and they would
                    // spend the turn of queries for this service. Those this socket read
                    // are answered, as upstream.
                    if handed {
                        continue;
                    }

                    this.send_buffer.push_back(build_service_discovery_response(
                        disc.query_id(),
                        this.ttl,
                        &this.service_name,
                    ));
                    continue;
                }
                Err(err) => {
                    tracing::debug!("Parsing mdns packet failed: {:?}", err);
                    continue;
                }
                Ok(None) => continue,
            }
        }
    }
}

/// Returns `true` if the first protocol component of `addr` is an IP address equal to
/// `iface_ip`.
///
/// Used when answering mDNS queries to advertise only the listening addresses that are
/// reachable on the interface the query arrived on, instead of every listening address.
fn addr_matches_interface(addr: &Multiaddr, iface_ip: IpAddr) -> bool {
    match addr.iter().next() {
        Some(Protocol::Ip4(v4)) => IpAddr::V4(v4) == iface_ip,
        Some(Protocol::Ip6(v6)) => IpAddr::V6(v6) == iface_ip,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_responses_to_a_recent_own_query_from_the_network_are_taken() {
        let net: IpNet = "192.168.1.20/24".parse().unwrap();
        let now = Instant::now();
        let asked = VecDeque::from([(0x1234, now)]);
        let neighbour: IpAddr = [192, 168, 1, 30].into();
        assert!(answers(&asked, &net, 0x1234, neighbour, now));
        // Another id: an answer to someone else's query, or to none.
        assert!(!answers(&asked, &net, 0x4321, neighbour, now));
        // Late, as macOS's firewall may hold it while it checks a new sender: still taken.
        assert!(answers(
            &asked,
            &net,
            0x1234,
            neighbour,
            now + Duration::from_secs(10)
        ));
        // Too late.
        assert!(!answers(
            &asked,
            &net,
            0x1234,
            neighbour,
            now + ANSWER_WINDOW
        ));
        // From off this network: anyone on the internet who reaches port 5353.
        assert!(!answers(&asked, &net, 0x1234, [203, 0, 113, 9].into(), now));
        assert!(answers(&asked, &net, 0x1234, [169, 254, 3, 4].into(), now));
    }

    /// What a socket reads and what it sends, without the network.
    #[cfg(feature = "tokio")]
    #[derive(Clone, Default)]
    struct Wire {
        read: Arc<Mutex<VecDeque<Packet>>>,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    #[cfg(feature = "tokio")]
    struct WireSocket(Wire);

    #[cfg(feature = "tokio")]
    impl AsyncSocket for WireSocket {
        fn from_std(_: UdpSocket) -> io::Result<Self> {
            Err(io::Error::other("no socket on the network here"))
        }

        fn poll_read(
            &mut self,
            _: &mut Context,
            buf: &mut [u8],
        ) -> Poll<Result<(usize, SocketAddr), io::Error>> {
            match self.0.read.lock().unwrap().pop_front() {
                Some((bytes, from)) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Poll::Ready(Ok((bytes.len(), from)))
                }
                None => Poll::Pending,
            }
        }

        fn poll_write(
            &mut self,
            _: &mut Context,
            packet: &[u8],
            _: SocketAddr,
        ) -> Poll<Result<(), io::Error>> {
            self.0.sent.lock().unwrap().push(packet.to_vec());
            Poll::Ready(Ok(()))
        }
    }

    /// A device on two networks, each on an interface with a socket on port 5353 (no network
    /// under them), and the id of each one's first query, the two apart.
    #[cfg(feature = "tokio")]
    struct TwoNetworks {
        public: InterfaceState<WireSocket, crate::behaviour::timer::tokio::TokioTimer>,
        public_wire: Wire,
        public_id: u16,
        private: InterfaceState<WireSocket, crate::behaviour::timer::tokio::TokioTimer>,
        private_wire: Wire,
        private_id: u16,
        told: mpsc::Receiver<Found>,
        service: String,
    }

    #[cfg(feature = "tokio")]
    impl TwoNetworks {
        async fn new(hand_over: bool, public: &str, private: &str) -> Self {
            use libp2p_core::transport::ListenerId;
            use libp2p_swarm::behaviour::{FromSwarm, NewListenAddr};

            let config = Config::default();
            let listen = Arc::new(RwLock::new(ListenAddresses::default()));
            for net in [public, private] {
                let net: IpNet = net.parse().unwrap();
                let address: Multiaddr = format!("/ip4/{}/udp/41000/quic-v1", net.addr())
                    .parse()
                    .unwrap();
                listen
                    .write()
                    .unwrap()
                    .on_swarm_event(&FromSwarm::NewListenAddr(NewListenAddr {
                        listener_id: ListenerId::next(),
                        addr: &address,
                    }));
            }
            let query_id = |wire: &Wire| {
                let sent = wire.sent.lock().unwrap();
                u16::from_be_bytes([sent[0][0], sent[0][1]])
            };
            loop {
                let interfaces = Interfaces::new(hand_over);
                let (found, told) = mpsc::channel(10);
                let interface = |net: &str| {
                    let wire = Wire::default();
                    let state = InterfaceState::with_sockets(
                        net.parse().unwrap(),
                        (WireSocket(wire.clone()), None),
                        config.clone(),
                        PeerId::random(),
                        listen.clone(),
                        found.clone(),
                        interfaces.clone(),
                    );
                    (state, wire)
                };
                let (public, public_wire) = interface(public);
                let (private, private_wire) = interface(private);
                let mut two = TwoNetworks {
                    public,
                    public_wire,
                    public_id: 0,
                    private,
                    private_wire,
                    private_id: 0,
                    told,
                    service: config.service_name.clone(),
                };
                // Each sends its first query, due at once (once tokio's clock has moved).
                ::tokio::time::sleep(Duration::from_millis(10)).await;
                two.poll();
                two.public_id = query_id(&two.public_wire);
                two.private_id = query_id(&two.private_wire);
                // The same id by chance (once in 65 536): again.
                if two.public_id != two.private_id {
                    return two;
                }
            }
        }

        fn poll(&mut self) {
            self.poll_public();
            self.poll_private();
        }

        fn poll_public(&mut self) {
            let mut cx = Context::from_waker(futures::task::noop_waker_ref());
            assert!(Pin::new(&mut self.public).poll(&mut cx).is_pending());
        }

        fn poll_private(&mut self) {
            let mut cx = Context::from_waker(futures::task::noop_waker_ref());
            assert!(Pin::new(&mut self.private).poll(&mut cx).is_pending());
        }

        /// What `wire`'s socket reads next, from `from`.
        fn read(wire: &Wire, packet: Vec<u8>, from: &str) {
            let from: SocketAddr = format!("{from}:5353").parse().unwrap();
            wire.read.lock().unwrap().push_back((packet, from));
        }

        fn to_public(&self, packet: Vec<u8>, from: &str) {
            Self::read(&self.public_wire, packet, from);
        }

        fn to_private(&self, packet: Vec<u8>, from: &str) {
            Self::read(&self.private_wire, packet, from);
        }

        /// The answer of `peer`, at `at`, to the query `id`.
        fn answer(&self, id: u16, peer: PeerId, at: &str) -> Vec<u8> {
            let at: Multiaddr = format!("/ip4/{at}/udp/41000/quic-v1").parse().unwrap();
            build_query_response(id, peer, vec![&at], Duration::from_secs(60), &self.service)
                .remove(0)
        }

        /// The answers `wire`'s socket sent.
        fn answers_sent(&self, wire: &Wire) -> Vec<Vec<u8>> {
            let from: SocketAddr = "192.0.2.1:5353".parse().unwrap();
            wire.sent
                .lock()
                .unwrap()
                .iter()
                .filter(|p| {
                    matches!(
                        MdnsPacket::new_from_bytes(p, from, &self.service),
                        Ok(Some(MdnsPacket::Response(_)))
                    )
                })
                .cloned()
                .collect()
        }

        /// Who was told of, and by whom, since last asked.
        fn told(&mut self) -> Vec<(PeerId, IpAddr)> {
            let mut told = Vec::new();
            while let Ok((peer, _, _, by)) = self.told.try_recv() {
                told.push((peer, by));
            }
            told
        }
    }

    #[cfg(feature = "tokio")]
    fn contains(packet: &[u8], text: &[u8]) -> bool {
        packet.windows(text.len()).any(|w| w == text)
    }

    /// A device on a public network and on a private one of its own, and C on the private
    /// one. Linux gives C's packets to the public interface's socket as often as not (see
    /// `HAND_OVER`): C's answer to the private interface's query is still taken, and C's
    /// query is answered by the private interface, with its address, on its link. An answer
    /// from C to a query only the public interface sent, or to none, is still not taken;
    /// one from a link-local sender, or from one on no network of the device, stays with the
    /// socket that read it.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn a_packet_read_by_another_interface_s_socket_is_taken_as_on_its_own() {
        let mut two = TwoNetworks::new(true, "203.0.113.21/24", "192.168.60.1/24").await;
        let c = PeerId::random();
        let (public_id, private_id) = (two.public_id, two.private_id);

        // To a query only the public interface sent, and to none: not taken.
        let mut none = private_id.wrapping_add(1);
        while none == public_id {
            none = none.wrapping_add(1);
        }
        two.to_public(two.answer(public_id, c, "192.168.60.2"), "192.168.60.2");
        two.to_public(two.answer(none, c, "192.168.60.2"), "192.168.60.2");
        two.poll();
        assert_eq!(
            two.told(),
            vec![],
            "an answer to no query of its network was taken"
        );

        // To the private interface's query.
        two.to_public(two.answer(private_id, c, "192.168.60.2"), "192.168.60.2");
        two.poll();
        assert_eq!(two.told(), vec![(c, [192, 168, 60, 2].into())]);

        // From off the device's networks: stays with the public socket, which refuses it.
        two.to_public(two.answer(private_id, c, "198.51.100.7"), "198.51.100.7");
        // From a link-local address: stays with the public socket, which takes it.
        let near = PeerId::random();
        two.to_public(two.answer(public_id, near, "169.254.7.7"), "169.254.7.7");
        two.poll_public();
        assert!(two.private.handed.try_recv().is_err(), "handed over");
        assert_eq!(two.told(), vec![(near, [169, 254, 7, 7].into())]);

        // A browse for services from C is not answered, nor spends the turn of the query
        // that follows. C's query: answered on the private link with the private address
        // alone; another right after it is not (see `HANDED_ANSWER_GAP`; the gap made long
        // here, so that a slow computer cannot pass it).
        let browse = || build_query(std::str::from_utf8(crate::META_QUERY_SERVICE).unwrap());
        two.private.handed_answer_gap = Duration::from_secs(3600);
        two.to_public(browse(), "192.168.60.2");
        two.poll();
        two.to_public(build_query(&two.service), "192.168.60.2");
        two.poll();
        two.to_public(build_query(&two.service), "192.168.60.2");
        two.poll();
        assert!(two.answers_sent(&two.public_wire).is_empty());
        let answered = two.answers_sent(&two.private_wire);
        assert_eq!(answered.len(), 1);
        assert!(contains(&answered[0], b"/ip4/192.168.60.1/"));
        assert!(!contains(&answered[0], b"/ip4/203.0.113.21/"));

        // An interface that could not choose its link to send on answers none of them.
        two.private.last_handed_answer = None;
        two.private.sends_on_own_link = false;
        two.to_public(build_query(&two.service), "192.168.60.2");
        two.poll();
        assert_eq!(two.answers_sent(&two.private_wire).len(), 1);
        assert!(two.answers_sent(&two.public_wire).is_empty());

        // A query from the device's own private address (its own, looped back, or that of a
        // copy on this computer) stays with the socket that read it, which answers it as
        // upstream did. A browse an interface's own socket read is answered, as upstream.
        two.private.sends_on_own_link = true;
        two.private.handed_answer_gap = Duration::ZERO;
        two.to_public(build_query(&two.service), "192.168.60.1");
        two.poll_public();
        assert!(two.private.handed.try_recv().is_err(), "handed over");
        assert_eq!(two.answers_sent(&two.public_wire).len(), 1);
        two.to_private(browse(), "192.168.60.2");
        two.poll_private();
        assert_eq!(two.answers_sent(&two.private_wire).len(), 2);
    }

    /// A query handed over is answered at most once every `HANDED_ANSWER_GAP`; those an
    /// interface's own socket read, always.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn queries_handed_over_are_answered_at_most_once_a_gap() {
        let mut two = TwoNetworks::new(true, "203.0.113.21/24", "192.168.60.1/24").await;
        let now = Instant::now();
        let private = &mut two.private;
        assert!(private.answers_now(true, now));
        assert!(!private.answers_now(true, now + HANDED_ANSWER_GAP / 2));
        assert!(private.answers_now(false, now + HANDED_ANSWER_GAP / 2));
        assert!(private.answers_now(true, now + HANDED_ANSWER_GAP));
    }

    /// A task takes its own socket's packets and those handed to it in turns.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn a_task_takes_its_socket_s_packets_and_those_handed_to_it_in_turns() {
        let mut two = TwoNetworks::new(true, "203.0.113.21/24", "192.168.60.1/24").await;
        for _ in 0..4 {
            let own = two.answer(two.private_id, PeerId::random(), "192.168.60.3");
            two.to_private(own, "192.168.60.3");
            let handed = two.answer(two.private_id, PeerId::random(), "192.168.60.2");
            two.to_public(handed, "192.168.60.2");
        }
        two.poll_public();
        two.poll_private();
        let by: Vec<IpAddr> = two.told().into_iter().map(|(_, by)| by).collect();
        assert_eq!(by.len(), 8);
        assert!(by.windows(2).all(|w| w[0] != w[1]), "{by:?}");
    }

    /// Without the hand-over (every system but Linux and Android), a packet stays with the
    /// socket that read it, as upstream.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn without_the_hand_over_a_packet_stays_with_the_socket_that_read_it() {
        let mut two = TwoNetworks::new(false, "203.0.113.21/24", "192.168.60.1/24").await;
        let c = PeerId::random();
        two.to_public(
            two.answer(two.private_id, c, "192.168.60.2"),
            "192.168.60.2",
        );
        two.to_public(build_query(&two.service), "192.168.60.2");
        two.poll_public();
        assert!(two.private.handed.try_recv().is_err(), "handed over");
        two.poll();
        assert_eq!(two.told(), vec![]);
        assert!(two.answers_sent(&two.private_wire).is_empty());
        assert_eq!(two.answers_sent(&two.public_wire).len(), 1);
    }

    /// At most `MAX_HANDED` packets wait for an interface; those past it are dropped.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn packets_past_what_may_wait_for_an_interface_are_dropped() {
        let mut two = TwoNetworks::new(true, "203.0.113.21/24", "192.168.60.1/24").await;
        for _ in 0..MAX_HANDED + 4 {
            let answer = two.answer(two.private_id, PeerId::random(), "192.168.60.2");
            two.to_public(answer, "192.168.60.2");
        }
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());
        assert!(Pin::new(&mut two.public).poll(&mut cx).is_pending());
        let mut waiting = 0;
        while two.private.handed.try_recv().is_ok() {
            waiting += 1;
        }
        assert_eq!(waiting, MAX_HANDED);
    }

    /// A packet goes to the interface whose network is the narrowest that holds its sender,
    /// the one whose socket read it included: a local network inside a VPN's.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn a_packet_goes_to_the_narrowest_network_of_its_sender() {
        let mut two = TwoNetworks::new(true, "10.0.0.5/8", "10.1.0.1/16").await;
        let (c, d) = (PeerId::random(), PeerId::random());
        // C, on the /16, read by the /8's socket.
        two.to_public(two.answer(two.private_id, c, "10.1.0.2"), "10.1.0.2");
        // D, on the /8 alone, read by the /16's socket.
        two.to_private(two.answer(two.public_id, d, "10.2.0.9"), "10.2.0.9");
        two.poll();
        two.poll();
        let mut told = two.told();
        told.sort();
        let mut want = vec![(c, [10, 1, 0, 2].into()), (d, [10, 2, 0, 9].into())];
        want.sort();
        assert_eq!(told, want);
    }

    /// A packet goes to the narrowest network that holds its sender (the one whose socket
    /// read it included), the lowest address of a tie; never one of the device's own
    /// addresses, nor to an interface whose task ended.
    #[test]
    fn the_narrowest_network_of_the_sender_takes_a_packet() {
        let interfaces = Interfaces::new(true);
        let public: IpNet = "203.0.113.21/24".parse().unwrap();
        let lan: IpNet = "10.1.0.1/16".parse().unwrap();
        let _public = interfaces.add(public);
        let _lan = interfaces.add(lan);
        let _vpn_2 = interfaces.add("10.0.0.2/8".parse().unwrap());
        let vpn_1 = interfaces.add("10.0.0.1/8".parse().unwrap());
        let hand = |own: &IpNet, from: &str| {
            interfaces.hand(own, b"x", SocketAddr::new(from.parse().unwrap(), 5353))
        };
        let ip = |ip: &str| Some(ip.parse::<IpAddr>().unwrap());
        assert_eq!(hand(&public, "10.5.5.5"), ip("10.0.0.1"));
        assert_eq!(hand(&public, "10.1.0.9"), ip("10.1.0.1"));
        assert_eq!(hand(&lan, "10.1.0.9"), None);
        assert_eq!(hand(&lan, "10.7.0.9"), ip("10.0.0.1"));
        assert_eq!(hand(&public, "10.1.0.1"), None);
        assert_eq!(hand(&public, "10.0.0.2"), None);
        drop(vpn_1);
        assert_eq!(hand(&public, "10.5.5.5"), ip("10.0.0.2"));
    }

    /// An interface's task that ends takes its place out, and only its own: one made since
    /// for the same address (its interface down and up again) keeps its place.
    #[cfg(feature = "tokio")]
    #[::tokio::test]
    async fn an_interface_whose_task_ended_is_handed_nothing() {
        let interfaces = Interfaces::new(true);
        let bare = |net: &str| {
            let (found, _) = mpsc::channel(1);
            InterfaceState::<WireSocket, crate::behaviour::timer::tokio::TokioTimer>::with_sockets(
                net.parse().unwrap(),
                (WireSocket(Wire::default()), None),
                Config::default(),
                PeerId::random(),
                Default::default(),
                found,
                interfaces.clone(),
            )
        };
        let public: IpNet = "203.0.113.21/24".parse().unwrap();
        let _public = bare("203.0.113.21/24");
        let from: SocketAddr = "192.168.60.2:5353".parse().unwrap();
        let private: IpAddr = [192, 168, 60, 1].into();

        let old = bare("192.168.60.1/24");
        assert_eq!(interfaces.hand(&public, b"x", from), Some(private));
        drop(old);
        assert_eq!(interfaces.hand(&public, b"x", from), None);

        let old = bare("192.168.60.1/24");
        let mut new = bare("192.168.60.1/24");
        drop(old);
        assert_eq!(interfaces.hand(&public, b"x", from), Some(private));
        assert!(new.handed.try_recv().is_ok());
    }
}
