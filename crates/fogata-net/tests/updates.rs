//! Releases between two members over the loopback interface: a member is offered a newer
//! release and fetches its package chunk by chunk; a damaged chunk ends the download with
//! the good chunks kept; a device that is neither friend nor member gets nothing.

use std::{collections::BTreeMap, collections::HashMap, sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_net::{
    GroupSetup, Multiaddr, NodeConfig, NodeEvents, NodeHandle, PeerId,
    updates::{self, FetchError, Held, Misbehave, Shelf},
};
use fogata_state::GroupLog;
use fogata_update::{Artifact, CHANNEL, Manifest, PRODUCT, Signed, manifest::CHUNK};
use tokio::{sync::mpsc, time::timeout};

static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

fn node(
    identity: Arc<DeviceIdentity>,
    groups: Vec<GroupSetup>,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups,
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

/// A release of `build` with one package, `data`, signed by `key`.
fn release(key: &DeviceIdentity, build: u64, data: &[u8]) -> (Signed, Manifest) {
    let manifest = Manifest {
        format: fogata_update::manifest::FORMAT,
        product: PRODUCT.into(),
        channel: CHANNEL.into(),
        build,
        version: format!("0.{build}"),
        min_build: 1,
        published: 0,
        expires: fogata_update::manifest::DEFAULT_LIFETIME,
        notes: String::new(),
        artifacts: BTreeMap::from([("linux-flatpak".into(), Artifact::of("fogata.pack", data))]),
    };
    (Signed::sign(&manifest, key).unwrap(), manifest)
}

/// Waits until `node` sees `peer` as a member that takes the update protocol.
async fn wait_for(node: &NodeHandle, peer: PeerId) {
    timeout(Duration::from_secs(60), async {
        loop {
            if node.trusted_device(peer).await.is_some()
                && node.advertises(&peer, &updates::UPDATE_PROTOCOL)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("never saw the other member");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_fetches_a_release_and_a_stranger_gets_nothing() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let carl = Arc::new(DeviceIdentity::generate().unwrap());
    let key = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (node_a, _events_a) = node(
        ana.clone(),
        vec![GroupSetup::Member(log.clone())],
        port_a,
        Vec::new(),
    );
    let (node_b, _events_b) = node(
        bea.clone(),
        vec![GroupSetup::Member(log)],
        0,
        vec![loopback(port_a)],
    );
    // Carl is in no group of Ana's: a stranger that knows her address.
    let (node_c, _events_c) = node(carl, Vec::new(), 0, vec![loopback(port_a)]);

    let dir = std::env::temp_dir().join(format!("fogata-updates-net-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let data: Vec<u8> = (0..(3 * CHUNK + 1234)).map(|i| (i % 251) as u8).collect();
    let package = dir.join("fogata.pack");
    std::fs::write(&package, &data).unwrap();
    let (signed, manifest) = release(&key, 7, &data);
    let shelf = Shelf::default();
    shelf.set(Some(Held {
        signed: signed.clone(),
        manifest: manifest.clone(),
        packages: HashMap::from([("linux-flatpak".to_string(), package.clone())]),
    }));
    let (heard_tx, mut heard) = mpsc::unbounded_channel();
    updates::serve(
        node_a.clone(),
        shelf.clone(),
        heard_tx,
        Misbehave::default(),
    )
    .unwrap();

    let peer_a = node_a.local_peer();
    wait_for(&node_b, peer_a).await;

    // Bea has build 6: she is offered 7, with the package Ana has.
    let offer = updates::ask(&node_b, peer_a, 6, "linux-flatpak")
        .await
        .unwrap()
        .expect("no release offered");
    assert_eq!(offer.signed, signed);
    assert_eq!(offer.platforms, vec!["linux-flatpak".to_string()]);
    assert_eq!(
        offer
            .signed
            .verify(&[fogata_update::ReleaseKey::release(
                *key.public_key().as_bytes(),
            )])
            .unwrap(),
        manifest
    );
    // Asked again at once: ask later (asks are spaced); with build 7 later: nothing.
    let again = updates::ask(&node_b, peer_a, 6, "linux-flatpak").await;
    assert!(matches!(again, Err(FetchError::Busy)), "{again:?}");
    // Past the spacing between asks.
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(
        updates::ask(&node_b, peer_a, 7, "linux-flatpak")
            .await
            .unwrap()
            .is_none()
    );

    // The package, whole.
    let artifact = &manifest.artifacts["linux-flatpak"];
    let part = dir.join("got.part");
    let mut seen = 0;
    updates::fetch(
        &node_b,
        peer_a,
        7,
        "linux-flatpak",
        artifact,
        &part,
        |got| seen = got,
    )
    .await
    .unwrap();
    assert_eq!(seen, data.len() as u64);
    assert_eq!(std::fs::read(&part).unwrap(), data);
    // A platform Ana has no package of: not here, and Ana hears it was wanted.
    let mut other = manifest.clone();
    let windows = other.artifacts["linux-flatpak"].clone();
    other
        .artifacts
        .insert("windows-x64".into(), windows.clone());
    let other_signed = Signed::sign(&other, &key).unwrap();
    shelf.set(Some(Held {
        signed: other_signed,
        manifest: other,
        packages: HashMap::from([("linux-flatpak".to_string(), package.clone())]),
    }));
    let missing = updates::fetch(
        &node_b,
        peer_a,
        7,
        "windows-x64",
        &windows,
        &dir.join("win.part"),
        |_| {},
    )
    .await;
    assert!(matches!(missing, Err(FetchError::NotHere)), "{missing:?}");
    // Ana heard that she gave the Linux package, then that the Windows one was wanted.
    let given = timeout(Duration::from_secs(5), heard.recv()).await.unwrap();
    assert_eq!(
        given,
        Some(updates::Heard::Given {
            platform: "linux-flatpak".into(),
            build: 7
        })
    );
    let wanted = timeout(Duration::from_secs(5), heard.recv()).await.unwrap();
    assert_eq!(
        wanted,
        Some(updates::Heard::Wanted {
            platform: "windows-x64".into(),
            build: 7
        })
    );
    // Telling Ana of a build reaches her.
    updates::tell(&node_b, peer_a, 7).await.unwrap();
    let have = timeout(Duration::from_secs(5), heard.recv()).await.unwrap();
    assert!(matches!(have, Some(updates::Heard::Have { build: 7, .. })));

    // Carl connects, but is neither friend nor member: refused, for asks and for packages.
    timeout(Duration::from_secs(30), async {
        // A stranger is not told Ana's protocols, only that she identified herself.
        while node_c.peer_level(&peer_a).is_none() {
            node_c.dial(vec![loopback(port_a)], None).await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    })
    .await
    .expect("the stranger never connected");
    let asked = updates::ask(&node_c, peer_a, 0, "linux-flatpak").await;
    assert!(matches!(asked, Err(FetchError::Refused)), "{asked:?}");
    let fetched = updates::fetch(
        &node_c,
        peer_a,
        7,
        "linux-flatpak",
        artifact,
        &dir.join("carl.part"),
        |_| {},
    )
    .await;
    assert!(matches!(fetched, Err(FetchError::Refused)), "{fetched:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_damaged_chunk_ends_the_download_and_keeps_the_good_ones() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let key = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (node_a, _events_a) = node(
        ana.clone(),
        vec![GroupSetup::Member(log.clone())],
        port_a,
        Vec::new(),
    );
    let (node_b, _events_b) = node(
        bea.clone(),
        vec![GroupSetup::Member(log)],
        0,
        vec![loopback(port_a)],
    );
    let dir = std::env::temp_dir().join(format!("fogata-updates-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let data: Vec<u8> = (0..(2 * CHUNK + 99)).map(|i| (i % 249) as u8).collect();
    let package = dir.join("fogata.pack");
    std::fs::write(&package, &data).unwrap();
    let (signed, manifest) = release(&key, 3, &data);
    let shelf = Shelf::default();
    shelf.set(Some(Held {
        signed,
        manifest: manifest.clone(),
        packages: HashMap::from([("linux-flatpak".to_string(), package)]),
    }));
    let (heard_tx, _heard) = mpsc::unbounded_channel();
    updates::serve(
        node_a.clone(),
        shelf,
        heard_tx,
        Misbehave {
            damage_chunks: true,
            offer_always: false,
        },
    )
    .unwrap();
    let peer_a = node_a.local_peer();
    wait_for(&node_b, peer_a).await;

    // One good chunk already there; the next arrives damaged.
    let part = dir.join("got.part");
    std::fs::write(&part, &data[..CHUNK as usize]).unwrap();
    let artifact = &manifest.artifacts["linux-flatpak"];
    let result = updates::fetch(&node_b, peer_a, 3, "linux-flatpak", artifact, &part, |_| {}).await;
    assert!(matches!(result, Err(FetchError::Damaged)), "{result:?}");
    assert_eq!(std::fs::read(&part).unwrap(), &data[..CHUNK as usize]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether a stream opened on the update protocol and left silent is closed by the other
/// side within `seconds`.
async fn closed_unread(node: &NodeHandle, peer: PeerId, seconds: u64) -> bool {
    use futures::AsyncReadExt as _;
    let mut stream = node
        .streams()
        .open_stream(peer, updates::UPDATE_PROTOCOL)
        .await
        .expect("a stream opens");
    let mut byte = [0u8; 1];
    matches!(
        timeout(Duration::from_secs(seconds), stream.read(&mut byte)).await,
        Ok(Ok(0) | Err(_))
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn silent_streams_of_strangers_and_extra_ones_of_members_close_at_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let carl = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (node_a, _events_a) = node(
        ana.clone(),
        vec![GroupSetup::Member(log.clone())],
        port_a,
        Vec::new(),
    );
    let (node_b, _events_b) = node(
        bea,
        vec![GroupSetup::Member(log)],
        0,
        vec![loopback(port_a)],
    );
    let (node_c, _events_c) = node(carl, Vec::new(), 0, vec![loopback(port_a)]);
    let (heard_tx, _heard) = mpsc::unbounded_channel();
    updates::serve(
        node_a.clone(),
        Shelf::default(),
        heard_tx,
        Misbehave::default(),
    )
    .unwrap();
    let peer_a = node_a.local_peer();
    wait_for(&node_b, peer_a).await;
    timeout(Duration::from_secs(30), async {
        // A stranger is not told Ana's protocols, only that she identified herself.
        while node_c.peer_level(&peer_a).is_none() {
            node_c.dial(vec![loopback(port_a)], None).await.unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    })
    .await
    .expect("the stranger never connected");

    // Carl's stream is closed before anything is read from it, not after a long wait.
    assert!(
        closed_unread(&node_c, peer_a, 5).await,
        "a stranger's silent stream was kept open"
    );
    // Bea's first few silent streams wait for her request; one more is closed at once.
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(
            node_b
                .streams()
                .open_stream(peer_a, updates::UPDATE_PROTOCOL)
                .await
                .unwrap(),
        );
    }
    // Time for Ana to take the four, which nothing here can see (one more to check it would
    // be one more of them).
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        closed_unread(&node_b, peer_a, 5).await,
        "a member parked more silent streams than allowed"
    );
    drop(held);
}
