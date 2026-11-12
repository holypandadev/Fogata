//! Hole punches coordinated through a relay (libp2p's DCUtR), never at an address this device
//! may not dial.
//!
//! The peer at the other end of a relayed connection sends the addresses it says others see
//! it at, and DCUtR dials every one of them at once. A member that lies would have this
//! device send handshakes to any host and port of its own networks (a router's admin page, a
//! VPN peer); an honest one may name a local address of its own network among its public
//! ones (one a neighbour of its saw it at). So a punch naming any address
//! [`OwnNetworks::may_dial`] refuses is not made as asked: it is dialled again at the others
//! alone, if any are left.

use std::{
    collections::{HashSet, VecDeque},
    task::{Context, Poll},
};

use libp2p::{
    Multiaddr, PeerId,
    core::{Endpoint, transport::PortUse},
    dcutr,
    swarm::{
        ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
        THandlerOutEvent, ToSwarm,
        dial_opts::{DialOpts, PeerCondition},
    },
};

use crate::{
    addr::{OwnNetworks, Reach},
    strangers::Wanted,
};

/// Punches dialled again that wait to start, at most: one per relayed connection at a time,
/// in practice.
const MAX_WAITING: usize = 64;

/// A punch dialled again at the addresses this device may dial (see the module's notes).
#[derive(Debug, PartialEq, Eq)]
struct Again {
    peer: PeerId,
    addresses: Vec<Multiaddr>,
    /// The role DCUtR gave its own dial: the end that dials as listener sends the packets
    /// that open its own NAT.
    role: Endpoint,
}

pub(crate) struct Punches {
    inner: dcutr::Behaviour,
    wanted: Wanted,
    /// The dials DCUtR asked for that the swarm has not set up yet: each is checked as the
    /// swarm starts it, right after it is asked for, so this holds one at a time.
    asked: HashSet<ConnectionId>,
    again: VecDeque<Again>,
}

impl Punches {
    pub(crate) fn new(peer: PeerId, wanted: Wanted) -> Self {
        Self {
            inner: dcutr::Behaviour::new(peer),
            wanted,
            asked: HashSet::new(),
            again: VecDeque::new(),
        }
    }

    /// Whether DCUtR's dial `connection` to `peer` at `addresses` goes ahead: not when it
    /// names an address `own` refuses, in which case the others are dialled again (see the
    /// module's notes).
    fn check(
        &mut self,
        connection: ConnectionId,
        peer: Option<PeerId>,
        addresses: &[Multiaddr],
        role: Endpoint,
        own: &OwnNetworks,
    ) -> Result<(), ConnectionDenied> {
        let Some(peer) = peer.filter(|_| self.asked.remove(&connection)) else {
            return Ok(());
        };
        let (kept, refused): (Vec<Multiaddr>, Vec<Multiaddr>) = addresses
            .iter()
            .cloned()
            .partition(|a| own.may_dial(a, Reach::SamePlace));
        if refused.is_empty() {
            return Ok(());
        }
        tracing::debug!(
            %peer,
            kept = kept.len(),
            refused = refused.len(),
            "a hole punch names addresses this device does not dial"
        );
        if !kept.is_empty() && self.again.len() < MAX_WAITING {
            self.again.push_back(Again {
                peer,
                addresses: kept,
                role,
            });
        }
        Err(ConnectionDenied::new(
            "a hole punch at addresses this device does not dial",
        ))
    }
}

impl NetworkBehaviour for Punches {
    type ConnectionHandler = <dcutr::Behaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = dcutr::Event;

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        self.inner
            .handle_pending_inbound_connection(connection_id, local_addr, remote_addr)
    }

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        let own = self.wanted.own();
        self.check(connection_id, maybe_peer, addresses, effective_role, &own)?;
        self.inner.handle_pending_outbound_connection(
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        if let FromSwarm::DialFailure(failure) = &event {
            self.asked.remove(&failure.connection_id);
        }
        self.inner.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<ToSwarm<dcutr::Event, THandlerInEvent<Self>>> {
        if let Some(again) = self.again.pop_front() {
            // Not while a dial to the peer is on its way already (an earlier punch's).
            let opts = DialOpts::peer_id(again.peer)
                .addresses(again.addresses)
                .condition(PeerCondition::NotDialing);
            let opts = match again.role {
                Endpoint::Listener => opts.override_role().build(),
                Endpoint::Dialer => opts.build(),
            };
            return Poll::Ready(ToSwarm::Dial { opts });
        }
        let event = std::task::ready!(self.inner.poll(cx));
        if let ToSwarm::Dial { opts } = &event {
            self.asked.insert(opts.connection_id());
        }
        Poll::Ready(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    /// A punch that names local addresses of networks this device is not on, this computer
    /// or this device's own address is not dialled as asked, but at its other addresses
    /// alone, in the role it was given; one naming only addresses this device may dial goes
    /// ahead, as does any dial DCUtR did not ask for (the node's own, given by hand).
    #[test]
    fn a_punch_is_dialled_only_at_addresses_this_device_may_dial() {
        let me = PeerId::random();
        let mut punches = Punches::new(me, Wanted::default());
        let own = OwnNetworks::of(&[at("/ip4/192.168.1.20/udp/4001/quic-v1")]);
        let peer = PeerId::random();
        let honest = [
            at("/ip4/203.0.113.5/udp/4001/quic-v1"),
            at("/ip4/192.168.7.5/udp/4001/quic-v1"),
        ];
        let mut lying = honest.to_vec();
        lying.extend([
            at("/ip4/100.70.8.9/udp/80/quic-v1"),
            at("/ip6/fd99:1:2:1::1/udp/22/quic-v1"),
            at("/ip4/127.0.0.1/udp/22/quic-v1"),
            at("/ip4/192.168.1.20/udp/4001/quic-v1"),
        ]);
        let mut next = 9000;
        let mut punch = |addresses: &[Multiaddr], role: Endpoint, asked: bool| {
            next += 1;
            let connection = ConnectionId::new_unchecked(next);
            if asked {
                punches.asked.insert(connection);
            }
            let result = punches.check(connection, Some(peer), addresses, role, &own);
            (result.is_ok(), punches.again.pop_front())
        };
        assert_eq!(punch(&honest, Endpoint::Dialer, true), (true, None));
        assert_eq!(
            punch(&lying, Endpoint::Listener, true),
            (
                false,
                Some(Again {
                    peer,
                    addresses: honest.to_vec(),
                    role: Endpoint::Listener,
                })
            )
        );
        // Nothing left to dial: nothing dialled again.
        assert_eq!(punch(&lying[2..], Endpoint::Dialer, true), (false, None));
        // Not DCUtR's own dial: as it was given.
        assert_eq!(punch(&lying, Endpoint::Dialer, false), (true, None));
        assert!(punches.asked.is_empty());
    }
}
