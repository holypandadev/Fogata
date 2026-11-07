//! Copies of the command line built from earlier versions. A group banner and an animated
//! profile picture never reach one from before them: it keeps the profile with its still
//! picture and the whole log. A copy from a version whose animations and banners were
//! smaller refuses bigger ones the same way. An old copy that added someone takes the `Leave` they hand back when they
//! decline, marked as a decline, as any other. Entries for groups on MLS reach a copy that
//! keeps kinds it does not know (but knows nothing of MLS), which keeps them and whatever is
//! signed on them. A group picture chosen in a record apart from the log never reaches a copy
//! from before such records: it gets one entry with no picture instead, and shows the icon
//! rather than the picture an entry had set before. A voice message reaches a copy from before
//! them as a file of its name, which it keeps and confirms. Needs that binary: run with
//! `FOGATA_OLD_CLI=<path to fogata> cargo test -p fogata-core --test old_copy -- --ignored`.

use std::{
    io::Write,
    process::{Child, Command, Stdio},
    time::Duration,
};

use fogata_core::{Core, FIRST_ROOM, NetOptions, Store};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_state::GroupLog;
use tokio::time::timeout;

/// A GIF of one pixel, padded with a comment to `len` bytes of `fill`.
fn gif(len: usize, fill: u8) -> Vec<u8> {
    let mut gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00\x21\xfe"
        .to_vec();
    let mut left = len - gif.len() - 2;
    while left > 0 {
        let mut block = left.min(256);
        if left - block == 1 {
            block -= 1;
        }
        gif.push((block - 1) as u8);
        gif.resize(gif.len() + block - 1, fill);
        left -= block;
    }
    gif.extend([0, 0x3b]);
    gif
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
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

/// Whether `device` is in the first voice room of `group`, as `core` sees it.
async fn in_room(core: &Core, group: GroupId, device: DevicePublicKey) -> bool {
    core.room(group, FIRST_ROOM)
        .await
        .unwrap()
        .is_some_and(|r| r.participants.iter().any(|(p, _)| p.device == device))
}

/// The old binary's device key, made on first use.
fn old_identity(old: &str, dir: &std::path::Path) -> DevicePublicKey {
    let out = Command::new(old)
        .arg("id")
        .env("FOGATA_DATA_DIR", dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let hex = text
        .lines()
        .find_map(|l| l.strip_prefix("Device key: "))
        .expect("the old binary prints its key");
    DevicePublicKey::from_bytes(&hex::decode(hex.trim()).unwrap()).unwrap()
}

/// The old binary in the group's room, dialling Ana.
fn old_chat(old: &str, dir: &std::path::Path, port: u16) -> Child {
    Command::new(old)
        .args([
            "chat",
            "g",
            "--nick",
            "Leo",
            "--no-dht",
            "--no-mdns",
            "--peer",
        ])
        .arg(format!("/ip4/127.0.0.1/udp/{port}/quic-v1"))
        .env("FOGATA_DATA_DIR", dir)
        .env("RUST_LOG", "error")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// The old binary online for its friends, dialling Ana.
fn old_online(old: &str, dir: &std::path::Path, port: u16) -> Child {
    Command::new(old)
        .args(["friends", "online", "--no-dht", "--no-mdns", "--peer"])
        .arg(format!("/ip4/127.0.0.1/udp/{port}/quic-v1"))
        .env("FOGATA_DATA_DIR", dir)
        .env("RUST_LOG", "error")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version before banners and animations"]
async fn an_old_copy_keeps_the_group_and_the_still_picture() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-looks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let leo = old_identity(&old, &dir_l);

    let store_a = Store::open(dir_a.clone()).unwrap();
    let (ana_id, _) = store_a.identity().unwrap();
    let ana_key = ana_id.public_key();
    let mut log = GroupLog::create(&ana_id, "g", now()).unwrap();
    log.add(&ana_id, leo, "Leo", now()).unwrap();
    let group = log.group_id();
    store_a.save_group_log(&log, "g").unwrap();
    Store::open(dir_l.clone())
        .unwrap()
        .save_group_log(&log, "g")
        .unwrap();
    drop(store_a);

    let port = free_port();
    let (ana, _events) = Core::start(
        dir_a.clone(),
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    // A still picture with its animation, and a banner, before the old copy comes.
    let still = b"\x89PNG\r\n\x1a\nstill".to_vec();
    ana.set_profile(None, Some(fogata_friends::Avatar::Photo(still.clone())))
        .await
        .unwrap();
    let gif = gif(100 * 1024, 5);
    ana.set_animation(fogata_core::Animated::Photo, Some(gif.clone()))
        .await
        .unwrap();
    ana.set_group_banner(group, Some(gif.clone()))
        .await
        .unwrap();
    ana.join_room(group).await.unwrap();
    let mut chat = old_chat(&old, &dir_l, port);
    eventually("the old copy in the room", 60, || in_room(&ana, group, leo)).await;

    // Rosters go every heartbeat: give the banner time to be offered, then change the
    // banner and the name while connected.
    tokio::time::sleep(Duration::from_secs(25)).await;
    ana.set_group_banner(group, Some(b"\xff\xd8\xffnew".to_vec()))
        .await
        .unwrap();
    ana.rename_group(group, "h").await.unwrap();
    // Time for the rename to reach the old copy, which only its own folder shows once it
    // quits (reading that folder meanwhile would have this version seal it under the old
    // copy's feet).
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(
        in_room(&ana, group, leo).await,
        "the old copy left the room"
    );

    let _ = chat.stdin.as_mut().unwrap().write_all(b"/quit\n");
    let _ = timeout(Duration::from_secs(10), async {
        while chat.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let _ = chat.kill();
    let _ = chat.wait();
    ana.shutdown().await.unwrap();

    // Ana keeps her banner after a restart.
    let store_a = Store::open(dir_a).unwrap();
    let mine = store_a.load_log(&store_a.group("g").unwrap()).unwrap();
    assert_eq!(
        mine.banner().map(|b| b.picture.clone()),
        Some(b"\xff\xd8\xffnew".to_vec())
    );

    // The old copy: the rename applied, nothing waiting, no banner file; Ana's profile kept
    // with the still picture.
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert_eq!(kept.state().name, "h");
    assert!(kept.waiting().is_empty());
    assert!(kept.banner().is_none());
    let names: Vec<String> = base
        .join("leo/logs")
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names
            .iter()
            .all(|n| !n.ends_with(".banner") && !n.ends_with(".damaged"))
    );
    let profile = store_l
        .member_profiles()
        .into_iter()
        .find(|p| p.device == ana_key)
        .expect("the old copy kept Ana's profile");
    assert_eq!(profile.avatar, fogata_friends::Avatar::Photo(still));
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version before animations"]
async fn an_old_friend_keeps_the_still_picture_and_banner() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-friend-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let made = Command::new(&old)
        .args(["profile", "--name", "Leo"])
        .env("FOGATA_DATA_DIR", &dir_l)
        .output()
        .unwrap();
    assert!(made.status.success(), "the old binary made no profile");

    // Friends already, as after a link was accepted and both secrets were delivered.
    let (store_a, store_l) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_l.clone()).unwrap(),
    );
    let mut book_a = store_a.friends().unwrap().unwrap();
    let mut book_l = store_l.friends().unwrap().unwrap();
    let friend = |book: &fogata_friends::FriendBook| fogata_friends::Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    let (as_a, as_l) = (friend(&book_a), friend(&book_l));
    let (ana_key, leo) = (book_a.profile.device, book_l.profile.device);
    book_a.friends.insert(leo, as_l);
    book_l.friends.insert(ana_key, as_a);
    store_a.save_friends(&book_a).unwrap();
    store_l.save_friends(&book_l).unwrap();
    drop((store_a, store_l));

    let port = free_port();
    let (ana, _events) = Core::start(
        dir_a,
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    let mut online = old_online(&old, &dir_l, port);
    let leo_online = || async {
        ana.friends().await.unwrap().iter().any(|f| {
            f.person.device == leo && !matches!(f.presence, fogata_core::Presence::Offline { .. })
        })
    };
    eventually("the old friend online", 60, leo_online).await;

    // A still picture and banner, each with its animation.
    let still = b"\x89PNG\r\n\x1a\nstill".to_vec();
    let banner = b"\xff\xd8\xffbanner".to_vec();
    ana.set_profile(None, Some(fogata_friends::Avatar::Photo(still.clone())))
        .await
        .unwrap();
    ana.set_banner(0xFF3D21AD, Some(banner.clone()))
        .await
        .unwrap();
    let gif = gif(200 * 1024, 5);
    ana.set_animation(fogata_core::Animated::Photo, Some(gif.clone()))
        .await
        .unwrap();
    ana.set_animation(fogata_core::Animated::Banner, Some(gif))
        .await
        .unwrap();
    // Past a friend heartbeat (20 s): the old copy is still a friend online.
    tokio::time::sleep(Duration::from_secs(25)).await;
    assert!(leo_online().await, "the old friend went offline");

    let _ = online.kill();
    let _ = online.wait();
    ana.shutdown().await.unwrap();

    // The old copy's book: Ana's still picture and banner.
    let book = Store::open(dir_l).unwrap().friends().unwrap().unwrap();
    let ana_there = &book.friends[&ana_key];
    assert_eq!(
        ana_there.profile.avatar,
        fogata_friends::Avatar::Photo(still)
    );
    assert_eq!(
        ana_there.banner.as_ref().and_then(|b| b.picture.clone()),
        Some(banner)
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version whose animations and banners were smaller"]
async fn an_older_copy_refuses_a_bigger_animation_and_banner_and_keeps_the_rest() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-bigger-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let leo = old_identity(&old, &dir_l);

    let store_a = Store::open(dir_a.clone()).unwrap();
    let (ana_id, _) = store_a.identity().unwrap();
    let ana_key = ana_id.public_key();
    let mut log = GroupLog::create(&ana_id, "g", now()).unwrap();
    log.add(&ana_id, leo, "Leo", now()).unwrap();
    let group = log.group_id();
    store_a.save_group_log(&log, "g").unwrap();
    Store::open(dir_l.clone())
        .unwrap()
        .save_group_log(&log, "g")
        .unwrap();
    drop(store_a);

    let port = free_port();
    let (ana, _events) = Core::start(
        dir_a,
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    // A banner both versions take, then the old copy comes.
    let small = gif(100 * 1024, 1);
    ana.set_group_banner(group, Some(small.clone()))
        .await
        .unwrap();
    ana.join_room(group).await.unwrap();
    let mut chat = old_chat(&old, &dir_l, port);
    eventually("the old copy in the room", 60, || in_room(&ana, group, leo)).await;
    // Past a roster (one every heartbeat), with the banner offered.
    tokio::time::sleep(Duration::from_secs(25)).await;

    // Over what the old copy takes: a moving picture and a banner of 540 KiB.
    let still = b"\x89PNG\r\n\x1a\nstill".to_vec();
    ana.set_profile(None, Some(fogata_friends::Avatar::Photo(still.clone())))
        .await
        .unwrap();
    ana.set_animation(fogata_core::Animated::Photo, Some(gif(540 * 1024, 2)))
        .await
        .unwrap();
    ana.set_group_banner(group, Some(gif(540 * 1024, 3)))
        .await
        .unwrap();
    // Several rosters later the old copy is still there, and takes what else changes.
    tokio::time::sleep(Duration::from_secs(50)).await;
    ana.rename_group(group, "h").await.unwrap();
    // Time for the rename to reach the old copy, which only its own folder shows once it
    // quits (reading that folder meanwhile would have this version seal it under the old
    // copy's feet).
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(
        in_room(&ana, group, leo).await,
        "the old copy left the room"
    );

    let _ = chat.stdin.as_mut().unwrap().write_all(b"/quit\n");
    let _ = timeout(Duration::from_secs(10), async {
        while chat.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let _ = chat.kill();
    let _ = chat.wait();
    ana.shutdown().await.unwrap();

    // The old copy: the rename applied, nothing waiting, the banner it could take, Ana's
    // still picture and no animation of hers.
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert_eq!(kept.state().name, "h");
    assert!(kept.waiting().is_empty());
    assert_eq!(kept.banner().map(|b| b.picture.clone()), Some(small));
    let profile = store_l
        .member_profiles()
        .into_iter()
        .find(|p| p.device == ana_key)
        .expect("the old copy kept Ana's profile");
    assert_eq!(profile.avatar, fogata_friends::Avatar::Photo(still));
    assert!(store_l.animations().iter().all(|a| a.device != ana_key));
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version with unknown kinds of entry but before MLS"]
async fn an_older_copy_keeps_entries_for_mls_groups_and_the_rest_of_the_log() {
    use fogata_state::{Body, Changes, Entry, EpochLink, payload_digest};

    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-mls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let leo = old_identity(&old, &dir_l);

    // Ana's group with Leo in it; Leo's folder gets the log as it is now.
    let store_a = Store::open(dir_a.clone()).unwrap();
    let (ana_id, _) = store_a.identity().unwrap();
    let mut log = GroupLog::create(&ana_id, "g", now()).unwrap();
    log.add(&ana_id, leo, "Leo", now()).unwrap();
    let group = log.group_id();
    Store::open(dir_l.clone())
        .unwrap()
        .save_group_log(&log, "g")
        .unwrap();
    // Then Ana's log gets one entry of each kind for MLS groups, one on top of the other,
    // which Leo's copy does not know.
    let link = EpochLink {
        epoch: 0,
        prev: None,
    };
    let ticket = log.issue_ticket(&ana_id, now() + 60_000).unwrap();
    let mut mls = Vec::new();
    for body in [
        Body::MlsKeyPackage {
            key_package: vec![1; 369],
        },
        Body::MlsJoin {
            ticket,
            name: "Ana".into(),
            key_package: vec![2; 369],
        },
        Body::Commit {
            link,
            changes: Changes::new([leo], []),
            payload: payload_digest(b"commit"),
            welcome: Some(payload_digest(b"welcome")),
        },
        Body::Resync {
            link,
            payload: payload_digest(b"resync"),
        },
    ] {
        let entry = Entry::sign(&ana_id, group, log.heads(), now(), body);
        mls.push(entry.hash());
        log.insert(entry).unwrap();
    }
    store_a.save_group_log(&log, "g").unwrap();
    drop(store_a);

    let port = free_port();
    let (ana, _events) = Core::start(
        dir_a,
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    ana.join_room(group).await.unwrap();
    let mut chat = old_chat(&old, &dir_l, port);
    eventually("the old copy in the room", 60, || in_room(&ana, group, leo)).await;
    // A change signed on top of them: the old copy takes it only if it keeps them.
    ana.rename_group(group, "h").await.unwrap();
    // Time for the rename to reach the old copy, which only its own folder shows once it
    // quits (reading that folder meanwhile would have this version seal it under the old
    // copy's feet).
    tokio::time::sleep(Duration::from_secs(10)).await;
    assert!(
        in_room(&ana, group, leo).await,
        "the old copy left the room"
    );

    let _ = chat.stdin.as_mut().unwrap().write_all(b"/quit\n");
    let _ = timeout(Duration::from_secs(10), async {
        while chat.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let _ = chat.kill();
    let _ = chat.wait();
    ana.shutdown().await.unwrap();

    // The old copy's log file: every entry, the rename applied, nothing waiting.
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert_eq!(kept.state().name, "h");
    assert!(kept.waiting().is_empty());
    for hash in &mls {
        assert!(kept.get(hash).is_some_and(|e| e.body.is_mls()));
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version before declines were told"]
async fn an_old_copy_takes_a_declined_leave_as_before() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-decline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_b, dir_l) = (base.join("bea"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let made = Command::new(&old)
        .args(["profile", "--name", "Leo"])
        .env("FOGATA_DATA_DIR", &dir_l)
        .output()
        .unwrap();
    assert!(made.status.success(), "the old binary made no profile");

    // Friends already; Leo, the old copy, made a group and added Bea.
    let (store_b, store_l) = (
        Store::open(dir_b.clone()).unwrap(),
        Store::open(dir_l.clone()).unwrap(),
    );
    let mut book_b = store_b.friends().unwrap().unwrap();
    let mut book_l = store_l.friends().unwrap().unwrap();
    let friend = |book: &fogata_friends::FriendBook| fogata_friends::Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    let (as_b, as_l) = (friend(&book_b), friend(&book_l));
    let (bea, leo) = (book_b.profile.device, book_l.profile.device);
    book_b.friends.insert(leo, as_l);
    book_l.friends.insert(bea, as_b);
    store_b.save_friends(&book_b).unwrap();
    store_l.save_friends(&book_l).unwrap();
    let (leo_id, _) = store_l.identity().unwrap();
    let mut log = GroupLog::create(&leo_id, "g", now()).unwrap();
    log.add(&leo_id, bea, "Bea", now()).unwrap();
    let group = log.group_id();
    store_l.save_group_log(&log, "g").unwrap();
    store_b.save_offered_group(&log, &leo).unwrap();
    drop((store_b, store_l));

    let port = free_port();
    let (bea_core, _events) = Core::start(
        dir_b,
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    let mut chat = old_chat(&old, &dir_l, port);
    let leo_online = || async {
        bea_core.friends().await.unwrap().iter().any(|f| {
            f.person.device == leo && !matches!(f.presence, fogata_core::Presence::Offline { .. })
        })
    };
    eventually("the old copy online as a friend", 60, leo_online).await;

    // Bea says no: the old copy gets her leave with the new mark, and stays her friend.
    bea_core.decline_group_invitation(group).await.unwrap();
    // Past a friend heartbeat (20 s): the old copy is still a friend online.
    tokio::time::sleep(Duration::from_secs(25)).await;
    assert!(leo_online().await, "the old copy went offline");

    let _ = chat.stdin.as_mut().unwrap().write_all(b"/quit\n");
    let _ = timeout(Duration::from_secs(10), async {
        while chat.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let _ = chat.kill();
    let _ = chat.wait();
    bea_core.shutdown().await.unwrap();

    // The old copy's log: Bea's leave taken, Bea no longer a member, nothing damaged.
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert!(!kept.state().is_member(&bea));
    assert!(
        kept.entries()
            .any(|e| e.author == bea && e.body == fogata_state::Body::Leave)
    );
    assert!(kept.waiting().is_empty());
    assert!(!base.join("leo/logs").read_dir().unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".damaged")
    }));
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version before group picture records"]
async fn an_old_copy_shows_the_icon_once_a_picture_record_replaces_a_picture_entry() {
    use fogata_state::Body;

    let png = |fill: u8| {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.resize(20 * 1024, fill);
        png
    };
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-picture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let leo = old_identity(&old, &dir_l);

    // Ana's group with Leo in it and a picture set with an entry, as earlier versions do,
    // given to both folders.
    let store_a = Store::open(dir_a.clone()).unwrap();
    let (ana_id, _) = store_a.identity().unwrap();
    let mut log = GroupLog::create(&ana_id, "g", now()).unwrap();
    log.add(&ana_id, leo, "Leo", now()).unwrap();
    log.append(
        &ana_id,
        Body::SetPicture {
            picture: Some(png(1)),
        },
        now(),
    )
    .unwrap();
    let group = log.group_id();
    store_a.save_group_log(&log, "g").unwrap();
    Store::open(dir_l.clone())
        .unwrap()
        .save_group_log(&log, "g")
        .unwrap();
    drop(store_a);

    let port = free_port();
    let (ana, _events) = Core::start(
        dir_a.clone(),
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(ana.groups().await.unwrap()[0].picture, Some(png(1)));
    ana.join_room(group).await.unwrap();
    let mut chat = old_chat(&old, &dir_l, port);
    eventually("the old copy in the room", 60, || in_room(&ana, group, leo)).await;

    // Two pictures chosen while connected, rosters going between them, then a rename.
    tokio::time::sleep(Duration::from_secs(25)).await;
    ana.set_group_picture(group, Some(png(2))).await.unwrap();
    // A while between the two pictures.
    tokio::time::sleep(Duration::from_secs(5)).await;
    ana.set_group_picture(group, Some(png(3))).await.unwrap();
    ana.rename_group(group, "h").await.unwrap();
    // Time for the rename to reach the old copy, which only its own folder shows once it
    // quits (reading that folder meanwhile would have this version seal it under the old
    // copy's feet).
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_eq!(ana.groups().await.unwrap()[0].picture, Some(png(3)));
    assert!(
        in_room(&ana, group, leo).await,
        "the old copy left the room"
    );

    let _ = chat.stdin.as_mut().unwrap().write_all(b"/quit\n");
    let _ = timeout(Duration::from_secs(10), async {
        while chat.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    let _ = chat.kill();
    let _ = chat.wait();
    ana.shutdown().await.unwrap();

    // Ana: the newest picture, kept across a restart; one entry more, with no picture.
    let added = |kept: &GroupLog| -> Vec<Body> {
        kept.entries()
            .filter(|e| log.get(&e.hash()).is_none())
            .filter(|e| matches!(e.body, Body::SetPicture { .. }))
            .map(|e| e.body.clone())
            .collect()
    };
    let store_a = Store::open(dir_a).unwrap();
    let mine = store_a.load_log(&store_a.group("g").unwrap()).unwrap();
    assert_eq!(mine.picture(), Some(png(3)));
    assert_eq!(added(&mine), vec![Body::SetPicture { picture: None }]);

    // The old copy: the rename and the entry with no picture applied, nothing waiting, no
    // record: what it reads, the entries, shows the icon.
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert_eq!(kept.state().name, "h");
    assert!(kept.waiting().is_empty());
    assert_eq!(added(&kept), vec![Body::SetPicture { picture: None }]);
    assert_eq!(kept.state().picture, None);
    assert!(kept.picture_record().is_none());
    let names: Vec<String> = base
        .join("leo/logs")
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names
            .iter()
            .all(|n| !n.ends_with(".picture") && !n.ends_with(".damaged"))
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from a version before voice messages"]
async fn an_old_friend_keeps_a_voice_message_as_a_file() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-old-voice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let made = Command::new(&old)
        .args(["profile", "--name", "Leo"])
        .env("FOGATA_DATA_DIR", &dir_l)
        .output()
        .unwrap();
    assert!(made.status.success(), "the old binary made no profile");
    let (store_a, store_l) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_l.clone()).unwrap(),
    );
    let mut book_a = store_a.friends().unwrap().unwrap();
    let mut book_l = store_l.friends().unwrap().unwrap();
    let friend = |book: &fogata_friends::FriendBook| fogata_friends::Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    let (as_a, as_l) = (friend(&book_a), friend(&book_l));
    let (ana_key, leo) = (book_a.profile.device, book_l.profile.device);
    book_a.friends.insert(leo, as_l);
    book_l.friends.insert(ana_key, as_a);
    store_a.save_friends(&book_a).unwrap();
    store_l.save_friends(&book_l).unwrap();
    drop((store_a, store_l));

    let port = free_port();
    let (ana, _events) = Core::start(
        dir_a,
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    let mut online = old_online(&old, &dir_l, port);
    let leo_online = || async {
        ana.friends().await.unwrap().iter().any(|f| {
            f.person.device == leo && !matches!(f.presence, fogata_core::Presence::Offline { .. })
        })
    };
    eventually("the old friend online", 60, leo_online).await;

    // A voice message, then a few words: the old copy confirms both.
    let note = fogata_core::direct::VoiceNote {
        file: b"OggS voice".repeat(500),
        duration_ms: 3200,
        bars: vec![120; fogata_proto::VOICE_BARS],
    };
    ana.send_direct_voice(leo, note).await.unwrap();
    ana.send_direct(leo, "can you hear it?", vec![])
        .await
        .unwrap();
    eventually("both confirmed", 90, || async {
        let chat = ana.direct_chat(leo, 10).await.unwrap();
        chat.items.iter().filter(|i| i.item.delivered).count() >= 2
    })
    .await;
    // Past a friend heartbeat: the old copy runs on, a friend online.
    tokio::time::sleep(Duration::from_secs(25)).await;
    assert!(online.try_wait().unwrap().is_none(), "the old copy stopped");
    assert!(leo_online().await, "the old friend went offline");
    let _ = online.kill();
    let _ = online.wait();
    ana.shutdown().await.unwrap();

    // What the old copy kept, as this version reads it: a file of that name, not fetched.
    let (leo_core, _events) = Core::start(
        dir_l,
        NetOptions {
            listen_port: free_port(),
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        },
    )
    .unwrap();
    let chat = leo_core.direct_chat(ana_key, 10).await.unwrap();
    let files: Vec<_> = chat.items.iter().flat_map(|i| i.files.clone()).collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file.name, "Voice message.ogg");
    assert_eq!(
        files[0].file.kind,
        fogata_core::direct::model::FileKind::File
    );
    assert!(files[0].file.voice.is_none());
    assert!(chat.items.iter().any(|i| matches!(
        &i.item.body,
        fogata_core::direct::model::Body::Message { text, .. } if text == "can you hear it?"
    )));
    leo_core.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}
