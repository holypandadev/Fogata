// Streams of one protocol that arrive together, before the loop taking them runs again, all
// reach it: several previews, downloads or update asks from one member at once, on a busy
// computer. libp2p-stream as published hands them over through a channel with room for one
// and drops the others, which their askers see reset before any answer
// (vendor/libp2p-stream/FOGATA.md).

use std::{sync::Arc, time::Duration};

use fogata_crypto::DeviceIdentity;
use fogata_net::{NodeConfig, NodeHandle, StreamProtocol};
use futures::{AsyncReadExt as _, AsyncWriteExt as _};

const BURST: StreamProtocol = StreamProtocol::new("/fogata/test-burst/1");
/// Streams opened at once: as many as a room page's previews might be.
const STREAMS: usize = 48;

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
    })
    .unwrap()
    .0
}

/// Asks on a new stream and waits for the answer: whether it came.
async fn ask(node: NodeHandle, peer: fogata_net::PeerId) -> bool {
    let Ok(mut stream) = node.streams().open_stream(peer, BURST).await else {
        return false;
    };
    let asked = async {
        stream.write_all(&[1]).await?;
        stream.flush().await?;
        let mut answer = [0u8; 1];
        stream.read_exact(&mut answer).await?;
        std::io::Result::Ok(answer[0] == 2)
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(20), asked).await,
        Ok(Ok(true))
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn streams_opened_at_once_all_reach_a_late_taker() {
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let ana = node(port, Vec::new());
    let a = ana.local_peer();
    let mut incoming = ana.streams().accept(BURST).unwrap();
    let address = format!("/ip4/127.0.0.1/udp/{port}/quic-v1/p2p/{a}")
        .parse()
        .unwrap();
    let bea = node(0, vec![address]);

    // Bea asks on many streams at once while Ana's loop is busy elsewhere: it takes none of
    // them until they have all been opened (an opened stream is one Ana's end has taken the
    // protocol of) and a little more.
    let asks = (0..STREAMS)
        .map(|_| ask(bea.clone(), a))
        .collect::<Vec<_>>();
    let (opened, opened_rx) = tokio::sync::oneshot::channel::<()>();
    let taker = ana.background();
    let answering = tokio::spawn(async move {
        let _ = opened_rx.await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        while let Some((_, mut stream)) = taker.next_stream(&mut incoming).await {
            tokio::spawn(async move {
                let mut asked = [0u8; 1];
                if stream.read_exact(&mut asked).await.is_ok() {
                    let _ = stream.write_all(&[2]).await;
                    let _ = stream.close().await;
                }
            });
        }
    });
    let asked = tokio::spawn(futures::future::join_all(asks));
    // Every stream is opened within this; the answers wait for the taker.
    tokio::time::sleep(Duration::from_secs(1)).await;
    let _ = opened.send(());
    let answered = asked.await.unwrap().into_iter().filter(|a| *a).count();
    answering.abort();
    assert_eq!(answered, STREAMS, "streams answered of {STREAMS}");
}
