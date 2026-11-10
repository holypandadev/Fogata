//! A group's banner between nodes in one process: chosen by a manager before the others
//! connect, it reaches them when they compare banners; chosen while they are connected, it
//! reaches them at once; a member that is no manager cannot choose one, and the log stays the
//! same for versions that know nothing of banners.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvents, NodeHandle};
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
    })
    .unwrap()
}

/// Polls the group's banner on `node` until it is `want`.
async fn until_banner(node: &NodeHandle, log: &GroupLog, want: Option<&[u8]>, what: &str) {
    let group = log.group_id();
    let found = timeout(Duration::from_secs(60), async {
        loop {
            if let Ok(Some(log)) = node.group(group).await
                && log.banner().map(|b| b.picture.as_slice()) == want
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "timed out waiting for {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_managers_banner_reaches_the_members() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    // Written now: a key dated long ago is a week old, and the first heartbeat (on a slow
    // computer, before the test ends) would replace it with a rotation of its own.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let mut log = GroupLog::create(&ana, "g", now).unwrap();
    // Only Ana manages.
    let first = fogata_state::Body::SetManagers { first: Some(1) };
    log.append(&ana, first, now + 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", now + 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, _events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let mut first = b"GIF89a".to_vec();
    first.resize(300 * 1024, 3);
    node_a.set_group_banner(group, first.clone()).await.unwrap();
    let address = format!(
        "/ip4/127.0.0.1/udp/{port_a}/quic-v1/p2p/{}",
        node_a.local_peer()
    )
    .parse()
    .unwrap();
    let (node_b, _events_b) = node(bea.clone(), log.clone(), free_port(), vec![address]);
    until_banner(&node_b, &log, Some(&first), "the first banner").await;

    // Chosen again while both are connected, then taken away.
    let second = b"\xff\xd8\xffsecond".to_vec();
    node_a
        .set_group_banner(group, second.clone())
        .await
        .unwrap();
    until_banner(&node_b, &log, Some(&second), "the second banner").await;
    node_a.set_group_banner(group, Vec::new()).await.unwrap();
    until_banner(&node_b, &log, None, "no banner").await;

    // Bea does not manage the group.
    assert!(node_b.set_group_banner(group, second).await.is_err());

    // The entries are the same as without banners: what older versions keep. Ana, the
    // manager, may also have raised the group's level once she saw Bea's copy judge at it
    // (`raise_level_when_all_judge`), when the heartbeat came in time: that entry is
    // no banner's.
    let a = node_a.group(group).await.unwrap().unwrap();
    let b = node_b.group(group).await.unwrap().unwrap();
    assert_eq!(without_level_raises(&a), log.hashes_in_order());
    assert_eq!(without_level_raises(&b), log.hashes_in_order());
}

/// The hashes of `log` in order, but for the entries that raise the group's level.
fn without_level_raises(log: &GroupLog) -> Vec<fogata_state::EntryHash> {
    log.entries_in_order()
        .into_iter()
        .filter(|e| !matches!(e.body, fogata_state::Body::Level { .. }))
        .map(|e| e.hash())
        .collect()
}
