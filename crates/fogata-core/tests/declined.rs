//! A group invitation from a friend, declined: two devices that are friends, in one process.
//! Each dials the other's loopback address, and nothing else of the network is used.

use std::time::Duration;

use fogata_core::CoreEvents;
use fogata_core::{Core, CoreEvent, NetOptions, Store};
use fogata_friends::Friend;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
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

/// Polls `check` until it holds, for up to `seconds`.
async fn eventually<F, Fut>(what: &str, seconds: u64, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let held = timeout(Duration::from_secs(seconds), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .is_ok();
    assert!(held, "timed out waiting for {what}");
}

/// Two data folders whose profiles are already friends.
fn friends(base: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
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

/// Listening at `port` on this computer, and dialling the other device at `peer`.
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

/// Whether Bea's device is still among the members of `group` as Ana sees them.
async fn bea_is_member(
    ana: &Core,
    bea: fogata_crypto::DevicePublicKey,
    group: fogata_crypto::GroupId,
) -> bool {
    ana.members(group)
        .await
        .unwrap()
        .iter()
        .any(|m| m.person.device == bea)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_declined_invitation_does_not_come_back_until_added_again() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-declined-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a.clone(), options(port_a, port_b)).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, port_a)).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    let group = ana.create_group("crew", 0, &[bea.me()]).await.unwrap();
    let invited =
        |e: &CoreEvent| matches!(e, CoreEvent::GroupInvited { group: g, .. } if *g == group);
    wait_for(&mut events_b, "the group from Ana", invited).await;

    // Ana is gone when Bea says no: her copy of the group still counts Bea as a member.
    ana.shutdown().await.unwrap();
    drop(ana);
    bea.decline_group_invitation(group).await.unwrap();
    assert!(bea.group_invitations().await.unwrap().is_empty());

    // Back, Ana offers the group again as she meets Bea (at once or at the next heartbeat):
    // Bea is not asked again, and hands Ana the leave she signed, so Ana stops counting her
    // and hears that Bea said no.
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, port_b)).unwrap();
    wait_for(&mut events_a, "Bea online again", online).await;
    let me_b = bea.me();
    wait_for(&mut events_a, "Bea's no", |e| {
        matches!(e, CoreEvent::InvitationDeclined { group: g, device, .. } if *g == group && *device == me_b)
    })
    .await;
    eventually("Bea gone from Ana's members", 90, || async {
        !bea_is_member(&ana, me_b, group).await
    })
    .await;
    while let Ok(event) = events_b.try_recv() {
        assert!(!invited(&event), "the declined group was offered again");
    }
    assert!(bea.group_invitations().await.unwrap().is_empty());
    assert!(bea.groups().await.unwrap().is_empty());

    // Ana adds her again on purpose: that is a new invitation.
    ana.add_to_group(group, me_b).await.unwrap();
    wait_for(&mut events_b, "the group from Ana again", invited).await;
    assert_eq!(bea.group_invitations().await.unwrap().len(), 1);
    bea.accept_group_invitation(group).await.unwrap();
    eventually("both members connected", 30, || async {
        ana.groups().await.unwrap()[0].online == 1
    })
    .await;
    let _ = std::fs::remove_dir_all(&base);
}

/// The `InvitationDeclined` events among those that come within `seconds`.
async fn declines_within(events: &mut CoreEvents, seconds: u64) -> usize {
    let mut count = 0;
    let _ = timeout(Duration::from_secs(seconds), async {
        while let Some(event) = events.recv().await {
            if matches!(event, CoreEvent::InvitationDeclined { .. }) {
                count += 1;
            }
        }
    })
    .await;
    count
}

#[tokio::test(flavor = "multi_thread")]
async fn the_inviter_hears_of_each_declined_invitation_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-declined-told-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, port_b)).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options(port_b, port_a)).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    let group = ana.create_group("crew", 0, &[bea.me()]).await.unwrap();
    let invited =
        |e: &CoreEvent| matches!(e, CoreEvent::GroupInvited { group: g, .. } if *g == group);
    wait_for(&mut events_b, "the group from Ana", invited).await;

    // Bea says no while Ana is online: Ana hears it, with both names.
    let me_b = bea.me();
    bea.decline_group_invitation(group).await.unwrap();
    let mut said = None;
    wait_for(&mut events_a, "Bea's no", |e| match e {
        CoreEvent::InvitationDeclined {
            group: g,
            name,
            device,
            who,
        } if *g == group => {
            said = Some((name.clone(), *device, who.clone()));
            true
        }
        _ => false,
    })
    .await;
    assert_eq!(said, Some(("crew".to_string(), me_b, "Bea".to_string())));
    assert!(!bea_is_member(&ana, me_b, group).await);
    // Past a friend heartbeat (20 s), when offers go again: said once.
    assert_eq!(declines_within(&mut events_a, 25).await, 0);

    // Added again, that is another invitation; declined again, said again, once.
    ana.add_to_group(group, me_b).await.unwrap();
    wait_for(&mut events_b, "the group from Ana again", invited).await;
    bea.decline_group_invitation(group).await.unwrap();
    wait_for(
        &mut events_a,
        "Bea's second no",
        |e| matches!(e, CoreEvent::InvitationDeclined { group: g, .. } if *g == group),
    )
    .await;
    assert_eq!(declines_within(&mut events_a, 25).await, 0);
    // Bea, who made no invitation, hears nothing of the kind.
    assert_eq!(declines_within(&mut events_b, 1).await, 0);
    let _ = std::fs::remove_dir_all(&base);
}
