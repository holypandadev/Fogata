//! What a device keeps of other people's pictures goes with them: the animations and profiles
//! of devices that are no longer friends nor members of any of its groups, and the banner and
//! picture of a group it left, at the start and when they go while it runs.

use std::time::Duration;

use fogata_core::{Animated, Core, NetOptions, Store};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_friends::{Animation, Avatar, Profile};
use fogata_state::GroupLog;
use tokio::time::timeout;

/// A GIF of one pixel.
const ONE_PIXEL_GIF: &[u8] =
    b"GIF89a\x01\x00\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nstill";

fn options() -> NetOptions {
    NetOptions {
        listen_port: 0,
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// An animation and a member profile of `who`, as met in a group.
fn met(who: &DeviceIdentity, name: &str) -> (Animation, Profile) {
    let animation = Animation::sign(who, Animated::Photo, PNG, ONE_PIXEL_GIF.to_vec(), 5).unwrap();
    let profile = Profile::sign(who, name, Avatar::Photo(PNG.to_vec()), 5).unwrap();
    (animation, profile)
}

fn devices_animated(store: &Store) -> Vec<DevicePublicKey> {
    let mut out: Vec<_> = store.animations().iter().map(|a| a.device).collect();
    out.sort();
    out
}

fn devices_with_profiles(store: &Store) -> Vec<DevicePublicKey> {
    let mut out: Vec<_> = store.member_profiles().iter().map(|p| p.device).collect();
    out.sort();
    out
}

/// The group's files in the data folder: which of log, banner and picture there are.
fn group_files(dir: &std::path::Path, group: GroupId) -> Vec<String> {
    let id = hex::encode(group.as_bytes());
    ["log", "banner", "picture"]
        .into_iter()
        .filter(|kind| dir.join("logs").join(format!("{id}.{kind}")).exists())
        .map(String::from)
        .collect()
}

/// Cid's group, with Ana in it and a banner and a picture Cid chose.
fn cids_group(cid: &DeviceIdentity, ana: DevicePublicKey) -> GroupLog {
    let mut log = GroupLog::create(cid, "Cid's", now()).unwrap();
    log.add(cid, ana, "Ana", now()).unwrap();
    log.set_banner(cid, PNG.to_vec(), now()).unwrap();
    log.set_picture(cid, PNG.to_vec(), now()).unwrap();
    log
}

async fn start(dir: &std::path::Path) -> Core {
    // The folder stays locked a moment after a core before it stops.
    for _ in 0..50 {
        if let Ok((core, _events)) = Core::start(dir.to_path_buf(), options()) {
            return core;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the folder stayed locked");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_start_forgets_the_pictures_of_people_and_groups_gone() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-gone-start-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("ana");
    Core::create_profile(&dir, "Ana", None).unwrap();
    let store = Store::open(dir.clone()).unwrap();
    let (ana, _) = store.identity().unwrap();
    let (bea, cid) = (
        DeviceIdentity::generate().unwrap(),
        DeviceIdentity::generate().unwrap(),
    );

    // Ana's group with Bea; Cid's group, which removed Ana.
    let mut ours = GroupLog::create(&ana, "ours", now()).unwrap();
    ours.add(&ana, bea.public_key(), "Bea", now()).unwrap();
    store.save_group_log(&ours, "ours").unwrap();
    let mut theirs = cids_group(&cid, ana.public_key());
    theirs.remove(&cid, ana.public_key(), now()).unwrap();
    store.save_group_log(&theirs, "Cid's").unwrap();
    assert_eq!(
        group_files(&dir, theirs.group_id()),
        ["log", "banner", "picture"]
    );
    let (bea_moves, bea_profile) = met(&bea, "Bea");
    let (cid_moves, cid_profile) = met(&cid, "Cid");
    store.save_animations([&bea_moves, &cid_moves]).unwrap();
    store
        .save_member_profiles([&bea_profile, &cid_profile])
        .unwrap();
    drop(store);

    let core = start(&dir).await;
    core.shutdown().await.unwrap();
    drop(core);

    let store = Store::open(dir.clone()).unwrap();
    assert_eq!(devices_animated(&store), [bea.public_key()]);
    assert_eq!(devices_with_profiles(&store), [bea.public_key()]);
    // The group left keeps its log (to tell it if Ana is added again), nothing more.
    assert_eq!(group_files(&dir, theirs.group_id()), ["log"]);
    assert_eq!(group_files(&dir, ours.group_id()), ["log"]);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn pictures_go_with_a_member_removed_and_a_group_left() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-gone-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("ana");
    Core::create_profile(&dir, "Ana", None).unwrap();
    let store = Store::open(dir.clone()).unwrap();
    let (ana, _) = store.identity().unwrap();
    let (bea, cid) = (
        DeviceIdentity::generate().unwrap(),
        DeviceIdentity::generate().unwrap(),
    );
    let mut ours = GroupLog::create(&ana, "ours", now()).unwrap();
    ours.add(&ana, bea.public_key(), "Bea", now()).unwrap();
    store.save_group_log(&ours, "ours").unwrap();
    let theirs = cids_group(&cid, ana.public_key());
    store.save_group_log(&theirs, "Cid's").unwrap();
    let (bea_moves, bea_profile) = met(&bea, "Bea");
    let (cid_moves, cid_profile) = met(&cid, "Cid");
    store.save_animations([&bea_moves, &cid_moves]).unwrap();
    store
        .save_member_profiles([&bea_profile, &cid_profile])
        .unwrap();
    drop(store);

    let core = start(&dir).await;
    let store = Store::open(dir.clone()).unwrap();
    // Everyone is a member yet: all kept, given time to be forgotten.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(devices_animated(&store).len(), 2);
    assert_eq!(
        group_files(&dir, theirs.group_id()),
        ["log", "banner", "picture"]
    );

    let until = |what: &'static str, check: Box<dyn Fn() -> bool + Send>| async move {
        let held = timeout(Duration::from_secs(20), async {
            while !check() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(held.is_ok(), "timed out waiting for {what}");
    };
    core.remove_from_group(ours.group_id(), bea.public_key())
        .await
        .unwrap();
    let (s, b) = (Store::open(dir.clone()).unwrap(), bea.public_key());
    until(
        "Bea's pictures gone",
        Box::new(move || {
            !devices_animated(&s).contains(&b) && !devices_with_profiles(&s).contains(&b)
        }),
    )
    .await;
    assert_eq!(devices_animated(&store), [cid.public_key()]);

    core.leave_group(theirs.group_id()).await.unwrap();
    let (s, d, g) = (
        Store::open(dir.clone()).unwrap(),
        dir.clone(),
        theirs.group_id(),
    );
    until(
        "Cid's pictures and the group's gone",
        Box::new(move || {
            devices_animated(&s).is_empty()
                && devices_with_profiles(&s).is_empty()
                && group_files(&d, g) == ["log"]
        }),
    )
    .await;
    core.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}
