// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::{
    collections::HashSet,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use either::Either;
use futures::prelude::*;
use futures_bounded::Timeout;
use futures_timer::Delay;
use libp2p_core::{
    Multiaddr,
    upgrade::{ReadyUpgrade, SelectUpgrade},
};
use libp2p_identity::PeerId;
use libp2p_swarm::{
    ConnectionHandler, ConnectionHandlerEvent, StreamProtocol, StreamUpgradeError,
    SubstreamProtocol, SupportedProtocols,
    handler::{
        ConnectionEvent, DialUpgradeError, FullyNegotiatedInbound, FullyNegotiatedOutbound,
        ProtocolSupport,
    },
};
use smallvec::SmallVec;
use tracing::Level;

use crate::{
    PROTOCOL_NAME, PUSH_PROTOCOL_NAME,
    behaviour::{KeyType, Shown},
    protocol::{self, Info, PushInfo, UpgradeError},
};

const STREAM_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_CONCURRENT_STREAMS_PER_CONNECTION: usize = 10;

/// What a push on a connection was asked for (Fogata).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushReason {
    /// Asked by the node ([`crate::Behaviour::push`]): sent even if nothing changed, since
    /// a push that came before the peer's first identify ended is dropped at its end.
    Asked,
    /// What this node says changed (its protocols, its addresses): sent only if what the
    /// connection would say differs from what it said last.
    Changed,
}

/// What an outbound stream is opened for (Fogata): a failure to open a push ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outbound {
    Identify,
    Push,
}

/// Where a push on a connection stands (Fogata). At most one is open at a time; any asked
/// meanwhile are folded into one more, sent once it ends, so that a burst of changes (a
/// network joined, many addresses at once) is a push or two and never one per change: the
/// other end takes only a few streams at once and drops the rest.
#[derive(Debug, Default)]
struct Pushes {
    /// A push is being opened or sent.
    open: bool,
    /// Asked for while one was open, and why.
    next: Option<PushReason>,
    /// The addresses and protocols this connection said last, in an answer or a push.
    said: Option<(HashSet<Multiaddr>, HashSet<StreamProtocol>)>,
    /// Inbound streams dropped for want of room: only the first is warned of.
    dropped: u64,
    /// How many times this connection built who this node is, to answer or push it, and
    /// the number of the one that got there last: an answer and a push may end out of
    /// order, and an older one ending last must not stand for what was said last.
    built: u64,
    told: u64,
}

/// Protocol handler for sending and receiving identification requests.
///
/// Outbound requests are sent periodically. The handler performs expects
/// at least one identification request to be answered by the remote before
/// permitting the underlying connection to be closed.
pub struct Handler {
    remote_peer_id: PeerId,
    /// Pending events to yield.
    events: SmallVec<
        [ConnectionHandlerEvent<
            Either<ReadyUpgrade<StreamProtocol>, ReadyUpgrade<StreamProtocol>>,
            Outbound,
            Event,
        >; 4],
    >,

    active_streams: futures_bounded::FuturesSet<Result<Success, UpgradeError>>,

    /// Future that fires when we need to identify the node again.
    trigger_next_identify: Delay,

    /// Whether we have exchanged at least one periodic identify.
    exchanged_one_periodic_identify: bool,

    /// The interval of `trigger_next_identify`, i.e. the recurrent delay.
    interval: Duration,

    /// The key of the local peer.
    local_key: Arc<KeyType>,

    /// Application-specific version of the protocol family used by the peer,
    /// e.g. `ipfs/1.0.0` or `polkadot/1.0.0`.
    protocol_version: String,

    /// Name and version of the peer, similar to the `User-Agent` header in
    /// the HTTP protocol.
    agent_version: String,

    /// Address observed by or for the remote.
    observed_addr: Multiaddr,

    /// Identify information about the remote peer.
    remote_info: Option<Info>,

    local_supported_protocols: SupportedProtocols,
    remote_supported_protocols: HashSet<StreamProtocol>,
    external_addresses: HashSet<Multiaddr>,

    /// Protocols listed only to some peers (Fogata), and whether this connection's peer
    /// was taken: they are then listed on it for good.
    shown: Option<(Shown, bool)>,

    /// The pushes of this connection (Fogata, see [`Pushes`]).
    pushes: Pushes,
    /// The push being sent, apart from the other streams: its end, or its timeout, ends it.
    push_stream: futures_bounded::FuturesSet<Result<(u64, Info), UpgradeError>>,
}

/// An event from `Behaviour` with the information requested by the `Handler`.
#[derive(Debug)]
pub enum InEvent {
    AddressesChanged(HashSet<Multiaddr>),
    Push(PushReason),
}

/// Event produced by the `Handler`.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    /// We obtained identification information from the remote.
    Identified(Info),
    /// We replied to an identification request from the remote.
    Identification,
    /// We actively pushed our identification information to the remote.
    IdentificationPushed(Info),
    /// Failed to identify the remote, or to reply to an identification request.
    IdentificationError(StreamUpgradeError<UpgradeError>),
}

impl Handler {
    /// Creates a new `Handler`.
    pub(crate) fn new(
        interval: Duration,
        remote_peer_id: PeerId,
        local_key: Arc<KeyType>,
        protocol_version: String,
        agent_version: String,
        observed_addr: Multiaddr,
        external_addresses: HashSet<Multiaddr>,
        shown: Option<Shown>,
    ) -> Self {
        Self {
            remote_peer_id,
            events: SmallVec::new(),
            active_streams: futures_bounded::FuturesSet::new(
                move || futures_bounded::Delay::futures_timer(STREAM_TIMEOUT),
                MAX_CONCURRENT_STREAMS_PER_CONNECTION,
            ),
            trigger_next_identify: Delay::new(Duration::ZERO),
            exchanged_one_periodic_identify: false,
            interval,
            local_key,
            protocol_version,
            agent_version,
            observed_addr,
            local_supported_protocols: SupportedProtocols::default(),
            remote_supported_protocols: HashSet::default(),
            remote_info: Default::default(),
            external_addresses,
            shown: shown.map(|shown| (shown, false)),
            pushes: Pushes::default(),
            push_stream: futures_bounded::FuturesSet::new(
                move || futures_bounded::Delay::futures_timer(STREAM_TIMEOUT),
                1,
            ),
        }
    }

    /// Asks the other end whether it is now shown the protocols listed only to some, until
    /// it is: each time this connection says who this node is, or is about to.
    fn ask_shown(&mut self) {
        if let Some((shown, taken @ false)) = &mut self.shown {
            *taken = (shown.to)(&self.remote_peer_id);
        }
    }

    /// The addresses and protocols this connection says now.
    fn saying(&mut self) -> (HashSet<Multiaddr>, HashSet<StreamProtocol>) {
        self.ask_shown();
        (self.external_addresses.clone(), self.listed())
    }

    /// Pushes who this node is on this connection, or after the push open now (see
    /// [`Pushes`]). One for a change is left out when the connection would say what it
    /// said last.
    fn push(&mut self, reason: PushReason) {
        if self.pushes.open {
            self.pushes.next = match (self.pushes.next, reason) {
                (Some(PushReason::Asked), _) | (_, PushReason::Asked) => Some(PushReason::Asked),
                _ => Some(PushReason::Changed),
            };
            return;
        }
        let saying = self.saying();
        if reason == PushReason::Changed && self.pushes.said.as_ref() == Some(&saying) {
            tracing::trace!(peer=%self.remote_peer_id, "Nothing new to push to peer");
            return;
        }
        self.pushes.open = true;
        self.events
            .push(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(
                    Either::Right(ReadyUpgrade::new(PUSH_PROTOCOL_NAME)),
                    Outbound::Push,
                ),
            });
    }

    /// `info` reached the other end, in an answer or a push: what this connection said
    /// last. Only what got there counts, so an answer or a push that failed leaves the next
    /// change to be pushed.
    fn told(&mut self, built: u64, info: &Info) {
        if built < self.pushes.told {
            return;
        }
        self.pushes.told = built;
        self.pushes.said = Some((
            info.listen_addrs.iter().cloned().collect(),
            info.protocols.iter().cloned().collect(),
        ));
    }

    /// The push open ended, `sent` or not: the one asked for meanwhile, if any, goes now.
    /// One not sent leaves what the other end was told unknown, so the next change is
    /// pushed whatever it is.
    fn push_ended(&mut self, sent: bool) {
        self.pushes.open = false;
        if !sent {
            self.pushes.said = None;
        }
        if let Some(reason) = self.pushes.next.take() {
            self.push(reason);
        }
    }

    /// Warns of an inbound stream dropped for want of room, the first time only: a peer
    /// that opens streams without end would otherwise fill the log.
    fn dropped_inbound(&mut self, what: &str) {
        self.pushes.dropped += 1;
        if self.pushes.dropped == 1 {
            tracing::warn!(
                peer=%self.remote_peer_id,
                "Dropping inbound {what} stream because we are at capacity; others dropped on this connection are not logged"
            );
        } else {
            tracing::debug!(peer=%self.remote_peer_id, "Dropping inbound {what} stream because we are at capacity");
        }
    }

    /// The protocols this connection lists now.
    fn listed(&self) -> HashSet<StreamProtocol> {
        self.local_supported_protocols
            .iter()
            .filter(|p| match &self.shown {
                Some((shown, false)) => !p.as_ref().starts_with(shown.prefix.as_str()),
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn on_fully_negotiated_inbound(
        &mut self,
        FullyNegotiatedInbound {
            protocol: output, ..
        }: FullyNegotiatedInbound<<Self as ConnectionHandler>::InboundProtocol>,
    ) {
        match output {
            future::Either::Left(stream) => {
                let info = self.build_info();
                let built = self.pushes.built;

                if self
                    .active_streams
                    .try_push(
                        protocol::send_identify(stream, info)
                            .map_ok(move |info| Success::SentIdentify(built, info)),
                    )
                    .is_err()
                {
                    self.dropped_inbound("identify");
                } else {
                    self.exchanged_one_periodic_identify = true;
                }
            }
            future::Either::Right(stream) => {
                if self
                    .active_streams
                    .try_push(protocol::recv_push(stream).map_ok(Success::ReceivedIdentifyPush))
                    .is_err()
                {
                    self.dropped_inbound("identify push");
                }
            }
        }
    }

    fn on_fully_negotiated_outbound(
        &mut self,
        FullyNegotiatedOutbound {
            protocol: output, ..
        }: FullyNegotiatedOutbound<<Self as ConnectionHandler>::OutboundProtocol, Outbound>,
    ) {
        match output {
            future::Either::Left(stream) => {
                if self
                    .active_streams
                    .try_push(protocol::recv_identify(stream).map_ok(Success::ReceivedIdentify))
                    .is_err()
                {
                    tracing::warn!("Dropping outbound identify stream because we are at capacity");
                }
            }
            future::Either::Right(stream) => {
                let info = self.build_info();
                let built = self.pushes.built;

                if self
                    .push_stream
                    .try_push(
                        protocol::send_identify(stream, info).map_ok(move |info| (built, info)),
                    )
                    .is_err()
                {
                    // Never: one push is open at a time (see `Pushes`).
                    tracing::warn!(
                        "Dropping outbound identify push stream because we are at capacity"
                    );
                    self.push_ended(false);
                }
            }
        }
    }

    fn build_info(&mut self) -> Info {
        self.ask_shown();
        self.pushes.built += 1;
        let signed_envelope = match self.local_key.as_ref() {
            KeyType::PublicKey(_) => None,
            KeyType::Keypair { keypair, .. } => libp2p_core::PeerRecord::new(
                keypair,
                Vec::from_iter(self.external_addresses.iter().cloned()),
            )
            .ok()
            .map(|r| r.into_signed_envelope()),
        };
        Info {
            public_key: self.local_key.public_key().clone(),
            protocol_version: self.protocol_version.clone(),
            agent_version: self.agent_version.clone(),
            listen_addrs: Vec::from_iter(self.external_addresses.iter().cloned()),
            protocols: Vec::from_iter(self.listed()),
            observed_addr: self.observed_addr.clone(),
            signed_peer_record: signed_envelope,
        }
    }

    /// If the public key matches the remote peer, handles the given `info` and returns `true`.
    fn handle_incoming_info(&mut self, info: &Info) -> bool {
        let derived_peer_id = info.public_key.to_peer_id();
        if self.remote_peer_id != derived_peer_id {
            return false;
        }

        self.remote_info.replace(info.clone());

        self.update_supported_protocols_for_remote(info);
        true
    }

    fn update_supported_protocols_for_remote(&mut self, remote_info: &Info) {
        let new_remote_protocols = HashSet::from_iter(remote_info.protocols.clone());

        let remote_added_protocols = new_remote_protocols
            .difference(&self.remote_supported_protocols)
            .cloned()
            .collect::<HashSet<_>>();
        let remote_removed_protocols = self
            .remote_supported_protocols
            .difference(&new_remote_protocols)
            .cloned()
            .collect::<HashSet<_>>();

        if !remote_added_protocols.is_empty() {
            self.events
                .push(ConnectionHandlerEvent::ReportRemoteProtocols(
                    ProtocolSupport::Added(remote_added_protocols),
                ));
        }

        if !remote_removed_protocols.is_empty() {
            self.events
                .push(ConnectionHandlerEvent::ReportRemoteProtocols(
                    ProtocolSupport::Removed(remote_removed_protocols),
                ));
        }

        self.remote_supported_protocols = new_remote_protocols;
    }

    fn local_protocols_to_string(&mut self) -> String {
        self.local_supported_protocols
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl ConnectionHandler for Handler {
    type FromBehaviour = InEvent;
    type ToBehaviour = Event;
    type InboundProtocol =
        SelectUpgrade<ReadyUpgrade<StreamProtocol>, ReadyUpgrade<StreamProtocol>>;
    type OutboundProtocol = Either<ReadyUpgrade<StreamProtocol>, ReadyUpgrade<StreamProtocol>>;
    type OutboundOpenInfo = Outbound;
    type InboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol> {
        SubstreamProtocol::new(
            SelectUpgrade::new(
                ReadyUpgrade::new(PROTOCOL_NAME),
                ReadyUpgrade::new(PUSH_PROTOCOL_NAME),
            ),
            (),
        )
    }

    fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
        match event {
            InEvent::AddressesChanged(addresses) => {
                self.external_addresses = addresses;
            }
            InEvent::Push(reason) => self.push(reason),
        }
    }

    #[tracing::instrument(level = "trace", name = "ConnectionHandler::poll", skip(self, cx))]
    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ConnectionHandlerEvent<Self::OutboundProtocol, Outbound, Event>> {
        if let Some(event) = self.events.pop() {
            return Poll::Ready(event);
        }

        // Poll the future that fires when we need to identify the node again.
        if let Poll::Ready(()) = self.trigger_next_identify.poll_unpin(cx) {
            self.trigger_next_identify.reset(self.interval);
            let event = ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(
                    Either::Left(ReadyUpgrade::new(PROTOCOL_NAME)),
                    Outbound::Identify,
                ),
            };
            return Poll::Ready(event);
        }

        if let Poll::Ready(ready) = self.push_stream.poll_unpin(cx) {
            if let Ok(Ok((built, info))) = &ready {
                self.told(*built, info);
            }
            self.push_ended(matches!(ready, Ok(Ok(_))));
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(match ready {
                Ok(Ok((_, info))) => Event::IdentificationPushed(info),
                Ok(Err(e)) => Event::IdentificationError(StreamUpgradeError::Apply(e)),
                Err(Timeout { .. }) => Event::IdentificationError(StreamUpgradeError::Timeout),
            }));
        }

        while let Poll::Ready(ready) = self.active_streams.poll_unpin(cx) {
            match ready {
                Ok(Ok(Success::ReceivedIdentify(remote_info))) => {
                    if self.handle_incoming_info(&remote_info) {
                        return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                            Event::Identified(remote_info),
                        ));
                    } else {
                        tracing::warn!(
                            %self.remote_peer_id,
                            ?remote_info.public_key,
                            derived_peer_id=%remote_info.public_key.to_peer_id(),
                            "Discarding received identify message as public key does not match remote peer ID",
                        );
                    }
                }
                Ok(Ok(Success::SentIdentify(built, info))) => {
                    self.told(built, &info);
                    return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                        Event::Identification,
                    ));
                }
                Ok(Ok(Success::ReceivedIdentifyPush(remote_push_info))) => {
                    if let Some(mut info) = self.remote_info.clone() {
                        info.merge(remote_push_info);

                        if self.handle_incoming_info(&info) {
                            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                                Event::Identified(info),
                            ));
                        } else {
                            tracing::warn!(
                                %self.remote_peer_id,
                                ?info.public_key,
                                derived_peer_id=%info.public_key.to_peer_id(),
                                "Discarding received identify message as public key does not match remote peer ID",
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                        Event::IdentificationError(StreamUpgradeError::Apply(e)),
                    ));
                }
                Err(Timeout { .. }) => {
                    return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                        Event::IdentificationError(StreamUpgradeError::Timeout),
                    ));
                }
            }
        }

        Poll::Pending
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<Self::InboundProtocol, Self::OutboundProtocol, (), Outbound>,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(fully_negotiated_inbound) => {
                self.on_fully_negotiated_inbound(fully_negotiated_inbound)
            }
            ConnectionEvent::FullyNegotiatedOutbound(fully_negotiated_outbound) => {
                self.on_fully_negotiated_outbound(fully_negotiated_outbound)
            }
            ConnectionEvent::DialUpgradeError(DialUpgradeError { info, error }) => {
                self.events.push(ConnectionHandlerEvent::NotifyBehaviour(
                    Event::IdentificationError(
                        #[allow(unused)]
                        error.map_upgrade_err(|e| libp2p_core::util::unreachable(e.into_inner())),
                    ),
                ));
                match info {
                    Outbound::Identify => self.trigger_next_identify.reset(self.interval),
                    Outbound::Push => self.push_ended(false),
                }
            }
            ConnectionEvent::LocalProtocolsChange(change) => {
                let before = tracing::enabled!(Level::DEBUG)
                    .then(|| self.local_protocols_to_string())
                    .unwrap_or_default();
                let listed = self.listed();
                let protocols_changed = self.local_supported_protocols.on_protocols_change(change)
                    && self.listed() != listed;
                let after = tracing::enabled!(Level::DEBUG)
                    .then(|| self.local_protocols_to_string())
                    .unwrap_or_default();

                if protocols_changed && self.exchanged_one_periodic_identify {
                    tracing::debug!(
                        peer=%self.remote_peer_id,
                        %before,
                        %after,
                        "Supported listen protocols changed, pushing to peer"
                    );

                    self.push(PushReason::Changed);
                }
            }
            _ => {}
        }
    }
}

enum Success {
    SentIdentify(u64, Info),
    ReceivedIdentify(Info),
    ReceivedIdentifyPush(PushInfo),
}

#[cfg(test)]
mod tests {
    use libp2p_identity::Keypair;

    use super::*;

    fn handler() -> Handler {
        Handler::new(
            Duration::from_secs(300),
            PeerId::random(),
            Arc::new(Keypair::generate_ed25519().public().into()),
            "ipfs/0.1.0".to_string(),
            "test".to_string(),
            "/ip4/127.0.0.1/udp/1/quic-v1".parse().unwrap(),
            HashSet::new(),
            None,
        )
    }

    /// The pushes the handler opens, of what it asks the connection for now.
    fn pushes_opened(handler: &mut Handler) -> usize {
        handler
            .events
            .drain(..)
            .filter(|e| {
                matches!(
                    e,
                    ConnectionHandlerEvent::OutboundSubstreamRequest { protocol }
                        if *protocol.info() == Outbound::Push
                )
            })
            .count()
    }

    fn address(i: u8) -> Multiaddr {
        format!("/ip4/203.0.113.{i}/udp/4001/quic-v1")
            .parse()
            .unwrap()
    }

    /// Thirty changes at once open one push, and one more once it ended, which says
    /// them all; never one per change, which the other end would mostly drop.
    #[test]
    fn pushes_on_a_connection_are_one_at_a_time() {
        let mut handler = handler();
        let mut addresses = HashSet::new();
        for i in 1..=30 {
            addresses.insert(address(i));
            handler.on_behaviour_event(InEvent::AddressesChanged(addresses.clone()));
            handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        }
        assert_eq!(pushes_opened(&mut handler), 1);
        // The first one sent what it was opened for (built when the stream opens) and
        // ended: the others go as one more, unless the first said it all already.
        let info = handler.build_info();
        handler.told(handler.pushes.built, &info);
        handler.push_ended(true);
        assert_eq!(pushes_opened(&mut handler), 0, "the first push said it all");

        // A change while a push is open is pushed after it.
        handler.on_behaviour_event(InEvent::Push(PushReason::Asked));
        assert_eq!(pushes_opened(&mut handler), 1);
        let info = handler.build_info();
        handler.told(handler.pushes.built, &info);
        addresses.insert(address(31));
        handler.on_behaviour_event(InEvent::AddressesChanged(addresses.clone()));
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 0);
        handler.push_ended(true);
        assert_eq!(pushes_opened(&mut handler), 1, "the change is pushed after");
    }

    /// A push for a change that changes nothing said is left out; one the node asks for
    /// goes anyway (the first may come too early for the other end); one that failed
    /// leaves what was said unknown, so the next change goes.
    #[test]
    fn a_push_says_something_new_unless_asked() {
        let mut handler = handler();
        handler.on_behaviour_event(InEvent::AddressesChanged(HashSet::from([address(1)])));
        // Said in an answer to the other end's identify, which got there.
        let info = handler.build_info();
        handler.told(handler.pushes.built, &info);
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 0);
        handler.on_behaviour_event(InEvent::Push(PushReason::Asked));
        assert_eq!(pushes_opened(&mut handler), 1);
        // Asked for again while it is open: once more after it, even with nothing new.
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        handler.on_behaviour_event(InEvent::Push(PushReason::Asked));
        let info = handler.build_info();
        handler.told(handler.pushes.built, &info);
        handler.push_ended(true);
        assert_eq!(pushes_opened(&mut handler), 1);
        // The stream for it could not be opened: the next change goes, whatever it is.
        handler.on_connection_event(ConnectionEvent::DialUpgradeError(DialUpgradeError {
            info: Outbound::Push,
            error: StreamUpgradeError::Timeout,
        }));
        handler.events.clear();
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 1);
    }

    /// Polls `handler` until it says an identify failed, and gives that, with how many
    /// pushes it opened meanwhile.
    fn until_failed(handler: &mut Handler) -> (StreamUpgradeError<UpgradeError>, usize) {
        let mut opened = 0;
        futures::executor::block_on(future::poll_fn(|cx| {
            loop {
                match handler.poll(cx) {
                    Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(
                        Event::IdentificationError(e),
                    )) => return Poll::Ready((e, opened)),
                    Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest { protocol })
                        if *protocol.info() == Outbound::Push =>
                    {
                        opened += 1
                    }
                    Poll::Ready(_) => {}
                    Poll::Pending => return Poll::Pending,
                }
            }
        }))
    }

    /// An answer and a push ending out of order: the one built last stands for what was
    /// said, whichever ended last.
    #[test]
    fn what_was_built_last_stands_for_what_was_said() {
        let mut handler = handler();
        handler.on_behaviour_event(InEvent::AddressesChanged(HashSet::from([address(1)])));
        let older = handler.build_info();
        let older_built = handler.pushes.built;
        handler.on_behaviour_event(InEvent::AddressesChanged(HashSet::from([address(2)])));
        let newer = handler.build_info();
        let newer_built = handler.pushes.built;
        handler.told(newer_built, &newer);
        handler.told(older_built, &older);
        // Nothing new to say: the newer addresses were said.
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 0);
    }

    /// An answer that did not get there leaves what the other end knows unknown: the next
    /// change is pushed even if the answer said it.
    #[test]
    fn a_failed_answer_leaves_the_next_change_pushed() {
        let mut handler = handler();
        handler.on_behaviour_event(InEvent::AddressesChanged(HashSet::from([address(1)])));
        // Built for an answer whose stream failed: never told.
        let _ = handler.build_info();
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 1);
    }

    /// A push that timed out or failed ends: the one asked for meanwhile goes, and what
    /// failed leaves the next change to be pushed.
    #[test]
    fn a_push_that_times_out_or_fails_ends() {
        let mut handler = handler();
        handler.push_stream = futures_bounded::FuturesSet::new(
            || futures_bounded::Delay::futures_timer(Duration::from_millis(20)),
            1,
        );
        handler.on_behaviour_event(InEvent::Push(PushReason::Asked));
        assert_eq!(pushes_opened(&mut handler), 1);
        handler.on_behaviour_event(InEvent::AddressesChanged(HashSet::from([address(1)])));
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 0, "one at a time");
        // Its stream opened and never ended.
        assert!(
            handler
                .push_stream
                .try_push(future::pending().boxed())
                .is_ok()
        );
        let (error, opened) = until_failed(&mut handler);
        assert!(matches!(error, StreamUpgradeError::Timeout), "{error:?}");
        let opened = opened + pushes_opened(&mut handler);
        assert_eq!(opened, 1, "the change asked for meanwhile went");
        assert!(handler.pushes.open);

        // That one's stream failed.
        assert!(
            handler
                .push_stream
                .try_push(future::ready(Err(UpgradeError::StreamClosed)).boxed())
                .is_ok()
        );
        let (error, opened) = until_failed(&mut handler);
        assert!(matches!(
            error,
            StreamUpgradeError::Apply(UpgradeError::StreamClosed)
        ));
        assert_eq!(opened + pushes_opened(&mut handler), 0);
        assert!(!handler.pushes.open);
        assert!(handler.pushes.said.is_none());
        handler.on_behaviour_event(InEvent::Push(PushReason::Changed));
        assert_eq!(pushes_opened(&mut handler), 1);
    }
}
