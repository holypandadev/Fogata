//! Rooms: the live part of a group. A room exists only in the memory of the members online;
//! it holds who is in it, its chat and the files its participants share. Nothing of it is
//! ever written to disk. A file stays listed while someone in the room has it.
//!
//! Every event is a signed group message, so all members apply the same events and agree on
//! the room. A member that comes online asks one that is already online for the room's
//! events so far. Participants repeat that they are there every [`HERE_INTERVAL_MS`], which
//! also tells members that missed their join. A participant that leaves on purpose says so;
//! one that stops answering keeps its place for [`GRACE_MS`]; when nobody is left, the room
//! is gone. A participant that comes back later opens it again.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fogata_crypto::{
    DevicePublicKey,
    names::{shown_text, tidy},
};

use crate::state::{MAX_MEMBER_NAME_CHARS, fair_name, hiding_mark};

pub type RoomId = [u8; 16];

/// A participant not heard from for this long is shown as away.
pub const AWAY_MS: u64 = 45_000;
/// A participant not heard from for this long has left.
pub const GRACE_MS: u64 = 120_000;
/// How often participants say they are still there.
pub const HERE_INTERVAL_MS: u64 = 20_000;
/// A participant this device has no way left to hear from has left once not heard from for
/// this long, a little more than [`HERE_INTERVAL_MS`]: a leave it said could not reach this
/// device either (see [`RoomTracker::drop_cut_off`]).
pub const CUT_OFF_MS: u64 = 25_000;
/// Chat lines kept per room.
const MAX_CHAT: usize = 500;
/// What holding a chat line costs past its record, text and nickname (see
/// [`RoomTracker::chat_weights`]): the line's own fields and the allocations behind them.
const CHAT_LINE_OVERHEAD: usize = 256;
/// Ids of chat lines let go remembered per room (see [`RoomTracker::forget_chat`]), 16
/// bytes each.
const MAX_FORGOTTEN: usize = 4 * MAX_CHAT;
/// What a chat line or a share may weigh as sent (see [`RoomTracker::weighing`]) past what
/// its event says (its text and nickname, or its name and picture, as the message encodes
/// them): the rest of the message, its header and its seal, with room for a field or two a
/// later version adds. One heavier is ignored: every member would keep it, and hand it to
/// each member that comes online.
pub const EVENT_SLACK_BYTES: usize = 1024;
/// The heaviest join a room keeps for a participant, as sent: a join names a nickname and
/// the latest events, and weighs well under a kilobyte.
pub const MAX_JOIN_BYTES: usize = 8 * 1024;
/// Longest chat message accepted, in characters; a longer one is ignored, whoever sent it.
pub const MAX_CHAT_CHARS: usize = 2000;
/// Leaves remembered, so that late copies of earlier events do not bring a participant back.
const MAX_LEFT: usize = 256;
/// Longest file name accepted, in bytes.
const MAX_FILE_NAME: usize = 255;
/// Messages of this session remembered (see `RoomTracker::sending`).
const MAX_MINE: usize = 64;
/// Files one participant may have shared in a room at once.
pub const MAX_SHARES_EACH: usize = 100;
/// A join, presence or file dated this long before the room started is an earlier room's,
/// sent again: no clocks differ by that much.
const EARLIER_ROOM_MS: u64 = 10 * 60 * 1000;
/// Other ids of one room kept; merging more forgets the oldest.
const MAX_ALIASES: usize = 8;

/// The biggest first picture of a video a room keeps; a bigger one is left out.
pub const MAX_THUMB_BYTES: usize = 64 * 1024;
/// The most bytes of shared files (names and first pictures, each share as sent, a picture
/// left out for being too big counted all the same) the rooms of one group keep together,
/// from every participant: a share past it is ignored. Every member keeps what the
/// participants share, and each may share [`MAX_SHARES_EACH`] files with a picture of
/// [`MAX_THUMB_BYTES`]: 20 members could otherwise make every member keep 128 MiB a room.
/// Each room keeps at most its part of it, as many as the group has rooms, and each
/// participant shares at most its part of that, as many as the room takes at once (3 MiB in
/// the one room of a group, for 8: about 45 videos with the biggest pictures; 300 KiB in
/// each of ten rooms), so that one or two cannot keep the others from sharing; and the
/// rooms together keep no more than this, so a member with devices in every room cannot make
/// the others keep more. Both parts come from the group's log (how many rooms it has and how
/// many people a room takes), the same on every member, and not from what each member holds
/// of the other rooms: the members of a group that gets more rooms each keep the same part
/// of a room's shares, the earliest (see [`RoomTracker::set_room_for`]).
pub const MAX_SHARE_BYTES: usize = 24 * 1024 * 1024;
/// What a share costs on top of its name and picture.
const SHARE_OVERHEAD: usize = 128;

/// A file shared in a room. Its id is the BLAKE3 hash of its contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo {
    pub id: [u8; 32],
    pub name: String,
    pub size: u64,
    /// For a video, what it looks like before it is fetched.
    pub video: Option<VideoPreview>,
}

/// A video's first picture, made small by the device that shares it, its size and how long
/// it lasts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoPreview {
    /// A JPEG.
    pub thumb: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

/// A file listed in the room and the participants that have it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomFile {
    pub info: FileInfo,
    pub shared_by: DevicePublicKey,
    /// When it was listed, by the clock of whoever listed it first, as a chat line's time:
    /// a file shows among the lines in that order.
    pub shared_ms: u64,
    pub holders: BTreeSet<DevicePublicKey>,
}

/// What a room event says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomInput {
    /// The sender enters the room, which started at `started_ms` (the sender's view; a sender
    /// that knows no room starts one).
    Join {
        started_ms: u64,
        nick: String,
    },
    /// The sender is still in the room; the same as a join for members that missed it.
    Here {
        started_ms: u64,
        nick: String,
    },
    /// The sender leaves on purpose.
    Leave,
    Chat {
        nick: String,
        text: String,
    },
    /// The sender has this file and serves it to the others while it is in the room.
    Share(FileInfo),
    /// The sender stops serving this file.
    Unshare([u8; 32]),
}

/// A room event: who sent it, when (sender's clock), its id, and the room it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomEvent {
    pub room: RoomId,
    pub sender: DevicePublicKey,
    pub timestamp_ms: u64,
    pub message_id: [u8; 16],
    pub input: RoomInput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
    pub nick: String,
    /// Local time of the last event heard from it.
    pub last_heard_ms: u64,
    /// Its own time on the newest join or presence it sent.
    pub last_said_ms: u64,
    pub away: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatLine {
    pub sender: DevicePublicKey,
    pub nick: String,
    pub text: String,
    pub timestamp_ms: u64,
    pub message_id: [u8; 16],
}

/// What changed, for the user interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomChange {
    /// A room became known here: someone started it, or this device learnt about it.
    Opened {
        started_ms: u64,
    },
    Joined {
        device: DevicePublicKey,
        nick: String,
    },
    Left {
        device: DevicePublicKey,
        on_purpose: bool,
    },
    Away {
        device: DevicePublicKey,
    },
    Back {
        device: DevicePublicKey,
    },
    Chat(ChatLine),
    /// A file was listed in the room.
    FileShared {
        file: FileInfo,
        by: DevicePublicKey,
    },
    /// Nobody in the room has the file any more.
    FileGone {
        file: FileInfo,
    },
    /// Nobody is left; the room is gone.
    Closed,
}

/// The current room, with the events to hand to members that come online.
#[derive(Clone, Debug)]
pub struct Room<S> {
    pub id: RoomId,
    pub started_ms: u64,
    pub participants: BTreeMap<DevicePublicKey, Participant>,
    pub chat: Vec<ChatLine>,
    pub files: BTreeMap<[u8; 32], RoomFile>,
    /// Other ids of the same room: rooms started at the same moment and merged into it.
    aliases: Vec<RoomId>,
    /// Latest join of each participant, every chat line and each holder's share, as sent
    /// (a share with its sender's time and what it costs, see [`MAX_SHARE_BYTES`]).
    joins: BTreeMap<DevicePublicKey, S>,
    chat_records: VecDeque<([u8; 16], S)>,
    share_records: BTreeMap<([u8; 32], DevicePublicKey), (u64, usize, S)>,
    /// Chat lines let go to keep this device's memory within a bound (see
    /// [`RoomTracker::forget_chat`]), newest last: a copy that comes again is not taken.
    forgotten: VecDeque<[u8; 16]>,
}

impl<S> Room<S> {
    fn new(id: RoomId, started_ms: u64) -> Self {
        Self {
            id,
            started_ms,
            participants: BTreeMap::new(),
            chat: Vec::new(),
            files: BTreeMap::new(),
            aliases: Vec::new(),
            joins: BTreeMap::new(),
            chat_records: VecDeque::new(),
            share_records: BTreeMap::new(),
            forgotten: VecDeque::new(),
        }
    }

    fn has_id(&self, id: &RoomId) -> bool {
        self.id == *id || self.aliases.contains(id)
    }

    /// Takes `device` off the holders of file `id`; the file goes away when nobody has it.
    fn drop_holder(
        &mut self,
        id: [u8; 32],
        device: DevicePublicKey,
        changes: &mut Vec<RoomChange>,
    ) {
        self.share_records.remove(&(id, device));
        let Some(file) = self.files.get_mut(&id) else {
            return;
        };
        file.holders.remove(&device);
        if file.holders.is_empty()
            && let Some(file) = self.files.remove(&id)
        {
            changes.push(RoomChange::FileGone { file: file.info });
        }
    }

    /// Removes a participant and its shares; files nobody else has go away.
    fn drop_participant(
        &mut self,
        device: DevicePublicKey,
        on_purpose: bool,
        changes: &mut Vec<RoomChange>,
    ) {
        if self.participants.remove(&device).is_none() {
            return;
        }
        self.joins.remove(&device);
        changes.push(RoomChange::Left { device, on_purpose });
        self.share_records
            .retain(|(_, holder), _| *holder != device);
        let mut gone = Vec::new();
        for (id, file) in self.files.iter_mut() {
            file.holders.remove(&device);
            if file.holders.is_empty() {
                gone.push(*id);
            }
        }
        for id in gone {
            if let Some(file) = self.files.remove(&id) {
                changes.push(RoomChange::FileGone { file: file.info });
            }
        }
    }
}

/// The room of one group as this device sees it. `S` is the sent form of an event (a sealed
/// message), kept to give to members that come online.
#[derive(Debug)]
pub struct RoomTracker<S> {
    me: DevicePublicKey,
    room: Option<Room<S>>,
    /// When each device last left on purpose, by its own clock.
    left: BTreeMap<DevicePublicKey, u64>,
    /// When this session of the device started: its own joins from before belong to a
    /// session that ended without a goodbye (a crash, a power cut).
    session_ms: u64,
    /// A room an earlier session of this device was still listed in, to leave now.
    earlier: Option<RoomId>,
    /// Rooms already left for an earlier session.
    earlier_left: BTreeSet<RoomId>,
    /// Messages this session of the device sent, newest last: its own events are this
    /// session's whatever their time says, should the clock have been set back since.
    mine: VecDeque<[u8; 16]>,
    /// Local time of the last tick: a long gap means the computer slept.
    last_tick_ms: Option<u64>,
    /// What a record weighs as kept (see [`Self::weighing`]).
    weigh: fn(&S) -> usize,
    /// How many people the room takes at once, and how many rooms the group has: the room
    /// keeps that part of [`MAX_SHARE_BYTES`], and each participant shares at most that
    /// part of it.
    room_for: usize,
    rooms: usize,
    /// The room and participant parts the room's shares were last fitted to (see
    /// [`Self::fit_shares`]).
    fitted: (usize, usize),
}

impl<S: Clone> RoomTracker<S> {
    pub fn new(me: DevicePublicKey) -> Self {
        Self {
            me,
            room: None,
            left: BTreeMap::new(),
            session_ms: 0,
            earlier: None,
            earlier_left: BTreeSet::new(),
            mine: VecDeque::new(),
            last_tick_ms: None,
            weigh: |_| 0,
            room_for: usize::from(crate::state::MAX_IN_ROOM),
            rooms: 1,
            fitted: (usize::from(crate::state::MAX_IN_ROOM), 1),
        }
    }

    /// The same tracker, with `weigh` telling what each record it keeps weighs as sent: a
    /// member makes the others keep the whole record, whatever the event in it says, so a
    /// record much heavier than its event is ignored (see [`EVENT_SLACK_BYTES`],
    /// [`MAX_JOIN_BYTES`]). Without it, records weigh nothing.
    pub fn weighing(mut self, weigh: fn(&S) -> usize) -> Self {
        self.weigh = weigh;
        self
    }

    /// Notes how many people the room takes at once (the group's limit) and how many rooms
    /// the group has: from now on the room keeps that part of [`MAX_SHARE_BYTES`], and
    /// each participant shares at most that part of it. When a part gets smaller (the
    /// group got more rooms, or a room takes more people), the room keeps of what it has
    /// what fits in it, the earliest shares first, from the next event on (see
    /// [`Self::fit_shares`]).
    pub fn set_room_for(&mut self, people: u8, rooms: usize) {
        self.room_for = usize::from(people.max(1));
        self.rooms = rooms.max(1);
    }

    /// Keeps of the room's shares those that fit in its part of [`MAX_SHARE_BYTES`] and
    /// each participant's part of that, as they are now, the earliest first (by the time
    /// their sender gave them, then by id): shares kept while the parts were bigger go,
    /// and every member keeps the same ones of the same shares, whatever order they came in.
    fn fit_shares(&mut self, changes: &mut Vec<RoomChange>) {
        let parts = (self.room_for, self.rooms);
        if self.fitted == parts {
            return;
        }
        self.fitted = parts;
        let room_bytes = MAX_SHARE_BYTES / self.rooms;
        let each = room_bytes / self.room_for;
        let Some(room) = self.room.as_mut() else {
            return;
        };
        let mut shares: Vec<(u64, [u8; 32], DevicePublicKey, usize)> = room
            .share_records
            .iter()
            .map(|((id, sender), (at, cost, _))| (*at, *id, *sender, *cost))
            .collect();
        shares.sort_unstable();
        let (mut kept, mut own) = (0, BTreeMap::<DevicePublicKey, usize>::new());
        for (_, id, sender, cost) in shares {
            let theirs = own.entry(sender).or_default();
            if kept + cost > room_bytes || *theirs + cost > each {
                room.drop_holder(id, sender, changes);
            } else {
                kept += cost;
                *theirs += cost;
            }
        }
    }

    /// Each chat line the room keeps: its sender, its time, its id and what keeping it
    /// costs here (its record as sent, see [`Self::weighing`], its text and nickname, and
    /// [`CHAT_LINE_OVERHEAD`]). For a bound on the chat of every room a device is in.
    pub fn chat_weights(&self) -> Vec<(DevicePublicKey, u64, [u8; 16], usize)> {
        let Some(room) = &self.room else {
            return Vec::new();
        };
        let records: BTreeMap<&[u8; 16], usize> = room
            .chat_records
            .iter()
            .map(|(id, record)| (id, (self.weigh)(record)))
            .collect();
        room.chat
            .iter()
            .map(|line| {
                let record = records.get(&line.message_id).copied().unwrap_or(0);
                let weight = record + line.text.len() + line.nick.len() + CHAT_LINE_OVERHEAD;
                (line.sender, line.timestamp_ms, line.message_id, weight)
            })
            .collect()
    }

    /// Lets go of chat line `message_id`, and of its record: a device keeps the chat of
    /// every room of every group it is in, and lets the heaviest go past a bound of its own
    /// (what is kept for others, and what they keep, does not change). A copy of it that
    /// comes again is not taken, nor shown again as new. Whether it was here.
    pub fn forget_chat(&mut self, message_id: &[u8; 16]) -> bool {
        let Some(room) = self.room.as_mut() else {
            return false;
        };
        let Some(at) = room.chat.iter().position(|l| l.message_id == *message_id) else {
            return false;
        };
        room.chat.remove(at);
        room.chat_records.retain(|(id, _)| id != message_id);
        room.forgotten.push_back(*message_id);
        if room.forgotten.len() > MAX_FORGOTTEN {
            room.forgotten.pop_front();
        }
        true
    }

    /// How many bytes of shares this room keeps, as counted against [`MAX_SHARE_BYTES`].
    pub fn share_bytes(&self) -> usize {
        self.room.as_ref().map_or(0, |room| {
            room.share_records.values().map(|(_, cost, _)| cost).sum()
        })
    }

    /// Notes a message this device is sending now (see `mine`).
    pub fn sending(&mut self, message_id: [u8; 16]) {
        self.mine.push_back(message_id);
        if self.mine.len() > MAX_MINE {
            self.mine.pop_front();
        }
    }

    /// The same tracker for a session of this device that started at `session_ms`.
    pub fn since(mut self, session_ms: u64) -> Self {
        self.session_ms = session_ms;
        self
    }

    /// A room an earlier session of this device was still listed in, once per room: the
    /// device should say it left, so the others stop showing it there.
    pub fn take_earlier_session(&mut self) -> Option<RoomId> {
        self.earlier.take()
    }

    pub fn room(&self) -> Option<&Room<S>> {
        self.room.as_ref()
    }

    /// Whether this device is in the room.
    pub fn joined(&self) -> bool {
        self.room
            .as_ref()
            .is_some_and(|r| r.participants.contains_key(&self.me))
    }

    /// Whether `device` is in the room.
    pub fn is_participant(&self, device: &DevicePublicKey) -> bool {
        self.room
            .as_ref()
            .is_some_and(|r| r.participants.contains_key(device))
    }

    /// The room to join: the current one, or a new one with `fresh_id` started now.
    pub fn room_to_join(&self, fresh_id: RoomId, now_ms: u64) -> (RoomId, u64) {
        match &self.room {
            Some(room) => (room.id, room.started_ms),
            None => (fresh_id, now_ms),
        }
    }

    /// Events to give a member that comes online: each participant's join, the files shared
    /// and the chat.
    pub fn snapshot(&self) -> Vec<S> {
        let (joins, rest) = self.snapshot_parts();
        joins.into_iter().chain(rest).cloned().collect()
    }

    /// [`Self::snapshot`] in two parts: the participants' joins, which say who is in the
    /// room, and the rest, files first and then the chat.
    pub fn snapshot_parts(&self) -> (Vec<&S>, Vec<&S>) {
        let Some(room) = &self.room else {
            return (Vec::new(), Vec::new());
        };
        let joins = room
            .joins
            .iter()
            .filter(|(device, _)| room.participants.contains_key(*device))
            .map(|(_, record)| record)
            .collect();
        let rest = room
            .share_records
            .values()
            .map(|(_, _, record)| record)
            .chain(room.chat_records.iter().map(|(_, record)| record))
            .collect();
        (joins, rest)
    }

    /// Applies an event. `now_ms` is the local clock; `record` is the event as sent.
    pub fn apply(&mut self, event: RoomEvent, record: S, now_ms: u64) -> Vec<RoomChange> {
        let mut changes = Vec::new();
        self.fit_shares(&mut changes);
        let sender = event.sender;
        // Any member may send a room's signed events again, long after: one from before the
        // current room started would attach the room it came from, with its chat.
        let earlier_room = self
            .room
            .as_ref()
            .is_some_and(|r| event.timestamp_ms.saturating_add(EARLIER_ROOM_MS) < r.started_ms);
        match event.input {
            RoomInput::Join { started_ms, nick } | RoomInput::Here { started_ms, nick } => {
                if !fair_name(&nick, MAX_MEMBER_NAME_CHARS) {
                    return changes;
                }
                // As it shows: one of only blank characters is an empty one.
                let nick = tidy(&nick);
                // An event from before the sender's last leave, arriving late.
                if self
                    .left
                    .get(&sender)
                    .is_some_and(|t| *t >= event.timestamp_ms)
                {
                    return changes;
                }
                // This device in the room, said by a session that is over: not in it now.
                if sender == self.me
                    && event.timestamp_ms < self.session_ms
                    && !self.mine.contains(&event.message_id)
                {
                    // In this voice room again already (the app or the command line started
                    // and went in at once): a leave would take this session out too.
                    if self.earlier_left.insert(event.room) && !self.joined() {
                        self.earlier = Some(event.room);
                    }
                    return changes;
                }
                if earlier_room {
                    return changes;
                }
                // Kept for a new participant, to hand to members that come online.
                let heavy = (self.weigh)(&record) > MAX_JOIN_BYTES;
                if heavy && !self.is_participant(&sender) {
                    return changes;
                }
                self.adopt(event.room, started_ms, &mut changes);
                let room = self.room.as_mut().expect("adopted above");
                match room.participants.get_mut(&sender) {
                    Some(p) => {
                        p.last_heard_ms = now_ms;
                        // The name it said last: an older event sent again does not undo a
                        // change of name.
                        if event.timestamp_ms >= p.last_said_ms {
                            p.nick = nick;
                        }
                        p.last_said_ms = p.last_said_ms.max(event.timestamp_ms);
                        if p.away {
                            p.away = false;
                            changes.push(RoomChange::Back { device: sender });
                        }
                    }
                    None => {
                        room.joins.insert(sender, record);
                        room.participants.insert(
                            sender,
                            Participant {
                                nick: nick.clone(),
                                last_heard_ms: now_ms,
                                last_said_ms: event.timestamp_ms,
                                away: false,
                            },
                        );
                        changes.push(RoomChange::Joined {
                            device: sender,
                            nick,
                        });
                    }
                }
            }
            RoomInput::Leave => {
                // A leave older than what the sender said since in this room is an old copy
                // (among a room's events, say): it does not take them out.
                if self
                    .room
                    .as_ref()
                    .filter(|r| r.has_id(&event.room))
                    .and_then(|r| r.participants.get(&sender))
                    .is_some_and(|p| p.last_said_ms > event.timestamp_ms)
                {
                    return changes;
                }
                self.note_left(sender, event.timestamp_ms);
                let Some(room) = self.room.as_mut().filter(|r| r.has_id(&event.room)) else {
                    return changes;
                };
                room.drop_participant(sender, true, &mut changes);
            }
            RoomInput::Unshare(id) => {
                let Some(room) = self.room.as_mut().filter(|r| r.has_id(&event.room)) else {
                    return changes;
                };
                // Older than the share it would end: an earlier one's, sent again.
                if room
                    .share_records
                    .get(&(id, sender))
                    .is_some_and(|(at, _, _)| *at > event.timestamp_ms)
                {
                    return changes;
                }
                room.drop_holder(id, sender, &mut changes);
            }
            RoomInput::Share(mut info) => {
                let Some(room) = self.room.as_mut().filter(|r| r.has_id(&event.room)) else {
                    return changes;
                };
                if earlier_room {
                    return changes;
                }
                // Its picture counts whole, as sent, even when too big to show.
                let thumb = info.video.as_ref().map_or(0, |v| v.thumb.len());
                let said = json_len(&info.name) + thumb.div_ceil(3) * 4;
                if (self.weigh)(&record) > said + EVENT_SLACK_BYTES {
                    return changes;
                }
                if info
                    .video
                    .as_ref()
                    .is_some_and(|v| v.thumb.len() > MAX_THUMB_BYTES)
                {
                    info.video = None;
                }
                // Only participants share, a bounded number of files each, and only files with
                // a usable name: none that hides marks or turns around (`photo\u{202E}gnp.exe`),
                // and one that shows something. It is kept as it shows, without padding.
                let shared_by_sender = room
                    .share_records
                    .keys()
                    .filter(|(_, s)| *s == sender)
                    .count();
                if !room.participants.contains_key(&sender)
                    || tidy(&info.name).is_empty()
                    || info.name.len() > MAX_FILE_NAME
                    || info.name.chars().any(|c| c.is_control() || hiding_mark(c))
                    || (shared_by_sender >= MAX_SHARES_EACH
                        && !room.share_records.contains_key(&(info.id, sender)))
                {
                    return changes;
                }
                info.name = tidy(&info.name);
                // Within the room's part of the group's bytes for shares and the sender's part
                // of that, the one it replaces given back.
                let cost = SHARE_OVERHEAD + info.name.len() + thumb;
                let others = room
                    .share_records
                    .iter()
                    .filter(|(key, _)| **key != (info.id, sender));
                let (mut kept, mut own) = (0, 0);
                for ((_, holder), (_, cost, _)) in others {
                    kept += cost;
                    if *holder == sender {
                        own += cost;
                    }
                }
                let room_bytes = MAX_SHARE_BYTES / self.rooms;
                if kept + cost > room_bytes || own + cost > room_bytes / self.room_for {
                    return changes;
                }
                room.share_records
                    .insert((info.id, sender), (event.timestamp_ms, cost, record));
                match room.files.get_mut(&info.id) {
                    Some(file) => {
                        file.holders.insert(sender);
                    }
                    None => {
                        room.files.insert(
                            info.id,
                            RoomFile {
                                info: info.clone(),
                                shared_by: sender,
                                shared_ms: event.timestamp_ms,
                                holders: BTreeSet::from([sender]),
                            },
                        );
                        changes.push(RoomChange::FileShared {
                            file: info,
                            by: sender,
                        });
                    }
                }
            }
            RoomInput::Chat { nick, text } => {
                if !fair_name(&nick, MAX_MEMBER_NAME_CHARS)
                    || (self.weigh)(&record) > json_len(&text) + json_len(&nick) + EVENT_SLACK_BYTES
                {
                    return changes;
                }
                let nick = tidy(&nick);
                let text = shown_text(&text);
                // Chat of a room this device does not know: the caller asks for its events.
                let Some(room) = self.room.as_mut().filter(|r| r.has_id(&event.room)) else {
                    return changes;
                };
                // Copies of a line already here, for example among a room's events, or of
                // one let go (see `forget_chat`).
                if room.chat.iter().any(|l| l.message_id == event.message_id)
                    || room.forgotten.contains(&event.message_id)
                    || text.chars().count() > MAX_CHAT_CHARS
                {
                    return changes;
                }
                if let Some(p) = room.participants.get_mut(&sender) {
                    p.last_heard_ms = now_ms;
                }
                let line = ChatLine {
                    sender,
                    nick,
                    text,
                    timestamp_ms: event.timestamp_ms,
                    message_id: event.message_id,
                };
                // Keep the chat in sender-time order; events can arrive out of order.
                let at = room.chat.partition_point(|l| {
                    (l.timestamp_ms, l.message_id) <= (line.timestamp_ms, line.message_id)
                });
                // A full chat already let this line go, or never had it (another member keeps
                // other lines among its 500): older than all it keeps, it would go again at
                // once, and shown as new it would sound as new.
                if at == 0 && room.chat.len() >= MAX_CHAT {
                    return changes;
                }
                room.chat.insert(at, line.clone());
                room.chat_records.push_back((event.message_id, record));
                // The oldest line by its time goes, and its own record with it.
                if room.chat.len() > MAX_CHAT {
                    let oldest = room.chat.remove(0).message_id;
                    room.chat_records.retain(|(id, _)| *id != oldest);
                }
                changes.push(RoomChange::Chat(line));
            }
        }
        self.close_if_empty(&mut changes);
        changes
    }

    /// Makes `id` the current room, or merges it with the current one if both were started
    /// at about the same time: the earlier start wins, ties broken by id.
    fn adopt(&mut self, id: RoomId, started_ms: u64, changes: &mut Vec<RoomChange>) {
        match &mut self.room {
            None => {
                self.room = Some(Room::new(id, started_ms));
                changes.push(RoomChange::Opened { started_ms });
            }
            Some(room) if room.has_id(&id) => {}
            Some(room) => {
                if (started_ms, id) < (room.started_ms, room.id) {
                    room.aliases.push(room.id);
                    room.id = id;
                    room.started_ms = started_ms;
                } else {
                    room.aliases.push(id);
                }
                // Made-up ids would otherwise pile up without end.
                if room.aliases.len() > MAX_ALIASES {
                    room.aliases.remove(0);
                }
            }
        }
    }

    /// Marks participants that went quiet as away, and drops those quiet for longer than
    /// the grace period. This device never times out itself.
    pub fn tick(&mut self, now_ms: u64) -> Vec<RoomChange> {
        let mut changes = Vec::new();
        // Back from a sleep (ticks come every few seconds): nobody was heard because this
        // device was not listening, so everyone gets a fresh grace instead of all leaving
        // at once.
        let slept = self
            .last_tick_ms
            .is_some_and(|last| now_ms.saturating_sub(last) >= AWAY_MS);
        self.last_tick_ms = Some(now_ms);
        let Some(room) = self.room.as_mut() else {
            return changes;
        };
        if slept {
            for p in room.participants.values_mut() {
                p.last_heard_ms = now_ms;
            }
        }
        let me = self.me;
        let mut gone = Vec::new();
        for (device, p) in room.participants.iter_mut() {
            if *device == me {
                continue;
            }
            let quiet = now_ms.saturating_sub(p.last_heard_ms);
            if quiet >= GRACE_MS {
                gone.push((*device, p.last_said_ms));
            } else if quiet >= AWAY_MS && !p.away {
                p.away = true;
                changes.push(RoomChange::Away { device: *device });
            }
        }
        for (device, _) in &gone {
            room.drop_participant(*device, false, &mut changes);
        }
        // As if it had said it leaves after what it said last: any member could send its
        // earlier joins again (they are signed, not dated by this device), to show it in the
        // room, or to bring back the room they came from with its chat.
        for (device, last_said) in gone {
            self.note_left(device, last_said);
        }
        self.close_if_empty(&mut changes);
        changes
    }

    /// Drops the participants that `cut_off` names, those this device has no way left to
    /// hear from, once not heard from for [`CUT_OFF_MS`], as the grace period would later:
    /// the grace is for those still heard through someone, and a leave a participant cut off
    /// said could not reach this device. Nobody goes while the ticks say this device was
    /// asleep: the next tick gives everyone a fresh grace instead.
    pub fn drop_cut_off(
        &mut self,
        now_ms: u64,
        cut_off: impl Fn(&DevicePublicKey) -> bool,
    ) -> Vec<RoomChange> {
        let mut changes = Vec::new();
        let awake = self
            .last_tick_ms
            .is_some_and(|last| now_ms.saturating_sub(last) < AWAY_MS);
        let Some(room) = self.room.as_mut().filter(|_| awake) else {
            return changes;
        };
        let me = self.me;
        let gone: Vec<(DevicePublicKey, u64)> = room
            .participants
            .iter()
            .filter(|(device, p)| {
                **device != me
                    && now_ms.saturating_sub(p.last_heard_ms) >= CUT_OFF_MS
                    && cut_off(device)
            })
            .map(|(device, p)| (*device, p.last_said_ms))
            .collect();
        for (device, _) in &gone {
            room.drop_participant(*device, false, &mut changes);
        }
        for (device, last_said) in gone {
            self.note_left(device, last_said);
        }
        self.close_if_empty(&mut changes);
        changes
    }

    /// Remembers that `device` left at `at_ms` by its own clock: its events from before do
    /// not bring it back.
    fn note_left(&mut self, device: DevicePublicKey, at_ms: u64) {
        let at = self.left.entry(device).or_default();
        *at = (*at).max(at_ms);
        if self.left.len() > MAX_LEFT
            && let Some(oldest) = self.left.iter().min_by_key(|(_, t)| **t).map(|(d, _)| *d)
        {
            self.left.remove(&oldest);
        }
    }

    /// Takes `device` out of the room at once, with its files: it is no longer a member of
    /// the group, so it can neither say it leaves nor be heard from again.
    pub fn drop_device(&mut self, device: &DevicePublicKey) -> Vec<RoomChange> {
        let mut changes = Vec::new();
        if let Some(room) = self.room.as_mut() {
            room.drop_participant(*device, false, &mut changes);
        }
        self.close_if_empty(&mut changes);
        changes
    }

    /// Closes the room with everyone in it, this device too: its voice room is gone from
    /// the group.
    pub fn close(&mut self) -> Vec<RoomChange> {
        let mut changes = Vec::new();
        if let Some(room) = self.room.as_mut() {
            let everyone: Vec<DevicePublicKey> = room.participants.keys().copied().collect();
            for device in everyone {
                room.drop_participant(device, false, &mut changes);
            }
        }
        self.close_if_empty(&mut changes);
        changes
    }

    fn close_if_empty(&mut self, changes: &mut Vec<RoomChange>) {
        if self
            .room
            .as_ref()
            .is_some_and(|r| r.participants.is_empty())
        {
            self.room = None;
            changes.push(RoomChange::Closed);
        }
    }

    /// Whether `room` is not the room this device knows, so it should ask for its events.
    pub fn is_unknown(&self, room: &RoomId) -> bool {
        !self.room.as_ref().is_some_and(|r| r.has_id(room))
    }
}

/// How many bytes `text` takes as a JSON string's contents, as events are encoded: quotes
/// and backslashes escaped in two, control characters in two or six.
fn json_len(text: &str) -> usize {
    text.chars()
        .map(|c| match c {
            '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
            c if (c as u32) < 0x20 => 6,
            c => c.len_utf8(),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;

    fn key() -> DevicePublicKey {
        DeviceIdentity::generate().unwrap().public_key()
    }

    fn event(room: RoomId, sender: DevicePublicKey, ts: u64, input: RoomInput) -> RoomEvent {
        let mut message_id = [0u8; 16];
        message_id[..8].copy_from_slice(&ts.to_be_bytes());
        message_id[8] = sender.as_bytes()[0];
        RoomEvent {
            room,
            sender,
            timestamp_ms: ts,
            message_id,
            input,
        }
    }

    fn join(nick: &str, started_ms: u64) -> RoomInput {
        RoomInput::Join {
            started_ms,
            nick: nick.into(),
        }
    }

    fn here(nick: &str, started_ms: u64) -> RoomInput {
        RoomInput::Here {
            started_ms,
            nick: nick.into(),
        }
    }

    fn chat(text: &str) -> RoomInput {
        RoomInput::Chat {
            nick: "x".into(),
            text: text.into(),
        }
    }

    #[test]
    fn made_up_names_old_leaves_and_too_many_files_are_ignored() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [1; 16];
        t.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
        // A name that turns text around, or one far too long, joins nobody.
        assert!(
            t.apply(event(room, bea, 11, join("B\u{202E}ea", 10)), 2, 11)
                .is_empty()
        );
        assert!(
            t.apply(event(room, bea, 11, join(&"b".repeat(41), 10)), 2, 11)
                .is_empty()
        );
        t.apply(event(room, bea, 12, join("Bea", 10)), 3, 12);
        assert!(t.is_participant(&bea));
        // An old leave, sent again among a room's events, does not take Bea out.
        t.apply(event(room, bea, 20, here("Bea", 10)), 4, 20);
        assert!(
            t.apply(event(room, bea, 15, RoomInput::Leave), 5, 21)
                .is_empty()
        );
        assert!(t.is_participant(&bea));
        // A file name made to show as something else is not listed.
        let file = |n: u8, name: &str| FileInfo {
            id: [n; 32],
            name: name.into(),
            size: 1,
            video: None,
        };
        let shared = t.apply(
            event(room, bea, 22, RoomInput::Share(file(1, "a\u{202E}fdp.exe"))),
            6,
            22,
        );
        assert!(shared.is_empty());
        // Nor are more files than a participant may share at once.
        for n in 0..MAX_SHARES_EACH as u8 {
            t.apply(
                event(
                    room,
                    bea,
                    30 + u64::from(n),
                    RoomInput::Share(file(n + 2, "f")),
                ),
                7,
                30,
            );
        }
        let one_more = t.apply(
            event(room, bea, 400, RoomInput::Share(file(250, "f"))),
            8,
            400,
        );
        assert!(one_more.is_empty());
    }

    #[test]
    fn a_room_keeps_so_many_bytes_of_shares_from_everyone() {
        let ana = key();
        let others: Vec<DevicePublicKey> = (0..19).map(|_| key()).collect();
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [1; 16];
        t.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
        for (i, other) in others.iter().enumerate() {
            t.apply(event(room, *other, 11 + i as u64, join("x", 10)), 1, 11);
        }
        let video = |n: u32| FileInfo {
            id: blake(n),
            name: "clip.mp4".into(),
            size: 1,
            video: Some(VideoPreview {
                thumb: vec![0xff; MAX_THUMB_BYTES],
                width: 2,
                height: 2,
                duration_ms: 1,
            }),
        };
        // Each shares as many videos as it may, every one with the biggest picture: its part
        // of the room's bytes, until the room is full.
        let mut listed = 0;
        for (i, other) in others.iter().enumerate() {
            for n in 0..MAX_SHARES_EACH as u32 {
                let ts = 100 + (i as u64) * 1_000 + u64::from(n);
                let file = video(i as u32 * 1_000 + n);
                let changes = t.apply(event(room, *other, ts, RoomInput::Share(file)), 2, ts);
                listed += changes.len();
            }
        }
        let cost = SHARE_OVERHEAD + "clip.mp4".len() + MAX_THUMB_BYTES;
        assert_eq!(listed, MAX_SHARE_BYTES / cost);
        assert_eq!(t.room().unwrap().files.len(), listed);
        let each = MAX_SHARE_BYTES / usize::from(crate::state::MAX_IN_ROOM) / cost;
        let files = &t.room().unwrap().files;
        assert!(
            others
                .iter()
                .all(|o| files.values().filter(|f| f.shared_by == *o).count() <= each)
        );
        // In a group with ten rooms, each room keeps a tenth of that.
        let mut tenth: RoomTracker<u32> = RoomTracker::new(ana);
        tenth.set_room_for(crate::state::MAX_IN_ROOM, 10);
        tenth.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
        let mut listed_in_tenth = 0;
        for (i, other) in others.iter().enumerate() {
            tenth.apply(event(room, *other, 11 + i as u64, join("x", 10)), 1, 11);
            for n in 0..MAX_SHARES_EACH as u32 {
                let ts = 100 + (i as u64) * 1_000 + u64::from(n);
                let file = video(i as u32 * 1_000 + n);
                let changes = tenth.apply(event(room, *other, ts, RoomInput::Share(file)), 2, ts);
                listed_in_tenth += changes.len();
            }
        }
        assert_eq!(listed_in_tenth, MAX_SHARE_BYTES / 10 / cost);
        assert_eq!(tenth.share_bytes(), listed_in_tenth * cost);
        // Two members that hold the same shares, which came to them in different orders,
        // while the room was the group's only one: once the group has ten rooms each keeps a
        // tenth from its next event on, and the same shares, the earliest.
        let few = &others[..8];
        let held = |order: &[usize]| {
            let mut room_of: RoomTracker<u32> = RoomTracker::new(ana);
            room_of.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
            for &i in order {
                let other = few[i];
                room_of.apply(event(room, other, 11 + i as u64, join("x", 10)), 1, 11);
                for n in 0..MAX_SHARES_EACH as u32 {
                    let ts = 100 + (i as u64) * 1_000 + u64::from(n);
                    let file = video(i as u32 * 1_000 + n);
                    room_of.apply(event(room, other, ts, RoomInput::Share(file)), 2, ts);
                }
            }
            room_of
        };
        let forward: Vec<usize> = (0..few.len()).collect();
        let backward: Vec<usize> = (0..few.len()).rev().collect();
        let (mut first, mut second) = (held(&forward), held(&backward));
        let ids =
            |t: &RoomTracker<u32>| t.room().unwrap().files.keys().copied().collect::<Vec<_>>();
        // Everyone's part fits in the one room: both hold all of it.
        assert_eq!(first.share_bytes(), few.len() * each * cost);
        assert_eq!(ids(&first), ids(&second));
        let mut gone = Vec::new();
        for t in [&mut first, &mut second] {
            t.set_room_for(crate::state::MAX_IN_ROOM, 10);
            let changes = t.apply(event(room, ana, 60_000, here("Ana", 10)), 3, 60_000);
            gone.push(
                changes
                    .iter()
                    .filter(|c| matches!(c, RoomChange::FileGone { .. }))
                    .count(),
            );
        }
        assert_eq!(gone[0], gone[1]);
        assert_eq!(ids(&first), ids(&second));
        assert!(first.share_bytes() <= MAX_SHARE_BYTES / 10);
        assert_eq!(
            first.room().unwrap().files.len() + gone[0],
            few.len() * each
        );
        // A share given up makes room for another.
        let first = blake(0);
        t.apply(
            event(room, others[0], 50_000, RoomInput::Unshare(first)),
            3,
            50_000,
        );
        let again = video(999_999);
        let changes = t.apply(
            event(room, others[0], 50_001, RoomInput::Share(again)),
            4,
            50_001,
        );
        assert_eq!(changes.len(), 1);
    }

    fn blake(n: u32) -> [u8; 32] {
        let mut id = [0u8; 32];
        id[..4].copy_from_slice(&n.to_be_bytes());
        id
    }

    #[test]
    fn names_that_show_nothing_are_empty_and_padding_goes() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [1; 16];
        t.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
        t.apply(event(room, bea, 11, join("\u{3164}\u{2800}", 10)), 2, 11);
        let participants = &t.room().unwrap().participants;
        assert_eq!(participants[&bea].nick, "");
        t.apply(event(room, bea, 12, here("Ana\u{3164}\u{3164}", 10)), 3, 12);
        assert_eq!(t.room().unwrap().participants[&bea].nick, "Ana");
        let line = RoomInput::Chat {
            nick: "\u{FFA0}Bea\u{180E}".into(),
            text: "hi".into(),
        };
        t.apply(event(room, bea, 13, line), 4, 13);
        assert_eq!(t.room().unwrap().chat[0].nick, "Bea");
        let file = |n: u8, name: &str| FileInfo {
            id: [n; 32],
            name: name.into(),
            size: 1,
            video: None,
        };
        // A file name of nothing that shows is not listed, and padding before the
        // extension goes.
        for blank in ["\u{3164}", "\u{2800}\u{2800}", "\u{0301}"] {
            let shared = t.apply(
                event(room, bea, 14, RoomInput::Share(file(1, blank))),
                5,
                14,
            );
            assert!(shared.is_empty(), "{blank:?}");
        }
        let shared = t.apply(
            event(
                room,
                bea,
                15,
                RoomInput::Share(file(2, "photo\u{3164}\u{3164}\u{3164}.exe")),
            ),
            6,
            15,
        );
        assert!(
            matches!(&shared[..], [RoomChange::FileShared { file, .. }] if file.name == "photo .exe")
        );
    }

    #[test]
    fn a_room_lives_while_someone_is_in_it() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [1; 16];
        let c = t.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
        assert_eq!(c[0], RoomChange::Opened { started_ms: 10 });
        assert!(t.joined());
        t.apply(event(room, bea, 11, join("Bea", 10)), 2, 11);
        t.apply(event(room, bea, 12, chat("hi")), 3, 12);
        let mut all = t.snapshot();
        all.sort();
        assert_eq!(all, vec![1, 2, 3]);
        let c = t.apply(event(room, bea, 13, RoomInput::Leave), 4, 13);
        assert_eq!(
            c,
            vec![RoomChange::Left {
                device: bea,
                on_purpose: true
            }]
        );
        // Bea's join is no longer handed out; her chat stays.
        assert_eq!(t.snapshot(), vec![1, 3]);
        let c = t.apply(event(room, ana, 14, RoomInput::Leave), 5, 14);
        assert_eq!(c.last(), Some(&RoomChange::Closed));
        assert!(t.room().is_none());
        // A late copy of an event from before Bea left does not bring the room back.
        assert!(
            t.apply(event(room, bea, 12, here("Bea", 10)), 6, 15)
                .is_empty()
        );
        // Bea coming back later opens it again.
        let c = t.apply(event(room, bea, 16, here("Bea", 10)), 7, 16);
        assert_eq!(c[0], RoomChange::Opened { started_ms: 10 });
    }

    #[test]
    fn a_missed_join_is_learnt_from_here() {
        let (me, ana) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(me);
        let room = [6; 16];
        let c = t.apply(event(room, ana, 20, here("Ana", 3)), 1, 20);
        assert_eq!(
            c,
            vec![
                RoomChange::Opened { started_ms: 3 },
                RoomChange::Joined {
                    device: ana,
                    nick: "Ana".into()
                }
            ]
        );
        assert_eq!(t.snapshot(), vec![1]);
    }

    #[test]
    fn quiet_participants_go_away_and_then_leave() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [2; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        t.apply(event(room, bea, 0, join("Bea", 0)), 2, 0);
        assert!(t.tick(AWAY_MS - 1).is_empty());
        assert_eq!(t.tick(AWAY_MS), vec![RoomChange::Away { device: bea }]);
        let c = t.apply(event(room, bea, 50_000, here("Bea", 0)), 3, 50_000);
        assert_eq!(c, vec![RoomChange::Back { device: bea }]);
        for now in (60_000..50_000 + GRACE_MS).step_by(20_000) {
            t.tick(now);
        }
        let c = t.tick(50_000 + GRACE_MS);
        assert!(c.contains(&RoomChange::Left {
            device: bea,
            on_purpose: false
        }));
        // This device never times out itself.
        assert!(t.joined());
        assert!(t.room().is_some());
    }

    #[test]
    fn a_participant_cut_off_leaves_without_the_grace() {
        let (ana, bea, leo) = (key(), key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [2; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        t.apply(event(room, bea, 0, join("Bea", 0)), 2, 0);
        t.apply(event(room, leo, 0, join("Leo", 0)), 3, 0);
        t.tick(20_000);
        // Bea can no longer be heard from; Leo still can, through someone.
        let cut = |d: &DevicePublicKey| *d == bea;
        assert!(t.drop_cut_off(CUT_OFF_MS - 1, cut).is_empty());
        let c = t.drop_cut_off(CUT_OFF_MS, cut);
        assert_eq!(
            c,
            vec![RoomChange::Left {
                device: bea,
                on_purpose: false
            }]
        );
        assert!(t.is_participant(&leo));
        // Her earlier words do not bring her back; a later one does.
        t.apply(event(room, bea, 0, here("Bea", 0)), 4, 26_000);
        assert!(!t.is_participant(&bea));
        t.apply(event(room, bea, 30_000, here("Bea", 0)), 5, 30_000);
        assert!(t.is_participant(&bea));
        // Not after a sleep: the next tick gives everyone a fresh grace.
        assert!(t.drop_cut_off(20_000 + AWAY_MS, |_| true).is_empty());
    }

    #[test]
    fn waking_from_a_sleep_drops_nobody() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [2; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        t.apply(event(room, bea, 0, join("Bea", 0)), 2, 0);
        assert!(t.tick(20_000).is_empty());
        // Ten minutes asleep: Bea was not heard because this device was not listening.
        assert!(t.tick(620_000).is_empty());
        assert!(t.is_participant(&bea));
        // Still quiet after a fresh grace: now she has gone.
        for now in (640_000..=620_000 + GRACE_MS).step_by(20_000) {
            t.tick(now);
        }
        assert!(!t.is_participant(&bea));
    }

    #[test]
    fn a_line_older_than_a_full_chat_is_not_news() {
        let ana = key();
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [8; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 0, 0);
        for i in 0..MAX_CHAT as u64 {
            let ts = 1_000 + i;
            t.apply(event(room, ana, ts, chat("hi")), ts as u32, 1);
        }
        // A line the chat let go long ago, among another member's room events.
        let changes = t.apply(event(room, ana, 10, chat("old")), 10, 2);
        assert!(changes.is_empty());
        let lines = &t.room().unwrap().chat;
        assert_eq!(lines.len(), MAX_CHAT);
        assert!(lines.iter().all(|l| l.text == "hi"));
        // A newer one still comes in.
        let changes = t.apply(event(room, ana, 5_000, chat("new")), 5_000, 3);
        assert_eq!(changes.len(), 1);
    }

    /// A record of `len` bytes, its first ones its id.
    fn padded(id: u32, len: usize) -> Vec<u8> {
        let mut record = id.to_be_bytes().to_vec();
        record.resize(len.max(4), 0);
        record
    }

    #[test]
    fn records_heavier_than_their_events_are_ignored_and_each_shares_its_part() {
        let (ana, bea, leo) = (key(), key(), key());
        let mut t: RoomTracker<Vec<u8>> = RoomTracker::new(ana).weighing(Vec::len);
        let room = [8; 16];
        let mut n = 0u32;
        let mut next = || {
            n += 1;
            n
        };
        // A join padded far past what it says brings nobody in; one of its size does.
        let id = next();
        t.apply(event(room, ana, 1, join("Ana", 1)), padded(id, 200), 1);
        let id = next();
        let heavy = padded(id, MAX_JOIN_BYTES + 1);
        t.apply(event(room, bea, 2, join("Bea", 1)), heavy, 2);
        assert!(!t.is_participant(&bea));
        for (who, nick) in [(bea, "Bea"), (leo, "Leo")] {
            let id = next();
            t.apply(event(room, who, 3, join(nick, 1)), padded(id, 200), 3);
        }
        assert!(t.is_participant(&bea) && t.is_participant(&leo));
        // A chat line weighs what its text and nickname say and a little more, at most.
        let line = json_len("hi") + json_len("x") + EVENT_SLACK_BYTES;
        let id = next();
        assert!(
            t.apply(event(room, bea, 10, chat("hi")), padded(id, line + 1), 10)
                .is_empty()
        );
        let id = next();
        let changes = t.apply(event(room, bea, 11, chat("hi")), padded(id, line), 11);
        assert!(matches!(changes[..], [RoomChange::Chat(_)]));
        // Control characters are sent escaped: a line of them weighs six times as much.
        let control = "\u{1}".repeat(MAX_CHAT_CHARS);
        let id = next();
        let weight = 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES;
        let changes = t.apply(event(room, bea, 12, chat(&control)), padded(id, weight), 12);
        assert_eq!(changes.len(), 1);
        // A share weighs what its name and picture say, the picture as sent in text.
        let file = |n: u8, thumb: usize| FileInfo {
            id: [n; 32],
            name: format!("f{n}"),
            size: 1,
            video: (thumb > 0).then(|| VideoPreview {
                thumb: vec![0xff; thumb],
                width: 2,
                height: 2,
                duration_ms: 1,
            }),
        };
        let sent = |n: u8, thumb: usize| json_len(&format!("f{n}")) + thumb.div_ceil(3) * 4;
        let share = |n: u8, thumb| RoomInput::Share(file(n, thumb));
        let id = next();
        let padded_share = padded(id, 192 * 1024);
        assert!(
            t.apply(event(room, bea, 20, share(0, 0)), padded_share, 20)
                .is_empty()
        );
        // Each participant shares its part of the room's bytes, here an eighth, and Leo
        // still shares after Bea filled hers.
        let big = 60_000;
        for i in 0..MAX_SHARES_EACH as u8 {
            let (ts, id) = (2_000 + u64::from(i), next());
            let record = padded(id, sent(i, big) + EVENT_SLACK_BYTES);
            t.apply(event(room, bea, ts, share(i, big)), record, 2);
        }
        let cost = |i: u8| SHARE_OVERHEAD + format!("f{i}").len() + big;
        let part = MAX_SHARE_BYTES / usize::from(crate::state::MAX_IN_ROOM);
        let of = |t: &RoomTracker<Vec<u8>>, who| {
            let files = &t.room().unwrap().files;
            files.values().filter(|f| f.shared_by == who).count()
        };
        let bea_shares = of(&t, bea);
        assert!(bea_shares > 0 && bea_shares < MAX_SHARES_EACH);
        let kept: usize = (0..bea_shares as u8).map(cost).sum();
        assert!(kept <= part && kept + cost(bea_shares as u8) > part);
        let id = next();
        let record = padded(id, sent(200, big));
        let changes = t.apply(event(room, leo, 3_000, share(200, big)), record, 3);
        assert!(matches!(changes[..], [RoomChange::FileShared { .. }]));
        // A picture too big to show still counts, as sent.
        let too_big = MAX_THUMB_BYTES + 1_000;
        let id = next();
        let record = padded(id, sent(201, too_big));
        t.apply(event(room, leo, 3_001, share(201, too_big)), record, 3);
        let shared = &t.room().unwrap().share_records;
        assert_eq!(shared[&([201; 32], leo)].1, cost(201) - big + too_big);
        // In a room for two, each shares half.
        t.set_room_for(2, 1);
        for i in 0..MAX_SHARES_EACH as u8 {
            let (ts, id) = (4_000 + u64::from(i), next());
            let record = padded(id, sent(i, big));
            t.apply(event(room, bea, ts, share(i, big)), record, 4);
        }
        let bea_now = of(&t, bea);
        assert!(bea_now > bea_shares);
        let kept: usize = (0..bea_now as u8).map(cost).sum();
        assert!(kept <= MAX_SHARE_BYTES / 2);
    }

    #[test]
    fn the_oldest_chat_line_and_its_record_go_together() {
        let ana = key();
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [8; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 0, 0);
        // Lines arrive newest first, so the line dropped is not the first record kept.
        for i in 0..=MAX_CHAT as u64 {
            let ts = 10_000 - i;
            t.apply(event(room, ana, ts, chat("hi")), ts as u32, 1);
        }
        let chat = &t.room().unwrap().chat;
        assert_eq!(chat.len(), MAX_CHAT);
        let oldest = 10_000 - MAX_CHAT as u32;
        let kept: Vec<u32> = t.snapshot().into_iter().filter(|r| *r != 0).collect();
        assert_eq!(kept.len(), MAX_CHAT);
        assert!(!kept.contains(&oldest), "the dropped line's record stays");
    }

    #[test]
    fn this_sessions_own_join_counts_after_the_clock_went_back() {
        let me = key();
        let room = [7u8; 16];
        let mut t: RoomTracker<u32> = RoomTracker::new(me).since(1000);
        let joined = event(room, me, 900, join("Me", 900));
        t.sending(joined.message_id);
        t.apply(joined, 1, 1000);
        assert!(t.joined());
        assert_eq!(t.take_earlier_session(), None);
    }

    #[test]
    fn a_watcher_sees_the_room_close_when_everyone_drops() {
        let (me, ana) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(me);
        let room = [3; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        assert!(!t.joined());
        let c = t.tick(GRACE_MS);
        assert_eq!(c.last(), Some(&RoomChange::Closed));
    }

    #[test]
    fn rooms_started_at_once_merge_into_the_earlier_one() {
        let (ana, bea, cid) = (key(), key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(cid);
        let (early, late) = ([9; 16], [1; 16]);
        t.apply(event(late, bea, 6, join("Bea", 6)), 1, 6);
        t.apply(event(early, ana, 5, join("Ana", 5)), 2, 6);
        let room = t.room().unwrap();
        assert_eq!((room.id, room.started_ms), (early, 5));
        assert_eq!(room.participants.len(), 2);
        // Events for either id reach the same room.
        let c = t.apply(event(late, bea, 7, chat("hi")), 3, 7);
        assert!(matches!(c[0], RoomChange::Chat(_)));
        assert_eq!(t.room_to_join([0; 16], 8), (early, 5));
    }

    #[test]
    fn files_stay_while_someone_has_them() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [7; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        t.apply(event(room, bea, 0, join("Bea", 0)), 2, 0);
        let info = FileInfo {
            id: [1; 32],
            name: "notes.txt".into(),
            size: 12,
            video: None,
        };
        let c = t.apply(event(room, bea, 1, RoomInput::Share(info.clone())), 3, 1);
        assert_eq!(
            c,
            vec![RoomChange::FileShared {
                file: info.clone(),
                by: bea
            }]
        );
        // Ana downloaded it and shares it too.
        assert!(
            t.apply(event(room, ana, 2, RoomInput::Share(info.clone())), 4, 2)
                .is_empty()
        );
        assert_eq!(t.room().unwrap().files[&info.id].holders.len(), 2);
        let mut all = t.snapshot();
        all.sort();
        assert_eq!(all, vec![1, 2, 3, 4]);
        // Bea leaves: Ana still has it.
        let c = t.apply(event(room, bea, 3, RoomInput::Leave), 5, 3);
        assert!(!c.iter().any(|c| matches!(c, RoomChange::FileGone { .. })));
        // A watcher's view: once the last holder is gone, so is the file.
        let mut w: RoomTracker<u32> = RoomTracker::new(key());
        w.apply(event(room, bea, 0, join("Bea", 0)), 1, 0);
        w.apply(event(room, ana, 0, join("Ana", 0)), 2, 0);
        w.apply(event(room, bea, 1, RoomInput::Share(info.clone())), 3, 1);
        let c = w.apply(event(room, bea, 3, RoomInput::Leave), 4, 3);
        assert!(c.contains(&RoomChange::FileGone { file: info.clone() }));
        assert!(w.room().unwrap().files.is_empty());
        // Someone outside the room cannot share.
        let cid = key();
        assert!(
            w.apply(event(room, cid, 4, RoomInput::Share(info.clone())), 5, 4)
                .is_empty()
        );
        // Stopping a share: the file goes when its last holder stops.
        let c = t.apply(event(room, ana, 5, RoomInput::Unshare(info.id)), 6, 5);
        assert!(c.contains(&RoomChange::FileGone { file: info }));
        assert!(t.room().unwrap().files.is_empty());
    }

    #[test]
    fn chat_is_kept_in_sender_order() {
        let ana = key();
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [4; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        t.apply(event(room, ana, 20, chat("second")), 2, 1);
        t.apply(event(room, ana, 10, chat("first")), 3, 2);
        let texts: Vec<&str> = t
            .room()
            .unwrap()
            .chat
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(texts, ["first", "second"]);
        // A second copy of a line is not shown twice.
        assert!(
            t.apply(event(room, ana, 10, chat("first")), 4, 3)
                .is_empty()
        );
        assert_eq!(t.room().unwrap().chat.len(), 2);
    }

    /// A chat line let go is weighed as kept, goes with its record, and a copy of it that
    /// comes again (among a room's events) is not taken, nor shown again.
    #[test]
    fn a_chat_line_let_go_does_not_come_back() {
        let ana = key();
        let mut t: RoomTracker<u32> = RoomTracker::new(ana).weighing(|r| *r as usize);
        let room = [7; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        t.apply(event(room, ana, 10, chat("first")), 400, 1);
        t.apply(event(room, ana, 20, chat("second")), 500, 2);
        let weights = t.chat_weights();
        let first = event(room, ana, 10, chat("first")).message_id;
        assert_eq!(
            weights[0],
            (ana, 10, first, 400 + "first".len() + 1 + CHAT_LINE_OVERHEAD)
        );
        assert!(t.forget_chat(&first));
        assert!(!t.forget_chat(&first));
        assert_eq!(t.room().unwrap().chat.len(), 1);
        assert_eq!(t.snapshot(), [1, 500]);
        assert!(
            t.apply(event(room, ana, 10, chat("first")), 400, 3)
                .is_empty()
        );
        assert_eq!(t.chat_weights().len(), 1);
    }

    #[test]
    fn a_message_too_long_is_ignored() {
        let ana = key();
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [6; 16];
        t.apply(event(room, ana, 0, join("Ana", 0)), 1, 0);
        let longest = "ñ".repeat(MAX_CHAT_CHARS);
        assert_eq!(t.apply(event(room, ana, 10, chat(&longest)), 2, 1).len(), 1);
        let more = format!("{longest}a");
        assert!(t.apply(event(room, ana, 20, chat(&more)), 3, 2).is_empty());
        assert_eq!(t.room().unwrap().chat.len(), 1);
    }

    #[test]
    fn events_of_an_unknown_room_are_left_for_a_snapshot() {
        let (me, ana) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(me);
        let room = [5; 16];
        assert!(t.is_unknown(&room));
        assert!(t.apply(event(room, ana, 0, chat("hi")), 1, 0).is_empty());
        assert!(t.room().is_none());
    }

    #[test]
    fn an_earlier_session_is_not_left_by_a_device_back_in_the_room() {
        let (me, ana) = (key(), key());
        let (theirs, mine) = ([7u8; 16], [8u8; 16]);
        let mut t: RoomTracker<u32> = RoomTracker::new(me).since(1000);
        // Started again and went straight in, before hearing of the others' room.
        t.apply(event(mine, me, 1001, join("Me", 1001)), 1, 1001);
        // Then the others' room, which still lists the session that crashed.
        t.apply(event(theirs, ana, 900, join("Ana", 900)), 2, 1002);
        t.apply(event(theirs, me, 950, join("Me", 900)), 3, 1002);
        assert!(t.joined());
        assert_eq!(t.take_earlier_session(), None);
        // Even in the other order.
        let mut t: RoomTracker<u32> = RoomTracker::new(me).since(1000);
        t.apply(event(mine, me, 1001, join("Me", 1001)), 1, 1001);
        t.apply(event(theirs, me, 950, join("Me", 900)), 3, 1002);
        t.apply(event(theirs, ana, 900, join("Ana", 900)), 2, 1002);
        assert!(t.joined());
        assert_eq!(t.take_earlier_session(), None);
    }

    #[test]
    fn a_join_from_an_earlier_session_of_this_device_is_left() {
        let (me, ana) = (key(), key());
        let room = [7u8; 16];
        let mut t: RoomTracker<u32> = RoomTracker::new(me).since(1000);
        t.apply(event(room, ana, 900, join("Ana", 900)), 1, 1000);
        // Said by the session that crashed: this device is not in the room now.
        assert!(
            t.apply(event(room, me, 950, join("Me", 900)), 2, 1001)
                .is_empty()
        );
        assert!(!t.joined());
        assert_eq!(t.take_earlier_session(), Some(room));
        assert_eq!(t.take_earlier_session(), None);
        // Its "here" again from another member: already handled.
        t.apply(event(room, me, 960, here("Me", 900)), 3, 1002);
        assert_eq!(t.take_earlier_session(), None);
        // Joining in this session counts.
        t.apply(event(room, me, 1100, join("Me", 900)), 4, 1100);
        assert!(t.joined());
    }

    #[test]
    fn a_device_no_longer_in_the_group_leaves_at_once_and_its_files_with_it() {
        let (me, ana) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(me);
        let room = [8; 16];
        t.apply(event(room, me, 0, join("Me", 0)), 1, 0);
        t.apply(event(room, ana, 1, join("Ana", 0)), 2, 1);
        let file = FileInfo {
            id: [3; 32],
            name: "a.txt".into(),
            size: 1,
            video: None,
        };
        t.apply(event(room, ana, 2, RoomInput::Share(file.clone())), 3, 2);
        assert_eq!(
            t.drop_device(&ana),
            vec![
                RoomChange::Left {
                    device: ana,
                    on_purpose: false
                },
                RoomChange::FileGone { file }
            ]
        );
        assert!(!t.is_participant(&ana));
        assert!(t.joined());
        assert_eq!(t.snapshot(), vec![1]);
        // Nobody else: the room goes.
        let c = t.drop_device(&me);
        assert_eq!(c.last(), Some(&RoomChange::Closed));
        assert!(t.room().is_none());
    }

    #[test]
    fn a_closed_voice_room_takes_everyone_out() {
        let (me, ana) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(me);
        let room = [9; 16];
        t.apply(event(room, me, 0, join("Me", 0)), 1, 0);
        t.apply(event(room, ana, 1, join("Ana", 0)), 2, 1);
        let c = t.close();
        assert!(c.contains(&RoomChange::Left {
            device: me,
            on_purpose: false
        }));
        assert!(c.contains(&RoomChange::Left {
            device: ana,
            on_purpose: false
        }));
        assert_eq!(c.last(), Some(&RoomChange::Closed));
        assert!(!t.joined());
        assert!(t.close().is_empty());
    }

    #[test]
    fn someone_who_went_quiet_is_not_brought_back_by_its_old_events() {
        let (ana, bea, cid) = (key(), key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let old_room = [1; 16];
        t.apply(event(old_room, bea, 1_000, join("Bea", 1_000)), 1, 1_000);
        t.apply(event(old_room, bea, 1_500, chat("hi")), 2, 1_500);
        t.tick(1_500 + GRACE_MS);
        assert!(t.room().is_none());
        // Later, another room; a member sends Bea's join of the first one again.
        let room = [2; 16];
        t.apply(
            event(room, cid, 10_000_000, join("Cid", 10_000_000)),
            3,
            10_000_000,
        );
        t.apply(
            event(old_room, bea, 1_000, join("Bea", 1_000)),
            1,
            10_000_001,
        );
        assert!(!t.is_participant(&bea));
        assert!(
            t.room()
                .is_some_and(|r| r.has_id(&room) && !r.has_id(&old_room))
        );
        // What Bea says now still counts.
        t.apply(
            event(room, bea, 10_000_002, here("Bea", 10_000_000)),
            4,
            10_000_002,
        );
        assert!(t.is_participant(&bea));
    }

    #[test]
    fn an_earlier_rooms_events_sent_again_do_not_attach_it() {
        let (ana, bea, cid) = (key(), key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        // A room of an hour ago, which Bea was in (this device never saw her leave: it
        // started since), and the room now.
        let (old_room, room) = ([1; 16], [2; 16]);
        let (then, now) = (1_000_000, 1_000_000 + 3_600_000);
        t.apply(event(room, cid, now, join("Cid", now)), 1, now);
        let c = t.apply(event(old_room, bea, then, join("Bea", then)), 2, now + 1);
        assert!(c.is_empty());
        assert!(!t.is_participant(&bea));
        assert!(t.is_unknown(&old_room));
        // Nor do its files or its chat.
        let file = FileInfo {
            id: [3; 32],
            name: "f".into(),
            size: 1,
            video: None,
        };
        let shared = t.apply(event(room, cid, then, RoomInput::Share(file)), 3, now + 2);
        assert!(shared.is_empty());
        assert!(
            t.apply(event(old_room, bea, then + 1, chat("hi")), 4, now + 3)
                .is_empty()
        );
        // A room started a few minutes from this one is the same room, as before.
        let other = [3; 16];
        t.apply(
            event(other, bea, now + 120_000, join("Bea", now + 60_000)),
            5,
            now + 120_000,
        );
        assert!(t.is_participant(&bea));
        assert!(!t.is_unknown(&other));
    }

    #[test]
    fn a_room_keeps_only_a_few_other_ids() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        t.apply(event([0; 16], ana, 10, join("Ana", 10)), 1, 10);
        for n in 1..=50u8 {
            t.apply(
                event([n; 16], bea, 10 + u64::from(n), here("Bea", 20)),
                2,
                20,
            );
        }
        let room = t.room().unwrap();
        assert_eq!(room.id, [0; 16]);
        assert_eq!(room.aliases.len(), MAX_ALIASES);
        // The newest are kept.
        assert!(room.has_id(&[50; 16]) && !room.has_id(&[1; 16]));
    }

    #[test]
    fn an_old_presence_or_unshare_sent_again_undoes_nothing() {
        let (ana, bea) = (key(), key());
        let mut t: RoomTracker<u32> = RoomTracker::new(ana);
        let room = [7; 16];
        t.apply(event(room, ana, 10, join("Ana", 10)), 1, 10);
        t.apply(event(room, bea, 11, join("Bea", 10)), 2, 11);
        t.apply(event(room, bea, 30, here("Leo", 10)), 3, 30);
        assert_eq!(t.room().unwrap().participants[&bea].nick, "Leo");
        // Her join, among a room's events, does not give back the name she left.
        t.apply(event(room, bea, 11, join("Bea", 10)), 2, 31);
        t.apply(event(room, bea, 20, here("Bea", 10)), 4, 32);
        assert_eq!(t.room().unwrap().participants[&bea].nick, "Leo");
        // She shares a file again after she stopped: the stop, sent again, does not end it.
        let file = FileInfo {
            id: [1; 32],
            name: "f".into(),
            size: 1,
            video: None,
        };
        t.apply(event(room, bea, 40, RoomInput::Share(file.clone())), 5, 40);
        t.apply(event(room, bea, 41, RoomInput::Unshare(file.id)), 6, 41);
        t.apply(event(room, bea, 42, RoomInput::Share(file.clone())), 7, 42);
        assert!(
            t.apply(event(room, bea, 41, RoomInput::Unshare(file.id)), 6, 43)
                .is_empty()
        );
        assert!(t.room().unwrap().files.contains_key(&file.id));
    }
}
