//! The network this device is on: what others see of it (its public addresses, taken once
//! peers on two networks agree), whether the internet and UDP answer, and what a change of
//! network forgets and tries again.

use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    time::{Duration, Instant},
};

use libp2p::{
    Multiaddr, PeerId,
    multiaddr::Protocol,
    swarm::{
        ConnectionId, DialError,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use super::{ADDRESSES_WAIT, ConnectionInfo, Node, STARTUP, is_loopback, udp_check};
use crate::{NetworkReport, NodeEvent, addr};

/// Addresses others report for this device, and reporters per address, kept at most.
const MAX_OBSERVED: usize = 64;
const MAX_REPORTERS: usize = 64;
/// A confirmed address nobody has reported for this long is dropped.
const CONFIRMED_FOR: Duration = Duration::from_secs(15 * 60);
/// Networks ([`Network`]) whose peers must agree on an address of this device before it is
/// taken: with a member or friend among them, and with strangers only, one of them a
/// stranger this device dialled (see `Report`). Anyone can connect and say anything; two strangers'
/// providers are easy to come by, and the address taken is announced to everyone who looks
/// this device up.
const NETWORKS_WITH_CONTACT: usize = 2;
const NETWORKS_OF_STRANGERS: usize = 3;

/// Whether a failed check dial says UDP did not get through (see udp_check.rs): every
/// address tried timed out or could not be sent to.
pub(super) fn probe_lost_udp(error: &DialError) -> bool {
    match error {
        DialError::Transport(attempts) => {
            !attempts.is_empty()
                && attempts.iter().all(|(_, e)| match e {
                    libp2p::core::transport::TransportError::Other(e) => udp_check::udp_lost(e),
                    _ => false,
                })
        }
        _ => false,
    }
}

/// The /64 of a global IPv6 address: temporary addresses come and go within it.
pub(super) fn v6_net(address: &Multiaddr) -> Option<[u16; 4]> {
    match addr::ip(address) {
        Some(IpAddr::V6(ip)) if !addr::is_local(address) => {
            let s = ip.segments();
            Some([s[0], s[1], s[2], s[3]])
        }
        _ => None,
    }
}

/// Network changes that come this close together (an interface going down and another
/// coming up, each with its addresses) are taken as one.
const NETWORK_SETTLE: Duration = Duration::from_secs(2);

/// What one peer reported of this device's address: the address it reported from, and
/// whether this device dialled it itself. Strangers on [`NETWORKS_OF_STRANGERS`] networks
/// make an address taken, whoever connected, as before, but only once one of them is a
/// stranger this device dialled (see [`Node::confirm_if_agreed`]): anyone can connect
/// from wherever it likes, and three cheap machines at one cloud provider's three /16s that
/// connect by themselves would otherwise have this device announce an address of their
/// choice, which its members and friends then dial first. The peers it dials (the bootstrap
/// nodes, those its own lookups lead to) tell it its real address as soon as it starts; a
/// device that hears only from strangers that came in counts their networks as before
/// once one it dialled says the same.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Report {
    from: Option<std::net::IpAddr>,
    dialled: bool,
}

/// How many different addresses the reporters of an address reported from.
fn distinct_ips(reporters: &HashMap<PeerId, Report>) -> usize {
    reporters
        .values()
        .filter_map(|r| r.from)
        .collect::<HashSet<_>>()
        .len()
}

/// The network an address is in, as far as telling machines apart goes: its /16 for IPv4,
/// its /48 for IPv6. One machine, or a few at one provider, can take many addresses within
/// one, and pose as peers at each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Network {
    V4([u8; 2]),
    V6([u8; 6]),
}

impl Network {
    fn of(ip: &std::net::IpAddr) -> Self {
        match ip {
            std::net::IpAddr::V4(ip) => {
                let o = ip.octets();
                Network::V4([o[0], o[1]])
            }
            std::net::IpAddr::V6(ip) => {
                let o = ip.octets();
                Network::V6([o[0], o[1], o[2], o[3], o[4], o[5]])
            }
        }
    }
}

/// How many different networks ([`Network`]) the addresses in `ips` are in.
fn distinct_networks<'a>(ips: impl IntoIterator<Item = &'a std::net::IpAddr>) -> usize {
    ips.into_iter()
        .map(Network::of)
        .collect::<HashSet<_>>()
        .len()
}

fn udp_port(address: &Multiaddr) -> Option<u16> {
    address.iter().find_map(|p| match p {
        Protocol::Udp(port) => Some(port),
        _ => None,
    })
}

impl Node {
    /// This device joined or left a network while running. Peers are tried again at once;
    /// when a network was left, the public addresses others saw are forgotten until they
    /// are confirmed again, since they may belong to the network that is gone.
    pub(super) fn network_changed(&mut self, left: bool) {
        if left {
            self.confirmed_seen.clear();
            for address in self.confirmed.drain().collect::<Vec<_>>() {
                self.swarm.remove_external_address(&address);
            }
            self.observed.clear();
        }
        self.forget_network_view();
        // Part of a burst: acted on once, at the next slot, when the addresses have settled.
        if self
            .last_network_change
            .is_some_and(|t| t.elapsed() < NETWORK_SETTLE)
        {
            self.network_pending = Some(self.network_pending.unwrap_or(false) || left);
            return;
        }
        self.after_network_change(left);
    }

    /// Forgets what others saw of this device at `address`'s IP, which it no longer has.
    pub(super) fn forget_own_address(&mut self, address: &Multiaddr) {
        let ip = addr::ip(address);
        let gone: Vec<Multiaddr> = self
            .confirmed
            .iter()
            .filter(|a| addr::ip(a) == ip)
            .cloned()
            .collect();
        for a in gone {
            self.confirmed.remove(&a);
            self.confirmed_seen.remove(&a);
            self.swarm.remove_external_address(&a);
        }
        self.observed.retain(|a, _| addr::ip(a) != ip);
    }

    /// Tries friends and members again after a network change.
    pub(super) fn after_network_change(&mut self, left: bool) {
        tracing::info!("the network changed; trying friends and members again");
        self.last_network_change = Some(Instant::now());
        self.announced = false;
        if left {
            // QUIC links to members may be dead without knowing it until they time out:
            // new ones carry media at once.
            let members: Vec<PeerId> = self.members.keys().copied().collect();
            for peer in members {
                let quic = self.member_quic_addresses(&peer);
                if !quic.is_empty() {
                    self.quic_tried.insert(peer);
                    self.dial_quic(peer, quic);
                }
            }
        }
        self.retry_now();
    }

    pub(super) fn network_report(&mut self) -> NetworkReport {
        let mut observed: Vec<(Multiaddr, usize)> = self
            .observed
            .iter()
            .map(|(a, peers)| (a.clone(), peers.len()))
            .collect();
        observed.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let dht_peers = self
            .swarm
            .behaviour_mut()
            .kad
            .as_mut()
            .map(|kad| kad.kbuckets().map(|b| b.num_entries()).sum())
            .unwrap_or(0);
        let quic = self
            .connections
            .values()
            .filter(|c| addr::is_quic(&c.address))
            .count();
        NetworkReport {
            listening: self.listen_addrs.clone(),
            observed,
            confirmed: self.confirmed.iter().cloned().collect(),
            port_changes: self.port_change_reported,
            dht_peers,
            quic_connections: quic,
            tcp_connections: self.connections.len() - quic,
            lan_devices: self.lan_seen.len(),
            dns_note: self.dns_note.clone(),
            udp: self.udp.seen(Instant::now()),
            contact: self.public_contact.map(|at| at.elapsed()),
            failed_dials: self.failed_public_dials,
            settled: self
                .last_network_change
                .map_or(self.started, |t| t.max(self.started))
                .elapsed()
                >= crate::SETTLED_AFTER,
        }
    }

    /// Addresses at which this node can be reached: those others confirmed first, then the
    /// local network ones.
    pub(super) fn own_addresses(&self) -> Vec<Multiaddr> {
        let local = self.local_peer();
        self.confirmed
            .iter()
            .chain(self.listen_addrs.iter().filter(|a| !is_loopback(a)))
            .map(|a| a.clone().with(Protocol::P2p(local)))
            .collect()
    }

    /// Answers a request for this device's addresses (an invite's, say). Asked in the first
    /// moments after the start, before the system told what this device listens on, the
    /// answer would be empty, and an invite made with it would lead nobody here: the one
    /// who uses it would wait for the members' next heartbeat, or for the DHT. So it waits
    /// for the first address on a network, or [`ADDRESSES_WAIT`] after the start on a
    /// device on none.
    pub(super) fn answer_addresses(&mut self, reply: tokio::sync::oneshot::Sender<Vec<Multiaddr>>) {
        self.asked_addresses.push(reply);
        self.answer_waiting_addresses();
    }

    /// Answers the requests for this device's addresses that wait (see
    /// [`Self::answer_addresses`]), once it listens on a network or waited long enough.
    pub(super) fn answer_waiting_addresses(&mut self) {
        if self.asked_addresses.is_empty()
            || (self.listen_addrs.is_empty() && self.started.elapsed() < ADDRESSES_WAIT)
        {
            return;
        }
        let addresses = self.own_addresses();
        for reply in self.asked_addresses.drain(..) {
            let _ = reply.send(addresses.clone());
        }
    }

    /// Asks `peer`, which this node dialled over TCP, for a QUIC connection too while it is
    /// not known whether UDP gets out of this network (see udp_check.rs).
    pub(super) fn check_udp(&mut self, peer: PeerId) {
        let tcp = self
            .dialled_tcp
            .iter()
            .find(|(id, _)| {
                self.connections.get(id).is_some_and(|c| c.peer == peer)
                    && !self.closing.contains(id)
            })
            .map(|(_, tcp)| *tcp);
        let tcp_ip = tcp.map(|(ip, _)| ip);
        // Opened a moment ago, on this network (a change of network asks again, and its
        // older connections count only once they answer a ping).
        let fresh = tcp.is_some_and(|(_, at)| {
            at.elapsed() < udp_check::FRESH_TCP
                && self.last_network_change.is_none_or(|change| at > change)
        });
        let listening = self
            .peer_listening
            .get(&peer)
            .map_or(&[][..], Vec::as_slice);
        let quic = self
            .udp
            .probe_addresses(&peer, tcp_ip, listening, Instant::now());
        if quic.is_empty() {
            return;
        }
        let opts = DialOpts::peer_id(peer)
            .addresses(quic)
            .condition(PeerCondition::Always)
            .build();
        let connection = opts.connection_id();
        if self.swarm.dial(opts).is_ok() {
            tracing::debug!(%peer, "asking a peer over QUIC whether UDP gets out");
            self.udp.probing(connection, peer, fresh);
        }
    }

    /// A dial failed: one to a public address counts towards telling that nothing on the
    /// internet answers.
    pub(super) fn count_failed_dial(&mut self, error: &DialError) {
        if let DialError::Transport(attempts) = error
            && attempts
                .iter()
                .any(|(a, _)| !addr::is_local(a) && !addr::is_relayed(a) && addr::ip(a).is_some())
        {
            self.failed_public_dials += 1;
        }
    }

    /// A peer at a public address connected or answered: this device reaches the internet.
    pub(super) fn public_contact(&mut self) {
        self.public_contact = Some(Instant::now());
        self.failed_public_dials = 0;
    }

    /// `connection` to `peer` answered a ping: over a public address, the internet answers,
    /// and UDP gets out (QUIC) or the peer is there for the UDP check (TCP).
    pub(super) fn pinged(&mut self, peer: PeerId, connection: ConnectionId) {
        let Some(address) = self.connections.get(&connection).map(|c| c.address.clone()) else {
            return;
        };
        if addr::is_local(&address) || addr::is_relayed(&address) || addr::ip(&address).is_none() {
            return;
        }
        self.public_contact();
        if addr::is_quic(&address) {
            self.udp.quic_answered(Instant::now());
        } else {
            self.udp.tcp_answered(&peer);
        }
    }

    /// The network changed: the router seen before, whether UDP got out and who answered
    /// say nothing of the new one. Peers still connected over TCP are asked over QUIC again;
    /// a timeout counts only once they answer over TCP on this network, and live QUIC
    /// connections count again at their next ping.
    fn forget_network_view(&mut self) {
        self.port_change_reported = false;
        self.udp.forget();
        self.public_contact = None;
        self.failed_public_dials = 0;
        let peers: HashSet<PeerId> = self
            .dialled_tcp
            .keys()
            .filter_map(|id| self.connections.get(id).map(|c| c.peer))
            .collect();
        for peer in peers {
            self.check_udp(peer);
        }
    }

    /// Public IPs others have confirmed for this device, a member or a friend among them.
    /// Candidates behind them are taken for this device's own old sessions and rested, and
    /// are not punched: strangers at two addresses (DHT nodes) could otherwise make this
    /// device take another member's address for its own, and stop trying to reach it.
    pub(super) fn own_public_ips(&self) -> HashSet<std::net::IpAddr> {
        self.confirmed
            .iter()
            .filter(|a| {
                self.observed.get(*a).is_some_and(|reporters| {
                    reporters
                        .keys()
                        .any(|p| self.members.contains_key(p) || self.is_friend_peer(p))
                })
            })
            .filter_map(addr::ip)
            .collect()
    }

    pub(super) fn on_observed_address(&mut self, reporter: PeerId, observed: Multiaddr) {
        if !addr::is_quic(&observed) || addr::is_local(&observed) || addr::is_relayed(&observed) {
            return;
        }
        let observed = addr::without_peer(&observed);
        if self.confirmed.contains(&observed) {
            self.confirmed_seen.insert(observed.clone(), Instant::now());
        }
        // Strangers report what they like: a bounded number of addresses and reporters.
        if !self.observed.contains_key(&observed) && self.observed.len() >= MAX_OBSERVED {
            self.observed
                .retain(|a, r| r.len() > 1 || self.confirmed.contains(a));
            if self.observed.len() >= MAX_OBSERVED {
                return;
            }
        }
        // Whether this device dialled the reporter itself (at an address, or at a name), and
        // where.
        let dialled: Vec<&ConnectionInfo> = self
            .connections
            .iter()
            .filter(|(id, c)| c.peer == reporter && self.dialled_public.contains(id))
            .map(|(_, c)| c)
            .collect();
        let report = Report {
            from: dialled
                .iter()
                .find_map(|c| addr::ip(&c.address))
                .or_else(|| self.peer_ip(&reporter)),
            dialled: !dialled.is_empty(),
        };
        let reporters = self.observed.entry(observed.clone()).or_default();
        if let Some(known) = reporters.get_mut(&reporter) {
            // Said again over a connection this device dialled.
            if report.dialled && !known.dialled {
                *known = report;
            }
        } else if reporters.len() < MAX_REPORTERS {
            reporters.insert(reporter, report);
            // One line per milestone instead of one per peer keeps the log readable while
            // still showing how many peers agree on each address.
            let count = reporters.len();
            if [1, 2, 5, 10, 25, 50].contains(&count) {
                let dialled = reporters.values().filter(|r| r.dialled).count();
                tracing::debug!(%observed, peers = count, dialled, "address observed by peers");
            } else {
                tracing::trace!(%reporter, %observed, "address observed by a peer");
            }
        }
        self.confirm_if_agreed(&observed);

        // Same public IP seen with different ports, each from at least two addresses: this
        // NAT picks a new port per destination, which only a member in between can work
        // around. A single report, or several from one machine, is not enough.
        if !self.port_change_reported {
            let ip = addr::ip(&observed);
            let same_ip: Vec<&HashMap<PeerId, Report>> = self
                .observed
                .iter()
                .filter(|(a, _)| addr::ip(a) == ip && udp_port(a).is_some())
                .map(|(_, reporters)| reporters)
                .collect();
            let confirmed_ports = same_ip.iter().filter(|r| distinct_ips(r) >= 2).count();
            let all_ips: HashSet<std::net::IpAddr> = same_ip
                .iter()
                .flat_map(|r| r.values().filter_map(|r| r.from))
                .collect();
            // A member, a friend or a stranger this device dialled among them (see `Report`).
            let vouched = same_ip
                .iter()
                .flat_map(|r| r.iter())
                .any(|(p, r)| r.dialled || self.members.contains_key(p) || self.is_friend_peer(p));
            let all_different = all_ips.len() >= 3 && same_ip.iter().all(|r| r.len() == 1);
            if confirmed_ports >= 2 || all_different {
                self.port_change_reported = true;
                self.emit(NodeEvent::PortChangesPerDestination);
                // No port is right for everyone, but announcing the public IP still lets
                // members whose router filters by address only open it for us: once peers
                // on three networks said so, one of them dialled by this device, as for any
                // address taken by strangers (see `Report`).
                if self.confirmed.is_empty()
                    && vouched
                    && distinct_networks(&all_ips) >= NETWORKS_OF_STRANGERS
                    && self.confirmed.insert(observed.clone())
                {
                    tracing::info!(
                        %observed,
                        networks = distinct_networks(&all_ips),
                        "this device's public address taken, its port changing per destination"
                    );
                    self.confirmed_seen.insert(observed.clone(), Instant::now());
                    self.swarm.add_external_address(observed);
                }
            }
        }
    }

    /// Takes `observed` for this device's address once peers on enough networks say so (see
    /// [`NETWORKS_WITH_CONTACT`]): one machine, or a few at one or two providers, posing as
    /// many peers (fake DHT nodes, say) cannot make this device announce an address of its
    /// choice or take members for being behind its own address. Only a member or friend
    /// among them makes this device take it as having moved, which looks for everyone
    /// again: strangers could otherwise have it do so with every address they make up.
    fn confirm_if_agreed(&mut self, observed: &Multiaddr) {
        let Some(reporters) = self.observed.get(observed) else {
            return;
        };
        if self.confirmed.contains(observed) {
            return;
        }
        let contact = reporters
            .keys()
            .any(|p| self.members.contains_key(p) || self.is_friend_peer(p));
        // Every reporter's network counts, as before; strangers only alone once one of them
        // is one this device dialled (see `Report`).
        let networks = distinct_networks(reporters.values().filter_map(|r| r.from.as_ref()));
        let dialled = reporters.values().filter(|r| r.dialled).count();
        let needed = if contact {
            NETWORKS_WITH_CONTACT
        } else {
            NETWORKS_OF_STRANGERS
        };
        if networks < needed {
            return;
        }
        if !contact && dialled == 0 {
            tracing::debug!(
                %observed,
                reporters = reporters.len(),
                networks,
                "strangers that came in by themselves agree on an address of this device; \
                 not taken until one it dialled says so too"
            );
            return;
        }
        tracing::info!(
            %observed,
            reporters = reporters.len(),
            networks,
            dialled,
            contact,
            "this device's public address confirmed"
        );
        let ip = addr::ip(observed);
        let same_family = |a: &Multiaddr| {
            addr::ip(a).is_some_and(|a| Some(a.is_ipv4()) == ip.map(|ip| ip.is_ipv4()))
        };
        // Another public address than the ones known, with no local change: the router or
        // the carrier gave this device a new one (or it moved without noticing).
        let moved = contact
            && self.started.elapsed() > STARTUP
            && self.confirmed.iter().any(same_family)
            && !self.confirmed.iter().any(|a| addr::ip(a) == ip);
        self.confirmed.insert(observed.clone());
        self.confirmed_seen.insert(observed.clone(), Instant::now());
        self.swarm.add_external_address(observed.clone());
        self.emit(NodeEvent::ExternalAddress(observed.clone()));
        // Announced again at once: the others look for this device where it is now.
        self.provided.1 = None;
        if moved {
            tracing::info!(%observed, "this device's public address changed");
            self.network_changed(false);
        }
    }

    /// `peer` just proved membership: the addresses it reported for this device before
    /// that, which may now be agreed on (see [`Self::confirm_if_agreed`]).
    pub(super) fn reported_by_member(&mut self, peer: PeerId) {
        let reported: Vec<Multiaddr> = self
            .observed
            .iter()
            .filter(|(a, reporters)| reporters.contains_key(&peer) && !self.confirmed.contains(*a))
            .map(|(a, _)| a.clone())
            .collect();
        for observed in reported {
            self.confirm_if_agreed(&observed);
        }
    }

    /// Drops confirmed addresses nobody has reported for a while.
    pub(super) fn forget_stale_addresses(&mut self) {
        let stale: Vec<Multiaddr> = self
            .confirmed
            .iter()
            .filter(|a| {
                self.confirmed_seen
                    .get(*a)
                    .is_none_or(|seen| seen.elapsed() > CONFIRMED_FOR)
            })
            .cloned()
            .collect();
        for address in stale {
            tracing::info!(%address, "nobody reports this address any more");
            self.confirmed.remove(&address);
            self.confirmed_seen.remove(&address);
            self.observed.remove(&address);
            self.swarm.remove_external_address(&address);
            self.provided.1 = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use libp2p::{core::ConnectedPoint, swarm::SwarmEvent};

    use super::*;
    use crate::node::tests::{inspect, lone_member};

    fn quic_at(ip: &str) -> Multiaddr {
        if ip.contains(':') {
            format!("/ip6/{ip}/udp/4001/quic-v1")
        } else {
            format!("/ip4/{ip}/udp/4001/quic-v1")
        }
        .parse()
        .unwrap()
    }

    /// A connection this device dialled to `ip`.
    fn dialled_at(ip: &str) -> ConnectedPoint {
        ConnectedPoint::Dialer {
            address: quic_at(ip),
            role_override: libp2p::core::Endpoint::Dialer,
            port_use: libp2p::core::transport::PortUse::Reuse,
        }
    }

    /// A connection that came in from `ip`.
    fn came_in_from(ip: &str) -> ConnectedPoint {
        ConnectedPoint::Listener {
            local_addr: "/ip4/0.0.0.0/udp/4001/quic-v1".parse().unwrap(),
            send_back_addr: quic_at(ip),
        }
    }

    /// Strangers that connected by themselves, from three networks, cannot have this device
    /// take and announce an address of their choice, nor take one for a port that changes
    /// per destination. Once one stranger this device dialled says the same, the networks
    /// of all count as before: a bootstrap node dialled by its name counts as dialled, and
    /// two DHT nodes dialled and a device that came in through its router, on three
    /// networks, confirm its address (the network lab's case).
    #[tokio::test(flavor = "multi_thread")]
    async fn strangers_that_came_in_by_themselves_do_not_confirm_an_address() {
        let (node, _group) = lone_member();
        let found = inspect(&node, |n| {
            let mut next = 9700;
            let mut connect = |n: &mut Node, peer: PeerId, how: ConnectedPoint| {
                next += 1;
                n.on_connection_established(peer, ConnectionId::new_unchecked(next), &how);
            };
            let made_up: Multiaddr = "/ip4/198.51.100.66/udp/4001/quic-v1".parse().unwrap();
            for from in ["203.0.113.10", "192.0.2.11", "198.18.0.12"] {
                let peer = PeerId::random();
                connect(n, peer, came_in_from(from));
                n.on_observed_address(peer, made_up.clone());
            }
            let came_in = n.confirmed.contains(&made_up);
            // Each told another port of one address: the port changes per destination, but
            // no address is taken for it.
            for (i, from) in ["203.0.113.20", "192.0.2.21", "198.18.0.22"]
                .iter()
                .enumerate()
            {
                let peer = PeerId::random();
                connect(n, peer, came_in_from(from));
                let says = format!("/ip4/198.51.100.9/udp/{}/quic-v1", 5000 + i);
                n.on_observed_address(peer, says.parse().unwrap());
            }
            let port_changes = (n.port_change_reported, n.confirmed.is_empty());
            // A bootstrap node dialled by its name says the same: now it counts.
            let boot = PeerId::random();
            connect(
                n,
                boot,
                ConnectedPoint::Dialer {
                    address: format!("/dnsaddr/bootstrap.example/p2p/{boot}")
                        .parse()
                        .unwrap(),
                    role_override: libp2p::core::Endpoint::Dialer,
                    port_use: libp2p::core::transport::PortUse::Reuse,
                },
            );
            n.on_observed_address(boot, made_up.clone());
            let vouched = n.confirmed.contains(&made_up);
            // The lab's case: two DHT nodes dialled on two networks, the other device come
            // in from a third.
            let real: Multiaddr = "/ip4/198.51.100.50/udp/4001/quic-v1".parse().unwrap();
            for how in [
                dialled_at("203.0.113.30"),
                dialled_at("198.51.100.31"),
                came_in_from("192.0.2.70"),
            ] {
                let peer = PeerId::random();
                connect(n, peer, how);
                n.on_observed_address(peer, real.clone());
            }
            (came_in, port_changes, vouched, n.confirmed.contains(&real))
        })
        .await;
        assert_eq!(found, (false, (true, true), true, true));
        node.shutdown().await.unwrap();
    }

    /// Asked for its addresses before it listens on any network (just started, on a busy
    /// computer), a device waits for the first one rather than answer none: an invite made
    /// with none led nobody to it, and its joiner waited for the next heartbeat. On a
    /// device on no network the answer still comes, empty.
    #[tokio::test(flavor = "multi_thread")]
    async fn addresses_asked_before_listening_wait_for_the_first() {
        let (node, _group) = lone_member();
        // Its own listening told first (on a device on a network), then made as before it.
        let started = Instant::now();
        while inspect(&node, |n| n.listen_addrs.is_empty()).await
            && started.elapsed() < Duration::from_secs(5)
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        inspect(&node, |n| {
            n.listen_addrs.clear();
            n.confirmed.clear();
            n.started = Instant::now();
        })
        .await;
        let asked = {
            let node = node.clone();
            tokio::spawn(async move { node.addresses().await.unwrap() })
        };
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!asked.is_finished(), "answered before listening anywhere");
        let address: Multiaddr = "/ip4/192.168.1.20/udp/4001/quic-v1".parse().unwrap();
        let listening = address.clone();
        inspect(&node, move |n| {
            n.on_swarm_event_of(SwarmEvent::NewListenAddr {
                listener_id: libp2p::core::transport::ListenerId::next(),
                address: listening,
            })
        })
        .await;
        let got = tokio::time::timeout(Duration::from_secs(5), asked)
            .await
            .expect("no answer once listening")
            .unwrap();
        assert!(
            got.iter().any(|a| addr::without_peer(a) == address),
            "{got:?}"
        );
        inspect(&node, |n| {
            n.listen_addrs.clear();
            n.confirmed.clear();
            n.started = Instant::now() - ADDRESSES_WAIT;
        })
        .await;
        let none = tokio::time::timeout(Duration::from_secs(5), node.addresses())
            .await
            .expect("no answer on no network")
            .unwrap();
        assert!(none.is_empty(), "{none:?}");
    }

    /// Peers that report an address from one network (/16 of IPv4, /48 of IPv6), however
    /// many addresses they come from, do not confirm it: one machine or one provider's few
    /// can take many there. Strangers from a second network do not either, from a third
    /// they do. The same for an address taken because this device's network changes its
    /// port for each destination.
    #[tokio::test(flavor = "multi_thread")]
    async fn peers_on_one_network_do_not_confirm_an_address() {
        let (node, _group) = lone_member();
        let found = inspect(&node, |n| {
            let mut next = 9100;
            let mut report = |n: &mut Node, from: &str, says: &Multiaddr| {
                let peer = PeerId::random();
                next += 1;
                n.on_connection_established(
                    peer,
                    ConnectionId::new_unchecked(next),
                    &dialled_at(from),
                );
                n.on_observed_address(peer, says.clone());
            };
            let mut found = Vec::new();
            for (says, one, other, third) in [
                (
                    "/ip4/198.51.100.7/udp/4001/quic-v1",
                    ["203.0.113.10", "203.0.113.11", "203.0.200.12"],
                    "192.0.2.13",
                    "198.18.0.14",
                ),
                (
                    "/ip6/2001:db8:9::7/udp/4001/quic-v1",
                    ["2001:db8:1::1", "2001:db8:1:ffff::2", "2001:db8:1:8000::3"],
                    "2001:db8:2::4",
                    "2001:db8:3::5",
                ),
            ] {
                let says: Multiaddr = says.parse().unwrap();
                for from in one {
                    report(n, from, &says);
                }
                let by_one = n.confirmed.contains(&says);
                report(n, other, &says);
                let by_two = n.confirmed.contains(&says);
                report(n, third, &says);
                found.push((by_one, by_two, n.confirmed.contains(&says)));
            }
            // Three addresses of one network, each told another port of one address: the
            // port changes, but the address is not taken for it.
            n.confirmed.clear();
            for (port, from) in ["203.0.113.20", "203.0.113.21", "203.0.113.22"]
                .iter()
                .enumerate()
            {
                let says: Multiaddr = format!("/ip4/198.51.100.9/udp/{}/quic-v1", 5000 + port)
                    .parse()
                    .unwrap();
                report(n, from, &says);
            }
            found.push((n.port_change_reported, false, !n.confirmed.is_empty()));
            found
        })
        .await;
        assert_eq!(
            found,
            [
                (false, false, true),
                (false, false, true),
                (true, false, false)
            ]
        );
        node.shutdown().await.unwrap();
    }

    /// The UDP check asks over QUIC only peers this node dialled over TCP, only at the
    /// address of that connection; a dial that timed out counts once the peer answers over
    /// TCP, any other failure never; and the check's dials do not count against candidates.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_udp_check_asks_peers_it_dialled_at_their_own_address() {
        use libp2p::core::transport::TransportError;
        let (node, _group) = lone_member();
        let seen = inspect(&node, |n| {
            let tcp = |ip: &str| -> Multiaddr { format!("/ip4/{ip}/tcp/4001").parse().unwrap() };
            let quic =
                |ip: &str| -> Multiaddr { format!("/ip4/{ip}/udp/4001/quic-v1").parse().unwrap() };
            let dialled = |n: &mut Node, id: usize, ip: &str, listening: Vec<Multiaddr>| {
                let peer = PeerId::random();
                n.on_connection_established(
                    peer,
                    ConnectionId::new_unchecked(id),
                    &ConnectedPoint::Dialer {
                        address: tcp(ip),
                        role_override: libp2p::core::Endpoint::Dialer,
                        port_use: libp2p::core::transport::PortUse::Reuse,
                    },
                );
                n.peer_listening.insert(peer, listening);
                n.check_udp(peer);
                peer
            };
            let mut found = Vec::new();
            // Came in by itself: never asked, whatever it says it listens on.
            let inbound = PeerId::random();
            n.on_connection_established(
                inbound,
                ConnectionId::new_unchecked(9400),
                &ConnectedPoint::Listener {
                    local_addr: "/ip4/0.0.0.0/tcp/4001".parse().unwrap(),
                    send_back_addr: tcp("192.0.2.40"),
                },
            );
            n.peer_listening.insert(inbound, vec![quic("192.0.2.40")]);
            n.check_udp(inbound);
            found.push(n.udp.asking().contains(&inbound));
            // Dialled, but listening over QUIC only at another address: not asked.
            let elsewhere = dialled(n, 9401, "192.0.2.41", vec![quic("203.0.113.99")]);
            found.push(n.udp.asking().contains(&elsewhere));
            // Dialled, a candidate of the DHT, and its QUIC dial refused (a wrong peer
            // answered): asked, the failure proves nothing and leaves the candidate alone.
            let wrong = dialled(n, 9402, "192.0.2.42", vec![quic("192.0.2.42")]);
            found.push(n.udp.asking().contains(&wrong));
            n.candidates.insert(wrong, HashSet::new());
            let id = n.udp.dial_to(&wrong).unwrap();
            n.on_swarm_event_of(SwarmEvent::OutgoingConnectionError {
                peer_id: Some(wrong),
                connection_id: id,
                error: DialError::WrongPeerId {
                    obtained: PeerId::random(),
                    address: quic("192.0.2.42"),
                },
            });
            n.pinged(wrong, ConnectionId::new_unchecked(9402));
            found.push(n.failures.contains_key(&wrong));
            // Three more whose QUIC dials time out, over TCP connections older than the
            // network (as if it changed meanwhile, the connections still open): blocked only
            // once each answers a ping over TCP. (Fresh ones count at once: udp_check.rs.)
            n.last_network_change = Some(Instant::now() + Duration::from_secs(3600));
            let mut timed_out = Vec::new();
            for (i, ip) in ["192.0.2.43", "198.51.100.44", "203.0.113.45"]
                .iter()
                .enumerate()
            {
                let peer = dialled(n, 9403 + i, ip, vec![quic(ip)]);
                let id = n.udp.dial_to(&peer).unwrap();
                let error = std::io::Error::other(libp2p::quic::Error::HandshakeTimedOut);
                n.on_swarm_event_of(SwarmEvent::OutgoingConnectionError {
                    peer_id: Some(peer),
                    connection_id: id,
                    error: DialError::Transport(vec![(quic(ip), TransportError::Other(error))]),
                });
                timed_out.push((peer, ConnectionId::new_unchecked(9403 + i)));
            }
            found.push(n.network_report().udp == crate::UdpSeen::Blocked);
            for (peer, id) in timed_out {
                n.pinged(peer, id);
            }
            found.push(n.network_report().udp == crate::UdpSeen::Blocked);
            found
        })
        .await;
        assert_eq!(seen, [false, false, true, false, false, true]);
        node.shutdown().await.unwrap();
    }

    /// A temporary IPv6 address expiring while another of its /64 stays is no network left:
    /// what was seen of the router stays.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_temporary_ipv6_address_going_is_no_new_network() {
        let (node, _group) = lone_member();
        let kept = inspect(&node, |n| {
            let old: Multiaddr = "/ip6/2001:db8:1:2::aaaa/udp/4001/quic-v1".parse().unwrap();
            let new: Multiaddr = "/ip6/2001:db8:1:2::bbbb/udp/4001/quic-v1".parse().unwrap();
            n.listen_addrs.extend([old.clone(), new]);
            n.port_change_reported = true;
            n.on_swarm_event_of(SwarmEvent::ExpiredListenAddr {
                listener_id: libp2p::core::transport::ListenerId::next(),
                address: old,
            });
            let kept = n.port_change_reported;
            // The last address of a network going is a network left.
            let last: Multiaddr = "/ip6/2001:db8:1:2::bbbb/udp/4001/quic-v1".parse().unwrap();
            n.on_swarm_event_of(SwarmEvent::ExpiredListenAddr {
                listener_id: libp2p::core::transport::ListenerId::next(),
                address: last,
            });
            (kept, n.port_change_reported)
        })
        .await;
        assert_eq!(kept, (true, false));
        node.shutdown().await.unwrap();
    }

    /// A DHT table filled from the bootstrap list alone is nobody answering: with the dials
    /// failing, the report tells the internet is out of reach once settled.
    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_entries_alone_are_no_contact() {
        use libp2p::core::transport::TransportError;
        let (node, _group) = lone_member();
        let report = inspect(&node, |n| {
            for i in 0..3 {
                let address: Multiaddr = format!("/ip4/198.51.100.{}/tcp/4001", 50 + i)
                    .parse()
                    .unwrap();
                let error = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
                n.on_swarm_event_of(SwarmEvent::OutgoingConnectionError {
                    peer_id: Some(PeerId::random()),
                    connection_id: ConnectionId::new_unchecked(9500 + i),
                    error: DialError::Transport(vec![(address, TransportError::Other(error))]),
                });
            }
            n.network_report()
        })
        .await;
        assert_eq!(report.contact, None);
        assert_eq!(report.failed_dials, 3);
        node.shutdown().await.unwrap();
    }

    /// A network report says what the node saw on this network: a router that changes the
    /// port per destination and UDP getting out. Leaving the network forgets both, since the
    /// next network may be another.
    #[tokio::test(flavor = "multi_thread")]
    async fn leaving_a_network_forgets_its_router_and_udp() {
        let (node, _group) = lone_member();
        let seen = inspect(&node, |n| {
            for (i, from) in ["203.0.113.10", "198.51.100.11", "192.0.2.12"]
                .iter()
                .enumerate()
            {
                let peer = PeerId::random();
                n.on_connection_established(
                    peer,
                    ConnectionId::new_unchecked(9300 + i),
                    &ConnectedPoint::Listener {
                        local_addr: "/ip4/0.0.0.0/udp/4001/quic-v1".parse().unwrap(),
                        send_back_addr: format!("/ip4/{from}/udp/4001/quic-v1").parse().unwrap(),
                    },
                );
                let says = format!("/ip4/198.51.100.9/udp/{}/quic-v1", 5000 + i);
                n.on_observed_address(peer, says.parse().unwrap());
            }
            let before = n.network_report();
            n.network_changed(true);
            let after = n.network_report();
            (
                (before.port_changes, before.udp),
                (after.port_changes, after.udp),
            )
        })
        .await;
        assert_eq!(
            seen,
            (
                (true, crate::UdpSeen::Works),
                (false, crate::UdpSeen::Unknown)
            )
        );
        node.shutdown().await.unwrap();
    }

    /// Strangers on three networks that say this device is at a new address have it taken
    /// and announced, but do not have it taken as moved, which would look for everyone
    /// again; a member on a second network confirms an address, and its new one moves it.
    #[tokio::test(flavor = "multi_thread")]
    async fn only_a_member_can_make_this_device_take_itself_as_moved() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            n.started = Instant::now() - Duration::from_secs(3600);
            let mut next = 9600;
            let mut report = |n: &mut Node, peer: PeerId, how: ConnectedPoint, says: &Multiaddr| {
                next += 1;
                n.on_connection_established(peer, ConnectionId::new_unchecked(next), &how);
                n.on_observed_address(peer, says.clone());
            };
            let here: Multiaddr = "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap();
            let made_up: Multiaddr = "/ip4/198.51.100.66/udp/4001/quic-v1".parse().unwrap();
            let moved_to: Multiaddr = "/ip4/198.51.100.8/udp/4001/quic-v1".parse().unwrap();
            for from in ["203.0.113.10", "192.0.2.11", "198.18.0.12"] {
                report(n, PeerId::random(), dialled_at(from), &here);
            }
            n.last_network_change = None;
            for from in ["203.0.113.20", "192.0.2.21", "198.18.0.22"] {
                report(n, PeerId::random(), dialled_at(from), &made_up);
            }
            let by_strangers = (
                n.confirmed.contains(&made_up),
                n.last_network_change.is_some(),
            );
            // A member on a second network, its report made before it proved itself; both
            // came in by themselves, which counts with a member among them.
            let bea = PeerId::random();
            report(n, PeerId::random(), came_in_from("203.0.113.30"), &moved_to);
            report(n, bea, came_in_from("192.0.2.31"), &moved_to);
            let before_proof = n.confirmed.contains(&moved_to);
            crate::node::tests::add_member(n, group, bea);
            n.reported_by_member(bea);
            let by_member = (
                n.confirmed.contains(&moved_to),
                n.last_network_change.is_some(),
            );
            (by_strangers, before_proof, by_member)
        })
        .await;
        assert_eq!(found, ((true, false), false, (true, true)));
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn strangers_cannot_make_another_address_this_devices_own() {
        let (node, _group) = lone_member();
        let (confirmed, own) = inspect(&node, |n| {
            // Three DHT nodes on three networks, which this device dialled, say it is at a
            // member's address.
            let member_at: Multiaddr = "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap();
            for (i, from) in ["203.0.113.10", "192.0.2.11", "198.18.0.12"]
                .iter()
                .enumerate()
            {
                let stranger = PeerId::random();
                n.on_connection_established(
                    stranger,
                    ConnectionId::new_unchecked(9000 + i),
                    &dialled_at(from),
                );
                n.on_observed_address(stranger, member_at.clone());
            }
            (n.confirmed.contains(&member_at), n.own_public_ips())
        })
        .await;
        // Announced (nothing else tells this device its address before a member does), but
        // not taken as its own for resting or skipping members behind it.
        assert!(confirmed);
        assert!(own.is_empty(), "{own:?}");
        node.shutdown().await.unwrap();
    }
}
