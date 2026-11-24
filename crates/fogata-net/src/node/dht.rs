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
    /// Whether the names and the public addresses were more than the DHT announces, when
    /// last looked (said once in the log each time it becomes so).
    crowded: bool,
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
        let mut opening: Vec<lan::OpeningKey> =
            friends.iter().map(|s| (lan::key_of(s), None)).collect();
        let mut groups: Vec<(&GroupId, &GroupSlot)> =
            self.groups.iter().filter(|(_, s)| !s.direct).collect();
        groups.sort_by_key(|(id, _)| id.as_bytes());
        let mut group_keys: Vec<[u8; 32]> = Vec::new();
        for (_, slot) in groups {
            opening.extend(
                slot.ctx
                    .lookup_secrets(now_ms())
                    .iter()
                    .map(|s| (lan::key_of(s), None)),
            );
            if let Some(secret) = slot.ctx.lan_secret()
                && !slot.call
            {
                group_keys.push(lan::key_of(&secret));
            }
        }
        group_keys.truncate(lan::GROUP_NAMES);
        // The other computer of a move seals its local addresses under the key both made
        // from the code, as this one does; and the device at the other end of a friend link
        // or code shown or used here may be at the same place behind another router,
        // reachable only at its local address: it knows the link or code too. Names of their
        // own, so that no group loses its name to them.
        let mut invite_keys: Vec<[u8; 32]> = Vec::new();
        if let Some(key) = self.seek_lan_key() {
            opening.push((key, None));
            invite_keys.push(key);
        }
        if let Some(f) = &self.friends {
            for (secret, only) in f.invite_secrets() {
                let key = lan::key_of(&secret);
                opening.push((key, only));
                if !invite_keys.contains(&key) {
                    invite_keys.push(key);
                }
            }
        }
        if invite_keys.len() > lan::INVITE_NAMES {
            tracing::debug!(
                keys = invite_keys.len(),
                "more links, codes and moves at once than names for them: the last left out"
            );
            invite_keys.truncate(lan::INVITE_NAMES);
        }
        let mut sealing: Vec<[u8; 32]> = own_friends.iter().map(lan::key_of).collect();
        sealing.extend(group_keys);
        sealing.extend(invite_keys);
        let mut sealed = std::collections::HashSet::new();
        sealing.retain(|key| sealed.insert(*key));
        self.lan_keys.set(opening);
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
            self.check_announced_room();
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
        self.lan_names.inputs = inputs;
        self.lan_names.names = names;
        self.check_announced_room();
        self.provided.1 = None;
    }

    /// The DHT announces a few addresses at most, the oldest going first: the names must leave
    /// room for this device's public addresses, which may come later than the names. Looked
    /// at whenever the names are, at every slot.
    fn check_announced_room(&mut self) {
        let (names, public) = (self.lan_names.names.len(), self.confirmed.len());
        let crowded = names + public > lan::ANNOUNCED_ADDRESSES;
        if crowded && !self.lan_names.crowded {
            tracing::warn!(
                names,
                public,
                "more addresses to announce than the DHT keeps: the oldest go"
            );
        }
        self.lan_names.crowded = crowded;
    }

    /// Announces this device under the rendezvous keys it looks up, older epochs' too, once
    /// it knows an address others can use: a member that missed several rotations finds it
    /// there, as it finds that member there.
    pub(super) fn refresh_announcement(&mut self, now_secs: u64) {
        if self.confirmed.is_empty() {
            return;
        }
        let mut secrets: Vec<[u8; 32]> = self
            .groups
            .values()
            .filter(|s| !s.direct)
            .flat_map(|s| s.ctx.lookup_secrets(now_secs * 1000))
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
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_friends::{CodeSecrets, FriendBook, FriendCode, FriendLink};
    use fogata_state::GroupLog;
    use libp2p::PeerId;

    use super::*;
    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{
            friends::{ShownCode, SoughtCode},
            start,
            tests::inspect,
        },
    };

    /// Seven groups, and a friend link and a code shown here and one of each used here: the
    /// friends' name, the first five groups' (in the order of their ids) and the four
    /// invites' are all sealed, whatever invites come and go; the link used here opens only
    /// its maker's names.
    #[tokio::test(flavor = "multi_thread")]
    async fn invites_never_push_a_group_out_of_the_names() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let logs: Vec<GroupLog> = (0..7)
            .map(|i| GroupLog::create(&ana, &format!("g{i}"), 1).unwrap())
            .collect();
        let (node, _events) = start(NodeConfig {
            identity: ana.clone(),
            groups: logs.into_iter().map(GroupSetup::Member).collect(),
            friends: Some(FriendBook::new(&ana, "Ana", 1).unwrap()),
            listen_port: 0,
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
        let bea = DeviceIdentity::generate().unwrap();
        let maker = PeerId::random();
        let (sealed, opening, groups) = inspect(&node, move |n| {
            n.listen_addrs = vec!["/ip4/192.168.1.2/udp/4001/quic-v1".parse().unwrap()];
            let mut ids: Vec<GroupId> = n.groups.keys().copied().collect();
            ids.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            let groups: Vec<[u8; 32]> = ids
                .iter()
                .map(|id| lan::key_of(&n.groups[id].ctx.lan_secret().unwrap()))
                .collect();
            let link = |peer: PeerId| {
                FriendLink::new(bea.public_key(), "Bea", peer.to_bytes(), Vec::new(), 1).unwrap()
            };
            let at = 1_800_000_000;
            let code = || CodeSecrets::stretch(&FriendCode::generate(at).unwrap(), at);
            let f = n.friends.as_mut().unwrap();
            f.link = Some(link(PeerId::random()));
            f.code = Some(ShownCode::lasting(code()));
            f.sought_link = Some(link(maker));
            f.sought_code = Some(SoughtCode::lasting(code()));
            n.refresh_lan_names();
            let all = n.lan_names.inputs.as_ref().unwrap().keys.clone();
            // The link shown here goes: the groups' names stay as they were.
            n.friends.as_mut().unwrap().link = None;
            n.refresh_lan_names();
            let fewer = n.lan_names.inputs.as_ref().unwrap().keys.clone();
            let opening = n.lan_keys.held();
            (vec![all, fewer], opening, groups)
        })
        .await;
        let (all, fewer) = (&sealed[0], &sealed[1]);
        assert_eq!(all.len(), lan::MAX_NAMES - 1);
        assert_eq!(&all[1..6], &groups[..5]);
        assert!(!all.contains(&groups[5]) && !all.contains(&groups[6]));
        assert_eq!(fewer.len(), all.len() - 1);
        assert_eq!(&fewer[..6], &all[..6]);
        // Every group opens names, sealed for or not; the link used here only its maker's.
        assert!(groups.iter().all(|g| opening.contains(&(*g, None))));
        let tied: Vec<_> = opening.iter().filter(|(_, only)| only.is_some()).collect();
        assert_eq!(tied.len(), 1);
        assert_eq!(tied[0].1, Some(maker));
    }

    /// A member that missed two key changes announces only under the key before both; behind
    /// NATs both ends must find each other, so a member that made those changes announces
    /// under that key too, but only for two weeks after the key that replaced it began.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_is_announced_where_members_two_rotations_behind_look() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let now = now_ms();
        let mut log = GroupLog::create(&ana, "g", now - 3).unwrap();
        log.rotate(&ana, now - 2).unwrap();
        log.rotate(&ana, now - 1).unwrap();
        let group = log.group_id();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups: vec![GroupSetup::Member(log)],
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: true,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap();
        let (soon, later) = inspect(&node, move |n| {
            n.confirmed
                .insert("/ip4/198.51.100.1/udp/4001/quic-v1".parse().unwrap());
            let keys = n.groups[&group]
                .ctx
                .newest_epoch_keys(3)
                .collect::<Vec<_>>();
            assert_eq!(keys.len(), 3);
            let (newest, first) = (*keys[0].rendezvous_secret(), *keys[2].rendezvous_secret());
            // Whether `secret` is announced at `secs`, and the newest epoch too.
            let mut announced_at = |secs: u64| {
                n.provided.1 = None;
                n.refresh_announcement(secs);
                let under = |secret| {
                    (rendezvous::announce_keys(&[secret], secs).iter())
                        .all(|k| n.provided.0.contains(k))
                };
                (under(first), under(newest))
            };
            let secs = now / 1000;
            (announced_at(secs), announced_at(secs + 15 * 24 * 3600))
        })
        .await;
        node.shutdown().await.unwrap();
        assert_eq!(
            soon,
            (true, true),
            "not announced where the member behind looks"
        );
        assert_eq!(later, (false, true), "still announced there two weeks on");
    }

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
