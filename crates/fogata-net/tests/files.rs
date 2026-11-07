//! Files sent between two members, in one process over the loopback interface: a file that
//! stops being shared in the middle of a transfer is not sent to the end.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey};
use fogata_net::{
    GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle,
    files::{self, SharedFiles},
};
use fogata_state::GroupLog;
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

/// Waits for `device`, which dials `address`, to connect to the node of `events`, dialling
/// again now and then: a dial by address alone is not tried again by itself.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = timeout(Duration::from_secs(10), async {
            while let Some(e) = events.recv().await {
                if matches!(e, NodeEvent::MemberConnected { device: d, .. } if d == device) {
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

#[tokio::test(flavor = "multi_thread")]
async fn a_file_no_longer_shared_stops_on_its_way() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    // Bea's side has verified Ana too once she shows as a member there.
    let ana_key = ana.public_key();
    let peer_a = timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(Some(peer)) = node_b.member_peer(ana_key).await {
                return peer;
            }
            let _ = timeout(Duration::from_millis(200), events_b.recv()).await;
        }
    })
    .await
    .expect("Ana never became a member at Bea's");

    let dir = std::env::temp_dir().join(format!("fogata-files-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("holiday.mkv");
    let bytes: Vec<u8> = (0..64u32 << 20).map(|i| (i % 253) as u8).collect();
    std::fs::write(&source, &bytes).unwrap();
    let (id, size) = files::hash_file(&source).unwrap();
    let shared = SharedFiles::default();
    shared.insert(group, id, source);
    files::serve(node_a.clone(), shared.clone(), |_, _| {}).unwrap();

    // Ana stops sharing it as soon as some of it reached Bea, who takes it in slowly.
    let dest = dir.join("got.mkv");
    let mut stopped = false;
    let result = files::fetch(&node_b, group, peer_a, id, size, &dest, |_| {
        if !stopped {
            shared.remove(group, id);
            stopped = true;
        }
        std::thread::sleep(Duration::from_millis(2));
    })
    .await;
    assert!(
        result.is_err(),
        "the whole file arrived after it was no longer shared"
    );
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file kept sealed is served as what it holds; one kept plain in a folder sealed since is
/// served as it is; a download sealed as it arrives goes on from the chunks a cut-short one
/// left.
#[tokio::test(flavor = "multi_thread")]
async fn sealed_files_are_served_and_fetched_sealed() {
    let _one = ONE_AT_A_TIME.lock().await;
    use std::io::{Read, Write};

    use fogata_crypto::at_rest::{DataKey, SealReader, SealWriter, sealed_form};

    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    let ana_key = ana.public_key();
    let peer_a = timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(Some(peer)) = node_b.member_peer(ana_key).await {
                return peer;
            }
            let _ = timeout(Duration::from_millis(200), events_b.recv()).await;
        }
    })
    .await
    .expect("Ana never became a member at Bea's");

    let dir = std::env::temp_dir().join(format!("fogata-files-sealed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bytes: Vec<u8> = (0..(3u32 << 20) + 12345).map(|i| (i % 251) as u8).collect();
    let plain = dir.join("plain.mkv");
    std::fs::write(&plain, &bytes).unwrap();
    let (id, size) = files::hash_file(&plain).unwrap();
    let (key_a, key_b) = (DataKey::generate().unwrap(), DataKey::generate().unwrap());
    let context = files::kept_context(&id);
    let kept = dir.join("kept.mkv");
    let mut w = SealWriter::new(&key_a, &context, std::fs::File::create(&kept).unwrap()).unwrap();
    w.write_all(&bytes).unwrap();
    w.finish().unwrap();
    let shared = SharedFiles::default();
    files::serve(node_a.clone(), shared.clone(), |_, _| {}).unwrap();

    // Plain, from a folder sealed since: served as it is.
    shared.insert_kept(group, id, plain, Some(key_a.clone()));
    let got = dir.join("got-plain.mkv");
    files::fetch(&node_b, group, peer_a, id, size, &got, |_| {})
        .await
        .unwrap();
    assert_eq!(std::fs::read(&got).unwrap(), bytes);

    // Sealed: served as what it holds, and sealed again at Bea's with her key, going on from
    // a download that kept its first megabyte.
    shared.insert_kept(group, id, kept, Some(key_a));
    let dest = dir.join("got-sealed.mkv");
    let part = dir.join(format!("got-sealed.mkv.{}.part", hex::encode(&id[..4])));
    let mut w = SealWriter::new(&key_b, &context, std::fs::File::create(&part).unwrap()).unwrap();
    w.write_all(&bytes[..1 << 20]).unwrap();
    w.flush().unwrap();
    drop(w);
    let mut first = None;
    files::fetch_sealed(&node_b, group, peer_a, id, size, &dest, &key_b, |got| {
        first.get_or_insert(got);
    })
    .await
    .unwrap();
    // The first 15 chunks were kept; the 16th was still waiting to know whether it was the
    // last, so it came again.
    let first = first.unwrap();
    assert!(15 << 16 < first && first <= 16 << 16, "{first}");
    let on_disk = std::fs::read(&dest).unwrap();
    assert!(sealed_form(&on_disk).is_some());
    let mut back = Vec::new();
    SealReader::new(
        &key_b,
        &context,
        std::fs::File::open(&dest).unwrap(),
        on_disk.len() as u64,
    )
    .unwrap()
    .read_to_end(&mut back)
    .unwrap();
    assert_eq!(back, bytes);
    let _ = std::fs::remove_dir_all(&dir);
}
