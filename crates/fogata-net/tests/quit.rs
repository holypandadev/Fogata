// Two devices that are already friends, on the same machine, alone in this process: when
// one quits, the other sees it go at once, not after the connections time out. One dials
// the other's loopback address; nothing else of the network is used.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Friend, FriendBook};
use fogata_net::{Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use tokio::time::timeout;

/// A node listening on `listen_port` (0 for any) that dials `peers`; nothing else of the
/// network.
fn node(
    identity: Arc<DeviceIdentity>,
    book: FriendBook,
    listen_port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: Vec::new(),
        friends: Some(book),
        listen_port,
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

/// Two books that already hold each other as friends.
fn friends(ana: &DeviceIdentity, bea: &DeviceIdentity) -> (FriendBook, FriendBook) {
    let mut book_a = FriendBook::new(ana, "Ana", 1).unwrap();
    let mut book_b = FriendBook::new(bea, "Bea", 1).unwrap();
    book_a.friends.insert(
        bea.public_key(),
        Friend {
            profile: book_b.profile.clone(),
            secret: book_b.secret.clone(),
            since_ms: 2,
            delivered: true,
            banner: None,
            bio: None,
        },
    );
    book_b.friends.insert(
        ana.public_key(),
        Friend {
            profile: book_a.profile.clone(),
            secret: book_a.secret.clone(),
            since_ms: 2,
            delivered: true,
            banner: None,
            bio: None,
        },
    );
    (book_a, book_b)
}

async fn wait_for(
    events: &mut NodeEvents,
    limit: Duration,
    what: &str,
    mut done: impl FnMut(&NodeEvent) -> bool,
) {
    let found = timeout(limit, async {
        while let Some(event) = events.recv().await {
            if done(&event) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(found, "timed out waiting for {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_that_quits_goes_offline_right_away() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let port_a = free_port();
    let (_node_a, mut events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    // Bea runs on a runtime of her own, stopped right after her node shuts down, as the
    // process ends when the application quits.
    let bea_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let (node_b, mut events_b) = {
        let _inside = bea_runtime.enter();
        node(bea.clone(), book_b, 0, vec![loopback(port_a)])
    };
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;
    wait_for(&mut events_b, Duration::from_secs(60), "Ana online", online).await;

    bea_runtime
        .spawn(async move { node_b.shutdown().await })
        .await
        .unwrap()
        .unwrap();
    bea_runtime.shutdown_background();
    // Well under the QUIC idle timeout (30 s), after which Ana would notice anyway.
    let bea_key = bea.public_key();
    wait_for(
        &mut events_a,
        Duration::from_secs(3),
        "Bea offline",
        |e| matches!(e, NodeEvent::FriendOffline { device } if *device == bea_key),
    )
    .await;
}
