//! The node's loop: each turn waits for the next thing to do and does it. A panic in a turn
//! loses that turn, not the node: it is blamed on whoever's doing it was, as far as that is
//! known, and only one that keeps coming back makes the node start afresh, or stop.

use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

use futures::{FutureExt, StreamExt};
use libp2p::{
    PeerId, dcutr, identify,
    kad::{self, GetProvidersOk, QueryResult},
    mdns, ping, relay,
    swarm::SwarmEvent,
};

#[cfg(test)]
use super::Pending;
use super::{Node, links::LINK_CHECK, slot::until_next_slot};
use crate::{Command, behaviour::BehaviourEvent};

/// Longest wait, when the node stops, for its connections to close and those being set up
/// to end.
const CLOSE_WAIT: Duration = Duration::from_millis(800);
/// Then, with the swarm gone, a moment for the closes to leave: QUIC sends them from its
/// endpoint's own task, and a connection counts as closed before they are out.
const CLOSE_FLUSH: Duration = Duration::from_millis(200);
/// Longest wait, when the node stops, for its tasks to end once aborted: one ends at its
/// next await, unless it blocks a thread.
const TASKS_WAIT: Duration = Duration::from_secs(2);

/// Internal errors (panics) within this long of each other after which the node starts
/// afresh (see [`Node::start_afresh`]); so many again within [`AFRESH_WINDOW`] of that stop
/// it, for the application to start again whole.
const MAX_PANICS: usize = 3;
const PANIC_WINDOW: Duration = Duration::from_secs(60);
const AFRESH_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Internal errors within the network library, as it is polled, within [`PANIC_WINDOW`]
/// after which the node stops (see `Node::run`).
const MAX_SWARM_PANICS: usize = 100;
/// Internal errors each blamed on a peer, from at least so many peers, within this long,
/// after which the node starts afresh too: an error that every peer's doing brings (a state
/// gone wrong here) would otherwise disconnect one innocent peer after another. They never
/// stop it: peers are free to make. So many with DHT answers or local network
/// announcements make it start afresh too, and never stop it either: those cost their
/// sender a packet each, and are blamed on what can be named (the lookup an answer is to,
/// which gets no more answers; the address an announcement came from, which is not heard
/// for a while).
const MAX_PEER_PANICS: usize = 10;
const MAX_PEER_PANICS_PEERS: usize = 3;
const PEER_PANIC_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Starting afresh for others' doing ends every call and transfer: it comes at most once in
/// this long, which doubles each time it comes again soon after, up to
/// [`AFRESH_SPACING_MAX`]; meanwhile the errors only cost what they are blamed on.
const AFRESH_SPACING: Duration = Duration::from_secs(10 * 60);
const AFRESH_SPACING_MAX: Duration = Duration::from_secs(4 * 60 * 60);
/// How long the local network announcements of an address whose announcement made the node
/// panic are not heard, and how many such addresses are kept.
const TELLER_MUTED: Duration = Duration::from_secs(10 * 60);
const MAX_MUTED_TELLERS: usize = 256;
/// Lookups whose answers made the node panic, remembered so that answers already on their
/// way are not handled.
const MAX_DROPPED_LOOKUPS: usize = 64;

/// Whose doing a swarm event is, should handling it make the node panic (see `Node::run`).
enum Blame {
    /// The peer's that sent it, or whose connection it is about.
    Peer(PeerId),
    /// A DHT answer or a local network announcement: its sender's, which it does not name
    /// (the DHT library does not say which node answered; anyone on the network can send
    /// an announcement in any name), and which may name anyone, so nobody is disconnected.
    Answer(Answer),
    /// This device's own: its timers, listeners and dials.
    Nobody,
}

/// What an answer whose sender is not known can be blamed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Answer {
    /// An answer to this lookup.
    Lookup(kad::QueryId),
    /// Local network announcements, from these addresses as far as known.
    Told(Vec<std::net::IpAddr>),
}

/// When starting afresh for others' doing may come again (see [`AFRESH_SPACING`]).
#[derive(Debug)]
struct Spacing {
    last: Option<Instant>,
    wait: Duration,
}

impl Spacing {
    fn new() -> Self {
        Self {
            last: None,
            wait: AFRESH_SPACING,
        }
    }

    fn allows(&self, now: Instant) -> bool {
        self.last
            .is_none_or(|t| now.saturating_duration_since(t) >= self.wait)
    }

    /// The node started afresh now: the next time waits twice as long if this one came
    /// soon after the wait ended, else the first wait again.
    fn started(&mut self, now: Instant) {
        let soon = self
            .last
            .is_some_and(|t| now.saturating_duration_since(t) < 2 * self.wait);
        self.wait = if soon {
            (2 * self.wait).min(AFRESH_SPACING_MAX)
        } else {
            AFRESH_SPACING
        };
        self.last = Some(now);
    }
}

/// See [`Blame`]. Events no code here handles are left out.
fn blamed(event: &SwarmEvent<BehaviourEvent>) -> Blame {
    let peer = match event {
        SwarmEvent::ConnectionEstablished { peer_id, .. }
        | SwarmEvent::ConnectionClosed { peer_id, .. }
        | SwarmEvent::OutgoingConnectionError {
            peer_id: Some(peer_id),
            ..
        } => *peer_id,
        SwarmEvent::Behaviour(event) => match event {
            BehaviourEvent::Identify(
                identify::Event::Received { peer_id, .. }
                | identify::Event::Sent { peer_id, .. }
                | identify::Event::Pushed { peer_id, .. }
                | identify::Event::Error { peer_id, .. },
            )
            | BehaviourEvent::Ping(ping::Event { peer: peer_id, .. })
            | BehaviourEvent::Dcutr(dcutr::Event {
                remote_peer_id: peer_id,
                ..
            })
            | BehaviourEvent::RelayClient(relay::client::Event::ReservationReqAccepted {
                relay_peer_id: peer_id,
                ..
            })
            | BehaviourEvent::Relay(
                relay::Event::ReservationReqAccepted {
                    src_peer_id: peer_id,
                    ..
                }
                | relay::Event::CircuitReqAccepted {
                    src_peer_id: peer_id,
                    ..
                },
            ) => *peer_id,
            BehaviourEvent::Kad(kad::Event::OutboundQueryProgressed {
                id,
                result: QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders { .. })),
                ..
            }) => return Blame::Answer(Answer::Lookup(*id)),
            // Who told of what is filled in by the node, which knows (see `on_swarm_event`).
            BehaviourEvent::Mdns(_) => return Blame::Answer(Answer::Told(Vec::new())),
            _ => return Blame::Nobody,
        },
        _ => return Blame::Nobody,
    };
    Blame::Peer(peer)
}

/// Debug builds only, to see what the application does when the node stops by itself:
/// `FOGATA_TEST_NODE_PANIC=<seconds>[:<file>]` makes every turn of the node panic once it has
/// run that long, as a fault that keeps coming back would. With a file, only while the file
/// does not exist; it is made at the first panic, so a copy started again runs on.
#[cfg(debug_assertions)]
pub(super) struct TestPanic {
    at: Instant,
    file: Option<std::path::PathBuf>,
}

#[cfg(debug_assertions)]
impl TestPanic {
    pub(super) fn from_env() -> Option<Self> {
        let value = std::env::var("FOGATA_TEST_NODE_PANIC").ok()?;
        let (seconds, file) = match value.split_once(':') {
            Some((seconds, file)) => (seconds, Some(std::path::PathBuf::from(file))),
            None => (value.as_str(), None),
        };
        if file.as_ref().is_some_and(|f| f.exists()) {
            return None;
        }
        let seconds: u64 = seconds.parse().ok()?;
        tracing::warn!("the node will panic after {seconds} s (FOGATA_TEST_NODE_PANIC)");
        Some(Self {
            at: Instant::now() + Duration::from_secs(seconds),
            file,
        })
    }

    fn check(&self) {
        if Instant::now() < self.at {
            return;
        }
        if let Some(file) = &self.file {
            let _ = std::fs::write(file, b"");
        }
        panic!("FOGATA_TEST_NODE_PANIC");
    }
}

impl Node {
    pub(super) async fn run(mut self) {
        // Slots follow the wall clock, measured again each time: a timer of the monotonic
        // clock drifts away from it while the computer sleeps, and both ends of a punch
        // must agree on the slot.
        let slot = tokio::time::sleep(until_next_slot());
        tokio::pin!(slot);
        let mut links = tokio::time::interval(LINK_CHECK);
        links.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut panics: VecDeque<Instant> = VecDeque::new();
        let mut peer_panics: VecDeque<(Instant, PeerId)> = VecDeque::new();
        let mut afresh: Option<Instant> = None;
        let mut answer_panics: VecDeque<Instant> = VecDeque::new();
        let mut swarm_panics: VecDeque<Instant> = VecDeque::new();
        // Starting afresh for peers' or answers' doing, spaced out.
        let mut others = Spacing::new();
        let stopped = loop {
            // A panic in one turn (in a library's code, say) loses that turn, not the node:
            // without it nothing would reach this device or leave it until the application
            // starts again. One that keeps coming back makes the node start afresh, and
            // stops it if it keeps coming back still.
            let turn = std::panic::AssertUnwindSafe(self.turn(&mut slot, &mut links))
                .catch_unwind()
                .await;
            match turn {
                Ok(Some(stopped)) => break stopped,
                Ok(None) => {}
                Err(panic) => {
                    let what = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    // One that came while handling what a peer sent is that peer's doing:
                    // it is disconnected, and nobody can stop the node by sending the same
                    // thing again and again.
                    let now = Instant::now();
                    let answer = std::mem::take(&mut self.serving_answer);
                    let blamed = self.serving.take();
                    let in_swarm = std::mem::take(&mut self.polling_swarm);
                    // One from within the network library, as it polled, is counted with
                    // the answers' below: it may come from what anyone sent. Only so many
                    // in a row, as a state gone wrong in the library makes on every poll,
                    // stop the node, for the application to start it again whole rather
                    // than turn on it for good; a peer would have to send that many a
                    // minute to make it start again.
                    if in_swarm && blamed.is_none() && answer.is_none() {
                        swarm_panics.retain(|t| now.duration_since(*t) < PANIC_WINDOW);
                        swarm_panics.push_back(now);
                        if swarm_panics.len() >= MAX_SWARM_PANICS {
                            tracing::error!(
                                "the network node stops after {} internal errors within the \
                                 network library in a minute: {what}",
                                swarm_panics.len()
                            );
                            break None;
                        }
                    }
                    let answer = match (in_swarm, answer) {
                        (true, None) => Some(Answer::Told(Vec::new())),
                        (_, answer) => answer,
                    };
                    // One that came with a DHT answer or a local network announcement is
                    // its sender's, which may be anyone, with nobody to disconnect, and
                    // costs it one packet: the lookup gets no more answers, the address is
                    // not heard for a while, and so many make the node start afresh (a
                    // state gone wrong here), spaced out, but never stop it.
                    if let (None, Some(answer)) = (blamed, answer) {
                        self.blame_answer(answer, now);
                        answer_panics.retain(|t| now.duration_since(*t) < PEER_PANIC_WINDOW);
                        answer_panics.push_back(now);
                        if answer_panics.len() < MAX_PEER_PANICS {
                            tracing::error!(
                                "the network node goes on after an internal error caused by \
                                 an answer: {what}"
                            );
                            continue;
                        }
                        answer_panics.clear();
                        if !others.allows(now) {
                            tracing::error!(
                                "the network node goes on after internal errors caused by \
                                 answers, having started afresh lately: {what}"
                            );
                            continue;
                        }
                        tracing::error!(
                            "the network node starts afresh after internal errors caused by \
                             answers: {what}"
                        );
                        others.started(now);
                        self.start_afresh();
                        continue;
                    }
                    if let Some(peer) = blamed {
                        let _ = self.swarm.disconnect_peer_id(peer);
                        peer_panics.retain(|(t, _)| now.duration_since(*t) < PEER_PANIC_WINDOW);
                        peer_panics.push_back((now, peer));
                        let peers: HashSet<PeerId> = peer_panics.iter().map(|(_, p)| *p).collect();
                        if peer_panics.len() >= MAX_PEER_PANICS
                            && peers.len() >= MAX_PEER_PANICS_PEERS
                            && others.allows(now)
                        {
                            others.started(now);
                            tracing::error!(
                                "the network node starts afresh after {} internal errors \
                                 with {} peers: {what}",
                                peer_panics.len(),
                                peers.len()
                            );
                            peer_panics.clear();
                            self.start_afresh();
                            continue;
                        }
                        tracing::error!(
                            %peer,
                            "the network node goes on after an internal error caused by a \
                             peer, now disconnected: {what}"
                        );
                        continue;
                    }
                    panics.retain(|t| now.duration_since(*t) < PANIC_WINDOW);
                    panics.push_back(now);
                    if panics.len() >= MAX_PANICS {
                        if afresh.is_some_and(|t| now.duration_since(t) < AFRESH_WINDOW) {
                            tracing::error!(
                                "the network node stops after {} internal errors in a row, \
                                 again soon after starting afresh: {what}",
                                panics.len()
                            );
                            break None;
                        }
                        tracing::error!(
                            "the network node starts afresh after {} internal errors in a \
                             row: {what}",
                            panics.len()
                        );
                        panics.clear();
                        afresh = Some(now);
                        self.start_afresh();
                        continue;
                    }
                    tracing::error!("the network node goes on after an internal error: {what}");
                }
            }
        };
        // Connections still being set up close as the swarm goes, and the node's tasks are
        // aborted, whatever stopped it; then a moment for all the closes to leave before
        // saying it is done.
        let tasks = self.tasks.clone();
        drop(self);
        tasks.stop(TASKS_WAIT).await;
        if let Some(reply) = stopped {
            tokio::time::sleep(CLOSE_FLUSH).await;
            let _ = reply.send(());
        }
    }

    /// After internal errors that kept coming back: every connection closes, which drops
    /// all that was kept for each peer (and tells the application they went), and friends
    /// and members are tried again at the next slot, as after a network change. The groups,
    /// friends and settings stay, and whatever holds this node's handle keeps working.
    fn start_afresh(&mut self) {
        self.afresh += 1;
        let peers: Vec<PeerId> = self.swarm.connected_peers().copied().collect();
        for peer in peers {
            let _ = self.swarm.disconnect_peer_id(peer);
        }
        self.network_pending = Some(self.network_pending.unwrap_or(false));
    }

    /// Waits for the next thing to do and does it. Returns whether the node stops, with the
    /// reply to a shutdown when it was asked to.
    async fn turn(
        &mut self,
        slot: &mut std::pin::Pin<&mut tokio::time::Sleep>,
        links: &mut tokio::time::Interval,
    ) -> Option<Option<tokio::sync::oneshot::Sender<()>>> {
        #[cfg(debug_assertions)]
        if let Some(test) = &self.test_panic {
            test.check();
        }
        tokio::select! {
            event = {
                let (swarm, polling) = (&mut self.swarm, &mut self.polling_swarm);
                #[cfg(test)]
                let panic_polling = &mut self.panic_polling;
                std::future::poll_fn(move |cx| {
                    *polling = true;
                    #[cfg(test)]
                    if std::mem::take(panic_polling) {
                        panic!("a test asked the swarm to panic as it was polled");
                    }
                    let polled = swarm.poll_next_unpin(cx);
                    *polling = false;
                    match polled {
                        std::task::Poll::Ready(Some(event)) => std::task::Poll::Ready(event),
                        _ => std::task::Poll::Pending,
                    }
                })
            } => self.on_swarm_event(event),
            command = self.commands.recv() => match command {
                Some(Command::Shutdown(reply)) => {
                    self.close_all().await;
                    return Some(Some(reply));
                }
                #[cfg(test)]
                Some(Command::Panic) => panic!("a test asked the node to panic"),
                #[cfg(test)]
                Some(Command::PanicHolding(peer)) => {
                    let mut verified = crate::locked(&self.verified);
                    verified.insert(peer);
                    panic!("a test asked the node to panic holding a lock");
                }
                #[cfg(test)]
                Some(Command::PanicServing(peer)) => {
                    self.serving = Some(peer);
                    panic!("a test asked the node to panic handling a peer's request");
                }
                #[cfg(test)]
                Some(Command::PanicPolling) => self.panic_polling = true,
                #[cfg(test)]
                Some(Command::PanicAnswer) => {
                    self.serving_answer = Some(Answer::Told(Vec::new()));
                    panic!("a test asked the node to panic handling a DHT answer");
                }
                #[cfg(test)]
                Some(Command::PanicLookup(id)) => {
                    self.serving_answer = Some(Answer::Lookup(id));
                    panic!("a test asked the node to panic handling an answer to a lookup");
                }
                #[cfg(test)]
                Some(Command::PanicTold(from)) => {
                    self.serving_answer = Some(Answer::Told(vec![from]));
                    panic!("a test asked the node to panic handling an announcement");
                }
                #[cfg(test)]
                Some(Command::Request(peer, request)) => {
                    self.request(peer, *request, Pending::Push);
                }
                #[cfg(test)]
                Some(Command::Inspect(inspect)) => inspect(self as &mut dyn std::any::Any),
                #[cfg(feature = "fuzzing")]
                Some(Command::Fuzz {
                    request,
                    from_member,
                    reply,
                }) => {
                    self.fuzz_request(*request, from_member);
                    let _ = reply.send(());
                }
                #[cfg(feature = "fuzzing")]
                Some(Command::FuzzJoin {
                    setup,
                    steps,
                    reply,
                }) => {
                    self.fuzz_join(*setup, steps);
                    let _ = reply.send(());
                }
                #[cfg(feature = "fuzzing")]
                Some(Command::FuzzFriend { book, steps, reply }) => {
                    self.fuzz_friend(*book, steps);
                    let _ = reply.send(());
                }
                Some(command) => self.on_command(command),
                None => return Some(None),
            },
            () = slot.as_mut() => {
                self.on_slot();
                slot.as_mut().reset(tokio::time::Instant::now() + until_next_slot());
            }
            _ = links.tick() => {
                self.answer_waiting_addresses();
                self.push_shown_again();
                self.retry_mdns_dials();
                self.check_member_links();
                self.redial_let_go();
                self.take_held_updates();
            }
            Some((peer, signal)) = self.typing.next() => {
                self.serving = Some(peer);
                self.on_typing(peer, signal);
                self.serving = None;
            }
            judged = self.judging.next() => {
                self.judging.finished(&judged);
                self.serving = Some(judged.peer);
                self.on_judged(judged);
                self.serving = None;
            }
        }
        None
    }

    /// Closes every connection and drives the swarm until they are closed, for at most
    /// [`CLOSE_WAIT`]; connections that come in meanwhile are closed as well.
    async fn close_all(&mut self) {
        let peers: Vec<PeerId> = self.swarm.connected_peers().copied().collect();
        for peer in peers {
            let _ = self.swarm.disconnect_peer_id(peer);
        }
        let deadline = tokio::time::sleep(CLOSE_WAIT);
        tokio::pin!(deadline);
        // Connections still being set up count too: one that completes after the swarm is
        // gone is never closed here, and the other end keeps it until it times out.
        while self.swarm.connected_peers().next().is_some()
            || self
                .swarm
                .network_info()
                .connection_counters()
                .num_pending()
                > 0
        {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        let _ = self.swarm.disconnect_peer_id(peer_id);
                    }
                }
                _ = &mut deadline => break,
            }
        }
    }

    fn on_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        // What a peer's connection or what it said brings is its doing, should it make the
        // node panic (see `run`); the member and friend protocols say so themselves.
        match blamed(&event) {
            Blame::Peer(peer) => self.serving = Some(peer),
            Blame::Answer(answer) => self.serving_answer = Some(self.tellers(answer, &event)),
            Blame::Nobody => {}
        }
        self.on_swarm_event_of(event);
        self.serving = None;
        self.serving_answer = None;
    }

    /// `answer` with the addresses of the responses that told of what a local network
    /// announcement `event` brings, as far as they are known.
    fn tellers(&self, answer: Answer, event: &SwarmEvent<BehaviourEvent>) -> Answer {
        let (
            Answer::Told(_),
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(found))),
            Some(mdns),
        ) = (&answer, event, self.swarm.behaviour().mdns.as_ref())
        else {
            return answer;
        };
        let mut by: Vec<std::net::IpAddr> = found
            .iter()
            .filter_map(|(peer, address)| mdns.told_by(peer, address))
            .collect();
        by.sort();
        by.dedup();
        Answer::Told(by)
    }

    /// An answer whose sender is not known made the node panic: the lookup it is to gets no
    /// more answers, and the addresses that told of an announcement are not heard for a
    /// while.
    fn blame_answer(&mut self, answer: Answer, now: Instant) {
        match answer {
            Answer::Lookup(id) => {
                if let Some(kad) = self.swarm.behaviour_mut().kad.as_mut()
                    && let Some(mut query) = kad.query_mut(&id)
                {
                    query.finish();
                }
                if self.dropped_lookups.len() >= MAX_DROPPED_LOOKUPS {
                    self.dropped_lookups.pop_front();
                }
                self.dropped_lookups.push_back(id);
            }
            Answer::Told(by) => {
                self.muted_tellers.retain(|_, until| *until > now);
                for ip in by {
                    if self.muted_tellers.len() >= MAX_MUTED_TELLERS
                        && !self.muted_tellers.contains_key(&ip)
                    {
                        let soonest = self
                            .muted_tellers
                            .iter()
                            .min_by_key(|(_, until)| **until)
                            .map(|(ip, _)| *ip);
                        if let Some(soonest) = soonest {
                            self.muted_tellers.remove(&soonest);
                        }
                    }
                    tracing::debug!(%ip, "local network announcements from there not heard now");
                    self.muted_tellers.insert(ip, now + TELLER_MUTED);
                }
            }
        }
    }

    /// Whether local network announcements from `ip` are not heard now (see
    /// [`Self::blame_answer`]).
    pub(super) fn teller_muted(&self, ip: &std::net::IpAddr) -> bool {
        self.muted_tellers
            .get(ip)
            .is_some_and(|until| *until > Instant::now())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::GroupLog;
    use libp2p::{Multiaddr, kad::RecordKey, swarm::ConnectionId};

    use super::*;
    use crate::{
        NodeConfig, NodeHandle,
        group::GroupSetup,
        node::{
            start,
            tests::{inspect, lone_member},
        },
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn the_node_goes_on_after_a_panic_starts_afresh_after_several_and_stops_after_more() {
        let (node, _events) = start(NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        node.commands.send(Command::Panic).await.unwrap();
        assert!(
            node.addresses().await.is_ok(),
            "the node stopped at one panic"
        );
        // A panic while a lock was held leaves what the lock keeps usable, not answering no
        // for ever.
        let peer = PeerId::random();
        node.commands
            .send(Command::PanicHolding(peer))
            .await
            .unwrap();
        assert!(node.addresses().await.is_ok());
        assert!(node.verified.is_poisoned());
        assert!(node.is_member(&peer));
        // Panics while handling what a peer sent cost that peer its connections, and never
        // stop the node.
        for _ in 0..2 * MAX_PANICS {
            node.commands
                .send(Command::PanicServing(PeerId::random()))
                .await
                .unwrap();
        }
        assert!(
            node.addresses().await.is_ok(),
            "a peer's requests stopped the node"
        );
        // Its own that keep coming back make it start afresh; again soon after, they stop
        // it, for the application to start again whole.
        for _ in 2..MAX_PANICS {
            node.commands.send(Command::Panic).await.unwrap();
        }
        assert!(node.addresses().await.is_ok(), "the node stopped at once");
        assert_eq!(inspect(&node, |n| n.afresh).await, 1);
        for _ in 0..MAX_PANICS {
            node.commands.send(Command::Panic).await.unwrap();
        }
        assert!(node.addresses().await.is_err(), "the node never stops");
    }

    /// A panic while handling what the identify and relay protocols say is the peer's doing
    /// (it loses its connections), and one while handling a local network announcement is
    /// its unnamed sender's; this device's own events are nobody's.
    #[test]
    fn a_peer_s_identify_relay_and_announcements_are_its_doing() {
        let (peer, other) = (PeerId::random(), PeerId::random());
        let connection_id = ConnectionId::new_unchecked(1);
        let of = |event| match blamed(&SwarmEvent::Behaviour(event)) {
            Blame::Peer(p) => Some(Some(p)),
            Blame::Answer(_) => Some(None),
            Blame::Nobody => None,
        };
        assert_eq!(
            of(BehaviourEvent::Identify(identify::Event::Sent {
                connection_id,
                peer_id: peer,
            })),
            Some(Some(peer))
        );
        assert_eq!(
            of(BehaviourEvent::Relay(
                relay::Event::ReservationReqAccepted {
                    src_peer_id: peer,
                    renewed: false,
                }
            )),
            Some(Some(peer))
        );
        assert_eq!(
            of(BehaviourEvent::Relay(relay::Event::CircuitReqAccepted {
                src_peer_id: peer,
                dst_peer_id: other,
            })),
            Some(Some(peer))
        );
        assert_eq!(
            of(BehaviourEvent::Mdns(mdns::Event::Discovered(vec![(
                other,
                "/ip4/192.168.1.7/udp/4001/quic-v1".parse().unwrap(),
            )]))),
            Some(None)
        );
        assert!(matches!(
            blamed(&SwarmEvent::NewListenAddr {
                listener_id: libp2p::core::transport::ListenerId::next(),
                address: "/ip4/127.0.0.1/udp/4001/quic-v1".parse().unwrap(),
            }),
            Blame::Nobody
        ));
    }

    /// Peers are free to make: however many make the node panic, it never stops. Peer
    /// after peer, something here is wrong rather than each of them, and it starts afresh,
    /// but not again and again: every call would end each time.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_node_whose_every_peer_makes_it_panic_starts_afresh_once_and_goes_on() {
        let (node, _group) = lone_member();
        // One peer again and again: only that peer's connections go.
        let one = PeerId::random();
        for _ in 0..2 * MAX_PEER_PANICS {
            node.commands
                .send(Command::PanicServing(one))
                .await
                .unwrap();
        }
        assert!(node.addresses().await.is_ok(), "one peer stopped the node");
        assert_eq!(inspect(&node, |n| n.afresh).await, 0);
        for _ in 1..=3 {
            for _ in 0..MAX_PEER_PANICS {
                node.commands
                    .send(Command::PanicServing(PeerId::random()))
                    .await
                    .unwrap();
            }
            assert!(node.addresses().await.is_ok(), "peers stopped the node");
            assert_eq!(
                inspect(&node, |n| n.afresh).await,
                1,
                "peers made the node start afresh again and again"
            );
        }
        // Nor DHT answers, which do not say who sent them, soon after.
        for _ in 0..MAX_PEER_PANICS {
            node.commands.send(Command::PanicAnswer).await.unwrap();
        }
        assert!(
            node.addresses().await.is_ok(),
            "DHT answers stopped the node"
        );
        assert_eq!(inspect(&node, |n| n.afresh).await, 1);
        node.shutdown().await.unwrap();
    }

    /// A panic within the network library, as the node polls it, may come from what anyone
    /// sent: however many, the node goes on, starting afresh once.
    #[tokio::test(flavor = "multi_thread")]
    async fn panics_within_the_network_library_never_stop_the_node() {
        let (node, _group) = lone_member();
        for _ in 0..3 * MAX_PEER_PANICS {
            assert!(
                panic_polling(&node).await,
                "the library's panics stopped the node"
            );
        }
        assert_eq!(inspect(&node, |n| n.afresh).await, 1);
        node.shutdown().await.unwrap();
    }

    /// A panic within the network library on poll after poll, as a state gone wrong there
    /// makes, stops the node rather than have it turn on it for good.
    #[tokio::test(flavor = "multi_thread")]
    async fn panics_within_the_network_library_on_every_poll_stop_the_node() {
        let (node, _group) = lone_member();
        for i in 1..=MAX_SWARM_PANICS {
            let went_on = panic_polling(&node).await;
            assert_eq!(went_on, i < MAX_SWARM_PANICS, "after {i} panics");
        }
        assert!(node.addresses().await.is_err(), "the node went on");
    }

    /// Makes the next poll of the swarm panic, and waits until it has: whether the node goes
    /// on after it. A turn that handles a command does not poll the swarm when the command
    /// comes first, so a second `PanicPolling` sent before the swarm was polled would only
    /// ask again for the panic still waiting: one panic for two.
    async fn panic_polling(node: &NodeHandle) -> bool {
        if node.commands.send(Command::PanicPolling).await.is_err() {
            return false;
        }
        loop {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let asked = node
                .commands
                .send(Command::Inspect(Box::new(move |n| {
                    let n = n.downcast_mut::<Node>().expect("the node");
                    let _ = tx.send(n.panic_polling);
                })))
                .await;
            match (asked, rx.await) {
                (Ok(()), Ok(false)) => return true,
                (Ok(()), Ok(true)) => {}
                _ => return false,
            }
        }
    }

    /// Answers on the DHT or the local network do not say who sent them, and each costs its
    /// sender a packet: one sender making the node panic again and again neither stops it
    /// nor makes it start afresh, ending every call, again and again.
    #[tokio::test(flavor = "multi_thread")]
    async fn answers_that_make_the_node_panic_never_stop_it() {
        let (node, _group) = lone_member();
        for _ in 0..3 * MAX_PEER_PANICS {
            node.commands.send(Command::PanicAnswer).await.unwrap();
        }
        assert!(
            node.addresses().await.is_ok(),
            "answers making the node panic stopped it"
        );
        assert_eq!(inspect(&node, |n| n.afresh).await, 1);
        node.shutdown().await.unwrap();
    }

    /// Starting afresh for others' doing comes again only after a while, longer each time
    /// it comes again soon after.
    #[test]
    fn starting_afresh_for_others_is_spaced_out() {
        let start = Instant::now();
        let mut spacing = Spacing::new();
        assert!(spacing.allows(start));
        spacing.started(start);
        assert!(!spacing.allows(start + AFRESH_SPACING / 2));
        let second = start + AFRESH_SPACING;
        assert!(spacing.allows(second));
        spacing.started(second);
        assert!(!spacing.allows(second + AFRESH_SPACING));
        let third = second + 2 * AFRESH_SPACING;
        assert!(spacing.allows(third));
        spacing.started(third);
        // A long quiet time brings the first wait back.
        let much_later = third + 10 * AFRESH_SPACING_MAX;
        spacing.started(much_later);
        assert!(spacing.allows(much_later + AFRESH_SPACING));
    }

    /// A local network announcement that made the node panic is blamed on the address it
    /// came from: what that address tells of is not dialled for a while, while what others
    /// tell of still is.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_announcement_that_made_the_node_panic_mutes_its_sender() {
        let (node, _group) = lone_member();
        let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
        node.commands
            .send(Command::PanicTold(neighbour))
            .await
            .unwrap();
        let (from_neighbour, from_another) = inspect(&node, move |n| {
            let told = |n: &mut Node, by: std::net::IpAddr| {
                let peer = PeerId::random();
                let address: Multiaddr = format!("/ip4/{by}/udp/4001/quic-v1").parse().unwrap();
                n.dial_mdns_found(vec![(peer, address, Some(by))]);
                n.mdns_dials.iter().any(|(p, _)| *p == peer)
            };
            (
                told(n, neighbour),
                told(n, std::net::IpAddr::from([192, 168, 1, 67])),
            )
        })
        .await;
        assert!(
            !from_neighbour,
            "the address whose announcement made the node panic was heard"
        );
        assert!(from_another);
        node.shutdown().await.unwrap();
    }

    /// An answer to a lookup that made the node panic stops that lookup: answers to it
    /// already on their way are not handled, so one lookup makes the node panic once.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_lookup_whose_answer_made_the_node_panic_gets_no_more_answers() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let group = log.group_id();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups: vec![GroupSetup::Member(log)],
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: true,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let key = RecordKey::new(b"a lookup");
        let query = inspect(&node, {
            let key = key.clone();
            move |n| {
                n.group_keys.insert(key.clone(), group);
                n.look_up(key.clone());
                n.lookup_query[&key]
            }
        })
        .await;
        node.commands
            .send(Command::PanicLookup(query))
            .await
            .unwrap();
        let taken = inspect(&node, move |n| {
            let peer = PeerId::random();
            n.on_kad_answer(
                query,
                QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders {
                    key,
                    providers: HashSet::from([peer]),
                })),
            );
            n.candidates.contains_key(&peer)
        })
        .await;
        assert!(
            !taken,
            "an answer to a lookup that made the node panic was handled"
        );
        assert!(node.addresses().await.is_ok());
        node.shutdown().await.unwrap();
    }
}
