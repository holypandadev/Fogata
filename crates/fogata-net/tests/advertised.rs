// Two devices on one machine, one dialling the other: the one dialled learns which of
// Fogata's protocols the other takes, as soon as they connect and when the other starts
// taking one more, so a newer feature (media sent in parts) is used only with devices that
// know it. The one that dialled is not told the other's: they go only to peers a device chose
// to reach, and to members and friends once they proved themselves.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::DeviceIdentity;
use fogata_net::{NodeConfig, NodeHandle, StreamProtocol};

const NEW: StreamProtocol = StreamProtocol::new("/fogata/test-new/1");
const LATER: StreamProtocol = StreamProtocol::new("/fogata/test-later/1");
/// Every device takes it: once it shows, the other has identified itself.
const MEMBER: StreamProtocol = StreamProtocol::new("/fogata/member/1");

fn node(listen_port: u16, peers: Vec<fogata_net::Multiaddr>) -> NodeHandle {
    let identity = Arc::new(DeviceIdentity::generate().unwrap());
    fogata_net::start(NodeConfig {
        identity,
        groups: Vec::new(),
        friends: None,
        listen_port,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: false,
        bridge: false,
        peers,
        remembered: Vec::new(),
        hidden: false,
        spill_dir: None,
    })
    .unwrap()
    .0
}

async fn until(limit: Duration, what: &str, done: impl Fn() -> bool) {
    let end = Instant::now() + limit;
    while !done() {
        assert!(Instant::now() < end, "not in time: {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_knows_which_of_our_protocols_the_other_takes() {
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let ana = node(port, Vec::new());
    let address = format!("/ip4/127.0.0.1/udp/{port}/quic-v1/p2p/{}", ana.local_peer())
        .parse()
        .unwrap();
    let bea = node(0, vec![address]);
    // Bea takes the new protocol from the start.
    let _new = bea.streams().accept(NEW).unwrap();
    let (a, b) = (ana.local_peer(), bea.local_peer());

    until(Duration::from_secs(15), "Bea identified by Ana", || {
        ana.advertises(&b, &MEMBER)
    })
    .await;
    until(
        Duration::from_secs(15),
        "Bea's new protocol told to Ana",
        || ana.advertises(&b, &NEW),
    )
    .await;
    assert!(!ana.advertises(&b, &LATER));
    // Ana did not choose Bea, which proved nothing: Bea is not told Ana's.
    assert!(
        !bea.advertises(&a, &MEMBER),
        "a stranger was told Ana's protocols"
    );

    // Bea starts taking one more while connected: Ana hears of it with the connection's
    // next exchange (pings go every 15 s).
    let _later = bea.streams().accept(LATER).unwrap();
    until(
        Duration::from_secs(30),
        "Bea's new protocol told to Ana",
        || ana.advertises(&b, &LATER),
    )
    .await;
}
