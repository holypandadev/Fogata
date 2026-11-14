//! Members start a new key epoch when someone who is no longer a member still holds the
//! current secret. Two or three nodes in one process, over the loopback interface.

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

async fn wait_for(
    events: &mut NodeEvents,
    seconds: u64,
    what: &str,
    done: impl FnMut(&NodeEvent) -> bool,
) {
    assert!(
        seen(events, seconds, done).await,
        "timed out waiting for {what}"
    );
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

/// Waits until `node`'s log of `group` has `epochs` epochs.
async fn wait_for_epochs(
    node: &NodeHandle,
    events: &mut NodeEvents,
    epochs: usize,
    seconds: u64,
) -> GroupLog {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    loop {
        let log = node.group(group_of(node).await).await.unwrap().unwrap();
        if log.state().epochs.len() >= epochs {
            return log;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no new epoch after {seconds} s"
        );
        let _ = timeout(Duration::from_secs(2), events.recv()).await;
    }
}

async fn group_of(node: &NodeHandle) -> fogata_crypto::GroupId {
    node.groups().await.unwrap()[0].group_id
}

#[tokio::test(flavor = "multi_thread")]
async fn leaving_makes_the_first_member_rotate_the_key() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let first = log.state().current_epoch().unwrap().id;

    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), log.clone(), 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;

    // Bea leaves: Ana, the only member left with the key, rotates at once.
    let mut leaving = log.clone();
    leaving.append(&bea, Body::Leave, 3).unwrap();
    node_b.merge_group(leaving).await.unwrap();
    wait_for(
        &mut events_b,
        30,
        "Bea to be out",
        |e| matches!(e, NodeEvent::Removed { group: g, .. } if *g == group),
    )
    .await;
    let after = wait_for_epochs(&node_a, &mut events_a, 2, 30).await;
    let state = after.state();
    assert!(!state.is_member(&bea.public_key()));
    assert!(state.current_secret_leaked_to().is_empty());
    assert_ne!(state.current_epoch().unwrap().id, first);
    assert!(after.current_keys(&ana).is_some());
    assert!(after.current_keys(&bea).is_none());
    // Nobody rotated twice.
    assert_eq!(state.epochs.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_first_member_the_others_rotate_after_a_while() {
    let _one = ONE_AT_A_TIME.lock().await;
    // Ana (admitted first) is offline; Bea leaves; Luis rotates after a couple of heartbeats.
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let luis = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, luis.public_key(), "luis", 2).unwrap();
    log.add(&ana, bea.public_key(), "bea", 3).unwrap();
    log.append(&bea, Body::Leave, 4).unwrap();
    assert_eq!(log.state().current_holders_in_order()[0], ana.public_key());

    let port = free_port();
    let (node_l, mut events_l) = node(luis.clone(), log.clone(), port, Vec::new());
    let before = node_l.group(log.group_id()).await.unwrap().unwrap();
    assert_eq!(before.state().epochs.len(), 1);
    let after = wait_for_epochs(&node_l, &mut events_l, 2, 90).await;
    assert!(after.state().current_secret_leaked_to().is_empty());
    assert!(after.current_keys(&luis).is_some());
    assert!(after.current_keys(&bea).is_none());
    // Ana gets the new key through the log when she comes back (sealed to her too).
    assert!(after.current_keys(&ana).is_some());
}
