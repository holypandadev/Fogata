//! A device joining with an invite believes a refusal only when the manager who wrote the
//! invite signed it: a peer that learnt the group's id, or a member, cannot end the join by
//! saying no. Another manager's no reaches it once the issuing manager confirms it. Nodes in one
//! process, over the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, GroupId};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse};
use fogata_state::{Body, Entry, GroupLog, Ticket};
use futures::StreamExt;
use libp2p::{
    StreamProtocol,
    request_response::{self, ProtocolSupport},
    swarm::SwarmEvent,
};
use serde_bytes::ByteBuf;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

fn node(
    identity: Arc<DeviceIdentity>,
    setup: GroupSetup,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: vec![setup],
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

fn joining(ticket: &Ticket) -> GroupSetup {
    GroupSetup::Joining {
        group_id: ticket.group_id,
        ticket: ticket.clone(),
        rendezvous: [3; 32],
        name: "bea".into(),
    }
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

fn declined(group: GroupId) -> impl FnMut(&NodeEvent) -> bool {
    move |e| matches!(e, NodeEvent::JoinDeclined { group: g } if *g == group)
}

fn revoke(by: &DeviceIdentity, log: &GroupLog, ticket: &Ticket) -> Vec<u8> {
    let body = Body::Revoke {
        ticket_id: ticket.ticket_id,
    };
    Entry::sign(by, log.group_id(), log.heads(), 10, body).encode()
}

/// A group of Ana with Luis as a second manager, and an invite Ana wrote.
fn group_with_invite(ana: &DeviceIdentity, luis: &DeviceIdentity) -> (GroupLog, Ticket) {
    let mut log = GroupLog::create(ana, "g", 1).unwrap();
    log.add(ana, luis.public_key(), "luis", 2).unwrap();
    let up = Body::Promote {
        member: luis.public_key(),
    };
    log.append(ana, up, 3).unwrap();
    let (ticket, _) = log.invite(ana, u64::MAX / 2, 4).unwrap();
    (log, ticket)
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_inviting_manager_can_say_no() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let luis = DeviceIdentity::generate().unwrap();
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (log, ticket) = group_with_invite(&ana, &luis);
    let group = log.group_id();

    // Someone who learnt the invite answers it with a no, unsigned, then signed by itself
    // and by Luis, a real manager whose key Bea cannot tell from any other.
    let mallory = DeviceIdentity::generate().unwrap();
    let forged = [
        None,
        Some(revoke(&mallory, &log, &ticket)),
        Some(revoke(&luis, &log, &ticket)),
    ];
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
        .build();
    let port_m = free_port();
    swarm.listen_on(loopback(port_m)).unwrap();

    let (node_b, mut events_b) = node(bea.clone(), joining(&ticket), 0, Vec::new());
    // As in an invite link, with its peer id: Bea shows the invite to it.
    let to_m = loopback(port_m).with(libp2p::multiaddr::Protocol::P2p(*swarm.local_peer_id()));
    node_b.dial(vec![to_m], Some(group)).await.unwrap();
    let (mut shown, mut refused) = (false, 0);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline && !(shown && refused == forged.len()) {
        let Ok(event) = tokio::time::timeout_at(deadline, swarm.select_next_some()).await else {
            break;
        };
        match event {
            SwarmEvent::Behaviour(request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            }) => {
                let answer = if matches!(request, MemberRequest::JoinRequest { .. }) {
                    shown = true;
                    for revoke in &forged {
                        swarm.behaviour_mut().send_request(
                            &peer,
                            MemberRequest::JoinDeclined {
                                group_id: group.as_bytes().to_vec(),
                                revoke: revoke.clone().map(ByteBuf::from),
                            },
                        );
                    }
                    MemberResponse::JoinDeclined
                } else {
                    MemberResponse::Refused
                };
                let _ = swarm.behaviour_mut().send_response(channel, answer);
            }
            SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Response { response, .. },
                ..
            }) => {
                assert!(
                    matches!(response, MemberResponse::Refused),
                    "a no it cannot check is refused, got {response:?}"
                );
                refused += 1;
            }
            _ => {}
        }
    }
    assert!(shown, "Bea showed the invite to the peer it was given");
    assert_eq!(refused, forged.len(), "every made-up no was answered");
    assert!(
        !seen(&mut events_b, 3, declined(group)).await,
        "Bea believed a no the inviting manager did not sign"
    );

    // Ana, who wrote the invite, says no: that one counts.
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), GroupSetup::Member(log), port_a, Vec::new());
    node_b
        .dial(vec![loopback(port_a)], Some(group))
        .await
        .unwrap();
    assert!(
        seen(&mut events_a, 30, |e| matches!(
            e,
            NodeEvent::JoinAsked { device, .. } if *device == bea.public_key()
        ))
        .await,
        "Ana is asked"
    );
    node_a
        .answer_join(group, bea.public_key(), false)
        .await
        .unwrap();
    assert!(
        seen(&mut events_b, 20, declined(group)).await,
        "Bea learns Ana's no"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_managers_no_counts_once_the_inviting_manager_confirms_it() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let luis = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (log, ticket) = group_with_invite(&ana, &luis);
    let group = log.group_id();

    // Bea only reaches Luis, who says no while Ana is away.
    let port_l = free_port();
    let (node_l, mut events_l) = node(
        luis.clone(),
        GroupSetup::Member(log.clone()),
        port_l,
        Vec::new(),
    );
    let (node_b, mut events_b) = node(bea.clone(), joining(&ticket), 0, Vec::new());
    node_b
        .dial(vec![loopback(port_l)], Some(group))
        .await
        .unwrap();
    assert!(
        seen(&mut events_l, 30, |e| matches!(
            e,
            NodeEvent::JoinAsked { device, .. } if *device == bea.public_key()
        ))
        .await,
        "Luis is asked"
    );
    node_l
        .answer_join(group, bea.public_key(), false)
        .await
        .unwrap();
    assert!(
        !seen(&mut events_b, 5, declined(group)).await,
        "Bea cannot check Luis's no yet"
    );

    // Ana comes back, takes Luis's `Revoke` and confirms it; Bea, still showing the invite
    // to Luis, gets Ana's.
    let (_node_a, _events_a) = node(
        ana.clone(),
        GroupSetup::Member(log),
        0,
        vec![loopback(port_l)],
    );
    assert!(
        seen(&mut events_b, 90, declined(group)).await,
        "Bea learns the no once Ana confirms it"
    );
    let now = node_l.group(group).await.unwrap().unwrap();
    assert!(now.issuer_revoke(&ticket).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_join_ends_when_its_invite_has_run_out() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = DeviceIdentity::generate().unwrap();
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let group = GroupId::random().unwrap();
    // Ran out long ago: nobody can let Bea in any more, and nobody may say so.
    let ticket = Ticket::issue(&ana, group, 1_000).unwrap();
    let (_node_b, mut events_b) = node(bea, joining(&ticket), 0, Vec::new());
    assert!(
        seen(&mut events_b, 45, |e| matches!(
            e,
            NodeEvent::JoinExpired { group: g } if *g == group
        ))
        .await,
        "the join ends"
    );
}

/// A group whose log is longer than one part of a sync takes (as one with years of history,
/// or pictures set with entries by earlier versions): the device let in gets it in parts,
/// one after the other, and is in.
#[tokio::test(flavor = "multi_thread")]
async fn a_long_log_reaches_a_device_let_in_part_after_part() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let luis = DeviceIdentity::generate().unwrap();
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (mut log, ticket) = group_with_invite(&ana, &luis);
    for i in 0..12u64 {
        let picture = Some(vec![i as u8; fogata_state::MAX_PICTURE_BYTES]);
        log.append(&ana, Body::SetPicture { picture }, 10 + i)
            .unwrap();
    }
    let group = log.group_id();
    let entries = log.len();
    let bytes: usize = log.entries().map(|e| e.encode().len()).sum();
    assert!(bytes > 2 * 256 * 1024, "{bytes} bytes");
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), GroupSetup::Member(log), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), joining(&ticket), 0, Vec::new());
    node_b
        .dial(vec![loopback(port_a)], Some(group))
        .await
        .unwrap();
    assert!(
        seen(&mut events_a, 30, |e| matches!(
            e,
            NodeEvent::JoinAsked { device, .. } if *device == bea.public_key()
        ))
        .await,
        "Ana is asked"
    );
    node_a
        .answer_join(group, bea.public_key(), true)
        .await
        .unwrap();
    assert!(
        seen(&mut events_b, 30, |e| matches!(
            e,
            NodeEvent::Joined { group: g, .. } if *g == group
        ))
        .await,
        "Bea got the whole log"
    );
    let at_bea = node_b.group(group).await.unwrap().unwrap();
    // Ana's log, her `Welcome` for Bea and Bea's join.
    assert!(at_bea.len() >= entries + 2, "{} of {entries}", at_bea.len());
    assert!(at_bea.state().is_member(&bea.public_key()));
}
