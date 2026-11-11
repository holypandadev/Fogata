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
