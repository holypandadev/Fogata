//! Network lab helpers (tools/netlab): never needed outside it.

use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result};
use fogata_core::Store;
use fogata_net::Multiaddr;

/// Prints, for each group of this device, the DHT key it is announced under now (by the
/// rendezvous secret of its newest epoch), in hex as a lab DHT node writes it down: what the
/// lab's lying DHT nodes are made near (`lab-dht --lie-near`).
pub fn print_keys(store: Store) -> Result<()> {
    let (identity, _) = store.identity()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    for group in store.groups()? {
        let log = store.load_log(&group)?;
        if let Some(keys) = log.all_keys(&identity).values().next_back() {
            let current = fogata_net::lab_lookup_keys(keys.rendezvous_secret(), now);
            println!("{}", current[0]);
        }
    }
    Ok(())
}

/// Reads the announcements a lab DHT node wrote down (`lab-dht --records`) and tries to open
/// every sealed local-address name in them with this device's secrets: its own and its
/// friends' friend secrets, and every epoch of its groups. Prints what opens with which, and
/// how many names opened; then who announced under each friend's secret, with the public
/// addresses it gave.
pub fn open_records(store: Store, records: &Path) -> Result<()> {
    let text = std::fs::read_to_string(records)
        .with_context(|| format!("cannot read {}", records.display()))?;
    let names: BTreeSet<Multiaddr> = text
        .split_whitespace()
        .filter_map(|word| word.parse::<Multiaddr>().ok())
        .filter(fogata_net::lab_is_sealed)
        .collect();

    let (identity, _) = store.identity()?;
    let mut secrets: Vec<(String, [u8; 32])> = Vec::new();
    if let Some(book) = store.friends()? {
        secrets.push(("own friend secret".into(), book.secret.rendezvous()));
        for friend in book.friends.values() {
            let who = friend.profile.name.clone();
            secrets.push((format!("friend {who}"), friend.secret.rendezvous()));
        }
    }
    for group in store.groups()? {
        let log = store.load_log(&group)?;
        for (epoch, keys) in log.all_keys(&identity) {
            let label = format!("group {} epoch {epoch}", group.name);
            secrets.push((label, *keys.rendezvous_secret()));
        }
    }

    let mut opened = 0;
    for name in &names {
        let found = secrets.iter().find_map(|(label, secret)| {
            let inside = fogata_net::lab_open_sealed(name, secret);
            (!inside.is_empty()).then_some((label, inside))
        });
        if let Some((label, inside)) = found {
            opened += 1;
            let inside: Vec<String> = inside.iter().map(|a| a.to_string()).collect();
            println!("opened with {label}: {name} -> {}", inside.join(" "));
        }
    }
    println!(
        "sealed names: {}, opened: {opened}, secrets tried: {}",
        names.len(),
        secrets.len()
    );

    // Who announced where this device looks for its friends: under a friend's own secret,
    // anyone else than that friend is another friend of it.
    if let Some(book) = store.friends()? {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        for friend in book.friends.values() {
            let keys = fogata_net::lab_lookup_keys(&friend.secret.rendezvous(), now);
            for line in text.lines() {
                let mut words = line.split_whitespace();
                let (Some(key), Some(provider)) = (words.next(), words.next()) else {
                    continue;
                };
                if keys.iter().any(|k| k == key) {
                    let addresses: Vec<&str> = words.filter(|w| w.starts_with("/ip")).collect();
                    println!(
                        "announced under friend {}: {provider} {}",
                        friend.profile.name,
                        addresses.join(" ")
                    );
                }
            }
        }
    }
    Ok(())
}

/// `lab-datagrams`: what a call's media goes as, measured. Once a member connects, sends it
/// a datagram every `interval_ms` (the unreliable QUIC datagrams a call's sound and picture
/// go in), takes the member's, and says how long the longest wait between two received
/// ones was: a socket that stops sending for a while shows here as a long gap.
#[derive(clap::Args)]
pub struct DatagramArgs {
    /// Group whose members to send to.
    group: String,
    /// Milliseconds between two datagrams sent, as a voice call's 20 ms.
    #[arg(long, default_value_t = 20)]
    interval_ms: u64,
    /// Bytes in each, about an encrypted voice packet.
    #[arg(long, default_value_t = 160)]
    size: usize,
    /// How long to send and listen once the member connected.
    #[arg(long, default_value_t = 60)]
    seconds: u64,
    #[command(flatten)]
    net: crate::chat::NetArgs,
}

pub async fn datagrams(store: Store, args: DatagramArgs) -> Result<()> {
    use std::time::{Duration, Instant};

    use anyhow::bail;
    use fogata_net::{GroupSetup, NodeEvent, datagram};
    use futures::StreamExt;

    let (identity, _) = store.identity()?;
    let log = store.load_log(&store.group(&args.group)?)?;
    let (node, mut events) = fogata_net::start(args.net.node_config(
        identity,
        vec![GroupSetup::Member(log)],
        None,
    ))?;
    let checker = node.clone();
    let mut received = datagram::receive(1024, move |peer| checker.is_member(peer));

    let peer = loop {
        let event = tokio::time::timeout(Duration::from_secs(120), events.recv())
            .await
            .context("no member connected within two minutes")?;
        match event {
            Some(NodeEvent::MemberConnected { peer, path, .. }) => {
                say!("member connected ({path})");
                break peer;
            }
            Some(NodeEvent::Info(text)) => say!("· {text}"),
            Some(_) => {}
            None => bail!("the node stopped"),
        }
    };
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            if let NodeEvent::Info(text) = event {
                say!("· {text}");
            }
        }
    });

    let interval = Duration::from_millis(args.interval_ms.max(1));
    let size = args.size.max(16);
    let end = tokio::time::Instant::now() + Duration::from_secs(args.seconds);
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut report = tokio::time::interval(Duration::from_secs(5));
    let (mut sent, mut unsent, mut got) = (0u64, 0u64, 0u64);
    let mut last: Option<Instant> = None;
    let mut longest = Duration::ZERO;
    let mut longest_at = String::new();
    let mut seq = 0u64;
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(end) => break,
            _ = tick.tick() => {
                seq += 1;
                let mut packet = vec![0u8; size];
                packet[..8].copy_from_slice(&seq.to_be_bytes());
                match datagram::link(&peer) {
                    Some(link) if link.send(packet.into()).is_ok() => sent += 1,
                    _ => unsent += 1,
                }
            }
            Some((from, _)) = received.next() => {
                if from != peer {
                    continue;
                }
                let now = Instant::now();
                got += 1;
                if let Some(before) = last {
                    let gap = now - before;
                    if gap > longest {
                        longest = gap;
                        longest_at = crate::clock();
                    }
                }
                last = Some(now);
            }
            _ = report.tick() => {
                say!(
                    "datagrams: sent {sent}, not sent {unsent}, received {got}, longest gap {} ms",
                    longest.as_millis()
                );
            }
        }
    }
    println!(
        "RESULT sent={sent} unsent={unsent} received={got} longest_gap_ms={} at={longest_at}",
        longest.as_millis()
    );
    node.shutdown().await?;
    Ok(())
}
