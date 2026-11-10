//! A copy of the command line built from the previous release, with this version: what this
//! version writes to a group's log, the previous release takes in whole (nothing left
//! waiting, nothing put aside as damaged), and the two meet in the group's voice room. A
//! change of what a version accepts or writes that splits a group between two releases fails
//! here. Unlike tests/old_copy.rs, whose copies must come from before a given feature, this
//! takes the newest release, whatever it is. Needs that binary: run with
//! `FOGATA_OLD_CLI=<path to fogata> cargo test -p fogata-core --test previous_release -- --ignored`
//! (.github/workflows/weekly.yml builds it from the last release tag; docs/releasing-beta.md).

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

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a command line built from the previous release (FOGATA_OLD_CLI)"]
async fn the_previous_release_takes_in_what_this_version_writes() {
    let old = std::env::var("FOGATA_OLD_CLI").expect("FOGATA_OLD_CLI");
    let base = std::env::temp_dir().join(format!("fogata-previous-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_l) = (base.join("ana"), base.join("leo"));
    std::fs::create_dir_all(&dir_l).unwrap();
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    let leo = old_identity(&old, &dir_l);

    // The group as this version makes it, with the old copy a member from the start.
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
    ana.set_profile(
        None,
        Some(fogata_friends::Avatar::Photo(
            b"\x89PNG\r\n\x1a\nstill".to_vec(),
        )),
    )
    .await
    .unwrap();
    ana.join_room(group).await.unwrap();
    let mut chat = old_chat(&old, &dir_l, port);
    eventually("the previous release in the room", 60, || {
        in_room(&ana, group, leo)
    })
    .await;

    // Entries this version writes while the old copy is there.
    ana.rename_group(group, "h").await.unwrap();
    ana.rename_group(group, "i").await.unwrap();
    // Time for them to reach the old copy, which only its own folder shows once it quits
    // (reading that folder meanwhile would have this version seal it under the old copy's
    // feet).
    tokio::time::sleep(Duration::from_secs(10)).await;
    assert!(
        in_room(&ana, group, leo).await,
        "the previous release left the room"
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

    // The old copy took every entry: the same log as Ana's, nothing waiting, nothing put
    // aside as damaged, and Ana's profile kept.
    let store_a = Store::open(dir_a).unwrap();
    let mine = store_a.load_log(&store_a.group("g").unwrap()).unwrap();
    let store_l = Store::open(dir_l).unwrap();
    let kept = store_l.load_log(&store_l.group("g").unwrap()).unwrap();
    assert_eq!(kept.state().name, "i");
    assert!(
        kept.waiting().is_empty(),
        "the previous release left entries waiting"
    );
    assert_eq!(kept.state().name, mine.state().name);
    let names: Vec<String> = base
        .join("leo/logs")
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.ends_with(".damaged")),
        "the previous release put a file aside: {names:?}"
    );
    assert!(
        store_l
            .member_profiles()
            .into_iter()
            .any(|p| p.device == ana_key),
        "the previous release did not keep Ana's profile"
    );
    let _ = std::fs::remove_dir_all(&base);
}
