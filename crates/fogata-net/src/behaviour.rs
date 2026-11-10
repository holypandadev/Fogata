//! The libp2p behaviour stack of a Fogata node.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use fogata_proto::{
    FRIEND_PROTOCOL, FriendRequest, FriendResponse, MEMBER_PROTOCOL, MemberRequest, MemberResponse,
};
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, dcutr, identify,
    identity::Keypair,
    kad, mdns, ping, relay,
    request_response::{self, ProtocolSupport},
    swarm::{
        NetworkBehaviour,
        behaviour::{ExternalAddrConfirmed, ExternalAddrExpired, FromSwarm, toggle::Toggle},
    },
};

use crate::{
    codec::Guarded,
    rendezvous::AMINO_PROTOCOL,
    strangers::{KeepWanted, Wanted},
};

/// Peers that proved membership of a group on this node, or the part of them the relay
/// server serves (see [`Options::relay_peers`]).
pub(crate) type VerifiedPeers = Arc<Mutex<HashSet<PeerId>>>;

/// Peers this device lists Fogata's own protocols to in identify, besides those it wants
/// (see [`Behaviour::new`]): those that proved they are members or friends, while connected.
pub(crate) type ShownPeers = Arc<Mutex<HashSet<PeerId>>>;

/// The beginning of the names of Fogata's own protocols.
pub(crate) const FOGATA_PROTOCOLS: &str = "/fogata/";

/// The protocol version identify says.
pub(crate) const PROTOCOL_VERSION: &str = "ipfs/0.1.0";

/// How often the local network is asked for other devices.
const MDNS_QUERY_INTERVAL: Duration = Duration::from_secs(30);

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub(crate) relay_client: relay::client::Behaviour,
    pub(crate) relay: relay::Behaviour,
    pub(crate) dcutr: dcutr::Behaviour,
    pub(crate) identify: identify::Behaviour,
    pub(crate) ping: ping::Behaviour,
    pub(crate) kad: Toggle<kad::Behaviour<kad::store::MemoryStore>>,
    pub(crate) mdns: Toggle<mdns::tokio::Behaviour>,
    pub(crate) member: request_response::Behaviour<Guarded<MemberRequest, MemberResponse>>,
    pub(crate) friend: request_response::Behaviour<Guarded<FriendRequest, FriendResponse>>,
    /// Raw streams for protocols built on top of the node, such as media.
    pub(crate) stream: libp2p_stream::Behaviour,
    keep: KeepWanted,
}

pub(crate) struct Options {
    pub(crate) use_dht: bool,
    pub(crate) use_mdns: bool,
    pub(crate) bridge: bool,
    /// The peers the relay serves: those that proved membership of a group other than a
    /// direct chat's (a chat's group has no relay).
    pub(crate) relay_peers: VerifiedPeers,
    pub(crate) wanted: Wanted,
    /// See [`ShownPeers`].
    pub(crate) shown: ShownPeers,
}

impl Behaviour {
    pub(crate) fn new(
        key: &Keypair,
        relay_client: relay::client::Behaviour,
        options: Options,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let peer_id = key.public().to_peer_id();

        let kad = if options.use_dht {
            let mut config = kad::Config::new(StreamProtocol::new(AMINO_PROTOCOL));
            config.set_query_timeout(Duration::from_secs(60));
            // The node bootstraps again itself, seldom while the routing table is healthy
            // (see `Node::bootstrap_when_due`); the library still does when a peer joins a
            // table with fewer than 20.
            config.set_periodic_bootstrap_interval(None);
            let mut kad =
                kad::Behaviour::with_config(peer_id, kad::store::MemoryStore::new(peer_id), config);
            // Fogata devices only use the DHT; they never serve it, even when reachable.
            kad.set_mode(Some(kad::Mode::Client));
            Some(kad)
        } else {
            None
        };

        let mdns = if options.use_mdns {
            // libp2p stops asking as soon as any device answers, then asks again only every
            // five minutes: a lost answer, or a friend that restarts while this device's
            // firewall keeps it from coming in, would wait that long on the same network.
            let config = mdns::Config {
                query_interval: MDNS_QUERY_INTERVAL,
                ..mdns::Config::default()
            };
            Some(mdns::tokio::Behaviour::new(config, peer_id)?)
        } else {
            None
        };

        // A member relays for the others whether or not it can be reached from the internet:
        // it only has to be connected to both ends. libp2p would serve only once an address
        // of this device is confirmed from outside, which never happens on a network without
        // internet and often not behind a NAT; only verified members get through anyway.
        let mut relay =
            relay::Behaviour::new(peer_id, relay_config(options.bridge, options.relay_peers));
        relay.set_status(Some(relay::Status::Enable));

        // Fogata's own protocols are listed only to peers this device wants (chose to reach,
        // or that proved something) and to members and friends while they are connected:
        // a DHT node or anyone else that connects does not learn from identify that this
        // device runs Fogata, nor its version. One that knows a protocol's name can still
        // try it. A peer that proves itself later is told them then (`Node::show_protocols`).
        let wanted = options.wanted.clone();
        let shown = options.shown;
        let shown_to =
            move |peer: &PeerId| wanted.is_wanted(peer) || crate::locked(&shown).contains(peer);

        Ok(Self {
            relay_client,
            relay,
            dcutr: dcutr::Behaviour::new(peer_id),
            identify: identify::Behaviour::new(
                // The protocol version most libp2p peers say (go-libp2p's default), so that
                // this field does not tell DHT nodes the device runs Fogata either.
                identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
                    // Only share addresses others have confirmed, never LAN addresses.
                    .with_hide_listen_addrs(true)
                    .with_push_listen_addr_updates(true)
                    .with_protocols_shown(FOGATA_PROTOCOLS, shown_to),
            ),
            ping: ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(15))),
            kad: kad.into(),
            mdns: mdns.into(),
            member: request_response::Behaviour::with_codec(
                message_codec(),
                [(StreamProtocol::new(MEMBER_PROTOCOL), ProtocolSupport::Full)],
                message_config(),
            ),
            friend: request_response::Behaviour::with_codec(
                message_codec(),
                [(StreamProtocol::new(FRIEND_PROTOCOL), ProtocolSupport::Full)],
                message_config(),
            ),
            stream: libp2p_stream::Behaviour::new(),
            keep: KeepWanted::new(options.wanted),
        })
    }
}

impl Behaviour {
    /// Gives the relay server an address of this device (`added`) or takes it back. A slot
    /// the server grants must name at least one address of the server, and without one
    /// confirmed from outside it has none: its own listen addresses serve. Only members get
    /// slots, and they learn this device's local addresses anyway.
    pub(crate) fn relay_address(&mut self, address: &Multiaddr, added: bool) {
        let event = if added {
            FromSwarm::ExternalAddrConfirmed(ExternalAddrConfirmed { addr: address })
        } else {
            FromSwarm::ExternalAddrExpired(ExternalAddrExpired { addr: address })
        };
        self.relay.on_swarm_event(event);
    }
}

/// Requests and answers carry whole group logs (to a joining device, to a friend added to
/// a group), which grow past the library's 1 MiB for requests: up to 4 MiB each way, and
/// never more data items than such a log needs (see codec.rs).
pub(crate) fn message_codec<Req, Resp>() -> Guarded<Req, Resp> {
    Guarded::new(MAX_MESSAGE_BYTES)
}

pub(crate) const MAX_MESSAGE_BYTES: u64 = 4 * 1024 * 1024;

fn message_config() -> request_response::Config {
    request_response::Config::default()
        .with_request_timeout(Duration::from_secs(20))
        .with_max_concurrent_streams(16)
}

/// How long one circuit through this device's relay lasts with `bridge`: a long call (3 hours
/// of video) and some, not a whole day of someone else's traffic.
const BRIDGE_CIRCUIT_DURATION: Duration = Duration::from_secs(6 * 60 * 60);
/// Most bytes one circuit carries with `bridge`: enough for a long call through this device
/// (about 3 hours of video at 1.5 Mbit/s), not for someone to route everything through it.
const BRIDGE_CIRCUIT_BYTES: u64 = 2 << 30;
/// Circuits one member may open through this device's relay with `bridge`: a few at once
/// (a call, its reconnections), then one every few minutes. libp2p knows nothing of the
/// bytes a circuit carried, only how many it may carry, so this bounds what one member can
/// make this device carry over time: [`BRIDGE_CIRCUIT_BYTES`] per circuit, a little more
/// than 25 Mbit/s at most when used to the full, instead of four circuits at a time at the
/// speed of the link.
const BRIDGE_CIRCUIT_BURST: f64 = 8.0;
const BRIDGE_CIRCUITS_PER_SECOND: f64 = 1.0 / (10.0 * 60.0);
/// Sources whose circuit budget is kept; past it, those whose budget is full again go.
const BRIDGE_SOURCES_KEPT: usize = 256;

/// The circuits each source may still open through the relay (see
/// [`BRIDGE_CIRCUIT_BURST`]).
#[derive(Default)]
struct CircuitBudget {
    sources: HashMap<PeerId, (f64, Instant)>,
}

impl CircuitBudget {
    fn tokens(&self, (tokens, at): (f64, Instant), now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(at).as_secs_f64();
        (tokens + elapsed * BRIDGE_CIRCUITS_PER_SECOND).min(BRIDGE_CIRCUIT_BURST)
    }
}

impl relay::RateLimiter for CircuitBudget {
    fn try_next(&mut self, peer: PeerId, _: &Multiaddr, now: Instant) -> bool {
        if self.sources.len() >= BRIDGE_SOURCES_KEPT && !self.sources.contains_key(&peer) {
            let sources = std::mem::take(&mut self.sources);
            self.sources = sources
                .into_iter()
                .filter(|(_, budget)| self.tokens(*budget, now) < BRIDGE_CIRCUIT_BURST)
                .collect();
            if self.sources.len() >= BRIDGE_SOURCES_KEPT {
                return false;
            }
        }
        let budget = self
            .sources
            .get(&peer)
            .copied()
            .unwrap_or((BRIDGE_CIRCUIT_BURST, now));
        let tokens = self.tokens(budget, now);
        if tokens < 1.0 {
            self.sources.insert(peer, (tokens, now));
            return false;
        }
        self.sources.insert(peer, (tokens - 1.0, now));
        true
    }
}

/// Relay server limited to verified members of regular groups. With `bridge` it carries
/// their traffic for as long as a call needs, a bounded number of circuits per member over
/// time; without it, circuits only live long enough to coordinate a hole punch (libp2p's
/// defaults: 2 minutes, 128 KiB). libp2p's relay asks nothing about a circuit's other end
/// beyond it holding a reservation here, which only verified members get: a circuit between
/// members of two different groups of this device's is not told apart.
fn relay_config(bridge: bool, verified: VerifiedPeers) -> relay::Config {
    let mut config = relay::Config {
        max_reservations: 32,
        max_reservations_per_peer: 2,
        reservation_duration: Duration::from_secs(60 * 60),
        max_circuits: 32,
        max_circuits_per_peer: 4,
        ..Default::default()
    };
    if bridge {
        config.max_circuit_duration = BRIDGE_CIRCUIT_DURATION;
        config.max_circuit_bytes = BRIDGE_CIRCUIT_BYTES;
        config
            .circuit_src_rate_limiters
            .push(Box::new(CircuitBudget::default()));
    }
    let for_reservations = verified.clone();
    config.reservation_rate_limiters.push(Box::new(
        move |peer: PeerId, _: &libp2p::Multiaddr, _| is_verified(&for_reservations, &peer),
    ));
    config.circuit_src_rate_limiters.push(Box::new(
        move |peer: PeerId, _: &libp2p::Multiaddr, _| is_verified(&verified, &peer),
    ));
    config
}

fn is_verified(verified: &VerifiedPeers, peer: &PeerId) -> bool {
    crate::locked(verified).contains(peer)
}

#[cfg(test)]
mod tests {
    use libp2p::request_response::Codec as _;
    use serde_bytes::ByteBuf;

    use super::*;

    const PROTOCOL: StreamProtocol = StreamProtocol::new(MEMBER_PROTOCOL);

    /// What a stranger (a DHT node) reads in a device's identify: the version most libp2p
    /// peers say, an agent of the library's, not Fogata's name, and none of Fogata's
    /// protocols.
    #[tokio::test(flavor = "multi_thread")]
    async fn identify_does_not_name_fogata() {
        use futures::StreamExt;
        use libp2p::{SwarmBuilder, swarm::SwarmEvent};

        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (_node, _events) = crate::start(crate::NodeConfig {
            identity: Arc::new(fogata_crypto::DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
            friends: None,
            listen_port: port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let mut stranger = SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_quic()
            .with_behaviour(|key| {
                identify::Behaviour::new(identify::Config::new(
                    "ipfs/0.1.0".to_string(),
                    key.public(),
                ))
            })
            .unwrap()
            .build();
        let address: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        stranger.dial(address).unwrap();
        let info = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let SwarmEvent::Behaviour(identify::Event::Received { info, .. }) =
                    stranger.select_next_some().await
                {
                    return info;
                }
            }
        })
        .await
        .expect("the device identified itself");
        assert_eq!(info.protocol_version, PROTOCOL_VERSION);
        assert!(!info.protocol_version.to_lowercase().contains("fogata"));
        assert!(!info.agent_version.to_lowercase().contains("fogata"));
        let listed: Vec<&str> = info.protocols.iter().map(|p| p.as_ref()).collect();
        assert!(
            listed.iter().all(|p| !p.contains("fogata")),
            "a stranger was told {listed:?}"
        );
    }

    /// A member opens circuits through the relay a few at once, then one every few
    /// minutes, whatever it tries; another member has its own.
    #[test]
    fn a_member_opens_a_few_circuits_then_one_every_few_minutes() {
        use relay::RateLimiter as _;
        let mut budget = CircuitBudget::default();
        let (bea, cai) = (PeerId::random(), PeerId::random());
        let at: Multiaddr = "/ip4/192.0.2.1/udp/4001/quic-v1".parse().unwrap();
        let start = Instant::now();
        let opened = (0..100)
            .filter(|_| budget.try_next(bea, &at, start))
            .count();
        assert_eq!(opened, BRIDGE_CIRCUIT_BURST as usize);
        assert!(budget.try_next(cai, &at, start));
        assert!(!budget.try_next(bea, &at, start + Duration::from_secs(60)));
        assert!(budget.try_next(bea, &at, start + Duration::from_secs(10 * 60)));
        assert!(!budget.try_next(bea, &at, start + Duration::from_secs(10 * 60)));
        // Made-up sources, as many as one likes: a bounded number kept.
        for _ in 0..1000 {
            budget.try_next(PeerId::random(), &at, start);
        }
        assert!(budget.sources.len() <= BRIDGE_SOURCES_KEPT);
    }

    fn round_trip(request: MemberRequest) -> std::io::Result<MemberRequest> {
        futures::executor::block_on(async {
            let mut codec = message_codec::<MemberRequest, MemberResponse>();
            let mut wire = Vec::new();
            codec.write_request(&PROTOCOL, &mut wire, request).await?;
            let mut io = futures::io::Cursor::new(wire);
            codec.read_request(&PROTOCOL, &mut io).await
        })
    }

    #[test]
    fn a_message_of_countless_tiny_items_is_refused_before_decoding() {
        // Each empty byte string is one byte on the wire and a 24-byte value once decoded.
        let have = vec![ByteBuf::new(); 200_000];
        let request = MemberRequest::Sync {
            group_id: vec![0; 16],
            have,
        };
        let error = round_trip(request).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn whole_logs_and_their_hashes_still_go() {
        let entries = vec![fogata_proto::SharedBytes::from(vec![7u8; 1024]); 3000];
        let push = MemberRequest::Push {
            group_id: vec![0; 16],
            entries,
        };
        let Ok(MemberRequest::Push { entries, .. }) = round_trip(push) else {
            panic!("a log of 3 MB did not go");
        };
        assert_eq!(entries.len(), 3000);
        assert_eq!(*entries[2999], [7u8; 1024]);
        let have = vec![ByteBuf::from(vec![9u8; 32]); 100_000];
        let sync = MemberRequest::Sync {
            group_id: vec![0; 16],
            have,
        };
        let Ok(MemberRequest::Sync { have, .. }) = round_trip(sync) else {
            panic!("the hashes of a long log did not go");
        };
        assert_eq!(have.len(), 100_000);
    }
}
