//! Two members verify each other at the same time, so one may hear the other's profile and
//! whether it appears offline before it has checked who that is. A member here plays the
//! other side by hand and sends both before it proves itself: they still count once it has.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, MembershipProof};
use fogata_friends::FriendBook;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent};
use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse, WireProof};
use fogata_state::GroupLog;
use futures::StreamExt;
use libp2p::{
    StreamProtocol,
    request_response::{self, ProtocolSupport},
    swarm::SwarmEvent,
};
use tokio::time::timeout;

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_profile_and_presence_sent_before_the_proof_still_count() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    let keys = log.current_keys(&bea).unwrap();
    let profile = FriendBook::new(&bea, "Beatriz", 7)
        .unwrap()
        .profile
        .encode();

    // Bea's side, by hand.
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
    let port_b = free_port();
    swarm.listen_on(loopback(port_b)).unwrap();
    let bea_peer = swarm.local_peer_id().to_bytes();

    let (_node_a, mut events_a) = fogata_net::start(NodeConfig {
        identity: ana,
        groups: vec![GroupSetup::Member(log)],
        friends: None,
        listen_port: 0,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers: vec![loopback(port_b)],
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap();

    // Ana challenges Bea; Bea first says she appears offline and sends her new profile,
    // waits for both answers, and only then proves herself.
    let mut held = None;
    let mut answered = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline {
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
            }) => match request {
                MemberRequest::Challenge { challenge, .. } if held.is_none() => {
                    let behaviour = swarm.behaviour_mut();
                    behaviour.send_request(
                        &peer,
                        MemberRequest::Presence {
                            hidden: true,
                            away: false,
                        },
                    );
                    behaviour.send_request(
                        &peer,
                        MemberRequest::Profile {
                            profile: profile.clone(),
                            animation: None,
                        },
                    );
                    held = Some((peer, challenge, channel));
                }
                _ => {
                    let _ = swarm
                        .behaviour_mut()
                        .send_response(channel, MemberResponse::Refused);
                }
            },
            SwarmEvent::Behaviour(request_response::Event::Message {
                message: request_response::Message::Response { .. },
                ..
            }) => {
                answered += 1;
                if answered == 2 {
                    let (peer, challenge, channel) = held.take().unwrap();
                    let challenge: [u8; 32] = challenge.try_into().unwrap();
                    let ana_peer = peer.to_bytes();
                    let proof =
                        MembershipProof::create(&keys, &bea, &bea_peer, &ana_peer, &challenge);
                    let sealed = proof.seal(&keys, &bea_peer, &ana_peer, &challenge).unwrap();
                    let _ = swarm
                        .behaviour_mut()
                        .send_response(channel, MemberResponse::Proof(WireProof::from(&sealed)));
                    break;
                }
            }
            _ => {}
        }
    }
    assert_eq!(answered, 2, "Ana answered the presence and the profile");
    // Keep Bea's side running while Ana takes the proof.
    let (mut hidden, mut named) = (false, false);
    let _ = timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                _ = swarm.select_next_some() => {}
                event = events_a.recv() => match event {
                    Some(NodeEvent::MemberPresence { hidden: true, .. }) => hidden = true,
                    Some(NodeEvent::MemberProfile { profile }) if profile.name == "Beatriz" => {
                        named = true
                    }
                    None => return,
                    _ => {}
                },
            }
            if hidden && named {
                return;
            }
        }
    })
    .await;
    assert!(hidden, "Bea appears offline at Ana's");
    assert!(named, "Ana has Bea's new name");
}

/// A peer this device is checking sends a profile that is no profile, as large as a message
/// may be: it is refused at once rather than kept until the peer proves itself, which it
/// may never do.
#[tokio::test(flavor = "multi_thread")]
async fn a_profile_that_does_not_decode_is_not_kept_before_the_proof() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    let profile = FriendBook::new(&bea, "Beatriz", 7)
        .unwrap()
        .profile
        .encode();

    // The other side, by hand, with a codec that writes what Ana's reads (up to 4 MiB).
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
    let port_b = free_port();
    swarm.listen_on(loopback(port_b)).unwrap();

    let (_node_a, _events_a) = fogata_net::start(NodeConfig {
        identity: ana,
        groups: vec![GroupSetup::Member(log)],
        friends: None,
        listen_port: 0,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers: vec![loopback(port_b)],
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap();

    // Once challenged, junk of 3 MiB as a profile, then a real one; neither proves anything.
    let mut held = None;
    let mut answers = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline && answers.len() < 2 {
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
            }) => match request {
                MemberRequest::Challenge { .. } if held.is_none() => {
                    let behaviour = swarm.behaviour_mut();
                    let junk = MemberRequest::Profile {
                        profile: vec![7; 3 * 1024 * 1024],
                        animation: None,
                    };
                    let junk = behaviour.send_request(&peer, junk);
                    let real = MemberRequest::Profile {
                        profile: profile.clone(),
                        animation: Some(serde_bytes::ByteBuf::from(vec![9; 1024])),
                    };
                    let real = behaviour.send_request(&peer, real);
                    held = Some((channel, junk, real));
                }
                _ => {
                    let _ = swarm
                        .behaviour_mut()
                        .send_response(channel, MemberResponse::Refused);
                }
            },
            SwarmEvent::Behaviour(request_response::Event::Message {
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
                ..
            }) => answers.push((request_id, response)),
            _ => {}
        }
    }
    let (_channel, junk, real) = held.expect("Ana challenged Bea");
    let answer = |id| {
        answers
            .iter()
            .find(|(r, _)| *r == id)
            .map(|(_, a)| format!("{a:?}"))
    };
    assert_eq!(answer(junk).as_deref(), Some("Refused"), "junk was kept");
    assert_eq!(
        answer(real).as_deref(),
        Some("Ack"),
        "a real profile is kept"
    );
}
