//! How members reach each other: one that knows where only one other member is reaches the
//! rest through it at once, and a participant nobody can hear from any more leaves the
//! rooms soon, without the grace given to those still heard through someone. Nodes in one
//! process, over the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvents, NodeHandle};
use fogata_state::{FIRST_ROOM, GroupLog};
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

/// A node of `log`'s group that knows where `peers` are, and lets members connect through
/// it when `bridge`.
fn node(
    identity: Arc<DeviceIdentity>,
    log: GroupLog,
    port: u16,
    peers: Vec<Multiaddr>,
    bridge: bool,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log)],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge,
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

/// Polls `check` until it holds, for up to `seconds`; whether it did.
async fn within<F, Fut>(seconds: u64, mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    timeout(Duration::from_secs(seconds), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok()
}

/// Whether `node` is connected to `device` as a member of `group`.
async fn reaches(node: &NodeHandle, group: GroupId, device: DevicePublicKey) -> bool {
    node.members(group)
        .await
        .unwrap()
        .iter()
        .any(|(d, _)| *d == device)
}

/// Who `node` sees in the first voice room of `group`.
async fn in_room(node: &NodeHandle, group: GroupId) -> Vec<DevicePublicKey> {
    node.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .map(|r| r.participants.iter().map(|(d, _, _)| *d).collect())
        .unwrap_or_default()
}

/// A group of Ana, Bea and as many more as `others` names.
fn group(others: &[&Arc<DeviceIdentity>]) -> (Arc<DeviceIdentity>, Arc<DeviceIdentity>, GroupLog) {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    for (i, other) in others.iter().enumerate() {
        log.add(&ana, other.public_key(), "other", 3 + i as u64)
            .unwrap();
    }
    (ana, bea, log)
}

#[tokio::test(flavor = "multi_thread")]
async fn members_that_come_in_through_one_member_reach_the_others_at_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let newcomers: Vec<Arc<DeviceIdentity>> = (0..3)
        .map(|_| Arc::new(DeviceIdentity::generate().unwrap()))
        .collect();
    let (ana, bea, log) = group(&newcomers.iter().collect::<Vec<_>>());
    let group = log.group_id();
    let (port_a, port_b) = (free_port(), free_port());
    let (node_a, _events_a) = node(ana.clone(), log.clone(), port_a, Vec::new(), true);
    let (node_b, _events_b) = node(
        bea.clone(),
        log.clone(),
        port_b,
        vec![loopback(port_a)],
        true,
    );
    assert!(within(30, || reaches(&node_b, group, ana.public_key())).await);
    assert!(within(30, || reaches(&node_a, group, bea.public_key())).await);

    // One after the other, members that know only where Bea is: each reaches Ana through
    // Bea within moments, not at Bea's next roster, whichever of the two counted the other
    // in first.
    let mut running = Vec::new();
    for newcomer in &newcomers {
        let (node_n, events_n) = node(
            newcomer.clone(),
            log.clone(),
            free_port(),
            vec![loopback(port_b)],
            true,
        );
        assert!(
            within(8, || reaches(&node_n, group, ana.public_key())).await,
            "a newcomer did not reach Ana"
        );
        assert!(within(2, || reaches(&node_a, group, newcomer.public_key())).await);
        running.push((node_n, events_n));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_participant_nobody_can_hear_from_any_more_leaves_soon() {
    let _one = ONE_AT_A_TIME.lock().await;
    let carla = Arc::new(DeviceIdentity::generate().unwrap());
    let (ana, bea, log) = group(&[&carla]);
    let group = log.group_id();
    let (port_a, port_b) = (free_port(), free_port());
    let (node_a, _events_a) = node(ana.clone(), log.clone(), port_a, Vec::new(), true);
    let (node_b, _events_b) = node(
        bea.clone(),
        log.clone(),
        port_b,
        vec![loopback(port_a)],
        true,
    );
    let (node_c, _events_c) = node(
        carla.clone(),
        log,
        free_port(),
        vec![loopback(port_a), loopback(port_b)],
        true,
    );
    for (node, others) in [
        (&node_a, [&bea, &carla]),
        (&node_b, [&ana, &carla]),
        (&node_c, [&ana, &bea]),
    ] {
        for other in others {
            assert!(within(30, || reaches(node, group, other.public_key())).await);
        }
    }
    for (node, nick) in [(&node_a, "ana"), (&node_b, "bea"), (&node_c, "carla")] {
        node.join_room(group, FIRST_ROOM, nick.into())
            .await
            .unwrap();
    }
    let everyone = [ana.public_key(), bea.public_key(), carla.public_key()];
    assert!(
        within(30, || async {
            let seen = in_room(&node_a, group).await;
            everyone.iter().all(|d| seen.contains(d))
        })
        .await
    );

    // Carla's node stops without a word, as after a crash: nobody can say she left, and
    // nobody reaches her any more. Ana takes her out of the room once Bea says she does not
    // reach her either and her next word does not come, not after two minutes of grace.
    node_c.shutdown().await.unwrap();
    let gone = std::time::Instant::now();
    assert!(
        within(60, || async {
            !in_room(&node_a, group).await.contains(&carla.public_key())
        })
        .await,
        "Carla stayed in Ana's room"
    );
    assert!(gone.elapsed() < Duration::from_secs(60));
    let mut left = in_room(&node_a, group).await;
    left.sort();
    let mut want = vec![ana.public_key(), bea.public_key()];
    want.sort();
    assert_eq!(left, want);
}
