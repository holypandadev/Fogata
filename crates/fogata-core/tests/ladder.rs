//! A group's ladder with real data folders: the first leaves alone and the others go on
//! without it; a device removed by someone who was moved down at the same time learns, from
//! a friend in the group, that it was never out, and runs the group again without being
//! asked. Two devices that are friends, in one process, each dialling the other's loopback
//! address; nothing else of the network.

use std::time::Duration;

use fogata_core::CoreEvents;
use fogata_core::{ActivityWhat, Core, CoreEvent, NetOptions, Store};
use fogata_crypto::DeviceIdentity;
use fogata_friends::Friend;
use fogata_state::{Body, GroupLog};
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn wait_for(events: &mut CoreEvents, what: &str, mut done: impl FnMut(&CoreEvent) -> bool) {
    let found = timeout(Duration::from_secs(90), async {
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
    let (dir_a, dir_b) = (base.join("ana"), base.join("dani"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Dani", None).unwrap();
    let (store_a, store_b) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_b.clone()).unwrap(),
    );
    let mut book_a = store_a.friends().unwrap().unwrap();
    let mut book_b = store_b.friends().unwrap().unwrap();
    let friend = |book: &fogata_friends::FriendBook| Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    book_a
        .friends
        .insert(book_b.profile.device, friend(&book_b));
    book_b
        .friends
        .insert(book_a.profile.device, friend(&book_a));
    store_a.save_friends(&book_a).unwrap();
    store_b.save_friends(&book_b).unwrap();
    (dir_a, dir_b)
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Listening on `port`, dialling the loopback address of the device at `peer`.
fn options(port: u16, peer: u16) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers: vec![
            format!("/ip4/127.0.0.1/udp/{peer}/quic-v1")
                .parse()
                .unwrap(),
        ],
        ..Default::default()
    }
}

/// No network beyond this computer, and nobody found on it.
fn alone() -> NetOptions {
    NetOptions {
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_whose_removal_was_undone_is_back_without_being_asked() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-ladder-undone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_d) = friends(&base);
    let (store_a, store_d) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_d.clone()).unwrap(),
    );
    let (ana, _) = store_a.identity().unwrap();
    let (dani, _) = store_d.identity().unwrap();
    let bea = DeviceIdentity::generate().unwrap();
    let now = now_ms();
    let mut base_log = GroupLog::create(&ana, "Sundays", now).unwrap();
    base_log.add(&ana, bea.public_key(), "Bea", now).unwrap();
    base_log
        .add(&ana, dani.public_key(), "Dani", now + 1)
        .unwrap();
    // Ana, alone, sends Bea to the end. Bea, not knowing, removes Dani and renames the
    // group; Dani was told, and stopped running it.
    let (mut at_ana, mut at_bea) = (base_log.clone(), base_log.clone());
    at_ana
        .append(
            &ana,
            Body::Demote {
                member: bea.public_key(),
            },
            now + 10,
        )
        .unwrap();
    at_bea.remove(&bea, dani.public_key(), now + 11).unwrap();
    at_bea
        .append(
            &bea,
            Body::Rename {
                name: "Taken".into(),
            },
            now + 12,
        )
        .unwrap();
    assert!(!at_bea.state().is_member(&dani.public_key()));
    store_d.save_group_log(&at_bea, "Sundays").unwrap();
    // Ana then met Bea: the removal and the rename are undone.
    at_ana.insert_all(at_bea.entries().cloned());
    let state = at_ana.state();
    assert!(state.is_member(&dani.public_key()));
    assert_eq!(state.name, "Sundays");
    assert_eq!(state.ladder.last(), Some(&bea.public_key()));
    store_a.save_group_log(&at_ana, "Sundays").unwrap();
    let group = at_ana.group_id();
    drop((store_a, store_d));

    let (port_a, port_d) = (free_port(), free_port());
    let (core_d, mut events_d) = Core::start(dir_d, options(port_d, port_a)).unwrap();
    assert!(core_d.groups().await.unwrap().is_empty(), "removed here");
    let (core_a, _events_a) = Core::start(dir_a, options(port_a, port_d)).unwrap();
    // Ana offers the group to her friend, whom she counts as a member: Dani runs it again,
    // with no invitation to answer.
    wait_for(
        &mut events_d,
        "the group back",
        |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
    )
    .await;
    assert!(core_d.group_invitations().await.unwrap().is_empty());
    let groups = core_d.groups().await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Sundays");
    // Third, below Ana and above Bea, who was sent to the end.
    assert_eq!(groups[0].rank, 1);
    let members = core_d.members(group).await.unwrap();
    assert_eq!(members.len(), 3);
    assert_eq!(members.last().unwrap().person.device, bea.public_key());
    drop(core_a);
    drop(core_d);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_leaves_alone_and_the_others_go_on() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-ladder-first-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_d) = friends(&base);
    let (store_a, store_d) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_d.clone()).unwrap(),
    );
    let (ana, _) = store_a.identity().unwrap();
    let (dani, _) = store_d.identity().unwrap();
    let leo = DeviceIdentity::generate().unwrap();
    let now = now_ms();
    let mut log = GroupLog::create(&ana, "Sundays", now).unwrap();
    log.add(&ana, dani.public_key(), "Dani", now).unwrap();
    log.add(&ana, leo.public_key(), "Leo", now + 1).unwrap();
    store_a.save_group_log(&log, "Sundays").unwrap();
    store_d.save_group_log(&log, "Sundays").unwrap();
    let group = log.group_id();
    drop((store_a, store_d));

    // Ana, first, leaves with nobody else online: nothing stops her.
    let (core_a, _events_a) = Core::start(dir_a, alone()).unwrap();
    assert_eq!(core_a.groups().await.unwrap()[0].rank, 0);
    core_a.leave_group(group).await.unwrap();
    assert!(core_a.groups().await.unwrap().is_empty());
    core_a.shutdown().await.unwrap();
    drop(core_a);

    // Her leave never reaches Dani: Ana stays first in Dani's list, and Dani, second,
    // still does everything on the group and on those below.
    let (core_d, _events_d) = Core::start(dir_d, alone()).unwrap();
    let mine = &core_d.groups().await.unwrap()[0];
    assert_eq!((mine.rank, mine.manager), (1, true));
    assert_eq!(mine.outranks, vec![leo.public_key()]);
    core_d.rename_group(group, "Sin Ana").await.unwrap();
    assert!(core_d.invite(group, 1).await.is_ok());
    core_d
        .remove_from_group(group, leo.public_key())
        .await
        .unwrap();
    let members = core_d.members(group).await.unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].person.device, ana.public_key());
    assert_eq!(core_d.groups().await.unwrap()[0].name, "Sin Ana");
    // Nobody acts on the one above everyone.
    assert!(core_d.demote(group, ana.public_key()).await.is_err());
    // The activity, the newest first, says who did what: Leo managed (everyone does), so
    // the invite he could answer ended with his removal.
    let (lines, more) = core_d.group_activity(group, 0, 4).await.unwrap();
    assert!(more);
    let said: Vec<_> = lines
        .iter()
        .map(|l| (l.by.device, l.what.clone()))
        .collect();
    assert!(matches!(&said[0], (by, ActivityWhat::Revoked) if *by == dani.public_key()));
    assert!(matches!(&said[1], (by, ActivityWhat::Removed { member })
        if *by == dani.public_key() && member.device == leo.public_key()));
    assert!(matches!(&said[3], (by, ActivityWhat::Renamed { name })
        if *by == dani.public_key() && name == "Sin Ana"));
    let (all, more) = core_d.group_activity(group, 0, 50).await.unwrap();
    assert!(!more);
    assert!(
        matches!(&all.last().unwrap().what, ActivityWhat::Created { name } if name == "Sundays")
    );
    assert!(all.iter().all(|l| l.undone.is_none() && !l.seen_here));
    drop(core_d);
    let _ = std::fs::remove_dir_all(&base);
}
