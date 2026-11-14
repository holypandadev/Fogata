//! Rooms under pressure: many messages at once, and rooms when the group's log changes
//! under them. Nodes in one process, over the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::{FIRST_ROOM, GroupLog};
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
        spill_dir: None,
    })
    .unwrap()
}

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
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

/// Waits for `device`, which dials `address`, to connect to the node of `events`, dialling
/// again now and then: a dial by address alone is not tried again by itself.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = seen(
            events,
            10,
            |e| matches!(e, NodeEvent::MemberConnected { device: d, .. } if *d == device),
        )
        .await;
        if connected {
            return;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    panic!("timed out waiting for the member to connect");
}

/// Polls `check` until it holds, for up to `seconds`.
async fn eventually<F, Fut>(what: &str, seconds: u64, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let held = timeout(Duration::from_secs(seconds), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok();
    assert!(held, "timed out waiting for {what}");
}

/// Who `node` sees in the first voice room of `group`.
async fn in_room(node: &NodeHandle, group: GroupId) -> Vec<DevicePublicKey> {
    node.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .map(|r| r.participants.iter().map(|(d, _, _)| *d).collect())
        .unwrap_or_default()
}

/// Ana's group with Bea in it, both connected.
async fn two_members() -> (
    (Arc<DeviceIdentity>, NodeHandle, NodeEvents),
    (Arc<DeviceIdentity>, NodeHandle, NodeEvents),
    GroupId,
) {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let (node_b, events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    ((ana, node_a, events_a), (bea, node_b, events_b), group)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_messages_arrives_whole() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, _events_a), (bea, node_b, _events_b), group) = two_members().await;
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    let both = [ana.public_key(), bea.public_key()];
    eventually("both in the room", 30, || async {
        let seen = in_room(&node_b, group).await;
        both.iter().all(|d| seen.contains(d))
    })
    .await;
    // Pasted lines, sent as fast as they are written.
    const LINES: usize = 120;
    for i in 0..LINES {
        node_a.room_chat(group, format!("line {i}")).await.unwrap();
    }
    let count = || async {
        node_b
            .room(group, FIRST_ROOM)
            .await
            .unwrap()
            .map(|r| r.chat.len())
            .unwrap_or(0)
    };
    let all = timeout(Duration::from_secs(30), async {
        while count().await < LINES {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .is_ok();
    assert!(all, "Bea got {} of {LINES} lines", count().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn members_see_away_until_a_room_or_appearing_offline() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, _events_a), (_bea, _node_b, mut events_b), group) = two_members().await;
    let ana_key = ana.public_key();
    let presence = |hidden: bool, away: bool| {
        move |e: &NodeEvent| {
            matches!(e, NodeEvent::MemberPresence { device, hidden: h, away: a }
                if *device == ana_key && *h == hidden && *a == away)
        }
    };
    assert!(node_a.set_away(true).await.unwrap());
    assert!(seen(&mut events_b, 20, presence(false, true)).await, "away");
    assert!(!node_a.set_away(false).await.unwrap());
    assert!(
        seen(&mut events_b, 20, presence(false, false)).await,
        "back"
    );
    assert!(node_a.set_away(true).await.unwrap());
    assert!(
        seen(&mut events_b, 20, presence(false, true)).await,
        "away again"
    );
    // Someone in a room may just be listening: not away there.
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    assert!(
        seen(&mut events_b, 20, presence(false, false)).await,
        "in a room"
    );
    assert!(!node_a.set_away(true).await.unwrap());
    node_a.leave_room(group).await.unwrap();
    assert!(
        seen(&mut events_b, 20, presence(false, true)).await,
        "out of it"
    );
    // Appearing offline says nothing more.
    node_a.set_hidden(true).await.unwrap();
    assert!(
        seen(&mut events_b, 20, presence(true, false)).await,
        "hidden"
    );
    assert!(!node_a.set_away(true).await.unwrap());
}

/// Ana's log of `group` with one more change of hers, given to her node.
async fn change(
    node: &NodeHandle,
    ana: &DeviceIdentity,
    group: GroupId,
    change: impl FnOnce(&mut GroupLog, &DeviceIdentity),
) {
    let mut log = node.group(group).await.unwrap().unwrap();
    change(&mut log, ana);
    node.merge_group(log).await.unwrap();
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_member_leaves_the_room_at_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, _events_a), (bea, node_b, mut events_b), group) = two_members().await;
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    let (a, b) = (ana.public_key(), bea.public_key());
    eventually("both in the room", 30, || async {
        let seen = in_room(&node_a, group).await;
        seen.contains(&a) && seen.contains(&b)
    })
    .await;
    change(&node_a, &ana, group, |log, ana| {
        log.remove(ana, b, now()).unwrap();
    })
    .await;
    // Bea can no longer say anything in the group, a goodbye included.
    eventually("Bea out of Ana's room", 5, || async {
        in_room(&node_a, group).await == [a]
    })
    .await;
    assert!(
        seen(
            &mut events_b,
            30,
            |e| matches!(e, NodeEvent::Removed { group: g, .. } if *g == group)
        )
        .await
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_voice_room_closed_with_people_in_it_lets_them_go() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, _events_a), (bea, node_b, _events_b), group) = two_members().await;
    let gaming = [7u8; 8];
    change(&node_a, &ana, group, |log, ana| {
        log.append(
            ana,
            fogata_state::Body::SetRoom {
                id: gaming,
                name: "Gaming".into(),
            },
            now(),
        )
        .unwrap();
    })
    .await;
    eventually("the voice room at Bea's", 30, || async {
        node_b
            .group(group)
            .await
            .unwrap()
            .is_some_and(|log| log.state().rooms.len() == 2)
    })
    .await;
    node_b.join_room(group, gaming, "bea".into()).await.unwrap();
    eventually("Bea in Gaming at Ana's", 30, || async {
        node_a
            .room(group, gaming)
            .await
            .unwrap()
            .is_some_and(|r| r.participants.len() == 1)
    })
    .await;
    // A manager who saw it empty a moment before closes it.
    change(&node_a, &ana, group, |log, ana| {
        log.append(ana, fogata_state::Body::RemoveRoom { id: gaming }, now())
            .unwrap();
    })
    .await;
    eventually("Bea in no room", 30, || async {
        node_b
            .rooms(group)
            .await
            .unwrap()
            .iter()
            .all(|(_, r)| !r.joined)
    })
    .await;
    assert!(node_a.rooms(group).await.unwrap().is_empty());
    // She is free to go into the first one.
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    let b = bea.public_key();
    eventually("Bea in the first room at Ana's", 30, || async {
        in_room(&node_a, group).await == [b]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_coming_online_gets_a_room_bigger_than_one_answer() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    // A long evening while Bea was away: the longest messages, and videos.
    let long = "🔥".repeat(fogata_state::MAX_CHAT_CHARS);
    for _ in 0..500 {
        node_a.room_chat(group, long.clone()).await.unwrap();
    }
    for n in 0..40u8 {
        let file = fogata_state::FileInfo {
            id: [n; 32],
            name: format!("clip {n}.mp4"),
            size: 1000,
            video: Some(fogata_state::VideoPreview {
                thumb: vec![n; 60_000],
                width: 640,
                height: 360,
                duration_ms: 1000,
            }),
        };
        node_a.share_file(group, file).await.unwrap();
    }
    let (node_b, _events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    let whole = || async {
        node_b
            .room(group, FIRST_ROOM)
            .await
            .unwrap()
            .map(|r| (r.chat.len(), r.files.len()))
    };
    let got = timeout(Duration::from_secs(30), async {
        while whole().await != Some((500, 40)) {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await;
    assert!(got.is_ok(), "Bea has {:?} of (500, 40)", whole().await);
}

/// [`node`], for a device with a profile.
fn node_with_profile(
    identity: Arc<DeviceIdentity>,
    name: &str,
    log: GroupLog,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    let book = fogata_friends::FriendBook::new(&identity, name, 1).unwrap();
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
        spill_dir: None,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_name_shows_in_the_room_under_it() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) =
        node_with_profile(ana.clone(), "Ana", log.clone(), port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    node_a
        .join_room(group, FIRST_ROOM, "Ana".into())
        .await
        .unwrap();
    let a = ana.public_key();
    let nick = || async {
        node_b
            .room(group, FIRST_ROOM)
            .await
            .unwrap()
            .and_then(|r| r.participants.into_iter().find(|(d, _, _)| *d == a))
            .map(|(_, nick, _)| nick)
    };
    eventually("Ana in the room at Bea's", 30, || async {
        nick().await.as_deref() == Some("Ana")
    })
    .await;
    // Bea may know Ana only by what she says in the room (no connection between them, the
    // room's events reaching her through others): the room says the new name.
    node_a
        .set_profile(Some("Ana María".into()), None)
        .await
        .unwrap();
    eventually("the new name in the room", 10, || async {
        nick().await.as_deref() == Some("Ana María")
    })
    .await;
}

/// Started again after a crash, a device that enters the room at once, before it hears that
/// the others still list its earlier session there, stays in the room.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_back_from_a_crash_that_enters_at_once_stays_in() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (a, b) = (ana.public_key(), bea.public_key());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, b, "bea", 2).unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana, log.clone(), port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), log.clone(), 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, b).await;
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    eventually("both in the room", 30, || async {
        let seen = in_room(&node_a, group).await;
        seen.contains(&a) && seen.contains(&b)
    })
    .await;
    // No goodbye: the node just stops, and starts again.
    node_b.shutdown().await.unwrap();
    drop(node_b);
    let (node_b, _events_b) = node(bea, log, 0, vec![loopback(port_a)]);
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, b).await;
    // Long enough for Ana to hear of Bea's earlier session and count it gone: Bea stays in.
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(in_room(&node_b, group).await.contains(&b));
    assert!(in_room(&node_a, group).await.contains(&b));
    node_b.room_chat(group, "back".into()).await.unwrap();
    eventually("the line at Ana's", 10, || async {
        node_a
            .room(group, FIRST_ROOM)
            .await
            .unwrap()
            .is_some_and(|r| r.chat.iter().any(|l| l.text == "back"))
    })
    .await;
}

/// Four members, each connected to the others: every line any of them writes reaches every
/// one, each once.
#[tokio::test(flavor = "multi_thread")]
async fn lines_spread_to_every_member_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let people: Vec<Arc<DeviceIdentity>> = (0..4)
        .map(|_| Arc::new(DeviceIdentity::generate().unwrap()))
        .collect();
    let mut log = GroupLog::create(&people[0], "g", 1).unwrap();
    for (n, person) in people.iter().enumerate().skip(1) {
        log.add(
            &people[0],
            person.public_key(),
            &format!("p{n}"),
            n as u64 + 1,
        )
        .unwrap();
    }
    let group = log.group_id();
    let port_a = free_port();
    let (first, mut events_a) = node(people[0].clone(), log.clone(), port_a, Vec::new());
    let mut nodes = vec![first];
    let mut kept = Vec::new();
    for person in &people[1..] {
        let (handle, events) = node(person.clone(), log.clone(), 0, vec![loopback(port_a)]);
        wait_to_connect(
            &handle,
            loopback(port_a),
            &mut events_a,
            person.public_key(),
        )
        .await;
        nodes.push(handle);
        kept.push(events);
    }
    for (n, handle) in nodes.iter().enumerate() {
        handle
            .join_room(group, FIRST_ROOM, format!("p{n}"))
            .await
            .unwrap();
    }
    // The others are found through the first one's roster.
    for handle in &nodes {
        eventually("everyone in the room everywhere", 60, || async {
            in_room(handle, group).await.len() == people.len()
        })
        .await;
    }
    const LINES: usize = 15;
    for i in 0..LINES {
        for (n, handle) in nodes.iter().enumerate() {
            handle
                .room_chat(group, format!("line {i} from p{n}"))
                .await
                .unwrap();
        }
    }
    let all = LINES * people.len();
    for (n, handle) in nodes.iter().enumerate() {
        let chat = || async {
            handle
                .room(group, FIRST_ROOM)
                .await
                .unwrap()
                .map(|r| r.chat)
                .unwrap_or_default()
        };
        let whole = timeout(Duration::from_secs(30), async {
            while chat().await.len() < all {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
        .await
        .is_ok();
        let lines = chat().await;
        assert!(whole, "p{n} has {} of {all} lines", lines.len());
        let mut ids: Vec<[u8; 16]> = lines.iter().map(|l| l.message_id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all, "p{n} has each line once");
    }
}
