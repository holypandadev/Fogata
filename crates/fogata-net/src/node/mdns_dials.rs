//! Dials the peers mDNS finds on the local network: a few new ones from each address that
//! tells of them, each once at every address it was told of, and again a few times at its
//! own addresses when every dial failed.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use libp2p::{
    Multiaddr, PeerId,
    swarm::{
        ConnectionId,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use super::{Node, PUNCH_ATTEMPT};
use crate::addr;

/// Addresses of one peer dialled at once for what mDNS told of it, over every dial still
/// going; those it tells of past that wait for the dials to end. mDNS keeps 8 of a peer's
/// addresses, and a neighbour may name it at made-up ones too.
const MAX_MDNS_DIALLED: usize = 16;
/// A peer mDNS found whose every dial failed is dialled again, this many times, this long
/// after each failure. mDNS tells of an address once, until it expires (a minute): without
/// this, a first attempt lost (macOS's firewall holds the first packets from each new
/// sender while it checks it, longer than an attempt lasts on a busy computer; a copy not
/// listening yet) waited for that. Only at the addresses its own answer gave (sent from
/// them), each try counted on that answer's share of new peers (`Limits::take_mdns_peer`),
/// and never for an answer from an address not heard now: a neighbour cannot make this
/// device dial anyone else more than once.
const MDNS_RETRIES: u8 = 2;
const MDNS_RETRY_AFTER: Duration = Duration::from_secs(2);

/// The dials mDNS findings started for one peer that have not ended, the addresses they
/// were given, and those told of since past [`MAX_MDNS_DIALLED`], for when they end.
#[derive(Default)]
pub(super) struct MdnsDials {
    pending: HashSet<ConnectionId>,
    dialled: Vec<Multiaddr>,
    later: Vec<Multiaddr>,
    /// The addresses dialled since the first of these dials that a response sent from
    /// that very address told of (what looks like the peer's own answer), with that
    /// address, at most [`MAX_MDNS_DIALLED`]: the only ones dialled again should they all
    /// fail (see [`MDNS_RETRIES`]).
    own: Vec<(std::net::IpAddr, Multiaddr)>,
}

/// A peer mDNS found whose dials all failed, to dial again (see [`MDNS_RETRIES`]).
pub(super) struct MdnsRetry {
    /// Its own addresses, each with the address of the response that told of it.
    own: Vec<(std::net::IpAddr, Multiaddr)>,
    /// Tries left.
    left: u8,
    /// When the next one is due.
    at: Instant,
}

impl Node {
    /// Peers mDNS found on the local network, with their addresses: new ones are dialled,
    /// a few at a time from each address that told of them.
    pub(super) fn on_mdns_discovered(&mut self, found: Vec<(PeerId, Multiaddr)>) {
        let Some(mdns) = self.swarm.behaviour().mdns.as_ref() else {
            return;
        };
        let told: Vec<_> = found
            .into_iter()
            .map(|(peer, address)| {
                let by = mdns.told_by(&peer, &address);
                (peer, address, by)
            })
            .collect();
        self.dial_mdns_found(told);
    }

    /// Dials the peers mDNS found, each counted by the address of a response that told of
    /// it ([`Self::mdns_teller_lets_in`]): the first neighbour to name a peer, whatever it
    /// said of it, spends neither the peer's own answers nor its dial. A peer is dialled
    /// once, at the addresses every response gave, those of what looks like its own answer
    /// (one sent from an address it names) first: a dial of its own per response would let
    /// a neighbour's, at made-up addresses, hold the swarm from dialling the real one until
    /// it fails, and mDNS tells of an address it already holds only once. What later events
    /// tell of it while that dial goes on is dialled beside it ([`Self::dial_mdns_peer`]).
    pub(super) fn dial_mdns_found(
        &mut self,
        found: Vec<(PeerId, Multiaddr, Option<std::net::IpAddr>)>,
    ) {
        type Told = Vec<(std::net::IpAddr, Vec<Multiaddr>)>;
        let mut by_peer: HashMap<PeerId, Told> = HashMap::new();
        for (peer, address, by) in found {
            // One whose response is no longer known (it made room for others at once) is
            // not dialled at what it told, nor one from an address not heard for a while.
            let Some(by) = by.filter(|by| !self.teller_muted(by)) else {
                continue;
            };
            if peer == self.local_peer() {
                continue;
            }
            let tellers = by_peer.entry(peer).or_default();
            match tellers.iter_mut().find(|(teller, _)| *teller == by) {
                Some((_, addresses)) => addresses.push(address),
                None => tellers.push((by, vec![address])),
            }
        }
        for (peer, mut tellers) in by_peer {
            tellers.sort_by_key(|(by, told)| !told.iter().any(|a| addr::ip(a) == Some(*by)));
            // Counted on the first response that lets it in; once in, the peer is dialled
            // at what every response said, which costs nothing more.
            if !tellers
                .iter()
                .any(|(by, _)| self.mdns_teller_lets_in(peer, *by))
            {
                continue;
            }
            // What each response told of at its own address: the peer's own answer.
            let tellers_own: Vec<(std::net::IpAddr, Multiaddr)> = tellers
                .iter()
                .flat_map(|(by, told)| {
                    told.iter()
                        .filter(|a| addr::ip(a) == Some(*by) && addr::is_quic(a))
                        .map(|a| (*by, a.clone()))
                })
                .collect();
            // Over QUIC only when it can (media needs it), per response: one response's
            // QUIC addresses do not leave out another's.
            let lists: Vec<Vec<Multiaddr>> = tellers
                .into_iter()
                .map(|(_, told)| {
                    let quic: Vec<Multiaddr> =
                        told.iter().filter(|a| addr::is_quic(a)).cloned().collect();
                    if quic.is_empty() { told } else { quic }
                })
                .collect();
            // Each response's first address before any one's second: however many a
            // neighbour names, the peer's own come among the first the swarm dials at once
            // (8). Whose answer is the peer's own cannot be known for sure, since a neighbour
            // may name made-up addresses at its own.
            let own: Vec<(std::net::IpAddr, Multiaddr)> = tellers_own;
            let mut addresses: Vec<Multiaddr> = Vec::new();
            let longest = lists.iter().map(Vec::len).max().unwrap_or(0);
            for i in 0..longest {
                for address in lists.iter().filter_map(|l| l.get(i)) {
                    if !addresses.contains(address) {
                        addresses.push(address.clone());
                    }
                }
            }
            self.dial_mdns_peer(peer, addresses, own);
        }
    }

    /// Whether mDNS may dial `peer`, which a response from `from` told of: not when that
    /// address told of too many new peers lately.
    fn mdns_teller_lets_in(&mut self, peer: PeerId, from: std::net::IpAddr) -> bool {
        // Anyone on the network can make up any number of peers, with any addresses: only
        // a few new ones from each address that answers are dialled. They are not wanted
        // (kept, never closed for others) until they prove something. Copies on this very
        // computer (tests, a second profile) are not limited.
        let this_computer = self.listen_addrs.iter().any(|a| addr::ip(a) == Some(from));
        if this_computer
            || self.lan_seen.contains(&peer)
            || self.swarm.is_connected(&peer)
            || self.limits.take_mdns_peer(from, Instant::now())
        {
            return true;
        }
        tracing::debug!(%peer, ?from, "too many new peers on the local network");
        false
    }

    /// Dials `peer`, found by mDNS at `addresses` ([`Self::dial_mdns_found`]). While an
    /// earlier dial of it is still going, only at those that one was not given, in a dial of
    /// their own: mDNS tells of a peer's addresses as its responses come, and a neighbour's
    /// at made-up ones may come first, in an event of its own; the dial at those must not
    /// keep the peer's own from being tried, as mDNS tells of each only once. Past
    /// [`MAX_MDNS_DIALLED`] addresses at once, the rest are dialled once those dials end.
    fn dial_mdns_peer(
        &mut self,
        peer: PeerId,
        addresses: Vec<Multiaddr>,
        own: Vec<(std::net::IpAddr, Multiaddr)>,
    ) {
        self.interesting.insert(peer);
        self.wanted.found_nearby(&peer);
        self.lan_seen.insert(peer);
        self.friend_lan_peer(peer);
        if self.swarm.is_connected(&peer) {
            self.mdns_dialling.remove(&peer);
            return;
        }
        let dials = self.mdns_dialling.entry(peer).or_default();
        // Kept for a retry: the peer's own addresses first heard come first.
        for told in own {
            if dials.own.len() < MAX_MDNS_DIALLED && !dials.own.iter().any(|o| o.1 == told.1) {
                dials.own.push(told);
            }
        }
        let mut new: Vec<Multiaddr> = Vec::new();
        for address in addresses {
            if !dials.dialled.contains(&address) && !new.contains(&address) {
                new.push(address);
            }
        }
        let room = MAX_MDNS_DIALLED.saturating_sub(dials.dialled.len());
        for address in new.split_off(room.min(new.len())) {
            if !dials.later.contains(&address) {
                // The newest kept: mDNS itself keeps only so many of a peer's.
                if dials.later.len() == MAX_MDNS_DIALLED {
                    dials.later.remove(0);
                }
                dials.later.push(address);
            }
        }
        dials.later.retain(|a| !new.contains(a));
        dials.dialled.extend(new.iter().cloned());
        self.start_mdns_dial(peer, new);
    }

    /// Starts a dial of `peer` at `addresses` for mDNS, beside any going on, and keeps it
    /// with the others ([`Self::mdns_dialling`]) until it ends.
    fn start_mdns_dial(&mut self, peer: PeerId, addresses: Vec<Multiaddr>) {
        if !addresses.is_empty() {
            let opts = DialOpts::peer_id(peer)
                .addresses(addresses.clone())
                .condition(PeerCondition::Disconnected)
                .build();
            let id = opts.connection_id();
            tracing::debug!(%peer, ?addresses, "dialling a peer mDNS found");
            if self.swarm.dial(opts).is_ok() {
                #[cfg(test)]
                self.mdns_dials.push((peer, addresses));
                if let Some(dials) = self.mdns_dialling.get_mut(&peer) {
                    dials.pending.insert(id);
                }
            }
        }
        if self
            .mdns_dialling
            .get(&peer)
            .is_some_and(|d| d.pending.is_empty())
        {
            self.mdns_dialling.remove(&peer);
        }
    }

    /// A dial mDNS started failed: once the last of a peer's has, it is dialled at what
    /// mDNS told of meanwhile past [`MAX_MDNS_DIALLED`], if anything.
    pub(super) fn mdns_dial_failed(&mut self, peer: PeerId, connection: ConnectionId) {
        let Some(dials) = self.mdns_dialling.get_mut(&peer) else {
            return;
        };
        tracing::debug!(%peer, "a dial of a peer mDNS found failed");
        if !dials.pending.remove(&connection) || !dials.pending.is_empty() {
            return;
        }
        let later = std::mem::take(&mut dials.later);
        dials.dialled = later.clone();
        if later.is_empty() {
            let own = std::mem::take(&mut dials.own);
            self.all_mdns_dials_failed(peer, own);
        }
        self.start_mdns_dial(peer, later);
    }

    /// Every dial of `peer` that mDNS found failed: it is dialled again at its `own`
    /// addresses a moment later, while tries are left (see [`MDNS_RETRIES`]).
    fn all_mdns_dials_failed(&mut self, peer: PeerId, own: Vec<(std::net::IpAddr, Multiaddr)>) {
        let left = self
            .mdns_retry
            .get(&peer)
            .map_or(MDNS_RETRIES, |retry| retry.left);
        if left == 0 || own.is_empty() {
            self.mdns_retry.remove(&peer);
            return;
        }
        self.mdns_retry.insert(
            peer,
            MdnsRetry {
                own,
                left,
                at: Instant::now() + MDNS_RETRY_AFTER,
            },
        );
    }

    /// Dials again the peers mDNS found whose dials all failed, once their time came (see
    /// [`MDNS_RETRIES`]); those connected meanwhile, or dialled anew for what mDNS told,
    /// are left.
    pub(super) fn retry_mdns_dials(&mut self) {
        let now = Instant::now();
        let due: Vec<PeerId> = self
            .mdns_retry
            .iter()
            .filter(|(_, retry)| retry.at <= now)
            .map(|(peer, _)| *peer)
            .collect();
        for peer in due {
            if self.swarm.is_connected(&peer) || self.mdns_dialling.contains_key(&peer) {
                self.mdns_retry.remove(&peer);
                continue;
            }
            let Some(retry) = self.mdns_retry.get_mut(&peer) else {
                continue;
            };
            if retry.left == 0 {
                self.mdns_retry.remove(&peer);
                continue;
            }
            retry.left -= 1;
            // Not due again unless this one fails too.
            retry.at = now + PUNCH_ATTEMPT * 10;
            let own = retry.own.clone();
            // Only what an answer heard now said, each try on its share (see
            // `MDNS_RETRIES`); copies on this same computer are not limited.
            let teller = own[0].0;
            let this_computer = self
                .listen_addrs
                .iter()
                .any(|a| addr::ip(a) == Some(teller));
            if self.teller_muted(&teller)
                || !(this_computer || self.limits.take_mdns_peer(teller, now))
            {
                tracing::debug!(%peer, %teller, "a peer mDNS found is not dialled again");
                self.mdns_retry.remove(&peer);
                continue;
            }
            tracing::debug!(%peer, "dialling again a peer mDNS found");
            let addresses: Vec<Multiaddr> = own.iter().map(|(_, a)| a.clone()).collect();
            let dials = self.mdns_dialling.entry(peer).or_default();
            dials.dialled = addresses.clone();
            dials.own = own;
            self.start_mdns_dial(peer, addresses);
            // A dial that could not even start ended at once: as if it failed.
            if !self.mdns_dialling.contains_key(&peer)
                && let Some(retry) = self.mdns_retry.get_mut(&peer)
            {
                if retry.left == 0 {
                    self.mdns_retry.remove(&peer);
                } else {
                    retry.at = now + MDNS_RETRY_AFTER;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use libp2p::core::ConnectedPoint;

    use super::*;
    use crate::node::{
        limits,
        tests::{inspect, lone_member},
    };

    /// The peers mDNS tells of are counted by the address the response came from, not by
    /// the addresses it names, which are whatever its sender likes: a neighbour naming
    /// made-up peers at a new address each gets only its own few, and one whose response
    /// is no longer known is not dialled.
    #[tokio::test(flavor = "multi_thread")]
    async fn peers_mdns_tells_of_are_counted_by_who_told() {
        let (node, _group) = lone_member();
        let (taken, unknown) = inspect(&node, |n| {
            let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
            let taken = (0..100u32)
                .filter(|i| {
                    let peer = PeerId::random();
                    let at = format!("/ip4/192.168.{}.{}/udp/4001/quic-v1", i / 200, 2 + i % 200);
                    n.dial_mdns_found(vec![(peer, at.parse().unwrap(), Some(neighbour))]);
                    n.lan_seen.contains(&peer)
                })
                .count();
            let peer = PeerId::random();
            n.dial_mdns_found(vec![(
                peer,
                "/ip4/192.168.1.20/udp/4001/quic-v1".parse().unwrap(),
                None,
            )]);
            (taken, n.lan_seen.contains(&peer))
        })
        .await;
        assert_eq!(taken, limits::MDNS_PEER_BURST as usize);
        assert!(!unknown);
        node.shutdown().await.unwrap();
    }

    /// A neighbour that named a peer first and spent its own share on made-up peers does
    /// not keep that peer from being dialled at the address its own answer gave: that dial
    /// is counted by the peer's answer.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_peer_mdns_tells_of_is_dialled_on_the_share_of_whoever_told_of_its_address() {
        let (node, _group) = lone_member();
        let dialled = inspect(&node, |n| {
            let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
            let honest = std::net::IpAddr::from([192, 168, 1, 20]);
            let at = |s: &str| s.parse::<Multiaddr>().unwrap();
            for i in 0..100u32 {
                let made_up = format!("/ip4/192.168.1.66/udp/{}/quic-v1", 5000 + i);
                n.dial_mdns_found(vec![(PeerId::random(), at(&made_up), Some(neighbour))]);
            }
            let friend = PeerId::random();
            n.dial_mdns_found(vec![
                (
                    friend,
                    at("/ip4/192.168.1.66/udp/4001/quic-v1"),
                    Some(neighbour),
                ),
                (
                    friend,
                    at("/ip4/192.168.1.20/udp/4001/quic-v1"),
                    Some(honest),
                ),
            ]);
            n.lan_seen.contains(&friend)
        })
        .await;
        assert!(
            dialled,
            "the neighbour's share kept the peer from being dialled"
        );
        node.shutdown().await.unwrap();
    }

    /// A peer that mDNS tells of in the responses of two addresses in one go, a
    /// neighbour's naming it at made-up addresses and its own, is dialled once with the
    /// addresses of both, its own answer's first: a dial at the made-up ones alone would
    /// keep the swarm from dialling the real one until it fails, and mDNS tells of an
    /// address it holds only once.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_peer_mdns_tells_of_twice_at_once_is_dialled_once_at_every_address() {
        let (node, _group) = lone_member();
        let dials = inspect(&node, |n| {
            let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
            let at = |s: &str| s.parse::<Multiaddr>().unwrap();
            (0..32u8)
                .map(|i| {
                    let friend = PeerId::random();
                    let honest = std::net::IpAddr::from([192, 168, 2, 20 + i]);
                    let own = at(&format!("/ip4/{honest}/udp/4001/quic-v1"));
                    // Made up at the neighbour's own address, or at another; several.
                    let host = if i % 4 < 2 {
                        "192.168.1.66"
                    } else {
                        "192.168.1.77"
                    };
                    let made_up: Vec<Multiaddr> = (0..10)
                        .map(|port| at(&format!("/ip4/{host}/udp/{}/quic-v1", 5000 + port)))
                        .collect();
                    // Either response may come first in the batch.
                    let theirs = made_up.iter().map(|a| (friend, a.clone(), Some(neighbour)));
                    let ours = (friend, own.clone(), Some(honest));
                    let batch: Vec<_> = if i % 2 == 0 {
                        theirs.chain([ours]).collect()
                    } else {
                        [ours].into_iter().chain(theirs).collect()
                    };
                    n.dial_mdns_found(batch);
                    let dials: Vec<_> = n
                        .mdns_dials
                        .iter()
                        .filter(|(p, _)| *p == friend)
                        .map(|(_, a)| a.clone())
                        .collect();
                    (dials, own, made_up, host == "192.168.1.66")
                })
                .collect::<Vec<_>>()
        })
        .await;
        // The peer's own answer is counted first, so the neighbour's share, spent or not,
        // changes nothing; its own address is among the first two dialled, the very first
        // when the neighbour's made-up ones are not at its own.
        for (dials, own, made_up, at_neighbour) in dials {
            let [addresses] = &dials[..] else {
                panic!("dialled as {dials:?}");
            };
            let place = addresses.iter().position(|a| *a == own);
            let first = if at_neighbour { 2 } else { 1 };
            assert!(place.is_some_and(|p| p < first), "dialled as {dials:?}");
            assert_eq!(addresses.len(), 1 + made_up.len());
        }
        node.shutdown().await.unwrap();
    }

    /// A neighbour names a peer at made-up addresses in one mDNS event, and the peer's own
    /// answer comes in the next: it is dialled at its own address at once, beside the dial
    /// at the made-up ones, which is still going. Past the addresses dialled at once, what
    /// comes waits for those dials to fail, and is dialled then.
    /// A peer mDNS found whose dials all failed (a first attempt lost while macOS's firewall
    /// checked the new sender, a copy not listening yet) is dialled again a few times, a
    /// moment after each failure, at the addresses its own answers gave over the round (not
    /// only the last dial's); once connected, or past the tries, no more.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_peer_mdns_found_whose_dials_failed_is_dialled_again_a_few_times() {
        let (node, _group) = lone_member();
        let (dials, retried_after_connect) = inspect(&node, |n| {
            let peer = PeerId::random();
            let by = std::net::IpAddr::from([192, 168, 2, 20]);
            let own: Multiaddr = "/ip4/192.168.2.20/udp/4001/quic-v1".parse().unwrap();
            let second: Multiaddr = "/ip4/192.168.2.20/udp/4002/quic-v1".parse().unwrap();
            let fail_all = |n: &mut Node, peer: PeerId| {
                let pending: Vec<ConnectionId> = n
                    .mdns_dialling
                    .get(&peer)
                    .map(|d| d.pending.iter().copied().collect())
                    .unwrap_or_default();
                for id in pending {
                    n.mdns_dial_failed(peer, id);
                }
            };
            // Told of in two events, the second while the first dial goes on.
            n.dial_mdns_found(vec![(peer, own.clone(), Some(by))]);
            n.dial_mdns_found(vec![(peer, second.clone(), Some(by))]);
            for _ in 0..MDNS_RETRIES + 2 {
                fail_all(n, peer);
                // Not before its time.
                n.retry_mdns_dials();
                if let Some(retry) = n.mdns_retry.get_mut(&peer) {
                    retry.at = Instant::now();
                }
                n.retry_mdns_dials();
            }
            let dials: Vec<Vec<Multiaddr>> = n
                .mdns_dials
                .iter()
                .filter(|(p, _)| *p == peer)
                .map(|(_, a)| a.clone())
                .collect();
            assert!(!n.mdns_retry.contains_key(&peer));

            // Another, connected after a failure: not dialled again.
            let other = PeerId::random();
            n.dial_mdns_found(vec![(other, own.clone(), Some(by))]);
            fail_all(n, other);
            assert!(n.mdns_retry.contains_key(&other));
            n.on_connection_established(
                other,
                ConnectionId::new_unchecked(77),
                &ConnectedPoint::Dialer {
                    address: own.clone(),
                    role_override: libp2p::core::Endpoint::Dialer,
                    port_use: libp2p::core::transport::PortUse::Reuse,
                },
            );
            (dials, n.mdns_retry.contains_key(&other))
        })
        .await;
        let own: Multiaddr = "/ip4/192.168.2.20/udp/4001/quic-v1".parse().unwrap();
        let second: Multiaddr = "/ip4/192.168.2.20/udp/4002/quic-v1".parse().unwrap();
        assert_eq!(
            dials.len(),
            2 + MDNS_RETRIES as usize,
            "dialled as {dials:?}"
        );
        // The retries: both of its own addresses, the first heard first.
        for retry in &dials[2..] {
            assert_eq!(*retry, [own.clone(), second.clone()]);
        }
        assert!(!retried_after_connect);
        node.shutdown().await.unwrap();
    }

    /// A retry is never made for what a neighbour said of a peer at someone else's
    /// address, nor for an answer from an address not heard now, nor past the teller's
    /// share; a retry whose dial cannot even start counts as a try, and the last one ends
    /// it (no tries left to count below zero).
    #[tokio::test(flavor = "multi_thread")]
    async fn retries_are_only_for_a_peer_s_own_answer_and_end() {
        let (node, _group) = lone_member();
        inspect(&node, |n| {
            let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
            let honest = std::net::IpAddr::from([192, 168, 2, 20]);
            let at = |s: &str| s.parse::<Multiaddr>().unwrap();
            let fail_all = |n: &mut Node, peer: PeerId| {
                let pending: Vec<ConnectionId> = n
                    .mdns_dialling
                    .get(&peer)
                    .map(|d| d.pending.iter().copied().collect())
                    .unwrap_or_default();
                for id in pending {
                    n.mdns_dial_failed(peer, id);
                }
            };
            // A neighbour names a peer at another computer's address: no retry.
            let named = PeerId::random();
            n.dial_mdns_found(vec![(
                named,
                at("/ip4/192.168.2.20/udp/4001/quic-v1"),
                Some(neighbour),
            )]);
            fail_all(n, named);
            assert!(!n.mdns_retry.contains_key(&named));

            // Its own answer, but from an address not heard now by the time it is due.
            let muted = PeerId::random();
            n.dial_mdns_found(vec![(
                muted,
                at("/ip4/192.168.2.20/udp/4001/quic-v1"),
                Some(honest),
            )]);
            fail_all(n, muted);
            assert!(n.mdns_retry.contains_key(&muted));
            n.muted_tellers
                .insert(honest, Instant::now() + Duration::from_secs(60));
            n.mdns_retry.get_mut(&muted).unwrap().at = Instant::now();
            let before = n.mdns_dials.len();
            n.retry_mdns_dials();
            assert_eq!(n.mdns_dials.len(), before);
            assert!(!n.mdns_retry.contains_key(&muted));
            n.muted_tellers.clear();

            // A retry whose dial cannot start (this node itself): each counts as a try, and
            // the last ends it.
            let me = n.local_peer();
            n.mdns_retry.insert(
                me,
                MdnsRetry {
                    own: vec![(honest, at("/ip4/192.168.2.20/udp/4001/quic-v1"))],
                    left: 2,
                    at: Instant::now(),
                },
            );
            for _ in 0..4 {
                if let Some(retry) = n.mdns_retry.get_mut(&me) {
                    retry.at = Instant::now();
                }
                n.retry_mdns_dials();
            }
            assert!(!n.mdns_retry.contains_key(&me));
            // One left at none ends without a dial.
            n.mdns_retry.insert(
                me,
                MdnsRetry {
                    own: vec![(honest, at("/ip4/192.168.2.20/udp/4001/quic-v1"))],
                    left: 0,
                    at: Instant::now(),
                },
            );
            n.retry_mdns_dials();
            assert!(!n.mdns_retry.contains_key(&me));

            // Past the teller's share: no retry.
            let late = PeerId::random();
            n.dial_mdns_found(vec![(
                late,
                at("/ip4/192.168.2.20/udp/4009/quic-v1"),
                Some(honest),
            )]);
            fail_all(n, late);
            while n.limits.take_mdns_peer(honest, Instant::now()) {}
            n.mdns_retry.get_mut(&late).unwrap().at = Instant::now();
            let before = n.mdns_dials.len();
            n.retry_mdns_dials();
            assert_eq!(n.mdns_dials.len(), before);
            assert!(!n.mdns_retry.contains_key(&late));
        })
        .await;
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_peer_mdns_tells_of_in_two_events_is_dialled_at_its_own_address_at_once() {
        let (node, _group) = lone_member();
        let (first, waited, after) = inspect(&node, |n| {
            let neighbour = std::net::IpAddr::from([192, 168, 1, 66]);
            let honest = std::net::IpAddr::from([192, 168, 2, 20]);
            let at = |s: &str| s.parse::<Multiaddr>().unwrap();
            let made_up = |from: u16| {
                (from..from + 8)
                    .map(|port| {
                        (
                            at(&format!("/ip4/192.168.1.66/udp/{port}/quic-v1")),
                            neighbour,
                        )
                    })
                    .collect::<Vec<_>>()
            };
            let own = (at("/ip4/192.168.2.20/udp/4001/quic-v1"), honest);
            let tell = |n: &mut Node, peer: PeerId, told: Vec<(Multiaddr, std::net::IpAddr)>| {
                n.dial_mdns_found(
                    told.into_iter()
                        .map(|(a, by)| (peer, a, Some(by)))
                        .collect(),
                );
            };
            let dials = |n: &Node, peer: PeerId| -> Vec<Vec<Multiaddr>> {
                (n.mdns_dials.iter())
                    .filter(|(p, _)| *p == peer)
                    .map(|(_, a)| a.clone())
                    .collect()
            };
            let friend = PeerId::random();
            tell(n, friend, made_up(5000));
            tell(n, friend, vec![own.clone()]);
            // Told again: nothing more.
            tell(n, friend, vec![own.clone()]);
            let first = dials(n, friend);
            // Another, named at more made-up addresses than are dialled at once first.
            let other = PeerId::random();
            tell(n, other, made_up(6000));
            tell(n, other, made_up(7000));
            tell(n, other, vec![own.clone()]);
            let waited = dials(n, other);
            let pending: Vec<ConnectionId> =
                n.mdns_dialling[&other].pending.iter().copied().collect();
            for id in pending {
                n.mdns_dial_failed(other, id);
            }
            let after = dials(n, other);
            (first, waited, after)
        })
        .await;
        let own: Multiaddr = "/ip4/192.168.2.20/udp/4001/quic-v1".parse().unwrap();
        assert_eq!(first.len(), 2, "dialled as {first:?}");
        assert_eq!(first[0].len(), 8);
        assert_eq!(first[1], std::slice::from_ref(&own));
        assert_eq!(waited.len(), 2, "dialled as {waited:?}");
        assert!(waited.iter().all(|d| !d.contains(&own)));
        assert_eq!(after.len(), 3, "dialled as {after:?}");
        assert_eq!(after[2], [own]);
        node.shutdown().await.unwrap();
    }
}
