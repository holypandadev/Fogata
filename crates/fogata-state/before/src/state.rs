//! The rules: what each entry may do, and the group state that results from applying the log
//! in order.

use std::collections::{BTreeMap, BTreeSet};

pub use fogata_crypto::names::hiding_mark;
use fogata_crypto::{DevicePublicKey, GroupId, names::tidy};

use crate::entry::{
    Body, Entry, EntryHash, EpochId, FIRST_ROOM, InviteToken, Rotation, Ticket, VoiceRoomId,
    epoch_number,
};

/// The most members a group takes, and the most it starts with.
pub const MAX_MEMBERS: u8 = 20;
/// The most people in a voice room at once, and the most a room starts with: voice and
/// video go straight between everyone in it.
pub const MAX_IN_ROOM: u8 = 8;
/// The most voice rooms a group has.
pub const MAX_ROOMS: usize = 10;
/// The longest voice room name, in characters.
pub const MAX_ROOM_NAME_CHARS: usize = 40;
/// Most characters in a group's name.
pub const MAX_GROUP_NAME_CHARS: usize = 60;
/// Most characters in the name a device joins or is added under.
pub const MAX_MEMBER_NAME_CHARS: usize = 40;

/// Whether `name` is at most `max` characters, none of them control characters or marks
/// that hide in text or turn it around: those let a name pass for another one, or reverse
/// what is shown after it. Names are kept as they show ([`tidy`]): one of nothing but blank
/// characters counts as empty, and padding goes.
pub fn fair_name(name: &str, max: usize) -> bool {
    name.chars().count() <= max && !name.chars().any(|c| c.is_control() || hiding_mark(c))
}

/// Why a `Welcome` by a member that does not manage the group is ignored.
pub const WELCOME_NOT_MANAGER: &str = "keys come from those who manage the group";
/// Why a `Welcome` lets nobody in: its device is no member, and no `Join` of that device
/// before it can still be used.
pub const NO_JOIN_TO_WELCOME: &str = "not a member";

/// Why a `Rotate` by a member that does not manage the group is ignored when it had no
/// reason to rotate.
pub const ROTATE_NOT_NEEDED: &str = "a member rotates only when someone gone has the key";

/// Why an entry of the kinds for MLS groups changes nothing in this group.
pub const MLS_NOT_YET: &str = "entries for MLS groups count only under group rules that take them";

/// The biggest group picture, so that its entry stays well under the entry limit.
pub const MAX_PICTURE_BYTES: usize = 48 * 1024;

/// A voice room of the group: its own people, chat, files and calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceRoom {
    pub id: VoiceRoomId,
    /// Empty for the first room until someone names it.
    pub name: String,
}

/// A member as recorded in the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
}

/// Who manages the group: changes its name, pictures, rooms and limits, invites and lets
/// people in, and hands out keys. Everyone acts on the members below it on the ladder
/// either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Managers {
    /// Every member.
    #[default]
    Everyone,
    /// The first `n` members on the ladder.
    First(u8),
}

impl Managers {
    /// Whether the member at `rank` (0 for the first) manages the group.
    pub fn include(self, rank: usize) -> bool {
        match self {
            Managers::Everyone => true,
            Managers::First(n) => rank < usize::from(n),
        }
    }

    /// The setting as a `SetManagers` entry carries it: `None` for everyone.
    pub fn first(self) -> Option<u8> {
        match self {
            Managers::Everyone => None,
            Managers::First(n) => Some(n),
        }
    }
}

/// What a member had just before it was demoted, removed or left: the ladder and who
/// managed then, by the clock of the entry that ended it. A room rule it signed before still
/// counts with those rights.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loss {
    time_ms: u64,
    ladder: Vec<DevicePublicKey>,
    managers: Managers,
}

/// A member kept out of a voice room: by whom, since when and until when (by the clocks of
/// the member who did it), or until someone above it lets it back (`until_ms` is `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptOut {
    pub by: DevicePublicKey,
    pub since_ms: u64,
    pub until_ms: Option<u64>,
}

/// Who may enter the voice rooms, from the room rules of the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RoomRules {
    /// Rooms only some members may enter, with those members.
    pub only: BTreeMap<VoiceRoomId, BTreeSet<DevicePublicKey>>,
    /// For each of those rooms, the members the rule's author could not leave out, who may
    /// enter anyway: itself and those above it on the ladder.
    pub always_in: BTreeMap<VoiceRoomId, BTreeSet<DevicePublicKey>>,
    /// Members kept out of a room, by room and member, the ones run out included.
    pub kept_out: BTreeMap<(VoiceRoomId, DevicePublicKey), KeptOut>,
}

/// Why a member may not enter a voice room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Barrier {
    /// Someone above it kept it out, for a while or until someone above it lets it back.
    TimedOut(KeptOut),
    /// Only some members may enter the room, and it is not one of them.
    NotChosen,
}

/// A key epoch that started validly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Epoch {
    pub id: EpochId,
    pub commitment: [u8; 32],
    /// When it started, by the clock of the device that started it.
    pub since_ms: u64,
}

/// Group state after applying every known entry.
#[derive(Clone, Debug, Default)]
pub struct GroupState {
    pub group_id: Option<GroupId>,
    pub name: String,
    /// One of the preset icons the application draws.
    pub icon: u8,
    /// A small PNG or JPEG shown instead of the icon, from the last `SetPicture` entry
    /// (see [`crate::GroupLog::picture`] for the one to show).
    pub picture: Option<Vec<u8>>,
    /// When that entry was signed, by its author's clock; `None` when none applied.
    pub picture_ms: Option<u64>,
    /// The most members the group takes: adds and joins past it are ignored.
    pub max_members: u8,
    /// The most people in each voice room at once.
    pub max_in_room: u8,
    /// The voice rooms in the order they were opened; the first one always stays.
    pub rooms: Vec<VoiceRoom>,
    /// The device that started the group. No rights come with it: those come from the
    /// ladder.
    pub creator: Option<DevicePublicKey>,
    pub members: BTreeMap<DevicePublicKey, Member>,
    /// Every member, the first one first: each acts on those below it. A member that comes
    /// in goes to the end; one that leaves or is removed is taken out, and goes to the end
    /// again if it comes back. A member moves someone below it up to just below itself, or
    /// down to the end.
    pub ladder: Vec<DevicePublicKey>,
    /// Who manages the group (see [`Managers`]).
    pub managers: Managers,
    /// Valid epochs in the order they started; the last one is current.
    pub epochs: Vec<Epoch>,
    pub used_tickets: BTreeSet<[u8; 16]>,
    /// Invites not used yet, by ticket id: their token and when they expire.
    pub open_invites: BTreeMap<[u8; 16], (InviteToken, u64)>,
    /// Entries that applied, in order.
    pub applied: Vec<EntryHash>,
    /// Entries that were ignored, with the reason.
    pub rejected: Vec<(EntryHash, &'static str)>,
    /// Who may enter the voice rooms.
    pub rules: RoomRules,
    /// What each device had before each time it was demoted, removed or left, in the order
    /// of the log (it may be back since).
    losses: BTreeMap<DevicePublicKey, Vec<Loss>>,
    /// Who was given each epoch's secret.
    holders: BTreeMap<EpochId, BTreeSet<DevicePublicKey>>,
    /// Members given a key since they last came in: the creator, a device a manager added,
    /// one a manager welcomed after its `Join`. A rotation hands such a member newer keys,
    /// and gives none to a member that is not one: any member can write a `Join` with a
    /// ticket it saw in the log.
    keyed: BTreeSet<DevicePublicKey>,
    /// `Join`s that could still let their device in, by device, in the order they came:
    /// a join only lets its device in, and uses its ticket, with a `Welcome` of a manager
    /// signed on it (see [`Body::Join`]).
    waiting_joins: BTreeMap<DevicePublicKey, Vec<WaitingJoin>>,
}

/// A `Join` that waits for a manager to let its device in.
#[derive(Clone, Debug)]
struct WaitingJoin {
    hash: EntryHash,
    ticket: Ticket,
    name: String,
}

/// What applying an entry did.
enum Applied {
    /// It applied.
    Yes,
    /// A `Join`: it waits for a `Welcome` of a manager (see [`Body::Join`]).
    Waits,
    /// A `Welcome` that let in a device with its `Join`, which applies with it, just before.
    LetIn(EntryHash),
}

impl GroupState {
    pub fn current_epoch(&self) -> Option<&Epoch> {
        self.epochs.last()
    }

    pub fn epoch(&self, id: &EpochId) -> Option<&Epoch> {
        self.epochs.iter().find(|e| &e.id == id)
    }

    pub fn is_member(&self, key: &DevicePublicKey) -> bool {
        self.members.contains_key(key)
    }

    /// Where `key` is on the ladder, 0 for the first; `None` for a device that is no member.
    pub fn rank(&self, key: &DevicePublicKey) -> Option<usize> {
        self.ladder.iter().position(|k| k == key)
    }

    /// The first member on the ladder.
    pub fn first(&self) -> Option<DevicePublicKey> {
        self.ladder.first().copied()
    }

    /// Whether `key` is a member that manages the group (see [`Managers`]).
    pub fn is_manager(&self, key: &DevicePublicKey) -> bool {
        self.rank(key).is_some_and(|r| self.managers.include(r))
    }

    /// Members that have not been given the current epoch's secret yet.
    pub fn lacking_current_secret(&self) -> Vec<DevicePublicKey> {
        let Some(current) = self.current_epoch() else {
            return Vec::new();
        };
        let holders = self.holders.get(&current.id);
        self.members
            .keys()
            .filter(|k| holders.is_none_or(|h| !h.contains(k)))
            .copied()
            .collect()
    }

    /// Tokens of the invites still open at `now_ms`.
    pub fn open_invite_tokens(&self, now_ms: u64) -> Vec<InviteToken> {
        self.open_invites
            .values()
            .filter(|(_, expires)| now_ms <= *expires)
            .map(|(token, _)| *token)
            .collect()
    }

    /// Whether `key` is a member a manager gave a key since it came in (see `keyed`): the
    /// only members that may prove themselves.
    pub fn keyed(&self, key: &DevicePublicKey) -> bool {
        self.is_member(key) && self.keyed.contains(key)
    }

    /// The ticket of the first `Join` of `device`, no member, that a manager could still
    /// let it in with (see [`Body::Join`]).
    pub fn waiting_join(&self, device: &DevicePublicKey) -> Option<&Ticket> {
        self.usable_join(device).map(|j| &j.ticket)
    }

    /// The entry of [`Self::waiting_join`].
    pub(crate) fn waiting_join_entry(&self, device: &DevicePublicKey) -> Option<EntryHash> {
        self.usable_join(device).map(|j| j.hash)
    }

    fn usable_join(&self, device: &DevicePublicKey) -> Option<&WaitingJoin> {
        if self.is_member(device) {
            return None;
        }
        self.waiting_joins
            .get(device)?
            .iter()
            .find(|j| self.ticket_usable(&j.ticket))
    }

    /// Whether a device could still be let in with `ticket` here: one of this group's, by a
    /// manager, not used nor cancelled.
    fn ticket_usable(&self, ticket: &Ticket) -> bool {
        Some(ticket.group_id) == self.group_id
            && self.is_manager(&ticket.issuer)
            && !self.used_tickets.contains(&ticket.ticket_id)
    }

    /// Whether every device given the secret of the epoch numbered `number` is still a
    /// member: a proof made with its key then comes from a member, whichever epoch it is.
    /// False for an epoch this state does not have.
    pub fn epoch_kept(&self, number: u32) -> bool {
        let mut epochs = self
            .epochs
            .iter()
            .filter(|e| epoch_number(&e.id) == number)
            .peekable();
        epochs.peek().is_some()
            && epochs.all(|e| {
                self.holders
                    .get(&e.id)
                    .is_some_and(|h| h.iter().all(|k| self.is_member(k)))
            })
    }

    /// Whether `key` was given the secret of any epoch, that is, ever had a group key.
    pub fn ever_held(&self, key: &DevicePublicKey) -> bool {
        self.holders.values().any(|h| h.contains(key))
    }

    /// Devices that hold the current epoch's secret without being members: someone left
    /// or was removed, or a rotation was discarded. The members should start a new epoch.
    pub fn current_secret_leaked_to(&self) -> Vec<DevicePublicKey> {
        let Some(current) = self.current_epoch() else {
            return Vec::new();
        };
        self.holders
            .get(&current.id)
            .into_iter()
            .flatten()
            .filter(|k| !self.is_member(k))
            .copied()
            .collect()
    }

    /// Members that hold the current secret, in the order of the ladder. The first one is
    /// the one expected to act (for example, to rotate) when several could.
    pub fn current_holders_in_order(&self) -> Vec<DevicePublicKey> {
        let Some(current) = self.current_epoch() else {
            return Vec::new();
        };
        let holders = self.holders.get(&current.id);
        self.ladder
            .iter()
            .filter(|k| holders.is_some_and(|h| h.contains(k)))
            .copied()
            .collect()
    }

    /// Why `device` may not enter voice room `room` at `now_ms`, by this device's clock, if
    /// it may not. Every member applies the same rules; each compares the end of a time out
    /// with its own clock, so a skewed one lets someone back that much early or late, and
    /// the presence a member in a room repeats every few seconds brings it in everywhere
    /// once every clock agrees.
    pub fn barrier(
        &self,
        room: &VoiceRoomId,
        device: &DevicePublicKey,
        now_ms: u64,
    ) -> Option<Barrier> {
        if let Some(kept) = self.rules.kept_out.get(&(*room, *device))
            && kept.until_ms.is_none_or(|until| now_ms < until)
        {
            return Some(Barrier::TimedOut(*kept));
        }
        let always_in = |device: &DevicePublicKey| {
            self.rules
                .always_in
                .get(room)
                .is_some_and(|keys| keys.contains(device))
        };
        if self
            .rules
            .only
            .get(room)
            .is_some_and(|only| !only.contains(device))
            && !always_in(device)
        {
            return Some(Barrier::NotChosen);
        }
        None
    }

    /// Members kept out of a room at `now_ms` (see [`Self::barrier`]), by room and member.
    pub fn kept_out_now(&self, now_ms: u64) -> Vec<(VoiceRoomId, DevicePublicKey, KeptOut)> {
        self.rules
            .kept_out
            .iter()
            .filter(|(_, kept)| kept.until_ms.is_none_or(|until| now_ms < until))
            .map(|((room, member), kept)| (*room, *member, *kept))
            .collect()
    }

    /// Whether `author` is above `member` on the ladder, both members: it may then remove
    /// it, move it up or down, and keep it out of the rooms.
    pub fn outranks(&self, author: &DevicePublicKey, member: &DevicePublicKey) -> bool {
        match (self.rank(author), self.rank(member)) {
            (Some(a), Some(m)) => a < m,
            _ => false,
        }
    }

    /// The members below `author` on the ladder, the next one first.
    pub fn below(&self, author: &DevicePublicKey) -> &[DevicePublicKey] {
        match self.rank(author) {
            Some(r) => &self.ladder[r + 1..],
            None => &[],
        }
    }

    /// Where `device` stood on the ladder just before the last time it was removed or
    /// left, while it is no member.
    pub(crate) fn rank_when_gone(&self, device: &DevicePublicKey) -> Option<usize> {
        if self.is_member(device) {
            return None;
        }
        let loss = self.losses.get(device)?.last()?;
        loss.ladder.iter().position(|k| k == device)
    }

    fn admit(&mut self, key: DevicePublicKey, member: Member) {
        self.members.insert(key, member);
        self.keyed.remove(&key);
        self.waiting_joins.remove(&key);
        self.ladder.retain(|k| *k != key);
        self.ladder.push(key);
    }

    /// Notes what `device` had before an entry signed at `time_ms` demotes it, removes it
    /// or has it leave.
    fn note_loss(&mut self, device: &DevicePublicKey, time_ms: u64) {
        let loss = Loss {
            time_ms,
            ladder: self.ladder.clone(),
            managers: self.managers,
        };
        self.losses.entry(*device).or_default().push(loss);
    }

    /// Takes `device` out of the group.
    fn drop_member(&mut self, device: &DevicePublicKey) {
        self.members.remove(device);
        self.ladder.retain(|k| k != device);
    }

    pub(crate) fn refuse(&mut self, hash: EntryHash, reason: &'static str) {
        self.rejected.push((hash, reason));
    }

    /// Applies `entry`, or notes why not. `rotation_needed` says, for a `Rotate` by a
    /// member that does not manage the group here, whether its author managed it or saw someone who
    /// is no longer a member hold the current secret, in the log as it was when it wrote it
    /// (`None` for any other entry).
    ///
    /// `before` tells whether an entry of the log is before this one (an ancestor): a
    /// `Welcome` lets a device in only with a `Join` it is signed on.
    pub(crate) fn apply(
        &mut self,
        hash: EntryHash,
        entry: &Entry,
        rotation_needed: Option<bool>,
        before: &dyn Fn(&EntryHash) -> bool,
    ) {
        match self.check_and_apply(hash, entry, rotation_needed, before) {
            Ok(Applied::Yes) => self.applied.push(hash),
            Ok(Applied::Waits) => {}
            Ok(Applied::LetIn(join)) => {
                self.applied.push(join);
                self.applied.push(hash);
            }
            Err(reason) => self.rejected.push((hash, reason)),
        }
    }

    /// Applies a room rule, once every other entry applied: rules are checked against the
    /// group as it is, the same on every member whatever order the rules arrived in. An
    /// author that was demoted, removed or left since still counts, for the rules it signed
    /// before, with the place it had then (by the entries' clocks, as when no entry was
    /// signed on a rule).
    pub(crate) fn apply_rule(&mut self, hash: EntryHash, entry: &Entry) {
        match self.check_rule(entry) {
            Ok(()) => self.applied.push(hash),
            Err(reason) => self.rejected.push((hash, reason)),
        }
    }

    fn check_rule(&mut self, entry: &Entry) -> Result<(), &'static str> {
        let author = entry.author;
        // The ladder and managers the author's rights are judged on: those it had before its
        // first loss after the rule, or those of now.
        let lost_later = self
            .losses
            .get(&author)
            .and_then(|losses| losses.iter().find(|l| entry.time_ms < l.time_ms));
        let (ladder, managers) = match lost_later {
            Some(loss) => (loss.ladder.clone(), loss.managers),
            None => (self.ladder.clone(), self.managers),
        };
        let Some(rank) = ladder.iter().position(|k| *k == author) else {
            return Err("author is not a member");
        };
        // A member that came in after that ladder was below the author.
        let acts_on = |state: &Self, member: &DevicePublicKey| {
            author != *member
                && state.is_member(member)
                && ladder
                    .iter()
                    .position(|k| k == member)
                    .is_none_or(|m| rank < m)
        };
        let room_known = |room: &VoiceRoomId| self.rooms.iter().any(|r| r.id == *room);
        match &entry.body {
            Body::RoomAccess { room, members } => {
                if !managers.include(rank) {
                    return Err("author does not manage the group");
                }
                if !room_known(room) {
                    return Err("no such room");
                }
                match members {
                    Some(members) => {
                        self.rules
                            .only
                            .insert(*room, members.iter().copied().collect());
                        self.rules
                            .always_in
                            .insert(*room, ladder[..=rank].iter().copied().collect());
                    }
                    None => {
                        self.rules.only.remove(room);
                        self.rules.always_in.remove(room);
                    }
                }
            }
            Body::KeepOut {
                room,
                member,
                until_ms,
            } => {
                if !room_known(room) {
                    return Err("no such room");
                }
                if !acts_on(self, member) {
                    return Err("not someone the author may keep out");
                }
                if until_ms.is_some_and(|until| until <= entry.time_ms) {
                    return Err("over before it starts");
                }
                self.rules.kept_out.insert(
                    (*room, *member),
                    KeptOut {
                        by: author,
                        since_ms: entry.time_ms,
                        until_ms: *until_ms,
                    },
                );
            }
            Body::LetBack { room, member } => {
                if !acts_on(self, member) {
                    return Err("not someone the author may let back");
                }
                self.rules.kept_out.remove(&(*room, *member));
            }
            _ => return Err("not a room rule"),
        }
        Ok(())
    }

    /// Whether `entry` would apply on top of this group, as the last entry of a log whose
    /// group this is: what its author could make in its own view of the log (see
    /// [`crate::GroupLog::insert`]). `rotation_needed` as for [`Self::apply`]. Changes
    /// nothing here.
    ///
    /// A `Join` that would wait for a manager's `Welcome` is one its author could make;
    /// `before` as for [`Self::apply`].
    pub(crate) fn allows(
        &self,
        entry: &Entry,
        rotation_needed: Option<bool>,
        before: &dyn Fn(&EntryHash) -> bool,
    ) -> Result<(), &'static str> {
        let mut trial = self.for_judging(false, entry.body.is_room_rule());
        if entry.body.is_room_rule() {
            trial.check_rule(entry)
        } else {
            trial
                .check_and_apply(entry.hash(), entry, rotation_needed, before)
                .map(drop)
        }
    }

    /// Whether `rotation` is sealed only to members other than `removed`: what a removal
    /// written here holds. (Its epoch may still not start, under a number another epoch
    /// took at the same time; the removal stands then.)
    pub(crate) fn rotation_for_members(
        &self,
        rotation: &Rotation,
        removed: &DevicePublicKey,
    ) -> bool {
        rotation
            .sealed
            .iter()
            .all(|(k, _)| self.is_member(k) && k != removed)
    }

    /// This group without what no entry is judged on (the lists of the entries that applied
    /// and were refused, who was given a key since it came in, open invites, the room
    /// rules): what judging an entry on the group it saw needs ([`Self::allows`], and whether
    /// a rotation was needed). `with_holders` keeps who was given each epoch's secret, which
    /// only that last question reads, and `with_losses` what members had before they lost
    /// it, which only room rules read.
    pub(crate) fn for_judging(&self, with_holders: bool, with_losses: bool) -> Self {
        Self {
            group_id: self.group_id,
            name: self.name.clone(),
            icon: self.icon,
            picture: None,
            picture_ms: self.picture_ms,
            max_members: self.max_members,
            max_in_room: self.max_in_room,
            rooms: self.rooms.clone(),
            creator: self.creator,
            members: self.members.clone(),
            ladder: self.ladder.clone(),
            managers: self.managers,
            epochs: self.epochs.clone(),
            used_tickets: self.used_tickets.clone(),
            open_invites: BTreeMap::new(),
            applied: Vec::new(),
            rejected: Vec::new(),
            rules: RoomRules::default(),
            losses: if with_losses {
                self.losses.clone()
            } else {
                BTreeMap::new()
            },
            holders: if with_holders {
                self.holders.clone()
            } else {
                BTreeMap::new()
            },
            keyed: BTreeSet::new(),
            waiting_joins: self.waiting_joins.clone(),
        }
    }

    /// Whether `entry` is a `Rotate` whose author is a member that does not manage the group
    /// at this point: the one kind of entry that needs its author's view of the log to be
    /// judged.
    pub(crate) fn rotation_to_judge(&self, entry: &Entry) -> bool {
        matches!(entry.body, Body::Rotate { .. })
            && self.is_member(&entry.author)
            && !self.is_manager(&entry.author)
    }

    fn check_and_apply(
        &mut self,
        hash: EntryHash,
        entry: &Entry,
        rotation_needed: Option<bool>,
        before: &dyn Fn(&EntryHash) -> bool,
    ) -> Result<Applied, &'static str> {
        let author = entry.author;
        if let Body::Create { name, start } = &entry.body {
            if self.creator.is_some() {
                return Err("second create");
            }
            if !entry.parents.is_empty() {
                return Err("create with parents");
            }
            if !fair_name(name, MAX_GROUP_NAME_CHARS) {
                return Err("invalid group name");
            }
            self.group_id = Some(entry.group_id);
            self.creator = Some(author);
            self.name = tidy(name);
            self.max_members = MAX_MEMBERS;
            self.max_in_room = MAX_IN_ROOM;
            self.rooms = vec![VoiceRoom {
                id: FIRST_ROOM,
                name: String::new(),
            }];
            self.admit(
                author,
                Member {
                    name: String::new(),
                },
            );
            self.keyed.insert(author);
            return self
                .start_epoch(start, entry.time_ms)
                .map(|()| Applied::Yes)
                .map_err(|_| "invalid first epoch");
        }
        if self.creator.is_none() {
            return Err("before create");
        }
        match &entry.body {
            Body::Create { .. } => unreachable!("handled above"),
            Body::Rename { name } => {
                self.require_manager(&author)?;
                if tidy(name).is_empty() || !fair_name(name, MAX_GROUP_NAME_CHARS) {
                    return Err("invalid group name");
                }
                self.name = tidy(name);
            }
            Body::SetIcon { icon } => {
                self.require_manager(&author)?;
                self.icon = *icon;
            }
            Body::Promote { member } => {
                self.require_above(&author, member)?;
                self.ladder.retain(|k| k != member);
                let below_author = self.rank(&author).expect("a member") + 1;
                self.ladder.insert(below_author, *member);
            }
            Body::Demote { member } => {
                self.require_above(&author, member)?;
                self.note_loss(member, entry.time_ms);
                self.ladder.retain(|k| k != member);
                self.ladder.push(*member);
            }
            Body::SetManagers { first } => {
                self.require_manager(&author)?;
                if let Some(n) = first {
                    if !(1..=MAX_MEMBERS).contains(n) {
                        return Err("managers out of range");
                    }
                    // Nobody leaves itself out, nor anyone above it.
                    if !Managers::First(*n).include(self.rank(&author).expect("a member")) {
                        return Err("would leave its author out");
                    }
                }
                self.managers = match first {
                    Some(n) => Managers::First(*n),
                    None => Managers::Everyone,
                };
            }
            Body::Add {
                member,
                name,
                epoch,
                ..
            } => {
                self.require_manager(&author)?;
                if !fair_name(name, MAX_MEMBER_NAME_CHARS) {
                    return Err("invalid member name");
                }
                if self.is_member(member) {
                    return Err("already a member");
                }
                self.require_room_for_one_more()?;
                if self.epoch(epoch).is_none() {
                    return Err("unknown epoch");
                }
                self.admit(*member, Member { name: tidy(name) });
                self.holders.entry(*epoch).or_default().insert(*member);
                self.keyed.insert(*member);
            }
            Body::Join { ticket, name } => {
                // A join lets nobody in by itself, nor uses its ticket: any member can sign
                // one with a ticket it saw in the log, under a key of its own and on parents
                // as old as it likes, and the first in the order would otherwise take the
                // ticket from the device a manager let in (and its place, its keys). It
                // waits for a manager's `Welcome` signed on it.
                if !fair_name(name, MAX_MEMBER_NAME_CHARS) {
                    return Err("invalid member name");
                }
                if self.is_member(&author) {
                    return Err("already a member");
                }
                if Some(ticket.group_id) != self.group_id || !ticket.signature_valid() {
                    return Err("invalid ticket");
                }
                if !self.is_manager(&ticket.issuer) {
                    return Err("ticket issuer does not manage the group");
                }
                if self.used_tickets.contains(&ticket.ticket_id) {
                    return Err("ticket already used");
                }
                if entry.time_ms > ticket.expires_ms {
                    return Err("ticket expired");
                }
                self.require_room_for_one_more()?;
                // Each one: a `Welcome` lets the device in with one it is signed on.
                self.waiting_joins
                    .entry(author)
                    .or_default()
                    .push(WaitingJoin {
                        hash,
                        ticket: ticket.clone(),
                        name: tidy(name),
                    });
                return Ok(Applied::Waits);
            }
            Body::Welcome { member, epoch, .. } => {
                // Keys come from those who manage the group, and so does letting in a device
                // that joined with a ticket: the manager that let it in, who checked the
                // ticket against the clock, signs its `Welcome` on that `Join`, which then
                // applies and uses the ticket. Otherwise a member that does not manage could
                // let anyone in with an expired ticket and a backdated join, or take the
                // ticket from the one let in. And a member's box that opens to nothing would
                // mark it as given the key, so that nobody gives it again.
                if !self.is_manager(&author) {
                    return Err(WELCOME_NOT_MANAGER);
                }
                if self.epoch(epoch).is_none() {
                    return Err("unknown epoch");
                }
                let mut let_in = None;
                if !self.is_member(member) {
                    // The join it is signed on directly, the one the manager saw; else the
                    // first before it in the order.
                    let usable = |j: &&WaitingJoin| self.ticket_usable(&j.ticket);
                    let joins = || self.waiting_joins.get(member).into_iter().flatten();
                    let join = joins()
                        .filter(usable)
                        .find(|j| entry.parents.contains(&j.hash))
                        .or_else(|| joins().filter(usable).find(|j| before(&j.hash)))
                        .cloned()
                        .ok_or(NO_JOIN_TO_WELCOME)?;
                    self.require_room_for_one_more()?;
                    self.used_tickets.insert(join.ticket.ticket_id);
                    self.open_invites.remove(&join.ticket.ticket_id);
                    self.admit(*member, Member { name: join.name });
                    let_in = Some(join.hash);
                }
                self.holders.entry(*epoch).or_default().insert(*member);
                self.keyed.insert(*member);
                if let Some(join) = let_in {
                    return Ok(Applied::LetIn(join));
                }
            }
            Body::Remove { member, rotation } => {
                self.require_above(&author, member)?;
                self.note_loss(member, entry.time_ms);
                self.drop_member(member);
                // A removal stands even if its rotation cannot be used.
                let _ = self.start_epoch(rotation, entry.time_ms);
            }
            Body::Invite { ticket, token } => {
                self.require_manager(&author)?;
                if ticket.issuer != author
                    || Some(ticket.group_id) != self.group_id
                    || !ticket.signature_valid()
                {
                    return Err("invalid ticket");
                }
                if !self.used_tickets.contains(&ticket.ticket_id) {
                    self.open_invites
                        .insert(ticket.ticket_id, (*token, ticket.expires_ms));
                }
            }
            Body::SetPicture { picture } => {
                self.require_manager(&author)?;
                if picture
                    .as_ref()
                    .is_some_and(|p| p.len() > MAX_PICTURE_BYTES)
                {
                    return Err("picture too big");
                }
                self.picture = picture.clone();
                self.picture_ms = Some(entry.time_ms);
            }
            Body::SetLimits { members, room } => {
                self.require_manager(&author)?;
                if !(2..=MAX_MEMBERS).contains(members) || !(2..=MAX_IN_ROOM).contains(room) {
                    return Err("limits out of range");
                }
                if usize::from(*members) < self.members.len() {
                    return Err("fewer than the members already in");
                }
                self.max_members = *members;
                self.max_in_room = *room;
            }
            Body::SetRoom { id, name } => {
                self.require_manager(&author)?;
                if tidy(name).is_empty() || !fair_name(name, MAX_ROOM_NAME_CHARS) {
                    return Err("invalid room name");
                }
                let name = tidy(name);
                if let Some(room) = self.rooms.iter_mut().find(|r| r.id == *id) {
                    room.name = name;
                } else if self.rooms.len() >= MAX_ROOMS {
                    return Err("too many rooms");
                } else {
                    self.rooms.push(VoiceRoom { id: *id, name });
                }
            }
            Body::RemoveRoom { id } => {
                self.require_manager(&author)?;
                if *id == FIRST_ROOM {
                    return Err("the first room stays");
                }
                let at = self
                    .rooms
                    .iter()
                    .position(|r| r.id == *id)
                    .ok_or("no such room")?;
                self.rooms.remove(at);
            }
            Body::Revoke { ticket_id } => {
                self.require_manager(&author)?;
                self.open_invites.remove(ticket_id);
                self.used_tickets.insert(*ticket_id);
            }
            Body::Leave => {
                self.require_member(&author)?;
                self.note_loss(&author, entry.time_ms);
                self.drop_member(&author);
            }
            Body::Rotate { rotation } => {
                self.require_member(&author)?;
                // Managers may always start a new epoch; another member only when it saw
                // someone who is no longer a member hold the current secret, which is when
                // every version rotates by itself. Judged by what its author saw, not by the
                // common order: two members that rotate at once both apply.
                if !self.is_manager(&author) && rotation_needed == Some(false) {
                    return Err(ROTATE_NOT_NEEDED);
                }
                self.start_epoch(rotation, entry.time_ms)
                    .map_err(|_| "rotation includes non-members or an epoch number in use")?;
            }
            Body::RoomAccess { .. } | Body::KeepOut { .. } | Body::LetBack { .. } => {
                return Err("room rules apply on top");
            }
            // Group rules that take them are still to come: until then they change nothing,
            // as in versions that read them as kinds they do not know.
            Body::MlsKeyPackage { .. }
            | Body::MlsJoin { .. }
            | Body::Commit { .. }
            | Body::Resync { .. } => return Err(MLS_NOT_YET),
            // Its place in the order stays, so whatever was written on top of it applies.
            Body::Unknown { .. } => return Err("kind of entry unknown to this version"),
        }
        Ok(Applied::Yes)
    }

    /// Starts the epoch of `rotation` if every recipient is a member at this point and no
    /// epoch already has its number. A
    /// rotation written before a concurrent removal may still include the removed device;
    /// its secret is then never used, so that device never gets a newer key.
    fn start_epoch(&mut self, rotation: &Rotation, since_ms: u64) -> Result<(), ()> {
        let recipients: BTreeSet<DevicePublicKey> =
            rotation.sealed.iter().map(|(k, _)| *k).collect();
        // Keys and message headers name an epoch by its number, so a second epoch with the
        // number of one already in the log would take its place.
        let number = epoch_number(&rotation.epoch);
        if recipients.is_empty()
            || !recipients.iter().all(|k| self.is_member(k))
            || self.epochs.iter().any(|e| epoch_number(&e.id) == number)
        {
            return Err(());
        }
        self.epochs.push(Epoch {
            id: rotation.epoch,
            commitment: rotation.commitment,
            since_ms,
        });
        self.holders.insert(rotation.epoch, recipients);
        Ok(())
    }

    fn require_room_for_one_more(&self) -> Result<(), &'static str> {
        if self.members.len() < usize::from(self.max_members) {
            Ok(())
        } else {
            Err("the group is full")
        }
    }

    fn require_member(&self, key: &DevicePublicKey) -> Result<(), &'static str> {
        if self.is_member(key) {
            Ok(())
        } else {
            Err("author is not a member")
        }
    }

    fn require_manager(&self, key: &DevicePublicKey) -> Result<(), &'static str> {
        if self.is_manager(key) {
            Ok(())
        } else if self.is_member(key) {
            Err("author does not manage the group")
        } else {
            Err("author is not a member")
        }
    }

    fn require_above(
        &self,
        author: &DevicePublicKey,
        member: &DevicePublicKey,
    ) -> Result<(), &'static str> {
        if !self.is_member(member) {
            Err("not a member")
        } else if self.outranks(author, member) {
            Ok(())
        } else {
            Err("not someone below the author")
        }
    }
}
