//! This device's own lines said in a room that not every member in the room has yet, kept on
//! disk so that a line said while a member was out of reach still gets to it: when that
//! member next proves membership, in this session or a later one. Nothing else of a room is
//! kept, and nothing of it is on the wire: a member takes a line sent again as any copy of a
//! message, by its id.
//!
//! A line is kept for the other members that were in its room when it was said: those listed
//! there, and those the room let go without a leave (cut off, or not heard from for the
//! grace) since it opened on this device, remembered in memory while the room lasts. A
//! member that never entered the room, or left it saying so, is owed nothing, and one that
//! leaves it saying so later is let go of the lines of that voice room. With nobody owed, a
//! line is not kept.
//!
//! A member answers [`fogata_proto::MemberResponse::Ack`] to a line whatever it did with it,
//! also one sealed with a key it does not have yet, which it drops. So a line goes to a
//! member, and its `Ack` lets the member go of it, only once the member is known to read the
//! line's epoch, by a key the log gave it: the epoch it proved itself with and those it was
//! given before, or, once it names the same log heads in a roster, every epoch it was given
//! (a member that missed a new key gets it by syncing, and the next roster says so; one that
//! was not given it yet is not counted until it is). What is known of a member goes when it
//! is no longer connected, and is learnt again at its next proof. A refusal (a member far
//! behind, or still checking this device) keeps the line for that member, and it goes again
//! at the next room heartbeat; so does one whose request failed or was dropped. Otherwise a
//! line goes to a member at most once each [`AGAIN`], at its proof or at a heartbeat, and
//! sooner only after that member refused it, a send to it did not get there, or it came to
//! be known to read the line's key: at most [`MOST`] kept lines at once, each under 16 KiB
//! as a room takes them, well within what a member takes of one author before it drops the
//! rest unseen (256 messages and 8 MiB at once, 32 and 512 KiB more each second, see
//! limits.rs). A line sent again names every connected member as reached, so that a member
//! that takes `Spread` passes it on to none of them; an older one gets `Deliver` and passes
//! it on as it does any. A member whose room is not the line's any more (the room closed and
//! another opened since) takes it all the same and is let go: its room ignores a line of a
//! room it does not have, so the line never shows in a later room.
//!
//! A line goes once every member owed it took it, [`KEPT_FOR_MS`] after it was said (by
//! this computer's clock, either way: a clock set a day back or forward lets go of what is
//! kept; a line that expires while the program is not running goes at its next start), or
//! when this device is known no longer to be a member of its group; a member that is no
//! longer one is let go. At most [`MOST_PER_GROUP`] lines of a group and [`MOST`] in all are
//! kept, the oldest going first. Leaves and shares are not kept: a member that missed a
//! leave lets the participant go after a while, and a share is listed while its holder is
//! there.
//!
//! Only with a file to keep them in, given by the application with the data folder's key
//! (see `NodeHandle::keep_own_lines`): one box sealed with that key, holding each line as
//! it was sent (sealed already with the group's key) and the members that lack it, padded to
//! a size of a few fixed ones so that the file does not tell how many lines it holds nor how
//! long. The file follows the lines a moment late: [`SETTLE`] after a change, at the next
//! check, so that a line every member takes at once never reaches the disk; and when the
//! node stops. A line said just after a key changed is the exception: a member's `Ack`
//! counts once it is known to read the new key, at its next roster with the same log, up to
//! a heartbeat later, so the line is written meanwhile, and sent to it once more. It is
//! written whole through a temporary file, and removed when nothing is kept. After a write
//! that failed, none is tried for [`WRITE_AGAIN`] but when the node stops; the file holds
//! what it held until then. A file that does not open (another key, damaged) is removed,
//! its lines lost as they were before lines were kept. Saying a line never waits for any of
//! it, and the log names no line.

use std::{
    collections::{BTreeSet, HashMap},
    io::{self, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

use fogata_crypto::{DevicePublicKey, GroupId, SealedMessage, at_rest::DataKey};
use fogata_proto::{GroupPayload, MemberRequest, WireMessage};
use fogata_state::{RoomChange, VoiceRoomId};
use libp2p::PeerId;

use super::{Node, Pending, links::LINK_CHECK, members::group_of, now_ms, spill};

/// How long after it was said a line is kept at most.
pub(super) const KEPT_FOR_MS: u64 = 24 * 60 * 60 * 1000;
/// Lines of one group kept at most.
pub(super) const MOST_PER_GROUP: usize = 16;
/// Lines kept at most, of every group.
pub(super) const MOST: usize = 64;
/// How long after a line went to a member that lacks it it goes to it again, at its proof or
/// at a room heartbeat; a refusal has it go at the next heartbeat.
const AGAIN: Duration = Duration::from_secs(60);
/// How long after a change the file follows it, at the next check of the links.
const SETTLE: Duration = LINK_CHECK;
/// After a write that failed (a full or read-only disk), how long before writing again.
const WRITE_AGAIN: Duration = Duration::from_secs(60);
/// What the file is sealed for.
const CONTEXT: &str = "room lines";
/// The first byte of what the file holds: how it is laid out.
const LAYOUT: u8 = 1;
/// The smallest size of what the file holds: a larger one is the next power of two.
const LEAST_BODY: usize = 4096;

/// A line kept: the message as sent, the members that lack it, and when it last went to
/// each of them in this session.
struct Line {
    group: GroupId,
    id: [u8; 16],
    wire: WireMessage,
    lacking: BTreeSet<DevicePublicKey>,
    sent: HashMap<DevicePublicKey, Instant>,
}

/// This device's lines kept, the oldest first, and the file they are kept in.
#[derive(Default)]
pub(super) struct KeptLines {
    file: Option<(PathBuf, DataKey)>,
    lines: Vec<Line>,
    /// The members each voice room let go without a leave since it opened here, by group
    /// and voice room: still owed what is said there.
    dropped: HashMap<(GroupId, VoiceRoomId), BTreeSet<DevicePublicKey>>,
    /// The epochs of each group each member is known to read.
    reads: HashMap<(GroupId, DevicePublicKey), BTreeSet<u32>>,
    /// Whether every group of this device's was read at start: a group the node does not
    /// run is then one it left, and its lines go.
    all_groups: bool,
    /// Since when the file lags behind the lines.
    changed: Option<Instant>,
    /// Whether the file is there, as far as this device knows.
    on_disk: bool,
    /// When a write last failed, until one is made.
    failed_at: Option<Instant>,
}

impl KeptLines {
    /// The lines kept at `path`, sealed with `key`, which keeps them from now on, but those
    /// said more than [`KEPT_FOR_MS`] from `now_ms`; with `all_groups`, every group of this
    /// device's was read. A file that does not open is removed. Blocking.
    pub(super) fn open(path: PathBuf, key: DataKey, all_groups: bool, now_ms: u64) -> Self {
        let (lines, on_disk) = match std::fs::read(&path) {
            Ok(bytes) => {
                let lines = key
                    .open_whole(CONTEXT, &bytes)
                    .ok()
                    .and_then(|p| decode(&p));
                if lines.is_none() {
                    tracing::warn!("this device's room lines kept on disk do not open; removed");
                    if let Err(e) = std::fs::remove_file(&path) {
                        tracing::warn!("cannot remove this device's room lines kept on disk: {e}");
                    }
                }
                (lines.unwrap_or_default(), true)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => (Vec::new(), false),
            Err(e) => {
                tracing::warn!("cannot read this device's room lines kept on disk: {e}");
                (Vec::new(), true)
            }
        };
        let mut kept = Self {
            file: Some((path, key)),
            lines,
            all_groups,
            on_disk,
            ..Self::default()
        };
        kept.tidy(now_ms, |_| None);
        kept
    }

    /// The file lags behind the lines from now on.
    fn changed(&mut self) {
        self.changed.get_or_insert_with(Instant::now);
    }

    /// Keeps line `wire`, message `id` of `group` sent just now to `sent_to`, until each of
    /// `lacking` took it. Nothing is kept without a file.
    fn keep(
        &mut self,
        group: GroupId,
        id: [u8; 16],
        wire: &WireMessage,
        lacking: BTreeSet<DevicePublicKey>,
        sent_to: &BTreeSet<DevicePublicKey>,
    ) {
        if self.file.is_none() || lacking.is_empty() {
            return;
        }
        let now = Instant::now();
        let sent = (lacking.intersection(sent_to)).map(|d| (*d, now)).collect();
        self.lines.push(Line {
            group,
            id,
            wire: wire.clone(),
            lacking,
            sent,
        });
        let mut of_group = self.lines.iter().filter(|l| l.group == group).count();
        while of_group > MOST_PER_GROUP {
            if let Some(oldest) = self.lines.iter().position(|l| l.group == group) {
                self.lines.remove(oldest);
            }
            of_group -= 1;
        }
        if self.lines.len() > MOST {
            self.lines.drain(..self.lines.len() - MOST);
        }
        self.changed();
    }

    /// Whether `device` is known to read `line`.
    fn reads(&self, line: &Line, device: &DevicePublicKey) -> bool {
        (self.reads.get(&(line.group, *device))).is_some_and(|e| e.contains(&line.wire.epoch))
    }

    /// Member `device` answered line `id`: taken, it is let go of it once known to read it;
    /// refused, the line goes to it again at the next heartbeat.
    fn answered(&mut self, id: &[u8; 16], device: &DevicePublicKey, taken: bool) {
        let Some(at) = self.lines.iter().position(|l| l.id == *id) else {
            return;
        };
        if !taken {
            self.lines[at].sent.remove(device);
            return;
        }
        if !self.reads(&self.lines[at], device) {
            return;
        }
        let line = &mut self.lines[at];
        line.lacking.remove(device);
        if line.lacking.is_empty() {
            self.lines.remove(at);
            self.changed();
        }
    }

    /// Whether member `device` is known to read `group`'s `epoch`.
    fn knows(&self, group: GroupId, device: &DevicePublicKey, epoch: u32) -> bool {
        (self.reads.get(&(group, *device))).is_some_and(|e| e.contains(&epoch))
    }

    /// Member `device` reads `group`'s `epochs`: a line it could not read before is due to
    /// it now. Nothing is noted without a file.
    fn read(&mut self, group: GroupId, device: DevicePublicKey, epochs: Vec<u32>) {
        if self.file.is_none() {
            return;
        }
        let known = self.reads.entry((group, device)).or_default();
        let new: Vec<u32> = epochs.into_iter().filter(|e| known.insert(*e)).collect();
        for line in self.lines.iter_mut().filter(|l| l.group == group) {
            if new.contains(&line.wire.epoch) {
                line.sent.remove(&device);
            }
        }
    }

    /// Member `device` is no longer connected in `group`: what it reads is noted again when
    /// it proves itself, and what went to it unanswered goes again then.
    fn member_gone(&mut self, group: GroupId, device: &DevicePublicKey) {
        self.reads.remove(&(group, *device));
        for line in self.lines.iter_mut().filter(|l| l.group == group) {
            line.sent.remove(device);
        }
    }

    /// The members of `group` that are not `connected` are no longer connected in it, their
    /// devices not known here any more: as for [`Self::member_gone`], for each of them.
    fn members_gone_but(&mut self, group: GroupId, connected: &BTreeSet<DevicePublicKey>) {
        (self.reads).retain(|(g, device), _| *g != group || connected.contains(device));
        for line in self.lines.iter_mut().filter(|l| l.group == group) {
            line.sent.retain(|device, _| connected.contains(device));
        }
    }

    /// Line `id` went to `device` at `now`.
    fn sent(&mut self, id: &[u8; 16], device: DevicePublicKey, now: Instant) {
        if let Some(line) = self.lines.iter_mut().find(|l| l.id == *id) {
            line.sent.insert(device, now);
        }
    }

    /// Line `id` sent to `device` did not get there (failed, or dropped unsent): it goes to
    /// it again at its next proof or heartbeat.
    fn unsent(&mut self, id: &[u8; 16], device: &DevicePublicKey) {
        if let Some(line) = self.lines.iter_mut().find(|l| l.id == *id) {
            line.sent.remove(device);
        }
    }

    /// Lets `device` go of the lines `which` names.
    fn let_go(&mut self, device: &DevicePublicKey, which: impl Fn(&Line) -> bool) {
        let before = self.lines.len();
        for line in self.lines.iter_mut().filter(|l| which(l)) {
            line.lacking.remove(device);
        }
        self.lines.retain(|l| !l.lacking.is_empty());
        if self.lines.len() != before {
            self.changed();
        }
    }

    /// Lets go of the lines said more than [`KEPT_FOR_MS`] from `now_ms`, and of what
    /// `members` says no longer counts: a group it says this device left (`Some(None)`), a
    /// member it does not list. A group it knows nothing of (`None`) keeps its lines.
    fn tidy(
        &mut self,
        now_ms: u64,
        members: impl Fn(&GroupId) -> Option<Option<BTreeSet<DevicePublicKey>>>,
    ) {
        let before = self.lines.len();
        self.lines.retain_mut(|line| {
            if now_ms.abs_diff(line.wire.timestamp_ms) > KEPT_FOR_MS {
                return false;
            }
            match members(&line.group) {
                None => true,
                Some(None) => false,
                Some(Some(members)) => {
                    line.lacking.retain(|d| members.contains(d));
                    !line.lacking.is_empty()
                }
            }
        });
        if self.lines.len() != before {
            self.changed();
        }
    }

    /// The lines of `group` to send `device` as of `now`: it lacks them and is known to read
    /// them, and none went to it in the last [`AGAIN`].
    fn due_to(
        &self,
        group: GroupId,
        device: &DevicePublicKey,
        now: Instant,
    ) -> Vec<([u8; 16], WireMessage)> {
        (self.lines.iter())
            .filter(|l| l.group == group && l.lacking.contains(device))
            .filter(|l| self.knows(group, device, l.wire.epoch))
            .filter(|l| (l.sent.get(device)).is_none_or(|at| now.duration_since(*at) >= AGAIN))
            .map(|l| (l.id, l.wire.clone()))
            .collect()
    }

    /// Brings the file up to the lines: once they changed [`SETTLE`] ago, and not for
    /// [`WRITE_AGAIN`] after a write failed; with `now`, at once whatever came before (the
    /// node stops). Blocking.
    pub(super) fn follow(&mut self, now: bool) {
        let Some(changed) = self.changed else {
            return;
        };
        let Some((path, key)) = &self.file else {
            return;
        };
        if !now
            && (changed.elapsed() < SETTLE
                || self.failed_at.is_some_and(|at| at.elapsed() < WRITE_AGAIN))
        {
            return;
        }
        let written = if self.lines.is_empty() {
            match std::fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound && self.on_disk => Err(e),
                _ => Ok(()),
            }
        } else {
            key.seal_whole(CONTEXT, &encode(&self.lines))
                .map_err(io::Error::other)
                .and_then(|sealed| write_whole(path, &sealed))
        };
        match written {
            Ok(()) => {
                self.on_disk = !self.lines.is_empty();
                self.changed = None;
                self.failed_at = None;
            }
            Err(e) => {
                // Said once, not at every line, until a write is made.
                if self.failed_at.is_none() {
                    tracing::warn!("cannot write this device's room lines kept on disk: {e}");
                }
                self.failed_at = Some(Instant::now());
            }
        }
    }
}

/// Writes `bytes` to `path` whole: to a new file beside it first (never a file or a link
/// already there), readable by its owner only (on Unix), then put in its place. Not synced
/// to the disk, which would hold the node up: a file a power cut damages does not open, and
/// its lines are lost as they were before they were kept.
fn write_whole(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    match std::fs::remove_file(&temp) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options
        .open(&temp)
        .and_then(|mut file| file.write_all(bytes))
        .and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// The lines as kept: [`LAYOUT`], the length of what follows that is lines (4 bytes), then
/// for each its message's length (4 bytes), the message as spill.rs writes it, how many
/// members lack it (a byte) and their keys; then zeros, up to [`LEAST_BODY`] or the next
/// power of two.
fn encode(lines: &[Line]) -> Vec<u8> {
    let mut body = Vec::new();
    for line in lines {
        let wire = spill::encode(&line.wire);
        body.extend_from_slice(&(wire.len() as u32).to_le_bytes());
        body.extend_from_slice(&wire);
        // A group has at most 255 members.
        body.push(line.lacking.len().min(255) as u8);
        for device in line.lacking.iter().take(255) {
            body.extend_from_slice(device.as_bytes());
        }
    }
    let mut out = vec![LAYOUT];
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.append(&mut body);
    out.resize(out.len().next_power_of_two().max(LEAST_BODY), 0);
    out
}

fn decode(bytes: &[u8]) -> Option<Vec<Line>> {
    let (&LAYOUT, rest) = bytes.split_first()? else {
        return None;
    };
    let (len, rest) = rest.split_first_chunk::<4>()?;
    let (mut rest, _padding) = rest.split_at_checked(u32::from_le_bytes(*len) as usize)?;
    let mut lines = Vec::new();
    while !rest.is_empty() && lines.len() < MOST {
        let (len, after) = rest.split_first_chunk::<4>()?;
        let len = u32::from_le_bytes(*len) as usize;
        let (wire, after) = after.split_at_checked(len)?;
        let wire = spill::decode(wire)?;
        let (count, mut after) = after.split_first()?;
        let mut lacking = BTreeSet::new();
        for _ in 0..*count {
            let (device, next) = after.split_first_chunk::<32>()?;
            lacking.insert(DevicePublicKey::from_bytes(device).ok()?);
            after = next;
        }
        lines.push(Line {
            group: group_of(&wire.group_id)?,
            id: wire.message_id.as_slice().try_into().ok()?,
            wire,
            lacking,
            sent: HashMap::new(),
        });
        rest = after;
    }
    Some(lines)
}

impl Node {
    /// Keeps this device's lines at `path`, sealed with `key`, from now on; with
    /// `all_groups`, the node runs every group of this device's. Those an earlier session
    /// kept there go to the members that lack them as they prove themselves.
    pub(super) fn keep_own_lines(&mut self, path: PathBuf, key: DataKey, all_groups: bool) {
        self.kept_lines = KeptLines::open(path, key, all_groups, now_ms());
        self.kept_heartbeat();
    }

    /// Keeps line `wire` of this device's, message `id` said just now in `group`'s voice room
    /// `voice`, for the other members in that room: listed there, or let go without a leave
    /// since it opened.
    pub(super) fn keep_line(
        &mut self,
        group: GroupId,
        voice: VoiceRoomId,
        id: [u8; 16],
        wire: &WireMessage,
    ) {
        let me = self.identity.public_key();
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let listed = (slot.rooms.get(&voice).and_then(|t| t.room()))
            .into_iter()
            .flat_map(|room| room.participants.keys());
        let dropped = self
            .kept_lines
            .dropped
            .get(&(group, voice))
            .into_iter()
            .flatten();
        let members = slot.ctx.member_devices();
        let lacking = (listed.chain(dropped))
            .filter(|d| **d != me && members.contains(d))
            .copied()
            .collect();
        // It goes to these now (see `deliver`).
        let connected = (slot.members.iter())
            .filter_map(|p| self.members.get(p))
            .map(|m| m.device)
            .collect();
        self.kept_lines.keep(group, id, wire, lacking, &connected);
    }

    /// Follows `changes` of `group`'s voice room `voice` for the lines kept: who it let go
    /// without a leave is remembered while it lasts; who leaves it saying so is let go of
    /// the lines said in it.
    pub(super) fn kept_room_changes(
        &mut self,
        group: GroupId,
        voice: VoiceRoomId,
        changes: &[RoomChange],
    ) {
        for change in changes {
            match change {
                RoomChange::Opened { .. } | RoomChange::Closed => {
                    self.kept_lines.dropped.remove(&(group, voice));
                }
                RoomChange::Left {
                    device,
                    on_purpose: false,
                } => {
                    let dropped = self.kept_lines.dropped.entry((group, voice)).or_default();
                    dropped.insert(*device);
                }
                RoomChange::Left {
                    device,
                    on_purpose: true,
                } => {
                    if let Some(dropped) = self.kept_lines.dropped.get_mut(&(group, voice)) {
                        dropped.remove(device);
                    }
                    let Some(slot) = self.groups.get(&group) else {
                        continue;
                    };
                    // The voice room a line was said in, read from the line itself.
                    let said_in = |line: &Line| {
                        let sealed = SealedMessage::try_from(&line.wire).ok()?;
                        let keys = slot.ctx.keys(sealed.header.epoch)?;
                        match GroupPayload::decode(&sealed.open(keys).ok()?).ok()? {
                            GroupPayload::RoomChat { voice, .. } => Some(voice),
                            _ => None,
                        }
                    };
                    let of_voice = |line: &Line| {
                        line.group == group
                            && line.lacking.contains(device)
                            && said_in(line) == Some(voice)
                    };
                    self.kept_lines.let_go(device, of_voice);
                }
                _ => {}
            }
        }
    }

    /// Member `peer` answered a request carrying this device's room event `id`: `taken`,
    /// or refused.
    pub(super) fn kept_answered(&mut self, peer: PeerId, id: [u8; 16], taken: bool) {
        if let Some(device) = self.members.get(&peer).map(|m| m.device) {
            self.kept_lines.answered(&id, &device, taken);
        }
    }

    /// Member `peer` of `group` just proved itself with the key of `epoch`: it reads that
    /// epoch and those before it the log gave it, and the lines it lacks of them go to it.
    pub(super) fn kept_proved(&mut self, group: GroupId, peer: PeerId, epoch: u32) {
        self.tidy_kept();
        let device = self.members.get(&peer).map(|m| m.device);
        let epochs = (self.groups.get(&group))
            .zip(device)
            .map(|(s, d)| s.ctx.epochs_given(&d, Some(epoch)));
        if let (Some(epochs), Some(device)) = (epochs, device) {
            self.kept_lines.read(group, device, epochs);
            self.send_kept(group, peer, device);
        }
    }

    /// Member `peer` named the same log heads as this device's for `group`: it reads every
    /// epoch the log gave it, and the lines it lacks of them go to it. Nothing is noted when
    /// it is known to read the current epoch already.
    pub(super) fn kept_caught_up(&mut self, group: GroupId, peer: PeerId) {
        if self.kept_lines.file.is_none() {
            return;
        }
        let Some(device) = self.members.get(&peer).map(|m| m.device) else {
            return;
        };
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let known =
            (slot.ctx.current_epoch()).is_some_and(|e| self.kept_lines.knows(group, &device, e));
        if !known {
            let epochs = slot.ctx.epochs_given(&device, None);
            self.kept_lines.read(group, device, epochs);
        }
        self.send_kept(group, peer, device);
    }

    /// Member `peer` of `group`, of `device`, is no longer connected in it (see
    /// `drop_member`).
    pub(super) fn kept_member_gone(&mut self, group: GroupId, device: &DevicePublicKey) {
        self.kept_lines.member_gone(group, device);
    }

    /// A member of `group` is no longer connected in it, and its device is not known here
    /// any more (see `drop_member`): what is known of those of its devices not connected in
    /// it goes.
    pub(super) fn kept_unknown_member_gone(&mut self, group: GroupId) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let connected = (slot.members.iter())
            .filter_map(|p| self.members.get(p))
            .map(|m| m.device)
            .collect();
        self.kept_lines.members_gone_but(group, &connected);
    }

    /// A request carrying this device's room event `id` to `peer` did not get there.
    pub(super) fn kept_unsent(&mut self, peer: PeerId, id: [u8; 16]) {
        if let Some(device) = self.members.get(&peer).map(|m| m.device) {
            self.kept_lines.unsent(&id, &device);
        }
    }

    /// Sends member `peer`, of `device`, the lines of `group` due to it; each counts as
    /// sent to it once it went.
    fn send_kept(&mut self, group: GroupId, peer: PeerId, device: DevicePublicKey) {
        let now = Instant::now();
        for (id, wire) in self.kept_lines.due_to(group, &device, now) {
            if self.send_kept_line(group, id, peer, wire) {
                self.kept_lines.sent(&id, device, now);
            }
        }
    }

    /// At a room heartbeat: what is no longer kept goes, and the lines due go to the
    /// connected members that lack them.
    pub(super) fn kept_heartbeat(&mut self) {
        self.tidy_kept();
        let mut groups: Vec<GroupId> = self.kept_lines.lines.iter().map(|l| l.group).collect();
        groups.dedup();
        let mut to = Vec::new();
        for group in groups {
            let Some(slot) = self.groups.get(&group) else {
                continue;
            };
            for peer in &slot.members {
                if let Some(member) = self.members.get(peer) {
                    to.push((group, *peer, member.device));
                }
            }
        }
        to.sort_unstable_by_key(|(g, p, _)| (*g, *p));
        to.dedup_by_key(|(g, p, _)| (*g, *p));
        for (group, peer, device) in to {
            self.send_kept(group, peer, device);
        }
    }

    /// Lets go of what is no longer kept: lines too old, a group this device is known to
    /// have left, members no longer in it.
    pub(super) fn tidy_kept(&mut self) {
        let (groups, all_groups) = (&self.groups, self.kept_lines.all_groups);
        self.kept_lines
            .tidy(now_ms(), |group| match groups.get(group) {
                Some(slot) if slot.ctx.i_am_member() => Some(Some(slot.ctx.member_devices())),
                Some(_) => Some(None),
                None => all_groups.then_some(None),
            });
    }

    /// Sends kept line `wire`, message `id` of `group`, to member `peer`, naming every
    /// connected member as reached, so that `peer`, when it takes `Spread`, passes it on to
    /// none of them (an older device gets `Deliver`, and passes it on as it does any). One
    /// still on its way in this session is left to own.rs. Returns whether it went.
    fn send_kept_line(
        &mut self,
        group: GroupId,
        id: [u8; 16],
        peer: PeerId,
        wire: WireMessage,
    ) -> bool {
        if self.own_going.holds(&id) {
            return false;
        }
        let request = if self.takes_spread(&peer) {
            let members: Vec<PeerId> = (self.groups.get(&group))
                .map(|slot| slot.members.iter().copied().collect())
                .unwrap_or_default();
            MemberRequest::Spread {
                message: wire,
                reached: self.reached_with(&[], &members),
            }
        } else {
            MemberRequest::Deliver(wire)
        };
        self.request(peer, request, Pending::Own(group, id, None));
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::{DeviceIdentity, GroupKeys, SealedMessage};
    use fogata_proto::{GroupPayload, MemberResponse};
    use fogata_state::{FIRST_ROOM, GroupLog};

    use super::*;
    use crate::{
        Command, GroupSetup, NodeConfig, NodeEvents, NodeHandle,
        node::{
            spread::Forward,
            tests::{add_member, inspect, through_relay},
        },
        start,
    };

    /// A file in a new folder of the test's own, removed with the folder when dropped.
    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("fogata-kept-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir.join("lines"))
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            if let Some(dir) = self.0.parent() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    /// A line of `author`'s sealed with `keys`, said at `at`.
    fn line(
        keys: &GroupKeys,
        author: &DeviceIdentity,
        text: &str,
        at: u64,
    ) -> ([u8; 16], WireMessage) {
        let payload = GroupPayload::RoomChat {
            voice: FIRST_ROOM,
            room: [3; 16],
            nick: "ana".into(),
            text: text.into(),
        };
        let sealed = SealedMessage::seal(keys, author, at, &payload.encode()).unwrap();
        (sealed.header.message_id, WireMessage::from(&sealed))
    }

    /// A group of Ana's, and its key.
    fn group() -> (Arc<DeviceIdentity>, GroupLog, GroupKeys) {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let keys = log.current_keys(&ana).unwrap();
        (ana, log, keys)
    }

    /// A node of Ana's in `log`'s group.
    fn node(ana: &Arc<DeviceIdentity>, log: GroupLog) -> (NodeHandle, NodeEvents) {
        start(NodeConfig {
            identity: ana.clone(),
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
        .unwrap()
    }

    fn someone() -> DevicePublicKey {
        DeviceIdentity::generate().unwrap().public_key()
    }

    /// The times of the lines kept, the oldest first.
    fn times(kept: &KeptLines) -> Vec<u64> {
        kept.lines.iter().map(|l| l.wire.timestamp_ms).collect()
    }

    /// How many lines of `group` `device` lacks.
    fn owed(kept: &KeptLines, group: GroupId, device: &DevicePublicKey) -> usize {
        (kept.lines.iter())
            .filter(|l| l.group == group && l.lacking.contains(device))
            .count()
    }

    /// Makes member `device` of `group` connected to `n` as `peer`, still owed what is said
    /// in the first voice room.
    fn connect(n: &mut Node, group: GroupId, device: DevicePublicKey) -> PeerId {
        let dropped = n.kept_lines.dropped.entry((group, FIRST_ROOM)).or_default();
        dropped.insert(device);
        let peer = PeerId::random();
        add_member(n, group, peer);
        n.members.get_mut(&peer).unwrap().device = device;
        through_relay(n, peer, PeerId::random(), 1);
        peer
    }

    /// Member `peer` answers a request carrying line `id` of `group` with `response`.
    fn answer(n: &mut Node, peer: PeerId, group: GroupId, id: [u8; 16], response: MemberResponse) {
        n.on_response(peer, Pending::Own(group, id, None), response);
    }

    /// The requests carrying line `id` to `peer`, sent or waiting their turn.
    fn sent(n: &Node, peer: PeerId, id: [u8; 16]) -> usize {
        let waiting =
            (n.outbox.get(&peer).into_iter()).flat_map(|o| o.waiting.iter().map(|(_, kind)| *kind));
        (n.pending.values())
            .filter(|(p, _)| *p == peer)
            .map(|(_, kind)| *kind)
            .chain(waiting)
            .filter(|kind| matches!(kind, Pending::Own(_, i, _) if *i == id))
            .count()
    }

    /// A line is kept for a member until it takes it, also for a later session: a refusal
    /// keeps it, and it goes again to the member at the next heartbeat, not at every one.
    /// Once every member owed it took it, it goes, and so does the file.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_line_is_kept_until_taken_and_a_refusal_keeps_it() {
        let (ana, mut log, _) = group();
        let bea = someone();
        log.add(&ana, bea, "bea", 2).unwrap();
        let group = log.group_id();
        let keys = log.current_keys(&ana).unwrap();
        let (node, _events) = node(&ana, log);
        let file = TempFile::new("taken");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let (id, wire) = line(&keys, &ana, "while away", now_ms());
        let epoch = keys.epoch();
        let found = inspect(&node, move |n| {
            n.keep_own_lines(path.clone(), key.clone(), true);
            let dropped = n.kept_lines.dropped.entry((group, FIRST_ROOM)).or_default();
            dropped.insert(bea);
            n.keep_line(group, FIRST_ROOM, id, &wire);
            n.kept_lines.follow(true);
            let reread = owed(
                &KeptLines::open(path.clone(), key, true, now_ms()),
                group,
                &bea,
            );
            let peer = connect(n, group, bea);
            n.kept_proved(group, peer, epoch);
            let on_proof = sent(n, peer, id);
            answer(n, peer, group, id, MemberResponse::Refused);
            let after_refusal = owed(&n.kept_lines, group, &bea);
            n.kept_heartbeat();
            let again = sent(n, peer, id);
            n.kept_heartbeat();
            let not_again = sent(n, peer, id);
            answer(n, peer, group, id, MemberResponse::Ack);
            let left = n.kept_lines.lines.len();
            n.kept_lines.follow(true);
            let file_left = path.exists();
            (
                reread,
                on_proof,
                after_refusal,
                again,
                not_again,
                left,
                file_left,
            )
        })
        .await;
        assert_eq!(found, (1, 1, 1, 2, 2, 0, false));
        node.shutdown().await.unwrap();
    }

    /// A line sealed with a key a member is not known to have does not go to it, and its
    /// `Ack` (which it gives to a line it cannot read) lets it go of nothing; a member is
    /// known to have only the keys the log gave it: the one it proved itself with and those
    /// it was given before, and, once its roster names the same log, every one it was given.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_line_goes_to_a_member_only_once_it_reads_its_key() {
        let (ana, mut log, first) = group();
        // A key Bea is never given, from before she came in.
        log.rotate(&ana, 2).unwrap();
        let bea = someone();
        log.add(&ana, bea, "bea", 3).unwrap();
        let before = log.current_keys(&ana).unwrap().epoch();
        log.rotate(&ana, 4).unwrap();
        let keys = log.current_keys(&ana).unwrap();
        assert_ne!(keys.epoch(), before);
        let group = log.group_id();
        let (node, _events) = node(&ana, log);
        let file = TempFile::new("epoch");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let (id, wire) = line(&keys, &ana, "after the new key", now_ms());
        let (old, old_wire) = line(&first, &ana, "under a key Bea never had", now_ms());
        let found = inspect(&node, move |n| {
            n.keep_own_lines(path, key, true);
            let peer = connect(n, group, bea);
            n.keep_line(group, FIRST_ROOM, old, &old_wire);
            n.keep_line(group, FIRST_ROOM, id, &wire);
            // Bea proves herself with the key she has, the one before.
            n.kept_proved(group, peer, before);
            let on_proof = [sent(n, peer, id), sent(n, peer, old)];
            answer(n, peer, group, id, MemberResponse::Ack);
            let after_ack = owed(&n.kept_lines, group, &bea);
            // Her roster names the same log.
            let heads = n.groups[&group].ctx.heads_digest();
            n.on_request(
                peer,
                MemberRequest::Roster {
                    group_id: group.as_bytes().to_vec(),
                    heads: Some(serde_bytes::ByteBuf::from(heads)),
                    banner: None,
                    picture: None,
                },
            );
            let caught_up = [sent(n, peer, id), sent(n, peer, old)];
            answer(n, peer, group, id, MemberResponse::Ack);
            (on_proof, after_ack, caught_up, n.kept_lines.lines.len())
        })
        .await;
        assert_eq!(found, ([0, 0], 2, [1, 0], 1));
        node.shutdown().await.unwrap();
    }

    /// A member no longer connected in a group is no longer known to read that group's
    /// keys: a line of the group does not go to it, and its `Ack` lets it go of nothing,
    /// until it proves itself again; then the line goes at once, whatever went to it before.
    /// What is known of it in another group, and what went to it there, stays.
    #[test]
    fn a_member_gone_is_known_to_read_again_only_once_it_proves_itself() {
        let (ana, log, keys) = group();
        let other = GroupLog::create(&ana, "h", 1).unwrap();
        let other_keys = other.current_keys(&ana).unwrap();
        let (group, elsewhere) = (log.group_id(), other.group_id());
        let bea = someone();
        let file = TempFile::new("gone");
        let key = DataKey::generate().unwrap();
        let mut kept = KeptLines::open(file.0.clone(), key, true, now_ms());
        let (id, wire) = line(&keys, &ana, "here", now_ms());
        let (other_id, other_wire) = line(&other_keys, &ana, "there", now_ms());
        let to_bea = BTreeSet::from([bea]);
        kept.read(group, bea, vec![keys.epoch()]);
        kept.read(elsewhere, bea, vec![other_keys.epoch()]);
        // Both went to her just now.
        kept.keep(group, id, &wire, to_bea.clone(), &to_bea);
        kept.keep(elsewhere, other_id, &other_wire, to_bea.clone(), &to_bea);
        let now = Instant::now();
        assert!(kept.due_to(group, &bea, now).is_empty());
        kept.member_gone(group, &bea);
        // What went to her counts no more there, and still does in her other group.
        let went = |kept: &KeptLines, g: GroupId| {
            (kept.lines.iter()).any(|l| l.group == g && l.sent.contains_key(&bea))
        };
        assert!(!went(&kept, group));
        assert!(went(&kept, elsewhere));
        assert!(!kept.knows(group, &bea, keys.epoch()));
        assert!(kept.due_to(group, &bea, now).is_empty());
        kept.answered(&id, &bea, true);
        assert_eq!(owed(&kept, group, &bea), 1);
        // Her other group is as it was: known, and not due again before its time.
        assert!(kept.knows(elsewhere, &bea, other_keys.epoch()));
        assert!(kept.due_to(elsewhere, &bea, now).is_empty());
        assert_eq!(owed(&kept, elsewhere, &bea), 1);
        // She proves herself again: the line is due at once, not a minute after it went.
        kept.read(group, bea, vec![keys.epoch()]);
        let due: Vec<[u8; 16]> = (kept.due_to(group, &bea, Instant::now()).into_iter())
            .map(|(i, _)| i)
            .collect();
        assert_eq!(due, vec![id]);
        kept.answered(&id, &bea, true);
        assert_eq!(owed(&kept, group, &bea), 0);
    }

    /// A line whose request to a member did not get there goes to it again at the next
    /// heartbeat, not a minute after it went, and only once; a request to another member,
    /// or of a peer that is no member, changes nothing.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_line_that_did_not_get_there_goes_again_at_the_next_heartbeat() {
        let (ana, mut log, _) = group();
        let (bea, cai) = (someone(), someone());
        log.add(&ana, bea, "bea", 2).unwrap();
        log.add(&ana, cai, "cai", 3).unwrap();
        let group = log.group_id();
        let keys = log.current_keys(&ana).unwrap();
        let epoch = keys.epoch();
        let (node, _events) = node(&ana, log);
        let file = TempFile::new("unsent");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let (id, wire) = line(&keys, &ana, "lost on the way", now_ms());
        let found = inspect(&node, move |n| {
            n.keep_own_lines(path, key, true);
            let to_bea = connect(n, group, bea);
            let to_cai = connect(n, group, cai);
            n.kept_proved(group, to_bea, epoch);
            n.kept_proved(group, to_cai, epoch);
            // Said while both are connected: it goes to them now.
            n.keep_line(group, FIRST_ROOM, id, &wire);
            n.kept_heartbeat();
            let before = [sent(n, to_bea, id), sent(n, to_cai, id)];
            n.kept_unsent(to_bea, id);
            n.kept_unsent(to_cai, [9; 16]);
            n.kept_unsent(PeerId::random(), id);
            n.kept_heartbeat();
            let after = [sent(n, to_bea, id), sent(n, to_cai, id)];
            n.kept_heartbeat();
            let once = [sent(n, to_bea, id), sent(n, to_cai, id)];
            (before, after, once)
        })
        .await;
        assert_eq!(found, ([0, 0], [1, 0], [1, 0]));
        node.shutdown().await.unwrap();
    }

    /// A line is owed to the members in its room: one listed there, or let go of it without
    /// a leave (cut off, as a closed lid is), but not one that never entered it nor one that
    /// left it saying so; and one that comes back and leaves saying so is let go of it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_line_is_owed_only_to_those_in_its_room_that_did_not_leave() {
        let (ana, mut log, keys) = group();
        let others: Vec<DeviceIdentity> = (0..3)
            .map(|_| DeviceIdentity::generate().unwrap())
            .collect();
        for (n, other) in others.iter().enumerate() {
            log.add(&ana, other.public_key(), "m", n as u64 + 2)
                .unwrap();
        }
        let group = log.group_id();
        let keys = log.current_keys(&ana).unwrap_or(keys);
        let (node, _events) = node(&ana, log);
        let file = TempFile::new("owed");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let (bea, cai, dan) = (
            others[0].public_key(),
            others[1].public_key(),
            others[2].public_key(),
        );
        let (id, wire) = line(&keys, &ana, "while away", now_ms());
        let found = inspect(&node, move |n| {
            n.keep_own_lines(path, key, true);
            n.on_command(Command::JoinRoom {
                group,
                voice: FIRST_ROOM,
                nick: "ana".into(),
            });
            let room = n.groups[&group].rooms[&FIRST_ROOM].room().unwrap().id;
            let now = now_ms();
            let say = |n: &mut Node, who: &DeviceIdentity, payload: GroupPayload, at: u64| {
                let sealed = SealedMessage::seal(&keys, who, at, &payload.encode()).unwrap();
                n.receive(PeerId::random(), WireMessage::from(&sealed), Forward::No);
            };
            let join = |nick: &str| GroupPayload::RoomJoin {
                voice: FIRST_ROOM,
                room,
                started_ms: now,
                nick: nick.into(),
            };
            let leave = GroupPayload::RoomLeave {
                voice: FIRST_ROOM,
                room,
            };
            // Bea and Cai enter; Cai leaves saying so; Dan never comes.
            say(n, &others[0], join("bea"), now);
            say(n, &others[1], join("cai"), now);
            say(n, &others[1], leave.clone(), now + 1);
            // Bea is cut off and let go without a leave.
            let tracker = n
                .groups
                .get_mut(&group)
                .unwrap()
                .rooms
                .get_mut(&FIRST_ROOM)
                .unwrap();
            // Past the 25 s a participant nobody can hear from keeps its place.
            let later = now + 25_000;
            tracker.tick(later);
            let changes = tracker.drop_cut_off(later, |d| *d == bea);
            n.kept_room_changes(group, FIRST_ROOM, &changes);
            n.keep_line(group, FIRST_ROOM, id, &wire);
            let owed = [bea, cai, dan].map(|d| owed(&n.kept_lines, group, &d));
            // Bea comes back and leaves saying so: the line goes.
            say(n, &others[0], join("bea"), later + 1);
            say(n, &others[0], leave, later + 2);
            (owed, n.kept_lines.lines.len())
        })
        .await;
        assert_eq!(found, ([1, 0, 0], 0));
        node.shutdown().await.unwrap();
    }

    /// At most 16 lines of a group and 64 in all are kept, the newest; a line goes a day
    /// after it was said, by the clock either way, and with its group once this device is
    /// no longer in it; a member that is no longer one is let go of it.
    #[test]
    fn kept_lines_are_bounded_in_number_and_time_and_go_with_their_group() {
        let (ana, log, keys) = group();
        let others: Vec<(GroupId, GroupKeys)> = (0..4)
            .map(|_| {
                let log = GroupLog::create(&ana, "g", 1).unwrap();
                (log.group_id(), log.current_keys(&ana).unwrap())
            })
            .collect();
        let (bea, cai) = (someone(), someone());
        let file = TempFile::new("bounds");
        let now = now_ms();
        let mut kept = KeptLines::open(file.0.clone(), DataKey::generate().unwrap(), true, now);
        let none = BTreeSet::new();
        for at in 0..20 {
            let (id, wire) = line(&keys, &ana, "a", now + at);
            kept.keep(log.group_id(), id, &wire, BTreeSet::from([bea, cai]), &none);
        }
        assert_eq!(times(&kept), (now + 4..now + 20).collect::<Vec<_>>());
        for (n, (group, keys)) in others.iter().enumerate() {
            for at in 0..16 {
                let (id, wire) = line(keys, &ana, "b", now + 100 * (n as u64 + 1) + at);
                kept.keep(*group, id, &wire, BTreeSet::from([bea]), &none);
            }
        }
        // The first group's lines, the oldest, went first.
        assert_eq!(kept.lines.len(), MOST);
        assert!(kept.lines.iter().all(|l| l.group != log.group_id()));
        // Bea no longer a member of the first of the others; this device no longer in the
        // second; the third not known here.
        let (first, second, third) = (others[0].0, others[1].0, others[2].0);
        kept.tidy(now, |group| match *group {
            g if g == first => Some(Some(BTreeSet::from([cai]))),
            g if g == second => Some(None),
            g if g == third => None,
            _ => Some(Some(BTreeSet::from([bea, cai]))),
        });
        assert_eq!(kept.lines.len(), 32);
        assert!(
            kept.lines
                .iter()
                .all(|l| l.group != first && l.group != second)
        );
        let everyone = |_: &GroupId| Some(Some(BTreeSet::from([bea, cai])));
        // A day on, by a clock set back or forward, it goes.
        let mut back = KeptLines {
            lines: kept.lines.drain(..1).collect(),
            ..KeptLines::default()
        };
        back.tidy(now - KEPT_FOR_MS - 1, everyone);
        assert!(back.lines.is_empty());
        kept.tidy(now + KEPT_FOR_MS - 1, everyone);
        assert_eq!(kept.lines.len(), 31);
        kept.tidy(now + KEPT_FOR_MS + 1_000, everyone);
        assert!(kept.lines.is_empty());
    }

    /// The file opens only with its key (with another it is removed), its size does not
    /// change with a line more, and the lines cannot be read in it, not even their ids.
    #[test]
    fn the_file_of_lines_opens_only_with_its_key() {
        let (ana, log, keys) = group();
        let file = TempFile::new("sealed");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let mut kept = KeptLines::open(path.clone(), key.clone(), true, now_ms());
        let (id, wire) = line(&keys, &ana, "secret", now_ms());
        let none = BTreeSet::new();
        kept.keep(
            log.group_id(),
            id,
            &wire,
            BTreeSet::from([someone()]),
            &none,
        );
        kept.follow(true);
        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.windows(16).any(|w| w == id));
        assert!(!bytes.windows(32).any(|w| w == &wire.ciphertext[..32]));
        let (more, wire) = line(&keys, &ana, "and more", now_ms());
        kept.keep(
            log.group_id(),
            more,
            &wire,
            BTreeSet::from([someone()]),
            &none,
        );
        kept.follow(true);
        assert_eq!(std::fs::read(&path).unwrap().len(), bytes.len());
        assert_eq!(
            KeptLines::open(path.clone(), key, true, now_ms())
                .lines
                .len(),
            2
        );
        let other = KeptLines::open(path.clone(), DataKey::generate().unwrap(), true, now_ms());
        assert!(other.lines.is_empty());
        assert!(!path.exists());
    }

    /// A line that expired while nothing ran is gone as the file opens, before anything
    /// could send it.
    #[test]
    fn a_line_expired_in_the_file_is_gone_at_open() {
        let (ana, log, keys) = group();
        let file = TempFile::new("expired");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let said = now_ms() - KEPT_FOR_MS - 1_000;
        let mut kept = KeptLines::open(path.clone(), key.clone(), true, said);
        let (id, wire) = line(&keys, &ana, "long ago", said);
        kept.keep(
            log.group_id(),
            id,
            &wire,
            BTreeSet::from([someone()]),
            &BTreeSet::new(),
        );
        kept.follow(true);
        assert!(path.exists());
        assert!(KeptLines::open(path, key, true, now_ms()).lines.is_empty());
    }

    /// A member dropped from a group after its device was no longer known here lets go of
    /// what is known of what it reads there, and of what went to it, as one whose device is
    /// known does; what is known of a member still connected stays, also of a device still
    /// connected through another of its sessions, and of a member of another group.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_dropped_without_its_device_known_is_not_known_to_read() {
        let (ana, mut log, _) = group();
        let (bea, cai, dan, eve) = (someone(), someone(), someone(), someone());
        let ana_too = ana.clone();
        log.add(&ana, bea, "bea", 2).unwrap();
        log.add(&ana, cai, "cai", 3).unwrap();
        log.add(&ana, dan, "dan", 4).unwrap();
        let group = log.group_id();
        let keys = log.current_keys(&ana).unwrap();
        let epoch = keys.epoch();
        let (node, _events) = node(&ana, log);
        let file = TempFile::new("unknown");
        let (path, key) = (file.0.clone(), DataKey::generate().unwrap());
        let (id, wire) = line(&keys, &ana, "to all", now_ms());
        let found = inspect(&node, move |n| {
            n.keep_own_lines(path, key, true);
            let to_bea = connect(n, group, bea);
            let to_cai = connect(n, group, cai);
            let to_dan = connect(n, group, dan);
            let to_dan_too = connect(n, group, dan);
            for peer in [to_bea, to_cai, to_dan, to_dan_too] {
                n.kept_proved(group, peer, epoch);
            }
            // Said while all are connected: it goes to each now.
            n.keep_line(group, FIRST_ROOM, id, &wire);
            let known = |n: &Node| [bea, cai, dan].map(|d| n.kept_lines.knows(group, &d, epoch));
            let went = |n: &Node| {
                let line = n.kept_lines.lines.iter().find(|l| l.id == id).unwrap();
                [bea, cai, dan].map(|d| line.sent.contains_key(&d))
            };
            // Eve is a member of another group of Ana's only, connected there.
            let mut other = GroupLog::create(&ana_too, "h", 1).unwrap();
            other.add(&ana_too, eve, "eve", 2).unwrap();
            let elsewhere = other.group_id();
            let other_epoch = other.current_keys(&ana_too).unwrap().epoch();
            n.add_group(GroupSetup::Member(other));
            let to_eve = connect(n, elsewhere, eve);
            n.kept_proved(elsewhere, to_eve, other_epoch);
            let before = (known(n), went(n));
            for peer in [to_bea, to_dan] {
                n.members.remove(&peer);
                n.drop_member(group, peer);
            }
            let eve_known = n.kept_lines.knows(elsewhere, &eve, other_epoch);
            let after = (known(n), went(n), eve_known);
            (before, after, n.kept_lines.reads.len())
        })
        .await;
        let (all, bea_gone) = ([true; 3], [false, true, true]);
        assert_eq!(found, ((all, all), (bea_gone, bea_gone, true), 3));
        node.shutdown().await.unwrap();
    }
}
