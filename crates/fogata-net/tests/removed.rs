//! A device removed while away is shown its removal and nothing after, and gets no proof
//! made with a key it still holds; a member that was away too catches up, however many key
//! rotations it missed. Nodes in one process, over the loopback interface.

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
async fn a_device_removed_while_away_learns_it_and_no_more() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let luis = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let eva = DeviceIdentity::generate().unwrap();
    let mut before = GroupLog::create(&ana, "g", 1).unwrap();
    before.add(&ana, luis.public_key(), "luis", 2).unwrap();
    before.add(&ana, bea.public_key(), "bea", 3).unwrap();
    let group = before.group_id();
    // While Luis and Bea were away, Ana removed Bea, let Eva in and renamed the group.
    let mut now = before.clone();
    now.remove(&ana, bea.public_key(), 4).unwrap();
    now.add(&ana, eva.public_key(), "eva", 5).unwrap();
    now.append(&ana, Body::Rename { name: "h".into() }, 6)
        .unwrap();

    let port_a = free_port();
    let (_node_a, mut events_a) = node(ana.clone(), now.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), before.clone(), 0, vec![loopback(port_a)]);
    let (mut removed, mut proved, mut shown) = (false, false, None);
    for _ in 0..6 {
        removed = seen(&mut events_b, 10, |e| {
            if let NodeEvent::Removed { log, .. } = e {
                shown = Some(log.clone());
            }
            proved |= matches!(e, NodeEvent::MemberConnected { device, .. } if *device == ana.public_key());
            matches!(e, NodeEvent::Removed { group: g, .. } if *g == group)
        })
        .await;
        if removed {
            break;
        }
        node_b.dial(vec![loopback(port_a)], None).await.unwrap();
    }
    assert!(removed, "Bea never learnt she was removed");
    assert!(!proved, "Ana proved herself to Bea with the old key");
    // What Bea was shown stops at her removal.
    let state = shown.unwrap().state();
    assert!(!state.is_member(&bea.public_key()));
    assert!(!state.is_member(&eva.public_key()));
    assert_eq!(state.name, "g");
    // Ana never took her for a member.
    while let Ok(event) = events_a.try_recv() {
        assert!(
            !matches!(event, NodeEvent::MemberConnected { device, .. } if device == bea.public_key()),
            "Ana took Bea for a member"
        );
    }

    // Luis was away too, with the same old key, but he is still in: he catches up.
    let (node_l, mut events_l) = node(luis.clone(), before.clone(), 0, vec![loopback(port_a)]);
    wait_to_connect(&node_l, loopback(port_a), &mut events_a, luis.public_key()).await;
    let log = wait_for_epochs(&node_l, &mut events_l, 2, 60).await;
    let state = log.state();
    assert!(!state.is_member(&bea.public_key()));
    assert!(state.is_member(&eva.public_key()));
    assert!(log.current_keys(&luis).is_some());
    assert!(log.current_keys(&bea).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_away_for_many_rotations_catches_up() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    // Bea goes away here, and the key changes six times without her.
    let beas = log.clone();
    for i in 0..6 {
        log.rotate(&ana, 10 + i).unwrap();
    }
    let port_a = free_port();
    let (_node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), beas, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    let caught_up = wait_for_epochs(&node_b, &mut events_b, 7, 60).await;
    assert!(caught_up.current_keys(&bea).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_removed_while_online_learns_it_at_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), log.clone(), 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    let mut now = node_a.group(group).await.unwrap().unwrap();
    now.remove(&ana, bea.public_key(), 10).unwrap();
    node_a.merge_group(now).await.unwrap();
    // Nobody dials again here: the removal reaches her on the connection she has.
    assert!(
        seen(
            &mut events_b,
            10,
            |e| matches!(e, NodeEvent::Removed { group: g, .. } if *g == group)
        )
        .await,
        "Bea was not told she was removed"
    );
}
