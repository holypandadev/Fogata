//! What the swarm tells the node: its listen addresses coming and going, connections opening
//! and closing, dials that failed, and what each behaviour heard (identify, ping, mDNS, hole
//! punches, relays, the DHT and the member and friend protocols).

use std::{collections::HashSet, time::Instant};

use fogata_crypto::GroupId;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol,
    core::ConnectedPoint,
    dcutr, identify, mdns,
    multiaddr::Protocol,
    ping, relay, request_response,
    swarm::{ConnectionId, DialError, SwarmEvent},
};

use super::{
    ConnectionInfo, Node, STARTUP,
    candidates::{Budget, MAX_FAILURES, MAX_RESTS},
    dial::{PUNCHED_WITHIN, private_block},
    is_loopback,
    network::{probe_lost_udp, v6_net},
};
use crate::{NodeEvent, Path, addr, behaviour::BehaviourEvent, short_peer};

/// At most this many of a peer's protocols are kept (see `Node::advertised`).
const ADVERTISED_MAX: usize = 64;

/// The peer whose relay a circuit from `from` came in through, found by the relay's
/// address in the circuit's local address among `connections`: libp2p leaves the relay's
/// identity out of that address at times.
fn relay_at<'a>(
    local_addr: &Multiaddr,
    from: PeerId,
    mut connections: impl Iterator<Item = &'a ConnectionInfo>,
) -> Option<PeerId> {
    let relay: Multiaddr = local_addr
        .iter()
        .take_while(|p| !matches!(p, Protocol::P2pCircuit))
        .collect();
    let relay = addr::without_peer(&relay);
    connections
        .find(|c| {
            c.peer != from
                && !matches!(c.path, Path::ThroughMember { .. })
                && addr::without_peer(&c.address) == relay
        })
        .map(|c| c.peer)
}

impl Node {
    pub(super) fn on_swarm_event_of(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                // A slot at a member's relay is no network of this device's own.
                if !is_loopback(&address) && !addr::is_relayed(&address) {
                    self.swarm.behaviour_mut().relay_address(&address, true);
                    let block = private_block(&address);
                    let new_block = block.is_some()
                        && !self.listen_addrs.iter().any(|a| private_block(a) == block);
                    // A new temporary IPv6 address in a /64 this device is on already: the
                    // same network.
                    let same_v6 = v6_net(&address).is_some_and(|net| {
                        self.listen_addrs.iter().any(|a| v6_net(a) == Some(net))
                    });
                    self.listen_addrs.push(address.clone());
                    self.answer_waiting_addresses();
                    // A network joined while running (another Wi-Fi, a cable plugged in).
                    if self.started.elapsed() > STARTUP && addr::ip(&address).is_some() && !same_v6
                    {
                        self.network_changed(false);
                    } else if new_block {
                        // Starting: the remembered addresses on this network can be dialled
                        // now that this device knows it is on it.
                        self.dial_remembered_in(block);
                    }
                }
                self.emit(NodeEvent::Listening(address));
            }
            SwarmEvent::ExpiredListenAddr { address, .. } => {
                self.listen_addrs.retain(|a| *a != address);
                self.swarm.behaviour_mut().relay_address(&address, false);
                // A temporary IPv6 address that expired while another of its /64 stays: the
                // same network, and only what was seen at that address goes.
                let same_v6 = v6_net(&address)
                    .is_some_and(|net| self.listen_addrs.iter().any(|a| v6_net(a) == Some(net)));
                if same_v6 {
                    self.forget_own_address(&address);
                } else if !is_loopback(&address)
                    && !addr::is_relayed(&address)
                    && addr::ip(&address).is_some()
                {
                    // A network left: what was seen of this device from outside may be
                    // wrong now.
                    self.network_changed(true);
                }
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                ..
            } => self.on_connection_established(peer_id, connection_id, &endpoint),
            SwarmEvent::ConnectionClosed {
                peer_id,
                connection_id,
                num_established,
                ..
            } => self.on_connection_closed(peer_id, connection_id, num_established),
            SwarmEvent::OutgoingConnectionError {
                peer_id: None,
                connection_id,
                error,
            } => {
                self.explicit_dials.remove(&connection_id);
                self.count_failed_dial(&error);
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer),
                error,
                connection_id,
            } if self.udp.is_probe(&connection_id) => {
                // A check whether UDP gets out (see udp_check.rs), to a peer connected over
                // TCP: nothing about reaching it.
                let lost = probe_lost_udp(&error);
                tracing::debug!(%peer, lost, "a peer did not answer over QUIC: {error}");
                self.udp.dial_failed(connection_id, lost);
            }
            SwarmEvent::OutgoingConnectionError {
                peer_id: Some(peer),
                error,
                connection_id,
            } => {
                self.explicit_dials.remove(&connection_id);
                self.count_failed_dial(&error);
                self.punch_dials.remove(&connection_id);
                self.dialling_found.remove(&peer);
                self.mdns_dial_failed(peer, connection_id);
                // A lost member that cannot be reached yet is no stale announcement to rest.
                if !self.is_redial(peer, connection_id) {
                    self.on_dial_error(peer, &error)
                }
            }
            // A relay slot makes libp2p treat the relayed address as one of ours. It must not
            // be published: it would show on the DHT which member we are connected to.
            // Members reach us through relays by asking each other for rosters instead.
            SwarmEvent::ExternalAddrConfirmed { address } if addr::is_relayed(&address) => {
                tracing::debug!(%address, "relayed address kept out of announcements");
                self.swarm.remove_external_address(&address);
            }
            SwarmEvent::ListenerClosed { listener_id, .. } => {
                if let Some(peer) = self.reservations.remove(&listener_id) {
                    tracing::debug!("relay slot at {peer} closed");
                }
            }
            SwarmEvent::Behaviour(event) => self.on_behaviour_event(event),
            _ => {}
        }
    }

    pub(super) fn on_connection_established(
        &mut self,
        peer: PeerId,
        connection: ConnectionId,
        endpoint: &ConnectedPoint,
    ) {
        self.dialling_found.remove(&peer);
        self.mdns_retry.remove(&peer);
        if self.mdns_dialling.remove(&peer).is_some() {
            tracing::debug!(%peer, "connected to a peer mDNS found");
        }
        let (address, path) = match endpoint {
            ConnectedPoint::Dialer {
                address,
                role_override,
                ..
            } => {
                let path = if addr::is_relayed(address) {
                    Path::ThroughMember {
                        relay: addr::relay_of(address).unwrap_or(peer),
                        member: None,
                    }
                } else if addr::is_local(address) {
                    Path::Lan
                } else if *role_override == libp2p::core::Endpoint::Listener
                    || self.punch_dials.contains(&connection)
                {
                    Path::HolePunched
                } else {
                    Path::Direct
                };
                (address.clone(), path)
            }
            ConnectedPoint::Listener {
                local_addr,
                send_back_addr,
            } => {
                let path = if addr::is_relayed(local_addr) || addr::is_relayed(send_back_addr) {
                    let relay = addr::relay_of(local_addr)
                        .or_else(|| addr::relay_of(send_back_addr))
                        .or_else(|| relay_at(local_addr, peer, self.connections.values()))
                        .unwrap_or(peer);
                    Path::ThroughMember {
                        relay,
                        member: None,
                    }
                } else if addr::is_local(send_back_addr) {
                    Path::Lan
                } else if self
                    .punching
                    .get(&peer)
                    .is_some_and(|t| t.elapsed() < PUNCHED_WITHIN)
                {
                    Path::HolePunched
                } else {
                    Path::Direct
                };
                (send_back_addr.clone(), path)
            }
        };

        self.failures.remove(&peer);
        self.reached_peers.insert(peer);
        self.udp.connected(connection, &address, Instant::now());
        let public = !addr::is_local(&address) && !addr::is_relayed(&address);
        if public && addr::ip(&address).is_some() {
            self.public_contact();
            if !addr::is_quic(&address)
                && let ConnectedPoint::Dialer { .. } = endpoint
                && let Some(ip) = addr::ip(&address)
            {
                self.dialled_tcp.insert(connection, (ip, Instant::now()));
            }
        }
        if addr::is_quic(&address) && !addr::is_relayed(&address) && !addr::is_local(&address) {
            self.wanted.quic_works();
        }

        // Remember addresses of DHT candidates for later punch slots.
        let usable = self.usable(&address);
        if let Some(addrs) = self.candidates.get_mut(&peer)
            && usable
        {
            addrs.insert(addr::without_peer(&address));
        }

        let relayed = matches!(path, Path::ThroughMember { .. });
        self.connections.insert(
            connection,
            ConnectionInfo {
                peer,
                path: path.clone(),
                address,
            },
        );

        let dialled = self.dialled_here(peer, connection);
        if dialled {
            self.interesting.insert(peer);
        }
        if relayed || path == Path::Lan {
            self.interesting.insert(peer);
        }
        if path == Path::Lan {
            self.wanted.found_nearby(&peer);
        }
        // While a move goes on, the other computer may be on this local network, or one
        // found under the code (see `seek`).
        if self.is_seeking()
            && (path == Path::Lan
                || matches!(self.candidate_found.get(&peer), Some((Budget::Move, _))))
        {
            self.show_protocols(peer);
        }
        // Kept and never closed for others: members, friends online, peers this device chose
        // to dial, and those that come through a member's relay (which serves members only).
        // Anyone else, from the local network too, until it proves something.
        if dialled || relayed || self.members.contains_key(&peer) || self.is_friend_peer(&peer) {
            self.wanted.want(peer);
            self.show_protocols(peer);
        }

        if self.members.contains_key(&peer) {
            self.update_member_path(peer);
        }
        if relayed && self.silent.contains_key(&peer) {
            self.leave_silent_links(peer);
        }
        if self.interesting.contains(&peer) {
            self.approach(peer, None);
        }
        self.friend_connected(peer);
        // Dialled at an address given or remembered: maybe a friend, which only a probe tells.
        if dialled {
            self.friend_lan_peer(peer);
        }
    }

    pub(super) fn on_connection_closed(
        &mut self,
        peer: PeerId,
        connection: ConnectionId,
        remaining: u32,
    ) {
        let closed = self.connections.remove(&connection);
        self.closing.remove(&connection);
        self.dialled_tcp.remove(&connection);
        self.punch_dials.remove(&connection);
        self.ping_misses.remove(&connection);
        if remaining > 0 {
            if self.members.contains_key(&peer) {
                self.update_member_path(peer);
                // Its QUIC link, which media travels on, went while another one stays: QUIC
                // is tried again rather than leaving media on streams for the rest of the
                // session.
                let quic = |c: &ConnectionInfo| {
                    addr::is_quic(&c.address) && !matches!(c.path, Path::ThroughMember { .. })
                };
                if closed.as_ref().is_some_and(quic)
                    && !self.connections.values().any(|c| c.peer == peer && quic(c))
                {
                    self.quic_tried.remove(&peer);
                    self.want_datagrams(peer);
                    // Left on TCP alone after an outage, say, which only gets through once
                    // its own retries do: the local QUIC way is dialled again for a while too.
                    if !self.has_relayed(&peer) {
                        self.keep_lost(peer, closed.as_ref());
                    }
                }
            }
            return;
        }
        self.reservations.retain(|_, p| *p != peer);
        self.relay_users.remove(&peer);
        self.after_call.remove(&peer);
        // Requests still waiting would only dial a peer that just went.
        self.requests_peer_gone(peer);
        self.farewell.remove(&peer);
        self.session_gone(&peer);
        self.typing_peer_gone(&peer);
        self.limits.peer_gone(&peer);
        self.quic_tried.remove(&peer);
        self.peer_listening.remove(&peer);
        crate::locked(&self.advertised).remove(&peer);
        crate::locked(&self.shown).remove(&peer);
        self.keep_lost(peer, closed.as_ref());
        self.member_lan.remove(&peer);
        self.lan_redialled.remove(&peer);
        self.early.remove(&peer);
        self.friend_disconnected(peer);
        let lost: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, s)| s.members.contains(&peer))
            .map(|(id, _)| *id)
            .collect();
        let groups: Vec<GroupId> = self.groups.keys().copied().collect();
        for group in groups {
            let slot = self.groups.get_mut(&group).expect("listed above");
            slot.challenges.remove(&peer);
            slot.room_asked.remove(&peer);
            if let Some(tried) = slot.room_again.as_mut() {
                tried.remove(&peer);
            }
            slot.join_offers.remove(&peer);
            slot.join_sent.remove(&peer);
            slot.stop_join_with(&peer);
            slot.held_back.remove(&peer);
            self.drop_member(group, peer);
        }
        // Its direct way may be gone (client isolation on a Wi-Fi, a NAT that forgot its
        // mapping) while the other members still reach it.
        self.look_through_others(peer, &lost);
    }

    fn on_dial_error(&mut self, peer: PeerId, error: &DialError) {
        let DialError::Transport(attempts) = error else {
            return;
        };
        // The way through that member failed; another member's may be tried at once.
        if attempts.iter().any(|(a, _)| addr::is_relayed(a)) {
            self.through_dials.remove(&peer);
        }
        let usable: Vec<Multiaddr> = attempts
            .iter()
            .filter(|(address, _)| self.usable(address))
            .map(|(address, _)| addr::without_peer(address))
            .collect();
        if let Some(addrs) = self.candidates.get_mut(&peer) {
            addrs.extend(usable);
        }
        if self.candidates.contains_key(&peer) {
            tracing::debug!(%peer, ?attempts, "connection attempt to a member failed");
            // Every address behind our own public IP: most likely an announcement left by an
            // earlier session of this same device. Retrying cannot help.
            let own_ips = self.own_public_ips();
            let behind_us = !attempts.is_empty()
                && attempts
                    .iter()
                    .all(|(a, _)| addr::ip(a).is_some_and(|ip| own_ips.contains(&ip)));
            let entry = self.failures.entry(peer).or_insert((0, None, 0));
            entry.0 += 1;
            if entry.0 >= MAX_FAILURES || behind_us {
                *entry = (0, Some(Instant::now()), entry.2 + 1);
            }
            if entry.2 >= MAX_RESTS {
                self.forget_candidate(peer);
            }
        }
    }

    fn on_behaviour_event(&mut self, event: BehaviourEvent) {
        match event {
            // Peers that speak Fogata protocols are kept connected for a while.
            BehaviourEvent::Member(event) => {
                if let request_response::Event::Message { peer, .. } = &event {
                    self.spoke(*peer);
                    self.serving = Some(*peer);
                }
                self.on_member_event(event);
                self.serving = None;
            }
            BehaviourEvent::Friend(event) => {
                if let request_response::Event::Message { peer, .. } = &event {
                    self.spoke(*peer);
                    self.serving = Some(*peer);
                }
                self.on_friend_event(event);
                self.serving = None;
            }
            BehaviourEvent::Kad(event) => self.on_kad_event(event),
            BehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                self.on_observed_address(peer_id, info.observed_addr);
                // Fogata's own only, and a bounded number: anyone can connect and say
                // anything.
                let ours: HashSet<StreamProtocol> = info
                    .protocols
                    .into_iter()
                    .filter(|p| p.as_ref().starts_with("/fogata/"))
                    .take(ADVERTISED_MAX)
                    .collect();
                crate::locked(&self.advertised).insert(peer_id, ours);
                self.note_level(peer_id);
                // A bootstrap peer known by name is dialled at its plain addresses from now
                // on, without asking DNS again at each dial (see strangers.rs).
                if self.named_bootstrap.contains(&peer_id)
                    && let Some(kad) = self.swarm.behaviour_mut().kad.as_mut()
                {
                    for address in info.listen_addrs.iter().filter(|a| {
                        addr::is_dialable(a) && !addr::is_named(a) && !addr::is_local(a)
                    }) {
                        kad.add_address(&peer_id, addr::without_peer(address));
                    }
                }
                self.peer_listening.insert(peer_id, info.listen_addrs);
                self.want_datagrams(peer_id);
                self.check_udp(peer_id);
            }
            // A link that stopped answering (Wi-Fi gone, a sleep, a router that forgot it)
            // stays open until the system gives up on it, which takes minutes over TCP.
            // Two missed pings in a row close it, so the peer is dialled again.
            #[cfg(test)]
            BehaviourEvent::Identify(identify::Event::Pushed { peer_id, .. }) => {
                *self.pushed.entry(peer_id).or_default() += 1;
            }
            BehaviourEvent::Ping(ping::Event {
                peer,
                connection,
                result,
            }) => match result {
                Ok(_) => {
                    self.ping_misses.remove(&connection);
                    self.pinged(peer, connection);
                }
                Err(e) => {
                    let misses = self.ping_misses.entry(connection).or_default();
                    *misses += 1;
                    if *misses >= 2 {
                        tracing::debug!("closing a connection that stopped answering: {e}");
                        self.ping_misses.remove(&connection);
                        self.swarm.close_connection(connection);
                    }
                }
            },
            BehaviourEvent::Mdns(mdns::Event::Expired(gone)) => {
                // Gone from the local network: only counted while there.
                for (peer, _) in gone {
                    self.lan_seen.remove(&peer);
                }
            }
            BehaviourEvent::Mdns(mdns::Event::Discovered(found)) => {
                self.on_mdns_discovered(found);
            }
            BehaviourEvent::Dcutr(dcutr::Event {
                remote_peer_id,
                result,
            }) => match result {
                Ok(connection) => {
                    if let Some(info) = self.connections.get_mut(&connection) {
                        info.path = Path::HolePunched;
                    }
                    self.update_member_path(remote_peer_id);
                }
                Err(e) => {
                    let note = if self.bridge {
                        "the member in between keeps carrying the traffic"
                    } else {
                        "messages travel through other members"
                    };
                    self.info(format!(
                        "hole punch with {} through a member failed ({e}); {note}",
                        short_peer(&remote_peer_id)
                    ));
                }
            },
            BehaviourEvent::RelayClient(relay::client::Event::ReservationReqAccepted {
                relay_peer_id,
                renewal: false,
                ..
            }) => {
                self.info(format!(
                    "reachable through member {}",
                    short_peer(&relay_peer_id)
                ));
            }
            BehaviourEvent::Relay(relay::Event::ReservationReqAccepted { src_peer_id, .. }) => {
                self.relay_users.insert(src_peer_id);
            }
            BehaviourEvent::Relay(relay::Event::CircuitReqAccepted {
                src_peer_id,
                dst_peer_id,
            }) => {
                self.relay_users.insert(src_peer_id);
                self.relay_users.insert(dst_peer_id);
                self.info(format!(
                    "helping {} and {} to connect",
                    short_peer(&src_peer_id),
                    short_peer(&dst_peer_id)
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_circuit_names_the_member_it_came_through() {
        let (relay, other, from) = (PeerId::random(), PeerId::random(), PeerId::random());
        let at = |a: &str| a.parse::<Multiaddr>().unwrap();
        let connections = [
            ConnectionInfo {
                peer: other,
                path: Path::Direct,
                address: at("/ip4/203.0.113.60/udp/4001/quic-v1"),
            },
            ConnectionInfo {
                peer: relay,
                path: Path::HolePunched,
                address: at(&format!("/ip4/203.0.113.50/udp/4001/quic-v1/p2p/{relay}")),
            },
            // The one who came in: never its own relay.
            ConnectionInfo {
                peer: from,
                path: Path::Direct,
                address: at("/ip4/203.0.113.50/udp/4001/quic-v1"),
            },
        ];
        // libp2p gives the relay's address without its identity at times.
        let local = at("/ip4/203.0.113.50/udp/4001/quic-v1/p2p-circuit");
        assert_eq!(relay_at(&local, from, connections.iter()), Some(relay));
        let unknown = at("/ip4/203.0.113.70/udp/4001/quic-v1/p2p-circuit");
        assert_eq!(relay_at(&unknown, from, connections.iter()), None);
    }
}
