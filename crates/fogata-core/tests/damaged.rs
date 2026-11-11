//! A data folder damaged by a crash or on the disk: what can be read is kept, damaged files
//! are put aside and never deleted, and the application still starts.

use std::{fs, path::PathBuf};

use fogata_core::{Core, CoreEvent, NetOptions, Store};
use fogata_crypto::DeviceIdentity;
use fogata_friends::FriendBook;
use fogata_state::{Body, GroupLog};

/// One test at a time: one of them starts a device, and all of them open data folders,
/// whose keys go in the same keychain.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// An empty folder for one test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fogata-damaged-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_log_cut_short_keeps_the_entries_before_the_cut() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("log");
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let mut log = GroupLog::create(&me, "Home", 1).unwrap();
    log.append(
        &me,
        Body::Rename {
            name: "Campfire".into(),
        },
        2,
    )
    .unwrap();
    log.append(&me, Body::SetIcon { icon: 4 }, 3).unwrap();
    store.save_log("Home", &log).unwrap();
    let group = store.group("Home").unwrap();
    let file = dir.join("logs").join(format!("{}.log", group.group_id));
    let whole = fs::read(&file).unwrap();

    fs::write(&file, &whole[..whole.len() - 5]).unwrap();
    let read = store.load_log(&group).unwrap();
    assert_eq!(read.len(), 2);
    assert_eq!(read.state().name, "Campfire");
    assert_eq!(read.state().icon, 0);
    let aside = dir
        .join("logs")
        .join(format!("{}.log.damaged", group.group_id));
    assert_eq!(fs::read(&aside).unwrap(), whole[..whole.len() - 5]);

    // Garbage after the entries is damage too; the entries stay.
    let mut longer = whole.clone();
    longer.extend_from_slice(&[0, 0, 0, 3, 1, 2, 3]);
    fs::write(&file, &longer).unwrap();
    assert_eq!(store.load_log(&group).unwrap().len(), 3);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_damaged_entry_in_a_log_loses_only_that_entry() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("log-middle");
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let mut log = GroupLog::create(&me, "Home", 1).unwrap();
    log.append(
        &me,
        Body::Rename {
            name: "Campfire".into(),
        },
        2,
    )
    .unwrap();
    log.append(&me, Body::SetIcon { icon: 4 }, 3).unwrap();
    store.save_log("Home", &log).unwrap();
    let group = store.group("Home").unwrap();
    let file = dir.join("logs").join(format!("{}.log", group.group_id));
    let whole = fs::read(&file).unwrap();
    let length = |at: usize| u32::from_be_bytes(whole[at..at + 4].try_into().unwrap()) as usize;
    let second = 4 + length(0);
    let rename = log
        .entries()
        .find(|e| matches!(e.body, Body::Rename { .. }))
        .unwrap()
        .clone();

    // A byte changed inside the second entry (which then does not read, or reads with a
    // signature that does not hold), then its length: the third entry is kept either way,
    // waiting for the second, which a member hands back later.
    let mut inside = whole.clone();
    inside[second + 4 + length(second) / 2] ^= 0x55;
    let mut length_changed = whole.clone();
    length_changed[second..second + 4].copy_from_slice(&0xfff0_0000u32.to_be_bytes());
    for (damaged, unreadable) in [(inside, false), (length_changed, true)] {
        fs::write(&file, &damaged).unwrap();
        let _ = fs::remove_file(with_suffix(&file, ".damaged"));
        let mut read = store.load_log(&group).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read.state().icon, 0);
        if unreadable {
            assert_eq!(
                fs::read(with_suffix(&file, ".damaged")).unwrap(),
                damaged,
                "a copy is put aside"
            );
        }
        read.insert(rename.clone()).unwrap();
        assert_eq!(read.state().name, "Campfire");
        assert_eq!(read.state().icon, 4);
    }
    fs::remove_dir_all(&dir).unwrap();
}

fn with_suffix(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

#[test]
fn a_damaged_group_list_is_made_again_from_the_logs() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("groups");
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let mine = GroupLog::create(&me, "Home", 1).unwrap();
    store.save_log("Home", &mine).unwrap();
    // A friend's group of the same name, offered and not answered yet.
    let friend = DeviceIdentity::generate().unwrap();
    let mut offered = GroupLog::create(&friend, "Home", 1).unwrap();
    offered.add(&friend, me.public_key(), "Ana", 2).unwrap();
    store
        .save_offered_group(&offered, &friend.public_key())
        .unwrap();
    let damaged = br#"[{"name": "Home", "group_"#;
    fs::write(dir.join("groups.json"), damaged).unwrap();

    let groups = store.groups().unwrap();
    assert_eq!(groups.len(), 2);
    let stored = |log: &GroupLog| {
        let id = hex::encode(log.group_id().as_bytes());
        groups.iter().find(|g| g.group_id == id).unwrap().clone()
    };
    assert!(!stored(&mine).pending);
    // Nothing of this device is in the friend's log: it is asked again, not joined.
    assert!(stored(&offered).pending);
    assert_eq!(
        stored(&offered).invited_by,
        Some(hex::encode(friend.public_key().as_bytes()))
    );
    let mut names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Home", "Home (2)"]);
    assert_eq!(fs::read(dir.join("groups.json.damaged")).unwrap(), damaged);
    // The list made again is the one on the disk now.
    let again = Store::open(dir.clone()).unwrap().groups().unwrap();
    assert_eq!(again.len(), 2);
    assert!(!dir.join("groups.json.damaged.2").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_damaged_friends_file_falls_back_to_its_copy() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("friends");
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let mut book = FriendBook::new(&me, "Ana", 1).unwrap();
    store.save_friends(&book).unwrap();
    book.set_profile(&me, Some("Ana Luz"), None, 2).unwrap();
    store.save_friends(&book).unwrap();
    let file = dir.join("friends.bin");
    fs::write(&file, b"garbage").unwrap();

    // The copy from before the last change takes its place, and the person is told.
    assert!(!store.friends_restored());
    assert_eq!(store.friends().unwrap().unwrap().profile.name, "Ana");
    assert!(store.friends_restored());
    assert_eq!(
        fs::read(dir.join("friends.bin.damaged")).unwrap(),
        b"garbage"
    );
    assert!(FriendBook::decode(&fs::read(&file).unwrap()).is_ok());

    // With the copy damaged too, the device goes on without a profile; nothing is deleted,
    // and the folder says where the two were kept, so the start screen does not look
    // like a new device's.
    assert_eq!(store.friends_kept(), None);
    fs::write(&file, b"garbage 2").unwrap();
    fs::write(dir.join("friends.bin.bak"), b"garbage 3").unwrap();
    assert!(store.friends().unwrap().is_none());
    assert!(!file.exists());
    assert!(!store.has_profile().unwrap());
    assert_eq!(store.friends_kept(), Some(dir.clone()));
    let kept = [
        ("friends.bin.damaged.2", &b"garbage 2"[..]),
        ("friends.bin.damaged.3", &b"garbage 3"[..]),
    ];
    for (name, bytes) in kept {
        assert_eq!(fs::read(dir.join(name)).unwrap(), bytes, "{name}");
    }
    // A profile made then writes over neither.
    drop(store);
    Core::create_profile(&dir, "Ana again", None).unwrap();
    for (name, bytes) in kept {
        assert_eq!(fs::read(dir.join(name)).unwrap(), bytes, "{name}");
    }
    let store = Store::open(dir.clone()).unwrap();
    assert_eq!(store.friends().unwrap().unwrap().profile.name, "Ana again");
    assert_eq!(store.friends_kept(), None);
    fs::remove_dir_all(&dir).unwrap();
}

/// A copy of the friends file left without the file (put back by hand, say) is set aside by
/// the first profile made, never written over.
#[test]
fn a_first_profile_never_writes_over_a_copy_left_alone() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("copy-alone");
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    store
        .save_friends(&FriendBook::new(&me, "Ana", 1).unwrap())
        .unwrap();
    let copy = fs::read(dir.join("friends.bin.bak")).unwrap();
    fs::remove_file(dir.join("friends.bin")).unwrap();
    assert!(!store.has_profile().unwrap());
    assert_eq!(store.friends_kept(), Some(dir.clone()));
    store
        .save_friends(&FriendBook::new(&me, "Leo", 2).unwrap())
        .unwrap();
    assert_eq!(fs::read(dir.join("friends.bin.damaged")).unwrap(), copy);
    assert_eq!(store.friends().unwrap().unwrap().profile.name, "Leo");
    fs::remove_dir_all(&dir).unwrap();
}

/// A link left where a damaged file goes aside is never written through: the copy goes
/// under another name, or (a log's) is not made, and nothing is made where it points.
#[cfg(unix)]
#[test]
fn damaged_files_are_never_put_aside_through_a_link() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("aside-link");
    let outside = folder("aside-link-outside");
    fs::create_dir_all(&outside).unwrap();
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let book = FriendBook::new(&me, "Ana", 1).unwrap();
    store.save_friends(&book).unwrap();
    store.save_friends(&book).unwrap();
    let log = GroupLog::create(&me, "Home", 1).unwrap();
    store.save_log("Home", &log).unwrap();
    let group = store.group("Home").unwrap();
    let log_file = dir.join("logs").join(format!("{}.log", group.group_id));
    let whole = fs::read(&log_file).unwrap();
    let link = |name: &str, at: &std::path::Path| {
        std::os::unix::fs::symlink(outside.join(name), with_suffix(at, ".damaged")).unwrap();
    };
    link("friends", &dir.join("friends.bin"));
    link("groups", &dir.join("groups.json"));
    link("log", &log_file);

    fs::write(dir.join("friends.bin"), b"garbage").unwrap();
    assert_eq!(store.friends().unwrap().unwrap().profile.name, "Ana");
    assert_eq!(
        fs::read(dir.join("friends.bin.damaged.2")).unwrap(),
        b"garbage"
    );
    fs::write(dir.join("groups.json"), b"[{").unwrap();
    assert_eq!(store.groups().unwrap().len(), 1);
    assert_eq!(fs::read(dir.join("groups.json.damaged.2")).unwrap(), b"[{");
    fs::write(&log_file, &whole[..whole.len() - 5]).unwrap();
    assert!(store.load_log(&group).unwrap().is_empty());

    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        0,
        "written through a link"
    );
    fs::remove_dir_all(&dir).unwrap();
    fs::remove_dir_all(&outside).unwrap();
}

#[test]
fn friends_saved_by_a_newer_version_are_left_alone() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("newer");
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let book = FriendBook::new(&me, "Ana", 1).unwrap();
    store.save_friends(&book).unwrap();
    let file = dir.join("friends.bin");
    let mut newer = fs::read(&file).unwrap();
    newer[0] += 1;
    fs::write(&file, &newer).unwrap();
    let error = store.friends().unwrap_err().to_string();
    assert!(error.contains("newer version"), "{error}");
    // Neither put aside nor replaced by the copy.
    assert_eq!(fs::read(&file).unwrap(), newer);
    assert!(!dir.join("friends.bin.damaged").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_application_starts_with_its_files_damaged() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = folder("start");
    Core::create_profile(&dir, "Ana", None).unwrap();
    let store = Store::open(dir.clone()).unwrap();
    let (me, _) = store.identity().unwrap();
    let mut log = GroupLog::create(&me, "Home", 1).unwrap();
    log.append(&me, Body::SetIcon { icon: 4 }, 2).unwrap();
    store.save_log("Home", &log).unwrap();
    let file = dir
        .join("logs")
        .join(format!("{}.log", hex::encode(log.group_id().as_bytes())));
    let whole = fs::read(&file).unwrap();
    fs::write(&file, &whole[..whole.len() - 1]).unwrap();
    for name in ["groups.json", "friends.bin", "friends.bin.bak"] {
        fs::write(dir.join(name), b"\0\0\0").unwrap();
    }
    drop(store);

    let options = NetOptions {
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    };
    let (core, _events) = Core::start(dir.clone(), options).unwrap();
    let groups = core.groups().await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].local_name, "Home");
    assert_eq!(groups[0].icon, 0, "the entry cut short is not there");
    assert!(core.profile().await.unwrap().is_none());
    core.shutdown().await.unwrap();
    fs::remove_dir_all(&dir).unwrap();
}

/// Files refused by sealing are never kept without a word: a start says where they are.
#[tokio::test(flavor = "multi_thread")]
async fn a_start_says_where_refused_files_are_kept() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = folder("refused-notice");
    Core::create_profile(&dir, "Ana", None).unwrap();
    let options = NetOptions {
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    };
    let told = |events: &mut fogata_core::CoreEvents| {
        let mut told = Vec::new();
        while let Ok(e) = events.try_recv() {
            if let CoreEvent::Notice(text) = e {
                told.push(text);
            }
        }
        told
    };
    let refused = dir.join(fogata_core::store::REFUSED_DIR);
    fs::create_dir_all(&refused).unwrap();
    fs::write(refused.join("recovery.bin"), b"plain").unwrap();
    let (core, mut events) = Core::start(dir.clone(), options).unwrap();
    let told = told(&mut events);
    assert!(
        told.iter().any(|t| t.starts_with("Some of Fogata's files")
            && t.ends_with(&format!("kept in {}", refused.display()))),
        "{told:?}"
    );
    core.shutdown().await.unwrap();
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_lost_identity_is_not_replaced_while_friends_or_groups_remain() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("lost-identity");
    let store = Store::open(dir.clone()).unwrap();
    let (identity, _) = store.identity().unwrap();
    let log = GroupLog::create(&identity, "Home", 1).unwrap();
    store.save_log("Home", &log).unwrap();
    fs::remove_file(dir.join("identity.key")).unwrap();
    let error = store.identity().unwrap_err().to_string();
    assert!(error.contains("is missing"), "{error}");
    assert!(!dir.join("identity.key").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn one_folder_runs_one_device_at_a_time() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("running");
    let store = Store::open(dir.clone()).unwrap();
    let held = store.hold_running().unwrap();
    let error = Store::open(dir.clone())
        .unwrap()
        .hold_running()
        .unwrap_err()
        .to_string();
    assert!(error.contains("already running"), "{error}");
    drop(held);
    store.hold_running().unwrap();
    fs::remove_dir_all(&dir).unwrap();
}

/// A friends file saved by the version before (book version 5) opens with everything in it;
/// saved again in this version, with a former friend that removed this device noted, it
/// keeps the old file as its copy, and a write cut short (a temporary file left half
/// written, or the file itself torn) still leaves a book that opens with nothing lost.
#[test]
fn a_friends_file_of_the_version_before_opens_and_survives_a_write_cut_short() {
    let _one = ONE_AT_A_TIME.blocking_lock();
    let dir = folder("book-v5");
    let store = Store::open(dir.clone()).unwrap();
    store.identity().unwrap();
    let old = include_bytes!("../../fogata-friends/tests/data/book-v5.bin");
    let file = dir.join("friends.bin");
    fs::write(&file, old).unwrap();
    let key = |n: u8| DeviceIdentity::from_secret_bytes(&[n; 32]).public_key();
    let whole = |book: &FriendBook| {
        book.profile.name == "Ana"
            && book.friends[&key(2)].bio.as_ref().unwrap().text == "Bea's bio"
            && book.incoming.contains_key(&key(3))
            && book.outgoing.contains_key(&key(4))
            && book.departing.contains_key(&key(5))
            && book.previous.len() == 1
    };
    let mut book = store.friends().unwrap().unwrap();
    assert!(whole(&book));

    // Bea removes this device; the book is saved in this version.
    assert!(book.removed_by(&key(2), 1_000).unwrap());
    store.save_friends(&book).unwrap();
    let saved = fs::read(&file).unwrap();
    assert_ne!(saved[..], old[..]);
    assert_eq!(fs::read(dir.join("friends.bin.bak")).unwrap(), old);

    // A later write that never got to replace the file leaves its temporary file behind.
    let temp = dir.join(format!("friends.bin.{}.0.tmp", std::process::id()));
    fs::write(&temp, &saved[..saved.len() / 2]).unwrap();
    let book = store.friends().unwrap().unwrap();
    assert!(book.removed_us_lately(&key(2), 1_000));
    assert_eq!(book.incoming.len(), 1);
    let _ = fs::remove_file(&temp);

    // The file itself torn half way: the copy from before, the old one, opens whole.
    fs::write(&file, &saved[..saved.len() / 2]).unwrap();
    let book = store.friends().unwrap().unwrap();
    assert!(whole(&book));
    assert!(store.friends_restored());
    fs::remove_dir_all(&dir).unwrap();
}
