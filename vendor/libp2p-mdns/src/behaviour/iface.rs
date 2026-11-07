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
    collections::VecDeque,
    future::Future,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    pin::Pin,
    sync::{Arc, RwLock},
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
    ) -> io::Result<Self> {
        let addr = net.addr();
        tracing::info!(address=%addr, "creating instance on iface address");
        let recv_socket = match addr {
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
                if let Err(e) = socket.set_multicast_if_v4(&addr) {
                    tracing::warn!(address=%addr, "cannot send mDNS on this interface alone: {e}");
                }
                // Linux hands a socket the group's packets from every interface another
                // socket joined it on: this one hears only its own link, so what it takes
                // and answers is of the network it sends on.
                #[cfg(target_os = "linux")]
                if let Err(e) = socket.set_multicast_all_v4(false) {
                    tracing::debug!(address=%addr, "mDNS hears other links too: {e}");
                }
                U::from_std(UdpSocket::from(socket))?
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
                U::from_std(UdpSocket::from(socket))?
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

        // randomize timer to prevent all converging and firing at the same time.
        let query_interval = {
            let jitter = rand::random_range(0..100);
            config.query_interval + Duration::from_millis(jitter)
        };
        let multicast_addr = match addr {
            IpAddr::V4(_) => IpAddr::V4(crate::IPV4_MDNS_MULTICAST_ADDRESS),
            IpAddr::V6(_) => IpAddr::V6(crate::IPV6_MDNS_MULTICAST_ADDRESS),
        };
        Ok(Self {
            addr,
            net,
            recv_socket,
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
            probe_state: Default::default(),
            local_peer_id,
        })
    }

    pub(crate) fn reset_timer(&mut self) {
        tracing::trace!(address=%self.addr, probe_state=?self.probe_state, "reset timer");
        let interval = *self.probe_state.interval();
        self.timeout = T::interval(interval);
    }

    fn mdns_socket(&self) -> SocketAddr {
        SocketAddr::new(self.multicast_addr, 5353)
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
                let query = build_query();
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

            // 4th priority: Remote work: Answer incoming requests.
            match this
                .recv_socket
                .poll_read(cx, &mut this.recv_buffer)
                .map_ok(|(len, from)| MdnsPacket::new_from_bytes(&this.recv_buffer[..len], from))
            {
                Poll::Ready(Ok(Ok(Some(MdnsPacket::Query(query))))) => {
                    tracing::trace!(
                        address=%this.addr,
                        remote_address=%query.remote_addr(),
                        "received query from remote address on address"
                    );

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
                    ));
                    continue;
                }
                Poll::Ready(Ok(Ok(Some(MdnsPacket::Response(response))))) => {
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
                Poll::Ready(Ok(Ok(Some(MdnsPacket::ServiceDiscovery(disc))))) => {
                    tracing::trace!(
                        address=%this.addr,
                        remote_address=%disc.remote_addr(),
                        "received service discovery from remote address on address"
                    );

                    this.send_buffer
                        .push_back(build_service_discovery_response(disc.query_id(), this.ttl));
                    continue;
                }
                Poll::Ready(Err(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more bytes available on the socket to read
                    continue;
                }
                Poll::Ready(Err(err)) => {
                    tracing::error!("failed reading datagram: {}", err);
                    return Poll::Ready(());
                }
                Poll::Ready(Ok(Err(err))) => {
                    tracing::debug!("Parsing mdns packet failed: {:?}", err);
                    continue;
                }
                Poll::Ready(Ok(Ok(None))) => continue,
                Poll::Pending => {}
            }

            return Poll::Pending;
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
}
