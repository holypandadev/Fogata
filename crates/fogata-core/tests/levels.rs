//! A group's level of protection and the members' versions of Fogata, as a manager sees
//! them: who holds a group back from going up, and a friend too old for a group that went
//! up. One device, with a friend whose Fogata was last seen at the previous version.

use fogata_core::{Core, NetOptions, Store};
use fogata_friends::Friend;

/// A data folder of Ana's, friends with Bea, whose Fogata was seen at the version before
/// the current level (capability level 2). Returns the folder and Bea's device.
fn ana_with_an_old_friend(base: &std::path::Path) -> (std::path::PathBuf, Friend) {
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let store_a = Store::open(dir_a.clone()).unwrap();
    let book_b = Store::open(dir_b).unwrap().friends().unwrap().unwrap();
    let bea = Friend {
        profile: book_b.profile.clone(),
        secret: book_b.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    let mut book_a = store_a.friends().unwrap().unwrap();
    book_a.friends.insert(bea.profile.device, bea.clone());
    store_a.save_friends(&book_a).unwrap();
    store_a.note_level(&bea.profile.device, 2).unwrap();
    (dir_a, bea)
}

fn options() -> NetOptions {
    NetOptions {
        listen_port: 0,
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    }
}

/// A manager sees which members keep the group from going up to the protection this
/// version gives; a group that went up has nobody to show. A friend whose Fogata is known
/// to be too old for a group that went up is not added to it, and the manager is told why.
#[tokio::test(flavor = "multi_thread")]
async fn a_manager_sees_who_holds_the_group_back_and_cannot_add_a_friend_too_old() {
    let base = std::env::temp_dir().join(format!("fogata-levels-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir, bea) = ana_with_an_old_friend(&base);
    let bea = bea.profile.device;
    let (ana, _events) = Core::start(dir, options()).unwrap();

    let held = ana.create_group("crew", 0, &[bea]).await.unwrap();
    let alone = ana.create_group("solo", 0, &[]).await.unwrap();
    // Nobody holds the second one back: it goes up at once, here or at a heartbeat.
    let _ = ana
        .raise_group_level(alone, fogata_state::Level::CURRENT)
        .await;

    let groups = ana.groups().await.unwrap();
    let view = |id| groups.iter().find(|g| g.id == id).unwrap();
    let behind: Vec<_> = view(held).level_behind.iter().map(|p| p.device).collect();
    assert_eq!(behind, vec![bea]);
    assert!(view(alone).level_behind.is_empty());

    let refused = ana.add_to_group(alone, bea).await.unwrap_err();
    assert!(
        refused.to_string().contains("must update Fogata"),
        "{refused}"
    );
    assert_eq!(ana.members(alone).await.unwrap().len(), 1);

    ana.shutdown().await.unwrap();
    drop(ana);
    let _ = std::fs::remove_dir_all(&base);
}

/// A group held back by a member never seen at this version's level may be raised anyway,
/// but only once it has waited a week (kept across restarts), and only leaving behind the
/// members its manager was shown: then it goes up, and nobody holds it back any more.
#[tokio::test(flavor = "multi_thread")]
async fn a_group_that_waited_a_week_goes_up_anyway_naming_who_is_left_behind() {
    let base = std::env::temp_dir().join(format!("fogata-anyway-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir, bea) = ana_with_an_old_friend(&base);
    let bea = bea.profile.device;
    let (ana, _events) = Core::start(dir.clone(), options()).unwrap();
    let held = ana.create_group("crew", 0, &[bea]).await.unwrap();
    let view = |groups: Vec<fogata_core::GroupView>| groups.into_iter().find(|g| g.id == held);
    let first = view(ana.groups().await.unwrap()).unwrap();
    assert!(!first.level_behind.is_empty() && !first.raise_anyway);
    let early = ana.raise_group_level_anyway(held, vec![bea]).await;
    assert!(early.is_err());
    ana.shutdown().await.unwrap();
    drop(ana);

    // Started to wait more than a week ago.
    let store = Store::open(dir.clone()).unwrap();
    let mut meta = store.meta();
    let key = hex::encode(held.as_bytes());
    let since = *meta.level_wait.get(&key).expect("the wait is kept");
    meta.level_wait
        .insert(key.clone(), since - fogata_core::LEVEL_WAIT_MS - 1);
    store.save_meta(&meta).unwrap();
    drop(store);

    let (ana, _events) = Core::start(dir, options()).unwrap();
    let waited = view(ana.groups().await.unwrap()).unwrap();
    assert!(waited.raise_anyway);
    assert_eq!(waited.level_behind.len(), 1);
    // Not with others named than those who are behind.
    assert!(ana.raise_group_level_anyway(held, vec![]).await.is_err());
    ana.raise_group_level_anyway(held, vec![bea]).await.unwrap();
    // Up: nobody holds it back any more, and it waits no longer.
    let up = view(ana.groups().await.unwrap()).unwrap();
    assert!(up.level_behind.is_empty() && !up.raise_anyway);

    ana.shutdown().await.unwrap();
    drop(ana);
    let _ = std::fs::remove_dir_all(&base);
}
