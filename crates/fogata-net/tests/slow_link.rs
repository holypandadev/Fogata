// Two devices on one machine over TCP through a link that takes seconds each way, as a
// satellite or congested mobile link losing a packet or two: the connection is set up all
// the same, on both sides, however long its few round trips take within the bound.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::DeviceIdentity;
use fogata_net::{NodeConfig, NodeHandle, StreamProtocol};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};

/// Every device takes it: once it shows, the other has identified itself. A device lists it
/// only to peers it chose to reach: Bea's shows to Ana, whom Bea dialled.
const MEMBER: StreamProtocol = StreamProtocol::new("/fogata/member/1");

/// How long each byte takes from one end to the other: a TCP connection's setup (Noise and
/// the negotiations) is a few of these each way, more than five seconds in all here.
const ONE_WAY: Duration = Duration::from_millis(2600);

fn node(listen_port: u16, peers: Vec<fogata_net::Multiaddr>) -> NodeHandle {
    fogata_net::start(NodeConfig {
        identity: Arc::new(DeviceIdentity::generate().unwrap()),
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

/// Copies what `from` reads to `to`, each piece [`ONE_WAY`] after it was read.
async fn late(mut from: tokio::net::tcp::OwnedReadHalf, mut to: tokio::net::tcp::OwnedWriteHalf) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(tokio::time::Instant, Vec<u8>)>();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        while let Ok(n) = from.read(&mut buf).await {
            if n == 0
                || tx
                    .send((tokio::time::Instant::now(), buf[..n].to_vec()))
                    .is_err()
            {
                break;
            }
        }
    });
    while let Some((at, bytes)) = rx.recv().await {
        tokio::time::sleep_until(at + ONE_WAY).await;
        if to.write_all(&bytes).await.is_err() {
            break;
        }
    }
}

/// A slow link from a port of its own to `port`.
async fn slow_link(port: u16) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((inbound, _)) = listener.accept().await {
            let Ok(outbound) = TcpStream::connect(("127.0.0.1", port)).await else {
                continue;
            };
            let (in_read, in_write) = inbound.into_split();
            let (out_read, out_write) = outbound.into_split();
            tokio::spawn(late(in_read, out_write));
            tokio::spawn(late(out_read, in_write));
        }
    });
    local
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_over_a_slow_link_is_set_up() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let ana = node(port, Vec::new());
    let link = slow_link(port).await;
    let address = format!("/ip4/127.0.0.1/tcp/{link}/p2p/{}", ana.local_peer())
        .parse()
        .unwrap();
    let started = Instant::now();
    let bea = node(0, vec![address]);
    let (a, b) = (ana.local_peer(), bea.local_peer());
    // Ana did not choose Bea: Bea is not told Ana's protocols, only that Ana identified
    // herself.
    while !(ana.advertises(&b, &MEMBER) && bea.peer_level(&a).is_some()) {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "no connection over the slow link"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("identified both ways after {:?}", started.elapsed());
}
