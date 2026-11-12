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

mod found;
mod iface;
mod socket;
mod timer;

use std::{
    collections::{
        VecDeque,
        hash_map::{Entry, HashMap},
    },
    convert::Infallible,
    fmt,
    future::Future,
    io,
    net::IpAddr,
    pin::Pin,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::Instant,
};

use self::{
    found::Discovered,
    iface::{Found, InterfaceState},
};
use crate::{
    Config,
    behaviour::{socket::AsyncSocket, timer::Builder},
};
use futures::{Stream, StreamExt, channel::mpsc};
use if_watch::IfEvent;
use libp2p_core::{Endpoint, Multiaddr, transport::PortUse};
use libp2p_identity::PeerId;
use libp2p_swarm::{
    ConnectionDenied, ConnectionId, ListenAddresses, NetworkBehaviour, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm, behaviour::FromSwarm, dummy,
};

/// An abstraction to allow for compatibility with various async runtimes.
pub trait Provider: 'static {
    /// The Async Socket type.
    type Socket: AsyncSocket;
    /// The Async Timer type.
    type Timer: Builder + Stream;
    /// The IfWatcher type.
    type Watcher: Stream<Item = std::io::Result<IfEvent>> + fmt::Debug + Unpin;

    type TaskHandle: Abort;

    /// Create a new instance of the `IfWatcher` type.
    fn new_watcher() -> Result<Self::Watcher, std::io::Error>;

    #[track_caller]
    fn spawn(task: impl Future<Output = ()> + Send + 'static) -> Self::TaskHandle;
}

#[allow(unreachable_pub)] // Not re-exported.
pub trait Abort {
    fn abort(self);
}

/// The type of a [`Behaviour`] using the `tokio` implementation.
#[cfg(feature = "tokio")]
pub mod tokio {
    use std::future::Future;

    use if_watch::tokio::IfWatcher;
    use tokio::task::JoinHandle;

    use super::Provider;
    use crate::behaviour::{Abort, socket::tokio::TokioUdpSocket, timer::tokio::TokioTimer};

    #[doc(hidden)]
    pub enum Tokio {}

    impl Provider for Tokio {
        type Socket = TokioUdpSocket;
        type Timer = TokioTimer;
        type Watcher = IfWatcher;
        type TaskHandle = JoinHandle<()>;

        fn new_watcher() -> Result<Self::Watcher, std::io::Error> {
            IfWatcher::new()
        }

        fn spawn(task: impl Future<Output = ()> + Send + 'static) -> Self::TaskHandle {
            tokio::spawn(task)
        }
    }

    impl Abort for JoinHandle<()> {
        fn abort(self) {
            JoinHandle::abort(&self)
        }
    }

    pub type Behaviour = super::Behaviour<Tokio>;
}

/// A `NetworkBehaviour` for mDNS. Automatically discovers peers on the local network and adds
/// them to the topology.
#[derive(Debug)]
pub struct Behaviour<P>
where
    P: Provider,
{
    /// InterfaceState config.
    config: Config,

    /// Iface watcher.
    if_watch: P::Watcher,

    /// Handles to tasks running the mDNS queries.
    if_tasks: HashMap<IpAddr, P::TaskHandle>,

    query_response_receiver: mpsc::Receiver<Found>,
    query_response_sender: mpsc::Sender<Found>,

    /// Nodes that we have discovered, their addresses, and when their TTL expires: a
    /// bounded number (see found.rs), since anyone on the network can name any.
    discovered_nodes: Discovered,

    /// Future that fires when the TTL of at least one node in `discovered_nodes` expires.
    ///
    /// `None` if `discovered_nodes` is empty.
    closest_expiration: Option<P::Timer>,

    /// The current set of listen addresses.
    ///
    /// This is shared across all interface tasks using an [`RwLock`].
    /// The [`Behaviour`] updates this upon new [`FromSwarm`]
    /// events where as [`InterfaceState`]s read from it to answer inbound mDNS queries.
    listen_addresses: Arc<RwLock<ListenAddresses>>,

    local_peer_id: PeerId,

    /// Pending behaviour events to be emitted.
    pending_events: VecDeque<ToSwarm<Event, Infallible>>,
}

impl<P> Behaviour<P>
where
    P: Provider,
{
    /// Builds a new `Mdns` behaviour.
    pub fn new(config: Config, local_peer_id: PeerId) -> io::Result<Self> {
        // Fogata: a name its packets can carry (see `Config::service_name`).
        let name = &config.service_name;
        if !name.is_ascii()
            || name.len() > 255
            || name
                .split('.')
                .any(|label| label.is_empty() || label.len() > 63)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a DNS-SD service name: {name:?}"),
            ));
        }
        let (tx, rx) = mpsc::channel(10); // Chosen arbitrarily.

        Ok(Self {
            config,
            if_watch: P::new_watcher()?,
            if_tasks: Default::default(),
            query_response_receiver: rx,
            query_response_sender: tx,
            discovered_nodes: Default::default(),
            closest_expiration: Default::default(),
            listen_addresses: Default::default(),
            local_peer_id,
            pending_events: Default::default(),
        })
    }

    /// Returns true if the given `PeerId` is in the list of nodes discovered through mDNS.
    #[deprecated(note = "Use `discovered_nodes` iterator instead.")]
    pub fn has_node(&self, peer_id: &PeerId) -> bool {
        self.discovered_nodes().any(|p| p == peer_id)
    }

    /// Returns the list of nodes that we have discovered through mDNS and that are not expired.
    pub fn discovered_nodes(&self) -> impl ExactSizeIterator<Item = &PeerId> {
        self.discovered_nodes.peers()
    }

    /// Fogata: the address the response that first told of `peer` came from, while it is
    /// known: what to count it by, since the addresses a response names are whatever its
    /// sender likes.
    pub fn source_of(&self, peer_id: &PeerId) -> Option<IpAddr> {
        self.discovered_nodes.source(peer_id)
    }

    /// Fogata: the address the response that told of `address` of `peer_id` came from,
    /// while it is known: what to count a dial at that address by.
    pub fn told_by(&self, peer_id: &PeerId, address: &Multiaddr) -> Option<IpAddr> {
        self.discovered_nodes.told_by(peer_id, address)
    }

    /// Expires a node before the ttl.
    #[deprecated(note = "Unused API. Will be removed in the next release.")]
    pub fn expire_node(&mut self, peer_id: &PeerId) {
        let now = Instant::now();
        self.discovered_nodes.expire_peer(peer_id, now);
        self.closest_expiration = Some(P::Timer::at(now));
    }
}

/// The interfaces' tasks stop with the behaviour (Fogata). Upstream left them running once
/// the node that made them was gone: each went on answering every query on the network
/// with that node's addresses, dead by then, until its own next query (up to a query
/// interval later), and every device there dialled them for nothing.
impl<P> Drop for Behaviour<P>
where
    P: Provider,
{
    fn drop(&mut self) {
        for (_, task) in self.if_tasks.drain() {
            task.abort();
        }
    }
}

impl<P> NetworkBehaviour for Behaviour<P>
where
    P: Provider,
{
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = Event;

    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        _: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        _addresses: &[Multiaddr],
        _effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        let Some(peer_id) = maybe_peer else {
            return Ok(vec![]);
        };

        Ok(self.discovered_nodes.addresses(&peer_id))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        _: PeerId,
        _: &Multiaddr,
        _: Endpoint,
        _: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn on_connection_handler_event(
        &mut self,
        _: PeerId,
        _: ConnectionId,
        ev: THandlerOutEvent<Self>,
    ) {
        libp2p_core::util::unreachable(ev)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Fogata: an address a connection was made at is kept (see `found.rs`).
        if let FromSwarm::ConnectionEstablished(established) = &event
            && let libp2p_core::ConnectedPoint::Dialer { address, .. } = established.endpoint
        {
            self.discovered_nodes.reached(&established.peer_id, address);
        }
        self.listen_addresses
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .on_swarm_event(&event);
    }

    #[tracing::instrument(level = "trace", name = "NetworkBehaviour::poll", skip(self, cx))]
    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        loop {
            // Check for pending events and emit them.
            if let Some(event) = self.pending_events.pop_front() {
                return Poll::Ready(event);
            }

            // Poll ifwatch.
            while let Poll::Ready(Some(event)) = Pin::new(&mut self.if_watch).poll_next(cx) {
                match event {
                    Ok(IfEvent::Up(inet)) => {
                        let addr = inet.addr();
                        if addr.is_loopback() {
                            continue;
                        }
                        if addr.is_ipv4() && self.config.enable_ipv6
                            || addr.is_ipv6() && !self.config.enable_ipv6
                        {
                            continue;
                        }
                        if let Entry::Vacant(e) = self.if_tasks.entry(addr) {
                            match InterfaceState::<P::Socket, P::Timer>::new(
                                inet,
                                self.config.clone(),
                                self.local_peer_id,
                                self.listen_addresses.clone(),
                                self.query_response_sender.clone(),
                            ) {
                                Ok(iface_state) => {
                                    e.insert(P::spawn(iface_state));
                                }
                                Err(err) => {
                                    tracing::error!("failed to create `InterfaceState`: {}", err)
                                }
                            }
                        }
                    }
                    Ok(IfEvent::Down(inet)) => {
                        if let Some(handle) = self.if_tasks.remove(&inet.addr()) {
                            tracing::info!(instance=%inet.addr(), "dropping instance");

                            handle.abort();
                        }
                    }
                    Err(err) => tracing::error!("if watch returned an error: {}", err),
                }
            }
            // Emit discovered event.
            let mut discovered = Vec::new();
            // Those that made room for others are told of as expired.
            let mut expired = Vec::new();

            // Fogata: a few at a time, each lot handed on before the next is taken (see
            // `found::MAX_PEERS_PER_SOURCE`).
            let mut taken = 0;
            while taken < found::MAX_PEERS_PER_SOURCE
                && let Poll::Ready(Some((peer, addr, expiration, from))) =
                    self.query_response_receiver.poll_next_unpin(cx)
            {
                taken += 1;
                let (new, dropped) =
                    self.discovered_nodes
                        .insert(peer, addr.clone(), expiration, from);
                expired.extend(dropped);
                if new {
                    tracing::info!(%peer, address=%addr, "discovered peer on address");
                    discovered.push((peer, addr.clone()));

                    self.pending_events
                        .push_back(ToSwarm::NewExternalAddrOfPeer {
                            peer_id: peer,
                            address: addr,
                        });
                }
            }
            if !expired.is_empty() {
                self.pending_events
                    .push_back(ToSwarm::GenerateEvent(Event::Expired(expired)));
                expired = Vec::new();
            }

            if !discovered.is_empty() {
                let event = Event::Discovered(discovered);
                // Push to the front of the queue so that the behavior event is reported before
                // the individual discovered addresses.
                self.pending_events
                    .push_front(ToSwarm::GenerateEvent(event));
                continue;
            }
            if !self.pending_events.is_empty() || taken == found::MAX_PEERS_PER_SOURCE {
                continue;
            }
            // Emit expired event.
            let now = Instant::now();
            for (peer, addr) in self.discovered_nodes.expire(now) {
                tracing::info!(%peer, address=%addr, "expired peer on address");
                expired.push((peer, addr));
            }
            let closest_expiration = self.discovered_nodes.next_expiry();
            if !expired.is_empty() {
                let event = Event::Expired(expired);
                self.pending_events.push_back(ToSwarm::GenerateEvent(event));
                continue;
            }
            if let Some(closest_expiration) = closest_expiration {
                let mut timer = P::Timer::at(closest_expiration);
                let _ = Pin::new(&mut timer).poll_next(cx);

                self.closest_expiration = Some(timer);
            }

            return Poll::Pending;
        }
    }
}

/// Event that can be produced by the `Mdns` behaviour.
#[derive(Debug, Clone)]
pub enum Event {
    /// Discovered nodes through mDNS.
    Discovered(Vec<(PeerId, Multiaddr)>),

    /// The given combinations of `PeerId` and `Multiaddr` have expired.
    ///
    /// Each discovered record has a time-to-live. When this TTL expires and the address hasn't
    /// been refreshed, we remove it from the list and emit it as an `Expired` event.
    Expired(Vec<(PeerId, Multiaddr)>),
}

#[cfg(all(test, feature = "tokio"))]
mod tests {
    use std::{
        future::Future,
        io,
        net::IpAddr,
        task::{Context, Poll},
        time::{Duration, Instant},
    };

    use if_watch::IfEvent;
    use libp2p_core::Multiaddr;
    use libp2p_identity::PeerId;
    use libp2p_swarm::{NetworkBehaviour, ToSwarm};

    use super::{Behaviour, Event, Provider, tokio::Behaviour as TokioMdns};
    use crate::Config;

    /// Tokio, with no interface ever coming up: what the behaviour does with what its
    /// interfaces told, without a socket on the network.
    enum NoInterfaces {}

    impl Provider for NoInterfaces {
        type Socket = crate::behaviour::socket::tokio::TokioUdpSocket;
        type Timer = crate::behaviour::timer::tokio::TokioTimer;
        type Watcher = futures::stream::Pending<io::Result<IfEvent>>;
        type TaskHandle = ::tokio::task::JoinHandle<()>;

        fn new_watcher() -> io::Result<Self::Watcher> {
            Ok(futures::stream::pending())
        }

        fn spawn(task: impl Future<Output = ()> + Send + 'static) -> Self::TaskHandle {
            ::tokio::spawn(task)
        }
    }

    /// Copies on one computer, more than its share, answer every query at once. One that
    /// starts and answers first is told of, and is still known when that is handed on, so
    /// that the node can ask who told of it (`Behaviour::told_by`), though the answers that
    /// came right after it push it out (see `found::MAX_PEERS_PER_SOURCE`).
    #[::tokio::test]
    async fn a_peer_told_of_is_known_when_that_is_handed_on() {
        let mut behaviour =
            Behaviour::<NoInterfaces>::new(Config::default(), PeerId::random()).expect("mdns");
        let computer = IpAddr::from([192, 168, 1, 20]);
        let address = |i: usize, peer: PeerId| -> Multiaddr {
            format!("/ip4/192.168.1.20/udp/{}/quic-v1/p2p/{peer}", 4000 + i)
                .parse()
                .unwrap()
        };
        let copies: Vec<(PeerId, Multiaddr)> = (0..20)
            .map(|i| {
                let peer = PeerId::random();
                (peer, address(i, peer))
            })
            .collect();
        let newcomer = PeerId::random();
        let newcomer_at = address(99, newcomer);
        // What the interfaces told of in the meantime, all at once (each answer read at its
        // own moment).
        let tell = |behaviour: &mut Behaviour<NoInterfaces>, told: &[(PeerId, Multiaddr)]| {
            let at = Instant::now();
            for (i, (peer, address)) in told.iter().enumerate() {
                let expires = at + Duration::from_secs(60) + Duration::from_millis(i as u64);
                behaviour
                    .query_response_sender
                    .clone()
                    .try_send((*peer, address.clone(), expires, computer))
                    .expect("room");
            }
        };
        let mut cx = Context::from_waker(futures::task::noop_waker_ref());
        // The first query: the copies answer; the first four make room for the last four.
        tell(&mut behaviour, &copies);
        while behaviour.poll(&mut cx).is_ready() {}
        // The next: the newcomer answers first, then the copies kept, then the four left out,
        // the first of which now takes the newcomer's place.
        let mut next = vec![(newcomer, newcomer_at.clone())];
        next.extend(copies[5..].iter().cloned());
        next.extend(copies[..5].iter().cloned());
        tell(&mut behaviour, &next);
        let Poll::Ready(ToSwarm::GenerateEvent(Event::Discovered(found))) = behaviour.poll(&mut cx)
        else {
            panic!("nothing told of");
        };
        assert!(found.contains(&(newcomer, newcomer_at.clone())));
        assert_eq!(
            behaviour.told_by(&newcomer, &newcomer_at),
            Some(computer),
            "pushed out before the node could ask who told of it"
        );
        // Pushed out a moment later, it is told of again at its next answer.
        while behaviour.poll(&mut cx).is_ready() {}
        assert_eq!(behaviour.told_by(&newcomer, &newcomer_at), None);
        tell(&mut behaviour, &[(newcomer, newcomer_at.clone())]);
        let Poll::Ready(ToSwarm::GenerateEvent(Event::Discovered(found))) = behaviour.poll(&mut cx)
        else {
            panic!("not told of again");
        };
        assert_eq!(found, vec![(newcomer, newcomer_at)]);
    }

    /// A service name its packets cannot carry is refused, not met later in a panic.
    #[::tokio::test]
    async fn only_a_service_name_packets_can_carry_is_taken() {
        let config = |name: &str| Config {
            service_name: name.to_owned(),
            ..Config::default()
        };
        for name in ["_p2p._udp.local", "_fogata-test-0123abcd._udp.local"] {
            assert!(
                TokioMdns::new(config(name), PeerId::random()).is_ok(),
                "{name}"
            );
        }
        let long = format!("_{}._udp.local", "x".repeat(63));
        for name in [
            "",
            "_p2p..local",
            "_p2p._udp.local.",
            "_pé._udp.local",
            &long,
        ] {
            assert!(
                TokioMdns::new(config(name), PeerId::random()).is_err(),
                "{name}"
            );
        }
    }

    /// The tasks of the interfaces end when the behaviour goes.
    #[tokio::test]
    async fn the_interfaces_stop_with_the_behaviour() {
        let mut behaviour = TokioMdns::new(Config::default(), PeerId::random()).expect("mdns");
        let (told, ended) = futures::channel::oneshot::channel::<()>();
        behaviour.if_tasks.insert(
            IpAddr::from([192, 168, 1, 20]),
            tokio::spawn(async move {
                let _told = told;
                futures::future::pending::<()>().await;
            }),
        );
        drop(behaviour);
        let ended = tokio::time::timeout(std::time::Duration::from_secs(5), ended).await;
        assert!(
            matches!(ended, Ok(Err(futures::channel::oneshot::Canceled))),
            "the task outlived its behaviour"
        );
    }
}
