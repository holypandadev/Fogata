//! Helpers to classify multiaddresses.

use std::net::IpAddr;

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
}
