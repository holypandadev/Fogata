//! Typing indicators between nodes in one process, over the loopback interface: they reach
//! the people of the chat only, and what arrives from anyone else, too often or out of turn
//! is dropped.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_friends::{Friend, FriendBook};
use fogata_net::{
    FIRST_ROOM, GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle, PeerId,
    TYPING_PROTOCOL, TypingSignal,
};
use fogata_state::GroupLog;
use futures::AsyncWriteExt as _;
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
    logs: Vec<GroupLog>,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: logs.into_iter().map(GroupSetup::Member).collect(),
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

fn loopback(port: u16, peer: Option<PeerId>) -> Multiaddr {
    let base = format!("/ip4/127.0.0.1/udp/{port}/quic-v1");
    match peer {
        Some(peer) => format!("{base}/p2p/{peer}").parse().unwrap(),
        None => base.parse().unwrap(),
    }
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

/// The typing signals that come within `seconds`, as (group, from, typing).
async fn typing_within(
    events: &mut NodeEvents,
    seconds: u64,
) -> Vec<(GroupId, DevicePublicKey, bool)> {
    let mut got = Vec::new();
    let _ = timeout(Duration::from_secs(seconds), async {
        while let Some(event) = events.recv().await {
            if let NodeEvent::Typing {
                group,
                from,
                typing,
                ..
            } = event
            {
                got.push((group, from, typing));
            }
        }
    })
    .await;
    got
}

/// Waits until `device` is connected as a member of `group` on the node of `events`,
/// dialling again now and then.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    group: GroupId,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = seen(events, 10, |e| {
            matches!(e, NodeEvent::MemberConnected { group: g, device: d, .. } if *g == group && *d == device)
        })
        .await;
        if connected {
            return;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    panic!("timed out waiting for the member to connect");
}

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

/// Sends `signal` to `to` as it goes on the wire, from whichever node `from` is. The
/// receiver may close a stream unread (too many at once): that is no failure here.
async fn raw(from: &NodeHandle, to: PeerId, signal: TypingSignal) {
    let mut stream = from
        .streams()
        .open_stream(to, TYPING_PROTOCOL)
        .await
        .unwrap();
    if stream.write_all(&signal.encode()).await.is_ok() {
        let _ = stream.close().await;
    }
}

/// Ana in a room of `group` with the others, all seen by everyone.
async fn all_in_room(nodes: &[(&NodeHandle, &str, DevicePublicKey)], group: GroupId) {
    for (node, nick, _) in nodes {
        node.join_room(group, FIRST_ROOM, nick.to_string())
            .await
            .unwrap();
    }
    for (node, _, _) in nodes {
        eventually("everyone in the room", 30, || async {
            let seen = in_room(node, group).await;
            nodes.iter().all(|(_, _, d)| seen.contains(d))
        })
        .await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn typing_in_a_room_reaches_the_room_and_no_one_else() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let cleo = Arc::new(DeviceIdentity::generate().unwrap());
    // Ana and Bea's group, and a group of Ana and Cleo: Cleo is a member Ana knows, but
    // not of the first.
    let mut ours = GroupLog::create(&ana, "ours", 1).unwrap();
    ours.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let mut other = GroupLog::create(&ana, "other", 1).unwrap();
    other.add(&ana, cleo.public_key(), "cleo", 2).unwrap();
    let (g1, g2) = (ours.group_id(), other.group_id());
    let port_a = free_port();
    let (node_a, mut events_a) = node(
        ana.clone(),
        vec![ours.clone(), other.clone()],
        port_a,
        vec![],
    );
    let (node_b, mut events_b) = node(bea.clone(), vec![ours], 0, vec![loopback(port_a, None)]);
    wait_to_connect(
        &node_b,
        loopback(port_a, None),
        &mut events_a,
        g1,
        bea.public_key(),
    )
    .await;
    let (node_c, _events_c) = node(cleo.clone(), vec![other], 0, vec![loopback(port_a, None)]);
    wait_to_connect(
        &node_c,
        loopback(port_a, None),
        &mut events_a,
        g2,
        cleo.public_key(),
    )
    .await;
    let a = node_a.local_peer();
    // Both know the other takes typing signals.
    eventually("Bea takes typing signals", 10, || async {
        node_a.advertises(&node_b.local_peer(), &TYPING_PROTOCOL)
            && node_b.advertises(&a, &TYPING_PROTOCOL)
    })
    .await;

    // Nobody is in a room yet: nothing goes, and a signal about it is not shown.
    node_b.typing(g1, true).await.unwrap();
    raw(
        &node_b,
        a,
        TypingSignal {
            group: g1,
            voice: FIRST_ROOM,
            typing: true,
            seq: 1_000,
        },
    )
    .await;
    assert!(typing_within(&mut events_a, 2).await.is_empty());

    all_in_room(
        &[
            (&node_a, "ana", ana.public_key()),
            (&node_b, "bea", bea.public_key()),
        ],
        g1,
    )
    .await;
    // Past the gap between two "typing" for one chat.
    tokio::time::sleep(Duration::from_secs(2)).await;
    node_b.typing(g1, true).await.unwrap();
    let got = typing_within(&mut events_a, 3).await;
    assert_eq!(got, [(g1, bea.public_key(), true)]);
    node_b.typing(g1, false).await.unwrap();
    let got = typing_within(&mut events_a, 3).await;
    assert_eq!(got, [(g1, bea.public_key(), false)]);
    // And back.
    node_a.typing(g1, true).await.unwrap();
    assert!(
        seen(&mut events_b, 3, |e| matches!(e, NodeEvent::Typing { from, typing: true, voice: Some(v), .. } if *from == ana.public_key() && *v == FIRST_ROOM)).await
    );
    // A second "stopped" says nothing new: nothing goes.
    node_b.typing(g1, false).await.unwrap();
    assert!(typing_within(&mut events_a, 2).await.is_empty());

    // Cleo is a member of another group of Ana's, not of this one: refused.
    raw(
        &node_c,
        a,
        TypingSignal {
            group: g1,
            voice: FIRST_ROOM,
            typing: true,
            seq: 5_000,
        },
    )
    .await;
    // About another voice room than the one Ana is in: not shown either.
    raw(
        &node_b,
        a,
        TypingSignal {
            group: g1,
            voice: [9; 8],
            typing: true,
            seq: 5_001,
        },
    )
    .await;
    assert!(typing_within(&mut events_a, 2).await.is_empty());

    // Older than one already heard (they overtook each other): dropped. First the burst of
    // signals Ana takes from Bea fills up again (one a second).
    tokio::time::sleep(Duration::from_secs(5)).await;
    for seq in [9_000, 8_999] {
        raw(
            &node_b,
            a,
            TypingSignal {
                group: g1,
                voice: FIRST_ROOM,
                typing: seq == 8_999,
                seq,
            },
        )
        .await;
    }
    let got = typing_within(&mut events_a, 2).await;
    assert_eq!(got, [(g1, bea.public_key(), false)]);

    // A flood: a few at once, then about one a second. First the burst fills up again.
    tokio::time::sleep(Duration::from_secs(5)).await;
    for seq in 10_000..10_040 {
        raw(
            &node_b,
            a,
            TypingSignal {
                group: g1,
                voice: FIRST_ROOM,
                typing: true,
                seq,
            },
        )
        .await;
    }
    let got = typing_within(&mut events_a, 2).await;
    assert!(
        (1..=7).contains(&got.len()),
        "{} of 40 signals shown",
        got.len()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_member_gets_no_signal_and_sends_none() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    // Two groups of the two, so Bea stays connected after she is removed from one.
    let mut ours = GroupLog::create(&ana, "ours", 1).unwrap();
    ours.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let mut other = GroupLog::create(&ana, "other", 1).unwrap();
    other.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let (g1, g2) = (ours.group_id(), other.group_id());
    let port_a = free_port();
    let (node_a, mut events_a) = node(
        ana.clone(),
        vec![ours.clone(), other.clone()],
        port_a,
        vec![],
    );
    let (node_b, mut events_b) = node(
        bea.clone(),
        vec![ours, other],
        0,
        vec![loopback(port_a, None)],
    );
    wait_to_connect(
        &node_b,
        loopback(port_a, None),
        &mut events_a,
        g1,
        bea.public_key(),
    )
    .await;
    all_in_room(
        &[
            (&node_a, "ana", ana.public_key()),
            (&node_b, "bea", bea.public_key()),
        ],
        g1,
    )
    .await;

    // Ana removes Bea from the first group.
    let mut log = node_a.group(g1).await.unwrap().unwrap();
    log.remove(&ana, bea.public_key(), 3).unwrap();
    node_a.merge_group(log).await.unwrap();
    eventually("Bea out of Ana's room", 30, || async {
        !in_room(&node_a, g1).await.contains(&bea.public_key())
    })
    .await;
    assert!(node_a.is_member_of(&node_b.local_peer(), &g2));

    node_a.typing(g1, true).await.unwrap();
    raw(
        &node_b,
        node_a.local_peer(),
        TypingSignal {
            group: g1,
            voice: FIRST_ROOM,
            typing: true,
            seq: 1,
        },
    )
    .await;
    let to_bea = typing_within(&mut events_b, 3).await;
    let to_ana = typing_within(&mut events_a, 1).await;
    assert!(to_bea.is_empty(), "Bea was told: {to_bea:?}");
    assert!(to_ana.is_empty(), "Ana showed: {to_ana:?}");
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

fn friend_node(
    identity: Arc<DeviceIdentity>,
    book: FriendBook,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: Vec::new(),
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
async fn typing_in_a_direct_chat_and_none_while_appearing_offline() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut book_a = FriendBook::new(&ana, "Ana", 1).unwrap();
    let mut book_b = FriendBook::new(&bea, "Bea", 1).unwrap();
    book_a.friends.insert(bea.public_key(), friend_of(&book_b));
    book_b.friends.insert(ana.public_key(), friend_of(&book_a));
    let port_a = free_port();
    let (node_a, mut events_a) = friend_node(ana.clone(), book_a, port_a, vec![]);
    let (node_b, mut events_b) = friend_node(bea.clone(), book_b, 0, vec![]);
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    let address = loopback(port_a, Some(node_a.local_peer()));
    let mut met = false;
    for _ in 0..6 {
        node_b.dial(vec![address.clone()], None).await.unwrap();
        if seen(&mut events_a, 10, online).await {
            met = true;
            break;
        }
    }
    assert!(met, "the two friends never met");

    let mut log = GroupLog::create(&ana, "Direct", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    let group = log.group_id();
    node_a.add_direct(log).await.unwrap();
    let mut offered = None;
    assert!(
        seen(&mut events_b, 30, |e| match e {
            NodeEvent::DirectOffered { log, .. } => {
                offered = Some(log.clone());
                true
            }
            _ => false,
        })
        .await
    );
    node_b.add_direct(offered.unwrap()).await.unwrap();
    let connected =
        |e: &NodeEvent| matches!(e, NodeEvent::MemberConnected { group: g, .. } if *g == group);
    assert!(seen(&mut events_a, 30, connected).await);
    eventually("Ana takes typing signals", 10, || async {
        node_b.advertises(&node_a.local_peer(), &TYPING_PROTOCOL)
    })
    .await;

    node_b.typing(group, true).await.unwrap();
    assert!(
        seen(&mut events_a, 5, |e| matches!(e, NodeEvent::Typing { group: g, voice: None, from, typing: true } if *g == group && *from == bea.public_key())).await
    );
    node_b.typing(group, false).await.unwrap();
    assert!(
        seen(&mut events_a, 5, |e| matches!(
            e,
            NodeEvent::Typing { typing: false, .. }
        ))
        .await
    );

    // Typing when she turns on appearing offline: "stopped" goes first.
    node_b.typing(group, true).await.unwrap();
    assert!(
        seen(&mut events_a, 5, |e| matches!(
            e,
            NodeEvent::Typing { typing: true, .. }
        ))
        .await
    );
    node_b.set_hidden(true).await.unwrap();
    assert!(
        seen(&mut events_a, 5, |e| matches!(
            e,
            NodeEvent::Typing { typing: false, .. }
        ))
        .await,
        "no stop when appearing offline"
    );

    // Appearing offline, Bea sends nothing, past the two seconds between two "typing" too.
    tokio::time::sleep(Duration::from_secs(2)).await;
    node_b.typing(group, true).await.unwrap();
    assert!(typing_within(&mut events_a, 3).await.is_empty());
}

/// A member's typing stream that says nothing holds a task of Ana's, reading it: that task
/// is Ana's, and is over once her node's shutdown returns, rather than when its read gives
/// up by itself, seconds later. Cycle after cycle (`FOGATA_CYCLES`, 3 when not set), only
/// what ran before Ana started is left.
#[tokio::test(flavor = "multi_thread")]
async fn a_stream_being_read_ends_with_the_node() {
    let _one = ONE_AT_A_TIME.lock().await;
    let tasks = || {
        tokio::runtime::Handle::current()
            .metrics()
            .num_alive_tasks()
    };
    let cycles: usize = std::env::var("FOGATA_CYCLES")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(3);
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut ours = GroupLog::create(&ana, "ours", 1).unwrap();
    ours.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = ours.group_id();
    // Bea runs in a runtime of her own, so that only Ana's tasks (and the test's) are
    // counted here.
    let (bea_runtime, (node_b, _events_b)) = {
        let (bea, ours) = (bea.clone(), ours.clone());
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let node = runtime.block_on(async { node(bea, vec![ours], 0, vec![]) });
            (runtime, node)
        })
        .join()
        .unwrap()
    };
    // What starting Bea left here settles before the count.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let before = tasks();
    let mut left = Vec::new();
    for _ in 0..cycles {
        let port_a = free_port();
        let (node_a, mut events_a) = node(ana.clone(), vec![ours.clone()], port_a, vec![]);
        node_b
            .dial(vec![loopback(port_a, None)], None)
            .await
            .unwrap();
        wait_to_connect(
            &node_b,
            loopback(port_a, None),
            &mut events_a,
            group,
            bea.public_key(),
        )
        .await;
        // Bea opens streams and says nothing on them; each holds a task of Ana's reading it.
        let mut silent = Vec::new();
        for _ in 0..3 {
            silent.push(
                node_b
                    .streams()
                    .open_stream(node_a.local_peer(), TYPING_PROTOCOL)
                    .await
                    .unwrap(),
            );
        }
        // Time for Ana to start reading them.
        tokio::time::sleep(Duration::from_millis(300)).await;
        node_a.shutdown().await.unwrap();
        drop((node_a, events_a, silent));
        // What else of the cycle ends (the test's own tasks) is given up to `settle`.
        let settle: u64 = std::env::var("FOGATA_SETTLE_MS")
            .ok()
            .and_then(|c| c.parse().ok())
            .unwrap_or(500);
        let until = tokio::time::Instant::now() + Duration::from_millis(settle);
        while tasks() > before && tokio::time::Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        left.push(tasks() as isize - before as isize);
    }
    println!("tasks left after each of {cycles} cycles: {left:?}");
    assert!(
        left.iter().all(|n| *n <= 0),
        "Ana's tasks outlived her node: {left:?}"
    );
    node_b.shutdown().await.unwrap();
    std::thread::spawn(move || drop(bea_runtime))
        .join()
        .unwrap();
}
