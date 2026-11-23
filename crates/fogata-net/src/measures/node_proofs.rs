//! Measurement of what strangers' challenges cost a node, run by hand (`--ignored`).

use crate::{
    NodeConfig,
    group::GroupSetup,
    node::{Node, start, tests::inspect},
};
use fogata_crypto::DeviceIdentity;
use fogata_state::GroupLog;
use libp2p::PeerId;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// What a challenge from a peer that is no member costs this device, with 50 groups of
/// one key epoch and of 300: one that names none of the newest keys (as any stranger's
/// can), and one that makes this device try every key it has (a few each second at
/// most, from all strangers together; see limits.rs). Run by hand, in a release build:
/// `cargo test --release -p fogata-net --lib strangers_challenges_cost -- --ignored
/// --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn strangers_challenges_cost() {
    use fogata_proto::MemberRequest;
    for epochs in [1u64, 300] {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let groups: Vec<GroupSetup> = (0..50)
            .map(|i| {
                let mut log = GroupLog::create(&ana, &format!("g{i}"), 1).unwrap();
                for k in 1..epochs {
                    log.rotate(&ana, 1 + k).unwrap();
                }
                GroupSetup::Member(log)
            })
            .collect();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups,
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap();
        let (newest, every) = inspect(&node, |n| {
            let challenge = |n: &mut Node, peer: PeerId| {
                let tags = (0..5)
                    .map(|_| serde_bytes::ByteBuf::from(vec![7u8; 32]))
                    .collect();
                let request = MemberRequest::Challenge {
                    challenge: vec![1; 32],
                    tags,
                };
                n.serving = Some(peer);
                let at = Instant::now();
                n.on_request(peer, request);
                n.serving = None;
                at.elapsed()
            };
            let median = |mut times: Vec<Duration>| {
                times.sort();
                times[times.len() / 2]
            };
            // Each new peer makes this device try every key, while the budget of all
            // strangers together lasts.
            let every: Vec<Duration> = (0..16).map(|_| challenge(n, PeerId::random())).collect();
            // Past it, only the newest.
            let peer = PeerId::random();
            let newest: Vec<Duration> = (0..400).map(|_| challenge(n, peer)).collect();
            (median(newest), median(every))
        })
        .await;
        println!(
            "50 groups of {epochs} epochs: {newest:?} per challenge from a stranger, \
             {every:?} per one that tries every key"
        );
        node.shutdown().await.unwrap();
    }
}
