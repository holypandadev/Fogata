//! Local addresses for friends and members only.
//!
//! A friend or member at the same place but behind another router (two routers at home,
//! one public address that does not lead back inside) reaches this device only at its local
//! address. Public DHT nodes drop private addresses from announcements, and those addresses
//! are nobody else's business anyway. So they travel sealed inside a name,
//! `/dns4/<sealed, in hex>.fogata/udp/<port>/quic-v1`: one name for friends, under a key from
//! this device's friend rendezvous secret, and one for each group, under a key from the
//! group's current rendezvous secret. The names only go into DHT announcements (never to
//! identify or to other peers); DHT nodes see random hex, and whoever knows one of the
//! secrets opens the name back into addresses. A name is never looked up in DNS.
//!
//! Every announcement carries [`MAX_NAMES`] names whatever the number of groups: the ones
//! not needed are random bytes of the same length, which open to nothing, so DHT nodes cannot
//! count a device's groups.
//!
//! One more name proves to friends that an announcement is the device's own: its signature
//! over its transport identity and the rendezvous window, sealed under a key from its friend
//! rendezvous secret (a [`proof`]). Another friend of the device knows that secret too and
//! can announce under it, but cannot sign as the device, so a device dials what it finds
//! under a friend's secret only once the proof in it opens and verifies as that friend.

use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, random_bytes};
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};

use crate::addr;

/// Names announced: one for friends and the rest for groups, padded with random ones.
pub(crate) const MAX_NAMES: usize = 6;
/// Most local addresses in one name (a DNS label holds 63 characters).
const MAX_IPS: usize = 3;
const DOMAIN: &str = ".fogata";
const CONTEXT: &[u8] = b"fogata/v1/lan";
const PROOF_CONTEXT: &[u8] = b"fogata/v1/friend-announcement";
/// Most characters in one label of a name.
const LABEL: usize = 63;
/// How many rendezvous windows before or after this device's current one a proof may name:
/// DHT nodes may hand out the addresses of an earlier announcement for a while, and clocks
/// differ.
const PROOF_WINDOWS_BEFORE: u64 = 2;
const PROOF_WINDOWS_AFTER: u64 = 1;
/// The ports devices listen on (the application's own range): only the addresses are sealed,
/// so a DHT node that keeps an announcement could change the port, to point this device's
/// dials at another service of a local address. A name with any other port opens to nothing.
pub(crate) const PORTS: std::ops::Range<u16> = 20000..40000;

/// The key that seals names for those who know `rendezvous_secret`.
pub(crate) fn key_of(rendezvous_secret: &[u8; 32]) -> [u8; 32] {
    fogata_crypto::derive_secret(CONTEXT, rendezvous_secret, b"local addresses")
}

/// Whether `address` is a sealed name, which is never resolved.
pub(crate) fn is_name(address: &Multiaddr) -> bool {
    address.iter().any(|p| match p {
        Protocol::Dns(name) | Protocol::Dns4(name) | Protocol::Dns6(name) => {
            name.to_ascii_lowercase().ends_with(DOMAIN)
        }
        _ => false,
    })
}

/// A QUIC port and local IPv4 addresses on it.
pub(crate) type Local = (u16, Vec<Ipv4Addr>);

/// The QUIC port and the local IPv4 addresses worth sealing, from this device's listen
/// addresses: home networks first, then larger private ranges, then shared address space
/// (some VPNs), then the range container bridges usually take.
pub(crate) fn local(listen: &[Multiaddr]) -> Option<Local> {
    let mut port = None;
    let mut ips: Vec<Ipv4Addr> = Vec::new();
    for address in listen
        .iter()
        .filter(|a| addr::is_lan(a) && addr::is_quic(a))
    {
        let Some(IpAddr::V4(ip)) = addr::ip(address) else {
            continue;
        };
        let Some(Protocol::Udp(p)) = address.iter().find(|p| matches!(p, Protocol::Udp(_))) else {
            continue;
        };
        port.get_or_insert(p);
        if Some(p) == port && !ips.contains(&ip) {
            ips.push(ip);
        }
    }
    ips.sort_by_key(|ip| match ip.octets() {
        [192, 168, ..] => 0,
        [10, ..] => 1,
        [100, ..] => 2,
        _ => 3,
    });
    ips.truncate(MAX_IPS);
    Some((port?, ips)).filter(|(_, ips)| !ips.is_empty())
}

/// The port of this device's QUIC listen addresses, for names when it has no local address
/// to seal.
pub(crate) fn quic_port(listen: &[Multiaddr]) -> Option<u16> {
    listen.iter().filter(|a| addr::is_quic(a)).find_map(|a| {
        a.iter().find_map(|p| match p {
            Protocol::Udp(port) => Some(port),
            _ => None,
        })
    })
}

/// A name holding `ips` for whoever has `key`.
pub(crate) fn seal(key: &[u8; 32], port: u16, ips: &[Ipv4Addr]) -> Option<Multiaddr> {
    let plain: Vec<u8> = ips
        .iter()
        .take(MAX_IPS)
        .flat_map(|ip| ip.octets())
        .collect();
    let sealed = fogata_crypto::seal_short(key, CONTEXT, &plain).ok()?;
    Some(name(&sealed, port))
}

/// [`MAX_NAMES`] names: one holding `ips` for each of `keys` (at most that many), then
/// random ones of the same length, which nobody opens.
pub(crate) fn names(port: u16, ips: &[Ipv4Addr], keys: &[[u8; 32]]) -> Vec<Multiaddr> {
    let mut out: Vec<Multiaddr> = keys
        .iter()
        .take(MAX_NAMES)
        .filter_map(|key| seal(key, port, ips))
        .collect();
    // As long as a real one: what the local addresses take, sealed.
    let length = ips.len().clamp(1, MAX_IPS) * 4 + fogata_crypto::SHORT_OVERHEAD;
    while out.len() < MAX_NAMES {
        let mut bytes = vec![0u8; length];
        if random_bytes(&mut bytes).is_err() {
            break;
        }
        out.push(name(&bytes, port));
    }
    out
}

/// `/dns4/<sealed, in hex, in labels of at most 63 characters>.fogata/udp/<port>/quic-v1`.
fn name(sealed: &[u8], port: u16) -> Multiaddr {
    let hex = hex::encode(sealed);
    let labels: Vec<&str> = hex
        .as_bytes()
        .chunks(LABEL)
        .map(|c| std::str::from_utf8(c).expect("hex is ASCII"))
        .collect();
    Multiaddr::empty()
        .with(Protocol::Dns4(
            format!("{}{DOMAIN}", labels.join(".")).into(),
        ))
        .with(Protocol::Udp(port))
        .with(Protocol::QuicV1)
}

/// The sealed bytes of a name, from all its labels.
fn sealed_of(name: &Multiaddr) -> Option<Vec<u8>> {
    let Some(Protocol::Dns4(host)) = name.iter().next() else {
        return None;
    };
    let hex: String = host
        .to_ascii_lowercase()
        .strip_suffix(DOMAIN)?
        .split('.')
        .collect();
    hex::decode(hex).ok()
}

/// What the device signs to prove an announcement is its own.
fn proof_message(peer: &PeerId, window: u64) -> Vec<u8> {
    let mut message = PROOF_CONTEXT.to_vec();
    message.extend_from_slice(&peer.to_bytes());
    message.extend_from_slice(&window.to_be_bytes());
    message
}

fn proof_key(friend_rendezvous: &[u8; 32]) -> [u8; 32] {
    fogata_crypto::derive_secret(PROOF_CONTEXT, friend_rendezvous, b"proof")
}

/// The name that proves to friends that announcements of `peer` in `window` are this
/// device's: its signature over both, sealed under a key from its friend rendezvous secret.
pub(crate) fn proof(
    identity: &DeviceIdentity,
    friend_rendezvous: &[u8; 32],
    peer: &PeerId,
    window: u64,
    port: u16,
) -> Option<Multiaddr> {
    let signature = identity.sign(&proof_message(peer, window));
    let sealed =
        fogata_crypto::seal_short(&proof_key(friend_rendezvous), PROOF_CONTEXT, &signature).ok()?;
    Some(name(&sealed, port))
}

/// Whether one of `names`, announced by `peer`, proves it is `device`, the friend whose
/// rendezvous secret is `friend_rendezvous`, around rendezvous window `window`.
pub(crate) fn proven(
    names: &[Multiaddr],
    device: &DevicePublicKey,
    friend_rendezvous: &[u8; 32],
    peer: &PeerId,
    window: u64,
) -> bool {
    let key = proof_key(friend_rendezvous);
    let windows = window.saturating_sub(PROOF_WINDOWS_BEFORE)..=window + PROOF_WINDOWS_AFTER;
    names
        .iter()
        .filter(|name| is_name(name))
        .filter_map(sealed_of)
        .filter(|sealed| sealed.len() == fogata_crypto::SHORT_OVERHEAD + 64)
        .filter_map(|sealed| fogata_crypto::open_short(&key, PROOF_CONTEXT, &sealed).ok())
        .any(|signature| {
            windows
                .clone()
                .any(|w| device.verify(&proof_message(peer, w), &signature).is_ok())
        })
}

/// The addresses in `name` if one of `keys` opens it.
pub(crate) fn open(name: &Multiaddr, keys: &[[u8; 32]]) -> Vec<Multiaddr> {
    let mut parts = name.iter();
    let (Some(Protocol::Dns4(host)), Some(Protocol::Udp(port)), Some(Protocol::QuicV1)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return Vec::new();
    };
    if !PORTS.contains(&port) {
        return Vec::new();
    }
    let peer = parts.next().filter(|p| matches!(p, Protocol::P2p(_)));
    let Some(sealed) = host
        .to_ascii_lowercase()
        .strip_suffix(DOMAIN)
        .and_then(|h| hex::decode(h).ok())
    else {
        return Vec::new();
    };
    let Some(plain) = keys
        .iter()
        .find_map(|key| fogata_crypto::open_short(key, CONTEXT, &sealed).ok())
    else {
        return Vec::new();
    };
    plain
        .as_chunks::<4>()
        .0
        .iter()
        .map(|o| Ipv4Addr::from(*o))
        .filter(|ip| ip.is_private() || (ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64))
        .map(|ip| {
            let mut address = Multiaddr::empty()
                .with(Protocol::Ip4(ip))
                .with(Protocol::Udp(port))
                .with(Protocol::QuicV1);
            if let Some(peer) = peer.clone() {
                address.push(peer);
            }
            address
        })
        .collect()
}

/// Keys this device opens names with (those of its friends and of its groups), shared with
/// the code that picks the addresses of a dial.
#[derive(Clone, Debug, Default)]
pub(crate) struct LanKeys(Arc<Mutex<Vec<[u8; 32]>>>);

impl LanKeys {
    pub(crate) fn set(&self, keys: Vec<[u8; 32]>) {
        *crate::locked(&self.0) = keys;
    }

    /// The addresses in every name one of the keys opens.
    pub(crate) fn open_all(&self, names: &[Multiaddr]) -> Vec<Multiaddr> {
        let keys = crate::locked(&self.0);
        let mut out: Vec<Multiaddr> = Vec::new();
        for address in names.iter().flat_map(|name| open(name, &keys)) {
            if !out.contains(&address) {
                out.push(address);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn quic(text: &str) -> Multiaddr {
        text.parse().unwrap()
    }

    #[test]
    fn a_name_opens_for_its_key_only() {
        let key = key_of(&[3u8; 32]);
        let ips = [Ipv4Addr::new(192, 168, 1, 3), Ipv4Addr::new(10, 0, 0, 9)];
        let name = seal(&key, 31234, &ips).unwrap();
        assert!(is_name(&name));
        let host = name.iter().next().unwrap().to_string();
        assert!(host.len() < 63 + 6, "{host}");
        assert_eq!(
            open(&name, &[key_of(&[4u8; 32]), key]),
            vec![
                quic("/ip4/192.168.1.3/udp/31234/quic-v1"),
                quic("/ip4/10.0.0.9/udp/31234/quic-v1"),
            ]
        );
        assert!(open(&name, &[key_of(&[4u8; 32])]).is_empty());
        // The peer id a dial adds stays on.
        let peer = libp2p::PeerId::random();
        let with_peer = name.clone().with(Protocol::P2p(peer));
        assert_eq!(
            open(&with_peer, &[key])[0],
            quic("/ip4/192.168.1.3/udp/31234/quic-v1").with(Protocol::P2p(peer))
        );
        // Three addresses still fit in one DNS label.
        let three = [ips[0], ips[1], Ipv4Addr::new(172, 17, 0, 1)];
        let name = seal(&key, 31234, &three).unwrap();
        let Some(Protocol::Dns4(host)) = name.iter().next() else {
            unreachable!()
        };
        assert!(host.len() - DOMAIN.len() <= 63);
        assert_eq!(open(&name, &[key]).len(), 3);
    }

    #[test]
    fn a_name_with_a_port_devices_do_not_use_opens_to_nothing() {
        let key = key_of(&[3u8; 32]);
        let ips = [Ipv4Addr::new(192, 168, 1, 3)];
        let name = seal(&key, 31234, &ips).unwrap();
        // A DHT node puts another port in an announcement it keeps.
        let host = name.iter().next().unwrap();
        for port in [22, 5353, 19999, 40000, 41000] {
            let changed = Multiaddr::empty()
                .with(host.clone())
                .with(Protocol::Udp(port))
                .with(Protocol::QuicV1);
            assert!(open(&changed, &[key]).is_empty(), "port {port}");
        }
        assert_eq!(open(&name, &[key]).len(), 1);
    }

    #[test]
    fn only_names_under_the_domain_count() {
        assert!(is_name(&quic("/dns4/abc.FOGATA/udp/1/quic-v1")));
        assert!(!is_name(&quic("/dns4/abc.fogata.example/udp/1/quic-v1")));
        assert!(!is_name(&quic(
            "/dnsaddr/bootstrap.libp2p.io/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN"
        )));
        assert!(!is_name(&quic("/ip4/192.168.1.2/udp/1/quic-v1")));
    }

    #[test]
    fn every_announcement_carries_as_many_names_of_one_length() {
        let ips = [Ipv4Addr::new(192, 168, 1, 3), Ipv4Addr::new(10, 0, 0, 9)];
        let keys = [key_of(&[1u8; 32]), key_of(&[2u8; 32])];
        let hosts = |names: &[Multiaddr]| -> Vec<String> {
            names
                .iter()
                .map(|n| n.iter().next().unwrap().to_string())
                .collect()
        };
        for count in 0..=MAX_NAMES + 1 {
            let keys: Vec<[u8; 32]> = (0..count).map(|i| key_of(&[i as u8; 32])).collect();
            let names = names(31234, &ips, &keys);
            assert_eq!(names.len(), MAX_NAMES);
            let lengths: HashSet<usize> = hosts(&names).iter().map(String::len).collect();
            assert_eq!(lengths.len(), 1, "{names:?}");
            assert!(names.iter().all(is_name));
        }
        // The real ones open; the padding opens with no key.
        let sealed = names(31234, &ips, &keys);
        assert_eq!(open(&sealed[0], &[keys[0]]).len(), 2);
        assert_eq!(open(&sealed[1], &[keys[1]]).len(), 2);
        for padding in &sealed[2..] {
            assert!(open(padding, &keys).is_empty());
        }
        // Without local addresses, padding alone, as long as a name of one address.
        let none = names(31234, &[], &[]);
        assert_eq!(none.len(), MAX_NAMES);
        let one = seal(&keys[0], 31234, &ips[..1]).unwrap();
        assert_eq!(hosts(&none)[0].len(), hosts(&[one])[0].len());
    }

    #[test]
    fn an_announcement_proves_it_is_the_friend_s_own() {
        let bea = DeviceIdentity::generate().unwrap();
        let other = DeviceIdentity::generate().unwrap();
        let secret = [5u8; 32];
        let peer = PeerId::random();
        let window = 1000;
        let name = proof(&bea, &secret, &peer, window, 31234).unwrap();
        assert!(is_name(&name));
        // In labels a DNS name takes.
        let Some(Protocol::Dns4(host)) = name.iter().next() else {
            unreachable!()
        };
        assert!(host.split('.').all(|label| label.len() <= LABEL));
        assert!(host.len() <= 253);
        let ips = [Ipv4Addr::new(192, 168, 1, 3)];
        let mut announced = names(31234, &ips, &[key_of(&secret)]);
        announced.push(name.clone());
        let key = bea.public_key();
        assert!(proven(&announced, &key, &secret, &peer, window));
        // Around the window this device is in, not far from it.
        assert!(proven(&announced, &key, &secret, &peer, window + 2));
        assert!(proven(&announced, &key, &secret, &peer, window - 1));
        assert!(!proven(&announced, &key, &secret, &peer, window + 3));
        assert!(!proven(&announced, &key, &secret, &peer, window - 2));
        // Not for another peer that copies it, another device, or without the secret.
        assert!(!proven(
            &announced,
            &key,
            &secret,
            &PeerId::random(),
            window
        ));
        assert!(!proven(
            &announced,
            &other.public_key(),
            &secret,
            &peer,
            window
        ));
        assert!(!proven(&announced, &key, &[6u8; 32], &peer, window));
        // Someone else who knows the secret signs as itself only.
        let forged = proof(&other, &secret, &peer, window, 31234).unwrap();
        assert!(!proven(&[forged], &key, &secret, &peer, window));
        // It holds no local address for anyone.
        assert!(open(&name, &[key_of(&secret)]).is_empty());
    }

    #[test]
    fn local_addresses_are_ranked_and_capped() {
        let listen = [
            quic("/ip4/127.0.0.1/udp/31234/quic-v1"),
            quic("/ip4/172.17.0.1/udp/31234/quic-v1"),
            quic("/ip4/203.0.113.9/udp/31234/quic-v1"),
            quic("/ip4/10.1.2.3/udp/31234/quic-v1"),
            quic("/ip4/192.168.1.3/tcp/31234"),
            quic("/ip4/192.168.1.3/udp/31234/quic-v1"),
            quic("/ip4/100.70.1.1/udp/31234/quic-v1"),
            quic("/ip6/fd00::1/udp/31234/quic-v1"),
        ];
        let (port, ips) = local(&listen).unwrap();
        assert_eq!(port, 31234);
        assert_eq!(
            ips,
            [
                Ipv4Addr::new(192, 168, 1, 3),
                Ipv4Addr::new(10, 1, 2, 3),
                Ipv4Addr::new(100, 70, 1, 1),
            ]
        );
        assert!(local(&[quic("/ip4/203.0.113.9/udp/1/quic-v1")]).is_none());
    }
}
