//! One identity open on two computers at once: both copies are told, whether the others
//! see them as a friend or as a member. A restart, whose old session lingers at the others
//! until its connections time out, is not taken for another computer. Nodes in one process,
//! over the loopback interface; the copies only know Ana's address, so only Ana can tell.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Friend, FriendBook};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
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

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

fn node(
    identity: Arc<DeviceIdentity>,
    groups: Vec<GroupSetup>,
    friends: Option<FriendBook>,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups,
        friends,
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

/// Two books that already hold each other as friends.
fn friends(ana: &DeviceIdentity, bea: &DeviceIdentity) -> (FriendBook, FriendBook) {
    let mut book_a = FriendBook::new(ana, "Ana", 1).unwrap();
    let mut book_b = FriendBook::new(bea, "Bea", 1).unwrap();
    let friend = |book: &FriendBook| Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    book_a.friends.insert(bea.public_key(), friend(&book_b));
    book_b.friends.insert(ana.public_key(), friend(&book_a));
    (book_a, book_b)
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

fn told(e: &NodeEvent) -> bool {
    matches!(e, NodeEvent::OtherSession)
}

#[tokio::test(flavor = "multi_thread")]
async fn both_copies_of_a_friend_are_told() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let port_a = free_port();
    let (_node_a, mut events_a) = node(ana.clone(), Vec::new(), Some(book_a), port_a, Vec::new());
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });

    // Bea on one computer, then the same folder copied to another.
    let (_one, mut events_1) = node(
        bea.clone(),
        Vec::new(),
        Some(book_b.clone()),
        0,
        vec![loopback(port_a)],
    );
    assert!(seen(&mut events_1, 60, online).await, "Bea meets Ana");
    let (_two, mut events_2) = node(
        bea.clone(),
        Vec::new(),
        Some(book_b),
        0,
        vec![loopback(port_a)],
    );
    assert!(seen(&mut events_2, 60, online).await, "the copy meets Ana");
    assert!(
        seen(&mut events_1, 30, told).await,
        "the first copy is told"
    );
    assert!(
        seen(&mut events_2, 30, told).await,
        "the second copy is told"
    );
    assert!(!seen(&mut events_a, 1, told).await, "Ana has one session");
}

#[tokio::test(flavor = "multi_thread")]
async fn both_copies_of_a_member_are_told() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let member = |log: &GroupLog| vec![GroupSetup::Member(log.clone())];
    let (_node_a, _events_a) = node(ana.clone(), member(&log), None, port_a, Vec::new());
    let connected = |e: &NodeEvent| matches!(e, NodeEvent::MemberConnected { .. });

    let (_one, mut events_1) = node(bea.clone(), member(&log), None, 0, vec![loopback(port_a)]);
    assert!(seen(&mut events_1, 60, connected).await, "Bea meets Ana");
    let (_two, mut events_2) = node(bea.clone(), member(&log), None, 0, vec![loopback(port_a)]);
    assert!(
        seen(&mut events_2, 60, told).await,
        "the second copy is told"
    );
    assert!(
        seen(&mut events_1, 60, told).await,
        "the first copy is told"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_is_not_another_computer() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (_node_a, mut events_a) = node(
        ana.clone(),
        vec![GroupSetup::Member(log.clone())],
        Some(book_a),
        port_a,
        Vec::new(),
    );
    let bea_key = bea.public_key();
    let member = move |e: &NodeEvent| matches!(e, NodeEvent::MemberConnected { device, .. } if *device == bea_key);

    // Bea's first session runs on a runtime of its own, which then stops dead, as a
    // process that hangs or crashes: its connections stay open at Ana until they time out.
    let old_runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let (_old, _old_events) = {
        let _inside = old_runtime.enter();
        node(
            bea.clone(),
            vec![GroupSetup::Member(log.clone())],
            Some(book_b.clone()),
            0,
            vec![loopback(port_a)],
        )
    };
    assert!(seen(&mut events_a, 60, member).await, "Bea meets Ana");
    old_runtime.spawn(async { std::thread::sleep(Duration::from_secs(60)) });

    // Bea starts again at once.
    let (_new, mut events_new) = node(
        bea.clone(),
        vec![GroupSetup::Member(log)],
        Some(book_b),
        0,
        vec![loopback(port_a)],
    );
    assert!(seen(&mut events_a, 60, member).await, "Bea is back");
    // Past the time Ana gives the old session to answer (20 s).
    assert!(
        !seen(&mut events_new, 30, told).await,
        "a restart was taken for another computer"
    );
    old_runtime.shutdown_background();
}

#[tokio::test(flavor = "multi_thread")]
async fn two_copies_that_meet_tell_themselves() {
    let _one = ONE_AT_A_TIME.lock().await;
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let log = GroupLog::create(&bea, "g", 1).unwrap();
    let port_1 = free_port();
    let member = || vec![GroupSetup::Member(log.clone())];
    let (_one, mut events_1) = node(bea.clone(), member(), None, port_1, Vec::new());
    let (_two, mut events_2) = node(bea.clone(), member(), None, 0, vec![loopback(port_1)]);
    assert!(
        seen(&mut events_2, 60, told).await,
        "the copy that dialled is told"
    );
    assert!(
        seen(&mut events_1, 60, told).await,
        "the copy dialled is told"
    );
}
