//! A group's picture between nodes in one process, in a record apart from the log: chosen
//! by a manager, replaced and taken away, it reaches the members connected and the one that
//! connects later, which gets only the newest; no picture goes into the log. A picture an
//! earlier version set with a log entry is taken away there with one entry with no picture.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvents, NodeHandle};
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

fn png(len: usize, fill: u8) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.resize(len, fill);
    png
}

/// Polls the group on `node` until `check` holds for its log.
async fn until(node: &NodeHandle, log: &GroupLog, what: &str, check: impl Fn(&GroupLog) -> bool) {
    let group = log.group_id();
    let found = timeout(Duration::from_secs(60), async {
        loop {
            if let Ok(Some(log)) = node.group(group).await
                && check(&log)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "timed out waiting for {what}");
}

fn address(node: &NodeHandle, port: u16) -> Multiaddr {
    format!(
        "/ip4/127.0.0.1/udp/{port}/quic-v1/p2p/{}",
        node.local_peer()
    )
    .parse()
    .unwrap()
}

fn has_picture_entries(log: &GroupLog) -> bool {
    log.entries()
        .any(|e| matches!(e.body, Body::SetPicture { .. }))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_managers_picture_reaches_the_members_and_only_the_newest_a_newcomer() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let cid = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    // Only Ana manages.
    let first = fogata_state::Body::SetManagers { first: Some(1) };
    log.append(&ana, first, 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    log.add(&ana, cid.public_key(), "cid", 3).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, _events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let first = png(40 * 1024, 1);
    node_a
        .set_group_picture(group, first.clone())
        .await
        .unwrap();
    let (node_b, _events_b) = node(
        bea.clone(),
        log.clone(),
        free_port(),
        vec![address(&node_a, port_a)],
    );
    until(&node_b, &log, "the first picture", |l| {
        l.picture() == Some(first.clone())
    })
    .await;

    // Replaced and taken away while both are connected, then chosen again.
    let second = png(30 * 1024, 2);
    node_a
        .set_group_picture(group, second.clone())
        .await
        .unwrap();
    until(&node_b, &log, "the second picture", |l| {
        l.picture() == Some(second.clone())
    })
    .await;
    node_a.set_group_picture(group, Vec::new()).await.unwrap();
    until(&node_b, &log, "no picture", |l| {
        l.picture().is_none() && l.picture_record().is_some()
    })
    .await;
    let third = png(1000, 3);
    node_a
        .set_group_picture(group, third.clone())
        .await
        .unwrap();
    until(&node_b, &log, "the third picture", |l| {
        l.picture() == Some(third.clone())
    })
    .await;

    // Bea does not manage the group; a picture too big or of another kind is refused.
    assert!(node_b.set_group_picture(group, png(10, 0)).await.is_err());
    assert!(
        node_a
            .set_group_picture(group, png(fogata_state::MAX_RECORD_PICTURE_BYTES + 1, 0))
            .await
            .is_err()
    );
    assert!(
        node_a
            .set_group_picture(group, b"%PDF-1.7".to_vec())
            .await
            .is_err()
    );

    // Cid comes now: it gets the newest picture, and nothing of the ones before.
    let (node_c, _events_c) = node(
        cid.clone(),
        log.clone(),
        free_port(),
        vec![address(&node_a, port_a)],
    );
    until(&node_c, &log, "the newest picture at Cid", |l| {
        l.picture() == Some(third.clone())
    })
    .await;
    let c = node_c.group(group).await.unwrap().unwrap();
    assert_eq!(c.picture_record().unwrap().picture, third);

    // No picture went into the log.
    for node in [&node_a, &node_b, &node_c] {
        let kept = node.group(group).await.unwrap().unwrap();
        // A manager may also have raised the group's level once every copy judged at it
        // (`raise_level_when_all_judge`): that entry is no picture's.
        let kept_hashes: Vec<_> = kept
            .entries_in_order()
            .into_iter()
            .filter(|e| !matches!(e.body, fogata_state::Body::Level { .. }))
            .map(|e| e.hash())
            .collect();
        assert_eq!(kept_hashes, log.hashes_in_order());
        assert!(!has_picture_entries(&kept));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_picture_set_with_an_entry_goes_with_one_entry_without_a_picture() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let old = png(2000, 9);
    log.append(
        &ana,
        Body::SetPicture {
            picture: Some(old.clone()),
        },
        3,
    )
    .unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, _events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, _events_b) = node(
        bea.clone(),
        log.clone(),
        free_port(),
        vec![address(&node_a, port_a)],
    );
    until(&node_b, &log, "the picture of the entry", |l| {
        l.picture() == Some(old.clone())
    })
    .await;

    let new = png(3000, 4);
    node_a.set_group_picture(group, new.clone()).await.unwrap();
    // What an earlier version reads from the log, the entries, shows the icon; the record
    // shows the new picture.
    until(&node_b, &log, "the new picture and the entry", |l| {
        l.picture() == Some(new.clone()) && l.state().picture.is_none()
    })
    .await;
    node_a.set_group_picture(group, png(500, 5)).await.unwrap();
    until(&node_b, &log, "the next picture", |l| {
        l.picture() == Some(png(500, 5))
    })
    .await;
    // One entry more in all, with no picture. A manager may also have raised the group's
    // level once every copy judged at it (`raise_level_when_all_judge`): that entry is no
    // picture's, and whether it came depends on when the heartbeat fell.
    let b = node_b.group(group).await.unwrap().unwrap();
    let added: Vec<_> = b
        .entries()
        .filter(|e| log.get(&e.hash()).is_none())
        .filter(|e| !matches!(e.body, Body::Level { .. }))
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].body, Body::SetPicture { picture: None });
}
