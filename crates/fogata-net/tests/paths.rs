//! Two members that reach each other in several ways at once (QUIC and TCP, loopback and the
//! local network) are told how the other is reached once, not once for every connection that
//! opens: the application shows the way, and it must not flicker.

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, NodeEvent, Path};
use fogata_state::GroupLog;

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// This computer's address on its local network, if it has one (no packet is sent).
fn local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_reached_several_ways_is_told_once() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
    let ways = |port: u16| -> Vec<Multiaddr> {
        let mut ways: Vec<Multiaddr> = vec![
            format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
                .parse()
                .unwrap(),
            format!("/ip4/127.0.0.1/tcp/{port}").parse().unwrap(),
        ];
        if let Some(ip) = local_ip() {
            ways.push(format!("/ip4/{ip}/udp/{port}/quic-v1").parse().unwrap());
        }
        ways
    };
    let config = |identity, port, peers| NodeConfig {
        identity,
        groups: vec![GroupSetup::Member(log.clone())],
        friends: None,
        listen_port: port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: true,
        peers,
        remembered: Vec::new(),
        hidden: false,
    };
    let (port_a, port_b) = (free_port(), free_port());
    let (_node_a, mut events_a) = fogata_net::start(config(ana, port_a, ways(port_b))).unwrap();
    let (_node_b, _events_b) = fogata_net::start(config(bea, port_b, ways(port_a))).unwrap();
    let mut told = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = events_a.recv().await {
            if let NodeEvent::MemberConnected { path, .. } = event {
                told.push(path);
            }
        }
    })
    .await;
    assert_eq!(told, [Path::Lan], "told {told:?}");
}
