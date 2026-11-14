//! A TCP connection that never says anything is closed: one machine opening thousands would
//! otherwise hold a socket for each until the device has none left. From this computer, as
//! here, after ten seconds (the connection timeout the node puts around every transport);
//! from a stranger's address after five (see strangers.rs, and the network lab's
//! `silent-flood`).

use std::{
    io::Read,
    net::{SocketAddr, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::DeviceIdentity;
use fogata_net::NodeConfig;

/// A port free for both TCP and UDP here.
fn free_port() -> u16 {
    loop {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        if std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            return port;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_tcp_connection_is_closed() {
    let port = free_port();
    let (node, _events) = fogata_net::start(NodeConfig {
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
    // Listening.
    node.addresses().await.unwrap();

    let closed = tokio::task::spawn_blocking(move || {
        let at = SocketAddr::from(([127, 0, 0, 1], port));
        let streams: Vec<TcpStream> = (0..4).map(|_| TcpStream::connect(at).unwrap()).collect();
        let start = Instant::now();
        streams
            .into_iter()
            .map(|mut stream| {
                stream
                    .set_read_timeout(Some(
                        Duration::from_secs(30).saturating_sub(start.elapsed()),
                    ))
                    .unwrap();
                let mut buffer = [0u8; 256];
                // Whatever the node sends first (its half of the protocol negotiation), then
                // the end of the stream.
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) => return Some(start.elapsed()),
                        Ok(_) => continue,
                        Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {
                            return Some(start.elapsed());
                        }
                        Err(_) => return None,
                    }
                }
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap();
    for after in closed {
        let after = after.expect("a silent connection still open after 30 s");
        assert!(
            after < Duration::from_secs(13),
            "closed only after {after:?}"
        );
    }
    node.shutdown().await.unwrap();
}
