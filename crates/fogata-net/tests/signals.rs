//! Call signals between members: they reach a device only while it is in one of the group's
//! voice rooms, from the members in that room (a few from others, whose join may still be on
//! its way), and a member cannot send large ones or flood it with them; they go on the best
//! connection to the member, and on another once that one closes. Nodes in one process, over
//! the loopback interface; the other side is played by hand where the connection each signal
//! came on has to be seen.

use std::{collections::HashSet, sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId, GroupKeys, MembershipProof};
use fogata_net::{
    FIRST_ROOM, GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle,
};
use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse, WireProof};
use fogata_state::GroupLog;
use futures::StreamExt;
use libp2p::{
    PeerId, StreamProtocol, Swarm,
    request_response::{self, ProtocolSupport, ResponseChannel},
    swarm::{
        ConnectionId, SwarmEvent,
        dial_opts::{DialOpts, PeerCondition},
    },
};
use tokio::time::timeout;

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn node(
    identity: Arc<DeviceIdentity>,
    log: GroupLog,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log)],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers,
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap()
}

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

/// Waits until `device` is connected as a member on the node of `events`, dialling again now
/// and then.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = timeout(Duration::from_secs(10), async {
            while let Some(event) = events.recv().await {
                if matches!(event, NodeEvent::MemberConnected { device: d, .. } if d == device) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        if connected {
            return;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    panic!("timed out waiting for the member to connect");
}

/// The sizes of the call signals that come within `seconds`.
async fn signals_within(events: &mut NodeEvents, seconds: u64) -> Vec<usize> {
    let mut got = Vec::new();
    let _ = timeout(Duration::from_secs(seconds), async {
        while let Some(event) = events.recv().await {
            if let NodeEvent::Signal { payload, .. } = event {
                got.push(payload.len());
            }
        }
    })
    .await;
    got
}

async fn in_room(node: &NodeHandle, group: GroupId, device: DevicePublicKey) -> bool {
    node.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .is_some_and(|r| r.participants.iter().any(|(d, _, _)| *d == device))
}

#[tokio::test(flavor = "multi_thread")]
async fn signals_reach_a_device_in_a_room_from_its_people_and_only_so_many() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "ours", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, vec![]);
    let (node_b, mut events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;

    // Bea is in no room: nothing reaches her.
    node_a
        .signal(group, bea.public_key(), vec![1; 300])
        .await
        .unwrap();
    assert!(signals_within(&mut events_b, 2).await.is_empty());

    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    let joined = timeout(Duration::from_secs(10), async {
        while !in_room(&node_b, group, bea.public_key()).await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(joined.is_ok(), "Bea is not in the room");
    // Ana is in no room: Bea hears her only a few times (her join could be on its way).
    for _ in 0..12 {
        node_a
            .signal(group, bea.public_key(), vec![3; 300])
            .await
            .unwrap();
    }
    assert_eq!(signals_within(&mut events_b, 3).await.len(), 4);

    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    let joined = timeout(Duration::from_secs(10), async {
        while !in_room(&node_b, group, ana.public_key()).await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(joined.is_ok(), "Ana is not in the room at Bea's");
    // The burst is full again (a few signals a second).
    tokio::time::sleep(Duration::from_secs(5)).await;
    node_a
        .signal(group, bea.public_key(), vec![1; 300])
        .await
        .unwrap();
    assert_eq!(signals_within(&mut events_b, 3).await, [300]);
    // One far larger than a call needs is dropped.
    node_a
        .signal(group, bea.public_key(), vec![1; 100 * 1024])
        .await
        .unwrap();
    assert!(signals_within(&mut events_b, 2).await.is_empty());

    // A flood: a burst gets through, then a few a second.
    let started = std::time::Instant::now();
    for _ in 0..200 {
        node_a
            .signal(group, bea.public_key(), vec![2; 300])
            .await
            .unwrap();
    }
    let got = signals_within(&mut events_b, 4).await.len();
    let seconds = started.elapsed().as_secs_f64();
    assert!(
        got >= 32 && (got as f64) <= 33.0 + 8.0 * seconds,
        "{got} signals in {seconds:.1} s"
    );
}

type Hand = Swarm<request_response::cbor::Behaviour<MemberRequest, MemberResponse>>;
type HandEvent = SwarmEvent<request_response::Event<MemberRequest, MemberResponse>>;

/// A device that speaks only the member protocol, over TCP and QUIC, and keeps its
/// connections while nothing is said on them.
fn by_hand() -> Hand {
    libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .unwrap()
        .with_quic()
        .with_behaviour(|_| {
            request_response::cbor::Behaviour::<MemberRequest, MemberResponse>::new(
                [(StreamProtocol::new(MEMBER_PROTOCOL), ProtocolSupport::Full)],
                request_response::Config::default(),
            )
        })
        .unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
        .build()
}

/// Bea played by hand: she proves herself when challenged, takes every other request, and
/// keeps the connection each call signal and group banner came on. Signals that come on a
/// connection of `hold`, and banners while `hold_banners`, wait for their answer until
/// [`Bea::release`].
struct Bea {
    swarm: Hand,
    identity: DeviceIdentity,
    keys: GroupKeys,
    signals: Vec<ConnectionId>,
    banners: Vec<ConnectionId>,
    hold: HashSet<ConnectionId>,
    hold_banners: bool,
    held: Vec<ResponseChannel<MemberResponse>>,
}

impl Bea {
    /// Handles the next event of Bea's side: a request is answered, anything else returned.
    async fn step(&mut self) -> Option<HandEvent> {
        let event = self.swarm.select_next_some().await;
        let SwarmEvent::Behaviour(request_response::Event::Message {
            peer,
            connection_id,
            message:
                request_response::Message::Request {
                    request, channel, ..
                },
        }) = event
        else {
            return Some(event);
        };
        let answer = match request {
            MemberRequest::Challenge { challenge, .. } => {
                let challenge: [u8; 32] = challenge.try_into().unwrap();
                let bea_peer = self.swarm.local_peer_id().to_bytes();
                let ana_peer = peer.to_bytes();
                let (keys, bea) = (&self.keys, &self.identity);
                let proof = MembershipProof::create(keys, bea, &bea_peer, &ana_peer, &challenge);
                let sealed = proof.seal(keys, &bea_peer, &ana_peer, &challenge).unwrap();
                MemberResponse::Proof(WireProof::from(&sealed))
            }
            MemberRequest::Signal { .. } => {
                self.signals.push(connection_id);
                if self.hold.contains(&connection_id) {
                    self.held.push(channel);
                    return None;
                }
                MemberResponse::Ack
            }
            MemberRequest::GroupBanner { .. } => {
                self.banners.push(connection_id);
                if self.hold_banners {
                    self.held.push(channel);
                    return None;
                }
                MemberResponse::Ack
            }
            _ => MemberResponse::Ack,
        };
        let _ = self.swarm.behaviour_mut().send_response(channel, answer);
        None
    }

    /// Dials Ana at `address` on a connection of its own, and waits for it.
    async fn connect(&mut self, ana: PeerId, address: Multiaddr) -> ConnectionId {
        let dial = DialOpts::peer_id(ana)
            .addresses(vec![address])
            .condition(PeerCondition::Always)
            .build();
        let wanted = dial.connection_id();
        self.swarm.dial(dial).unwrap();
        timeout(Duration::from_secs(20), async {
            loop {
                if let Some(SwarmEvent::ConnectionEstablished { connection_id, .. }) =
                    self.step().await
                    && connection_id == wanted
                {
                    return;
                }
            }
        })
        .await
        .expect("connected");
        wanted
    }

    /// Keeps Bea's side going until `connection` is closed.
    async fn wait_closed(&mut self, connection: ConnectionId) {
        timeout(Duration::from_secs(10), async {
            loop {
                if let Some(SwarmEvent::ConnectionClosed { connection_id, .. }) = self.step().await
                    && connection_id == connection
                {
                    return;
                }
            }
        })
        .await
        .expect("closed");
    }

    /// Answers what waits for its answer, and holds nothing more.
    fn release(&mut self) {
        self.hold.clear();
        self.hold_banners = false;
        for channel in std::mem::take(&mut self.held) {
            let _ = self
                .swarm
                .behaviour_mut()
                .send_response(channel, MemberResponse::Ack);
        }
    }

    /// Has `ana` send Bea a signal at a time until one comes on `connection`: one may go
    /// another way, or be lost on a connection that just closed, while Ana's side does not
    /// count `connection` best yet.
    async fn until_signal_on(&mut self, ana: &NodeHandle, group: GroupId, on: ConnectionId) {
        let came = timeout(Duration::from_secs(20), async {
            loop {
                self.signals.clear();
                ana.signal(group, self.identity.public_key(), vec![5; 300])
                    .await
                    .unwrap();
                let _ = timeout(Duration::from_secs(1), async {
                    while self.signals.is_empty() {
                        self.step().await;
                    }
                })
                .await;
                if self.signals == [on] {
                    return;
                }
            }
        })
        .await;
        assert!(came.is_ok(), "no signal came on {on:?}");
        self.signals.clear();
    }

    /// Keeps Bea's side going for `time`.
    async fn run_for(&mut self, time: Duration) {
        let _ = timeout(time, async {
            loop {
                self.step().await;
            }
        })
        .await;
    }

    /// Has `ana` send Bea `count` signals, and keeps Bea's side going until they all came
    /// (or for 10 s).
    async fn take_signals(&mut self, ana: &NodeHandle, group: GroupId, count: usize) {
        self.signals.clear();
        for _ in 0..count {
            ana.signal(group, self.identity.public_key(), vec![5; 300])
                .await
                .unwrap();
        }
        let _ = timeout(Duration::from_secs(10), async {
            while self.signals.len() < count {
                self.step().await;
            }
        })
        .await;
    }
}

/// Ana, and Bea by hand connected to her over TCP first and over QUIC then, a member at
/// Ana's; with Bea's TCP and QUIC connections.
async fn ana_and_bea_two_ways() -> (
    NodeHandle,
    NodeEvents,
    Bea,
    GroupId,
    ConnectionId,
    ConnectionId,
) {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let identity = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "ours", 1).unwrap();
    log.add(&ana, identity.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let keys = log.current_keys(&identity).unwrap();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana, log, port_a, vec![]);
    let ana_peer = node_a.local_peer();
    let mut bea = Bea {
        swarm: by_hand(),
        identity,
        keys,
        signals: Vec::new(),
        banners: Vec::new(),
        hold: HashSet::new(),
        hold_banners: false,
        held: Vec::new(),
    };
    let tcp = format!("/ip4/127.0.0.1/tcp/{port_a}").parse().unwrap();
    let tcp = bea.connect(ana_peer, tcp).await;
    let quic = bea.connect(ana_peer, loopback(port_a)).await;
    let device = bea.identity.public_key();
    let member = timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                _ = bea.step() => {}
                event = events_a.recv() => match event {
                    Some(NodeEvent::MemberConnected { device: d, .. }) if d == device => return,
                    None => panic!("Ana's node stopped"),
                    _ => {}
                },
            }
        }
    })
    .await;
    assert!(member.is_ok(), "Bea is not a member at Ana's");
    // Bea may have proved herself on the TCP connection before Ana's side counts the QUIC
    // one open (it completes its handshake a moment after Bea's side does).
    bea.until_signal_on(&node_a, group, quic).await;
    (node_a, events_a, bea, group, tcp, quic)
}

/// Signals in a row to a member reached over TCP and over QUIC all go on its best connection,
/// the QUIC one, not on whichever libp2p picks: it took the two in turn, so about every
/// other signal went the slower way, or on a connection about to close.
#[tokio::test(flavor = "multi_thread")]
async fn signals_in_a_row_all_go_on_the_best_connection() {
    let (node_a, _events_a, mut bea, group, _tcp, quic) = ana_and_bea_two_ways().await;
    bea.take_signals(&node_a, group, 24).await;
    assert_eq!(bea.signals.len(), 24, "signals that came");
    assert!(
        bea.signals.iter().all(|c| *c == quic),
        "came on {:?}; the QUIC connection is {quic:?}",
        bea.signals
    );
}

/// Once the best connection to a member closes, its signals go on the one left, at once.
#[tokio::test(flavor = "multi_thread")]
async fn signals_go_on_the_connection_left_once_the_best_one_closed() {
    let (node_a, _events_a, mut bea, group, tcp, quic) = ana_and_bea_two_ways().await;
    bea.take_signals(&node_a, group, 4).await;
    assert_eq!(bea.signals, [quic; 4]);

    assert!(bea.swarm.close_connection(quic));
    bea.wait_closed(quic).await;
    // Ana's side hears of it a moment later: a signal sent before is lost with the connection.
    bea.until_signal_on(&node_a, group, tcp).await;
    let started = std::time::Instant::now();
    bea.take_signals(&node_a, group, 12).await;
    assert_eq!(bea.signals, [tcp; 12]);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
}

/// A little past `SUSPECT_AFTER` (crates/fogata-net/src/node/outbox.rs).
const PAST_SUSPECT: Duration = Duration::from_millis(3500);

/// The best connection to a member stops answering while another lives (here the member
/// holds the answers to the signals that come on its QUIC connection): once a signal has
/// waited there for 3 s, the next ones go on TCP, and once the held ones are answered,
/// on QUIC again. Nothing is sent twice.
#[tokio::test(flavor = "multi_thread")]
async fn signals_leave_a_best_connection_that_stopped_answering() {
    let (node_a, _events_a, mut bea, group, tcp, quic) = ana_and_bea_two_ways().await;
    bea.hold.insert(quic);
    bea.take_signals(&node_a, group, 2).await;
    assert_eq!(bea.signals, [quic; 2]);
    bea.run_for(PAST_SUSPECT).await;
    bea.take_signals(&node_a, group, 4).await;
    assert_eq!(bea.signals, [tcp; 4]);

    bea.release();
    bea.until_signal_on(&node_a, group, quic).await;
    bea.take_signals(&node_a, group, 6).await;
    assert_eq!(bea.signals, [quic; 6]);
}

/// A large request slow to be answered on the best connection (a group banner of 200 KiB,
/// whose answer the member holds) does not make it suspect: signals still go on it.
#[tokio::test(flavor = "multi_thread")]
async fn a_large_slow_request_does_not_make_the_best_connection_suspect() {
    let (node_a, _events_a, mut bea, group, _tcp, quic) = ana_and_bea_two_ways().await;
    let mut picture = b"\x89PNG\r\n\x1a\n".to_vec();
    picture.resize(200 * 1024, 7);
    node_a.set_group_banner(group, picture).await.unwrap();
    bea.hold_banners = true;
    // Bea's roster ask names no banner: Ana sends hers.
    let roster = MemberRequest::Roster {
        group_id: group.as_bytes().to_vec(),
        heads: None,
        banner: Some(serde_bytes::ByteBuf::new()),
        picture: None,
    };
    bea.swarm
        .behaviour_mut()
        .send_request(&node_a.local_peer(), roster);
    let came = timeout(Duration::from_secs(10), async {
        while bea.banners.is_empty() {
            bea.step().await;
        }
    })
    .await;
    assert!(came.is_ok(), "no banner came");
    assert_eq!(bea.banners, [quic]);
    bea.run_for(PAST_SUSPECT).await;
    bea.take_signals(&node_a, group, 4).await;
    assert_eq!(bea.signals, [quic; 4]);
    bea.release();
}

/// When every connection to a member is suspect, its requests go on the best one, as
/// without the guard.
#[tokio::test(flavor = "multi_thread")]
async fn signals_go_on_the_best_connection_when_every_one_is_suspect() {
    let (node_a, _events_a, mut bea, group, tcp, quic) = ana_and_bea_two_ways().await;
    bea.hold.insert(quic);
    bea.take_signals(&node_a, group, 1).await;
    assert_eq!(bea.signals, [quic]);
    bea.run_for(PAST_SUSPECT).await;
    bea.hold.insert(tcp);
    bea.take_signals(&node_a, group, 1).await;
    assert_eq!(bea.signals, [tcp]);
    bea.run_for(PAST_SUSPECT).await;
    bea.take_signals(&node_a, group, 1).await;
    assert_eq!(bea.signals, [quic]);
    bea.release();
}

/// A request sent on a connection that closed goes on one still open, as libp2p would
/// choose, rather than failing or waiting: what member requests rely on when the connection
/// chosen for one closed a moment before (vendor/libp2p-request-response/FOGATA.md).
#[tokio::test(flavor = "multi_thread")]
async fn a_request_on_a_closed_connection_goes_on_another() {
    let mut asker = by_hand();
    let mut answerer = by_hand();
    let port = free_port();
    answerer.listen_on(loopback(port)).unwrap();
    let answerer_peer = *answerer.local_peer_id();
    let answering = tokio::spawn(async move {
        loop {
            if let SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Request { channel, .. },
                ..
            }) = answerer.select_next_some().await
            {
                let _ = answerer
                    .behaviour_mut()
                    .send_response(channel, MemberResponse::Ack);
            }
        }
    });
    let mut open = Vec::new();
    for _ in 0..2 {
        let dial = DialOpts::peer_id(answerer_peer)
            .addresses(vec![loopback(port)])
            .condition(PeerCondition::Always)
            .build();
        open.push(dial.connection_id());
        asker.dial(dial).unwrap();
    }
    let mut established = 0;
    timeout(Duration::from_secs(20), async {
        while established < 2 {
            if let SwarmEvent::ConnectionEstablished { .. } = asker.select_next_some().await {
                established += 1;
            }
        }
    })
    .await
    .expect("two connections");
    let (gone, left) = (open[0], open[1]);
    assert!(asker.close_connection(gone));
    timeout(Duration::from_secs(10), async {
        loop {
            if let SwarmEvent::ConnectionClosed { connection_id, .. } =
                asker.select_next_some().await
                && connection_id == gone
            {
                return;
            }
        }
    })
    .await
    .expect("closed");
    for _ in 0..4 {
        let presence = MemberRequest::Presence {
            hidden: false,
            away: false,
        };
        asker
            .behaviour_mut()
            .send_request_on(&answerer_peer, gone, presence);
    }
    let mut answered = 0;
    timeout(Duration::from_secs(10), async {
        while answered < 4 {
            match asker.select_next_some().await {
                SwarmEvent::Behaviour(request_response::Event::Message {
                    connection_id,
                    message: request_response::Message::Response { .. },
                    ..
                }) => {
                    assert_eq!(connection_id, left);
                    answered += 1;
                }
                SwarmEvent::Behaviour(request_response::Event::OutboundFailure {
                    error, ..
                }) => panic!("a request failed: {error}"),
                _ => {}
            }
        }
    })
    .await
    .expect("all four answered");
    answering.abort();
}
