//! What the Fogata application builds on. [`Core`] opens a device's data folder, runs one
//! network node for every group and friend, keeps the folder up to date as things change, and
//! gives the screens what they show: the groups and their rooms, the friends and their
//! presence, a room's people, chat and files. [`CoreEvent`]s tell the screens when to look
//! again.
//!
//! Nothing said or shared in a room is ever written to disk: the folder holds the identity,
//! the profile and friends, the group logs, a few times (when a friend was last seen, when
//! a group last had a room) and, in `direct/`, the direct chats with friends.
//!
//! What [`Core`] does is spread by subject: this device's profile and friends (friends.rs),
//! groups (groups.rs), voice rooms (rooms.rs) and room file downloads (downloads.rs);
//! node_events.rs turns what the network node tells into the core's state.

#![forbid(unsafe_code)]

mod active_files;
pub mod calls;
pub mod diagnose;
pub mod direct;
mod downloads;
pub mod events;
pub mod firewall;
mod friends;
#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzzing;
mod groups;
pub mod invite;
mod node_events;
pub mod recovery;
mod rooms;
mod saves;
mod shares;
pub mod store;
pub mod transfer;
pub mod updates;
pub mod vault;
mod writer;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
pub use fogata_friends::{
    Animated, Animation, Banner, MAX_BANNER_ANIMATION_BYTES, MAX_BANNER_BYTES,
    MAX_PHOTO_ANIMATION_BYTES,
};
use fogata_friends::{Avatar, FriendBook, PRESET_AVATARS, Profile};
use fogata_net::{
    ChatLine, FileInfo, GroupSetup, Multiaddr, NetworkReport, NodeConfig, NodeHandle, Path,
    RoomChange,
    files::{self, SharedFiles},
};
use fogata_state::Body;

pub use crate::calls::{ALONE_MS, CallEnd, CallPhase, CallView, MAX_IN_CALL, RING_MS};
use crate::downloads::{Fetching, Playing};
pub use crate::downloads::{free_file_name, mark_as_downloaded, save_received, saved_name};
pub use crate::events::CoreEvents;
use crate::node_events::{PUMP_DRAIN, pump, save_waiting_friends, save_waiting_profiles};
pub use crate::store::{DeclineLeave, Meta, Offered, Store, default_data_dir};
pub use fogata_state::{Barrier, FIRST_ROOM, KeptOut, VideoPreview, VoiceRoomId};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// How long a group this device left still runs at start, so that its leave reaches the
/// members who were away; as long as members look up the keys of older epochs.
const LEAVE_DELIVERY_MS: u64 = 14 * 24 * 3600 * 1000;
/// A friend's group invitations are announced at most [`OFFER_NOTICES`] times in this long;
/// the others still wait in the list.
const OFFER_NOTICE_MS: u64 = 60_000;
const OFFER_NOTICES: usize = 3;

/// A port for a new device: random, away from the well-known and the ephemeral ranges.
pub(crate) fn own_port() -> Result<u16> {
    let mut bytes = [0u8; 2];
    fogata_crypto::random_bytes(&mut bytes)?;
    Ok(20000 + u16::from_le_bytes(bytes) % 20000)
}

/// Network settings of the node.
#[derive(Clone, Debug)]
pub struct NetOptions {
    /// UDP/TCP port to listen on; 0 means this device's own port (see [`Meta::port`]).
    pub listen_port: u16,
    /// DHT bootstrap peers; empty means the public ones.
    pub bootstrap: Vec<Multiaddr>,
    pub use_dht: bool,
    pub use_mdns: bool,
    /// Carry other members' traffic when they cannot connect directly.
    pub bridge: bool,
    /// Addresses to dial at start.
    pub peers: Vec<Multiaddr>,
    /// Appear offline to friends and members.
    pub hidden: bool,
}

impl Default for NetOptions {
    fn default() -> Self {
        Self {
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: true,
            use_mdns: true,
            bridge: true,
            peers: Vec::new(),
            hidden: false,
        }
    }
}

/// Someone, as the screens show them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub device: DevicePublicKey,
    pub name: String,
    /// Short fingerprint, `7Q4M·K2XR`.
    pub short: String,
    /// Their avatar, or a preset picked from their key when their profile is not known.
    pub avatar: Avatar,
}

/// A group's room, for the home screen.
#[derive(Clone, Debug)]
pub struct RoomSummary {
    pub started_ms: u64,
    pub participants: Vec<Person>,
    pub joined: bool,
}

/// A group, for the home screen.
#[derive(Clone, Debug)]
pub struct GroupView {
    pub id: GroupId,
    /// The name every member sees.
    pub name: String,
    /// The name this device stores it under.
    pub local_name: String,
    /// One of the preset icons the application draws.
    pub icon: u8,
    pub members: usize,
    /// Members connected now, this device not included.
    pub online: usize,
    /// This device manages the group: changes it, invites and lets people in.
    pub manager: bool,
    /// Where this device is on the group's ladder, 0 for the first.
    pub rank: usize,
    /// Who manages the group: the first `n` on the ladder, or everyone (`None`).
    pub managers: Option<u8>,
    /// A small PNG or JPEG shown instead of the icon.
    pub picture: Option<Vec<u8>>,
    /// A short name of the group's banner, when it has one ([`Core::group_banner`]): it
    /// changes with the banner.
    pub banner: Option<[u8; 16]>,
    /// The most members the group takes, and the most people in each voice room.
    pub max_members: u8,
    pub max_in_room: u8,
    /// Every voice room in order, with its room when one is open.
    pub voice_rooms: Vec<VoiceRoomView>,
    /// The voice room this device is in, or else the first one with people in it.
    pub room: Option<RoomSummary>,
    pub last_room_ms: Option<u64>,
    /// Invites made and not used yet, nor cancelled or run out.
    pub open_invites: usize,
    /// The members below this device on the ladder, the next one first: those it may
    /// remove, move up or down, and take out of a voice room or keep out of one.
    pub outranks: Vec<DevicePublicKey>,
    /// Members kept out of a voice room now, in the order of the rooms.
    pub kept_out: Vec<KeptOutView>,
    /// For a manager, while the group waits to go up to the protection this version gives
    /// (see [`fogata_state::Level`]): the members whose Fogata was not seen to be new
    /// enough for it, who hold it back until they update. Empty otherwise.
    pub level_behind: Vec<Person>,
}

/// A voice room of a group: its own people, chat and files.
#[derive(Clone, Debug)]
pub struct VoiceRoomView {
    pub id: VoiceRoomId,
    /// Empty for the first room until someone names it.
    pub name: String,
    pub room: Option<RoomSummary>,
    /// Only these members may enter (and the rule's author and those above it); everyone
    /// when `None`.
    pub only: Option<Vec<DevicePublicKey>>,
    /// Why this device may not enter now, if it may not.
    pub barrier: Option<Barrier>,
}

/// A member a manager kept out of a voice room.
#[derive(Clone, Debug)]
pub struct KeptOutView {
    pub voice: VoiceRoomId,
    pub member: Person,
    pub by: Person,
    /// When it may come back, by the clock of the manager who kept it out; `None` until an
    /// manager lets it back.
    pub until_ms: Option<u64>,
}

/// A device that showed one of this manager's invites and waits to be let in.
#[derive(Clone, Debug)]
pub struct JoinAsk {
    pub group: GroupId,
    pub group_name: String,
    pub device: DevicePublicKey,
    /// The name it asks to join under.
    pub name: String,
    pub since_ms: u64,
}

/// What a member did in a group (see [`Core::group_activity`]), with the people named.
#[derive(Clone, Debug, PartialEq)]
pub enum ActivityWhat {
    Created {
        name: String,
    },
    Renamed {
        name: String,
    },
    Icon,
    /// Set the group's picture, or took it away.
    Picture {
        taken_away: bool,
    },
    /// Set the group's banner, or took it away.
    Banner {
        taken_away: bool,
    },
    Added {
        member: Person,
    },
    Joined {
        invited_by: Person,
    },
    /// Let in someone who joined with an invite.
    LetIn {
        member: Person,
    },
    Removed {
        member: Person,
    },
    Left,
    /// Moved `member` up to just below itself, to `place` (0 for the first).
    MovedUp {
        member: Person,
        place: usize,
    },
    MovedDown {
        member: Person,
    },
    /// Who manages: the first `first`, or everyone (`None`).
    Managers {
        first: Option<u8>,
    },
    Limits {
        members: u8,
        room: u8,
    },
    RoomOpened {
        name: String,
    },
    RoomRenamed {
        from: String,
        name: String,
    },
    RoomClosed {
        name: String,
    },
    Invited,
    Revoked,
    /// Who may enter voice room `room`: everyone, or only `members`.
    RoomAccess {
        room: String,
        members: Option<Vec<Person>>,
    },
    KeptOut {
        room: String,
        member: Person,
        until_ms: Option<u64>,
    },
    LetBack {
        room: String,
        member: Person,
    },
}

/// How what a member did was undone: it was removed, left or was moved to the end at the
/// same time, without having seen it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoneKind {
    Removed,
    Left,
    MovedDown,
}

/// One line of a group's activity.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityView {
    pub by: Person,
    /// When, by its author's clock.
    pub at_ms: u64,
    pub what: ActivityWhat,
    /// Done, then undone: how, and by whom (nobody when its author left).
    pub undone: Option<(UndoneKind, Option<Person>)>,
    /// Seen arriving on this device rather than told by the log: the banner and the
    /// picture, of which only the newest is kept.
    pub seen_here: bool,
}

#[derive(Clone, Debug)]
pub struct MemberView {
    pub person: Person,
    /// Where it is on the group's ladder, 0 for the first.
    pub rank: usize,
    /// It manages the group.
    pub manager: bool,
    pub me: bool,
    /// How it is reached, when connected.
    pub online: Option<Path>,
    /// Connected, and nobody used its computer for a while (never while in the group's
    /// room).
    pub away: bool,
    /// It has not been given the current group key yet.
    pub waiting_for_key: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    Offline {
        last_seen_ms: Option<u64>,
    },
    Online,
    /// Online, but nobody used the friend's computer for a while.
    Away,
    /// In a room; the group only when this device is a member of it too.
    InRoom {
        group: Option<(GroupId, String)>,
    },
}

#[derive(Clone, Debug)]
pub struct FriendView {
    pub person: Person,
    /// Full fingerprint, `7Q4M K2XR 9PLD 3VHC`.
    pub fingerprint: String,
    pub avatar: Avatar,
    pub presence: Presence,
    pub path: Option<Path>,
    /// Whether the friend has this device's current friend secret yet.
    pub delivered: bool,
    /// The friend's bio, once it sent one (empty when it shows none).
    pub bio: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RequestView {
    pub person: Person,
    pub fingerprint: String,
    pub since_ms: u64,
}

/// A file in a room.
#[derive(Clone, Debug)]
pub struct FileView {
    pub info: FileInfo,
    pub shared_by: Person,
    /// When it was listed, by the clock of whoever listed it.
    pub shared_ms: u64,
    pub holders: Vec<Person>,
    /// This device shares it.
    pub mine: bool,
    /// Bytes received so far while this device downloads it.
    pub downloading: Option<u64>,
    /// Times this device sent it to someone.
    pub sent: u32,
    /// Where this device keeps it, once shared from here or downloaded.
    pub path: Option<PathBuf>,
}

/// A group's room, for the room screen.
#[derive(Clone, Debug)]
pub struct RoomState {
    pub started_ms: u64,
    pub joined: bool,
    /// Participants, and whether each seems away.
    pub participants: Vec<(Person, bool)>,
    pub chat: Vec<ChatLine>,
    pub files: Vec<FileView>,
}

/// A group a friend added this device to, waiting for the person to join or decline.
#[derive(Clone, Debug)]
pub struct InvitationView {
    pub id: GroupId,
    pub name: String,
    pub by: Person,
    pub members: usize,
}

/// A friend invite to show: a link or a code, and when it stops working.
#[derive(Clone, Debug)]
pub struct Invitation {
    pub text: String,
    pub expires_ms: u64,
}

/// How this device's secrets (its identity, friends and direct chats) are kept in its data
/// folder ([`crate::vault`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtRest {
    /// Sealed with a key the system keychain keeps.
    Sealed,
    /// Being sealed with a key the system keychain keeps, and not all of it is yet: what
    /// could not be sealed (a full disk, say), a start that stopped half way, or attachments
    /// still being sealed. Sealing goes on at each start.
    Unfinished,
    /// Plain: no system keychain kept a key for them. Sealing is tried again at each start.
    NoKeychain,
    /// Plain, as this process was set to leave them (copies run for tests).
    Plain,
}

/// Something changed; the screens look again.
#[derive(Clone, Debug)]
pub enum CoreEvent {
    /// Groups, their members online or their rooms changed (home screen).
    GroupsChanged,
    /// A friend rings this device ([`Core::accept_call`], [`Core::decline_call`]).
    CallIncoming {
        group: GroupId,
        from: DevicePublicKey,
    },
    /// The friend this device rang answered.
    CallAccepted {
        group: GroupId,
    },
    CallEnded {
        group: GroupId,
        with: DevicePublicKey,
        reason: CallEnd,
    },
    /// A friend rung into the call `group` did not come in, for `reason`.
    CallAddEnded {
        group: GroupId,
        friend: DevicePublicKey,
        reason: CallEnd,
    },
    /// Calls rang, were answered or ended ([`Core::calls`]).
    CallsChanged,
    /// Friends, requests or presence changed (friends screen).
    FriendsChanged,
    /// Friends and members see this device as away, or no longer ([`Core::set_away`]).
    Away {
        away: bool,
    },
    Room {
        group: GroupId,
        voice: VoiceRoomId,
        change: RoomChange,
    },
    /// Someone showed an invite of this manager and waits to be let in ([`Core::join_asks`],
    /// [`Core::answer_join`]).
    JoinAsked {
        group: GroupId,
        device: DevicePublicKey,
        name: String,
    },
    /// A device waiting to be let in is not waiting any more.
    JoinAsksChanged,
    /// This device showed its invite to a manager and waits for a person to let it in.
    JoinWaiting {
        group: GroupId,
        name: String,
    },
    /// A manager said no to this device joining; the invite no longer works.
    JoinDeclined {
        group: GroupId,
        name: String,
    },
    /// Someone (`name`) asked this manager to be let into `group`, but their Fogata is too
    /// old for the group's protection: they must update it before they can get in. Their
    /// invite still works once they have.
    MustUpdateToJoin {
        group: GroupId,
        device: DevicePublicKey,
        name: String,
    },
    /// The group this device is joining (`name`) needs a newer Fogata than this one: it
    /// gets in once Fogata is updated.
    UpdateToJoin {
        group: GroupId,
        name: String,
    },
    /// A member of a group connected, or is now reached by a better path.
    MemberConnected {
        group: GroupId,
        device: DevicePublicKey,
        path: Path,
    },
    MemberDisconnected {
        group: GroupId,
        device: DevicePublicKey,
    },
    FriendRequest {
        profile: Profile,
    },
    /// A friend or member changed the animation of its picture or banner
    /// ([`Core::animation`]).
    Animation {
        device: DevicePublicKey,
    },
    FriendRequestSent {
        device: DevicePublicKey,
        name: String,
    },
    FriendRequestFailed(String),
    FriendAdded {
        profile: Profile,
    },
    FriendOnline {
        device: DevicePublicKey,
    },
    FriendOffline {
        device: DevicePublicKey,
    },
    /// An invite was accepted.
    Joined {
        group: GroupId,
        name: String,
    },
    /// This device left a group or was removed from it.
    Removed {
        group: GroupId,
        name: String,
    },
    /// This device left voice room `voice` of `group`, which it was in: a manager (`by`)
    /// took it out of the room (no `barrier`), or it is kept out of it now (`barrier`).
    TakenOut {
        group: GroupId,
        voice: VoiceRoomId,
        by: Option<DevicePublicKey>,
        barrier: Option<Barrier>,
    },
    /// A friend added this device to a group: an invitation waits for the person's answer
    /// ([`Core::group_invitations`]).
    GroupInvited {
        group: GroupId,
        name: String,
        by: DevicePublicKey,
    },
    /// `device` (called `who` here) declined an invitation of this device to the group
    /// called `name`. Said once per invitation.
    InvitationDeclined {
        group: GroupId,
        name: String,
        device: DevicePublicKey,
        who: String,
    },
    Download {
        group: GroupId,
        file: [u8; 32],
        received: u64,
        total: u64,
    },
    DownloadDone {
        group: GroupId,
        file: [u8; 32],
        path: PathBuf,
    },
    DownloadFailed {
        group: GroupId,
        file: [u8; 32],
        reason: String,
    },
    /// Other peers see this device at this address.
    Address(Multiaddr),
    /// The router changes the public port for each destination.
    HardNat,
    /// A member of `group` sent this device a call signal for its WebRTC engine.
    Signal {
        group: GroupId,
        from: DevicePublicKey,
        payload: Vec<u8>,
    },
    /// Call signals `from` sent in `group` were dropped on their way to the screens, which
    /// fell far behind (see events.rs): the call with `from` is to agree anew. No signal of
    /// `from` comes between the last one kept and this.
    SignalsLost {
        group: GroupId,
        from: DevicePublicKey,
    },
    Info(String),
    /// Something the person should know, said in words for them.
    Notice(String),
    /// This device's identity is open on another computer at the same time
    /// ([`OTHER_SESSION_NOTICE`] says it in words). Said at most once in ten minutes.
    OtherSession,
    /// The network part stopped by itself after an internal error: nothing reaches this
    /// device or leaves it until the application starts again.
    Stopped,
    /// A direct chat changed: a message came or went, was delivered or read, an attachment
    /// came or was removed, the friend connected ([`Core::direct_chat`]).
    DirectChanged {
        friend: DevicePublicKey,
    },
    /// A friend is typing in the chat with this device now, or stopped. Never kept.
    DirectTyping {
        friend: DevicePublicKey,
        typing: bool,
    },
    /// Someone in the voice room `voice` of `group` this device is in is typing in its
    /// chat now, or stopped. Never kept.
    Typing {
        group: GroupId,
        voice: VoiceRoomId,
        from: DevicePublicKey,
        typing: bool,
    },
    /// A friend wrote in the chat with this device; `text` is the message's first line.
    DirectMessage {
        friend: DevicePublicKey,
        text: String,
    },
    /// An attachment of a direct chat is coming.
    DirectDownload {
        friend: DevicePublicKey,
        file: [u8; 32],
        received: u64,
        total: u64,
    },
    DirectDownloadFailed {
        friend: DevicePublicKey,
        file: [u8; 32],
        reason: String,
    },
    /// Where updates stand changed ([`Core::update_status`]).
    UpdateChanged,
}

/// What the person is told when this device's identity is open on another computer too.
pub const OTHER_SESSION_NOTICE: &str = "This identity is open on another computer too. Close \
     Fogata there: one identity works on one computer at a time.";

/// The running application core. Cheap to clone; every clone is the same core.
#[derive(Clone)]
pub struct Core {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("device", &self.inner.identity.public_key())
            .finish_non_exhaustive()
    }
}

/// Where the local WebRTC engine finds this device's media bridge (a TURN endpoint that
/// carries media over member connections), and its credentials.
#[derive(Clone, Debug)]
pub struct MediaBridge {
    /// `127.0.0.1`, or on Windows the device's own address.
    pub host: std::net::IpAddr,
    pub port: u16,
    pub username: String,
    pub password: String,
}

struct Inner {
    store: Arc<Store>,
    /// Writes files off the event pump, in order (see writer.rs).
    writer: Arc<writer::Writer>,
    /// The task handling the node's events, until the core shuts down.
    pump: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Locked while this core runs (see [`Store::hold_running`]).
    _running: std::fs::File,
    identity: Arc<DeviceIdentity>,
    node: NodeHandle,
    shared: SharedFiles,
    /// Copies of the files shared in rooms, served in their place (see shares.rs).
    copies: files::Copies,
    /// Bytes of each disk held for copies being made and files fetched now (see
    /// `shares::Room`).
    copying: Mutex<HashMap<shares::Disk, u64>>,
    /// The memory previews being fetched may hold together, in KiB (see
    /// `Core::fetch_preview`).
    previews: tokio::sync::Semaphore,
    state: Mutex<State>,
    events: events::Sender,
    bridge: tokio::sync::Mutex<Option<(fogata_media::bridge::Bridge, MediaBridge)>>,
    /// Held while a call's room is entered or let go of (see calls.rs): a call that comes
    /// back as it is let go of finds its room and group either let go of already, or kept.
    call_rooms: tokio::sync::Mutex<()>,
    /// Held from asking the node for a group's log until it is handed to the writer
    /// ([`Inner::save_group`]): the writer keeps the last log handed to it for a file, so the
    /// logs must reach it in the order the node gave them, newest last. Without it a save
    /// that asked before a change and was slow to hand its log over (the event pump, while
    /// [`Core::shutdown`] saves every group) would write the log from before the change.
    saving_groups: tokio::sync::Mutex<()>,
    /// The node is being shut down on purpose ([`Core::shutdown`]).
    stopping: AtomicBool,
    /// Direct chats with friends.
    direct: direct::Chats,
    /// New versions from friends and members, once started ([`Core::start_updates`]).
    updates: std::sync::OnceLock<Arc<updates::Updater>>,
}

#[derive(Default)]
struct State {
    /// Profiles of members met in groups.
    profiles: HashMap<DevicePublicKey, Profile>,
    /// Animations of friends' and members' pictures and banners.
    animations: HashMap<(DevicePublicKey, Animated), Animation>,
    /// Those of people no longer friends or members are about to be forgotten
    /// (`forget_gone_soon` in node_events.rs).
    forgetting: bool,
    online: HashMap<GroupId, HashMap<DevicePublicKey, Path>>,
    friends_online: HashMap<DevicePublicKey, Path>,
    /// Friends and members that appear offline: shown as offline, except in a room they are
    /// in.
    hidden: HashSet<DevicePublicKey>,
    /// Friends and members away from their computers, while connected.
    away: HashSet<DevicePublicKey>,
    /// Friends in a room, with the group when we share it.
    friend_rooms: HashMap<DevicePublicKey, Option<GroupId>>,
    downloads: HashMap<(GroupId, [u8; 32]), u64>,
    /// Fetches of each file running now: a download, and a video fetched to play. Its
    /// progress goes once the last one ends.
    fetching: HashMap<(GroupId, [u8; 32]), u32>,
    /// Which holder sent what a room file's `.part` holds, by the `.part`'s path: another
    /// holder never goes on from it (`fetch_from_holders` in downloads.rs).
    part_from: HashMap<PathBuf, DevicePublicKey>,
    /// Where each download running now will be saved, by file: asked again, it goes on.
    saving_files: HashMap<(GroupId, [u8; 32]), PathBuf>,
    /// Videos of rooms being fetched to play, by file and folder: asked for again meanwhile
    /// into the same folder (its view closed and opened again), the same fetch is waited for.
    playing: HashMap<(GroupId, [u8; 32], PathBuf), Playing>,
    /// The free space on the disk of a folder, as the system tells it ([`Core::use_free_space`]).
    free_space: Option<fn(&FsPath) -> Option<u64>>,
    /// Told each time this device leaves a room on purpose (`out_of_room` in downloads.rs).
    left_room: Arc<tokio::sync::Notify>,
    /// Where downloads running now will be saved, so that two of the same name never land
    /// in one place.
    saving: HashSet<PathBuf>,
    sent: HashMap<(GroupId, [u8; 32]), u32>,
    /// Local names for groups being joined with an invite.
    joining: HashMap<GroupId, String>,
    /// Devices waiting for this manager to let them in, with the name they asked under and
    /// since when.
    join_asks: BTreeMap<(GroupId, DevicePublicKey), (String, u64)>,
    meta: Meta,
    /// Direct calls ringing or going on, by their group.
    calls: HashMap<GroupId, calls::CallEntry>,
    /// Calls this device left a moment ago while others were still in them.
    left_calls: HashMap<GroupId, calls::LeftCall>,
    /// This device appears offline: calls do not ring here.
    appear_offline: bool,
    /// Friends and members see this device as away ([`Core::set_away`]).
    shown_away: bool,
    /// When the friend book and the members' profiles were last written (see saves.rs),
    /// and the newest book while its write waits.
    friends_saves: saves::Saves,
    unsaved_book: Option<FriendBook>,
    profiles_saves: saves::Saves,
    /// When each friend's group invitations were announced lately (see `OFFER_NOTICE_MS`).
    offer_notices: HashMap<DevicePublicKey, Vec<u64>>,
}

impl State {
    /// Whether a connected member shows as online: not when it appears offline, unless it is
    /// in the group's room, which shows who is in it anyway.
    fn shows_online(&self, device: &DevicePublicKey, in_room: &HashSet<DevicePublicKey>) -> bool {
        !self.hidden.contains(device) || in_room.contains(device)
    }

    /// Whether a friend or member shows as away: connected, visible, away, and in no room.
    fn shows_away(&self, device: &DevicePublicKey, in_room: bool) -> bool {
        self.away.contains(device) && !self.hidden.contains(device) && !in_room
    }

    /// A device no longer connected as a friend nor as a member is away no more: it says
    /// so again when it comes back.
    fn forget_away_if_gone(&mut self, device: &DevicePublicKey) {
        let connected = self.friends_online.contains_key(device)
            || self.online.values().any(|m| m.contains_key(device));
        if !connected {
            self.away.remove(device);
        }
    }
}

/// What a start says while files refused by sealing are kept in `dir` (the screens find it
/// by its first words, and the folder after "kept in ").
fn refused_notice(dir: &FsPath) -> String {
    format!(
        "Some of Fogata's files on this computer were found unencrypted where they are kept \
         encrypted, so they were not used. Nothing was deleted: they are kept in {}",
        dir.display()
    )
}

impl Core {
    /// Whether the data folder has a profile yet (the welcome screen makes one).
    pub fn has_profile(data_dir: &FsPath) -> Result<bool> {
        transfer::recover(data_dir);
        Store::open(data_dir.to_path_buf())?.has_profile()
    }

    /// Where the profile and friends made in the data folder were kept when they could not
    /// be read or were refused, while it has no profile ([`Store::friends_kept`]): the
    /// welcome screen says so, rather than look like a new device's.
    pub fn kept_profile(data_dir: &FsPath) -> Result<Option<PathBuf>> {
        Ok(Store::open(data_dir.to_path_buf())?.friends_kept())
    }

    /// Makes this device's identity, if it has none, and its profile.
    pub fn create_profile(
        data_dir: &FsPath,
        name: &str,
        avatar: Option<Avatar>,
    ) -> Result<Profile> {
        let store = Store::open(data_dir.to_path_buf())?;
        transfer::refuse_if_staged(data_dir)?;
        if store.friends()?.is_some() {
            bail!("this device already has a profile");
        }
        let (identity, _) = store.identity()?;
        let mut book = FriendBook::new(&identity, name, now_ms())?;
        if avatar.is_some() {
            book.set_profile(&identity, None, avatar, now_ms())?;
        }
        store.save_friends(&book)?;
        Ok(book.profile)
    }

    /// Opens the data folder and starts the node with every group and the friends.
    pub fn start(data_dir: PathBuf, options: NetOptions) -> Result<(Core, CoreEvents)> {
        let store = Store::open(data_dir)?;
        let running = store.hold_running()?;
        // A move into this folder that was being put in place goes on first.
        transfer::recover_held(store.dir(), &vault::chosen())?;
        store.mark_layout();
        let (identity, _) = store.identity()?;
        let me = identity.public_key();
        // Files that cannot be read nor mended (the folder cannot be written) leave the device
        // without friends or groups this time, rather than not starting; they stay as they are.
        // So does what is kept of friends and members: only with every friend and group read
        // are the others' pictures forgotten.
        let mut all_read = true;
        let book = store.friends().unwrap_or_else(|e| {
            tracing::error!("cannot read the profile and friends: {e:#}");
            all_read = false;
            None
        });
        let stored_groups = store.groups().unwrap_or_else(|e| {
            tracing::error!("cannot read the list of groups: {e:#}");
            all_read = false;
            Vec::new()
        });
        let mut groups = Vec::new();
        for stored in stored_groups {
            // Invitations wait for the person; nothing of them runs or is announced.
            if stored.pending {
                continue;
            }
            let log = match store.load_log(&stored) {
                Ok(log) => log,
                Err(e) => {
                    let id = stored.group_id.get(..12).unwrap_or(&stored.group_id);
                    tracing::warn!("group {id} not loaded: {e:#}");
                    all_read = false;
                    continue;
                }
            };
            // A leave not delivered yet still needs the group running, for a while: nothing
            // says when a member got it, and a group left long ago would run at every start.
            let since = now_ms().saturating_sub(LEAVE_DELIVERY_MS);
            let leaving = log
                .entries()
                .any(|e| e.author == me && matches!(e.body, Body::Leave) && e.time_ms > since);
            if log.shared_state().is_member(&me) || leaving {
                groups.push(GroupSetup::Member(log));
            } else {
                // Left or removed: its log stays, its banner and picture are not kept.
                store.forget_group_pictures(&log.group_id());
            }
        }
        // Joins with an invite that were still waiting for a manager go on.
        let mut joining = HashMap::new();
        let mut invite_addrs = Vec::new();
        for pending in store.joins() {
            let Ok(invite) = invite::decode(&pending.invite) else {
                continue;
            };
            let group = invite.group_id();
            if hex::encode(group.as_bytes()) != pending.group_id
                || groups.iter().any(|g| match g {
                    GroupSetup::Member(log) => log.group_id() == group,
                    _ => false,
                })
            {
                if let Err(e) = store.note_join(group, None) {
                    tracing::warn!("cannot forget a join: {e:#}");
                }
                continue;
            }
            groups.push(GroupSetup::Joining {
                group_id: group,
                ticket: invite.ticket.clone(),
                rendezvous: invite.rendezvous(),
                name: pending.nick,
            });
            joining.insert(group, pending.name);
            invite_addrs.push((group, invite.addrs));
        }
        // Where friends and members were reached before, for those still friends or members.
        let known: HashSet<DevicePublicKey> = book
            .iter()
            .flat_map(|b| b.friends.keys().copied())
            .chain(groups.iter().flat_map(|g| match g {
                GroupSetup::Member(log) => log.shared_state().members.keys().copied().collect(),
                _ => Vec::new(),
            }))
            .collect();
        // The last profiles met of members, for those still members.
        let members: HashSet<DevicePublicKey> = groups
            .iter()
            .flat_map(|g| match g {
                GroupSetup::Member(log) => log.shared_state().members.keys().copied().collect(),
                _ => Vec::new(),
            })
            .collect();
        // Animations of those still friends or members, and the others' gone from the disk.
        let stored = store.animations();
        let animations: HashMap<_, _> = stored
            .iter()
            .filter(|a| known.contains(&a.device))
            .map(|a| ((a.device, a.of), a.clone()))
            .collect();
        if all_read
            && animations.len() < stored.len()
            && let Err(e) = store.save_animations(animations.values())
        {
            tracing::warn!("cannot save the animations: {e:#}");
        }
        let stored = store.member_profiles();
        let profiles: HashMap<DevicePublicKey, Profile> = stored
            .iter()
            .filter(|p| members.contains(&p.device))
            .map(|p| (p.device, p.clone()))
            .collect();
        if all_read
            && profiles.len() < stored.len()
            && let Err(e) = store.save_member_profiles(profiles.values())
        {
            tracing::warn!("cannot save the members' profiles: {e:#}");
        }
        let identity = Arc::new(identity);
        let mut meta = store.meta();
        let remembered = meta
            .reached_addresses(now_ms())
            .into_iter()
            .filter_map(|(address, device)| {
                let device = DevicePublicKey::from_bytes(&hex::decode(device).ok()?).ok()?;
                Some((address.parse().ok()?, device)).filter(|(_, d)| known.contains(d))
            })
            .collect();
        let listen_port = match (options.listen_port, meta.port) {
            (0, Some(port)) => port,
            (0, None) => {
                let port = own_port()?;
                meta.port = Some(port);
                store.save_meta(&meta)?;
                port
            }
            (port, _) => port,
        };
        let (node, node_events) = fogata_net::start(NodeConfig {
            identity: identity.clone(),
            groups,
            friends: book,
            listen_port,
            bootstrap: options.bootstrap,
            use_dht: options.use_dht,
            use_mdns: options.use_mdns,
            bridge: options.bridge,
            peers: options.peers,
            remembered,
            hidden: options.hidden,
        })?;
        node.remember_levels(store.levels());
        // Right as the node starts, so members learn it when they first connect, with the
        // rest of what this device takes.
        fogata_media::bridge::advertise_parts(&node);
        direct::advertise_pins(&node);
        let hidden = options.hidden;
        // The node holds back what peers would add while the screens are far behind.
        let (events, receiver) = events::queue(Some(node_events.backlog()));
        if store.friends_restored() {
            let _ = events.send(CoreEvent::Notice(
                "Your list of friends was damaged on this computer and was restored from the \
                 copy made before its last change. A friend added or removed just before may \
                 need adding or removing again."
                    .into(),
            ));
        }
        // Never kept without a word: they may be the person's own (copied back by hand, say).
        if store.holds_refused() {
            let _ = events.send(CoreEvent::Notice(refused_notice(
                &store.dir().join(store::REFUSED_DIR),
            )));
        }
        let writer = Arc::new(writer::Writer::start()?);
        let chats = direct::Chats::open(store.dir(), store.data_key()?, writer.clone());
        let copies = files::Copies::open(store.dir().join(shares::COPIES_DIR), store.data_key()?)
            .context("cannot make the folder for shared files")?;
        let inner = Arc::new(Inner {
            store: Arc::new(store),
            writer,
            pump: Mutex::new(None),
            identity,
            node: node.clone(),
            shared: SharedFiles::default(),
            copies,
            copying: Mutex::new(HashMap::new()),
            previews: tokio::sync::Semaphore::new(downloads::PREVIEWS_KIB as usize),
            state: Mutex::new(State {
                meta,
                animations,
                profiles,
                appear_offline: hidden,
                joining,
                ..Default::default()
            }),
            events,
            bridge: tokio::sync::Mutex::new(None),
            call_rooms: tokio::sync::Mutex::new(()),
            saving_groups: tokio::sync::Mutex::new(()),
            stopping: AtomicBool::new(false),
            _running: running,
            direct: chats,
            updates: std::sync::OnceLock::new(),
        });
        let weak = Arc::downgrade(&inner);
        files::serve(node, inner.shared.clone(), move |group, id| {
            if let Some(inner) = weak.upgrade() {
                *inner.lock().sent.entry((group, id)).or_default() += 1;
            }
        })?;
        let events = tokio::spawn(pump(Arc::downgrade(&inner), node_events));
        *inner.pump.lock().unwrap_or_else(|e| e.into_inner()) = Some(events);
        for (group, addrs) in invite_addrs {
            let node = inner.node.clone();
            tokio::spawn(async move { node.dial(addrs, Some(group)).await });
        }
        direct::start(&inner);
        if let Some(port) = inner.lock().meta.port {
            tokio::spawn(watch_firewall(Arc::downgrade(&inner), port));
        }
        // Started whole with its sealed data: the plain copies of an earlier start can go.
        inner.store.drop_backup();
        Ok((Core { inner }, receiver))
    }

    /// Starts taking new versions from friends and members, and giving them on
    /// ([`updates`]). Once per core.
    pub fn start_updates(&self, options: updates::UpdateOptions) -> Result<()> {
        if self.inner.updates.get().is_some() {
            bail!("updates already run");
        }
        let updater = updates::Updater::start(
            self.inner.store.dir(),
            self.inner.node.clone(),
            options,
            self.inner.events.clone(),
        )?;
        let _ = self.inner.updates.set(updater);
        Ok(())
    }

    /// Where updates stand; `None` while they do not run.
    pub fn update_status(&self) -> Option<updates::UpdateStatus> {
        self.inner.updates.get().map(|u| u.status())
    }

    /// Asks every friend and member connected for a newer release now.
    pub fn check_for_update(&self) {
        if let Some(updater) = self.inner.updates.get() {
            updater.check_now();
        }
    }

    /// What the updater did lately, in words, each line after the time it happened (ms).
    pub fn update_notes(&self) -> Vec<String> {
        self.inner
            .updates
            .get()
            .map(|u| u.notes())
            .unwrap_or_default()
    }

    /// Installs the release that is ready. On success the application quits at once: the new
    /// version starts in its place (see `updates/install.rs`).
    pub async fn install_update(&self) -> Result<()> {
        let updater = self
            .inner
            .updates
            .get()
            .context("updates do not run")?
            .clone();
        updater.install().await
    }

    pub fn me(&self) -> DevicePublicKey {
        self.inner.identity.public_key()
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    /// How this device's secrets are kept in its data folder. Walks the attachments kept
    /// (reading the start of each) once the rest is sealed.
    pub fn at_rest(&self) -> AtRest {
        let store = &self.inner.store;
        if store.sealed() {
            if !self.inner.direct.holds_plain_files() {
                AtRest::Sealed
            } else {
                AtRest::Unfinished
            }
        } else if store.unfinished().is_some() {
            AtRest::Unfinished
        } else if store.may_seal() {
            AtRest::NoKeychain
        } else {
            AtRest::Plain
        }
    }

    /// The secret that keeps this device's data from being all sealed, sealed and not
    /// opening with the folder's key (a bit flipped on the disk, say), if that is what keeps
    /// it (see [`Store::damaged`]).
    pub fn damaged_secret(&self) -> Option<PathBuf> {
        self.inner.store.damaged()
    }

    /// Sets the secret [`Core::damaged_secret`] names aside at the next start, which then
    /// takes back the copy of it from before sealing and finishes sealing (see
    /// [`Store::set_aside_at_next_start`]). The application starts again after this.
    pub fn set_damaged_secret_aside(&self) -> Result<()> {
        let damaged = self
            .damaged_secret()
            .context("every secret opens with its key")?;
        self.inner.store.set_aside_at_next_start(&damaged)
    }

    // ----- Network -----

    /// The command that lets Fogata through this computer's own firewall when that lets
    /// nothing in for it (ufw on Linux, Windows' firewall, macOS's application firewall);
    /// `None` when nothing stands in the way, as far as the rules show.
    pub async fn firewall_command(&self) -> Option<String> {
        let port = self.inner.lock().meta.port?;
        firewall_blocks(port).await
    }

    /// Appears offline or not: friends and members show this device as offline, except in
    /// a room they share. Nothing is disconnected.
    pub async fn set_hidden(&self, hidden: bool) -> Result<()> {
        self.inner.lock().appear_offline = hidden;
        self.inner.node.set_hidden(hidden).await?;
        if !hidden {
            // What waited for a receipt meanwhile goes now, not at the next tick.
            direct::flush_waiting(&self.inner).await;
        }
        Ok(())
    }

    /// Whether nobody has used this computer for a while. Friends and members then see this
    /// device as away, except while it appears offline or is in a room or a call; nothing
    /// else changes. Returns whether they see it as away now, and says so with
    /// [`CoreEvent::Away`] when that changed.
    pub async fn set_away(&self, away: bool) -> Result<bool> {
        let shown = self.inner.node.set_away(away).await?;
        let changed = {
            let mut state = self.inner.lock();
            std::mem::replace(&mut state.shown_away, shown) != shown
        };
        if changed {
            self.inner.emit(CoreEvent::Away { away: shown });
        }
        Ok(shown)
    }

    /// Closes every connection and stops the node: friends and members see this device go
    /// at once. For when the application quits. The calls this device left are over for it
    /// then, and noted in its chats as far as it knows them.
    pub async fn shutdown(&self) -> Result<()> {
        // A file being copied to share stops at once: the copy holds the data folder.
        self.inner.copies.stop();
        calls::on_shutdown(&self.inner).await;
        self.inner.stopping.store(true, Ordering::Relaxed);
        // Changes that wait to be written in a moment are written now.
        save_waiting_friends(&self.inner);
        save_waiting_profiles(&self.inner);
        // The logs as the node has them now: a change it told just before may not have been
        // handled yet, and once it stops it cannot be asked for them.
        let me = self.me();
        for group in self.inner.node.groups().await.unwrap_or_default() {
            let id = group.group_id;
            if group.state.is_member(&me)
                && !self.inner.is_call(id)
                && !self.inner.direct.is_chat_group(&id)
            {
                self.inner.save_group(id).await;
            }
        }
        // The media bridge's tasks and TURN endpoint go before the node they use.
        let bridge = self.inner.bridge.lock().await.take();
        if let Some((bridge, _)) = bridge {
            bridge.close().await;
        }
        let stopped = self.inner.node.shutdown().await;
        // What the node told before it stopped is handled, and what that wrote is on disk.
        let events = self
            .inner
            .pump
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(events) = events {
            let _ = tokio::time::timeout(PUMP_DRAIN, events).await;
        }
        self.inner.writer.then(|| ()).await?;
        Ok(stopped?)
    }

    pub async fn network_report(&self) -> Result<NetworkReport> {
        Ok(self.inner.node.network_report().await?)
    }

    /// Addresses this device can be reached at now.
    pub async fn addresses(&self) -> Result<Vec<Multiaddr>> {
        Ok(self.inner.node.addresses().await?)
    }

    // ----- Names -----

    /// Someone as known here: this device's profile, a friend's, a member's, the name in the
    /// log or the room, or just the fingerprint.
    fn person_with(
        &self,
        device: DevicePublicKey,
        room_nick: Option<&str>,
        book: Option<&FriendBook>,
        state: &fogata_state::GroupState,
    ) -> Person {
        let known = book.and_then(|b| {
            if b.profile.device == device {
                Some(b.profile.clone())
            } else {
                b.friends.get(&device).map(|f| f.profile.clone())
            }
        });
        let shared = self.inner.lock().profiles.get(&device).cloned();
        // A friend's profile can also come as a member's: the newest wins.
        let profile = match (known, shared) {
            (Some(a), Some(b)) => Some(if b.version_ms > a.version_ms { b } else { a }),
            (a, b) => a.or(b),
        };
        if let Some(profile) = profile {
            return person_of(&profile);
        }
        let name = room_nick
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .or_else(|| {
                state
                    .members
                    .get(&device)
                    .map(|m| m.name.clone())
                    .filter(|n| !n.is_empty())
            })
            .unwrap_or_else(|| device.short_fingerprint());
        person(device, &name)
    }
}

/// What waits to be written goes now, also when the core goes without a shutdown (a command
/// that ends right after a change).
impl Drop for Inner {
    fn drop(&mut self) {
        save_waiting_friends(self);
        save_waiting_profiles(self);
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn emit(&self, event: CoreEvent) {
        let _ = self.events.send(event);
    }

    /// A join that no longer waits: let in, said no to, or run out.
    fn forget_join(&self, group: GroupId) {
        if let Err(e) = self.store.note_join(group, None) {
            tracing::warn!("cannot forget a join: {e:#}");
        }
    }

    fn save_meta(&self) {
        let meta = self.lock().meta.clone();
        let store = self.store.clone();
        self.writer.soon(writer::File::Meta, move || {
            if let Err(e) = store.save_meta(&meta) {
                tracing::warn!("cannot save the screen times: {e:#}");
            }
        });
    }

    /// Whether a group invitation from `by` may be announced now: at most
    /// [`OFFER_NOTICES`] per friend every [`OFFER_NOTICE_MS`], so a friend cannot ring the
    /// bell without end.
    fn offer_noticed(&self, by: DevicePublicKey, now: u64) -> bool {
        let mut state = self.lock();
        let notices = &mut state.offer_notices;
        for times in notices.values_mut() {
            times.retain(|at| now.saturating_sub(*at) < OFFER_NOTICE_MS);
        }
        notices.retain(|_, times| !times.is_empty());
        let times = notices.entry(by).or_default();
        if times.len() >= OFFER_NOTICES {
            return false;
        }
        times.push(now);
        true
    }

    async fn save_group(&self, group: GroupId) {
        let _in_order = self.saving_groups.lock().await;
        let Ok(Some(log)) = self.node.group(group).await else {
            return;
        };
        #[cfg(test)]
        save_tests::paused(group).await;
        let name = self.lock().joining.get(&group).cloned();
        let name = name.unwrap_or_else(|| log.shared_state().name.clone());
        let store = self.store.clone();
        self.writer.soon(writer::File::Group(group), move || {
            if let Err(e) = store.save_group_log(&log, &name) {
                tracing::warn!("cannot save group {group:?}: {e:#}");
            }
        });
    }
}

fn person(device: DevicePublicKey, name: &str) -> Person {
    Person {
        device,
        name: name.to_string(),
        short: device.short_fingerprint(),
        avatar: Avatar::Preset(device.as_bytes()[0] % PRESET_AVATARS),
    }
}

fn person_of(profile: &Profile) -> Person {
    Person {
        avatar: profile.avatar.clone(),
        ..person(profile.device, &profile.name)
    }
}

impl<'a> Fetching<'a> {
    fn start(inner: &'a Inner, key: (GroupId, [u8; 32])) -> Self {
        let mut state = inner.lock();
        *state.fetching.entry(key).or_default() += 1;
        state.downloads.entry(key).or_insert(0);
        Self { inner, key }
    }
}

impl Drop for Fetching<'_> {
    fn drop(&mut self) {
        let mut state = self.inner.lock();
        let left = state.fetching.get_mut(&self.key).map(|n| {
            *n -= 1;
            *n
        });
        if left.unwrap_or(0) == 0 {
            state.fetching.remove(&self.key);
            state.downloads.remove(&self.key);
        }
    }
}

/// When this computer's firewall begins to let the port in (the person ran the command the
/// app showed), friends and members that could not come in are tried again at once rather
/// than after their next lookup. Off Linux, looking starts other programs, so the watch ends
/// once nothing is in the way.
async fn watch_firewall(inner: Weak<Inner>, port: u16) {
    let mut blocked = firewall_blocks(port).await.is_some();
    loop {
        if !blocked && !cfg!(target_os = "linux") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        let now = firewall_blocks(port).await.is_some();
        let Some(inner) = inner.upgrade() else {
            break;
        };
        if blocked && !now {
            tracing::info!("the firewall lets Fogata in now; trying friends and members again");
            let _ = inner.node.retry_now().await;
        }
        blocked = now;
    }
}

/// [`firewall::blocking_command`] off the async threads: on Windows and macOS it waits for
/// other programs.
async fn firewall_blocks(port: u16) -> Option<String> {
    tokio::task::spawn_blocking(move || firewall::blocking_command(port))
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod save_tests {
    use std::sync::{Arc, Mutex};

    use tokio::sync::Notify;

    use super::*;

    /// A save of this group that waits, once it has the log, until told to go on.
    struct Pause {
        group: GroupId,
        reached: Notify,
        go: Notify,
    }

    static PAUSE: Mutex<Option<Arc<Pause>>> = Mutex::new(None);

    /// Called by [`Inner::save_group`] between asking for the log and handing it over.
    pub(super) async fn paused(group: GroupId) {
        let pause = {
            let mut armed = PAUSE.lock().unwrap();
            match armed.as_ref() {
                Some(p) if p.group == group => armed.take(),
                _ => None,
            }
        };
        if let Some(pause) = pause {
            pause.reached.notify_one();
            pause.go.notified().await;
        }
    }

    /// A save that asked for a group's log before a change, and hands it over only after a
    /// later save handed over the log with the change, does not write the older log over the
    /// newer: the saves hand their logs over in the order they asked for them.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_slow_save_never_writes_an_older_log_over_a_newer_one() {
        let dir = std::env::temp_dir().join(format!("fogata-save-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Core::create_profile(&dir, "Ana", None).unwrap();
        let options = NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, _events) = Core::start(dir.clone(), options).unwrap();
        let inner = core.inner.clone();
        let group = core.create_group("crew", 0, &[]).await.unwrap();
        let mut gif = b"GIF89a".to_vec();
        gif.resize(1024, 1);
        let png = b"\x89PNG\r\n\x1a\nmore".to_vec();
        core.set_group_picture(group, Some(gif)).await.unwrap();
        inner.save_group(group).await;
        inner.writer.then(|| ()).await.unwrap();

        // A save takes the log with the GIF and stops before handing it over.
        let pause = Arc::new(Pause {
            group,
            reached: Notify::new(),
            go: Notify::new(),
        });
        *PAUSE.lock().unwrap() = Some(pause.clone());
        let slow = tokio::spawn({
            let inner = inner.clone();
            async move { inner.save_group(group).await }
        });
        pause.reached.notified().await;
        // The picture changes, and another save takes the log with the PNG.
        core.set_group_picture(group, Some(png.clone()))
            .await
            .unwrap();
        let later = tokio::spawn({
            let inner = inner.clone();
            async move { inner.save_group(group).await }
        });
        // Time for the later save to hand its log over, were it not held behind the slow one.
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        pause.go.notify_one();
        slow.await.unwrap();
        later.await.unwrap();
        inner.writer.then(|| ()).await.unwrap();

        let store = &inner.store;
        let stored = store
            .groups()
            .unwrap()
            .into_iter()
            .find(|g| g.group_id == hex::encode(group.as_bytes()))
            .unwrap();
        let log = store.load_log(&stored).unwrap();
        assert_eq!(log.picture(), Some(png));
        core.shutdown().await.unwrap();
        drop(core);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
