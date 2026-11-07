//! A data folder copied to another computer and opened there while the first copy runs:
//! both copies are told the identity is open twice. Uses only the loopback interface; no DHT
//! and no mDNS, so the copies only meet the member they both know.

use std::{path::Path, time::Duration};

use fogata_core::CoreEvents;
use fogata_core::{Core, CoreEvent, NetOptions};
use fogata_net::Multiaddr;
use tokio::time::timeout;

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn options(port: u16, peers: Vec<Multiaddr>) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers,
        ..Default::default()
    }
}

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

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn open_twice(e: &CoreEvent) -> bool {
    matches!(e, CoreEvent::OtherSession)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_copied_folder_open_on_two_computers_is_noticed_on_both() {
    let base = std::env::temp_dir().join(format!("fogata-core-twice-{}", std::process::id()));
    let (dir_a, dir_b, dir_copy) = (base.join("ana"), base.join("bea"), base.join("bea-copy"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let port_a = free_port();
    let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
    let group = ana.create_group("crew", 0, &[]).await.unwrap();
    let invite = ana.invite(group, 1).await.unwrap();
    let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
        .parse()
        .unwrap();

    let (bea, mut events_b) = Core::start(dir_b.clone(), options(0, vec![to_ana.clone()])).unwrap();
    bea.join_group(&invite, None, None).await.unwrap();
    let who = bea.me();
    wait_for(
        &mut events_a,
        "Bea's ask",
        |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
    )
    .await;
    ana.answer_join(group, who, true).await.unwrap();
    wait_for(
        &mut events_b,
        "the join",
        |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
    )
    .await;

    // Bea quits and starts again on the same folder: one computer, nothing to tell.
    bea.shutdown().await.unwrap();
    drop((bea, events_b));
    let (bea, mut events_b) = Core::start(dir_b.clone(), options(0, vec![to_ana.clone()])).unwrap();
    wait_for(
        &mut events_a,
        "Bea back",
        |e| matches!(e, CoreEvent::MemberConnected { device, .. } if *device == who),
    )
    .await;

    let quiet = timeout(Duration::from_secs(3), async {
        while let Some(event) = events_b.recv().await {
            assert!(
                !open_twice(&event),
                "a restart was taken for another computer"
            );
        }
    });
    let _ = quiet.await;

    // Her folder, copied to another computer and opened there.
    copy_folder(&dir_b, &dir_copy);
    let (copy, mut events_copy) = Core::start(dir_copy, options(0, vec![to_ana])).unwrap();
    assert_eq!(copy.me(), who);
    wait_for(&mut events_copy, "the notice on the copy", open_twice).await;
    wait_for(&mut events_b, "the notice on Bea's computer", open_twice).await;
    let _ = ana.shutdown().await;
    let _ = bea.shutdown().await;
    let _ = copy.shutdown().await;
    let _ = std::fs::remove_dir_all(&base);
}
