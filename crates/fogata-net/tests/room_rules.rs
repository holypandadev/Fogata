//! Who may enter a voice room: managers take members out of a room, keep them out for a while
//! or until let back, and keep a room for some members. Nodes in one process, over the
//! loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_net::{Barrier, GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::{Body, FIRST_ROOM, GroupLog, VoiceRoomId};
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

/// Who `node` sees in voice room `voice` of `group`.
async fn people(node: &NodeHandle, group: GroupId, voice: VoiceRoomId) -> Vec<DevicePublicKey> {
    node.room(group, voice)
        .await
        .unwrap()
        .map(|r| r.participants.iter().map(|(d, _, _)| *d).collect())
        .unwrap_or_default()
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

type Running = (Arc<DeviceIdentity>, NodeHandle, NodeEvents);

/// Ana's group with `others` more members (Bea, Leo…), each connected to Ana, and a second
/// voice room `[1; 8]`.
async fn group_of(others: usize) -> (Running, Vec<Running>, GroupId, GroupLog) {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let devices: Vec<Arc<DeviceIdentity>> = (0..others)
        .map(|_| Arc::new(DeviceIdentity::generate().unwrap()))
        .collect();
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    for (i, d) in devices.iter().enumerate() {
        log.add(&ana, d.public_key(), "x", 2 + i as u64).unwrap();
    }
    log.append(
        &ana,
        Body::SetRoom {
            id: [1; 8],
            name: "Gaming".into(),
        },
        9,
    )
    .unwrap();
    let group = log.group_id();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, Vec::new());
    let mut out = Vec::new();
    for d in devices {
        let (n, e) = node(d.clone(), log.clone(), 0, vec![loopback(port_a)]);
        wait_to_connect(&n, loopback(port_a), &mut events_a, d.public_key()).await;
        out.push((d, n, e));
    }
    ((ana, node_a, events_a), out, group, log)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_kept_out_leaves_and_comes_back_once_let_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, mut events_a), others, group, _) = group_of(2).await;
    let [(bea, node_b, _), (leo, node_l, mut events_l)] =
        <[Running; 2]>::try_from(others).ok().unwrap();
    let (a, b, l) = (ana.public_key(), bea.public_key(), leo.public_key());
    for (node, nick) in [(&node_a, "ana"), (&node_b, "bea"), (&node_l, "leo")] {
        node.join_room(group, FIRST_ROOM, nick.into())
            .await
            .unwrap();
    }
    eventually("all three in the room", 30, || async {
        let seen = people(&node_a, group, FIRST_ROOM).await;
        let seen_b = people(&node_b, group, FIRST_ROOM).await;
        [a, b, l]
            .iter()
            .all(|d| seen.contains(d) && seen_b.contains(d))
    })
    .await;

    change(&node_a, &ana, group, |log, ana| {
        log.keep_out(ana, FIRST_ROOM, l, None, now()).unwrap();
    })
    .await;
    // Leo is told why, leaves, and the others stop showing him.
    assert!(
        seen(&mut events_l, 20, |e| matches!(
            e,
            NodeEvent::TakenOut { group: g, voice, by: Some(by), barrier: Some(Barrier::TimedOut(k)) }
                if *g == group && *voice == FIRST_ROOM && *by == a && k.until_ms.is_none()
        ))
        .await
    );
    eventually("Leo out of the room everywhere", 20, || async {
        !people(&node_a, group, FIRST_ROOM).await.contains(&l)
            && !people(&node_b, group, FIRST_ROOM).await.contains(&l)
            && !people(&node_l, group, FIRST_ROOM).await.contains(&l)
    })
    .await;
    // His node does not take him back in, and the others take nothing of a call from him.
    node_l
        .join_room(group, FIRST_ROOM, "leo".into())
        .await
        .unwrap();
    node_l.signal(group, a, b"offer".to_vec()).await.unwrap();
    node_b.signal(group, a, b"offer".to_vec()).await.unwrap();
    let mut from_leo = false;
    let mut from_bea = false;
    let _ = timeout(Duration::from_secs(4), async {
        while let Some(event) = events_a.recv().await {
            if let NodeEvent::Signal { from, .. } = event {
                from_leo |= from == l;
                from_bea |= from == b;
            }
        }
    })
    .await;
    assert!(from_bea, "a signal from Bea should arrive");
    assert!(!from_leo, "nothing of a call from Leo while he is kept out");
    assert!(!people(&node_a, group, FIRST_ROOM).await.contains(&l));
    // The other voice room is still his.
    node_l.join_room(group, [1; 8], "leo".into()).await.unwrap();
    eventually("Leo in the other room", 20, || async {
        people(&node_a, group, [1; 8]).await.contains(&l)
    })
    .await;

    change(&node_a, &ana, group, |log, ana| {
        log.let_back(ana, FIRST_ROOM, l, now()).unwrap();
    })
    .await;
    eventually("the rule lifted at Leo's", 20, || async {
        let log = node_l.group(group).await.unwrap().unwrap();
        log.state().barrier(&FIRST_ROOM, &l, now()).is_none()
    })
    .await;
    node_l
        .join_room(group, FIRST_ROOM, "leo".into())
        .await
        .unwrap();
    eventually("Leo back in the room", 20, || async {
        people(&node_a, group, FIRST_ROOM).await.contains(&l)
            && people(&node_b, group, FIRST_ROOM).await.contains(&l)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_taken_out_of_a_room_leaves_it_and_may_come_back_at_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, _events_a), others, group, _) = group_of(1).await;
    let [(bea, node_b, mut events_b)] = <[Running; 1]>::try_from(others).ok().unwrap();
    let (a, b) = (ana.public_key(), bea.public_key());
    node_a
        .join_room(group, FIRST_ROOM, "ana".into())
        .await
        .unwrap();
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    eventually("both in the room", 30, || async {
        people(&node_a, group, FIRST_ROOM).await.len() == 2
    })
    .await;
    node_a.remove_from_room(group, FIRST_ROOM, b).await.unwrap();
    assert!(
        seen(&mut events_b, 20, |e| matches!(
            e,
            NodeEvent::TakenOut { by: Some(by), barrier: None, .. } if *by == a
        ))
        .await
    );
    eventually("Bea out of Ana's room", 10, || async {
        people(&node_a, group, FIRST_ROOM).await == [a]
    })
    .await;
    node_b
        .join_room(group, FIRST_ROOM, "bea".into())
        .await
        .unwrap();
    eventually("Bea back in", 20, || async {
        people(&node_a, group, FIRST_ROOM).await.contains(&b)
    })
    .await;
    // A member takes nobody out: Bea's removal of Ana does nothing.
    node_b.remove_from_room(group, FIRST_ROOM, a).await.unwrap();
    // Given time to, it does not.
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(people(&node_b, group, FIRST_ROOM).await.contains(&a));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_room_for_some_lets_in_only_those_chosen() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ((ana, node_a, _events_a), others, group, _) = group_of(2).await;
    let [(bea, node_b, _), (leo, node_l, _)] = <[Running; 2]>::try_from(others).ok().unwrap();
    let (b, l) = (bea.public_key(), leo.public_key());
    let gaming = [1; 8];
    change(&node_a, &ana, group, |log, ana| {
        log.set_room_access(ana, gaming, Some(vec![b]), now())
            .unwrap();
    })
    .await;
    eventually("the rule at Leo's", 20, || async {
        let log = node_l.group(group).await.unwrap().unwrap();
        log.state().barrier(&gaming, &l, now()) == Some(Barrier::NotChosen)
    })
    .await;
    node_a.join_room(group, gaming, "ana".into()).await.unwrap();
    node_b.join_room(group, gaming, "bea".into()).await.unwrap();
    node_l.join_room(group, gaming, "leo".into()).await.unwrap();
    eventually("Ana and Bea in the room", 20, || async {
        people(&node_a, group, gaming).await.len() == 2
    })
    .await;
    // Given time to, Leo does not go in.
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!people(&node_a, group, gaming).await.contains(&l));
    // Open to everyone again: Leo goes in.
    change(&node_a, &ana, group, |log, ana| {
        log.set_room_access(ana, gaming, None, now()).unwrap();
    })
    .await;
    eventually("the rule lifted at Leo's", 20, || async {
        let log = node_l.group(group).await.unwrap().unwrap();
        log.state().barrier(&gaming, &l, now()).is_none()
    })
    .await;
    node_l.join_room(group, gaming, "leo".into()).await.unwrap();
    eventually("Leo in", 20, || async {
        people(&node_a, group, gaming).await.contains(&l)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rules_made_while_a_member_was_away_reach_it_when_it_is_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let leo = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    log.add(&ana, leo.public_key(), "leo", 3).unwrap();
    let group = log.group_id();
    // Bea's copy is from before the rule; Ana's has it, the rest of the log the same.
    let before = log.clone();
    log.keep_out(
        &ana,
        FIRST_ROOM,
        leo.public_key(),
        Some(now() + 3_600_000),
        now(),
    )
    .unwrap();
    let port_a = free_port();
    let (_node_a, mut events_a) = node(ana.clone(), log, port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), before, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    eventually("the rule at Bea's", 40, || async {
        let log = node_b.group(group).await.unwrap().unwrap();
        matches!(
            log.state().barrier(&FIRST_ROOM, &leo.public_key(), now()),
            Some(Barrier::TimedOut(_))
        )
    })
    .await;
    // The rule came with the rest of the log, as part of it.
    let at_b = node_b.group(group).await.unwrap().unwrap();
    assert_eq!(
        at_b.entries_in_order()
            .iter()
            .filter(|e| e.body.is_room_rule())
            .count(),
        1
    );
    assert!(at_b.waiting().is_empty());
}
