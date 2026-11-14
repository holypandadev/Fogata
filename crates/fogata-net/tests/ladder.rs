//! The ladder between nodes in one process, over the loopback interface: of two renames made
//! apart, the one by the member who came in first stands on both; what a member moved down
//! did at the same time is undone once the two logs meet.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::{Body, GroupLog};
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

/// Waits for `device`, which dials `address`, to connect to the node of `events`. A dial
/// by address alone is not tried again, and on a busy machine its handshake can run out of
/// time, so it is dialed again now and then.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = seen(
            events,
            10,
            |e| matches!(e, NodeEvent::MemberConnected { device: d, .. } if *d == device),
        )
        .await;
        if connected {
            return;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    panic!("timed out waiting for the member to connect");
}

/// Waits until `node`'s log of `group` passes `done`.
async fn until(
    node: &NodeHandle,
    group: fogata_crypto::GroupId,
    seconds: u64,
    what: &str,
    done: impl Fn(&GroupLog) -> bool,
) -> GroupLog {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let log = node.group(group).await.unwrap().unwrap();
        if done(&log) {
            return log;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out: {what}");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Ana's group with Bea, Carlos and Dani, in that order.
fn four() -> (GroupLog, Vec<Arc<DeviceIdentity>>) {
    let ids: Vec<Arc<DeviceIdentity>> = (0..4)
        .map(|_| Arc::new(DeviceIdentity::generate().unwrap()))
        .collect();
    let mut log = GroupLog::create(&ids[0], "Sundays", 1).unwrap();
    for (i, d) in ids.iter().enumerate().skip(1) {
        log.add(&ids[0], d.public_key(), "m", 1 + i as u64).unwrap();
    }
    (log, ids)
}

#[tokio::test(flavor = "multi_thread")]
async fn of_two_renames_made_apart_the_one_who_came_in_first_stands_everywhere() {
    let _one = ONE_AT_A_TIME.lock().await;
    let (base, ids) = four();
    let (bea, carlos) = (ids[1].clone(), ids[2].clone());
    let group = base.group_id();
    let (mut at_bea, mut at_carlos) = (base.clone(), base.clone());
    at_carlos
        .append(
            &carlos,
            Body::Rename {
                name: "Carlos".into(),
            },
            10,
        )
        .unwrap();
    at_bea
        .append(&bea, Body::Rename { name: "Bea".into() }, 10)
        .unwrap();
    let port_b = free_port();
    let (node_b, mut events_b) = node(bea.clone(), at_bea, port_b, Vec::new());
    let (node_c, _events_c) = node(carlos.clone(), at_carlos, 0, vec![loopback(port_b)]);
    wait_to_connect(
        &node_c,
        loopback(port_b),
        &mut events_b,
        carlos.public_key(),
    )
    .await;
    for node in [&node_b, &node_c] {
        let log = until(node, group, 60, "both renames", |l| {
            l.len() == base.len() + 2
        })
        .await;
        assert_eq!(log.state().name, "Bea");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn what_someone_moved_down_did_at_once_is_undone_when_the_logs_meet() {
    let _one = ONE_AT_A_TIME.lock().await;
    let (base, ids) = four();
    let (ana, bea, dani) = (ids[0].clone(), ids[1].clone(), ids[3].clone());
    let group = base.group_id();
    // Ana, alone, sends Bea to the end; Bea, away from Ana, removes Dani and renames.
    let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
    at_ana
        .append(
            &ana,
            Body::Demote {
                member: bea.public_key(),
            },
            10,
        )
        .unwrap();
    at_bea.remove(&bea, dani.public_key(), 11).unwrap();
    at_bea
        .append(
            &bea,
            Body::Rename {
                name: "Taken".into(),
            },
            12,
        )
        .unwrap();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), at_ana, port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), at_bea, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    for node in [&node_a, &node_b] {
        let log = until(node, group, 60, "the logs met", |l| {
            l.len() == base.len() + 3
        })
        .await;
        let state = log.state();
        assert!(state.is_member(&dani.public_key()), "Dani is back");
        assert_eq!(state.name, "Sundays");
        assert_eq!(state.ladder.last(), Some(&bea.public_key()));
        // The key of Bea's removal is never used: Dani can still read the group.
        assert!(log.current_keys(&dani).is_some());
    }
}
