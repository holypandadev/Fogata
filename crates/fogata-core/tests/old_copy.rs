//! A copy of the command line built from a published release, against this commit's core.
//! A group raised anyway to a level an old copy does not judge at leaves that copy with the
//! group as it was, and told to update by the newest release, which the member that raised
//! the group gives it. Needs that binary: run with
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

/// The old binary's `update` command with `args`, online to Ana where they ask for it, never
/// reading the releases page; what it printed and whether it succeeded.
fn old_update(old: &str, dir: &std::path::Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(old)
        .arg("update")
        .args(args)
        .env("FOGATA_DATA_DIR", dir)
        .env("FOGATA_UPDATE_WEB", "off")
        .env("RUST_LOG", "error")
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

/// A group raised to the current level although a member's Fogata is from before it (its
/// manager chose "raise anyway" after the week's wait): that member's copy, 0.1.0-beta.9
/// here, which judges at level three at most, stops at the entry that raised the group and
/// keeps the group as it was just before, with nothing put aside as damaged. It is still
/// told to update, by the very member that raised the group and is no friend of it: that
/// member holds the newest release (a real one, signed by the release key, whose manifest
/// asks for a build above the old copy's), and the old copy fetches it whole from it and
/// says it must update. Besides the old binary this needs that release's folder as it is
/// on the releases page, with release.json, release.sig and every package the manifest
/// names (`FOGATA_NEWEST_RELEASE=<folder>`).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from 0.1.0-beta.9 and the newest release's folder"]
async fn a_member_left_behind_by_a_raise_keeps_the_group_as_it_was_and_is_told_to_update() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let release = std::env::var("FOGATA_NEWEST_RELEASE").expect("FOGATA_NEWEST_RELEASE");
    let base = std::env::temp_dir().join(format!("fogata-old-raised-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let leo = old_identity(&old, &dir_l);

    // The old copy holds nothing newer than itself to begin with.
    let (_, before) = old_update(&old, &dir_l, &["status"]);
    assert!(!before.contains("must update"), "{before}");

    // A group of the two from before the raise, and the newest release in Ana's folder.
    let store_a = Store::open(dir_a.clone()).unwrap();
    let (ana_id, _) = store_a.identity().unwrap();
    let mut log = GroupLog::create(&ana_id, "g", now()).unwrap();
    log.add(&ana_id, leo, "Leo", now()).unwrap();
    assert!(log.level() < fogata_state::Level::CURRENT);
    let group = log.group_id();
    store_a.save_group_log(&log, "g").unwrap();
    Store::open(dir_l.clone())
        .unwrap()
        .save_group_log(&log, "g")
        .unwrap();
    drop(store_a);
    let mut options = fogata_core::updates::UpdateOptions::compiled_in();
    options.web = None;
    let manifest = fogata_core::updates::publish(
        &dir_a,
        std::path::Path::new(&release),
        &options.keys,
        options.build,
    )
    .unwrap();
    assert!(
        manifest.min_build > 9,
        "the release does not ask the old copy to update"
    );

    let port = free_port();
    let net = || NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    };
    let (ana, _events) = Core::start(dir_a.clone(), net()).unwrap();
    let view = |groups: Vec<fogata_core::GroupView>| {
        groups
            .into_iter()
            .find(|g| g.id == group)
            .expect("the group")
    };
    ana.join_room(group).await.unwrap();
    let chat = old_chat(&old, &dir_l, port);
    eventually("the old copy in the room", 60, || in_room(&ana, group, leo)).await;
    ana.rename_group(group, "before").await.unwrap();
    // The old copy keeps the group from going up: Ana, its manager, is shown who.
    eventually("the old copy holding the group back", 60, || async {
        let seen = view(ana.groups().await.unwrap());
        seen.level_behind.iter().any(|p| p.device == leo)
    })
    .await;
    assert!(!view(ana.groups().await.unwrap()).raise_anyway);
    tokio::time::sleep(Duration::from_secs(10)).await;
    let quit = |mut chat: Child| async move {
        let _ = chat.stdin.as_mut().unwrap().write_all(b"/quit\n");
        let _ = timeout(Duration::from_secs(10), async {
            while chat.try_wait().unwrap().is_none() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        let _ = chat.kill();
        let _ = chat.wait();
    };
    quit(chat).await;
    ana.shutdown().await.unwrap();
    drop(ana);

    // The group started to wait more than a week ago: Ana raises it anyway.
    let store_a = Store::open(dir_a.clone()).unwrap();
    let mut meta = store_a.meta();
    let key = hex::encode(group.as_bytes());
    let since = *meta.level_wait.get(&key).expect("the wait is kept");
    meta.level_wait
        .insert(key, since - fogata_core::LEVEL_WAIT_MS - 1);
    store_a.save_meta(&meta).unwrap();
    drop(store_a);
    let (ana, _events) = Core::start(dir_a.clone(), net()).unwrap();
    ana.start_updates(options).unwrap();
    assert!(view(ana.groups().await.unwrap()).raise_anyway);
    ana.raise_group_level_anyway(group, vec![leo])
        .await
        .unwrap();
    assert!(view(ana.groups().await.unwrap()).level_behind.is_empty());
    ana.rename_group(group, "after").await.unwrap();
    ana.join_room(group).await.unwrap();

    // The old copy comes back to the group that went up without it.
    let chat = old_chat(&old, &dir_l, port);
    eventually("the old copy back with Ana", 60, || {
        in_room(&ana, group, leo)
    })
    .await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    quit(chat).await;

    // And is told to update by Ana, a member and no friend of it: it fetches the newest
    // release whole from her, and says it must update.
    let peer = format!("/ip4/127.0.0.1/udp/{port}/quic-v1");
    let old2 = old.clone();
    let dir2 = dir_l.clone();
    let (fetched, said) = tokio::task::spawn_blocking(move || {
        old_update(
            &old2,
            &dir2,
            &[
                "run",
                "--until-ready",
                "--for",
                "600",
                "--no-dht",
                "--no-mdns",
                "--peer",
                &peer,
            ],
        )
    })
    .await
    .unwrap();
    assert!(
        fetched,
        "the old copy did not get the newest release: {said}"
    );
    let (_, status) = old_update(&old, &dir_l, &["status"]);
    assert!(status.contains("must update"), "{status}");
    ana.shutdown().await.unwrap();
    drop(ana);

    // The old copy's group: as it was just before the raise, the raise and what was signed
    // on it not applied, and nothing put aside as damaged.
    let store_a = Store::open(dir_a).unwrap();
    let mine = store_a.load_log(&store_a.group("g").unwrap()).unwrap();
    assert_eq!(mine.level(), fogata_state::Level::CURRENT);
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert_eq!(kept.state().name, "before");
    assert!(kept.level() < fogata_state::Level::CURRENT);
    assert!(kept.state().is_member(&leo));
    let names: Vec<String> = base
        .join("leo/logs")
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.ends_with(".damaged")),
        "the old copy put a file aside: {names:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}
