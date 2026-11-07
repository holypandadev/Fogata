//! Two devices that are friends, in one process: one calls the other, who answers, and
//! either hangs up; a call can also be declined or stop ringing. Each dials the others'
//! loopback addresses, and nothing else of the network is used.

use std::time::Duration;

use fogata_core::CoreEvents;
use fogata_core::{CallEnd, CallPhase, Core, CoreEvent, NetOptions, Store, direct::model::Body};
use fogata_crypto::DevicePublicKey;
use fogata_friends::Friend;
use tokio::time::timeout;

/// One test at a time: each starts several devices that talk over this computer's network,
/// and many at once crowd it.
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

/// Two data folders whose profiles are already friends.
fn friends(base: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let dirs = all_friends(base, &["Ana", "Bea"]);
    (dirs[0].clone(), dirs[1].clone())
}

/// A data folder for each name, whose profiles are all friends of each other.
fn all_friends(base: &std::path::Path, names: &[&str]) -> Vec<std::path::PathBuf> {
    let mut pairs = Vec::new();
    for i in 0..names.len() {
        for j in i + 1..names.len() {
            pairs.push((i, j));
        }
    }
    some_friends(base, names, &pairs)
}

/// A data folder for each name; the profiles of each pair (by index) are friends.
fn some_friends(
    base: &std::path::Path,
    names: &[&str],
    pairs: &[(usize, usize)],
) -> Vec<std::path::PathBuf> {
    let dirs: Vec<std::path::PathBuf> = names
        .iter()
        .map(|name| base.join(name.to_lowercase()))
        .collect();
    for (dir, name) in dirs.iter().zip(names) {
        Core::create_profile(dir, name, None).unwrap();
    }
    let stores: Vec<Store> = dirs
        .iter()
        .map(|dir| Store::open(dir.clone()).unwrap())
        .collect();
    let books: Vec<fogata_friends::FriendBook> = stores
        .iter()
        .map(|store| store.friends().unwrap().unwrap())
        .collect();
    let friend = |book: &fogata_friends::FriendBook| Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    for (i, store) in stores.iter().enumerate() {
        let mut book = books[i].clone();
        for (j, other) in books.iter().enumerate() {
            if pairs.contains(&(i, j)) || pairs.contains(&(j, i)) {
                book.friends.insert(other.profile.device, friend(other));
            }
        }
        store.save_friends(&book).unwrap();
    }
    dirs
}

/// Waits until `core` is the only one left in `call`, waiting for the others. Someone who
/// hangs up leaves the call's room at once; when that word is lost on a busy computer, the
/// room counts them gone after two minutes without hearing from them.
async fn waits_alone(core: &Core, call: fogata_crypto::GroupId) {
    let alone = timeout(Duration::from_secs(180), async {
        loop {
            let calls = core.calls().await.unwrap();
            if calls
                .iter()
                .any(|c| c.group == call && c.alone_since_ms.is_some())
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(alone.is_ok(), "not left waiting alone");
}

/// Waits until `core` sees `count` friends online.
async fn online(core: &Core, count: usize) {
    let all = timeout(Duration::from_secs(60), async {
        loop {
            let friends = core.friends().await.unwrap();
            if friends.iter().filter(|f| f.path.is_some()).count() >= count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(all.is_ok(), "friends did not come online");
}

/// Options for `count` devices: each listens on a loopback port of its own and dials all the
/// others, so they find each other without the local network.
fn dialling(count: usize) -> Vec<NetOptions> {
    // All bound at once, so no two get the same port.
    let sockets: Vec<std::net::UdpSocket> = (0..count)
        .map(|_| std::net::UdpSocket::bind("127.0.0.1:0").unwrap())
        .collect();
    let ports: Vec<u16> = sockets
        .iter()
        .map(|s| s.local_addr().unwrap().port())
        .collect();
    drop(sockets);
    ports
        .iter()
        .map(|&port| NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            peers: ports
                .iter()
                .filter(|&&other| other != port)
                .map(|other| {
                    format!("/ip4/127.0.0.1/udp/{other}/quic-v1")
                        .parse()
                        .unwrap()
                })
                .collect(),
            ..Default::default()
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn friends_call_answer_and_hang_up() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-calls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let options = dialling(2);
    let (ana, mut events_a) = Core::start(dir_a, options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options[1].clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    // Ana calls; Bea's rings; Bea answers and both are in the call's room.
    let call = ana.start_call(bea.me()).await.unwrap();
    assert_eq!(ana.calls().await.unwrap()[0].phase, CallPhase::Ringing);
    wait_for(&mut events_b, "the ring", |e| {
        matches!(e, CoreEvent::CallIncoming { group, from } if *group == call && *from == ana.me())
    })
    .await;
    bea.accept_call(call).await.unwrap();
    wait_for(
        &mut events_a,
        "the answer",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == call),
    )
    .await;
    let both = || async {
        let room = ana.room(call, fogata_core::FIRST_ROOM).await.unwrap();
        room.is_some_and(|r| r.participants.len() == 2)
    };
    let met = timeout(Duration::from_secs(30), async {
        while !both().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(met.is_ok(), "they did not meet in the call's room");
    // A file shared in the call reaches the other, as in a group's room.
    let picture = base.join("lake.png");
    let bytes: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&picture, &bytes).unwrap();
    let info = ana.share_file(call, picture, None).await.unwrap();
    let listed = timeout(Duration::from_secs(30), async {
        loop {
            let room = bea.room(call, fogata_core::FIRST_ROOM).await.unwrap();
            if room.is_some_and(|r| r.files.iter().any(|f| f.info.id == info.id)) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(listed.is_ok(), "the file did not show in the call");
    let got = bea.fetch_preview(call, info.id, 1 << 20).await.unwrap();
    assert_eq!(got, bytes);
    let saved = bea.download(call, info.id, &base).await.unwrap();
    wait_for(&mut events_b, "the download", |e| {
        matches!(e, CoreEvent::DownloadDone { file, .. } if *file == info.id)
            || matches!(e, CoreEvent::DownloadFailed { file, .. } if *file == info.id)
    })
    .await;
    assert_eq!(std::fs::read(&saved).unwrap(), bytes);
    // The call's group is neither listed nor saved.
    assert!(ana.groups().await.unwrap().is_empty());
    assert!(bea.groups().await.unwrap().is_empty());
    assert!(ana.store().groups().unwrap().is_empty());
    assert!(bea.store().groups().unwrap().is_empty());

    // Ana hangs up: Bea is left waiting for her, and hangs up too.
    ana.end_call(call).await.unwrap();
    waits_alone(&bea, call).await;
    bea.end_call(call).await.unwrap();
    assert!(ana.calls().await.unwrap().is_empty());
    assert!(bea.calls().await.unwrap().is_empty());

    // Bea calls back and Ana declines.
    let again = bea.start_call(ana.me()).await.unwrap();
    wait_for(
        &mut events_a,
        "the second ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == again),
    )
    .await;
    ana.decline_call(again).await.unwrap();
    wait_for(&mut events_b, "the decline", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Declined, .. } if *group == again)
    })
    .await;

    // Bea calls and stops ringing before Ana answers: Ana missed it.
    let third = bea.start_call(ana.me()).await.unwrap();
    wait_for(
        &mut events_a,
        "the third ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == third),
    )
    .await;
    bea.end_call(third).await.unwrap();
    wait_for(&mut events_a, "the missed call", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Missed, .. } if *group == third)
    })
    .await;

    // A friend appearing offline cannot be called.
    ana.set_hidden(true).await.unwrap();
    let shown_offline = timeout(Duration::from_secs(30), async {
        loop {
            let friends = bea.friends().await.unwrap();
            if friends.iter().all(|f| f.path.is_none()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(shown_offline.is_ok(), "Ana did not show as offline");
    assert!(bea.start_call(ana.me()).await.is_err());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_shows_the_friend_as_known_now() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-names-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let options = dialling(2);
    let (ana, mut events_a) = Core::start(dir_a, options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options[1].clone()).unwrap();
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
    wait_for(
        &mut events_a,
        "the answer",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == call),
    )
    .await;
    let name = || async { ana.calls().await.unwrap()[0].with.name.clone() };
    let held = |what: &'static str, want: &'static str| async move {
        let shown = timeout(Duration::from_secs(60), async {
            while name().await != want {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(shown.is_ok(), "{what}: the call shows {:?}", name().await);
    };
    bea.set_profile(Some("Beatriz".into()), None).await.unwrap();
    held("the new name", "Beatriz").await;
    // Bea removes Ana in the middle of the call: the call ends on both sides.
    bea.remove_friend(ana.me()).await.unwrap();
    assert!(bea.calls().await.unwrap().is_empty(), "Bea's call goes on");
    wait_for(
        &mut events_a,
        "the hang-up",
        |e| matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::HungUp, .. } if *group == call),
    )
    .await;
    assert!(ana.calls().await.unwrap().is_empty(), "Ana's call goes on");
    let gone = timeout(Duration::from_secs(60), async {
        while !ana.friends().await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(gone.is_ok(), "Ana still has Bea as a friend");
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn friends_who_call_each_other_at_once_meet_in_one_call() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-crossed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let options = dialling(2);
    let (ana, mut events_a) = Core::start(dir_a, options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options[1].clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    // Both press Call before either ring arrives.
    let (a, b) = tokio::join!(ana.start_call(bea.me()), bea.start_call(ana.me()));
    let kept = a.unwrap().min(b.unwrap());
    let accepted = |e: &CoreEvent| matches!(e, CoreEvent::CallAccepted { group } if *group == kept);
    wait_for(&mut events_a, "Ana in the call", accepted).await;
    wait_for(&mut events_b, "Bea in the call", accepted).await;
    for core in [&ana, &bea] {
        let calls = core.calls().await.unwrap();
        assert_eq!(calls.len(), 1, "one call each");
        assert_eq!(
            (calls[0].group, calls[0].phase),
            (kept, CallPhase::Connected)
        );
    }
    let met = timeout(Duration::from_secs(30), async {
        loop {
            let room = ana.room(kept, fogata_core::FIRST_ROOM).await.unwrap();
            if room.is_some_and(|r| r.participants.len() == 2) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(met.is_ok(), "they did not meet in the call's room");
    // Neither heard of a busy friend, given time for word of one to come.
    tokio::time::sleep(Duration::from_secs(2)).await;
    for events in [&mut events_a, &mut events_b] {
        while let Ok(e) = events.try_recv() {
            assert!(
                !matches!(e, CoreEvent::CallEnded { group, .. } if group == kept),
                "the call ended"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_who_rings_during_a_call_shows_as_missed() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-busy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dirs = all_friends(&base, &["Ana", "Bea", "Carla"]);
    let options = dialling(3);
    let (ana, _events_a) = Core::start(dirs[0].clone(), options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dirs[1].clone(), options[1].clone()).unwrap();
    let (carla, mut events_c) = Core::start(dirs[2].clone(), options[2].clone()).unwrap();
    for core in [&ana, &bea, &carla] {
        online(core, 2).await;
    }

    // Ana rings Bea; Carla rings Bea while Ana's call still rings there.
    let call = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "Ana's ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    let first = carla.start_call(bea.me()).await.unwrap();
    wait_for(&mut events_c, "Bea busy", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Busy, .. } if *group == first)
    })
    .await;
    wait_for(&mut events_b, "Carla's missed call", |e| {
        matches!(e, CoreEvent::CallEnded { with, reason: CallEnd::MissedInCall, .. } if *with == carla.me())
    })
    .await;

    // Bea answers Ana; Carla rings again, in the middle of the call.
    bea.accept_call(call).await.unwrap();
    let second = carla.start_call(bea.me()).await.unwrap();
    wait_for(&mut events_c, "Bea busy again", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Busy, .. } if *group == second)
    })
    .await;
    wait_for(&mut events_b, "Carla's second missed call", |e| {
        matches!(e, CoreEvent::CallEnded { group, with, reason: CallEnd::MissedInCall } if *group == second && *with == carla.me())
    })
    .await;
    // Bea's call with Ana goes on.
    let calls = bea.calls().await.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (calls[0].group, calls[0].phase),
        (call, CallPhase::Connected)
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn calling_someone_declines_a_ring_waiting_from_another_friend() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-waiting-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dirs = all_friends(&base, &["Ana", "Bea", "Carla"]);
    let options = dialling(3);
    let (ana, mut events_a) = Core::start(dirs[0].clone(), options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dirs[1].clone(), options[1].clone()).unwrap();
    let (carla, mut events_c) = Core::start(dirs[2].clone(), options[2].clone()).unwrap();
    for core in [&ana, &bea, &carla] {
        online(core, 2).await;
    }

    // Ana rings Bea, who calls Carla instead: Ana hears Bea cannot talk now.
    let rung = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "Ana's ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == rung),
    )
    .await;
    let call = bea.start_call(carla.me()).await.unwrap();
    wait_for(&mut events_a, "the decline", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Declined, .. } if *group == rung)
    })
    .await;
    let calls = bea.calls().await.unwrap();
    assert_eq!(calls.len(), 1, "Bea has one call");
    assert_eq!((calls[0].group, calls[0].phase), (call, CallPhase::Ringing));
    wait_for(
        &mut events_c,
        "Bea's ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    carla.accept_call(call).await.unwrap();
    wait_for(
        &mut events_b,
        "Carla answers",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == call),
    )
    .await;
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn calling_a_friend_who_rings_here_answers_them() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-back-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let options = dialling(2);
    let (ana, mut events_a) = Core::start(dir_a, options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options[1].clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    // Ana's ring is already here when Bea presses Call: Bea is in Ana's call at once.
    let call = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "the ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    assert_eq!(bea.start_call(ana.me()).await.unwrap(), call);
    wait_for(
        &mut events_a,
        "the answer",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == call),
    )
    .await;
    for core in [&ana, &bea] {
        let calls = core.calls().await.unwrap();
        assert_eq!(calls.len(), 1, "one call each");
        assert_eq!(
            (calls[0].group, calls[0].phase),
            (call, CallPhase::Connected)
        );
    }
    // Nobody missed, cancelled or found the other busy, given time for word of it to come.
    tokio::time::sleep(Duration::from_secs(2)).await;
    for events in [&mut events_a, &mut events_b] {
        while let Ok(e) = events.try_recv() {
            assert!(
                !matches!(e, CoreEvent::CallEnded { .. }),
                "a call ended: {e:?}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_call_back_while_the_ring_is_on_its_way_misses_nothing() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let options = dialling(2);
    let (ana, mut events_a) = Core::start(dir_a, options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options[1].clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    // Bea presses Call a moment after Ana's ring left, before, as or after it comes in.
    for wait_ms in [0, 1, 2, 3, 5, 8, 13, 21] {
        let a = ana.start_call(bea.me()).await.unwrap();
        // That moment.
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        let b = bea.start_call(ana.me()).await.unwrap();
        let kept = a.min(b);
        let together = timeout(Duration::from_secs(40), async {
            loop {
                let (x, y) = (ana.calls().await.unwrap(), bea.calls().await.unwrap());
                let one = |calls: &[fogata_core::CallView]| {
                    calls.len() == 1
                        && calls[0].group == kept
                        && calls[0].phase == CallPhase::Connected
                };
                if one(&x) && one(&y) {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(together.is_ok(), "not in one call after {wait_ms} ms");
        // Time for a wrong end of either call to come in, were one on its way.
        tokio::time::sleep(Duration::from_millis(500)).await;
        // Only this round's two calls.
        for (who, events) in [("Ana", &mut events_a), ("Bea", &mut events_b)] {
            while let Ok(e) = events.try_recv() {
                assert!(
                    !matches!(
                        e,
                        CoreEvent::CallEnded {
                            group,
                            reason: CallEnd::Missed
                                | CallEnd::MissedInCall
                                | CallEnd::Busy
                                | CallEnd::NoAnswer,
                            ..
                        } if group == a || group == b
                    ),
                    "{who} after {wait_ms} ms: {e:?}"
                );
            }
        }
        ana.end_call(kept).await.unwrap();
        waits_alone(&bea, kept).await;
        bea.end_call(kept).await.unwrap();
        // Ana, who left first, has heard Bea go too: calling her before that rings her back
        // into the call just left, which is not what this round is about.
        nobody_to_go_back_to(&ana).await;
        nobody_to_go_back_to(&bea).await;
        // The round is over on both sides: what it left is dropped, so that nothing of it is
        // taken for the next round's.
        for core in [&ana, &bea] {
            let idle = timeout(Duration::from_secs(60), async {
                while !core.calls().await.unwrap().is_empty() {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await;
            assert!(idle.is_ok(), "a call left after {wait_ms} ms");
        }
        while events_a.try_recv().is_ok() {}
        while events_b.try_recv().is_ok() {}
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Waits until `core` has no call it left that it could go back to.
async fn nobody_to_go_back_to(core: &Core) {
    let over = timeout(Duration::from_secs(60), async {
        while !core.call_returns().is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(over.is_ok(), "still a call to go back to");
}

fn copy_folder(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Fails if `events` holds a call that ended for one of `reasons`.
fn none_ended(events: &mut CoreEvents, who: &str, reasons: &[CallEnd]) {
    while let Ok(e) = events.try_recv() {
        if let CoreEvent::CallEnded { reason, .. } = &e {
            assert!(!reasons.contains(reason), "{who}: {e:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ring_reaches_every_computer_of_an_identity_open_twice() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-twice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dirs = all_friends(&base, &["Ana", "Bea", "Carla"]);
    // Bea's data folder, copied to another computer and open on both.
    let copy = base.join("bea-copy");
    copy_folder(&dirs[1], &copy);
    let options = dialling(4);
    let (ana, mut events_a) = Core::start(dirs[0].clone(), options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dirs[1].clone(), options[1].clone()).unwrap();
    let (bea2, mut events_b2) = Core::start(copy, options[2].clone()).unwrap();
    let (carla, mut events_c) = Core::start(dirs[2].clone(), options[3].clone()).unwrap();
    assert_eq!(bea.me(), bea2.me());
    for core in [&ana, &carla, &bea, &bea2] {
        online(core, 2).await;
    }
    // Each of Bea's computers is known to Ana and Carla soon after it knows them, which
    // nothing here can see: give their side of the second greeting time to end.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let ring = |mut events: CoreEvents, call| async move {
        wait_for(
            &mut events,
            "the ring",
            |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
        )
        .await;
        events
    };

    // Ana calls Bea: both of Bea's computers ring; the first to answer takes the call and
    // the other stops ringing, with nothing missed.
    let call = ana.start_call(bea.me()).await.unwrap();
    events_b = ring(events_b, call).await;
    events_b2 = ring(events_b2, call).await;
    bea.accept_call(call).await.unwrap();
    wait_for(
        &mut events_a,
        "the answer",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == call),
    )
    .await;
    wait_for(&mut events_b2, "taken elsewhere", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Elsewhere, .. } if *group == call)
    })
    .await;
    assert!(bea2.calls().await.unwrap().is_empty());

    // Carla calls Bea in the middle of it: the computer in the call is busy, the other one
    // rings and answers; Carla never hears that Bea is busy.
    let second = carla.start_call(bea.me()).await.unwrap();
    events_b2 = ring(events_b2, second).await;
    wait_for(&mut events_b, "Carla's call missed in the call", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::MissedInCall, .. } if *group == second)
    })
    .await;
    bea2.accept_call(second).await.unwrap();
    wait_for(
        &mut events_c,
        "Bea's other computer answers",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == second),
    )
    .await;
    none_ended(&mut events_c, "Carla", &[CallEnd::Busy]);
    carla.end_call(second).await.unwrap();
    waits_alone(&bea2, second).await;
    bea2.end_call(second).await.unwrap();

    // Ana hangs up: the computer in the call waits for her; the other has nothing to end.
    ana.end_call(call).await.unwrap();
    waits_alone(&bea, call).await;
    bea.end_call(call).await.unwrap();

    // Ana rings and stops: both computers missed it.
    let third = ana.start_call(bea.me()).await.unwrap();
    events_b = ring(events_b, third).await;
    events_b2 = ring(events_b2, third).await;
    ana.end_call(third).await.unwrap();
    for events in [&mut events_b, &mut events_b2] {
        wait_for(events, "the missed call", |e| {
            matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Missed, .. } if *group == third)
        })
        .await;
    }

    // Ana rings; one computer declines: Ana hears it, the other one stops ringing.
    let fourth = ana.start_call(bea.me()).await.unwrap();
    events_b = ring(events_b, fourth).await;
    events_b2 = ring(events_b2, fourth).await;
    bea2.decline_call(fourth).await.unwrap();
    wait_for(&mut events_a, "the decline", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Declined, .. } if *group == fourth)
    })
    .await;
    wait_for(&mut events_b, "declined elsewhere", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Elsewhere, .. } if *group == fourth)
    })
    .await;
    // Neither shows a missed call, given time for one to come.
    tokio::time::sleep(Duration::from_secs(1)).await;
    for (who, events) in [("Bea", &mut events_b), ("Bea's copy", &mut events_b2)] {
        none_ended(events, who, &[CallEnd::Missed]);
    }
    for core in [&ana, &bea, &bea2, &carla] {
        assert!(core.calls().await.unwrap().is_empty());
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The calls noted in `core`'s chat with `friend`: whether this device started each, who
/// did when neither of the two, and how long each lasted.
async fn noted(core: &Core, friend: DevicePublicKey) -> Vec<(bool, Option<String>, u64)> {
    core.direct_chat(friend, 100)
        .await
        .unwrap()
        .items
        .iter()
        .filter_map(|i| match &i.item.body {
            Body::Call { lasted_ms, starter } => Some((i.item.mine, starter.clone(), *lasted_ms)),
            _ => None,
        })
        .collect()
}

/// Waits until `core` has noted `count` calls with `friend`.
async fn noted_soon(
    core: &Core,
    friend: DevicePublicKey,
    count: usize,
    what: &str,
) -> Vec<(bool, Option<String>, u64)> {
    let found = timeout(Duration::from_secs(60), async {
        loop {
            let found = noted(core, friend).await;
            if found.len() >= count {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    found.unwrap_or_else(|_| panic!("{what}"))
}

/// Waits until the friends `core` can go back into a call with are `want`.
async fn returns(core: &Core, mut want: Vec<DevicePublicKey>, what: &str) {
    want.sort();
    let found = timeout(Duration::from_secs(60), async {
        while core.call_returns() != want {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "{what}: {:?}", core.call_returns());
}

/// Waits until `core`'s room of `call` has `count` people in it.
async fn in_room(core: &Core, call: fogata_crypto::GroupId, count: usize, what: &str) {
    let met = timeout(Duration::from_secs(60), async {
        loop {
            let room = core.room(call, fogata_core::FIRST_ROOM).await.unwrap();
            if room.is_some_and(|r| r.participants.len() == count) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(met.is_ok(), "{what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_one_waits_a_minute_for_the_other_to_come_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-alone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let options = dialling(2);
    let (ana, mut events_a) = Core::start(dir_a.clone(), options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options[1].clone()).unwrap();
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
    in_room(&ana, call, 2, "they did not meet").await;
    // Each has heard that the other takes calls of more than two.
    for core in [&ana, &bea] {
        let told = timeout(Duration::from_secs(60), async {
            while core.calls().await.unwrap()[0].add_blocked.is_some() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(told.is_ok(), "not told");
    }

    // Bea hangs up: Ana stays, waiting for her, and Bea can go back to her.
    bea.end_call(call).await.unwrap();
    assert_eq!(bea.call_returns(), vec![ana.me()]);
    in_room(&ana, call, 1, "Bea did not leave").await;
    let waiting = timeout(Duration::from_secs(60), async {
        loop {
            let view = ana.calls().await.unwrap();
            if view
                .first()
                .is_some_and(|c| c.alone_since_ms.is_some() && !c.away.is_empty())
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(waiting.is_ok(), "Ana is not waiting for Bea");
    let view = ana.calls().await.unwrap();
    assert_eq!(view.len(), 1, "Ana's call ended");
    assert_eq!(view[0].phase, CallPhase::Connected);
    assert!(view[0].alone_since_ms.is_some(), "Ana is not waiting");
    assert_eq!(view[0].away.len(), 1);
    assert_eq!(view[0].away[0].device, bea.me());
    none_ended(&mut events_a, "Ana", &[CallEnd::HungUp, CallEnd::Alone]);

    // Bea calls Ana again a moment later: she is back in the same call.
    assert_eq!(bea.start_call(ana.me()).await.unwrap(), call);
    wait_for(
        &mut events_b,
        "back in the call",
        |e| matches!(e, CoreEvent::CallAccepted { group } if *group == call),
    )
    .await;
    in_room(&ana, call, 2, "Bea did not come back").await;
    let back = timeout(Duration::from_secs(60), async {
        loop {
            let view = ana.calls().await.unwrap();
            if view
                .first()
                .is_some_and(|c| c.alone_since_ms.is_none() && c.away.is_empty())
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(back.is_ok(), "{:?}", ana.calls().await.unwrap());
    assert!(bea.call_returns().is_empty());
    // Nothing is noted while the call goes on.
    assert!(noted(&ana, bea.me()).await.is_empty());
    assert!(noted(&bea, ana.me()).await.is_empty());
    // The call goes on a little longer: it lasts a few seconds in all.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Ana hangs up this time, and Bea waits; nobody comes back, so the call ends after a
    // minute. Meanwhile Bea keeps telling Ana she is still in it, longer than Ana would
    // count her in it without a word.
    ana.end_call(call).await.unwrap();
    let left = std::time::Instant::now();
    assert_eq!(ana.call_returns(), vec![bea.me()]);
    // Longer than Ana would count Bea in the call without a word from her.
    tokio::time::sleep(Duration::from_secs(50)).await;
    assert_eq!(
        ana.call_returns(),
        vec![bea.me()],
        "Bea's word did not come"
    );
    let ended = timeout(
        Duration::from_millis(fogata_core::ALONE_MS + 15_000),
        async {
            while let Some(e) = events_b.recv().await {
                if let CoreEvent::CallEnded { group, reason, .. } = e
                    && group == call
                {
                    return reason;
                }
            }
            CallEnd::Ended
        },
    )
    .await;
    assert_eq!(ended, Ok(CallEnd::Alone));
    assert!(left.elapsed() >= Duration::from_millis(fogata_core::ALONE_MS - 1_000));
    assert!(bea.calls().await.unwrap().is_empty());
    // Bea's call ended: Ana hears it, and the call shows as Call again.
    returns(&ana, vec![], "Ana still sees Bea in the call").await;

    // Both note the call in their chat: Ana started it, and it lasted until Ana left, the
    // minute Bea waited aside; the same on both sides.
    let ana_saw = noted_soon(&ana, bea.me(), 1, "Ana noted nothing").await;
    let bea_saw = noted_soon(&bea, ana.me(), 1, "Bea noted nothing").await;
    assert_eq!(ana_saw.len(), 1);
    assert_eq!(bea_saw.len(), 1);
    let ((mine_a, starter_a, lasted_a), (mine_b, starter_b, lasted_b)) =
        (ana_saw[0].clone(), bea_saw[0].clone());
    assert!(mine_a && !mine_b && starter_a.is_none() && starter_b.is_none());
    assert!(
        (2_000..30_000).contains(&lasted_a) && lasted_a.abs_diff(lasted_b) < 2_000,
        "{lasted_a} {lasted_b}"
    );
    // Never unread, and never sent: each device noted its own.
    let chats = bea.direct_chats().await.unwrap();
    assert_eq!(chats[0].unread, 0);
    assert_eq!(
        chats[0]
            .last
            .as_ref()
            .map(|(text, mine)| (text.as_str(), *mine)),
        Some(("Ana started a call that lasted a few seconds", false))
    );

    // It stays in the chat after a restart.
    ana.shutdown().await.unwrap();
    drop(ana);
    drop(events_a);
    let ana = start_again(&dir_a, options[0].clone()).await;
    assert_eq!(noted(&ana.0, bea.me()).await, ana_saw);
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
async fn anyone_in_a_call_rings_their_own_friends_into_it() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-group-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    // Ana knows Bea and Carla; Bea knows Dani; Carla and Dani know nobody else.
    let dirs = some_friends(
        &base,
        &["Ana", "Bea", "Carla", "Dani"],
        &[(0, 1), (0, 2), (1, 3)],
    );
    let options = dialling(4);
    let (ana, _events_a) = Core::start(dirs[0].clone(), options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dirs[1].clone(), options[1].clone()).unwrap();
    let (carla, mut events_c) = Core::start(dirs[2].clone(), options[2].clone()).unwrap();
    let (dani, mut events_d) = Core::start(dirs[3].clone(), options[3].clone()).unwrap();
    online(&ana, 2).await;
    online(&bea, 2).await;
    online(&carla, 1).await;
    online(&dani, 1).await;

    let call = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "the ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    bea.accept_call(call).await.unwrap();
    in_room(&ana, call, 2, "Ana and Bea did not meet").await;
    // Both newer: each may ring friends in.
    let can_add = |core: &Core| {
        let core = core.clone();
        async move {
            let all = timeout(Duration::from_secs(60), async {
                loop {
                    let calls = core.calls().await.unwrap();
                    if calls.first().is_some_and(|c| c.add_blocked.is_none()) {
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await;
            assert!(
                all.is_ok(),
                "{:?}",
                core.calls().await.unwrap()[0].add_blocked
            );
        }
    };
    can_add(&ana).await;
    can_add(&bea).await;

    // Ana rings Carla in; Bea rings Dani in. Carla and Dani are no friends of each other.
    ana.add_to_call(call, carla.me()).await.unwrap();
    assert_eq!(ana.calls().await.unwrap()[0].adding.len(), 1);
    wait_for(
        &mut events_c,
        "Carla rung in",
        |e| matches!(e, CoreEvent::CallIncoming { group, from } if *group == call && *from == ana.me()),
    )
    .await;
    carla.accept_call(call).await.unwrap();
    bea.add_to_call(call, dani.me()).await.unwrap();
    wait_for(
        &mut events_d,
        "Dani rung in",
        |e| matches!(e, CoreEvent::CallIncoming { group, from } if *group == call && *from == bea.me()),
    )
    .await;
    dani.accept_call(call).await.unwrap();
    for (core, who) in [
        (&ana, "Ana"),
        (&bea, "Bea"),
        (&carla, "Carla"),
        (&dani, "Dani"),
    ] {
        in_room(core, call, 4, &format!("{who} does not see all four")).await;
    }
    // Carla sees Dani by the name she gave herself, though they are no friends.
    let room = carla
        .room(call, fogata_core::FIRST_ROOM)
        .await
        .unwrap()
        .unwrap();
    assert!(
        room.participants.iter().any(|(p, _)| p.name == "Dani"),
        "{:?}",
        room.participants
            .iter()
            .map(|(p, _)| &p.name)
            .collect::<Vec<_>>()
    );
    assert!(ana.calls().await.unwrap()[0].adding.is_empty());
    // Dani, rung in by Bea, is made able to ring friends in too.
    can_add(&dani).await;

    // Ana leaves: the three others go on.
    ana.end_call(call).await.unwrap();
    for core in [&bea, &carla, &dani] {
        in_room(core, call, 3, "the others did not go on").await;
    }
    // Nobody's call ends, given time for an end to come.
    tokio::time::sleep(Duration::from_secs(1)).await;
    for (who, events) in [
        ("Bea", &mut events_b),
        ("Carla", &mut events_c),
        ("Dani", &mut events_d),
    ] {
        none_ended(
            events,
            who,
            &[CallEnd::HungUp, CallEnd::Alone, CallEnd::Ended],
        );
    }

    // Ana can go back to her friends still in it, Bea and Carla; Dani is no friend of hers.
    returns(&ana, vec![bea.me(), carla.me()], "Ana's way back").await;
    assert!(noted(&ana, bea.me()).await.is_empty());
    // Bea leaves: Ana hears it, and Bea can go back to Dani, her friend still in it.
    bea.end_call(call).await.unwrap();
    returns(&ana, vec![carla.me()], "Ana still sees Bea in the call").await;
    returns(&bea, vec![dani.me()], "Bea's way back").await;
    // Carla leaves Dani, no friend of hers, alone: the call is over for her, and for Ana,
    // whose last friend in it left.
    in_room(&carla, call, 2, "Bea did not leave").await;
    carla.end_call(call).await.unwrap();
    returns(&ana, vec![], "Ana still sees Carla in the call").await;
    waits_alone(&dani, call).await;
    // Dani is the last: when she hangs up, Bea hears it.
    dani.end_call(call).await.unwrap();
    returns(&bea, vec![], "Bea still sees Dani in the call").await;

    // Each notes it with the one who started it, Ana, and with who rang whom in: Bea and
    // Dani, no friend of Ana's, note it in their chat, under Ana's name.
    let mut lasted = Vec::new();
    for (core, friend, mine, starter, who) in [
        (&ana, bea.me(), true, None, "Ana with Bea"),
        (&ana, carla.me(), true, None, "Ana with Carla"),
        (&bea, ana.me(), false, None, "Bea with Ana"),
        (&carla, ana.me(), false, None, "Carla with Ana"),
        (&bea, dani.me(), false, Some("Ana"), "Bea with Dani"),
        (&dani, bea.me(), false, Some("Ana"), "Dani with Bea"),
    ] {
        let found = noted_soon(core, friend, 1, who).await;
        assert_eq!(found.len(), 1, "{who}");
        assert_eq!(found[0].0, mine, "{who}");
        assert_eq!(found[0].1.as_deref(), starter, "{who}");
        lasted.push(found[0].2);
    }
    // Until Carla left Dani alone, as each knows it.
    let (least, most) = (lasted.iter().min().unwrap(), lasted.iter().max().unwrap());
    assert!(most - least < 3_000 && *most < 60_000, "{lasted:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_friend_rung_into_a_call_can_say_no_or_be_stopped() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-call-add-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dirs = some_friends(&base, &["Ana", "Bea", "Carla"], &[(0, 1), (0, 2)]);
    let options = dialling(3);
    let (ana, mut events_a) = Core::start(dirs[0].clone(), options[0].clone()).unwrap();
    let (bea, mut events_b) = Core::start(dirs[1].clone(), options[1].clone()).unwrap();
    let (carla, mut events_c) = Core::start(dirs[2].clone(), options[2].clone()).unwrap();
    online(&ana, 2).await;
    online(&bea, 1).await;
    online(&carla, 1).await;
    let call = ana.start_call(bea.me()).await.unwrap();
    wait_for(
        &mut events_b,
        "the ring",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    bea.accept_call(call).await.unwrap();
    in_room(&ana, call, 2, "Ana and Bea did not meet").await;
    // Bea has no friend to ring in, and Ana's other friends are no friends of hers.
    assert!(bea.add_to_call(call, carla.me()).await.is_err());

    // Carla says no.
    let ready = timeout(Duration::from_secs(60), async {
        while ana.calls().await.unwrap()[0].add_blocked.is_some() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(ready.is_ok());
    ana.add_to_call(call, carla.me()).await.unwrap();
    wait_for(
        &mut events_c,
        "Carla rung in",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    carla.decline_call(call).await.unwrap();
    wait_for(&mut events_a, "Carla says no", |e| {
        matches!(e, CoreEvent::CallAddEnded { group, friend, reason: CallEnd::Declined } if *group == call && *friend == carla.me())
    })
    .await;
    assert!(ana.calls().await.unwrap()[0].adding.is_empty());

    // Ana rings her in again and stops before she answers: Carla missed it.
    ana.add_to_call(call, carla.me()).await.unwrap();
    wait_for(
        &mut events_c,
        "Carla rung in again",
        |e| matches!(e, CoreEvent::CallIncoming { group, .. } if *group == call),
    )
    .await;
    ana.cancel_add(call, carla.me()).await.unwrap();
    wait_for(&mut events_c, "Carla missed it", |e| {
        matches!(e, CoreEvent::CallEnded { group, reason: CallEnd::Missed, .. } if *group == call)
    })
    .await;
    // Ana and Bea talk on.
    in_room(&bea, call, 2, "the call did not go on").await;
    assert_eq!(ana.calls().await.unwrap()[0].phase, CallPhase::Connected);
    let _ = std::fs::remove_dir_all(&base);
}
