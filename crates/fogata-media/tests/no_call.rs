//! A member of a group in no call with this device gets no media taken from it: its media
//! stream is refused before anything on it is read, put together or queued. Nodes in one
//! process, over the loopback interface.

use std::{sync::Arc, time::Duration};

use fogata_crypto::{DeviceIdentity, DevicePublicKey};
use fogata_media::bridge::{self, BridgeConfig, MEDIA_PROTOCOL};
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::GroupLog;
use futures::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn node(
    identity: Arc<DeviceIdentity>,
    log: GroupLog,
    port: u16,
    peers: Vec<Multiaddr>,
) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log)],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers,
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap()
}

fn loopback(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

/// Waits until `device` is connected as a member on the node of `events`, dialling again now
/// and then.
async fn wait_to_connect(
    dialer: &NodeHandle,
    address: Multiaddr,
    events: &mut NodeEvents,
    device: DevicePublicKey,
) {
    for _ in 0..6 {
        let connected = timeout(Duration::from_secs(10), async {
            while let Some(event) = events.recv().await {
                if matches!(event, NodeEvent::MemberConnected { device: d, .. } if d == device) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        if connected {
            return;
        }
        dialer.dial(vec![address.clone()], None).await.unwrap();
    }
    panic!("timed out waiting for the member to connect");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_in_no_call_here_gets_its_media_stream_refused() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "ours", 1).unwrap();
    log.add(&ana, bea.public_key(), "bea", 2).unwrap();
    let port_a = free_port();
    let (node_a, mut events_a) = node(ana.clone(), log.clone(), port_a, vec![]);
    let (node_b, _events_b) = node(bea.clone(), log, 0, vec![loopback(port_a)]);
    wait_to_connect(&node_b, loopback(port_a), &mut events_a, bea.public_key()).await;
    let bridge = bridge::start(
        node_a.clone(),
        BridgeConfig {
            listen: "127.0.0.1:0".parse().unwrap(),
            username: "lab".into(),
            password: "pass".into(),
            impairment: None,
            streams_only: false,
            unanswered: None,
        },
    )
    .await
    .unwrap();

    // Bea, a member with no call with Ana (Ana's engine has no allocation for her), opens a
    // media stream and sends a packet's first part on it.
    let ana_peer = node_a.local_peer();
    let mut stream = None;
    for _ in 0..50 {
        if let Ok(s) = node_b.streams().open_stream(ana_peer, MEDIA_PROTOCOL).await {
            stream = Some(s);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut stream = stream.expect("a stream to Ana");
    let _ = stream.write_all(&[0, 4, 1, 2, 3, 4]).await;
    let _ = stream.flush().await;
    // Refused: the stream ends at once, rather than staying open to be read.
    let mut byte = [0u8; 1];
    let ended = timeout(Duration::from_secs(5), stream.read(&mut byte)).await;
    assert!(
        matches!(ended, Ok(Ok(0) | Err(_))),
        "the stream stayed open: {ended:?}"
    );
    bridge.close().await;
    node_a.shutdown().await.unwrap();
    node_b.shutdown().await.unwrap();
}
