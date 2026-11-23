//! A member flooding a node, run by hand (`--ignored`).

use super::*;
use crate::{Command, NodeConfig, group::GroupSetup, node::start};
use fogata_crypto::DeviceIdentity;
use fogata_state::{Body, GroupLog};
use libp2p::Multiaddr;
use std::sync::Arc;

/// A member sends this device message after message of log entries signed with its own
/// key that do not verify, and of group messages; meanwhile the node must go on answering.
/// Prints how long it took to answer at worst. Ignored by default: it is meant for release
/// builds, where a signature check costs what it costs on a real device
/// (`cargo test --release -p fogata-net -- --ignored flooding`).
#[ignore]
#[tokio::test(flavor = "multi_thread")]
async fn a_member_flooding_entries_and_messages_leaves_the_node_answering() {
    use fogata_proto::{MemberRequest, WireMessage};
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let config = |identity: Arc<DeviceIdentity>, port: u16, peers: Vec<Multiaddr>| NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log.clone())],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: false,
        peers,
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    };
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let at: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap();
    let (node_a, _events_a) = start(config(ana.clone(), port, Vec::new())).unwrap();
    let (node_b, mut events_b) = start(config(bea.clone(), 0, vec![at])).unwrap();
    let ana_peer = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match events_b.recv().await {
                Some(NodeEvent::MemberConnected { device, peer, .. })
                    if device == ana.public_key() =>
                {
                    return peer;
                }
                Some(_) => {}
                None => panic!("Bea's node stopped"),
            }
        }
    })
    .await
    .expect("Bea reached Ana");
    // As many of the smallest entries as a message carries, in Bea's name, each with a
    // signature that does not verify.
    let heads = log.heads();
    let entries: Vec<fogata_proto::SharedBytes> = (0..24_000u64)
        .map(|i| {
            let mut bytes = Entry::sign(&bea, group, heads.clone(), 10 + i, Body::Leave).encode();
            let n = bytes.len();
            bytes[n - 1] ^= 1;
            bytes.into()
        })
        .collect();
    let message = WireMessage {
        version: 1,
        group_id: group.as_bytes().to_vec(),
        epoch: 0,
        sender: bea.public_key().as_bytes().to_vec(),
        timestamp_ms: 1,
        message_id: vec![1; 16],
        nonce: vec![0; 24],
        ciphertext: vec![0; 150 * 1024],
        signature: vec![0; 64],
    };
    let flood = async {
        for round in 0..10u8 {
            let push = MemberRequest::Push {
                group_id: group.as_bytes().to_vec(),
                entries: entries.clone(),
            };
            node_b
                .commands
                .send(Command::Request(ana_peer, Box::new(push)))
                .await
                .unwrap();
            for k in 0..200u8 {
                let mut wire = message.clone();
                wire.message_id = vec![round, k, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
                let spread = MemberRequest::Spread {
                    message: wire,
                    reached: Vec::new(),
                };
                node_b
                    .commands
                    .send(Command::Request(ana_peer, Box::new(spread)))
                    .await
                    .unwrap();
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    };
    let watch = async {
        let mut worst = Duration::ZERO;
        let until = Instant::now() + Duration::from_secs(12);
        while Instant::now() < until {
            let asked = Instant::now();
            node_a.addresses().await.unwrap();
            worst = worst.max(asked.elapsed());
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        worst
    };
    let ((), worst) = tokio::join!(flood, watch);
    eprintln!("longest wait for an answer while flooded: {worst:?}");
    assert!(worst < Duration::from_secs(1), "{worst:?}");
    let kept = node_a.group(group).await.unwrap().unwrap();
    assert_eq!(kept.len(), log.len());
}
