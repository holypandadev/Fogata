//! Helpers to classify multiaddresses, and the rule for dialling an address someone else
//! named ([`OwnNetworks::may_dial`]).

use std::{collections::HashSet, net::IpAddr};

use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};

/// The IP address at the start of `addr`, if any.
pub(crate) fn ip(addr: &Multiaddr) -> Option<IpAddr> {
    addr.iter().find_map(|p| match p {
        Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
        _ => None,
    })
}

/// Whether `addr` names a host that must be looked up in DNS to dial it (`/dns`, `/dns4`,
/// `/dns6` or `/dnsaddr`).
pub(crate) fn is_named(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| {
        matches!(
            p,
            Protocol::Dns(_) | Protocol::Dns4(_) | Protocol::Dns6(_) | Protocol::Dnsaddr(_)
        )
    })
}

/// Whether this node's transports can dial `addr` at all: IP or DNS, then TCP or QUIC, maybe
/// through a relay. Anything else (WebSockets, TLS, WebTransport, WebRTC) fails anyway, and
/// a name in it would first be looked up in DNS for nothing.
pub(crate) fn is_dialable(addr: &Multiaddr) -> bool {
    addr.iter().all(|p| {
        matches!(
            p,
            Protocol::Ip4(_)
                | Protocol::Ip6(_)
                | Protocol::Dns(_)
                | Protocol::Dns4(_)
                | Protocol::Dns6(_)
                | Protocol::Dnsaddr(_)
                | Protocol::Tcp(_)
                | Protocol::Udp(_)
                | Protocol::QuicV1
                | Protocol::P2p(_)
                | Protocol::P2pCircuit
        )
    })
}

/// True for loopback, private, link-local and carrier-grade NAT ranges: addresses that are
/// meaningless outside the local network and must never be published.
pub(crate) fn is_local(addr: &Multiaddr) -> bool {
    match ip(addr) {
        Some(IpAddr::V4(ip)) => {
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64)
        }
        Some(IpAddr::V6(ip)) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
        }
        None => false,
    }
}

/// True for an address on a local network that another device there could use: private and
/// carrier-grade NAT IPv4, unique-local IPv6. Not loopback, unspecified or link-local.
pub(crate) fn is_lan(addr: &Multiaddr) -> bool {
    match ip(addr) {
        Some(IpAddr::V4(ip)) => {
            ip.is_private() || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64)
        }
        Some(IpAddr::V6(ip)) => (ip.segments()[0] & 0xfe00) == 0xfc00,
        None => false,
    }
}

/// The local network an address names a computer on, as far as the address tells, for an
/// address that names one only there: elsewhere it names some other computer, or one on a
/// network of this device's own that whoever sent it has no business making this device
/// send packets to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LocalBlock {
    /// The /24 of a private or link-local IPv4 address.
    Private([u8; 3]),
    /// Carrier-grade NAT's 100.64/10, all of it: a carrier's network, or a private one such
    /// as Tailscale's, which hands out addresses all over it, so two devices on one rarely
    /// share any smaller block.
    Shared,
    /// The /48 of a unique-local IPv6 address (fc00::/7): one site's prefix, which every
    /// device on that home network has its addresses in.
    UniqueLocal([u16; 3]),
}

/// The [`LocalBlock`] of `address`, or none for a public or loopback one.
pub(crate) fn local_block(address: &Multiaddr) -> Option<LocalBlock> {
    match ip(address)? {
        IpAddr::V4(ip) if ip.is_private() || ip.is_link_local() => {
            let [a, b, c, _] = ip.octets();
            Some(LocalBlock::Private([a, b, c]))
        }
        IpAddr::V4(ip) if ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64 => {
            Some(LocalBlock::Shared)
        }
        IpAddr::V6(ip) if (ip.segments()[0] & 0xfe00) == 0xfc00 => {
            let [a, b, c, ..] = ip.segments();
            Some(LocalBlock::UniqueLocal([a, b, c]))
        }
        _ => None,
    }
}

/// Whether `address` may be dialled from a device on the networks of `own` (see
/// [`OwnNetworks`]): a public one, or a local one in a block of `own`.
pub(crate) fn on_own_network(address: &Multiaddr, own: &HashSet<LocalBlock>) -> bool {
    local_block(address).is_none_or(|b| own.contains(&b))
}

/// [`on_own_network`], but a private IPv4 address in any /24: where nothing showed that the
/// device named is on this device's own network, it may be on another one of the same
/// place that this one's router leads to (two routers at home, a guest network, a large
/// 10.0.0.0/16), so those dials stay as they always were. Carrier-grade NAT and
/// unique-local IPv6 addresses still only of a network this device is on.
pub(crate) fn may_route_to(address: &Multiaddr, own: &HashSet<LocalBlock>) -> bool {
    match local_block(address) {
        Some(LocalBlock::Private(_)) => true,
        block => block.is_none_or(|b| own.contains(&b)),
    }
}

/// How far from this device's own networks an address someone else named may lead (see
/// [`OwnNetworks::may_dial`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reach {
    /// A local address only on a network this device is on ([`on_own_network`]): for the
    /// dials that go there at once and again (media, a lost member).
    OwnNetwork,
    /// A private IPv4 address in another /24 too ([`may_route_to`]): for addresses that may
    /// be those of a device at the same place behind another router (sealed local names,
    /// links, invites, the punch slots).
    SamePlace,
}

/// The networks this device is on and its own addresses, from its listen addresses: what an
/// address someone else named is checked against before this device dials it.
#[derive(Clone, Debug, Default)]
pub(crate) struct OwnNetworks {
    blocks: HashSet<LocalBlock>,
    /// Its listen addresses, without a peer id.
    addresses: HashSet<Multiaddr>,
}

impl OwnNetworks {
    pub(crate) fn of<'a>(listen: impl IntoIterator<Item = &'a Multiaddr>) -> Self {
        let mut own = Self::default();
        for address in listen {
            own.blocks.extend(local_block(address));
            own.addresses.insert(without_peer(address));
        }
        own
    }

    /// The [`LocalBlock`]s of the networks this device is on.
    pub(crate) fn blocks(&self) -> &HashSet<LocalBlock> {
        &self.blocks
    }

    /// Whether `address` is one this device listens on.
    pub(crate) fn is_own(&self, address: &Multiaddr) -> bool {
        self.addresses.contains(&without_peer(address))
    }

    /// Whether `address`, which someone else named (a peer, a DHT record, mDNS, a link, an
    /// invite), may be dialled from here: the one test every such path takes. Never this
    /// computer (loopback), no address at all (unspecified), many (multicast, broadcast), an
    /// IPv6 link-local one (which names a computer only with an interface a dial cannot
    /// give), an IPv4 one written as IPv6, or one of this device's own; a local one only as
    /// `reach` allows. Each dial of a local address is a packet sent inside this device's own
    /// networks (a router's admin port, a VPN peer) at someone else's choice. A name has no
    /// address to check: those are left to the paths that look names up, or never do.
    pub(crate) fn may_dial(&self, address: &Multiaddr, reach: Reach) -> bool {
        let Some(ip) = ip(address) else {
            return true;
        };
        let never = match ip {
            IpAddr::V4(ip) => {
                ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast()
            }
            IpAddr::V6(ip) => {
                ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_multicast()
                    || (ip.segments()[0] & 0xffc0) == 0xfe80
                    || ip.to_ipv4_mapped().is_some()
            }
        };
        if never || self.is_own(address) {
            return false;
        }
        match reach {
            Reach::OwnNetwork => on_own_network(address, &self.blocks),
            Reach::SamePlace => may_route_to(address, &self.blocks),
        }
    }
}

pub(crate) fn is_quic(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| matches!(p, Protocol::QuicV1))
}

pub(crate) fn is_relayed(addr: &Multiaddr) -> bool {
    addr.iter().any(|p| matches!(p, Protocol::P2pCircuit))
}

/// `addr` without a trailing `/p2p/<id>`.
pub(crate) fn without_peer(addr: &Multiaddr) -> Multiaddr {
    addr.iter()
        .filter(|p| !matches!(p, Protocol::P2p(_)))
        .collect()
}

/// The relay peer of a `/.../p2p/<relay>/p2p-circuit/...` address.
pub(crate) fn relay_of(addr: &Multiaddr) -> Option<PeerId> {
    let mut last_peer = None;
    for p in addr.iter() {
        match p {
            Protocol::P2p(peer) => last_peer = Some(peer),
            Protocol::P2pCircuit => return last_peer,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    #[test]
    fn local_ranges_are_detected() {
        assert!(is_local(&a("/ip4/192.168.1.10/udp/1/quic-v1")));
        assert!(is_local(&a("/ip4/10.0.0.1/tcp/1")));
        assert!(is_local(&a("/ip4/100.64.3.1/tcp/1")));
        assert!(is_local(&a("/ip6/fe80::1/tcp/1")));
        assert!(is_local(&a("/ip6/fd00::1/tcp/1")));
        assert!(!is_local(&a("/ip4/8.8.8.8/udp/1/quic-v1")));
        assert!(!is_local(&a("/ip6/2001:db8::1/tcp/1")));
        assert!(is_lan(&a("/ip4/192.168.2.123/udp/1/quic-v1")));
        assert!(is_lan(&a("/ip4/100.64.3.1/tcp/1")));
        assert!(is_lan(&a("/ip6/fd00::1/tcp/1")));
        assert!(!is_lan(&a("/ip4/127.0.0.1/tcp/1")));
        assert!(!is_lan(&a("/ip4/169.254.1.1/tcp/1")));
        assert!(!is_lan(&a("/ip6/fe80::1/tcp/1")));
        assert!(!is_lan(&a("/ip4/8.8.8.8/udp/1/quic-v1")));
    }

    #[test]
    fn relay_peer_is_extracted() {
        let relay = PeerId::random();
        let target = PeerId::random();
        let addr = a("/ip4/1.2.3.4/udp/5/quic-v1")
            .with(Protocol::P2p(relay))
            .with(Protocol::P2pCircuit)
            .with(Protocol::P2p(target));
        assert!(is_relayed(&addr));
        assert_eq!(relay_of(&addr), Some(relay));
        assert_eq!(without_peer(&addr).iter().count(), 4);
    }

    /// Two devices on one home network (IPv4 /24, unique-local IPv6 site) or both on
    /// carrier-grade NAT or a VPN in 100.64/10 are on the same block; elsewhere they are not.
    #[test]
    fn local_addresses_are_on_this_device_s_networks_only_in_its_blocks() {
        let a = |s: &str| -> Multiaddr { s.parse().unwrap() };
        let own: HashSet<LocalBlock> = [
            "/ip4/192.168.1.20/udp/1/quic-v1",
            "/ip6/fd12:3456:789a:1:aaaa::20/udp/1/quic-v1",
            "/ip4/100.101.2.3/udp/1/quic-v1",
            "/ip6/2001:db8::20/udp/1/quic-v1",
        ]
        .iter()
        .filter_map(|s| local_block(&a(s)))
        .collect();
        assert_eq!(own.len(), 3);
        for ok in [
            "/ip4/192.168.1.30/udp/1/quic-v1",
            "/ip6/fd12:3456:789a:2::30/udp/1/quic-v1",
            "/ip4/100.70.8.9/udp/1/quic-v1",
            "/ip4/100.127.255.1/udp/1/quic-v1",
            "/ip4/203.0.113.5/udp/1/quic-v1",
            "/ip6/2001:db8::30/udp/1/quic-v1",
        ] {
            assert!(on_own_network(&a(ok), &own), "{ok}");
            assert!(may_route_to(&a(ok), &own), "{ok}");
        }
        for elsewhere in [
            "/ip4/192.168.2.30/udp/1/quic-v1",
            "/ip4/10.0.0.1/udp/1/quic-v1",
            "/ip4/169.254.1.1/udp/1/quic-v1",
            "/ip6/fd12:3456:789b:1::30/udp/1/quic-v1",
            "/ip6/fc00::1/udp/1/quic-v1",
        ] {
            assert!(!on_own_network(&a(elsewhere), &own), "{elsewhere}");
        }
        // Another private IPv4 network may be one this device's router leads to.
        assert!(may_route_to(&a("/ip4/192.168.2.30/udp/1/quic-v1"), &own));
        assert!(may_route_to(&a("/ip4/10.0.0.1/udp/1/quic-v1"), &own));
        assert!(!may_route_to(
            &a("/ip6/fd12:3456:789b:1::30/udp/1/quic-v1"),
            &own
        ));
        // Off carrier-grade NAT and any VPN in it, none of its addresses.
        let home: HashSet<LocalBlock> = local_block(&a("/ip4/192.168.1.20/tcp/1"))
            .into_iter()
            .collect();
        for vpn in [
            "/ip4/100.70.8.9/udp/1/quic-v1",
            "/ip6/fd12:3456:789a:1::30/udp/1/quic-v1",
        ] {
            assert!(!on_own_network(&a(vpn), &home), "{vpn}");
            assert!(!may_route_to(&a(vpn), &home), "{vpn}");
        }
    }

    /// What someone else names is dialled only as the rule says: never this computer, no
    /// address, many, an IPv6 link-local or IPv4-in-IPv6 one, nor this device's own (a copy on
    /// this computer, at another port, is); a local one on this device's networks, and with
    /// [`Reach::SamePlace`] a private IPv4 one in another /24 too, but never a carrier-grade
    /// NAT or unique-local one of a network this device is not on. Names are not this test's.
    #[test]
    fn named_addresses_are_dialled_only_as_the_rule_says() {
        let own = OwnNetworks::of(&[
            a("/ip4/192.168.1.20/udp/4001/quic-v1"),
            a("/ip6/fd12:3456:789a:1:aaaa::20/udp/4001/quic-v1"),
            a("/ip4/203.0.113.20/udp/4001/quic-v1"),
        ]);
        let both = [Reach::OwnNetwork, Reach::SamePlace];
        for never in [
            "/ip4/127.0.0.1/udp/4001/quic-v1",
            "/ip4/127.8.9.10/tcp/22",
            "/ip6/::1/udp/4001/quic-v1",
            "/ip4/0.0.0.0/udp/4001/quic-v1",
            "/ip6/::/udp/4001/quic-v1",
            "/ip4/224.0.0.251/udp/5353/quic-v1",
            "/ip4/239.1.2.3/udp/4001/quic-v1",
            "/ip4/255.255.255.255/udp/4001/quic-v1",
            "/ip6/ff02::1/udp/4001/quic-v1",
            "/ip6/fe80::1/udp/4001/quic-v1",
            "/ip6/::ffff:192.168.1.30/udp/4001/quic-v1",
            "/ip6/::ffff:127.0.0.1/udp/4001/quic-v1",
            "/ip4/192.168.1.20/udp/4001/quic-v1",
            "/ip4/100.70.8.9/udp/4001/quic-v1",
            "/ip6/fd99:1:2:1::1/udp/4001/quic-v1",
        ] {
            for reach in both {
                assert!(!own.may_dial(&a(never), reach), "{never} {reach:?}");
            }
        }
        for always in [
            "/ip4/192.168.1.20/udp/4002/quic-v1",
            "/ip4/192.168.1.30/udp/4001/quic-v1",
            "/ip6/fd12:3456:789a:2::30/udp/4001/quic-v1",
            "/ip4/198.51.100.30/udp/4001/quic-v1",
            "/ip4/203.0.113.20/tcp/4001",
            "/ip6/2001:db8::30/udp/4001/quic-v1",
            "/dnsaddr/bootstrap.example",
        ] {
            for reach in both {
                assert!(own.may_dial(&a(always), reach), "{always} {reach:?}");
            }
        }
        // Its own address with a peer id on is still its own.
        let mine = a("/ip4/203.0.113.20/udp/4001/quic-v1").with(Protocol::P2p(PeerId::random()));
        assert!(!own.may_dial(&mine, Reach::SamePlace));
        // Another /24 of the same place: only where that may be meant.
        for elsewhere in [
            "/ip4/192.168.2.123/udp/4001/quic-v1",
            "/ip4/10.0.0.1/udp/4001/quic-v1",
        ] {
            assert!(
                !own.may_dial(&a(elsewhere), Reach::OwnNetwork),
                "{elsewhere}"
            );
            assert!(own.may_dial(&a(elsewhere), Reach::SamePlace), "{elsewhere}");
        }
        // On a VPN in carrier-grade NAT, its addresses too.
        let vpn = OwnNetworks::of(&[a("/ip4/100.101.2.3/udp/4001/quic-v1")]);
        for reach in both {
            assert!(vpn.may_dial(&a("/ip4/100.70.8.9/udp/4001/quic-v1"), reach));
        }
        // On no network yet: no local address but another /24's private ones.
        let none = OwnNetworks::default();
        assert!(!none.may_dial(&a("/ip4/192.168.1.30/udp/4001/quic-v1"), Reach::OwnNetwork));
        assert!(none.may_dial(&a("/ip4/192.168.1.30/udp/4001/quic-v1"), Reach::SamePlace));
        assert!(!none.may_dial(&a("/ip4/100.70.8.9/udp/4001/quic-v1"), Reach::SamePlace));
    }
}
