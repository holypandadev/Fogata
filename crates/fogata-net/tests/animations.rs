//! Animations of pictures and banners between nodes in one process: a member's picture's
//! animation reaches the other members with its profile, a friend's banner's animation reaches
//! its friends, and neither goes without the still picture it moves in place of.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Animated, Avatar, Friend, FriendBook};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle, PeerId};
use fogata_state::GroupLog;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nstill";

/// A GIF of one pixel, padded with a comment to `len` bytes of `fill`.
fn gif(len: usize, fill: u8) -> Vec<u8> {
    let mut gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00\x21\xfe"
        .to_vec();
    let mut left = len - gif.len() - 2;
    while left > 0 {
        let mut block = left.min(256);
        if left - block == 1 {
            block -= 1;
        }
        gif.push((block - 1) as u8);
        gif.resize(gif.len() + block - 1, fill);
        left -= block;
    }
    gif.extend([0, 0x3b]);
    gif
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn loopback(port: u16, peer: PeerId) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1/p2p/{peer}")
        .parse()
        .unwrap()
}

fn node(
    identity: Arc<DeviceIdentity>,
    book: FriendBook,
    logs: Vec<GroupLog>,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: logs.into_iter().map(GroupSetup::Member).collect(),
        friends: Some(book),
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

async fn wait_for(
    events: &mut NodeEvents,
    seconds: u64,
    what: &str,
    mut done: impl FnMut(&NodeEvent) -> bool,
) {
    let found = timeout(Duration::from_secs(seconds), async {
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
async fn a_members_picture_moves_for_the_other_members() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (node_a, _events_a) = node(
        ana.clone(),
        FriendBook::new(&ana, "Ana", 1).unwrap(),
        vec![log.clone()],
        port_a,
        Vec::new(),
    );
    // A picture and its animation, set before Bea connects: they go with the profile.
    node_a
        .set_profile(None, Some(Avatar::Photo(PNG.to_vec())))
        .await
        .unwrap();
    node_a
        .set_animation(Animated::Photo, gif(200 * 1024, 7))
        .await
        .unwrap();
    // Without a still picture there is nothing to animate.
    assert!(
        node_a
            .set_animation(Animated::Banner, gif(200 * 1024, 7))
            .await
            .is_err()
    );

    let (node_b, mut events_b) = node(
        bea.clone(),
        FriendBook::new(&bea, "Bea", 1).unwrap(),
        vec![log],
        free_port(),
        vec![loopback(port_a, node_a.local_peer())],
    );
    wait_for(&mut events_b, 60, "Ana's animation", |e| {
        matches!(e, NodeEvent::Animation { animation }
            if animation.device == ana.public_key()
                && animation.of == Animated::Photo
                && animation.animates(PNG))
    })
    .await;

    // Taken away: members hear it at once, with a newer version.
    node_a
        .set_animation(Animated::Photo, Vec::new())
        .await
        .unwrap();
    wait_for(&mut events_b, 10, "no animation", |e| {
        matches!(e, NodeEvent::Animation { animation }
            if animation.device == ana.public_key() && animation.picture.is_empty())
    })
    .await;
    drop(node_b);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friends_banner_moves_for_its_friends() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut book_a = FriendBook::new(&ana, "Ana", 1).unwrap();
    let mut book_b = FriendBook::new(&bea, "Bea", 1).unwrap();
    let as_friend = |book: &FriendBook| Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    let (a, b) = (as_friend(&book_a), as_friend(&book_b));
    book_a.friends.insert(bea.public_key(), b);
    book_b.friends.insert(ana.public_key(), a);
    let port_a = free_port();
    let (node_a, _events_a) = node(ana.clone(), book_a, Vec::new(), port_a, Vec::new());
    let (node_b, mut events_b) = node(
        bea.clone(),
        book_b,
        Vec::new(),
        free_port(),
        vec![loopback(port_a, node_a.local_peer())],
    );
    wait_for(&mut events_b, 60, "Ana online", |e| {
        matches!(e, NodeEvent::FriendOnline { .. })
    })
    .await;
    node_a
        .set_banner(0xFF3D21AD, Some(PNG.to_vec()))
        .await
        .unwrap();
    let big = gif(500 * 1024, 1);
    node_a
        .set_animation(Animated::Banner, big.clone())
        .await
        .unwrap();
    wait_for(&mut events_b, 10, "Ana's banner animation", |e| {
        matches!(e, NodeEvent::Animation { animation }
            if animation.of == Animated::Banner && animation.picture == big && animation.animates(PNG))
    })
    .await;
    drop((node_a, node_b));
}
