//! The peers mDNS discovered, bounded: anyone on the local network can answer a query with
//! any number of made-up peers, each with a far-off expiry.

use std::{collections::HashMap, net::IpAddr, time::Instant};

use libp2p_core::{Multiaddr, multiaddr::Protocol};
use libp2p_identity::PeerId;

/// Peers kept at once in all, and from the responses of one address.
///
/// Past its share, a new peer of an address takes the place of that address's peer that
/// expires first: one told of a moment ago becomes that one once every other of its address
/// was heard again after it, and the next new one would then push it out before the node
/// had asked who told of it (`Behaviour::told_by`), had the behaviour taken all those
/// answers in one go: copies on one computer, more than its share, all answer every query
/// at once. So the behaviour takes at most this many addresses before it hands on what
/// they told: a peer cannot be told of and pushed out in one lot.
const MAX_PEERS: usize = 128;
pub(crate) const MAX_PEERS_PER_SOURCE: usize = 16;
/// Addresses kept for one peer: a computer with a few interfaces, each with IPv4 and IPv6
/// over QUIC and TCP, has fewer.
const MAX_ADDRESSES: usize = 8;

#[derive(Debug)]
struct Peer {
    addresses: HashMap<Multiaddr, Address>,
    /// The address of the response that first named it, or whose address of it was reached
    /// since. It stays the peer's source: a neighbour cannot make another address's peer its
    /// own, which would count it among its share and let it push the peer out, or fill its
    /// addresses with made-up ones. A connection at an address another response told of
    /// makes that response's address the source: the first to answer, who may have made
    /// up what it said, does not keep the peer for good.
    from: IpAddr,
}

/// One address of a peer.
#[derive(Debug)]
struct Address {
    expires: Instant,
    /// The address of the response that told of it.
    told_by: IpAddr,
    /// Whether a connection to the peer was made at it: it is never dropped for another.
    reached: bool,
}

impl Peer {
    fn expires(&self) -> Option<Instant> {
        self.addresses.values().map(|a| a.expires).max()
    }
}

#[derive(Debug, Default)]
pub(crate) struct Discovered {
    peers: HashMap<PeerId, Peer>,
}

impl Discovered {
    /// Keeps `address` of `peer`, told of by a response from `from`, until `expires`
    /// (or later, if it was known until later). Returns whether the address is new, and the
    /// addresses of the peers dropped to make room for it: past [`MAX_PEERS_PER_SOURCE`],
    /// the one of the same source that expires first; past [`MAX_PEERS`], that of the
    /// source with the most peers, so that one source cannot push out the others.
    ///
    /// For a peer already known, only its source adds addresses freely. Another address
    /// adds only addresses of its own (the same computer answering from a second
    /// interface, or the peer itself after someone else named it first). Past
    /// [`MAX_ADDRESSES`] a new one takes the place of one never reached, that expires first,
    /// told of by another address: for the source, one another address told of; for another
    /// address, any but its own. One reached is never dropped.
    pub(crate) fn insert(
        &mut self,
        peer: PeerId,
        address: Multiaddr,
        expires: Instant,
        from: IpAddr,
    ) -> (bool, Vec<(PeerId, Multiaddr)>) {
        if let Some(known) = self.peers.get_mut(&peer) {
            let own = from == known.from;
            if !own && ip_of(&address) != Some(from) {
                return (false, Vec::new());
            }
            if let Some(at) = known.addresses.get_mut(&address) {
                at.expires = at.expires.max(expires);
                return (false, Vec::new());
            }
            let mut dropped = Vec::new();
            if known.addresses.len() >= MAX_ADDRESSES {
                let other = known
                    .addresses
                    .iter()
                    .filter(|(_, a)| !a.reached && a.told_by != from)
                    .min_by_key(|(_, a)| a.expires)
                    .map(|(a, _)| a.clone());
                let Some(gone) = other else {
                    return (false, Vec::new());
                };
                known.addresses.remove(&gone);
                dropped.push((peer, gone));
            }
            known.addresses.insert(address, Address::new(expires, from));
            return (true, dropped);
        }
        let mut dropped = Vec::new();
        let from_source = self.peers.values().filter(|p| p.from == from).count();
        let crowded = if from_source >= MAX_PEERS_PER_SOURCE {
            Some(from)
        } else if self.peers.len() >= MAX_PEERS {
            let mut counts: HashMap<IpAddr, usize> = HashMap::new();
            for p in self.peers.values() {
                *counts.entry(p.from).or_default() += 1;
            }
            counts.into_iter().max_by_key(|(_, n)| *n).map(|(ip, _)| ip)
        } else {
            None
        };
        if let Some(source) = crowded {
            let first = self
                .peers
                .iter()
                .filter(|(_, p)| p.from == source)
                .min_by_key(|(_, p)| p.expires())
                .map(|(id, _)| *id);
            if let Some(gone) = first.and_then(|id| self.peers.remove_entry(&id)) {
                dropped.extend(gone.1.addresses.into_keys().map(|a| (gone.0, a)));
            }
        }
        self.peers.insert(
            peer,
            Peer {
                addresses: HashMap::from([(address, Address::new(expires, from))]),
                from,
            },
        );
        (true, dropped)
    }

    /// Notes that a connection to `peer` was made at `address`: that address stays, and the
    /// response that told of it becomes the peer's source.
    pub(crate) fn reached(&mut self, peer: &PeerId, address: &Multiaddr) {
        let Some(known) = self.peers.get_mut(peer) else {
            return;
        };
        let address = without_peer(address);
        if let Some(at) = known.addresses.get_mut(&address) {
            at.reached = true;
            known.from = at.told_by;
        }
    }

    /// The address of the response that told of `address` of `peer`, while it is known.
    pub(crate) fn told_by(&self, peer: &PeerId, address: &Multiaddr) -> Option<IpAddr> {
        let known = self.peers.get(peer)?;
        known.addresses.get(address).map(|a| a.told_by)
    }

    /// Drops every address that expired by `now`, and returns them.
    pub(crate) fn expire(&mut self, now: Instant) -> Vec<(PeerId, Multiaddr)> {
        let mut expired = Vec::new();
        self.peers.retain(|id, peer| {
            peer.addresses.retain(|address, at| {
                if at.expires <= now {
                    expired.push((*id, address.clone()));
                    return false;
                }
                true
            });
            !peer.addresses.is_empty()
        });
        expired
    }

    /// Makes every address of `peer` expire at `now`.
    pub(crate) fn expire_peer(&mut self, peer: &PeerId, now: Instant) {
        if let Some(peer) = self.peers.get_mut(peer) {
            for at in peer.addresses.values_mut() {
                at.expires = now;
            }
        }
    }

    /// When the next address expires.
    pub(crate) fn next_expiry(&self) -> Option<Instant> {
        self.peers
            .values()
            .flat_map(|p| p.addresses.values().map(|a| a.expires))
            .min()
    }

    pub(crate) fn addresses(&self, peer: &PeerId) -> Vec<Multiaddr> {
        self.peers
            .get(peer)
            .map(|p| p.addresses.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) fn peers(&self) -> impl ExactSizeIterator<Item = &PeerId> {
        self.peers.keys()
    }

    /// The address of the response that is `peer`'s source (see [`Peer::from`]).
    pub(crate) fn source(&self, peer: &PeerId) -> Option<IpAddr> {
        self.peers.get(peer).map(|p| p.from)
    }
}

impl Address {
    fn new(expires: Instant, told_by: IpAddr) -> Self {
        Self {
            expires,
            told_by,
            reached: false,
        }
    }
}

/// `address` without the peer id a dial puts at its end.
fn without_peer(address: &Multiaddr) -> Multiaddr {
    let mut address = address.clone();
    if matches!(address.iter().last(), Some(Protocol::P2p(_))) {
        address.pop();
    }
    address
}

/// The IP address at the start of `address`, if any.
fn ip_of(address: &Multiaddr) -> Option<IpAddr> {
    address.iter().find_map(|p| match p {
        Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn address(i: usize) -> Multiaddr {
        format!("/ip4/192.168.1.2/udp/{}/quic-v1", 4000 + i)
            .parse()
            .unwrap()
    }

    fn address_at(ip: &str, i: usize) -> Multiaddr {
        format!("/ip4/{ip}/udp/{}/quic-v1", 4000 + i)
            .parse()
            .unwrap()
    }

    #[test]
    fn a_flood_of_made_up_peers_keeps_a_bounded_number() {
        let mut found = Discovered::default();
        let now = Instant::now();
        let honest: IpAddr = [192, 168, 1, 20].into();
        let friend = PeerId::random();
        assert!(
            found
                .insert(friend, address(0), now + Duration::from_secs(60), honest)
                .0
        );
        // One neighbour, ten thousand made-up peers: it keeps only its own share, the
        // newest, and pushes out no one else's.
        let flooder: IpAddr = [192, 168, 1, 66].into();
        for i in 0..10_000 {
            let expires = now + Duration::from_secs(60) + Duration::from_millis(i as u64);
            found.insert(PeerId::random(), address(i), expires, flooder);
        }
        assert_eq!(found.peers().len(), 1 + MAX_PEERS_PER_SOURCE);
        assert_eq!(found.addresses(&friend), vec![address(0)]);
        // Many neighbours (or one using many addresses): bounded in all, and the one with
        // the fewest peers is the last to lose any.
        for source in 0..64u8 {
            for i in 0..MAX_PEERS_PER_SOURCE {
                let expires = now + Duration::from_secs(60);
                found.insert(
                    PeerId::random(),
                    address(i),
                    expires,
                    [10, 0, 0, source].into(),
                );
            }
        }
        assert_eq!(found.peers().len(), MAX_PEERS);
        assert_eq!(found.addresses(&friend), vec![address(0)]);
        // And a bounded number of addresses for one peer.
        for i in 1..100 {
            found.insert(friend, address(i), now + Duration::from_secs(60), honest);
        }
        assert_eq!(found.addresses(&friend).len(), MAX_ADDRESSES);
    }

    #[test]
    fn addresses_expire_and_dropped_ones_are_told() {
        let mut found = Discovered::default();
        let now = Instant::now();
        let from: IpAddr = [192, 168, 1, 66].into();
        let first = PeerId::random();
        found.insert(first, address(0), now + Duration::from_secs(1), from);
        for i in 1..MAX_PEERS_PER_SOURCE {
            found.insert(
                PeerId::random(),
                address(i),
                now + Duration::from_secs(60),
                from,
            );
        }
        // The one of that source that expires first makes room.
        let (new, dropped) = found.insert(
            PeerId::random(),
            address(99),
            now + Duration::from_secs(60),
            from,
        );
        assert!(new);
        assert_eq!(dropped, vec![(first, address(0))]);
        assert_eq!(found.next_expiry(), Some(now + Duration::from_secs(60)));
        assert_eq!(
            found.expire(now + Duration::from_secs(61)).len(),
            MAX_PEERS_PER_SOURCE
        );
        assert_eq!(found.peers().len(), 0);
    }

    fn with_peer(address: Multiaddr, peer: PeerId) -> Multiaddr {
        address.with(Protocol::P2p(peer))
    }

    /// A neighbour that names a peer first, with made-up addresses, does not keep it: the
    /// peer's own answer still goes in, and once reached there the peer is that answer's,
    /// which the neighbour can no longer push out.
    #[test]
    fn the_first_to_answer_for_a_peer_does_not_keep_it() {
        let mut found = Discovered::default();
        let now = Instant::now();
        let later = now + Duration::from_secs(60);
        let neighbour: IpAddr = [192, 168, 1, 66].into();
        let honest: IpAddr = [192, 168, 1, 20].into();
        let friend = PeerId::random();
        for i in 0..MAX_ADDRESSES {
            found.insert(friend, address_at("192.168.1.66", i), later, neighbour);
        }
        assert_eq!(found.source(&friend), Some(neighbour));
        let real = address_at("192.168.1.20", 0);
        let (new, dropped) = found.insert(friend, real.clone(), later, honest);
        assert!(new, "the peer's own address was not taken");
        assert_eq!(dropped.len(), 1);
        assert!(found.addresses(&friend).contains(&real));
        assert_eq!(found.told_by(&friend, &real), Some(honest));
        found.reached(&friend, &with_peer(real.clone(), friend));
        assert_eq!(found.source(&friend), Some(honest));
        // The neighbour goes on answering: its addresses take no place of the one reached,
        // and its flood of made-up peers pushes out its own, never the friend.
        for i in 0..100 {
            let later = later + Duration::from_secs(i);
            found.insert(
                friend,
                address_at("192.168.1.66", 100 + i as usize),
                later,
                neighbour,
            );
            found.insert(
                PeerId::random(),
                address_at("192.168.1.66", 200 + i as usize),
                later,
                neighbour,
            );
        }
        assert!(found.addresses(&friend).contains(&real));
        assert_eq!(found.source(&friend), Some(honest));
    }

    #[test]
    fn a_neighbour_cannot_speak_for_a_peer_another_address_told_of() {
        let mut found = Discovered::default();
        let now = Instant::now();
        let later = now + Duration::from_secs(60);
        let honest: IpAddr = [192, 168, 1, 20].into();
        let friend = PeerId::random();
        found.insert(friend, address_at("192.168.1.20", 0), later, honest);
        // Dialled at once, and reached there.
        found.reached(&friend, &with_peer(address_at("192.168.1.20", 0), friend));
        // A neighbour names the friend with addresses that are not its own: not taken.
        let neighbour: IpAddr = [192, 168, 1, 66].into();
        for i in 1..20 {
            let (new, _) = found.insert(friend, address_at("192.168.1.20", i), later, neighbour);
            assert!(!new);
        }
        assert_eq!(found.addresses(&friend).len(), 1);
        // With addresses of its own (as a second interface of the friend's computer
        // would), a few, never in the place of the one reached; the friend's own still go
        // in, in their place.
        for i in 0..20 {
            found.insert(friend, address_at("192.168.1.66", i), later, neighbour);
        }
        assert_eq!(found.addresses(&friend).len(), MAX_ADDRESSES);
        let (new, dropped) = found.insert(friend, address_at("192.168.1.20", 1), later, honest);
        assert!(new);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].0, friend);
        assert!(dropped[0].1.to_string().contains("192.168.1.66"));
        assert!(
            found
                .addresses(&friend)
                .contains(&address_at("192.168.1.20", 1))
        );
        // The friend stays the honest address's peer: the neighbour's flood of made-up
        // peers pushes out its own, never the friend.
        for i in 0..100 {
            found.insert(
                PeerId::random(),
                address_at("192.168.1.66", 100 + i),
                later,
                neighbour,
            );
        }
        assert_eq!(found.peers().len(), 1 + MAX_PEERS_PER_SOURCE);
        assert!(
            found
                .addresses(&friend)
                .contains(&address_at("192.168.1.20", 0))
        );
    }
}
