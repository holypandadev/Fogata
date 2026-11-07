//! Two devices that start together on one local network find each other over mDNS within
//! seconds, while a third device there, started earlier and of no group of theirs, answers
//! their first queries before either is ready to answer the other's.
//!
//! On Linux each round takes some tens of milliseconds, under load under a second. On macOS
//! with its firewall on, each new sender a socket of this test binary hears from (an mDNS
//! sender, each QUIC dial between copies) waits for `socketfilterfw` to check the binary,
//! one flow after the other: about 0.4 s each for a test binary signed ad hoc, over a second
//! on a busy computer, so a round takes 2 to 5 seconds idle and up to about 12 under load
//! (measured with 10 busy processes on 10 cores). The application, signed, is checked by
//! its signing identifier in about 2 ms a flow. The bound leaves room for the test binary on
//! macOS, and still fails a round that waits for a query interval (30 s) or for mDNS to
//! tell of a peer again.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::GroupLog;

fn node(identity: Arc<DeviceIdentity>, groups: Vec<GroupLog>) -> (NodeHandle, NodeEvents) {
    fogata_net::start(NodeConfig {
        identity,
        groups: groups.into_iter().map(GroupSetup::Member).collect(),
        friends: None,
        listen_port: 0,
        bootstrap: Vec::new(),
        use_dht: false,
        use_mdns: true,
        bridge: false,
        peers: Vec::new(),
        remembered: Vec::new(),
        hidden: false,
    })
    .unwrap()
}

/// How long until `events` tells of a member connected, up to `limit`.
async fn connected_within(events: &mut NodeEvents, limit: Duration) -> Option<Duration> {
    let started = Instant::now();
    tokio::time::timeout(limit, async {
        while let Some(event) = events.recv().await {
            if matches!(event, NodeEvent::MemberConnected { .. }) {
                return Some(started.elapsed());
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// What the nodes do, with times, when FOGATA_TRACE names what to show (an env filter, such
/// as `libp2p_mdns=debug,fogata_net=debug`): where a slow round loses its time.
fn trace() {
    if let Ok(filter) = std::env::var("FOGATA_TRACE") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
            .with_test_writer()
            .try_init();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn two_devices_starting_together_find_each_other_beside_an_older_one() {
    trace();
    // Started earlier, of a group of its own: it answers every query at once.
    let stale = Arc::new(DeviceIdentity::generate().unwrap());
    let other = GroupLog::create(&stale, "other", 1).unwrap();
    let (_stale, _stale_events) = node(stale, vec![other]);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let rounds: usize = std::env::var("FOGATA_ROUNDS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(4);
    let mut took = Vec::new();
    for _ in 0..rounds {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut ours = GroupLog::create(&ana, "ours", 1).unwrap();
        ours.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let (node_a, mut events_a) = node(ana, vec![ours.clone()]);
        let (node_b, _events_b) = node(bea, vec![ours]);
        tracing::info!(a = %node_a.local_peer(), b = %node_b.local_peer(), "round started");
        took.push(connected_within(&mut events_a, Duration::from_secs(25)).await);
        node_a.shutdown().await.unwrap();
        node_b.shutdown().await.unwrap();
    }
    println!("found each other after {took:?}");
    assert!(
        took.iter()
            .all(|t| t.is_some_and(|t| t < Duration::from_secs(20))),
        "devices starting together took most of a query interval: {took:?}"
    );
}
