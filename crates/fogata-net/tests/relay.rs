//! A member keeps a relay slot at another member. Once removed from the group they shared,
//! while still a friend (so still connected), it loses the slot: its connections close, and
//! the other dials it again a moment later, as a friend only.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Friend, FriendBook};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::GroupLog;
use libp2p::multiaddr::Protocol;
use tokio::time::timeout;

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
    log: GroupLog,
    book: FriendBook,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log)],
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

fn friend_of(book: &FriendBook) -> Friend {
    Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    }
}

/// Whether `done` holds for an event that comes within `seconds`.
async fn comes(
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

#[tokio::test(flavor = "multi_thread")]
async fn a_member_removed_but_still_a_friend_loses_its_relay_slot() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut book_a = FriendBook::new(&ana, "Ana", 1).unwrap();
    let mut book_b = FriendBook::new(&bea, "Bea", 1).unwrap();
    book_a.friends.insert(bea.public_key(), friend_of(&book_b));
    book_b.friends.insert(ana.public_key(), friend_of(&book_a));
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let (port_a, port_b) = (free_port(), free_port());
    let (node_a, _events_a) = node(
        ana.clone(),
        log.clone(),
        book_a,
        port_a,
        vec![loopback(port_b)],
    );
    let (_node_b, mut events_b) = node(bea.clone(), log, book_b, port_b, vec![loopback(port_a)]);

    // Bea holds a slot at Ana's relay, and knows Ana as a friend.
    let (mut slot, mut friend) = (false, false);
    let both = comes(&mut events_b, 40, |e| {
        match e {
            NodeEvent::Listening(address) => {
                slot |= address.iter().any(|p| p == Protocol::P2pCircuit);
            }
            NodeEvent::FriendOnline { device, .. } if *device == ana.public_key() => friend = true,
            _ => {}
        }
        slot && friend
    })
    .await;
    assert!(
        both,
        "Bea got no relay slot at Ana's, or never saw her as a friend"
    );

    let mut now = node_a.group(group).await.unwrap().unwrap();
    now.remove(&ana, bea.public_key(), 10).unwrap();
    node_a.merge_group(now).await.unwrap();
    assert!(
        comes(&mut events_b, 20, |e| matches!(
            e,
            NodeEvent::FriendOffline { device } if *device == ana.public_key()
        ))
        .await,
        "Bea kept her connection to Ana's relay"
    );
    // Still friends: Ana dials her again a moment later.
    assert!(
        comes(&mut events_b, 20, |e| matches!(
            e,
            NodeEvent::FriendOnline { device, .. } if *device == ana.public_key()
        ))
        .await,
        "Bea was not reached again as a friend"
    );
}
