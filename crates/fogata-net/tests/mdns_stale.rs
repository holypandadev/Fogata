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
//!
//! The devices ask for and answer an mDNS service of this run's own
//! ([`fogata_net::start_on_mdns_service`]), over the computer's real interfaces: no other
//! device on the network takes part (the user's own Fogata, another run of this test, any
//! libp2p program), which would make the result depend on what else answers there, and the
//! test's devices take no part in theirs. What other devices answering does to discovery is
//! for the unit tests (fogata-net's node/mdns_dials.rs, libp2p-mdns's found.rs and
//! behaviour.rs). `FOGATA_TEST_MDNS_SHARED=1` puts the devices on libp2p's service, among
//! all the others on the network, as the application is.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::DeviceIdentity;
use fogata_net::{GroupSetup, NodeConfig, NodeEvent, NodeEvents, NodeHandle};
use fogata_state::GroupLog;

/// An mDNS service of this run's own, or none (libp2p's) with `FOGATA_TEST_MDNS_SHARED`.
fn service() -> Option<String> {
    if std::env::var_os("FOGATA_TEST_MDNS_SHARED").is_some() {
        return None;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let run = nanos ^ std::process::id().rotate_left(16);
    Some(format!("_fogata-test-{run:08x}._udp.local"))
}

fn node(
    identity: Arc<DeviceIdentity>,
    groups: Vec<GroupLog>,
    service: Option<&str>,
) -> (NodeHandle, NodeEvents) {
    let config = NodeConfig {
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
        spill_dir: None,
    };
    match service {
        Some(service) => fogata_net::start_on_mdns_service(config, service),
        None => fogata_net::start(config),
    }
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
    // GitHub's macOS computers carry no multicast at all, so no device is ever found there;
    // the workflow says so with FOGATA_TEST_NO_MULTICAST, and this test is left to the others.
    if std::env::var_os("FOGATA_TEST_NO_MULTICAST").is_some() {
        println!("skipped: FOGATA_TEST_NO_MULTICAST says this network carries no multicast");
        return;
    }
    trace();
    let service = service();
    println!("mDNS service: {}", service.as_deref().unwrap_or("libp2p's"));
    let service = service.as_deref();
    // Started earlier, of a group of its own: it answers every query at once.
    let stale = Arc::new(DeviceIdentity::generate().unwrap());
    let other = GroupLog::create(&stale, "other", 1).unwrap();
    let (_stale, _stale_events) = node(stale, vec![other], service);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let rounds: usize = std::env::var("FOGATA_ROUNDS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(4);
    let mut took = Vec::new();
    let mut held = None;
    for _ in 0..rounds {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut ours = GroupLog::create(&ana, "ours", 1).unwrap();
        ours.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let (node_a, mut events_a) = node(ana, vec![ours.clone()], service);
        let (node_b, _events_b) = node(bea, vec![ours], service);
        tracing::info!(a = %node_a.local_peer(), b = %node_b.local_peer(), "round started");
        took.push(connected_within(&mut events_a, Duration::from_secs(25)).await);
        node_a.shutdown().await.unwrap();
        node_b.shutdown().await.unwrap();
        if took.last() == Some(&None) {
            held = firewall_holds_this_binary();
            if held.is_some() {
                break;
            }
        }
    }
    println!("found each other after {took:?}");
    if let Some(held) = held {
        panic!("{held}; found each other after {took:?}");
    }
    assert!(
        took.iter()
            .all(|t| t.is_some_and(|t| t < Duration::from_secs(20))),
        "devices starting together took most of a query interval: {took:?}"
    );
}

/// Why macOS's application firewall keeps this test binary from hearing the network, if it
/// does. With the firewall on, a program it has no rule for has every packet from the network
/// held (`socketfilterfw`: "Enqueuing new inbound flow") until someone answers its dialog
/// asking whether that program may accept incoming connections, and the dialogs come one
/// after the other. A test binary is signed ad hoc under its file name, which changes with
/// what it is built from (another Cargo.lock, other features: `cargo nextest run -p fogata-net`
/// alone builds another binary than with fogata-core beside it), so each new one waits for
/// a dialog of its own, for as long as nobody is at the computer; every round then finds no
/// one. The application, signed with a Developer ID, is let in by the firewall's rule for
/// signed software, on unless the user turned it off. The firewall's own list (`--listapps`)
/// tells whether it has a rule for this binary's name. Asked only once a round found no one,
/// this never fails a test that would pass.
#[cfg(target_os = "macos")]
fn firewall_holds_this_binary() -> Option<String> {
    const FIREWALL: &str = "/usr/libexec/ApplicationFirewall/socketfilterfw";
    let ask = |arg: &str| {
        std::process::Command::new(FIREWALL)
            .arg(arg)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if ask("--getglobalstate")?.contains("State = 0") {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let name = exe.file_name()?.to_str()?;
    let apps = ask("--listapps")?;
    let mut lines = apps.lines();
    let mut known = false;
    while let Some(line) = lines.next() {
        let listed = line.split_once(" : ").map(|(_, path)| path.trim());
        if listed.is_some_and(|path| path.rsplit('/').next() == Some(name)) {
            if lines.next().is_some_and(|rule| rule.contains("Allow")) {
                return None;
            }
            known = true;
        }
    }
    Some(if known {
        format!(
            "macOS's firewall blocks incoming connections to {}",
            exe.display()
        )
    } else {
        format!(
            "macOS's firewall holds every packet from the network for {} until someone answers \
             its dialog asking whether it may accept incoming connections (one for each newly \
             built test binary, one after the other): allow it, then run the test again",
            exe.display()
        )
    })
}

#[cfg(not(target_os = "macos"))]
fn firewall_holds_this_binary() -> Option<String> {
    None
}
