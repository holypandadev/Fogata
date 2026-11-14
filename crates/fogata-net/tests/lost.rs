//! Two members whose only connection dies in an outage longer than QUIC's idle timeout are
//! connected again soon after the way opens, by dialling each other where they were reached:
//! without discovery of any kind (no mDNS, no DHT), nothing else would bring them back.

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, NodeConfig, NodeEvent, NodeEvents};
use fogata_state::GroupLog;
use tokio::{net::UdpSocket, time::timeout};

/// A UDP forwarder between the first device to send to `front` and the device at `back`,
/// which drops every packet while `cut` is set: a link that goes dead and comes back.
async fn forwarder(back: SocketAddr, cut: Arc<AtomicBool>) -> SocketAddr {
    let front = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let behind = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let address = front.local_addr().unwrap();
    let client: Arc<std::sync::Mutex<Option<SocketAddr>>> = Default::default();
    {
        let (front, behind, cut, client) =
            (front.clone(), behind.clone(), cut.clone(), client.clone());
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            while let Ok((n, from)) = front.recv_from(&mut buf).await {
                *client.lock().unwrap() = Some(from);
                if !cut.load(Ordering::SeqCst) {
                    let _ = behind.send_to(&buf[..n], back).await;
                }
            }
        });
    }
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        while let Ok((n, _)) = behind.recv_from(&mut buf).await {
            let to = *client.lock().unwrap();
            if let Some(to) = to.filter(|_| !cut.load(Ordering::SeqCst)) {
                let _ = front.send_to(&buf[..n], to).await;
            }
        }
    });
    address
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn next_member_event(events: &mut NodeEvents, connected: bool) -> bool {
    while let Some(event) = events.recv().await {
        match event {
            NodeEvent::MemberConnected { .. } if connected => return true,
            NodeEvent::MemberDisconnected { .. } if !connected => return true,
            _ => {}
        }
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_lost_in_a_long_outage_is_dialled_again() {
    let ana = Arc::new(DeviceIdentity::generate().unwrap());
    let bea = Arc::new(DeviceIdentity::generate().unwrap());
    let mut log = GroupLog::create(&ana, "g", 1).unwrap();
    log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
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
        spill_dir: None,
    };
    let port_b = free_port();
    let cut = Arc::new(AtomicBool::new(false));
    let front = forwarder(([127, 0, 0, 1], port_b).into(), cut.clone()).await;
    let through = format!("/ip4/127.0.0.1/udp/{}/quic-v1", front.port())
        .parse()
        .unwrap();
    let (_node_b, _events_b) = fogata_net::start(config(bea, port_b, Vec::new())).unwrap();
    let (_node_a, mut events_a) = fogata_net::start(config(ana, 0, vec![through])).unwrap();

    let met = timeout(
        Duration::from_secs(15),
        next_member_event(&mut events_a, true),
    );
    assert!(met.await.unwrap_or(false), "the members never met");

    // QUIC gives up on the connection after 30 s without a packet.
    cut.store(true, Ordering::SeqCst);
    let lost = timeout(
        Duration::from_secs(60),
        next_member_event(&mut events_a, false),
    );
    assert!(lost.await.unwrap_or(false), "the connection never closed");

    // The way opens again: the first dial goes 2 s after the loss, the next ones 4 and 8 s
    // after that.
    cut.store(false, Ordering::SeqCst);
    let back = timeout(
        Duration::from_secs(20),
        next_member_event(&mut events_a, true),
    );
    assert!(
        back.await.unwrap_or(false),
        "the member was not dialled again"
    );
}
