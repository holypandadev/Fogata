//! Capability levels: two members learn each other's level when they connect, the one that
//! was dialled too (it lists its protocols to a peer once that proves membership), and a
//! device once seen at a level is never taken for an older one. Nodes in one process, over
//! the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey};
use fogata_net::{
    GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle,
    caps::{CAPS_LEVEL, Decision},
};
use fogata_state::GroupLog;
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

/// Waits for an event `done` takes, for at most `seconds`.
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
async fn members_learn_each_others_level_and_never_take_it_lower() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    let device: DevicePublicKey = bea.public_key();
    let mut peer = None;
    let mut level = None;
    for _ in 0..6 {
        let got = seen(&mut events_a, 10, |e| match e {
            NodeEvent::MemberConnected {
                device: d, peer: p, ..
            } if *d == device => {
                peer = Some(*p);
                level.is_some()
            }
            NodeEvent::LevelSeen {
                device: d,
                level: l,
            } if *d == device => {
                level = Some(*l);
                peer.is_some()
            }
            _ => false,
        })
        .await;
        if got {
            break;
        }
        node_b.dial(vec![loopback(port_a)], None).await.unwrap();
    }
    let peer = peer.expect("Bea connected");
    assert_eq!(level, Some(CAPS_LEVEL), "Ana learnt Bea's level");
    // Bea dialled Ana: Ana lists her protocols to Bea once Bea proved she is a member.
    let ana_device = ana.public_key();
    assert!(
        seen(&mut events_b, 20, |e| matches!(
            e,
            NodeEvent::LevelSeen { device, level } if *device == ana_device && *level == CAPS_LEVEL
        ))
        .await,
        "Bea learnt Ana's level"
    );
    assert_eq!(node_a.peer_level(&peer), Some(CAPS_LEVEL));
    assert_eq!(node_a.level_seen(&device), CAPS_LEVEL);
    // A feature of this level is used; one of the next level is not, the older way goes.
    assert_eq!(node_a.decide(&peer, &device, CAPS_LEVEL), Decision::Use);
    assert_eq!(
        node_a.decide(&peer, &device, CAPS_LEVEL + 1),
        Decision::Older
    );
    // Seen at the next level in an earlier session (kept by the application): the same
    // device saying it is older now gets neither.
    node_a.remember_levels([(device, CAPS_LEVEL + 1)]);
    assert_eq!(
        node_a.decide(&peer, &device, CAPS_LEVEL + 1),
        Decision::Refuse
    );
    // A lower level remembered never replaces a higher one.
    node_a.remember_levels([(device, 1)]);
    assert_eq!(node_a.level_seen(&device), CAPS_LEVEL + 1);
    // A device nobody saw is of the first level.
    let other = DeviceIdentity::generate().unwrap().public_key();
    assert_eq!(node_a.level_seen(&other), 1);
}
