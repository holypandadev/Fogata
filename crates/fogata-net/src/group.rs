//! The node's view of its group: the log, the keys of every epoch it can read, and whether it
//! is still joining with an invite.

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId, GroupKeys};
use fogata_state::{Barrier, Entry, EntryHash, GroupLog, GroupState, StateError, Taken, Ticket};

/// How long after a key epoch began the two before the ones announced are still looked up
/// (see `GroupContext::lookup_secrets`).
const OLDER_EPOCHS_FOR_MS: u64 = 14 * 24 * 3600 * 1000;

/// Most entries this device lacks that a removal shown by a device it does not verify may
/// bring along (see `GroupContext::my_removal`).
pub(crate) const MAX_REMOVAL_ENTRIES: usize = 512;
/// Most removals of this device among those entries whose signature is checked: honest
/// members send one (two removals at once from different members are rare).
const MAX_REMOVALS_CHECKED: usize = 2;

/// Most bytes of entries, and most hashes of entries wanted back, in one answer to a sync,
/// and in one part of a log sent to a device let in: a message must arrive within a
/// request's 20 seconds also through a member's relay or a phone's connection (256 KiB
/// takes 2 seconds at 1 Mbit/s, 16 at 128 kbit/s). The rest goes in the next sync, asked for
/// at once while answers bring new entries, or in the next part.
pub(crate) const SYNC_BYTES: usize = 256 * 1024;
const SYNC_WANTED: usize = 4096;

/// How long a key epoch lasts: a manager online starts a new one once the current one is
/// this old, so a key someone took reads the group's messages for a week at most.
pub(crate) const KEY_LIFETIME_MS: u64 = 7 * 24 * 3600 * 1000;
/// Each manager after the first (in the order of the ladder) waits this much longer, so
/// that the managers online do not all rotate at once.
const KEY_LIFETIME_STAGGER_MS: u64 = 3600 * 1000;
/// How far ahead of this device's clock an epoch may say it started and still be aged from
/// then: clocks differ by less. One dated later is taken as a week old when this device
/// first saw it (see `GroupContext::epoch_began`).
const EPOCH_AHEAD_MS: u64 = 24 * 3600 * 1000;

/// A manager writes a new key epoch at most this often for a group whose current key it was
/// not given (see `GroupContext::mend_key`).
const MEND_EVERY_MS: u64 = 60_000;
/// Invites written again at once (see [`GroupContext::name_unnamed_invites`]): a person makes
/// a few, one at a time.
const MAX_INVITES_NAMED_AGAIN: usize = 16;
/// How many new epochs one member's reports of a box that does not open make this manager
/// write in [`MENDS_FOR_MS`]: each is an entry every member keeps, with a box for each.
const MENDS_PER_MEMBER: u8 = 3;
const MENDS_FOR_MS: u64 = 24 * 3600 * 1000;

/// How a node takes part in its group.
// Made once per group when a node starts, and the log moved out at once: its size matters
// little, and boxing it would only add a step for every caller.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum GroupSetup {
    /// The device is a member and has the group log.
    Member(GroupLog),
    /// The device holds an invite and has not joined yet.
    Joining {
        group_id: GroupId,
        ticket: Ticket,
        /// Rendezvous secret from the invite, to find members on the DHT.
        rendezvous: [u8; 32],
        /// Name to show the others.
        name: String,
    },
}

/// Why a manager's yes did not let a device in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NotLetIn {
    /// The invite no longer works, the group is full, or the join is not one to take.
    Invalid,
    /// This device has no group key to give yet: it lets the device in once it has one.
    NoKeyYet,
    /// This device wrote more than the other members have seen yet (see
    /// [`fogata_state::PAST_RUN`]).
    WroteTooMuch,
    /// The join is signed on entries the group does not keep.
    SignedOnSpare,
}

pub(crate) struct GroupContext {
    identity: Arc<DeviceIdentity>,
    log: GroupLog,
    state: GroupState,
    /// Keys by epoch number.
    keys: BTreeMap<u32, Arc<GroupKeys>>,
    current: Option<Arc<GroupKeys>>,
    joining: Option<(Ticket, [u8; 32], String)>,
    /// Devices whose join this device let in, in this session, with that join: a manager
    /// gives a first key only to them (see `welcome_lacking`).
    accepted: BTreeMap<DevicePublicKey, EntryHash>,
    /// When this device last wrote a new epoch because the current key was not given to it
    /// or a member's did not open, and the epoch it replaced for a member's report.
    mended_ms: Option<u64>,
    mended_for: Option<fogata_state::EpochId>,
    /// New epochs written for each member's reports, since when (see [`MENDS_PER_MEMBER`]).
    mended_by: BTreeMap<DevicePublicKey, (u64, u8)>,
    /// Since when this manager has been left without the current key while a manager above
    /// it was connected, by this device's clock.
    keyless_since: Option<u64>,
    /// The epoch whose missing key was last reported, so it is reported once.
    reported: Option<fogata_state::EpochId>,
    /// The current epoch and when this device first saw it, by its own clock (see
    /// `GroupContext::epoch_began`).
    current_seen: Option<(fogata_state::EpochId, u64)>,
    /// The digests of the banner and the picture record last asked for, by the record they
    /// are of (see [`DigestCache`]).
    digests: std::sync::Mutex<DigestCache>,
}

/// The digests of a group's banner and picture record, each with the time and signature of
/// the record it is of: a digest hashes the whole record, up to a few hundred KiB, and every
/// roster a member asks for compares them, so each is worked out once per record. A record
/// is known by its time and signature: one with the same of both is the same record.
#[derive(Default)]
struct DigestCache {
    banner: Option<(u64, [u8; 64], Vec<u8>)>,
    picture: Option<(u64, [u8; 64], Vec<u8>)>,
}

/// `record`'s digest from `cached` when it is of the same record, worked out (and kept)
/// otherwise; empty for none.
fn cached_digest<K: fogata_state::RecordKind>(
    cached: &mut Option<(u64, [u8; 64], Vec<u8>)>,
    record: Option<&fogata_state::GroupRecord<K>>,
) -> Vec<u8> {
    let Some(record) = record else {
        return Vec::new();
    };
    if let Some((time, signature, digest)) = cached
        && *time == record.time_ms
        && *signature == record.signature
    {
        return digest.clone();
    }
    let digest = record.digest().to_vec();
    *cached = Some((record.time_ms, record.signature, digest.clone()));
    digest
}

/// What a room event weighs as a room keeps it, to hand to members that come online: the
/// whole message as sent (see [`fogata_state::RoomTracker::weighing`]).
pub(crate) fn room_record_weight(wire: &fogata_proto::WireMessage) -> usize {
    // The version, epoch and time.
    13 + [
        &wire.group_id,
        &wire.sender,
        &wire.message_id,
        &wire.nonce,
        &wire.ciphertext,
        &wire.signature,
    ]
    .iter()
    .map(|b| b.len())
    .sum::<usize>()
}

/// This device's clock, in milliseconds since 1970.
fn wall_clock_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A short digest of a log's heads.
fn digest(mut heads: Vec<EntryHash>) -> Vec<u8> {
    heads.sort_unstable();
    let mut hasher = blake3::Hasher::new();
    for head in &heads {
        hasher.update(head);
    }
    hasher.finalize().as_bytes()[..16].to_vec()
}

impl GroupContext {
    pub(crate) fn new(identity: Arc<DeviceIdentity>, setup: GroupSetup) -> Self {
        let (log, joining) = match setup {
            GroupSetup::Member(log) => (log, None),
            GroupSetup::Joining {
                group_id,
                ticket,
                rendezvous,
                name,
            } => (GroupLog::empty(group_id), Some((ticket, rendezvous, name))),
        };
        let mut ctx = Self {
            identity,
            state: GroupState::default(),
            log,
            keys: BTreeMap::new(),
            current: None,
            joining,
            accepted: BTreeMap::new(),
            mended_ms: None,
            mended_for: None,
            mended_by: BTreeMap::new(),
            keyless_since: None,
            reported: None,
            current_seen: None,
            digests: Default::default(),
        };
        ctx.refresh();
        ctx
    }

    /// Lets go of spare entries past their room (see [`GroupLog::trim`]). Returns whether
    /// any went; the group is the same without them.
    pub(crate) fn trim(&mut self) -> bool {
        let trimmed = self.log.trim();
        if trimmed {
            self.refresh();
        }
        trimmed
    }

    /// Recomputes state and keys after the log changed.
    fn refresh(&mut self) {
        let (state, keys, current) = self.log.state_and_keys(&self.identity);
        self.state = state;
        self.keys = keys.into_iter().map(|(n, k)| (n, Arc::new(k))).collect();
        self.current = current.map(Arc::new);
        if self.joining.is_some() && self.current.is_some() {
            self.joining = None;
        }
        self.report_key_problem();
    }

    /// Says once per epoch why this device, a member, lacks the current key, naming who
    /// wrote the entry that should have given it.
    fn report_key_problem(&mut self) {
        let Some(epoch) = self.state.current_epoch().map(|e| e.id) else {
            return;
        };
        if self.current.is_some() || !self.i_am_member() || self.reported == Some(epoch) {
            return;
        }
        self.reported = Some(epoch);
        match self.log.current_key_problem(&self.identity, &self.state) {
            Some(problem) if problem.bad_box => tracing::warn!(
                "the group key of {:?} is broken: {} gave this device a key that does not \
                 open; a manager can make a new one",
                self.group_id(),
                problem.by.fingerprint()
            ),
            Some(problem) => tracing::info!(
                "{} started a new key of {:?} without this device; waiting for a manager",
                problem.by.fingerprint(),
                self.group_id()
            ),
            None => {}
        }
    }

    /// Adds every entry of `other` this context lacks, a copy of the log this device holds.
    /// Returns the new hashes.
    pub(crate) fn merge(&mut self, other: &GroupLog) -> Vec<EntryHash> {
        let new = self
            .log
            .insert_all(other.entries_in_order().into_iter().cloned());
        self.log.keep_ahead_of(other);
        if !new.is_empty() {
            self.refresh();
        }
        new
    }

    pub(crate) fn group_id(&self) -> GroupId {
        self.log.group_id()
    }

    pub(crate) fn name(&self) -> &str {
        &self.state.name
    }

    /// Whether the log lists voice room `voice` (the first one always is).
    pub(crate) fn has_voice_room(&self, voice: &fogata_state::VoiceRoomId) -> bool {
        self.state.rooms.iter().any(|r| r.id == *voice)
    }

    /// How many people a voice room of the group takes at once.
    pub(crate) fn max_in_room(&self) -> u8 {
        self.state.max_in_room
    }

    /// How many voice rooms the group has (the first one always counts).
    pub(crate) fn voice_rooms(&self) -> usize {
        self.state.rooms.len().max(1)
    }

    pub(crate) fn log(&self) -> &GroupLog {
        &self.log
    }

    /// Ends the join as being let in does, for the tests.
    #[cfg(test)]
    pub(crate) fn end_join_for_tests(&mut self) {
        self.joining = None;
    }

    pub(crate) fn joining(&self) -> Option<&Ticket> {
        self.joining.as_ref().map(|(t, _, _)| t)
    }

    /// Whether `device` is a member that a manager let in: one in the log that no manager has
    /// given a key yet (a `Join` any member could have written with a ticket it saw) counts
    /// for nothing, even if it got the key some other way.
    pub(crate) fn is_member_device(&self, device: &DevicePublicKey) -> bool {
        self.state.keyed(device)
    }

    /// Whether this device is a member (false while joining or after being removed).
    pub(crate) fn i_am_member(&self) -> bool {
        self.state.is_member(&self.identity.public_key())
    }

    /// The `Leave` this device is no longer a member by, when it left rather than being
    /// removed (or never let in).
    pub(crate) fn my_leave(&self) -> Option<EntryHash> {
        if self.joining.is_some() || self.i_am_member() {
            return None;
        }
        let me = self.identity.public_key();
        for hash in self.state.applied.iter().rev() {
            let Some(entry) = self.log.get(hash) else {
                continue;
            };
            match &entry.body {
                fogata_state::Body::Leave if entry.author == me => return Some(*hash),
                fogata_state::Body::Remove { member, .. }
                | fogata_state::Body::Add { member, .. }
                    if *member == me =>
                {
                    return None;
                }
                fogata_state::Body::Join { .. } if entry.author == me => return None,
                _ => {}
            }
        }
        None
    }

    /// Whether one of the entries `new` is another device's, signed on `entry`: that
    /// device has it.
    pub(crate) fn signed_on_by_others(&self, new: &[EntryHash], entry: &EntryHash) -> bool {
        let me = self.identity.public_key();
        new.iter()
            .filter_map(|h| self.log.get(h))
            .any(|e| e.author != me && e.parents.contains(entry))
    }

    /// How many members the group has, this device included when it is one.
    pub(crate) fn member_count(&self) -> usize {
        self.state.members.len()
    }

    pub(crate) fn i_am_manager(&self) -> bool {
        self.state.is_manager(&self.identity.public_key())
    }

    pub(crate) fn current(&self) -> Option<&Arc<GroupKeys>> {
        self.current.as_ref()
    }

    pub(crate) fn keys(&self, epoch: u32) -> Option<&Arc<GroupKeys>> {
        self.keys.get(&epoch)
    }

    /// The keys of every epoch this device has.
    pub(crate) fn epoch_keys(&self) -> impl Iterator<Item = &Arc<GroupKeys>> {
        self.keys.values()
    }

    /// The keys of the newest `count` epochs this device has, newest first.
    pub(crate) fn newest_epoch_keys(&self, count: usize) -> impl Iterator<Item = &Arc<GroupKeys>> {
        self.state
            .epochs
            .iter()
            .rev()
            .filter_map(|epoch| self.keys.get(&fogata_state::epoch_number(&epoch.id)))
            .take(count)
    }

    /// Epoch numbers this device can prove with, newest first.
    pub(crate) fn known_epochs(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for epoch in self.state.epochs.iter().rev() {
            let n = fogata_state::epoch_number(&epoch.id);
            if self.keys.contains_key(&n) {
                out.push(n);
            }
            if out.len() == 8 {
                break;
            }
        }
        out
    }

    /// Rendezvous secrets to announce and look up: the current epoch and the previous one,
    /// so members who missed a rotation still find the group; for managers, also the tokens of
    /// the invites open at `now_ms`, where the invited devices look for them; or the
    /// invite's while joining.
    pub(crate) fn rendezvous_secrets(&self, now_ms: u64) -> Vec<[u8; 32]> {
        if let Some((_, secret, _)) = &self.joining {
            return vec![*secret];
        }
        let mut out: Vec<[u8; 32]> = self
            .known_epochs()
            .into_iter()
            .take(2)
            .filter_map(|n| self.keys.get(&n).map(|k| *k.rendezvous_secret()))
            .collect();
        if self.i_am_manager() {
            // Only where this device may let the invited device in: an invite that names who
            // does is answered by them alone (`GroupState::may_let_in`).
            out.extend(
                self.state
                    .invite_tokens_for(&self.identity.public_key(), now_ms)
                    .iter()
                    .map(fogata_state::invite_rendezvous),
            );
        }
        out
    }

    /// Rendezvous secrets to look up, and to open sealed local addresses with: those
    /// announced under ([`Self::rendezvous_secrets`]) and two epochs before them, where a
    /// member that missed several rotations still announces itself; it can only find the
    /// others once one of them finds it and sends it the log. Nothing is announced there, so
    /// a removed device that knows those epochs does not see this one's announcements; one
    /// that announces there learns the address of whoever dials it, so an epoch is looked
    /// up only for two weeks after the one that replaced it began.
    pub(crate) fn lookup_secrets(&self, now_ms: u64) -> Vec<[u8; 32]> {
        let mut out = self.rendezvous_secrets(now_ms);
        if self.joining.is_some() || !self.i_am_member() {
            return out;
        }
        let epochs = &self.state.epochs;
        let held: Vec<usize> = (0..epochs.len())
            .rev()
            .filter(|&i| {
                self.keys
                    .contains_key(&fogata_state::epoch_number(&epochs[i].id))
            })
            .collect();
        for &i in held.iter().skip(2).take(2) {
            // A successor dated far ahead of this clock tells nothing of when it began.
            let replaced_ms = epochs
                .get(i + 1)
                .map(|e| e.since_ms)
                .filter(|&t| t <= now_ms.saturating_add(EPOCH_AHEAD_MS))
                .unwrap_or(0);
            if now_ms.saturating_sub(replaced_ms) < OLDER_EPOCHS_FOR_MS
                && let Some(keys) = self.keys.get(&fogata_state::epoch_number(&epochs[i].id))
            {
                out.push(*keys.rendezvous_secret());
            }
        }
        out
    }

    /// The secret this device seals its local addresses under for the group's members: the
    /// current epoch's rendezvous secret, while a member.
    pub(crate) fn lan_secret(&self) -> Option<[u8; 32]> {
        self.current
            .as_ref()
            .filter(|_| self.i_am_member())
            .map(|k| *k.rendezvous_secret())
    }

    /// A short digest of the heads of the log's common order, entries waiting for parents
    /// and spare ones left out (see [`GroupLog::spare`]): two devices with the same one have
    /// the same group, whatever else each keeps (which entries a device keeps waiting, or
    /// spare, depends on when they came, so two devices could otherwise sync for ever).
    pub(crate) fn heads_digest(&self) -> Vec<u8> {
        digest(self.log.needed_heads())
    }

    /// A short digest of the group's banner, empty for none: two devices with the same one
    /// have the same banner.
    pub(crate) fn banner_digest(&self) -> Vec<u8> {
        let mut cache = crate::locked(&self.digests);
        cached_digest(&mut cache.banner, self.log.banner_record())
    }

    /// The group's banner, encoded, when there is one (a record taking it away included).
    pub(crate) fn encoded_banner(&self) -> Option<Vec<u8>> {
        self.log
            .banner_record()
            .map(fogata_state::GroupBanner::encode)
    }

    /// Takes a banner received from anywhere, when it is newer and its author a manager.
    /// Returns whether it was taken.
    /// Whether a banner record encoded as `bytes` may be newer than the one here: false for
    /// one this device would drop as older, read without checking its signature, and an
    /// error for bytes that are no record (see [`fogata_state::GroupRecord::peek`]).
    pub(crate) fn banner_may_be_newer(
        &self,
        bytes: &[u8],
    ) -> Result<bool, fogata_state::DecodeError> {
        let stamp = fogata_state::GroupBanner::peek(bytes)?;
        Ok(self.log.banner_older_than(&stamp))
    }

    pub(crate) fn take_banner(&mut self, banner: fogata_state::GroupBanner) -> bool {
        self.log.take_banner_at(banner, wall_clock_ms())
    }

    /// Chooses the group's banner (a manager), or takes it away with no picture.
    pub(crate) fn set_banner(&mut self, picture: Vec<u8>, now_ms: u64) -> Result<(), String> {
        self.log
            .set_banner(&self.identity, picture, now_ms)
            .map(drop)
            .map_err(|e| e.to_string())
    }

    /// A short digest of the group's picture record, empty for none: two devices with the
    /// same one have the same record.
    pub(crate) fn picture_digest(&self) -> Vec<u8> {
        let mut cache = crate::locked(&self.digests);
        cached_digest(&mut cache.picture, self.log.picture_record())
    }

    /// The group's picture record, encoded, when there is one (one taking it away included).
    pub(crate) fn encoded_picture(&self) -> Option<Vec<u8>> {
        self.log
            .picture_record()
            .map(fogata_state::GroupPicture::encode)
    }

    /// Takes a picture record received from anywhere, when it is newer and its author an
    /// manager. Returns whether it was taken.
    /// [`Self::banner_may_be_newer`] for a picture record.
    pub(crate) fn picture_may_be_newer(
        &self,
        bytes: &[u8],
    ) -> Result<bool, fogata_state::DecodeError> {
        let stamp = fogata_state::GroupPicture::peek(bytes)?;
        Ok(self.log.picture_older_than(&stamp))
    }

    pub(crate) fn take_picture(&mut self, picture: fogata_state::GroupPicture) -> bool {
        self.log.take_picture_at(picture, wall_clock_ms())
    }

    /// Chooses the group's picture (a manager), or takes it away with no picture. Returns
    /// the log entry written with it, if one was (see [`GroupLog::set_picture`]).
    pub(crate) fn set_picture(
        &mut self,
        picture: Vec<u8>,
        now_ms: u64,
    ) -> Result<Vec<EntryHash>, String> {
        let (_, cleared) = self
            .log
            .set_picture(&self.identity, picture, now_ms)
            .map_err(|e| e.to_string())?;
        if cleared.is_some() {
            self.refresh();
        }
        Ok(cleared.map(|e| e.hash()).into_iter().collect())
    }

    /// Why `device` may not enter voice room `voice` now, if it may not.
    pub(crate) fn barrier(
        &self,
        voice: &fogata_state::VoiceRoomId,
        device: &DevicePublicKey,
        now_ms: u64,
    ) -> Option<Barrier> {
        self.state.barrier(voice, device, now_ms)
    }

    /// Whether `author` may take `member` out of a voice room now.
    pub(crate) fn outranks(&self, author: &DevicePublicKey, member: &DevicePublicKey) -> bool {
        self.state.outranks(author, member)
    }

    /// Hashes in the order to send entries in.
    pub(crate) fn hashes(&self) -> Vec<EntryHash> {
        self.log.hashes_in_order()
    }

    pub(crate) fn heads(&self) -> Vec<EntryHash> {
        self.log.heads()
    }

    /// The entries of `hashes` this device has, encoded, up to
    /// [`SYNC_BYTES`] in all: what one answer to a sync, or the entries it asked for, may
    /// carry. The rest goes in the next sync.
    pub(crate) fn encoded_part(&self, hashes: impl IntoIterator<Item = EntryHash>) -> Vec<Vec<u8>> {
        let mut room = SYNC_BYTES;
        hashes
            .into_iter()
            .filter_map(|h| self.log.get(&h))
            .map(Entry::encode)
            .take_while(|bytes| {
                let fits = bytes.len() <= room;
                room = room.saturating_sub(bytes.len());
                fits
            })
            .collect()
    }

    /// The whole log in the log's encoding, in parts of at most
    /// [`SYNC_BYTES`] (an entry longer than that goes alone): what a device let in is sent,
    /// part after part.
    pub(crate) fn encoded_parts(&self) -> Vec<Vec<Vec<u8>>> {
        let mut parts: Vec<Vec<Vec<u8>>> = Vec::new();
        let mut size = 0;
        for entry in self.encoded(self.hashes()) {
            match parts.last_mut() {
                Some(part) if size + entry.len() <= SYNC_BYTES => {
                    size += entry.len();
                    part.push(entry);
                }
                _ => {
                    size = entry.len();
                    parts.push(vec![entry]);
                }
            }
        }
        parts
    }

    /// The entries of `hashes` in the log's encoding.
    pub(crate) fn encoded(&self, hashes: impl IntoIterator<Item = EntryHash>) -> Vec<Vec<u8>> {
        hashes
            .into_iter()
            .filter_map(|h| self.log.get(&h))
            .map(Entry::encode)
            .collect()
    }

    /// Entries we have that are not in `have`, and hashes in `have` we lack.
    pub(crate) fn diff(&self, have: &[EntryHash]) -> (Vec<Vec<u8>>, Vec<EntryHash>) {
        let theirs: HashSet<&EntryHash> = have.iter().collect();
        let entries = self.encoded_part(
            self.log
                .hashes_to_send()
                .into_iter()
                .filter(|h| !theirs.contains(h)),
        );
        let want = have
            .iter()
            .filter(|h| self.log.get(h).is_none())
            .take(SYNC_WANTED)
            .copied()
            .collect();
        (entries, want)
    }

    /// Adds received entries, as many as are worth judging now (see
    /// [`GroupLog::insert_some`]; the rest come with the next sync). Returns the hashes that
    /// were new, and how many were refused.
    pub(crate) fn insert_encoded(&mut self, entries: &[Vec<u8>]) -> Taken {
        let decoded = entries.iter().filter_map(|bytes| Entry::decode(bytes).ok());
        let taken = self.log.insert_some(decoded);
        if !taken.new.is_empty() {
            self.refresh();
        }
        taken
    }

    /// Whether a ticket presented by a joining device is acceptable now. Only managers accept
    /// joins: a member's first key must come from the manager who let it in, who checks the
    /// ticket against the clock (a member could otherwise relay a backdated join with an
    /// expired ticket).
    pub(crate) fn ticket_acceptable(&self, ticket: &Ticket, now_ms: u64) -> bool {
        self.i_am_manager()
            && ticket.group_id == self.group_id()
            && ticket.signature_valid()
            && self.state.is_manager(&ticket.issuer)
            && !self.state.used_tickets.contains(&ticket.ticket_id)
            && now_ms <= ticket.expires_ms
            && self.state.may_let_in(&self.identity.public_key(), ticket)
    }

    /// Whether `ticket` is one of this group's, signed by its issuer: checked before anything
    /// that costs more, since anyone can send one.
    pub(crate) fn ticket_genuine(&self, ticket: &Ticket) -> bool {
        ticket.group_id == self.group_id() && ticket.signature_valid()
    }

    /// Whether a device joined with ticket `ticket_id`.
    pub(crate) fn ticket_used(&self, ticket_id: &[u8; 16]) -> bool {
        self.state.used_tickets.contains(ticket_id)
    }

    /// Lets in a joining device with its signed `Join`: the `Welcome` signed on it gives it
    /// the current key and makes the join apply (a join by itself lets nobody in). Returns
    /// the new entries, or why the device is not in (with no key to give yet, this device
    /// lets it in once it has one, see [`Self::welcome_lacking`]).
    pub(crate) fn accept_join(
        &mut self,
        entry: Entry,
        ticket_id: &[u8; 16],
        now_ms: u64,
    ) -> Result<Vec<EntryHash>, NotLetIn> {
        let fogata_state::Body::Join { ticket, .. } = &entry.body else {
            return Err(NotLetIn::Invalid);
        };
        if &ticket.ticket_id != ticket_id {
            return Err(NotLetIn::Invalid);
        }
        let joiner = entry.author;
        let join_hash = entry.hash();
        self.log.insert(entry).map_err(|_| NotLetIn::Invalid)?;
        self.accepted.insert(joiner, join_hash);
        let welcome = self.log.let_in(&self.identity, &join_hash, now_ms);
        self.refresh();
        match welcome {
            Ok(welcome) if self.state.is_member(&joiner) => Ok(vec![join_hash, welcome.hash()]),
            Ok(_) => Err(NotLetIn::Invalid),
            Err(e) => {
                tracing::debug!("cannot let the new member in: {e}");
                Err(match e {
                    StateError::NoSecret => NotLetIn::NoKeyYet,
                    StateError::NotAllowed(fogata_state::PAST_RUN) => NotLetIn::WroteTooMuch,
                    StateError::NotAllowed(fogata_state::JOIN_ON_SPARE) => NotLetIn::SignedOnSpare,
                    _ => NotLetIn::Invalid,
                })
            }
        }
    }

    /// Gives the current key to members that lack it (managers do this), and lets in the
    /// devices this manager said yes to that are not in yet, with the join it said yes to.
    /// A device that joined gets its first key only from the manager that let it in, while
    /// its ticket is valid: any member can put a `Join` in the log with a ticket it saw
    /// there, also an expired one with an earlier date. Returns new entries.
    pub(crate) fn welcome_lacking(&mut self, now_ms: u64) -> Vec<EntryHash> {
        if !self.i_am_manager() || self.current.is_none() {
            return Vec::new();
        }
        let mut new = Vec::new();
        let waiting: Vec<EntryHash> = self
            .accepted
            .iter()
            .filter(|(device, _)| !self.state.is_member(device))
            .filter(|(device, join)| {
                let ticket = self.log.get(join).and_then(|e| match &e.body {
                    fogata_state::Body::Join { ticket, .. } => Some(ticket),
                    _ => None,
                });
                self.state.waiting_join(device).is_some()
                    && ticket.is_some_and(|t| now_ms <= t.expires_ms)
            })
            .map(|(_, join)| *join)
            .collect();
        for join in waiting {
            if let Ok(entry) = self.log.let_in(&self.identity, &join, now_ms) {
                new.push(entry.hash());
            }
        }
        for member in self.state.lacking_current_secret() {
            let first_key = !self.state.keyed(&member);
            if first_key && !self.accepted.contains_key(&member) {
                continue;
            }
            if first_key
                && self
                    .log
                    .join_ticket_of(&member)
                    .is_some_and(|t| now_ms > t.expires_ms)
            {
                tracing::debug!("not giving a first key to a member whose ticket expired");
                continue;
            }
            if let Ok(entry) = self.log.welcome(&self.identity, member, now_ms) {
                new.push(entry.hash());
            }
        }
        if !new.is_empty() {
            self.refresh();
        }
        new
    }

    /// Starts a new epoch when someone who is no longer a member holds the current secret
    /// (it left, or a rotation was discarded by a concurrent removal, or it was sent the
    /// secret in an entry that did not apply). The member admitted first among the current
    /// holders that may rotate for it rotates at once; the others only with `insist`, for
    /// when that member is not around. Two rotations at once both apply, so insisting is
    /// safe. Returns the new entry.
    ///
    /// For a secret sent in an entry that did not apply, only managers rotate: a rotation
    /// of a member that does not manage counts only where the log counts those entries,
    /// and a version from before that would refuse it, and wait for ever on what is signed
    /// on it.
    pub(crate) fn rotate_if_leaked(&mut self, now_ms: u64, insist: bool) -> Vec<EntryHash> {
        if !self.i_am_member()
            || self.current.is_none()
            || self.state.current_secret_leaked_to().is_empty()
        {
            return Vec::new();
        }
        let anyone = !self.state.current_secret_left_with().is_empty();
        if !anyone && !self.i_am_manager() {
            return Vec::new();
        }
        let me = self.identity.public_key();
        let first = self
            .state
            .current_holders_in_order()
            .into_iter()
            .find(|k| anyone || self.state.is_manager(k));
        if !insist && first != Some(me) {
            return Vec::new();
        }
        match self.log.rotate(&self.identity, now_ms) {
            Ok(entry) => {
                tracing::info!("new group key: someone who left still had the old one");
                self.refresh();
                vec![entry.hash()]
            }
            Err(e) => {
                tracing::debug!("cannot rotate the group key: {e}");
                Vec::new()
            }
        }
    }

    /// Whether a proof made with the key of `epoch` only says something to members: nobody
    /// who left or was removed was given that key, so a device that is no longer in the
    /// group never reads one. Usually only the current epoch is like that; an older one
    /// stays so until someone given it goes.
    ///
    /// A member left without the current key (an epoch started with a box for it that does
    /// not open, or none) answers with the newest key it has, as a member that had not seen
    /// that epoch yet would: otherwise two members both left without it would never prove
    /// themselves to each other, nor get the new key a manager then makes.
    pub(crate) fn key_kept_in(&self, epoch: u32) -> bool {
        self.keys.contains_key(&epoch)
            && (self.state.epoch_kept(epoch)
                || (self.current.is_none()
                    && self.i_am_member()
                    && self.known_epochs().first() == Some(&epoch)))
    }

    /// Whether this device can ask peers to prove membership: a member with a key of the
    /// group (see [`Self::key_kept_in`]).
    pub(crate) fn can_challenge(&self) -> bool {
        self.current.is_some() || (self.i_am_member() && !self.keys.is_empty())
    }

    /// Manager: starts a new epoch once the current one is [`KEY_LIFETIME_MS`] old (by the
    /// clock of the device that started it), later for each manager above it on the ladder
    /// that holds it. In a direct chat's group the one that made it goes first.
    /// Returns the new entry.
    pub(crate) fn rotate_if_old(&mut self, now_ms: u64) -> Vec<EntryHash> {
        let Some(since) = self.epoch_began(now_ms) else {
            return Vec::new();
        };
        if !self.i_am_manager() || self.current.is_none() {
            return Vec::new();
        }
        let me = self.identity.public_key();
        let before_me = self
            .state
            .current_holders_in_order()
            .into_iter()
            .filter(|k| self.state.is_manager(k))
            .take_while(|k| *k != me)
            .count() as u64;
        let due = since
            .saturating_add(KEY_LIFETIME_MS)
            .saturating_add(before_me.saturating_mul(KEY_LIFETIME_STAGGER_MS));
        if now_ms < due {
            return Vec::new();
        }
        match self.log.rotate(&self.identity, now_ms) {
            Ok(entry) => {
                tracing::info!("new group key: the last one was a week old");
                self.refresh();
                vec![entry.hash()]
            }
            Err(e) => {
                tracing::debug!("cannot rotate the group key: {e}");
                Vec::new()
            }
        }
    }

    /// When the current epoch began, to age it by: the time its author put on the entry that
    /// started it, or when this device first saw it if that was earlier (the author's clock
    /// ran ahead). An author can date that entry as late as it likes, and one dated more than
    /// [`EPOCH_AHEAD_MS`] ahead of this device's clock counts as a week old when this device
    /// first saw it: a time near the end of all time would otherwise keep the epoch from ever
    /// being a week old, and the group's key would never change again.
    fn epoch_began(&mut self, now_ms: u64) -> Option<u64> {
        let current = self.state.current_epoch()?;
        let seen = match self.current_seen {
            Some((id, at)) if id == current.id => at,
            _ => {
                self.current_seen = Some((current.id, now_ms));
                now_ms
            }
        };
        Some(
            if current.since_ms > now_ms.saturating_add(EPOCH_AHEAD_MS) {
                seen.saturating_sub(KEY_LIFETIME_MS)
            } else {
                current.since_ms.min(seen)
            },
        )
    }

    /// The number of the current epoch, when this device is a member whose box of it does
    /// not open: worth telling the managers ([`Self::mend_reported_key`]).
    pub(crate) fn broken_epoch(&self) -> Option<u32> {
        if self.current.is_some() || !self.i_am_member() {
            return None;
        }
        let current = self.state.current_epoch()?;
        self.log
            .current_key_problem(&self.identity, &self.state)
            .filter(|p| p.bad_box)
            .map(|_| fogata_state::epoch_number(&current.id))
    }

    /// Where `device` is on the ladder, 0 for the first; past the end for a device that is
    /// no member.
    pub(crate) fn rank_of(&self, device: &DevicePublicKey) -> usize {
        self.state.rank(device).unwrap_or(usize::MAX)
    }

    /// Whether `device` manages the group.
    pub(crate) fn is_manager_device(&self, device: &DevicePublicKey) -> bool {
        self.state.is_manager(device)
    }

    /// Manager: member `device` says its box of the current epoch, numbered `epoch`, does not
    /// open. The log cannot show it, so the manager starts a new epoch, but only once for
    /// each epoch another device started (this manager's own boxes open), only for a member
    /// the log counts as given that key (one it does not is welcomed instead), at most
    /// once every [`MEND_EVERY_MS`] and [`MENDS_PER_MEMBER`] times a day for each member: a
    /// member cannot make it rotate without end. Returns the new entry.
    pub(crate) fn mend_reported_key(
        &mut self,
        device: &DevicePublicKey,
        epoch: u32,
        now_ms: u64,
    ) -> Vec<EntryHash> {
        let Some(current) = self.state.current_epoch().map(|e| e.id) else {
            return Vec::new();
        };
        let me = self.identity.public_key();
        let started_by_me = self.state.applied.iter().any(|h| {
            self.log.get(h).is_some_and(|e| {
                e.author == me
                    && match &e.body {
                        fogata_state::Body::Create { start: r, .. }
                        | fogata_state::Body::Remove { rotation: r, .. }
                        | fogata_state::Body::Rotate { rotation: r } => r.epoch == current,
                        _ => false,
                    }
            })
        });
        if !self.i_am_manager()
            || self.current.is_none()
            || fogata_state::epoch_number(&current) != epoch
            || !self.state.is_member(device)
            || self.state.lacking_current_secret().contains(device)
            || started_by_me
            || self.mended_for == Some(current)
            || self
                .mended_ms
                .is_some_and(|t| now_ms.saturating_sub(t) < MEND_EVERY_MS)
        {
            return Vec::new();
        }
        self.mended_by
            .retain(|_, (since, _)| now_ms.saturating_sub(*since) < MENDS_FOR_MS);
        let (_, mends) = self.mended_by.entry(*device).or_insert((now_ms, 0));
        if *mends >= MENDS_PER_MEMBER {
            tracing::info!(
                "{} keeps saying its key does not open: not starting another epoch for it today",
                device.fingerprint()
            );
            return Vec::new();
        }
        *mends += 1;
        self.mended_ms = Some(now_ms);
        self.mended_for = Some(current);
        match self.log.rotate(&self.identity, now_ms) {
            Ok(entry) => {
                tracing::info!(
                    "new group key: the current one does not open for {}",
                    device.fingerprint()
                );
                self.refresh();
                vec![entry.hash()]
            }
            Err(e) => {
                tracing::debug!("cannot rotate the group key: {e}");
                Vec::new()
            }
        }
    }

    /// Managers: raises the group to the highest level of judgment this version knows
    /// (`fogata_state::Level::CURRENT`) once every other member's copy was seen judging at
    /// it: `seen` gives the highest capability level each device was seen at (see
    /// [`crate::caps`]), which says the levels it judges at. A copy below it would refuse
    /// the `Level` entry and wait on everything signed after it, so one member never seen at
    /// that level keeps the group where it is. While a manager above this one is connected
    /// (`one_above`), that one raises it. Returns the new entry.
    pub(crate) fn raise_level_when_all_judge(
        &mut self,
        now_ms: u64,
        one_above: bool,
        seen: impl Fn(&DevicePublicKey) -> u32,
    ) -> Vec<EntryHash> {
        let level = fogata_state::Level::CURRENT;
        if one_above || self.joining.is_some() || !self.i_am_manager() || self.log.level() >= level
        {
            return Vec::new();
        }
        let me = self.identity.public_key();
        let all = self
            .state
            .members
            .keys()
            .filter(|device| **device != me)
            .all(|device| crate::caps::judges_at(seen(device), level));
        if !all {
            return Vec::new();
        }
        match self.log.raise_level(&self.identity, level, now_ms) {
            Ok(entry) => {
                tracing::info!(
                    "group raised to level {}: every member's copy judges at it",
                    level.number()
                );
                self.refresh();
                vec![entry.hash()]
            }
            Err(e) => {
                tracing::debug!("cannot raise the group's level: {e}");
                Vec::new()
            }
        }
    }

    /// Manager: its own invites written as level two writes them ([`fogata_state::Body::Invite`],
    /// naming nobody) that do not apply, because the group went up to level three before
    /// them in the common order (this device had not seen the raise when it wrote them).
    /// Level three applies no such invite: nobody would answer under its token, and the
    /// link given out would never work with nobody told why. Each one still open is written
    /// again as level three writes invites ([`fogata_state::NamedInvite`], as
    /// [`GroupLog::invite`] does now), with the same ticket and token, so the link works as
    /// it was given. Only this device's own: only the issuer signs its ticket's note, and
    /// what any copy takes of the log is not changed. Returns the new entries.
    pub(crate) fn name_unnamed_invites(&mut self, now_ms: u64) -> Vec<EntryHash> {
        if self.joining.is_some()
            || !self.i_am_manager()
            || self.log.level() < fogata_state::Level::Three
        {
            return Vec::new();
        }
        let me = self.identity.public_key();
        let applied: HashSet<&EntryHash> = self.state.applied.iter().collect();
        let lost: Vec<(Ticket, fogata_state::InviteToken)> = self
            .log
            .entries()
            .filter(|e| e.author == me)
            .filter_map(|e| match &e.body {
                fogata_state::Body::Invite { ticket, token } if !applied.contains(&e.hash()) => {
                    Some((ticket.clone(), *token))
                }
                _ => None,
            })
            .filter(|(ticket, _)| {
                now_ms <= ticket.expires_ms
                    && !self.state.used_tickets.contains(&ticket.ticket_id)
                    && !self.ticket_revoked(&ticket.ticket_id)
                    && !self.log.invited_with(ticket)
            })
            .take(MAX_INVITES_NAMED_AGAIN)
            .collect();
        if lost.is_empty() {
            return Vec::new();
        }
        // As `GroupLog::invite` names them: the managers above this device.
        let welcomers: Vec<DevicePublicKey> = self
            .state
            .ladder
            .iter()
            .take_while(|k| **k != me)
            .filter(|k| self.state.is_manager(k))
            .copied()
            .collect();
        let mut new = Vec::new();
        for (ticket, token) in lost {
            let body = fogata_state::NamedInvite {
                ticket,
                token,
                welcomers: welcomers.clone(),
            }
            .body();
            match self.log.append(&self.identity, body, now_ms) {
                Ok(entry) => new.push(entry.hash()),
                Err(e) => tracing::debug!("cannot write an invite again: {e}"),
            }
        }
        if !new.is_empty() {
            tracing::info!(
                "{} invites written before the group went up a level written again",
                new.len()
            );
            self.refresh();
        }
        new
    }

    /// Manager whose box of the current epoch is missing or does not open: writes a new epoch
    /// for every member at once, at most once every [`MEND_EVERY_MS`]. Any member could
    /// otherwise lock everyone out with a rotation that gives the others nothing usable,
    /// and a manager may always rotate. While a manager above it is connected
    /// (`one_above`), that one goes first: this one waits [`MEND_EVERY_MS`] for its key.
    /// Returns the new entry.
    pub(crate) fn mend_key(&mut self, now_ms: u64, one_above: bool) -> Vec<EntryHash> {
        if self.current.is_some() {
            self.keyless_since = None;
        }
        if !self.i_am_manager()
            || self.current.is_some()
            || self.state.current_epoch().is_none()
            || self
                .mended_ms
                .is_some_and(|t| now_ms.saturating_sub(t) < MEND_EVERY_MS)
        {
            return Vec::new();
        }
        if one_above {
            let since = *self.keyless_since.get_or_insert(now_ms);
            if now_ms.saturating_sub(since) < MEND_EVERY_MS {
                return Vec::new();
            }
        }
        self.mended_ms = Some(now_ms);
        match self.log.rotate(&self.identity, now_ms) {
            Ok(entry) => {
                tracing::info!("new group key: this manager was not given the current one");
                self.refresh();
                vec![entry.hash()]
            }
            Err(e) => {
                tracing::debug!("cannot rotate the group key: {e}");
                Vec::new()
            }
        }
    }

    /// What a removed device may still be sent: the log up to its removal.
    pub(crate) fn until_removed(&self, device: &DevicePublicKey) -> Vec<Vec<u8>> {
        self.log
            .until_removed(device)
            .into_iter()
            .map(Entry::encode)
            .collect()
    }

    /// This device's removal among `entries`, with what came before it: a member it no
    /// longer verifies (a removed device is not answered with old keys) may still show it
    /// that, signed by a manager, and nothing else. Only those entries are kept.
    #[cfg(any(test, feature = "fuzzing"))]
    pub(crate) fn my_removal(&self, entries: &[Vec<u8>]) -> Option<Vec<Vec<u8>>> {
        Some(self.removal_trial(entries)?.judge()?.0)
    }

    /// Takes `judged` in place of the log: a copy of it with more entries, judged away
    /// from the node's loop (see judging.rs), so that what they bring is taken whole at
    /// once, however much judging them cost. Only when it holds every entry the log holds
    /// now and the same banner and picture: nothing came meanwhile that it lacks. `None`
    /// if not, and nothing changes. Returns the hashes new here.
    pub(crate) fn take_judged(&mut self, judged: GroupLog) -> Option<Vec<EntryHash>> {
        if judged.group_id() != self.group_id()
            || judged.banner_record() != self.log.banner_record()
            || judged.picture_record() != self.log.picture_record()
            || self.log.hashes().any(|h| judged.get(h).is_none())
        {
            return None;
        }
        let new: Vec<EntryHash> = judged
            .hashes()
            .filter(|h| self.log.get(h).is_none())
            .copied()
            .collect();
        let mut judged = judged;
        // What tells the person that the group went up past this version stays.
        judged.keep_ahead_of(&self.log);
        self.log = judged;
        if !new.is_empty() {
            self.refresh();
        }
        Some(new)
    }

    /// What [`Self::my_removal`] checks before it judges: whatever costs little, on the
    /// node's loop. The rest ([`RemovalTrial::judge`]) costs as much as the entries are made
    /// to cost, and goes elsewhere.
    ///
    /// Anyone who knows the group's id may send this, so only entries this device lacks are
    /// read, at most [`MAX_REMOVAL_ENTRIES`] of them, and only if one of them removes it and
    /// is signed by another member (one signature checked, before the log is copied and the
    /// rest are): checking a message's worth of signatures would hold the node up for a
    /// while, and the author of a removal is only a name until its signature is checked.
    pub(crate) fn removal_trial(&self, entries: &[Vec<u8>]) -> Option<RemovalTrial> {
        let me = self.identity.public_key();
        if !self.i_am_member() {
            return None;
        }
        let mut lacking = Vec::new();
        for entry in entries.iter().filter_map(|bytes| Entry::decode(bytes).ok()) {
            if self.log.get(&entry.hash()).is_none() {
                if lacking.len() == MAX_REMOVAL_ENTRIES {
                    return None;
                }
                lacking.push(entry);
            }
        }
        // Only a removal written by a member as this device knows the group can be one
        // (someone moved up above it meanwhile included): checked before any signature.
        let removes_me = |e: &Entry| {
            matches!(&e.body, fogata_state::Body::Remove { member, .. } if *member == me)
                && self.state.is_member(&e.author)
                && e.author != me
        };
        if !lacking
            .iter()
            .filter(|e| removes_me(e))
            .take(MAX_REMOVALS_CHECKED)
            .any(Entry::signature_valid)
        {
            return None;
        }
        let bytes = lacking.iter().map(|e| e.encode().len()).sum();
        Some(RemovalTrial {
            log: self.log.clone(),
            lacking,
            me,
            bytes,
        })
    }

    /// Joining device: a log a manager sent is taken only if it is the group of the
    /// invite, with the issuing manager's own note of it, and, when that note names who lets
    /// this device in, a `Welcome` of one of them ([`AdmittedTrial::judge`]). Someone
    /// else can make up a log under the same id, but not that entry. That is judged on the
    /// whole log, on a copy of this one, away from the node's loop. `None` when this device
    /// is not joining, and the log is taken as any other.
    pub(crate) fn admitted_trial(&self, entries: &[Vec<u8>]) -> Option<AdmittedTrial> {
        let ticket = self.joining()?.clone();
        Some(AdmittedTrial {
            log: self.log.clone(),
            entries: entries
                .iter()
                .filter_map(|bytes| Entry::decode(bytes).ok())
                .collect(),
            ticket,
            me: self.identity.public_key(),
        })
    }

    /// Whether someone who is no longer a member holds the current secret.
    pub(crate) fn secret_leaked(&self) -> bool {
        !self.state.current_secret_leaked_to().is_empty()
    }

    /// Checks a joining device's signed `Join` without applying it: it shows the ticket
    /// offered, still acceptable, comes from a device not in the group, is signed on the
    /// heads this device offered it (`offered`: entries every member keeps, so letting it in
    /// makes nobody keep anything else) and asks under a name the log takes. Returns that
    /// name as it shows.
    pub(crate) fn join_valid(
        &self,
        entry: &Entry,
        ticket_id: &[u8; 16],
        offered: &[EntryHash],
        now_ms: u64,
    ) -> Option<String> {
        let fogata_state::Body::Join { ticket, name } = &entry.body else {
            return None;
        };
        let mut offered = offered.to_vec();
        offered.sort_unstable();
        offered.dedup();
        let valid = &ticket.ticket_id == ticket_id
            && entry.group_id == self.group_id()
            && entry.parents == offered
            && entry.parents.iter().all(|p| self.log.get(p).is_some())
            && entry.signature_valid()
            && !self.state.is_member(&entry.author)
            && fogata_state::fair_name(name, fogata_state::MAX_MEMBER_NAME_CHARS)
            && self.ticket_acceptable(ticket, now_ms);
        // As the log keeps it: one of only blank characters is an empty one.
        valid.then(|| fogata_crypto::names::tidy(name))
    }

    /// Whether `device` joined with ticket `ticket_id` and is a member: shown again, it only
    /// needs the log it missed.
    pub(crate) fn joined_with(&self, device: &DevicePublicKey, ticket_id: &[u8; 16]) -> bool {
        self.state.is_member(device)
            && self
                .log
                .join_ticket_of(device)
                .is_some_and(|t| &t.ticket_id == ticket_id)
    }

    /// Whether a manager cancelled ticket `ticket_id`.
    pub(crate) fn ticket_revoked(&self, ticket_id: &[u8; 16]) -> bool {
        self.log.entries().any(
            |e| matches!(&e.body, fogata_state::Body::Revoke { ticket_id: t } if t == ticket_id),
        )
    }

    /// The `Revoke` that cancelled `ticket`, to show the device refused with it: its
    /// issuer's when the log has one, the only kind that device believes, else any other
    /// (older devices take any refusal).
    pub(crate) fn refusal_of(&self, ticket: &Ticket) -> Option<Vec<u8>> {
        if let Some(entry) = self.log.issuer_revoke(ticket) {
            return Some(entry.encode());
        }
        self.log
            .entries()
            .find(|e| {
                matches!(&e.body, fogata_state::Body::Revoke { ticket_id } if *ticket_id == ticket.ticket_id)
            })
            .map(Entry::encode)
    }

    /// Joining device: whether `revoke` is the issuer of its invite cancelling it, the
    /// only refusal it can check.
    pub(crate) fn refused_by_issuer(&self, revoke: &[u8]) -> bool {
        let Some(ticket) = self.joining() else {
            return false;
        };
        Entry::decode(revoke).is_ok_and(|entry| ticket.revoked_by(&entry))
    }

    /// Managers: cancels, in this device's own name, its invites another manager cancelled,
    /// so the invited device gets a refusal it can check, and its open invites a manager
    /// removed or demoted since still knows ([`GroupLog::invites_a_manager_left_with`]):
    /// that device could let the invited one into a copy of the group as it was before.
    /// Returns the new entries.
    pub(crate) fn confirm_revokes(&mut self, now_ms: u64) -> Vec<EntryHash> {
        if !self.i_am_manager() {
            return Vec::new();
        }
        let me = self.identity.public_key();
        let mut tickets = self.log.revokes_to_confirm(&me);
        let known = self.log.invites_a_manager_left_with(&me, now_ms);
        if !known.is_empty() {
            tracing::info!(
                "cancelling {} open invites: a manager removed or demoted since knows them",
                known.len()
            );
        }
        tickets.extend(known);
        tickets.sort_unstable();
        tickets.dedup();
        let mut new = Vec::new();
        for ticket_id in tickets {
            new.extend(self.revoke(ticket_id, now_ms));
        }
        new
    }

    /// Cancels an invite so nobody joins with it any more (managers). Returns the new entry.
    pub(crate) fn revoke(&mut self, ticket_id: [u8; 16], now_ms: u64) -> Vec<EntryHash> {
        match self.log.append(
            &self.identity,
            fogata_state::Body::Revoke { ticket_id },
            now_ms,
        ) {
            Ok(entry) => {
                self.refresh();
                vec![entry.hash()]
            }
            Err(e) => {
                tracing::debug!("cannot cancel the invite: {e}");
                Vec::new()
            }
        }
    }

    /// The signed `Join` for this device, on top of `heads`, dated `now_ms` or, past the
    /// ticket's expiry by this device's clock, at the expiry: the manager letting it in
    /// checks the ticket against its own clock, and versions from before that refused a
    /// join dated after its ticket expired, and waited for ever on what is signed on it.
    pub(crate) fn sign_join(&self, heads: Vec<EntryHash>, now_ms: u64) -> Option<Entry> {
        let (ticket, _, name) = self.joining.as_ref()?;
        Some(Entry::sign(
            &self.identity,
            self.group_id(),
            heads,
            now_ms.min(ticket.expires_ms),
            fogata_state::Body::Join {
                ticket: ticket.clone(),
                name: fogata_crypto::names::tidy(name),
            },
        ))
    }
}

/// A removal shown to this device, checked as far as costs little
/// ([`GroupContext::removal_trial`]), with a copy of the log to judge it on.
pub(crate) struct RemovalTrial {
    log: GroupLog,
    lacking: Vec<Entry>,
    me: DevicePublicKey,
    bytes: usize,
}

impl RemovalTrial {
    /// Bytes of the entries shown.
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }

    /// The entries to take: the removal and those before it this device lacks, if they
    /// remove it, with the log as it is with them (a copy of it as it was, judged whole; see
    /// [`GroupContext::take_judged`]). Judging them all costs as much as they were made to
    /// cost (seconds, for entries signed on old parents), so this runs away from the node's
    /// loop.
    pub(crate) fn judge(self) -> Option<(Vec<Vec<u8>>, GroupLog)> {
        let Self {
            log: before,
            lacking,
            me,
            ..
        } = self;
        let lacking_here: HashSet<EntryHash> = lacking.iter().map(Entry::hash).collect();
        let count = lacking.len();
        let mut log = before.clone();
        log.insert_all(lacking);
        if log.state().is_member(&me) {
            return None;
        }
        let shown: Vec<Entry> = log
            .until_removed(&me)
            .into_iter()
            .filter(|e| lacking_here.contains(&e.hash()))
            .cloned()
            .collect();
        if shown.is_empty() {
            return None;
        }
        // Only what came up to the removal is kept: anything else sent along is not.
        if shown.len() < count {
            log = before;
            log.insert_all(shown.iter().cloned());
            drop(log.state());
        }
        Some((shown.iter().map(Entry::encode).collect(), log))
    }
}

/// The log a manager sent a joining device, with a copy of the device's own and the
/// invite it joins with ([`GroupContext::admitted_trial`]).
pub(crate) struct AdmittedTrial {
    log: GroupLog,
    entries: Vec<Entry>,
    ticket: Ticket,
    /// The joining device.
    me: DevicePublicKey,
}

impl AdmittedTrial {
    /// The log with the entries sent, if it is the group of the invite, with the issuing
    /// manager's own note of it, and this device let in, if it is, by someone that note
    /// allows ([`GroupLog::admits_as_invited`]). Judged whole, so away from the node's loop.
    pub(crate) fn judge(self) -> Option<GroupLog> {
        let Self {
            mut log,
            entries,
            ticket,
            me,
        } = self;
        log.insert_all(entries);
        log.admits_as_invited(&ticket, &me).then_some(log)
    }
}

#[cfg(test)]
mod tests {
    use fogata_state::{Entry, Ticket};

    use super::*;

    fn ctx(identity: &Arc<DeviceIdentity>, log: &GroupLog) -> GroupContext {
        GroupContext::new(identity.clone(), GroupSetup::Member(log.clone()))
    }

    /// The banner's and picture's digests, kept between asks, are those of the records
    /// there now, after each change too.
    #[test]
    fn the_kept_digests_follow_the_banner_and_picture() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1).unwrap();
        let mut ctx = ctx(&ana, &log);
        assert!(ctx.banner_digest().is_empty());
        assert!(ctx.picture_digest().is_empty());
        let mut seen = Vec::new();
        for now in [10, 20, 30] {
            ctx.set_banner(Vec::new(), now).unwrap();
            ctx.set_picture(Vec::new(), now).unwrap();
            for _ in 0..2 {
                let banner = ctx.log.banner_record().unwrap().digest().to_vec();
                let picture = ctx.log.picture_record().unwrap().digest().to_vec();
                assert_eq!(ctx.banner_digest(), banner);
                assert_eq!(ctx.picture_digest(), picture);
            }
            seen.push(ctx.banner_digest());
        }
        seen.dedup();
        assert_eq!(seen.len(), 3);
    }

    fn join(log: &mut GroupLog, joiner: &DeviceIdentity, ticket: Ticket, now: u64) -> EntryHash {
        let entry = Entry::sign(
            joiner,
            log.group_id(),
            log.heads(),
            now,
            fogata_state::Body::Join {
                ticket,
                name: "zed".into(),
            },
        );
        let hash = entry.hash();
        log.insert(entry).unwrap();
        hash
    }

    #[test]
    fn only_managers_accept_joins() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1_000).unwrap();
        let first = fogata_state::Body::SetManagers { first: Some(1) };
        log.append(&ana, first, 1_000).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2_000).unwrap();
        let ticket = log.issue_ticket(&ana, 5_000).unwrap();
        assert!(ctx(&ana, &log).ticket_acceptable(&ticket, 3_000));
        assert!(!ctx(&luis, &log).ticket_acceptable(&ticket, 3_000));
        assert!(!ctx(&ana, &log).ticket_acceptable(&ticket, 6_000));
    }

    #[test]
    fn a_join_says_and_shows_its_name_as_it_shows() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1_000).unwrap();
        let ticket = log.issue_ticket(&ana, 5_000).unwrap();
        let joining = |name: &str| {
            GroupContext::new(
                zed.clone(),
                GroupSetup::Joining {
                    group_id: log.group_id(),
                    ticket: ticket.clone(),
                    rendezvous: [0; 32],
                    name: name.into(),
                },
            )
        };
        let admin = ctx(&ana, &log);
        let id = ticket.ticket_id;
        let heads = log.heads();
        let entry = joining("\u{3164}Zed\u{2800}\u{2800}")
            .sign_join(log.heads(), 2_000)
            .unwrap();
        assert!(matches!(&entry.body, fogata_state::Body::Join { name, .. } if name == "Zed"));
        assert_eq!(
            admin.join_valid(&entry, &id, &heads, 2_000).as_deref(),
            Some("Zed")
        );
        // Signed elsewhere: padding is not shown, a blank name is an empty one, and a name
        // the log refuses asks nobody.
        let raw = |name: &str| {
            Entry::sign(
                &zed,
                log.group_id(),
                log.heads(),
                2_000,
                fogata_state::Body::Join {
                    ticket: ticket.clone(),
                    name: name.into(),
                },
            )
        };
        let padded = raw("Ana\u{3164}\u{3164}");
        assert_eq!(
            admin.join_valid(&padded, &id, &heads, 2_000).as_deref(),
            Some("Ana")
        );
        let blank = raw("\u{3164}\u{FFA0}");
        assert_eq!(
            admin.join_valid(&blank, &id, &heads, 2_000).as_deref(),
            Some("")
        );
        let turned = raw("A\u{202E}na");
        assert_eq!(admin.join_valid(&turned, &id, &heads, 2_000), None);
        // Signed on an entry nobody has, it would only wait in the log for ever.
        let mut parents = log.heads();
        parents.push([0xEE; 32]);
        parents.sort();
        let dangling = Entry::sign(
            &zed,
            log.group_id(),
            parents,
            2_000,
            fogata_state::Body::Join {
                ticket: ticket.clone(),
                name: "Zed".into(),
            },
        );
        assert_eq!(admin.join_valid(&dangling, &id, &heads, 2_000), None);
        // Signed on other entries than the heads offered: letting it in would make every
        // member keep whatever they are.
        assert_eq!(admin.join_valid(&entry, &id, &[], 2_000), None);
    }

    #[test]
    fn a_made_up_ticket_is_told_before_anything_else() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1_000).unwrap();
        let ticket = log.issue_ticket(&ana, 5_000).unwrap();
        let admin = ctx(&ana, &log);
        assert!(admin.ticket_genuine(&ticket));
        assert!(!admin.ticket_used(&ticket.ticket_id));
        let mut longer = ticket.clone();
        longer.expires_ms += 1_000_000;
        assert!(!admin.ticket_genuine(&longer));
        let other = GroupLog::create(&ana, "h", 1_000).unwrap();
        assert!(!admin.ticket_genuine(&other.issue_ticket(&ana, 5_000).unwrap()));
        join(&mut log, &zed, ticket.clone(), 2_000);
        // Used once a manager lets the device in, not by the join alone.
        assert!(!ctx(&ana, &log).ticket_used(&ticket.ticket_id));
        log.welcome(&ana, zed.public_key(), 2_001).unwrap();
        assert!(ctx(&ana, &log).ticket_used(&ticket.ticket_id));
    }

    #[test]
    fn a_first_key_is_not_given_once_the_ticket_expired() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1_000).unwrap();
        // A join dated before the ticket's expiry, relayed by someone, that reaches the
        // manager once the ticket has expired.
        let ticket = log.issue_ticket(&ana, 5_000).unwrap();
        let joined = join(&mut log, &zed, ticket, 4_000);
        let mut admin = ctx(&ana, &log);
        admin.accepted.insert(zed.public_key(), joined);
        assert!(admin.welcome_lacking(9_000).is_empty());
        assert!(admin.log().current_keys(&zed).is_none());
        // While the ticket is valid, the manager welcomes.
        assert_eq!(admin.welcome_lacking(4_500).len(), 1);
        assert!(admin.log().current_keys(&zed).is_some());
    }

    #[test]
    fn a_join_a_member_wrote_lets_nobody_in() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let dave = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        // An open invite, which every member sees in the log, used by a device of Luis's
        // that no manager was asked about.
        let (ticket, _) = log.invite(&ana, 100, 3).unwrap();
        let joined = join(&mut log, &dave, ticket, 50);
        let mut admin = ctx(&ana, &log);
        assert!(admin.welcome_lacking(60).is_empty());
        assert!(!admin.is_member_device(&dave.public_key()));
        assert!(admin.is_member_device(&luis.public_key()));
        // Let in by this manager, it gets its key and counts.
        admin.accepted.insert(dave.public_key(), joined);
        assert_eq!(admin.welcome_lacking(60).len(), 1);
        assert!(admin.is_member_device(&dave.public_key()));
    }

    /// A device holds two invites and signs a join with each, the second after the first; the
    /// manager says yes to the second. That one is what lets it in: its name, its ticket.
    /// And a join signed on entries nobody keeps (a member's spare ones) is not let in.
    #[test]
    fn a_manager_lets_in_the_join_it_said_yes_to() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bob = Arc::new(DeviceIdentity::generate().unwrap());
        let dee = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1_000).unwrap();
        log.add(&ana, bob.public_key(), "bob", 1_001).unwrap();
        let (by_bob, _) = log.invite(&bob, 10_000_000, 2_000).unwrap();
        let (by_ana, _) = log.invite(&ana, 10_000_000, 2_001).unwrap();
        let group = log.group_id();
        let sign = |parents: Vec<EntryHash>, ticket: &Ticket, name: &str| {
            Entry::sign(
                &dee,
                group,
                parents,
                3_000,
                fogata_state::Body::Join {
                    ticket: ticket.clone(),
                    name: name.into(),
                },
            )
        };
        let first = sign(log.heads(), &by_bob, "with bob's");
        log.insert(first.clone()).unwrap();
        // Bob signs an entry on that join, so that it is before the second one.
        let mut parents = log.heads();
        parents.push(first.hash());
        let rename = Entry::sign(
            &bob,
            group,
            parents,
            3_000,
            fogata_state::Body::Rename { name: "g2".into() },
        );
        log.insert(rename).unwrap();
        let second = sign(log.heads(), &by_ana, "with ana's");
        let mut admin = ctx(&ana, &log);
        assert!(admin.accept_join(second, &by_ana.ticket_id, 3_100).is_ok());
        let state = admin.log().state();
        assert_eq!(state.members[&dee.public_key()].name, "with ana's");
        assert!(state.used_tickets.contains(&by_ana.ticket_id));
        assert!(!state.used_tickets.contains(&by_bob.ticket_id));

        // Bob writes entries of a kind nobody knows (spare: nobody keeps them for long);
        // a device signs its join on them.
        let carl = DeviceIdentity::generate().unwrap();
        let mut log = admin.log().clone();
        let (ticket, _) = log.invite(&ana, 10_000_000, 4_000).unwrap();
        let junk = Entry::sign(
            &bob,
            log.group_id(),
            log.heads(),
            4_001,
            fogata_state::Body::Unknown {
                kind: 200,
                bytes: vec![7; 60_000],
            },
        );
        log.insert(junk.clone()).unwrap();
        let mut parents = log.heads();
        parents.push(junk.hash());
        let on_junk = Entry::sign(
            &carl,
            log.group_id(),
            parents,
            4_002,
            fogata_state::Body::Join {
                ticket: ticket.clone(),
                name: "carl".into(),
            },
        );
        let mut admin = ctx(&ana, &log);
        assert_eq!(
            admin.join_valid(&on_junk, &ticket.ticket_id, &log.heads(), 4_003),
            None
        );
        assert_eq!(
            admin.accept_join(on_junk, &ticket.ticket_id, 4_003),
            Err(NotLetIn::SignedOnSpare)
        );
        assert!(!admin.log().state().is_member(&carl.public_key()));
        assert!(admin.log().spare().contains(&junk.hash()));
    }

    #[test]
    fn a_log_larger_than_a_message_syncs_in_parts() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        let behind = ctx(&luis, &log);
        for i in 0..90u64 {
            let picture = vec![i as u8; fogata_state::MAX_PICTURE_BYTES];
            log.append(
                &ana,
                fogata_state::Body::SetPicture {
                    picture: Some(picture),
                },
                3 + i,
            )
            .unwrap();
            // Luis signs on them now and then: one device alone writes at most 4 MiB.
            if i % 40 == 39 {
                let name = format!("g{i}");
                log.append(&luis, fogata_state::Body::Rename { name }, 3 + i)
                    .unwrap();
            }
        }
        let at_ana = ctx(&ana, &log);
        let mut at_luis = behind;
        let mut rounds = 0;
        while at_luis.log().len() < log.len() {
            let (entries, _) = at_ana.diff(&at_luis.hashes());
            assert!(entries.iter().map(Vec::len).sum::<usize>() <= SYNC_BYTES);
            assert!(!at_luis.insert_encoded(&entries).new.is_empty());
            rounds += 1;
        }
        let whole: usize = at_ana.encoded(at_ana.hashes()).iter().map(Vec::len).sum();
        assert!(rounds >= whole / SYNC_BYTES && rounds <= whole / SYNC_BYTES + 2);
        // A device let in gets the same log in parts of the same size, in order.
        let parts = at_ana.encoded_parts();
        assert!(parts.len() > 1);
        assert!(
            parts
                .iter()
                .all(|p| p.iter().map(Vec::len).sum::<usize>() <= SYNC_BYTES)
        );
        assert_eq!(parts.concat(), at_ana.encoded(at_ana.hashes()));
    }

    /// A manager removed or demoted keeps the log with the invites open then, and could
    /// let their device into a copy of the group where it still manages: the issuer
    /// cancels them, once. Invites made after cancel nothing, and nor does a manager that
    /// leaves by itself where everyone manages.
    #[test]
    fn invites_a_removed_manager_knows_are_cancelled_by_their_issuer() {
        let id = || Arc::new(DeviceIdentity::generate().unwrap());
        let (ana, bea, leo, zoe) = (id(), id(), id(), id());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        log.add(&ana, leo.public_key(), "leo", 3).unwrap();
        log.add(&ana, zoe.public_key(), "zoe", 4).unwrap();
        let (before, _) = log.invite(&ana, 1_000, 4).unwrap();
        log.append(&zoe, fogata_state::Body::Leave, 5).unwrap();
        assert!(ctx(&ana, &log).confirm_revokes(6).is_empty());
        log.remove(&ana, bea.public_key(), 7).unwrap();
        let (after, _) = log.invite(&ana, 1_000, 7).unwrap();
        // Only its issuer's refusal counts for the invited device.
        assert!(ctx(&leo, &log).confirm_revokes(8).is_empty());
        let mut at_ana = ctx(&ana, &log);
        assert_eq!(at_ana.confirm_revokes(8).len(), 1);
        assert!(at_ana.log().issuer_revoke(&before).is_some());
        assert!(at_ana.log().issuer_revoke(&after).is_none());
        assert!(at_ana.confirm_revokes(9).is_empty());
    }

    /// A manager that leaves by itself keeps the log too: it cancels the invites it could
    /// answer, those that name it from level three, and those of a group where a few
    /// manage. Not the others, nor those made after it left.
    #[test]
    fn invites_a_manager_that_left_could_answer_are_cancelled_by_their_issuer() {
        let id = || Arc::new(DeviceIdentity::generate().unwrap());
        let (ana, leo, bea) = (id(), id(), id());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, leo.public_key(), "leo", 2).unwrap();
        log.add(&ana, bea.public_key(), "bea", 3).unwrap();
        let mut few = log.clone();
        log.raise_level(&ana, fogata_state::Level::Three, 4)
            .unwrap();
        // Bea's invite names Ana and Leo, above her; Ana's, first, names nobody.
        let (by_bea, _) = log.invite(&bea, 1_000, 5).unwrap();
        let (by_ana, _) = log.invite(&ana, 1_000, 5).unwrap();
        log.append(&leo, fogata_state::Body::Leave, 6).unwrap();
        let (after, _) = log.invite(&bea, 1_000, 7).unwrap();
        assert!(ctx(&ana, &log).confirm_revokes(8).is_empty());
        let mut at_bea = ctx(&bea, &log);
        assert_eq!(at_bea.confirm_revokes(8).len(), 1);
        assert!(at_bea.log().issuer_revoke(&by_bea).is_some());
        assert!(at_bea.log().issuer_revoke(&after).is_none());
        assert!(at_bea.log().issuer_revoke(&by_ana).is_none());

        // At level two, invites name nobody: where Ana and Leo alone manage, Leo could
        // answer any.
        let first_two = fogata_state::Body::SetManagers { first: Some(2) };
        few.append(&ana, first_two, 4).unwrap();
        let (by_ana, _) = few.invite(&ana, 1_000, 5).unwrap();
        few.append(&bea, fogata_state::Body::Leave, 6).unwrap();
        assert!(ctx(&ana, &few).confirm_revokes(7).is_empty());
        few.append(&leo, fogata_state::Body::Leave, 7).unwrap();
        let mut at_ana = ctx(&ana, &few);
        assert_eq!(at_ana.confirm_revokes(8).len(), 1);
        assert!(at_ana.log().issuer_revoke(&by_ana).is_some());
    }

    /// A member that rotates after someone left may seal the new key to itself alone: the
    /// log takes it, as every version does. A manager holding that key gives it to every
    /// member left out; one left out itself starts a new epoch for everyone.
    #[test]
    fn members_left_out_of_a_members_rotation_get_the_key_from_a_manager() {
        let id = || Arc::new(DeviceIdentity::generate().unwrap());
        let (ana, bea, leo, zoe) = (id(), id(), id(), id());
        let mut base = GroupLog::create(&ana, "g", 1).unwrap();
        base.append(&ana, fogata_state::Body::SetManagers { first: Some(1) }, 2)
            .unwrap();
        base.add(&ana, bea.public_key(), "bea", 3).unwrap();
        base.add(&ana, leo.public_key(), "leo", 4).unwrap();
        base.add(&ana, zoe.public_key(), "zoe", 5).unwrap();
        base.append(&zoe, fogata_state::Body::Leave, 6).unwrap();
        for with_ana in [true, false] {
            let mut log = base.clone();
            let to: Vec<_> = if with_ana {
                vec![ana.public_key(), bea.public_key()]
            } else {
                vec![bea.public_key()]
            };
            let (rotation, _) = fogata_state::Rotation::new(&log.group_id(), to).unwrap();
            log.append(&bea, fogata_state::Body::Rotate { rotation }, 7)
                .unwrap();
            assert!(log.current_keys(&leo).is_none());
            let mut at_ana = ctx(&ana, &log);
            let new = if with_ana {
                at_ana.welcome_lacking(8)
            } else {
                at_ana.mend_key(8, false)
            };
            assert_eq!(new.len(), 1, "{with_ana}");
            let log = at_ana.log();
            assert!(log.state().lacking_current_secret().is_empty());
            for member in [&ana, &bea, &leo] {
                assert!(log.current_keys(member).is_some(), "{with_ana}");
            }
        }
    }

    #[test]
    fn the_first_holder_rotates_at_once_and_the_others_only_insisting() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        log.add(&ana, bea.public_key(), "bea", 3).unwrap();
        assert!(ctx(&ana, &log).rotate_if_leaked(4, false).is_empty());
        log.append(&bea, fogata_state::Body::Leave, 4).unwrap();
        let mut at_luis = ctx(&luis, &log);
        assert!(at_luis.secret_leaked());
        assert!(at_luis.rotate_if_leaked(5, false).is_empty());
        assert_eq!(at_luis.rotate_if_leaked(5, true).len(), 1);
        assert!(!at_luis.secret_leaked());
        let mut at_ana = ctx(&ana, &log);
        assert_eq!(at_ana.rotate_if_leaked(5, false).len(), 1);
        assert!(at_ana.log().current_keys(&bea).is_none());
        assert!(at_ana.rotate_if_leaked(6, true).is_empty());
    }

    #[test]
    fn only_managers_rotate_for_a_key_sent_in_an_entry_that_did_not_apply() {
        let id = || Arc::new(DeviceIdentity::generate().unwrap());
        let (ana, bea, carlos, dani, zoe) = (id(), id(), id(), id(), id());
        let mut base = GroupLog::create(&ana, "g", 1).unwrap();
        base.add(&ana, bea.public_key(), "bea", 2).unwrap();
        base.add(&ana, carlos.public_key(), "carlos", 3).unwrap();
        base.add(&ana, dani.public_key(), "dani", 4).unwrap();
        let first_two = fogata_state::Body::SetManagers { first: Some(2) };
        base.append(&ana, first_two, 5).unwrap();
        // Ana moves Bea to the end while Bea, not knowing, adds Zoe with the current key:
        // the add does not apply, and Zoe, no member, was sent the key.
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        let demote = fogata_state::Body::Demote {
            member: bea.public_key(),
        };
        at_ana.append(&ana, demote, 6).unwrap();
        at_bea.add(&bea, zoe.public_key(), "zoe", 6).unwrap();
        for e in at_bea.entries() {
            let _ = at_ana.insert(e.clone());
        }
        let state = at_ana.state();
        assert_eq!(state.current_secret_leaked_to(), vec![zoe.public_key()]);
        assert!(state.current_secret_left_with().is_empty());
        // Dani and Bea, who do not manage, leave it to Carlos and Ana, who do.
        for member in [&dani, &bea] {
            let mut at = ctx(member, &at_ana);
            assert!(at.secret_leaked());
            assert!(at.rotate_if_leaked(7, true).is_empty());
        }
        let mut at_carlos = ctx(&carlos, &at_ana);
        assert!(at_carlos.rotate_if_leaked(7, false).is_empty());
        assert_eq!(at_carlos.rotate_if_leaked(7, true).len(), 1);
        assert!(!at_carlos.secret_leaked());
        let mut at_first = ctx(&ana, &at_ana);
        assert_eq!(at_first.rotate_if_leaked(7, false).len(), 1);
        assert!(!at_first.secret_leaked());
    }

    #[test]
    fn a_join_is_never_dated_past_its_ticket() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = Arc::new(DeviceIdentity::generate().unwrap());
        let log = GroupLog::create(&ana, "g", 1_000).unwrap();
        let ticket = log.issue_ticket(&ana, 5_000).unwrap();
        let joining = GroupContext::new(
            zed,
            GroupSetup::Joining {
                group_id: log.group_id(),
                ticket,
                rendezvous: [0; 32],
                name: "Zed".into(),
            },
        );
        assert_eq!(
            joining.sign_join(log.heads(), 2_000).unwrap().time_ms,
            2_000
        );
        // This device's clock is past the ticket's expiry; the manager's may not be.
        let late = joining.sign_join(log.heads(), 9_000).unwrap();
        assert_eq!(late.time_ms, 5_000);
        let fogata_state::Body::Join { ticket, .. } = &late.body else {
            unreachable!("a join")
        };
        let admin = ctx(&ana, &log);
        let valid = admin.join_valid(&late, &ticket.ticket_id, &log.heads(), 4_000);
        assert!(valid.is_some());
    }

    /// An invite a manager wrote naming nobody, before it saw the group go up to level
    /// three, does not apply there; the manager writes it again as level three writes
    /// invites, with the same ticket and token, so the link given out works: the device it
    /// was given to finds the issuer's note of it, and is let in. Written once, and never
    /// for an invite used, cancelled or run out.
    #[test]
    fn an_invite_written_before_the_raise_is_written_again_and_works() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = Arc::new(DeviceIdentity::generate().unwrap());
        let mut base = GroupLog::create(&ana, "g", 1).unwrap();
        base.add(&ana, bea.public_key(), "bea", 2).unwrap();
        // Bea raises the group while Ana, not knowing, invites Zed and cancels another
        // invite. Ana came in first, so what she did at once goes after the raise.
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        at_bea
            .raise_level(&bea, fogata_state::Level::Three, 3)
            .unwrap();
        let (ticket, token) = at_ana.invite(&ana, 50_000, 3).unwrap();
        let (cancelled, _) = at_ana.invite(&ana, 50_000, 4).unwrap();
        let revoke = fogata_state::Body::Revoke {
            ticket_id: cancelled.ticket_id,
        };
        at_ana.append(&ana, revoke, 5).unwrap();
        for e in at_bea.entries() {
            let _ = at_ana.insert(e.clone());
        }
        assert_eq!(at_ana.level(), fogata_state::Level::Three);
        assert!(!at_ana.invited_with(&ticket), "the link does not work");
        let mut ana_ctx = ctx(&ana, &at_ana);
        // Not after it ran out; once while it is open.
        assert!(ana_ctx.name_unnamed_invites(60_000).is_empty());
        assert_eq!(ana_ctx.name_unnamed_invites(6).len(), 1);
        assert!(ana_ctx.name_unnamed_invites(7).is_empty());
        let log = ana_ctx.log().clone();
        assert!(log.invited_with(&ticket));
        assert!(!log.invited_with(&cancelled));
        assert!(log.state().open_invite_tokens(8).contains(&token));
        // Every copy takes it as written, and the device invited is let in with it.
        for e in log.entries() {
            let _ = at_bea.insert(e.clone());
        }
        assert!(at_bea.invited_with(&ticket));
        let joining = GroupContext::new(
            zed.clone(),
            GroupSetup::Joining {
                group_id: log.group_id(),
                ticket: ticket.clone(),
                rendezvous: [0; 32],
                name: "Zed".into(),
            },
        );
        let join = joining.sign_join(log.heads(), 9).unwrap();
        let mut at_ana = log;
        at_ana.insert(join.clone()).unwrap();
        at_ana.let_in(&ana, &join.hash(), 10).unwrap();
        assert!(at_ana.state().is_member(&zed.public_key()));
        assert!(at_ana.admits_as_invited(&ticket, &zed.public_key()));
        // Another manager writes none for it: only the issuer's note counts.
        let mut bea_ctx = ctx(&bea, &at_bea);
        assert!(bea_ctx.name_unnamed_invites(8).is_empty());
    }

    /// A member's copy sent a raise past the levels this version judges at says so once,
    /// keeps saying so (in what the application is shown of the group) and keeps it when a
    /// copy judged elsewhere or another copy of the log is taken in.
    #[test]
    fn a_group_gone_past_this_version_is_told_and_stays_told() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        let next = fogata_state::Level::CURRENT.number() + 1;
        let raise = Entry::sign(
            &ana,
            log.group_id(),
            log.heads(),
            3,
            fogata_state::Body::Level { level: next },
        );
        let mut at_luis = ctx(&luis, &log);
        let taken = at_luis.insert_encoded(&[raise.encode()]);
        assert!(taken.ahead && taken.new.is_empty() && taken.refused.is_empty());
        assert!(!at_luis.insert_encoded(&[raise.encode()]).ahead);
        assert_eq!(
            crate::GroupSummary::of(at_luis.log()).level_ahead,
            Some(next)
        );
        // A copy judged away from the loop, with something new, takes its place: still told.
        let mut judged = log.clone();
        judged
            .append(&ana, fogata_state::Body::SetIcon { icon: 2 }, 4)
            .unwrap();
        assert_eq!(at_luis.take_judged(judged).map(|n| n.len()), Some(1));
        assert_eq!(at_luis.log().level_ahead(), Some(next));
        let mut other = ctx(&luis, &log);
        other.merge(at_luis.log());
        assert_eq!(other.log().level_ahead(), Some(next));
        assert_eq!(crate::GroupSummary::of(&log).level_ahead, None);
    }

    /// A manager raises the group to this version's level of judgment once every other
    /// member was seen at it, never before, and only once; one with a manager above it
    /// connected leaves it to that one.
    #[test]
    fn a_group_goes_up_a_level_once_every_member_judges_at_it() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        let level = fogata_state::Level::CURRENT;
        let mut at_ana = ctx(&ana, &log);
        let luis_key = luis.public_key();
        let seen_at = |n: u32| move |d: &DevicePublicKey| if *d == luis_key { n } else { 0 };
        assert!(
            at_ana
                .raise_level_when_all_judge(3, false, seen_at(level.number() - 1))
                .is_empty()
        );
        assert!(
            at_ana
                .raise_level_when_all_judge(3, true, seen_at(level.number()))
                .is_empty()
        );
        assert_eq!(
            at_ana
                .raise_level_when_all_judge(4, false, seen_at(level.number()))
                .len(),
            1
        );
        assert_eq!(at_ana.log().level(), level);
        assert!(
            at_ana
                .raise_level_when_all_judge(5, false, seen_at(level.number()))
                .is_empty()
        );
        // Luis does not manage once the managers are the first one.
        log.append(&ana, fogata_state::Body::SetManagers { first: Some(1) }, 3)
            .unwrap();
        assert!(
            ctx(&luis, &log)
                .raise_level_when_all_judge(4, false, |_| level.number())
                .is_empty()
        );
    }

    /// From level three, a manager answers only under the tokens of the invites it may let
    /// a device in with, and takes only the joins of those; the device invited takes a log
    /// only where the issuer or a manager its invite names let it in.
    #[test]
    fn only_a_manager_an_invite_names_answers_it_and_is_believed() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let before_three = log.clone();
        log.raise_level(&ana, fogata_state::Level::Three, 3)
            .unwrap();
        // Ana is first: her invite names nobody else.
        let (ticket, token) = log.invite(&ana, 5_000, 4).unwrap();
        let secret = fogata_state::invite_rendezvous(&token);
        assert!(ctx(&ana, &log).rendezvous_secrets(10).contains(&secret));
        assert!(!ctx(&bea, &log).rendezvous_secrets(10).contains(&secret));
        assert!(ctx(&ana, &log).ticket_acceptable(&ticket, 10));
        assert!(!ctx(&bea, &log).ticket_acceptable(&ticket, 10));

        let joining = GroupContext::new(
            zed.clone(),
            GroupSetup::Joining {
                group_id: log.group_id(),
                ticket: ticket.clone(),
                rendezvous: secret,
                name: "Zed".into(),
            },
        );
        // Bea, in a copy of the group as it was before level three, lets Zed in there.
        let mut fork = before_three;
        let join = joining.sign_join(fork.heads(), 5).unwrap();
        fork.insert(join.clone()).unwrap();
        let welcome = fork.let_in(&bea, &join.hash(), 6).unwrap();
        let shown: Vec<Vec<u8>> = log
            .entries()
            .chain([&join, &welcome])
            .map(Entry::encode)
            .collect();
        let trial = joining.admitted_trial(&shown).unwrap();
        assert!(trial.judge().is_none());
        // Or, leaving out his `Join`, adds him and seals him her copy's key: as refused.
        let mut fork = log.clone();
        fork.add(&bea, zed.public_key(), "Zed", 6).unwrap();
        let shown: Vec<Vec<u8>> = fork.entries().map(Entry::encode).collect();
        let trial = joining.admitted_trial(&shown).unwrap();
        assert!(trial.judge().is_none());
        // Ana lets Zed in: taken.
        let mut real = log.clone();
        let join = joining.sign_join(real.heads(), 5).unwrap();
        real.insert(join.clone()).unwrap();
        real.let_in(&ana, &join.hash(), 6).unwrap();
        let shown: Vec<Vec<u8>> = real.entries().map(Entry::encode).collect();
        let taken = joining.admitted_trial(&shown).unwrap().judge().unwrap();
        assert!(taken.state().is_member(&zed.public_key()));
    }

    #[test]
    fn managers_announce_under_open_invites() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let first = fogata_state::Body::SetManagers { first: Some(1) };
        log.append(&ana, first, 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        let plain = ctx(&ana, &log).rendezvous_secrets(3).len();
        let (_, token) = log.invite(&ana, 100, 3).unwrap();
        let secret = fogata_state::invite_rendezvous(&token);
        assert!(ctx(&ana, &log).rendezvous_secrets(50).contains(&secret));
        assert_eq!(ctx(&ana, &log).rendezvous_secrets(50).len(), plain + 1);
        // Not after it expires, and never for members who are not managers.
        assert!(!ctx(&ana, &log).rendezvous_secrets(101).contains(&secret));
        assert!(!ctx(&luis, &log).rendezvous_secrets(50).contains(&secret));
    }

    #[test]
    fn members_look_for_others_a_few_rotations_behind() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        // Luis went away here.
        let at_luis = ctx(&luis, &log);
        let his = at_luis.rendezvous_secrets(10);
        for i in 0..3 {
            log.rotate(&ana, 10 + i).unwrap();
        }
        let at_ana = ctx(&ana, &log);
        let announced = at_ana.rendezvous_secrets(20);
        let sought = at_ana.lookup_secrets(20);
        assert_eq!(announced.len(), 2);
        assert_eq!(sought.len(), 4);
        // Three rotations behind, Luis announces under a key Ana looks up but no longer
        // announces under.
        assert!(his.iter().all(|s| !announced.contains(s)));
        assert!(his.iter().any(|s| sought.contains(s)));
        // Not for ever after: two weeks after the next key began.
        let later = 20 + OLDER_EPOCHS_FOR_MS;
        assert_eq!(at_ana.lookup_secrets(later).len(), 2);
    }

    #[test]
    fn older_keys_are_not_looked_up_for_ever_after_one_dated_far_ahead() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.rotate(&ana, 10).unwrap();
        log.rotate(&ana, u64::MAX - 1).unwrap();
        log.rotate(&ana, 12).unwrap();
        // The epoch that replaced the first says it began near the end of all time.
        let at_ana = ctx(&ana, &log);
        assert_eq!(at_ana.lookup_secrets(2 * OLDER_EPOCHS_FOR_MS).len(), 2);
    }

    #[test]
    fn a_removal_from_a_device_not_verified_is_read_only_when_it_is_one() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let luis = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, luis.public_key(), "luis", 2).unwrap();
        let at_luis = ctx(&luis, &log);
        log.remove(&ana, luis.public_key(), 3).unwrap();
        let removal: Vec<Vec<u8>> = log
            .until_removed(&luis.public_key())
            .into_iter()
            .map(Entry::encode)
            .collect();
        assert_eq!(at_luis.my_removal(&removal).map(|e| e.len()), Some(1));
        // Anything else, or a removal under a pile of entries, is not looked at.
        let stranger = DeviceIdentity::generate().unwrap();
        let junk: Vec<Vec<u8>> = (0..MAX_REMOVAL_ENTRIES as u64)
            .map(|i| {
                Entry::sign(
                    &stranger,
                    log.group_id(),
                    vec![],
                    i,
                    fogata_state::Body::Leave,
                )
                .encode()
            })
            .collect();
        assert_eq!(at_luis.my_removal(&junk), None);
        let buried: Vec<Vec<u8>> = junk.iter().chain(&removal).cloned().collect();
        assert_eq!(at_luis.my_removal(&buried), None);
        // A removal signed by a device that is no member here is not checked at all: with
        // many entries along, it would cost a signature check each.
        let forged = Entry::sign(
            &stranger,
            log.group_id(),
            log.heads(),
            4,
            fogata_state::Body::Remove {
                member: luis.public_key(),
                rotation: fogata_state::Rotation::new(&log.group_id(), [ana.public_key()])
                    .unwrap()
                    .0,
            },
        );
        let started = std::time::Instant::now();
        let mut many: Vec<Vec<u8>> = junk[..MAX_REMOVAL_ENTRIES - 1].to_vec();
        many.push(forged.encode());
        for _ in 0..50 {
            assert_eq!(at_luis.my_removal(&many), None);
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        // Nor one that names Ana, a member, as its author without her signature: the
        // author is only a name until the signature is checked, which costs one check.
        let mut named = forged.clone();
        named.author = ana.public_key();
        let mut many: Vec<Vec<u8>> = junk[..MAX_REMOVAL_ENTRIES - 1].to_vec();
        many.push(named.encode());
        let started = std::time::Instant::now();
        for _ in 0..50 {
            assert_eq!(at_luis.my_removal(&many), None);
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
    }

    /// A log judged away from the node's loop, a copy of the group's with more entries, is
    /// taken whole only when nothing came to the group meanwhile that it lacks: otherwise
    /// nothing changes, and its entries go the usual way.
    #[test]
    fn a_log_judged_elsewhere_is_taken_whole_only_if_nothing_came_meanwhile() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let mut at_bea = ctx(&bea, &log);
        let mut judged = log.clone();
        judged
            .append(&ana, fogata_state::Body::Rename { name: "h".into() }, 3)
            .unwrap();
        let mut taken = ctx(&bea, &log);
        assert_eq!(taken.take_judged(judged.clone()).map(|n| n.len()), Some(1));
        assert_eq!(taken.name(), "h");
        // Something came meanwhile: the log stays as it is.
        log.append(&ana, fogata_state::Body::SetIcon { icon: 2 }, 4)
            .unwrap();
        at_bea.merge(&log);
        assert_eq!(at_bea.take_judged(judged.clone()), None);
        assert_eq!(at_bea.name(), "g");
        assert_eq!(at_bea.log().len(), log.len());
        // So does a log of another group.
        let other = GroupLog::create(&ana, "g", 1).unwrap();
        assert_eq!(ctx(&bea, &log).take_judged(other), None);
    }

    /// What a removal shown with as many entries as it may bring costs the node's loop, in a
    /// group of 4096 entries written apart to cost the most: the checks and the copy of the
    /// log before it is judged elsewhere, and taking the judged log after; and what judging
    /// it costs. For a release build: `cargo test --release -p fogata-net --lib --
    /// --ignored measure_removal_on_the_loop --nocapture`.
    #[test]
    #[ignore]
    fn measure_removal_on_the_loop() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let leo = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, leo.public_key(), "leo", 2).unwrap();
        let group = log.group_id();
        let write_apart = |log: &mut GroupLog, n: usize, from: u64| {
            let mut last = [log.heads(), log.heads()];
            let mut written = Vec::new();
            for i in 0..n {
                let by: &DeviceIdentity = [&ana, &leo][i % 2];
                let body = fogata_state::Body::Rename {
                    name: format!("n{i}"),
                };
                let entry = Entry::sign(by, group, last[i % 2].clone(), from + i as u64, body);
                last[i % 2] = vec![entry.hash()];
                written.push(entry);
            }
            log.insert_all(written);
        };
        write_apart(&mut log, 4096 - 2 - MAX_REMOVAL_ENTRIES, 10);
        let at_leo = ctx(&leo, &log);
        write_apart(&mut log, MAX_REMOVAL_ENTRIES - 1, 10_000);
        log.remove(&ana, leo.public_key(), 20_000).unwrap();
        let shown: Vec<Vec<u8>> = log
            .entries_in_order()
            .into_iter()
            .filter(|e| at_leo.log().get(&e.hash()).is_none())
            .map(Entry::encode)
            .collect();
        let started = std::time::Instant::now();
        let trial = at_leo.removal_trial(&shown).unwrap();
        let checked = started.elapsed();
        let started = std::time::Instant::now();
        let (_, judged) = trial.judge().unwrap();
        let judging = started.elapsed();
        let mut at_leo = at_leo;
        let started = std::time::Instant::now();
        let new = at_leo.take_judged(judged).unwrap();
        let taken = started.elapsed();
        assert!(!at_leo.i_am_member());
        println!(
            "{} entries shown in a log of {}: checked and copied on the loop in {checked:?}, \
             judged elsewhere in {judging:?}, taken on the loop in {taken:?}",
            new.len(),
            at_leo.log().len() - new.len(),
        );
    }

    #[test]
    fn entries_asked_for_go_in_parts_a_message_carries() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 1).unwrap();
        for i in 0..100u64 {
            log.append(
                &ana,
                fogata_state::Body::SetPicture {
                    picture: Some(vec![i as u8; fogata_state::MAX_PICTURE_BYTES]),
                },
                2 + i,
            )
            .unwrap();
            // Bea signs on them now and then: one device alone writes at most 4 MiB.
            if i % 40 == 39 {
                let name = format!("g{i}");
                log.append(&bea, fogata_state::Body::Rename { name }, 2 + i)
                    .unwrap();
            }
        }
        let at_ana = ctx(&ana, &log);
        let all = at_ana.hashes();
        let whole: usize = at_ana.encoded(all.clone()).iter().map(Vec::len).sum();
        assert!(whole > SYNC_BYTES);
        let part = at_ana.encoded_part(all.clone());
        let size: usize = part.iter().map(Vec::len).sum();
        assert!(size <= SYNC_BYTES);
        assert!(part.len() >= SYNC_BYTES / fogata_state::MAX_PICTURE_BYTES - 1);
        assert!(part.len() < all.len());
        // The first ones, in the order asked.
        assert_eq!(part, at_ana.encoded(all.into_iter().take(part.len())));
    }

    #[test]
    fn a_manager_rotates_for_a_reported_key_once_and_only_for_another_s_epoch() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let leo = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let managers = |n| fogata_state::Body::SetManagers { first: Some(n) };
        log.append(&ana, managers(1), 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        log.add(&ana, leo.public_key(), "leo", 3).unwrap();
        let mut at_ana = ctx(&ana, &log);
        let first = at_ana.current().unwrap().epoch();
        // Ana's own epoch: its boxes open.
        assert!(
            at_ana
                .mend_reported_key(&bea.public_key(), first, 10)
                .is_empty()
        );
        // Leo, moved up, manages; Bea, now third, does not.
        log.append(&ana, managers(2), 4).unwrap();
        let up = fogata_state::Body::Promote {
            member: leo.public_key(),
        };
        log.append(&ana, up, 4).unwrap();
        log.rotate(&leo, 5).unwrap();
        let mut at_ana = ctx(&ana, &log);
        let leos = at_ana.current().unwrap().epoch();
        // Not for another epoch, nor for a device that is not a member.
        assert!(
            at_ana
                .mend_reported_key(&bea.public_key(), first, 10)
                .is_empty()
        );
        let stranger = DeviceIdentity::generate().unwrap().public_key();
        assert!(at_ana.mend_reported_key(&stranger, leos, 10).is_empty());
        // Once for Leo's epoch, then its own epoch is current.
        assert_eq!(
            at_ana.mend_reported_key(&bea.public_key(), leos, 10).len(),
            1
        );
        let mine = at_ana.current().unwrap().epoch();
        assert!(
            at_ana
                .mend_reported_key(&bea.public_key(), mine, 100_000)
                .is_empty()
        );
        // A member is never the one to rotate for it.
        let mut at_bea = ctx(&bea, &log);
        assert!(
            at_bea
                .mend_reported_key(&leo.public_key(), leos, 10)
                .is_empty()
        );
    }

    #[test]
    fn one_member_s_reports_start_only_so_many_epochs_a_day() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let leo = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        log.add(&ana, leo.public_key(), "leo", 3).unwrap();
        let mut at_ana = ctx(&ana, &log);
        // Leo starts an epoch, Bea says its box does not open, again and again.
        let mut report = |at_ana: &mut GroupContext, now: u64| {
            for entry in at_ana.log().entries() {
                let _ = log.insert(entry.clone());
            }
            let rotation = log.rotate(&leo, now).unwrap();
            at_ana.insert_encoded(&[rotation.encode()]);
            let epoch = at_ana.current().unwrap().epoch();
            at_ana
                .mend_reported_key(&bea.public_key(), epoch, now)
                .len()
        };
        let minute = MEND_EVERY_MS + 1;
        let mended: usize = (1..=6).map(|i| report(&mut at_ana, i * minute)).sum();
        assert_eq!(mended, usize::from(MENDS_PER_MEMBER));
        // A day later, again.
        assert_eq!(report(&mut at_ana, 6 * minute + MENDS_FOR_MS), 1);
    }

    #[test]
    fn managers_replace_a_week_old_key_one_after_another() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let leo = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        log.add(&ana, leo.public_key(), "leo", 3).unwrap();
        let first_two = fogata_state::Body::SetManagers { first: Some(2) };
        log.append(&ana, first_two, 4).unwrap();
        let week = 1 + KEY_LIFETIME_MS;
        let (mut at_ana, mut at_bea, mut at_leo) =
            (ctx(&ana, &log), ctx(&bea, &log), ctx(&leo, &log));
        assert!(at_ana.rotate_if_old(week - 1).is_empty());
        // Leo is no manager; Bea, let in after Ana, waits a little longer.
        assert!(at_leo.rotate_if_old(week * 2).is_empty());
        assert!(at_bea.rotate_if_old(week).is_empty());
        assert_eq!(
            at_bea.rotate_if_old(week + KEY_LIFETIME_STAGGER_MS).len(),
            1
        );
        assert_eq!(at_ana.rotate_if_old(week).len(), 1);
        // A new epoch for everyone, once a week.
        let log = at_ana.log().clone();
        assert_eq!(log.state().epochs.len(), 2);
        assert!(log.current_keys(&leo).is_some());
        assert!(at_ana.rotate_if_old(week + 1).is_empty());
        assert_eq!(at_ana.rotate_if_old(2 * week).len(), 1);
    }

    #[test]
    fn a_key_dated_far_ahead_is_still_replaced() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        // Bea, who may rotate, dates her rotation near the end of all time.
        let mut at_bea = ctx(&bea, &log);
        let rotation = at_bea.log.rotate(&bea, u64::MAX - 1).unwrap();
        log.insert(rotation).unwrap();
        let now = 10 * KEY_LIFETIME_MS;
        // Seen as a week old when first seen: Ana, first on the ladder, rotates at once;
        // Bea an hour after she first saw it, were Ana not around.
        let mut at_ana = ctx(&ana, &log);
        let mut at_bea = ctx(&bea, &log);
        assert!(at_bea.rotate_if_old(now).is_empty());
        assert!(
            at_bea
                .rotate_if_old(now + KEY_LIFETIME_STAGGER_MS - 1)
                .is_empty()
        );
        assert_eq!(at_bea.rotate_if_old(now + KEY_LIFETIME_STAGGER_MS).len(), 1);
        assert_eq!(at_ana.rotate_if_old(now).len(), 1);
        // The new epoch is dated by Ana's clock and ages as usual.
        assert!(at_ana.rotate_if_old(now + KEY_LIFETIME_MS - 1).is_empty());
        assert_eq!(at_ana.rotate_if_old(now + KEY_LIFETIME_MS).len(), 1);
        // A rotation dated a few hours ahead ages from when this device saw it.
        let soon = at_ana.log().clone();
        let mut at_bea = ctx(&bea, &soon);
        let later = now + KEY_LIFETIME_MS + 1;
        let ahead = at_bea.log.rotate(&bea, later + 3_600_000).unwrap();
        at_ana.insert_encoded(&[ahead.encode()]);
        assert!(at_ana.rotate_if_old(later).is_empty());
        assert!(at_ana.rotate_if_old(later + KEY_LIFETIME_MS - 1).is_empty());
        assert_eq!(at_ana.rotate_if_old(later + KEY_LIFETIME_MS).len(), 1);
    }

    /// A member that sends in every voice room of a group a chat full of lines and as many
    /// shares as a participant may have, each one as heavy as the room takes it (`padded`:
    /// each padded to the largest group message, which the room refuses).
    fn fill_rooms(
        rooms: u8,
        padded: bool,
    ) -> Vec<fogata_state::RoomTracker<fogata_proto::WireMessage>> {
        use fogata_state::{
            EVENT_SLACK_BYTES, FileInfo, MAX_CHAT_CHARS, MAX_SHARES_EACH, MAX_THUMB_BYTES,
            RoomEvent, RoomInput, RoomTracker, VideoPreview,
        };
        let me = DeviceIdentity::generate().unwrap().public_key();
        let bad = DeviceIdentity::generate().unwrap().public_key();
        // A message whose whole weighs `weight`.
        let wire = |n: u32, weight: usize| {
            let mut wire = fogata_proto::WireMessage {
                version: 1,
                group_id: vec![1; 16],
                epoch: 3,
                sender: bad.as_bytes().to_vec(),
                timestamp_ms: 1_000,
                message_id: n.to_be_bytes().repeat(4),
                nonce: vec![4; 24],
                ciphertext: Vec::new(),
                signature: vec![6; 64],
            };
            let header = room_record_weight(&wire);
            wire.ciphertext = vec![5; weight.saturating_sub(header)];
            wire
        };
        let largest = 192 * 1024;
        let control = "\u{1}".repeat(MAX_CHAT_CHARS);
        let line = 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES;
        let share = MAX_THUMB_BYTES.div_ceil(3) * 4 + 4 + EVENT_SLACK_BYTES;
        let mut n = 0u32;
        let mut trackers = Vec::new();
        for v in 0..rooms {
            let mut t = RoomTracker::new(me).weighing(room_record_weight);
            let room = [v; 16];
            let mut send = |t: &mut RoomTracker<_>, input, weight| {
                n += 1;
                let mut message_id = [0u8; 16];
                message_id[..4].copy_from_slice(&n.to_be_bytes());
                let event = RoomEvent {
                    room,
                    sender: bad,
                    timestamp_ms: 1_000 + u64::from(n),
                    message_id,
                    input,
                };
                t.apply(event, wire(n, weight), 1_000);
            };
            let join = RoomInput::Join {
                started_ms: 1_000,
                nick: "m".into(),
            };
            send(&mut t, join, 300);
            for _ in 0..500 {
                let chat = RoomInput::Chat {
                    nick: "m".into(),
                    text: control.clone(),
                };
                send(&mut t, chat, if padded { largest } else { line });
            }
            for i in 0..MAX_SHARES_EACH as u8 {
                let file = FileInfo {
                    id: [i; 32],
                    name: format!("f{i:03}"),
                    size: 1,
                    video: Some(VideoPreview {
                        thumb: vec![9; MAX_THUMB_BYTES],
                        width: 2,
                        height: 2,
                        duration_ms: 1,
                    }),
                };
                send(
                    &mut t,
                    RoomInput::Share(file),
                    if padded { largest } else { share },
                );
            }
            trackers.push(t);
        }
        trackers
    }

    #[test]
    fn a_member_padding_room_events_makes_others_keep_little() {
        let line = 6 * fogata_state::MAX_CHAT_CHARS + 1 + fogata_state::EVENT_SLACK_BYTES;
        let quota = fogata_state::MAX_SHARE_BYTES / usize::from(fogata_state::MAX_IN_ROOM);
        for padded in [true, false] {
            for t in fill_rooms(1, padded) {
                let kept: usize = t.snapshot().iter().map(room_record_weight).sum();
                let room = t.room().unwrap();
                if padded {
                    // Only the join.
                    assert_eq!((room.chat.len(), room.files.len()), (0, 0));
                    assert!(kept < 1024, "{kept} bytes kept");
                } else {
                    // The chat, and the sender's part of the shares as sent.
                    assert_eq!(room.chat.len(), 500);
                    assert!(kept <= 500 * line + quota * 4 / 3 + 100 * 1024 + 1024);
                }
            }
        }
    }

    /// The rooms of a group with `rooms` rooms, each with `devices` devices of one member in
    /// it, every one sharing as many videos as it may, each with the biggest picture, one
    /// room after the other (the first while it was the group's only room, if
    /// `first_alone`); what they make a member keep of them, as sent.
    fn rooms_shared_in(
        devices: u8,
        rooms: u8,
        first_alone: bool,
    ) -> (
        Vec<fogata_state::RoomTracker<fogata_proto::WireMessage>>,
        usize,
    ) {
        use fogata_proto::WireMessage;
        use fogata_state::{
            FileInfo, MAX_SHARES_EACH, MAX_THUMB_BYTES, RoomEvent, RoomInput, RoomTracker,
            VideoPreview,
        };
        let me = DeviceIdentity::generate().unwrap().public_key();
        let senders: Vec<DevicePublicKey> = (0..devices)
            .map(|_| DeviceIdentity::generate().unwrap().public_key())
            .collect();
        let mut n = 0u32;
        let mut trackers = BTreeMap::new();
        for v in 0..rooms {
            let mut t = RoomTracker::new(me).weighing(room_record_weight);
            let now = if first_alone && v == 0 { 1 } else { rooms };
            t.set_room_for(fogata_state::MAX_IN_ROOM, usize::from(now));
            let room = [v; 16];
            let mut send = |t: &mut RoomTracker<WireMessage>, sender, input: RoomInput| {
                n += 1;
                let mut message_id = [0u8; 16];
                message_id[..4].copy_from_slice(&n.to_be_bytes());
                let (sender_bytes, timestamp_ms) = (sender, 1_000 + u64::from(n));
                let event = RoomEvent {
                    room,
                    sender,
                    timestamp_ms,
                    message_id,
                    input: input.clone(),
                };
                // The event as sent: its own size, under what the room takes.
                let said = match &input {
                    RoomInput::Share(file) => {
                        file.name.len() + file.video.as_ref().map_or(0, |v| v.thumb.len() * 4 / 3)
                    }
                    _ => 64,
                };
                let wire = WireMessage {
                    version: 1,
                    group_id: vec![1; 16],
                    epoch: 3,
                    sender: sender_bytes.as_bytes().to_vec(),
                    timestamp_ms,
                    message_id: message_id.to_vec(),
                    nonce: vec![4; 24],
                    ciphertext: vec![5; said + 64],
                    signature: vec![6; 64],
                };
                t.apply(event, wire, 1_000);
            };
            for (d, sender) in senders.iter().enumerate() {
                let join = RoomInput::Join {
                    started_ms: 1_000,
                    nick: "m".into(),
                };
                send(&mut t, *sender, join);
                for i in 0..MAX_SHARES_EACH as u32 {
                    let mut id = [0u8; 32];
                    id[..4].copy_from_slice(
                        &(u32::from(v) * 100_000 + d as u32 * 1_000 + i).to_be_bytes(),
                    );
                    let file = FileInfo {
                        id,
                        name: format!("f{i:03}"),
                        size: 1,
                        video: Some(VideoPreview {
                            thumb: vec![9; MAX_THUMB_BYTES],
                            width: 2,
                            height: 2,
                            duration_ms: 1,
                        }),
                    };
                    send(&mut t, *sender, RoomInput::Share(file));
                }
            }
            trackers.insert([v; 8], t);
        }
        // The first room hears of the others with its next event: the node tells a room how
        // many the group has with each.
        if first_alone && let Some(first) = trackers.get_mut(&[0; 8]) {
            first.set_room_for(fogata_state::MAX_IN_ROOM, usize::from(rooms));
            let here = RoomInput::Here {
                started_ms: 1_000,
                nick: "m".into(),
            };
            let (event, wire) = room_event(0, senders[0], 1_000_000, here);
            first.apply(event, wire, 1_000);
        }
        let trackers: Vec<_> = trackers.into_values().collect();
        let kept = trackers
            .iter()
            .map(|t| t.snapshot().iter().map(room_record_weight).sum::<usize>())
            .sum();
        (trackers, kept)
    }

    /// A member with eight devices in each of the ten rooms of a group, every one sharing
    /// as much as it may, makes the others keep at most the group's bytes for shares, not
    /// that much a room for each device; also when the first room filled up while it was
    /// the group's only one.
    #[test]
    fn a_member_with_devices_in_every_room_makes_others_keep_the_groups_shares_at_most() {
        use fogata_state::MAX_SHARE_BYTES;
        for first_alone in [false, true] {
            let (trackers, kept) = rooms_shared_in(8, 10, first_alone);
            let files: usize = trackers.iter().map(|t| t.room().unwrap().files.len()).sum();
            println!("{files} files, {} MiB kept", kept >> 20);
            // Each share as sent, its picture encoded (4 bytes for 3), and the joins.
            assert!(files > 0);
            assert!(
                kept <= MAX_SHARE_BYTES * 4 / 3 + 8 * 10 * 1024,
                "{kept} bytes kept"
            );
            let counted: usize = trackers.iter().map(|t| t.share_bytes()).sum();
            assert!(counted <= MAX_SHARE_BYTES, "{counted} bytes counted");
        }
        // One device in one room keeps its part, as before.
        let (_, one) = rooms_shared_in(1, 1, false);
        assert!(one >= MAX_SHARE_BYTES / 8 - 100 * 1024, "{one} bytes kept");
    }

    /// A room event as a member sends it: `n` numbers it, and dates it.
    fn room_event(
        room: u8,
        sender: DevicePublicKey,
        n: u32,
        input: fogata_state::RoomInput,
    ) -> (fogata_state::RoomEvent, fogata_proto::WireMessage) {
        let mut message_id = [0u8; 16];
        message_id[..4].copy_from_slice(&n.to_be_bytes());
        let timestamp_ms = 1_000 + u64::from(n);
        let said = match &input {
            fogata_state::RoomInput::Share(file) => {
                file.name.len() + file.video.as_ref().map_or(0, |v| v.thumb.len() * 4 / 3)
            }
            _ => 64,
        };
        let wire = fogata_proto::WireMessage {
            version: 1,
            group_id: vec![1; 16],
            epoch: 3,
            sender: sender.as_bytes().to_vec(),
            timestamp_ms,
            message_id: message_id.to_vec(),
            nonce: vec![4; 24],
            ciphertext: vec![5; said + 64],
            signature: vec![6; 64],
        };
        let event = fogata_state::RoomEvent {
            room: [room; 16],
            sender,
            timestamp_ms,
            message_id,
            input,
        };
        (event, wire)
    }

    type Rooms =
        BTreeMap<fogata_state::VoiceRoomId, fogata_state::RoomTracker<fogata_proto::WireMessage>>;

    /// A room event of voice room `voice` reaching a device whose group has `rooms` rooms,
    /// taken as the node takes it.
    fn reach(
        device: &mut Rooms,
        me: DevicePublicKey,
        voice: u8,
        rooms: usize,
        (event, wire): (fogata_state::RoomEvent, fogata_proto::WireMessage),
    ) {
        let tracker = device
            .entry([voice; 8])
            .or_insert_with(|| fogata_state::RoomTracker::new(me).weighing(room_record_weight));
        tracker.set_room_for(fogata_state::MAX_IN_ROOM, rooms);
        tracker.apply(event, wire, 1_000);
    }

    /// The files each room of a device lists, by room.
    fn listed(
        device: &Rooms,
    ) -> BTreeMap<fogata_state::VoiceRoomId, std::collections::BTreeSet<[u8; 32]>> {
        device
            .iter()
            .map(|(v, t)| {
                let files = t
                    .room()
                    .map(|r| r.files.keys().copied().collect())
                    .unwrap_or_default();
                (*v, files)
            })
            .collect()
    }

    /// Two members of a group whose shares fill what its rooms keep list the same files in
    /// each room once both have every event, whatever came first: Marta saw the first room
    /// fill up while it was the group's only one, and the second room's shares after; Leo
    /// came later, and got the first room's shares after or before the second's.
    #[test]
    fn two_members_list_the_same_shares_whatever_came_first() {
        use fogata_state::{FileInfo, MAX_SHARES_EACH, MAX_THUMB_BYTES, RoomInput, VideoPreview};
        let senders: Vec<DevicePublicKey> = (0..8)
            .map(|_| DeviceIdentity::generate().unwrap().public_key())
            .collect();
        let video = |room: u8, d: usize, i: u32| {
            let mut id = [room; 32];
            id[..4].copy_from_slice(&(d as u32 * 1_000 + i).to_be_bytes());
            FileInfo {
                id,
                name: format!("v{i:03}"),
                size: 1,
                video: Some(VideoPreview {
                    thumb: vec![9; MAX_THUMB_BYTES],
                    width: 2,
                    height: 2,
                    duration_ms: 1,
                }),
            }
        };
        // Each room's events in the order they were sent: everyone joins, then shares as
        // many videos as it may.
        let mut n = 0;
        let mut sent = |room: u8| {
            let mut events = Vec::new();
            for sender in &senders {
                n += 1;
                let join = RoomInput::Join {
                    started_ms: 1_000,
                    nick: "m".into(),
                };
                events.push(room_event(room, *sender, n, join));
            }
            for (d, sender) in senders.iter().enumerate() {
                for i in 0..MAX_SHARES_EACH as u32 {
                    n += 1;
                    events.push(room_event(
                        room,
                        *sender,
                        n,
                        RoomInput::Share(video(room, d, i)),
                    ));
                }
            }
            events
        };
        let (first, second) = (sent(0), sent(1));
        // Everyone in the first room still there after the second room came.
        let here: Vec<_> = senders
            .iter()
            .enumerate()
            .map(|(d, s)| {
                let here = RoomInput::Here {
                    started_ms: 1_000,
                    nick: "m".into(),
                };
                room_event(0, *s, 900_000 + d as u32, here)
            })
            .collect();
        let me = DeviceIdentity::generate().unwrap().public_key();
        let mut marta = Rooms::new();
        for event in first.iter().cloned() {
            reach(&mut marta, me, 0, 1, event);
        }
        for event in second.iter().cloned() {
            reach(&mut marta, me, 1, 2, event);
        }
        for event in here.iter().cloned() {
            reach(&mut marta, me, 0, 2, event);
        }
        let listed_by_marta = listed(&marta);
        assert!(listed_by_marta.values().all(|files| !files.is_empty()));
        for first_room_first in [true, false] {
            let mut leo = Rooms::new();
            let rooms: Vec<(u8, &Vec<_>)> = if first_room_first {
                vec![(0, &first), (1, &second), (0, &here)]
            } else {
                vec![(1, &second), (0, &first), (0, &here)]
            };
            for (voice, events) in rooms {
                for event in events.iter().cloned() {
                    reach(&mut leo, me, voice, 2, event);
                }
            }
            assert_eq!(
                listed(&leo),
                listed_by_marta,
                "Leo, the first room's shares first: {first_room_first}"
            );
        }
    }

    /// How much memory a member sending the heaviest events a room takes makes a device keep
    /// in the ten rooms of a group, measured (Linux).
    #[test]
    #[ignore]
    fn measure_room_memory() {
        let rss = || {
            let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
            let pages: u64 = statm.split_whitespace().nth(1).unwrap().parse().unwrap();
            pages * 4096
        };
        let before = rss();
        let trackers = fill_rooms(10, false);
        let after = rss();
        let records: usize = trackers.iter().map(|t| t.snapshot().len()).sum();
        println!("{records} records kept; {} MiB", (after - before) >> 20);
    }
}
