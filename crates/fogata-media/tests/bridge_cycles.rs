//! Bridges started and closed (or dropped) again and again on one node, in one runtime,
//! leave no task and no open file behind, also when starting fails half way.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_media::bridge::{self, BridgeConfig};

/// Tasks alive in this runtime, and files open in this process (Linux).
fn held() -> (usize, usize) {
    let tasks = tokio::runtime::Handle::current()
        .metrics()
        .num_alive_tasks();
    let files = std::fs::read_dir("/proc/self/fd").map_or(0, |d| d.count());
    (tasks, files)
}

fn config(listen: std::net::SocketAddr) -> BridgeConfig {
    BridgeConfig {
        listen,
        username: "lab".into(),
        password: "pass".into(),
        impairment: None,
        streams_only: false,
        unanswered: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fifty_bridges_leave_nothing_running() {
    let (node, _events) = fogata_net::start(fogata_net::NodeConfig {
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
    // A port taken already: the bridge cannot listen there.
    let taken = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut after = Vec::new();
    for i in 0..50 {
        let bridge = bridge::start(node.clone(), config("127.0.0.1:0".parse().unwrap()))
            .await
            .unwrap();
        if i % 2 == 0 {
            bridge.close().await;
        }
        drop(bridge);
        assert!(
            bridge::start(node.clone(), config(taken.local_addr().unwrap()))
                .await
                .is_err()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        after.push(held());
    }
    let (first, last) = (after[4], after[49]);
    println!("after 5 cycles {first:?}, after 50 {last:?}");
    assert!(last.0 <= first.0 + 2, "tasks grew: {after:?}");
    assert!(last.1 <= first.1 + 2, "files grew: {after:?}");
    let _ = node.shutdown().await;
}

/// An allocation of the local WebRTC engine on the bridge at `bridge`, for a member: what
/// the engine's connection for that member makes. Its client is closed once it is made:
/// the allocation stays on the bridge, as it would for an engine that stopped answering.
async fn allocate(bridge: std::net::SocketAddr) {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let client = turn::client::Client::new(turn::client::ClientConfig {
        stun_serv_addr: bridge.to_string(),
        turn_serv_addr: bridge.to_string(),
        username: bridge::member_user("lab", &[7; 32]),
        password: "pass".into(),
        realm: "fogata".into(),
        software: String::new(),
        rto_in_ms: 0,
        conn: Arc::new(socket),
        vnet: None,
    })
    .await
    .unwrap();
    client.listen().await.unwrap();
    let relay = client.allocate().await.unwrap();
    drop(relay);
    client.close().await.unwrap();
}

/// Bridges dropped on a thread without a runtime (where the application's last handle on
/// one may go), each with an allocation of the engine open, stop their TURN endpoint too:
/// its port is free again, and nothing they ran stays alive.
#[tokio::test(flavor = "multi_thread")]
async fn bridges_dropped_away_from_their_runtime_leave_nothing_running() {
    let (node, _events) = fogata_net::start(fogata_net::NodeConfig {
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
    let mut after = Vec::new();
    for _ in 0..12 {
        let bridge = bridge::start(node.clone(), config("127.0.0.1:0".parse().unwrap()))
            .await
            .unwrap();
        let addr = bridge.local_addr();
        allocate(addr).await;
        std::thread::spawn(move || drop(bridge)).join().unwrap();
        let mut free = false;
        for _ in 0..60 {
            if std::net::UdpSocket::bind(addr).is_ok() {
                free = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            free,
            "the TURN endpoint of a bridge dropped still listens at {addr}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        after.push(held());
    }
    let (first, last) = (after[1], after[11]);
    println!("after 2 bridges {first:?}, after 12 {last:?}");
    assert!(last.0 <= first.0 + 2, "tasks grew: {after:?}");
    assert!(last.1 <= first.1 + 2, "files grew: {after:?}");
    let _ = node.shutdown().await;
}
