//! Transport probe for real-time media: one device sends packets at a steady pace, the
//! other reports how late they arrived.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use fogata_media::probe::{self, Mode, PROBE_PROTOCOL, ProbeConfig};
use fogata_net::{GroupSetup, NodeEvent};

use fogata_core::Store;

use crate::chat::NetArgs;

#[derive(clap::Args)]
pub struct ProbeArgs {
    /// Group whose members to measure against.
    group: String,
    /// Send packets; without it, receive and report.
    #[arg(long)]
    send: bool,
    /// single, stripedN (for example striped4) or per-packet.
    #[arg(long, default_value = "single")]
    mode: Mode,
    #[arg(long, default_value_t = 1500)]
    packets: u32,
    /// Bytes per packet; about the size of an encrypted voice packet by default.
    #[arg(long, default_value_t = 160)]
    size: usize,
    #[arg(long, default_value_t = 20)]
    interval_ms: u64,
    /// Short form of the receiving device's key (`fogata id`); by default the first member
    /// that connects.
    #[arg(long)]
    to: Option<String>,
    #[command(flatten)]
    net: NetArgs,
}

pub async fn run(store: Store, args: ProbeArgs) -> Result<()> {
    let (identity, _) = store.identity()?;
    let log = store.load_log(&store.group(&args.group)?)?;
    let config = ProbeConfig {
        mode: args.mode,
        packets: args.packets,
        size: args.size,
        interval: Duration::from_millis(args.interval_ms),
    };
    let (node, mut events) = fogata_net::start(args.net.node_config(
        identity,
        vec![GroupSetup::Member(log)],
        None,
    ))?;

    // Register before any member connects, so no stream is refused.
    let incoming = if args.send {
        None
    } else {
        Some(
            node.streams()
                .accept(PROBE_PROTOCOL)
                .context("probe protocol")?,
        )
    };

    let peer = loop {
        let event = tokio::time::timeout(Duration::from_secs(240), events.recv())
            .await
            .context("no member connected within four minutes")?;
        match event {
            Some(NodeEvent::MemberConnected {
                peer, path, device, ..
            }) => {
                say!("· member {} connected ({path})", device.fingerprint());
                if args
                    .to
                    .as_ref()
                    .is_none_or(|to| *to == device.fingerprint())
                {
                    break peer;
                }
            }
            Some(NodeEvent::Info(text)) => say!("· {text}"),
            Some(_) => {}
            None => bail!("the node stopped"),
        }
    };
    // Keep reporting path changes while the probe runs.
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let NodeEvent::MemberConnected { path, .. } = event {
                say!("· member path now {path}");
            }
        }
    });

    if args.send {
        tokio::time::sleep(Duration::from_secs(2)).await;
        say!("· sending {} packets ({:?})", config.packets, config.mode);
        probe::send(node.streams(), peer, config).await?;
        say!("· done");
    } else {
        let checker = node.clone();
        let report = probe::receive(
            incoming.expect("receiver"),
            move |p| checker.is_member(p),
            config,
            Duration::from_secs(120),
        )
        .await;
        let pct = |p: f64| {
            report
                .percentile(p)
                .map_or("-".to_string(), |v| format!("{v:.1}"))
        };
        say!(
            "RESULT mode={:?} received={}/{} p50={} p95={} p99={} max={} late>100ms_or_lost={:.2}% late>200ms_or_lost={:.2}%",
            config.mode,
            report.received,
            report.expected,
            pct(0.5),
            pct(0.95),
            pct(0.99),
            pct(1.0),
            report.late_or_lost(100.0) * 100.0,
            report.late_or_lost(200.0) * 100.0
        );
    }
    Ok(())
}
