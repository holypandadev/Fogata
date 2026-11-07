// Two devices that are already friends, on the same machine: a group one of them creates
// with the other reaches the other as an invitation right away, not at the next heartbeat,
// and so do a changed profile, a banner, a bio and appearing offline. Friends in common are found
// without either showing its list, and a removed friend forgets the device too, at once or
// when it comes back.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::DeviceIdentity;
use fogata_friends::{Friend, FriendBook};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::{Body, Entry, GroupLog};
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The invitation must arrive well before the 20 s friend heartbeat would carry it.
const PROMPT: Duration = Duration::from_secs(5);

/// Entries of a group written to cost the most to judge, in a test: seconds in a
/// debug build.
const COSTLY_ENTRIES: usize = 900;

/// A node listening on `listen_port` (0 for any) that dials `peers`; nothing else of the
/// network.
fn node(
    identity: Arc<DeviceIdentity>,
    book: FriendBook,
    listen_port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    node_as(identity, book, false, listen_port, peers)
}

fn node_as(
    identity: Arc<DeviceIdentity>,
    book: FriendBook,
    hidden: bool,
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
        hidden,
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

/// Two books that already hold each other as friends, as after a link was accepted and
/// both secrets were delivered.
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

/// Adds `who` to `book` as a friend, with a made-up secret: enough for lists of friends.
fn befriend(book: &mut FriendBook, who: &DeviceIdentity, name: &str) {
    let other = FriendBook::new(who, name, 1).unwrap();
    book.friends.insert(
        who.public_key(),
        Friend {
            profile: other.profile,
            secret: other.secret,
            since_ms: 2,
            delivered: true,
            banner: None,
            bio: None,
        },
    );
}

/// Polls `node`'s friend book until `done` holds.
async fn until_book(
    node: &NodeHandle,
    limit: Duration,
    what: &str,
    done: impl Fn(&FriendBook) -> bool,
) {
    let found = timeout(limit, async {
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

/// Takes every event until none came for a second, for up to 30 s: what is under way when
/// it is called has come.
async fn quiet(events: &mut NodeEvents) {
    let settled = timeout(Duration::from_secs(30), async {
        while let Ok(Some(_)) = timeout(Duration::from_secs(1), events.recv()).await {}
    })
    .await;
    assert!(settled.is_ok(), "events kept coming");
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
async fn a_friend_gets_the_group_invitation_right_away() {
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

    // Ana creates a group with Bea in it: Bea is invited at once.
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let group = log.group_id();
    node_a.add_group(GroupSetup::Member(log)).await.unwrap();
    wait_for(&mut events_b, PROMPT, "the invitation", |e| {
        matches!(e, NodeEvent::GroupOffered { log, by } if log.group_id() == group && *by == ana.public_key())
    })
    .await;

    // Bea is still only invited: a later change (Ana renames the group) reaches her as the
    // whole log again, also at once.
    let mut renamed = node_a.group(group).await.unwrap().unwrap();
    renamed
        .append(&ana, fogata_state::Body::Rename { name: "g2".into() }, 3)
        .unwrap();
    node_a.merge_group(renamed).await.unwrap();
    wait_for(&mut events_b, PROMPT, "the renamed invitation", |e| {
        matches!(e, NodeEvent::GroupOffered { log, .. } if log.group_id() == group && log.state().name == "g2")
    })
    .await;
    drop(node_b);
}

/// A group whose log is longer than one part (years of history, or pictures set with entries
/// by earlier versions) reaches a friend added to it whole, in parts.
#[tokio::test(flavor = "multi_thread")]
async fn a_long_group_reaches_a_friend_in_parts() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    for i in 0..12u64 {
        let picture = Some(vec![i as u8; fogata_state::MAX_PICTURE_BYTES]);
        log.append(&ana, fogata_state::Body::SetPicture { picture }, 10 + i)
            .unwrap();
    }
    let bytes: usize = log.entries().map(|e| e.encode().len()).sum();
    assert!(bytes > 2 * 256 * 1024, "{bytes} bytes");
    let (group, entries) = (log.group_id(), log.len());
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;
    wait_for(&mut events_b, Duration::from_secs(60), "Ana online", online).await;
    node_a.add_group(GroupSetup::Member(log)).await.unwrap();
    wait_for(
        &mut events_b,
        Duration::from_secs(30),
        "the whole invitation",
        |e| {
            matches!(e, NodeEvent::GroupOffered { log, by } if log.group_id() == group
            && log.len() == entries && *by == ana.public_key())
        },
    )
    .await;
    drop(node_b);
}

/// Longest a node may take to answer a command while a friend's offer is judged.
const ANSWERS_WITHIN: Duration = Duration::from_millis(250);

/// A group whose log costs seconds to judge reaches a friend whole, and the friend's node
/// answers commands meanwhile: the log is judged away from the node's loop. Two members
/// write apart, each entry signed only on its author's last one, so every entry is judged
/// on the group worked out for its own parents (an honest group whose members wrote at the
/// same time for long costs the same, and one sent to hold the node up as much).
#[tokio::test(flavor = "multi_thread")]
async fn a_group_costly_to_judge_reaches_a_friend_that_goes_on_answering() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let leo = DeviceIdentity::generate().unwrap();
    let (book_a, book_b) = friends(&ana, &bea);
    let mut first = GroupLog::create(&ana, "g", 1).unwrap();
    first.add(&ana, bea.public_key(), "bea", 2).unwrap();
    first.add(&ana, leo.public_key(), "leo", 3).unwrap();
    let group = first.group_id();
    let mut entries: Vec<Entry> = first.entries_in_order().into_iter().cloned().collect();
    let mut last = [first.heads(), first.heads()];
    for i in 0..COSTLY_ENTRIES {
        let by: &DeviceIdentity = [&ana, &leo][i % 2];
        let body = Body::Rename {
            name: format!("n{i}"),
        };
        let entry = Entry::sign(by, group, last[i % 2].clone(), 10 + i as u64, body);
        last[i % 2] = vec![entry.hash()];
        entries.push(entry);
    }
    let mut log = GroupLog::empty(group);
    let started = Instant::now();
    log.insert_all(entries.clone());
    let judging = started.elapsed();
    assert_eq!(log.len(), entries.len());
    // Long enough that a node judging it on its loop would not answer in time.
    assert!(judging > 4 * ANSWERS_WITHIN, "judged in {judging:?}");
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;
    wait_for(&mut events_b, Duration::from_secs(60), "Ana online", online).await;
    // Bea's node is asked something every few milliseconds until the group is offered.
    let (stop, mut stopped) = tokio::sync::oneshot::channel::<()>();
    let asking = node_b.clone();
    let slowest = tokio::spawn(async move {
        let mut slowest = Duration::ZERO;
        loop {
            let started = Instant::now();
            asking.groups().await.unwrap();
            slowest = slowest.max(started.elapsed());
            tokio::select! {
                _ = &mut stopped => return slowest,
                () = tokio::time::sleep(Duration::from_millis(5)) => {}
            }
        }
    });
    node_a.add_group(GroupSetup::Member(log)).await.unwrap();
    wait_for(
        &mut events_b,
        Duration::from_secs(60) + 20 * judging,
        "the whole invitation",
        |e| {
            matches!(e, NodeEvent::GroupOffered { log, by } if log.group_id() == group
            && log.len() == entries.len() && *by == ana.public_key())
        },
    )
    .await;
    let _ = stop.send(());
    let slowest = slowest.await.unwrap();
    assert!(
        slowest < ANSWERS_WITHIN,
        "Bea's node took {slowest:?} to answer while judging a log of {judging:?}"
    );
    drop(node_b);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_profile_reaches_an_online_friend_right_away() {
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

    // The greetings of the new connection carry profiles too: let them finish first.
    quiet(&mut events_a).await;
    quiet(&mut events_b).await;
    node_a
        .set_profile(Some("Ana Two".into()), None)
        .await
        .unwrap();
    let ana_key = ana.public_key();
    let found = timeout(PROMPT, async {
        loop {
            let book = node_b.friends().await.unwrap().unwrap();
            if book.friends[&ana_key].profile.name == "Ana Two" {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "Bea did not get Ana's new name");
    drop(node_a);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_that_appears_offline_says_so() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let ana_key = ana.public_key();
    // Ana starts hidden: Bea learns it with the greeting itself.
    let port_a = free_port();
    let (node_a, mut events_a) = node_as(ana.clone(), book_a, true, port_a, Vec::new());
    let (_node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    wait_for(
        &mut events_b,
        Duration::from_secs(60),
        "Ana, hidden",
        |e| matches!(e, NodeEvent::FriendOnline { device, hidden: true, .. } if *device == ana_key),
    )
    .await;
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;

    node_a.set_hidden(false).await.unwrap();
    wait_for(&mut events_b, PROMPT, "Ana visible", |e| {
        matches!(e, NodeEvent::FriendStatus { device, hidden: false, .. } if *device == ana_key)
    })
    .await;
    node_a.set_hidden(true).await.unwrap();
    wait_for(&mut events_b, PROMPT, "Ana hidden again", |e| {
        matches!(
            e,
            NodeEvent::FriendStatus { device, hidden: true, in_room: false, group: None, away: false }
                if *device == ana_key
        )
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_away_from_the_computer_says_so() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let ana_key = ana.public_key();
    let status = move |hidden: bool, away: bool| {
        move |e: &NodeEvent| {
            matches!(e, NodeEvent::FriendStatus { device, hidden: h, away: a, .. }
                if *device == ana_key && *h == hidden && *a == away)
        }
    };
    // Away before Bea connects: the status after the greeting says so.
    let port_a = free_port();
    let (node_a, _events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    assert!(node_a.set_away(true).await.unwrap());
    let (_node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    wait_for(
        &mut events_b,
        Duration::from_secs(60),
        "Ana online",
        |e| matches!(e, NodeEvent::FriendOnline { device, hidden: false, .. } if *device == ana_key),
    )
    .await;
    wait_for(&mut events_b, PROMPT, "Ana away", status(false, true)).await;
    assert!(!node_a.set_away(false).await.unwrap());
    wait_for(&mut events_b, PROMPT, "Ana back", status(false, false)).await;
    assert!(node_a.set_away(true).await.unwrap());
    wait_for(&mut events_b, PROMPT, "Ana away again", status(false, true)).await;
    // Appearing offline wins, and says nothing more.
    node_a.set_hidden(true).await.unwrap();
    wait_for(&mut events_b, PROMPT, "Ana hidden", status(true, false)).await;
    assert!(!node_a.set_away(true).await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_banner_a_bio_and_friends_in_common_reach_only_friends() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let cid = DeviceIdentity::generate().unwrap();
    let dan = DeviceIdentity::generate().unwrap();
    let eva = DeviceIdentity::generate().unwrap();
    let (mut book_a, mut book_b) = friends(&ana, &bea);
    // Cid is a friend of both; Dan only of Ana, Eva only of Bea.
    befriend(&mut book_a, &cid, "Cid");
    befriend(&mut book_b, &cid, "Cid");
    befriend(&mut book_a, &dan, "Dan");
    befriend(&mut book_b, &eva, "Eva");
    // A banner and a bio set before connecting go with the greeting.
    book_a.set_banner(&ana, 0xFF3D21AD, None, 5).unwrap();
    book_a.set_bio(&ana, "Hey there!", 5).unwrap();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    let (node_b, mut events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    let online = |e: &NodeEvent| matches!(e, NodeEvent::FriendOnline { .. });
    wait_for(&mut events_a, Duration::from_secs(60), "Bea online", online).await;
    wait_for(&mut events_b, Duration::from_secs(60), "Ana online", online).await;

    let ana_key = ana.public_key();
    let bea_key = bea.public_key();
    until_book(&node_b, PROMPT, "Ana's banner", |b| {
        b.friends[&ana_key]
            .banner
            .as_ref()
            .is_some_and(|b| b.color == 0xFF3D21AD)
    })
    .await;
    node_a.set_banner(0xFF0E7C66, None).await.unwrap();
    until_book(&node_b, PROMPT, "Ana's new banner", |b| {
        b.friends[&ana_key]
            .banner
            .as_ref()
            .is_some_and(|b| b.color == 0xFF0E7C66)
    })
    .await;
    until_book(&node_b, PROMPT, "Ana's bio", |b| {
        b.friends[&ana_key]
            .bio
            .as_ref()
            .is_some_and(|b| b.text == "Hey there!")
    })
    .await;
    // A changed bio shows at once, as a changed name does.
    node_a
        .set_bio("Two lines\nnow  ".to_string())
        .await
        .unwrap();
    until_book(&node_b, PROMPT, "Ana's new bio", |b| {
        b.friends[&ana_key]
            .bio
            .as_ref()
            .is_some_and(|b| b.text == "Two lines\nnow")
    })
    .await;
    // One that would hide what follows it is refused before it goes.
    assert!(node_a.set_bio("Ana\u{202E}gnp".to_string()).await.is_err());
    assert!(node_a.set_bio("1\n2\n3\n4\n5".to_string()).await.is_err());

    assert_eq!(
        node_b.mutual_friends(ana_key).await.unwrap(),
        vec![cid.public_key()]
    );
    // Asked again, the friend waits ten minutes before answering again: each question can
    // test a thousand device keys.
    assert!(node_b.mutual_friends(ana_key).await.is_err());
    // Nor a few seconds later.
    tokio::time::sleep(Duration::from_secs(6)).await;
    assert!(node_b.mutual_friends(ana_key).await.is_err());
    // Bea has not asked Ana anything yet.
    assert_eq!(
        node_a.mutual_friends(bea_key).await.unwrap(),
        vec![cid.public_key()]
    );
    // Not a friend connected now.
    assert!(node_a.mutual_friends(dan.public_key()).await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_friend_forgets_us_right_away() {
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

    let bea_key = bea.public_key();
    let ana_key = ana.public_key();
    node_a.remove_friend(bea_key).await.unwrap();
    until_book(&node_b, PROMPT, "Bea to forget Ana", |b| {
        !b.friends.contains_key(&ana_key)
    })
    .await;
    until_book(&node_a, PROMPT, "Ana to stop telling Bea", |b| {
        b.departing.is_empty() && !b.friends.contains_key(&bea_key)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_removed_while_away_hears_it_when_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let bea_key = bea.public_key();
    let ana_key = ana.public_key();
    // Bea is away when Ana removes her.
    let (node_a, _events_a) = node(ana.clone(), book_a, 0, Vec::new());
    node_a.remove_friend(bea_key).await.unwrap();
    until_book(&node_a, PROMPT, "Bea waiting to hear it", |b| {
        b.departing.contains_key(&bea_key)
    })
    .await;

    let port_b = free_port();
    let (node_b, _events_b) = node(bea.clone(), book_b, port_b, Vec::new());
    // Bea is back, and Ana finds her (dials her address): Bea hears it then.
    node_a.dial(vec![loopback(port_b)], None).await.unwrap();
    until_book(&node_b, Duration::from_secs(60), "Bea to forget Ana", |b| {
        !b.friends.contains_key(&ana_key)
    })
    .await;
    until_book(&node_a, PROMPT, "Ana to stop telling Bea", |b| {
        b.departing.is_empty()
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removal_is_kept_for_the_former_friend_when_another_device_refuses_it() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let cid = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let bea_key = bea.public_key();
    // Cid answers probes with a tag of Bea's secret, as a friend of Bea's that knows it
    // could: Ana takes it for Bea, and it cannot open what is sealed to Bea.
    let mut book_c = FriendBook::new(&cid, "Cid", 1).unwrap();
    book_c.secret = book_b.secret.clone();
    let (node_a, _events_a) = node(ana.clone(), book_a, 0, Vec::new());
    node_a.remove_friend(bea_key).await.unwrap();
    until_book(&node_a, PROMPT, "Bea waiting to hear it", |b| {
        b.departing.contains_key(&bea_key)
    })
    .await;
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (node_c, _events_c) = fogata_net::start(NodeConfig {
        identity: cid.clone(),
        groups: Vec::new(),
        friends: Some(book_c),
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers: Vec::new(),
        remembered: Vec::new(),
        hidden: false,
    })
    .unwrap();
    // Dialled at an address given: probed at once, and told of the removal.
    let at_cid = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap();
    node_a.dial(vec![at_cid], None).await.unwrap();
    // Given time for a confirmation, none comes from Cid, who cannot open what is sealed to
    // Bea: Bea is still to be told.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let book = node_a.friends().await.unwrap().unwrap();
    assert!(book.departing.contains_key(&bea_key));
    let _ = node_c.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn friends_who_both_changed_secrets_while_apart_meet_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (mut book_a, mut book_b) = friends(&ana, &bea);
    // Each removed someone else while the other was away: each knows only the other's
    // earlier secret.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    book_a.rotate_secret(now).unwrap();
    book_b.rotate_secret(now).unwrap();
    let (secret_a, secret_b) = (book_a.secret.clone(), book_b.secret.clone());
    let (ana_key, bea_key) = (ana.public_key(), bea.public_key());

    let port_a = free_port();
    let (node_a, _events_a) = node(ana.clone(), book_a, port_a, Vec::new());
    let (node_b, _events_b) = node(bea.clone(), book_b, 0, vec![loopback(port_a)]);
    // Bea dials Ana: they meet at once.
    let has = |device, secret: &fogata_friends::FriendSecret| {
        let secret = *secret.as_bytes();
        move |b: &FriendBook| {
            b.friends
                .get(&device)
                .is_some_and(|f| *f.secret.as_bytes() == secret && f.delivered)
        }
    };
    until_book(
        &node_a,
        Duration::from_secs(60),
        "Ana to have Bea's new secret",
        has(bea_key, &secret_b),
    )
    .await;
    until_book(
        &node_b,
        PROMPT,
        "Bea to have Ana's new secret",
        has(ana_key, &secret_a),
    )
    .await;
    // Everyone has the new ones: the earlier ones are no longer kept.
    until_book(&node_a, PROMPT, "Ana to forget her old secret", |b| {
        b.previous.is_empty()
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_is_found_again_where_it_was_reached() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, book_b) = friends(&ana, &bea);
    let ana_key = ana.public_key();
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address: fogata_net::Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap();
    // Neither the local network nor the DHT: only the address Bea remembers.
    let start = |identity, book, listen_port, remembered| {
        fogata_net::start(NodeConfig {
            identity,
            groups: Vec::new(),
            friends: Some(book),
            listen_port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: true,
            peers: Vec::new(),
            remembered,
            hidden: false,
        })
        .unwrap()
    };
    let (_node_a, _events_a) = start(ana.clone(), book_a, port, Vec::new());
    let (_node_b, mut events_b) = start(bea.clone(), book_b, 0, vec![(address.clone(), ana_key)]);
    let (mut online, mut reached) = (false, false);
    wait_for(
        &mut events_b,
        PROMPT,
        "Ana online and her address noted",
        |e| {
            match e {
                NodeEvent::FriendOnline { device, .. } if *device == ana_key => online = true,
                NodeEvent::Reached { address: a, device } if *a == address => {
                    assert_eq!(*device, ana_key);
                    reached = true
                }
                _ => {}
            }
            online && reached
        },
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_address_of_a_former_friend_is_not_dialled() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let (book_a, mut book_b) = friends(&ana, &bea);
    // Bea removed Ana since she was reached there.
    book_b.friends.clear();
    let ana_key = ana.public_key();
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let address: fogata_net::Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap();
    let start = |identity, book, listen_port, remembered| {
        fogata_net::start(NodeConfig {
            identity,
            groups: Vec::new(),
            friends: Some(book),
            listen_port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: true,
            peers: Vec::new(),
            remembered,
            hidden: false,
        })
        .unwrap()
    };
    let (_node_a, _events_a) = start(ana.clone(), book_a, port, Vec::new());
    let (node_b, _events_b) = start(bea.clone(), book_b, 0, vec![(address, ana_key)]);
    // Given time to dial, Bea does not.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let report = node_b.network_report().await.unwrap();
    assert_eq!(
        report.quic_connections, 0,
        "Bea dialled the address of a former friend"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_is_told_when_it_uses_its_own_link_or_code() {
    let _one = ONE_AT_A_TIME.lock().await;
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let book = FriendBook::new(&ana, "Ana", 1).unwrap();
    let (node_a, mut events_a) = node(ana, book, 0, Vec::new());
    let own = |e: &NodeEvent| matches!(e, NodeEvent::FriendRequestFailed(why) if why.contains("made on this device"));
    let link = node_a.create_friend_link().await.unwrap();
    node_a.use_friend_link(link).await.unwrap();
    wait_for(&mut events_a, PROMPT, "the link refused", own).await;
    let code = node_a.create_friend_code().await.unwrap();
    node_a.use_friend_code(code).await.unwrap();
    wait_for(&mut events_a, PROMPT, "the code refused", own).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_that_changes_its_profile_again_and_again_is_taken_now_and_then() {
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
    // What the greetings changed comes first: none of it is counted.
    quiet(&mut events_b).await;

    // Each change would rewrite Bea's friend book: the first two are taken at once, the rest
    // wait for their turn, and only the newest of them is kept.
    for n in 1..=30 {
        node_a
            .set_profile(Some(format!("Ana {n}")), None)
            .await
            .unwrap();
    }
    let mut changes = 0;
    let _ = timeout(Duration::from_secs(3), async {
        while let Some(event) = events_b.recv().await {
            if matches!(event, NodeEvent::FriendsChanged) {
                changes += 1;
            }
        }
    })
    .await;
    assert!(changes <= 2, "{changes} changes of the friend book");
    let ana_key = ana.public_key();
    until_book(
        &node_b,
        Duration::from_secs(20),
        "Ana's last name",
        |book| book.friends[&ana_key].profile.name == "Ana 30",
    )
    .await;
    drop(node_a);
}
