//! `fogata network`: what the network this device is on means for reaching friends, in plain
//! words. It talks to the public DHT for a short while, as any Fogata device does, and reads
//! what the peers it met saw.

use std::time::Duration;

use anyhow::{Result, bail};
use fogata_core::diagnose::diagnose;
use tokio::time::Instant;

use fogata_core::Store;

use crate::chat::NetArgs;

/// Longest check, and the shortest before it may stop because it has seen enough.
const LONGEST: Duration = Duration::from_secs(40);
const SHORTEST: Duration = Duration::from_secs(15);
/// Peers whose view of this device's address is enough to judge the router.
const ENOUGH_REPORTS: usize = 6;

pub async fn run(store: Store, net: NetArgs) -> Result<()> {
    let (identity, _) = store.identity()?;
    if !net.use_dht() {
        bail!("the check needs the public DHT; leave out --no-dht");
    }
    let (node, mut events) = fogata_net::start(net.node_config(identity, Vec::new(), None))?;
    println!("Checking this network; this takes up to 40 seconds...");
    let start = Instant::now();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let report = node.network_report().await?;
                let reports: usize = report.observed.iter().map(|(_, n)| n).sum();
                let elapsed = start.elapsed();
                if elapsed >= LONGEST || (elapsed >= SHORTEST && reports >= ENOUGH_REPORTS) {
                    break;
                }
            }
            event = events.recv() => {
                if event.is_none() {
                    break;
                }
            }
        }
    }
    let report = node.network_report().await?;
    println!();
    // The port the app keeps for this device, if it ever ran here.
    if let Some(command) = store
        .meta()
        .port
        .and_then(fogata_core::firewall::blocking_command)
    {
        println!(
            "[!!] This computer's firewall lets nothing in for Fogata, so a friend in the same place behind another router cannot reach it. To let Fogata in, run: {command}"
        );
    }
    for finding in diagnose(&report) {
        println!("{} {}", finding.mark(), finding.text);
    }
    Ok(())
}
