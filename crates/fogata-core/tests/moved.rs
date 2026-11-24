//! A group that moved to a new log, as a device starts: the new log is run and listed in the
//! group's place, and a new log this device left leaves the old group stopped too.

use std::time::{SystemTime, UNIX_EPOCH};

use fogata_core::{Core, NetOptions, Store};
use fogata_crypto::DeviceIdentity;
use fogata_state::{Body, GroupLog, Level};

/// One test at a time: each starts a device that listens on this computer's network.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn options() -> NetOptions {
    NetOptions {
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    }
}

/// A device that moved a group and stopped before the new log was listed (it was kept on the
/// disk first) lists it at its next start, under the group's name, in the old one's place.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_log_kept_before_a_stop_is_listed_at_the_next_start() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Ana", None).unwrap();
    let (old, new) = {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let t = now_ms() - 10_000;
        let mut old = GroupLog::create(&me, "Campfire", t).unwrap();
        old.raise_level(&me, Level::Five, t + 1).unwrap();
        let new = old.new_log_for_move(&me, t + 2).unwrap();
        store.save_group_log(&old, "Our fire").unwrap();
        store.keep_new_log(&new).unwrap();
        // The move written and saved, and then the device stopped.
        old.move_to(&me, &new, t + 3).unwrap();
        store.save_group_log(&old, "Our fire").unwrap();
        (old.group_id(), new.group_id())
    };
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    let groups = core.groups().await.unwrap();
    let listed: Vec<_> = groups
        .iter()
        .map(|g| (g.id, g.local_name.as_str()))
        .collect();
    assert_eq!(listed, [(new, "Our fire")]);
    // What showed as the old group shows as the new one.
    assert_eq!(core.moved_to(old).await.unwrap(), Some(new));
    assert_eq!(core.moved_to(new).await.unwrap(), None);
    let stored = core.store().groups().unwrap();
    let was = (stored.iter())
        .find(|g| g.group_id == hex::encode(old.as_bytes()))
        .unwrap();
    assert_eq!(
        was.moved.as_ref().map(|m| m.to.clone()),
        Some(hex::encode(new.as_bytes()))
    );
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A device that left the new log long ago does not run the old group either: it would be
/// listed again, and follow the move again into the group it left.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_log_left_leaves_the_old_group_stopped() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-left-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Bea", None).unwrap();
    {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        // Long ago: the leave is past the while a leave keeps a group running.
        let t = 1_000;
        let mut old = GroupLog::create(&ana, "Campfire", t).unwrap();
        old.add(&ana, me.public_key(), "Bea", t + 1).unwrap();
        old.raise_level(&ana, Level::Five, t + 2).unwrap();
        let mut new = old.new_log_for_move(&ana, t + 3).unwrap();
        old.move_to(&ana, &new, t + 4).unwrap();
        new.append(&me, Body::Leave, t + 5).unwrap();
        store.save_group_log(&old, "Our fire").unwrap();
        store.save_group_log(&new, "Campfire").unwrap();
        // Moved here not long ago: the old group would still run for a while.
        store.record_move(&old.group_id(), &new, now_ms()).unwrap();
    }
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    assert!(core.groups().await.unwrap().is_empty());
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A device that left the new log and was added to it again since, which waits for the
/// person's yes, runs neither the old group nor the new log: following the move would run
/// the invitation without asking.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_log_offered_again_leaves_the_old_group_stopped() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-offered-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Bea", None).unwrap();
    let new_id = {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let t = 1_000;
        let mut old = GroupLog::create(&ana, "Campfire", t).unwrap();
        old.add(&ana, me.public_key(), "Bea", t + 1).unwrap();
        old.raise_level(&ana, Level::Five, t + 2).unwrap();
        let mut new = old.new_log_for_move(&ana, t + 3).unwrap();
        old.move_to(&ana, &new, t + 4).unwrap();
        new.append(&me, Body::Leave, t + 5).unwrap();
        store.save_group_log(&old, "Our fire").unwrap();
        store.save_group_log(&new, "Campfire").unwrap();
        store.record_move(&old.group_id(), &new, now_ms()).unwrap();
        // Ana adds Bea again: an invitation, waiting.
        new.add(&ana, me.public_key(), "Bea", t + 6).unwrap();
        let offered = store.save_offered_group(&new, &ana.public_key()).unwrap();
        assert!(matches!(offered, fogata_core::Offered::Pending(_)));
        new.group_id()
    };
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    assert!(core.groups().await.unwrap().is_empty());
    let stored = core.store().groups().unwrap();
    let new = (stored.iter())
        .find(|g| g.group_id == hex::encode(new_id.as_bytes()))
        .unwrap();
    assert!(new.pending);
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A member that got the move and was offered the new log before following it, and
/// declined it: the old group stays stopped, or following the move would run the group
/// declined.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_log_declined_leaves_the_old_group_stopped() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-declined-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Bea", None).unwrap();
    let (old, new) = {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let t = now_ms() - 10_000;
        let mut old = GroupLog::create(&ana, "Campfire", t).unwrap();
        old.add(&ana, me.public_key(), "Bea", t + 1).unwrap();
        old.raise_level(&ana, Level::Five, t + 2).unwrap();
        let new = old.new_log_for_move(&ana, t + 3).unwrap();
        old.move_to(&ana, &new, t + 4).unwrap();
        // The move is in the old log here, not followed yet.
        store.save_group_log(&old, "Our fire").unwrap();
        let offered = store.save_offered_group(&new, &ana.public_key()).unwrap();
        assert!(matches!(offered, fogata_core::Offered::Pending(_)));
        let id = hex::encode(new.group_id().as_bytes());
        store.decline_group(&id, &me, t + 5).unwrap();
        (old.group_id(), new.group_id())
    };
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    assert!(core.groups().await.unwrap().is_empty());
    // Left with the new log: noted as moved, so it stops being kept after a while.
    let stored = core.store().groups().unwrap();
    let was = (stored.iter())
        .find(|g| g.group_id == hex::encode(old.as_bytes()))
        .unwrap();
    assert_eq!(
        was.moved.as_ref().map(|m| m.to.clone()),
        Some(hex::encode(new.as_bytes()))
    );
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A new log listed here whose file cannot be read at a start leaves the old group stopped
/// too: following the move would run the new log and write it over the file.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_new_log_that_cannot_be_read_leaves_the_old_group_stopped() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-unread-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Bea", None).unwrap();
    {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let t = now_ms() - 10_000;
        let mut old = GroupLog::create(&ana, "Campfire", t).unwrap();
        old.add(&ana, me.public_key(), "Bea", t + 1).unwrap();
        old.raise_level(&ana, Level::Five, t + 2).unwrap();
        let new = old.new_log_for_move(&ana, t + 3).unwrap();
        old.move_to(&ana, &new, t + 4).unwrap();
        store.save_group_log(&old, "Our fire").unwrap();
        store.record_move(&old.group_id(), &new, now_ms()).unwrap();
        // Out of reach this time: a link to itself where the file was.
        let path = dir
            .join("logs")
            .join(format!("{}.log", hex::encode(new.group_id().as_bytes())));
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&path, &path).unwrap();
    }
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    assert!(core.groups().await.unwrap().is_empty());
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A group that moved twice, whose last log is an invitation here: the one before it stays
/// stopped, and so does the first, or following both moves would run the invitation.
#[tokio::test(flavor = "multi_thread")]
async fn a_line_of_moves_stops_back_to_its_first_log() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-line-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Bea", None).unwrap();
    {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let t = now_ms() - 10_000;
        let mut first = GroupLog::create(&ana, "Campfire", t).unwrap();
        first.add(&ana, me.public_key(), "Bea", t + 1).unwrap();
        first.raise_level(&ana, Level::Five, t + 2).unwrap();
        let mut second = first.new_log_for_move(&ana, t + 3).unwrap();
        first.move_to(&ana, &second, t + 4).unwrap();
        if second.state().log_level < Level::Five {
            second.raise_level(&ana, Level::Five, t + 5).unwrap();
        }
        let third = second.new_log_for_move(&ana, t + 6).unwrap();
        second.move_to(&ana, &third, t + 7).unwrap();
        store.save_group_log(&first, "Our fire").unwrap();
        store
            .record_move(&first.group_id(), &second, now_ms())
            .unwrap();
        store.save_group_log(&second, "Campfire").unwrap();
        store.save_group_log(&third, "Campfire").unwrap();
        let id = hex::encode(third.group_id().as_bytes());
        store
            .set_pending(&id, Some(hex::encode(ana.public_key().as_bytes())))
            .unwrap();
    }
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    assert!(core.groups().await.unwrap().is_empty());
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A group stopped at a start beside an invitation to the log it moved to is run once the
/// invitation is taken: the move is followed in that run, and the new log takes the
/// group's name.
#[tokio::test(flavor = "multi_thread")]
async fn a_new_log_taken_is_followed_in_the_same_run() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-moved-taken-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Bea", None).unwrap();
    let (old_id, new_id) = {
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let ana = DeviceIdentity::generate().unwrap();
        let t = now_ms() - 10_000;
        let mut old = GroupLog::create(&ana, "Campfire", t).unwrap();
        old.add(&ana, me.public_key(), "Bea", t + 1).unwrap();
        old.raise_level(&ana, Level::Five, t + 2).unwrap();
        let new = old.new_log_for_move(&ana, t + 3).unwrap();
        old.move_to(&ana, &new, t + 4).unwrap();
        store.save_group_log(&old, "Our fire").unwrap();
        let offered = store.save_offered_group(&new, &ana.public_key()).unwrap();
        assert!(matches!(offered, fogata_core::Offered::Pending(_)));
        (old.group_id(), new.group_id())
    };
    let (core, _events) = Core::start(dir.clone(), options()).unwrap();
    assert!(core.groups().await.unwrap().is_empty());
    core.accept_group_invitation(new_id).await.unwrap();
    let hex_of = |id: &fogata_crypto::GroupId| hex::encode(id.as_bytes());
    let mut named = None;
    for _ in 0..600 {
        let stored = core.store().groups().unwrap();
        let new = stored
            .iter()
            .find(|g| g.group_id == hex_of(&new_id))
            .unwrap();
        if new.moved_from.is_some() {
            named = Some(new.name.clone());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(named.as_deref(), Some("Our fire"));
    assert_eq!(core.moved_to(old_id).await.unwrap(), Some(new_id));
    let stored = core.store().groups().unwrap();
    let old = stored
        .iter()
        .find(|g| g.group_id == hex_of(&old_id))
        .unwrap();
    assert_eq!(
        old.moved.as_ref().map(|m| m.to.clone()),
        Some(hex_of(&new_id))
    );
    core.shutdown().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
