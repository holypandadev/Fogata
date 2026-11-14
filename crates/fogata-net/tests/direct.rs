// Two devices that are already friends, on the same machine: a direct chat's two-person
// group made by one reaches the other over their friend connection, both prove it, and
// sealed messages go from one to the other only, in the order they were sent. A group that
// is not a chat of the two is never offered as one.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Friend, FriendBook};
use fogata_net::{Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::GroupLog;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What goes over the friend connection must not wait for the 20 s heartbeat.
const PROMPT: Duration = Duration::from_secs(10);

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

/// Two books that already hold each other as friends.
fn friends(ana: &DeviceIdentity, bea: &DeviceIdentity) -> (FriendBook, FriendBook) {
    let mut book_a = FriendBook::new(ana, "Ana", 1).unwrap();
    let mut book_b = FriendBook::new(bea, "Bea", 1).unwrap();
    book_a.friends.insert(bea.public_key(), friend_of(&book_b));
    book_b.friends.insert(ana.public_key(), friend_of(&book_a));
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

/// Whether `done` holds for an event that comes within `limit`.
async fn comes(
    events: &mut NodeEvents,
    limit: Duration,
    mut done: impl FnMut(&NodeEvent) -> bool,
) -> bool {
    timeout(limit, async {
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
async fn a_direct_chat_goes_between_the_two_friends() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;
    wait_for(&mut events_b, Duration::from_secs(60), "Ana online", online).await;

    // Ana makes the chat's group: it reaches Bea as a chat, not as a group invitation.
    let mut log = GroupLog::create(&ana, "Direct", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    let group = log.group_id();
    let nothing = node_a
        .send_direct(group, bea.public_key(), vec![b"too soon".to_vec()])
        .await
        .unwrap();
    assert!(nothing.is_err(), "no chat runs yet");
    node_a.add_direct(log).await.unwrap();
    let mut offered = None;
    wait_for(&mut events_b, PROMPT, "the chat's group", |e| match e {
        NodeEvent::DirectOffered { log, by } if *by == ana.public_key() => {
            offered = Some(log.clone());
            true
        }
        NodeEvent::GroupOffered { .. } => panic!("a chat's group came as a group"),
        _ => false,
    })
    .await;
    let offered = offered.unwrap();
    assert_eq!(offered.group_id(), group);
    node_b.add_direct(offered).await.unwrap();
    let connected =
        |e: &NodeEvent| matches!(e, NodeEvent::MemberConnected { group: g, .. } if *g == group);
    wait_for(&mut events_a, PROMPT, "Bea in the chat", connected).await;
    wait_for(&mut events_b, PROMPT, "Ana in the chat", connected).await;

    // Sealed with the chat's key and opened by Bea, from Ana.
    node_a
        .send_direct(
            group,
            bea.public_key(),
            vec![b"hi".to_vec(), b"how are you".to_vec()],
        )
        .await
        .unwrap()
        .unwrap();
    let mut got = Vec::new();
    wait_for(&mut events_b, PROMPT, "Ana's messages", |e| match e {
        NodeEvent::Direct {
            group: g,
            from,
            payloads,
        } if *g == group && *from == ana.public_key() => {
            got.extend(payloads.iter().cloned());
            got.len() >= 2
        }
        _ => false,
    })
    .await;
    assert_eq!(got, [b"hi".to_vec(), b"how are you".to_vec()]);
    node_b
        .send_direct(group, ana.public_key(), vec![b"fine".to_vec()])
        .await
        .unwrap()
        .unwrap();
    wait_for(&mut events_a, PROMPT, "Bea's answer", |e| {
        matches!(e, NodeEvent::Direct { from, payloads, .. } if *from == bea.public_key() && payloads[0] == b"fine")
    })
    .await;

    // Sent one after another, each on its own, they come in the order they were sent.
    let sent: Vec<Vec<u8>> = (0..60).map(|n| format!("linea {n}").into_bytes()).collect();
    for payload in &sent {
        node_a
            .send_direct(group, bea.public_key(), vec![payload.clone()])
            .await
            .unwrap()
            .unwrap();
    }
    let mut got = Vec::new();
    wait_for(&mut events_b, PROMPT, "Ana's lines", |e| match e {
        NodeEvent::Direct { from, payloads, .. } if *from == ana.public_key() => {
            got.extend(payloads.iter().cloned());
            got.len() >= sent.len()
        }
        _ => false,
    })
    .await;
    let text = |lines: &[Vec<u8>]| -> Vec<String> {
        lines
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    };
    assert_eq!(text(&got), text(&sent));

    // Bea runs the chat's group as Ana made it.
    let runs = node_b.group(group).await.unwrap().unwrap();
    assert_eq!(runs.state().members.len(), 2);
    drop(node_a);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_with_someone_else_in_it_is_no_chat() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let carla = DeviceIdentity::generate().unwrap();
    let (book_a, book_b) = friends(&ana, &bea);
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;
    wait_for(&mut events_b, Duration::from_secs(60), "Ana online", online).await;

    let mut log = GroupLog::create(&ana, "Direct", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    log.add(&ana, carla.public_key(), "Carla", 3).unwrap();
    node_a.add_direct(log).await.unwrap();
    let offered = comes(&mut events_b, Duration::from_secs(5), |e| {
        matches!(
            e,
            NodeEvent::DirectOffered { .. } | NodeEvent::GroupOffered { .. }
        )
    })
    .await;
    assert!(!offered, "a three-person group was offered as a chat");
    drop(node_b);
}
