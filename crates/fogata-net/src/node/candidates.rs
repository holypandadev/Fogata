//! The peers DHT lookups name as members, friends or the other computer of a move: a few new
//! ones are taken from each answer and each lookup, made-up ones make room for real ones,
//! and those that keep failing rest and are forgotten.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use fogata_crypto::GroupId;
use libp2p::{Multiaddr, PeerId, kad::RecordKey};

use super::{Node, friends::Target};
use crate::{addr, short_peer};

/// Failed attempts before a candidate is rested, and for how long.
pub(super) const MAX_FAILURES: u32 = 6;
const REST: Duration = Duration::from_secs(10 * 60);
/// A resting candidate is still dialled at every this many slots of the wall clock. The
/// other side may only find this device after it went to rest; since both count the same
/// slots, their dials meet there, and with an odd number each dials as listener in turn.
pub(super) const RESTING_EVERY: u64 = 5;
/// New candidates taken from one DHT answer and from all the answers to one lookup: a DHT
/// node can name any number of made-up members, each with someone else's address, and every
/// one is dialled again and again.
pub(super) const MAX_NEW_PER_ANSWER: usize = 8;
const MAX_NEW_PER_LOOKUP: usize = 16;
/// New candidates one lookup may take in all, each dialled at once, counting those that
/// made room for later ones (see [`Node::admit_candidate`]): nodes on the way naming
/// made-up members take a few dials each, not one per node the lookup asks.
const MAX_TAKEN_PER_LOOKUP: usize = 4 * MAX_NEW_PER_LOOKUP;
/// Peers named by the answers to one lookup that are kept to tell one named by a second
/// answer (see [`Node::take_named`]); past it, the earliest named go first.
const MAX_NAMED_PER_LOOKUP: usize = 2 * MAX_TAKEN_PER_LOOKUP;
/// Candidates never reached kept at once for one group or friend (or kind of friend
/// request, or a move), and in all. Past either, a new one takes the place of the one there
/// that failed most (the oldest of those), unless every one there was seen on the local
/// network or named by a member, so made-up ones cannot keep a member's new session out.
/// Resting candidates do not count: they are dialled only now and then.
const MAX_UNREACHED_PER_TARGET: usize = 16;
const MAX_UNREACHED: usize = 64;
/// Rests after which a candidate is forgotten: an announcement of a session that ended
/// (they stay on the DHT for up to two days), or a member that closed the app. A later
/// lookup brings it back if it is still announced.
pub(super) const MAX_RESTS: u32 = 3;

/// What a candidate found on the DHT was looked up for (see [`MAX_UNREACHED_PER_TARGET`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Budget {
    Group(GroupId),
    Friend(Target),
    Move,
}

impl Node {
    /// Stops trying to reach a candidate that never answered; a lookup that still finds it
    /// announced makes it a candidate again.
    pub(super) fn forget_candidate(&mut self, peer: PeerId) {
        tracing::debug!(%peer, "candidate forgotten after repeated failures");
        self.candidates.remove(&peer);
        self.candidate_found.remove(&peer);
        self.dialling_found.remove(&peer);
        self.corroborated.remove(&peer);
        self.failures.remove(&peer);
        self.hinted.remove(&peer);
        self.interesting.remove(&peer);
        if let Some(f) = self.friends.as_mut() {
            f.targets.remove(&peer);
        }
    }

    /// Whether an address of another member is worth dialling from here.
    pub(super) fn usable(&self, address: &Multiaddr) -> bool {
        if addr::is_relayed(address) || addr::is_local(address) {
            return false;
        }
        match addr::ip(address) {
            // Without a public IPv6 address of our own, IPv6 destinations are unreachable.
            Some(std::net::IpAddr::V6(_)) => self.listen_addrs.iter().any(|a| {
                matches!(addr::ip(a), Some(std::net::IpAddr::V6(_))) && !addr::is_local(a)
            }),
            _ => true,
        }
    }

    pub(super) fn resting(&self, peer: &PeerId) -> bool {
        self.failures
            .get(peer)
            .and_then(|(_, since, _)| *since)
            .is_some_and(|since| since.elapsed() < REST)
    }

    /// Peers announced under `key`, one of `group`'s keys: new ones become candidates, a
    /// few at a time (see [`Self::admit_candidate`]), and are dialled.
    pub(super) fn on_group_providers(
        &mut self,
        key: &RecordKey,
        group: GroupId,
        providers: HashSet<PeerId>,
    ) {
        let local = self.local_peer();
        let mut room = MAX_NEW_PER_ANSWER;
        for peer in providers {
            if peer == local {
                continue;
            }
            let new = !self.candidates.contains_key(&peer);
            if !self.take_named(peer, Budget::Group(group), key, &mut room) {
                continue;
            }
            self.interesting.insert(peer);
            self.hinted.entry(peer).or_default().insert(group);
            self.candidates.entry(peer).or_default();
            if new {
                self.info(format!(
                    "found a member announcement ({})",
                    short_peer(&peer)
                ));
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.quiet_lookups = 0;
                }
            }
            if self.swarm.is_connected(&peer) {
                self.approach(peer, Some(group));
            } else if !self.resting(&peer) && !self.lookup_dials.contains(&peer) {
                // The DHT query still holds the announced addresses, so dial now.
                self.dial_found(peer);
            }
        }
    }

    /// Dials `peer`, just found by a lookup, once in that lookup.
    pub(super) fn dial_found(&mut self, peer: PeerId) {
        if self.dial_candidate(peer, None, None) {
            self.lookup_dials.insert(peer);
            self.dialling_found.insert(peer);
        }
    }

    /// `peer`, named by an answer to the lookup of `key` for `budget`: whether it is a
    /// candidate, already or taken now (see [`Self::admit_candidate`]). One that an earlier
    /// answer to the same lookup named too is corroborated.
    pub(super) fn take_named(
        &mut self,
        peer: PeerId,
        budget: Budget,
        key: &RecordKey,
        room: &mut usize,
    ) -> bool {
        let again = self.named_again(key, peer);
        if !self.candidates.contains_key(&peer)
            && !self.admit_candidate(peer, budget, key, room, again)
        {
            return false;
        }
        if again {
            self.corroborated.insert(peer);
        }
        true
    }

    /// Notes that an answer to the lookup of `key` named `peer`; whether an earlier answer
    /// to it did too. Each answer names a peer once, and a lookup asks each node once.
    fn named_again(&mut self, key: &RecordKey, peer: PeerId) -> bool {
        if self.answering.is_some() && self.lookup_query.get(key) != self.answering.as_ref() {
            return false;
        }
        let named = self.lookup_named.entry(key.clone()).or_default();
        if named.contains(&peer) {
            return true;
        }
        if named.len() >= MAX_NAMED_PER_LOOKUP {
            named.pop_front();
        }
        named.push_back(peer);
        false
    }

    /// Whether `peer`, found under `key` for `budget` and not a candidate yet, becomes one:
    /// at most `room` more from this answer (counted down), [`MAX_NEW_PER_LOOKUP`] from this
    /// lookup (past that, the earliest of the lookup's that was never reached, and whose
    /// dial is over, makes room, up to [`MAX_TAKEN_PER_LOOKUP`] taken in all; for one
    /// `corroborated`, the earliest only one answer named, its dial stopped), and while
    /// there is room among the unreached candidates, made by dropping one if needed (see
    /// [`MAX_UNREACHED_PER_TARGET`]). One known only from the DHT gets no local address
    /// dialled (see strangers.rs).
    pub(super) fn admit_candidate(
        &mut self,
        peer: PeerId,
        budget: Budget,
        key: &RecordKey,
        room: &mut usize,
        corroborated: bool,
    ) -> bool {
        if *room == 0 || self.lookup_taken.get(key).copied().unwrap_or(0) >= MAX_TAKEN_PER_LOOKUP {
            return false;
        }
        let found = self.lookup_new.get(key).map_or(0, Vec::len);
        // The one of this lookup's that makes room, if it must: forgotten only once the
        // newcomer is sure to be taken, not for one the bounds below refuse anyway.
        let mut making_room = None;
        if found >= MAX_NEW_PER_LOOKUP {
            // A lookup walks towards the key: nodes on the way answer first, and the nodes
            // nearest the key, which keep the announcements, last. Nodes on the way
            // naming made-up members must not use up the places before those come.
            // One being dialled may be a member whose announcement came first: made-up ones
            // named after it wait for its dial to end.
            let found = &self.lookup_new[key];
            let earlier = found
                .iter()
                .copied()
                .find(|p| self.never_reached(p) && !self.dialling_found.contains(p))
                // Made-up ones dialled at an address that answers slowly would hold the
                // places for as long as a dial may take: one that a second answer named
                // takes the place of one only one did, whose dial is stopped.
                .or_else(|| {
                    found
                        .iter()
                        .copied()
                        .filter(|_| corroborated)
                        .find(|p| self.never_reached(p) && !self.corroborated.contains(p))
                });
            let Some(gone) = earlier else {
                tracing::debug!(%peer, "left out of a lookup: its places are taken");
                return false;
            };
            making_room = Some(gone);
        }
        for (within, limit) in [
            (Some(budget), MAX_UNREACHED_PER_TARGET),
            (None, MAX_UNREACHED),
        ] {
            let mut unreached = self.unreached_candidates(within);
            unreached.retain(|p| Some(*p) != making_room);
            if unreached.len() >= limit && !self.drop_unreached(&unreached) {
                return false;
            }
        }
        if let Some(gone) = making_room {
            if self.candidates.contains_key(&gone) {
                if self.dialling_found.contains(&gone) {
                    let _ = self.swarm.disconnect_peer_id(gone);
                }
                self.forget_candidate(gone);
            }
            if let Some(found) = self.lookup_new.get_mut(key) {
                found.retain(|p| *p != gone);
            }
        }
        *room -= 1;
        self.lookup_new.entry(key.clone()).or_default().push(peer);
        *self.lookup_taken.entry(key.clone()).or_default() += 1;
        self.candidate_found.insert(peer, (budget, Instant::now()));
        if !self.lan_seen.contains(&peer) {
            self.wanted.found_on_dht(peer);
        }
        true
    }

    /// The candidates never reached and not resting, of `budget` or of every one.
    fn unreached_candidates(&self, budget: Option<Budget>) -> Vec<PeerId> {
        self.candidates
            .keys()
            .filter(|p| {
                !self.swarm.is_connected(p)
                    && !self.members.contains_key(p)
                    && !self.resting(p)
                    && budget
                        .is_none_or(|b| self.candidate_found.get(p).is_some_and(|(of, _)| *of == b))
            })
            .copied()
            .collect()
    }

    /// Whether `peer` was seen on the local network or named by a member: no DHT node can
    /// make that up, so it never makes room for a newer candidate.
    fn vouched_candidate(&self, peer: &PeerId) -> bool {
        self.lan_seen.contains(peer)
            || self
                .groups
                .values()
                .any(|s| s.rosters.values().any(|(listed, _)| listed.contains(peer)))
    }

    /// Whether `peer`, taken from a lookup, never answered: no longer a candidate, or one
    /// not connected now or before, not a member and not vouched for.
    fn never_reached(&self, peer: &PeerId) -> bool {
        !self.candidates.contains_key(peer)
            || (!self.swarm.is_connected(peer)
                && !self.members.contains_key(peer)
                && !self.reached_peers.contains(peer)
                && !self.vouched_candidate(peer))
    }

    /// Forgets one of `among`: one never connected first, of those one that only one answer
    /// to a lookup named, of those one whose dial from a lookup is over, then the one that
    /// failed most, of those the oldest found; never one seen on the local network or named
    /// by a member. Whether one was forgotten.
    fn drop_unreached(&mut self, among: &[PeerId]) -> bool {
        let failed = |peer: &PeerId| {
            self.failures
                .get(peer)
                .map_or(0, |(count, _, rests)| rests * MAX_FAILURES + count)
        };
        let found = |peer: &PeerId| self.candidate_found.get(peer).map(|(_, at)| *at);
        let Some(gone) = among
            .iter()
            .filter(|p| !self.vouched_candidate(p))
            .max_by_key(|p| {
                (
                    !self.reached_peers.contains(p),
                    !self.corroborated.contains(p),
                    !self.dialling_found.contains(p),
                    failed(p),
                    std::cmp::Reverse(found(p)),
                )
            })
            .copied()
        else {
            return false;
        };
        tracing::debug!(peer = %gone, "candidate dropped to make room for a newer one");
        self.forget_candidate(gone);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use libp2p::{core::ConnectedPoint, swarm::ConnectionId};

    use super::*;
    use crate::{
        NodeConfig,
        node::{
            start,
            tests::{inspect, lone_member},
        },
    };

    /// The candidate of a lookup that would make room for a newcomer stays when the bounds on
    /// the unreached refuse the newcomer anyway: the newcomer is left out, not both.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_candidate_stays_when_the_newcomer_it_made_room_for_is_refused() {
        let (node, group) = lone_member();
        let (kept, listed) = inspect(&node, move |n| {
            let key = RecordKey::new(b"full");
            let budget = Budget::Group(group);
            let take = |n: &mut Node, vouched: bool| {
                let peer = PeerId::random();
                n.candidates.entry(peer).or_default();
                n.candidate_found.insert(peer, (budget, Instant::now()));
                if vouched {
                    n.lan_seen.insert(peer);
                }
                peer
            };
            // The lookup's places are taken, the earliest by one never reached, which would
            // make room; every other unreached candidate is vouched for and stays.
            let gone = take(n, false);
            let mut found = vec![gone];
            for _ in 1..MAX_NEW_PER_LOOKUP {
                found.push(take(n, true));
            }
            for _ in 0..2 {
                take(n, true);
            }
            n.lookup_new.insert(key.clone(), found);
            let mut room = 1;
            let taken = n.admit_candidate(PeerId::random(), budget, &key, &mut room, false);
            assert!(!taken, "the newcomer was taken past the bounds");
            (
                n.candidates.contains_key(&gone),
                n.lookup_new[&key].contains(&gone),
            )
        })
        .await;
        assert_eq!(
            (kept, listed),
            (true, true),
            "a candidate was forgotten for nothing"
        );
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn made_up_members_on_the_dht_make_room_for_a_real_one() {
        let (node, group) = lone_member();
        // DHT nodes answer lookup after lookup with made-up members that never answer.
        let fake: Vec<PeerId> = (0..200).map(|_| PeerId::random()).collect();
        let real = PeerId::random();
        let (found, unreached, one_lookup) = inspect(&node, move |n| {
            for (i, answer) in fake.chunks(MAX_NEW_PER_ANSWER).enumerate() {
                let key = RecordKey::new(&(i as u64).to_be_bytes());
                n.on_group_providers(&key, group, answer.iter().copied().collect());
                // Each was tried and failed a few times since.
                for peer in answer {
                    n.failures.insert(*peer, (3, None, 0));
                }
            }
            let unreached = n.unreached_candidates(None).len();
            n.on_group_providers(&RecordKey::new(b"later"), group, HashSet::from([real]));
            let found = n.candidates.contains_key(&real);
            // One lookup gets only so many new ones, however many DHT nodes answer it.
            let key = RecordKey::new(b"one lookup");
            for _ in 0..10 {
                let answer = (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                n.on_group_providers(&key, group, answer);
            }
            let one_lookup = n.lookup_new.get(&key).map(Vec::len);
            (found, unreached, one_lookup)
        })
        .await;
        assert!(found, "a member's new session was left out");
        assert_eq!(unreached, MAX_UNREACHED_PER_TARGET);
        assert_eq!(one_lookup, Some(MAX_NEW_PER_LOOKUP));
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn made_up_announcements_of_a_friend_are_bounded_and_forgotten() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let book = fogata_friends::FriendBook::new(&ana, "Ana", 1).unwrap();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups: Vec::new(),
            friends: Some(book),
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let bea = DeviceIdentity::generate().unwrap().public_key();
        let (targets, candidates, from_dht) = inspect(&node, move |n| {
            // A DHT node answers every lookup of the secret Ana and Bea share with made-up
            // peers: nothing proves them, as only the two know that secret.
            for i in 0..25u64 {
                let key = RecordKey::new(&i.to_be_bytes());
                let answer = (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                n.on_friend_providers(&key, Target::Friend(bea), None, answer);
            }
            let targets: Vec<PeerId> = n
                .friends
                .as_ref()
                .unwrap()
                .targets
                .keys()
                .copied()
                .collect();
            let from_dht = targets.iter().all(|p| n.wanted.only_from_dht(p));
            // Forgotten after failing, they are no longer looked for.
            for peer in &targets {
                n.forget_candidate(*peer);
            }
            let left = n.friends.as_ref().unwrap().targets.len();
            (targets.len(), n.candidates.len(), (from_dht, left))
        })
        .await;
        assert_eq!(targets, MAX_UNREACHED_PER_TARGET);
        assert_eq!(candidates, 0);
        assert_eq!(from_dht, (true, 0));
        node.shutdown().await.unwrap();
    }

    /// A DHT lookup walks towards the key: nodes on the way answer first, and the nodes
    /// nearest the key, which keep the announcements, answer last. Two nodes on the way
    /// answering with made-up members must not use up the lookup's places before the real
    /// announcement comes.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_the_nodes_nearest_the_key_announce_gets_in_after_made_up_ones() {
        let (node, group) = lone_member();
        let real = PeerId::random();
        let (found, unreached) = inspect(&node, move |n| {
            let key = RecordKey::new(b"one lookup");
            for _ in 0..2 {
                let answer = (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                n.on_group_providers(&key, group, answer);
            }
            n.on_group_providers(&key, group, HashSet::from([real]));
            (
                n.candidates.contains_key(&real),
                n.unreached_candidates(None).len(),
            )
        })
        .await;
        assert!(found, "the member's announcement was left out");
        assert_eq!(unreached, MAX_NEW_PER_LOOKUP);
        node.shutdown().await.unwrap();
    }

    /// Made-up members found on the way make room for later ones, but one lookup still
    /// takes (and dials) only so many in all, however many nodes it asks.
    #[tokio::test(flavor = "multi_thread")]
    async fn one_lookup_dials_only_so_many_made_up_members() {
        let (node, group) = lone_member();
        let taken = inspect(&node, move |n| {
            let key = RecordKey::new(b"one long lookup");
            let mut taken = 0;
            for _ in 0..40 {
                let answer: HashSet<PeerId> =
                    (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                n.on_group_providers(&key, group, answer.clone());
                taken += answer
                    .iter()
                    .filter(|p| n.candidate_found.contains_key(p))
                    .count();
            }
            taken
        })
        .await;
        assert_eq!(taken, MAX_TAKEN_PER_LOOKUP);
        node.shutdown().await.unwrap();
    }

    /// A member announced in an early answer, whose dial is still going, is not pushed out
    /// by made-up ones that later answers of the same lookup name.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_being_dialled_keeps_its_place_in_the_lookup() {
        let (node, group) = lone_member();
        let (dialled, kept) = inspect(&node, move |n| {
            let key = RecordKey::new(b"one lookup");
            let real = PeerId::random();
            // Where the announcement says it is: an address that takes long to answer.
            n.swarm
                .add_peer_address(real, "/ip4/203.0.113.7/udp/4001/quic-v1".parse().unwrap());
            n.on_group_providers(&key, group, HashSet::from([real]));
            let dialled = n.dialling_found.contains(&real);
            for _ in 0..3 {
                let answer = (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                n.on_group_providers(&key, group, answer);
            }
            (
                dialled,
                n.candidates.contains_key(&real) && n.hinted.contains_key(&real),
            )
        })
        .await;
        assert!(dialled, "the member's dial did not start");
        assert!(kept, "made-up candidates pushed out a member being dialled");
        node.shutdown().await.unwrap();
    }

    /// Nodes on the way to a key name made-up members at addresses that answer slowly, so
    /// that the dials to them hold every place of the lookup for as long as a dial may
    /// take; the nodes nearest the key, which keep the real announcement, answer last, each
    /// naming the member. Named by a second answer, the member takes the place of a made-up
    /// one only one answer named, whose dial is stopped, and is dialled at once.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_two_answers_name_gets_in_while_made_up_ones_are_dialled() {
        let (node, group) = lone_member();
        let (first, second, places, made_up) = inspect(&node, move |n| {
            let key = RecordKey::new(b"one lookup");
            let slow: Multiaddr = "/ip4/203.0.113.7/udp/4001/quic-v1".parse().unwrap();
            let mut made_up = Vec::new();
            for _ in 0..2 {
                let answer: HashSet<PeerId> =
                    (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                for peer in &answer {
                    n.swarm.add_peer_address(*peer, slow.clone());
                }
                made_up.extend(answer.iter().copied());
                n.on_group_providers(&key, group, answer);
            }
            let real = PeerId::random();
            n.swarm
                .add_peer_address(real, "/ip4/203.0.113.8/udp/4001/quic-v1".parse().unwrap());
            n.on_group_providers(&key, group, HashSet::from([real]));
            let first = n.candidates.contains_key(&real);
            n.on_group_providers(&key, group, HashSet::from([real]));
            let second = n.candidates.contains_key(&real) && n.dialling_found.contains(&real);
            let made_up = made_up
                .iter()
                .filter(|p| n.dialling_found.contains(p))
                .count();
            (first, second, n.lookup_new[&key].len(), made_up)
        })
        .await;
        assert!(
            !first,
            "one answer naming a peer took a place being dialled"
        );
        assert!(second, "the member two answers named was left out");
        assert_eq!(places, MAX_NEW_PER_LOOKUP);
        assert_eq!(made_up, 2 * MAX_NEW_PER_ANSWER - 1);
        node.shutdown().await.unwrap();
    }

    /// A member named by two answers keeps its place, while its dial goes on, against
    /// made-up ones that two answers name too, as several nodes on the way could.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_corroborated_member_keeps_its_place_against_corroborated_made_up_ones() {
        let (node, group) = lone_member();
        let kept = inspect(&node, move |n| {
            let key = RecordKey::new(b"one lookup");
            let real = PeerId::random();
            n.swarm
                .add_peer_address(real, "/ip4/203.0.113.8/udp/4001/quic-v1".parse().unwrap());
            for _ in 0..2 {
                n.on_group_providers(&key, group, HashSet::from([real]));
            }
            for _ in 0..4 {
                let answer: HashSet<PeerId> =
                    (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                for peer in &answer {
                    n.swarm.add_peer_address(
                        *peer,
                        "/ip4/203.0.113.7/udp/4001/quic-v1".parse().unwrap(),
                    );
                }
                n.on_group_providers(&key, group, answer.clone());
                n.on_group_providers(&key, group, answer);
            }
            n.candidates.contains_key(&real) && n.dialling_found.contains(&real)
        })
        .await;
        assert!(
            kept,
            "made-up members named twice pushed out a member being dialled"
        );
        node.shutdown().await.unwrap();
    }

    /// A member that was connected once, whose dials fail now (its session restarted
    /// behind a harder router, say), is the last to make room for made-up ones that
    /// never answered anything.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_candidate_once_connected_is_the_last_to_make_room() {
        let (node, group) = lone_member();
        let (kept, unreached) = inspect(&node, move |n| {
            let real = PeerId::random();
            n.on_group_providers(&RecordKey::new(b"earlier"), group, HashSet::from([real]));
            n.on_connection_established(
                real,
                ConnectionId::new_unchecked(9001),
                &ConnectedPoint::Listener {
                    local_addr: "/ip4/203.0.113.50/udp/4001/quic-v1".parse().unwrap(),
                    send_back_addr: "/ip4/203.0.113.7/udp/4001/quic-v1".parse().unwrap(),
                },
            );
            n.on_connection_closed(real, ConnectionId::new_unchecked(9001), 0);
            n.failures.insert(real, (3, None, 0));
            // Made-up ones found since, never tried yet, fill the group's places.
            for i in 0..3u64 {
                let key = RecordKey::new(&i.to_be_bytes());
                let answer = (0..MAX_NEW_PER_ANSWER).map(|_| PeerId::random()).collect();
                n.on_group_providers(&key, group, answer);
            }
            (
                n.candidates.contains_key(&real),
                n.unreached_candidates(None).len(),
            )
        })
        .await;
        assert!(kept, "the member once connected was dropped first");
        assert_eq!(unreached, MAX_UNREACHED_PER_TARGET);
        node.shutdown().await.unwrap();
    }
}
