//! Network lab helpers (tools/netlab): never needed outside it.

use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result};
use fogata_core::Store;
use fogata_net::Multiaddr;

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
