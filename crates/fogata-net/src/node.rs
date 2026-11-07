//! The event loop of a node: discovery, connection scheduling, and the groups and friends it
//! runs. One node runs every group of the device at once; connections, the DHT and hole
//! punching are shared, and each group keeps its own members, keys and room.
//!
//! This file holds the node's state and its start; the loop itself is in run.rs, and each
//! module below handles one part of what the loop does.

mod call_sessions;
mod candidates;
mod commands;
mod dht;
mod dial;
mod direct;
mod early;
mod friend_contact;
mod friend_lookups;
mod friend_offers;
mod friend_profile;
mod friends;
mod group_changes;
mod heartbeat;
mod joins;
mod judging;
mod limits;
mod links;
mod mdns_dials;
mod member_reach;
mod members;
mod network;
mod outbox;
mod presence;
mod proofs;
mod room;
#[cfg(feature = "fuzzing")]
pub(crate) use room::{room_input, room_part};
mod run;
mod sessions;
mod shown;
mod slot;
mod spread;
mod swarm_events;
mod transfer;
pub(crate) mod typing;
mod udp_check;
mod wanted;

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_proto::{MemberRequest, WireMessage};
use fogata_state::{RoomTracker, VoiceRoomId};
use libp2p::{
    Multiaddr, PeerId, Swarm, SwarmBuilder,
    kad::{self, RecordKey},
    noise,
    request_response::OutboundRequestId,
    swarm::ConnectionId,
    tcp, yamux,
};
use tokio::sync::mpsc;

use crate::{
    Advertised, Command, GroupMembers, Levels, NetError, NodeConfig, NodeEvent, NodeHandle, Path,
    addr,
    behaviour::{Behaviour, Options, ShownPeers, VerifiedPeers},
    caps,
    group::{GroupContext, GroupSetup},
    lan::{self, LanKeys},
    strangers::{self, StrangerDials, Wanted},
};

use self::candidates::Budget;
use self::dht::LanNames;
use self::dial::MAX_REMEMBERED;
use self::friends::FriendContext;
use self::mdns_dials::{MdnsDials, MdnsRetry};
use self::run::Answer;
#[cfg(debug_assertions)]
use self::run::TestPanic;

/// Every device tries to connect to the members it knows about at the same wall-clock
/// instants, so that both ends of a hole punch send at roughly the same time without having
/// to talk first.
const PUNCH_SLOT: Duration = Duration::from_secs(10);
/// How long one connection attempt may take; shorter than a slot.
const PUNCH_ATTEMPT: Duration = Duration::from_secs(7);
/// How long any connection may take to set up, in or out, before it is closed: a TCP
/// connection takes about five round trips (TCP, Noise and the two negotiations), a QUIC one
/// two, and one through a member's relay a few more, and a slow or lossy link (a satellite,
/// a crowded mobile network) loses a packet or two on the way. It bounds each punch's
/// attempt too, which must have its whole [`PUNCH_ATTEMPT`]. An incoming TCP connection from
/// a stranger gets less ([`strangers::STRANGER_SETUP`]).
pub(crate) const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const _: () = assert!(PUNCH_ATTEMPT.as_millis() < CONNECTION_TIMEOUT.as_millis());
/// Incoming QUIC connections being set up past which a new one must prove its address
/// first (see vendor/libp2p-quic, `Config::retry_above`): a few dozen is more than a busy
/// device sees at once, and the extra round trip then costs honest peers little.
const QUIC_RETRY_ABOVE: usize = 64;
/// How often to look for a group's members on the DHT while none is connected, and
/// afterwards.
const LOOKUP_IDLE: Duration = Duration::from_secs(30);
const LOOKUP_CONNECTED: Duration = Duration::from_secs(120);
const LOOKUP_QUIET: Duration = Duration::from_secs(240);
/// How often to look up when there is nobody to find: every member of the group is
/// connected (or there is no other), or every friend is online and nothing else is sought.
/// Someone that goes away, a change and a network move make the lookups frequent again at
/// once; this only spares the public DHT (and this device) the lookups of an idle device.
const LOOKUP_NOBODY_MISSING: Duration = Duration::from_secs(15 * 60);

/// Listen addresses that appear this long after the start come from a network joined while
/// running, not from the start itself.
const STARTUP: Duration = Duration::from_secs(10);

/// How long after the start a request for this device's addresses waits for the first
/// address on a network (see [`Node::answer_addresses`]).
const ADDRESSES_WAIT: Duration = Duration::from_secs(5);

/// After a member or friend went without a goodbye (another network, say), how long it is
/// looked up at every slot: it announces its new address within moments, and the other
/// side has to learn it to punch through to it.
const SEARCH_HARD: Duration = Duration::from_secs(120);

/// How long a QUIC connection waits without hearing anything before it is closed.
const QUIC_IDLE: Duration = Duration::from_secs(30);
/// Message ids remembered to drop duplicates.
const SEEN_LIMIT: usize = 4096;

pub(crate) fn start(config: NodeConfig) -> Result<(NodeHandle, crate::NodeEvents), NetError> {
    let setup = |e: &dyn std::fmt::Display| NetError::Setup(e.to_string());

    // A new transport identity for every session, unrelated to the device identity, so the
    // network cannot link sessions of the same device.
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    let verified: VerifiedPeers = Default::default();
    let relay_peers: VerifiedPeers = Default::default();
    let group_members: GroupMembers = Default::default();
    let member_devices: crate::MemberDevices = Default::default();
    let advertised: Advertised = Default::default();
    let levels: Levels = Default::default();
    let wanted = Wanted::default();
    let shown: ShownPeers = Default::default();
    let lan_keys = LanKeys::default();
    let options = Options {
        use_dht: config.use_dht,
        use_mdns: config.use_mdns,
        bridge: config.bridge,
        relay_peers: relay_peers.clone(),
        wanted: wanted.clone(),
        shown: shown.clone(),
    };

    let dns = crate::dns::system_settings();
    let dns_note = dns.note.clone();
    let mut swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_quic_config(|mut quic| {
            // Each attempt must end before the next punch slot starts; otherwise a still
            // running attempt blocks the next one and the two ends drift out of phase.
            quic.handshake_timeout = PUNCH_ATTEMPT;
            // A connection outlives an outage of up to this long (a router restarting, a
            // train going through a tunnel), so a call goes on where it was. Keep-alives
            // every 5 s still keep NAT mappings open; the shorter of both ends' applies.
            quic.max_idle_timeout = QUIC_IDLE.as_millis() as u32;
            // While many QUIC connections are being set up, a new one from an address
            // not yet proven to be the sender's is answered with a retry packet first:
            // a flood of first packets with made-up source addresses holds no handshake
            // state, and the limits per address in strangers.rs count real addresses.
            quic.retry_above = QUIC_RETRY_ABOVE;
            quic
        })
        // TCP as the builder's own, with a shorter time for strangers' incoming connections.
        .with_other_transport(|key| {
            use libp2p::core::{Transport as _, muxing::StreamMuxerBox, upgrade::Version};
            let tcp = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
                .upgrade(Version::V1Lazy)
                .authenticate(noise::Config::new(key).map_err(Box::new)?)
                .multiplex(yamux_config())
                .map(|(peer, muxer), _| (peer, StreamMuxerBox::new(muxer)))
                .boxed();
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(strangers::StrangerSetup::new(
                tcp,
                wanted.clone(),
            ))
        })
        .map_err(|e| setup(&e))?
        .with_dns_config(dns.config, dns.opts)
        .with_relay_client(noise::Config::new, yamux_config)
        .map_err(|e| setup(&e))?
        .with_behaviour(|key, relay_client| {
            Behaviour::new(key, relay_client, options)
                .map(|behaviour| StrangerDials::new(behaviour, wanted.clone(), lan_keys.clone()))
        })
        .map_err(|e| setup(&e))?
        // Connections to wanted peers stay open longer (see strangers.rs).
        .with_swarm_config(|c| c.with_idle_connection_timeout(strangers::IDLE))
        .with_connection_timeout(CONNECTION_TIMEOUT)
        .build();

    let port = config.listen_port;
    for (ip, transport) in [
        ("/ip4/0.0.0.0", "udp"),
        ("/ip6/::", "udp"),
        ("/ip4/0.0.0.0", "tcp"),
        ("/ip6/::", "tcp"),
    ] {
        let at = |port: u16| -> Multiaddr {
            let quic = if transport == "udp" { "/quic-v1" } else { "" };
            format!("{ip}/{transport}/{port}{quic}")
                .parse()
                .expect("static listen address")
        };
        // The device's own port, or another free one when another program holds it (a
        // second copy, in tests): the device is still found, only a firewall rule would not
        // match. Another of the devices' ports first, which others take in its sealed local
        // addresses (see lan.rs), then any.
        if let Err(e) = swarm.listen_on(at(port)) {
            tracing::warn!("cannot listen on {}: {e}", at(port));
            let mut others = (0..8).filter_map(|_| {
                let mut bytes = [0u8; 2];
                fogata_crypto::random_bytes(&mut bytes).ok()?;
                let span = lan::PORTS.end - lan::PORTS.start;
                Some(lan::PORTS.start + u16::from_le_bytes(bytes) % span)
            });
            let listening = port == 0
                || others.any(|other| swarm.listen_on(at(other)).is_ok())
                || swarm.listen_on(at(0)).is_ok();
            if !listening {
                tracing::warn!("cannot listen on {}", at(0));
            }
        }
    }

    let (commands_tx, commands_rx) = mpsc::channel(64);
    let (stopping, stopped) = tokio::sync::watch::channel(());
    let (events_tx, events_rx) = crate::events::queue();
    let local_peer = *swarm.local_peer_id();
    let streams = swarm.behaviour().stream.new_control();
    // A start that fails from here on drops the tasks, which aborts them.
    let tasks = crate::Tasks::new();
    let typing = typing::Typing::start(
        swarm.behaviour().stream.new_control(),
        verified.clone(),
        stopped.clone(),
        tasks.clone(),
    )?;
    // Said in identify, so members know this device takes `Spread` and `RoomEvents`; nothing
    // is sent over it, and a stream opened on it is dropped.
    let mut marker = swarm
        .behaviour()
        .stream
        .new_control()
        .accept(spread::SPREAD_PROTOCOL)
        .map_err(|e| setup(&e))?;
    let stop = stopped.clone();
    tasks.spawn(async move {
        while crate::next_until_stopped(&stop, &mut marker)
            .await
            .is_some()
        {}
    });
    // This device's capability level, listed to every peer it connects to. Nothing is
    // sent over it: a stream someone opens is closed at once.
    let mut caps_streams = swarm
        .behaviour()
        .stream
        .new_control()
        .accept(caps::caps_protocol(caps::CAPS_LEVEL))
        .map_err(|e| setup(&e))?;
    let stop = stopped.clone();
    tasks.spawn(async move {
        while crate::next_until_stopped(&stop, &mut caps_streams)
            .await
            .is_some()
        {}
    });

    let mut node = Node {
        swarm,
        last_sealed_ms: 0,
        groups: BTreeMap::new(),
        friends: config.friends.map(FriendContext::new),
        lan_seen: HashSet::new(),
        #[cfg(test)]
        mdns_dials: Vec::new(),
        #[cfg(test)]
        pushed: HashMap::new(),
        mdns_dialling: HashMap::new(),
        mdns_retry: HashMap::new(),
        quic_tried: HashSet::new(),
        lan_redialled: HashMap::new(),
        peer_listening: HashMap::new(),
        member_lan: HashMap::new(),
        early: HashMap::new(),
        dns_note: dns_note.clone(),
        identity: config.identity,
        verified: verified.clone(),
        relay_peers,
        relay_users: HashSet::new(),
        relay_redial: HashMap::new(),
        after_call: HashMap::new(),
        group_members: group_members.clone(),
        member_devices: member_devices.clone(),
        advertised: advertised.clone(),
        levels: levels.clone(),
        wanted,
        shown,
        push_again: HashMap::new(),
        bridge: config.bridge,
        hidden: config.hidden,
        away: false,
        members: HashMap::new(),
        connections: HashMap::new(),
        pending: HashMap::new(),
        outbox: HashMap::new(),
        presence: presence::PresenceWays::default(),
        farewell: HashSet::new(),
        sessions: sessions::Sessions::default(),
        call_routes: call_sessions::CallRoutes::default(),
        typing,
        interesting: HashSet::new(),
        hinted: HashMap::new(),
        group_keys: HashMap::new(),
        candidates: HashMap::new(),
        failures: HashMap::new(),
        explicit_dials: HashSet::new(),
        explicit_peers: HashMap::new(),
        remembered: Vec::new(),
        came_from: VecDeque::new(),
        reached: HashSet::new(),
        punch_dials: HashSet::new(),
        punching: HashMap::new(),
        reservations: HashMap::new(),
        observed: HashMap::new(),
        confirmed_seen: HashMap::new(),
        confirmed: HashSet::new(),
        listen_addrs: Vec::new(),
        asked_addresses: Vec::new(),
        lan_keys,
        lan_names: LanNames::default(),
        port_change_reported: false,
        udp: udp_check::UdpCheck::default(),
        dialled_tcp: HashMap::new(),
        public_contact: None,
        failed_public_dials: 0,
        seen: HashSet::new(),
        seen_order: VecDeque::new(),
        provided: (Vec::new(), None),
        announce_retry: (None, 0),
        announced: false,
        lookup_dials: HashSet::new(),
        dialling_found: HashSet::new(),
        candidate_found: HashMap::new(),
        lookup_new: HashMap::new(),
        lookup_taken: HashMap::new(),
        lookup_named: HashMap::new(),
        lookup_query: HashMap::new(),
        answering: None,
        corroborated: HashSet::new(),
        reached_peers: HashSet::new(),
        slot_count: 0,
        started: Instant::now(),
        #[cfg(debug_assertions)]
        test_panic: TestPanic::from_env(),
        last_slot_ms: None,
        last_bootstrap: Instant::now(),
        named_bootstrap: HashSet::new(),
        last_network_change: None,
        network_pending: None,
        ping_misses: HashMap::new(),
        through_dials: HashMap::new(),
        silent: HashMap::new(),
        closing: HashSet::new(),
        redundant: HashMap::new(),
        lost: HashMap::new(),
        limits: Default::default(),
        room_removes: Default::default(),
        judging: Default::default(),
        events: events_tx,
        serving: None,
        serving_answer: None,
        polling_swarm: false,
        #[cfg(test)]
        panic_polling: false,
        dropped_lookups: VecDeque::new(),
        muted_tellers: HashMap::new(),
        afresh: 0,
        commands: commands_rx,
        _stopping: stopping,
        tasks: tasks.clone(),
        seeking: None,
    };
    for setup in config.groups {
        node.add_group(setup);
    }
    if let Some(note) = dns_note {
        node.info(note);
    }
    node.bootstrap(&config.bootstrap);
    node.dial_addresses(config.peers);
    node.remembered = config.remembered;
    node.remembered.truncate(MAX_REMEMBERED);
    node.publish_known_addresses();
    node.dial_remembered();
    tokio::spawn(node.run());

    Ok((
        NodeHandle {
            commands: crate::Commands::Keeps(commands_tx),
            stopped,
            tasks,
            local_peer,
            streams,
            verified,
            group_members,
            member_devices,
            advertised,
            levels,
        },
        events_rx,
    ))
}

/// Streams at once on one TCP or relayed connection, as on a QUIC one: yamux takes 512 by
/// default, and each stream of a member's can hold a task (and, for a file, a file) here.
const MAX_STREAMS: usize = 256;

fn yamux_config() -> yamux::Config {
    let mut config = yamux::Config::default();
    config.set_max_num_streams(MAX_STREAMS);
    config
}

/// Joins waiting for a manager's answer at once with one invite, and in all for a group. An
/// invite passed on (or posted where anyone reads it) would otherwise let anyone ask without
/// end, every made-up device one more request on the manager's screen; one that is refused
/// keeps asking, and gets its turn once the ones before it are answered.
const MAX_WAITING_PER_INVITE: usize = 3;
const MAX_WAITING_JOINS: usize = 16;

/// A device that showed a valid invite and signed its `Join`, waiting for a manager's yes.
struct WaitingJoin {
    peer: PeerId,
    entry: fogata_state::Entry,
    ticket: fogata_state::Ticket,
}

/// What a member request was, to handle its answer.
#[derive(Clone, Copy, Debug)]
enum Pending {
    /// A challenge about a group, with the challenge sent: its answer is checked against
    /// that one, even when another was sent since.
    Challenge(GroupId, [u8; 32]),
    Deliver,
    Roster(GroupId),
    /// A group's banner, with its digest: a member that refuses it is not sent it again.
    Banner(GroupId, [u8; 32]),
    /// A group's picture record, with its digest, the same way.
    Picture(GroupId, [u8; 32]),
    Sync(GroupId),
    Push,
    /// A part of the log sent to a device let in (see `members::admit`).
    AdmittedPart(GroupId),
    Join(GroupId),
    JoinCommit(GroupId),
    Profile,
    /// The room's events of a group, from the event given.
    RoomSync(GroupId, u32),
    /// Room events of a group asked for by id.
    RoomEvents(GroupId),
    Signal,
    /// Log entries that carry this device's leave of the group (see `GroupSlot::leaving`).
    Leave(GroupId),
    Addresses,
    /// This device's presence, under the number it went with (see `presence::PresenceWays`).
    Presence(u64),
    /// A refusal sent to a device that showed an invite (see `GroupSlot::refusing`).
    Refusal(GroupId),
    Direct,
    /// About two sessions of a member's or friend's device.
    OtherSession(DevicePublicKey, sessions::SessionCheck),
    KeyProblem,
}

/// Member requests to one peer: how many are on their way, and those waiting their turn.
#[derive(Default)]
struct Outbox {
    in_flight: usize,
    waiting: VecDeque<(MemberRequest, Pending)>,
    /// A direct chat's messages waiting for the ones before them: they go one request at a
    /// time, since requests on their way together can arrive in any order.
    direct: VecDeque<MemberRequest>,
}

/// A peer that proved membership of at least one group, and the best way it is reached.
#[derive(Clone, Debug)]
struct Member {
    device: DevicePublicKey,
    path: Path,
    address: Multiaddr,
    /// The path last told (`MemberConnected`), and since when a worse one is the best: on a
    /// network with several ways between two devices (local IPv4, global IPv6) connections
    /// come and go at once, and the path told would flicker between them.
    told: Path,
    worse_since: Option<Instant>,
}

#[derive(Clone, Debug)]
struct ConnectionInfo {
    peer: PeerId,
    path: Path,
    address: Multiaddr,
}

/// A room event kept to name in presences and to give to members that lack it.
struct RecentEvent {
    room: fogata_state::RoomId,
    id: [u8; 16],
    at: Instant,
    /// The event itself while it fits among the latest kept (see room.rs); past that, only
    /// its id is kept, to name.
    wire: Option<WireMessage>,
}

/// A long log on its way to a device let in: the parts still to send, the number of the
/// first of them, how many there are in all, and how many times the first was tried.
struct Admitting {
    parts: VecDeque<Vec<serde_bytes::ByteBuf>>,
    next: u32,
    count: u32,
    tries: u8,
}

/// Parts of a long log received: how many there are, and those here by number.
#[derive(Default)]
struct PartsIn {
    count: u32,
    bytes: usize,
    parts: BTreeMap<u32, Vec<serde_bytes::ByteBuf>>,
}

/// One group the node runs.
struct GroupSlot {
    ctx: GroupContext,
    /// The voice rooms heard of so far, each with its own people, chat and files.
    rooms: BTreeMap<VoiceRoomId, RoomTracker<WireMessage>>,
    /// This device, and when the node started running the group: its own joins from
    /// before then are an earlier session's.
    me: DevicePublicKey,
    since_ms: u64,
    /// Peers that proved membership of this group.
    members: HashSet<PeerId>,
    /// Challenges sent for this group and waiting for a proof.
    challenges: HashMap<PeerId, [u8; 32]>,
    /// Joining devices we offered to accept, by the ticket they showed and the heads we
    /// gave them to sign on.
    join_offers: HashMap<PeerId, ([u8; 16], Vec<fogata_state::EntryHash>)>,
    /// Joins with an invite waiting for a manager of this device to let them in, by device.
    join_waiting: HashMap<DevicePublicKey, WaitingJoin>,
    /// Members we already asked to accept our invite.
    join_sent: HashSet<PeerId>,
    /// Managers we sent our signed join to: only they may let this device in.
    join_committed: HashSet<PeerId>,
    /// Devices let in that are being sent a long log, part after part.
    admitting: HashMap<PeerId, Admitting>,
    /// Refusals on their way to devices that showed an invite: the `Revoke` that goes along,
    /// and how many times it was tried.
    refusing: HashMap<PeerId, (Option<Vec<u8>>, u8)>,
    /// Parts of the log a manager that let this device in sent so far, by their number,
    /// with how many there are and their bytes.
    parts_in: HashMap<PeerId, PartsIn>,
    /// Peers that asked us to prove membership with a key someone outside the group may
    /// hold, and got nothing: they are asked to prove theirs first, and once they have,
    /// to ask again.
    held_back: HashSet<PeerId>,
    /// Removed devices already sent the log up to their removal.
    told_removed: HashSet<DevicePublicKey>,
    /// Members asked for the room's events, and when.
    room_asked: HashMap<PeerId, Instant>,
    /// What the room said may have been refused while the application was far behind
    /// (see events.rs): it is asked for whole again, of one member at a time, and these
    /// are the members asked so far (see `room_heartbeat`).
    room_again: Option<HashSet<PeerId>>,
    /// Members whose version keeps group banners (they said so in a roster): the only ones
    /// sent them.
    banner_peers: HashSet<PeerId>,
    /// The banner (its digest) each member refused, as a version that takes only smaller
    /// banners does: it is not sent the same one again while it stays connected.
    banner_refused: HashMap<PeerId, [u8; 32]>,
    /// Members whose version keeps group picture records (they said so in a roster): the
    /// only ones sent them.
    picture_peers: HashSet<PeerId>,
    /// The picture record (its digest) each member refused: it is not sent the same one
    /// again while it stays connected.
    picture_refused: HashMap<PeerId, [u8; 32]>,
    /// The members each member said it is directly connected to (its roster), and when.
    rosters: HashMap<PeerId, (HashSet<PeerId>, Instant)>,
    /// Members that refused this device's last ask for their roster, not having counted it
    /// in yet; and members that asked for this device's roster, so that count it in. Each
    /// side asks the other right after counting it in, so one of the two asks may come too
    /// early: it is made again once the other has asked (see `members::roster_refused`).
    roster_refused: HashSet<PeerId>,
    roster_heard: HashSet<PeerId>,
    /// Each voice room's latest events here (joins, leaves, chat, files), oldest first: the
    /// room they belong to, their id, when they came and the event as sent (see
    /// `room::RECENT`).
    recent: BTreeMap<VoiceRoomId, VecDeque<RecentEvent>>,
    /// Room events asked for by id, and when (see `room::ASK_IDS_AGAIN`).
    asked_ids: HashMap<[u8; 16], Instant>,
    /// When a full list of recent events, all of them missing here, last made this device
    /// ask for the whole room again.
    recent_resync: Option<Instant>,
    last_lookup: Option<Instant>,
    /// Lookups in a row that found no announcement (see `lookup_interval`).
    quiet_lookups: u32,
    /// Until when a member that went is looked up at every slot (see [`SEARCH_HARD`]).
    searching_until: Option<Instant>,
    /// A direct call's group: never offered to friends as a group.
    call: bool,
    /// A direct chat's group: reached only over the friend connection, never announced,
    /// looked up or offered as a group, and without rooms.
    direct: bool,
    /// Heartbeats in a row during which the current key was known to be held by someone who
    /// is no longer a member, waiting for the member expected to rotate.
    leak_heartbeats: u32,
    /// Managers told that this device's box of the current key does not open: the epoch
    /// told, and when.
    key_told: HashMap<PeerId, (u32, Instant)>,
    /// Invitations from this device (the entries that made them) said to be declined, so
    /// each is said once.
    declines_told: HashSet<fogata_state::EntryHash>,
    /// This device's `Leave`, while no member is known to have it: a group this device
    /// left runs on until one has (see `members::leave_taken`), since a member that has not
    /// counted the device in yet refuses it, and one away gets nothing.
    leaving: Option<fogata_state::EntryHash>,
}

impl GroupSlot {
    /// `peer` is no longer a manager this device asked to let it in: what it sent of the
    /// log goes with it.
    fn stop_join_with(&mut self, peer: &PeerId) {
        self.join_committed.remove(peer);
        self.parts_in.remove(peer);
    }

    /// The challenge to `peer` is answered or lost; one sent since stays pending.
    fn forget_challenge(&mut self, peer: &PeerId, challenge: &[u8; 32]) {
        if self.challenges.get(peer) == Some(challenge) {
            self.challenges.remove(peer);
        }
    }

    fn new(identity: Arc<DeviceIdentity>, setup: GroupSetup) -> Self {
        let me = identity.public_key();
        let ctx = GroupContext::new(identity, setup);
        Self {
            leaving: ctx.my_leave().filter(|_| ctx.member_count() > 0),
            ctx,
            rooms: BTreeMap::new(),
            me,
            since_ms: now_ms(),
            members: HashSet::new(),
            challenges: HashMap::new(),
            join_offers: HashMap::new(),
            join_waiting: HashMap::new(),
            join_sent: HashSet::new(),
            join_committed: HashSet::new(),
            admitting: HashMap::new(),
            refusing: HashMap::new(),
            parts_in: HashMap::new(),
            held_back: HashSet::new(),
            told_removed: HashSet::new(),
            room_asked: HashMap::new(),
            room_again: None,
            banner_peers: HashSet::new(),
            banner_refused: HashMap::new(),
            picture_peers: HashSet::new(),
            picture_refused: HashMap::new(),
            rosters: HashMap::new(),
            roster_refused: HashSet::new(),
            roster_heard: HashSet::new(),
            recent: BTreeMap::new(),
            asked_ids: HashMap::new(),
            recent_resync: None,
            last_lookup: None,
            quiet_lookups: 0,
            searching_until: None,
            call: false,
            direct: false,
            leak_heartbeats: 0,
            key_told: HashMap::new(),
            declines_told: HashSet::new(),
        }
    }

    /// Keeps a join for a manager's answer. Returns whether it is new, or `None` when too
    /// many already wait (see [`MAX_WAITING_PER_INVITE`]); a device already waiting is
    /// updated.
    fn wait_for_answer(&mut self, device: DevicePublicKey, join: WaitingJoin) -> Option<bool> {
        if !self.join_waiting.contains_key(&device) {
            let same_invite = self
                .join_waiting
                .values()
                .filter(|w| w.ticket.ticket_id == join.ticket.ticket_id)
                .count();
            if same_invite >= MAX_WAITING_PER_INVITE || self.join_waiting.len() >= MAX_WAITING_JOINS
            {
                return None;
            }
        }
        Some(self.join_waiting.insert(device, join).is_none())
    }

    /// A voice room's tracker, made the first time the room is heard of.
    fn tracker(&mut self, voice: VoiceRoomId) -> &mut RoomTracker<WireMessage> {
        let (me, since) = (self.me, self.since_ms);
        self.rooms.entry(voice).or_insert_with(|| {
            RoomTracker::new(me)
                .since(since)
                .weighing(crate::group::room_record_weight)
        })
    }

    /// The voice room this device is in, if any.
    fn joined(&self) -> Option<(VoiceRoomId, &RoomTracker<WireMessage>)> {
        self.rooms
            .iter()
            .find(|(_, tracker)| tracker.joined())
            .map(|(voice, tracker)| (*voice, tracker))
    }

    /// How often to look the group up. With nobody connected it starts at every half
    /// minute and slows down while the lookups find nobody new, up to every four minutes; a
    /// device that comes online looks up at once and finds this one's announcement anyway.
    /// With every other member connected there is nobody to find; one reached only through
    /// another member (`relayed` of them) counts as missing, since the punch slots need its
    /// addresses, and it is lost with that member.
    fn lookup_interval(&self, relayed: usize) -> Duration {
        if self.searching_until.is_some_and(|t| Instant::now() < t) {
            PUNCH_SLOT
        } else if self.ctx.joining().is_some() {
            LOOKUP_IDLE
        } else if self.ctx.member_count() + relayed <= self.members.len() + 1 {
            LOOKUP_NOBODY_MISSING
        } else if self.members.is_empty() {
            // The first few at the usual pace (just started, the others may be starting
            // too), then less and less often while nobody is there.
            (LOOKUP_IDLE * 2u32.pow(self.quiet_lookups.saturating_sub(3).min(3))).min(LOOKUP_QUIET)
        } else {
            LOOKUP_CONNECTED
        }
    }
}

struct Node {
    swarm: Swarm<StrangerDials<Behaviour>>,
    identity: Arc<DeviceIdentity>,
    /// The time the last message this device sealed carries (see [`Node::seal_time`]).
    last_sealed_ms: u64,
    groups: BTreeMap<GroupId, GroupSlot>,
    friends: Option<FriendContext>,
    /// Fogata devices found on the local network.
    lan_seen: HashSet<PeerId>,
    /// The dials mDNS findings started, with their addresses, for tests to see.
    #[cfg(test)]
    mdns_dials: Vec<(PeerId, Vec<Multiaddr>)>,
    /// How many times identify pushed to each peer, for tests to see.
    #[cfg(test)]
    pushed: HashMap<PeerId, usize>,
    /// The dials mDNS findings started that have not ended, by peer (see
    /// [`Node::dial_mdns_peer`]). Each peer leaves once connected or once they all fail.
    mdns_dialling: HashMap<PeerId, MdnsDials>,
    /// Peers mDNS found whose dials all failed, to dial again (see `MDNS_RETRIES` in
    /// mdns_dials.rs).
    mdns_retry: HashMap<PeerId, MdnsRetry>,
    /// Members this node already tried to reach over QUIC because their only connection
    /// could not carry datagrams (media needs them).
    quic_tried: HashSet<PeerId>,
    /// When the local addresses each member sent were last dialled anew.
    lan_redialled: HashMap<PeerId, Instant>,
    /// Addresses peers said they listen on (identify), for reaching members over QUIC.
    peer_listening: HashMap<PeerId, Vec<Multiaddr>>,
    /// QUIC addresses members told us they have on their local networks.
    member_lan: HashMap<PeerId, Vec<Multiaddr>>,
    /// A profile and presence a peer sent while this device was still checking its proof;
    /// they count once it has proved itself.
    early: HashMap<PeerId, early::Early>,
    dns_note: Option<String>,
    /// Peers that proved membership of any group.
    verified: VerifiedPeers,
    /// Those of a group other than a direct chat's: the relay serves only them.
    relay_peers: VerifiedPeers,
    /// Peers that had a relay slot here or a way through this device's relay, while they
    /// stay connected.
    relay_users: HashSet<PeerId>,
    /// Peers disconnected for that, to dial again then, and where.
    relay_redial: HashMap<PeerId, (Instant, Vec<Multiaddr>)>,
    /// The people of a call that ended here that the relay no longer serves, and when they
    /// lose what it gave them (see `group_changes::CALL_GRACE`).
    after_call: HashMap<PeerId, Instant>,
    /// The same per group, for stream protocols.
    group_members: GroupMembers,
    /// The device each of those proved to be.
    member_devices: crate::MemberDevices,
    /// The Fogata protocols connected peers said they take.
    advertised: Advertised,
    /// The highest capability level each friend and member was seen at.
    levels: Levels,
    /// Peers whose connections are kept open while idle; every other peer is a stranger.
    wanted: Wanted,
    /// Peers identify lists Fogata's protocols to besides the wanted ones (see behaviour.rs).
    shown: ShownPeers,
    /// Peers shown them lately, told again (see `show_protocols`).
    push_again: HashMap<PeerId, Instant>,
    bridge: bool,
    /// Appear offline (see [`NodeConfig::hidden`]).
    hidden: bool,
    /// Nobody used this computer for a while (see [`Node::shown_away`]).
    away: bool,
    /// Peers that proved membership of at least one group, keyed by transport identity.
    members: HashMap<PeerId, Member>,
    connections: HashMap<ConnectionId, ConnectionInfo>,
    pending: HashMap<OutboundRequestId, (PeerId, Pending)>,
    /// Member requests to each peer, sent a few at a time (see `outbox::IN_FLIGHT`).
    outbox: HashMap<PeerId, Outbox>,
    /// This device's presence on its way to each member, one at a time to each.
    presence: presence::PresenceWays,
    /// Peers just removed from a group, disconnected once what is on its way to them (the
    /// log that tells them) has gone.
    farewell: HashSet<PeerId>,
    /// Two live sessions of one device (see sessions.rs).
    sessions: sessions::Sessions,
    /// Direct calls' steps to and from the sessions of a friend.
    call_routes: call_sessions::CallRoutes,
    /// Typing indicators (see typing.rs).
    typing: typing::Typing,
    /// Peers that may be members: they announce one of our groups' rendezvous keys, were
    /// found on the LAN, were listed by a member or came through a member's relay.
    interesting: HashSet<PeerId>,
    /// Groups a peer may belong to, when we know which; others are tried with every group.
    hinted: HashMap<PeerId, HashSet<GroupId>>,
    /// DHT keys looked up for each group.
    group_keys: HashMap<RecordKey, GroupId>,
    /// Members found on the DHT, with every address learnt for them.
    candidates: HashMap<PeerId, HashSet<Multiaddr>>,
    /// Failed scheduled attempts per candidate, when its rest started, and how many rests
    /// it has had. Announcements of ended sessions stay on the DHT for up to 48 hours, so
    /// candidates that keep failing are rested for a while and then forgotten.
    failures: HashMap<PeerId, (u32, Option<Instant>, u32)>,
    explicit_dials: HashSet<ConnectionId>,
    /// Peers dialled at an address given or remembered that have not answered yet, with
    /// when: a connection to one later, by whatever dial, counts as this device's choice
    /// too, rather than the first dial's failure losing it.
    explicit_peers: HashMap<PeerId, Instant>,
    /// Where friends and members were reached, and which device each was, newest first:
    /// dialled at start and whenever the network changes (see `dial_remembered`).
    remembered: Vec<(Multiaddr, DevicePublicKey)>,
    /// The addresses friends and members were connected from in this run (not through a
    /// relay), over TCP too, and which device each was, newest first: where a member on a
    /// network that blocks UDP, never reached over QUIC and so never remembered, comes back
    /// from (see `publish_known_addresses`).
    came_from: VecDeque<(std::net::IpAddr, DevicePublicKey)>,
    /// Addresses friends and members were reached at this run, told once each.
    reached: HashSet<Multiaddr>,
    /// Dials made in a punch slot; a connection from one of them crossed both NATs.
    punch_dials: HashSet<ConnectionId>,
    /// Peers we are currently dialling as part of a scheduled punch.
    punching: HashMap<PeerId, Instant>,
    /// Members we asked for a relay slot, by the listener that holds it.
    reservations: HashMap<libp2p::core::transport::ListenerId, PeerId>,
    /// Our own addresses as reported by others, and who reported them.
    /// Addresses others reported for this device, with each reporter and the address it
    /// reported from.
    observed: HashMap<Multiaddr, HashMap<PeerId, Option<std::net::IpAddr>>>,
    confirmed: HashSet<Multiaddr>,
    /// When each confirmed address was last reported: one nobody reports any more (a new
    /// public IP from the provider, another network) is dropped.
    confirmed_seen: HashMap<Multiaddr, Instant>,
    listen_addrs: Vec<Multiaddr>,
    /// Asked for this device's addresses before it listened on any network (see
    /// [`Node::answer_addresses`]).
    asked_addresses: Vec<tokio::sync::oneshot::Sender<Vec<Multiaddr>>>,
    /// Keys that open the local addresses friends and members announce (see lan.rs).
    lan_keys: LanKeys,
    /// This device's own sealed local addresses, announced on the DHT only.
    lan_names: LanNames,
    port_change_reported: bool,
    /// Whether UDP gets out of this network (see udp_check.rs).
    udp: udp_check::UdpCheck,
    /// Open TCP connections this node dialled itself to public addresses, with the address
    /// dialled: the peers the UDP check may ask over QUIC.
    dialled_tcp: HashMap<ConnectionId, (IpAddr, Instant)>,
    /// When a peer at a public address last connected or answered a ping, on this network.
    public_contact: Option<Instant>,
    /// Dials to public addresses that failed since then.
    failed_public_dials: usize,
    seen: HashSet<[u8; 16]>,
    seen_order: VecDeque<[u8; 16]>,
    /// Keys we announce under, and when we last did.
    provided: (Vec<RecordKey>, Option<Instant>),
    /// When to announce again after the DHT did not take an announcement, and how many
    /// failed in a row.
    announce_retry: (Option<Instant>, u32),
    /// An announcement went through since starting or since the last network change.
    announced: bool,
    /// Candidates already dialled because of the current DHT lookup. A lookup reports the
    /// same provider once per DHT node that answers; one attempt per lookup is enough.
    lookup_dials: HashSet<PeerId>,
    /// Candidates whose dial from a lookup has not connected or failed yet: they do not make
    /// room for later ones of the lookup (see [`Node::admit_candidate`]). Each leaves at
    /// the dial's end.
    dialling_found: HashSet<PeerId>,
    /// What each candidate found on the DHT was looked up for, and when it was found.
    candidate_found: HashMap<PeerId, (Budget, Instant)>,
    /// New candidates taken from the answers to each lookup running (see
    /// `MAX_NEW_PER_LOOKUP` in candidates.rs), in the order found.
    lookup_new: HashMap<RecordKey, Vec<PeerId>>,
    /// How many each lookup running took in all (see `MAX_TAKEN_PER_LOOKUP` in
    /// candidates.rs).
    lookup_taken: HashMap<RecordKey, usize>,
    /// The peers the answers to each lookup running named, the earliest first (see
    /// `MAX_NAMED_PER_LOOKUP` in candidates.rs).
    lookup_named: HashMap<RecordKey, VecDeque<PeerId>>,
    /// The query of each lookup running: answers to an earlier lookup of the same key, which
    /// may still come, name nobody a second time for this one.
    lookup_query: HashMap<RecordKey, kad::QueryId>,
    /// The query whose answer is being handled.
    answering: Option<kad::QueryId>,
    /// Candidates two answers to one lookup named: they are announced on more than the one
    /// node naming them, and one node on the way to a key cannot make that up.
    corroborated: HashSet<PeerId>,
    /// Peers that had a connection in this run: a candidate among them answered once, so
    /// it is the last to make room for ones that never did.
    reached_peers: HashSet<PeerId>,
    slot_count: u64,
    started: Instant,
    #[cfg(debug_assertions)]
    test_panic: Option<TestPanic>,
    /// Wall-clock time of the last slot, to notice sleeps and clock changes.
    last_slot_ms: Option<u64>,
    /// When the DHT was last bootstrapped.
    last_bootstrap: Instant,
    /// Bootstrap peers given by a DNS name (`/dnsaddr`).
    named_bootstrap: HashSet<PeerId>,
    /// When the last network change was acted on.
    last_network_change: Option<Instant>,
    /// A network change that came too soon after the last one, acted on at the next slot:
    /// whether a network was left in it.
    network_pending: Option<bool>,
    /// Pings missed in a row, by connection.
    ping_misses: HashMap<ConnectionId, u8>,
    /// When each peer was last dialled through a member.
    through_dials: HashMap<PeerId, Instant>,
    /// Members whose direct connections hear nothing any more, and when the other members
    /// were last asked for a way to them.
    silent: HashMap<PeerId, Instant>,
    /// Connections this device is closing; they no longer count as a member's way.
    closing: HashSet<ConnectionId>,
    /// Members reached directly again, and since when: their connections through other
    /// members close a moment later.
    redundant: HashMap<PeerId, Instant>,
    /// Members on the local network whose last QUIC connection closed while they were
    /// members, dialled again for a while (see `redial_lost`).
    lost: HashMap<PeerId, member_reach::Lost>,
    /// What each peer may still make this device do (see limits.rs).
    limits: limits::Limits,
    /// Takings out of a voice room acted on (see room.rs).
    room_removes: room::Removes,
    /// Logs taken whole, judged away from this loop (see judging.rs).
    judging: judging::Judging,
    events: crate::events::EventSender,
    /// The peer whose request or answer is being handled: a panic then is its doing.
    serving: Option<PeerId>,
    /// A DHT answer or a local network announcement is being handled: a panic then is the
    /// doing of whoever sent it, which it does not say (see `blamed` in run.rs).
    serving_answer: Option<Answer>,
    /// The swarm is being polled: a panic then came from within the network library, while
    /// it handled what someone sent or kept its own state, with nobody to name.
    polling_swarm: bool,
    /// Makes the next poll of the swarm panic.
    #[cfg(test)]
    panic_polling: bool,
    /// Lookups whose answers are no longer handled (see [`Node::blame_answer`]).
    dropped_lookups: VecDeque<kad::QueryId>,
    /// Addresses whose local network announcements are not heard until then.
    muted_tellers: HashMap<std::net::IpAddr, Instant>,
    /// Times the node started afresh after internal errors (see [`Node::start_afresh`]).
    afresh: u32,
    commands: mpsc::Receiver<Command>,
    /// Dropped with the node, which ends the loops taking its streams ([`crate::Stopped`]).
    _stopping: tokio::sync::watch::Sender<()>,
    /// The tasks that are this node's (see [`crate::Tasks`]).
    tasks: crate::Tasks,
    /// A rendezvous point where this node meets another computer, to move an account.
    seeking: Option<transfer::Seeking>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Node {
    /// The time for the next message this device seals: now, and always later than the one
    /// before. A room orders what one device says by these times, and takes no join that is
    /// not later than that device's last leave: a leave and a join sealed in the same
    /// millisecond would leave this device out of the room it went back into.
    fn seal_time(&mut self) -> u64 {
        self.last_sealed_ms = next_seal_time(self.last_sealed_ms, now_ms());
        self.last_sealed_ms
    }

    fn emit(&self, event: NodeEvent) {
        match &event {
            NodeEvent::MemberConnected {
                device,
                peer,
                path,
                address,
                ..
            } => {
                tracing::info!(%peer, %path, %address, "member {} connected", device.fingerprint())
            }
            NodeEvent::MemberDisconnected { device, peer, .. } => {
                tracing::info!(%peer, "member {} disconnected", device.fingerprint())
            }
            NodeEvent::Room { change, .. } => tracing::debug!(?change, "room"),
            NodeEvent::FriendStatus {
                device, in_room, ..
            } => tracing::debug!(in_room, "status of friend {}", device.fingerprint()),
            NodeEvent::ExternalAddress(address) => tracing::info!(%address, "address confirmed"),
            NodeEvent::Reached { address, device } => {
                tracing::debug!(%address, "{} reached here", device.fingerprint())
            }
            NodeEvent::PortChangesPerDestination => {
                tracing::info!("the NAT changes ports per destination")
            }
            NodeEvent::Info(text) => tracing::info!("{text}"),
            NodeEvent::OtherSession | NodeEvent::Resync | NodeEvent::SignalsLost { .. } => {}
            NodeEvent::GroupChanged(_) => tracing::debug!("group log changed"),
            NodeEvent::Joined { group, .. } => tracing::info!("joined the group {group:?}"),
            NodeEvent::JoinAsked { device, .. } => {
                tracing::info!("{} asks to join a group", device.fingerprint())
            }
            NodeEvent::JoinAnswered { .. } => tracing::debug!("a join was answered"),
            NodeEvent::JoinWaiting { .. } => tracing::info!("waiting for a manager to let us in"),
            NodeEvent::JoinDeclined { .. } => tracing::info!("a manager said no to joining"),
            NodeEvent::JoinExpired { .. } => {
                tracing::info!("the invite ran out before a manager let this device in")
            }
            NodeEvent::Removed { .. } => tracing::info!("this device left a group"),
            NodeEvent::MemberProfile { profile } => {
                tracing::debug!("profile of member {}", profile.device.fingerprint())
            }
            NodeEvent::Animation { animation } => tracing::debug!(
                "animation of {} ({} bytes)",
                animation.device.fingerprint(),
                animation.picture.len()
            ),
            NodeEvent::Signal { from, payload, .. } => {
                tracing::debug!(
                    bytes = payload.len(),
                    "call signal from {}",
                    from.fingerprint()
                )
            }
            NodeEvent::FriendRequest { profile } => {
                tracing::info!("friend request from {}", profile.device.fingerprint())
            }
            NodeEvent::FriendRequestSent { device, .. } => {
                tracing::info!("friend request delivered to {}", device.fingerprint())
            }
            NodeEvent::FriendRequestFailed(reason) => {
                tracing::info!("friend request failed: {reason}")
            }
            NodeEvent::FriendAdded { profile } => {
                tracing::info!("new friend {}", profile.device.fingerprint())
            }
            NodeEvent::FriendOnline {
                device,
                peer,
                path,
                hidden,
            } => {
                tracing::info!(%peer, %path, hidden, "friend {} online", device.fingerprint())
            }
            NodeEvent::MemberPresence {
                device,
                hidden,
                away,
            } => {
                tracing::debug!(hidden, away, "presence of member {}", device.fingerprint())
            }
            NodeEvent::Call { from, message } => {
                tracing::info!(signal = ?message.signal, "call step from {}", from.fingerprint())
            }
            NodeEvent::CallUndelivered { to, .. } => {
                tracing::info!("a call step did not reach {}", to.fingerprint())
            }
            NodeEvent::FriendOffline { device } => {
                tracing::info!("friend {} offline", device.fingerprint())
            }
            NodeEvent::FriendsChanged => tracing::debug!("friends changed"),
            NodeEvent::LevelSeen { device, level } => {
                tracing::debug!(level, "{} speaks a newer level", device.fingerprint())
            }
            NodeEvent::GroupOffered { by, .. } => {
                tracing::info!("{} offered a group", by.fingerprint())
            }
            NodeEvent::InvitationDeclined { device, .. } => {
                tracing::info!("{} declined an invitation from here", device.fingerprint())
            }
            NodeEvent::DirectOffered { by, .. } => {
                tracing::debug!("{} offered a direct chat's group", by.fingerprint())
            }
            NodeEvent::Typing { from, typing, .. } => {
                tracing::debug!(typing, "typing signal from {}", from.fingerprint())
            }
            NodeEvent::Direct { from, payloads, .. } => {
                tracing::debug!(
                    count = payloads.len(),
                    "direct messages from {}",
                    from.fingerprint()
                )
            }
            NodeEvent::TakenOut { voice, by, .. } => tracing::info!(
                "left voice room {} at {}",
                hex::encode(voice),
                by.map(|b| b.fingerprint())
                    .unwrap_or_else(|| "a room rule".into())
            ),
            NodeEvent::Listening(_) => {}
        }
        self.events.send(event);
    }

    fn info(&self, text: impl Into<String>) {
        self.emit(NodeEvent::Info(text.into()));
    }

    fn local_peer(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// Records a message id; returns false if it was already known.
    fn remember(&mut self, id: [u8; 16]) -> bool {
        if !self.seen.insert(id) {
            return false;
        }
        self.seen_order.push_back(id);
        if self.seen_order.len() > SEEN_LIMIT
            && let Some(old) = self.seen_order.pop_front()
        {
            self.seen.remove(&old);
        }
        true
    }

    /// Whether `peer` proved membership of a group other than a direct chat's: only those
    /// have rooms and media, and get relay slots, local addresses and room events.
    pub(super) fn in_regular_group(&self, peer: &PeerId) -> bool {
        self.groups
            .values()
            .any(|s| !s.direct && s.members.contains(peer))
    }

    /// Copies the per-group member sets to the views the relay and the stream protocols use.
    fn publish_members(&self) {
        *crate::locked(&self.verified) = self.members.keys().copied().collect();
        *crate::locked(&self.relay_peers) = self
            .groups
            .values()
            .filter(|s| !s.direct)
            .flat_map(|s| s.members.iter().copied())
            .collect();
        *crate::locked(&self.member_devices) =
            self.members.iter().map(|(p, m)| (*p, m.device)).collect();
        *crate::locked(&self.group_members) = self
            .groups
            .iter()
            .map(|(id, slot)| (*id, slot.members.clone()))
            .collect();
    }

    /// Handles `request` as one from a member of every group here (device 1 of the fuzzing
    /// runs) or from a stranger, both connected over a local address.
    #[cfg(feature = "fuzzing")]
    fn fuzz_request(&mut self, request: MemberRequest, from_member: bool) {
        let key = libp2p::identity::Keypair::ed25519_from_bytes([7; 32]).expect("32 bytes");
        let member = key.public().to_peer_id();
        let stranger = libp2p::identity::Keypair::ed25519_from_bytes([8; 32])
            .expect("32 bytes")
            .public()
            .to_peer_id();
        if !self.members.contains_key(&member) {
            let address: Multiaddr = "/ip4/192.168.1.30/udp/4001/quic-v1".parse().expect("valid");
            self.connections.insert(
                ConnectionId::new_unchecked(1),
                ConnectionInfo {
                    peer: member,
                    path: Path::Lan,
                    address: address.clone(),
                },
            );
            self.members.insert(
                member,
                Member {
                    device: crate::fuzzing::device(1).public_key(),
                    path: Path::Lan,
                    address: address.clone(),
                    told: Path::Lan,
                    worse_since: None,
                },
            );
            for slot in self.groups.values_mut() {
                slot.members.insert(member);
            }
            self.publish_members();
        }
        let peer = if from_member { member } else { stranger };
        self.serving = Some(peer);
        let _ = self.on_request(peer, request);
        self.serving = None;
    }
}

/// The peers of the stateful fuzzing runs, by number, each connected over a local address
/// (made once, under connection ids of their own).
#[cfg(feature = "fuzzing")]
impl Node {
    fn fuzz_peer(&mut self, n: u8) -> PeerId {
        let peer = libp2p::identity::Keypair::ed25519_from_bytes([20 + (n & 3); 32])
            .expect("32 bytes")
            .public()
            .to_peer_id();
        let id = ConnectionId::new_unchecked(20 + usize::from(n & 3));
        self.connections
            .entry(id)
            .or_insert_with(|| ConnectionInfo {
                peer,
                path: Path::Lan,
                address: format!("/ip4/192.168.1.{}/udp/4001/quic-v1", 40 + (n & 3))
                    .parse()
                    .expect("valid"),
            });
        peer
    }

    /// Runs `steps` against a group being joined with an invite, made anew from `setup`:
    /// requests, and answers to this device's own asks, each from peer 0 (a stranger) or
    /// peers 1 to 3 (managers it sent its join to). What it holds of logs in parts must stay
    /// within its bounds after every step.
    fn fuzz_join(&mut self, setup: GroupSetup, steps: Vec<(u8, crate::fuzzing::JoinStep)>) {
        use crate::fuzzing::JoinStep;
        let group = match &setup {
            GroupSetup::Joining { group_id, .. } => *group_id,
            GroupSetup::Member(log) => log.group_id(),
        };
        self.groups
            .insert(group, GroupSlot::new(self.identity.clone(), setup));
        let peers: Vec<PeerId> = (0..4).map(|n| self.fuzz_peer(n)).collect();
        if let Some(slot) = self.groups.get_mut(&group) {
            slot.join_committed.extend(&peers[1..]);
            slot.join_sent.extend(&peers[1..]);
        }
        for (from, step) in steps {
            let peer = peers[usize::from(from & 3)];
            self.serving = Some(peer);
            match step {
                JoinStep::Request(request) => {
                    let _ = self.on_request(peer, request);
                }
                JoinStep::Offered(answer) => self.on_response(peer, Pending::Join(group), answer),
                JoinStep::Committed(answer) => {
                    self.on_response(peer, Pending::JoinCommit(group), answer);
                }
            }
            self.serving = None;
            let held: Vec<usize> = self
                .groups
                .values()
                .flat_map(|s| s.parts_in.values())
                .map(|p| p.parts.values().flatten().map(|e| e.len()).sum())
                .collect();
            assert!(held.iter().all(|b| *b <= joins::MAX_ADMITTED_BYTES));
            assert!(held.iter().sum::<usize>() <= joins::MAX_ADMITTED_BYTES_ALL);
        }
    }

    /// Runs friend requests against `book`, taken anew: from peer 0 (a stranger), peers 1
    /// and 3 (two sessions of the book's first friend) and peer 2 (its second friend), each
    /// friend recognised on its connection. What offers in parts hold must stay within
    /// their bound after every request.
    fn fuzz_friend(
        &mut self,
        book: fogata_friends::FriendBook,
        steps: Vec<(u8, fogata_proto::FriendRequest)>,
    ) {
        let friends: Vec<DevicePublicKey> = book.friends.keys().copied().collect();
        let peers: Vec<PeerId> = (0..4).map(|n| self.fuzz_peer(n)).collect();
        let mut context = FriendContext::new(book);
        for (n, peer) in peers.iter().enumerate().skip(1) {
            let friend = if n == 2 {
                friends.get(1)
            } else {
                friends.first()
            };
            if let Some(device) = friend {
                context.online.insert(*peer, *device);
            }
        }
        self.friends = Some(context);
        for (from, request) in steps {
            let peer = peers[usize::from(from & 3)];
            self.serving = Some(peer);
            let _ = self.on_friend_request(peer, request);
            self.serving = None;
            if let Some(f) = self.friends.as_ref() {
                assert!(f.offered_bytes() <= friend_offers::MAX_OFFERED_WAITING);
            }
        }
    }
}

fn is_loopback(address: &Multiaddr) -> bool {
    addr::ip(address).is_some_and(|ip| ip.is_loopback() || ip.is_unspecified())
}

#[cfg(test)]
mod tests {
    use fogata_state::{Body, Entry, GroupLog};
    use libp2p::core::ConnectedPoint;

    use super::*;

    /// A node with one group of its own, nothing else.
    pub(super) fn lone_member() -> (NodeHandle, GroupId) {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let group = log.group_id();
        let (node, _events) = start(NodeConfig {
            identity: ana,
            groups: vec![GroupSetup::Member(log)],
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        (node, group)
    }

    /// A member request whose answer nothing waits for.
    pub(super) fn empty_push(group: GroupId) -> MemberRequest {
        MemberRequest::Push {
            group_id: group.as_bytes().to_vec(),
            entries: Vec::new(),
        }
    }

    /// Runs `inspect` on the node and returns what it gives.
    pub(super) async fn inspect<T: Send + 'static>(
        node: &NodeHandle,
        inspect: impl FnOnce(&mut Node) -> T + Send + 'static,
    ) -> T {
        let (tx, rx) = tokio::sync::oneshot::channel();
        node.commands
            .send(Command::Inspect(Box::new(move |n| {
                let n = n.downcast_mut::<Node>().expect("the node");
                let _ = tx.send(inspect(n));
            })))
            .await
            .unwrap();
        rx.await.unwrap()
    }

    /// Connects `peer` to the node as coming in through `relay`'s slot.
    pub(super) fn through_relay(n: &mut Node, peer: PeerId, relay: PeerId, id: usize) {
        let address: Multiaddr =
            format!("/ip4/203.0.113.9/udp/4001/quic-v1/p2p/{relay}/p2p-circuit/p2p/{peer}")
                .parse()
                .unwrap();
        n.on_connection_established(
            peer,
            ConnectionId::new_unchecked(id),
            &ConnectedPoint::Dialer {
                address,
                role_override: libp2p::core::Endpoint::Dialer,
                port_use: libp2p::core::transport::PortUse::Reuse,
            },
        );
    }

    /// What is kept of a member can be copied and shown in logs, as before it was moved
    /// beside the mDNS dials.
    #[test]
    fn a_member_can_be_copied_and_shown() {
        fn shown<T: Clone + std::fmt::Debug>() {}
        shown::<Member>();
    }

    /// A member for the tests below, at a made-up address, counted in `group`.
    pub(super) fn add_member(n: &mut Node, group: GroupId, peer: PeerId) {
        n.members.insert(
            peer,
            Member {
                device: DeviceIdentity::generate().unwrap().public_key(),
                path: Path::Direct,
                address: "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap(),
                told: Path::Direct,
                worse_since: None,
            },
        );
        n.groups.get_mut(&group).unwrap().members.insert(peer);
    }

    #[test]
    fn an_invite_lets_only_a_few_devices_wait_for_an_answer_at_once() {
        let admin = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&admin, "Friends", 1_000).unwrap();
        let group = log.group_id();
        let mut slot = GroupSlot::new(admin.clone(), GroupSetup::Member(log.clone()));
        let ask = |ticket: &fogata_state::Ticket| {
            let joiner = DeviceIdentity::generate().unwrap();
            let entry = Entry::sign(
                &joiner,
                group,
                log.heads(),
                2_000,
                Body::Join {
                    ticket: ticket.clone(),
                    name: "Leo".into(),
                },
            );
            let join = WaitingJoin {
                peer: PeerId::random(),
                entry,
                ticket: ticket.clone(),
            };
            (joiner.public_key(), join)
        };
        let ticket = log.issue_ticket(&admin, 10_000).unwrap();
        let mut first = None;
        for _ in 0..MAX_WAITING_PER_INVITE {
            let (device, join) = ask(&ticket);
            first.get_or_insert(device);
            assert_eq!(slot.wait_for_answer(device, join), Some(true));
        }
        let (device, join) = ask(&ticket);
        assert_eq!(slot.wait_for_answer(device, join), None);
        // One already waiting may show its join again.
        let (_, again) = ask(&ticket);
        assert_eq!(slot.wait_for_answer(first.unwrap(), again), Some(false));
        // Other invites have their own turns, up to the group's bound.
        while slot.join_waiting.len() < MAX_WAITING_JOINS {
            let other = log.issue_ticket(&admin, 10_000).unwrap();
            let (device, join) = ask(&other);
            assert_eq!(slot.wait_for_answer(device, join), Some(true));
        }
        let other = log.issue_ticket(&admin, 10_000).unwrap();
        let (device, join) = ask(&other);
        assert_eq!(slot.wait_for_answer(device, join), None);
    }
}

/// The time to seal a message at, `now` or, when that is not later, one millisecond after
/// the `last` one sealed.
fn next_seal_time(last: u64, now: u64) -> u64 {
    now.max(last.saturating_add(1))
}

#[cfg(test)]
mod seal_time_tests {
    use super::next_seal_time;

    #[test]
    fn messages_sealed_at_once_or_with_the_clock_set_back_go_in_order() {
        assert_eq!(next_seal_time(0, 1_000), 1_000);
        assert_eq!(next_seal_time(1_000, 1_000), 1_001);
        assert_eq!(next_seal_time(1_001, 1_000), 1_002);
        assert_eq!(next_seal_time(1_002, 5_000), 5_000);
    }
}
