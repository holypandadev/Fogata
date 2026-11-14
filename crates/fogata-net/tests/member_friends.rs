// Members of a group who are not friends, on the same machine: one sends the other a friend
// request from the group, with no link or code; the other sees it as any request and, by
// accepting it, makes the two friends as a link would. A device that shares no group with
// another cannot ask it this way.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_friends::FriendBook;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::GroupLog;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn node(
    identity: Arc<DeviceIdentity>,
    name: &str,
    groups: Vec<GroupSetup>,
    listen_port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    let book = FriendBook::new(&identity, name, 1).unwrap();
    fogata_net::start(NodeConfig {
        identity,
        groups,
        friends: Some(book),
        listen_port,
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

/// Waits up to `limit` for an event that `done` picks, and returns it.
async fn wait_for(
    events: &mut NodeEvents,
    limit: Duration,
    what: &str,
    mut done: impl FnMut(&NodeEvent) -> bool,
) -> NodeEvent {
    let found = timeout(limit, async {
        while let Some(event) = events.recv().await {
            if done(&event) {
                return Some(event);
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    found.unwrap_or_else(|| panic!("timed out waiting for {what}"))
}

/// Polls `node`'s friend book until `done` holds.
async fn until_book(node: &NodeHandle, what: &str, done: impl Fn(&FriendBook) -> bool) {
    let found = timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(Some(book)) = node.friends().await
                && done(&book)
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
async fn a_member_asks_another_to_be_friends_and_accepting_makes_them_friends() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (a, b) = (ana.public_key(), bea.public_key());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, b, "bea", 2).unwrap();
    let port_a = free_port();
    let groups = |log: &GroupLog| vec![GroupSetup::Member(log.clone())];
    let (node_a, mut events_a) = node(ana.clone(), "Ana", groups(&log), port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), "Bea", groups(&log), 0, vec![loopback(port_a)]);
    wait_for(
        &mut events_a,
        Duration::from_secs(60),
        "Bea connected",
        |e| matches!(e, NodeEvent::MemberConnected { device, .. } if *device == b),
    )
    .await;
    wait_for(
        &mut events_b,
        Duration::from_secs(60),
        "Ana connected",
        |e| matches!(e, NodeEvent::MemberConnected { device, .. } if *device == a),
    )
    .await;

    // Ana asks Bea from the group: the request waits at Bea's for an answer, listed at Ana's
    // as one she sent.
    node_a.ask_member_friend(b, "Bea".into()).await.unwrap();
    let request = wait_for(&mut events_b, Duration::from_secs(20), "the request", |e| {
        matches!(e, NodeEvent::FriendRequest { .. })
    })
    .await;
    let NodeEvent::FriendRequest { profile } = request else {
        unreachable!()
    };
    assert_eq!((profile.device, profile.name.as_str()), (a, "Ana"));
    wait_for(&mut events_a, Duration::from_secs(20), "delivered", |e| {
        matches!(e, NodeEvent::FriendRequestSent { device, name } if *device == b && name == "Bea")
    })
    .await;
    until_book(&node_a, "the request listed at Ana's", |book| {
        book.outgoing.contains_key(&b)
    })
    .await;
    until_book(&node_b, "the request listed at Bea's", |book| {
        book.incoming.contains_key(&a)
    })
    .await;

    // Asking again does not show Bea a second request.
    node_a.ask_member_friend(b, "Bea".into()).await.unwrap();
    let again = timeout(Duration::from_secs(3), async {
        while let Some(event) = events_b.recv().await {
            if matches!(event, NodeEvent::FriendRequest { .. }) {
                return true;
            }
        }
        false
    })
    .await;
    assert!(!matches!(again, Ok(true)), "a second request was shown");

    // Bea accepts: the two are friends, both secrets delivered, as with a link.
    node_b.answer_friend(a, true).await.unwrap();
    wait_for(
        &mut events_a,
        Duration::from_secs(30),
        "Ana's new friend",
        |e| matches!(e, NodeEvent::FriendAdded { profile } if profile.device == b),
    )
    .await;
    until_book(&node_a, "Bea a delivered friend at Ana's", |book| {
        book.outgoing.is_empty() && book.friends.get(&b).is_some_and(|f| f.delivered)
    })
    .await;
    until_book(&node_b, "Ana a delivered friend at Bea's", |book| {
        book.incoming.is_empty() && book.friends.get(&a).is_some_and(|f| f.delivered)
    })
    .await;
    wait_for(
        &mut events_a,
        Duration::from_secs(30),
        "Bea online",
        |e| matches!(e, NodeEvent::FriendOnline { device, .. } if *device == b),
    )
    .await;
    node_a.shutdown().await.unwrap();
    node_b.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_that_shares_no_group_cannot_ask_this_way() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let cid = Arc::new(DeviceIdentity::generate().unwrap());
    let (a, c) = (ana.public_key(), cid.public_key());
    // Each in a group of its own, connected to each other.
    let ana_log = GroupLog::create(&ana, "g", 1).unwrap();
    let cid_log = GroupLog::create(&cid, "h", 1).unwrap();
    let port_a = free_port();
    let (node_a, mut events_a) = node(
        ana.clone(),
        "Ana",
        vec![GroupSetup::Member(ana_log)],
        port_a,
        Vec::new(),
    );
    let (node_c, mut events_c) = node(
        cid.clone(),
        "Cid",
        vec![GroupSetup::Member(cid_log)],
        0,
        vec![loopback(port_a)],
    );
    let ana_peer = node_a.local_peer();
    let connected = timeout(Duration::from_secs(30), async {
        while !node_c.identified_peers().contains(&ana_peer) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(connected.is_ok(), "Cid never connected");

    // Cid is not a member Ana proved: its own device cannot even send the request.
    node_c.ask_member_friend(a, "Ana".into()).await.unwrap();
    wait_for(
        &mut events_c,
        Duration::from_secs(10),
        "the failure",
        |e| matches!(e, NodeEvent::FriendRequestFailed(reason) if reason.contains("not connected")),
    )
    .await;
    until_book(&node_c, "no request left at Cid's", |book| {
        book.outgoing.is_empty()
    })
    .await;
    let shown = timeout(Duration::from_secs(3), async {
        while let Some(event) = events_a.recv().await {
            if matches!(event, NodeEvent::FriendRequest { .. }) {
                return true;
            }
        }
        false
    })
    .await;
    assert!(!matches!(shown, Ok(true)), "Ana was shown a request");
    let book = node_a.friends().await.unwrap().unwrap();
    assert!(!book.incoming.contains_key(&c));
    node_a.shutdown().await.unwrap();
    node_c.shutdown().await.unwrap();
}
