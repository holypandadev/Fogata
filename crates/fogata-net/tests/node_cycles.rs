//! Nodes started, serving files and taking the streams every node takes, and shut down again
//! and again in one runtime, leave no task and no open file behind.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;

/// Tasks alive in this runtime, and files open in this process (Linux).
fn held() -> (usize, usize) {
    let tasks = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();
    let files = std::fs::read_dir("/proc/self/fd").map_or(0, |d| d.count());
    (tasks, files)
}

#[tokio::test(flavor = "multi_thread")]
async fn fifty_nodes_leave_nothing_running() {
    let mut after = Vec::new();
    for _ in 0..50 {
        let (node, events) = fogata_net::start(fogata_net::NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap();
        fogata_net::files::serve(node.clone(), Default::default(), |_, _| {}).unwrap();
        node.shutdown().await.unwrap();
        drop((node, events));
        // The same pause in every cycle before counting, so that the counts compare.
        tokio::time::sleep(Duration::from_millis(100)).await;
        after.push(held());
    }
    let (first, last) = (after[4], after[49]);
    println!("after 5 cycles {first:?}, after 50 {last:?}");
    assert!(last.0 <= first.0 + 2, "tasks grew: {after:?}");
    assert!(last.1 <= first.1 + 2, "files grew: {after:?}");
}

/// Whether nothing listens on TCP `port` any more on this computer.
fn port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// A node whose handles are all dropped without a shutdown stops by itself, its streams'
/// loops and file and release serving included, and gives its listening port back.
#[tokio::test(flavor = "multi_thread")]
async fn a_node_stops_when_its_handles_are_dropped() {
    let port = std::net::TcpListener::bind(("0.0.0.0", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let (node, events) = fogata_net::start(fogata_net::NodeConfig {
        identity: Arc::new(DeviceIdentity::generate().unwrap()),
        groups: Vec::new(),
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: false,
        peers: Vec::new(),
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap();
    fogata_net::files::serve(node.clone(), Default::default(), |_, _| {}).unwrap();
    let (heard, _hear) = tokio::sync::mpsc::unbounded_channel();
    fogata_net::updates::serve(node.clone(), Default::default(), heard, Default::default())
        .unwrap();
    // Listening.
    let started = tokio::time::Instant::now();
    while port_free(port) && started.elapsed() < Duration::from_secs(5) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!port_free(port), "the node listens on {port}");
    drop((node, events));
    let dropped = tokio::time::Instant::now();
    while !port_free(port) && dropped.elapsed() < Duration::from_secs(3) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(port_free(port), "the node still listens on {port}");
}

/// A node's own tasks (the loops taking its streams, file and release serving's among them)
/// are its: they run while it does, are all gone once its shutdown returns, and cycle
/// after cycle leave neither tasks nor open files behind.
#[tokio::test(flavor = "multi_thread")]
async fn a_node_s_tasks_are_gone_when_its_shutdown_returns() {
    let mut after = Vec::new();
    for _ in 0..50 {
        let (node, events) = fogata_net::start(fogata_net::NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap();
        fogata_net::files::serve(node.clone(), Default::default(), |_, _| {}).unwrap();
        let (heard, _hear) = tokio::sync::mpsc::unbounded_channel();
        fogata_net::updates::serve(node.clone(), Default::default(), heard, Default::default())
            .unwrap();
        // The protocol marker's and capability level's loops, typing's, files' and releases'.
        assert!(node.tasks_running() >= 5, "{} tasks", node.tasks_running());
        node.shutdown().await.unwrap();
        assert_eq!(node.tasks_running(), 0, "tasks outlived the node");
        drop((node, events));
        // The same pause in every cycle before counting, so that the counts compare.
        tokio::time::sleep(Duration::from_millis(100)).await;
        after.push(held());
    }
    let (first, last) = (after[4], after[49]);
    println!("after 5 cycles {first:?}, after 50 {last:?}");
    assert!(last.0 <= first.0 + 2, "tasks grew: {after:?}");
    assert!(last.1 <= first.1 + 2, "files grew: {after:?}");
}
