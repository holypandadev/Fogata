//! Whether UDP gets out of the network this device is on, from what the node sees.
//!
//! QUIC runs over UDP. A QUIC connection with a public address that opens or answers a
//! ping shows that UDP gets through, for [`UDP_FRESH`]. That no QUIC connection opened
//! shows nothing by itself: a DHT node may only answer over TCP, and a dial to several
//! addresses stops at the first that answers. So the node asks a few peers it dialled
//! itself over TCP for a QUIC connection too, at the address of that TCP connection only
//! (another address would be one the peer chose, anywhere). A dial that timed out, or that
//! the system refused to send, counts when the TCP connection opened a moment before it, on
//! this network, or once a ping over it answers after: the peer was there all along. Several such peers, none answering over QUIC and
//! no dial still going, mean UDP does not get out. Any other end (a wrong peer, a refused
//! handshake) proves nothing either way. With less, the result stays unknown.

use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    time::{Duration, Instant},
};

use libp2p::{Multiaddr, PeerId, swarm::ConnectionId};

use crate::{UdpSeen, addr};

/// Peers alive over TCP that must not answer over QUIC before UDP counts as blocked.
pub(super) const BLOCKED_AFTER: usize = 3;

/// The most peers asked for a QUIC connection on one network: enough for [`BLOCKED_AFTER`]
/// when some DHT nodes do not answer over QUIC at all, and few enough to cost nothing.
pub(super) const MAX_PROBES: usize = 6;

/// The most addresses one peer is dialled at.
pub(super) const MAX_ADDRESSES: usize = 2;

/// How long after a TCP connection opened a timed-out QUIC dial to the same peer counts at
/// once: the peer answered over TCP just before, from this network.
pub(super) const FRESH_TCP: Duration = Duration::from_secs(30);

/// How long a QUIC connection with a public address that answered shows UDP works. Live
/// QUIC connections answer pings every 15 s.
pub(super) const UDP_FRESH: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
pub(super) struct UdpCheck {
    /// When a QUIC connection with a public address last opened or answered.
    works_at: Option<Instant>,
    /// The QUIC dials made to check, to whom, and whether the peer's TCP connection had just
    /// opened then.
    probing: HashMap<ConnectionId, (PeerId, bool)>,
    /// Dials made to check on a network left since: their end says nothing of this one.
    stale: HashSet<ConnectionId>,
    /// The peers asked, answered or not.
    probed: HashSet<PeerId>,
    /// Peers whose QUIC dial timed out, until their TCP connection answers.
    timed_out: HashSet<PeerId>,
    /// Peers alive over TCP whose QUIC dial timed out.
    failed: HashSet<PeerId>,
}

/// Whether `address` is one that proves UDP gets out once a QUIC connection with it answers.
pub(super) fn public_quic(address: &Multiaddr) -> bool {
    addr::is_quic(address) && !addr::is_local(address) && !addr::is_relayed(address)
}

impl UdpCheck {
    fn works(&self, now: Instant) -> bool {
        self.works_at
            .is_some_and(|at| now.saturating_duration_since(at) < UDP_FRESH)
    }

    /// What is known now.
    pub(super) fn seen(&self, now: Instant) -> UdpSeen {
        if self.works(now) {
            UdpSeen::Works
        } else if self.failed.len() >= BLOCKED_AFTER && self.probing.is_empty() {
            UdpSeen::Blocked
        } else {
            UdpSeen::Unknown
        }
    }

    /// A connection opened with `address`, by any dial or coming in.
    pub(super) fn connected(
        &mut self,
        connection: ConnectionId,
        address: &Multiaddr,
        now: Instant,
    ) {
        if self.stale.remove(&connection) {
            return;
        }
        self.probing.remove(&connection);
        if public_quic(address) {
            self.quic_answered(now);
        }
    }

    /// A QUIC connection with a public address opened or answered a ping.
    pub(super) fn quic_answered(&mut self, now: Instant) {
        self.works_at = Some(now);
        self.timed_out.clear();
        self.failed.clear();
    }

    /// A ping over a TCP connection to `peer` with a public address answered: a QUIC dial
    /// to it that timed out before counts.
    pub(super) fn tcp_answered(&mut self, peer: &PeerId) {
        if self.timed_out.remove(peer) {
            self.failed.insert(*peer);
        }
    }

    /// Whether `connection` is one of the check's dials (those of a network left included).
    pub(super) fn is_probe(&self, connection: &ConnectionId) -> bool {
        self.probing.contains_key(connection) || self.stale.contains(connection)
    }

    /// A check's dial failed; `udp_lost` when it timed out or the system refused to send.
    pub(super) fn dial_failed(&mut self, connection: ConnectionId, udp_lost: bool) {
        if self.stale.remove(&connection) {
            return;
        }
        if let Some((peer, fresh)) = self.probing.remove(&connection)
            && udp_lost
        {
            if fresh {
                self.failed.insert(peer);
            } else {
                self.timed_out.insert(peer);
            }
        }
    }

    /// The addresses to ask `peer` at over QUIC, when it is worth asking: UDP is not known
    /// to work, this node dialled `peer` over TCP at the public address `tcp_ip`, `peer` was
    /// not asked before on this network, and the check has dials left. Only the QUIC
    /// addresses `peer` listens on at that same address, at most [`MAX_ADDRESSES`].
    pub(super) fn probe_addresses(
        &self,
        peer: &PeerId,
        tcp_ip: Option<IpAddr>,
        listening: &[Multiaddr],
        now: Instant,
    ) -> Vec<Multiaddr> {
        let Some(tcp_ip) = tcp_ip else {
            return Vec::new();
        };
        if self.works(now) || self.probed.contains(peer) || self.probed.len() >= MAX_PROBES {
            return Vec::new();
        }
        let mut out: Vec<Multiaddr> = Vec::new();
        for address in listening {
            if public_quic(address) && addr::ip(address) == Some(tcp_ip) {
                let address = addr::without_peer(address);
                if !out.contains(&address) {
                    out.push(address);
                }
            }
        }
        out.truncate(MAX_ADDRESSES);
        out
    }

    /// A check dial to `peer` went out as `connection`; `fresh` when the peer's TCP
    /// connection opened a moment before ([`FRESH_TCP`]), on this network.
    pub(super) fn probing(&mut self, connection: ConnectionId, peer: PeerId, fresh: bool) {
        self.probed.insert(peer);
        self.probing.insert(connection, (peer, fresh));
    }

    /// The peers being asked now, for tests.
    #[cfg(test)]
    pub(super) fn asking(&self) -> Vec<PeerId> {
        self.probing.values().map(|(peer, _)| *peer).collect()
    }

    /// The check's dial to `peer` going now, for tests.
    #[cfg(test)]
    pub(super) fn dial_to(&self, peer: &PeerId) -> Option<ConnectionId> {
        self.probing
            .iter()
            .find(|(_, (p, _))| p == peer)
            .map(|(id, _)| *id)
    }

    /// This device's network changed: what was seen before says nothing of the new one,
    /// and the check's dials still going are left out of it.
    pub(super) fn forget(&mut self) {
        let mut stale = std::mem::take(&mut self.stale);
        stale.extend(self.probing.keys().copied());
        *self = Self {
            stale,
            ..Self::default()
        };
    }
}

/// Whether a failed QUIC dial says UDP did not get through: it timed out, or the system
/// refused to send (a local firewall). Anything else (a TLS or handshake error, a wrong
/// peer) means packets went both ways, or proves nothing.
pub(super) fn udp_lost(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut next: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(e) = next {
        if let Some(io) = e.downcast_ref::<std::io::Error>() {
            if matches!(
                io.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::PermissionDenied
            ) {
                return true;
            }
            // `io::Error::source` skips what it wraps: look at it itself.
            if let Some(inner) = io.get_ref() {
                next = Some(inner);
                continue;
            }
        }
        if let Some(quic) = e.downcast_ref::<libp2p::quic::Error>() {
            match quic {
                libp2p::quic::Error::HandshakeTimedOut => return true,
                libp2p::quic::Error::Io(io) => {
                    next = Some(io);
                    continue;
                }
                libp2p::quic::Error::Connection(c) => {
                    return c.to_string().contains("timed out");
                }
                _ => return false,
            }
        }
        if e.downcast_ref::<
            libp2p::core::transport::timeout::TransportTimeoutError<std::io::Error>,
        >()
        .is_some_and(|t| {
            matches!(
                t,
                libp2p::core::transport::timeout::TransportTimeoutError::Timeout
            )
        }) {
            return true;
        }
        next = e.source();
    }
    // The DNS transport gathers the errors of its attempts in a list of its own, generic over
    // the transport below and without a source: read what it says. Every attempt must have
    // timed out or been refused by the system.
    every_attempt_lost(&error.to_string())
}

/// What a dial error's own words say: each attempt in `text` ("Multiple dial errors
/// occurred:\n - …") timed out or could not be sent.
fn every_attempt_lost(text: &str) -> bool {
    const LOST: [&str; 4] = [
        "timed out",
        "Timeout has been reached",
        "Operation not permitted",
        "Permission denied",
    ];
    let leaves: Vec<&str> = text
        .split("\n - ")
        .flat_map(|part| part.split(": "))
        .map(str::trim)
        .filter(|leaf| !leaf.is_empty() && !leaf.starts_with("Multiple dial errors occurred"))
        .collect();
    !leaves.is_empty()
        && leaves
            .iter()
            .all(|leaf| LOST.iter().any(|l| leaf.contains(l)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    const QUIC: &str = "/ip4/198.51.100.7/udp/4001/quic-v1";
    fn tcp_ip() -> Option<IpAddr> {
        Some("198.51.100.7".parse().unwrap())
    }

    /// Asks `peer`, lets the dial time out and the peer answer over TCP afterwards.
    fn fails(check: &mut UdpCheck, peer: PeerId, now: Instant) {
        let to = check.probe_addresses(&peer, tcp_ip(), &[a(QUIC)], now);
        assert_eq!(to, [a(QUIC)]);
        let connection = ConnectionId::new_unchecked(check.probed.len() + 1000);
        check.probing(connection, peer, false);
        check.dial_failed(connection, true);
        check.tcp_answered(&peer);
    }

    #[test]
    fn nothing_seen_is_unknown() {
        assert_eq!(UdpCheck::default().seen(Instant::now()), UdpSeen::Unknown);
    }

    #[test]
    fn a_public_quic_connection_shows_udp_works_for_a_while() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        // Local, relayed and TCP connections say nothing about UDP to the internet.
        for address in [
            "/ip4/192.168.1.4/udp/4001/quic-v1",
            "/ip4/198.51.100.7/tcp/4001",
            "/ip4/198.51.100.7/udp/4001/quic-v1/p2p/12D3KooWLRPJAA5o6XuZ9DBbnZVkBHcsGFYuGcwX6ydGcATUm5uz/p2p-circuit",
        ] {
            check.connected(ConnectionId::new_unchecked(1), &a(address), now);
        }
        assert_eq!(check.seen(now), UdpSeen::Unknown);
        check.connected(ConnectionId::new_unchecked(2), &a(QUIC), now);
        assert_eq!(check.seen(now), UdpSeen::Works);
        // Once it works, nobody is asked.
        assert!(
            check
                .probe_addresses(&PeerId::random(), tcp_ip(), &[a(QUIC)], now)
                .is_empty()
        );
        // Nothing heard over QUIC for long: not known any more.
        let later = now + UDP_FRESH;
        assert_eq!(check.seen(later), UdpSeen::Unknown);
        check.quic_answered(later);
        assert_eq!(check.seen(later), UdpSeen::Works);
    }

    #[test]
    fn udp_is_blocked_once_enough_peers_alive_over_tcp_do_not_answer_over_quic() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        for _ in 0..BLOCKED_AFTER - 1 {
            fails(&mut check, PeerId::random(), now);
            // Fewer is not enough: some DHT nodes only answer over TCP.
            assert_eq!(check.seen(now), UdpSeen::Unknown);
        }
        fails(&mut check, PeerId::random(), now);
        assert_eq!(check.seen(now), UdpSeen::Blocked);
        // A QUIC connection after all says otherwise.
        check.connected(ConnectionId::new_unchecked(7), &a(QUIC), now);
        assert_eq!(check.seen(now), UdpSeen::Works);
    }

    #[test]
    fn a_timeout_counts_only_once_the_peer_answers_over_tcp() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        let peers: Vec<PeerId> = (0..BLOCKED_AFTER).map(|_| PeerId::random()).collect();
        for (i, peer) in peers.iter().enumerate() {
            check.probing(ConnectionId::new_unchecked(i), *peer, false);
            check.dial_failed(ConnectionId::new_unchecked(i), true);
        }
        // The TCP connections may have died with the network: nothing yet.
        assert_eq!(check.seen(now), UdpSeen::Unknown);
        for peer in &peers {
            check.tcp_answered(peer);
        }
        assert_eq!(check.seen(now), UdpSeen::Blocked);
    }

    #[test]
    fn a_timeout_counts_at_once_when_the_tcp_connection_had_just_opened() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        for i in 0..BLOCKED_AFTER {
            check.probing(ConnectionId::new_unchecked(i), PeerId::random(), true);
            check.dial_failed(ConnectionId::new_unchecked(i), true);
        }
        assert_eq!(check.seen(now), UdpSeen::Blocked);
    }

    #[test]
    fn other_failures_and_dials_still_going_say_nothing() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        let refused = PeerId::random();
        check.probing(ConnectionId::new_unchecked(1), refused, false);
        check.dial_failed(ConnectionId::new_unchecked(1), false);
        check.tcp_answered(&refused);
        assert!(check.failed.is_empty());
        for _ in 0..BLOCKED_AFTER {
            fails(&mut check, PeerId::random(), now);
        }
        check.probing(ConnectionId::new_unchecked(2), PeerId::random(), false);
        assert_eq!(check.seen(now), UdpSeen::Unknown);
        check.dial_failed(ConnectionId::new_unchecked(2), false);
        assert_eq!(check.seen(now), UdpSeen::Blocked);
        // Failures of dials that were no check do not count.
        assert!(!check.is_probe(&ConnectionId::new_unchecked(3)));
    }

    #[test]
    fn one_peer_failing_again_counts_once() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        let peer = PeerId::random();
        fails(&mut check, peer, now);
        // Not asked twice.
        assert!(
            check
                .probe_addresses(&peer, tcp_ip(), &[a(QUIC)], now)
                .is_empty()
        );
        assert_eq!(check.failed.len(), 1);
    }

    #[test]
    fn only_the_tcp_connections_own_address_is_asked_at() {
        let now = Instant::now();
        let check = UdpCheck::default();
        let peer = PeerId::random();
        // Not dialled over TCP by this node: nobody to ask.
        assert!(
            check
                .probe_addresses(&peer, None, &[a(QUIC)], now)
                .is_empty()
        );
        let listening = [
            a("/ip4/10.0.0.3/udp/4001/quic-v1"),
            a("/ip4/198.51.100.7/tcp/4001"),
            // Another address the peer says it has: anyone's, maybe.
            a("/ip4/203.0.113.66/udp/4001/quic-v1"),
            a("/ip6/2001:db8::7/udp/4001/quic-v1"),
            a(&format!("{QUIC}/p2p/{peer}")),
            a("/ip4/198.51.100.7/udp/4002/quic-v1"),
            a("/ip4/198.51.100.7/udp/4003/quic-v1"),
        ];
        assert_eq!(
            check.probe_addresses(&peer, tcp_ip(), &listening, now),
            [a(QUIC), a("/ip4/198.51.100.7/udp/4002/quic-v1")]
        );
    }

    #[test]
    fn a_bounded_number_of_peers_is_asked() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        for i in 0..MAX_PROBES {
            check.probing(ConnectionId::new_unchecked(i), PeerId::random(), false);
            check.dial_failed(ConnectionId::new_unchecked(i), false);
        }
        assert!(
            check
                .probe_addresses(&PeerId::random(), tcp_ip(), &[a(QUIC)], now)
                .is_empty()
        );
    }

    #[test]
    fn a_new_network_forgets_what_was_seen_and_the_dials_going() {
        let now = Instant::now();
        let mut check = UdpCheck::default();
        for _ in 0..BLOCKED_AFTER {
            fails(&mut check, PeerId::random(), now);
        }
        let going = ConnectionId::new_unchecked(77);
        check.probing(going, PeerId::random(), false);
        check.forget();
        assert_eq!(check.seen(now), UdpSeen::Unknown);
        assert_eq!(
            check.probe_addresses(&PeerId::random(), tcp_ip(), &[a(QUIC)], now),
            [a(QUIC)]
        );
        // A dial of the old network that connects now is not taken for this one.
        assert!(check.is_probe(&going));
        check.connected(going, &a(QUIC), now);
        assert_eq!(check.seen(now), UdpSeen::Unknown);
        assert!(!check.is_probe(&going));
    }

    #[test]
    fn only_timeouts_and_refused_sends_mean_udp_was_lost() {
        use std::io::{Error, ErrorKind};
        let lost = |e: Error| udp_lost(&e);
        assert!(lost(Error::new(ErrorKind::TimedOut, "timed out")));
        assert!(lost(Error::new(ErrorKind::PermissionDenied, "sendmsg")));
        assert!(lost(Error::other(libp2p::quic::Error::HandshakeTimedOut)));
        assert!(lost(Error::other(libp2p::quic::Error::Io(Error::new(
            ErrorKind::PermissionDenied,
            "sendmsg"
        )))));
        assert!(lost(Error::other(
            libp2p::core::transport::timeout::TransportTimeoutError::<Error>::Timeout
        )));
        assert!(!lost(Error::other("tls: certificate of another peer")));
        assert!(!lost(Error::new(ErrorKind::ConnectionRefused, "refused")));
        assert!(!lost(Error::other(
            libp2p::quic::Error::NoActiveListenerForDialAsListener
        )));
        // As the DNS transport words a list of attempts.
        let timed_out = "Multiple dial errors occurred:\n - Handshake with the remote timed out.: Multiple dial errors occurred:\n - Handshake with the remote timed out.: Handshake with the remote timed out.";
        assert!(lost(Error::other(timed_out)));
        let mixed = "Multiple dial errors occurred:\n - Handshake with the remote timed out.\n - Handshake failed: tls: bad certificate";
        assert!(!lost(Error::other(mixed)));
        assert!(!lost(Error::other("Multiple dial errors occurred:")));
    }
}
