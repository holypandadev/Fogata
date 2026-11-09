//! Networking for Fogata: finding group members, getting through NATs and exchanging
//! sealed group messages over libp2p.
//!
//! The node never uses servers or relays run by strangers. Members find each other through
//! opaque provider records on the public IPFS DHT, then connect directly, hole punch on a
//! shared schedule, or go through another member who is connected to both.

#![forbid(unsafe_code)]

mod addr;
mod allowance;
mod behaviour;
pub mod caps;
mod codec;
mod dns;
mod events;
pub mod files;
#[cfg(feature = "fuzzing")]
pub mod fuzzing;
mod group;
mod lab;
mod lan;
mod node;
mod rendezvous;
mod strangers;
pub mod updates;

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_friends::{CodeSecrets, FriendBook, FriendCode, FriendLink, Profile};
pub use fogata_state::{
    Barrier, ChatLine, FIRST_ROOM, FileInfo, KeptOut, RoomChange, RoomFile, VoiceRoomId,
};
use fogata_state::{Entry, EntryHash, GroupLog};
pub use libp2p::{Multiaddr, PeerId, Stream, StreamProtocol, multiaddr};
pub use libp2p_stream::{Control as StreamControl, IncomingStreams, OpenStreamError};

/// Unreliable QUIC datagrams on direct connections, for real-time media.
pub use libp2p::quic::datagram;
use tokio::sync::mpsc;

pub use crate::{
    events::{Backlog, NodeEvents},
    group::GroupSetup,
    lab::{
        lab_is_sealed, lab_lookup_keys, lab_open_sealed, run_lab_dht, run_lab_flood, run_lab_liar,
        run_lab_mdns_flood, run_lab_silent,
    },
    node::typing::{TYPING_PROTOCOL, TypingSignal},
    rendezvous::AMINO_BOOTSTRAP,
};

/// Settings for a running node.
pub struct NodeConfig {
    /// Long-term identity of this device.
    pub identity: Arc<DeviceIdentity>,
    /// The groups: their logs, or invites to join them. Empty for a node that only talks to
    /// friends.
    pub groups: Vec<GroupSetup>,
    /// Profile and friends. Without them the node neither finds friends nor can be found by
    /// them.
    pub friends: Option<FriendBook>,
    /// UDP/TCP port to listen on; 0 picks a free port.
    pub listen_port: u16,
    /// DHT bootstrap peers. Empty means the public Amino bootstrap peers.
    pub bootstrap: Vec<Multiaddr>,
    /// Use the public DHT to find members.
    pub use_dht: bool,
    /// Use mDNS to find members on the local network.
    pub use_mdns: bool,
    /// Forward traffic between other members when they cannot connect directly. When
    /// false, this device still helps them coordinate a hole punch but carries no data.
    pub bridge: bool,
    /// Addresses to dial directly, for example from an invite.
    pub peers: Vec<Multiaddr>,
    /// Where friends and members were reached before ([`NodeEvent::Reached`]), and which
    /// device each was: dialled at start and whenever the network changes, while that device
    /// is still a friend or a member, and a local address only on that same network.
    pub remembered: Vec<(Multiaddr, DevicePublicKey)>,
    /// Appear offline: friends and members are asked to show this device as offline. It
    /// still connects, and it shows in the rooms it is in.
    pub hidden: bool,
}

impl fmt::Debug for NodeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeConfig")
            .field("device", &self.identity.public_key())
            .field("groups", &self.groups.len())
            .field("listen_port", &self.listen_port)
            .field("use_dht", &self.use_dht)
            .field("use_mdns", &self.use_mdns)
            .field("bridge", &self.bridge)
            .finish_non_exhaustive()
    }
}

/// How a member was reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Path {
    /// Same local network.
    Lan,
    /// One side was reachable from the internet.
    Direct,
    /// Both sides opened their NAT for the other.
    HolePunched,
    /// Through another member's relay slot: `relay` is that member's peer, and `member` its
    /// device when it is a member of the group the path is told to (only members of a group
    /// learn who carries its members' traffic).
    ThroughMember {
        relay: PeerId,
        member: Option<DevicePublicKey>,
    },
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Path::Lan => f.write_str("local network"),
            Path::Direct => f.write_str("direct"),
            Path::HolePunched => f.write_str("hole punched"),
            Path::ThroughMember { relay, .. } => {
                write!(f, "through member {}", short_peer(relay))
            }
        }
    }
}

/// Something the user interface may want to show.
#[derive(Clone, Debug)]
pub enum NodeEvent {
    Listening(Multiaddr),
    /// An address of ours that at least two different peers observed.
    ExternalAddress(Multiaddr),
    /// A friend or member was reached at this address, the first time this run: worth
    /// remembering, and passing in [`NodeConfig::remembered`] at the next start.
    Reached {
        address: Multiaddr,
        device: DevicePublicKey,
    },
    /// The local NAT gave different public ports to different peers.
    PortChangesPerDestination,
    /// A member of `group` proved its membership, or is now reached by a better path.
    MemberConnected {
        group: GroupId,
        device: DevicePublicKey,
        peer: PeerId,
        path: Path,
        address: Multiaddr,
    },
    MemberDisconnected {
        group: GroupId,
        device: DevicePublicKey,
        peer: PeerId,
    },
    /// Something happened in one of a group's voice rooms.
    Room {
        group: GroupId,
        voice: VoiceRoomId,
        change: RoomChange,
    },
    /// This device left voice room `voice` of `group`, which it was in: a manager (`by`)
    /// took it out of the room (no `barrier`), or it is kept out of it now (`barrier`, with
    /// the manager who did it for a time out).
    TakenOut {
        group: GroupId,
        voice: VoiceRoomId,
        by: Option<DevicePublicKey>,
        barrier: Option<Barrier>,
    },
    /// A group log changed; save it ([`NodeHandle::group`]).
    GroupChanged(GroupId),
    /// A device showed a valid invite to this manager and waits for a person to let it in
    /// ([`NodeHandle::answer_join`]).
    JoinAsked {
        group: GroupId,
        device: DevicePublicKey,
        name: String,
    },
    /// A device waiting to join is not waiting here any more: a manager answered, here or
    /// elsewhere, or its invite stopped working.
    JoinAnswered {
        group: GroupId,
        device: DevicePublicKey,
    },
    /// This device showed its invite and waits for a manager to let it in.
    JoinWaiting {
        group: GroupId,
    },
    /// The manager who wrote this device's invite said no (signed), or confirmed another
    /// manager's no.
    JoinDeclined {
        group: GroupId,
    },
    /// This device's invite ran out before a manager let it in. The node still holds the
    /// group until it is dropped ([`NodeHandle::drop_group`]).
    JoinExpired {
        group: GroupId,
    },
    /// An invite was accepted: this device is now a member of the group called `name`.
    Joined {
        group: GroupId,
        name: String,
    },
    /// This device is not a member of `group` any more (it left or a manager removed it). The
    /// node stops running the group; `log` is its final log.
    Removed {
        group: GroupId,
        log: GroupLog,
    },
    /// A connected member's profile.
    MemberProfile {
        profile: Profile,
    },
    /// A friend's or member's animation of its picture or banner, signed by it.
    Animation {
        animation: fogata_friends::Animation,
    },
    /// A member of `group` sent this device a call signal (see [`NodeHandle::signal`]).
    Signal {
        group: GroupId,
        from: DevicePublicKey,
        payload: Vec<u8>,
    },
    /// Someone asked to be friends; answer with [`NodeHandle::answer_friend`].
    FriendRequest {
        profile: Profile,
    },
    /// The friend request made here reached the other device, which has not answered yet.
    FriendRequestSent {
        device: DevicePublicKey,
        name: String,
    },
    /// The friend request made here cannot go through.
    FriendRequestFailed(String),
    /// A device became a friend: this device accepted its request, or it accepted ours.
    FriendAdded {
        profile: Profile,
    },
    FriendOnline {
        device: DevicePublicKey,
        peer: PeerId,
        path: Path,
        /// The friend appears offline.
        hidden: bool,
    },
    FriendOffline {
        device: DevicePublicKey,
    },
    /// Whether an online friend is in a room. `group` is set only when this device is a
    /// member of that room's group.
    FriendStatus {
        device: DevicePublicKey,
        in_room: bool,
        group: Option<GroupId>,
        /// The friend appears offline; then it says nothing about rooms.
        hidden: bool,
        /// Nobody used the friend's computer for a while; never while it is in a room or
        /// appears offline.
        away: bool,
    },
    /// Whether a connected member appears offline, or is away from its computer.
    MemberPresence {
        device: DevicePublicKey,
        hidden: bool,
        away: bool,
    },
    /// A step of a direct call from a friend.
    Call {
        from: DevicePublicKey,
        message: fogata_friends::CallMessage,
    },
    /// A step of a direct call did not reach the friend; `refused` when every session of
    /// the friend refused it, as an older device does with a step it cannot read.
    CallUndelivered {
        to: DevicePublicKey,
        call: [u8; 16],
        refused: bool,
    },
    /// Profile or friends changed; save them ([`NodeHandle::friends`]).
    FriendsChanged,
    /// A friend added this device to a group it did not know and sent its log. The node
    /// does not run it until the application adds it ([`NodeHandle::add_group`]), so the
    /// person can accept or decline first.
    GroupOffered {
        log: GroupLog,
        by: DevicePublicKey,
    },
    /// `device` declined an invitation to `group` that this device made: it was added here,
    /// or let in with an invite this device wrote, and handed back the `Leave` it signed
    /// ([`NodeHandle::give_entries`]). Said once per invitation while the node runs.
    InvitationDeclined {
        group: GroupId,
        device: DevicePublicKey,
    },
    /// A friend sent the log of a two-person group for a direct chat between the two, one
    /// the node does not run. The application decides whether the chat goes over it
    /// ([`NodeHandle::add_direct`]).
    DirectOffered {
        log: GroupLog,
        by: DevicePublicKey,
    },
    /// Messages of a direct chat from its other person, opened and checked: the payloads
    /// as they were sealed.
    Direct {
        group: GroupId,
        from: DevicePublicKey,
        payloads: Vec<Vec<u8>>,
    },
    /// Someone in a chat this device is in is typing there now, or stopped (`typing`
    /// false): in a direct chat's group, or in the voice room `voice` of a group's room.
    /// Never kept; the application shows it for a few seconds after the last one.
    Typing {
        group: GroupId,
        voice: Option<VoiceRoomId>,
        from: DevicePublicKey,
        typing: bool,
    },
    /// This device's identity is open on another computer at the same time: a session of
    /// it proved itself here, or a member or friend saw two at once. Said at most once in
    /// ten minutes.
    OtherSession,
    /// A friend or member was seen speaking a higher capability level than known before
    /// ([`caps`]): worth keeping, and passing to [`NodeHandle::remember_levels`] at the next
    /// start.
    LevelSeen {
        device: DevicePublicKey,
        level: u32,
    },
    /// Events were dropped on their way to the application, which fell far behind (see
    /// events.rs): what it knows of the groups, their members connected and the friends
    /// may be out of date, and is to be read anew. It comes after every event kept.
    Resync,
    /// Call signals `from` sent in `group` were dropped on their way to the application
    /// (see events.rs): its call with `from` is to agree anew. It comes after every signal
    /// of `from` kept, and none of `from` comes after it until it is received.
    SignalsLost {
        group: GroupId,
        from: DevicePublicKey,
    },
    /// Progress information (DHT, hole punching, relay slots).
    Info(String),
}

/// Says that the node stopped: the node holds the only sender, so it is dropped with the
/// node, whatever stopped it. Waiting on it keeps nothing running, as a sender of the
/// node's commands would.
pub(crate) type Stopped = tokio::sync::watch::Receiver<()>;

/// The tasks of one node besides its own: the loops taking its streams, and what answers
/// each stream or sends on one. They are the node's: once it stops, whatever stopped it,
/// they are aborted and waited for before its shutdown returns, so none outlives it. One
/// spawned after that is dropped unrun.
#[derive(Clone)]
pub(crate) struct Tasks(Arc<Mutex<Option<tokio::task::JoinSet<()>>>>);

impl Tasks {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(Some(tokio::task::JoinSet::new()))))
    }

    /// Runs `task` as the node's, if it still runs.
    pub(crate) fn spawn(&self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        let mut tasks = locked(&self.0);
        let Some(tasks) = tasks.as_mut() else {
            return;
        };
        // What is over is let go of here, so that the set holds only what runs.
        while tasks.try_join_next().is_some() {}
        tasks.spawn(task);
    }

    /// Aborts every task and waits up to `wait` for them to end.
    pub(crate) async fn stop(&self, wait: std::time::Duration) {
        let Some(mut tasks) = locked(&self.0).take() else {
            return;
        };
        tasks.abort_all();
        let _ =
            tokio::time::timeout(wait, async { while tasks.join_next().await.is_some() {} }).await;
    }

    /// How many run now.
    fn running(&self) -> usize {
        let mut tasks = locked(&self.0);
        tasks.as_mut().map_or(0, |tasks| {
            while tasks.try_join_next().is_some() {}
            tasks.len()
        })
    }
}

/// See [`NodeHandle::next_stream`].
pub(crate) async fn next_until_stopped(
    stopped: &Stopped,
    incoming: &mut libp2p_stream::IncomingStreams,
) -> Option<(PeerId, libp2p::Stream)> {
    use futures::StreamExt as _;
    let mut stopped = stopped.clone();
    tokio::select! {
        next = incoming.next() => next,
        // Nothing is ever sent: it ends only when the sender is dropped.
        _ = stopped.changed() => None,
    }
}

/// Errors when starting a node.

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("network setup failed: {0}")]
    Setup(String),
    #[error("the node has stopped")]
    Stopped,
    #[error("{0}")]
    Refused(String),
}

/// What the node has learnt about the network it is on, for a diagnosis.
#[derive(Clone, Debug, Default)]
pub struct NetworkReport {
    /// Addresses the node listens on (local network ones included).
    pub listening: Vec<Multiaddr>,
    /// Public addresses other peers saw for this device, and how many saw each.
    pub observed: Vec<(Multiaddr, usize)>,
    /// Observed addresses at least two peers agree on.
    pub confirmed: Vec<Multiaddr>,
    /// The NAT gave different public ports to different peers.
    pub port_changes: bool,
    /// Peers in the DHT routing table.
    pub dht_peers: usize,
    /// Open connections over QUIC (UDP) and over TCP.
    pub quic_connections: usize,
    pub tcp_connections: usize,
    /// Other Fogata devices found on the local network.
    pub lan_devices: usize,
    /// A note about the system DNS settings, if they could not be read as a whole.
    pub dns_note: Option<String>,
    /// Whether UDP gets out of this network, as far as the node could tell.
    pub udp: UdpSeen,
    /// How long ago a peer at a public address last connected or answered a ping, on this
    /// network; `None` when none did since starting or since the network changed.
    pub contact: Option<std::time::Duration>,
    /// Dials to public addresses that failed since that contact (or since this network).
    pub failed_dials: usize,
    /// The node has been on this network for a while ([`SETTLED_AFTER`]): until then, that
    /// nothing answered yet means nothing.
    pub settled: bool,
}

/// How long after starting, or after the network changed, the node's view of the network
/// counts: by then it has met DHT nodes when there is an internet to meet them on.
pub const SETTLED_AFTER: std::time::Duration = std::time::Duration::from_secs(30);

/// Whether UDP gets out of the network this device is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UdpSeen {
    /// Not known yet: nothing proved it either way.
    #[default]
    Unknown,
    /// A QUIC connection with a public address opened.
    Works,
    /// Several peers alive over TCP did not answer over QUIC, and no QUIC connection opened.
    Blocked,
}

/// What a group shows, without a copy of its log: the state the node worked out, shared.
#[derive(Clone, Debug)]
pub struct GroupSummary {
    pub group_id: GroupId,
    pub state: std::sync::Arc<fogata_state::GroupState>,
    /// The picture to show, if any (see [`GroupLog::picture_in`]).
    pub picture: Option<Vec<u8>>,
    /// The digest of the banner to show, if any.
    pub banner: Option<[u8; 16]>,
}

impl GroupSummary {
    pub fn of(log: &GroupLog) -> Self {
        let state = log.shared_state();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        Self {
            group_id: log.group_id(),
            picture: log.picture_in(&state, now_ms).map(<[u8]>::to_vec),
            banner: log.banner().map(fogata_state::GroupBanner::digest),
            state,
        }
    }
}

/// A group's room as this node sees it.
#[derive(Clone, Debug)]
pub struct RoomView {
    pub started_ms: u64,
    /// Participants with their nicknames, and whether they seem away.
    pub participants: Vec<(DevicePublicKey, String, bool)>,
    /// Whether this device is in the room.
    pub joined: bool,
    /// Files shared in the room, with who has them.
    pub files: Vec<RoomFile>,
    /// The chat so far, in the order it was said.
    pub chat: Vec<ChatLine>,
}

enum Command {
    AddGroup(GroupSetup),
    MergeGroup(GroupLog),
    Dial {
        addrs: Vec<Multiaddr>,
        group: Option<GroupId>,
    },
    SetProfile {
        name: Option<String>,
        avatar: Option<fogata_friends::Avatar>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    Groups(tokio::sync::oneshot::Sender<Vec<GroupSummary>>),
    Group(GroupId, tokio::sync::oneshot::Sender<Option<GroupLog>>),
    Members(
        GroupId,
        tokio::sync::oneshot::Sender<Vec<(DevicePublicKey, Path)>>,
    ),
    JoinRoom {
        group: GroupId,
        voice: VoiceRoomId,
        nick: String,
    },
    LeaveRoom(GroupId, tokio::sync::oneshot::Sender<()>),
    AnswerJoin {
        group: GroupId,
        device: DevicePublicKey,
        accept: bool,
    },
    RoomChat {
        group: GroupId,
        text: String,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    Room(
        GroupId,
        VoiceRoomId,
        tokio::sync::oneshot::Sender<Option<RoomView>>,
    ),
    Rooms(
        GroupId,
        tokio::sync::oneshot::Sender<Vec<(VoiceRoomId, RoomView)>>,
    ),
    ShareFile {
        group: GroupId,
        file: FileInfo,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    UnshareFile {
        group: GroupId,
        id: [u8; 32],
    },
    RemoveFromRoom {
        group: GroupId,
        voice: VoiceRoomId,
        member: DevicePublicKey,
    },
    Report(tokio::sync::oneshot::Sender<NetworkReport>),
    MemberPeer(
        DevicePublicKey,
        tokio::sync::oneshot::Sender<Option<PeerId>>,
    ),
    Addresses(tokio::sync::oneshot::Sender<Vec<Multiaddr>>),
    MemberPeers(tokio::sync::oneshot::Sender<Vec<PeerId>>),
    Signal {
        group: GroupId,
        to: DevicePublicKey,
        payload: Vec<u8>,
    },
    Friends(tokio::sync::oneshot::Sender<Option<FriendBook>>),
    FriendsOnline(tokio::sync::oneshot::Sender<Vec<(DevicePublicKey, Path)>>),
    CreateLink(tokio::sync::oneshot::Sender<Result<FriendLink, String>>),
    ShowCode(CodeSecrets),
    UseLink(FriendLink),
    UseCode(CodeSecrets),
    AskMember {
        device: DevicePublicKey,
        name: String,
    },
    AnswerFriend {
        device: DevicePublicKey,
        accept: bool,
    },
    RemoveFriend(DevicePublicKey),
    SetHidden(bool),
    SetAway(bool, tokio::sync::oneshot::Sender<bool>),
    RetryNow,
    Seek(Option<[u8; 32]>, Option<[u8; 32]>),
    MoveCandidates(tokio::sync::oneshot::Sender<Vec<PeerId>>),
    PeerPath(PeerId, tokio::sync::oneshot::Sender<Option<Path>>),
    Trusted(
        PeerId,
        tokio::sync::oneshot::Sender<Option<DevicePublicKey>>,
    ),
    AddCall(GroupLog),
    AddDirect(GroupLog),
    SendDirect {
        group: GroupId,
        to: DevicePublicKey,
        payloads: Vec<Vec<u8>>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    DropGroup(GroupId),
    GiveEntries {
        to: Vec<DevicePublicKey>,
        entries: Vec<Vec<u8>>,
        declined: Option<EntryHash>,
    },
    Typing {
        group: GroupId,
        typing: bool,
    },
    SendCall {
        to: DevicePublicKey,
        message: fogata_friends::CallMessage,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetBanner {
        color: u32,
        picture: Option<Vec<u8>>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetBio {
        text: String,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetGroupBanner {
        group: GroupId,
        picture: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetGroupPicture {
        group: GroupId,
        picture: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    SetAnimation {
        of: fogata_friends::Animated,
        picture: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    MutualFriends {
        device: DevicePublicKey,
        reply: tokio::sync::oneshot::Sender<Result<Vec<DevicePublicKey>, String>>,
    },
    Shutdown(tokio::sync::oneshot::Sender<()>),
    /// Makes the node panic, to check that it goes on.
    #[cfg(test)]
    Panic,
    /// Makes the node panic while it holds the lock of the members verified, after it put
    /// the peer among them.
    #[cfg(test)]
    PanicHolding(PeerId),
    /// Makes the node panic as if handling a request of the peer had.
    #[cfg(test)]
    PanicServing(PeerId),
    /// Makes the node panic as if handling a DHT answer or local network announcement whose
    /// sender is not known had.
    #[cfg(test)]
    PanicAnswer,
    /// Makes the node panic within the next poll of the swarm.
    #[cfg(test)]
    PanicPolling,
    /// Makes the node panic as if handling an answer to that lookup had.
    #[cfg(test)]
    PanicLookup(libp2p::kad::QueryId),
    /// Makes the node panic as if handling a local network announcement from that address had.
    #[cfg(test)]
    PanicTold(std::net::IpAddr),
    /// Sends a member request as it is, for tests of what a misbehaving member can do.
    #[cfg(test)]
    Request(PeerId, Box<fogata_proto::MemberRequest>),
    /// Runs a closure on the node in its own task, for tests of how it keeps its state.
    #[cfg(test)]
    Inspect(Inspection),
    /// Handles a member request as if from a verified member (or a stranger), for fuzzing
    /// what a peer can send against the node's real state.
    #[cfg(feature = "fuzzing")]
    Fuzz {
        request: Box<fogata_proto::MemberRequest>,
        from_member: bool,
        reply: tokio::sync::oneshot::Sender<()>,
    },
    /// Runs steps from a few peers against a group being joined with an invite, made anew
    /// from `setup` first (see `fuzzing::node_join`).
    #[cfg(feature = "fuzzing")]
    FuzzJoin {
        setup: Box<group::GroupSetup>,
        steps: Vec<(u8, fuzzing::JoinStep)>,
        reply: tokio::sync::oneshot::Sender<()>,
    },
    /// Runs friend requests from a few peers against `book`, taken anew first (see
    /// `fuzzing::node_friend`).
    #[cfg(feature = "fuzzing")]
    FuzzFriend {
        book: Box<fogata_friends::FriendBook>,
        steps: Vec<(u8, fogata_proto::FriendRequest)>,
        reply: tokio::sync::oneshot::Sender<()>,
    },
}

/// What [`Command::Inspect`] runs, given the node.
#[cfg(test)]
type Inspection = Box<dyn FnOnce(&mut dyn std::any::Any) + Send>;

/// Locks `mutex` even when a panic left it poisoned. What the node shares behind locks
/// (peers verified, files served, peers wanted) is changed one whole step at a time, so it
/// stays right; failing closed instead would leave the node going on but unable to say yes
/// to any member, after it lived through a panic.
pub(crate) fn locked<T: ?Sized>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Handle to a running node. The node stops when every handle has been dropped, except
/// those made with [`NodeHandle::background`].
#[derive(Clone)]
pub struct NodeHandle {
    commands: Commands,
    stopped: Stopped,
    tasks: Tasks,
    local_peer: PeerId,
    streams: libp2p_stream::Control,
    verified: behaviour::VerifiedPeers,
    group_members: GroupMembers,
    member_devices: MemberDevices,
    advertised: Advertised,
    levels: Levels,
}

/// How a handle reaches the node: a strong sender keeps it running.
#[derive(Clone)]
enum Commands {
    Keeps(mpsc::Sender<Command>),
    Background(mpsc::WeakSender<Command>),
}

impl Commands {
    async fn send(&self, command: Command) -> Result<(), NetError> {
        let sender = match self {
            Self::Keeps(sender) => sender.clone(),
            Self::Background(weak) => weak.upgrade().ok_or(NetError::Stopped)?,
        };
        sender.send(command).await.map_err(|_| NetError::Stopped)
    }
}

/// Peers that proved membership, per group. Shared with the handle for stream protocols.
pub(crate) type GroupMembers =
    Arc<std::sync::Mutex<std::collections::HashMap<GroupId, std::collections::HashSet<PeerId>>>>;

/// The device each peer that proved membership proved to be. Shared with the handle.
pub(crate) type MemberDevices =
    Arc<std::sync::Mutex<std::collections::HashMap<PeerId, DevicePublicKey>>>;

/// The highest capability level each friend and member was seen at ([`caps`]). Shared with
/// the handle.
pub(crate) type Levels = Arc<std::sync::Mutex<std::collections::HashMap<DevicePublicKey, u32>>>;

/// The Fogata protocols each connected peer said it takes (identify), so a newer feature is
/// used only with peers that know it. Shared with the handle.
pub(crate) type Advertised = Arc<
    std::sync::Mutex<std::collections::HashMap<PeerId, std::collections::HashSet<StreamProtocol>>>,
>;

impl fmt::Debug for NodeHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeHandle")
            .field("local_peer", &self.local_peer)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Command::AddGroup(_) => "AddGroup",
            Command::MergeGroup(_) => "MergeGroup",
            Command::Dial { .. } => "Dial",
            Command::SetProfile { .. } => "SetProfile",
            Command::Groups(_) => "Groups",
            Command::Group(..) => "Group",
            Command::Members(..) => "Members",
            Command::JoinRoom { .. } => "JoinRoom",
            Command::LeaveRoom(..) => "LeaveRoom",
            Command::AnswerJoin { .. } => "AnswerJoin",
            Command::RoomChat { .. } => "RoomChat",
            Command::Room(..) => "Room",
            Command::Rooms(..) => "Rooms",
            Command::ShareFile { .. } => "ShareFile",
            Command::UnshareFile { .. } => "UnshareFile",
            Command::RemoveFromRoom { .. } => "RemoveFromRoom",
            Command::Report(_) => "Report",
            Command::MemberPeer(..) => "MemberPeer",
            Command::Addresses(_) => "Addresses",
            Command::MemberPeers(_) => "MemberPeers",
            Command::Signal { .. } => "Signal",
            Command::Friends(_) => "Friends",
            Command::FriendsOnline(_) => "FriendsOnline",
            Command::CreateLink(_) => "CreateLink",
            Command::ShowCode(_) => "ShowCode",
            Command::UseLink(_) => "UseLink",
            Command::UseCode(_) => "UseCode",
            Command::AnswerFriend { .. } => "AnswerFriend",
            Command::AskMember { .. } => "AskMember",
            Command::RemoveFriend(_) => "RemoveFriend",
            Command::SetHidden(_) => "SetHidden",
            Command::SetAway(..) => "SetAway",
            Command::RetryNow => "RetryNow",
            Command::Seek(..) => "Seek",
            Command::MoveCandidates(_) => "MoveCandidates",
            Command::PeerPath(..) => "PeerPath",
            Command::Trusted(..) => "Trusted",
            Command::AddCall(_) => "AddCall",
            Command::AddDirect(_) => "AddDirect",
            Command::SendDirect { .. } => "SendDirect",
            Command::DropGroup(_) => "DropGroup",
            Command::GiveEntries { .. } => "GiveEntries",
            Command::Typing { .. } => "Typing",
            Command::SendCall { .. } => "SendCall",
            Command::SetBanner { .. } => "SetBanner",
            Command::SetBio { .. } => "SetBio",
            Command::SetGroupBanner { .. } => "SetGroupBanner",
            Command::SetGroupPicture { .. } => "SetGroupPicture",
            Command::SetAnimation { .. } => "SetAnimation",
            Command::MutualFriends { .. } => "MutualFriends",
            Command::Shutdown(_) => "Shutdown",
            #[cfg(test)]
            Command::Panic => "Panic",
            #[cfg(test)]
            Command::PanicHolding(_) => "PanicHolding",
            #[cfg(test)]
            Command::PanicServing(_) => "PanicServing",
            #[cfg(test)]
            Command::PanicAnswer => "PanicAnswer",
            #[cfg(test)]
            Command::PanicPolling => "PanicPolling",
            #[cfg(test)]
            Command::PanicLookup(_) => "PanicLookup",
            #[cfg(test)]
            Command::PanicTold(_) => "PanicTold",
            #[cfg(test)]
            Command::Request(..) => "Request",
            #[cfg(test)]
            Command::Inspect(_) => "Inspect",
            #[cfg(feature = "fuzzing")]
            Command::Fuzz { .. } => "Fuzz",
            #[cfg(feature = "fuzzing")]
            Command::FuzzJoin { .. } => "FuzzJoin",
            #[cfg(feature = "fuzzing")]
            Command::FuzzFriend { .. } => "FuzzFriend",
        };
        f.write_str(name)
    }
}

impl NodeHandle {
    /// Starts running a group: a new one, one joined elsewhere, or an invite to use.
    pub async fn add_group(&self, setup: GroupSetup) -> Result<(), NetError> {
        self.tell(Command::AddGroup(setup)).await
    }

    /// Adds entries made on this device (for example a manager's change) to a group the node
    /// runs, and sends them to the members.
    pub async fn merge_group(&self, log: GroupLog) -> Result<(), NetError> {
        self.tell(Command::MergeGroup(log)).await
    }

    /// Chooses a group's banner (a PNG, JPEG, GIF or WebP; empty to take it away), as an
    /// manager, and sends it to the members connected now that take banners; the others get
    /// it when they next compare banners.
    pub async fn set_group_banner(&self, group: GroupId, picture: Vec<u8>) -> Result<(), NetError> {
        self.ask(|reply| Command::SetGroupBanner {
            group,
            picture,
            reply,
        })
        .await?
        .map_err(NetError::Refused)
    }

    /// Chooses a group's picture (a PNG or JPEG; empty to take it away), as a manager, in a
    /// record apart from the log, and sends it to the members connected now that take such
    /// records; the others get it when they next compare pictures.
    pub async fn set_group_picture(
        &self,
        group: GroupId,
        picture: Vec<u8>,
    ) -> Result<(), NetError> {
        self.ask(|reply| Command::SetGroupPicture {
            group,
            picture,
            reply,
        })
        .await?
        .map_err(NetError::Refused)
    }

    /// Dials addresses, for example those in an invite. With `group`, the peers there are
    /// only asked about that group.
    pub async fn dial(
        &self,
        addrs: Vec<Multiaddr>,
        group: Option<GroupId>,
    ) -> Result<(), NetError> {
        self.tell(Command::Dial { addrs, group }).await
    }

    /// Changes this device's name or avatar and sends it to the friends and members
    /// connected now; the others get it when they connect.
    pub async fn set_profile(
        &self,
        name: Option<String>,
        avatar: Option<fogata_friends::Avatar>,
    ) -> Result<(), NetError> {
        self.ask(|reply| Command::SetProfile {
            name,
            avatar,
            reply,
        })
        .await?
        .map_err(NetError::Refused)
    }

    /// What every group the node runs shows (see [`Self::group`] for a whole log).
    pub async fn groups(&self) -> Result<Vec<GroupSummary>, NetError> {
        self.ask(Command::Groups).await
    }

    /// A copy of a group's log as the node knows it now.
    pub async fn group(&self, group: GroupId) -> Result<Option<GroupLog>, NetError> {
        self.ask(|reply| Command::Group(group, reply)).await
    }

    /// Members of a group currently connected and how they were reached.
    pub async fn members(&self, group: GroupId) -> Result<Vec<(DevicePublicKey, Path)>, NetError> {
        self.ask(|reply| Command::Members(group, reply)).await
    }

    /// Enters a voice room of a group as `nick`, starting a room there if nobody is in it,
    /// and leaving another voice room of the group this device is in.
    pub async fn join_room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        nick: String,
    ) -> Result<(), NetError> {
        // Said as it shows, as the others keep it.
        let nick = fogata_crypto::names::tidy(&nick);
        self.tell(Command::JoinRoom { group, voice, nick }).await
    }

    /// Leaves the voice room of a group this device is in, on purpose; returns once the
    /// others were told.
    pub async fn leave_room(&self, group: GroupId) -> Result<(), NetError> {
        self.ask(|reply| Command::LeaveRoom(group, reply)).await
    }

    /// Lets a device waiting to join with an invite in, or says no, which also cancels the
    /// invite (managers).
    pub async fn answer_join(
        &self,
        group: GroupId,
        device: DevicePublicKey,
        accept: bool,
    ) -> Result<(), NetError> {
        self.tell(Command::AnswerJoin {
            group,
            device,
            accept,
        })
        .await
    }

    /// Appears offline or not: tells the friends and members connected now, and later ones
    /// when they connect.
    pub async fn set_hidden(&self, hidden: bool) -> Result<(), NetError> {
        self.tell(Command::SetHidden(hidden)).await
    }

    /// Whether nobody has used this computer for a while. Friends and members connected now,
    /// and later ones when they connect, see it as away, unless this device appears offline
    /// or is in a room (a call too), where someone may just be listening. Returns whether it
    /// shows as away now.
    pub async fn set_away(&self, away: bool) -> Result<bool, NetError> {
        self.ask(|reply| Command::SetAway(away, reply)).await
    }

    /// Tries every friend and member again at once, forgetting earlier failures: after
    /// something in the way changed, such as this computer's firewall now letting the port in.
    pub async fn retry_now(&self) -> Result<(), NetError> {
        self.tell(Command::RetryNow).await
    }

    /// Runs a direct call's group: the caller's two-person group, made for one call. It is
    /// not offered to friends as a group, and the other member is approached at once.
    pub async fn add_call(&self, log: GroupLog) -> Result<(), NetError> {
        self.tell(Command::AddCall(log)).await
    }

    /// Runs the two-person group of a direct chat, or merges this copy of its log into the
    /// one running. It is never announced on the DHT nor offered as a group: the friend gets
    /// its log over the friend connection and is approached there.
    pub async fn add_direct(&self, log: GroupLog) -> Result<(), NetError> {
        self.tell(Command::AddDirect(log)).await
    }

    /// Seals `payloads` with the key of a direct chat's group and sends them to `to`, its
    /// other person; fails when that one is not connected as a member of it now.
    pub async fn send_direct(
        &self,
        group: GroupId,
        to: DevicePublicKey,
        payloads: Vec<Vec<u8>>,
    ) -> Result<Result<(), String>, NetError> {
        self.ask(|reply| Command::SendDirect {
            group,
            to,
            payloads,
            reply,
        })
        .await
    }

    /// Hands signed entries of a group this device does not run (a declined invitation's
    /// `Leave`) to those of the friends `to` that are online now, as friends offer a group's
    /// log: a friend that runs the group takes them from a member like any other entry.
    /// `declined` names the entry that made the invitation the `Leave` declines, so the
    /// friend that made it can say so ([`NodeEvent::InvitationDeclined`]).
    pub async fn give_entries(
        &self,
        to: Vec<DevicePublicKey>,
        entries: Vec<Entry>,
        declined: Option<EntryHash>,
    ) -> Result<(), NetError> {
        let entries = entries.iter().map(Entry::encode).collect();
        self.tell(Command::GiveEntries {
            to,
            entries,
            declined,
        })
        .await
    }

    /// Manager: takes `member` out of voice room `voice` of `group` now; it may come back at
    /// once. Sent as a room event, which only the member acts on.
    pub async fn remove_from_room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        member: DevicePublicKey,
    ) -> Result<(), NetError> {
        self.tell(Command::RemoveFromRoom {
            group,
            voice,
            member,
        })
        .await
    }

    /// Stops running a group without leaving it, as when a call ends.
    pub async fn drop_group(&self, group: GroupId) -> Result<(), NetError> {
        self.tell(Command::DropGroup(group)).await
    }

    /// Sends a step of a direct call to a friend connected now; fails when it is not.
    pub async fn send_call(
        &self,
        to: DevicePublicKey,
        message: fogata_friends::CallMessage,
    ) -> Result<Result<(), String>, NetError> {
        self.ask(|reply| Command::SendCall { to, message, reply })
            .await
    }

    /// Changes this device's banner and sends it to the friends connected now; the others
    /// get it when they connect. Nothing happens if it is the banner it had.
    pub async fn set_banner(&self, color: u32, picture: Option<Vec<u8>>) -> Result<(), NetError> {
        self.ask(|reply| Command::SetBanner {
            color,
            picture,
            reply,
        })
        .await?
        .map_err(NetError::Refused)
    }

    /// Changes this device's bio and sends it to the friends connected now; the others get
    /// it when they connect. Nothing happens if it is the bio it had.
    pub async fn set_bio(&self, text: String) -> Result<(), NetError> {
        self.ask(|reply| Command::SetBio { text, reply })
            .await?
            .map_err(NetError::Refused)
    }

    /// Makes `picture` (a GIF or WebP, empty for none) move in place of this device's
    /// picture or banner, which must be the animation's first frame, and sends it to the
    /// friends and members connected now; the others get it when they connect. Set it again
    /// after the still picture changes: an animation shows only with the still picture it
    /// was made for.
    pub async fn set_animation(
        &self,
        of: fogata_friends::Animated,
        picture: Vec<u8>,
    ) -> Result<(), NetError> {
        self.ask(|reply| Command::SetAnimation { of, picture, reply })
            .await?
            .map_err(NetError::Refused)
    }

    /// Asks a friend connected now which friends the two have in common, without either
    /// showing its list; fails when it is not connected.
    pub async fn mutual_friends(
        &self,
        device: DevicePublicKey,
    ) -> Result<Vec<DevicePublicKey>, NetError> {
        self.ask(|reply| Command::MutualFriends { device, reply })
            .await?
            .map_err(NetError::Refused)
    }

    /// Closes every connection and stops the node, so that friends and members see this
    /// device go at once instead of after the connections time out. For when the
    /// application quits: the node takes no commands after it.
    pub async fn shutdown(&self) -> Result<(), NetError> {
        self.ask(Command::Shutdown).await
    }

    /// Tells the people of a chat who are connected now that this device is typing there,
    /// or stopped: the other person of a direct chat's `group`, or those in the voice room
    /// of `group` this device is in. Nothing goes while it appears offline, and "typing"
    /// at most every two seconds for one chat.
    pub async fn typing(&self, group: GroupId, typing: bool) -> Result<(), NetError> {
        self.tell(Command::Typing { group, typing }).await
    }

    /// Says `text` in a group's room.
    pub async fn room_chat(&self, group: GroupId, text: String) -> Result<(), NetError> {
        self.ask(|reply| Command::RoomChat { group, text, reply })
            .await?
            .map_err(NetError::Refused)
    }

    /// A voice room of a group, if a room is open in it.
    pub async fn room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
    ) -> Result<Option<RoomView>, NetError> {
        self.ask(|reply| Command::Room(group, voice, reply)).await
    }

    /// The voice rooms of a group with a room open in them.
    pub async fn rooms(&self, group: GroupId) -> Result<Vec<(VoiceRoomId, RoomView)>, NetError> {
        self.ask(|reply| Command::Rooms(group, reply)).await
    }

    /// Lists a file in a group's room as one this device serves (see [`files::serve`]).
    pub async fn share_file(&self, group: GroupId, file: FileInfo) -> Result<(), NetError> {
        self.ask(|reply| Command::ShareFile { group, file, reply })
            .await?
            .map_err(NetError::Refused)
    }

    /// Says in a group's room that this device stops serving file `id`.
    pub async fn unshare_file(&self, group: GroupId, id: [u8; 32]) -> Result<(), NetError> {
        self.tell(Command::UnshareFile { group, id }).await
    }

    /// Sends a call signal to a connected member of `group`; it arrives as
    /// [`NodeEvent::Signal`]. Signals to members that are not connected are dropped.
    pub async fn signal(
        &self,
        group: GroupId,
        to: DevicePublicKey,
        payload: Vec<u8>,
    ) -> Result<(), NetError> {
        self.tell(Command::Signal { group, to, payload }).await
    }

    /// What the node has learnt about the network so far.
    pub async fn network_report(&self) -> Result<NetworkReport, NetError> {
        self.ask(Command::Report).await
    }

    /// Transport identity of a connected member of any group.
    pub async fn member_peer(&self, device: DevicePublicKey) -> Result<Option<PeerId>, NetError> {
        self.ask(|reply| Command::MemberPeer(device, reply)).await
    }

    /// Addresses at which this node can currently be reached, for invites.
    pub async fn addresses(&self) -> Result<Vec<Multiaddr>, NetError> {
        self.ask(Command::Addresses).await
    }

    /// Transport identities of the members of any group currently connected.
    pub async fn member_peers(&self) -> Result<Vec<PeerId>, NetError> {
        self.ask(Command::MemberPeers).await
    }

    async fn ask<T>(
        &self,
        command: impl FnOnce(tokio::sync::oneshot::Sender<T>) -> Command,
    ) -> Result<T, NetError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.commands.send(command(tx)).await?;
        rx.await.map_err(|_| NetError::Stopped)
    }

    async fn tell(&self, command: Command) -> Result<(), NetError> {
        self.commands.send(command).await
    }

    /// A copy of the profile and friends as the node knows them now.
    pub async fn friends(&self) -> Result<Option<FriendBook>, NetError> {
        self.ask(Command::Friends).await
    }

    /// Friends currently connected and how they were reached.
    pub async fn friends_online(&self) -> Result<Vec<(DevicePublicKey, Path)>, NetError> {
        self.ask(Command::FriendsOnline).await
    }

    /// Makes a friend invite link with this node's current addresses. It is valid for ten
    /// minutes and accepts one request.
    pub async fn create_friend_link(&self) -> Result<FriendLink, NetError> {
        self.ask(Command::CreateLink)
            .await?
            .map_err(NetError::Refused)
    }

    /// Makes a short friend code, valid for ten minutes and for one request.
    pub async fn create_friend_code(&self) -> Result<FriendCode, NetError> {
        let now = unix_secs();
        let code = FriendCode::generate(now).map_err(|e| NetError::Refused(e.to_string()))?;
        let secrets = stretch(code.clone(), now).await?;
        self.tell(Command::ShowCode(secrets)).await?;
        Ok(code)
    }

    /// Sends a friend request to the device that made `link`.
    pub async fn use_friend_link(&self, link: FriendLink) -> Result<(), NetError> {
        self.tell(Command::UseLink(link)).await
    }

    /// Sends a friend request to the device showing `code`.
    pub async fn use_friend_code(&self, code: FriendCode) -> Result<(), NetError> {
        let secrets = stretch(code, unix_secs()).await?;
        self.tell(Command::UseCode(secrets)).await
    }

    /// Sends a friend request to `device`, a member of a group this device is in, over the
    /// connection where it proved to be that member: no link or code needed. `name` is what
    /// the request is listed as until it is answered. It fails (a
    /// [`NodeEvent::FriendRequestFailed`]) when the member is not connected now or its copy
    /// is too old to take it; when that member asked first, this accepts its request.
    pub async fn ask_member_friend(
        &self,
        device: DevicePublicKey,
        name: String,
    ) -> Result<(), NetError> {
        self.tell(Command::AskMember { device, name }).await
    }

    /// Answers a friend request.
    pub async fn answer_friend(
        &self,
        device: DevicePublicKey,
        accept: bool,
    ) -> Result<(), NetError> {
        self.tell(Command::AnswerFriend { device, accept }).await
    }

    /// Forgets a friend, or a friend request in either direction.
    pub async fn remove_friend(&self, device: DevicePublicKey) -> Result<(), NetError> {
        self.tell(Command::RemoveFriend(device)).await
    }

    /// Announces this node under the rendezvous secret `secret` and looks up who else does,
    /// dialling them at the punch slots, until called again with `None`: how two computers
    /// meet to move an account. With `lan`, its local addresses go sealed under it in the
    /// announcements, and those of the other computer, sealed under the same, are opened and
    /// dialled (two routers at home, one public address without a way back in). What they
    /// say goes over a stream protocol of their own ([`Self::streams`],
    /// [`Self::peers_taking`]).
    pub async fn seek(
        &self,
        secret: Option<[u8; 32]>,
        lan: Option<[u8; 32]>,
    ) -> Result<(), NetError> {
        self.tell(Command::Seek(secret, lan)).await
    }

    /// Connected peers this node found announced under the move's code (see
    /// [`Self::seek`]), whether or not they list the move's protocol: the other computer lists
    /// it only to peers it found itself, and may not have found this one yet. Anyone can
    /// announce under the code, so what they say is checked with the code.
    pub async fn move_candidates(&self) -> Result<Vec<PeerId>, NetError> {
        self.ask(Command::MoveCandidates).await
    }

    /// How a connected peer is reached now, by its best connection.
    pub async fn peer_path(&self, peer: PeerId) -> Result<Option<Path>, NetError> {
        self.ask(|reply| Command::PeerPath(peer, reply)).await
    }

    /// The device `peer` is, if it is a friend recognised on its connection or a verified
    /// member of a group this node runs: those this device trusts with what it tells only
    /// friends and members.
    pub async fn trusted_device(&self, peer: PeerId) -> Option<DevicePublicKey> {
        self.ask(|reply| Command::Trusted(peer, reply))
            .await
            .ok()
            .flatten()
    }

    /// Connected peers that identified themselves, whatever they listed: a peer lists
    /// Fogata's protocols only to those it chose to reach and to members and friends.
    pub fn identified_peers(&self) -> Vec<PeerId> {
        locked(&self.advertised).keys().copied().collect()
    }

    /// Connected peers that said they take `protocol` (see [`Self::advertises`]).
    pub fn peers_taking(&self, protocol: &StreamProtocol) -> Vec<PeerId> {
        locked(&self.advertised)
            .iter()
            .filter(|(_, protocols)| protocols.contains(protocol))
            .map(|(peer, _)| *peer)
            .collect()
    }

    /// Transport identity of this node for the current session.
    pub fn local_peer(&self) -> PeerId {
        self.local_peer
    }

    /// Opens and accepts raw streams to other peers. Callers must check
    /// [`NodeHandle::is_member`] before trusting a peer.
    pub fn streams(&self) -> libp2p_stream::Control {
        self.streams.clone()
    }

    /// Runs `task` as one of the node's own: it is aborted when the node stops, before its
    /// shutdown returns (a loop taking streams, say, or what answers one).
    pub fn spawn(&self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        self.tasks.spawn(task);
    }

    /// How many of the node's own tasks (see [`Self::spawn`]) run now: none once it stopped.
    pub fn tasks_running(&self) -> usize {
        self.tasks.running()
    }

    /// The next stream `incoming` takes, or `None` once the node stopped: a loop accepting
    /// them ends with the node. Waiting on `incoming` alone, it would wait for ever, since
    /// the handles such loops hold keep the streams' state open. Such a loop holds a
    /// [`Self::background`] handle, or the node would never stop.
    pub async fn next_stream(
        &self,
        incoming: &mut libp2p_stream::IncomingStreams,
    ) -> Option<(PeerId, libp2p::Stream)> {
        next_until_stopped(&self.stopped, incoming).await
    }

    /// A handle for a task that runs as long as the node does (a loop taking streams, say):
    /// it reaches the node like any other, but does not keep it running, so the node still
    /// stops once the application's own handles are dropped, and the task ends with it.
    pub fn background(&self) -> NodeHandle {
        let commands = match &self.commands {
            Commands::Keeps(sender) => Commands::Background(sender.downgrade()),
            weak @ Commands::Background(_) => weak.clone(),
        };
        NodeHandle {
            commands,
            ..self.clone()
        }
    }

    /// Whether `peer` proved membership of any group on this node.
    pub fn is_member(&self, peer: &PeerId) -> bool {
        locked(&self.verified).contains(peer)
    }

    /// The device `peer` proved to be, when it proved membership of a group here: one
    /// device may connect under several transport identities.
    pub fn member_device(&self, peer: &PeerId) -> Option<DevicePublicKey> {
        locked(&self.member_devices).get(peer).copied()
    }

    /// Whether `peer` proved membership of `group` on this node.
    pub fn is_member_of(&self, peer: &PeerId, group: &GroupId) -> bool {
        locked(&self.group_members)
            .get(group)
            .is_some_and(|set| set.contains(peer))
    }

    /// Whether connected `peer` said it takes `protocol` (one of Fogata's own, `/fogata/...`),
    /// as a peer does for every protocol it accepts streams of ([`Self::streams`]). False
    /// until the peer has identified itself, and for older versions that do not have it.
    pub fn advertises(&self, peer: &PeerId, protocol: &StreamProtocol) -> bool {
        locked(&self.advertised)
            .get(peer)
            .is_some_and(|set| set.contains(protocol))
    }

    /// The capability level connected `peer` says it speaks ([`caps`]); `None` until it has
    /// identified itself.
    pub fn peer_level(&self, peer: &PeerId) -> Option<u32> {
        locked(&self.advertised).get(peer).map(caps::level_listed)
    }

    /// The highest capability level `device` was ever seen at, or [`caps::FIRST_LEVEL`].
    pub fn level_seen(&self, device: &DevicePublicKey) -> u32 {
        locked(&self.levels)
            .get(device)
            .copied()
            .unwrap_or(caps::FIRST_LEVEL)
    }

    /// Levels kept from earlier sessions ([`NodeEvent::LevelSeen`]); a lower one never
    /// replaces a higher one known.
    pub fn remember_levels(&self, levels: impl IntoIterator<Item = (DevicePublicKey, u32)>) {
        let mut known = locked(&self.levels);
        for (device, level) in levels {
            let level = level.min(caps::MAX_LEVEL);
            let at = known.entry(device).or_insert(level);
            *at = (*at).max(level);
        }
    }

    /// Whether to use a feature that came with capability level `since` with `peer`, a
    /// session of `device`: never the older way with a device once seen newer
    /// ([`caps::decide`]).
    pub fn decide(&self, peer: &PeerId, device: &DevicePublicKey, since: u32) -> caps::Decision {
        caps::decide(self.level_seen(device), self.peer_level(peer), since)
    }
}

/// Starts a node in the background. Events arrive on the returned receiver; the node stops
/// when every [`NodeHandle`] has been dropped (those of [`NodeHandle::background`] aside).
pub fn start(config: NodeConfig) -> Result<(NodeHandle, NodeEvents), NetError> {
    node::start(config)
}

/// Stretches a friend code made or used at `unix_secs` away from the async threads; it takes
/// a noticeable moment.
async fn stretch(code: FriendCode, unix_secs: u64) -> Result<CodeSecrets, NetError> {
    tokio::task::spawn_blocking(move || CodeSecrets::stretch(&code, unix_secs))
        .await
        .map_err(|_| NetError::Stopped)
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Short printable form of a peer id (its last 8 characters).
pub fn short_peer(peer: &PeerId) -> String {
    let text = peer.to_base58();
    text[text.len().saturating_sub(8)..].to_string()
}
