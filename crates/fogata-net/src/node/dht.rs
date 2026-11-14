//! The node on the DHT: its routing table refreshed now and then, this device announced under
//! the rendezvous keys of its groups, friends and moves with its local addresses sealed, and
//! the answers to its lookups handed to whoever looked.

use std::time::{Duration, Instant};

use fogata_crypto::GroupId;
use libp2p::{
    Multiaddr,
    kad::{self, GetProvidersOk, QueryResult, RecordKey},
    multiaddr::Protocol,
    swarm::{
        FromSwarm, NetworkBehaviour,
        behaviour::{ExternalAddrConfirmed, ExternalAddrExpired},
    },
};

use super::{GroupSlot, Node, now_ms};
use crate::{
    addr, lan,
    rendezvous::{self, AMINO_BOOTSTRAP},
};

/// How often to refresh our own announcement.
const PROVIDE_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// How often the DHT routing table is refreshed with a bootstrap: the routing table of a
/// client that only looks things up stays useful far longer than the library's five
/// minutes, and every lookup refreshes the part of it it passes through, so a table with
/// enough peers is refreshed seldom. Each bootstrap is a burst of lookups through dozens of
/// strangers; on the public DHT, every 30 minutes, they took about two fifths of an idle
/// device's CPU.
const BOOTSTRAP_HEALTHY: Duration = Duration::from_secs(2 * 60 * 60);
const BOOTSTRAP_THIN: Duration = Duration::from_secs(30 * 60);
/// Peers in the routing table under which it is thin (the DHT's replication factor).
const HEALTHY_TABLE: usize = 20;
/// How soon an announcement the DHT did not take is tried again, doubled after each
/// failure in a row up to [`ANNOUNCE_RETRY_MAX`].
const ANNOUNCE_RETRY: Duration = Duration::from_secs(15);
const ANNOUNCE_RETRY_MAX: Duration = Duration::from_secs(4 * 60);
/// How often to announce again while a friend link or code is shown or used. Each
/// announcement reaches the DHT nodes closest to the key as this device sees them; on a slow
/// uplink few of them take it, and the other side's lookups can miss it for many minutes.
const INVITE_ANNOUNCE: Duration = Duration::from_secs(60);

/// Whether the DHT is due a bootstrap, `since` the last one with `peers` in its routing
/// table.
fn bootstrap_due(since: Duration, peers: usize) -> bool {
    since
        >= if peers >= HEALTHY_TABLE {
            BOOTSTRAP_HEALTHY
        } else {
            BOOTSTRAP_THIN
        }
}

/// What this device's sealed names were made from: the port, the local addresses and the
/// keys sealed for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct NamesFrom {
    port: u16,
    ips: Vec<std::net::Ipv4Addr>,
    keys: Vec<[u8; 32]>,
    /// This device's friend rendezvous secret and the rendezvous window its proof is for.
    proof: Option<([u8; 32], u64)>,
}

/// What this device's sealed local addresses were made from, and the names.
#[derive(Default)]
pub(super) struct LanNames {
    inputs: Option<NamesFrom>,
    names: Vec<Multiaddr>,
}

impl Node {
    pub(super) fn bootstrap(&mut self, bootstrap: &[Multiaddr]) {
        let Some(kad) = self.swarm.behaviour_mut().kad.as_mut() else {
            return;
        };
        let defaults: Vec<Multiaddr>;
        let list = if bootstrap.is_empty() {
            defaults = AMINO_BOOTSTRAP
                .iter()
                .map(|a| a.parse().expect("static bootstrap address"))
                .collect();
            &defaults
        } else {
            bootstrap
        };
        for address in list {
            if let Some(Protocol::P2p(peer)) = address.iter().last() {
                kad.add_address(&peer, addr::without_peer(address));
                if addr::is_named(address) {
                    self.named_bootstrap.insert(peer);
                }
            }
        }
        if let Err(e) = kad.bootstrap() {
            tracing::warn!("DHT bootstrap failed to start: {e}");
        }
        self.last_bootstrap = Instant::now();
    }

    /// Bootstraps the DHT again when its routing table is due a refresh (see
    /// [`BOOTSTRAP_HEALTHY`]).
    pub(super) fn bootstrap_when_due(&mut self) {
        let Some(kad) = self.swarm.behaviour_mut().kad.as_mut() else {
            return;
        };
        let peers: usize = kad.kbuckets().map(|bucket| bucket.num_entries()).sum();
        if bootstrap_due(self.last_bootstrap.elapsed(), peers) {
            tracing::debug!(peers, "refreshing the DHT routing table");
            let _ = kad.bootstrap();
            self.last_bootstrap = Instant::now();
        }
    }

    /// Keeps the keys that open friends' and members' local addresses current, and this
    /// device's own sealed local addresses in its DHT announcements (see lan.rs). A name is
    /// sealed again only when what it holds or who it is for changes.
    pub(super) fn refresh_lan_names(&mut self) {
        let (own_friends, friends) = self.friend_lan_secrets();
        let mut opening: Vec<[u8; 32]> = friends.iter().map(lan::key_of).collect();
        let mut sealing: Vec<[u8; 32]> = own_friends.iter().map(lan::key_of).collect();
        let mut groups: Vec<(&GroupId, &GroupSlot)> =
            self.groups.iter().filter(|(_, s)| !s.direct).collect();
        groups.sort_by_key(|(id, _)| id.as_bytes());
        for (_, slot) in groups {
            opening.extend(slot.ctx.lookup_secrets(now_ms()).iter().map(lan::key_of));
            if let Some(secret) = slot.ctx.lan_secret()
                && !slot.call
            {
                sealing.push(lan::key_of(&secret));
            }
        }
        // The other computer of a move seals its local addresses under the key both made
        // from the code, as this one does.
        if let Some(key) = self.seek_lan_key() {
            opening.push(key);
            sealing.insert(0, key);
        }
        self.lan_keys.set(opening);
        sealing.truncate(lan::MAX_NAMES);
        // Friends dial what they find under this device's friend secret only with its
        // proof, made again for each rendezvous window.
        let proof = own_friends.map(|secret| (secret, fogata_crypto::window_at(now_ms() / 1000)));
        // Names go out even with no local address to seal (random ones), so that their
        // number says nothing.
        let inputs = match lan::local(&self.listen_addrs) {
            Some((port, ips)) => Some(NamesFrom {
                port,
                ips,
                keys: sealing,
                proof,
            }),
            None => lan::quic_port(&self.listen_addrs).map(|port| NamesFrom {
                port,
                ips: Vec::new(),
                keys: Vec::new(),
                proof,
            }),
        };
        if inputs == self.lan_names.inputs {
            return;
        }
        let local = self.local_peer();
        let names: Vec<Multiaddr> = inputs
            .as_ref()
            .map(|from| {
                let mut names = lan::names(from.port, &from.ips, &from.keys);
                names.extend(from.proof.and_then(|(secret, window)| {
                    lan::proof(&self.identity, &secret, &local, window, from.port)
                }));
                names
            })
            .unwrap_or_default();
        // Given to the DHT behaviour alone, as if confirmed: identify and the rest never
        // hear of them.
        let kad = &mut self.swarm.behaviour_mut().kad;
        for addr in &self.lan_names.names {
            kad.on_swarm_event(FromSwarm::ExternalAddrExpired(ExternalAddrExpired { addr }));
        }
        for addr in &names {
            kad.on_swarm_event(FromSwarm::ExternalAddrConfirmed(ExternalAddrConfirmed {
                addr,
            }));
        }
        tracing::debug!(names = names.len(), "local addresses sealed for the DHT");
        self.lan_names = LanNames { inputs, names };
        self.provided.1 = None;
    }

    /// Announces this device under the current rendezvous keys once it knows an address
    /// others can use.
    pub(super) fn refresh_announcement(&mut self, now_secs: u64) {
        if self.confirmed.is_empty() {
            return;
        }
        let mut secrets: Vec<[u8; 32]> = self
            .groups
            .values()
            .filter(|s| !s.direct)
            .flat_map(|s| s.ctx.rendezvous_secrets(now_secs * 1000))
            .collect();
        secrets.extend(self.friend_announce_secrets());
        secrets.extend(self.seek_announce_secrets());
        let keys = rendezvous::announce_keys(&secrets, now_secs);
        let retry = self.announce_retry.0.is_some_and(|at| Instant::now() >= at)
            || ((self.friends_inviting() || self.is_seeking())
                && self
                    .provided
                    .1
                    .is_some_and(|at| at.elapsed() >= INVITE_ANNOUNCE));
        let due = keys != self.provided.0
            || retry
            || self
                .provided
                .1
                .is_none_or(|at| at.elapsed() >= PROVIDE_INTERVAL);
        if !due || keys.is_empty() {
            return;
        }
        let kad = self.swarm.behaviour_mut().kad.as_mut().expect("enabled");
        for old in &self.provided.0 {
            if !keys.contains(old) {
                kad.stop_providing(old);
            }
        }
        for key in &keys {
            if let Err(e) = kad.start_providing(key.clone()) {
                tracing::warn!("cannot announce on the DHT: {e}");
            }
        }
        self.provided = (keys, Some(Instant::now()));
        self.announce_retry.0 = None;
    }

    /// Starts a lookup of the peers announced under `key`, afresh: what the last one of
    /// the same key took and named is forgotten.
    pub(super) fn look_up(&mut self, key: RecordKey) {
        self.lookup_new.remove(&key);
        self.lookup_taken.remove(&key);
        self.lookup_named.remove(&key);
        let kad = self.swarm.behaviour_mut().kad.as_mut().expect("enabled");
        let query = kad.get_providers(key.clone());
        self.lookup_query.insert(key, query);
    }

    pub(super) fn on_kad_event(&mut self, event: kad::Event) {
        let kad::Event::OutboundQueryProgressed { id, result, .. } = event else {
            return;
        };
        self.on_kad_answer(id, result);
    }

    /// `result` of the query `id`.
    pub(super) fn on_kad_answer(&mut self, id: kad::QueryId, result: QueryResult) {
        if self.dropped_lookups.contains(&id) {
            return;
        }
        self.answering = Some(id);
        self.on_kad_result(result);
        self.answering = None;
    }

    fn on_kad_result(&mut self, result: QueryResult) {
        match result {
            QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders { key, providers })) => {
                if self.is_seek_key(&key) {
                    self.on_seek_providers(&key, providers);
                    return;
                }
                if let Some((target, proof)) = self.friend_target(&key) {
                    self.on_friend_providers(&key, target, proof, providers);
                    return;
                }
                let Some(group) = self.group_keys.get(&key).copied() else {
                    return;
                };
                self.on_group_providers(&key, group, providers);
            }
            QueryResult::StartProviding(Ok(_)) => {
                self.announce_retry.1 = 0;
                self.info("announced on the DHT");
                if !self.announced {
                    self.announced = true;
                    self.look_again_from_the_start();
                }
            }
            QueryResult::GetProviders(Err(e)) => {
                tracing::debug!("DHT lookup incomplete: {e:?}");
            }
            QueryResult::StartProviding(Err(e)) => {
                tracing::debug!("DHT announcement incomplete: {e:?}");
                // Until the announcement is taken, a friend or member looking for this
                // device does not find it, and one that cannot be reached directly never
                // gets the chance to open its side: on a slow network the first one often
                // times out. Try again soon instead of at the next refresh, up to half an hour
                // away, waiting longer after each failure in a row.
                if self.announce_retry.0.is_none() {
                    let wait = (ANNOUNCE_RETRY * 2u32.pow(self.announce_retry.1.min(5)))
                        .min(ANNOUNCE_RETRY_MAX);
                    self.announce_retry = (Some(Instant::now() + wait), self.announce_retry.1 + 1);
                }
            }
            QueryResult::Bootstrap(Ok(_)) => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_healthy_routing_table_is_refreshed_seldom() {
        let min = Duration::from_secs(60);
        assert!(!bootstrap_due(31 * min, HEALTHY_TABLE));
        assert!(!bootstrap_due(119 * min, 200));
        assert!(bootstrap_due(120 * min, 200));
        assert!(bootstrap_due(30 * min, HEALTHY_TABLE - 1));
        assert!(!bootstrap_due(29 * min, 0));
    }
}
