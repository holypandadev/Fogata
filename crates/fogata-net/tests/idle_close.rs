// A QUIC connection that one device closes for being idle ends at the other device at once,
// with a close it receives, not when that device's own idle timeout or QUIC's runs out. Two
// bare swarms on the loopback address, alone in this process; nothing else of the network.

use std::time::{Duration, Instant};

use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, Swarm, SwarmBuilder,
    swarm::{SwarmEvent, dummy},
};
use tokio::time::timeout;

/// A swarm on QUIC only whose connections close once idle for `idle`.
fn swarm(idle: Duration) -> Swarm<dummy::Behaviour> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_quic()
        .with_behaviour(|_| dummy::Behaviour)
        .unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(idle))
        .build()
}

#[tokio::test]
async fn a_connection_closed_for_being_idle_is_closed_at_the_other_end_too() {
    // The dialer lets its connection go after 1 s; the listener would keep it a minute, and
    // QUIC's keep-alives (every 5 s, idle timeout 10 s) keep it up while both ends answer.
    let mut dialer = swarm(Duration::from_secs(1));
    let mut listener = swarm(Duration::from_secs(60));
    let listener_peer: PeerId = *listener.local_peer_id();
    listener
        .listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap())
        .unwrap();
    let address: Multiaddr = loop {
        if let SwarmEvent::NewListenAddr { address, .. } = listener.select_next_some().await {
            break address;
        }
    };
    dialer.dial(address).unwrap();

    // Both ends connected; then the dialer closes the connection for being idle.
    let mut connected = (false, false);
    let mut dialer_closed: Option<Instant> = None;
    let listener_closed = timeout(Duration::from_secs(30), async {
        loop {
            tokio::select! {
                event = dialer.select_next_some() => match event {
                    SwarmEvent::ConnectionEstablished { .. } => connected.0 = true,
                    SwarmEvent::ConnectionClosed { .. } => {
                        assert!(connected.0);
                        dialer_closed = Some(Instant::now());
                    }
                    _ => {}
                },
                event = listener.select_next_some() => match event {
                    SwarmEvent::ConnectionEstablished { .. } => connected.1 = true,
                    SwarmEvent::ConnectionClosed { cause, .. } => {
                        assert!(connected.1);
                        break (Instant::now(), cause);
                    }
                    _ => {}
                },
            }
        }
    })
    .await;
    let dialer_closed = dialer_closed.expect("the dialer closes its idle connection");
    // Before the listener's QUIC idle timeout (10 s) could end it: it was told.
    let (listener_closed, cause) = listener_closed.expect("the listener sees the connection close");
    let after = listener_closed.saturating_duration_since(dialer_closed);
    assert!(
        after < Duration::from_secs(3),
        "closed at the listener {after:?} after the dialer, by {cause:?}"
    );
    let cause = format!("{cause:?}");
    assert!(cause.contains("ApplicationClosed"), "{cause}");
    // The dialer's datagram registry lets go of the connection with it.
    assert!(fogata_net::datagram::link(&listener_peer).is_none());
}
