//! Call signals between members: they reach a device only while it is in one of the group's
//! voice rooms, from the members in that room (a few from others, whose join may still be on
//! its way), and a member cannot send large ones or flood it with them. Nodes in one process,
//! over the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_net::{
    FIRST_ROOM, GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle,
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

/// Waits until `device` is connected as a member on the node of `events`, dialling again now
/// and then.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = timeout(Duration::from_secs(10), async {
            while let Some(event) = events.recv().await {
                if matches!(event, NodeEvent::MemberConnected { device: d, .. } if d == device) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        if connected {
            return;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    panic!("timed out waiting for the member to connect");
}

/// The sizes of the call signals that come within `seconds`.
async fn signals_within(events: &mut NodeEvents, seconds: u64) -> Vec<usize> {
    let mut got = Vec::new();
    let _ = timeout(Duration::from_secs(seconds), async {
        while let Some(event) = events.recv().await {
            if let NodeEvent::Signal { payload, .. } = event {
                got.push(payload.len());
            }
        }
    })
    .await;
    got
}

async fn in_room(node: &NodeHandle, group: GroupId, device: DevicePublicKey) -> bool {
    node.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .is_some_and(|r| r.participants.iter().any(|(d, _, _)| *d == device))
}

#[tokio::test(flavor = "multi_thread")]
async fn signals_reach_a_device_in_a_room_from_its_people_and_only_so_many() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "ours", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, vec![]);
    let (node_b, mut events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;

    // Bea is in no room: nothing reaches her.
    node_a
        .signal(group, bea.public_key(), vec![1; 300])
        .await
        .unwrap();
    assert!(signals_within(&mut events_b, 2).await.is_empty());

    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    let joined = timeout(Duration::from_secs(10), async {
        while !in_room(&node_b, group, bea.public_key()).await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(joined.is_ok(), "Bea is not in the room");
    // Ana is in no room: Bea hears her only a few times (her join could be on its way).
    for _ in 0..12 {
        node_a
            .signal(group, bea.public_key(), vec![3; 300])
            .await
            .unwrap();
    }
    assert_eq!(signals_within(&mut events_b, 3).await.len(), 4);

    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    let joined = timeout(Duration::from_secs(10), async {
        while !in_room(&node_b, group, ana.public_key()).await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(joined.is_ok(), "Ana is not in the room at Bea's");
    // The burst is full again (a few signals a second).
    tokio::time::sleep(Duration::from_secs(5)).await;
    node_a
        .signal(group, bea.public_key(), vec![1; 300])
        .await
        .unwrap();
    assert_eq!(signals_within(&mut events_b, 3).await, [300]);
    // One far larger than a call needs is dropped.
    node_a
        .signal(group, bea.public_key(), vec![1; 100 * 1024])
        .await
        .unwrap();
    assert!(signals_within(&mut events_b, 2).await.is_empty());

    // A flood: a burst gets through, then a few a second.
    let started = std::time::Instant::now();
    for _ in 0..200 {
        node_a
            .signal(group, bea.public_key(), vec![2; 300])
            .await
            .unwrap();
    }
    let got = signals_within(&mut events_b, 4).await.len();
    let seconds = started.elapsed().as_secs_f64();
    assert!(
        got >= 32 && (got as f64) <= 33.0 + 8.0 * seconds,
        "{got} signals in {seconds:.1} s"
    );
}
