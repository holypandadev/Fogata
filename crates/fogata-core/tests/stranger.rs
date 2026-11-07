//! A stranger on the same network asks a member to prove its membership. Without the
//! group's key it cannot name the group, and it gets neither the member's device key nor a
//! yes or no. The member's own challenges never name the group either.

use std::time::Duration;

use fogata_core::{Core, NetOptions};
use fogata_crypto::SEALED_PROOF_LEN;
use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse};
use futures::StreamExt;
use libp2p::{
    Multiaddr, StreamProtocol,
    request_response::{self, ProtocolSupport},
    swarm::{SwarmEvent, dial_opts::DialOpts},
};
use serde_bytes::ByteBuf;

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stranger_learns_nothing() {
    let base = std::env::temp_dir().join(format!("fogata-stranger-{}", std::process::id()));
    let dir = base.join("ana");
    Core::create_profile(&dir, "Ana", None).unwrap();
    let port = free_port();
    let (ana, _events) = Core::start(
        dir,
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    // Ana listens before she is dialled.
    let listening = tokio::time::timeout(Duration::from_secs(10), async {
        while std::net::TcpListener::bind(("0.0.0.0", port)).is_ok() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(listening.is_ok(), "Ana does not listen on {port}");

    // A bare libp2p node that speaks only the member protocol: no key, no membership.
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
    let addr: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap();
    swarm
        .dial(DialOpts::unknown_peer_id().address(addr).build())
        .unwrap();

    let mut asked = false;
    let mut answers = Vec::new();
    let mut challenges = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut settle = None;
    while tokio::time::Instant::now() < deadline
        && settle.is_none_or(|at| tokio::time::Instant::now() < at)
    {
        if answers.len() == 2 && settle.is_none() {
            // Long enough for a challenge back to arrive, if one were sent.
            settle = Some(tokio::time::Instant::now() + Duration::from_secs(2));
        }
        let until = settle.unwrap_or(deadline).min(deadline);
        let Ok(event) = tokio::time::timeout_at(until, swarm.select_next_some()).await else {
            continue;
        };
        match event {
            SwarmEvent::ConnectionEstablished { peer_id, .. } if !asked => {
                asked = true;
                // Made-up tags, then none at all.
                for tags in [vec![ByteBuf::from(vec![9u8; 16]); 4], Vec::new()] {
                    swarm.behaviour_mut().send_request(
                        &peer_id,
                        MemberRequest::Challenge {
                            challenge: vec![7u8; 32],
                            tags,
                        },
                    );
                }
            }
            SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Response { response, .. },
                ..
            }) => answers.push(response),
            SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Request { request, .. },
                ..
            }) => challenges.push(request),
            _ => {}
        }
    }
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(answers.len(), 2, "both challenges are answered");
    // Ana asks every new peer on her network about each of her groups, by tag only.
    assert!(
        !challenges.is_empty(),
        "Ana challenges a peer on her network"
    );
    for request in &challenges {
        let MemberRequest::Challenge { challenge, tags } = request else {
            panic!("only challenges, got {request:?}");
        };
        assert!(
            !tags
                .iter()
                .chain([&ByteBuf::from(challenge.clone())])
                .any(|bytes| bytes
                    .windows(group.as_bytes().len())
                    .any(|w| w == group.as_bytes())),
            "a challenge names the group"
        );
    }
    let me = ana.me();
    for answer in &answers {
        let MemberResponse::Proof(proof) = answer else {
            panic!("every answer looks like a proof, got {answer:?}");
        };
        assert_eq!(proof.ciphertext.len(), SEALED_PROOF_LEN);
        assert!(
            !proof.ciphertext.windows(32).any(|w| w == me.as_bytes()),
            "the device key is not in the answer"
        );
    }
}
