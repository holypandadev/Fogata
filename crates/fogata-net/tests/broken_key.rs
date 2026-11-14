//! A member that starts a key epoch nobody else can open (boxes that open to nothing) must
//! not lock the group out: a manager left without the key makes a new one at once, and
//! members still prove themselves to each other with an older key only members hold. Nodes
//! in one process, over the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, Sealed};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::{Body, GroupLog, Rotation};
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

/// Whether `device`, which dials `address`, connects to the node of `events` within about
/// a minute, dialling again now and then.
async fn connects(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) -> bool {
    for _ in 0..6 {
        let connected = seen(
            events,
            10,
            |e| matches!(e, NodeEvent::MemberConnected { device: d, .. } if *d == device),
        )
        .await;
        if connected {
            return true;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    false
}

/// `author` starts a new epoch for every member of `log`, but the boxes of `victims` open
/// to nothing.
fn broken_rotation(log: &mut GroupLog, author: &DeviceIdentity, victims: &[DevicePublicKey]) {
    let members: Vec<DevicePublicKey> = log.state().members.keys().copied().collect();
    let (mut rotation, _) = Rotation::new(&log.group_id(), members).unwrap();
    for (key, sealed) in rotation.sealed.iter_mut() {
        if victims.contains(key) {
            *sealed = Sealed::seal(key, b"elsewhere", &[7; 32]).unwrap();
        }
    }
    log.append(author, Body::Rotate { rotation }, 10).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manager_left_without_the_key_makes_a_new_one() {
    let _one = ONE_AT_A_TIME.lock().await;
    // Max left, so the members rotate; Leo does it with a key only he can open, and is
    // offline. Ana and Bea both manage: Ana, above, makes the new key, and Bea waits.
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let leo = DeviceIdentity::generate().unwrap();
    let max = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    log.add(&ana, leo.public_key(), "leo", 3).unwrap();
    log.add(&ana, max.public_key(), "max", 4).unwrap();
    log.append(&max, Body::Leave, 5).unwrap();
    broken_rotation(&mut log, &leo, &[ana.public_key(), bea.public_key()]);
    assert!(log.current_keys(&ana).is_none());
    assert!(log.current_keys(&bea).is_none());
    let group = log.group_id();

    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), log.clone(), 0, vec![loopback(port_a)]);
    assert!(
        connects(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await,
        "Bea connects to Ana"
    );
    // Ana makes a new key at once, and Bea gets it through the log.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let a = node_a.group(group).await.unwrap().unwrap();
        let b = node_b.group(group).await.unwrap().unwrap();
        let key_a = a.current_keys(&ana).map(|k| k.epoch());
        if key_a.is_some() && key_a == b.current_keys(&bea).map(|k| k.epoch()) {
            assert_eq!(a.state().epochs.len(), 3, "one new epoch, not more");
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no new key for Ana and Bea"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn members_left_without_the_key_still_connect() {
    let _one = ONE_AT_A_TIME.lock().await;
    // Leo, a manager, starts an epoch whose box for Bea opens to nothing; Ana has hers. Bea
    // comes online: she must still reach Ana, who can then mend things (or remove Leo).
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let leo = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    log.add(&ana, leo.public_key(), "leo", 3).unwrap();
    log.append(
        &ana,
        Body::Promote {
            member: leo.public_key(),
        },
        4,
    )
    .unwrap();
    broken_rotation(&mut log, &leo, &[bea.public_key()]);
    assert!(log.current_keys(&ana).is_some());
    assert!(log.current_keys(&bea).is_none());

    let port_a = free_port();
    let (_node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    assert!(
        connects(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await,
        "Ana verifies Bea"
    );
    assert!(
        seen(
            &mut events_b,
            30,
            |e| matches!(e, NodeEvent::MemberConnected { device, .. } if *device == ana.public_key())
        )
        .await,
        "Bea verifies Ana"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_whose_key_does_not_open_gets_a_new_one_from_a_manager() {
    let _one = ONE_AT_A_TIME.lock().await;
    // As before, Leo (a manager) gives Bea a box that opens to nothing; Ana has hers, and
    // the log counts Bea as given the key, so nobody would welcome her. Bea tells Ana.
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let leo = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    log.add(&ana, leo.public_key(), "leo", 3).unwrap();
    log.append(
        &ana,
        Body::Promote {
            member: leo.public_key(),
        },
        4,
    )
    .unwrap();
    broken_rotation(&mut log, &leo, &[bea.public_key()]);
    assert!(log.state().lacking_current_secret().is_empty());
    let group = log.group_id();

    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    assert!(connects(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let b = node_b.group(group).await.unwrap().unwrap();
        if b.current_keys(&bea).is_some() {
            assert_eq!(b.state().epochs.len(), 3, "one new epoch, not more");
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Bea never got a key that opens"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    // Told again, Ana does not rotate again: her own epoch's boxes open.
    tokio::time::sleep(Duration::from_secs(25)).await;
    let a = node_a.group(group).await.unwrap().unwrap();
    assert_eq!(a.state().epochs.len(), 3);
}
