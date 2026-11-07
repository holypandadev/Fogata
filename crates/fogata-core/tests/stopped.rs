//! The screens are told when the network part stops by itself, and not when the application
//! shuts it down.

use std::time::Duration;

use fogata_core::{Core, CoreEvent, NetOptions};

/// One test at a time: each starts a device that listens on this computer's network, and
/// several at once on a busy computer leave its timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread")]
async fn shutting_down_is_not_told_as_a_stop() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-stopped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Ana", None).unwrap();
    let options = NetOptions {
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    };
    let (core, mut events) = Core::start(dir.clone(), options).unwrap();
    core.shutdown().await.unwrap();
    let stopped = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(event) = events.recv().await {
            if matches!(event, CoreEvent::Stopped) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(!stopped);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The application may shut the core down before a move of the account stops it again: the
/// second time says at once that it had stopped, and waits on nothing.
#[tokio::test(flavor = "multi_thread")]
async fn shutting_down_twice_answers_at_once() {
    let _one = ONE_AT_A_TIME.lock().await;
    let dir = std::env::temp_dir().join(format!("fogata-stopped-twice-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Ana", None).unwrap();
    let options = NetOptions {
        use_dht: false,
        use_mdns: false,
        ..Default::default()
    };
    let (core, _events) = Core::start(dir.clone(), options).unwrap();
    core.shutdown().await.unwrap();
    let again = tokio::time::timeout(Duration::from_secs(2), core.shutdown())
        .await
        .expect("a second shutdown answers at once");
    assert!(again.is_err(), "it says the node had stopped");
    drop(core);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A core dropped without a shutdown, as one whose start fails after its node started,
/// stops its node too: the port is free again for the next start. So does one that started
/// its media bridge for a call.
#[tokio::test(flavor = "multi_thread")]
async fn a_core_dropped_stops_its_node() {
    let _one = ONE_AT_A_TIME.lock().await;
    dropped_stops_its_node("drop", false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_core_dropped_after_a_call_stops_its_node() {
    let _one = ONE_AT_A_TIME.lock().await;
    dropped_stops_its_node("drop-bridge", true).await;
}

async fn dropped_stops_its_node(name: &str, call: bool) {
    let dir = std::env::temp_dir().join(format!("fogata-stopped-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Core::create_profile(&dir, "Ana", None).unwrap();
    let port = std::net::TcpListener::bind(("0.0.0.0", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let free = || std::net::TcpListener::bind(("0.0.0.0", port)).is_ok();
    let options = NetOptions {
        use_dht: false,
        use_mdns: false,
        listen_port: port,
        ..Default::default()
    };
    let (core, events) = Core::start(dir.clone(), options).unwrap();
    let started = tokio::time::Instant::now();
    while free() && started.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!free(), "the node listens on {port}");
    if call {
        core.media_bridge().await.unwrap();
    }
    drop((core, events));
    let dropped = tokio::time::Instant::now();
    while !free() && dropped.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(free(), "the node still listens on {port}");
    std::fs::remove_dir_all(&dir).unwrap();
}
