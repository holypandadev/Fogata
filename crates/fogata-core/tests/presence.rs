//! What friends see of each other while groups change under them: two devices that are
//! friends and share a group, in one process. Each dials the other's loopback address, and
//! nothing else of the network is used.

use std::time::Duration;

use fogata_core::CoreEvents;
use fogata_core::{Core, CoreEvent, NetOptions, Presence, Store};
use fogata_friends::Friend;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
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
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok();
    assert!(held, "timed out waiting for {what}");
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

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Listening at `port` on this computer, and dialling the other device at `peer`.
fn options(port: u16, peer: u16) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers: vec![
            format!("/ip4/127.0.0.1/udp/{peer}/quic-v1")
                .parse()
                .unwrap(),
        ],
        ..Default::default()
    }
}

/// How Ana sees her friend Bea.
async fn presence(ana: &Core) -> Presence {
    ana.friends().await.unwrap()[0].presence.clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_removed_from_a_group_while_in_its_room_is_no_longer_shown_in_it() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-presence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, port_b)).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, port_a)).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    let group = ana.create_group("crew", 0, &[bea.me()]).await.unwrap();
    wait_for(
        &mut events_b,
        "the group from Ana",
        |e| matches!(e, CoreEvent::GroupInvited { group: g, .. } if *g == group),
    )
    .await;
    bea.accept_group_invitation(group).await.unwrap();
    eventually("both members connected", 60, || async {
        ana.groups().await.unwrap()[0].online == 1
    })
    .await;
    bea.join_room(group).await.unwrap();
    eventually("Bea shown in the room", 60, || async {
        matches!(presence(&ana).await, Presence::InRoom { group: Some((g, _)) } if g == group)
    })
    .await;

    ana.remove_from_group(group, bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "Bea's removal",
        |e| matches!(e, CoreEvent::Removed { group: g, .. } if *g == group),
    )
    .await;
    eventually("Bea shown online, in no room", 60, || async {
        presence(&ana).await == Presence::Online
    })
    .await;
    let _ = std::fs::remove_dir_all(&base);
}

/// Starts Ana's data folder again, on the same port, once the core before let go of it: its
/// lock goes with the last of its tasks.
async fn restart(dir: &std::path::Path, port: u16) -> (Core, CoreEvents) {
    let options = NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    };
    for _ in 0..50 {
        if let Ok(started) = Core::start(dir.to_path_buf(), options.clone()) {
            return started;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the folder stayed locked");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_shows_as_last_known_after_a_restart_while_it_is_away() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-members-known-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (ana, mut events_a) = restart(&dir_a, port_a).await;
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: fogata_net::Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(
        dir_b,
        NetOptions {
            use_dht: false,
            use_mdns: false,
            peers: vec![to_ana],
            ..Default::default()
        },
    )
    .unwrap();
    bea.join_group(&invite, None, None).await.unwrap();
    let who = bea.me();
    wait_for(
        &mut events_a,
        "the ask to join",
        |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
    )
    .await;
    ana.answer_join(group, who, true).await.unwrap();
    wait_for(
        &mut events_b,
        "the join",
        |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
    )
    .await;
    // Bea, who is not Ana's friend, takes another name and picture.
    bea.set_profile(
        Some("Beatriz".into()),
        Some(fogata_friends::Avatar::Preset(2)),
    )
    .await
    .unwrap();
    let beas = || async {
        ana.members(group)
            .await
            .unwrap()
            .into_iter()
            .find(|m| !m.me)
            .map(|m| (m.person.name, m.person.avatar))
    };
    let new = Some(("Beatriz".to_string(), fogata_friends::Avatar::Preset(2)));
    eventually("Ana to see Bea's new name", 30, || async {
        beas().await == new
    })
    .await;
    // Bea goes away; Ana's Fogata starts again.
    bea.shutdown().await.unwrap();
    drop(bea);
    ana.shutdown().await.unwrap();
    drop(ana);
    drop(events_a);
    let (ana, _events_a) = restart(&dir_a, port_a).await;
    let members = ana.members(group).await.unwrap();
    let shown = members
        .into_iter()
        .find(|m| !m.me)
        .map(|m| (m.person.name, m.person.avatar));
    assert_eq!(shown, new, "Bea shows as she was when she joined");
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_and_member_away_from_the_computer_shows_as_away_until_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-away-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, port_b)).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, port_a)).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let group = ana.create_group("crew", 0, &[bea.me()]).await.unwrap();
    wait_for(
        &mut events_b,
        "the group from Ana",
        |e| matches!(e, CoreEvent::GroupInvited { group: g, .. } if *g == group),
    )
    .await;
    bea.accept_group_invitation(group).await.unwrap();
    eventually("both members connected", 60, || async {
        ana.groups().await.unwrap()[0].online == 1
    })
    .await;
    let who = bea.me();
    let member_away = || async {
        ana.members(group)
            .await
            .unwrap()
            .iter()
            .any(|m| m.person.device == who && m.away && m.online.is_some())
    };
    let chat_away = || async { ana.direct_chat(who, 10).await.unwrap().away };

    assert!(bea.set_away(true).await.unwrap());
    wait_for(&mut events_b, "Bea told she is away", |e| {
        matches!(e, CoreEvent::Away { away: true })
    })
    .await;
    eventually("Bea away for Ana", 30, || async {
        presence(&ana).await == Presence::Away && member_away().await && chat_away().await
    })
    .await;
    // Away people still count as online.
    assert_eq!(ana.groups().await.unwrap()[0].online, 1);

    // Not in a room: someone there may be listening.
    bea.join_room(group).await.unwrap();
    eventually("Bea in the room, not away", 30, || async {
        matches!(presence(&ana).await, Presence::InRoom { .. }) && !member_away().await
    })
    .await;
    assert!(!bea.set_away(true).await.unwrap());
    bea.leave_room(group).await.unwrap();
    eventually("Bea away again", 30, || async {
        presence(&ana).await == Presence::Away && member_away().await
    })
    .await;

    // Appearing offline wins.
    bea.set_hidden(true).await.unwrap();
    eventually("Bea offline", 30, || async {
        matches!(presence(&ana).await, Presence::Offline { .. }) && !member_away().await
    })
    .await;
    bea.set_hidden(false).await.unwrap();
    eventually("Bea away once visible", 30, || async {
        presence(&ana).await == Presence::Away
    })
    .await;

    // Back at the computer.
    assert!(!bea.set_away(false).await.unwrap());
    eventually("Bea back", 30, || async {
        presence(&ana).await == Presence::Online && !member_away().await && !chat_away().await
    })
    .await;
    let _ = std::fs::remove_dir_all(&base);
}
