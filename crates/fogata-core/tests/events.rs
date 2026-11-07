//! What the core keeps for screens that stop reading its events for a while: bounded in
//! number and in bytes whatever happens meanwhile, and what the screens know once they read
//! again is what the core knows. Devices in one process, each dialling the other's loopback
//! address, and nothing else of the network.

use std::time::Duration;

use fogata_core::{Core, CoreEvent, CoreEvents, NetOptions, Store, direct::model::Body};
use fogata_friends::Friend;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn wait_for(events: &mut CoreEvents, what: &str, mut done: impl FnMut(&CoreEvent) -> bool) {
    let found = timeout(Duration::from_secs(60), async {
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

/// Polls `check` until it holds, for up to `seconds`.
async fn eventually<F, Fut>(what: &str, seconds: u64, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let held = timeout(Duration::from_secs(seconds), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .is_ok();
    assert!(held, "timed out waiting for {what}");
}

/// Every event waiting, received now.
fn all(events: &mut CoreEvents) -> Vec<CoreEvent> {
    let mut got = Vec::new();
    while let Ok(event) = events.try_recv() {
        got.push(event);
    }
    got
}

/// What an event holds that can be large.
fn held(event: &CoreEvent) -> usize {
    match event {
        CoreEvent::Signal { payload, .. } => payload.len(),
        CoreEvent::Room {
            change: fogata_net::RoomChange::Chat(line),
            ..
        } => line.text.len(),
        CoreEvent::DirectMessage { text, .. } | CoreEvent::Notice(text) => text.len(),
        _ => 0,
    }
}

/// This process's resident memory, in bytes (Linux; 0 elsewhere).
fn resident() -> usize {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<usize>().ok())
        .map_or(0, |pages| pages * 4096)
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Listening at `port` on this computer, and dialling the other device at `peer`.
fn options(port: u16, peer: Option<u16>) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers: peer
            .map(|peer| {
                format!("/ip4/127.0.0.1/udp/{peer}/quic-v1")
                    .parse()
                    .unwrap()
            })
            .into_iter()
            .collect(),
        ..Default::default()
    }
}

/// Two data folders whose profiles are already friends.
fn friends(base: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let (store_a, store_b) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_b.clone()).unwrap(),
    );
    let mut book_a = store_a.friends().unwrap().unwrap();
    let mut book_b = store_b.friends().unwrap().unwrap();
    let friend = |book: &fogata_friends::FriendBook| Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    book_a
        .friends
        .insert(book_b.profile.device, friend(&book_b));
    book_b
        .friends
        .insert(book_a.profile.device, friend(&book_a));
    store_a.save_friends(&book_a).unwrap();
    store_b.save_friends(&book_b).unwrap();
    (dir_a, dir_b)
}

/// Twenty thousand changes made here while the screens read nothing (this device away and
/// back, a group renamed) leave a handful of events waiting, the last of each saying how
/// things are now.
#[tokio::test(flavor = "multi_thread")]
async fn local_changes_while_the_screens_read_nothing_stay_bounded() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-events-local-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("ana");
    Core::create_profile(&dir, "Ana", None).unwrap();
    let (ana, mut events) = Core::start(dir, options(free_port(), None)).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let before = resident();
    let mut away = false;
    for n in 0..20_000 {
        away = ana.set_away(n % 2 == 0).await.unwrap();
        if n % 10 == 0 {
            ana.rename_group(group, &format!("crew {n}")).await.unwrap();
        }
    }
    let after = resident();
    let got = all(&mut events);
    let bytes: usize = got.iter().map(held).sum();
    eprintln!(
        "{} events waiting ({bytes} bytes carried); resident {} MiB before, {} MiB after",
        got.len(),
        before >> 20,
        after >> 20
    );
    assert!(got.len() <= 16, "{} events waiting", got.len());
    let last_away = got.iter().rev().find_map(|e| match e {
        CoreEvent::Away { away } => Some(*away),
        _ => None,
    });
    assert_eq!(last_away, Some(away));
    assert!(got.iter().any(|e| matches!(e, CoreEvent::GroupsChanged)));
    ana.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

/// Bea's screens stop reading while she is in a call with Ana, who sends call signals,
/// types, writes in the call's chat and to Bea directly, changes her name, and at last
/// takes Bea out of a group they share. What waits for Bea's screens stays bounded in
/// number and bytes, signals included; once they read again they know what the core
/// knows: the group gone, every message (those Bea's node held back meanwhile come later),
/// and the call agrees anew with Ana, whose lost signals they are told of, and then ends
/// as calls do.
#[tokio::test(flavor = "multi_thread")]
async fn a_stuck_screen_holds_a_bounded_backlog_and_catches_up() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-events-stuck-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Some(port_b))).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, Some(port_a))).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    // A group to be taken out of, and a call.
    let group = ana.create_group("crew", 0, &[bea.me()]).await.unwrap();
    wait_for(
        &mut events_b,
        "the group from Ana",
        |e| matches!(e, CoreEvent::GroupInvited { group: g, .. } if *g == group),
    )
    .await;
    bea.accept_group_invitation(group).await.unwrap();
    eventually("both members connected", 60, || async {
        ana.groups()
            .await
            .unwrap()
            .iter()
            .any(|g| g.id == group && g.online == 1)
    })
    .await;
    let call = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "Ana's ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    bea.accept_call(call).await.unwrap();
    let from_ana = |e: &CoreEvent, payload: &[u8]| {
        matches!(e, CoreEvent::Signal { group, from, payload: p }
            if *group == call && *from == ana.me() && p == payload)
    };
    // Signals go through once both are in the call's room.
    let reached = timeout(Duration::from_secs(60), async {
        loop {
            ana.signal(call, bea.me(), b"hello".to_vec()).await.unwrap();
            let got = timeout(Duration::from_secs(1), async {
                while let Some(e) = events_b.recv().await {
                    if from_ana(&e, b"hello") {
                        return;
                    }
                }
            })
            .await;
            if got.is_ok() {
                return;
            }
        }
    })
    .await;
    assert!(reached.is_ok(), "no signal reached Bea");
    while events_b.try_recv().is_ok() {}

    // Bea's screens stop reading.
    let before = resident();
    let signal = vec![b's'; 32 * 1024];
    let mut said = Vec::new();
    for n in 0..1300 {
        if n % 16 == 0 {
            // Slower than the eight a second Bea's node takes from a member, so all of them
            // reach her core, whatever this computer's speed, and more than it keeps of one
            // sender (64) for screens that read nothing.
            ana.signal(call, bea.me(), signal.clone()).await.unwrap();
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        if n % 13 == 0 {
            ana.typing(call, n % 2 == 0).await.unwrap();
            ana.send_direct(bea.me(), &format!("direct {n}"), Vec::new())
                .await
                .unwrap();
        }
        if n % 100 == 0 {
            ana.set_profile(Some(format!("Ana {n}")), None)
                .await
                .unwrap();
        }
        let text = format!("line {n}");
        ana.say(call, &text).await.unwrap();
        said.push(text);
    }
    ana.remove_from_group(group, bea.me()).await.unwrap();
    // What is on its way arrives, the removal last.
    eventually("Bea out of the group", 60, || async {
        !bea.groups().await.unwrap().iter().any(|g| g.id == group)
    })
    .await;
    let after = resident();
    let got = all(&mut events_b);
    let signals = got
        .iter()
        .filter(|e| matches!(e, CoreEvent::Signal { .. }))
        .count();
    let bytes: usize = got.iter().map(held).sum();
    eprintln!(
        "{} events waiting for Bea, {signals} of them signals, {bytes} bytes carried; \
         resident {} MiB before, {} MiB after",
        got.len(),
        before >> 20,
        after >> 20
    );
    // Within the bounds: no more signals of one sender than it keeps.
    assert!(signals <= 64, "{signals} signals waiting");
    assert!(bytes <= 8 * 1024 * 1024, "{bytes} bytes waiting");
    // The call is told to agree anew, after the last signal kept, with none after it.
    let lost = got
        .iter()
        .rposition(|e| matches!(e, CoreEvent::SignalsLost { group, from } if *group == call && *from == ana.me()))
        .expect("Bea's screens hear signals were lost");
    assert!(
        !got[lost..]
            .iter()
            .any(|e| matches!(e, CoreEvent::Signal { .. }))
    );
    assert!(
        got.iter()
            .any(|e| matches!(e, CoreEvent::Removed { group: g, .. } if *g == group))
    );

    // Reading again: the screens know what the core knows.
    assert!(!bea.groups().await.unwrap().iter().any(|g| g.id == group));
    let last = said.last().unwrap().clone();
    eventually("every line of the call's chat", 120, || {
        while events_b.try_recv().is_ok() {}
        async {
            let room = bea.room(call, fogata_core::FIRST_ROOM).await.unwrap();
            room.is_some_and(|r| r.chat.last().is_some_and(|line| line.text == last))
        }
    })
    .await;
    eventually("every direct message", 120, || {
        while events_b.try_recv().is_ok() {}
        async {
        bea.direct_chat(ana.me(), 1000).await.is_ok_and(|chat| {
            chat.items
                .iter()
                .filter(|i| matches!(&i.item.body, Body::Message { text, .. } if text.starts_with("direct")))
                .count()
                == 100
        })
        }
    })
    .await;
    // Signals come again, as the call agrees anew.
    let fresh = timeout(Duration::from_secs(30), async {
        loop {
            ana.signal(call, bea.me(), b"again".to_vec()).await.unwrap();
            let got = timeout(Duration::from_secs(1), async {
                while let Some(e) = events_b.recv().await {
                    if from_ana(&e, b"again") {
                        return;
                    }
                }
            })
            .await;
            if got.is_ok() {
                return;
            }
        }
    })
    .await;
    assert!(
        fresh.is_ok(),
        "no signal of Ana reached Bea after the loss was heard"
    );
    ana.end_call(call).await.unwrap();
    // As Bea's core has it (the events that tell it may have gone with those read above):
    // Ana is gone from the call, which waits a while for her alone, or is over.
    eventually("Ana gone from the call", 60, || async {
        bea.calls()
            .await
            .unwrap()
            .iter()
            .all(|c| c.group != call || c.alone_since_ms.is_some())
    })
    .await;
    ana.shutdown().await.unwrap();
    bea.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

/// The lines of a room's chat that Bea's node refused while her screens were stuck, for
/// longer than what a member names of what it said lately (two minutes), still come once
/// they read again: her node asks for the room whole. Slow (about five minutes): run with
/// `--ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn lines_refused_during_a_long_stall_come_once_the_screens_read_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-events-stall-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Some(port_b))).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, Some(port_a))).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let call = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "the ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    bea.accept_call(call).await.unwrap();
    let both = |core: &Core| {
        let core = core.clone();
        async move {
            core.room(call, fogata_core::FIRST_ROOM)
                .await
                .unwrap()
                .is_some_and(|r| r.participants.len() == 2)
        }
    };
    eventually("both in the call's room", 60, || async {
        both(&ana).await && both(&bea).await
    })
    .await;
    while events_b.try_recv().is_ok() {}
    // Ana's screens read; Bea's are stuck while Ana says a line every 40 ms.
    let reader = tokio::spawn(async move { while events_a.recv().await.is_some() {} });
    for n in 0..1400 {
        ana.say(call, &format!("line {n}")).await.unwrap();
        // The pace of someone talking.
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    // Bea's screens stay stuck longer than the two minutes a member names what it said in.
    tokio::time::sleep(Duration::from_secs(130)).await;
    let have = |room: &fogata_core::RoomState| {
        room.chat
            .iter()
            .filter_map(|line| line.text.strip_prefix("line ")?.parse::<u32>().ok())
            .collect::<std::collections::BTreeSet<u32>>()
    };
    // Bea reads again: every line comes.
    let mut missing = Vec::new();
    for _ in 0..90 {
        while events_b.try_recv().is_ok() {}
        let room = bea
            .room(call, fogata_core::FIRST_ROOM)
            .await
            .unwrap()
            .unwrap();
        let held = have(&room);
        missing = (1300..1400).filter(|n| !held.contains(n)).collect();
        if missing.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    reader.abort();
    ana.shutdown().await.unwrap();
    bea.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
    assert!(
        missing.is_empty(),
        "{} of the last 100 lines never came",
        missing.len()
    );
}
