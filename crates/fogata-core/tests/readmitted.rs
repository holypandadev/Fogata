//! A device removed from a group and then added to it again is asked before the group runs
//! again here, even when the disk fails half way through keeping the offer: the folder is
//! made unwritable where each file of the offer goes, and the device starts again after.

#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

use fogata_core::{Core, NetOptions, Store};
use fogata_crypto::{DeviceIdentity, GroupId};
use fogata_state::GroupLog;

/// One test at a time: each starts a device on this computer's network.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn alone() -> NetOptions {
    NetOptions {
        listen_port: free_port(),
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    }
}

fn base(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fogata-readmitted-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn writable(path: &Path, yes: bool) {
    let mode = if yes { 0o700 } else { 0o500 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

/// Where the device stands with group `id` once it starts: whether the group runs, and
/// whether it is an invitation waiting for the person.
async fn on_start(dir: &Path, id: GroupId) -> (bool, bool) {
    let mut started = None;
    for _ in 0..50 {
        match Core::start(dir.to_path_buf(), alone()) {
            Ok(core) => {
                started = Some(core);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let (core, _events) = started.expect("the folder stayed locked");
    let runs = core.groups().await.unwrap().iter().any(|g| g.id == id);
    let asks = core
        .group_invitations()
        .await
        .unwrap()
        .iter()
        .any(|i| i.id == id);
    core.shutdown().await.unwrap();
    (runs, asks)
}

/// The folder where this device was a member of Ana's group "Home" and was removed from it,
/// and the log Ana offers once she added it again.
fn removed(dir: &Path) -> (DeviceIdentity, GroupLog) {
    Core::create_profile(dir, "Dani", None).unwrap();
    let store = Store::open(dir.to_path_buf()).unwrap();
    let me = store.identity().unwrap().0.public_key();
    let ana = DeviceIdentity::generate().unwrap();
    let mut log = GroupLog::create(&ana, "Home", 1).unwrap();
    log.add(&ana, me, "Dani", 2).unwrap();
    store.save_group_log(&log, "Home").unwrap();
    log.remove(&ana, me, 3).unwrap();
    store.save_log("Home", &log).unwrap();
    log.add(&ana, me, "Dani", 4).unwrap();
    assert!(log.state().is_member(&me));
    (ana, log)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_added_again_is_asked_whatever_write_fails() {
    let _one = ONE_AT_A_TIME.lock().await;
    // Unwritable at the list of groups (the folder), or at the group's log (`logs/`).
    for (case, unwritable) in [("list", ""), ("log", "logs")] {
        let dir = base(case);
        let (ana, offered) = removed(&dir);
        let id = offered.group_id();
        let (runs, asks) = on_start(&dir, id).await;
        assert!(
            !runs && !asks,
            "{case}: removed, the group neither runs nor asks"
        );

        let store = Store::open(dir.clone()).unwrap();
        writable(&dir.join(unwritable), false);
        let kept = store.save_offered_group(&offered, &ana.public_key());
        writable(&dir.join(unwritable), true);
        assert!(kept.is_err(), "{case}: the write went through");
        drop(store);
        let (runs, _) = on_start(&dir, id).await;
        assert!(
            !runs,
            "{case}: the group runs again without the person's yes"
        );

        // The friend offers it again, as friends do each time they meet this device: it is
        // an invitation, and still does not run.
        let store = Store::open(dir.clone()).unwrap();
        store
            .save_offered_group(&offered, &ana.public_key())
            .unwrap();
        drop(store);
        let (runs, asks) = on_start(&dir, id).await;
        assert!(!runs && asks, "{case}: offered again, the group asks");
        fs::remove_dir_all(&dir).unwrap();
    }
}
