//! Groups' rooms on the node: room events travel as sealed group messages, spread to the
//! group's members like any group message (see members.rs); members that come online ask a
//! connected member for the room's events so far. Participants name the room's latest
//! events in each presence, and a member that lacks one asks for it, so an event lost on
//! the way arrives within a presence or two.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use fogata_crypto::{DevicePublicKey, GroupId, SealedMessage, random_bytes};
use fogata_proto::{GroupPayload, MemberRequest, WireMessage, WireVideo};
use fogata_state::{FileInfo, RoomChange, RoomEvent, RoomId, RoomInput, VideoPreview, VoiceRoomId};
use libp2p::PeerId;

use super::{Kept, Node, Pending, RecentEvent, limits::Sender, now_ms, spill::Spill};
use crate::{Command, NodeEvent, RoomView};

/// How long after it was sent a member taken out of a voice room still acts on it, and how
/// far ahead of this device's clock its time may be (clocks differ a little).
const REMOVE_FOR_MS: u64 = 2 * 60 * 1000;
const REMOVE_AHEAD_MS: u64 = 60 * 1000;
/// Most takings out remembered, to act on each once.
const MAX_REMOVES: usize = 64;

/// The takings out of a voice room this device acted on, by message, with when they were
/// sent: each counts once, however often a member sends it again while it is still new.
#[derive(Default)]
pub(super) struct Removes(std::collections::HashMap<[u8; 16], u64>);

impl Removes {
    /// Whether a taking out (message `id`, sent at `sent_ms` by the manager's clock) is acted
    /// on at `now_ms`: it is new, not dated ahead of this clock by more than clocks differ,
    /// and was not acted on before. Remembers it if so.
    pub(super) fn take(&mut self, id: [u8; 16], sent_ms: u64, now_ms: u64) -> bool {
        let new = now_ms.saturating_sub(sent_ms) <= REMOVE_FOR_MS
            && sent_ms <= now_ms.saturating_add(REMOVE_AHEAD_MS);
        if !new {
            return false;
        }
        // Gone once too old to act on anyway.
        self.0
            .retain(|_, sent| now_ms.saturating_sub(*sent) <= REMOVE_FOR_MS);
        if self.0.contains_key(&id) {
            return false;
        }
        if self.0.len() >= MAX_REMOVES {
            let oldest = self
                .0
                .iter()
                .min_by_key(|(_, sent)| **sent)
                .map(|(id, _)| *id);
            if let Some(oldest) = oldest {
                self.0.remove(&oldest);
            }
        }
        self.0.insert(id, sent_ms);
        true
    }
}

/// A member asked for the room's events is asked again after this long at the earliest.
const ASK_AGAIN: Duration = Duration::from_secs(15);
/// Most bytes of room events in one answer; a request or answer takes 4 MiB at most.
const ROOM_PART_BYTES: usize = 1024 * 1024;
/// Latest events of a voice room kept, and named in each presence, and for how long: one
/// lost on the way is asked for at the next presence of any participant, every 20 s each.
/// A member that lacks every one of a full list may have lost more, and asks for the whole
/// room again, at most every [`RESYNC_EVERY`].
pub(super) const RECENT: usize = 16;
const RECENT_FOR: Duration = Duration::from_secs(120);
/// Most bytes of a voice room's latest events kept whole, the newest first: what one answer
/// holds. Older ones are kept by id only, still named, so a member that lost them all still
/// asks for the whole room.
const RECENT_BYTES: usize = ROOM_PART_BYTES;
const RESYNC_EVERY: Duration = Duration::from_secs(60);
/// Most groups whose room is asked for again in one heartbeat after the application was far
/// behind (see `Node::ask_rooms_again`).
const ROOMS_AGAIN_PER_BEAT: usize = 3;
/// An event asked for by id is asked for again, if still missing, after this long.
pub(super) const ASK_IDS_AGAIN: Duration = Duration::from_secs(10);
/// Most events asked for, or given, at once.
const MAX_ASKED: usize = 64;

/// Bytes of chat this device keeps from the rooms of all its groups together, at most (as
/// [`fogata_state::RoomTracker::chat_weights`] counts them). Each room keeps its last 500
/// lines, and one member that floods every room of a group with the heaviest lines it may
/// send makes every member keep about 70 MiB of them; nothing bounded that across groups.
/// Honest lines weigh about a kilobyte each as kept: this is some 130 000 of them, which
/// no device in many busy groups comes near, and past it the heaviest sender loses its
/// oldest lines first (see [`fit_chat`]).
pub(super) const ROOM_CHAT_BYTES: usize = 128 * 1024 * 1024;

/// Bytes of the messages of shares this device holds in memory from the rooms of all its
/// groups together, at most (each counted as sent, as `room_record_weight` weighs it). Each
/// group's rooms keep up to 24 MiB of shares as counted (`MAX_SHARE_BYTES`), and a member
/// in many groups that fills them all would make this device hold some 32 MiB more for
/// each. Past this the messages go to a file (see spill.rs), the sender with the most held
/// first, its oldest first (see [`fit_shares`]): each share stays kept and is read back
/// when the room is asked for or shown, so the rooms list and answer the same.
pub(super) const ROOM_SHARE_BYTES: usize = 32 * 1024 * 1024;

impl Node {
    /// Keeps the messages of shares held in memory, from every room of every group, within
    /// [`Self::room_share_budget`] (see [`fit_shares`]), at the same slot as the chat: a
    /// file for them is needed, and one that was found damaged takes no more.
    pub(super) fn fit_room_shares(&mut self) {
        let Some(spill) = self.room_spill.as_mut() else {
            return;
        };
        let trackers = self.groups.values_mut().flat_map(|s| s.rooms.values_mut());
        let moved = fit_shares(trackers, spill, self.room_share_budget);
        if moved > 0 {
            let (file, kept) = spill.bytes();
            tracing::debug!(moved, file, kept, "rooms' shares moved to disk");
        }
    }

    /// Keeps the chat of every room of every group within [`ROOM_CHAT_BYTES`] (see
    /// [`fit_chat`]). Every slot, rather than at each event: going over by what arrives in
    /// one costs little, and looking at every line kept costs more than most events.
    pub(super) fn fit_room_chat(&mut self) {
        let trackers = self.groups.values_mut().flat_map(|s| s.rooms.values_mut());
        let dropped = fit_chat(trackers, ROOM_CHAT_BYTES);
        if dropped > 0 {
            tracing::debug!(dropped, "room chat lines let go to keep within the bound");
        }
    }
}

/// Lets go of chat lines kept by `trackers` until they keep at most `budget` bytes
/// together (see [`fogata_state::RoomTracker::forget_chat`]): each time the oldest line of
/// whoever has the most kept in all of them. A member that floods its rooms (in one group
/// or many, with one device or several) so loses its own lines, and nobody else's while it
/// still keeps more than they do; the others' lines go only once every sender keeps as
/// little as they do. What is kept for the room's other members does not change: each
/// member answers with what it keeps. Returns how many lines went.
fn fit_chat<'a, S: Clone + 'a>(
    trackers: impl Iterator<Item = &'a mut fogata_state::RoomTracker<S>>,
    budget: usize,
) -> usize {
    use std::collections::{BinaryHeap, HashMap};
    let mut trackers: Vec<_> = trackers.collect();
    // A line: its time, its id, its weight and the tracker it is in.
    type Line = (u64, [u8; 16], usize, usize);
    // Every sender's lines, newest first, so the oldest comes off the end.
    let mut lines: HashMap<DevicePublicKey, Vec<Line>> = HashMap::new();
    let mut total = 0usize;
    for (at, tracker) in trackers.iter().enumerate() {
        for (sender, time, id, weight) in tracker.chat_weights() {
            total += weight;
            lines
                .entry(sender)
                .or_default()
                .push((time, id, weight, at));
        }
    }
    if total <= budget {
        return 0;
    }
    let mut senders: BinaryHeap<(usize, DevicePublicKey)> = BinaryHeap::new();
    for (sender, theirs) in lines.iter_mut() {
        theirs.sort_unstable_by_key(|l| std::cmp::Reverse((l.0, l.1)));
        senders.push((theirs.iter().map(|l| l.2).sum(), *sender));
    }
    let mut dropped = 0;
    while total > budget {
        let Some((kept, sender)) = senders.pop() else {
            break;
        };
        let Some((_, id, weight, at)) = lines.get_mut(&sender).and_then(Vec::pop) else {
            continue;
        };
        if trackers[at].forget_chat(&id) {
            dropped += 1;
        }
        total = total.saturating_sub(weight);
        let kept = kept.saturating_sub(weight);
        if lines.get(&sender).is_some_and(|l| !l.is_empty()) {
            senders.push((kept, sender));
        }
    }
    dropped
}

/// Moves the messages of shares that `trackers` hold in memory to `spill` until they hold at
/// most `budget` bytes of them (weighed as sent), as [`fit_chat`] lets chat lines go: each
/// time the oldest share (by its sender's time, then id) of whoever has the most held. One
/// that is also among its room's latest events (the same message) would free nothing, and
/// goes once it is no longer there. The records of messages on disk no room keeps any more
/// are let go of first, so their pages hold the next. Nothing the rooms list, take or
/// answer changes (see `RoomTracker::replace_share_record`). Returns how many moved.
fn fit_shares<'a>(
    trackers: impl Iterator<Item = &'a mut fogata_state::RoomTracker<Kept>>,
    spill: &mut Spill,
    budget: usize,
) -> usize {
    use std::collections::{BinaryHeap, HashMap};
    let mut trackers: Vec<_> = trackers.collect();
    let on_disk: HashSet<u64> = trackers
        .iter()
        .flat_map(|t| t.share_records())
        .filter_map(|(.., kept)| match kept {
            Kept::Spilled { number, .. } => Some(*number),
            Kept::Inline(_) => None,
        })
        .collect();
    spill.keep_only(&on_disk);
    // A file found damaged: nothing more goes to it (see `Node::room_snapshot`).
    if spill.lost() {
        return 0;
    }
    // A share that may move: its time, its file, its weight, its message and the tracker
    // it is in.
    type Share = (u64, [u8; 32], usize, Arc<WireMessage>, usize);
    let mut shares: HashMap<DevicePublicKey, (usize, Vec<Share>)> = HashMap::new();
    let mut total = 0usize;
    for (at, tracker) in trackers.iter().enumerate() {
        for (id, holder, time, kept) in tracker.share_records() {
            let Kept::Inline(wire) = kept else {
                continue;
            };
            let weight = kept.weight();
            total += weight;
            let theirs = shares.entry(holder).or_default();
            theirs.0 += weight;
            if Arc::strong_count(wire) == 1 {
                theirs.1.push((time, id, weight, wire.clone(), at));
            }
        }
    }
    if total <= budget {
        return 0;
    }
    let mut holders: BinaryHeap<(usize, DevicePublicKey)> = BinaryHeap::new();
    for (holder, (held, theirs)) in shares.iter_mut() {
        // Newest first, so the oldest comes off the end.
        theirs.sort_unstable_by_key(|s| std::cmp::Reverse((s.0, s.1)));
        holders.push((*held, *holder));
    }
    let mut moved = 0;
    while total > budget {
        let Some((held, holder)) = holders.pop() else {
            break;
        };
        let Some((_, id, weight, wire, at)) = shares.get_mut(&holder).and_then(|s| s.1.pop())
        else {
            continue;
        };
        let number = match spill.put(&wire) {
            Ok(number) => number,
            Err(e) => {
                tracing::warn!("cannot keep rooms' shares on disk now: {e}");
                break;
            }
        };
        let kept = Kept::Spilled {
            number,
            weight,
            size: wire_size(&wire),
        };
        if trackers[at].replace_share_record(&id, &holder, kept) {
            moved += 1;
            total = total.saturating_sub(weight);
        }
        let held = held.saturating_sub(weight);
        if shares.get(&holder).is_some_and(|s| !s.1.is_empty()) {
            holders.push((held, holder));
        }
    }
    moved
}

/// About how many bytes a sealed message takes in an answer.
fn wire_size(wire: &WireMessage) -> usize {
    wire.group_id.len()
        + wire.sender.len()
        + wire.message_id.len()
        + wire.nonce.len()
        + wire.ciphertext.len()
        + wire.signature.len()
        + 64
}

/// From `all`, the events from `from` on that fit one answer of [`ROOM_PART_BYTES`] (at
/// least one, so a single large event still goes), and where the next part starts if any
/// are left.
pub(crate) fn room_part(all: &[&WireMessage], from: u32) -> (Vec<WireMessage>, Option<u32>) {
    let sizes: Vec<usize> = all.iter().map(|wire| wire_size(wire)).collect();
    let (part, more) = part_of(&sizes, from);
    (all[part].iter().map(|wire| (*wire).clone()).collect(), more)
}

/// Of events of `sizes` (as [`wire_size`] counts them), those from `from` on that fit one
/// answer (see [`room_part`]), and where the next part starts if any are left.
fn part_of(sizes: &[usize], from: u32) -> (std::ops::Range<usize>, Option<u32>) {
    let start = usize::try_from(from).unwrap_or(usize::MAX).min(sizes.len());
    let (mut end, mut bytes) = (start, 0);
    for size in &sizes[start..] {
        if end > start && bytes + size > ROOM_PART_BYTES {
            break;
        }
        bytes += size;
        end += 1;
    }
    let more = (end < sizes.len()).then(|| u32::try_from(end).unwrap_or(u32::MAX));
    (start..end, more)
}

/// Of `messages`, a member's answer with room events, those an honest answer holds: those
/// of the group, as many as one part of [`ROOM_PART_BYTES`] holds and at most `most`.
/// Whether every one was kept.
fn honest_part(
    group: GroupId,
    messages: Vec<WireMessage>,
    most: usize,
) -> (Vec<WireMessage>, bool) {
    let given = messages.len();
    let mut bytes = 0;
    let part: Vec<WireMessage> = messages
        .into_iter()
        .filter(|wire| super::members::group_of(&wire.group_id) == Some(group))
        .take(most)
        .take_while(|wire| {
            bytes += wire_size(wire);
            bytes <= ROOM_PART_BYTES || bytes == wire_size(wire)
        })
        .collect();
    let all = part.len() == given;
    (part, all)
}

/// The voice room and the room an event belongs to, and what it says; none for a manager
/// taking someone out of a room, which is no event of the room itself.
pub(crate) fn room_input(payload: GroupPayload) -> Option<(VoiceRoomId, RoomId, RoomInput)> {
    Some(match payload {
        GroupPayload::RoomJoin {
            voice,
            room,
            started_ms,
            nick,
        } => (voice, room, RoomInput::Join { started_ms, nick }),
        GroupPayload::RoomHere {
            voice,
            room,
            started_ms,
            nick,
            ..
        } => (voice, room, RoomInput::Here { started_ms, nick }),
        GroupPayload::RoomLeave { voice, room } => (voice, room, RoomInput::Leave),
        GroupPayload::RoomChat {
            voice,
            room,
            nick,
            text,
        } => (voice, room, RoomInput::Chat { nick, text }),
        GroupPayload::RoomShare {
            voice,
            room,
            id,
            name,
            size,
            video,
        } => {
            let video = video.map(|v| VideoPreview {
                thumb: v.thumb,
                width: v.width,
                height: v.height,
                duration_ms: v.duration_ms,
            });
            let info = FileInfo {
                id,
                name,
                size,
                video,
            };
            (voice, room, RoomInput::Share(info))
        }
        GroupPayload::RoomUnshare { voice, room, id } => (voice, room, RoomInput::Unshare(id)),
        GroupPayload::RoomRemove { .. } => return None,
    })
}

impl Node {
    /// Opens the room events `peer` answered an ask for a room's past with (`asked`: by id,
    /// at most [`MAX_ASKED`]; or a part of the whole room). Each is a signature check, seen
    /// before or not, so a member is held to what an honest answer holds and to a budget
    /// of its own (see limits.rs); the rest are dropped. Whether every one was opened: only
    /// then is the next part asked for.
    pub(super) fn open_room_log(
        &mut self,
        peer: PeerId,
        group: GroupId,
        messages: Vec<WireMessage>,
        asked: bool,
    ) -> bool {
        let most = if asked { MAX_ASKED } else { usize::MAX };
        let (mut part, mut all) = honest_part(group, messages, most);
        let sender = self
            .members
            .get(&peer)
            .map_or(Sender::Peer(peer), |m| Sender::Device(m.device));
        let taken = self
            .limits
            .take_room_log(sender, part.len(), Instant::now());
        if taken < part.len() {
            tracing::debug!("a member answered with more room events than this device opens now");
            part.truncate(taken);
            all = false;
        }
        for wire in part {
            self.receive(peer, wire, super::spread::Forward::No);
        }
        all
    }

    /// The voice room of `group` this device is in, and the id of its room now.
    fn joined_room(&self, group: &GroupId) -> Option<(VoiceRoomId, RoomId)> {
        let (voice, tracker) = self.groups.get(group)?.joined()?;
        Some((voice, tracker.room()?.id))
    }

    /// Whether `device` may not enter the voice room of `group` this device is in.
    pub(super) fn kept_out_of_my_room(&self, group: &GroupId, device: &DevicePublicKey) -> bool {
        let Some(slot) = self.groups.get(group) else {
            return false;
        };
        slot.joined()
            .is_some_and(|(voice, _)| slot.ctx.barrier(&voice, device, now_ms()).is_some())
    }

    /// Whether this device is in any room.
    pub(super) fn in_a_room(&self) -> Option<GroupId> {
        self.groups
            .iter()
            .find(|(_, s)| s.joined().is_some())
            .map(|(id, _)| *id)
    }

    pub(super) fn on_room_command(&mut self, command: Command) {
        match command {
            Command::JoinRoom { group, voice, nick } => {
                let me = self.identity.public_key();
                if self
                    .groups
                    .get(&group)
                    .is_some_and(|slot| slot.ctx.barrier(&voice, &me, now_ms()).is_some())
                {
                    self.info("this device may not enter that voice room now");
                    return;
                }
                let mut fresh = [0u8; 16];
                if random_bytes(&mut fresh).is_err() {
                    return;
                }
                // One voice room of a group at a time: leave the other one first.
                if let Some((other, room)) = self.joined_room(&group)
                    && other != voice
                {
                    self.send_payload(group, GroupPayload::RoomLeave { voice: other, room });
                }
                let Some(slot) = self.groups.get_mut(&group) else {
                    return;
                };
                let (room, started_ms) = slot.tracker(voice).room_to_join(fresh, now_ms());
                self.send_payload(
                    group,
                    GroupPayload::RoomJoin {
                        voice,
                        room,
                        started_ms,
                        nick,
                    },
                );
            }
            Command::LeaveRoom(group, reply) => {
                if let Some((voice, room)) = self.joined_room(&group) {
                    self.send_payload(group, GroupPayload::RoomLeave { voice, room });
                }
                let _ = reply.send(());
            }
            Command::RoomChat { group, text, reply } => {
                let Some((voice, room)) = self.joined_room(&group) else {
                    let _ = reply.send(Err("join the room first".into()));
                    return;
                };
                let nick = self.room_nick(&group).unwrap_or_default();
                let sent = self.try_send_payload(
                    group,
                    GroupPayload::RoomChat {
                        voice,
                        room,
                        nick,
                        text,
                    },
                );
                let _ = reply.send(sent);
            }
            Command::Room(group, voice, reply) => {
                // Asked for the screens, which show the videos' first pictures.
                let mut view = self
                    .groups
                    .get(&group)
                    .and_then(|slot| slot.rooms.get(&voice))
                    .and_then(room_view);
                if let Some(view) = view.as_mut() {
                    self.show_pictures(group, voice, &mut view.files);
                }
                let _ = reply.send(view);
            }
            Command::Rooms(group, reply) => {
                let views = self
                    .groups
                    .get(&group)
                    .map(|slot| {
                        slot.rooms
                            .iter()
                            .filter_map(|(voice, tracker)| Some((*voice, room_view(tracker)?)))
                            .collect()
                    })
                    .unwrap_or_default();
                let _ = reply.send(views);
            }
            Command::RemoveFromRoom {
                group,
                voice,
                member,
            } => {
                let Some(room) = self
                    .groups
                    .get(&group)
                    .and_then(|slot| slot.rooms.get(&voice))
                    .and_then(|tracker| tracker.room())
                    .map(|room| room.id)
                else {
                    return;
                };
                self.send_payload(
                    group,
                    GroupPayload::RoomRemove {
                        voice,
                        room,
                        member: *member.as_bytes(),
                    },
                );
            }
            Command::UnshareFile { group, id } => {
                if let Some((voice, room)) = self.joined_room(&group) {
                    self.send_payload(group, GroupPayload::RoomUnshare { voice, room, id });
                }
            }
            Command::ShareFile {
                group,
                mut file,
                reply,
            } => {
                let Some((voice, room)) = self.joined_room(&group) else {
                    let _ = reply.send(Err("join the room first".into()));
                    return;
                };
                // A file of the room shared on from here (downloaded) goes with the picture
                // its share in the room has, which the room lists without it.
                if let Some(video) = file.video.as_mut()
                    && video.thumb.is_empty()
                    && let Some(thumb) = self.room_picture(group, &file.id)
                {
                    video.thumb = thumb.to_vec();
                }
                let sent = self.try_send_payload(
                    group,
                    GroupPayload::RoomShare {
                        voice,
                        room,
                        id: file.id,
                        name: file.name,
                        size: file.size,
                        video: file.video.map(|v| WireVideo {
                            thumb: v.thumb,
                            width: v.width,
                            height: v.height,
                            duration_ms: v.duration_ms,
                        }),
                    },
                );
                let _ = reply.send(sent);
            }
            _ => {}
        }
    }

    /// Seals a room event, applies it here and sends it to the group's connected members;
    /// what kept it from going is said in the log.
    fn send_payload(&mut self, group: GroupId, payload: GroupPayload) {
        if let Err(problem) = self.try_send_payload(group, payload) {
            self.info(problem);
        }
    }

    /// Seals a room event, applies it here and sends it to the group's connected members,
    /// or says why it cannot.
    fn try_send_payload(&mut self, group: GroupId, payload: GroupPayload) -> Result<(), String> {
        let keys = self
            .groups
            .get(&group)
            .and_then(|s| s.ctx.current().cloned())
            .ok_or("this device does not have the group key yet")?;
        let at = self.seal_time();
        let sealed = SealedMessage::seal(&keys, &self.identity, at, &payload.encode())
            .map_err(|e| format!("message not sent: {e}"))?;
        self.remember(sealed.header.message_id);
        let wire = WireMessage::from(&sealed);
        self.apply_room(group, &sealed, payload, wire.clone(), None);
        self.deliver(group, wire);
        Ok(())
    }

    /// Applies an opened group message from `from` (or from here) to `group`'s room.
    pub(super) fn apply_room(
        &mut self,
        group: GroupId,
        sealed: &SealedMessage,
        payload: GroupPayload,
        wire: WireMessage,
        from: Option<PeerId>,
    ) {
        if let GroupPayload::RoomRemove {
            voice,
            room,
            member,
        } = &payload
        {
            self.on_room_remove(group, *voice, *room, sealed, member);
            return;
        }
        let Some((voice, room, input)) = room_input(payload) else {
            return;
        };
        // Kept once: the room's record and its place among the latest events are the same
        // message.
        let wire = Arc::new(wire);
        let in_room_before = self.in_a_room();
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        // A voice room the group does not have: nothing is kept for it, so made-up ids cannot
        // make this device keep rooms without end.
        if !slot.ctx.has_voice_room(&voice) {
            return;
        }
        // Someone kept out of the room: nothing it says there counts but that it leaves.
        if !matches!(input, RoomInput::Leave)
            && slot
                .ctx
                .barrier(&voice, &sealed.header.sender, now_ms())
                .is_some()
        {
            return;
        }
        // Events that change the room are kept to name in presences; presences come again.
        if !matches!(input, RoomInput::Here { .. }) {
            keep_recent(
                slot.recent.entry(voice).or_default(),
                room,
                sealed.header.message_id,
                &wire,
            );
        }
        let (room_for, rooms) = (slot.ctx.max_in_room(), slot.ctx.voice_rooms());
        let tracker = slot.tracker(voice);
        tracker.set_room_for(room_for, rooms);
        if from.is_none() {
            tracker.sending(sealed.header.message_id);
        }
        // Something this device cannot place: a room it does not know, or chat or a file from
        // someone it does not know to be in the room. The room's events will tell.
        let unknown = (tracker.is_unknown(&room) && !matches!(input, RoomInput::Join { .. }))
            || (matches!(input, RoomInput::Chat { .. } | RoomInput::Share(_))
                && !tracker.is_participant(&sealed.header.sender));
        let event = RoomEvent {
            room,
            sender: sealed.header.sender,
            timestamp_ms: sealed.header.timestamp_ms,
            message_id: sealed.header.message_id,
            input,
        };
        let changes = tracker.apply(event, Kept::Inline(wire), now_ms());
        let earlier = tracker.take_earlier_session();
        // Not taken now, and marked as seen, so nothing else brings it again: the room's
        // events tell. Asked of that member a moment ago (on entering the room, say), the
        // room is asked for again at the next heartbeat, or the event is lost for good.
        if unknown
            && let Some(peer) = from
            && !self.ask_room(group, peer)
            && let Some(slot) = self.groups.get_mut(&group)
        {
            slot.room_again.get_or_insert_default();
        }

        self.after_room_changes(group, voice, changes, in_room_before);
        // The room still lists this device from a session that ended without a goodbye:
        // say it left, so nobody (this device included) shows it there.
        if let Some(room) = earlier {
            tracing::debug!("left a room an earlier session of this device was in");
            self.send_payload(group, GroupPayload::RoomLeave { voice, room });
        }
    }

    /// A manager takes `member` out of voice room `voice`, in the sitting `room`: when that
    /// is this device and it is in that very sitting, it leaves it now. The others see it
    /// leave like anyone who does. One sent more than [`REMOVE_FOR_MS`] ago (by the
    /// manager's clock), or dated ahead of this clock, is old or false news, and each counts
    /// once: a member could otherwise send it again later to take this device out once more.
    fn on_room_remove(
        &mut self,
        group: GroupId,
        voice: VoiceRoomId,
        room: RoomId,
        sealed: &SealedMessage,
        member: &[u8; 32],
    ) {
        let me = self.identity.public_key();
        let by = sealed.header.sender;
        if member != me.as_bytes() {
            return;
        }
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        if !slot.ctx.outranks(&by, &me) {
            return;
        }
        if self.joined_room(&group) != Some((voice, room)) {
            return;
        }
        let (id, sent_ms) = (sealed.header.message_id, sealed.header.timestamp_ms);
        if !self.room_removes.take(id, sent_ms, now_ms()) {
            return;
        }
        self.send_payload(group, GroupPayload::RoomLeave { voice, room });
        self.emit(NodeEvent::TakenOut {
            group,
            voice,
            by: Some(by),
            barrier: None,
        });
    }

    fn after_room_changes(
        &mut self,
        group: GroupId,
        voice: VoiceRoomId,
        changes: Vec<RoomChange>,
        in_room_before: Option<GroupId>,
    ) {
        for change in changes {
            self.emit(NodeEvent::Room {
                group,
                voice,
                change,
            });
        }
        self.tell_room_changed(in_room_before);
    }

    /// The log of `group` changed: a voice room it no longer has is closed with everyone in
    /// it, and devices that are no longer members (this one included) leave every room at
    /// once. Neither would ever be heard leaving otherwise: events of a voice room the group
    /// does not have are dropped, and only members may speak in the group. Others now kept
    /// out of a room are dropped from it the same way; this device, kept out of the room it
    /// is in, leaves it.
    pub(super) fn rooms_follow_log(&mut self, group: GroupId) {
        let in_room_before = self.in_a_room();
        let me = self.identity.public_key();
        let now = now_ms();
        if let Some((voice, room)) = self.joined_room(&group)
            && let Some(barrier) = self
                .groups
                .get(&group)
                .and_then(|slot| slot.ctx.barrier(&voice, &me, now))
        {
            self.send_payload(group, GroupPayload::RoomLeave { voice, room });
            self.emit(NodeEvent::TakenOut {
                group,
                voice,
                by: match barrier {
                    fogata_state::Barrier::TimedOut(kept) => Some(kept.by),
                    fogata_state::Barrier::NotChosen => None,
                },
                barrier: Some(barrier),
            });
        }
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let mut changed = Vec::new();
        let voices: Vec<VoiceRoomId> = slot.rooms.keys().copied().collect();
        for voice in voices {
            let changes = if slot.ctx.has_voice_room(&voice) {
                let tracker = slot.rooms.get_mut(&voice).expect("listed above");
                let gone: Vec<DevicePublicKey> = tracker
                    .room()
                    .map(|room| {
                        room.participants
                            .keys()
                            .filter(|device| {
                                !slot.ctx.is_member_device(device)
                                    || (**device != me
                                        && slot.ctx.barrier(&voice, device, now).is_some())
                            })
                            .copied()
                            .collect()
                    })
                    .unwrap_or_default();
                gone.iter()
                    .flat_map(|device| tracker.drop_device(device))
                    .collect()
            } else {
                slot.rooms
                    .remove(&voice)
                    .map(|mut tracker| tracker.close())
                    .unwrap_or_default()
            };
            changed.push((voice, changes));
        }
        for (voice, changes) in changed {
            for change in changes {
                self.emit(NodeEvent::Room {
                    group,
                    voice,
                    change,
                });
            }
        }
        self.tell_room_changed(in_room_before);
    }

    /// Asks a member for `group`'s room events, not more often than [`ASK_AGAIN`]. While
    /// the application is far behind, when the answer would not be opened, the room is
    /// asked for again once it is not, of one member (see `room_heartbeat`), and this one
    /// counts as asked. Returns false when that member was asked too recently to ask again.
    pub(super) fn ask_room(&mut self, group: GroupId, peer: PeerId) -> bool {
        let full = self.events.full();
        let Some(slot) = self.groups.get_mut(&group) else {
            return false;
        };
        if slot
            .room_asked
            .get(&peer)
            .is_some_and(|at| at.elapsed() < ASK_AGAIN)
        {
            return false;
        }
        slot.room_asked.insert(peer, Instant::now());
        if full {
            slot.room_again.get_or_insert_default();
            return true;
        }
        self.request(
            peer,
            MemberRequest::RoomSync {
                group_id: group.as_bytes().to_vec(),
                from: 0,
            },
            Pending::RoomSync(group, 0),
        );
        true
    }

    /// This device took another name: in the rooms it is in under the earlier one, it is
    /// known by the new one at once, also by members it has no connection to, which never
    /// get its profile and know it only by the name it gives in the room.
    pub(super) fn renamed_in_rooms(&mut self, before: &str, after: &str) {
        let groups: Vec<GroupId> = self.groups.keys().copied().collect();
        for group in groups {
            if self.room_nick(&group).as_deref() == Some(before) {
                self.say_here_as(group, after.to_string());
            }
        }
    }

    /// Says this device is in a voice room of `group`, if it is.
    pub(super) fn say_here(&mut self, group: GroupId) {
        let nick = self.room_nick(&group).unwrap_or_default();
        self.say_here_as(group, nick);
    }

    /// [`Self::say_here`], under the name `nick`.
    fn say_here_as(&mut self, group: GroupId, nick: String) {
        let Some((voice, tracker)) = self.groups.get(&group).and_then(|s| s.joined()) else {
            return;
        };
        let Some(room) = tracker.room() else {
            return;
        };
        let (room, started_ms) = (room.id, room.started_ms);
        let recent = self
            .groups
            .get(&group)
            .and_then(|s| s.recent.get(&voice))
            .map(|events| {
                events
                    .iter()
                    .filter(|e| e.room == room && e.at.elapsed() < RECENT_FOR)
                    .flat_map(|e| e.id)
                    .collect()
            })
            .unwrap_or_default();
        self.send_payload(
            group,
            GroupPayload::RoomHere {
                voice,
                room,
                started_ms,
                nick,
                recent,
            },
        );
    }

    /// Every heartbeat, for each group: say we are still in its room, ask members whose
    /// answer did not come yet (for example because they refused while they had not verified
    /// this device), and let quiet participants go.
    pub(super) fn room_heartbeat(&mut self) {
        self.ask_rooms_again();
        let groups: Vec<GroupId> = self.groups.keys().copied().collect();
        for group in groups {
            self.say_here(group);
            // A room asked for again is asked of one member at a time (see above).
            let unasked: Vec<PeerId> = self
                .groups
                .get(&group)
                .filter(|s| s.room_again.is_none())
                .map(|s| {
                    s.members
                        .iter()
                        .filter(|p| !s.room_asked.contains_key(p))
                        .copied()
                        .collect()
                })
                .unwrap_or_default();
            for peer in unasked {
                self.ask_room(group, peer);
            }
            let in_room_before = self.in_a_room();
            let Some(slot) = self.groups.get_mut(&group) else {
                continue;
            };
            let changes: Vec<(VoiceRoomId, Vec<RoomChange>)> = slot
                .rooms
                .iter_mut()
                .map(|(voice, tracker)| (*voice, tracker.tick(now_ms())))
                .collect();
            for (voice, changes) in changes {
                self.after_room_changes(group, voice, changes, in_room_before);
            }
        }
    }

    /// Room events refused while the application was far behind come back only with the
    /// room asked for whole: a member's word of what it said lately names only the last two
    /// minutes of it. Once the application is not behind, the rooms of the groups something
    /// was refused in are asked for again, of one member each, at most
    /// [`ROOMS_AGAIN_PER_BEAT`] groups a heartbeat, groups and members in random order;
    /// another member only when the one asked did not give it (see `on_response`). Asking
    /// every member of every group at once would bring the same room back from each and
    /// could fill the queue again.
    fn ask_rooms_again(&mut self) {
        if self.events.full() {
            return;
        }
        let mut waiting: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, s)| s.room_again.is_some())
            .map(|(g, _)| *g)
            .collect();
        shuffle(&mut waiting);
        let mut asked = 0;
        for group in waiting {
            if asked == ROOMS_AGAIN_PER_BEAT {
                break;
            }
            let Some(slot) = self.groups.get_mut(&group) else {
                continue;
            };
            let tried = slot.room_again.get_or_insert_default();
            // Members asked a moment ago, whose answer may still be on its way, wait.
            let mut untried: Vec<PeerId> = slot
                .members
                .iter()
                .filter(|p| !tried.contains(*p))
                .copied()
                .collect();
            if untried.is_empty() {
                // Each member connected was asked and none gave it: those that connect
                // later are asked as they come (`ask_room`).
                if !slot.members.is_empty() {
                    slot.room_again = None;
                }
                continue;
            }
            if slot
                .room_asked
                .iter()
                .any(|(p, at)| tried.contains(p) && at.elapsed() < ASK_AGAIN)
            {
                continue;
            }
            shuffle(&mut untried);
            let peer = untried[0];
            tried.insert(peer);
            slot.room_asked.insert(peer, Instant::now());
            asked += 1;
            self.request(
                peer,
                MemberRequest::RoomSync {
                    group_id: group.as_bytes().to_vec(),
                    from: 0,
                },
                Pending::RoomSync(group, 0),
            );
        }
    }

    /// Takes out of the rooms the participants this device has no way left to hear from
    /// (see `RoomTracker::drop_cut_off`): those it has no connection to, when each member it
    /// is connected to in the group said, in a fresh roster, that it reaches nobody else. A
    /// member whose roster is not in, or that reaches a peer this device is not connected
    /// to, may carry anyone's words. A device with no connection at all has most likely lost
    /// its own network, and gives the others the grace period.
    pub(super) fn drop_cut_off(&mut self) {
        if self.connections.is_empty() {
            return;
        }
        let me = self.local_peer();
        let groups: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, s)| !s.rooms.is_empty())
            .map(|(id, _)| *id)
            .collect();
        for group in groups {
            let slot = &self.groups[&group];
            let carried = slot
                .members
                .iter()
                .any(|peer| match slot.rosters.get(peer) {
                    Some((listed, at)) if at.elapsed() < super::spread::ROSTER_FRESH => listed
                        .iter()
                        .any(|other| *other != me && !slot.members.contains(other)),
                    _ => true,
                });
            if carried {
                continue;
            }
            let connected: HashSet<DevicePublicKey> = slot
                .members
                .iter()
                .filter_map(|peer| self.members.get(peer))
                .map(|m| m.device)
                .collect();
            let in_room_before = self.in_a_room();
            let now = now_ms();
            let slot = self.groups.get_mut(&group).expect("listed above");
            let changes: Vec<(VoiceRoomId, Vec<RoomChange>)> = slot
                .rooms
                .iter_mut()
                .map(|(voice, tracker)| {
                    let changes = tracker.drop_cut_off(now, |d| !connected.contains(d));
                    (*voice, changes)
                })
                .filter(|(_, changes)| !changes.is_empty())
                .collect();
            for (voice, changes) in changes {
                tracing::debug!(?changes, "participants nobody can hear from any more");
                self.after_room_changes(group, voice, changes, in_room_before);
            }
        }
    }

    /// The events of `group`'s voice rooms for a member that asked, from event `from` on,
    /// as many as fit one answer, and where the next part starts if more are left. Who is in
    /// each room comes first, then the files and the chat: a room with long chats and many
    /// videos holds far more than one answer takes, and an answer too big never arrives.
    /// Shares' messages kept on disk are read back, so the answer is the same as with all
    /// in memory. None once the file of those messages failed to give one back, for any
    /// group that has messages there: an answer without them would be another room than
    /// the one this device lists, so the member asks the others (it asks again later, and
    /// is refused until this device starts again).
    pub(super) fn room_snapshot(
        &self,
        group: GroupId,
        from: u32,
    ) -> Option<(Vec<WireMessage>, Option<u32>)> {
        let Some(slot) = self.groups.get(&group) else {
            return Some((Vec::new(), None));
        };
        rooms_part(slot.rooms.values(), self.room_spill.as_ref(), from)
    }

    /// The message of a room event as sent, read back from disk if it went there; none when
    /// it cannot be (see `Spill::read`).
    fn read_kept(&self, kept: &Kept) -> Option<WireMessage> {
        read_kept(kept, self.room_spill.as_ref())
    }

    /// A participant's presence named `recent` events (ids one after the other); asks for
    /// those this device lacks: from the participant itself when connected to it, or from
    /// `from`, which passed the presence on. A device that does not take the ask is asked
    /// for the whole room instead, and so is one whose whole full list is missing here.
    pub(super) fn ask_missing(
        &mut self,
        group: GroupId,
        author: DevicePublicKey,
        from: PeerId,
        recent: &[u8],
    ) {
        // Asked for once the application has caught up (see events.rs): they would not be
        // opened now.
        if self.events.full() {
            return;
        }
        let named: Vec<[u8; 16]> = recent
            .as_chunks::<16>()
            .0
            .iter()
            .take(RECENT)
            .copied()
            .collect();
        let missing: Vec<[u8; 16]> = named
            .iter()
            .filter(|id| !self.seen.contains(*id))
            .copied()
            .collect();
        if missing.is_empty() {
            return;
        }
        let author_peer = self.groups.get(&group).and_then(|slot| {
            self.members
                .iter()
                .find(|(p, m)| m.device == author && slot.members.contains(*p))
                .map(|(p, _)| *p)
        });
        let ask = author_peer
            .filter(|p| self.takes_spread(p))
            .or_else(|| self.takes_spread(&from).then_some(from));
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let now = Instant::now();
        slot.asked_ids
            .retain(|_, at| now.duration_since(*at) < ASK_IDS_AGAIN);
        let all_lost = named.len() == RECENT && missing.len() == named.len();
        let resync = all_lost
            && slot
                .recent_resync
                .is_none_or(|t| now.duration_since(t) >= RESYNC_EVERY);
        if resync {
            slot.recent_resync = Some(now);
        }
        let wanted: Vec<u8> = missing
            .iter()
            .filter(|id| !slot.asked_ids.contains_key(*id))
            .take(MAX_ASKED)
            .flatten()
            .copied()
            .collect();
        for id in wanted.as_chunks::<16>().0 {
            slot.asked_ids.insert(*id, now);
        }
        match ask {
            Some(peer) if !wanted.is_empty() => {
                tracing::debug!(
                    count = wanted.len() / 16,
                    "asking for room events missing here"
                );
                self.request(
                    peer,
                    MemberRequest::RoomEvents {
                        group_id: group.as_bytes().to_vec(),
                        ids: wanted,
                    },
                    Pending::RoomEvents(group),
                );
            }
            None if !wanted.is_empty() => {
                self.ask_room(group, from);
            }
            _ => {}
        }
        if resync {
            tracing::debug!("every recent room event is missing here; asking for the room");
            self.ask_room(group, ask.unwrap_or(from));
        }
    }

    /// The room events of `group` kept here among `ids` (one after the other), as many as
    /// fit one answer (see [`ROOM_PART_BYTES`]); the rest are asked for again later.
    pub(super) fn recent_events(&self, group: GroupId, ids: &[u8]) -> Vec<WireMessage> {
        let Some(slot) = self.groups.get(&group) else {
            return Vec::new();
        };
        let wanted = ids.as_chunks::<16>().0;
        let wanted = &wanted[..wanted.len().min(MAX_ASKED)];
        let found: Vec<&WireMessage> = slot
            .recent
            .values()
            .flatten()
            .filter(|e| wanted.contains(&e.id))
            .filter_map(|e| e.wire.as_deref())
            .collect();
        room_part(&found, 0).0
    }

    /// This device's nickname in the voice room of `group` it is in.
    fn room_nick(&self, group: &GroupId) -> Option<String> {
        let me = self.identity.public_key();
        let (_, tracker) = self.groups.get(group)?.joined()?;
        tracker
            .room()?
            .participants
            .get(&me)
            .map(|p| p.nick.clone())
    }
}

/// The first pictures of the videos of the room the screens showed last, read from their
/// shares' records: the rooms keep a share's picture in its record only (see
/// `RoomTracker::keeping_pictures`), so a room nobody looks at holds it once, as sent, and
/// the room shown holds it read as well, until another is shown.
#[derive(Default)]
pub(super) struct ShownPictures {
    room: Option<(GroupId, VoiceRoomId)>,
    pictures: HashMap<[u8; 32], Arc<[u8]>>,
}

impl Node {
    /// Puts in `files`, of `group`'s voice room `voice` shown now, the first pictures of
    /// its videos: those read already, and the others read from their shares' records.
    fn show_pictures(&mut self, group: GroupId, voice: VoiceRoomId, files: &mut [crate::RoomFile]) {
        if self.shown_pictures.room != Some((group, voice)) {
            self.shown_pictures = ShownPictures {
                room: Some((group, voice)),
                pictures: HashMap::new(),
            };
        }
        // Those of files no longer listed go.
        self.shown_pictures
            .pictures
            .retain(|id, _| files.iter().any(|f| f.info.id == *id));
        for file in files {
            let id = file.info.id;
            let Some(video) = file.info.video.as_mut().filter(|v| v.thumb.is_empty()) else {
                continue;
            };
            let picture = match self.shown_pictures.pictures.get(&id) {
                Some(picture) => picture.clone(),
                None => {
                    let Some(picture) = self.read_picture(group, voice, &id) else {
                        continue;
                    };
                    self.shown_pictures.pictures.insert(id, picture.clone());
                    picture
                }
            };
            video.thumb = picture.to_vec();
        }
    }

    /// The first picture of video `id` as a share of it in one of `group`'s rooms has it.
    fn room_picture(&self, group: GroupId, id: &[u8; 32]) -> Option<Arc<[u8]>> {
        let voices: Vec<VoiceRoomId> = self.groups.get(&group)?.rooms.keys().copied().collect();
        voices
            .into_iter()
            .find_map(|voice| self.read_picture(group, voice, id))
    }

    /// Reads the first picture of video `id` from the record of its share in `group`'s
    /// voice room `voice`: opened again with the key of its epoch, which the group keeps
    /// while its log does.
    fn read_picture(&self, group: GroupId, voice: VoiceRoomId, id: &[u8; 32]) -> Option<Arc<[u8]>> {
        let slot = self.groups.get(&group)?;
        let record = slot.rooms.get(&voice)?.share_record(id)?;
        let sealed = SealedMessage::try_from(&self.read_kept(record)?).ok()?;
        let keys = slot.ctx.keys(sealed.header.epoch)?;
        let payload = sealed.open(keys).ok()?;
        match GroupPayload::decode(&payload).ok()? {
            GroupPayload::RoomShare {
                video: Some(video), ..
            } => Some(video.thumb.into()),
            _ => None,
        }
    }
}

/// The events of the rooms `trackers` keep from event `from` on, as many as fit one answer,
/// and where the next part starts if more are left (see `Node::room_snapshot`), shares'
/// messages on disk read back from `spill`; none when the file was found damaged and the
/// rooms have messages there.
fn rooms_part<'a>(
    trackers: impl Iterator<Item = &'a fogata_state::RoomTracker<Kept>>,
    spill: Option<&Spill>,
    from: u32,
) -> Option<(Vec<WireMessage>, Option<u32>)> {
    let (mut joins, mut rest) = (Vec::new(), Vec::new());
    for tracker in trackers {
        let (j, r) = tracker.snapshot_parts();
        joins.extend(j);
        rest.extend(r);
    }
    let all: Vec<&Kept> = joins.into_iter().chain(rest).collect();
    let spilled = |kept: &&Kept| matches!(kept, Kept::Spilled { .. });
    if spill.is_some_and(Spill::lost) && all.iter().any(spilled) {
        return None;
    }
    // A message on disk takes what it took in memory, so the parts end at the same events.
    let sizes: Vec<usize> = all
        .iter()
        .map(|kept| match kept {
            Kept::Inline(wire) => wire_size(wire),
            Kept::Spilled { size, .. } => *size,
        })
        .collect();
    let (part, more) = part_of(&sizes, from);
    let part = all[part]
        .iter()
        .map(|kept| read_kept(kept, spill))
        .collect::<Option<Vec<WireMessage>>>()?;
    Some((part, more))
}

/// The message of a room event as sent, read back from `spill` if it went there.
fn read_kept(kept: &Kept, spill: Option<&Spill>) -> Option<WireMessage> {
    match kept {
        Kept::Inline(wire) => Some((**wire).clone()),
        Kept::Spilled { number, .. } => spill?.read(*number).ok(),
    }
}

/// Keeps a room event among a voice room's latest (see [`RECENT`]), once: whole while the
/// newest fit [`RECENT_BYTES`] (the newest one always), by id past that.
fn keep_recent(
    events: &mut VecDeque<RecentEvent>,
    room: RoomId,
    id: [u8; 16],
    wire: &Arc<WireMessage>,
) {
    if events.iter().any(|e| e.id == id) {
        return;
    }
    if events.len() >= RECENT {
        events.pop_front();
    }
    events.push_back(RecentEvent {
        room,
        id,
        at: Instant::now(),
        wire: Some(wire.clone()),
    });
    let mut bytes = 0;
    for (newer, event) in events.iter_mut().rev().enumerate() {
        bytes += event.wire.as_deref().map_or(0, wire_size);
        if newer > 0 && bytes > RECENT_BYTES {
            event.wire = None;
        }
    }
}

/// What a voice room shows, if a room is open in it.
fn room_view(tracker: &fogata_state::RoomTracker<Kept>) -> Option<RoomView> {
    tracker.room().map(|room| RoomView {
        started_ms: room.started_ms,
        participants: room
            .participants
            .iter()
            .map(|(device, p)| (*device, p.nick.clone(), p.away))
            .collect(),
        joined: tracker.joined(),
        files: room.files.values().cloned().collect(),
        chat: room.chat.clone(),
    })
}

/// Puts `items` in a random order.
fn shuffle<T>(items: &mut [T]) {
    for i in (1..items.len()).rev() {
        let mut bytes = [0u8; 8];
        if fogata_crypto::random_bytes(&mut bytes).is_err() {
            return;
        }
        let j = (u64::from_le_bytes(bytes) % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use fogata_state::GroupLog;
    use libp2p::swarm::ConnectionId;

    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{
            start,
            tests::{add_member, inspect},
        },
    };

    #[test]
    fn a_taking_out_counts_once_and_only_while_new() {
        let now = 1_000_000_000;
        let mut removes = super::Removes::default();
        // Dated well ahead of this clock: it would stay new for as long, and be sent again.
        assert!(!removes.take([1; 16], now + super::REMOVE_AHEAD_MS + 1, now));
        // Old news.
        assert!(!removes.take([2; 16], now - super::REMOVE_FOR_MS - 1, now));
        // New, a little ahead of this clock: once.
        assert!(removes.take([3; 16], now + 5_000, now));
        assert!(!removes.take([3; 16], now + 5_000, now + 10_000));
        assert!(removes.take([4; 16], now, now));
    }

    use fogata_crypto::DeviceIdentity;
    use fogata_proto::{MEMBER_PROTOCOL, MemberRequest, MemberResponse};
    use fogata_state::{RoomEvent, RoomTracker};
    use libp2p::{StreamProtocol, request_response::Codec as _};

    use super::*;
    use crate::behaviour::{MAX_MESSAGE_BYTES, message_codec};

    /// Requests made of a peer so far: those on their way and those waiting their turn
    /// (only so many go to one peer at once: asked for again and again, as these tests
    /// do of peers that never answer, one more waits and is no less asked for).
    fn sent(outbox: &super::super::Outbox) -> usize {
        outbox.in_flight + outbox.waiting.len()
    }

    fn record(id: u8, ciphertext: usize) -> WireMessage {
        WireMessage {
            version: 1,
            group_id: vec![1; 16],
            epoch: 3,
            sender: vec![2; 32],
            timestamp_ms: 1_000,
            message_id: vec![id; 16],
            nonce: vec![4; 24],
            ciphertext: vec![5; ciphertext],
            signature: vec![6; 64],
        }
    }

    /// A folder of its own for a test's file of shares' messages.
    fn spill_dir(name: &str) -> std::path::PathBuf {
        let mut random = [0u8; 8];
        fogata_crypto::random_bytes(&mut random).unwrap();
        std::env::temp_dir().join(format!("fogata-room-{name}-{}", hex::encode(random)))
    }

    /// The answer as it goes on the wire, and whether the member that asked can read it.
    fn answer(messages: Vec<WireMessage>) -> (usize, bool) {
        futures::executor::block_on(async {
            let protocol = StreamProtocol::new(MEMBER_PROTOCOL);
            let mut codec = message_codec::<MemberRequest, MemberResponse>();
            let mut wire = Vec::new();
            codec
                .write_response(
                    &protocol,
                    &mut wire,
                    MemberResponse::RoomLog {
                        messages,
                        more: None,
                    },
                )
                .await
                .unwrap();
            let size = wire.len();
            let mut io = futures::io::Cursor::new(wire);
            (size, codec.read_response(&protocol, &mut io).await.is_ok())
        })
    }

    /// A voice room's latest events are kept whole within a few bytes: sixteen of the
    /// largest a member may send would be 3 MiB a room. Past them, the older ones are kept by
    /// id, still named in presences; the newest stays whole however large.
    #[test]
    fn a_rooms_latest_events_are_kept_within_a_few_bytes() {
        let room = [1; 16];
        let mut events = VecDeque::new();
        for id in 0..RECENT as u8 {
            keep_recent(
                &mut events,
                room,
                [id; 16],
                &Arc::new(record(id, 192 * 1024)),
            );
        }
        let whole: Vec<&RecentEvent> = events.iter().filter(|e| e.wire.is_some()).collect();
        let bytes: usize = whole
            .iter()
            .filter_map(|e| e.wire.as_deref())
            .map(wire_size)
            .sum();
        assert!(bytes <= RECENT_BYTES, "{bytes} bytes kept");
        assert_eq!(events.len(), RECENT);
        assert_eq!(whole.last().map(|e| e.id), Some([RECENT as u8 - 1; 16]));
        // Only those kept whole are given to a member that asks.
        let ids: Vec<u8> = (0..RECENT as u8).flat_map(|id| [id; 16]).collect();
        let given: usize = events
            .iter()
            .filter(|e| ids.as_chunks::<16>().0.contains(&e.id))
            .filter_map(|e| e.wire.as_deref())
            .count();
        assert_eq!(given, whole.len());
        // One larger than all of that is still kept whole, alone.
        keep_recent(
            &mut events,
            room,
            [99; 16],
            &Arc::new(record(99, 2 * RECENT_BYTES)),
        );
        let whole: Vec<[u8; 16]> = events
            .iter()
            .filter(|e| e.wire.is_some())
            .map(|e| e.id)
            .collect();
        assert_eq!(whole, [[99; 16]]);
        // Small ones are all kept whole.
        let mut small = VecDeque::new();
        for id in 0..RECENT as u8 {
            keep_recent(&mut small, room, [id; 16], &Arc::new(record(id, 1024)));
        }
        assert!(small.iter().all(|e| e.wire.is_some()));
    }

    /// A room's events in the parts a member that asks gets them in, read back from `spill`
    /// where they went there, with where each part said the next starts.
    fn all_parts(
        tracker: &RoomTracker<Kept>,
        spill: Option<&Spill>,
    ) -> Vec<(Vec<WireMessage>, Option<u32>)> {
        let mut parts = Vec::new();
        let mut from = Some(0);
        while let Some(at) = from {
            let (part, more) = rooms_part(std::iter::once(tracker), spill, at).unwrap();
            parts.push((part, more));
            from = more;
        }
        parts
    }

    /// The same messages, byte for byte, in the same parts.
    fn same_parts(
        a: &[(Vec<WireMessage>, Option<u32>)],
        b: &[(Vec<WireMessage>, Option<u32>)],
    ) -> bool {
        format!("{a:?}") == format!("{b:?}")
    }

    /// Moves every share `tracker` keeps in memory to a new file, as past the device's
    /// bound; the file and its folder.
    fn all_on_disk(tracker: &mut RoomTracker<Kept>, name: &str) -> (Spill, std::path::PathBuf) {
        let dir = spill_dir(name);
        let mut spill = Spill::open(dir.clone()).unwrap();
        let moved = fit_shares(std::iter::once(&mut *tracker), &mut spill, 0);
        assert_eq!(moved, tracker.share_records().count());
        (spill, dir)
    }

    /// With its shares' messages on disk too: the same parts, read back from there. Prints
    /// how long a part read from disk takes.
    #[test]
    fn a_room_of_many_videos_still_reaches_a_member_that_asks() {
        let me = DeviceIdentity::generate().unwrap().public_key();
        let ana = DeviceIdentity::generate().unwrap().public_key();
        let room = [9; 16];
        let mut tracker = RoomTracker::new(me);
        let join = RoomEvent {
            room,
            sender: ana,
            timestamp_ms: 1_000,
            message_id: [0; 16],
            input: fogata_state::RoomInput::Join {
                started_ms: 1_000,
                nick: "Ana".into(),
            },
        };
        tracker.apply(join, Kept::Inline(Arc::new(record(0, 200))), 1_000);
        // A hundred videos, each with a first picture near the largest a room keeps.
        for i in 1..=100u8 {
            let share = RoomEvent {
                room,
                sender: ana,
                timestamp_ms: 1_000 + u64::from(i),
                message_id: [i; 16],
                input: fogata_state::RoomInput::Share(fogata_state::FileInfo {
                    id: [i; 32],
                    name: format!("video {i}.mp4"),
                    size: 1_000_000,
                    video: None,
                }),
            };
            tracker.apply(share, Kept::Inline(Arc::new(record(i, 90_000))), 1_000);
        }
        let all: Vec<WireMessage> = tracker
            .snapshot()
            .iter()
            .map(|kept| read_kept(kept, None).unwrap())
            .collect();
        assert_eq!(all.len(), 101);
        let (size, read) = answer(all);
        assert!(size as u64 > MAX_MESSAGE_BYTES && !read);

        let parts = all_parts(&tracker, None);
        let mut received = Vec::new();
        for (part, _) in &parts {
            let (size, read) = answer(part.clone());
            assert!(size as u64 <= MAX_MESSAGE_BYTES && read);
            received.extend(part.iter().cloned());
        }
        assert_eq!(received[0].message_id, vec![0; 16], "the join goes first");
        assert_eq!(received.len(), 101, "every event arrives over the parts");

        let (spill, dir) = all_on_disk(&mut tracker, "videos");
        let started = Instant::now();
        let (part, _) = rooms_part(std::iter::once(&tracker), Some(&spill), 0).unwrap();
        println!(
            "a part of {} shares ({} KiB) read back from disk in {:?}",
            part.len(),
            part.iter().map(wire_size).sum::<usize>() >> 10,
            started.elapsed()
        );
        assert!(same_parts(&all_parts(&tracker, Some(&spill)), &parts));
        drop(spill);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_answer_with_more_room_events_than_a_part_holds_is_cut() {
        let group = GroupId::from_bytes(&[1; 16]).unwrap();
        // What an honest member sends whole: a part, and the events asked for by id.
        let all: Vec<WireMessage> = (0..200).map(|i| record(i, 90_000)).collect();
        let refs: Vec<&WireMessage> = all.iter().collect();
        let (part, _) = room_part(&refs, 0);
        let (kept, whole) = honest_part(group, part.clone(), usize::MAX);
        assert!(whole && kept.len() == part.len());
        let asked: Vec<WireMessage> = (0..MAX_ASKED as u8).map(|i| record(i, 1_000)).collect();
        let (kept, whole) = honest_part(group, asked, MAX_ASKED);
        assert!(whole && kept.len() == MAX_ASKED);
        // Tens of thousands of small ones, or more than were asked for: only a part's worth
        // is opened, and the next part is not asked for.
        let many: Vec<WireMessage> = (0..20_000).map(|i| record(i as u8, 16)).collect();
        let (kept, whole) = honest_part(group, many.clone(), usize::MAX);
        assert!(!whole);
        assert!(kept.iter().map(wire_size).sum::<usize>() <= ROOM_PART_BYTES);
        let (kept, whole) = honest_part(group, many, MAX_ASKED);
        assert!(!whole && kept.len() == MAX_ASKED);
        // Events of another group are not opened.
        let mut other = record(1, 16);
        other.group_id = vec![2; 16];
        let (kept, whole) = honest_part(group, vec![other], usize::MAX);
        assert!(kept.is_empty() && !whole);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_taking_out_acts_only_on_the_sitting_it_names() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        // Bea made the group and manages it; Ana is a member.
        let mut log = GroupLog::create(&bea, "g", 1).unwrap();
        log.add(&bea, ana.public_key(), "Ana", 2).unwrap();
        let group = log.group_id();
        let (node, mut events) = start(NodeConfig {
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
            spill_dir: None,
        })
        .unwrap();
        let by = bea.public_key();
        let joined = inspect(&node, move |n| {
            let voice = fogata_state::FIRST_ROOM;
            n.on_room_command(Command::JoinRoom {
                group,
                voice,
                nick: "Ana".into(),
            });
            let (voice, room) = n.joined_room(&group)?;
            let me = *n.identity.public_key().as_bytes();
            let order = |id: u8| fogata_crypto::SealedMessage {
                header: fogata_crypto::MessageHeader {
                    version: 1,
                    group_id: group,
                    epoch: 0,
                    sender: by,
                    timestamp_ms: now_ms(),
                    message_id: [id; 16],
                },
                nonce: [0; fogata_crypto::NONCE_LEN],
                ciphertext: Vec::new(),
                signature: [0; fogata_crypto::SIGNATURE_LEN],
            };
            // An order for another sitting of that room (an earlier one, sent again): Ana
            // stays. One for the sitting she is in: she leaves.
            let mut earlier = room;
            earlier[0] ^= 1;
            n.on_room_remove(group, voice, earlier, &order(1), &me);
            let stayed = n.joined_room(&group).is_some();
            n.on_room_remove(group, voice, room, &order(2), &me);
            Some(stayed)
        })
        .await;
        assert_eq!(
            joined,
            Some(true),
            "not in the room, or taken out of the wrong sitting"
        );
        let mut taken_out = 0;
        while let Ok(event) = events.try_recv() {
            taken_out += usize::from(matches!(event, NodeEvent::TakenOut { .. }));
        }
        assert_eq!(taken_out, 1);
        node.shutdown().await.unwrap();
    }

    /// Out of a stall (the application far behind), the room of each group something was
    /// refused in is asked for again of one member, a few groups a heartbeat, never of every
    /// member of every group at once; another member of a group only when the one asked did
    /// not give it, and none once one did.
    #[tokio::test(flavor = "multi_thread")]
    async fn out_of_a_stall_each_room_is_asked_of_one_member_a_few_groups_at_a_time() {
        use fogata_proto::{MemberResponse, WireMessage};
        // Room asks of a heartbeat: by group, the members asked.
        fn beat(n: &mut Node) -> HashMap<GroupId, Vec<PeerId>> {
            let before: HashMap<PeerId, usize> =
                n.outbox.iter().map(|(p, o)| (*p, sent(o))).collect();
            n.room_heartbeat();
            let mut asked: HashMap<GroupId, Vec<PeerId>> = HashMap::new();
            for (peer, outbox) in &n.outbox {
                if sent(outbox) > before.get(peer).copied().unwrap_or(0) {
                    let group = n
                        .groups
                        .iter()
                        .find(|(_, s)| s.members.contains(peer))
                        .map(|(g, _)| *g)
                        .expect("a member");
                    asked.entry(group).or_default().push(*peer);
                }
            }
            asked
        }
        let per_beat = super::ROOMS_AGAIN_PER_BEAT;
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let logs: Vec<GroupLog> = (0..10)
            .map(|i| GroupLog::create(&ana, &format!("g{i}"), 1).unwrap())
            .collect();
        let groups: Vec<GroupId> = logs.iter().map(|l| l.group_id()).collect();
        let (node, events) = start(NodeConfig {
            identity: ana,
            groups: logs.into_iter().map(GroupSetup::Member).collect(),
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
            spill_dir: None,
        })
        .unwrap();
        let backlog = events.backlog();
        // All in one go: the members are made up, and asks of them fail as soon as the node
        // runs again.
        inspect(&node, move |n| {
            // Twenty members in each group, each asked for the room already.
            for group in &groups {
                for _ in 0..20 {
                    let peer = PeerId::random();
                    add_member(n, *group, peer);
                    let slot = n.groups.get_mut(group).unwrap();
                    slot.room_asked.insert(peer, Instant::now());
                }
            }
            // Behind: a message of each of the first eight groups is refused.
            backlog.set(true);
            for group in &groups[..8] {
                let peer = *n.groups[group].members.iter().next().unwrap();
                let message = WireMessage {
                    version: 1,
                    group_id: group.as_bytes().to_vec(),
                    epoch: 0,
                    sender: vec![0; 32],
                    timestamp_ms: 0,
                    message_id: vec![0; 16],
                    nonce: vec![0; 24],
                    ciphertext: vec![0; 64],
                    signature: vec![0; 64],
                };
                assert!(matches!(
                    n.on_request(peer, MemberRequest::Deliver(message)),
                    MemberResponse::Refused
                ));
            }
            assert!(beat(n).is_empty(), "asked while still behind");

            backlog.set(false);
            let first = beat(n);
            assert_eq!(first.len(), per_beat, "groups asked: {first:?}");
            assert!(first.values().all(|peers| peers.len() == 1));
            assert!(first.keys().all(|g| groups[..8].contains(g)));
            // Others next, the members asked a moment ago having their answer to give.
            let second = beat(n);
            assert_eq!(second.len(), per_beat);
            assert!(second.keys().all(|g| !first.contains_key(g)));
            assert!(second.values().all(|peers| peers.len() == 1));

            // One group's member gave the room, another's did not.
            let mut asked = first.iter();
            let (gave, gave_peer) = asked.next().map(|(g, p)| (*g, p[0])).unwrap();
            let (failed, failed_peer) = asked.next().map(|(g, p)| (*g, p[0])).unwrap();
            n.on_response(
                gave_peer,
                Pending::RoomSync(gave, 0),
                MemberResponse::RoomLog {
                    messages: Vec::new(),
                    more: None,
                },
            );
            n.on_response(
                failed_peer,
                Pending::RoomSync(failed, 0),
                MemberResponse::Refused,
            );
            assert!(n.groups[&gave].room_again.is_none());
            // The asks of the others were a while ago.
            for group in &groups {
                let slot = n.groups.get_mut(group).unwrap();
                for at in slot.room_asked.values_mut() {
                    *at -= Duration::from_secs(60);
                }
            }
            let mut others = Vec::new();
            for _ in 0..6 {
                let asked = beat(n);
                // Never more than a few groups at once, one member each, and never the
                // groups nothing was refused in nor the one whose room came.
                assert!(asked.len() <= per_beat, "groups asked: {asked:?}");
                assert!(asked.values().all(|peers| peers.len() == 1));
                assert!(asked.keys().all(|g| groups[..8].contains(g) && *g != gave));
                others.extend(asked.get(&failed).cloned().unwrap_or_default());
            }
            // The group whose member failed: another member, once its turn came.
            assert!(
                others.iter().any(|p| *p != failed_peer),
                "no other member of the group whose member failed was asked"
            );
        })
        .await;
        drop(events);
        node.shutdown().await.unwrap();
    }

    /// However many stalls come, the room is asked for again once the application is not
    /// behind: a member whose answer came while the queue was full again is one to ask once
    /// more, also once every member answered so. And a member asked that goes makes way for
    /// another at once, without waiting for an answer that will not come.
    /// A file shared by a member this device does not know yet to be in the room, just after
    /// the room was asked of that member (as on entering it), is not lost: the share is not
    /// taken, the ask cannot go again yet, and the share is marked as seen, so nothing else
    /// would ever bring it. The room is asked for again at the next heartbeat instead.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_share_from_someone_not_yet_seen_in_the_room_has_the_room_asked_again() {
        // The room's events asked of `peer` and not answered yet.
        fn room_asks(n: &Node, peer: PeerId) -> usize {
            n.pending
                .values()
                .filter(|(p, kind)| *p == peer && matches!(kind, Pending::RoomSync(..)))
                .count()
        }
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        let group = log.group_id();
        let (node, events) = start(NodeConfig {
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
            spill_dir: None,
        })
        .unwrap();
        let asked_again = inspect(&node, move |n| {
            let peer = PeerId::random();
            add_member(n, group, peer);
            n.members.get_mut(&peer).unwrap().device = bea.public_key();
            n.on_room_command(Command::JoinRoom {
                group,
                voice: fogata_state::FIRST_ROOM,
                nick: "Ana".into(),
            });
            let (voice, room) = n.joined_room(&group).unwrap();
            // The room was asked of Bea a moment ago.
            n.groups
                .get_mut(&group)
                .unwrap()
                .room_asked
                .insert(peer, Instant::now());
            let keys = n.groups[&group].ctx.current().cloned().unwrap();
            let share = GroupPayload::RoomShare {
                voice,
                room,
                id: [7; 32],
                name: "lake.png".into(),
                size: 50_000,
                video: None,
            };
            let sealed = SealedMessage::seal(&keys, &bea, now_ms(), &share.encode()).unwrap();
            n.receive(
                peer,
                WireMessage::from(&sealed),
                super::super::spread::Forward::Flooded,
            );
            let before = room_asks(n, peer);
            n.room_heartbeat();
            room_asks(n, peer) > before
        })
        .await;
        assert!(asked_again, "the room was not asked for again");
        drop(events);
        node.shutdown().await.unwrap();
    }

    /// The rooms keep a video's first picture once, in its share's record: the room lists it
    /// with an empty picture, the screens get it read from the record, those of the room
    /// shown last alone are kept read, and what the room answers a member that asks is the
    /// records as they were sent, pictures and all. A file shared on from here goes with
    /// the room's picture. All of it the same with the shares' messages on disk, past what
    /// the device holds in memory, but for those among the room's latest events; and with
    /// that file damaged, the room is no longer given. Prints how long reading a full room's
    /// pictures takes.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_rooms_pictures_are_kept_once_and_read_for_the_room_shown() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let group = log.group_id();
        let dir = spill_dir("pictures");
        let on_disk_file = dir.join("shares");
        let spill_file = on_disk_file.clone();
        let (node, events) = start(NodeConfig {
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
            spill_dir: Some(dir.clone()),
        })
        .unwrap();
        let checked = inspect(&node, move |n| {
            let voice = fogata_state::FIRST_ROOM;
            n.on_room_command(Command::JoinRoom {
                group,
                voice,
                nick: "Ana".into(),
            });
            let picture = |i: u8| vec![i; fogata_state::MAX_THUMB_BYTES];
            let video = |i: u8| FileInfo {
                id: [i; 32],
                name: format!("clip {i}.mp4"),
                size: 1_000_000,
                video: Some(VideoPreview {
                    thumb: picture(i),
                    width: 640,
                    height: 360,
                    duration_ms: 10_000,
                }),
            };
            // As many as one person's part of a room takes.
            let mut shared = 0;
            for i in 0..60u8 {
                let (reply, _) = tokio::sync::oneshot::channel();
                n.on_room_command(Command::ShareFile {
                    group,
                    file: video(i),
                    reply,
                });
                let listed = n.groups[&group].rooms[&voice].room().unwrap().files.len();
                if listed == shared {
                    break;
                }
                shared = listed;
            }
            assert!(shared > 30, "{shared} shared");
            // Past what the device holds in memory: all go to disk but those among the
            // room's latest events, which would free nothing.
            n.room_share_budget = 0;
            n.fit_room_shares();
            let tracker = &n.groups[&group].rooms[&voice];
            let on_disk = tracker
                .share_records()
                .filter(|(.., kept)| matches!(kept, Kept::Spilled { .. }))
                .count();
            assert!(
                on_disk > shared / 2 && on_disk < shared,
                "{on_disk} on disk"
            );
            assert!(matches!(
                tracker.share_record(&[3; 32]),
                Some(Kept::Spilled { .. })
            ));
            let room = tracker.room().unwrap();
            assert!(
                room.files
                    .values()
                    .all(|f| f.info.video.as_ref().unwrap().thumb.is_empty())
            );
            // Shown: every picture read from its record.
            let mut files = room_view(tracker).unwrap().files;
            let started = Instant::now();
            n.show_pictures(group, voice, &mut files);
            let took = started.elapsed();
            println!(
                "{shared} pictures of {} KiB read in {took:?}",
                picture(0).len() >> 10
            );
            for f in &files {
                assert_eq!(f.info.video.as_ref().unwrap().thumb, picture(f.info.id[0]));
            }
            assert_eq!(n.shown_pictures.pictures.len(), shared);
            // What a member that asks gets: the records as sent, each with its picture, in
            // as many parts as it takes.
            let mut answer = Vec::new();
            let mut from = Some(0);
            while let Some(at) = from {
                let (part, next) = n.room_snapshot(group, at).unwrap();
                answer.extend(part);
                from = next;
            }
            let keys = n.groups[&group].ctx.current().cloned().unwrap();
            let with_pictures = answer
                .iter()
                .filter_map(|wire| {
                    let sealed = SealedMessage::try_from(wire).ok()?;
                    match GroupPayload::decode(&sealed.open(&keys).ok()?).ok()? {
                        GroupPayload::RoomShare {
                            id, video: Some(v), ..
                        } => Some(v.thumb == picture(id[0])),
                        _ => None,
                    }
                })
                .filter(|whole| *whole)
                .count();
            assert_eq!(with_pictures, shared);
            // The room's picture of a file, for sharing it on.
            assert_eq!(
                n.room_picture(group, &[3; 32]).as_deref(),
                Some(&picture(3)[..])
            );
            // Shared on from here (downloaded, so without its picture): it goes with the
            // picture read from disk.
            let mut downloaded = video(3);
            downloaded.video.as_mut().unwrap().thumb = Vec::new();
            let (reply, _) = tokio::sync::oneshot::channel();
            n.on_room_command(Command::ShareFile {
                group,
                file: downloaded,
                reply,
            });
            let Some(Kept::Inline(wire)) = n.groups[&group].rooms[&voice].share_record(&[3; 32])
            else {
                panic!("not shared on");
            };
            let sealed = SealedMessage::try_from(&**wire).unwrap();
            let sent = GroupPayload::decode(&sealed.open(&keys).unwrap()).unwrap();
            assert!(matches!(
                sent,
                GroupPayload::RoomShare { video: Some(v), .. } if v.thumb == picture(3)
            ));
            // The file damaged under the device: the room is no longer given, rather than
            // given without the shares it cannot read back.
            let len = std::fs::metadata(&spill_file).unwrap().len();
            std::fs::write(&spill_file, vec![0; len as usize]).unwrap();
            assert!(n.room_snapshot(group, 0).is_none());
            assert!(n.room_picture(group, &[5; 32]).is_none());
            n.fit_room_shares();
            // Another room shown: the first one's pictures are no longer kept read.
            let other: VoiceRoomId = [5; 8];
            n.show_pictures(group, other, &mut []);
            n.shown_pictures.pictures.is_empty()
        })
        .await;
        assert!(checked);
        drop(events);
        node.shutdown().await.unwrap();
        assert!(!on_disk_file.exists(), "the file outlived the node");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Someone left the room and came back, and the join that said so did not reach this
    /// device (or not yet): its call signals do. The room is asked of it at once, not at its
    /// next presence, and the signals still reach the application, which keeps them until
    /// the room says it is in. However many it sends, the room is asked of it once.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_call_signal_from_someone_not_seen_in_the_room_has_the_room_asked() {
        fn room_asks(n: &Node, peer: PeerId) -> usize {
            n.pending
                .values()
                .filter(|(p, kind)| *p == peer && matches!(kind, Pending::RoomSync(..)))
                .count()
        }
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        let group = log.group_id();
        let (node, mut events) = start(NodeConfig {
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
            spill_dir: None,
        })
        .unwrap();
        let bea_key = bea.public_key();
        let asks = inspect(&node, move |n| {
            let peer = PeerId::random();
            add_member(n, group, peer);
            n.members.get_mut(&peer).unwrap().device = bea_key;
            n.on_room_command(Command::JoinRoom {
                group,
                voice: fogata_state::FIRST_ROOM,
                nick: "Ana".into(),
            });
            // Asked of Bea long ago, when she connected.
            n.groups
                .get_mut(&group)
                .unwrap()
                .room_asked
                .insert(peer, Instant::now() - Duration::from_secs(60));
            let signal = || MemberRequest::Signal {
                group_id: group.as_bytes().to_vec(),
                payload: br#"{"t":"again"}"#.to_vec(),
            };
            let before = room_asks(n, peer);
            let mut taken = 0;
            let mut asks = Vec::new();
            for _ in 0..3 {
                taken += usize::from(matches!(n.on_request(peer, signal()), MemberResponse::Ack));
                asks.push(room_asks(n, peer) - before);
            }
            (taken, asks)
        })
        .await;
        assert_eq!(asks.1, [1, 1, 1], "the room asked of Bea: {:?}", asks.1);
        assert_eq!(asks.0, 3, "signals of hers taken");
        let mut signals = 0;
        while let Ok(event) = events.try_recv() {
            signals +=
                usize::from(matches!(event, NodeEvent::Signal { from, .. } if from == bea_key));
        }
        assert_eq!(signals, 3);
        node.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_room_is_asked_again_after_stall_after_stall() {
        use fogata_proto::MemberResponse;
        fn asked(n: &mut Node) -> Vec<PeerId> {
            let before: HashMap<PeerId, usize> =
                n.outbox.iter().map(|(p, o)| (*p, sent(o))).collect();
            n.room_heartbeat();
            n.outbox
                .iter()
                .filter(|(p, o)| sent(o) > before.get(*p).copied().unwrap_or(0))
                .map(|(p, _)| *p)
                .collect()
        }
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let group = log.group_id();
        let (node, events) = start(NodeConfig {
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
            spill_dir: None,
        })
        .unwrap();
        let backlog = events.backlog();
        inspect(&node, move |n| {
            let members: Vec<PeerId> = (0..3).map(|_| PeerId::random()).collect();
            for peer in &members {
                add_member(n, group, *peer);
                n.groups
                    .get_mut(&group)
                    .unwrap()
                    .room_asked
                    .insert(*peer, Instant::now());
            }
            n.groups.get_mut(&group).unwrap().room_again = Some(HashSet::new());
            // Each member, asked in turn, answers while the queue is full again.
            for _ in 0..members.len() + 2 {
                backlog.set(false);
                let peers = asked(n);
                assert_eq!(peers.len(), 1, "asked {peers:?}");
                backlog.set(true);
                n.on_response(
                    peers[0],
                    Pending::RoomSync(group, 0),
                    MemberResponse::RoomLog {
                        messages: Vec::new(),
                        more: None,
                    },
                );
                assert!(
                    n.groups[&group].room_again.is_some(),
                    "the room was given up"
                );
            }
            // Not behind any more: asked again, and given at last.
            backlog.set(false);
            let peers = asked(n);
            assert_eq!(peers.len(), 1);
            n.on_response(
                peers[0],
                Pending::RoomSync(group, 0),
                MemberResponse::RoomLog {
                    messages: Vec::new(),
                    more: None,
                },
            );
            assert!(n.groups[&group].room_again.is_none());

            // Asked again; the member asked goes before it answers: another, at once.
            n.groups.get_mut(&group).unwrap().room_again = Some(HashSet::new());
            for at in n.groups.get_mut(&group).unwrap().room_asked.values_mut() {
                *at -= Duration::from_secs(60);
            }
            let first = asked(n);
            assert_eq!(first.len(), 1);
            assert!(
                asked(n).is_empty(),
                "asked another while the first may answer"
            );
            n.on_connection_closed(first[0], ConnectionId::new_unchecked(9), 0);
            let next = asked(n);
            assert_eq!(next.len(), 1);
            assert_ne!(next, first);
        })
        .await;
        drop(events);
        node.shutdown().await.unwrap();
    }

    /// Rooms of `groups` groups, in each of which `honest` senders say `lines` lines of
    /// `chars` characters each, the first half before and the rest after a member that
    /// floods says `flood` of the heaviest lines it may send.
    fn chat_rooms(
        groups: usize,
        honest: &[DevicePublicKey],
        lines: usize,
        chars: usize,
        flooder: DevicePublicKey,
        flood: usize,
    ) -> Vec<RoomTracker<WireMessage>> {
        use fogata_state::{EVENT_SLACK_BYTES, MAX_CHAT_CHARS};
        let me = DeviceIdentity::generate().unwrap().public_key();
        let mut n = 0u32;
        let mut send = |t: &mut RoomTracker<WireMessage>, sender, input: RoomInput, weight| {
            n += 1;
            let mut message_id = [0u8; 16];
            message_id[..4].copy_from_slice(&n.to_be_bytes());
            let mut wire = record(0, 0);
            wire.message_id = message_id.to_vec();
            wire.ciphertext = vec![5; weight - crate::group::room_record_weight(&wire)];
            let event = RoomEvent {
                room: [1; 16],
                sender,
                timestamp_ms: 1_000 + u64::from(n),
                message_id,
                input,
            };
            t.apply(event, wire, 1_000);
        };
        let chat = |text: String| RoomInput::Chat {
            nick: "m".into(),
            text,
        };
        let honest_line = "a".repeat(chars);
        // The text as the message encodes it, and the rest of a sealed message.
        let honest_weight = chars + 300;
        let heavy = "\u{1}".repeat(MAX_CHAT_CHARS);
        let heavy_weight = 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES;
        (0..groups)
            .map(|_| {
                let mut t = RoomTracker::new(me).weighing(crate::group::room_record_weight);
                for sender in honest.iter().chain([&flooder]) {
                    let join = RoomInput::Join {
                        started_ms: 1_000,
                        nick: "m".into(),
                    };
                    send(&mut t, *sender, join, 300);
                }
                for i in 0..lines {
                    if i == lines / 2 {
                        for _ in 0..flood {
                            send(&mut t, flooder, chat(heavy.clone()), heavy_weight);
                        }
                    }
                    for sender in honest {
                        send(&mut t, *sender, chat(honest_line.clone()), honest_weight);
                    }
                }
                t
            })
            .collect()
    }

    fn kept(trackers: &[RoomTracker<WireMessage>], sender: DevicePublicKey) -> (usize, usize) {
        let theirs: Vec<usize> = trackers
            .iter()
            .flat_map(|t| t.chat_weights())
            .filter(|(s, ..)| *s == sender)
            .map(|(.., weight)| weight)
            .collect();
        (theirs.len(), theirs.iter().sum())
    }

    /// A member that floods every room of many groups with the heaviest lines loses its own
    /// lines, the oldest first, until the device keeps what the bound allows; the others'
    /// lines, older and newer than the flood, all stay. Two that flood lose theirs alike.
    #[test]
    fn a_flood_of_chat_in_many_groups_loses_only_its_own_lines() {
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let (ana, bea, bad, worse) = (key(), key(), key(), key());
        let mut trackers = chat_rooms(6, &[ana, bea], 40, 120, bad, 400);
        trackers.extend(chat_rooms(4, &[ana], 40, 120, worse, 450));
        let total = |t: &[RoomTracker<WireMessage>]| -> usize {
            t.iter().flat_map(|t| t.chat_weights()).map(|l| l.3).sum()
        };
        let before = total(&trackers);
        let times = |t: &[RoomTracker<WireMessage>]| -> Vec<u64> {
            let mut times: Vec<u64> = t
                .iter()
                .flat_map(|t| t.chat_weights())
                .filter(|(s, ..)| *s == bad)
                .map(|(_, time, ..)| time)
                .collect();
            times.sort_unstable();
            times
        };
        let bad_before = times(&trackers);
        let honest = |t: &[RoomTracker<WireMessage>]| (kept(t, ana), kept(t, bea));
        let honest_before = honest(&trackers);
        let budget = honest_before.0.1 + honest_before.1.1 + 8 * 1024 * 1024;
        assert!(before > 4 * budget, "{before} of {budget}");
        let dropped = fit_chat(trackers.iter_mut(), budget);
        assert!(dropped > 0);
        assert!(total(&trackers) <= budget);
        assert_eq!(honest(&trackers), honest_before);
        // What the two floods keep is about even, each its newest lines.
        let (bad_kept, worse_kept) = (kept(&trackers, bad).1, kept(&trackers, worse).1);
        assert!(
            bad_kept.abs_diff(worse_kept) < 16 * 1024,
            "{bad_kept} {worse_kept}"
        );
        let bad_after = times(&trackers);
        assert!(!bad_after.is_empty() && bad_after.len() < bad_before.len());
        assert_eq!(
            bad_after,
            bad_before[bad_before.len() - bad_after.len()..],
            "the flood's oldest lines go first"
        );
        // Within the bound already: nothing more goes.
        assert_eq!(fit_chat(trackers.iter_mut(), budget), 0);
    }

    /// Two members of a group whose devices keep different amounts of chat (one in many
    /// groups, past its own bound: [`fit_chat`]) still answer each other's room questions
    /// alike: who is in the room and which files it lists are the same on both, and the same
    /// for a member that asks either of them; what one lets go of the chat it does not take
    /// back from the other's answer, and the other loses nothing for asking it. Only how far
    /// back the chat goes differs, which is this device's own.
    #[test]
    fn devices_keeping_less_chat_still_agree_on_the_room() {
        use fogata_state::{EVENT_SLACK_BYTES, MAX_CHAT_CHARS};
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let people = [key(), key(), key()];
        let tracker = || RoomTracker::new(key()).weighing(crate::group::room_record_weight);
        let mut events: HashMap<Vec<u8>, RoomEvent> = HashMap::new();
        let mut records = Vec::new();
        let mut n = 0u32;
        let mut send = |sender, input, weight: usize| {
            n += 1;
            let mut message_id = [0u8; 16];
            message_id[..4].copy_from_slice(&n.to_be_bytes());
            let mut wire = record(0, 0);
            wire.message_id = message_id.to_vec();
            wire.ciphertext = vec![5; weight - crate::group::room_record_weight(&wire)];
            events.insert(
                wire.message_id.clone(),
                RoomEvent {
                    room: [1; 16],
                    sender,
                    timestamp_ms: 1_000 + u64::from(n),
                    message_id,
                    input,
                },
            );
            records.push(wire);
        };
        for who in people {
            let join = RoomInput::Join {
                started_ms: 1_000,
                nick: "m".into(),
            };
            send(who, join, 300);
        }
        let heavy = "\u{1}".repeat(MAX_CHAT_CHARS);
        for i in 0..120u8 {
            let who = people[usize::from(i) % people.len()];
            if i % 10 == 0 {
                let file = FileInfo {
                    id: [i; 32],
                    name: format!("clip{i}.mp4"),
                    size: 1000,
                    video: Some(VideoPreview {
                        thumb: vec![9; 1024],
                        width: 2,
                        height: 2,
                        duration_ms: 1,
                    }),
                };
                send(who, RoomInput::Share(file), 2000);
            }
            let chat = RoomInput::Chat {
                nick: "m".into(),
                text: heavy.clone(),
            };
            send(who, chat, 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES);
        }
        let apply_all = |t: &mut RoomTracker<WireMessage>, records: &[WireMessage]| {
            for wire in records {
                t.apply(events[&wire.message_id].clone(), wire.clone(), 1_000);
            }
        };
        let (mut small, mut large) = (tracker(), tracker());
        apply_all(&mut small, &records);
        apply_all(&mut large, &records);
        let all_chat = large.chat_weights();
        let budget = all_chat.iter().map(|l| l.3).sum::<usize>() / 3;
        assert!(fit_chat(std::iter::once(&mut small), budget) > 0);
        let kept = |t: &RoomTracker<WireMessage>| -> Vec<[u8; 16]> {
            t.chat_weights().iter().map(|l| l.2).collect()
        };
        let (small_chat, large_chat) = (kept(&small), kept(&large));
        assert!(small_chat.len() < large_chat.len());
        // The room as each says it: who is in it and its files.
        let room = |t: &RoomTracker<WireMessage>| {
            let room = t.room().unwrap();
            let who: Vec<(DevicePublicKey, String)> = room
                .participants
                .iter()
                .map(|(d, p)| (*d, p.nick.clone()))
                .collect();
            (who, room.files.clone())
        };
        assert_eq!(room(&small), room(&large));
        assert_eq!(room(&small).1.len(), 12);
        // A member that comes online asks either: the same room.
        let (mut from_small, mut from_large) = (tracker(), tracker());
        apply_all(&mut from_small, &small.snapshot());
        apply_all(&mut from_large, &large.snapshot());
        assert_eq!(room(&from_small), room(&large));
        assert_eq!(room(&from_large), room(&large));
        assert_eq!(kept(&from_small), small_chat);
        assert_eq!(kept(&from_large), large_chat);
        // Each asks the other: nothing changes on either.
        apply_all(&mut large, &small.snapshot());
        apply_all(&mut small, &large.snapshot());
        assert_eq!(kept(&large), large_chat);
        assert_eq!(kept(&small), small_chat, "what was let go came back");
        assert_eq!(room(&small), room(&large));
    }

    /// Two members of a group, one of which holds its shares' messages on disk past its own
    /// bound (in many groups, say: [`fit_shares`]), still show the same room and answer a
    /// member that asks with the same messages, byte for byte, in the same parts; a member
    /// asking either gets the same room, and asking each other changes nothing. A message
    /// still among the room's latest events stays in memory until it leaves them.
    #[test]
    fn devices_keeping_shares_on_disk_still_agree_on_the_room() {
        use fogata_state::{EVENT_SLACK_BYTES, MAX_CHAT_CHARS};
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let people = [key(), key(), key()];
        let tracker = || RoomTracker::new(key()).weighing(Kept::weight);
        let mut events: HashMap<Vec<u8>, RoomEvent> = HashMap::new();
        let mut records = Vec::new();
        let mut n = 0u32;
        let mut send = |sender, input, weight: usize| {
            n += 1;
            let mut message_id = [0u8; 16];
            message_id[..4].copy_from_slice(&n.to_be_bytes());
            let mut wire = record(0, 0);
            wire.message_id = message_id.to_vec();
            wire.ciphertext = vec![n as u8; weight - crate::group::room_record_weight(&wire)];
            events.insert(
                wire.message_id.clone(),
                RoomEvent {
                    room: [1; 16],
                    sender,
                    timestamp_ms: 1_000 + u64::from(n),
                    message_id,
                    input,
                },
            );
            records.push(wire);
        };
        for who in people {
            let join = RoomInput::Join {
                started_ms: 1_000,
                nick: "m".into(),
            };
            send(who, join, 300);
        }
        let heavy = "\u{1}".repeat(MAX_CHAT_CHARS);
        for i in 0..120u8 {
            let who = people[usize::from(i) % people.len()];
            if i % 3 == 0 {
                let file = FileInfo {
                    id: [i; 32],
                    name: format!("clip{i}.mp4"),
                    size: 1000,
                    video: Some(VideoPreview {
                        thumb: vec![9; 12_000],
                        width: 2,
                        height: 2,
                        duration_ms: 1,
                    }),
                };
                send(who, RoomInput::Share(file), 16_000 + usize::from(i));
            }
            let chat = RoomInput::Chat {
                nick: "m".into(),
                text: heavy.clone(),
            };
            send(who, chat, 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES);
        }
        let apply_all = |t: &mut RoomTracker<Kept>, records: &[WireMessage]| {
            for wire in records {
                let kept = Kept::Inline(Arc::new(wire.clone()));
                t.apply(events[&wire.message_id].clone(), kept, 1_000);
            }
        };
        let (mut small, mut large) = (tracker(), tracker());
        apply_all(&mut small, &records);
        apply_all(&mut large, &records);
        // The latest share, also among the room's latest events: the same message.
        let latest = small
            .share_records()
            .max_by_key(|(_, _, at, _)| *at)
            .map(|(id, holder, ..)| (id, holder))
            .unwrap();
        let recent = match small.share_records().find(|s| (s.0, s.1) == latest) {
            Some((.., Kept::Inline(wire))) => wire.clone(),
            _ => unreachable!(),
        };
        let held = |t: &RoomTracker<Kept>| -> usize {
            t.share_records()
                .filter(|(.., k)| matches!(k, Kept::Inline(_)))
                .map(|(.., k)| k.weight())
                .sum()
        };
        let all = held(&large);
        let budget = all / 3;
        let dir = spill_dir("agree");
        let mut spill = Spill::open(dir.clone()).unwrap();
        assert!(fit_shares(std::iter::once(&mut small), &mut spill, budget) > 0);
        assert!(held(&small) <= budget);
        let on_disk = |t: &RoomTracker<Kept>| -> Vec<([u8; 32], DevicePublicKey)> {
            t.share_records()
                .filter(|(.., k)| matches!(k, Kept::Spilled { .. }))
                .map(|(id, holder, ..)| (id, holder))
                .collect()
        };
        assert!(
            !on_disk(&small).contains(&latest),
            "moved while still latest"
        );
        // The oldest of whoever held the most went first: the latest held are all newer.
        let times = |t: &RoomTracker<Kept>, spilled: bool| -> Vec<u64> {
            t.share_records()
                .filter(|(.., k)| matches!(k, Kept::Spilled { .. }) == spilled)
                .map(|(_, _, at, _)| at)
                .collect()
        };
        assert!(times(&small, true).iter().max() < times(&small, false).iter().max());
        // The room as each shows it: who is in it and its files.
        let room = |t: &RoomTracker<Kept>| {
            let room = t.room().unwrap();
            let who: Vec<(DevicePublicKey, String)> = room
                .participants
                .iter()
                .map(|(d, p)| (*d, p.nick.clone()))
                .collect();
            (who, room.files.clone())
        };
        assert_eq!(room(&small), room(&large));
        assert_eq!(room(&small).1.len(), 40);
        // What each answers a member that asks: the same, byte for byte, in the same parts.
        let small_parts = all_parts(&small, Some(&spill));
        let large_parts = all_parts(&large, None);
        assert!(small_parts.len() > 1);
        assert!(same_parts(&small_parts, &large_parts));
        // A member that comes online asks either: the same room.
        let answer = |parts: &[(Vec<WireMessage>, Option<u32>)]| -> Vec<WireMessage> {
            parts.iter().flat_map(|(p, _)| p.iter().cloned()).collect()
        };
        let (mut from_small, mut from_large) = (tracker(), tracker());
        apply_all(&mut from_small, &answer(&small_parts));
        apply_all(&mut from_large, &answer(&large_parts));
        assert_eq!(room(&from_small), room(&large));
        assert_eq!(room(&from_large), room(&large));
        // Each asks the other: the same room on both; what came again from the other is
        // held in memory until the next slot moves it again.
        apply_all(&mut large, &answer(&small_parts));
        apply_all(&mut small, &answer(&large_parts));
        assert_eq!(room(&small), room(&large));
        drop(recent);
        fit_shares(std::iter::once(&mut small), &mut spill, budget);
        assert!(held(&small) <= budget);
        assert!(same_parts(
            &all_parts(&small, Some(&spill)),
            &all_parts(&large, None)
        ));
        // What went out of the room no longer takes room on disk: its pages hold the next.
        let (file, _) = spill.bytes();
        let on_disk_now = on_disk(&small).len();
        assert!(
            file as usize <= (on_disk_now + 1) * 20 * 1024,
            "{file} bytes"
        );
        drop(spill);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A room as a device keeps it, with its latest events.
    type FloodedRoom = (RoomTracker<Arc<WireMessage>>, Vec<Arc<WireMessage>>);

    /// The ten rooms of a group whose every room has eight devices of a member that floods
    /// it: each device shares as many videos with the biggest picture as the room takes from
    /// it, and one says 500 of the heaviest lines; numbered on from `n`. With `lean`, kept as
    /// the node keeps them now: each record once, shared with the room's latest events, and
    /// a share's picture in its record only; else as before, the latest events copied and
    /// each picture kept read as well. Each room's latest events go with it.
    fn flooded_group(n: &mut u32, lean: bool) -> Vec<FloodedRoom> {
        use fogata_state::{
            EVENT_SLACK_BYTES, MAX_CHAT_CHARS, MAX_IN_ROOM, MAX_SHARES_EACH, MAX_THUMB_BYTES,
        };
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let devices: Vec<DevicePublicKey> = (0..MAX_IN_ROOM).map(|_| key()).collect();
        let me = key();
        let heavy = "\u{1}".repeat(MAX_CHAT_CHARS);
        let thumb = vec![9; MAX_THUMB_BYTES];
        (0..10u8)
            .map(|v| {
                let mut t = RoomTracker::new(me)
                    .weighing(|record: &Arc<WireMessage>| crate::group::room_record_weight(record))
                    .keeping_pictures(!lean);
                t.set_room_for(MAX_IN_ROOM, 10);
                let mut recent: VecDeque<RecentEvent> = VecDeque::new();
                let mut send = |t: &mut RoomTracker<Arc<WireMessage>>, sender, input, weight| {
                    *n += 1;
                    let mut message_id = [0u8; 16];
                    message_id[..4].copy_from_slice(&n.to_be_bytes());
                    let mut wire = record(0, 0);
                    wire.message_id = message_id.to_vec();
                    wire.ciphertext = vec![5; weight - crate::group::room_record_weight(&wire)];
                    let wire = Arc::new(wire);
                    // As apply_room keeps it among the latest: the same message, or before a
                    // copy of it.
                    let kept = if lean {
                        wire.clone()
                    } else {
                        Arc::new((*wire).clone())
                    };
                    keep_recent(&mut recent, [v; 16], message_id, &kept);
                    let event = RoomEvent {
                        room: [v; 16],
                        sender,
                        timestamp_ms: 1_000 + u64::from(*n),
                        message_id,
                        input,
                    };
                    t.apply(event, wire, 1_000);
                };
                for (d, device) in devices.iter().enumerate() {
                    let join = RoomInput::Join {
                        started_ms: 1_000,
                        nick: "m".into(),
                    };
                    send(&mut t, *device, join, 300);
                    for i in 0..MAX_SHARES_EACH as u32 {
                        let mut id = [0u8; 32];
                        id[..4].copy_from_slice(&(d as u32 * 1_000 + i).to_be_bytes());
                        let file = FileInfo {
                            id,
                            name: format!("f{i:03}"),
                            size: 1,
                            video: Some(VideoPreview {
                                thumb: thumb.clone(),
                                width: 2,
                                height: 2,
                                duration_ms: 1,
                            }),
                        };
                        let weight = MAX_THUMB_BYTES.div_ceil(3) * 4 + 4 + EVENT_SLACK_BYTES;
                        send(&mut t, *device, RoomInput::Share(file), weight);
                    }
                }
                for _ in 0..500 {
                    let chat = RoomInput::Chat {
                        nick: "m".into(),
                        text: heavy.clone(),
                    };
                    let weight = 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES;
                    send(&mut t, devices[0], chat, weight);
                }
                let recent = recent.into_iter().filter_map(|e| e.wire).collect();
                (t, recent)
            })
            .collect()
    }

    /// This process's resident memory, in bytes.
    fn resident() -> u64 {
        #[cfg(target_os = "linux")]
        {
            let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
            let pages: u64 = statm.split_whitespace().nth(1).unwrap().parse().unwrap();
            pages * 4096
        }
        #[cfg(not(target_os = "linux"))]
        {
            let out = std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
                .unwrap();
            let kib: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
            kib * 1024
        }
    }

    /// What the rooms of ten groups, each flooded as [`flooded_group`] does, make a device
    /// keep: with every group's own bounds only (`bounded` false), or also within the bound
    /// on the chat of all its rooms ([`fit_chat`], every slot of the node, here after each
    /// group came); kept `lean` as the node does now, or as before. Linux and macOS, one at
    /// a time, in a release build:
    /// `cargo test --release -p fogata-net --lib device_room_memory -- --ignored --nocapture`.
    fn measure_device_room_memory(bounded: bool, lean: bool) {
        let before = resident();
        let mut n = 0;
        let mut rooms = Vec::new();
        for _ in 0..10 {
            rooms.extend(flooded_group(&mut n, lean));
            if bounded {
                fit_chat(rooms.iter_mut().map(|(t, _)| t), ROOM_CHAT_BYTES);
            }
        }
        let after = resident();
        let chat: usize = rooms
            .iter()
            .flat_map(|(t, _)| t.chat_weights())
            .map(|l| l.3)
            .sum();
        let shares: usize = rooms.iter().map(|(t, _)| t.share_bytes()).sum();
        let records: usize = rooms.iter().map(|(t, _)| t.snapshot().len()).sum();
        let pictures: usize = rooms
            .iter()
            .filter_map(|(t, _)| t.room())
            .flat_map(|r| r.files.values())
            .filter_map(|f| f.info.video.as_ref())
            .map(|v| v.thumb.len())
            .sum();
        println!(
            "10 groups{}{}: {records} records kept; chat {} MiB, shares {} MiB as counted, \
             pictures read {} MiB; {} MiB resident",
            if bounded { ", chat bounded" } else { "" },
            if lean { ", kept once" } else { "" },
            chat >> 20,
            shares >> 20,
            pictures >> 20,
            after.saturating_sub(before) >> 20
        );
    }

    #[test]
    #[ignore]
    fn measure_device_room_memory_lean() {
        measure_device_room_memory(true, true);
    }

    #[test]
    #[ignore]
    fn measure_device_room_memory_bounded() {
        measure_device_room_memory(true, false);
    }

    #[test]
    #[ignore]
    fn measure_device_room_memory_per_group_only() {
        measure_device_room_memory(false, false);
    }

    /// What ten groups of one room each make a device hold of their shares, in each of which
    /// a member that floods it has eight devices sharing as many videos with the biggest
    /// picture as the room takes from each (47): every share in memory (`on_disk` false),
    /// or past [`ROOM_SHARE_BYTES`] on disk ([`fit_shares`], every slot of the node, here
    /// after each group came). Kept as the node keeps them, with each room's latest events.
    /// Also how long the slowest part of a room's answer takes to read back. Linux and
    /// macOS, one at a time, in a release build:
    /// `cargo test --release -p fogata-net --lib device_share_memory -- --ignored --nocapture`.
    fn measure_device_share_memory(on_disk: bool) {
        use fogata_state::{EVENT_SLACK_BYTES, MAX_IN_ROOM, MAX_SHARES_EACH, MAX_THUMB_BYTES};
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let dir = spill_dir("measure");
        let mut spill = on_disk.then(|| Spill::open(dir.clone()).unwrap());
        let me = key();
        let thumb = vec![9; MAX_THUMB_BYTES];
        let mut n = 0u32;
        let before = resident();
        let mut rooms: Vec<(RoomTracker<Kept>, Vec<Arc<WireMessage>>)> = Vec::new();
        for g in 0..10u8 {
            let mut t = RoomTracker::new(me)
                .weighing(Kept::weight)
                .keeping_pictures(false);
            t.set_room_for(MAX_IN_ROOM, 1);
            let mut recent: VecDeque<RecentEvent> = VecDeque::new();
            let mut send = |t: &mut RoomTracker<Kept>, sender, input, weight| {
                n += 1;
                let mut message_id = [0u8; 16];
                message_id[..4].copy_from_slice(&n.to_be_bytes());
                let mut wire = record(0, 0);
                wire.message_id = message_id.to_vec();
                wire.ciphertext = vec![5; weight - crate::group::room_record_weight(&wire)];
                let wire = Arc::new(wire);
                keep_recent(&mut recent, [g; 16], message_id, &wire);
                let event = RoomEvent {
                    room: [g; 16],
                    sender,
                    timestamp_ms: 1_000 + u64::from(n),
                    message_id,
                    input,
                };
                t.apply(event, Kept::Inline(wire), 1_000);
            };
            for d in 0..u32::from(MAX_IN_ROOM) {
                let device = key();
                let join = RoomInput::Join {
                    started_ms: 1_000,
                    nick: "m".into(),
                };
                send(&mut t, device, join, 300);
                for i in 0..MAX_SHARES_EACH as u32 {
                    let mut id = [0u8; 32];
                    id[..4].copy_from_slice(&(d * 1_000 + i).to_be_bytes());
                    let file = FileInfo {
                        id,
                        name: format!("f{i:03}"),
                        size: 1,
                        video: Some(VideoPreview {
                            thumb: thumb.clone(),
                            width: 2,
                            height: 2,
                            duration_ms: 1,
                        }),
                    };
                    let weight = MAX_THUMB_BYTES.div_ceil(3) * 4 + 4 + EVENT_SLACK_BYTES;
                    send(&mut t, device, RoomInput::Share(file), weight);
                }
            }
            let recent = recent.into_iter().filter_map(|e| e.wire).collect();
            rooms.push((t, recent));
            if let Some(spill) = spill.as_mut() {
                fit_shares(rooms.iter_mut().map(|(t, _)| t), spill, ROOM_SHARE_BYTES);
            }
        }
        let after = resident();
        let listed: usize = rooms.iter().map(|(t, _)| t.share_records().count()).sum();
        let counted: usize = rooms.iter().map(|(t, _)| t.share_bytes()).sum();
        let (mut held, mut moved) = (0, 0);
        for (kept, weight) in rooms
            .iter()
            .flat_map(|(t, _)| t.share_records())
            .map(|(.., k)| (k, k.weight()))
        {
            match kept {
                Kept::Inline(_) => held += weight,
                Kept::Spilled { .. } => moved += weight,
            }
        }
        let file = spill.as_ref().map_or(0, |s| s.bytes().0);
        // Each part of the first room's answer, read back, the slowest.
        let mut slowest = (Duration::ZERO, 0, 0);
        let mut from = Some(0);
        while let Some(at) = from {
            let started = Instant::now();
            let (part, more) =
                rooms_part(std::iter::once(&rooms[0].0), spill.as_ref(), at).unwrap();
            let took = started.elapsed();
            if took > slowest.0 {
                slowest = (took, part.len(), part.iter().map(wire_size).sum::<usize>());
            }
            from = more;
        }
        println!(
            "10 one-room groups{}: {listed} shares, {} MiB as counted; messages {} MiB in \
             memory, {} MiB on disk (file {} MiB); {} MiB resident; slowest part of an \
             answer ({} shares, {} KiB) read in {:?}",
            if on_disk {
                ", on disk past the bound"
            } else {
                ""
            },
            counted >> 20,
            held >> 20,
            moved >> 20,
            file >> 20,
            after.saturating_sub(before) >> 20,
            slowest.1,
            slowest.2 >> 10,
            slowest.0,
        );
        drop(spill);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn measure_device_share_memory_in_memory() {
        measure_device_share_memory(false);
    }

    #[test]
    #[ignore]
    fn measure_device_share_memory_on_disk() {
        measure_device_share_memory(true);
    }

    /// A busy device in many groups, every room of each full of honest chat (500 lines of
    /// 300 characters), keeps all of it within the bound.
    #[test]
    fn honest_chat_in_many_busy_groups_is_all_kept() {
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let people: Vec<DevicePublicKey> = (0..5).map(|_| key()).collect();
        // 100 rooms (ten groups of ten), each with its 500 lines.
        let mut trackers = chat_rooms(100, &people, 100, 300, key(), 0);
        let lines: usize = trackers.iter().map(|t| t.chat_weights().len()).sum();
        assert_eq!(lines, 100 * 500);
        assert_eq!(fit_chat(trackers.iter_mut(), ROOM_CHAT_BYTES), 0);
    }
}
