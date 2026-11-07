//! A device joining with an invite gets in, or learns it may not, although something is lost on
//! the way: an address in the invite that does not answer (another network's, or a connection
//! the system has not let go of yet), its ask, or the manager's answer, on a connection that
//! closes while another stays. Nodes in one process, over the loopback interface; the other side is
//! played by hand where a message has to be lost.

use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse};
use fogata_state::{Body, Entry, GroupLog, Ticket};
use futures::StreamExt;
use libp2p::{
    PeerId, StreamProtocol, Swarm,
    multiaddr::Protocol,
    request_response::{self, ProtocolSupport},
    swarm::{
        SwarmEvent,
        dial_opts::{DialOpts, PeerCondition},
    },
};
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

type Hand = Swarm<request_response::cbor::Behaviour<MemberRequest, MemberResponse>>;

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn node(identity: Arc<DeviceIdentity>, setup: GroupSetup, port: u16) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: vec![setup],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers: Vec::new(),
        remembered: Vec::new(),
        hidden: false,
    })
    .unwrap()
}

fn joining(ticket: &Ticket) -> GroupSetup {
    GroupSetup::Joining {
        group_id: ticket.group_id,
        ticket: ticket.clone(),
        rendezvous: [3; 32],
        name: "bea".into(),
    }
}

/// A group of Ana's with an invite she wrote.
fn group_with_invite(ana: &DeviceIdentity) -> (GroupLog, Ticket) {
    let mut log = GroupLog::create(ana, "g", 1).unwrap();
    let (ticket, _) = log.invite(ana, u64::MAX / 2, 2).unwrap();
    (log, ticket)
}

/// A device that speaks only the member protocol, listening at `port`, which keeps its
/// connections while nothing is said on them.
fn by_hand(port: u16) -> Hand {
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| {
            request_response::cbor::Behaviour::<MemberRequest, MemberResponse>::new(
                [(StreamProtocol::new(MEMBER_PROTOCOL), ProtocolSupport::Full)],
                request_response::Config::default(),
            )
        })
        .unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();
    swarm.listen_on(loopback(port)).unwrap();
    swarm
}

/// Opens two connections from `swarm` to `peer` at `port`, and waits for both.
async fn two_connections(swarm: &mut Hand, peer: PeerId, port: u16) {
    for _ in 0..2 {
        swarm
            .dial(
                DialOpts::peer_id(peer)
                    .addresses(vec![loopback(port)])
                    .condition(PeerCondition::Always)
                    .build(),
            )
            .unwrap();
    }
    let mut open = 0;
    timeout(Duration::from_secs(20), async {
        while open < 2 {
            if let SwarmEvent::ConnectionEstablished { peer_id, .. } =
                swarm.select_next_some().await
                && peer_id == peer
            {
                open += 1;
            }
        }
    })
    .await
    .expect("two connections");
}

/// Whether an event `done` takes comes within `seconds`.
async fn seen(
    events: &mut NodeEvents,
    seconds: u64,
    mut done: impl FnMut(&NodeEvent) -> bool,
) -> bool {
    timeout(Duration::from_secs(seconds), async {
        while let Some(event) = events.recv().await {
            if done(&event) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_join_reaches_the_manager_past_an_invite_address_that_does_not_answer() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (log, ticket) = group_with_invite(&ana);
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana, GroupSetup::Member(log), port_a);
    let (node_b, _events_b) = node(bea.clone(), joining(&ticket), 0);

    // The invite's first address refuses the connection; the next one is Ana's.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let ana_peer = node_a.local_peer();
    let addrs: Vec<Multiaddr> = [
        format!("/ip4/127.0.0.1/tcp/{closed}/p2p/{ana_peer}"),
        format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1/p2p/{ana_peer}"),
    ]
    .iter()
    .map(|a| a.parse().unwrap())
    .collect();
    node_b.dial(addrs, Some(group)).await.unwrap();
    assert!(
        seen(&mut events_a, 30, |e| matches!(
            e,
            NodeEvent::JoinAsked { device, .. } if *device == bea.public_key()
        ))
        .await,
        "Ana is asked"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ask_to_join_lost_on_a_connection_that_closed_is_made_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = DeviceIdentity::generate().unwrap();
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (_, ticket) = group_with_invite(&ana);
    let group = ticket.group_id;
    let port_m = free_port();
    let mut manager = by_hand(port_m);
    let port_b = free_port();
    let (node_b, _events_b) = node(bea, joining(&ticket), port_b);
    two_connections(&mut manager, node_b.local_peer(), port_b).await;
    let to_m = loopback(port_m).with(Protocol::P2p(*manager.local_peer_id()));
    node_b.dial(vec![to_m], Some(group)).await.unwrap();

    // The first ask goes unanswered: its connection closes, and the others stay.
    let mut asks = 0;
    let found = timeout(Duration::from_secs(60), async {
        loop {
            if let SwarmEvent::Behaviour(request_response::Event::Message {
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                connection_id,
                ..
            }) = manager.select_next_some().await
            {
                if !matches!(request, MemberRequest::JoinRequest { .. }) {
                    let _ = manager
                        .behaviour_mut()
                        .send_response(channel, MemberResponse::Refused);
                    continue;
                }
                asks += 1;
                if asks == 2 {
                    return;
                }
                drop(channel);
                manager.close_connection(connection_id);
            }
        }
    })
    .await;
    assert!(found.is_ok(), "the invite is shown again");
}

/// Ana answers Bea, who joins by hand, with a yes or a no: the first answer is lost with its
/// connection while another stays, and Ana sends it again.
async fn answer_lost_and_sent_again(accept: bool) {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = DeviceIdentity::generate().unwrap();
    let (log, ticket) = group_with_invite(&ana);
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana, GroupSetup::Member(log), port_a);
    let mut joiner = by_hand(free_port());
    let ana_peer = node_a.local_peer();
    two_connections(&mut joiner, ana_peer, port_a).await;

    // Bea shows the invite and signs her join on the heads Ana offers.
    let ask = MemberRequest::JoinRequest {
        ticket: ticket.encode(),
    };
    joiner.behaviour_mut().send_request(&ana_peer, ask);
    let mut answers = 0;
    let mut answered = 0;
    let mut asked = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(40);
    while answered < 2 {
        let event = tokio::select! {
            event = joiner.select_next_some() => event,
            Some(event) = events_a.recv(), if !asked => {
                if matches!(&event, NodeEvent::JoinAsked { device, .. } if *device == bea.public_key()) {
                    asked = true;
                    node_a.answer_join(group, bea.public_key(), accept).await.unwrap();
                }
                continue;
            }
            _ = tokio::time::sleep_until(deadline) => break,
        };
        match event {
            SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Response { response, .. },
                ..
            }) => {
                answers += 1;
                if let MemberResponse::JoinOffer { heads } = response {
                    let heads = heads
                        .iter()
                        .map(|h| <[u8; 32]>::try_from(h.as_slice()).unwrap())
                        .collect();
                    let body = Body::Join {
                        ticket: ticket.clone(),
                        name: "Bea".into(),
                    };
                    let entry = Entry::sign(&bea, group, heads, now_ms(), body);
                    let binding = entry.join_binding(
                        &bea,
                        &joiner.local_peer_id().to_bytes(),
                        &ana_peer.to_bytes(),
                    );
                    joiner.behaviour_mut().send_request(
                        &ana_peer,
                        MemberRequest::JoinCommit {
                            entry: entry.encode(),
                            binding: Some(binding.to_vec().into()),
                        },
                    );
                }
            }
            // The first answer is lost with its connection; the other stays.
            SwarmEvent::Behaviour(request_response::Event::Message {
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                connection_id,
                ..
            }) => {
                let answer = if accept {
                    matches!(request, MemberRequest::Admitted { .. })
                } else {
                    matches!(
                        request,
                        MemberRequest::JoinDeclined {
                            revoke: Some(_),
                            ..
                        }
                    )
                };
                if answer {
                    answered += 1;
                    if answered == 1 {
                        drop(channel);
                        joiner.close_connection(connection_id);
                        continue;
                    }
                    let _ = joiner
                        .behaviour_mut()
                        .send_response(channel, MemberResponse::Ack);
                } else {
                    let _ = joiner
                        .behaviour_mut()
                        .send_response(channel, MemberResponse::Refused);
                }
            }
            _ => {}
        }
    }
    assert!(asked, "Ana is asked ({answers} answers)");
    assert_eq!(answered, 2, "Ana sends her answer again");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_let_in_lost_on_a_connection_that_closed_is_sent_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    answer_lost_and_sent_again(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_lost_on_a_connection_that_closed_is_sent_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    answer_lost_and_sent_again(false).await;
}
