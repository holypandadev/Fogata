//! Two devices in one process: one makes a group and an invite, the other joins with it, and
//! they talk in the group's room. Uses only the loopback interface and local addresses; no
//! DHT and no mDNS.

use std::time::Duration;

use fogata_core::CoreEvents;
use fogata_core::{Core, CoreEvent, FIRST_ROOM, NetOptions};
use fogata_crypto::GroupId;
use fogata_net::{Multiaddr, RoomChange};
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

fn options(port: u16, peers: Vec<Multiaddr>) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers,
        ..Default::default()
    }
}

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

/// Bea joins Ana's group with an invite and Ana lets her in.
async fn let_in(
    ana: &Core,
    events_a: &mut CoreEvents,
    bea: &Core,
    events_b: &mut CoreEvents,
    invite: &str,
    group: GroupId,
) {
    bea.join_group(invite, None, None).await.unwrap();
    let who = bea.me();
    wait_for(
        events_a,
        "the ask to join",
        |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
    )
    .await;
    ana.answer_join(group, who, true).await.unwrap();
    wait_for(
        events_b,
        "the join",
        |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn invite_join_and_talk() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-test-{}", std::process::id()));
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();

    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 3, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();

    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
    let (joined, name) = bea.join_group(&invite, None, None).await.unwrap();
    assert_eq!((joined, name.as_str()), (group, "crew"));
    // The invite alone does not let her in: Ana is asked, with Bea's name.
    wait_for(
        &mut events_b,
        "waiting for an admin",
        |e| matches!(e, CoreEvent::JoinWaiting { group: g, .. } if *g == group),
    )
    .await;
    // Bea may hear she waits before Ana's core has taken the ask.
    let who = bea.me();
    wait_for(
        &mut events_a,
        "the ask to join",
        |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
    )
    .await;
    let asks = ana.join_asks().await.unwrap();
    assert_eq!(asks.len(), 1);
    assert_eq!((asks[0].name.as_str(), asks[0].device), ("Bea", bea.me()));
    assert!(bea.groups().await.unwrap().is_empty());
    ana.answer_join(group, bea.me(), true).await.unwrap();
    wait_for(
        &mut events_b,
        "the join",
        |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
    )
    .await;
    // Ana's own list of asks changes when her node has the answer, which may come after
    // Bea already joined.
    let answered = timeout(Duration::from_secs(30), async {
        while !ana.join_asks().await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(
        answered.is_ok(),
        "Ana still lists Bea's ask after she joined"
    );
    let groups = bea.groups().await.unwrap();
    assert_eq!((groups[0].name.as_str(), groups[0].icon), ("crew", 3));

    ana.join_room(group).await.unwrap();
    bea.join_room(group).await.unwrap();
    wait_for(&mut events_a, "Bea in the room", |e| {
        matches!(e, CoreEvent::Room { change: RoomChange::Joined { device, .. }, .. } if *device == bea.me())
    })
    .await;
    ana.say(group, "hi Bea").await.unwrap();
    wait_for(&mut events_b, "Ana's message", |e| {
        matches!(e, CoreEvent::Room { change: RoomChange::Chat(line), .. } if line.text == "hi Bea")
    })
    .await;
    let room = bea.room(group, FIRST_ROOM).await.unwrap().unwrap();
    assert_eq!(room.participants.len(), 2);
    assert!(room.participants.iter().any(|(p, _)| p.name == "Ana"));

    // The group was kept on disk; what was said was not.
    let stored: Vec<String> = std::fs::read_dir(base.join("bea").join("logs"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let id = hex::encode(group.as_bytes());
    let mut kept = vec![format!("{id}.log"), format!("{id}.log.judged")];
    let mut stored = stored;
    stored.sort();
    kept.sort();
    assert_eq!(stored, kept);
    let _ = std::fs::remove_dir_all(&base);
}

/// Polls `check` until it holds, for up to 60 s: what members see of each other also
/// changes with the heartbeat, every 20 s.
async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let held = timeout(Duration::from_secs(60), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok();
    assert!(held, "timed out waiting for {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_appears_offline_shows_offline_outside_the_room() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-hidden-{}", std::process::id()));
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();

    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    let ana_online = || async {
        let members = bea.members(group).await.unwrap();
        let ana = members.iter().find(|m| !m.me).unwrap();
        (bea.groups().await.unwrap()[0].online, ana.online.is_some())
    };
    eventually("Bea to see Ana online", || async {
        ana_online().await == (1, true)
    })
    .await;

    ana.set_hidden(true).await.unwrap();
    eventually("Ana to show as offline", || async {
        ana_online().await == (0, false)
    })
    .await;

    // A room shows who is in it, so there she is online.
    ana.join_room(group).await.unwrap();
    eventually("Ana online in the room", || async {
        ana_online().await == (1, true)
    })
    .await;
    ana.leave_room(group).await.unwrap();
    eventually("Ana offline again", || async {
        ana_online().await == (0, false)
    })
    .await;

    ana.set_hidden(false).await.unwrap();
    eventually("Ana online again", || async {
        ana_online().await == (1, true)
    })
    .await;
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_picture_is_fetched_into_memory_for_its_preview() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-preview-{}", std::process::id()));
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    ana.join_room(group).await.unwrap();
    bea.join_room(group).await.unwrap();

    let picture = base.join("lake.png");
    let bytes: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&picture, &bytes).unwrap();
    let info = ana.share_file(group, picture, None).await.unwrap();
    eventually("Bea to see the picture", || async {
        let room = bea.room(group, FIRST_ROOM).await.unwrap();
        room.is_some_and(|r| r.files.iter().any(|f| f.info.id == info.id))
    })
    .await;

    assert!(bea.fetch_preview(group, info.id, 1_000).await.is_err());
    let got = bea.fetch_preview(group, info.id, 1 << 20).await.unwrap();
    assert_eq!(got, bytes);
    // Only Ana still has it: Bea did not start sharing it.
    let room = bea.room(group, FIRST_ROOM).await.unwrap().unwrap();
    let file = room.files.iter().find(|f| f.info.id == info.id).unwrap();
    assert_eq!(file.holders.len(), 1);

    // A video goes with its first picture and length; it is fetched to play, not shared on.
    let clip = base.join("clip.mp4");
    let movie: Vec<u8> = (0..80_000u32).map(|i| (i % 241) as u8).collect();
    std::fs::write(&clip, &movie).unwrap();
    let preview = fogata_core::VideoPreview {
        thumb: vec![0xFF, 0xD8, 1, 2, 3],
        width: 1280,
        height: 720,
        duration_ms: 42_000,
    };
    let video = ana
        .share_file(group, clip, Some(preview.clone()))
        .await
        .unwrap();
    eventually("Bea to see the video and its first picture", || async {
        let room = bea.room(group, FIRST_ROOM).await.unwrap();
        room.is_some_and(|r| {
            r.files
                .iter()
                .any(|f| f.info.id == video.id && f.info.video.as_ref() == Some(&preview))
        })
    })
    .await;
    let cache = base.join("cache");
    let played = bea.fetch_to_play(group, video.id, &cache).await.unwrap();
    assert_eq!(std::fs::read(&played).unwrap(), movie);
    assert!(played.to_string_lossy().ends_with(".mp4"));
    // Asked again, it is already there.
    assert_eq!(
        bea.fetch_to_play(group, video.id, &cache).await.unwrap(),
        played
    );
    let room = bea.room(group, FIRST_ROOM).await.unwrap().unwrap();
    let file = room.files.iter().find(|f| f.info.id == video.id).unwrap();
    assert_eq!(file.holders.len(), 1);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_declined_invite_lets_nobody_in() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-declined-{}", std::process::id()));
    let (dir_a, dir_b, dir_c) = (base.join("ana"), base.join("bea"), base.join("cris"));
    for (dir, name) in [(&dir_a, "Ana"), (&dir_b, "Bea"), (&dir_c, "Cris")] {
        Core::create_profile(dir, name, None).unwrap();
    }
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();

    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana.clone()])).unwrap();
    bea.join_group(&invite, None, None).await.unwrap();
    let who = bea.me();
    wait_for(
        &mut events_a,
        "Bea's ask",
        |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
    )
    .await;
    ana.answer_join(group, who, false).await.unwrap();
    wait_for(
        &mut events_b,
        "the no",
        |e| matches!(e, CoreEvent::JoinDeclined { group: g, .. } if *g == group),
    )
    .await;
    assert!(bea.groups().await.unwrap().is_empty());
    assert_eq!(ana.groups().await.unwrap()[0].members, 1);

    // The invite was passed on: it no longer works for anyone.
    let (cris, mut events_c) = Core::start(dir_c, options(0, vec![to_ana])).unwrap();
    cris.join_group(&invite, None, None).await.unwrap();
    wait_for(
        &mut events_c,
        "the no for Cris",
        |e| matches!(e, CoreEvent::JoinDeclined { group: g, .. } if *g == group),
    )
    .await;
    assert!(ana.join_asks().await.unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn voice_rooms_are_apart_and_fill_up() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-voice-{}", std::process::id()));
    let (dir_a, dir_b, dir_c) = (base.join("ana"), base.join("bea"), base.join("cris"));
    for (dir, name) in [(&dir_a, "Ana"), (&dir_b, "Bea"), (&dir_c, "Cris")] {
        Core::create_profile(dir, name, None).unwrap();
    }
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana.clone()])).unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    let (cris, mut events_c) = Core::start(dir_c, options(0, vec![to_ana])).unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &cris, &mut events_c, &invite, group).await;

    let gaming = ana.set_voice_room(group, None, "Gaming").await.unwrap();
    eventually("both voice rooms at Bea's", || async {
        let groups = bea.groups().await.unwrap();
        groups[0]
            .voice_rooms
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>()
            == ["", "Gaming"]
    })
    .await;

    ana.join_room(group).await.unwrap();
    bea.join_voice_room(group, gaming, None).await.unwrap();
    eventually("each in their own room", || async {
        let groups = ana.groups().await.unwrap();
        let names = |i: usize| -> Vec<String> {
            groups[0].voice_rooms[i]
                .room
                .iter()
                .flat_map(|r| r.participants.iter().map(|p| p.name.clone()))
                .collect()
        };
        names(0) == ["Ana"] && names(1) == ["Bea"]
    })
    .await;
    ana.say(group, "only the first room").await.unwrap();
    bea.say(group, "only gaming").await.unwrap();
    eventually("the chats to stay apart", || async {
        let first = ana.room(group, FIRST_ROOM).await.unwrap().unwrap();
        let other = bea.room(group, gaming).await.unwrap().unwrap();
        let texts = |chat: &[fogata_net::ChatLine]| -> Vec<String> {
            chat.iter().map(|l| l.text.clone()).collect()
        };
        texts(&first.chat) == ["only the first room"] && texts(&other.chat) == ["only gaming"]
    })
    .await;

    // Two at most in each room: Cris gets in with Bea, Ana does not.
    ana.set_group_limits(group, 20, 2).await.unwrap();
    eventually("the limit at Cris's", || async {
        cris.groups().await.unwrap()[0].max_in_room == 2
    })
    .await;
    cris.join_voice_room(group, gaming, None).await.unwrap();
    eventually("Cris in gaming", || async {
        ana.room(group, gaming)
            .await
            .unwrap()
            .is_some_and(|r| r.participants.len() == 2)
    })
    .await;
    assert!(ana.join_voice_room(group, gaming, None).await.is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_whose_name_a_room_refuses_is_shared_under_a_fair_one() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-fair-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir_a = base.join("ana");
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let (ana, _events_a) = Core::start(dir_a, options(free_port(), Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    ana.join_room(group).await.unwrap();
    // A right-to-left mark, as Arabic or Hebrew names can carry.
    let path = base.join("notes\u{200F}.txt");
    std::fs::write(&path, b"hi").unwrap();
    let info = ana.share_file(group, path, None).await.unwrap();
    assert_eq!(info.name, "notes.txt");
    let room = ana.room(group, FIRST_ROOM).await.unwrap().unwrap();
    let names: Vec<&str> = room.files.iter().map(|f| f.info.name.as_str()).collect();
    assert_eq!(names, ["notes.txt"]);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_downloaded_twice_at_once_is_saved_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-twice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    ana.join_room(group).await.unwrap();
    bea.join_room(group).await.unwrap();
    let path = base.join("party.mp4");
    let bytes: Vec<u8> = (0..8u32 << 20).map(|i| (i % 249) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let info = ana.share_file(group, path, None).await.unwrap();
    eventually("Bea to see the file", || async {
        let room = bea.room(group, FIRST_ROOM).await.unwrap();
        room.is_some_and(|r| r.files.iter().any(|f| f.info.id == info.id))
    })
    .await;
    // A double click.
    let downloads = base.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let first = bea.download(group, info.id, &downloads).await.unwrap();
    let second = bea.download(group, info.id, &downloads).await.unwrap();
    assert_eq!(first, second);
    wait_for(
        &mut events_b,
        "the download",
        |e| matches!(e, CoreEvent::DownloadDone { file, .. } if *file == info.id),
    )
    .await;
    // Nothing else is still arriving.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let saved: Vec<_> = std::fs::read_dir(&downloads)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&base);
}

/// Two devices in one group's room, Bea having joined Ana's, with a video of `bytes` that
/// Ana shares and Bea sees.
async fn sharing_a_video(
    base: &std::path::Path,
    bytes: &[u8],
) -> (Core, Core, CoreEvents, GroupId, [u8; 32]) {
    let _ = std::fs::remove_dir_all(base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    ana.join_room(group).await.unwrap();
    bea.join_room(group).await.unwrap();
    let path = base.join("party.mp4");
    std::fs::write(&path, bytes).unwrap();
    let info = ana.share_file(group, path, None).await.unwrap();
    eventually("Bea to see the video", || async {
        let room = bea.room(group, FIRST_ROOM).await.unwrap();
        room.is_some_and(|r| r.files.iter().any(|f| f.info.id == info.id))
    })
    .await;
    (ana, bea, events_b, group, info.id)
}

/// Waits until something has come into the `.part` of a fetch into `dir`.
async fn part_grows(dir: &std::path::Path) {
    let grown = || {
        std::fs::read_dir(dir).is_ok_and(|mut d| {
            d.any(|e| {
                e.is_ok_and(|e| {
                    // The file's own size: on Windows a folder's listing keeps the size a
                    // file being written had when it was opened.
                    e.file_name().to_string_lossy().ends_with(".part")
                        && std::fs::metadata(e.path()).is_ok_and(|m| m.len() > 0)
                })
            })
        })
    };
    let grew = timeout(Duration::from_secs(60), async {
        while !grown() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await;
    assert!(grew.is_ok(), "nothing came");
}

/// A video opened to play again while it comes (its view closed and opened again) is
/// fetched once: a second fetch would go on from the `.part` the first one writes, both
/// adding to it, each checking only what it got itself.
#[tokio::test(flavor = "multi_thread")]
async fn a_video_fetched_to_play_again_while_it_comes_comes_whole() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-play-twice-{}", std::process::id()));
    let bytes: Vec<u8> = (0..32u32 << 20).map(|i| (i % 253) as u8).collect();
    let (_ana, bea, _events, group, id) = sharing_a_video(&base, &bytes).await;
    let cache = base.join("videos");
    let first = {
        let (bea, cache) = (bea.clone(), cache.clone());
        tokio::spawn(async move { bea.fetch_to_play(group, id, &cache).await })
    };
    part_grows(&cache).await;
    let second = {
        let (bea, cache) = (bea.clone(), cache.clone());
        tokio::spawn(async move { bea.fetch_to_play(group, id, &cache).await })
    };
    // Whole as soon as the first one says where it is: the player opens it then.
    let first = first.await.unwrap().unwrap();
    let got = std::fs::read(&first).unwrap();
    assert_eq!(got.len(), bytes.len());
    assert_eq!(blake3::hash(&got).as_bytes(), &id);
    let second = timeout(Duration::from_secs(60), second).await.unwrap();
    assert_eq!(second.unwrap().unwrap(), first);
    // Nothing else is still arriving, and nothing else is left.
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    let left: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// A video fetched to play into another folder while it comes into one is fetched there
/// too: each asker gets it in the folder it gave.
#[tokio::test(flavor = "multi_thread")]
async fn a_video_fetched_to_play_into_two_folders_comes_into_each() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-play-two-{}", std::process::id()));
    let bytes: Vec<u8> = (0..16u32 << 20).map(|i| (i % 251) as u8).collect();
    let (_ana, bea, _events, group, id) = sharing_a_video(&base, &bytes).await;
    let (one, two) = (base.join("videos"), base.join("other videos"));
    let first = {
        let (bea, one) = (bea.clone(), one.clone());
        tokio::spawn(async move { bea.fetch_to_play(group, id, &one).await })
    };
    part_grows(&one).await;
    let second = timeout(Duration::from_secs(60), bea.fetch_to_play(group, id, &two))
        .await
        .unwrap()
        .unwrap();
    let first = first.await.unwrap().unwrap();
    assert!(first.starts_with(&one), "{}", first.display());
    assert!(second.starts_with(&two), "{}", second.display());
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    assert_eq!(std::fs::read(&second).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&base);
}

/// A room file that would not fit on its disk, as its sharer gives its size, is neither
/// downloaded nor fetched to play: nothing is written.
#[tokio::test(flavor = "multi_thread")]
async fn a_room_file_too_big_for_the_disk_is_not_fetched() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-no-room-{}", std::process::id()));
    let bytes: Vec<u8> = (0..1u32 << 20).map(|i| (i % 241) as u8).collect();
    let (_ana, bea, _events, group, id) = sharing_a_video(&base, &bytes).await;
    // A disk with 256 MiB free: the file and the 256 MiB kept free do not fit.
    bea.use_free_space(|_| Some(256 << 20));
    let (cache, downloads) = (base.join("videos"), base.join("downloads"));
    std::fs::create_dir_all(&downloads).unwrap();
    let played = bea.fetch_to_play(group, id, &cache).await.unwrap_err();
    assert!(played.to_string().contains("free space"), "{played}");
    let saved = bea.download(group, id, &downloads).await.unwrap_err();
    assert!(saved.to_string().contains("free space"), "{saved}");
    // Nor is anything written a moment later.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(std::fs::read_dir(&cache).map_or(true, |mut d| d.next().is_none()));
    assert!(std::fs::read_dir(&downloads).unwrap().next().is_none());
    // With room for it, it comes.
    bea.use_free_space(|_| Some(1 << 40));
    let played = bea.fetch_to_play(group, id, &cache).await.unwrap();
    assert_eq!(std::fs::read(played).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&base);
}

/// A video being fetched to play stops when this device leaves the room, and what came of
/// it goes: it would otherwise go on filling the disk for a room nobody looks at.
#[tokio::test(flavor = "multi_thread")]
async fn leaving_the_room_stops_a_video_fetched_to_play() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-play-leave-{}", std::process::id()));
    let bytes: Vec<u8> = (0..32u32 << 20).map(|i| (i % 247) as u8).collect();
    let (_ana, bea, _events, group, id) = sharing_a_video(&base, &bytes).await;
    // What Ana sends is no longer that video: no try gets it, and between tries the fetch
    // waits a few seconds to try again.
    let other: Vec<u8> = bytes.iter().map(|b| b ^ 0x55).collect();
    std::fs::write(base.join("party.mp4"), &other).unwrap();
    let cache = base.join("videos");
    let fetching = {
        let (bea, cache) = (bea.clone(), cache.clone());
        tokio::spawn(async move { bea.fetch_to_play(group, id, &cache).await })
    };
    part_grows(&cache).await;
    bea.leave_room(group).await.unwrap();
    let ended = timeout(Duration::from_millis(1000), fetching).await;
    let fetched = ended.expect("the fetch stops at once").unwrap();
    assert!(fetched.is_err());
    let left: Vec<_> = std::fs::read_dir(&cache)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(&base);
}

/// What an earlier download left in a `.part` file, sent by another holder or by one this
/// run of the app does not know, is not gone on from: a holder that sent wrong bytes and
/// stopped would spoil the download from the next. It starts again from the first byte.
#[tokio::test(flavor = "multi_thread")]
async fn a_partial_download_from_another_holder_is_not_gone_on_from() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-part-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    ana.join_room(group).await.unwrap();
    bea.join_room(group).await.unwrap();
    let path = base.join("party.mp4");
    let bytes: Vec<u8> = (0..4u32 << 20).map(|i| (i % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let info = ana.share_file(group, path, None).await.unwrap();
    eventually("Bea to see the file", || async {
        let room = bea.room(group, FIRST_ROOM).await.unwrap();
        room.is_some_and(|r| r.files.iter().any(|f| f.info.id == info.id))
    })
    .await;
    // Half of it, wrong, as another holder left it.
    let downloads = base.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let part = downloads.join(format!("party.mp4.{}.part", hex::encode(&info.id[..4])));
    std::fs::write(&part, vec![0u8; 2 << 20]).unwrap();
    bea.download(group, info.id, &downloads).await.unwrap();
    // The first bytes that come count from the start, not from the half.
    let mut first = None;
    wait_for(&mut events_b, "the download", |e| match e {
        CoreEvent::Download { file, received, .. } if *file == info.id => {
            first.get_or_insert(*received);
            false
        }
        CoreEvent::DownloadDone { file, .. } => *file == info.id,
        CoreEvent::DownloadFailed { file, .. } => *file == info.id,
        _ => false,
    })
    .await;
    let first = first.unwrap();
    assert!(
        first < 2 << 20,
        "went on from the half: {first} bytes first"
    );
    assert_eq!(std::fs::read(downloads.join("party.mp4")).unwrap(), bytes);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_past_the_most_a_room_lists_is_refused_with_a_reason() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-many-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir_a = base.join("ana");
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let (ana, _events_a) = Core::start(dir_a, options(free_port(), Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    ana.join_room(group).await.unwrap();
    let files = base.join("files");
    std::fs::create_dir_all(&files).unwrap();
    let most = fogata_state::MAX_SHARES_EACH;
    for n in 0..most {
        let path = files.join(format!("{n}.txt"));
        std::fs::write(&path, n.to_string()).unwrap();
        ana.share_file(group, path, None).await.unwrap();
    }
    let path = files.join("one more.txt");
    std::fs::write(&path, b"one more").unwrap();
    let refused = ana.share_file(group, path, None).await;
    let room = ana.room(group, FIRST_ROOM).await.unwrap().unwrap();
    assert_eq!(room.files.len(), most);
    assert!(
        refused.is_err(),
        "said shared, but the room does not list it"
    );
    // One already shared can be shared again.
    let again = files.join("0.txt");
    ana.share_file(group, again, None).await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_left_can_be_joined_again_after_a_restart() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-rejoin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options(0, vec![to_ana.clone()])).unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    // Bea leaves at once, maybe before Ana has counted her device in: the leave still
    // reaches Ana.
    bea.leave_group(group).await.unwrap();
    eventually("Ana to see Bea leave", || async {
        ana.groups().await.unwrap()[0].members == 1
    })
    .await;

    // Bea starts again: the group she left runs a while to deliver her leave, and she can
    // still join it again with a new invite.
    bea.shutdown().await.unwrap();
    drop(bea);
    let mut started = None;
    for _ in 0..50 {
        match Core::start(dir_b.clone(), options(0, vec![to_ana.clone()])) {
            Ok(s) => {
                started = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let (bea, mut events_b) = started.expect("the folder stayed locked");
    assert!(bea.groups().await.unwrap().is_empty());
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    eventually("Bea in the group again", || async {
        bea.groups().await.unwrap().len() == 1 && ana.groups().await.unwrap()[0].members == 2
    })
    .await;
    let _ = std::fs::remove_dir_all(&base);
}

/// Starts a core on a folder another core just let go of: its lock goes with the last of
/// its tasks.
async fn start_again(dir: &std::path::Path, options: NetOptions) -> (Core, CoreEvents) {
    for _ in 0..50 {
        match Core::start(dir.to_path_buf(), options.clone()) {
            Ok(started) => return started,
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("the folder stayed locked");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_leave_made_while_the_others_are_away_reaches_them_when_they_come_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-leave-away-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let (port_a, port_b) = (free_port(), free_port());
    let to = |port: u16| -> Multiaddr {
        format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap()
    };
    let (ana, mut events_a) = Core::start(dir_a.clone(), options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, vec![to(port_a)])).unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    eventually("Ana to count Bea in", || async {
        ana.groups().await.unwrap()[0].online == 1
    })
    .await;

    // Ana closes Fogata, and Bea leaves while she is away: the group is gone for Bea at
    // once.
    ana.shutdown().await.unwrap();
    drop(ana);
    drop(events_a);
    bea.leave_group(group).await.unwrap();
    assert!(bea.groups().await.unwrap().is_empty());

    // Ana comes back: Bea's leave reaches her without Bea starting again.
    let (ana, _events_a) = start_again(&dir_a, options(port_a, vec![to(port_b)])).await;
    eventually("Ana to see Bea leave", || async {
        ana.groups().await.unwrap()[0].members == 1
    })
    .await;
    assert!(bea.groups().await.unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_join_waiting_for_an_admin_goes_on_after_a_restart() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-join-later-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    // Bea dials Ana on this machine, as in the other tests, rather than at the local network
    // addresses the invite names, which a computer's firewall may hold back.
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, _events_b) = Core::start(dir_b.clone(), options(0, vec![to_ana.clone()])).unwrap();
    bea.join_group(&invite, None, None).await.unwrap();
    let who = bea.me();
    wait_for(
        &mut events_a,
        "the ask to join",
        |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
    )
    .await;
    // Bea closes Fogata before Ana answers; Ana lets her in while she is away.
    bea.shutdown().await.unwrap();
    drop(bea);
    ana.answer_join(group, who, true).await.unwrap();
    let mut started = None;
    for _ in 0..50 {
        match Core::start(dir_b.clone(), options(0, vec![to_ana.clone()])) {
            Ok(s) => {
                started = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let (bea, mut events_b) = started.expect("the folder stayed locked");
    wait_for(
        &mut events_b,
        "the join after the restart",
        |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
    )
    .await;
    assert_eq!(bea.groups().await.unwrap().len(), 1);
    // Nothing waits any more: the next start has no join to go on with.
    assert!(bea.store().joins().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_picture_is_a_png_or_jpeg() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-picture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir_a = base.join("ana");
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let (ana, _events) = Core::start(dir_a.clone(), options(free_port(), Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let png = b"\x89PNG\r\n\x1a\nmore".to_vec();
    ana.set_group_picture(group, Some(png.clone()))
        .await
        .unwrap();
    assert_eq!(ana.groups().await.unwrap()[0].picture, Some(png.clone()));
    assert!(
        ana.set_group_picture(group, Some(b"GIF89a".to_vec()))
            .await
            .is_err()
    );
    ana.shutdown().await.unwrap();
    drop(ana);

    // Kept in a record of its own across a restart, not in the log.
    let store = fogata_core::Store::open(dir_a.clone()).unwrap();
    let (identity, _) = store.identity().unwrap();
    let stored = store.groups().unwrap().remove(0);
    let mut log = store.load_log(&stored).unwrap();
    assert_eq!(log.picture(), Some(png));
    assert!(
        log.entries()
            .all(|e| !matches!(e.body, fogata_state::Body::SetPicture { .. }))
    );

    // One that another device signed anyway, later, shows the icon here.
    let picture = Some(b"GIF89a".to_vec());
    let later = log.picture_record().unwrap().time_ms + 1;
    log.append(&identity, fogata_state::Body::SetPicture { picture }, later)
        .unwrap();
    store.save_group_log(&log, &stored.name).unwrap();
    drop(store);
    let mut started = None;
    for _ in 0..50 {
        match Core::start(dir_a.clone(), options(0, Vec::new())) {
            Ok(s) => {
                started = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let (ana, _events) = started.expect("the folder stayed locked");
    assert_eq!(ana.groups().await.unwrap()[0].picture, None);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn someone_gone_quiet_keeps_no_place_in_a_full_room() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-core-full-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b, dir_c) = (base.join("ana"), base.join("bea"), base.join("cris"));
    for (dir, name) in [(&dir_a, "Ana"), (&dir_b, "Bea"), (&dir_c, "Cris")] {
        Core::create_profile(dir, name, None).unwrap();
    }
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana.clone()])).unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &bea, &mut events_b, &invite, group).await;
    let (cris, mut events_c) = Core::start(dir_c, options(0, vec![to_ana])).unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let_in(&ana, &mut events_a, &cris, &mut events_c, &invite, group).await;
    ana.set_group_limits(group, 20, 2).await.unwrap();
    ana.join_room(group).await.unwrap();
    bea.join_room(group).await.unwrap();
    eventually("the room full at Cris's", || async {
        cris.groups().await.unwrap()[0].max_in_room == 2
            && cris
                .room(group, FIRST_ROOM)
                .await
                .unwrap()
                .is_some_and(|r| r.participants.len() == 2)
    })
    .await;
    assert!(cris.join_room(group).await.is_err(), "the room is full");

    // Bea's Fogata stops without saying it leaves, as in a crash: once she seems away, her
    // place is free, well before the grace runs out. Nobody reaches her any more, so she may
    // be dropped first.
    bea.shutdown().await.unwrap();
    drop(bea);
    wait_for(&mut events_c, "Bea away", |e| {
        matches!(
            e,
            CoreEvent::Room {
                change: RoomChange::Away { .. }
                    | RoomChange::Left {
                        on_purpose: false,
                        ..
                    },
                ..
            }
        )
    })
    .await;
    cris.join_room(group).await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}
