//! What the node does at every punch slot of the wall clock: notice a sleep or a move, keep
//! its lists bounded, announce and look up on the DHT, dial the members it misses at the same
//! moment as they dial it, and every other slot a heartbeat.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use fogata_crypto::GroupId;
use libp2p::{Multiaddr, PeerId};

use super::{
    Node, PUNCH_SLOT, candidates::RESTING_EVERY, dial::PUNCHED_WITHIN, friends::Target,
    links::THROUGH_DIAL_WAIT, now_ms,
};
use crate::{NodeEvent, addr, addr::Reach, rendezvous};

/// Peers kept at most in each list of peers that may be members or friends; past it, those
/// not connected now are forgotten. Anyone on a local network can name any number of
/// made-up peers.
const MAX_PEERS_KEPT: usize = 512;

/// DHT keys remembered to tell which group a provider was found for.
const GROUP_KEYS_LIMIT: usize = 2048;

pub(super) fn until_next_slot() -> Duration {
    let slot = PUNCH_SLOT.as_millis() as u64;
    let wait = slot - now_ms() % slot;
    // Woken a moment early: the slot after that one.
    Duration::from_millis(if wait < 500 { wait + slot } else { wait })
}

impl Node {
    pub(super) fn on_slot(&mut self) {
        self.slot_count += 1;
        let now_secs = now_ms() / 1000;
        // Much more time went by than a slot: the computer slept, or its clock was set.
        // Its network may be another one now, and what others saw of it may be old.
        let woke = self
            .last_slot_ms
            .is_some_and(|last| now_ms().abs_diff(last) > 3 * PUNCH_SLOT.as_millis() as u64);
        self.last_slot_ms = Some(now_ms());
        if woke {
            tracing::info!("woke up, or the clock moved");
            self.network_changed(true);
            if let Some(kad) = self.swarm.behaviour_mut().kad.as_mut() {
                let _ = kad.bootstrap();
                self.last_bootstrap = Instant::now();
            }
        }
        self.forget_stale_addresses();
        self.forget_extra_peers();
        self.want_proven();
        self.publish_known_addresses();
        self.trim_logs();
        self.fit_room_chat();
        if let Some(left) = self.network_pending.take() {
            self.after_network_change(left);
        }

        if self.swarm.behaviour().kad.is_enabled() {
            self.bootstrap_when_due();
            self.refresh_lan_names();
            self.refresh_announcement(now_secs);
            let due: Vec<GroupId> = self
                .groups
                .iter()
                .filter(|(_, s)| {
                    !s.direct
                        && s.last_lookup.is_none_or(|t| {
                            let relayed = s.members.iter().filter(|p| self.only_relayed(p)).count();
                            t.elapsed() >= s.lookup_interval(relayed)
                        })
                })
                .map(|(id, _)| *id)
                .collect();
            if !due.is_empty() {
                self.lookup_dials.clear();
            }
            if self.group_keys.len() > GROUP_KEYS_LIMIT {
                self.group_keys.clear();
            }
            if self.lookup_new.len() > GROUP_KEYS_LIMIT {
                self.lookup_new.clear();
            }
            if self.lookup_taken.len() > GROUP_KEYS_LIMIT {
                self.lookup_taken.clear();
            }
            if self.lookup_named.len() > GROUP_KEYS_LIMIT {
                self.lookup_named.clear();
                self.lookup_query.clear();
            }
            for group in due {
                let slot = self.groups.get_mut(&group).expect("listed above");
                slot.last_lookup = Some(Instant::now());
                slot.quiet_lookups += 1;
                let keys = rendezvous::lookup_keys(&slot.ctx.lookup_secrets(now_ms()), now_secs);
                for key in keys {
                    self.group_keys.insert(key.clone(), group);
                    self.look_up(key);
                }
            }
        }
        self.friend_slot(now_secs);
        self.seek_slot(now_secs);

        // Scheduled punch towards every known member we are not connected to. Addresses
        // behind our own public IP are left out: a punch cannot help there, and the one
        // attempt made on discovery already tells whether the router lets it through.
        // Members reached only through another member are tried too, now and then: the
        // direct way may have opened again (see `check_member_links`), and both ends try
        // in the same slots.
        let own_ips = self.own_public_ips();
        let slot = now_ms() / PUNCH_SLOT.as_millis() as u64;
        let resting_too = slot.is_multiple_of(RESTING_EVERY);
        let own = self.own_networks();
        let targets: Vec<(PeerId, Vec<Multiaddr>)> = self
            .candidates
            .iter()
            .filter(|(peer, _)| {
                if self.swarm.is_connected(peer) {
                    resting_too && self.only_relayed(peer)
                } else {
                    resting_too || !self.resting(peer)
                }
            })
            .map(|(peer, addrs)| {
                let mut addrs: Vec<Multiaddr> = addrs
                    .iter()
                    .filter(|a| addr::ip(a).is_none_or(|ip| !own_ips.contains(&ip)))
                    .cloned()
                    .collect();
                // The way back may be on a local network the two share, or one this
                // device's router leads to (two routers at home): a private IPv4 address
                // in any /24, but a carrier-grade NAT or unique-local IPv6 one only of a
                // network this device is on, never one of a VPN it is not on.
                if self.only_relayed(peer) {
                    addrs.extend(self.member_lan_for(peer, &own, Reach::SamePlace).cloned());
                }
                (*peer, addrs)
            })
            .filter(|(_, addrs)| !addrs.is_empty())
            .collect();
        for (peer, addrs) in targets {
            let _ = self.dial_candidate(peer, Some(addrs), Some(slot));
        }

        // Heartbeat: keeps connections and NAT mappings alive and refreshes rosters.
        if self.slot_count.is_multiple_of(2) {
            self.member_heartbeat();
            self.friend_heartbeat();
            self.sessions_heartbeat();
            self.room_heartbeat();
        }
    }

    /// Lets go of spare log entries past their room in every group (see
    /// [`fogata_state::GroupLog::trim`]), every slot rather than as entries come, so a sync
    /// that brings what makes one needed is never cut short; the application saves a log
    /// that changed.
    fn trim_logs(&mut self) {
        let trimmed: Vec<GroupId> = self
            .groups
            .iter_mut()
            .filter_map(|(id, slot)| slot.ctx.trim().then_some(*id))
            .collect();
        for group in trimmed {
            self.emit(NodeEvent::GroupChanged(group));
        }
    }

    /// Forgets punch dials that are over, and bounds the lists of peers that may be members
    /// or friends (see [`MAX_PEERS_KEPT`]).
    fn forget_extra_peers(&mut self) {
        self.punching.retain(|_, t| t.elapsed() < PUNCHED_WITHIN);
        self.through_dials
            .retain(|_, t| t.elapsed() < THROUGH_DIAL_WAIT);
        let (swarm, members) = (&self.swarm, &self.members);
        let live = |peer: &PeerId| swarm.is_connected(peer) || members.contains_key(peer);
        // Found under one of this device's secret keys or named by a member: no stranger
        // can make many of those, so they go only past a much larger bound.
        if self.candidates.len() > 4 * MAX_PEERS_KEPT {
            self.candidates.retain(|p, _| live(p));
        }
        if self.reached_peers.len() > 4 * MAX_PEERS_KEPT {
            let candidates = &self.candidates;
            self.reached_peers
                .retain(|p| live(p) || candidates.contains_key(p));
        }
        if self.candidate_found.len() > self.candidates.len() {
            let candidates = &self.candidates;
            self.candidate_found
                .retain(|p, _| candidates.contains_key(p));
        }
        if self.hinted.len() > 4 * MAX_PEERS_KEPT {
            self.hinted.retain(|p, _| live(p));
        }
        let (candidates, hinted) = (&self.candidates, &self.hinted);
        let friend_targets: HashSet<PeerId> = self
            .friends
            .as_ref()
            .map(|f| {
                f.targets
                    .iter()
                    .filter(|(_, t)| matches!(t, Target::Friend(_) | Target::Departing(_)))
                    .map(|(p, _)| *p)
                    .collect()
            })
            .unwrap_or_default();
        let found = |peer: &PeerId| {
            live(peer)
                || candidates.contains_key(peer)
                || hinted.contains_key(peer)
                || friend_targets.contains(peer)
        };
        if self.interesting.len() > MAX_PEERS_KEPT {
            self.interesting.retain(|p| found(p));
        }
        if self.lan_seen.len() > MAX_PEERS_KEPT {
            self.lan_seen.retain(|p| live(p));
        }
        if let Some(f) = self.friends.as_mut() {
            if f.lan.len() > MAX_PEERS_KEPT {
                f.lan.retain(|p| live(p));
            }
            // A friend's (or former friend's) announcement is still dialled while it is a
            // candidate; DHT nodes can name any number of made-up ones.
            if f.targets.len() > MAX_PEERS_KEPT {
                let candidates = &self.candidates;
                f.targets.retain(|p, t| {
                    live(p)
                        || matches!(t, Target::Friend(_) | Target::Departing(_))
                            && candidates.contains_key(p)
                });
            }
            if f.contacted.len() > MAX_PEERS_KEPT {
                f.contacted.retain(|(p, _)| live(p));
            }
        }
    }
}
