//! What the screens get: plain copies of the core's views.

use fogata_core::{
    ActivityView, ActivityWhat, Barrier, CallEnd, CallPhase, CallView, CoreEvent, FileView,
    FriendView, GroupView, Invitation, InvitationView, JoinAsk, MemberView, Person, Presence,
    RequestView, RoomState, RoomSummary, UndoneKind,
    diagnose::{Cause, Finding, Level, Reach, Topic, Verdict},
};
use fogata_crypto::DevicePublicKey;
use fogata_friends::{Avatar, PRESET_AVATARS, Profile};
use fogata_net::{ChatLine, Path, RoomChange};

#[derive(Clone, Debug)]
pub struct UiAvatar {
    /// One of the preset avatars; behind a photo, the colour while it loads.
    pub preset: u8,
    /// A PNG or JPEG picture instead of the preset.
    pub photo: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct UiPerson {
    pub id: String,
    pub name: String,
    /// Short fingerprint, `7Q4M·K2XR`.
    pub short: String,
    pub avatar: UiAvatar,
}

#[derive(Clone, Debug)]
pub struct UiProfile {
    pub person: UiPerson,
    /// Full fingerprint, `7Q4M K2XR 9PLD 3VHC`.
    pub fingerprint: String,
}

/// How another device is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiPath {
    Lan,
    Direct,
    HolePunched,
    ThroughMember,
}

#[derive(Clone, Debug)]
pub struct UiRoomSummary {
    pub started_ms: i64,
    pub participants: Vec<UiPerson>,
    pub joined: bool,
}

#[derive(Clone, Debug)]
pub struct UiGroup {
    pub id: String,
    pub name: String,
    pub local_name: String,
    pub icon: u8,
    pub members: u32,
    /// Members connected now, this device not included.
    pub online: u32,
    /// This device manages the group: changes it, invites and lets people in.
    pub manager: bool,
    /// Where this device is on the group's list, 0 for the first: it acts on those below.
    pub rank: u32,
    /// Who manages the group: the first `n` on its list, or everyone (`None`).
    pub managers: Option<u8>,
    /// A small PNG or JPEG shown instead of the icon.
    pub picture: Option<Vec<u8>>,
    /// A short name of the group's banner when it has one (see
    /// [`super::app::group_banner`]); it changes with the banner.
    pub banner: Option<String>,
    /// The most members the group takes, and the most people in each voice room.
    pub max_members: u8,
    pub max_in_room: u8,
    /// Every voice room in order, with its room when one is open.
    pub voice_rooms: Vec<UiVoiceRoom>,
    /// The voice room this device is in, or else the first one with people in it.
    pub room: Option<UiRoomSummary>,
    pub last_room_ms: Option<i64>,
    /// Invites made and not used yet, nor cancelled or run out.
    pub open_invites: u32,
    /// The members below this device (device ids), the next one first: those it may move up
    /// or down, remove, and take out of a voice room or keep out.
    pub outranks: Vec<String>,
    /// For a manager, or a member with someone below it, the members kept out of a voice
    /// room now; none otherwise.
    pub kept_out: Option<Vec<UiKeptOut>>,
    /// For a manager, while the group waits to go up to the protection this version gives:
    /// the members whose Fogata must be updated first. None otherwise.
    pub level_behind: Option<Vec<UiPerson>>,
    /// For a manager, while the group waits: it has waited long enough that it may be
    /// raised anyway ([`super::app::raise_group_level_anyway`]), leaving `level_behind` to
    /// update before they take part again.
    pub raise_anyway: bool,
    /// A manager raised the group to rules this version of Fogata does not know: until it
    /// is updated, this device stays with the group as it was then and sees none of its
    /// changes since. Said again after a restart, until Fogata is updated.
    pub needs_update: bool,
}

/// A voice room of a group: its own people, chat and files.
#[derive(Clone, Debug)]
pub struct UiVoiceRoom {
    pub id: String,
    /// Empty for the first room until someone names it.
    pub name: String,
    pub room: Option<UiRoomSummary>,
    /// Only these members (device ids) and the managers may enter; everyone when none.
    pub only: Option<Vec<String>>,
    /// Why this device may not enter now, if it may not.
    pub barrier: Option<UiBarrier>,
}

/// What a member did in a group, for one line of its activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiActivityKind {
    /// Started the group, called `name`.
    Created,
    /// Renamed it `name`.
    Renamed,
    Icon,
    /// Set the group's picture; with `taken_away`, took it away.
    Picture,
    /// Set the group's banner; with `taken_away`, took it away.
    Banner,
    /// Added `member`.
    Added,
    /// Joined with an invite of `member`.
    Joined,
    /// Let in `member`, who joined with an invite.
    LetIn,
    /// Removed `member`.
    Removed,
    Left,
    /// Moved `member` up to just below itself, to place `number` (0 for the first).
    MovedUp,
    /// Moved `member` to the end.
    MovedDown,
    /// Who manages: the first `number`, or everyone without one.
    Managers,
    /// The most members (`number`) and people in a voice room (`number2`).
    Limits,
    /// Opened voice room `name`.
    RoomOpened,
    /// Renamed voice room `from` to `name`.
    RoomRenamed,
    /// Closed voice room `name`.
    RoomClosed,
    Invited,
    Revoked,
    /// Who may enter voice room `name`: everyone without `members`, or only them.
    RoomAccess,
    /// Kept `member` out of voice room `name` until `until_ms`, or until let back.
    KeptOut,
    /// Let `member` back into voice room `name`.
    LetBack,
}

/// How something done was undone: its author was removed, left or was moved to the end at
/// the same time, without having seen it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiUndoneKind {
    Removed,
    Left,
    MovedDown,
}

/// One line of a group's activity.
#[derive(Clone, Debug)]
pub struct UiActivity {
    pub kind: UiActivityKind,
    pub by: UiPerson,
    pub member: Option<UiPerson>,
    pub members: Option<Vec<UiPerson>>,
    pub name: Option<String>,
    pub from: Option<String>,
    pub number: Option<u32>,
    pub number2: Option<u32>,
    pub until_ms: Option<i64>,
    pub taken_away: bool,
    /// When, by its author's clock.
    pub at_ms: i64,
    pub undone: Option<UiUndoneKind>,
    /// Who undid it (none when its author left).
    pub undone_by: Option<UiPerson>,
    /// Seen arriving on this device, not told by the log (the banner and the picture).
    pub seen_here: bool,
}

/// Lines of a group's activity, the newest first, and whether older ones follow.
#[derive(Clone, Debug)]
pub struct UiActivityPage {
    pub lines: Vec<UiActivity>,
    pub more: bool,
}

pub(crate) fn activity(a: ActivityView) -> UiActivity {
    let mut line = UiActivity {
        kind: UiActivityKind::Icon,
        by: person(&a.by),
        member: None,
        members: None,
        name: None,
        from: None,
        number: None,
        number2: None,
        until_ms: None,
        taken_away: false,
        at_ms: ms(a.at_ms),
        undone: a.undone.as_ref().map(|(how, _)| match how {
            UndoneKind::Removed => UiUndoneKind::Removed,
            UndoneKind::Left => UiUndoneKind::Left,
            UndoneKind::MovedDown => UiUndoneKind::MovedDown,
        }),
        undone_by: a
            .undone
            .as_ref()
            .and_then(|(_, by)| by.as_ref().map(person)),
        seen_here: a.seen_here,
    };
    let number = |n: usize| Some(u32::try_from(n).unwrap_or(u32::MAX));
    line.kind = match a.what {
        ActivityWhat::Created { name } => {
            line.name = Some(name);
            UiActivityKind::Created
        }
        ActivityWhat::Renamed { name } => {
            line.name = Some(name);
            UiActivityKind::Renamed
        }
        ActivityWhat::Icon => UiActivityKind::Icon,
        ActivityWhat::Picture { taken_away } => {
            line.taken_away = taken_away;
            UiActivityKind::Picture
        }
        ActivityWhat::Banner { taken_away } => {
            line.taken_away = taken_away;
            UiActivityKind::Banner
        }
        ActivityWhat::Added { member } => {
            line.member = Some(person(&member));
            UiActivityKind::Added
        }
        ActivityWhat::Joined { invited_by } => {
            line.member = Some(person(&invited_by));
            UiActivityKind::Joined
        }
        ActivityWhat::LetIn { member } => {
            line.member = Some(person(&member));
            UiActivityKind::LetIn
        }
        ActivityWhat::Removed { member } => {
            line.member = Some(person(&member));
            UiActivityKind::Removed
        }
        ActivityWhat::Left => UiActivityKind::Left,
        ActivityWhat::MovedUp { member, place } => {
            line.member = Some(person(&member));
            line.number = number(place);
            UiActivityKind::MovedUp
        }
        ActivityWhat::MovedDown { member } => {
            line.member = Some(person(&member));
            UiActivityKind::MovedDown
        }
        ActivityWhat::Managers { first } => {
            line.number = first.map(u32::from);
            UiActivityKind::Managers
        }
        ActivityWhat::Limits { members, room } => {
            line.number = Some(u32::from(members));
            line.number2 = Some(u32::from(room));
            UiActivityKind::Limits
        }
        ActivityWhat::RoomOpened { name } => {
            line.name = Some(name);
            UiActivityKind::RoomOpened
        }
        ActivityWhat::RoomRenamed { from, name } => {
            line.from = Some(from);
            line.name = Some(name);
            UiActivityKind::RoomRenamed
        }
        ActivityWhat::RoomClosed { name } => {
            line.name = Some(name);
            UiActivityKind::RoomClosed
        }
        ActivityWhat::Invited => UiActivityKind::Invited,
        ActivityWhat::Revoked => UiActivityKind::Revoked,
        ActivityWhat::RoomAccess { room, members } => {
            line.name = Some(room);
            line.members = members.map(|m| m.iter().map(person).collect());
            UiActivityKind::RoomAccess
        }
        ActivityWhat::KeptOut {
            room,
            member,
            until_ms,
        } => {
            line.name = Some(room);
            line.member = Some(person(&member));
            line.until_ms = until_ms.map(ms);
            UiActivityKind::KeptOut
        }
        ActivityWhat::LetBack { room, member } => {
            line.name = Some(room);
            line.member = Some(person(&member));
            UiActivityKind::LetBack
        }
    };
    line
}

/// Why this device may not enter a voice room, or why it left one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiBarrierKind {
    /// Someone above it took it out of the room; it may come back at once.
    Removed,
    /// Someone above it keeps it out until `until_ms`.
    TimedOut,
    /// Someone above it keeps it out until let back.
    UntilLetBack,
    /// Only some members may enter, and it is not one of them.
    NotChosen,
}

#[derive(Clone, Debug)]
pub struct UiBarrier {
    pub kind: UiBarrierKind,
    /// The manager who did it (a device id), when one did.
    pub by: Option<String>,
    /// When it started and ends, by that manager's clock, for a time out.
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
}

/// A member a manager kept out of a voice room.
#[derive(Clone, Debug)]
pub struct UiKeptOut {
    pub voice: String,
    pub member: UiPerson,
    pub by: UiPerson,
    /// When it may come back, by that manager's clock; none until a manager lets it back.
    pub until_ms: Option<i64>,
}

/// Someone who showed one of this manager's invites and waits to be let in.
#[derive(Clone, Debug)]
pub struct UiJoinAsk {
    pub group_id: String,
    pub group_name: String,
    pub device_id: String,
    /// The name it asks to join under.
    pub name: String,
    /// Its fingerprint, to tell it from someone else with the same name.
    pub fingerprint: String,
    pub since_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UiMember {
    pub person: UiPerson,
    /// Where it is on the group's list, 0 for the first.
    pub rank: u32,
    /// It manages the group.
    pub manager: bool,
    pub me: bool,
    /// How it is reached, when connected.
    pub path: Option<UiPath>,
    /// Connected, and nobody used its computer for a while.
    pub away: bool,
    /// The member of the same group whose relay slot carries it, when it comes through one
    /// that is known (a device id).
    pub through: Option<String>,
    pub waiting_for_key: bool,
    /// Full fingerprint, `7Q4M K2XR 9PLD 3VHC`, to tell it from someone else with the same
    /// short one.
    pub fingerprint: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiPresence {
    Offline,
    Online,
    /// Online, but nobody used the friend's computer for a while.
    Away,
    InRoom,
}

#[derive(Clone, Debug)]
pub struct UiFriend {
    pub person: UiPerson,
    pub fingerprint: String,
    pub presence: UiPresence,
    pub last_seen_ms: Option<i64>,
    /// The group whose room the friend is in, when this device is a member too.
    pub room_group: Option<String>,
    pub room_group_name: Option<String>,
    pub path: Option<UiPath>,
    pub delivered: bool,
    /// The friend's bio, once it sent one; empty when it shows none.
    pub bio: Option<String>,
}

/// What an animation moves in place of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAnimated {
    /// The profile's picture.
    Photo,
    /// The banner's picture.
    Banner,
}

/// An animation (a GIF or WebP) and the still picture it moves in place of.
#[derive(Clone, Debug)]
pub struct UiAnimation {
    pub still: Vec<u8>,
    pub picture: Vec<u8>,
}

/// A banner: the colour (0xAARRGGBB) and the picture over it.
#[derive(Clone, Debug)]
pub struct UiBanner {
    pub color: u32,
    pub picture: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct UiRequest {
    pub person: UiPerson,
    pub fingerprint: String,
    pub since_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UiRequests {
    /// Requests to answer.
    pub incoming: Vec<UiRequest>,
    /// Requests this device sent.
    pub outgoing: Vec<UiRequest>,
}

#[derive(Clone, Debug)]
pub struct UiChatLine {
    pub id: String,
    pub sender: String,
    pub nick: String,
    pub text: String,
    pub timestamp_ms: i64,
    pub mine: bool,
}

#[derive(Clone, Debug)]
pub struct UiFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub shared_by: UiPerson,
    /// When it was listed, by the clock of whoever listed it, as a chat line's time.
    pub shared_ms: i64,
    pub holders: Vec<UiPerson>,
    /// This device shares it.
    pub mine: bool,
    /// Bytes received so far while this device downloads it.
    pub downloading: Option<i64>,
    /// Times this device sent it to someone.
    pub sent: u32,
    /// Where this device keeps it, once shared from here or downloaded.
    pub path: Option<String>,
    /// For a video, what it looks like before it is fetched.
    pub video: Option<UiVideo>,
}

/// A video's first picture (a small JPEG), its size and how long it lasts.
#[derive(Clone, Debug)]
pub struct UiVideo {
    pub thumb: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub duration_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UiParticipant {
    pub person: UiPerson,
    pub away: bool,
    pub me: bool,
}

#[derive(Clone, Debug)]
pub struct UiRoom {
    pub started_ms: i64,
    pub joined: bool,
    pub participants: Vec<UiParticipant>,
    pub chat: Vec<UiChatLine>,
    pub files: Vec<UiFile>,
}

#[derive(Clone, Debug)]
pub struct UiInvitation {
    pub text: String,
    pub expires_ms: i64,
}

/// A group a friend added this device to, waiting for the person to join or decline.
#[derive(Clone, Debug)]
pub struct UiGroupInvite {
    pub id: String,
    pub name: String,
    pub by: UiPerson,
    pub members: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiLevel {
    Good,
    Note,
    Bad,
}

/// What a network finding is about; the screens word it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiTopic {
    Offline,
    Internet,
    Dns,
    UdpWorks,
    UdpBlocked,
    CarrierNat,
    PublicIpv4,
    HardNat,
    EasyNat,
    UnknownRouter,
    NoIpv6,
    Ipv6Works,
    Ipv6Unreached,
    LanDevices,
    Firewall,
    SummaryNoUdp,
    SummaryPublic,
    SummaryEasyNat,
    SummaryHardNatWithIpv6,
    SummaryHardNat,
    SummaryUnknown,
}

#[derive(Clone, Debug)]
pub struct UiFinding {
    pub level: UiLevel,
    pub topic: UiTopic,
    /// DHT peers for `Internet`, devices for `LanDevices`.
    pub count: u32,
    /// The finding in English.
    pub text: String,
}

/// Something on this computer's side that keeps friends and members from reaching it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCause {
    /// Nothing on the internet answers.
    Offline,
    /// UDP does not get out of this network.
    UdpBlocked,
    /// The router gives a new public port to every destination (a symmetric NAT).
    SymmetricNat,
    /// No public IPv6 address either (said only with `SymmetricNat`).
    NoIpv6,
}

/// What this computer's side tells about reaching friends and members, from what the
/// running node has seen; this computer's own firewall is read apart (`firewall_command`).
#[derive(Clone, Debug)]
pub struct UiReach {
    /// The most telling first; empty when nothing is known to be in the way.
    pub causes: Vec<UiCause>,
    /// Nothing on this side is in the way, as far as the node saw: UDP gets out and the
    /// router, if any, can be opened from both sides.
    pub fine: bool,
}

pub(crate) fn reach(r: &Reach) -> UiReach {
    UiReach {
        causes: r
            .causes
            .iter()
            .map(|c| match c {
                Cause::Offline => UiCause::Offline,
                Cause::UdpBlocked => UiCause::UdpBlocked,
                Cause::SymmetricNat => UiCause::SymmetricNat,
                Cause::NoIpv6 => UiCause::NoIpv6,
            })
            .collect(),
        fine: r.fine,
    }
}

/// A short sound for what happens in a room, a call or the bell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiSound {
    /// Someone came into the room, or this device did.
    Join,
    /// Someone left the room, or this device did.
    Leave,
    /// Someone wrote in the chat.
    Message,
    /// This device muted its microphone.
    Mute,
    /// This device unmuted its microphone.
    Unmute,
    /// Someone started sharing a camera, a screen or a window.
    ShareStart,
    /// Someone stopped sharing.
    ShareStop,
    /// Someone started watching a shared screen: this device, or someone watching this
    /// device's screen.
    WatchStart,
    /// Someone stopped watching a shared screen.
    WatchStop,
    /// A friend rings this device; played again while it rings.
    Ring,
    /// This device rings a friend; played again while it rings.
    Ringback,
    /// Something new came to the bell: a missed call, a friend request, someone asking to
    /// join, a group a friend added this device to.
    Notice,
    /// A direct call ended: hung up on either side, or it did not go through.
    CallEnded,
}

/// Where the WebRTC engine finds this device's media bridge.
#[derive(Clone, Debug)]
pub struct UiMediaBridge {
    /// The address to reach it at.
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

/// A QR code: `size` × `size` modules, row by row, `true` for dark.
#[derive(Clone, Debug)]
pub struct UiQr {
    pub size: u32,
    pub dark: Vec<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiRoomChangeKind {
    Opened,
    Joined,
    Left,
    Away,
    Back,
    Chat,
    FileShared,
    FileGone,
    Closed,
}

#[derive(Clone, Debug)]
pub struct UiRoomChange {
    pub kind: UiRoomChangeKind,
    pub device: Option<String>,
    pub nick: Option<String>,
    /// For `Left`: the participant left on purpose rather than going quiet.
    pub on_purpose: bool,
    pub chat: Option<UiChatLine>,
    pub file_id: Option<String>,
    pub file_name: Option<String>,
    pub file_size: i64,
    pub started_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiEventKind {
    /// Groups, their members online or their rooms changed (home screen).
    GroupsChanged,
    /// Friends, requests or presence changed (friends screen).
    FriendsChanged,
    /// Friends and members see this device as away now: nobody used the computer for a
    /// while.
    Away,
    /// They no longer see it as away.
    NotAway,
    Room,
    MemberConnected,
    MemberDisconnected,
    FriendRequest,
    FriendRequestSent,
    FriendRequestFailed,
    FriendAdded,
    FriendOnline,
    FriendOffline,
    /// An invite to a group was accepted.
    Joined,
    /// This device left a group or was removed from it.
    Removed,
    /// A friend added this device to a group; the invitation waits for an answer.
    GroupInvited,
    Download,
    DownloadDone,
    DownloadFailed,
    /// Other peers see this device at the address in `text`.
    Address,
    /// The router changes the public port for each destination.
    HardNat,
    /// A call signal from `device` in `group`; the payload is in `text`.
    Signal,
    /// Call signals from `device` in `group` were dropped on their way here: the call with
    /// it is to agree anew. No signal of `device` comes between the last one kept and this.
    SignalsLost,
    Info,
    /// Something to show the person: `text`.
    Notice,
    /// A friend (`device`) rings this device; the call is `group`.
    CallIncoming,
    /// The friend this device rang answered the call `group`.
    CallAccepted,
    /// The call `group` with `device` ended, for `call_end`.
    CallEnded,
    /// A friend (`device`) rung into the call `group` did not come in, for `call_end`.
    CallAddEnded,
    /// Calls rang, were answered or ended.
    CallsChanged,
    /// Someone (`device`, called `name`) showed an invite of this manager to `group` and
    /// waits to be let in.
    JoinAsked,
    /// Someone waiting to be let in is not waiting any more.
    JoinAsksChanged,
    /// This device waits for a manager of `group` (`name`) to let it in.
    JoinWaiting,
    /// A manager of `group` (`name`) said no to this device joining.
    JoinDeclined,
    /// The direct chat with `device` changed.
    Direct,
    /// `device` wrote in the direct chat; `text` is the message's first line.
    DirectMessage,
    /// An attachment (`file`) of the chat with `device` is coming: `received` of `total`.
    DirectDownload,
    /// An attachment (`file`) of the chat with `device` did not come, for `text`.
    DirectDownloadFailed,
    /// `device` is typing now: in the chat of the voice room `voice` of `group`, or, with
    /// no `group`, in the direct chat with that friend. Never kept; shown for a few seconds
    /// after the last one.
    Typing,
    /// `device` stopped typing there.
    StoppedTyping,
    /// This device's identity is open on another computer too; `text` says so in words.
    OtherSession,
    /// This device left voice room `voice` of `group`, which it was in: a manager
    /// (`device`) took it out of it, or keeps it out (`barrier`).
    TakenOut,
    /// The network part stopped by itself after internal errors; `text` says so in words.
    /// Nothing reaches this device or leaves it until the application starts again.
    Stopped,
    /// A friend or member (`device`) changed the animation of its picture or banner
    /// ([`super::app::animation`]).
    Animation,
    /// `device` (called `text`) declined this device's invitation to `group` (`name`).
    InvitationDeclined,
    /// Where updates stand changed ([`super::update::update_status`]).
    UpdateChanged,
    /// Someone (`device`, called `name`) asked to be let into `group` with an invite or as
    /// a friend added to it, but must update Fogata first.
    MustUpdateToJoin,
    /// The group this device is joining (`group`, called `name`) needs a newer Fogata.
    UpdateToJoin,
}

/// Where a direct call stands here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCallPhase {
    /// This device rings the friend.
    Ringing,
    /// The friend rings this device.
    Incoming,
    Connected,
}

/// How this device's secrets are kept on this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAtRest {
    /// Sealed with a key the system keychain keeps.
    Sealed,
    /// Being sealed with a key the system keychain keeps, and not all of it is yet (a full
    /// disk, say). Sealing goes on at each start.
    Unfinished,
    /// Unencrypted: no system keychain kept a key. Tried again at each start.
    NoKeychain,
    /// Unencrypted, as a copy run for tests is set to leave them.
    Plain,
}

/// Why a direct call ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCallEnd {
    NoAnswer,
    Declined,
    Busy,
    Unreachable,
    Cancelled,
    Missed,
    /// The friend rang while this device was in another call.
    MissedInCall,
    /// The friend rang while this device appeared offline.
    MissedOffline,
    /// This identity open on another computer answered or declined first.
    Elsewhere,
    HungUp,
    Ended,
    /// The friend's Fogata is too old for a call of more than two.
    Outdated,
    /// Everyone else left, and nobody came back in a minute.
    Alone,
}

/// A direct call with a friend; its room is the group `group_id`.
#[derive(Clone, Debug)]
pub struct UiCall {
    pub group_id: String,
    pub with: UiPerson,
    pub outgoing: bool,
    pub phase: UiCallPhase,
    /// When it started ringing, or when it was answered.
    pub since_ms: i64,
    /// Everyone else in the call's group.
    pub members: Vec<UiPerson>,
    /// Friends this device rings into the call now.
    pub adding: Vec<UiPerson>,
    /// Those who left in the last minute and may come back.
    pub away: Vec<UiPerson>,
    /// Since when this device is the only one left, waiting a minute for the others.
    pub alone_since_ms: Option<i64>,
    /// Why this device cannot ring anyone into the call now; none when it can.
    pub add_blocked: Option<String>,
}

pub(crate) fn call(view: &CallView) -> UiCall {
    UiCall {
        group_id: hex_id(view.group.as_bytes()),
        with: person(&view.with),
        outgoing: view.outgoing,
        phase: match view.phase {
            CallPhase::Ringing => UiCallPhase::Ringing,
            CallPhase::Incoming => UiCallPhase::Incoming,
            CallPhase::Connected => UiCallPhase::Connected,
        },
        since_ms: ms(view.since_ms),
        members: view.members.iter().map(person).collect(),
        adding: view.adding.iter().map(person).collect(),
        away: view.away.iter().map(person).collect(),
        alone_since_ms: view.alone_since_ms.map(ms),
        add_blocked: view.add_blocked.clone(),
    }
}

fn typing_kind(typing: bool) -> UiEventKind {
    if typing {
        UiEventKind::Typing
    } else {
        UiEventKind::StoppedTyping
    }
}

fn call_end(reason: CallEnd) -> UiCallEnd {
    match reason {
        CallEnd::NoAnswer => UiCallEnd::NoAnswer,
        CallEnd::Declined => UiCallEnd::Declined,
        CallEnd::Busy => UiCallEnd::Busy,
        CallEnd::Unreachable => UiCallEnd::Unreachable,
        CallEnd::Cancelled => UiCallEnd::Cancelled,
        CallEnd::Missed => UiCallEnd::Missed,
        CallEnd::MissedInCall => UiCallEnd::MissedInCall,
        CallEnd::MissedOffline => UiCallEnd::MissedOffline,
        CallEnd::Elsewhere => UiCallEnd::Elsewhere,
        CallEnd::HungUp => UiCallEnd::HungUp,
        CallEnd::Ended => UiCallEnd::Ended,
        CallEnd::Outdated => UiCallEnd::Outdated,
        CallEnd::Alone => UiCallEnd::Alone,
    }
}

/// Something changed; the screens look again. Only the fields that fit the kind are set.
#[derive(Clone, Debug)]
pub struct UiEvent {
    pub kind: UiEventKind,
    pub group: Option<String>,
    pub device: Option<String>,
    /// A person's or a group's name.
    pub name: Option<String>,
    /// A reason, a note, an address or a saved file's path.
    pub text: Option<String>,
    pub path: Option<UiPath>,
    /// With a path through a member: that member of the same group, when known.
    pub through: Option<String>,
    pub room: Option<UiRoomChange>,
    /// The voice room of a room change.
    pub voice: Option<String>,
    pub file: Option<String>,
    pub received: i64,
    pub total: i64,
    pub call_end: Option<UiCallEnd>,
    /// For [`UiEventKind::TakenOut`]: why.
    pub barrier: Option<UiBarrier>,
}

pub(crate) fn hex_id(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

pub(crate) fn ms(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn avatar(avatar: &Avatar, device: &DevicePublicKey) -> UiAvatar {
    match avatar {
        Avatar::Preset(n) => UiAvatar {
            preset: *n,
            photo: None,
        },
        Avatar::Photo(bytes) => UiAvatar {
            preset: device.as_bytes()[0] % PRESET_AVATARS,
            photo: Some(bytes.clone()),
        },
    }
}

pub(crate) fn person(p: &Person) -> UiPerson {
    UiPerson {
        id: hex_id(p.device.as_bytes()),
        name: p.name.clone(),
        short: p.short.clone(),
        avatar: avatar(&p.avatar, &p.device),
    }
}

pub(crate) fn profile(p: &Profile) -> UiProfile {
    UiProfile {
        person: UiPerson {
            id: hex_id(p.device.as_bytes()),
            name: p.name.clone(),
            short: p.device.short_fingerprint(),
            avatar: avatar(&p.avatar, &p.device),
        },
        fingerprint: p.device.fingerprint_grouped(),
    }
}

fn path(p: &Path) -> UiPath {
    match p {
        Path::Lan => UiPath::Lan,
        Path::Direct => UiPath::Direct,
        Path::HolePunched => UiPath::HolePunched,
        Path::ThroughMember { .. } => UiPath::ThroughMember,
    }
}

/// The member whose relay slot carries `p`, when the core names one.
fn through(p: &Path) -> Option<String> {
    match p {
        Path::ThroughMember {
            member: Some(device),
            ..
        } => Some(hex_id(device.as_bytes())),
        _ => None,
    }
}

fn barrier(b: &Barrier) -> UiBarrier {
    match b {
        Barrier::TimedOut(kept) => UiBarrier {
            kind: if kept.until_ms.is_some() {
                UiBarrierKind::TimedOut
            } else {
                UiBarrierKind::UntilLetBack
            },
            by: Some(hex_id(kept.by.as_bytes())),
            since_ms: Some(ms(kept.since_ms)),
            until_ms: kept.until_ms.map(ms),
        },
        Barrier::NotChosen => UiBarrier {
            kind: UiBarrierKind::NotChosen,
            by: None,
            since_ms: None,
            until_ms: None,
        },
    }
}

pub(crate) fn group(g: GroupView) -> UiGroup {
    UiGroup {
        id: hex_id(g.id.as_bytes()),
        name: g.name,
        local_name: g.local_name,
        icon: g.icon,
        members: g.members as u32,
        online: g.online as u32,
        manager: g.manager,
        rank: u32::try_from(g.rank).unwrap_or(u32::MAX),
        managers: g.managers,
        picture: g.picture,
        banner: g.banner.map(hex::encode),
        max_members: g.max_members,
        max_in_room: g.max_in_room,
        voice_rooms: g
            .voice_rooms
            .into_iter()
            .map(|v| UiVoiceRoom {
                id: hex_id(&v.id),
                name: v.name,
                room: v.room.map(room_summary),
                only: v
                    .only
                    .map(|only| only.iter().map(|d| hex_id(d.as_bytes())).collect()),
                barrier: v.barrier.as_ref().map(barrier),
            })
            .collect(),
        room: g.room.map(room_summary),
        last_room_ms: g.last_room_ms.map(ms),
        open_invites: g.open_invites as u32,
        outranks: g.outranks.iter().map(|d| hex_id(d.as_bytes())).collect(),
        kept_out: (g.manager || !g.outranks.is_empty()).then(|| {
            g.kept_out
                .iter()
                .map(|k| UiKeptOut {
                    voice: hex_id(&k.voice),
                    member: person(&k.member),
                    by: person(&k.by),
                    until_ms: k.until_ms.map(ms),
                })
                .collect()
        }),
        level_behind: (!g.level_behind.is_empty())
            .then(|| g.level_behind.iter().map(person).collect()),
        raise_anyway: g.raise_anyway,
        needs_update: g.level_ahead.is_some(),
    }
}

fn room_summary(r: RoomSummary) -> UiRoomSummary {
    UiRoomSummary {
        started_ms: ms(r.started_ms),
        participants: r.participants.iter().map(person).collect(),
        joined: r.joined,
    }
}

pub(crate) fn join_ask(a: JoinAsk) -> UiJoinAsk {
    UiJoinAsk {
        group_id: hex_id(a.group.as_bytes()),
        group_name: a.group_name,
        device_id: hex_id(a.device.as_bytes()),
        name: a.name,
        fingerprint: a.device.fingerprint_grouped(),
        since_ms: ms(a.since_ms),
    }
}

pub(crate) fn member(m: MemberView) -> UiMember {
    UiMember {
        person: person(&m.person),
        rank: u32::try_from(m.rank).unwrap_or(u32::MAX),
        manager: m.manager,
        me: m.me,
        path: m.online.as_ref().map(path),
        away: m.away,
        through: m.online.as_ref().and_then(through),
        waiting_for_key: m.waiting_for_key,
        fingerprint: m.person.device.fingerprint_grouped(),
    }
}

pub(crate) fn friend(f: FriendView) -> UiFriend {
    let (presence, last_seen_ms, room) = match f.presence {
        Presence::Offline { last_seen_ms } => (UiPresence::Offline, last_seen_ms.map(ms), None),
        Presence::Online => (UiPresence::Online, None, None),
        Presence::Away => (UiPresence::Away, None, None),
        Presence::InRoom { group } => (UiPresence::InRoom, None, group),
    };
    UiFriend {
        person: person(&f.person),
        fingerprint: f.fingerprint,
        presence,
        last_seen_ms,
        room_group: room.as_ref().map(|(id, _)| hex_id(id.as_bytes())),
        room_group_name: room.map(|(_, name)| name),
        path: f.path.as_ref().map(path),
        delivered: f.delivered,
        bio: f.bio,
    }
}

pub(crate) fn request(r: RequestView) -> UiRequest {
    UiRequest {
        person: person(&r.person),
        fingerprint: r.fingerprint,
        since_ms: ms(r.since_ms),
    }
}

pub(crate) fn group_invite(i: InvitationView) -> UiGroupInvite {
    UiGroupInvite {
        id: hex_id(i.id.as_bytes()),
        name: i.name,
        by: person(&i.by),
        members: i.members as u32,
    }
}

pub(crate) fn invitation(i: Invitation) -> UiInvitation {
    UiInvitation {
        text: i.text,
        expires_ms: ms(i.expires_ms),
    }
}

pub(crate) fn chat_line(line: &ChatLine, me: &DevicePublicKey) -> UiChatLine {
    UiChatLine {
        id: hex_id(&line.message_id),
        sender: hex_id(line.sender.as_bytes()),
        nick: line.nick.clone(),
        text: line.text.clone(),
        timestamp_ms: ms(line.timestamp_ms),
        mine: line.sender == *me,
    }
}

fn file(f: &FileView) -> UiFile {
    UiFile {
        id: hex_id(&f.info.id),
        name: f.info.name.clone(),
        size: ms(f.info.size),
        shared_by: person(&f.shared_by),
        shared_ms: ms(f.shared_ms),
        holders: f.holders.iter().map(person).collect(),
        mine: f.mine,
        downloading: f.downloading.map(ms),
        sent: f.sent,
        path: f.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        video: f.info.video.as_ref().map(|v| UiVideo {
            thumb: v.thumb.clone(),
            width: v.width,
            height: v.height,
            duration_ms: ms(v.duration_ms),
        }),
    }
}

pub(crate) fn room(r: RoomState, me: &DevicePublicKey) -> UiRoom {
    UiRoom {
        started_ms: ms(r.started_ms),
        joined: r.joined,
        participants: r
            .participants
            .iter()
            .map(|(p, away)| UiParticipant {
                person: person(p),
                away: *away,
                me: p.device == *me,
            })
            .collect(),
        chat: r.chat.iter().map(|l| chat_line(l, me)).collect(),
        files: r.files.iter().map(file).collect(),
    }
}

pub(crate) fn finding(f: &Finding) -> UiFinding {
    let (topic, count) = match f.topic {
        Topic::Offline => (UiTopic::Offline, 0),
        Topic::Internet(n) => (UiTopic::Internet, n),
        Topic::Dns => (UiTopic::Dns, 0),
        Topic::UdpWorks => (UiTopic::UdpWorks, 0),
        Topic::UdpBlocked => (UiTopic::UdpBlocked, 0),
        Topic::CarrierNat => (UiTopic::CarrierNat, 0),
        Topic::PublicIpv4 => (UiTopic::PublicIpv4, 0),
        Topic::HardNat => (UiTopic::HardNat, 0),
        Topic::EasyNat => (UiTopic::EasyNat, 0),
        Topic::UnknownRouter => (UiTopic::UnknownRouter, 0),
        Topic::NoIpv6 => (UiTopic::NoIpv6, 0),
        Topic::Ipv6Works => (UiTopic::Ipv6Works, 0),
        Topic::Ipv6Unreached => (UiTopic::Ipv6Unreached, 0),
        Topic::LanDevices(n) => (UiTopic::LanDevices, n),
        Topic::Firewall => (UiTopic::Firewall, 0),
        Topic::Summary(Verdict::NoUdp) => (UiTopic::SummaryNoUdp, 0),
        Topic::Summary(Verdict::Public) => (UiTopic::SummaryPublic, 0),
        Topic::Summary(Verdict::EasyNat) => (UiTopic::SummaryEasyNat, 0),
        Topic::Summary(Verdict::HardNatWithIpv6) => (UiTopic::SummaryHardNatWithIpv6, 0),
        Topic::Summary(Verdict::HardNat) => (UiTopic::SummaryHardNat, 0),
        Topic::Summary(Verdict::Unknown) => (UiTopic::SummaryUnknown, 0),
    };
    UiFinding {
        level: match f.level {
            Level::Good => UiLevel::Good,
            Level::Note => UiLevel::Note,
            Level::Bad => UiLevel::Bad,
        },
        topic,
        count: count as u32,
        text: f.text.clone(),
    }
}

fn room_change(change: RoomChange, me: &DevicePublicKey) -> UiRoomChange {
    let mut out = UiRoomChange {
        kind: UiRoomChangeKind::Closed,
        device: None,
        nick: None,
        on_purpose: false,
        chat: None,
        file_id: None,
        file_name: None,
        file_size: 0,
        started_ms: 0,
    };
    let device = |d: DevicePublicKey| Some(hex_id(d.as_bytes()));
    match change {
        RoomChange::Opened { started_ms } => {
            out.kind = UiRoomChangeKind::Opened;
            out.started_ms = ms(started_ms);
        }
        RoomChange::Joined { device: d, nick } => {
            out.kind = UiRoomChangeKind::Joined;
            out.device = device(d);
            out.nick = Some(nick);
        }
        RoomChange::Left {
            device: d,
            on_purpose,
        } => {
            out.kind = UiRoomChangeKind::Left;
            out.device = device(d);
            out.on_purpose = on_purpose;
        }
        RoomChange::Away { device: d } => {
            out.kind = UiRoomChangeKind::Away;
            out.device = device(d);
        }
        RoomChange::Back { device: d } => {
            out.kind = UiRoomChangeKind::Back;
            out.device = device(d);
        }
        RoomChange::Chat(line) => {
            out.kind = UiRoomChangeKind::Chat;
            out.device = device(line.sender);
            out.chat = Some(chat_line(&line, me));
        }
        RoomChange::FileShared { file, by } => {
            out.kind = UiRoomChangeKind::FileShared;
            out.device = device(by);
            out.file_id = Some(hex_id(&file.id));
            out.file_name = Some(file.name);
            out.file_size = ms(file.size);
        }
        RoomChange::FileGone { file } => {
            out.kind = UiRoomChangeKind::FileGone;
            out.file_id = Some(hex_id(&file.id));
            out.file_name = Some(file.name);
            out.file_size = ms(file.size);
        }
        RoomChange::Closed => {}
    }
    out
}

pub(crate) fn event(event: CoreEvent, me: &DevicePublicKey) -> UiEvent {
    let mut out = UiEvent {
        kind: UiEventKind::Info,
        group: None,
        device: None,
        name: None,
        text: None,
        path: None,
        through: None,
        room: None,
        voice: None,
        file: None,
        received: 0,
        total: 0,
        call_end: None,
        barrier: None,
    };
    let id = |bytes: &[u8]| Some(hex_id(bytes));
    match event {
        CoreEvent::GroupsChanged => out.kind = UiEventKind::GroupsChanged,
        CoreEvent::FriendsChanged => out.kind = UiEventKind::FriendsChanged,
        CoreEvent::Away { away: true } => out.kind = UiEventKind::Away,
        CoreEvent::Away { away: false } => out.kind = UiEventKind::NotAway,
        CoreEvent::Room {
            group,
            voice,
            change,
        } => {
            out.kind = UiEventKind::Room;
            out.group = id(group.as_bytes());
            out.voice = Some(hex_id(&voice));
            out.room = Some(room_change(change, me));
        }
        CoreEvent::JoinAsked {
            group,
            device,
            name,
        } => {
            out.kind = UiEventKind::JoinAsked;
            out.group = id(group.as_bytes());
            out.device = id(device.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::JoinAsksChanged => out.kind = UiEventKind::JoinAsksChanged,
        CoreEvent::JoinWaiting { group, name } => {
            out.kind = UiEventKind::JoinWaiting;
            out.group = id(group.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::JoinDeclined { group, name } => {
            out.kind = UiEventKind::JoinDeclined;
            out.group = id(group.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::MemberConnected {
            group,
            device,
            path: p,
        } => {
            out.kind = UiEventKind::MemberConnected;
            out.group = id(group.as_bytes());
            out.device = id(device.as_bytes());
            out.path = Some(path(&p));
            out.through = through(&p);
        }
        CoreEvent::MemberDisconnected { group, device } => {
            out.kind = UiEventKind::MemberDisconnected;
            out.group = id(group.as_bytes());
            out.device = id(device.as_bytes());
        }
        CoreEvent::Animation { device } => {
            out.kind = UiEventKind::Animation;
            out.device = id(device.as_bytes());
        }
        CoreEvent::FriendRequest { profile } => {
            out.kind = UiEventKind::FriendRequest;
            out.device = id(profile.device.as_bytes());
            out.name = Some(profile.name);
        }
        CoreEvent::FriendRequestSent { device, name } => {
            out.kind = UiEventKind::FriendRequestSent;
            out.device = id(device.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::FriendRequestFailed(reason) => {
            out.kind = UiEventKind::FriendRequestFailed;
            out.text = Some(crate::bridge::told(reason));
        }
        CoreEvent::FriendAdded { profile } => {
            out.kind = UiEventKind::FriendAdded;
            out.device = id(profile.device.as_bytes());
            out.name = Some(profile.name);
        }
        CoreEvent::FriendOnline { device } => {
            out.kind = UiEventKind::FriendOnline;
            out.device = id(device.as_bytes());
        }
        CoreEvent::FriendOffline { device } => {
            out.kind = UiEventKind::FriendOffline;
            out.device = id(device.as_bytes());
        }
        CoreEvent::Joined { group, name } => {
            out.kind = UiEventKind::Joined;
            out.group = id(group.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::Removed { group, name } => {
            out.kind = UiEventKind::Removed;
            out.group = id(group.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::TakenOut {
            group,
            voice,
            by,
            barrier: why,
        } => {
            out.kind = UiEventKind::TakenOut;
            out.group = id(group.as_bytes());
            out.voice = Some(hex_id(&voice));
            out.device = by.map(|d| hex_id(d.as_bytes()));
            out.barrier = Some(match why {
                Some(why) => barrier(&why),
                None => UiBarrier {
                    kind: UiBarrierKind::Removed,
                    by: out.device.clone(),
                    since_ms: None,
                    until_ms: None,
                },
            });
        }
        CoreEvent::GroupInvited { group, name, by } => {
            out.kind = UiEventKind::GroupInvited;
            out.group = id(group.as_bytes());
            out.name = Some(name);
            out.device = id(by.as_bytes());
        }
        CoreEvent::InvitationDeclined {
            group,
            name,
            device,
            who,
        } => {
            out.kind = UiEventKind::InvitationDeclined;
            out.group = id(group.as_bytes());
            out.name = Some(name);
            out.device = id(device.as_bytes());
            out.text = Some(who);
        }
        CoreEvent::Download {
            group,
            file,
            received,
            total,
        } => {
            out.kind = UiEventKind::Download;
            out.group = id(group.as_bytes());
            out.file = id(&file);
            out.received = ms(received);
            out.total = ms(total);
        }
        CoreEvent::DownloadDone { group, file, path } => {
            out.kind = UiEventKind::DownloadDone;
            out.group = id(group.as_bytes());
            out.file = id(&file);
            out.text = Some(path.to_string_lossy().into_owned());
        }
        CoreEvent::DownloadFailed {
            group,
            file,
            reason,
        } => {
            out.kind = UiEventKind::DownloadFailed;
            out.group = id(group.as_bytes());
            out.file = id(&file);
            out.text = Some(crate::bridge::told(reason));
        }
        CoreEvent::Address(address) => {
            out.kind = UiEventKind::Address;
            out.text = Some(address.to_string());
        }
        CoreEvent::HardNat => out.kind = UiEventKind::HardNat,
        CoreEvent::Signal {
            group,
            from,
            payload,
        } => {
            out.kind = UiEventKind::Signal;
            out.group = id(group.as_bytes());
            out.device = id(from.as_bytes());
            out.text = Some(String::from_utf8_lossy(&payload).into_owned());
        }
        CoreEvent::SignalsLost { group, from } => {
            out.kind = UiEventKind::SignalsLost;
            out.group = id(group.as_bytes());
            out.device = id(from.as_bytes());
        }
        CoreEvent::Info(text) => {
            out.kind = UiEventKind::Info;
            out.text = Some(crate::bridge::told(text));
        }
        CoreEvent::Stopped => {
            out.kind = UiEventKind::Stopped;
            out.text = Some("Fogata stopped working. Restart it to connect again.".into());
        }
        CoreEvent::Notice(text) => {
            out.kind = UiEventKind::Notice;
            out.text = Some(crate::bridge::told(text));
        }
        CoreEvent::OtherSession => {
            out.kind = UiEventKind::OtherSession;
            out.text = Some(fogata_core::OTHER_SESSION_NOTICE.into());
        }
        CoreEvent::CallIncoming { group, from } => {
            out.kind = UiEventKind::CallIncoming;
            out.group = id(group.as_bytes());
            out.device = id(from.as_bytes());
        }
        CoreEvent::CallAccepted { group } => {
            out.kind = UiEventKind::CallAccepted;
            out.group = id(group.as_bytes());
        }
        CoreEvent::CallEnded {
            group,
            with,
            reason,
        } => {
            out.kind = UiEventKind::CallEnded;
            out.group = id(group.as_bytes());
            out.device = id(with.as_bytes());
            out.call_end = Some(call_end(reason));
        }
        CoreEvent::CallAddEnded {
            group,
            friend,
            reason,
        } => {
            out.kind = UiEventKind::CallAddEnded;
            out.group = id(group.as_bytes());
            out.device = id(friend.as_bytes());
            out.call_end = Some(call_end(reason));
        }
        CoreEvent::CallsChanged => out.kind = UiEventKind::CallsChanged,
        CoreEvent::UpdateChanged => out.kind = UiEventKind::UpdateChanged,
        CoreEvent::MustUpdateToJoin {
            group,
            device,
            name,
        } => {
            out.kind = UiEventKind::MustUpdateToJoin;
            out.group = id(group.as_bytes());
            out.device = id(device.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::UpdateToJoin { group, name } => {
            out.kind = UiEventKind::UpdateToJoin;
            out.group = id(group.as_bytes());
            out.name = Some(name);
        }
        CoreEvent::DirectChanged { friend } => {
            out.kind = UiEventKind::Direct;
            out.device = id(friend.as_bytes());
        }
        CoreEvent::DirectTyping { friend, typing } => {
            out.kind = typing_kind(typing);
            out.device = id(friend.as_bytes());
        }
        CoreEvent::Typing {
            group,
            voice,
            from,
            typing,
        } => {
            out.kind = typing_kind(typing);
            out.group = id(group.as_bytes());
            out.voice = Some(hex_id(&voice));
            out.device = id(from.as_bytes());
        }
        CoreEvent::DirectMessage { friend, text } => {
            out.kind = UiEventKind::DirectMessage;
            out.device = id(friend.as_bytes());
            out.text = Some(text);
        }
        CoreEvent::DirectDownload {
            friend,
            file,
            received,
            total,
        } => {
            out.kind = UiEventKind::DirectDownload;
            out.device = id(friend.as_bytes());
            out.file = id(&file);
            out.received = ms(received);
            out.total = ms(total);
        }
        CoreEvent::DirectDownloadFailed {
            friend,
            file,
            reason,
        } => {
            out.kind = UiEventKind::DirectDownloadFailed;
            out.device = id(friend.as_bytes());
            out.file = id(&file);
            out.text = Some(crate::bridge::told(reason));
        }
    }
    out
}
