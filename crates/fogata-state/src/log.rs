//! The set of entries a device knows about a group, their common order, and helpers to
//! write new entries.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::{Arc, Mutex, PoisonError},
};

use fogata_crypto::{
    CryptoError, DeviceIdentity, DevicePublicKey, GroupId, GroupKeys, GroupSecret, Sealed,
    names::tidy,
};
use sha2::{Digest, Sha256};

use crate::{
    banner::GroupBanner,
    budget::{RUN_ROOM, RunRoom},
    cache::{Cache, View},
    entry::{
        Body, Entry, EntryHash, EpochId, InviteToken, MAX_ENTRY_BYTES, Rotation, Ticket,
        VoiceRoomId, commitment, epoch_number, secret_context,
    },
    picture::{GroupPicture, is_group_picture},
    record::MAX_RECORD_AHEAD_MS,
    state::{GroupState, LEVEL_UNKNOWN, Level, MAX_GROUP_NAME_CHARS, fair_name},
};

/// Why an entry from a device the log never named is refused.
const UNNAMED: &str = "author never named in the group";

/// Most parents an entry may name. A new entry names the heads, and honest devices see few
/// at once; an entry naming more costs every member more to check and to keep.
pub const MAX_PARENTS: usize = 64;

/// Why a manager does not let a device in with a `Join` signed on entries nobody keeps.
pub const JOIN_ON_SPARE: &str = "the join is signed on entries the group does not keep";

/// Why a manager does not let a device in with a ticket that expired by its clock.
pub const TICKET_EXPIRED: &str = "the invite expired";

/// Why [`GroupLog::append`] does not write a `Level` entry: [`GroupLog::raise_level`] does.
pub const RAISE_LEVEL_ONLY: &str = "a group's level is raised with raise_level";

/// Most entries kept whose parents are not known yet. They wait for the parents to arrive;
/// past this many, those of the author with the most waiting go, the last to arrive first,
/// so a member cannot grow everyone's log with entries that never apply.
pub const MAX_WAITING: usize = 256;
/// Most bytes of entries that wait for parents, all together.
pub const MAX_WAITING_BYTES: usize = 4 << 20;
/// Most entries of one author that wait for parents: one member cannot take the room the
/// others' entries wait in.
pub const MAX_WAITING_PER_AUTHOR: usize = 64;
/// Most bytes of one author's entries that wait for parents.
pub const MAX_WAITING_BYTES_PER_AUTHOR: usize = 1 << 20;

/// Most spare entries kept ([`GroupLog::spare`]: entries past their author's run or of a kind
/// that never applies here, that nothing else in the log needs), all together. Past it, or
/// past an author's share, those of the author with the most bytes go, the oldest first: a
/// member cannot fill everyone's disk with them, and what applies, or could change what
/// applies, is never let go.
pub const MAX_SPARE: usize = 512;
/// Most bytes of spare entries, all together.
pub const MAX_SPARE_BYTES: usize = 8 << 20;
/// Most spare entries of one author.
pub const MAX_SPARE_PER_AUTHOR: usize = 64;
/// Most bytes of one author's spare entries.
pub const MAX_SPARE_BYTES_PER_AUTHOR: usize = 1 << 20;

/// Views of the group as entries saw it (see [`GroupLog::insert`]) kept at once, by the
/// entries' parents.
const MAX_VIEWS_KEPT: usize = 32;

/// Entries of a batch from another device judged on a group worked out for their own
/// parents ([`GroupLog::insert_some`]): each costs about as much as working out the whole
/// group, and as much again to place it early in the order, so a member sending entries
/// signed on old parents holds the node up for that long. Entries signed on what everyone
/// signs on cost nothing of this. In a log of 4096 entries (one member's whole burst) each
/// takes 10 to 27 ms in a release build, so a batch holds the node up a quarter of a
/// second at most, and the rest come with the next sync, asked for at once while one
/// brings something new. Entries that came before their parents count too, when they are
/// judged once the parents are here: sent ahead of the parent they share, they would
/// otherwise all be judged with the batch.
const MAX_VIEWS_PER_BATCH: usize = 8;

/// What judging a batch may still cost, in groups worked out for an entry's own parents
/// (see [`MAX_VIEWS_PER_BATCH`]), and the entries refused meanwhile, by who answers for them
/// (see [`Taken::refused`]).
struct Work {
    views: usize,
    refused: BTreeMap<DevicePublicKey, usize>,
    /// Entries left out past `views`.
    left_out: usize,
    /// Entries this code judged already and kept, taken without judging them again (see
    /// [`Verdicts`]).
    judged: HashSet<EntryHash>,
    /// Whether the signatures of those were checked too, in this process.
    signed: bool,
}

impl Work {
    fn unbounded() -> Self {
        Self::with_views(usize::MAX)
    }

    fn with_views(views: usize) -> Self {
        Self {
            views,
            refused: BTreeMap::new(),
            left_out: 0,
            judged: Default::default(),
            signed: false,
        }
    }

    /// Counts `entry`, judged, as refused for `why`, against its author; but not a manager's
    /// `Level` entry raising the group above every level this version judges at: that comes
    /// from a copy newer than this one (see [`Level`]), and it is this copy that is behind.
    fn refuse(&mut self, entry: &Entry, why: &str) {
        if why == LEVEL_UNKNOWN
            && matches!(entry.body, Body::Level { level } if level > Level::CURRENT.number())
        {
            return;
        }
        *self.refused.entry(entry.author).or_default() += 1;
    }
}

/// What a batch from another device brought ([`GroupLog::insert_some`]).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Taken {
    /// The entries that were new and are kept.
    pub new: Vec<EntryHash>,
    /// How many entries were judged and refused, by author: entries their author could not
    /// make, each of which may have cost working out the group to find so. An honest device
    /// writes none. Those of the batch and those kept from an earlier one that are refused
    /// once this one brings their parents alike: an author does not get away with them by
    /// sending them ahead of their parent. Counted against the author, who signed them, and
    /// not whoever passed them on: an honest member hands on entries that wait for their
    /// parents without judging them. A join's author may be a key made up for it, which
    /// nobody pays for: a join costs as much to judge taken as refused.
    pub refused: BTreeMap<DevicePublicKey, usize>,
    /// How many new entries were left out, past what judging a batch may cost: they come
    /// again with the next sync or offer.
    pub left_out: usize,
}

impl Taken {
    /// How many entries were refused in all.
    pub fn refused_count(&self) -> usize {
        self.refused.values().sum()
    }
}

/// When an entry arrived on this device, counted in entries, and its length in bytes.
type Arrival = (u64, usize);

/// How many entries, and how many bytes of them, one author and everyone together may keep
/// of one kind that applies to nothing.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Room {
    pub(crate) per_author: usize,
    pub(crate) author_bytes: usize,
    pub(crate) all: usize,
    pub(crate) all_bytes: usize,
}

const WAITING_ROOM: Room = Room {
    per_author: MAX_WAITING_PER_AUTHOR,
    author_bytes: MAX_WAITING_BYTES_PER_AUTHOR,
    all: MAX_WAITING,
    all_bytes: MAX_WAITING_BYTES,
};

const SPARE_ROOM: Room = Room {
    per_author: MAX_SPARE_PER_AUTHOR,
    author_bytes: MAX_SPARE_BYTES_PER_AUTHOR,
    all: MAX_SPARE,
    all_bytes: MAX_SPARE_BYTES,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StateError {
    #[error("entry belongs to another group")]
    WrongGroup,
    #[error("entry signature is invalid")]
    BadSignature,
    #[error("not allowed: {0}")]
    NotAllowed(&'static str),
    #[error("entry is too long")]
    TooLong,
    #[error("this device does not have the current group key")]
    NoSecret,
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

/// Why a device lacks the current epoch's secret ([`GroupLog::current_key_problem`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyProblem {
    /// Who wrote the entry that should have given it.
    pub by: DevicePublicKey,
    /// The entry held a box for this device that does not open to the secret the log
    /// commits to; otherwise the epoch started without one for it.
    pub bad_box: bool,
}

/// Every entry this device knows for one group.
#[derive(Clone, Debug)]
pub struct GroupLog {
    group_id: GroupId,
    pub(crate) entries: BTreeMap<EntryHash, Entry>,
    /// The group's `Create`: the first one this log took. Another is never kept, so nobody
    /// can start the group over under the same id and take it.
    root: Option<EntryHash>,
    /// Devices the log has named: its creator, devices added, and devices that joined with
    /// an invite of a named device. Entries come only from them: a member could otherwise
    /// make everyone keep any number of entries signed by keys made up for it.
    named: BTreeSet<DevicePublicKey>,
    /// The group's banner, the newest a manager chose; kept apart from the entries (see
    /// [`GroupBanner`]).
    banner: Option<GroupBanner>,
    /// The group's picture, the newest a manager chose; kept apart from the entries like the
    /// banner (see [`GroupPicture`]).
    picture: Option<GroupPicture>,
    /// The group as each MLS Commit judged so far saw it (see [`crate::chain`]): like
    /// the rotations judged (see [`Self::rotation_needed`]), fixed by the Commit's hash, so
    /// copies of the log share it.
    pub(crate) commit_views: Arc<Mutex<BTreeMap<EntryHash, Arc<crate::chain::CommitView>>>>,
    /// When each entry arrived on this device, counted in entries, and its length: the
    /// waiting room lets go of the last to arrive, which the sender cannot choose as it can
    /// an entry's time.
    arrived: BTreeMap<EntryHash, Arrival>,
    arrivals: u64,
    /// Room for spare entries ([`SPARE_ROOM`]; smaller in tests).
    spare_room: Room,
    /// How much a device writes in a run ([`crate::budget`]; smaller in tests).
    pub(crate) run_room: RunRoom,
    /// Entries kept before they could be judged: they came before a parent, or on one not
    /// judged yet. Each is judged once all its parents are (see [`Self::insert`]).
    pub(crate) unjudged: BTreeSet<EntryHash>,
    /// The order and state worked out from the entries, the rotations judged and the boxes
    /// of this device opened (see [`crate::cache`]).
    pub(crate) cache: Box<Cache>,
    /// How many entries were checked for insertion, and how many signatures, for tests of
    /// the work a batch costs.
    #[cfg(test)]
    tries: usize,
    #[cfg(test)]
    verified: usize,
    /// How many times the group was worked out to judge an entry on its own parents.
    #[cfg(test)]
    views_worked: std::cell::Cell<usize>,
}

impl GroupLog {
    /// Starts a new group with `identity` as creator.
    pub fn create(identity: &DeviceIdentity, name: &str, now_ms: u64) -> Result<Self, StateError> {
        let name = tidy(name);
        if name.is_empty() || !fair_name(&name, MAX_GROUP_NAME_CHARS) {
            return Err(StateError::NotAllowed("invalid group name"));
        }
        let group_id = GroupId::random()?;
        let (start, _) = Rotation::new(&group_id, [identity.public_key()])?;
        let entry = Entry::sign(
            identity,
            group_id,
            vec![],
            now_ms,
            Body::Create { name, start },
        );
        let mut log = Self::empty(group_id);
        log.insert(entry)?;
        Ok(log)
    }

    /// A log that only knows the group id, for a device about to join.
    pub fn empty(group_id: GroupId) -> Self {
        Self {
            group_id,
            entries: BTreeMap::new(),
            root: None,
            named: BTreeSet::new(),
            banner: None,
            picture: None,
            commit_views: Arc::default(),
            arrived: BTreeMap::new(),
            arrivals: 0,
            spare_room: SPARE_ROOM,
            run_room: RUN_ROOM,
            unjudged: BTreeSet::new(),
            cache: Box::default(),
            #[cfg(test)]
            tries: 0,
            #[cfg(test)]
            verified: 0,
            #[cfg(test)]
            views_worked: std::cell::Cell::new(0),
        }
    }

    /// The group's banner, if a manager chose one (and did not take it away again).
    pub fn banner(&self) -> Option<&GroupBanner> {
        self.banner.as_ref().filter(|b| !b.picture.is_empty())
    }

    /// The newest banner record, one that takes the banner away included: what members
    /// compare and pass on.
    pub fn banner_record(&self) -> Option<&GroupBanner> {
        self.banner.as_ref()
    }

    /// Takes a banner received from anywhere when it is newer than the one here, of this
    /// group, signed by a device that manages it now. Returns whether it was taken.
    pub fn take_banner(&mut self, banner: GroupBanner) -> bool {
        let newer = self.banner.as_ref().is_none_or(|b| banner.replaces(b));
        if !newer
            || banner.group_id != self.group_id
            || !banner.verify()
            || !self.state().is_manager(&banner.author)
        {
            return false;
        }
        self.banner = Some(banner);
        true
    }

    /// [`Self::take_banner`] for a banner received at `now_ms`, by this device's clock: one
    /// dated more than a day later is refused ([`GroupBanner::from_the_future`]), or it
    /// would show over every banner chosen until then.
    pub fn take_banner_at(&mut self, banner: GroupBanner, now_ms: u64) -> bool {
        !banner.from_the_future(now_ms) && self.take_banner(banner)
    }

    /// Chooses the group's banner (a manager), or takes it away with no picture. Returns
    /// the signed record to pass on.
    pub fn set_banner(
        &mut self,
        identity: &DeviceIdentity,
        picture: Vec<u8>,
        now_ms: u64,
    ) -> Result<GroupBanner, StateError> {
        if !self.state().is_manager(&identity.public_key()) {
            return Err(StateError::NotAllowed("only managers choose the banner"));
        }
        // Newer than the one it replaces, whatever the clock says, unless that one is dated
        // so far ahead that the others would not take one dated after it.
        let time = self
            .banner
            .as_ref()
            .filter(|b| !b.from_the_future(now_ms))
            .map_or(now_ms, |b| now_ms.max(b.time_ms.saturating_add(1)));
        let banner = GroupBanner::sign(identity, self.group_id, picture, time)
            .map_err(|_| StateError::TooLong)?;
        self.banner = Some(banner.clone());
        Ok(banner)
    }

    /// The group's picture to show at `now_ms`, by this device's clock, given the log's
    /// `state`: the newer of the record a manager signed apart and the last `SetPicture`
    /// entry, which earlier versions wrote and still write. `None` shows the icon. An entry
    /// dated more than a day ahead (see [`MAX_RECORD_AHEAD_MS`]) is older than any record:
    /// dated near the end of time, it would otherwise show over every picture chosen after,
    /// its author removed or not.
    pub fn picture_in<'a>(&'a self, state: &'a GroupState, now_ms: u64) -> Option<&'a [u8]> {
        let at = entry_picture_ms(state, now_ms);
        match &self.picture {
            Some(record) if at.is_none_or(|at| record.time_ms > at) => {
                Some(record.picture.as_slice()).filter(|p| !p.is_empty())
            }
            // An entry holds only a PNG or a JPEG; anything else signed there shows the icon.
            _ => state.picture.as_deref().filter(|p| is_group_picture(p)),
        }
    }

    /// The group's picture to show (see [`Self::picture_in`]) by a clock that counts every
    /// date: for a device that has none to compare with.
    pub fn picture(&self) -> Option<Vec<u8>> {
        self.picture_in(&self.state(), u64::MAX).map(<[u8]>::to_vec)
    }

    /// The newest picture record, one that takes the picture away included: what members
    /// compare and pass on.
    pub fn picture_record(&self) -> Option<&GroupPicture> {
        self.picture.as_ref()
    }

    /// Takes a picture record received from anywhere when it is newer than the one here, of
    /// this group, signed by a device that manages it now. Returns whether it was taken.
    pub fn take_picture(&mut self, picture: GroupPicture) -> bool {
        let newer = self.picture.as_ref().is_none_or(|p| picture.replaces(p));
        if !newer
            || picture.group_id != self.group_id
            || !picture.verify()
            || !self.state().is_manager(&picture.author)
        {
            return false;
        }
        self.picture = Some(picture);
        true
    }

    /// [`Self::take_picture`] for a record received at `now_ms`, by this device's clock, as
    /// [`Self::take_banner_at`].
    pub fn take_picture_at(&mut self, picture: GroupPicture, now_ms: u64) -> bool {
        !picture.from_the_future(now_ms) && self.take_picture(picture)
    }

    /// Chooses the group's picture (a manager), or takes it away with no picture, in a record
    /// apart from the log. When a `SetPicture` entry left a picture, a `SetPicture` with none
    /// is written first, so that versions from before records, which never get the record,
    /// show the icon rather than the picture this one replaces; that entry carries no
    /// picture. Returns the signed record to pass on and that entry, if one was written.
    pub fn set_picture(
        &mut self,
        identity: &DeviceIdentity,
        picture: Vec<u8>,
        now_ms: u64,
    ) -> Result<(GroupPicture, Option<Entry>), StateError> {
        let state = self.state();
        if !state.is_manager(&identity.public_key()) {
            return Err(StateError::NotAllowed("only managers choose the picture"));
        }
        if !picture.is_empty()
            && (picture.len() > crate::picture::MAX_RECORD_PICTURE_BYTES
                || !crate::picture::is_record_picture(&picture))
        {
            return Err(StateError::NotAllowed(
                "a group's picture is a PNG, JPEG, GIF or WebP",
            ));
        }
        let cleared = if state.picture.is_some() {
            Some(self.append(identity, Body::SetPicture { picture: None }, now_ms)?)
        } else {
            None
        };
        // Newer than the record and the entry it replaces, whatever the clock says, but for
        // those dated so far ahead that the others would not take one dated after them.
        let after = |at: Option<u64>| at.map_or(0, |t| t.saturating_add(1));
        let record = self.picture.as_ref().filter(|p| !p.from_the_future(now_ms));
        let ahead = now_ms.saturating_add(MAX_RECORD_AHEAD_MS);
        let time = now_ms
            .max(after(record.map(|p| p.time_ms)))
            .max(after(entry_picture_ms(&state, now_ms)))
            .max(after(
                cleared.as_ref().map(|e| e.time_ms).filter(|t| *t <= ahead),
            ));
        let record = GroupPicture::sign(identity, self.group_id, picture, time)
            .map_err(|_| StateError::TooLong)?;
        self.picture = Some(record.clone());
        Ok((record, cleared))
    }

    pub fn group_id(&self) -> GroupId {
        self.group_id
    }
}

/// When the `SetPicture` entry that left `state`'s picture was signed, unless that is more
/// than [`MAX_RECORD_AHEAD_MS`] after `now_ms`, by this device's clock (see
/// [`GroupLog::picture_in`]).
fn entry_picture_ms(state: &GroupState, now_ms: u64) -> Option<u64> {
    state
        .picture_ms
        .filter(|at| *at <= now_ms.saturating_add(MAX_RECORD_AHEAD_MS))
}

impl GroupLog {
    /// The hash of the group's `Create`, once the log has it: two logs with the same group
    /// id and another root are two different groups.
    pub fn root(&self) -> Option<EntryHash> {
        self.root
    }

    /// Adds an entry received from anywhere. Returns whether it was new and is kept.
    ///
    /// An entry is kept only if its author could make it in its own view of the log: it
    /// applies on top of the group its parents, theirs and so on make, as the last entry
    /// (see [`Self::judge`]). That depends on nothing but the entry and the entries before
    /// it, which its hash fixes, so every device comes to the same answer, whatever else it
    /// has and in whatever order the entries came. An entry whose parents are not all here
    /// and judged is kept until they are, and judged then. One refused is not kept, nor
    /// passed on: a member cannot make every device keep entries it could never make.
    pub fn insert(&mut self, entry: Entry) -> Result<bool, StateError> {
        let mut work = Work::unbounded();
        let new = self.insert_only(entry, &mut work)?;
        if new.is_some() {
            self.judge_waited(&mut work);
            self.forget_old_waiting();
        }
        Ok(new.is_some_and(|hash| self.entries.contains_key(&hash)))
    }

    /// Adds entries received together (a log from disk or from another device), all before
    /// any waiting entry is dropped: one that comes before its parents in the batch is not
    /// lost for that. Entries that are refused are skipped. Returns the new ones.
    pub fn insert_all(&mut self, entries: impl IntoIterator<Item = Entry>) -> Vec<EntryHash> {
        self.insert_batch(entries, Work::unbounded()).new
    }

    /// [`Self::insert_all`] for entries another device sent, which may cost a lot to judge:
    /// at most [`MAX_VIEWS_PER_BATCH`] of them that need the group worked out for their own
    /// parents are judged, those that came first and those that waited for parents alike,
    /// and the others are left out, as if they had not come; they come again with the next
    /// sync. Returns the new ones, and how many were refused.
    pub fn insert_some(&mut self, entries: impl IntoIterator<Item = Entry>) -> Taken {
        self.insert_batch(entries, Work::with_views(MAX_VIEWS_PER_BATCH))
    }

    fn insert_batch(&mut self, entries: impl IntoIterator<Item = Entry>, mut work: Work) -> Taken {
        // Entries judged already go in without the order: worked out after each, as it would
        // be, it would cost as much as judging them.
        if !work.judged.is_empty() {
            *self.cache = self.cache.kept();
        }
        let mut new = Vec::new();
        // An entry whose author (for a join, its ticket's issuer) is named further down the
        // batch waits for that device and is tried again once it is named, so every entry
        // is checked twice at most: a batch ordered against its chain of invites would
        // otherwise cost as many checks as the square of its length.
        let mut unnamed: BTreeMap<DevicePublicKey, Vec<Entry>> = BTreeMap::new();
        let mut next: Vec<Entry> = entries.into_iter().collect();
        next.reverse();
        while let Some(entry) = next.pop() {
            let named_by = match &entry.body {
                Body::Add { member, .. } => vec![entry.author, *member],
                _ => vec![entry.author],
            };
            let waits_for = match &entry.body {
                Body::Join { ticket, .. } => ticket.issuer,
                _ => entry.author,
            };
            match self.insert_only(entry.clone(), &mut work) {
                Ok(inserted) => {
                    new.extend(inserted);
                    for device in named_by {
                        if let Some(mut ready) = unnamed.remove(&device) {
                            ready.reverse();
                            next.extend(ready);
                        }
                    }
                }
                Err(StateError::NotAllowed(UNNAMED)) if !self.named.contains(&waits_for) => {
                    unnamed.entry(waits_for).or_default().push(entry);
                }
                Err(_) => {}
            }
        }
        if !new.is_empty() {
            self.judge_waited(&mut work);
            self.forget_old_waiting();
            new.retain(|h| self.entries.contains_key(h));
        }
        Taken {
            new,
            refused: work.refused,
            left_out: work.left_out,
        }
    }

    /// Checks one entry and keeps it; returns its hash, made once here, when it was new.
    /// Whatever costs little (its group, an entry already here, its length, whether the log
    /// named its author) is checked before its signature: anyone who learnt the group's id
    /// can send entries, and checking a message's worth of signatures holds the node up.
    /// `work` says how many more entries may be judged on a group worked out for their own
    /// parents; an entry that would need one past that is not taken (`None`). One judged
    /// and refused is counted there.
    fn insert_only(
        &mut self,
        entry: Entry,
        work: &mut Work,
    ) -> Result<Option<EntryHash>, StateError> {
        #[cfg(test)]
        {
            self.tries += 1;
        }
        if entry.group_id != self.group_id {
            return Err(StateError::WrongGroup);
        }
        let encoded = entry.encode();
        if encoded.len() > MAX_ENTRY_BYTES {
            return Err(StateError::TooLong);
        }
        if entry.parents.len() > MAX_PARENTS {
            return Err(StateError::NotAllowed("too many parents"));
        }
        let hash: EntryHash = Sha256::digest(&encoded).into();
        // The hash covers the signature: the same bytes were checked when they came first.
        if self.entries.contains_key(&hash) {
            return Ok(None);
        }
        match &entry.body {
            Body::Create { .. } => {
                if !entry.parents.is_empty() {
                    return Err(StateError::NotAllowed("create with parents"));
                }
                if self.root.is_some() {
                    return Err(StateError::NotAllowed("second create"));
                }
            }
            Body::Join { ticket, .. } => {
                if ticket.group_id != self.group_id || !self.named.contains(&ticket.issuer) {
                    return Err(StateError::NotAllowed(UNNAMED));
                }
            }
            _ => {
                if !self.named.contains(&entry.author) {
                    return Err(StateError::NotAllowed(UNNAMED));
                }
            }
        }
        let judged = work.judged.contains(&hash);
        if !(judged && work.signed) {
            #[cfg(test)]
            {
                self.verified += 1;
            }
            #[cfg(any(test, feature = "counts"))]
            crate::work::verified();
            if !entry.signature_valid() {
                return Err(StateError::BadSignature);
            }
            if let Body::Join { ticket, .. } = &entry.body
                && !ticket.signature_valid()
            {
                return Err(StateError::NotAllowed(UNNAMED));
            }
        }
        // Judged now if every entry before it is here and judged, else once they are. One
        // judged already needs no order worked out for that (see `insert_batch`): whether
        // its parents are here and judged is enough, as an entry is judged only once its
        // parents are, so one whose parents did not all come waits as any other, and is
        // judged once they do.
        let ready = if judged {
            (entry.parents.iter())
                .all(|p| self.entries.contains_key(p) && !self.unjudged.contains(p))
        } else {
            entry.parents.iter().all(|p| self.judged_and_placed(p))
        };
        if ready && !judged {
            match self.judge(&entry, &mut work.views) {
                Some(Ok(())) => {}
                Some(Err(why)) => {
                    work.refuse(&entry, why);
                    return Err(StateError::NotAllowed(why));
                }
                None => {
                    work.left_out += 1;
                    return Ok(None);
                }
            }
        }
        match &entry.body {
            Body::Create { .. } => {
                self.root = Some(hash);
                self.named.insert(entry.author);
            }
            Body::Join { .. } => {
                self.named.insert(entry.author);
            }
            Body::Add { member, .. } => {
                self.named.insert(*member);
            }
            _ => {}
        }
        self.arrived.insert(hash, (self.arrivals, encoded.len()));
        self.arrivals += 1;
        if !ready {
            self.unjudged.insert(hash);
        }
        self.entries.insert(hash, entry);
        self.note_new(hash);
        Ok(Some(hash))
    }

    /// Keeps `entry` without judging it, as versions before judging did: for tests of logs
    /// they stored.
    #[cfg(test)]
    pub(crate) fn keep_unjudged(&mut self, entry: Entry) {
        let hash = entry.hash();
        self.named.insert(entry.author);
        if let Body::Add { member, .. } = &entry.body {
            self.named.insert(*member);
        }
        self.arrived
            .insert(hash, (self.arrivals, entry.encode().len()));
        self.arrivals += 1;
        self.entries.insert(hash, entry);
        self.note_new(hash);
    }

    /// Whether entry `hash` is in the common order and was judged (see [`Self::insert`]).
    fn judged_and_placed(&self, hash: &EntryHash) -> bool {
        !self.unjudged.contains(hash) && self.full().order.place.contains_key(hash)
    }

    /// Judges the entries kept unjudged whose parents are now all here and judged, the
    /// first in the order first, and lets go of those refused. One after an entry refused
    /// waits as one after a parent that never came. Those that need the group worked out
    /// for their own parents past what `work` allows are let go too, as if they had not
    /// come (they come again with the next sync), and those after them wait.
    fn judge_waited(&mut self, work: &mut Work) {
        if self.unjudged.is_empty() {
            return;
        }
        let ready: Vec<EntryHash> = self
            .full()
            .order
            .hashes
            .iter()
            .filter(|h| self.unjudged.contains(*h))
            .copied()
            .collect();
        let mut blocked = BTreeSet::new();
        let mut refused = Vec::new();
        let mut left_out = Vec::new();
        for hash in ready {
            let entry = &self.entries[&hash];
            if entry.parents.iter().any(|p| blocked.contains(p)) {
                blocked.insert(hash);
                continue;
            }
            if work.judged.contains(&hash) {
                self.unjudged.remove(&hash);
                continue;
            }
            let view = match self.kept_view(entry) {
                Some(view) => view,
                None => match work.views.checked_sub(1) {
                    Some(left) => {
                        work.views = left;
                        self.view_at(entry)
                    }
                    None => {
                        blocked.insert(hash);
                        left_out.push(hash);
                        continue;
                    }
                },
            };
            let verdict = self.judge_in(entry, &view);
            self.unjudged.remove(&hash);
            if let Err(why) = verdict {
                // Whatever batch brought it: those that waited from an earlier one were
                // never judged then, so nobody answered for them yet.
                work.refuse(entry, why);
                blocked.insert(hash);
                refused.push(hash);
            }
        }
        work.left_out += left_out.len();
        for hash in refused.iter().chain(&left_out) {
            self.forget(hash);
        }
    }

    /// Whether `entry`, whose parents are all in the order and judged, is one its author
    /// could make in its own view: whether it applies on top of the group the entries
    /// before it make (see [`Self::insert`]); the reason if not.
    ///
    /// When the entry is signed on every entry new entries are signed on (see
    /// [`Self::heads`]: every entry of the order but those past their author's run, those
    /// of a kind that never applies here and joins), only those are left out of the ones
    /// before it, and they change nothing, not even who came in when or anyone's run (a join
    /// only waits, and a `Welcome` lets a device in only with a join it is signed on): the
    /// group and the runs as this log has them are its author's view. Otherwise that view is worked
    /// out from the entries before it, if `views` allows one more (`None` if not).
    fn judge(&self, entry: &Entry, views: &mut usize) -> Option<Result<(), &'static str>> {
        let on = |tips: &[EntryHash]| tips.iter().all(|h| entry.parents.contains(h));
        // Signed on every entry: the group as this log has it is exactly what it saw. On
        // every head (when they are known without working the group out): all but entries
        // that change nothing.
        let as_here = on(&self.full().tips) || self.cache.heads.get().is_some_and(|h| on(h));
        if as_here {
            // Without the room rules, which judging never reads.
            let state = self.settled_cache().state.clone();
            // That the entries left out change nothing is a rule of the level: one where
            // they do judges on the view before the entry, as below.
            match state.level {
                Level::Two => return Some(self.judge_on(entry, &state, &self.full().budget)),
            }
        }
        *views = views.checked_sub(1)?;
        Some(self.judge_in(entry, &self.view_at(entry)))
    }

    /// [`Self::judge`] on `view`, the group and the runs the entries before `entry` make.
    fn judge_in(&self, entry: &Entry, view: &View) -> Result<(), &'static str> {
        self.judge_on(entry, &view.0, &view.1)
    }

    /// [`Self::judge`] on `view`, the group the entries before `entry` make, and `budget`,
    /// the runs they make: at the level that group judges at (see [`Level`]).
    fn judge_on(
        &self,
        entry: &Entry,
        view: &GroupState,
        budget: &crate::budget::Budget,
    ) -> Result<(), &'static str> {
        match view.level {
            Level::Two => self.judge_at_two(entry, view, budget),
        }
    }

    /// [`Self::judge_on`] as [`Level::Two`] judges it.
    fn judge_at_two(
        &self,
        entry: &Entry,
        view: &GroupState,
        budget: &crate::budget::Budget,
    ) -> Result<(), &'static str> {
        match &entry.body {
            // The first entry, checked as it comes; and one this version cannot judge.
            Body::Create { .. } | Body::Unknown { .. } => return Ok(()),
            _ => {}
        }
        if !entry.body.boxes_hold_one_secret() {
            return Err("a box that holds no epoch secret");
        }
        budget.allows(self, entry)?;
        // A `Welcome` lets a device in only with a `Join` it is signed on: of a view, every
        // entry is before it, but the group as this log has it may hold others.
        let place = &self.full().order.place;
        let before = |h: &EntryHash| self.is_before(place, &entry.parents, h);
        match &entry.body {
            Body::MlsKeyPackage { .. } => {
                if view.is_member(&entry.author) {
                    Ok(())
                } else {
                    Err("author is not a member")
                }
            }
            // Judged as the join it is, until group rules take it.
            Body::MlsJoin { ticket, name, .. } => {
                let mut join = entry.clone();
                join.body = Body::Join {
                    ticket: ticket.clone(),
                    name: name.clone(),
                };
                view.allows(&join, None, &before)
            }
            Body::Commit { .. } | Body::Resync { .. } => {
                let view = crate::chain::CommitView::of(self, view, self.commits_before(entry));
                self.commit_allowed_in(entry, &view)
            }
            Body::Remove { member, rotation } => {
                view.allows(entry, None, &before)?;
                if view.rotation_for_members(rotation, member) {
                    Ok(())
                } else {
                    Err("a removal's new epoch is not for the members left")
                }
            }
            _ => {
                let needed = view
                    .rotation_to_judge(entry)
                    .then(|| !view.leaked_as_judged().is_empty());
                view.allows(entry, needed, &before)
            }
        }
    }

    /// The Commits and Resyncs before `entry`, whose parents are all here.
    fn commits_before(&self, entry: &Entry) -> BTreeSet<EntryHash> {
        self.ancestors_of(&entry.parents)
            .into_iter()
            .filter(|h| {
                matches!(
                    self.entries.get(h).map(|e| &e.body),
                    Some(Body::Commit { .. } | Body::Resync { .. })
                )
            })
            .collect()
    }

    /// The group made by the parents of `entry` and every entry before them: what it saw,
    /// and the runs they make. Kept, with only what judging reads, for a few sets of
    /// parents, since entries written at once share them; worked out whole for a Commit,
    /// whose judgment reads more.
    fn view_at(&self, entry: &Entry) -> Arc<View> {
        if let Some(view) = self.kept_view(entry) {
            return view;
        }
        #[cfg(test)]
        self.views_worked.set(self.views_worked.get() + 1);
        #[cfg(any(test, feature = "counts"))]
        crate::work::viewed();
        let parents = &entry.parents;
        if entry.body.is_mls() {
            return Arc::new(self.view_of(self.ancestors_of(parents).into_iter().collect()));
        }
        let (state, budget) = self.view_of(self.ancestors_of(parents).into_iter().collect());
        let view = Arc::new((state.for_judging(true, true), budget));
        let mut views = self
            .cache
            .views
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if views.len() >= MAX_VIEWS_KEPT {
            views.clear();
        }
        views.insert(parents.clone(), view.clone());
        view
    }

    /// [`Self::view_at`] when it is kept already, which costs nothing to judge on.
    fn kept_view(&self, entry: &Entry) -> Option<Arc<View>> {
        if entry.body.is_mls() {
            return None;
        }
        self.cache
            .views
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&entry.parents)
            .cloned()
    }

    /// Entries whose parents are not all known, in the order they would go if they were.
    pub fn waiting(&self) -> Vec<EntryHash> {
        self.full().waiting.iter().copied().collect()
    }

    /// Entries of the common order past their author's run, or of a kind that never applies
    /// here (one this version does not know, an MLS kind other than Commits and Resyncs),
    /// that no other entry needs: none is signed on them but entries like them. In the
    /// order. They take no part in anything: they never apply, in no round of the losses,
    /// they count in nobody's run (an entry signed on them would count them in its own) and
    /// bring nobody in nor take anybody out (see [`crate::budget`]). So a device may
    /// let them go past their room ([`MAX_SPARE`]), and the group comes out the same with
    /// any of them or none.
    pub fn spare(&self) -> Vec<EntryHash> {
        let needed = self.needed_cache();
        self.full()
            .order
            .hashes
            .iter()
            .filter(|h| !needed.hashes.contains(*h))
            .copied()
            .collect()
    }

    /// The entries of the common order that are not spare ([`Self::spare`]) that no other of
    /// them names as parent: two devices with the same ones have the same group, whatever
    /// spare entries each keeps.
    pub fn needed_heads(&self) -> Vec<EntryHash> {
        self.needed_cache().tips.clone()
    }

    /// Hashes of the entries to send another device that lacks them: those of the common
    /// order that are not spare, in [`Self::hashes_in_order`]'s order, then those waiting
    /// for parents. Spare entries are not sent again (they reach members when written):
    /// they change nothing, and a device past its room would only let them go.
    pub fn hashes_to_send(&self) -> Vec<EntryHash> {
        let needed = self.needed_cache();
        let waiting = &self.full().waiting;
        self.hashes_in_order()
            .into_iter()
            .filter(|h| needed.hashes.contains(h) || waiting.contains(h))
            .collect()
    }

    /// Makes the room a device writes in a run smaller, for tests of what happens past it.
    #[cfg(test)]
    pub(crate) fn set_run_room(&mut self, entries: u32, bytes: usize) {
        self.run_room = RunRoom { entries, bytes };
        self.work_out_again();
        self.cache
            .views
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// Makes the room for spare entries smaller, for tests of what happens past it.
    #[cfg(test)]
    pub(crate) fn set_spare_room(&mut self, per_author: usize, all: usize) {
        self.spare_room = Room {
            per_author,
            author_bytes: SPARE_ROOM.author_bytes,
            all,
            all_bytes: SPARE_ROOM.all_bytes,
        };
    }

    /// Drops waiting entries past their room: always one of the author with the most
    /// waiting, the last to arrive. Which entries wait only delays them (they come again
    /// with the next sync), so devices need not agree on it; what matters is that one
    /// author fills only its share.
    fn forget_old_waiting(&mut self) {
        let waiting = self.waiting();
        // Fewer waiting than an author may have wait: nobody is past the room.
        if waiting.len() <= WAITING_ROOM.per_author.min(WAITING_ROOM.all)
            && waiting.len() * MAX_ENTRY_BYTES
                <= WAITING_ROOM.author_bytes.min(WAITING_ROOM.all_bytes)
        {
            return;
        }
        let arrived = |hash: &EntryHash| self.arrived.get(hash).map_or(u64::MAX, |a| a.0);
        for hash in self.past_room(&waiting, WAITING_ROOM, arrived) {
            self.forget(&hash);
        }
    }

    /// Lets go of spare entries past their room ([`Self::spare`], [`MAX_SPARE`]): those of
    /// an author past its share, and then, while all of them are past the room, those of
    /// the author with the most bytes; the oldest to arrive first, which are the least
    /// likely to be needed by an entry still to come. Returns whether any went.
    ///
    /// The node does this every few seconds, not as entries come: an entry another device
    /// needs may be spare here until the entries after it arrive, in a later part of the
    /// same sync, and let go of at once it would be sent again and again while nothing
    /// after it came. Should the group come out otherwise without them, which spare
    /// entries never make it, they all stay.
    pub fn trim(&mut self) -> bool {
        let room = self.spare_room;
        // Fewer entries than an author may keep spare: nobody is past the room.
        if self.entries.len() <= room.per_author.min(room.all)
            && self.entries.len() * MAX_ENTRY_BYTES <= room.author_bytes.min(room.all_bytes)
        {
            return false;
        }
        let spare = self.spare();
        let oldest_first = |h: &EntryHash| u64::MAX - self.arrived.get(h).map_or(0, |a| a.0);
        let drop = self.past_room(&spare, room, oldest_first);
        if drop.is_empty() {
            return false;
        }
        let before = self.state_cache().clone();
        let taken: Vec<(EntryHash, Entry, Option<Arrival>)> = drop
            .iter()
            .filter_map(|h| Some((*h, self.entries.remove(h)?, self.arrived.remove(h))))
            .collect();
        self.work_out_again();
        let after = self.state_cache();
        if after.applied == before.applied && after.rules == before.rules {
            // Spare entries signed on those that went now wait for them, within their room.
            self.forget_old_waiting();
            return true;
        }
        for (hash, entry, at) in taken {
            self.entries.insert(hash, entry);
            if let Some(at) = at {
                self.arrived.insert(hash, at);
            }
        }
        self.work_out_again();
        false
    }

    /// The entries of `hashes` past `room`, last by `rank` and then by hash: for each author
    /// past its share, its last, and then, while all of them together are past the room,
    /// the last of the author with the most bytes.
    fn past_room(
        &self,
        hashes: &[EntryHash],
        room: Room,
        rank: impl Fn(&EntryHash) -> u64,
    ) -> Vec<EntryHash> {
        type Kept = (Vec<(u64, EntryHash, usize)>, usize);
        let mut by_author: BTreeMap<DevicePublicKey, Kept> = BTreeMap::new();
        let (mut total, mut total_bytes) = (0, 0);
        for hash in hashes {
            let entry = &self.entries[hash];
            let bytes = self
                .arrived
                .get(hash)
                .map_or_else(|| entry.encode().len(), |a| a.1);
            let (list, sum) = by_author.entry(entry.author).or_default();
            list.push((rank(hash), *hash, bytes));
            *sum += bytes;
            total += 1;
            total_bytes += bytes;
        }
        let mut drop = Vec::new();
        for (list, sum) in by_author.values_mut() {
            list.sort_unstable();
            while list.len() > room.per_author || *sum > room.author_bytes {
                let Some((_, hash, bytes)) = list.pop() else {
                    break;
                };
                drop.push(hash);
                *sum -= bytes;
                total -= 1;
                total_bytes -= bytes;
            }
        }
        while total > room.all || total_bytes > room.all_bytes {
            let Some((list, sum)) = by_author
                .values_mut()
                .max_by_key(|(list, sum)| (*sum, list.len()))
            else {
                break;
            };
            let Some((_, hash, bytes)) = list.pop() else {
                break;
            };
            drop.push(hash);
            *sum -= bytes;
            total -= 1;
            total_bytes -= bytes;
        }
        drop
    }

    /// Takes an entry out of the log.
    fn forget(&mut self, hash: &EntryHash) {
        self.entries.remove(hash);
        self.arrived.remove(hash);
        self.unjudged.remove(hash);
        self.forgot(hash);
    }

    /// Bytes of entry `hash` as encoded.
    pub(crate) fn encoded_len(&self, hash: &EntryHash) -> usize {
        self.arrived
            .get(hash)
            .map_or_else(|| self.entries[hash].encode().len(), |a| a.1)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn hashes(&self) -> impl Iterator<Item = &EntryHash> {
        self.entries.keys()
    }

    pub fn get(&self, hash: &EntryHash) -> Option<&Entry> {
        self.entries.get(hash)
    }

    /// The entries of the common order that are not spare, in it, then the spare ones
    /// ([`Self::spare`]) in it, then those waiting for parents by their own time: the order
    /// to store and send a log in, so that whoever reads it back never has an entry wait
    /// for a parent that comes later in the same batch.
    pub fn hashes_in_order(&self) -> Vec<EntryHash> {
        let full = self.full();
        let needed = self.needed_cache();
        let (mut out, spare): (Vec<EntryHash>, Vec<EntryHash>) = full
            .order
            .hashes
            .iter()
            .partition(|h| needed.hashes.contains(*h));
        out.extend(spare);
        if full.waiting.is_empty() {
            return out;
        }
        let mut waiting: Vec<EntryHash> = full.waiting.iter().copied().collect();
        waiting.sort_by_key(|h| (self.entries[h].time_ms, *h));
        out.extend(waiting);
        out
    }

    /// Entries in [`Self::hashes_in_order`]'s order.
    pub fn entries_in_order(&self) -> Vec<&Entry> {
        self.hashes_in_order()
            .iter()
            .map(|h| &self.entries[h])
            .collect()
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Entries in the common order that no other entry in it names as parent: what a new
    /// entry is signed on. An entry still waiting for parents is never one, or an entry
    /// naming a parent nobody has would make every entry written after it wait too, a
    /// removal of its author included.
    ///
    /// Nor is an entry past its author's run, of a kind that never applies here, or a join
    /// (the `Welcome` that lets its device in is signed on it, see [`Self::let_in`]): an entry
    /// signed on it would make every member keep it, past the room for spare entries, and
    /// count it in its own run (see [`crate::budget`]). Every other
    /// entry is a head, whether or not it applies: it takes part in the order, the losses or
    /// the runs, so an entry signed on all the heads saw everything it is judged on (see
    /// [`Self::judge`]). At most [`MAX_PARENTS`] at once: those left out are named by the
    /// next entry.
    pub fn heads(&self) -> Vec<EntryHash> {
        self.heads_cache()
            .iter()
            .take(MAX_PARENTS)
            .copied()
            .collect()
    }

    /// The `live` entries of `ordered` (part of a run, see [`crate::budget`]) that no other
    /// names as parent, in the order; new entries are signed on the first [`MAX_PARENTS`]:
    /// those left out are named by the next entry written.
    pub(crate) fn live_tips(
        &self,
        ordered: &[EntryHash],
        live: &crate::budget::Entries,
    ) -> Vec<EntryHash> {
        let live: Vec<&EntryHash> = ordered.iter().filter(|h| live.contains(*h)).collect();
        let referenced: BTreeSet<&EntryHash> = live
            .iter()
            .flat_map(|h| self.entries[*h].parents.iter())
            .collect();
        live.into_iter()
            .filter(|h| !referenced.contains(h))
            .copied()
            .collect()
    }

    /// Entries in the common order that no other in it names as parent, those that apply to
    /// nothing included and those waiting for parents left out: two devices with the same
    /// ones have the same entries in the order, whatever waits on each.
    pub fn placed_heads(&self) -> Vec<EntryHash> {
        self.full().tips.clone()
    }

    /// Entries no other known entry names as parent, waiting ones included: two devices
    /// with the same ones have the same entries.
    pub fn stored_heads(&self) -> Vec<EntryHash> {
        self.stored_heads_of(|_| true)
    }

    fn stored_heads_of(&self, include: impl Fn(&Entry) -> bool) -> Vec<EntryHash> {
        let referenced: BTreeSet<&EntryHash> = self
            .entries
            .values()
            .filter(|e| include(e))
            .flat_map(|e| e.parents.iter())
            .collect();
        self.entries
            .iter()
            .filter(|(h, e)| include(e) && !referenced.contains(h))
            .map(|(h, _)| *h)
            .collect()
    }

    /// The common order: an entry comes after all its parents; among entries that are ready
    /// at the same time, those of the members that came in last go first, so that of two
    /// changes made at once without seeing each other, the one of the member that came in
    /// first applies last and stands; then the smallest hash. Entries whose parents are
    /// unknown wait.
    pub fn ordered(&self) -> Vec<EntryHash> {
        self.full().order.hashes.clone()
    }

    /// The group as the log says, applying the entries in the common order. An entry signed
    /// by a device on what it saw before it was removed, demoted or left, and so neither
    /// before nor after that loss, is refused: otherwise it could be made to sort first and
    /// apply with rights its author no longer had. A demotion refuses only the entries that
    /// need a place on the ladder ([`Body::needs_rank`]). The same goes for a join with an
    /// invite of such a device. Removals win.
    ///
    /// Room rules apply last (see [`GroupState::apply_rule`]), on top of what every other
    /// entry made of the group; no loss outranks them.
    pub fn state(&self) -> GroupState {
        GroupState::clone(self.state_cache())
    }

    /// [`Self::state`], shared: worked out once until the log changes, and then mostly
    /// carried forward entry by entry rather than worked out again.
    pub fn shared_state(&self) -> Arc<GroupState> {
        self.state_cache().clone()
    }

    /// The group without its room rules (see [`Self::state`]), and the entries outranked,
    /// each with the loss that outranks it.
    pub(crate) fn settled(&self) -> (&GroupState, &BTreeMap<EntryHash, EntryHash>) {
        let settled = self.settled_cache();
        (&settled.state, &settled.outranked)
    }

    /// Whether the author of `Rotate` entry `hash` had a reason to rotate in the log as it
    /// was when it wrote the entry (everything before it): it managed the group, or a device that
    /// is no longer a member held the current secret. Worked out once per entry.
    pub(crate) fn rotation_needed(&self, hash: &EntryHash) -> bool {
        let lock = |judged: &Mutex<BTreeMap<EntryHash, bool>>| {
            judged
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(hash)
                .copied()
        };
        if let Some(needed) = lock(&self.cache.judged) {
            return needed;
        }
        self.rotation_needed_on(hash, &self.state_before(hash))
    }

    /// [`Self::rotation_needed`] with `view` the group as the entry saw it, unless it was
    /// judged already.
    pub(crate) fn rotation_needed_on(&self, hash: &EntryHash, view: &GroupState) -> bool {
        let mut judged = self
            .cache
            .judged
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let author = self.entries[hash].author;
        *judged
            .entry(*hash)
            .or_insert_with(|| view.is_manager(&author) || !view.leaked_as_judged().is_empty())
    }

    /// The group as entry `hash` saw it: the state of the log made of everything before it
    /// (its parents, theirs, and so on). It depends only on the entry, which its hash fixes.
    pub(crate) fn state_before(&self, hash: &EntryHash) -> GroupState {
        self.state_of(self.ancestors(hash).into_iter().collect())
    }

    /// Every entry before entry `hash`: its parents, theirs, and so on.
    pub(crate) fn ancestors(&self, hash: &EntryHash) -> BTreeSet<EntryHash> {
        self.reach(*hash, |h| self.entries.get(h).map(|e| e.parents.clone()))
    }

    /// `parents` and every entry before them.
    fn ancestors_of(&self, parents: &[EntryHash]) -> BTreeSet<EntryHash> {
        let mut seen: BTreeSet<EntryHash> = parents.iter().copied().collect();
        let mut todo = parents.to_vec();
        while let Some(hash) = todo.pop() {
            for parent in self.entries.get(&hash).map_or(&[][..], |e| &e.parents) {
                if seen.insert(*parent) {
                    todo.push(*parent);
                }
            }
        }
        seen
    }

    /// Every entry after entry `hash`: those signed on it, on those, and so on.
    pub(crate) fn descendants(&self, hash: &EntryHash) -> BTreeSet<EntryHash> {
        let mut children: BTreeMap<EntryHash, Vec<EntryHash>> = BTreeMap::new();
        for (h, entry) in &self.entries {
            for parent in &entry.parents {
                children.entry(*parent).or_default().push(*h);
            }
        }
        self.reach(*hash, |h| children.get(h).cloned())
    }

    /// Every entry reached from `from` by following `next` (parents or children).
    fn reach(
        &self,
        from: EntryHash,
        next: impl Fn(&EntryHash) -> Option<Vec<EntryHash>>,
    ) -> BTreeSet<EntryHash> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![from];
        while let Some(hash) = todo.pop() {
            for other in next(&hash).unwrap_or_default() {
                if seen.insert(other) {
                    todo.push(other);
                }
            }
        }
        seen
    }

    /// Every epoch secret this device was given, checked against the log's commitments.
    pub fn secrets(&self, identity: &DeviceIdentity) -> BTreeMap<EpochId, GroupSecret> {
        self.secrets_in(identity, &self.state())
    }

    /// [`Self::secrets`] with this log's `state`, worked out already.
    fn secrets_in(
        &self,
        identity: &DeviceIdentity,
        state: &GroupState,
    ) -> BTreeMap<EpochId, GroupSecret> {
        let me = identity.public_key();
        let mut out = BTreeMap::new();
        for hash in &state.applied {
            let mut take = |epoch: &EpochId, sealed: &Sealed| {
                let Some(valid) = state.epoch(epoch) else {
                    return;
                };
                let Some(secret) = self.open_box(hash, epoch, sealed, identity) else {
                    return;
                };
                if commitment(epoch, &secret) == valid.commitment
                    && let Ok(secret) = GroupSecret::from_bytes(secret.as_bytes())
                {
                    out.insert(*epoch, secret);
                }
            };
            match &self.entries[hash].body {
                Body::Create {
                    start: rotation, ..
                }
                | Body::Remove { rotation, .. }
                | Body::Rotate { rotation } => {
                    if let Some((_, sealed)) = rotation.sealed.iter().find(|(k, _)| *k == me) {
                        take(&rotation.epoch, sealed);
                    }
                }
                Body::Add {
                    member,
                    epoch,
                    secret,
                    ..
                }
                | Body::Welcome {
                    member,
                    epoch,
                    secret,
                } if *member == me => take(epoch, secret),
                _ => {}
            }
        }
        out
    }

    /// Why this device lacks the current epoch's secret, with `state` this log's: the
    /// device that wrote the last entry meant to give it (a rotation, an `Add` or a
    /// `Welcome`) and whether that entry held a box for it that does not open to the
    /// secret, or the device that started the epoch without a box for it. `None` while
    /// it has the secret, or before the group has an epoch.
    pub fn current_key_problem(
        &self,
        identity: &DeviceIdentity,
        state: &GroupState,
    ) -> Option<KeyProblem> {
        let current = state.current_epoch()?.id;
        if self.secrets_in(identity, state).contains_key(&current) {
            return None;
        }
        let me = identity.public_key();
        let mut problem = None;
        for hash in &state.applied {
            let entry = &self.entries[hash];
            match &entry.body {
                Body::Create {
                    start: rotation, ..
                }
                | Body::Remove { rotation, .. }
                | Body::Rotate { rotation }
                    if rotation.epoch == current =>
                {
                    problem = Some(KeyProblem {
                        by: entry.author,
                        bad_box: rotation.sealed.iter().any(|(k, _)| *k == me),
                    });
                }
                Body::Add { member, epoch, .. } | Body::Welcome { member, epoch, .. }
                    if *member == me && *epoch == current =>
                {
                    problem = Some(KeyProblem {
                        by: entry.author,
                        bad_box: true,
                    });
                }
                _ => {}
            }
        }
        problem
    }

    /// Keys of the current epoch, if this device has its secret.
    pub fn current_keys(&self, identity: &DeviceIdentity) -> Option<GroupKeys> {
        let state = self.state();
        let current = state.current_epoch()?;
        let secret = self.secrets_in(identity, &state).remove(&current.id)?;
        Some(GroupKeys::derive(
            self.group_id,
            epoch_number(&current.id),
            &secret,
        ))
    }

    /// Keys of every epoch this device can read, by epoch number.
    pub fn all_keys(&self, identity: &DeviceIdentity) -> BTreeMap<u32, GroupKeys> {
        self.secrets(identity)
            .into_iter()
            .map(|(id, secret)| {
                let n = epoch_number(&id);
                (n, GroupKeys::derive(self.group_id, n, &secret))
            })
            .collect()
    }

    /// The group's state, [`Self::all_keys`] and [`Self::current_keys`] at once: the log is
    /// gone through and the secrets opened once, not once each.
    pub fn state_and_keys(
        &self,
        identity: &DeviceIdentity,
    ) -> (GroupState, BTreeMap<u32, GroupKeys>, Option<GroupKeys>) {
        let state = self.state();
        let secrets = self.secrets_in(identity, &state);
        let current = state.current_epoch().and_then(|current| {
            let secret = secrets.get(&current.id)?;
            Some(GroupKeys::derive(
                self.group_id,
                epoch_number(&current.id),
                secret,
            ))
        });
        let all = secrets
            .iter()
            .map(|(id, secret)| {
                let n = epoch_number(id);
                (n, GroupKeys::derive(self.group_id, n, secret))
            })
            .collect();
        (state, all, current)
    }

    /// Signs `body` on top of the current heads and adds it. Names in it are signed as they
    /// show ([`tidy`]).
    pub fn append(
        &mut self,
        identity: &DeviceIdentity,
        body: Body,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let body = match body {
            Body::Rename { name } => Body::Rename { name: tidy(&name) },
            Body::SetRoom { id, name } => Body::SetRoom {
                id,
                name: tidy(&name),
            },
            Body::Add {
                member,
                name,
                epoch,
                secret,
            } => Body::Add {
                member,
                name: tidy(&name),
                epoch,
                secret,
            },
            // A group's level goes up in one place, which checks it can.
            Body::Level { .. } => return Err(StateError::NotAllowed(RAISE_LEVEL_ONLY)),
            body => body,
        };
        self.append_on(identity, self.heads(), body, now_ms)
    }

    /// [`Self::append`] on `parents`.
    fn append_on(
        &mut self,
        identity: &DeviceIdentity,
        parents: Vec<EntryHash>,
        body: Body,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let entry = Entry::sign(identity, self.group_id, parents, now_ms, body);
        self.insert(entry.clone())?;
        let hash = entry.hash();
        let refused = self
            .state_cache()
            .rejected
            .iter()
            .rev()
            .find(|(h, _)| *h == hash)
            .map(|(_, reason)| *reason);
        if let Some(reason) = refused {
            self.forget(&hash);
            return Err(StateError::NotAllowed(reason));
        }
        Ok(entry)
    }

    fn current_secret(
        &self,
        identity: &DeviceIdentity,
    ) -> Result<(EpochId, GroupSecret), StateError> {
        let state = self.state();
        let current = state.current_epoch().ok_or(StateError::NoSecret)?.id;
        let secret = self
            .secrets_in(identity, &state)
            .remove(&current)
            .ok_or(StateError::NoSecret)?;
        Ok((current, secret))
    }

    /// Manager: adds a device whose key is known, giving it the current secret.
    pub fn add(
        &mut self,
        identity: &DeviceIdentity,
        member: DevicePublicKey,
        name: &str,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let (epoch, secret) = self.current_secret(identity)?;
        let sealed = Sealed::seal(
            &member,
            &secret_context(&self.group_id, &epoch),
            secret.as_bytes(),
        )?;
        self.append(
            identity,
            Body::Add {
                member,
                name: name.to_string(),
                epoch,
                secret: sealed,
            },
            now_ms,
        )
    }

    /// Manager: gives the current secret to a member who lacks it, or lets in a device that
    /// joined with a ticket: signed on its first `Join` that can still let it in.
    pub fn welcome(
        &mut self,
        identity: &DeviceIdentity,
        member: DevicePublicKey,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let join = self.state_cache().waiting_join_entry(&member);
        self.welcome_on(identity, member, join, now_ms)
    }

    /// Manager: lets in the device that signed `Join` entry `join`, kept here, with a
    /// `Welcome` signed on it: that join applies, and uses its ticket.
    pub fn let_in(
        &mut self,
        identity: &DeviceIdentity,
        join: &EntryHash,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let member = match self.entries.get(join) {
            Some(entry) if matches!(entry.body, Body::Join { .. }) => entry.author,
            _ => return Err(StateError::NotAllowed(crate::state::NO_JOIN_TO_WELCOME)),
        };
        self.welcome_on(identity, member, Some(*join), now_ms)
    }

    fn welcome_on(
        &mut self,
        identity: &DeviceIdentity,
        member: DevicePublicKey,
        join: Option<EntryHash>,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        // A ticket expires by the clock of the manager that lets its device in: the join's
        // own time is the joining device's to choose.
        if let Some(join) = join
            && let Some(Body::Join { ticket, .. }) = self.entries.get(&join).map(|e| &e.body)
            && now_ms > ticket.expires_ms
        {
            return Err(StateError::NotAllowed(TICKET_EXPIRED));
        }
        let (epoch, secret) = self.current_secret(identity)?;
        let sealed = Sealed::seal(
            &member,
            &secret_context(&self.group_id, &epoch),
            secret.as_bytes(),
        )?;
        // A join is no head (nothing else is signed on it), so it is named here. One signed
        // on entries nobody keeps (past a run, of a kind that never applies, a join) would
        // make every member keep them, in this device's run: a device that wants in signs on
        // the heads it was offered.
        let live = &self.full().budget.live;
        if let Some(join) = join
            && self.entries[&join]
                .parents
                .iter()
                .any(|p| !live.contains(p))
        {
            return Err(StateError::NotAllowed(JOIN_ON_SPARE));
        }
        let mut parents = self.heads();
        if let Some(join) = join {
            parents.truncate(MAX_PARENTS - 1);
            parents.push(join);
        }
        self.append_on(
            identity,
            parents,
            Body::Welcome {
                member,
                epoch,
                secret: sealed,
            },
            now_ms,
        )
    }

    /// Removes a device below this one on the ladder and starts a new epoch for everyone
    /// else.
    pub fn remove(
        &mut self,
        identity: &DeviceIdentity,
        member: DevicePublicKey,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let state = self.state();
        let remaining = state.members.keys().filter(|k| **k != member).copied();
        let (rotation, _) = Rotation::new(&self.group_id, remaining)?;
        self.append(identity, Body::Remove { member, rotation }, now_ms)
    }

    /// The level the group is judged at after everything here (see [`Level`]).
    pub fn level(&self) -> Level {
        self.settled().0.level
    }

    /// Raises the group to `level`: what comes after is judged by that level's rules.
    /// Managers only, and only once every member's copy judges at it, or those below it
    /// stop at this entry.
    pub fn raise_level(
        &mut self,
        identity: &DeviceIdentity,
        level: Level,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        let body = Body::Level {
            level: level.number(),
        };
        self.append_on(identity, self.heads(), body, now_ms)
    }

    /// Starts a new epoch: a manager at any time; another member after someone left who
    /// still holds the current secret (other rotations of members are ignored).
    pub fn rotate(&mut self, identity: &DeviceIdentity, now_ms: u64) -> Result<Entry, StateError> {
        let state = self.state();
        let (rotation, _) = Rotation::new(&self.group_id, state.members.keys().copied())?;
        self.append(identity, Body::Rotate { rotation }, now_ms)
    }

    /// The ticket a member last joined with, if it joined with one.
    pub fn join_ticket_of(&self, member: &DevicePublicKey) -> Option<&Ticket> {
        let state = self.state();
        state
            .applied
            .iter()
            .rev()
            .find_map(|h| match &self.entries[h].body {
                Body::Join { ticket, .. } if self.entries[h].author == *member => Some(ticket),
                _ => None,
            })
    }

    /// Whether the manager who issued `ticket` noted it here as an open invite. Only the real
    /// group has that manager's own entry, so a joining device can tell the log it is given
    /// is the group it was invited to and not one made up under the same id.
    pub fn invited_with(&self, ticket: &Ticket) -> bool {
        self.state().applied.iter().any(|h| {
            let entry = &self.entries[h];
            entry.author == ticket.issuer
                && matches!(&entry.body, Body::Invite { ticket: t, .. } if t == ticket)
        })
    }

    /// The issuing manager's own `Revoke` of `ticket`, when it applies here: the refusal a
    /// device holding that ticket can check ([`Ticket::revoked_by`]).
    pub fn issuer_revoke(&self, ticket: &Ticket) -> Option<&Entry> {
        self.state()
            .applied
            .iter()
            .map(|h| &self.entries[h])
            .find(|e| ticket.revoked_by(e))
    }

    /// Invites `issuer` noted here that another member cancelled, that `issuer` has not
    /// cancelled itself and that nobody joined with. The invited device only believes a
    /// refusal its issuer signed, so the issuer cancels them too.
    pub fn revokes_to_confirm(&self, issuer: &DevicePublicKey) -> Vec<[u8; 16]> {
        let state = self.state();
        let applied: Vec<&Entry> = state.applied.iter().map(|h| &self.entries[h]).collect();
        let revoked = |ticket_id: &[u8; 16], by_issuer: bool| {
            applied.iter().any(|e| {
                (e.author == *issuer) == by_issuer
                    && matches!(&e.body, Body::Revoke { ticket_id: t } if t == ticket_id)
            })
        };
        let used = |ticket_id: &[u8; 16]| {
            applied.iter().any(
                |e| matches!(&e.body, Body::Join { ticket, .. } if ticket.ticket_id == *ticket_id),
            )
        };
        applied
            .iter()
            .filter_map(|e| match &e.body {
                Body::Invite { ticket, .. } if e.author == *issuer => Some(ticket.ticket_id),
                _ => None,
            })
            .filter(|t| revoked(t, false) && !revoked(t, true) && !used(t))
            .collect()
    }

    /// Invites `issuer` noted here, still open at `now_ms`, after which a device that managed
    /// the group was made to stop managing it: it was demoted or removed, or the managers
    /// were set to the first few (which may leave out some). That device keeps the log as it
    /// was before, the invite in it, and can answer under the invite's token as the manager
    /// it is there: the invited device, which knows the group only by the issuer's note of
    /// the invite, would take that log for the group, and be let in where only that device
    /// answers. Cancelled by the issuer, the invite brings the invited device a refusal it
    /// believes from any member of the group. A manager that leaves by itself is not
    /// counted: where everyone manages, as groups start, every leave would cancel every
    /// invite. It changes nothing of what the log takes.
    pub fn invites_a_manager_left_with(
        &self,
        issuer: &DevicePublicKey,
        now_ms: u64,
    ) -> Vec<[u8; 16]> {
        let state = self.state();
        let at: BTreeMap<&EntryHash, usize> = state
            .applied
            .iter()
            .enumerate()
            .map(|(i, h)| (h, i))
            .collect();
        let fewer_managers = state
            .applied
            .iter()
            .rposition(|h| matches!(&self.entries[h].body, Body::SetManagers { first: Some(_) }));
        let Some(last) = state
            .manager_losses()
            .filter(|(device, entry)| {
                *device != issuer && !matches!(self.entries[*entry].body, Body::Leave)
            })
            .filter_map(|(_, entry)| at.get(entry).copied())
            .chain(fewer_managers)
            .max()
        else {
            return Vec::new();
        };
        state.applied[..last]
            .iter()
            .map(|h| &self.entries[h])
            .filter_map(|e| match &e.body {
                Body::Invite { ticket, .. }
                    if e.author == *issuer
                        && state
                            .open_invites
                            .get(&ticket.ticket_id)
                            .is_some_and(|(_, expires)| now_ms <= *expires) =>
                {
                    Some(ticket.ticket_id)
                }
                _ => None,
            })
            .collect()
    }

    /// What a device that was removed may still be shown: the entry that removed it and
    /// everything before it, in order. Nothing for a device that left by itself or is still
    /// in, and nothing that happened in the group after.
    pub fn until_removed(&self, device: &DevicePublicKey) -> Vec<&Entry> {
        let state = self.state();
        if state.is_member(device) {
            return Vec::new();
        }
        let Some(removal) = state.applied.iter().rev().find(
            |h| matches!(&self.entries[*h].body, Body::Remove { member, .. } if member == device),
        ) else {
            return Vec::new();
        };
        let mut before = BTreeSet::from([*removal]);
        let mut next = vec![*removal];
        while let Some(hash) = next.pop() {
            for parent in &self.entries[&hash].parents {
                if before.insert(*parent) {
                    next.push(*parent);
                }
            }
        }
        self.ordered()
            .iter()
            .filter(|h| before.contains(*h))
            .map(|h| &self.entries[h])
            .collect()
    }

    /// Manager: who may enter voice room `room`, everyone (`None`) or only `members`, this
    /// device and those above it.
    pub fn set_room_access(
        &mut self,
        identity: &DeviceIdentity,
        room: VoiceRoomId,
        members: Option<Vec<DevicePublicKey>>,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        self.append(identity, Body::RoomAccess { room, members }, now_ms)
    }

    /// Keeps `member`, below this device on the ladder, out of voice room `room` until
    /// `until_ms`, or until someone above it lets it back (`None`).
    pub fn keep_out(
        &mut self,
        identity: &DeviceIdentity,
        room: VoiceRoomId,
        member: DevicePublicKey,
        until_ms: Option<u64>,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        self.append(
            identity,
            Body::KeepOut {
                room,
                member,
                until_ms,
            },
            now_ms,
        )
    }

    /// Lets `member`, below this device, back into voice room `room` before its time out
    /// runs out.
    pub fn let_back(
        &mut self,
        identity: &DeviceIdentity,
        room: VoiceRoomId,
        member: DevicePublicKey,
        now_ms: u64,
    ) -> Result<Entry, StateError> {
        self.append(identity, Body::LetBack { room, member }, now_ms)
    }

    /// Manager: an invite ticket valid until `expires_ms`.
    pub fn issue_ticket(
        &self,
        identity: &DeviceIdentity,
        expires_ms: u64,
    ) -> Result<Ticket, StateError> {
        if !self.state().is_manager(&identity.public_key()) {
            return Err(StateError::NotAllowed("only managers invite"));
        }
        Ok(Ticket::issue(identity, self.group_id, expires_ms)?)
    }

    /// Manager: an invite valid until `expires_ms`, noted in the log so that every manager
    /// announces itself under the invite's token while it is open. Returns the ticket and
    /// the token, which go in the invite link.
    pub fn invite(
        &mut self,
        identity: &DeviceIdentity,
        expires_ms: u64,
        now_ms: u64,
    ) -> Result<(Ticket, InviteToken), StateError> {
        let ticket = self.issue_ticket(identity, expires_ms)?;
        let mut token = [0u8; 16];
        fogata_crypto::random_bytes(&mut token)?;
        self.append(
            identity,
            Body::Invite {
                ticket: ticket.clone(),
                token,
            },
            now_ms,
        )?;
        Ok((ticket, token))
    }
}

mod verdicts;

pub use verdicts::Verdicts;

#[cfg(test)]
mod tests {
    mod hostile;
    mod level;

    use super::*;
    use crate::{
        budget::PAST_RUN,
        entry::FIRST_ROOM,
        record::MAX_RECORD_AHEAD_MS,
        state::{
            Barrier, MAX_MEMBERS, MAX_PICTURE_BYTES, MAX_ROOMS, Managers, OUTRANKED,
            ROTATE_NOT_NEEDED, WELCOME_NOT_MANAGER,
        },
    };

    struct Device {
        id: DeviceIdentity,
    }

    impl Device {
        fn new() -> Self {
            Self {
                id: DeviceIdentity::generate().unwrap(),
            }
        }

        fn key(&self) -> DevicePublicKey {
            self.id.public_key()
        }
    }

    fn join(log: &mut GroupLog, joiner: &Device, ticket: Ticket, now: u64) -> Entry {
        let entry = Entry::sign(
            &joiner.id,
            log.group_id(),
            log.heads(),
            now,
            Body::Join {
                ticket,
                name: "joiner".into(),
            },
        );
        // Kept only if the joiner could make it (see `GroupLog::insert`).
        let _ = log.insert(entry.clone());
        entry
    }

    /// Copies every entry of `from` into `to`, like a sync between two members.
    fn sync(from: &GroupLog, to: &mut GroupLog) {
        for e in from.entries() {
            to.insert(e.clone()).unwrap();
        }
    }

    #[test]
    fn state_and_keys_agree_with_asking_for_each() {
        let ana = Device::new();
        let bea = Device::new();
        let leo = Device::new();
        let mut log = GroupLog::create(&ana.id, "friends", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        log.rotate(&ana.id, 4).unwrap();
        log.remove(&ana.id, leo.key(), 5).unwrap();
        for device in [&ana, &bea, &leo] {
            let (state, all, current) = log.state_and_keys(&device.id);
            assert_eq!(state.applied, log.state().applied);
            assert_eq!(state.members, log.state().members);
            let alone = log.all_keys(&device.id);
            let secrets = |keys: &BTreeMap<u32, GroupKeys>| -> Vec<(u32, [u8; 32])> {
                keys.iter()
                    .map(|(n, k)| (*n, *k.rendezvous_secret()))
                    .collect()
            };
            assert_eq!(secrets(&all), secrets(&alone));
            assert_eq!(
                current.map(|k| (k.epoch(), *k.rendezvous_secret())),
                log.current_keys(&device.id)
                    .map(|k| (k.epoch(), *k.rendezvous_secret()))
            );
        }
        // Bea has the three epochs; Leo, removed, never gets the last one.
        assert_eq!(log.state_and_keys(&bea.id).1.len(), 3);
        assert_eq!(log.state_and_keys(&leo.id).1.len(), 2);
        assert!(log.state_and_keys(&leo.id).2.is_none());
    }

    #[test]
    fn creator_has_the_first_key() {
        let ana = Device::new();
        let log = GroupLog::create(&ana.id, "friends", 1).unwrap();
        let state = log.state();
        assert_eq!(state.name, "friends");
        assert!(state.is_manager(&ana.key()));
        assert!(log.current_keys(&ana.id).is_some());
        assert!(state.lacking_current_secret().is_empty());
    }

    #[test]
    fn a_manager_adds_a_member_who_then_holds_the_key() {
        let ana = Device::new();
        let luis = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        let a = log.current_keys(&ana.id).unwrap();
        let l = log.current_keys(&luis.id).unwrap();
        assert_eq!(a.epoch(), l.epoch());
        assert_eq!(a.rendezvous_point(1), l.rendezvous_point(1));
        // At the end of the ladder; everyone manages until someone says otherwise.
        let state = log.state();
        assert_eq!(state.ladder, [ana.key(), luis.key()]);
        assert!(state.is_manager(&luis.key()));
    }

    /// A weak key no device has, added as a member by a manager whose version writes it
    /// anyway (this one does not: nothing seals to it): the log takes it, as every version
    /// does, and new epochs and removals still go on, with no box for it.
    #[test]
    fn a_weak_key_added_as_a_member_keeps_no_epoch_from_starting() {
        let ana = Device::new();
        let bea = Device::new();
        let leo = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "leo", 3).unwrap();
        let mut small = [0u8; 32];
        small[0] = 1;
        let weak = DevicePublicKey::from_bytes(&small).unwrap();
        assert!(weak.is_weak());
        assert!(log.add(&ana.id, weak, "nobody", 4).is_err());
        let (epoch, _) = log.current_secret(&ana.id).unwrap();
        let sealed = Sealed::seal(&ana.key(), b"anything", &[0; 32]).unwrap();
        log.append(
            &ana.id,
            Body::Add {
                member: weak,
                name: "nobody".into(),
                epoch,
                secret: sealed,
            },
            4,
        )
        .unwrap();
        assert!(log.state().is_member(&weak));
        let rotated = log.rotate(&ana.id, 5).unwrap();
        let Body::Rotate { rotation } = &rotated.body else {
            unreachable!()
        };
        assert!(rotation.sealed.iter().all(|(k, _)| *k != weak));
        assert_eq!(rotation.sealed.len(), 3);
        log.remove(&ana.id, leo.key(), 6).unwrap();
        assert!(log.current_keys(&bea.id).is_some());
        assert!(log.current_keys(&leo.id).is_none());
        // And the weak key itself goes like any member.
        log.remove(&ana.id, weak, 7).unwrap();
        assert!(!log.state().is_member(&weak));
    }

    #[test]
    fn join_with_ticket_then_welcome() {
        let ana = Device::new();
        let eva = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        let join = join(&mut log, &eva, ticket.clone(), 50);
        // Kept, but in only with a manager's `Welcome`, which uses the ticket.
        assert!(log.get(&join.hash()).is_some());
        let state = log.state();
        assert!(!state.is_member(&eva.key()));
        assert_eq!(state.waiting_join(&eva.key()), Some(&ticket));
        assert!(!state.used_tickets.contains(&ticket.ticket_id));
        assert!(log.current_keys(&eva.id).is_none());
        let welcome = log.welcome(&ana.id, eva.key(), 51).unwrap();
        assert!(welcome.parents.contains(&join.hash()));
        let state = log.state();
        assert!(state.is_member(&eva.key()) && state.keyed(&eva.key()));
        assert!(state.used_tickets.contains(&ticket.ticket_id));
        assert_eq!(state.waiting_join(&eva.key()), None);
        let at = |h: EntryHash| state.applied.iter().position(|a| *a == h).unwrap();
        assert_eq!(at(join.hash()) + 1, at(welcome.hash()));
        assert!(log.current_keys(&eva.id).is_some());
        assert!(state.lacking_current_secret().is_empty());
    }

    #[test]
    fn tickets_are_single_use_expiring_and_from_managers() {
        let ana = Device::new();
        let luis = Device::new();
        let eva = Device::new();
        let mallory = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        assert!(log.issue_ticket(&luis.id, 100).is_err());

        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        join(&mut log, &eva, ticket.clone(), 50);
        join(&mut log, &mallory, ticket, 60);
        let _ = log.welcome(&ana.id, eva.key(), 61);
        let _ = log.welcome(&ana.id, mallory.key(), 62);
        let state = log.state();
        assert!(state.is_member(&eva.key()));
        assert!(!state.is_member(&mallory.key()));

        let late = log.issue_ticket(&ana.id, 100).unwrap();
        let other = Device::new();
        join(&mut log, &other, late, 101);
        assert!(log.welcome(&ana.id, other.key(), 102).is_err());
        assert!(!log.state().is_member(&other.key()));

        // A ticket signed by a non-admin is ignored.
        let forged = Ticket::issue(&luis.id, log.group_id(), 1000).unwrap();
        let sneaky = Device::new();
        join(&mut log, &sneaky, forged, 10);
        assert!(log.welcome(&ana.id, sneaky.key(), 103).is_err());
        assert!(!log.state().is_member(&sneaky.key()));
    }

    #[test]
    fn a_ticket_expires_by_the_clock_of_the_manager_letting_in() {
        let ana = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // The joining device's clock is ahead: its join is dated after the expiry, which
        // the manager's clock has not reached.
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        let ahead = Device::new();
        let entry = join(&mut log, &ahead, ticket, 5_000);
        assert!(log.get(&entry.hash()).is_some());
        log.let_in(&ana.id, &entry.hash(), 90).unwrap();
        assert!(log.state().is_member(&ahead.key()));
        // A join dated before the expiry, let in once the manager's clock passed it.
        let ticket = log.issue_ticket(&ana.id, 200).unwrap();
        let late = Device::new();
        let entry = join(&mut log, &late, ticket, 150);
        assert_eq!(
            log.let_in(&ana.id, &entry.hash(), 201).unwrap_err(),
            StateError::NotAllowed(TICKET_EXPIRED)
        );
        assert!(log.welcome(&ana.id, late.key(), 201).is_err());
        assert!(!log.state().is_member(&late.key()));
    }

    #[test]
    fn removed_member_gets_no_new_key() {
        let ana = Device::new();
        let luis = Device::new();
        let eva = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        log.add(&ana.id, eva.key(), "eva", 3).unwrap();
        let old = log.current_keys(&eva.id).unwrap().epoch();
        log.remove(&ana.id, eva.key(), 4).unwrap();
        let state = log.state();
        assert!(!state.is_member(&eva.key()));
        let new = log.current_keys(&luis.id).unwrap();
        assert_ne!(new.epoch(), old);
        assert!(log.current_keys(&eva.id).is_none());
        assert_eq!(log.current_keys(&ana.id).unwrap().epoch(), new.epoch());
        // Old keys stay readable for members.
        assert!(log.all_keys(&luis.id).contains_key(&old));
    }

    #[test]
    fn managers_set_the_icon_for_everyone() {
        let ana = Device::new();
        let luis = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        assert_eq!(log.state().icon, 0);
        assert!(log.append(&luis.id, Body::SetIcon { icon: 4 }, 3).is_err());
        log.append(&ana.id, Body::SetIcon { icon: 3 }, 3).unwrap();
        // The change travels like any entry and reads back the same elsewhere.
        let mut copy = GroupLog::empty(log.group_id());
        for entry in log.entries_in_order() {
            copy.insert(Entry::decode(&entry.encode()).unwrap())
                .unwrap();
        }
        assert_eq!(copy.state().icon, 3);
    }

    #[test]
    fn everyone_acts_only_on_those_below_it() {
        let (ana, luis, eva) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        log.add(&ana.id, eva.key(), "eva", 3).unwrap();
        // Nobody acts on someone above it, nor on itself.
        assert!(log.remove(&eva.id, luis.key(), 4).is_err());
        assert!(log.remove(&luis.id, ana.key(), 4).is_err());
        let mov = |member: &Device, up: bool| match up {
            true => Body::Promote {
                member: member.key(),
            },
            false => Body::Demote {
                member: member.key(),
            },
        };
        for up in [true, false] {
            assert!(log.append(&eva.id, mov(&luis, up), 4).is_err());
            assert!(log.append(&luis.id, mov(&luis, up), 4).is_err());
            assert!(log.append(&luis.id, mov(&ana, up), 4).is_err());
        }
        // Nor on a device that is no member.
        assert!(log.append(&ana.id, mov(&Device::new(), true), 4).is_err());
        // But everyone manages: Eva renames, adds and invites.
        log.append(&eva.id, Body::Rename { name: "e".into() }, 4)
            .unwrap();
        let (dani, sofia) = (Device::new(), Device::new());
        log.add(&eva.id, dani.key(), "dani", 5).unwrap();
        assert!(log.invite(&eva.id, 100, 5).is_ok());
        // And acts on whoever is below it.
        log.add(&ana.id, sofia.key(), "sofia", 6).unwrap();
        log.remove(&eva.id, sofia.key(), 7).unwrap();
        log.remove(&luis.id, eva.key(), 8).unwrap();
        let state = log.state();
        assert_eq!(state.ladder, [ana.key(), luis.key(), dani.key()]);
    }

    #[test]
    fn promote_moves_up_to_just_below_and_demote_to_the_end() {
        let (ana, bea, leo, sofia, eva) = (
            Device::new(),
            Device::new(),
            Device::new(),
            Device::new(),
            Device::new(),
        );
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        for (i, d) in [&bea, &leo, &sofia, &eva].into_iter().enumerate() {
            log.add(&ana.id, d.key(), "x", 2 + i as u64).unwrap();
        }
        let ladder = |log: &GroupLog| log.state().ladder;
        // Bea moves Eva up to just below her, never above.
        log.append(&bea.id, Body::Promote { member: eva.key() }, 10)
            .unwrap();
        assert_eq!(
            ladder(&log),
            [ana.key(), bea.key(), eva.key(), leo.key(), sofia.key()]
        );
        // Ana moves Sofía up: she goes right below Ana, above Bea.
        log.append(
            &ana.id,
            Body::Promote {
                member: sofia.key(),
            },
            11,
        )
        .unwrap();
        assert_eq!(
            ladder(&log),
            [ana.key(), sofia.key(), bea.key(), eva.key(), leo.key()]
        );
        // Moving up someone already just below changes nothing.
        log.append(
            &ana.id,
            Body::Promote {
                member: sofia.key(),
            },
            12,
        )
        .unwrap();
        assert_eq!(ladder(&log)[1], sofia.key());
        // Sofía sends Bea to the end; Bea can no longer act on Eva or Leo.
        log.append(&sofia.id, Body::Demote { member: bea.key() }, 13)
            .unwrap();
        assert_eq!(
            ladder(&log),
            [ana.key(), sofia.key(), eva.key(), leo.key(), bea.key()]
        );
        assert!(log.remove(&bea.id, leo.key(), 14).is_err());
        assert!(log.state().outranks(&leo.key(), &bea.key()));
        // Someone who leaves and comes back starts at the end again.
        log.append(&sofia.id, Body::Leave, 15).unwrap();
        log.add(&ana.id, sofia.key(), "x", 16).unwrap();
        assert_eq!(
            ladder(&log),
            [ana.key(), eva.key(), leo.key(), bea.key(), sofia.key()]
        );
        assert_eq!(log.state().rank(&sofia.key()), Some(4));
        assert_eq!(log.state().below(&leo.key()), [bea.key(), sofia.key()]);
    }

    #[test]
    fn the_first_n_manage_and_nobody_leaves_itself_out() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "leo", 3).unwrap();
        let first = |n| Body::SetManagers { first: n };
        assert_eq!(log.state().managers, Managers::Everyone);
        // Leo, third, cannot close it to the first two, nor to 0 or past the limit.
        assert!(log.append(&leo.id, first(Some(2)), 4).is_err());
        assert!(log.append(&ana.id, first(Some(0)), 4).is_err());
        assert!(
            log.append(&ana.id, first(Some(MAX_MEMBERS + 1)), 4)
                .is_err()
        );
        log.append(&bea.id, first(Some(2)), 4).unwrap();
        let state = log.state();
        assert_eq!(state.managers, Managers::First(2));
        assert!(state.is_manager(&bea.key()) && !state.is_manager(&leo.key()));
        // Leo no longer manages, but still acts on those below him.
        assert!(
            log.append(&leo.id, Body::Rename { name: "x".into() }, 5)
                .is_err()
        );
        assert!(log.append(&leo.id, first(None), 5).is_err());
        assert!(log.issue_ticket(&leo.id, 100).is_err());
        let sofia = Device::new();
        log.add(&bea.id, sofia.key(), "sofia", 6).unwrap();
        log.append(
            &leo.id,
            Body::Demote {
                member: sofia.key(),
            },
            7,
        )
        .unwrap();
        log.remove(&leo.id, sofia.key(), 8).unwrap();
        // Moved up below Ana, Leo manages and Bea, now third, does not.
        log.append(&ana.id, Body::Promote { member: leo.key() }, 9)
            .unwrap();
        let state = log.state();
        assert!(state.is_manager(&leo.key()) && !state.is_manager(&bea.key()));
        // Bea cannot open it again; Leo can.
        assert!(log.append(&bea.id, first(None), 10).is_err());
        log.append(&leo.id, first(None), 10).unwrap();
        assert!(log.state().is_manager(&bea.key()));
    }
    #[test]
    fn concurrent_changes_converge_and_never_leak_keys() {
        // Ana (creator) and Luis (admin) work offline at the same time: Ana removes Eva while
        // Luis rotates the key, still including Eva.
        let ana = Device::new();
        let luis = Device::new();
        let eva = Device::new();
        let mut base = GroupLog::create(&ana.id, "g", 1).unwrap();
        base.add(&ana.id, luis.key(), "luis", 2).unwrap();
        base.add(&ana.id, eva.key(), "eva", 3).unwrap();
        base.append(&ana.id, Body::Promote { member: luis.key() }, 4)
            .unwrap();
        let mut at_ana = base.clone();
        let mut at_luis = base.clone();
        at_ana.remove(&ana.id, eva.key(), 10).unwrap();
        at_luis.rotate(&luis.id, 10).unwrap();

        sync(&at_luis, &mut at_ana);
        sync(&at_ana, &mut at_luis);
        let a = at_ana.state();
        let l = at_luis.state();
        assert_eq!(a.applied, l.applied);
        assert_eq!(a.current_epoch(), l.current_epoch());
        assert!(!a.is_member(&eva.key()));
        // Whatever the order, Eva never holds the current key and both admins do: a rotation
        // that still includes Eva is either superseded by the removal or not used at all.
        assert!(at_ana.current_keys(&eva.id).is_none());
        let a_keys = at_ana.current_keys(&ana.id).unwrap();
        let l_keys = at_luis.current_keys(&luis.id).unwrap();
        assert_eq!(a_keys.epoch(), l_keys.epoch());
        assert!(a.lacking_current_secret().is_empty());
    }

    #[test]
    fn member_added_during_a_concurrent_rotation_is_welcomed() {
        let ana = Device::new();
        let luis = Device::new();
        let eva = Device::new();
        let mut base = GroupLog::create(&ana.id, "g", 1).unwrap();
        base.add(&ana.id, luis.key(), "luis", 2).unwrap();
        base.append(&ana.id, Body::Promote { member: luis.key() }, 3)
            .unwrap();
        let mut at_ana = base.clone();
        let mut at_luis = base.clone();
        // Ana adds Eva while Luis rotates without knowing about her.
        at_ana.add(&ana.id, eva.key(), "eva", 10).unwrap();
        at_luis.rotate(&luis.id, 10).unwrap();
        sync(&at_luis, &mut at_ana);
        sync(&at_ana, &mut at_luis);
        let state = at_ana.state();
        assert!(state.is_member(&eva.key()));
        if !state.lacking_current_secret().is_empty() {
            assert_eq!(state.lacking_current_secret(), vec![eva.key()]);
            at_ana.welcome(&ana.id, eva.key(), 11).unwrap();
        }
        assert_eq!(
            at_ana.current_keys(&eva.id).unwrap().epoch(),
            at_ana.current_keys(&luis.id).unwrap().epoch()
        );
    }

    #[test]
    fn leave_then_rotate() {
        let ana = Device::new();
        let luis = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        log.append(&luis.id, Body::Leave, 3).unwrap();
        assert!(!log.state().is_member(&luis.key()));
        let before = log.current_keys(&ana.id).unwrap().epoch();
        log.rotate(&ana.id, 4).unwrap();
        assert_ne!(log.current_keys(&ana.id).unwrap().epoch(), before);
        assert!(log.current_keys(&luis.id).is_none());
    }

    #[test]
    fn entries_with_unknown_parents_wait() {
        let ana = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let orphan = Entry::sign(
            &ana.id,
            log.group_id(),
            vec![[7u8; 32]],
            2,
            Body::Rename { name: "x".into() },
        );
        log.insert(orphan).unwrap();
        assert_eq!(log.state().name, "g");
        assert_eq!(log.ordered().len(), 1);
    }

    #[test]
    fn foreign_and_forged_entries_are_refused() {
        let ana = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let other = GroupLog::create(&ana.id, "h", 1).unwrap();
        let foreign = other.entries().next().unwrap().clone();
        assert_eq!(log.insert(foreign), Err(StateError::WrongGroup));
        let mut forged = Entry::sign(
            &ana.id,
            log.group_id(),
            log.heads(),
            2,
            Body::Rename { name: "x".into() },
        );
        forged.time_ms = 3;
        assert_eq!(log.insert(forged), Err(StateError::BadSignature));
    }

    #[test]
    fn wrong_secrets_in_a_welcome_are_ignored() {
        let ana = Device::new();
        let eva = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        let join = join(&mut log, &eva, ticket, 50);
        let epoch = log.state().current_epoch().unwrap().id;
        let bogus = GroupSecret::generate().unwrap();
        let sealed = Sealed::seal(
            &eva.key(),
            &secret_context(&log.group_id(), &epoch),
            bogus.as_bytes(),
        )
        .unwrap();
        let mut parents = log.heads();
        parents.push(join.hash());
        log.append_on(
            &ana.id,
            parents,
            Body::Welcome {
                member: eva.key(),
                epoch,
                secret: sealed,
            },
            51,
        )
        .unwrap();
        assert!(log.state().is_member(&eva.key()));
        assert!(log.current_keys(&eva.id).is_none());
    }

    #[test]
    fn leaving_leaks_the_current_secret_until_someone_rotates() {
        let ana = Device::new();
        let luis = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        assert!(log.state().current_secret_leaked_to().is_empty());
        log.append(&luis.id, Body::Leave, 3).unwrap();
        assert_eq!(log.state().current_secret_leaked_to(), vec![luis.key()]);
        assert_eq!(log.state().current_holders_in_order(), vec![ana.key()]);
        log.rotate(&ana.id, 4).unwrap();
        assert!(log.state().current_secret_leaked_to().is_empty());
        assert!(log.current_keys(&luis.id).is_none());
    }

    /// Both copies take each other's entries; checks they agree on who holds the current
    /// secret without being a member.
    fn met(a: &mut GroupLog, b: &mut GroupLog) -> GroupState {
        sync(a, b);
        sync(b, a);
        let (one, two) = (a.state(), b.state());
        assert_eq!(one.applied, two.applied);
        assert_eq!(
            one.current_secret_leaked_to(),
            two.current_secret_leaked_to()
        );
        one
    }

    /// Ana's group, with Bea managing too and Carlos a member.
    fn two_managers() -> (GroupLog, Device, Device, Device) {
        let (ana, bea, carlos) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "bea", 2).unwrap();
        log.add(&ana.id, carlos.key(), "carlos", 3).unwrap();
        log.append(&ana.id, Body::SetManagers { first: Some(2) }, 4)
            .unwrap();
        (log, ana, bea, carlos)
    }

    #[test]
    fn a_device_let_in_by_a_welcome_that_lost_a_race_counts_as_holding_the_key() {
        for _ in 0..8 {
            // An invite link passed on: Ana lets in Xavi and, at once, Bea lets in Yago,
            // both with the one ticket. One welcome wins; the other device was still sent
            // the log with its box of the current secret.
            let (base, ana, bea, carlos) = two_managers();
            let (xavi, yago) = (Device::new(), Device::new());
            let ticket = base.issue_ticket(&ana.id, 1_000).unwrap();
            let mut at_ana = base.clone();
            at_ana
                .append(
                    &ana.id,
                    Body::Invite {
                        ticket: ticket.clone(),
                        token: [1; 16],
                    },
                    5,
                )
                .unwrap();
            let mut at_bea = at_ana.clone();
            let x = join(&mut at_ana, &xavi, ticket.clone(), 6);
            at_ana.let_in(&ana.id, &x.hash(), 7).unwrap();
            let y = join(&mut at_bea, &yago, ticket, 6);
            at_bea.let_in(&bea.id, &y.hash(), 7).unwrap();
            // Each was sent the log as it was let in, and took its key.
            let xavi_key = at_ana.current_keys(&xavi.id).unwrap().epoch();
            let yago_key = at_bea.current_keys(&yago.id).unwrap().epoch();
            let state = met(&mut at_ana, &mut at_bea);
            let (won, lost, lost_key) = if state.is_member(&xavi.key()) {
                (&xavi, &yago, yago_key)
            } else {
                (&yago, &xavi, xavi_key)
            };
            assert!(state.is_member(&won.key()) && !state.is_member(&lost.key()));
            let current = epoch_number(&state.current_epoch().unwrap().id);
            assert_eq!(lost_key, current, "it holds the current key");
            assert_eq!(state.current_secret_leaked_to(), vec![lost.key()]);
            assert!(!state.epoch_kept(current));
            // The first holder starts a new epoch, as when someone leaves; or another
            // member, which a leak allows.
            assert_eq!(state.current_holders_in_order()[0], ana.key());
            at_bea.rotate(&carlos.id, 8).unwrap();
            let state = met(&mut at_ana, &mut at_bea);
            assert!(state.current_secret_leaked_to().is_empty());
            assert!(at_ana.current_keys(&lost.id).is_none());
            assert!(at_ana.current_keys(&won.id).is_some());
        }
    }

    #[test]
    fn a_device_refused_and_let_in_at_once_holds_the_key() {
        for _ in 0..8 {
            // Bea says no to Xavi (a `Revoke` of his ticket) while Ana says yes. Bea came in
            // later: her no applies first, and Ana's yes lets nobody in.
            let (base, ana, bea, _) = two_managers();
            let xavi = Device::new();
            let ticket = base.issue_ticket(&ana.id, 1_000).unwrap();
            let mut at_ana = base.clone();
            let x = join(&mut at_ana, &xavi, ticket.clone(), 5);
            let mut at_bea = at_ana.clone();
            at_bea
                .append(
                    &bea.id,
                    Body::Revoke {
                        ticket_id: ticket.ticket_id,
                    },
                    6,
                )
                .unwrap();
            at_ana.let_in(&ana.id, &x.hash(), 6).unwrap();
            assert!(at_ana.current_keys(&xavi.id).is_some());
            let state = met(&mut at_ana, &mut at_bea);
            assert!(!state.is_member(&xavi.key()));
            assert_eq!(state.current_secret_leaked_to(), vec![xavi.key()]);
        }
    }

    #[test]
    fn a_device_added_into_a_full_group_or_by_a_demoted_manager_holds_the_key() {
        for _ in 0..8 {
            // Room for one more: Ana and Bea add one each at once.
            let (mut base, ana, bea, _) = two_managers();
            base.append(
                &ana.id,
                Body::SetLimits {
                    members: 4,
                    room: 8,
                },
                5,
            )
            .unwrap();
            let (xavi, yago) = (Device::new(), Device::new());
            let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
            at_ana.add(&ana.id, xavi.key(), "xavi", 6).unwrap();
            at_bea.add(&bea.id, yago.key(), "yago", 6).unwrap();
            let state = met(&mut at_ana, &mut at_bea);
            assert_eq!(state.members.len(), 4);
            let lost = if state.is_member(&xavi.key()) {
                yago.key()
            } else {
                xavi.key()
            };
            assert_eq!(state.current_secret_leaked_to(), vec![lost]);

            // Ana sends Bea to the end while Bea adds Zoe: the add is outranked, and Zoe,
            // never a member, has the key.
            let zoe = Device::new();
            let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
            at_ana
                .append(&ana.id, Body::Demote { member: bea.key() }, 6)
                .unwrap();
            at_bea.add(&bea.id, zoe.key(), "zoe", 6).unwrap();
            let state = met(&mut at_ana, &mut at_bea);
            assert!(!state.is_member(&zoe.key()));
            assert_eq!(state.current_secret_leaked_to(), vec![zoe.key()]);
            // A member that does not manage may rotate for it.
            assert!(!state.is_manager(&bea.key()));
            at_ana.rotate(&bea.id, 7).unwrap();
            assert!(at_ana.state().current_secret_leaked_to().is_empty());
        }
    }

    #[test]
    fn a_refused_box_counts_only_for_a_device_that_is_no_member() {
        // Ana sends Bea to the end and starts a new epoch while Bea, not knowing, gives
        // Carlos a box that opens to nothing and adds Zoe with the epoch she knew.
        let (base, ana, bea, carlos) = two_managers();
        let mut at_ana = base.clone();
        at_ana
            .append(&ana.id, Body::Demote { member: bea.key() }, 5)
            .unwrap();
        at_ana.rotate(&ana.id, 6).unwrap();
        let mut at_bea = base.clone();
        let first = base.state().epochs[0].id;
        let junk = Sealed::seal(
            &carlos.key(),
            &secret_context(&base.group_id(), &first),
            &[7; 32],
        )
        .unwrap();
        let welcome = at_bea
            .append(
                &bea.id,
                Body::Welcome {
                    member: carlos.key(),
                    epoch: first,
                    secret: junk,
                },
                5,
            )
            .unwrap();
        let zoe = Device::new();
        let add = at_bea.add(&bea.id, zoe.key(), "zoe", 6).unwrap();
        let state = met(&mut at_ana, &mut at_bea);
        let refused = |h: EntryHash| state.rejected.iter().any(|(r, _)| *r == h);
        assert!(refused(welcome.hash()) && refused(add.hash()));
        // Carlos is a member: not marked as given a key by a manager. Zoe holds the first
        // epoch's secret, not the current one.
        assert!(state.current_secret_leaked_to().is_empty());
        assert!(state.lacking_current_secret().is_empty());
        assert!(!state.epoch_kept(epoch_number(&first)));
        assert!(state.epoch_kept(epoch_number(&state.current_epoch().unwrap().id)));
    }

    #[test]
    fn a_member_sent_a_key_in_an_entry_that_did_not_apply_holds_it_once_gone() {
        // Carlos is left out of Ana's new epoch. Ana moves Bea to the end while Bea, not
        // knowing, gives Carlos that key: her welcome does not apply, but Carlos was sent
        // it. Then Carlos leaves.
        let (mut base, ana, bea, carlos) = two_managers();
        let group = base.group_id();
        let (rotation, _) = Rotation::new(&group, [ana.key(), bea.key()]).unwrap();
        base.append(&ana.id, Body::Rotate { rotation }, 5).unwrap();
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        at_ana
            .append(&ana.id, Body::Demote { member: bea.key() }, 6)
            .unwrap();
        let welcome = at_bea.welcome(&bea.id, carlos.key(), 6).unwrap();
        let state = met(&mut at_ana, &mut at_bea);
        assert!(state.rejected.iter().any(|(h, _)| *h == welcome.hash()));
        let current = epoch_number(&state.current_epoch().unwrap().id);
        // A member still: not counted as given the key, so a manager gives it again.
        assert_eq!(state.lacking_current_secret(), vec![carlos.key()]);
        assert!(state.current_secret_leaked_to().is_empty());
        assert!(state.epoch_kept(current));
        at_ana.append(&carlos.id, Body::Leave, 7).unwrap();
        let state = at_ana.state();
        assert_eq!(state.current_secret_leaked_to(), vec![carlos.key()]);
        assert!(!state.epoch_kept(current));
        // Only managers rotate for it (a rotation of a member that does not manage is
        // judged as before: someone given the key by an entry that applied must be gone).
        assert!(state.leaked_as_judged().is_empty());
        assert!(state.current_secret_left_with().is_empty());
        at_ana.rotate(&ana.id, 8).unwrap();
        assert!(at_ana.state().current_secret_leaked_to().is_empty());
    }

    #[test]
    fn concurrent_removals_leak_the_secret_and_a_rotation_repairs_it() {
        // Ana removes Eva while Luis removes Dani: one rotation is discarded and the other
        // still includes a removed member.
        let ana = Device::new();
        let luis = Device::new();
        let eva = Device::new();
        let dani = Device::new();
        let mut base = GroupLog::create(&ana.id, "g", 1).unwrap();
        base.add(&ana.id, luis.key(), "luis", 2).unwrap();
        base.add(&ana.id, eva.key(), "eva", 3).unwrap();
        base.add(&ana.id, dani.key(), "dani", 4).unwrap();
        base.append(&ana.id, Body::Promote { member: luis.key() }, 5)
            .unwrap();
        let mut at_ana = base.clone();
        let mut at_luis = base.clone();
        at_ana.remove(&ana.id, eva.key(), 10).unwrap();
        at_luis.remove(&luis.id, dani.key(), 10).unwrap();
        sync(&at_luis, &mut at_ana);
        sync(&at_ana, &mut at_luis);
        let state = at_ana.state();
        assert!(!state.is_member(&eva.key()) && !state.is_member(&dani.key()));
        let leaked = state.current_secret_leaked_to();
        assert_eq!(leaked.len(), 1);
        assert!(leaked[0] == eva.key() || leaked[0] == dani.key());
        // The first holder admitted is Ana; whoever rotates, the leak is gone.
        assert_eq!(state.current_holders_in_order()[0], ana.key());
        at_luis.rotate(&luis.id, 11).unwrap();
        sync(&at_luis, &mut at_ana);
        assert!(at_ana.state().current_secret_leaked_to().is_empty());
        assert!(at_ana.current_keys(&eva.id).is_none());
        assert!(at_ana.current_keys(&dani.id).is_none());
        assert!(at_ana.current_keys(&luis.id).is_some());
    }

    #[test]
    fn a_first_key_only_comes_from_a_manager() {
        // Mallory (a member) relays Zed's join with an expired ticket and a backdated time.
        let ana = Device::new();
        let mallory = Device::new();
        let zed = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1_000).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, mallory.key(), "mallory", 2_000).unwrap();
        let ticket = log.issue_ticket(&ana.id, 5_000).unwrap();
        join(&mut log, &zed, ticket.clone(), 4_000);
        assert!(!log.state().is_member(&zed.key()));
        assert_eq!(log.join_ticket_of(&zed.key()), None);
        assert!(!log.state().ever_held(&zed.key()));
        // Mallory can neither let Zed in nor give it the key; Ana (an admin) can.
        assert!(log.welcome(&mallory.id, zed.key(), 9_000).is_err());
        assert!(log.current_keys(&zed.id).is_none());
        // Ana only while the ticket is valid by her clock.
        assert!(log.welcome(&ana.id, zed.key(), 9_001).is_err());
        log.welcome(&ana.id, zed.key(), 4_900).unwrap();
        assert!(log.state().is_member(&zed.key()));
        assert_eq!(log.join_ticket_of(&zed.key()), Some(&ticket));
        assert!(log.current_keys(&zed.id).is_some());
        // Not even a newer one for a member that already had a key: keys come from admins.
        log.rotate(&ana.id, 9_002).unwrap();
        let rotated = log.state().current_epoch().unwrap().id;
        let mut copy = log.clone();
        // Pretend the rotation missed Zed: give it the key through Mallory.
        let (epoch, secret) = {
            let s = copy.secrets(&ana.id);
            (
                rotated,
                GroupSecret::from_bytes(s[&rotated].as_bytes()).unwrap(),
            )
        };
        let sealed = Sealed::seal(
            &zed.key(),
            &secret_context(&copy.group_id(), &epoch),
            secret.as_bytes(),
        )
        .unwrap();
        assert_eq!(
            copy.append(
                &mallory.id,
                Body::Welcome {
                    member: zed.key(),
                    epoch,
                    secret: sealed,
                },
                9_003,
            ),
            Err(StateError::NotAllowed(WELCOME_NOT_MANAGER))
        );
    }

    #[test]
    fn the_ladder_closes_up_when_the_first_leaves() {
        let (ana, luis, eva, dani) = (Device::new(), Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        log.add(&ana.id, eva.key(), "eva", 3).unwrap();
        log.add(&ana.id, dani.key(), "dani", 4).unwrap();
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 5)
            .unwrap();
        // Ana chooses Eva to come after her, then leaves.
        log.append(&ana.id, Body::Promote { member: eva.key() }, 5)
            .unwrap();
        log.append(&ana.id, Body::Leave, 6).unwrap();
        let state = log.state();
        assert_eq!(state.first(), Some(eva.key()));
        assert_eq!(state.ladder, [eva.key(), luis.key(), dani.key()]);
        assert!(state.is_manager(&eva.key()) && !state.is_manager(&luis.key()));
        // The one who started the group is still told apart, with no rights.
        assert_eq!(state.creator, Some(ana.key()));
        // Eva acts on everyone; Luis on Dani only.
        log.remove(&eva.id, luis.key(), 7).unwrap();
        assert!(log.remove(&dani.id, eva.key(), 8).is_err());
        // The last one may leave too: nobody is left.
        log.append(&eva.id, Body::Leave, 9).unwrap();
        log.append(&dani.id, Body::Leave, 10).unwrap();
        let state = log.state();
        assert!(state.members.is_empty() && state.ladder.is_empty());
    }

    #[test]
    fn a_first_that_left_unseen_blocks_nothing() {
        // Ana's leave never reaches anyone: she stays first, and the others go on.
        let (ana, bea, leo, dani) = (Device::new(), Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "leo", 3).unwrap();
        log.add(&ana.id, dani.key(), "dani", 4).unwrap();
        let mut lost = log.clone();
        lost.append(&ana.id, Body::Leave, 5).unwrap();
        log.append(&bea.id, Body::Rename { name: "b".into() }, 6)
            .unwrap();
        log.append(&bea.id, Body::Promote { member: dani.key() }, 7)
            .unwrap();
        log.remove(&bea.id, leo.key(), 8).unwrap();
        assert!(log.invite(&dani.id, 100, 9).is_ok());
        let state = log.state();
        assert_eq!(state.ladder, [ana.key(), bea.key(), dani.key()]);
        assert_eq!(state.name, "b");
        // Nobody moves or removes the one above them all.
        assert!(log.remove(&bea.id, ana.key(), 10).is_err());
    }
    #[test]
    fn invites_are_open_until_used_or_expired() {
        let ana = Device::new();
        let luis = Device::new();
        let eva = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        assert!(log.invite(&luis.id, 100, 3).is_err());
        let (ticket, token) = log.invite(&ana.id, 100, 3).unwrap();
        assert_eq!(log.state().open_invite_tokens(50), vec![token]);
        assert!(log.state().open_invite_tokens(101).is_empty());
        join(&mut log, &eva, ticket, 60);
        // Open until a manager lets the device in.
        assert_eq!(log.state().open_invite_tokens(61), vec![token]);
        log.welcome(&ana.id, eva.key(), 61).unwrap();
        assert!(log.state().is_member(&eva.key()));
        assert!(log.state().open_invite_tokens(61).is_empty());
        // A ticket noted by someone other than its issuer is ignored.
        let other = log.issue_ticket(&ana.id, 100).unwrap();
        log.append(&ana.id, Body::Promote { member: luis.key() }, 70)
            .unwrap();
        assert!(
            log.append(
                &luis.id,
                Body::Invite {
                    ticket: other,
                    token: [7; 16]
                },
                71
            )
            .is_err()
        );
        // The invite's rendezvous comes from the token alone.
        assert_ne!(
            crate::invite_rendezvous(&token),
            crate::invite_rendezvous(&[0; 16])
        );
    }

    #[test]
    fn a_full_group_takes_nobody_else() {
        let ana = Device::new();
        let (luis, eva) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        let state = log.state();
        assert_eq!((state.max_members, state.max_in_room), (20, 8));
        let limits = |members, room| Body::SetLimits { members, room };
        log.append(&ana.id, limits(2, 4), 2).unwrap();
        log.add(&ana.id, luis.key(), "luis", 3).unwrap();
        assert!(log.add(&ana.id, eva.key(), "eva", 4).is_err());
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        join(&mut log, &eva, ticket, 5);
        assert!(!log.state().is_member(&eva.key()));
        // Limits stay within 2 to 20 and 2 to 8, set by admins, never below the members in.
        assert!(log.append(&ana.id, limits(21, 8), 6).is_err());
        assert!(log.append(&ana.id, limits(20, 9), 6).is_err());
        assert!(log.append(&ana.id, limits(20, 1), 6).is_err());
        assert!(log.append(&luis.id, limits(20, 8), 6).is_err());
        log.append(&ana.id, limits(20, 8), 6).unwrap();
        log.add(&ana.id, eva.key(), "eva", 7).unwrap();
        assert!(log.append(&ana.id, limits(2, 8), 8).is_err());
        assert_eq!(log.state().max_in_room, 8);
    }

    #[test]
    fn managers_name_open_and_close_voice_rooms() {
        let (ana, luis) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        let first = &log.state().rooms;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, FIRST_ROOM);
        let room = |id: u8, name: &str| Body::SetRoom {
            id: [id; 8],
            name: name.into(),
        };
        log.append(&ana.id, room(0, "General"), 3).unwrap();
        log.append(&ana.id, room(1, "Gaming"), 4).unwrap();
        log.append(&ana.id, room(1, "Games"), 5).unwrap();
        assert!(log.append(&luis.id, room(2, "Mine"), 6).is_err());
        assert!(log.append(&ana.id, room(2, "  "), 6).is_err());
        assert!(log.append(&ana.id, room(2, &"x".repeat(41)), 6).is_err());
        assert!(
            log.append(&ana.id, Body::RemoveRoom { id: FIRST_ROOM }, 7)
                .is_err()
        );
        let names: Vec<String> = log.state().rooms.iter().map(|r| r.name.clone()).collect();
        assert_eq!(names, ["General", "Games"]);
        log.append(&ana.id, Body::RemoveRoom { id: [1; 8] }, 8)
            .unwrap();
        assert_eq!(log.state().rooms.len(), 1);
        for i in 1..MAX_ROOMS as u8 {
            log.append(&ana.id, room(10 + i, "Room"), 9).unwrap();
        }
        assert!(log.append(&ana.id, room(99, "One more"), 10).is_err());
    }

    /// A group of four in this order on the ladder: Ana made it, then Bea, Leo and Sofía;
    /// the first two manage. A second voice room `[1; 8]`.
    fn four() -> (GroupLog, Device, Device, Device, Device) {
        let (ana, bea, leo, sofia) = (Device::new(), Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        for (i, d) in [&bea, &leo, &sofia].into_iter().enumerate() {
            log.add(&ana.id, d.key(), "x", 2 + i as u64).unwrap();
        }
        log.append(&ana.id, Body::SetManagers { first: Some(2) }, 5)
            .unwrap();
        log.append(
            &ana.id,
            Body::SetRoom {
                id: [1; 8],
                name: "Gaming".into(),
            },
            6,
        )
        .unwrap();
        (log, ana, bea, leo, sofia)
    }

    #[test]
    fn members_above_keep_others_out_of_a_room_for_a_while_or_until_let_back() {
        let (mut log, ana, bea, leo, sofia) = four();
        let general = FIRST_ROOM;
        // An hour from Bea's clock at 100.
        log.keep_out(&bea.id, general, leo.key(), Some(3_600_100), 100)
            .unwrap();
        let state = log.state();
        let Some(Barrier::TimedOut(kept)) = state.barrier(&general, &leo.key(), 200) else {
            panic!("Leo should be kept out");
        };
        assert_eq!(kept.by, bea.key());
        assert_eq!(kept.until_ms, Some(3_600_100));
        // Only that room, and only until then.
        assert_eq!(state.barrier(&[1; 8], &leo.key(), 200), None);
        assert_eq!(state.barrier(&general, &leo.key(), 3_600_100), None);
        assert_eq!(state.barrier(&general, &sofia.key(), 200), None);
        assert_eq!(state.kept_out_now(200).len(), 1);
        assert!(state.kept_out_now(3_600_100).is_empty());

        // Until an admin lets him back; then he is back.
        log.keep_out(&ana.id, [1; 8], leo.key(), None, 101).unwrap();
        assert!(log.state().barrier(&[1; 8], &leo.key(), u64::MAX).is_some());
        log.let_back(&bea.id, [1; 8], leo.key(), 102).unwrap();
        assert_eq!(log.state().barrier(&[1; 8], &leo.key(), 200), None);
        // A time out that is over before it starts is no time out.
        assert!(
            log.keep_out(&bea.id, general, sofia.key(), Some(50), 100)
                .is_err()
        );
        // Nor is one of a room the group does not have.
        assert!(
            log.keep_out(&bea.id, [7; 8], sofia.key(), None, 100)
                .is_err()
        );
    }

    #[test]
    fn only_someone_above_keeps_out_or_lets_back() {
        let (mut log, ana, bea, leo, sofia) = four();
        let general = FIRST_ROOM;
        // Leo, who does not manage, still keeps out Sofía, below him; not Bea, above.
        log.keep_out(&leo.id, general, sofia.key(), None, 10)
            .unwrap();
        assert!(log.keep_out(&leo.id, general, bea.key(), None, 10).is_err());
        // Sofía cannot let herself back, nor anyone; nobody keeps itself or the first out.
        assert!(log.let_back(&sofia.id, general, sofia.key(), 11).is_err());
        assert!(log.keep_out(&bea.id, general, ana.key(), None, 13).is_err());
        assert!(log.keep_out(&bea.id, general, bea.key(), None, 13).is_err());
        // Ana keeps Bea out, and Bea cannot let herself back; Leo cannot either.
        log.keep_out(&ana.id, general, bea.key(), None, 16).unwrap();
        assert!(log.let_back(&bea.id, general, bea.key(), 17).is_err());
        assert!(log.let_back(&leo.id, general, bea.key(), 17).is_err());
        let state = log.state();
        assert!(state.barrier(&general, &bea.key(), 20).is_some());
        assert!(state.outranks(&ana.key(), &bea.key()));
        assert!(!state.outranks(&leo.key(), &bea.key()));
        assert!(state.outranks(&leo.key(), &sofia.key()));
        assert!(!state.outranks(&sofia.key(), &leo.key()));
        log.let_back(&ana.id, general, bea.key(), 18).unwrap();
        assert_eq!(log.state().barrier(&general, &bea.key(), 20), None);
        // Moved up above Leo, Sofía is no longer kept out by him.
        log.append(
            &ana.id,
            Body::Promote {
                member: sofia.key(),
            },
            19,
        )
        .unwrap();
        assert_eq!(log.state().barrier(&general, &sofia.key(), 20), None);
    }
    #[test]
    fn a_room_for_some_lets_in_those_chosen_and_those_at_or_above_its_author() {
        let (mut log, ana, bea, leo, sofia) = four();
        let gaming = [1; 8];
        log.set_room_access(&bea.id, gaming, Some(vec![sofia.key()]), 10)
            .unwrap();
        let state = log.state();
        assert_eq!(
            state.barrier(&gaming, &leo.key(), 20),
            Some(Barrier::NotChosen)
        );
        assert_eq!(state.barrier(&gaming, &sofia.key(), 20), None);
        assert_eq!(state.barrier(&gaming, &ana.key(), 20), None);
        assert_eq!(state.barrier(&gaming, &bea.key(), 20), None);
        assert_eq!(state.barrier(&FIRST_ROOM, &leo.key(), 20), None);
        // Those who do not manage choose nothing; the room must exist.
        assert!(log.set_room_access(&leo.id, gaming, None, 11).is_err());
        assert!(log.set_room_access(&ana.id, [7; 8], None, 11).is_err());
        // A time out goes before the list: someone chosen can still be kept out.
        log.keep_out(&ana.id, gaming, sofia.key(), Some(100), 12)
            .unwrap();
        assert!(matches!(
            log.state().barrier(&gaming, &sofia.key(), 20),
            Some(Barrier::TimedOut(_))
        ));
        // Ana's rule leaves Bea out: she is below Ana.
        log.set_room_access(&ana.id, gaming, Some(vec![leo.key()]), 13)
            .unwrap();
        let state = log.state();
        assert_eq!(
            state.barrier(&gaming, &bea.key(), 20),
            Some(Barrier::NotChosen)
        );
        assert_eq!(state.barrier(&gaming, &leo.key(), 20), None);
        log.set_room_access(&ana.id, gaming, None, 14).unwrap();
        assert_eq!(log.state().barrier(&gaming, &bea.key(), 20), None);
    }
    #[test]
    fn a_rule_stays_when_its_author_is_moved_down() {
        let (mut log, ana, bea, leo, sofia) = four();
        log.keep_out(&bea.id, FIRST_ROOM, leo.key(), None, 100)
            .unwrap();
        log.append(&ana.id, Body::Demote { member: bea.key() }, 200)
            .unwrap();
        let state = log.state();
        assert!(state.barrier(&FIRST_ROOM, &leo.key(), 300).is_some());
        // Afterwards, Bea is no admin: what she signs now counts for nothing.
        let late = Entry::sign(
            &bea.id,
            log.group_id(),
            log.heads(),
            300,
            Body::KeepOut {
                room: FIRST_ROOM,
                member: sofia.key(),
                until_ms: None,
            },
        );
        assert!(log.insert(late).is_err());
        assert_eq!(log.state().barrier(&FIRST_ROOM, &sofia.key(), 400), None);
        // Leaving the group does not undo it either.
        log.append(&bea.id, Body::Leave, 500).unwrap();
        assert!(log.state().barrier(&FIRST_ROOM, &leo.key(), 600).is_some());
    }

    #[test]
    fn a_rule_signed_apart_from_its_author_s_loss_counts_for_nothing() {
        for loss in 0..3 {
            // Ana sends Bea to the end (or removes her, or Bea leaves). Bea then signs rules
            // on the entries from before, dated before it, with the place she had there.
            let (base, ana, bea, leo, sofia) = four();
            let mut log = base.clone();
            match loss {
                0 => drop(
                    log.append(&ana.id, Body::Demote { member: bea.key() }, 100)
                        .unwrap(),
                ),
                1 => drop(log.remove(&ana.id, bea.key(), 100).unwrap()),
                _ => drop(log.append(&bea.id, Body::Leave, 100).unwrap()),
            }
            let backdated = |body| Entry::sign(&bea.id, base.group_id(), base.heads(), 50, body);
            let keep_out = backdated(Body::KeepOut {
                room: FIRST_ROOM,
                member: leo.key(),
                until_ms: None,
            });
            let only = backdated(Body::RoomAccess {
                room: [1; 8],
                members: Some(vec![bea.key()]),
            });
            // Bea could make them where she signed them: they are kept, and apply to nothing.
            for rule in [&keep_out, &only] {
                log.insert(rule.clone()).unwrap();
                assert!(log.state().rejected.iter().any(|(h, _)| *h == rule.hash()));
            }
            let state = log.state();
            assert!(state.barrier(&FIRST_ROOM, &leo.key(), 200).is_none());
            assert!(state.barrier(&[1; 8], &sofia.key(), 200).is_none());
            // Any copy, whatever came first.
            let mut other = GroupLog::empty(log.group_id());
            let mut entries: Vec<Entry> = log.entries().cloned().collect();
            entries.reverse();
            other.insert_all(entries);
            assert_eq!(other.state().rules, state.rules);
            assert_eq!(other.state().applied, state.applied);
        }
    }

    #[test]
    fn room_rules_go_with_the_rest_of_the_log() {
        let (mut log, ana, bea, leo, sofia) = four();
        log.keep_out(&bea.id, FIRST_ROOM, leo.key(), None, 10)
            .unwrap();
        log.append(&ana.id, Body::Rename { name: "h".into() }, 11)
            .unwrap();
        log.set_room_access(&ana.id, [1; 8], Some(vec![sofia.key()]), 12)
            .unwrap();
        log.remove(&bea.id, sofia.key(), 13).unwrap();
        log.let_back(&ana.id, FIRST_ROOM, leo.key(), 14).unwrap();
        log.append(&bea.id, Body::SetIcon { icon: 4 }, 15).unwrap();
        // Entries are signed on rules like on any other.
        let rule_heads = log
            .entries()
            .filter(|e| !e.body.is_room_rule())
            .flat_map(|e| e.parents.iter())
            .filter(|p| log.get(p).unwrap().body.is_room_rule())
            .count();
        assert!(rule_heads > 0);
        // The log as stored and sent holds them, in the common order, and reads back the same.
        let all = log.entries_in_order();
        assert_eq!(all.len(), log.len());
        assert_eq!(all.iter().filter(|e| e.body.is_room_rule()).count(), 3);
        let mut back = GroupLog::empty(log.group_id());
        for entry in all {
            back.insert(entry.clone()).unwrap();
        }
        assert!(back.waiting().is_empty());
        assert_eq!(back.ordered(), log.ordered());
        assert_eq!(format!("{:?}", back.state()), format!("{:?}", log.state()));
        assert_eq!(back.heads(), log.heads());
    }

    /// Earlier versions kept room rules apart: nothing else was signed on them, and they
    /// were signed on everything, rules included. Such a log keeps its order and its group,
    /// and what is written after it is signed on its rules too.
    #[test]
    fn a_log_from_when_rules_went_apart_keeps_its_group() {
        let (base, ana, bea, leo, _) = four();
        let group = base.group_id();
        let (mut old, mut plain) = (base.clone(), base.clone());
        let keep_out = |parents: Vec<EntryHash>, time_ms| {
            Entry::sign(
                &bea.id,
                group,
                parents,
                time_ms,
                Body::KeepOut {
                    room: FIRST_ROOM,
                    member: leo.key(),
                    until_ms: None,
                },
            )
        };
        let first_rule = keep_out(base.heads(), 10);
        old.insert(first_rule.clone()).unwrap();
        let rename = Entry::sign(
            &ana.id,
            group,
            base.heads(),
            11,
            Body::Rename { name: "h".into() },
        );
        let second_rule = keep_out(vec![rename.hash(), first_rule.hash()], 12);
        let icon = Entry::sign(
            &bea.id,
            group,
            vec![rename.hash()],
            13,
            Body::SetIcon { icon: 4 },
        );
        for entry in [&rename, &second_rule, &icon] {
            old.insert((*entry).clone()).unwrap();
        }
        for entry in [&rename, &icon] {
            plain.insert((*entry).clone()).unwrap();
        }
        let without_rules: Vec<EntryHash> = old
            .ordered()
            .into_iter()
            .filter(|h| !old.get(h).unwrap().body.is_room_rule())
            .collect();
        assert_eq!(without_rules, plain.ordered());
        let (with, without) = (old.state(), plain.state());
        assert_eq!(with.members, without.members);
        assert_eq!(with.ladder, without.ladder);
        assert_eq!(with.epochs, without.epochs);
        assert_eq!((with.name.as_str(), with.icon), ("h", 4));
        assert_eq!((without.name.as_str(), without.icon), ("h", 4));
        assert!(with.barrier(&FIRST_ROOM, &leo.key(), 20).is_some());
        assert!(without.barrier(&FIRST_ROOM, &leo.key(), 20).is_none());
        // Both rules and the icon are what the next entry is signed on.
        let mut heads = old.heads();
        heads.sort_unstable();
        let mut want = vec![second_rule.hash(), icon.hash()];
        want.sort_unstable();
        assert_eq!(heads, want);
        let next = old
            .append(&ana.id, Body::Rename { name: "i".into() }, 14)
            .unwrap();
        assert_eq!(old.state().name, "i");
        assert_eq!(old.placed_heads(), vec![next.hash()]);
        assert!(old.state().barrier(&FIRST_ROOM, &leo.key(), 20).is_some());
    }

    #[test]
    fn rules_signed_at_once_settle_the_same_everywhere() {
        let (base, ana, bea, leo, _) = four();
        let (mut one, mut two) = (base.clone(), base);
        one.keep_out(&ana.id, FIRST_ROOM, leo.key(), None, 10)
            .unwrap();
        two.let_back(&bea.id, FIRST_ROOM, leo.key(), 10).unwrap();
        two.keep_out(&bea.id, FIRST_ROOM, leo.key(), Some(500), 11)
            .unwrap();
        let a: Vec<Entry> = one.entries_in_order().into_iter().cloned().collect();
        let b: Vec<Entry> = two.entries_in_order().into_iter().cloned().collect();
        one.insert_all(b);
        two.insert_all(a);
        assert_eq!(one.state().rules, two.state().rules);
        assert_eq!(one.ordered(), two.ordered());
    }

    #[test]
    fn a_revoked_invite_lets_nobody_in() {
        let (ana, eva) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let (ticket, _) = log.invite(&ana.id, 100, 2).unwrap();
        assert_eq!(log.state().open_invites.len(), 1);
        log.append(
            &ana.id,
            Body::Revoke {
                ticket_id: ticket.ticket_id,
            },
            3,
        )
        .unwrap();
        assert!(log.state().open_invites.is_empty());
        join(&mut log, &eva, ticket, 4);
        assert!(!log.state().is_member(&eva.key()));
    }

    #[test]
    fn only_the_issuer_signs_a_refusal_the_invited_device_believes() {
        let (ana, luis, mallory) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        log.append(&ana.id, Body::Promote { member: luis.key() }, 3)
            .unwrap();
        let (ticket, _) = log.invite(&ana.id, 100, 4).unwrap();
        let revoke = |by: &Device| {
            let body = Body::Revoke {
                ticket_id: ticket.ticket_id,
            };
            Entry::sign(&by.id, log.group_id(), log.heads(), 5, body)
        };
        // Someone who learnt the ticket, or another admin: the device cannot check them.
        assert!(!ticket.revoked_by(&revoke(&mallory)));
        assert!(!ticket.revoked_by(&revoke(&luis)));
        // The issuer's own, untouched, for this ticket and group only.
        let by_ana = revoke(&ana);
        assert!(ticket.revoked_by(&by_ana));
        let mut tampered = by_ana.clone();
        tampered.time_ms += 1;
        assert!(!ticket.revoked_by(&tampered));
        let other = log.issue_ticket(&ana.id, 100).unwrap();
        assert!(!other.revoked_by(&by_ana));

        // Luis says no: Ana, who wrote the invite, has to confirm it.
        assert!(log.revokes_to_confirm(&ana.key()).is_empty());
        log.append(
            &luis.id,
            Body::Revoke {
                ticket_id: ticket.ticket_id,
            },
            5,
        )
        .unwrap();
        assert!(log.issuer_revoke(&ticket).is_none());
        assert_eq!(log.revokes_to_confirm(&ana.key()), vec![ticket.ticket_id]);
        assert!(log.revokes_to_confirm(&luis.key()).is_empty());
        log.append(
            &ana.id,
            Body::Revoke {
                ticket_id: ticket.ticket_id,
            },
            6,
        )
        .unwrap();
        assert!(log.revokes_to_confirm(&ana.key()).is_empty());
        let found = log.issuer_revoke(&ticket).unwrap();
        assert!(ticket.revoked_by(found));

        // An invite someone joined with needs no refusal.
        let (used, _) = log.invite(&ana.id, 100, 7).unwrap();
        join(&mut log, &mallory, used.clone(), 8);
        log.welcome(&ana.id, mallory.key(), 8).unwrap();
        log.append(
            &luis.id,
            Body::Revoke {
                ticket_id: used.ticket_id,
            },
            9,
        )
        .unwrap();
        assert!(log.revokes_to_confirm(&ana.key()).is_empty());
    }

    #[test]
    fn managers_set_a_small_picture() {
        let (ana, luis) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        let picture = |p: Option<Vec<u8>>| Body::SetPicture { picture: p };
        log.append(&ana.id, picture(Some(vec![1; 1000])), 3)
            .unwrap();
        assert_eq!(log.state().picture.map(|p| p.len()), Some(1000));
        assert!(log.append(&luis.id, picture(None), 4).is_err());
        let too_big = vec![1; MAX_PICTURE_BYTES + 1];
        assert!(log.append(&ana.id, picture(Some(too_big)), 4).is_err());
        log.append(&ana.id, picture(None), 5).unwrap();
        assert!(log.state().picture.is_none());
    }

    #[test]
    fn nobody_starts_a_known_group_over() {
        let (ana, mallory) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let (start, _) = Rotation::new(&log.group_id(), [mallory.key()]).unwrap();
        // However many tries it takes to sort before the real one, it stays out.
        let mut forged = None;
        for time in 0.. {
            let entry = Entry::sign(
                &mallory.id,
                log.group_id(),
                vec![],
                time,
                Body::Create {
                    name: "taken".into(),
                    start: start.clone(),
                },
            );
            if log.ordered().first().is_some_and(|h| entry.hash() < *h) {
                forged = Some(entry);
                break;
            }
        }
        let forged = forged.unwrap();
        assert_eq!(
            log.insert(forged.clone()),
            Err(StateError::NotAllowed("second create"))
        );
        assert!(log.insert_all([forged.clone()]).is_empty());
        let state = log.state();
        assert_eq!((state.creator, state.name.as_str()), (Some(ana.key()), "g"));
        // A device that only knows the id takes the first group it is given.
        let mut fresh = GroupLog::empty(log.group_id());
        fresh.insert_all(log.entries_in_order().into_iter().cloned());
        assert!(fresh.insert(forged).is_err());
        assert_eq!(fresh.state().creator, Some(ana.key()));
    }

    /// A log read back takes each entry's hash from the bytes it checks, and the batch
    /// hashes no entry again: a log of thousands is read at every start. Working out the
    /// group's state hashes each entry it applies once more, and no more than that.
    #[test]
    fn a_batch_is_taken_in_without_hashing_its_entries_again() {
        let ana = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        for i in 0..50u64 {
            log.append(
                &ana.id,
                Body::Rename {
                    name: format!("g{i}"),
                },
                2 + i,
            )
            .unwrap();
        }
        let entries: Vec<Entry> = log.entries_in_order().into_iter().cloned().collect();
        let mut back = GroupLog::empty(log.group_id());
        let before = crate::entry::HASHED.with(std::cell::Cell::get);
        assert_eq!(back.insert_all(entries).len(), log.len());
        let hashed = crate::entry::HASHED.with(std::cell::Cell::get) - before;
        assert!(
            hashed <= log.len(),
            "{hashed} hashes for {} entries",
            log.len()
        );
    }

    #[test]
    fn a_long_log_reads_back_whole_in_any_order() {
        let (ana, luis) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        for i in 0..(MAX_WAITING as u64 + 50) {
            let name = format!("g{i}");
            log.append(&ana.id, Body::Rename { name }, 3 + i).unwrap();
        }
        log.remove(&ana.id, luis.key(), 1000).unwrap();
        let state = log.state();
        assert!(!state.is_member(&luis.key()));
        // Newest first: every entry but the first waits for its parent until the end.
        let mut back = GroupLog::empty(log.group_id());
        let mut entries: Vec<Entry> = log.entries_in_order().into_iter().cloned().collect();
        entries.reverse();
        back.insert_all(entries);
        assert_eq!(back.len(), log.len());
        assert!(!back.state().is_member(&luis.key()));
        assert_eq!(back.state().name, state.name);
        // The stored order has no entry before its parents.
        let order = log.hashes_in_order();
        assert_eq!(order, log.ordered());
    }

    #[test]
    fn a_made_up_group_lacks_the_invite() {
        let (ana, mallory, eva) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let (ticket, _) = log.invite(&ana.id, 100, 2).unwrap();
        assert!(log.invited_with(&ticket));
        // The same id, with someone else at the root who adds the joining device.
        let (start, _) = Rotation::new(&log.group_id(), [mallory.key()]).unwrap();
        let mut fake = GroupLog::empty(log.group_id());
        let create = Entry::sign(
            &mallory.id,
            log.group_id(),
            vec![],
            1,
            Body::Create {
                name: "g".into(),
                start,
            },
        );
        fake.insert(create).unwrap();
        fake.add(&mallory.id, eva.key(), "eva", 2).unwrap();
        // Even with the real entries mixed in, the invite never applies there.
        fake.insert_all(log.entries_in_order().into_iter().cloned());
        assert!(fake.state().is_member(&eva.key()));
        assert!(!fake.invited_with(&ticket));
        // Another invite of the same admin is not this one.
        let other = log.issue_ticket(&ana.id, 100).unwrap();
        assert!(!log.invited_with(&other));
    }

    #[test]
    fn a_removed_device_sees_up_to_its_removal() {
        let (ana, luis, eva) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        assert!(log.until_removed(&luis.key()).is_empty());
        let removal = log.remove(&ana.id, luis.key(), 3).unwrap();
        log.add(&ana.id, eva.key(), "eva", 4).unwrap();
        log.append(&ana.id, Body::Rename { name: "h".into() }, 5)
            .unwrap();
        let shown = log.until_removed(&luis.key());
        assert_eq!(shown.len(), 3);
        assert_eq!(shown.last().map(|e| e.hash()), Some(removal.hash()));
        assert!(shown.iter().all(|e| !matches!(&e.body,
            Body::Add { member, .. } if *member == eva.key())));
        // A device that left by itself is shown nothing.
        log.append(&eva.id, Body::Leave, 6).unwrap();
        assert!(log.until_removed(&eva.key()).is_empty());
    }

    #[test]
    fn names_that_hide_marks_or_run_long_are_refused() {
        let (ana, luis) = (Device::new(), Device::new());
        assert!(GroupLog::create(&ana.id, "a\u{202E}gnp.exe", 1).is_err());
        assert!(GroupLog::create(&ana.id, &"g".repeat(61), 1).is_err());
        assert!(GroupLog::create(&ana.id, "  ", 1).is_err());
        let mut log = GroupLog::create(&ana.id, &"g".repeat(60), 1).unwrap();
        let rename = |name: &str| Body::Rename { name: name.into() };
        assert!(log.append(&ana.id, rename("x\u{200B}y"), 2).is_err());
        assert!(log.append(&ana.id, rename(&"h".repeat(61)), 2).is_err());
        log.append(&ana.id, rename("Family 👨\u{200D}👩\u{200D}👧"), 2)
            .unwrap();
        assert!(log.add(&ana.id, luis.key(), &"l".repeat(41), 3).is_err());
        assert!(log.add(&ana.id, luis.key(), "lu\u{2066}is", 3).is_err());
        log.add(&ana.id, luis.key(), "luis", 3).unwrap();
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        let eva = Device::new();
        let long = Entry::sign(
            &eva.id,
            log.group_id(),
            log.heads(),
            4,
            Body::Join {
                ticket,
                name: "e".repeat(60_000),
            },
        );
        assert!(log.insert(long).is_err());
        assert!(!log.state().is_member(&eva.key()));
    }

    #[test]
    fn names_that_show_nothing_count_as_empty_and_padding_goes() {
        let (ana, luis, eva) = (Device::new(), Device::new(), Device::new());
        for blank in [
            "\u{3164}",
            "\u{2800}\u{115F}",
            "\u{0301}",
            "\u{FE0F}\u{200D}",
        ] {
            assert!(GroupLog::create(&ana.id, blank, 1).is_err(), "{blank:?}");
        }
        let mut log = GroupLog::create(&ana.id, "\u{3164}Friends\u{2800}", 1).unwrap();
        assert_eq!(log.state().name, "Friends");
        // Made here, names are signed as they show.
        let rename = |name: &str| Body::Rename { name: name.into() };
        let entry = log
            .append(&ana.id, rename("Club\u{3000}\u{3164}"), 2)
            .unwrap();
        assert!(matches!(&entry.body, Body::Rename { name } if name == "Club"));
        assert!(log.append(&ana.id, rename("\u{FFA0}\u{FFA0}"), 3).is_err());
        let room = |name: &str| Body::SetRoom {
            id: [4; 8],
            name: name.into(),
        };
        assert!(log.append(&ana.id, room("\u{2003}\u{180E}"), 3).is_err());
        log.append(&ana.id, room("Games\u{3164}\u{3164}"), 3)
            .unwrap();
        log.add(&ana.id, luis.key(), "\u{3164}luis", 4).unwrap();
        let state = log.state();
        assert_eq!(state.rooms[1].name, "Games");
        assert_eq!(state.members[&luis.key()].name, "luis");

        // Made elsewhere (an older version, or on purpose): a blank rename is refused like an
        // empty one, padding is not shown, and a blank member name is an empty one.
        let raw = |log: &GroupLog, who: &Device, time, body| {
            Entry::sign(&who.id, log.group_id(), log.heads(), time, body)
        };
        let blank = raw(&log, &ana, 5, rename("\u{3164}\u{2800}"));
        assert!(log.insert(blank).is_err());
        assert_eq!(log.state().name, "Club");
        let padded = raw(&log, &ana, 6, rename("Ana\u{3164}\u{3164}\u{3164}"));
        log.insert(padded).unwrap();
        assert_eq!(log.state().name, "Ana");
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        let join = raw(
            &log,
            &eva,
            7,
            Body::Join {
                ticket,
                name: "\u{3164}\u{3164}".into(),
            },
        );
        log.insert(join).unwrap();
        log.welcome(&ana.id, eva.key(), 8).unwrap();
        let state = log.state();
        assert!(state.is_member(&eva.key()));
        assert_eq!(state.members[&eva.key()].name, "");
    }

    /// Signs `body` as `who` on `parents`, at the first time from `start` whose entry sorts
    /// before `rival` among entries ready together: what an attacker would grind for.
    fn sorting_before(
        log: &GroupLog,
        who: &Device,
        parents: Vec<EntryHash>,
        body: Body,
        rival: EntryHash,
    ) -> Entry {
        (1_000u64..)
            .map(|time| Entry::sign(&who.id, log.group_id(), parents.clone(), time, body.clone()))
            .find(|entry| entry.hash() < rival)
            .unwrap()
    }

    #[test]
    fn a_removed_admin_cannot_sign_into_the_past() {
        let (ana, bob, carl) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bob.key(), "bob", 2).unwrap();
        log.add(&ana.id, carl.key(), "carl", 3).unwrap();
        let admin = Body::Promote { member: bob.key() };
        log.append(&ana.id, admin, 4).unwrap();
        let before = log.heads();
        let past = log.clone();
        // Ana demotes and removes Bob.
        let demote = Body::Demote { member: bob.key() };
        let demoted = log.append(&ana.id, demote, 5).unwrap();
        log.remove(&ana.id, bob.key(), 6).unwrap();
        assert!(!log.state().is_member(&bob.key()));

        // Bob renames the group on what he saw as an admin, sorted before his demotion.
        let rename = Body::Rename {
            name: "taken".into(),
        };
        let renamed = sorting_before(&log, &bob, before.clone(), rename, demoted.hash());
        log.insert(renamed).unwrap();
        assert_eq!(log.state().name, "g");

        // And removes Carl the same way.
        let mut bobs = past.clone();
        let removal = bobs.remove(&bob.id, carl.key(), 7).unwrap();
        log.insert(removal).unwrap();
        assert!(log.state().is_member(&carl.key()));

        // An invite Bob wrote then lets nobody in either.
        let ticket = past.issue_ticket(&bob.id, u64::MAX).unwrap();
        let eve = Device::new();
        let join = Body::Join {
            ticket,
            name: "eve".into(),
        };
        let joined = sorting_before(&log, &eve, before, join, demoted.hash());
        log.insert(joined).unwrap();
        assert!(!log.state().is_member(&eve.key()));
        // Ana's own entries still apply, then and later.
        log.append(&ana.id, Body::Rename { name: "h".into() }, 8)
            .unwrap();
        assert_eq!(log.state().name, "h");
        assert!(log.state().is_member(&carl.key()));
    }

    #[test]
    fn what_a_member_did_before_its_removal_stays() {
        let (ana, bob) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bob.key(), "bob", 2).unwrap();
        let admin = Body::Promote { member: bob.key() };
        log.append(&ana.id, admin, 3).unwrap();
        // Seen by Ana before she removes him: it stays.
        log.append(
            &bob.id,
            Body::Rename {
                name: "bob's".into(),
            },
            4,
        )
        .unwrap();
        log.remove(&ana.id, bob.key(), 5).unwrap();
        assert_eq!(log.state().name, "bob's");
        // Let back in later, he acts again.
        log.add(&ana.id, bob.key(), "bob", 6).unwrap();
        let admin = Body::Promote { member: bob.key() };
        log.append(&ana.id, admin, 7).unwrap();
        log.append(
            &bob.id,
            Body::Rename {
                name: "again".into(),
            },
            8,
        )
        .unwrap();
        assert_eq!(log.state().name, "again");
    }

    #[test]
    fn entries_come_only_from_devices_the_log_named() {
        let (ana, luis) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, luis.key(), "luis", 2).unwrap();
        // A key made up for it: refused, however many.
        let made_up = Device::new();
        let junk = Entry::sign(
            &made_up.id,
            log.group_id(),
            log.heads(),
            3,
            Body::Rename { name: "x".into() },
        );
        assert!(log.insert(junk.clone()).is_err());
        assert!(log.insert_all([junk]).is_empty());
        // A join needs an invite of a named device.
        let stranger_ticket = Ticket::issue(&made_up.id, log.group_id(), u64::MAX).unwrap();
        let join = Entry::sign(
            &Device::new().id,
            log.group_id(),
            log.heads(),
            4,
            Body::Join {
                ticket: stranger_ticket,
                name: "j".into(),
            },
        );
        assert!(log.insert(join).is_err());
        // Read back in any order, a whole log keeps every entry.
        let mut entries: Vec<Entry> = log.entries().cloned().collect();
        entries.reverse();
        let mut back = GroupLog::empty(log.group_id());
        assert_eq!(back.insert_all(entries).len(), log.len());
    }

    #[test]
    fn spare_entries_stay_within_their_room() {
        let ana = Device::new();
        let others: Vec<Device> = (0..12).map(|_| Device::new()).collect();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        for (i, d) in others.iter().enumerate() {
            log.add(&ana.id, d.key(), "m", 2 + i as u64).unwrap();
        }
        let group_id = log.group_id();
        let bytes = |log: &GroupLog, hashes: &[EntryHash]| -> usize {
            hashes.iter().map(|h| log.entries[h].encode().len()).sum()
        };
        let of = |log: &GroupLog, d: &Device| -> Vec<EntryHash> {
            log.spare()
                .into_iter()
                .filter(|h| log.entries[h].author == d.key())
                .collect()
        };
        // One member writes entries that apply to nothing, small and then large, on what
        // everyone signs on: the newest are kept, the oldest go past its share.
        let mut written = Vec::new();
        for i in 0..100u64 {
            let body = vec![b'x'; if i < 80 { 10 } else { 40_000 }];
            let entry = newer_kind_of(&others[0], group_id, log.heads(), 100 + i, &body);
            written.push(entry.hash());
            log.insert(entry).unwrap();
            log.trim();
        }
        let theirs = of(&log, &others[0]);
        assert!(theirs.len() <= MAX_SPARE_PER_AUTHOR);
        assert!(bytes(&log, &theirs) <= MAX_SPARE_BYTES_PER_AUTHOR);
        assert!(log.get(&written[99]).is_some() && log.get(&written[0]).is_none());
        // Their big ones past the bytes they may keep go too.
        let big = &others[1];
        for i in 0..30u64 {
            let entry = newer_kind_of(big, group_id, log.heads(), 300 + i, &vec![b'y'; 60_000]);
            log.insert(entry).unwrap();
        }
        // Kept as they come, until the node trims the log.
        assert_eq!(of(&log, big).len(), 30);
        log.trim();
        let theirs = of(&log, big);
        assert!(bytes(&log, &theirs) <= MAX_SPARE_BYTES_PER_AUTHOR);
        assert_eq!(theirs.len(), MAX_SPARE_BYTES_PER_AUTHOR / 60_000);
        // Many members at once: all together stay within the room too.
        for (k, d) in others.iter().enumerate().skip(2) {
            for i in 0..MAX_SPARE_PER_AUTHOR as u64 {
                let entry = newer_kind_of(d, group_id, log.heads(), 1000 * k as u64 + i, b"z");
                log.insert(entry).unwrap();
            }
        }
        log.trim();
        let unused = log.spare();
        assert!(unused.len() <= MAX_SPARE);
        assert!(bytes(&log, &unused) <= MAX_SPARE_BYTES);
        // What applies is all there.
        let state = log.state();
        assert_eq!(state.members.len(), 1 + others.len());
        assert!(state.applied.iter().all(|h| log.get(h).is_some()));
        // Read back, the log keeps what it kept.
        let mut back = GroupLog::empty(group_id);
        back.insert_all(log.entries_in_order().into_iter().cloned());
        assert_eq!(back.len(), log.len());
        assert_eq!(back.state().applied, state.applied);
    }

    #[test]
    fn a_spare_entry_that_comes_to_be_needed_is_kept() {
        // Bea writes an entry of a newer kind; Ana's copy has room for none and lets it
        // go. A later entry of Bea's, signed on it, makes it needed: sent again, it is
        // kept, and the later one applies.
        let (ana, bea) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "bea", 2).unwrap();
        let mut beas = log.clone();
        let newer = newer_kind(&bea, log.group_id(), beas.heads(), 3);
        beas.insert(newer.clone()).unwrap();
        let mut parents = beas.heads();
        parents.push(newer.hash());
        let leave = Entry::sign(&bea.id, log.group_id(), parents, 4, Body::Leave);
        beas.insert(leave.clone()).unwrap();
        assert!(!beas.state().is_member(&bea.key()));
        assert!(beas.spare().is_empty());
        log.set_spare_room(0, 0);
        log.insert(newer.clone()).unwrap();
        // Kept until the log is trimmed, then let go.
        assert!(log.get(&newer.hash()).is_some());
        assert!(log.trim());
        assert!(log.get(&newer.hash()).is_none());
        // The leave waits for it.
        log.insert(leave).unwrap();
        assert!(log.state().is_member(&bea.key()));
        assert_ne!(log.needed_heads(), beas.needed_heads());
        let have: BTreeSet<EntryHash> = log.hashes().copied().collect();
        let lacking: Vec<Entry> = beas
            .entries_in_order()
            .into_iter()
            .filter(|e| !have.contains(&e.hash()))
            .cloned()
            .collect();
        log.insert_all(lacking);
        assert!(!log.state().is_member(&bea.key()));
        assert!(log.get(&newer.hash()).is_some() && log.spare().is_empty());
    }

    #[test]
    fn only_so_many_entries_wait_for_unknown_parents() {
        let ana = Device::new();
        let others: Vec<Device> = (0..5).map(|_| Device::new()).collect();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        for (i, d) in others.iter().enumerate() {
            log.add(&ana.id, d.key(), "m", 2 + i as u64).unwrap();
        }
        let applied = log.len();
        let group_id = log.group_id();
        let orphan = |by: &Device, i: u64, time: u64| {
            Entry::sign(
                &by.id,
                group_id,
                vec![[i as u8; 32], [(i >> 8) as u8 + 1; 32]],
                time,
                Body::Rename { name: "x".into() },
            )
        };
        // One member sends many, dated so that the newest come first: those that came
        // first stay, whatever their time says.
        let first: Vec<Entry> = (0..300)
            .map(|i| orphan(&others[0], i, 10_000 - i))
            .collect();
        for entry in &first {
            log.insert(entry.clone()).unwrap();
        }
        assert_eq!(log.waiting().len(), MAX_WAITING_PER_AUTHOR);
        assert!(
            first[..MAX_WAITING_PER_AUTHOR]
                .iter()
                .all(|e| log.get(&e.hash()).is_some())
        );
        // The others' entries still find room: the one with the most makes it.
        for (k, d) in others.iter().enumerate().skip(1) {
            for i in 0..60 {
                log.insert(orphan(d, 1000 * k as u64 + i, 1)).unwrap();
            }
        }
        let waiting = log.waiting();
        assert_eq!(waiting.len(), MAX_WAITING);
        for d in &others {
            let theirs = waiting
                .iter()
                .filter(|h| log.get(h).unwrap().author == d.key())
                .count();
            assert!((51..=52).contains(&theirs), "{theirs}");
        }
        // The applied log is untouched.
        assert_eq!(log.state().name, "g");
        assert_eq!(log.len(), MAX_WAITING + applied);
        // An entry that is too long is refused before anything else.
        let long = Entry::sign(
            &ana.id,
            log.group_id(),
            log.heads(),
            2,
            Body::Rename {
                name: "x".repeat(MAX_ENTRY_BYTES),
            },
        );
        assert_eq!(log.insert(long), Err(StateError::TooLong));
    }

    #[test]
    fn a_batch_costs_each_entry_a_few_checks_whatever_its_order() {
        // Devices that each joined with an invite of the one before, let in by Ana, sent
        // last to first: each is named only once the one before it is.
        let ana = Device::new();
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        let mut made = log.clone();
        let mut ticket = log.issue_ticket(&ana.id, 1_000).unwrap();
        let mut chain: Vec<Entry> = Vec::new();
        // As many as the group takes, each joining on the one before.
        for _ in 1..crate::state::MAX_MEMBERS {
            let joiner = Device::new();
            let entry = Entry::sign(
                &joiner.id,
                log.group_id(),
                made.heads(),
                10,
                Body::Join {
                    ticket: ticket.clone(),
                    name: "k".into(),
                },
            );
            made.insert(entry.clone()).unwrap();
            let welcome = made.let_in(&ana.id, &entry.hash(), 10).unwrap();
            chain.push(entry);
            chain.push(welcome);
            ticket = Ticket::issue(&joiner.id, log.group_id(), 1_000).unwrap();
        }
        chain.reverse();
        let n = chain.len();
        log.tries = 0;
        assert_eq!(log.insert_all(chain).len(), n);
        assert!(log.tries <= 2 * n, "{} checks for {n} entries", log.tries);
        assert_eq!(
            log.state().members.len(),
            usize::from(crate::state::MAX_MEMBERS)
        );
    }

    #[test]
    fn entries_from_devices_never_named_cost_no_signature_check() {
        let (ana, bea) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        let made_up = Device::new();
        let forged: Vec<Entry> = (0..2000)
            .map(|i| {
                Entry::sign(
                    &made_up.id,
                    log.group_id(),
                    log.heads(),
                    10 + i,
                    Body::Leave,
                )
            })
            .collect();
        log.verified = 0;
        assert!(log.insert_all(forged).is_empty());
        assert_eq!(log.verified, 0);
        // Nor does a join with a ticket of a device never named, nor an entry already here.
        let ticket = Ticket::issue(&made_up.id, log.group_id(), u64::MAX).unwrap();
        let join = Entry::sign(
            &Device::new().id,
            log.group_id(),
            log.heads(),
            4,
            Body::Join {
                ticket,
                name: "j".into(),
            },
        );
        assert!(log.insert(join).is_err());
        let again: Vec<Entry> = log.entries().cloned().collect();
        assert!(log.insert_all(again).is_empty());
        assert_eq!(log.verified, 0);
    }

    #[test]
    fn nothing_is_signed_on_entries_that_apply_to_nothing_and_parents_stay_few() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 4)
            .unwrap();
        let applied = log.state().applied.len();
        // Bea, who does not manage, renames the group thousands of times at once: more than
        // an entry could name as parents.
        let heads = log.heads();
        let at_once: Vec<Entry> = (0..2100)
            .map(|i| {
                Entry::sign(
                    &bea.id,
                    log.group_id(),
                    heads.clone(),
                    10 + i,
                    Body::Rename { name: "x".into() },
                )
            })
            .collect();
        log.insert_all(at_once);
        assert_eq!(log.state().applied.len(), applied);
        // What is written next names none of them, so it is as short as ever.
        assert_eq!(log.heads(), heads);
        let next = log
            .append(&ana.id, Body::Rename { name: "h".into() }, 9000)
            .unwrap();
        assert_eq!(next.parents, heads);
        // Changes that apply, made at once, are named a few at a time.
        let base = log.heads();
        let mut sides = Vec::new();
        for i in 0..100 {
            sides.push(Entry::sign(
                &ana.id,
                log.group_id(),
                base.clone(),
                10_000 + i,
                Body::Demote { member: leo.key() },
            ));
        }
        log.insert_all(sides);
        assert_eq!(log.heads().len(), MAX_PARENTS);
        let a = log
            .append(&ana.id, Body::Rename { name: "a".into() }, 20_000)
            .unwrap();
        assert_eq!(a.parents.len(), MAX_PARENTS);
        let b = log
            .append(&ana.id, Body::Rename { name: "b".into() }, 20_001)
            .unwrap();
        assert!(b.parents.len() < MAX_PARENTS);
        assert_eq!(log.heads().len(), 1);
        // An entry naming more parents than that is refused before its signature is
        // checked.
        let mut many: Vec<EntryHash> = log.hashes().copied().take(MAX_PARENTS + 1).collect();
        many.sort_unstable();
        let wide = Entry::sign(&ana.id, log.group_id(), many, 30_000, Body::Leave);
        log.verified = 0;
        assert_eq!(
            log.insert(wide),
            Err(StateError::NotAllowed("too many parents"))
        );
        assert_eq!(log.verified, 0);
    }

    #[test]
    fn an_entry_on_a_parent_nobody_has_holds_up_no_later_entry() {
        let (ana, bob) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bob.key(), "bob", 2).unwrap();
        let mut parents = log.heads();
        parents.push([0xEE; 32]);
        parents.sort();
        log.insert(Entry::sign(
            &bob.id,
            log.group_id(),
            parents,
            u64::MAX,
            Body::Rename { name: "x".into() },
        ))
        .unwrap();
        assert_eq!(log.waiting().len(), 1);
        log.remove(&ana.id, bob.key(), 3).unwrap();
        assert!(!log.state().is_member(&bob.key()));
        assert_eq!(log.stored_heads().len(), 2);
    }

    #[test]
    fn only_a_manager_gives_the_key_that_lets_a_device_in() {
        let (ana, bob, carl) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, bob.key(), "bob", 2).unwrap();
        log.add(&ana.id, carl.key(), "carl", 3).unwrap();
        let state = log.state();
        assert!(state.keyed(&ana.key()) && state.keyed(&bob.key()));
        // Carl is removed, and comes back with a ticket Bob saw in the log.
        log.remove(&ana.id, carl.key(), 4).unwrap();
        let (ticket, _) = log.invite(&ana.id, 100, 5).unwrap();
        join(&mut log, &carl, ticket.clone(), 6);
        assert!(!log.state().is_member(&carl.key()));
        // Bob cannot let him in nor hand him the key, though he held one before; Ana can.
        assert!(log.welcome(&bob.id, carl.key(), 7).is_err());
        assert!(!log.state().is_member(&carl.key()));
        log.welcome(&ana.id, carl.key(), 8).unwrap();
        assert!(log.state().keyed(&carl.key()));
        assert_eq!(log.join_ticket_of(&carl.key()), Some(&ticket));
    }

    /// An entry of a kind this version does not know, signed by `author` on `parents` the
    /// way a newer version would write it: kind 200 and a body of its own.
    fn newer_kind(author: &Device, group_id: GroupId, parents: Vec<EntryHash>, time: u64) -> Entry {
        newer_kind_of(
            author,
            group_id,
            parents,
            time,
            b"something only newer copies read",
        )
    }

    /// An entry of a kind this version does not know, carrying `body`.
    fn newer_kind_of(
        author: &Device,
        group_id: GroupId,
        mut parents: Vec<EntryHash>,
        time: u64,
        body: &[u8],
    ) -> Entry {
        parents.sort_unstable();
        parents.dedup();
        let mut w = fogata_crypto::codec::Writer::default();
        w.group(&group_id).u32(parents.len() as u32);
        for parent in &parents {
            w.raw(parent);
        }
        w.key(&author.key()).u64(time).u8(200).bytes(body).u32(7);
        let mut bytes = w.into_bytes();
        let mut signed = b"fogata/v1/entry".to_vec();
        signed.extend_from_slice(&bytes);
        bytes.extend_from_slice(&author.id.sign(&signed));
        Entry::decode(&bytes).expect("an entry of a newer kind reads")
    }

    /// What every member must agree on.
    fn summary(log: &GroupLog) -> impl PartialEq + std::fmt::Debug {
        let state = log.state();
        (
            state.name,
            state.members,
            state.epochs,
            state.rooms,
            state.applied,
            state.rejected,
            log.hashes_in_order(),
        )
    }

    #[test]
    fn entries_of_a_newer_kind_are_kept_passed_on_and_never_stop_the_log() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut a = GroupLog::create(&ana.id, "friends", 1).unwrap();
        a.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        a.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        // Copies what one has to the other, as a sync does: in the order to send.
        let sync = |from: &GroupLog, to: &mut GroupLog| {
            to.insert_all(from.entries_in_order().into_iter().cloned());
        };
        let mut b = GroupLog::empty(a.group_id());
        sync(&a, &mut b);
        // Bea runs a newer version and writes an entry of a kind this one does not know.
        let before = b.heads();
        let newer = newer_kind(&bea, b.group_id(), before.clone(), 4);
        let hash = newer.hash();
        assert_eq!(Entry::decode(&newer.encode()).unwrap(), newer);
        assert!(b.insert(newer).unwrap());
        // It applies to nothing here, so nothing this version writes is signed on it.
        assert_eq!(b.heads(), before);
        // A newer copy writes on top of it, and that applies here.
        let on_top = Entry::sign(
            &bea.id,
            b.group_id(),
            vec![hash],
            4,
            Body::Rename {
                name: "on top".into(),
            },
        );
        assert!(b.insert(on_top.clone()).unwrap());
        assert_eq!(b.state().name, "on top");
        // Ana gets them and writes on top; Ana rotates the key on Bea's copy on top of that.
        sync(&b, &mut a);
        let rename = a
            .append(&ana.id, Body::Rename { name: "new".into() }, 5)
            .unwrap();
        assert!(rename.parents.contains(&on_top.hash()));
        assert!(!rename.parents.contains(&hash));
        a.append(
            &ana.id,
            Body::SetRoom {
                id: [3; 8],
                name: "Music".into(),
            },
            6,
        )
        .unwrap();
        sync(&a, &mut b);
        b.rotate(&ana.id, 7).unwrap();
        sync(&b, &mut a);
        // Leo reads the whole log in the opposite order, as from disk or a sync.
        let mut l = GroupLog::empty(a.group_id());
        let mut entries: Vec<Entry> = b.entries().cloned().collect();
        entries.reverse();
        assert_eq!(l.insert_all(entries).len(), b.len());
        for log in [&a, &b, &l] {
            let state = log.state();
            assert_eq!(state.name, "new");
            assert_eq!(state.rooms.len(), 2);
            assert_eq!(state.epochs.len(), 2);
            assert!(log.waiting().is_empty());
            // Kept as it came, but it changed nothing.
            assert_eq!(
                log.get(&hash).unwrap().encode(),
                b.get(&hash).unwrap().encode()
            );
            assert!(!state.applied.contains(&hash));
            assert!(state.rejected.iter().any(|(h, _)| *h == hash));
        }
        assert_eq!(summary(&a), summary(&b));
        assert_eq!(summary(&a), summary(&l));
        let keys = |log: &GroupLog, device: &Device| {
            log.current_keys(&device.id)
                .map(|k| (k.epoch(), *k.rendezvous_secret()))
        };
        assert!(keys(&a, &ana).is_some());
        assert_eq!(keys(&a, &ana), keys(&b, &bea));
        assert_eq!(keys(&a, &ana), keys(&l, &leo));
    }

    /// `entry` as a version from before MLS groups reads it: its body as a kind it does not
    /// know, the same bytes.
    fn as_older_copy_reads(entry: &Entry) -> Entry {
        let mut older = entry.clone();
        if entry.body.is_mls() {
            let mut w = fogata_crypto::codec::Writer::default();
            entry.body.write(&mut w);
            let bytes = w.into_bytes();
            older.body = Body::Unknown {
                kind: bytes[0],
                bytes: bytes[1..].to_vec(),
            };
            assert_eq!(older.encode(), entry.encode());
        }
        older
    }

    #[test]
    fn mls_entries_change_nothing_yet_and_older_copies_see_the_same_group() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "friends", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        let ticket = log.issue_ticket(&ana.id, 100).unwrap();
        let mut hashes = Vec::new();
        let mut time = 3;
        // Those of them their authors could make are kept; Commits on Commits nobody has,
        // a join by a member, a first Commit adding a stranger are not.
        let mut kept = 0;
        for body in crate::entry::tests::mls_bodies(&ana.id, log.group_id()) {
            let author = if time % 2 == 0 { &ana } else { &bea };
            let entry = Entry::sign(&author.id, log.group_id(), log.heads(), time, body);
            if log.insert(entry.clone()).is_ok() {
                hashes.push(entry.hash());
                kept += 1;
            }
            time += 1;
        }
        assert_eq!(kept, 1);
        let start = Entry::sign(
            &ana.id,
            log.group_id(),
            log.heads(),
            time,
            Body::Commit {
                link: crate::entry::EpochLink {
                    epoch: 0,
                    prev: None,
                },
                changes: crate::entry::Changes::new([bea.key()], []),
                payload: crate::entry::payload_digest(b"commit"),
                welcome: Some(crate::entry::payload_digest(b"welcome")),
            },
        );
        hashes.push(start.hash());
        log.insert(start.clone()).unwrap();
        let update = Entry::sign(
            &bea.id,
            log.group_id(),
            log.heads(),
            time + 1,
            Body::Commit {
                link: crate::entry::EpochLink {
                    epoch: 1,
                    prev: Some(start.hash()),
                },
                changes: crate::entry::Changes::default(),
                payload: crate::entry::payload_digest(b"update"),
                welcome: None,
            },
        );
        hashes.push(update.hash());
        log.insert(update).unwrap();
        time += 2;
        // Leo joins the group with its KeyPackage: not a join here, so whatever it signs is
        // from a device the log never named, as in older copies.
        let join = Entry::sign(
            &leo.id,
            log.group_id(),
            log.heads(),
            time,
            Body::MlsJoin {
                ticket,
                name: "Leo".into(),
                key_package: vec![3; 369],
            },
        );
        assert_eq!(log.insert(join), Err(StateError::NotAllowed(UNNAMED)));
        // Written on top of them, the rest of the group goes on.
        let rename = log
            .append(&ana.id, Body::Rename { name: "new".into() }, 20)
            .unwrap();
        log.rotate(&ana.id, 21).unwrap();
        let state = log.state();
        assert_eq!(state.name, "new");
        assert!(state.applied.contains(&rename.hash()));
        assert_eq!(state.epochs.len(), 2);
        for hash in &hashes {
            assert!(state.rejected.contains(&(*hash, crate::state::MLS_NOT_YET)));
        }
        // An older copy keeps the same entries and sees the same group.
        let mut older = GroupLog::empty(log.group_id());
        let mut entries: Vec<Entry> = log.entries().map(as_older_copy_reads).collect();
        entries.reverse();
        assert_eq!(older.insert_all(entries).len(), log.len());
        assert!(
            older
                .entries()
                .any(|e| matches!(e.body, Body::Unknown { .. }))
        );
        let other = older.state();
        assert_eq!(log.hashes_in_order(), older.hashes_in_order());
        assert_eq!(
            (
                &state.members,
                &state.epochs,
                &state.applied,
                state.name.as_str()
            ),
            (
                &other.members,
                &other.epochs,
                &other.applied,
                other.name.as_str()
            )
        );
        let refused = |s: &GroupState| s.rejected.iter().map(|(h, _)| *h).collect::<Vec<_>>();
        assert_eq!(refused(&state), refused(&other));
        assert_eq!(
            log.current_keys(&bea.id).map(|k| k.epoch()),
            older.current_keys(&bea.id).map(|k| k.epoch())
        );
    }

    #[test]
    fn entries_of_a_newer_kind_follow_the_same_limits() {
        let (ana, bea) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        // From a device the log never named: refused.
        let stranger = Device::new();
        let made_up = newer_kind(&stranger, log.group_id(), log.heads(), 3);
        assert_eq!(log.insert(made_up), Err(StateError::NotAllowed(UNNAMED)));
        // Forged: refused.
        let mut forged = newer_kind(&bea, log.group_id(), log.heads(), 3);
        forged.time_ms = 4;
        assert_eq!(log.insert(forged), Err(StateError::BadSignature));
        // On parents nobody has: they wait, and no more than any other entry.
        for i in 0..(MAX_WAITING as u64 + 20) {
            let orphan = newer_kind(
                &bea,
                log.group_id(),
                vec![[i as u8; 32], [(i >> 8) as u8 + 1; 32]],
                100 + i,
            );
            log.insert(orphan).unwrap();
        }
        assert_eq!(log.waiting().len(), MAX_WAITING_PER_AUTHOR);
        assert_eq!(log.len(), MAX_WAITING_PER_AUTHOR + 2);
        assert_eq!(log.state().members.len(), 2);
    }

    /// `rotation` with the boxes of `victims` replaced by ones that open to nothing.
    fn spoil(mut rotation: Rotation, victims: &[DevicePublicKey]) -> Rotation {
        for (key, sealed) in rotation.sealed.iter_mut() {
            if victims.contains(key) {
                *sealed = Sealed::seal(key, b"elsewhere", &[7; 32]).unwrap();
            }
        }
        rotation
    }

    #[test]
    fn a_broken_key_is_told_apart_and_older_kept_keys_still_count() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        let first = epoch_number(&log.state().epochs[0].id);
        assert!(log.state().epoch_kept(first));
        for device in [&ana, &bea, &leo] {
            assert_eq!(log.current_key_problem(&device.id, &log.state()), None);
        }
        // Leo, an admin, starts an epoch whose secret only he can open.
        log.append(&ana.id, Body::Promote { member: leo.key() }, 4)
            .unwrap();
        let members: Vec<DevicePublicKey> = log.state().members.keys().copied().collect();
        let (rotation, _) = Rotation::new(&log.group_id(), members).unwrap();
        let rotation = spoil(rotation, &[ana.key(), bea.key()]);
        log.append(&leo.id, Body::Rotate { rotation }, 4).unwrap();
        let state = log.state();
        assert_eq!(state.epochs.len(), 2);
        assert!(log.current_keys(&leo.id).is_some());
        assert!(log.current_keys(&bea.id).is_none());
        let broken = Some(KeyProblem {
            by: leo.key(),
            bad_box: true,
        });
        assert_eq!(log.current_key_problem(&bea.id, &state), broken);
        assert_eq!(log.current_key_problem(&ana.id, &state), broken);
        assert_eq!(log.current_key_problem(&leo.id, &state), None);
        // Everyone given the first epoch is still in: proofs with it still come from members.
        assert!(state.epoch_kept(first));
        assert!(!state.epoch_kept(first.wrapping_add(1)));
        // An admin's new rotation mends it.
        log.rotate(&ana.id, 5).unwrap();
        let state = log.state();
        assert_eq!(log.current_key_problem(&bea.id, &state), None);
        assert!(log.current_keys(&bea.id).is_some());
        // Once someone who had it leaves, an epoch is not kept any more.
        log.append(&leo.id, Body::Leave, 6).unwrap();
        let state = log.state();
        assert!(!state.epoch_kept(first));
        assert!(!state.epoch_kept(epoch_number(&state.epochs[2].id)));
    }

    #[test]
    fn a_member_left_out_of_an_epoch_is_told_who_started_it() {
        let (ana, bea) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        let (rotation, _) = Rotation::new(&log.group_id(), [ana.key()]).unwrap();
        log.append(&ana.id, Body::Rotate { rotation }, 3).unwrap();
        assert_eq!(
            log.current_key_problem(&bea.id, &log.state()),
            Some(KeyProblem {
                by: ana.key(),
                bad_box: false,
            })
        );
        log.welcome(&ana.id, bea.key(), 4).unwrap();
        assert_eq!(log.current_key_problem(&bea.id, &log.state()), None);
    }

    #[test]
    fn the_newest_banner_of_a_manager_shows() {
        let ana = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let gif = b"GIF89a".to_vec();
        let first = log.set_banner(&ana, gif.clone(), 10).unwrap();
        assert_eq!(log.banner(), Some(&first));
        // A member is no admin: neither its own banner nor one it signs elsewhere counts.
        assert!(log.set_banner(&bea, gif.clone(), 20).is_err());
        let mut other = GroupLog::create(&bea, "h", 1).unwrap();
        let foreign = other.set_banner(&bea, gif.clone(), 30).unwrap();
        assert!(!log.take_banner(foreign));
        let by_member = GroupBanner::sign(&bea, log.group_id(), gif.clone(), 40).unwrap();
        assert!(!log.take_banner(by_member));

        // Another copy of the log takes the newer one, not an older one after it.
        let mut copy = GroupLog::empty(log.group_id());
        copy.insert_all(log.entries().cloned());
        let second = log.set_banner(&ana, Vec::new(), 5).unwrap();
        assert!(second.time_ms > first.time_ms, "newer whatever the clock");
        assert_eq!(log.banner(), None, "taken away");
        assert!(copy.take_banner(second.clone()));
        assert!(!copy.take_banner(first));
        assert_eq!(copy.banner_record(), Some(&second));
    }

    #[test]
    fn a_record_dated_far_ahead_is_not_taken_nor_freezes_the_banner() {
        let ana = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let gif = b"GIF89a".to_vec();
        let now = 1_000_000;
        // Dated later than a day ahead: refused, banner or picture.
        let far = GroupBanner::sign(&ana, log.group_id(), gif.clone(), u64::MAX).unwrap();
        assert!(!log.take_banner_at(far.clone(), now));
        let ahead = now + MAX_RECORD_AHEAD_MS;
        let near = GroupBanner::sign(&ana, log.group_id(), gif.clone(), ahead).unwrap();
        assert!(log.take_banner_at(near.clone(), now));
        let group = log.group_id();
        let picture = |t| GroupPicture::sign(&ana, group, png(10, 0), t).unwrap();
        assert!(!log.take_picture_at(picture(ahead + 1), now));
        assert!(log.take_picture_at(picture(ahead), now));
        // One taken before (from a copy that took any) still lets a manager choose another,
        // dated by its clock so that the others take it.
        assert!(log.take_banner(far));
        let next = log.set_banner(&ana, Vec::new(), now).unwrap();
        assert_eq!(next.time_ms, now);
        let mut copy = GroupLog::empty(log.group_id());
        copy.insert_all(log.entries().cloned());
        assert!(copy.take_banner_at(next, now));
    }

    #[test]
    fn a_picture_entry_dated_far_ahead_freezes_nothing() {
        let (ana, bea) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "bea", 2).unwrap();
        // Bea, managing, sets a picture with an entry dated near the end of time; then she
        // is removed.
        let set = Body::SetPicture {
            picture: Some(png(500, 1)),
        };
        log.append(&bea.id, set, u64::MAX).unwrap();
        log.remove(&ana.id, bea.key(), 3).unwrap();
        let now = 1_000_000;
        let state = log.state();
        assert_eq!(log.picture_in(&state, now), Some(&png(500, 1)[..]));
        // A picture Ana chooses shows, here and on a copy that takes it.
        let mut copy = GroupLog::empty(log.group_id());
        copy.insert_all(log.entries().cloned());
        let (record, cleared) = log.set_picture(&ana.id, png(600, 2), now).unwrap();
        assert_eq!(record.time_ms, now + 1);
        copy.insert_all(cleared);
        assert!(copy.take_picture_at(record, now));
        for log in [&log, &copy] {
            assert_eq!(log.picture_in(&log.state(), now), Some(&png(600, 2)[..]));
        }
    }

    fn png(len: usize, fill: u8) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.resize(len, fill);
        png
    }

    #[test]
    fn the_newest_picture_of_a_manager_shows_and_writes_no_entry() {
        let ana = DeviceIdentity::generate().unwrap();
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let entries = log.len();
        let (first, cleared) = log.set_picture(&ana, png(1000, 1), 10).unwrap();
        assert!(cleared.is_none());
        assert_eq!(log.picture(), Some(png(1000, 1)));
        // Replaced and taken away: each only replaces the record, the log does not grow.
        let (second, _) = log.set_picture(&ana, png(2000, 2), 11).unwrap();
        assert_eq!(log.picture(), Some(png(2000, 2)));
        let (none, _) = log.set_picture(&ana, Vec::new(), 3).unwrap();
        assert!(none.time_ms > second.time_ms, "newer whatever the clock");
        assert_eq!(log.picture(), None);
        assert_eq!(log.picture_record(), Some(&none));
        assert_eq!(log.len(), entries);

        // Not an admin, too big, or not a picture: refused, nothing changes.
        assert!(log.set_picture(&bea, png(10, 0), 20).is_err());
        assert!(
            log.set_picture(&ana, png(crate::MAX_RECORD_PICTURE_BYTES + 1, 0), 20)
                .is_err()
        );
        assert!(log.set_picture(&ana, b"%PDF-1.7".to_vec(), 20).is_err());
        assert_eq!(log.picture_record(), Some(&none));
        let by_member = GroupPicture::sign(&bea, log.group_id(), png(10, 0), 40).unwrap();
        assert!(!log.take_picture(by_member));
        let mut other = GroupLog::create(&bea, "h", 1).unwrap();
        let (foreign, _) = other.set_picture(&bea, png(10, 0), 50).unwrap();
        assert!(!log.take_picture(foreign));

        // Another copy takes the newest only.
        let mut copy = GroupLog::empty(log.group_id());
        copy.insert_all(log.entries().cloned());
        assert!(copy.take_picture(second.clone()));
        assert!(copy.take_picture(none.clone()));
        assert!(!copy.take_picture(first));
        assert!(!copy.take_picture(second));
        assert_eq!(copy.picture(), None);
    }

    #[test]
    fn a_picture_set_with_an_entry_shows_until_a_newer_record() {
        let ana = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let set = |p: Option<Vec<u8>>| Body::SetPicture { picture: p };
        log.append(&ana, set(Some(png(500, 1))), 100).unwrap();
        assert_eq!(log.picture(), Some(png(500, 1)));

        // The record is newer than the entry even with a clock behind it, and one entry
        // with no picture goes first so that versions without records show the icon.
        let entries = log.len();
        let (record, cleared) = log.set_picture(&ana, png(600, 2), 50).unwrap();
        let cleared = cleared.expect("an entry taking the old picture away");
        assert_eq!(cleared.body, set(None));
        assert!(record.time_ms > cleared.time_ms && record.time_ms > 100);
        assert_eq!(log.len(), entries + 1);
        assert_eq!(log.state().picture, None);
        assert_eq!(log.picture(), Some(png(600, 2)));
        // Changing it again writes nothing more.
        log.set_picture(&ana, png(700, 3), 60).unwrap();
        assert_eq!(log.len(), entries + 1);
        assert_eq!(log.picture(), Some(png(700, 3)));

        // An admin on an earlier version sets one with an entry later: that one shows.
        let later = log.picture_record().unwrap().time_ms + 1000;
        log.append(&ana, set(Some(png(800, 4))), later).unwrap();
        assert_eq!(log.picture(), Some(png(800, 4)));
        // A copy that never got the record shows the entries' picture.
        let mut copy = GroupLog::empty(log.group_id());
        copy.insert_all(log.entries().cloned());
        assert_eq!(copy.picture(), Some(png(800, 4)));
    }

    /// A new epoch for the members of `log`, numbered like `like`.
    fn numbered_like(log: &GroupLog, like: &EpochId) -> Rotation {
        let members: Vec<DevicePublicKey> = log.state().members.keys().copied().collect();
        let (mut rotation, _) = Rotation::new(&log.group_id(), members).unwrap();
        rotation.epoch[..4].copy_from_slice(&like[..4]);
        rotation
    }

    #[test]
    fn an_epoch_number_already_in_use_is_never_taken_again() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        let first = log.state().epochs[0].id;
        let keys = log.current_keys(&bea.id).map(|k| *k.rendezvous_secret());
        let rotation = numbered_like(&log, &first);
        assert!(log.append(&ana.id, Body::Rotate { rotation }, 4).is_err());
        // A removal still stands; its epoch does not, and the members rotate again.
        let mut rotation = numbered_like(&log, &first);
        rotation.sealed.retain(|(k, _)| *k != leo.key());
        log.append(
            &ana.id,
            Body::Remove {
                member: leo.key(),
                rotation,
            },
            5,
        )
        .unwrap();
        let state = log.state();
        assert!(!state.is_member(&leo.key()));
        assert_eq!(state.epochs.len(), 1);
        assert_eq!(state.current_secret_leaked_to(), vec![leo.key()]);
        assert_eq!(
            log.current_keys(&bea.id).map(|k| *k.rendezvous_secret()),
            keys
        );
    }

    #[test]
    fn a_member_rotates_only_when_it_saw_the_key_with_someone_gone() {
        let (ana, bea, leo, max) = (Device::new(), Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        log.add(&ana.id, max.key(), "Max", 4).unwrap();
        // With nobody gone, Leo has no reason to: ignored. An admin may at any time.
        assert_eq!(
            log.rotate(&leo.id, 5).map(drop),
            Err(StateError::NotAllowed(ROTATE_NOT_NEEDED))
        );
        log.rotate(&ana.id, 6).unwrap();
        assert_eq!(log.state().epochs.len(), 2);
        // Max leaves with the key: Bea and Leo both rotate at once, and both apply.
        log.append(&max.id, Body::Leave, 7).unwrap();
        let mut at_bea = log.clone();
        let mut at_leo = log.clone();
        at_bea.rotate(&bea.id, 8).unwrap();
        at_leo.rotate(&leo.id, 8).unwrap();
        sync(&at_leo, &mut at_bea);
        let state = at_bea.state();
        assert_eq!(state.epochs.len(), 4);
        assert!(state.current_secret_leaked_to().is_empty());
        // Once that is mended, Leo has no reason again.
        assert!(at_bea.rotate(&leo.id, 9).is_err());
        // The same whatever order the log is read in.
        let mut entries: Vec<Entry> = at_bea.entries().cloned().collect();
        entries.reverse();
        let mut back = GroupLog::empty(at_bea.group_id());
        back.insert_all(entries);
        assert_eq!(back.state().epochs, state.epochs);
        assert_eq!(back.state().applied, state.applied);
    }

    #[test]
    fn a_manager_demoted_while_it_rotated_keeps_its_rotation() {
        let (ana, bea, leo) = (Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        log.append(&ana.id, Body::SetManagers { first: Some(2) }, 4)
            .unwrap();
        log.append(&ana.id, Body::Promote { member: leo.key() }, 4)
            .unwrap();
        let mut at_leo = log.clone();
        let rotation = at_leo.rotate(&leo.id, 5).unwrap();
        log.append(&ana.id, Body::Demote { member: leo.key() }, 5)
            .unwrap();
        sync(&at_leo, &mut log);
        let state = log.state();
        assert!(!state.is_manager(&leo.key()));
        assert!(state.applied.contains(&rotation.hash()));
        assert_eq!(state.epochs.len(), 2);
    }

    /// A rotation for every member of `log` whose epoch number is `number`.
    fn rotation_numbered(log: &GroupLog, number: u32) -> Rotation {
        let members: Vec<DevicePublicKey> = log.state().members.keys().copied().collect();
        let mut id = [9u8; 16];
        id[..4].copy_from_slice(&number.to_be_bytes());
        let secret = GroupSecret::generate().unwrap();
        let context = secret_context(&log.group_id(), &id);
        Rotation {
            epoch: id,
            commitment: commitment(&id, &secret),
            sealed: members
                .iter()
                .map(|k| (*k, Sealed::seal(k, &context, secret.as_bytes()).unwrap()))
                .collect(),
        }
    }

    #[test]
    fn an_epoch_number_already_in_the_log_is_not_used_again() {
        let (ana, bea) = (Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        let first = epoch_number(&log.state().epochs[0].id);
        let keys = log.current_keys(&bea.id).unwrap();
        // Even an admin's rotation under the first epoch's number is ignored: the key that
        // number names stays the same for everyone.
        let again = rotation_numbered(&log, first);
        assert!(
            log.append(&ana.id, Body::Rotate { rotation: again }, 3)
                .is_err()
        );
        let fresh = rotation_numbered(&log, first.wrapping_add(1));
        log.append(&ana.id, Body::Rotate { rotation: fresh }, 4)
            .unwrap();
        assert_eq!(log.state().epochs.len(), 2);
        let all = log.all_keys(&bea.id);
        assert_eq!(all[&first].rendezvous_secret(), keys.rendezvous_secret());
        // A removal under a number in use still removes; its key is not used, and the
        // members then rotate, as after two removals at once.
        let leo = Device::new();
        log.add(&ana.id, leo.key(), "Leo", 5).unwrap();
        let mut taken = rotation_numbered(&log, first);
        taken.sealed.retain(|(k, _)| *k != leo.key());
        log.append(
            &ana.id,
            Body::Remove {
                member: leo.key(),
                rotation: taken,
            },
            6,
        )
        .unwrap();
        let state = log.state();
        assert!(!state.is_member(&leo.key()));
        assert_eq!(state.epochs.len(), 2);
        assert_eq!(state.current_secret_leaked_to(), vec![leo.key()]);
    }

    #[test]
    fn a_member_rotates_only_when_someone_gone_has_the_key() {
        let (ana, bea, leo, max) = (Device::new(), Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        // Only the first manages.
        log.append(&ana.id, Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.add(&ana.id, leo.key(), "Leo", 3).unwrap();
        log.add(&ana.id, max.key(), "Max", 4).unwrap();
        // With nobody gone, a member's rotation is ignored; a manager's applies.
        assert_eq!(
            log.clone().rotate(&leo.id, 5),
            Err(StateError::NotAllowed(ROTATE_NOT_NEEDED))
        );
        log.clone().rotate(&ana.id, 5).unwrap();
        // Max leaves: Bea and Leo both see it and rotate at once, each on what it saw.
        // Both apply, in every copy, whichever comes first in the common order.
        log.append(&max.id, Body::Leave, 6).unwrap();
        let mut at_bea = log.clone();
        let mut at_leo = log.clone();
        at_bea.rotate(&bea.id, 7).unwrap();
        at_leo.rotate(&leo.id, 7).unwrap();
        let mut both = at_bea.clone();
        both.insert_all(at_leo.entries_in_order().into_iter().cloned());
        let mut other_way = GroupLog::empty(log.group_id());
        let mut entries: Vec<Entry> = both.entries().cloned().collect();
        entries.reverse();
        other_way.insert_all(entries);
        for copy in [&both, &other_way] {
            let state = copy.state();
            assert_eq!(state.epochs.len(), 3, "both rotations applied");
            assert!(
                !state
                    .rejected
                    .iter()
                    .any(|(_, reason)| *reason == ROTATE_NOT_NEEDED)
            );
            assert!(state.current_secret_leaked_to().is_empty());
        }
        assert_eq!(both.state().applied, other_way.state().applied);
        // Once that is mended, a member's rotation is ignored again.
        assert_eq!(
            both.rotate(&leo.id, 8),
            Err(StateError::NotAllowed(ROTATE_NOT_NEEDED))
        );
        // A manager demoted at the same time as it rotated still rotated as a manager.
        let mut at_ana = both.clone();
        both.append(&ana.id, Body::SetManagers { first: Some(2) }, 9)
            .unwrap();
        both.append(&ana.id, Body::Promote { member: bea.key() }, 9)
            .unwrap();
        let mut demoted = both.clone();
        demoted
            .append(&ana.id, Body::Demote { member: bea.key() }, 10)
            .unwrap();
        let mut at_bea = both.clone();
        let rotation = at_bea.rotate(&bea.id, 10).unwrap();
        demoted.insert(rotation.clone()).unwrap();
        assert!(demoted.state().applied.contains(&rotation.hash()));
        at_ana.insert_all(demoted.entries_in_order().into_iter().cloned());
        assert_eq!(at_ana.state().applied, demoted.state().applied);
    }

    #[test]
    fn honest_logs_never_meet_the_stricter_rules() {
        // Joins, adds, leaves, removals, concurrent removals, rotations after leaks and
        // welcomes, as members write them: no entry is ignored for a rule an older version
        // did not have.
        let (ana, bea, leo, max, eva) = (
            Device::new(),
            Device::new(),
            Device::new(),
            Device::new(),
            Device::new(),
        );
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        log.add(&ana.id, bea.key(), "Bea", 2).unwrap();
        log.append(&ana.id, Body::Promote { member: bea.key() }, 3)
            .unwrap();
        log.add(&ana.id, leo.key(), "Leo", 4).unwrap();
        let (ticket, _) = log.invite(&ana.id, 100, 5).unwrap();
        join(&mut log, &max, ticket, 6);
        log.welcome(&ana.id, max.key(), 7).unwrap();
        log.append(&leo.id, Body::Leave, 8).unwrap();
        log.rotate(&max.id, 9).unwrap();
        let mut at_bea = log.clone();
        log.add(&ana.id, eva.key(), "Eva", 10).unwrap();
        at_bea.remove(&bea.id, max.key(), 10).unwrap();
        log.insert_all(at_bea.entries_in_order().into_iter().cloned());
        // Eva was added while Max's removal rotated without her: an admin welcomes her.
        for member in log.state().lacking_current_secret() {
            log.welcome(&ana.id, member, 11).unwrap();
        }
        let state = log.state();
        for (_, reason) in &state.rejected {
            assert!(
                *reason != ROTATE_NOT_NEEDED && *reason != WELCOME_NOT_MANAGER,
                "{reason}"
            );
        }
        assert!(log.current_keys(&eva.id).is_some());
    }

    #[test]
    fn a_rotation_grows_the_log_by_about_140_bytes_a_member() {
        for members in [8usize, 20] {
            let ana = Device::new();
            let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
            for i in 1..members {
                log.add(&ana.id, Device::new().key(), "m", 1 + i as u64)
                    .unwrap();
            }
            let before: usize = log.entries().map(|e| e.encode().len()).sum();
            let entry = log.rotate(&ana.id, 100).unwrap();
            let bytes = entry.encode().len();
            let after: usize = log.entries().map(|e| e.encode().len()).sum();
            assert_eq!(after - before, bytes);
            println!("{members} members: {bytes} bytes a rotation");
            assert!(bytes <= 300 + members * 140, "{bytes}");
            assert!(bytes >= members * 130, "{bytes}");
        }
    }

    /// Ana's group with Bea, Carlos and Dani, in that order on the ladder.
    fn ladder_of_four() -> (GroupLog, Device, Device, Device, Device) {
        let (ana, bea, carlos, dani) = (Device::new(), Device::new(), Device::new(), Device::new());
        let mut log = GroupLog::create(&ana.id, "g", 1).unwrap();
        for (i, d) in [&bea, &carlos, &dani].into_iter().enumerate() {
            log.add(&ana.id, d.key(), "x", 2 + i as u64).unwrap();
        }
        (log, ana, bea, carlos, dani)
    }

    /// Both copies take each other's entries; returns the state they agree on.
    fn meet(a: &mut GroupLog, b: &mut GroupLog) -> GroupState {
        sync(a, b);
        sync(b, a);
        let (one, two) = (a.state(), b.state());
        assert_eq!(one.applied, two.applied);
        assert_eq!(one.ladder, two.ladder);
        one
    }

    #[test]
    fn of_two_changes_made_at_once_the_one_who_came_in_first_wins() {
        for _ in 0..8 {
            let (base, _, bea, carlos, _) = ladder_of_four();
            let (mut at_bea, mut at_carlos) = (base.clone(), base.clone());
            let rename = |name: &str| Body::Rename { name: name.into() };
            at_carlos.append(&carlos.id, rename("carlos"), 10).unwrap();
            at_bea.append(&bea.id, rename("bea"), 10).unwrap();
            assert_eq!(meet(&mut at_bea, &mut at_carlos).name, "bea");
            // One after the other, the last one stands, as always.
            at_carlos.append(&carlos.id, rename("again"), 11).unwrap();
            assert_eq!(meet(&mut at_bea, &mut at_carlos).name, "again");
        }
    }

    #[test]
    fn moves_of_one_member_at_once_go_the_way_of_the_one_above() {
        for _ in 0..8 {
            let (base, ana, bea, carlos, dani) = ladder_of_four();
            // Ana sends Dani to the end while Bea moves him up below her.
            let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
            at_bea
                .append(&bea.id, Body::Promote { member: dani.key() }, 10)
                .unwrap();
            at_ana
                .append(&ana.id, Body::Demote { member: dani.key() }, 10)
                .unwrap();
            let state = meet(&mut at_ana, &mut at_bea);
            assert_eq!(state.ladder.last(), Some(&dani.key()));
            // Bea removes Dani while Carlos moves him up: he is out.
            let (mut at_bea, mut at_carlos) = (base.clone(), base.clone());
            at_carlos
                .append(&carlos.id, Body::Promote { member: dani.key() }, 10)
                .unwrap();
            at_bea.remove(&bea.id, dani.key(), 10).unwrap();
            assert!(!meet(&mut at_bea, &mut at_carlos).is_member(&dani.key()));
            // Carlos sends Dani down while Dani removes Carlos, with what he saw then: Dani
            // was never above Carlos, and stays a member below him.
            let (mut at_carlos, mut at_dani) = (base.clone(), base.clone());
            at_carlos
                .append(&carlos.id, Body::Demote { member: dani.key() }, 10)
                .unwrap();
            let (rotation, _) =
                Rotation::new(&base.group_id(), [ana.key(), bea.key(), dani.key()]).unwrap();
            let entry = Entry::sign(
                &dani.id,
                base.group_id(),
                base.heads(),
                10,
                Body::Remove {
                    member: carlos.key(),
                    rotation,
                },
            );
            // Not even kept: Dani could not make it.
            assert!(at_dani.insert(entry).is_err());
            let state = meet(&mut at_carlos, &mut at_dani);
            assert!(state.is_member(&carlos.key()) && state.is_member(&dani.key()));
        }
    }

    #[test]
    fn a_move_up_at_once_with_a_removal_wins_only_from_above_and_later() {
        for _ in 0..8 {
            // Ana moves Dani, the last to come in, up below her: [Ana, Dani, Bea, Carlos].
            let (mut base, ana, bea, carlos, dani) = ladder_of_four();
            base.append(&ana.id, Body::Promote { member: dani.key() }, 5)
                .unwrap();
            // Dani moves Carlos up while Bea removes him. Dani stands above Bea and came in
            // after her: his move applies first, Carlos is then above Bea, and her removal is
            // ignored.
            let (mut at_bea, mut at_dani) = (base.clone(), base.clone());
            at_dani
                .append(
                    &dani.id,
                    Body::Promote {
                        member: carlos.key(),
                    },
                    10,
                )
                .unwrap();
            at_bea.remove(&bea.id, carlos.key(), 10).unwrap();
            let state = meet(&mut at_bea, &mut at_dani);
            assert_eq!(
                state.ladder,
                [ana.key(), dani.key(), carlos.key(), bea.key()]
            );
            // The same with a move to the end.
            let (mut at_bea, mut at_dani) = (base.clone(), base.clone());
            at_dani
                .append(
                    &dani.id,
                    Body::Promote {
                        member: carlos.key(),
                    },
                    10,
                )
                .unwrap();
            at_bea
                .append(
                    &bea.id,
                    Body::Demote {
                        member: carlos.key(),
                    },
                    10,
                )
                .unwrap();
            let state = meet(&mut at_bea, &mut at_dani);
            assert_eq!(
                state.ladder,
                [ana.key(), dani.key(), carlos.key(), bea.key()]
            );
            // Ana, above too but in before Bea, moves Carlos up while Bea removes him: the
            // removal applies first, and Carlos is out.
            let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
            at_ana
                .append(
                    &ana.id,
                    Body::Promote {
                        member: carlos.key(),
                    },
                    10,
                )
                .unwrap();
            at_bea.remove(&bea.id, carlos.key(), 10).unwrap();
            assert!(!meet(&mut at_ana, &mut at_bea).is_member(&carlos.key()));
        }
    }

    #[test]
    fn a_removal_by_someone_moved_down_at_once_is_undone_and_the_removed_is_back() {
        // Ana, alone, sends Bea to the end. Bea, who has not seen it, removes Dani and renames
        // the group with Dani. When Ana and Bea meet, both are undone.
        let (base, ana, bea, _, dani) = ladder_of_four();
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        at_ana
            .append(&ana.id, Body::Demote { member: bea.key() }, 10)
            .unwrap();
        at_bea.remove(&bea.id, dani.key(), 11).unwrap();
        at_bea
            .append(
                &bea.id,
                Body::Rename {
                    name: "taken".into(),
                },
                12,
            )
            .unwrap();
        assert!(!at_bea.state().is_member(&dani.key()));
        let state = meet(&mut at_ana, &mut at_bea);
        assert!(state.is_member(&dani.key()));
        assert_eq!(state.name, "g");
        assert_eq!(state.ladder.last(), Some(&bea.key()));
        // Bea's leave or rotations would not have been undone: only what needs a place.
        assert!(state.rank(&bea.key()).is_some());
    }

    #[test]
    fn undoing_a_move_up_undoes_what_it_made_possible() {
        // Ana sends Bea down. Bea, at once, had moved Dani up, and Dani then removed Carlos,
        // which only being above him let him do. Carlos is back.
        let (base, ana, bea, carlos, dani) = ladder_of_four();
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        at_ana
            .append(&ana.id, Body::Demote { member: bea.key() }, 10)
            .unwrap();
        at_bea
            .append(&bea.id, Body::Promote { member: dani.key() }, 11)
            .unwrap();
        at_bea.remove(&dani.id, carlos.key(), 12).unwrap();
        assert!(!at_bea.state().is_member(&carlos.key()));
        let state = meet(&mut at_ana, &mut at_bea);
        assert!(state.is_member(&carlos.key()));
        assert_eq!(
            state.ladder,
            [ana.key(), carlos.key(), dani.key(), bea.key()]
        );
    }

    #[test]
    fn a_member_limit_set_at_once_with_an_add_is_refused_when_it_leaves_someone_out() {
        let (base, ana, bea, _, _) = ladder_of_four();
        let (mut at_ana, mut at_bea) = (base.clone(), base.clone());
        let eva = Device::new();
        at_bea.add(&bea.id, eva.key(), "eva", 10).unwrap();
        let limits = at_ana
            .append(
                &ana.id,
                Body::SetLimits {
                    members: 4,
                    room: 8,
                },
                10,
            )
            .unwrap();
        let state = meet(&mut at_ana, &mut at_bea);
        assert!(state.is_member(&eva.key()));
        assert_eq!(state.max_members, MAX_MEMBERS);
        assert!(state.rejected.iter().any(|(h, _)| *h == limits.hash()));
    }

    #[test]
    fn logs_with_the_old_admin_entries_read_as_moves() {
        // The kind and flag an admin was appointed with read as a move up below its author,
        // and the one that took it away as a move to the end.
        let (mut log, ana, bea, carlos, dani) = ladder_of_four();
        let raw = |log: &GroupLog, member: &Device, flag: u8| {
            let mut w = fogata_crypto::codec::Writer::default();
            w.group(&log.group_id()).u32(log.heads().len() as u32);
            for p in log.heads() {
                w.raw(&p);
            }
            w.key(&ana.key()).u64(20).u8(3).key(&member.key()).u8(flag);
            let unsigned = w.into_bytes();
            let mut signed = b"fogata/v1/entry".to_vec();
            signed.extend_from_slice(&unsigned);
            let mut bytes = unsigned;
            bytes.extend_from_slice(&ana.id.sign(&signed));
            Entry::decode(&bytes).unwrap()
        };
        let appoint = raw(&log, &dani, 1);
        assert_eq!(appoint.body, Body::Promote { member: dani.key() });
        log.insert(appoint).unwrap();
        let demote = raw(&log, &bea, 0);
        assert_eq!(demote.body, Body::Demote { member: bea.key() });
        log.insert(demote).unwrap();
        assert_eq!(
            log.state().ladder,
            [ana.key(), dani.key(), carlos.key(), bea.key()]
        );
    }

    /// Several devices of one group, each with its own copy of the log, doing at random what
    /// members do with the ladder and meeting only now and then.
    /// Entries that count a device writes in a run, in random histories that go past it:
    /// the creator's first entries take five.
    const SMALL_RUN: u32 = 6;

    struct LadderWorld {
        rng: u64,
        time: u64,
        devices: Vec<Device>,
        logs: Vec<GroupLog>,
    }

    impl LadderWorld {
        fn new(seed: u64, n: usize) -> Self {
            let devices: Vec<Device> = (0..n).map(|_| Device::new()).collect();
            let mut log = GroupLog::create(&devices[0].id, "g", 1).unwrap();
            log.append(
                &devices[0].id,
                Body::SetRoom {
                    id: [1; 8],
                    name: "r".into(),
                },
                1,
            )
            .unwrap();
            // The last one starts outside.
            for (i, d) in devices.iter().enumerate().skip(1).take(n - 2) {
                log.add(&devices[0].id, d.key(), "m", 1 + i as u64).unwrap();
            }
            Self {
                rng: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1,
                time: 100,
                devices,
                logs: vec![log; n],
            }
        }

        /// The same world, where a device writes at most `entries` entries that count in a
        /// run (see [`crate::budget`]): odd seeds of the random histories run so, so that
        /// devices often go past their run.
        fn with_run_room(mut self, entries: u32) -> Self {
            for log in &mut self.logs {
                log.set_run_room(entries, RUN_ROOM.bytes);
            }
            self
        }

        /// A log that knows only the group, with the room for runs of this world's.
        fn empty_log(&self) -> GroupLog {
            let mut log = GroupLog::empty(self.logs[0].group_id());
            log.set_run_room(self.logs[0].run_room.entries, self.logs[0].run_room.bytes);
            log
        }

        fn below(&mut self, n: usize) -> usize {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            (self.rng % n.max(1) as u64) as usize
        }

        fn step(&mut self) {
            self.time += 1 + self.below(1000) as u64;
            let n = self.devices.len();
            let (i, j) = (self.below(n), self.below(n));
            let (me, other) = (self.devices[i].key(), self.devices[j].key());
            let state = self.logs[i].state();
            let roll = self.below(100);
            let id = &self.devices[i].id;
            let (log, now) = (&mut self.logs[i], self.time);
            let _ = match roll {
                0..10 if state.outranks(&me, &other) => log
                    .append(id, Body::Promote { member: other }, now)
                    .map(drop),
                10..20 if state.outranks(&me, &other) => log
                    .append(id, Body::Demote { member: other }, now)
                    .map(drop),
                20..26 if state.outranks(&me, &other) => log.remove(id, other, now).map(drop),
                26..29 if state.is_member(&me) => log.append(id, Body::Leave, now).map(drop),
                29..35 if !state.is_member(&other) && other != me => {
                    log.add(id, other, "m", now).map(drop)
                }
                35..40 => {
                    let first = [None, Some(1), Some(2), Some(3), Some(5)][roll % 5];
                    log.append(id, Body::SetManagers { first }, now).map(drop)
                }
                40..46 => log
                    .append(
                        id,
                        Body::Rename {
                            name: format!("g{roll}"),
                        },
                        now,
                    )
                    .map(drop),
                46..50 => log
                    .keep_out(id, [1; 8], other, Some(now + 5_000), now)
                    .map(drop),
                50..52 => log.let_back(id, [1; 8], other, now).map(drop),
                52..54 => log
                    .set_room_access(id, [1; 8], Some(vec![other]), now)
                    .map(drop),
                // Not for a key sent only in an entry that did not apply, unless managing:
                // devices leave that to managers.
                54..58
                    if state.is_manager(&me)
                        || !state.current_secret_left_with().is_empty()
                        || state.current_secret_leaked_to().is_empty() =>
                {
                    log.rotate(id, now).map(drop)
                }
                58..62 if !state.is_member(&other) && state.is_manager(&me) => {
                    // An invite that the other device uses at once on the inviter's log,
                    // which lets it in at once, or later.
                    match log.invite(id, now + 10_000, now) {
                        Ok((ticket, _)) => {
                            let entry = join(log, &self.devices[j], ticket, now);
                            if roll.is_multiple_of(2) {
                                let _ = log.let_in(id, &entry.hash(), now);
                            }
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                62..66 => {
                    let lacking = state.lacking_current_secret();
                    let waiting = self
                        .devices
                        .iter()
                        .map(Device::key)
                        .find(|d| state.waiting_join(d).is_some());
                    match lacking.first().copied().or(waiting) {
                        Some(member) => log.welcome(id, member, now).map(drop),
                        None => Ok(()),
                    }
                }
                66.. if i != j => {
                    let from = self.logs[i].clone();
                    if roll.is_multiple_of(3) {
                        // One at a time in the order they are sent in, as entries written
                        // apart arrive.
                        for entry in from.entries_in_order() {
                            let _ = self.logs[j].insert(entry.clone());
                        }
                    } else {
                        self.logs[j].insert_all(from.entries().cloned());
                    }
                    if roll.is_multiple_of(2) {
                        let back = self.logs[j].clone();
                        self.logs[i].insert_all(back.entries().cloned());
                    }
                    Ok(())
                }
                _ => Ok(()),
            };
        }

        /// Device `bad` writes an entry that is likely to apply to nothing (a rename it may
        /// not make, a move of someone above it, a leave again, a kind this version does not
        /// know, a rename signed on entries of such a kind) on what one copy has or had, and
        /// hands it to a few copies only; or, managing, brings a device of its own in and
        /// takes it out again on one copy.
        fn junk_step(&mut self, bad: usize) {
            self.time += 1 + self.below(1000) as u64;
            let n = self.devices.len();
            let k = self.below(n);
            let parents = match self.below(3) {
                0 => self.logs[k].heads(),
                _ => {
                    let all: Vec<EntryHash> = self.logs[k].hashes().copied().collect();
                    let at = self.below(all.len());
                    vec![all[at]]
                }
            };
            let group_id = self.logs[k].group_id();
            let first = self.logs[0].state().ladder.first().copied();
            let kind = self.below(6);
            let (links, to, leaves) = (1 + self.below(3), self.below(n), self.below(2) == 0);
            let device = &self.devices[bad];
            let entry = match kind {
                0 => newer_kind(device, group_id, parents, self.time),
                // A device of its own brought in with a ticket and taken out again, the
                // device leaving or removed: both count in its run.
                5 => {
                    let fresh = Device::new();
                    let log = &mut self.logs[to];
                    let out = match log.issue_ticket(&device.id, self.time + 1_000) {
                        Ok(ticket) => {
                            let entry = join(log, &fresh, ticket, self.time);
                            let _ = log.let_in(&device.id, &entry.hash(), self.time);
                            if leaves {
                                log.append(&fresh.id, Body::Leave, self.time)
                            } else {
                                log.remove(&device.id, fresh.key(), self.time)
                            }
                        }
                        Err(e) => Err(e),
                    };
                    match out {
                        Ok(entry) => entry,
                        Err(_) => {
                            Entry::sign(&device.id, group_id, parents, self.time, Body::Leave)
                        }
                    }
                }
                // A rename signed on a few entries of a newer kind of its own, each on the
                // one before: every copy that keeps it keeps them, in its run.
                4 => {
                    let mut parents = parents;
                    let mut chain = Vec::new();
                    for _ in 0..links {
                        let entry = newer_kind(device, group_id, parents, self.time);
                        parents = vec![entry.hash()];
                        chain.push(entry);
                    }
                    for entry in chain {
                        let _ = self.logs[to].insert(entry);
                    }
                    Entry::sign(
                        &device.id,
                        group_id,
                        parents,
                        self.time,
                        Body::Rename {
                            name: format!("k{}", self.time),
                        },
                    )
                }
                body => {
                    let body = match (body, first) {
                        (1, Some(first)) => Body::Demote { member: first },
                        (2, _) => Body::Leave,
                        _ => Body::Rename {
                            name: format!("j{}", self.time),
                        },
                    };
                    let mut parents = parents;
                    parents.sort_unstable();
                    parents.dedup();
                    Entry::sign(&device.id, group_id, parents, self.time, body)
                }
            };
            for _ in 0..1 + self.below(3) {
                let to = self.below(n);
                let _ = self.logs[to].insert(entry.clone());
            }
        }

        /// Two managers answer at once, each on its own copy, devices that join with one
        /// invite (its link passed on): the issuer lets one in; the other manager lets in
        /// another device with the same ticket, or says no to the first. Neither copy has
        /// seen the other's answer; the devices let in are new and never act again.
        fn race_step(&mut self) {
            self.time += 1 + self.below(1000) as u64;
            let n = self.devices.len();
            let (i, j) = (self.below(n), self.below(n));
            let now = self.time;
            let says_no = self.below(3) == 0;
            let (x, y) = (Device::new(), Device::new());
            let Ok((ticket, _)) = self.logs[i].invite(&self.devices[i].id, now + 10_000, now)
            else {
                return;
            };
            let first = join(&mut self.logs[i], &x, ticket.clone(), now);
            let from = self.logs[i].clone();
            self.logs[j].insert_all(from.entries().cloned());
            let _ = self.logs[i].let_in(&self.devices[i].id, &first.hash(), now);
            let other = &self.devices[j].id;
            let _ = if says_no {
                self.logs[j]
                    .append(
                        other,
                        Body::Revoke {
                            ticket_id: ticket.ticket_id,
                        },
                        now,
                    )
                    .map(drop)
            } else {
                let second = join(&mut self.logs[j], &y, ticket, now);
                self.logs[j].let_in(other, &second.hash(), now).map(drop)
            };
        }

        /// Copies `i` and `j` sync as devices do when they connect, and then while the heads
        /// of what they need differ: one sends the hashes it has, the other answers with a part of what the first
        /// lacks ([`GroupLog::hashes_to_send`]) and the hashes it lacks itself, which the
        /// first sends back; again while an answer brings something new. Returns whether
        /// they ended with the same heads.
        fn meet(&mut self, i: usize, j: usize, part: usize) -> bool {
            // They exchange once when they connect, and again while what they need differs.
            for round in 0..40 {
                if round > 0 && self.logs[i].needed_heads() == self.logs[j].needed_heads() {
                    return true;
                }
                for (asks, answers) in [(i, j), (j, i)] {
                    loop {
                        let have: BTreeSet<EntryHash> = self.logs[asks].hashes().copied().collect();
                        let from = &self.logs[answers];
                        let answer: Vec<Entry> = from
                            .hashes_to_send()
                            .into_iter()
                            .filter(|h| !have.contains(h))
                            .take(part)
                            .map(|h| from.entries[&h].clone())
                            .collect();
                        let wanted: Vec<Entry> = have
                            .iter()
                            .filter(|h| from.get(h).is_none())
                            .map(|h| self.logs[asks].entries[h].clone())
                            .collect();
                        self.logs[answers].insert_all(wanted);
                        if self.logs[asks].insert_all(answer).is_empty() {
                            break;
                        }
                    }
                }
            }
            self.logs[i].needed_heads() == self.logs[j].needed_heads()
        }

        fn sync_all(&mut self) {
            let mut all = self.logs[0].clone();
            for log in &self.logs[1..] {
                all.insert_all(log.entries().cloned());
            }
            for log in &mut self.logs {
                log.insert_all(all.entries().cloned());
            }
        }
    }

    /// `FOGATA_LADDER_SEEDS` sets how many histories run (4 by default).
    #[test]
    fn a_member_writing_junk_keeps_every_copy_alike() {
        let seeds = std::env::var("FOGATA_LADDER_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4u64);
        for seed in 1..=seeds {
            let mut world = LadderWorld::new(seed, 6);
            for i in 0..160 {
                if i % 3 == 0 {
                    world.junk_step(1 + i % 2);
                } else {
                    world.step();
                }
            }
            // Copies that meet two by two, each taking what the other has, end alike.
            let n = world.logs.len();
            for i in 0..n {
                for j in i + 1..n {
                    let (a, b) = (world.logs[i].clone(), world.logs[j].clone());
                    world.logs[i].insert_all(b.entries().cloned());
                    world.logs[j].insert_all(a.entries().cloned());
                }
            }
            world.sync_all();
            let state = world.logs[0].state();
            for (i, log) in world.logs.iter().enumerate() {
                let other = log.state();
                assert_eq!(other.applied, state.applied, "seed {seed}: device {i}");
                assert_eq!(other.ladder, state.ladder, "seed {seed}: device {i}");
                assert_eq!(other.rules, state.rules, "seed {seed}: device {i}");
                assert_eq!(log.ordered(), world.logs[0].ordered(), "seed {seed}: {i}");
                assert_eq!(
                    other.current_secret_leaked_to(),
                    state.current_secret_leaked_to(),
                    "seed {seed}: device {i}"
                );
            }
            // No entry names more parents than allowed.
            assert!(
                world.logs[0]
                    .entries()
                    .all(|e| e.parents.len() <= MAX_PARENTS)
            );
            check_every_step(&world.logs[0], &state);
            check_no_key_unaccounted(&world.logs[0], &state);
        }
    }

    /// Managers answering devices that join with one invite, at once on copies apart, as
    /// the rest of the group goes on: every copy ends with the same group, and a device
    /// whose welcome lost (or that a no beat) and still got the current key counts as
    /// holding it, so the members start a new epoch, after which nobody outside has it.
    /// `FOGATA_LADDER_SEEDS` sets how many histories run (4 by default).
    #[test]
    fn welcomes_that_lose_a_race_leave_no_key_unaccounted() {
        let seeds = std::env::var("FOGATA_LADDER_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4u64);
        let (mut leaks, mut mended, mut stayed) = (0, 0, 0);
        for seed in 1..=seeds {
            let mut world = LadderWorld::new(seed, 6);
            if seed % 2 == 1 {
                world = world.with_run_room(SMALL_RUN);
            }
            for i in 0..160 {
                if i % 4 == 0 {
                    world.race_step();
                } else {
                    world.step();
                }
                // Each copy accounts for every key it handed out, as it goes.
                if i % 10 == 9 {
                    for log in &world.logs {
                        check_no_key_unaccounted(log, &log.state());
                    }
                }
            }
            world.sync_all();
            let state = world.logs[0].state();
            for (i, log) in world.logs.iter().enumerate() {
                let other = log.state();
                assert_eq!(other.applied, state.applied, "seed {seed}: device {i}");
                assert_eq!(
                    other.current_secret_leaked_to(),
                    state.current_secret_leaked_to(),
                    "seed {seed}: device {i}"
                );
            }
            check_every_step(&world.logs[0], &state);
            check_no_key_unaccounted(&world.logs[0], &state);
            if state.current_secret_leaked_to().is_empty() {
                continue;
            }
            // A leak is mended by a new epoch: of the first holder that may write one (a
            // manager, or any member when someone given the key by an entry that applied is
            // gone), or of a manager without the key, as the node does. Devices let in by a
            // race step never act here, and in odd seeds a device's run is short: a leak
            // stays only when each of them is one of those or past its run.
            leaks += 1;
            let anyone = !state.current_secret_left_with().is_empty();
            let holders = state.current_holders_in_order();
            let others = state.ladder.iter().filter(|k| !holders.contains(k));
            let may: Vec<DevicePublicKey> = holders
                .iter()
                .filter(|k| anyone || state.is_manager(k))
                .chain(others.filter(|k| state.is_manager(k)))
                .copied()
                .collect();
            assert!(!may.is_empty(), "seed {seed}: nobody may mend the leak");
            let mut why = Vec::new();
            for device in may {
                let Some(k) = world.devices.iter().position(|d| d.key() == device) else {
                    why.push("let in by a race step");
                    continue;
                };
                let (log, id) = (&mut world.logs[k], &world.devices[k].id);
                match log.rotate(id, world.time + 1) {
                    Ok(_) => {
                        assert!(log.state().current_secret_leaked_to().is_empty());
                        why.clear();
                        mended += 1;
                        break;
                    }
                    Err(StateError::NotAllowed(PAST_RUN)) if seed % 2 == 1 => why.push(PAST_RUN),
                    Err(e) => panic!("seed {seed}: {e}"),
                }
            }
            if !why.is_empty() {
                stayed += 1;
                println!("seed {seed}: the leak stays: {why:?}");
            }
        }
        println!("{leaks} leaks, {mended} mended, {stayed} left as nobody could mend them");
        // Races that leave a key outside do happen in these histories, and they are mended.
        assert!(leaks > 0 || seeds < 4, "no leak in {seeds} histories");
        assert!(
            mended > 0 || seeds < 4,
            "no leak mended in {seeds} histories"
        );
        assert_eq!(leaks, mended + stayed);
    }

    /// With little room for spare entries, copies let go of different ones; meeting as
    /// devices sync, they still end with the same needed entries and the same group, the
    /// group a copy of what they all need works out. In odd seeds a device writes little in
    /// a run, so entries past it are let go too.
    /// `FOGATA_LADDER_SEEDS` sets how many histories run (4 by default).
    #[test]
    fn copies_that_let_go_of_spare_entries_still_end_alike() {
        let seeds = std::env::var("FOGATA_LADDER_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4u64);
        let (per_author, all) = (2, 5);
        for seed in 1..=seeds {
            let mut world = LadderWorld::new(seed, 6);
            if seed % 2 == 1 {
                world = world.with_run_room(SMALL_RUN);
            }
            for log in &mut world.logs {
                log.set_spare_room(per_author, all);
            }
            for i in 0..200 {
                if i % 2 == 0 {
                    world.junk_step(1 + i % 3);
                } else {
                    world.step();
                }
                // Trimmed as the node does every few seconds: the room holds, and what
                // applies stays the same.
                for log in &mut world.logs {
                    let applied = log.state().applied.clone();
                    log.trim();
                    assert!(log.spare().len() <= all, "seed {seed}");
                    assert_eq!(log.state().applied, applied, "seed {seed}");
                }
            }
            // Rounds of meetings two by two, as devices keep syncing, until all have the
            // same: what one let go and another still needs can take more than one round
            // to come back.
            let n = world.logs.len();
            let alike = |world: &LadderWorld| {
                let needed = world.logs[0].needed_heads();
                world.logs.iter().all(|l| l.needed_heads() == needed)
            };
            for round in 0..4 {
                for i in 0..n {
                    for j in i + 1..n {
                        let part = 1 + world.below(4);
                        let met = world.meet(i, j, part);
                        world.logs[i].trim();
                        world.logs[j].trim();
                        assert!(met || round == 0, "seed {seed}: {i} and {j} differ");
                    }
                }
                if round > 0 && alike(&world) {
                    break;
                }
            }
            let state = world.logs[0].state();
            let needed = world.logs[0].needed_heads();
            for (i, log) in world.logs.iter().enumerate() {
                let other = log.state();
                assert_eq!(other.applied, state.applied, "seed {seed}: device {i}");
                assert_eq!(other.ladder, state.ladder, "seed {seed}: device {i}");
                assert_eq!(other.rules, state.rules, "seed {seed}: device {i}");
                assert_eq!(log.needed_heads(), needed, "seed {seed}: device {i}");
                check_every_step(log, &other);
            }
            // The same as a copy that took what each copy needs and what waits on each: the
            // spare entries each let go change nothing. (Spare entries that different copies
            // kept could together bring in an entry that waits for good on each, so they are
            // left out.)
            let mut whole = world.empty_log();
            for log in &world.logs {
                let spare: BTreeSet<EntryHash> = log.spare().into_iter().collect();
                whole.insert_all(
                    log.entries()
                        .filter(|e| !spare.contains(&e.hash()))
                        .cloned(),
                );
            }
            assert_eq!(whole.state().applied, state.applied, "seed {seed}: whole");
        }
    }

    /// Applies `log`'s entries one by one as [`GroupLog::state`] settles them, and checks
    /// that each that applies was allowed on the group just before it: nobody acted on
    /// someone not below it, and nobody that did not manage changed the group.
    fn check_every_step(log: &GroupLog, settled: &GroupState) {
        let refused: BTreeSet<EntryHash> = settled
            .rejected
            .iter()
            .filter(|(_, r)| *r == OUTRANKED)
            .map(|(h, _)| *h)
            .collect();
        let over = &log.full().budget.over;
        let mut state = GroupState::default();
        for hash in log.ordered() {
            let entry = &log.entries[&hash];
            if entry.body.is_room_rule() {
                continue;
            }
            if over.contains(&hash) {
                state.refuse(hash, PAST_RUN);
                continue;
            }
            let before = state.clone();
            if refused.contains(&hash) {
                state.refuse_kept(hash, entry, OUTRANKED);
                continue;
            }
            let needed = state
                .rotation_to_judge(entry)
                .then(|| log.rotation_needed(&hash));
            let place = &log.full().order.place;
            let seen = |h: &EntryHash| log.is_before(place, &entry.parents, h);
            state.apply(hash, entry, needed, &seen);
            if state.applied.last() != Some(&hash) {
                continue;
            }
            let author = entry.author;
            match &entry.body {
                Body::Promote { member }
                | Body::Demote { member }
                | Body::Remove { member, .. } => {
                    assert!(before.outranks(&author, member), "{:?}", entry.body)
                }
                Body::Leave | Body::Rotate { .. } | Body::Create { .. } | Body::Join { .. } => {}
                body if body.needs_rank() => assert!(before.is_manager(&author), "{body:?}"),
                _ => {}
            }
        }
        let main: Vec<EntryHash> = settled
            .applied
            .iter()
            .filter(|h| !log.entries[*h].body.is_room_rule())
            .copied()
            .collect();
        assert_eq!(state.applied, main);
        assert_eq!(state.ladder, settled.ladder);
        // The ladder holds every member once, and nobody else.
        let mut on_ladder = state.ladder.clone();
        on_ladder.sort();
        on_ladder.dedup();
        assert_eq!(on_ladder.len(), state.ladder.len());
        assert!(on_ladder.iter().eq(state.members.keys()));
    }

    /// Checks that every device a kept `Add` or `Welcome` gave the current epoch's secret to
    /// is a member, or counts as holding it without being one, so that the members start a
    /// new epoch: entries past their author's run aside, which apply to nothing and are
    /// let go.
    fn check_no_key_unaccounted(log: &GroupLog, state: &GroupState) {
        let Some(current) = state.current_epoch() else {
            return;
        };
        let leaked = state.current_secret_leaked_to();
        let over = &log.full().budget.over;
        for hash in log.ordered() {
            if over.contains(&hash) {
                continue;
            }
            if let Body::Add { member, epoch, .. } | Body::Welcome { member, epoch, .. } =
                &log.entries[&hash].body
                && *epoch == current.id
            {
                assert!(state.is_member(member) || leaked.contains(member));
            }
        }
    }

    /// The group with its room rules worked out the long way: where each rule stands from
    /// each loss of its author by a walk of the graph for each of them.
    fn with_rules_walked(log: &GroupLog) -> GroupState {
        let (settled, _) = log.settled();
        let mut state = settled.clone();
        let full = log.full();
        let place = &full.order.place;
        for hash in &full.order.hashes {
            let entry = &log.entries[hash];
            if !entry.body.is_room_rule() {
                continue;
            }
            if full.budget.over.contains(hash) {
                state.refuse(*hash, PAST_RUN);
                continue;
            }
            let mut stand = crate::state::RuleStand::default();
            for (n, loss) in settled.losses_of(&entry.author).enumerate() {
                if log.is_before(place, &entry.parents, loss) {
                    continue;
                }
                if log.is_before(place, &log.entries[loss].parents, hash) {
                    stand.first_after.get_or_insert(n);
                } else {
                    stand.apart = true;
                    break;
                }
            }
            state.apply_rule(*hash, entry, stand);
        }
        state
    }

    /// Checks that what `log` carried forward is what it works out from nothing.
    fn same_as_worked_out_again(log: &GroupLog, identity: &DeviceIdentity, what: &str) {
        let mut again = log.clone();
        again.work_out_again();
        *again.cache = Cache::default();
        assert_eq!(log.ordered(), again.ordered(), "{what}: order");
        assert_eq!(log.heads(), again.heads(), "{what}: heads");
        assert_eq!(
            log.placed_heads(),
            again.placed_heads(),
            "{what}: placed heads"
        );
        assert_eq!(log.waiting(), again.waiting(), "{what}: waiting");
        assert_eq!(log.spare(), again.spare(), "{what}: spare");
        assert_eq!(
            log.needed_heads(),
            again.needed_heads(),
            "{what}: needed heads"
        );
        assert_eq!(log.hashes_in_order(), again.hashes_in_order(), "{what}");
        let (settled, outranked) = log.settled();
        let (settled_again, outranked_again) = again.settled();
        assert_eq!(outranked, outranked_again, "{what}: outranked");
        assert_eq!(
            format!("{settled:?}"),
            format!("{settled_again:?}"),
            "{what}: settled"
        );
        assert_eq!(
            format!("{:?}", log.state()),
            format!("{:?}", again.state()),
            "{what}: state"
        );
        assert_eq!(
            format!("{:?}", log.state()),
            format!("{:?}", with_rules_walked(log)),
            "{what}: rules"
        );
        let (_, keys, current) = log.state_and_keys(identity);
        let (_, keys_again, current_again) = again.state_and_keys(identity);
        assert_eq!(
            format!("{keys:?}"),
            format!("{keys_again:?}"),
            "{what}: keys"
        );
        assert_eq!(format!("{current:?}"), format!("{current_again:?}"));
    }

    /// What each log carries forward from one change to the next is, after every step of
    /// random histories, what it works out again from its entries.
    #[test]
    fn what_a_log_carries_forward_is_what_it_works_out_again() {
        let seeds = std::env::var("FOGATA_LADDER_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4u64);
        for seed in 1..=seeds {
            let mut world = LadderWorld::new(seed, 6);
            if seed % 2 == 1 {
                world = world.with_run_room(SMALL_RUN);
            }
            for step in 0..120 {
                world.step();
                for (i, log) in world.logs.iter().enumerate() {
                    let what = format!("seed {seed}, step {step}, device {i}");
                    same_as_worked_out_again(log, &world.devices[i].id, &what);
                }
            }
            world.sync_all();
            for (i, log) in world.logs.iter().enumerate() {
                same_as_worked_out_again(log, &world.devices[i].id, &format!("seed {seed}"));
            }
        }
    }

    /// `FOGATA_LADDER_SEEDS` sets how many histories run (4 by default).
    #[test]
    fn every_device_settles_on_the_same_ladder_whatever_happened_apart() {
        let seeds = std::env::var("FOGATA_LADDER_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4u64);
        for seed in 1..=seeds {
            let mut world = LadderWorld::new(seed, 6);
            if seed % 2 == 1 {
                world = world.with_run_room(SMALL_RUN);
            }
            for _ in 0..120 {
                world.step();
            }
            world.sync_all();
            let state = world.logs[0].state();
            for (i, log) in world.logs.iter().enumerate().skip(1) {
                let other = log.state();
                assert_eq!(other.applied, state.applied, "seed {seed}: device {i}");
                assert_eq!(other.ladder, state.ladder, "seed {seed}: device {i}");
                assert_eq!(other.managers, state.managers, "seed {seed}");
                assert_eq!(other.rules, state.rules, "seed {seed}");
            }
            // The same entries in any order make the same group.
            let mut entries: Vec<Entry> = world.logs[0].entries().cloned().collect();
            for k in 0..entries.len() {
                let at = world.below(entries.len());
                entries.swap(k, at);
            }
            let mut shuffled = world.empty_log();
            shuffled.insert_all(entries);
            let again = shuffled.state();
            assert_eq!(again.applied, state.applied, "seed {seed}: another order");
            assert_eq!(again.ladder, state.ladder, "seed {seed}: another order");
            assert_eq!(again.rules, state.rules, "seed {seed}: another order");
            check_every_step(&world.logs[0], &state);
            // The activity replays the same steps, and tells only entries that happened.
            let activity = world.logs[0].activity();
            assert!(activity.iter().all(|a| state.applied.contains(&a.entry)
                || (a.undone.is_some() && state.rejected.iter().any(|(h, _)| *h == a.entry))));
            // The same as working it out with a copy of the group at each entry.
            assert_eq!(activity, world.logs[0].activity_by_copies(), "seed {seed}");
        }
    }
}
