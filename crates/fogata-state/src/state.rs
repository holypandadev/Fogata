//! The rules: what each entry may do, and the group state that results from applying the log
//! in order.

use std::collections::{BTreeMap, BTreeSet};

pub use fogata_crypto::names::hiding_mark;
use fogata_crypto::{DevicePublicKey, GroupId, names::tidy};

use crate::entry::{
    Body, Entry, EntryHash, EpochId, FIRST_ROOM, InviteToken, NamedInvite, Rotation, Ticket,
    VoiceRoomId, epoch_number,
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

/// Why an entry that a removal, demotion or leave of its author outranks is ignored.
pub(crate) const OUTRANKED: &str = "signed apart from its author's removal";

/// Why an entry of the kinds for MLS groups changes nothing in this group.
pub const MLS_NOT_YET: &str = "entries for MLS groups count only under group rules that take them";

/// Why a `Level` entry naming a level this version does not judge at is refused: the
/// entries after it would be judged by rules this version does not have.
pub const LEVEL_UNKNOWN: &str = "a level this version does not judge at";

/// Why a `Level` entry that does not raise the group's level is ignored.
pub const LEVEL_NOT_HIGHER: &str = "not above the group's level";

/// Why, from level three, an invite that does not name who lets its device in is ignored.
pub const INVITE_NAMES_NOBODY: &str = "an invite names who may let its device in";

/// Why, from level three, a `Welcome` that would let a device in with an invite is ignored
/// when its author is neither the invite's issuer nor a manager the invite names.
pub const WELCOME_NOT_NAMED: &str =
    "only the issuer or a manager its invite names lets a device in";

/// Why, from level three, a `Rotate` that leaves out a member it saw is not kept.
pub const ROTATE_LEAVES_OUT: &str = "a rotation gives the new key to every member";

/// A level of judgment: the rules that decide what a log keeps and applies once a
/// [`Body::Level`] entry naming it applied. The rules of a level never
/// change once written: a change to what is kept or applied forks the log between copies
/// that judge differently (an entry one refuses is not kept there, and everything signed
/// on it waits for ever), so it comes as a new level, which a manager raises the group to
/// once every member's copy judges at it (the level a device speaks, see the network's
/// capability levels). Every judgment matches on the level, with an arm for each: what is
/// kept of an entry on the level of the group its author saw, what applies on the level of
/// the group in the common order. A new level fails to compile until each says what it does
/// there; the tests pin what level two keeps and applies of a corpus of entries, and
/// compare it with the level before.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// The first level with a number of its own in the log: the judgment of the version
    /// that brought levels. A group that no `Level` entry raised judges at it.
    #[default]
    Two,
    /// Level two, and (see the architecture document, 5.1 and 5.3):
    /// - an invite names, beside its issuer, the managers who may let its device in
    ///   ([`NamedInvite`]), and only they do; an invite that names nobody
    ///   ([`Body::Invite`]) applies no more;
    /// - a `Rotate` gives the new key to every member its author saw, but for keys nothing
    ///   can be sealed to: one that leaves some out is not kept.
    Three,
}

impl Level {
    /// The highest level this version judges at, and the one the groups it creates start
    /// at.
    pub const CURRENT: Level = Level::Three;

    /// The number this level has in a `Level` entry and among the network's capability
    /// levels: a level of judgment comes with the capability level of its number, so a
    /// device that lists that capability level or a later one judges at it.
    pub const fn number(self) -> u32 {
        match self {
            Level::Two => 2,
            Level::Three => 3,
        }
    }

    /// The level numbered `number`, if this version judges at it.
    pub fn numbered(number: u32) -> Option<Level> {
        let level = match number {
            2 => Level::Two,
            3 => Level::Three,
            _ => return None,
        };
        (level <= Level::highest()).then_some(level)
    }

    /// The highest level judged at: [`Self::CURRENT`]; in the tests that pin how an
    /// earlier level judges, that level, as a copy of the version that brought it.
    pub(crate) fn highest() -> Level {
        #[cfg(test)]
        {
            HIGHEST.with(std::cell::Cell::get)
        }
        #[cfg(not(test))]
        {
            Level::CURRENT
        }
    }

    /// The limits judging at this level reads. Judging reads them as the constants they are
    /// at level two, and level three keeps them; a level with other limits has judging
    /// read them from here instead.
    pub fn caps(self) -> Caps {
        match self {
            Level::Two | Level::Three => Caps {
                members: MAX_MEMBERS,
                in_room: MAX_IN_ROOM,
                rooms: MAX_ROOMS,
                room_name_chars: MAX_ROOM_NAME_CHARS,
                group_name_chars: MAX_GROUP_NAME_CHARS,
                member_name_chars: MAX_MEMBER_NAME_CHARS,
                picture_bytes: MAX_PICTURE_BYTES,
                entry_bytes: crate::entry::MAX_ENTRY_BYTES,
                key_package_bytes: crate::entry::MAX_KEY_PACKAGE_BYTES,
                parents: crate::log::MAX_PARENTS,
                run_entries: crate::budget::MAX_RUN_ENTRIES,
                run_bytes: crate::budget::MAX_RUN_BYTES,
                update_gap_ms: crate::chain::MIN_UPDATE_GAP_MS,
            },
        }
    }
}

#[cfg(test)]
thread_local! {
    /// See [`Level::highest`]: set by a test, for the thread it runs on.
    pub(crate) static HIGHEST: std::cell::Cell<Level> = const { std::cell::Cell::new(Level::CURRENT) };
}

/// The limits a level judges by (see [`Level::caps`]): a change to any of them changes what
/// a log keeps or applies, so it comes as a new level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub members: u8,
    pub in_room: u8,
    pub rooms: usize,
    pub room_name_chars: usize,
    pub group_name_chars: usize,
    pub member_name_chars: usize,
    pub picture_bytes: usize,
    pub entry_bytes: usize,
    pub key_package_bytes: usize,
    pub parents: usize,
    pub run_entries: u32,
    pub run_bytes: usize,
    pub update_gap_ms: u64,
}

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
/// managed then, and the entry that ended it. A room rule it signed before that entry (one
/// of its ancestors) still counts with those rights.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Loss {
    entry: EntryHash,
    ladder: Vec<DevicePublicKey>,
    managers: Managers,
}

/// Where a room rule stands from the losses of its author in the graph of entries. A loss
/// before the rule its author signed it knowing; one after it, its author still had what it
/// lost when it signed it; and one neither before nor after it, the rule was signed apart
/// from, maybe on old entries on purpose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RuleStand {
    /// Whether a loss of its author is neither before nor after it.
    pub(crate) apart: bool,
    /// The first loss of its author after it, as the place of that loss among the author's
    /// losses in the order they applied ([`GroupState::losses_of`]).
    pub(crate) first_after: Option<usize>,
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
    /// The level the group is judged at from here on (see [`Level`]).
    pub level: Level,
    /// Valid epochs in the order they started; the last one is current.
    pub epochs: Vec<Epoch>,
    pub used_tickets: BTreeSet<[u8; 16]>,
    /// Invites not used yet, by ticket id: their token and when they expire.
    pub open_invites: BTreeMap<[u8; 16], (InviteToken, u64)>,
    /// The issuer of each invite written as a [`NamedInvite`] and the managers it names, by
    /// ticket id, until it is used or cancelled: only they let its device in.
    welcomers: BTreeMap<[u8; 16], Vec<DevicePublicKey>>,
    /// Entries that applied, in order.
    pub applied: Vec<EntryHash>,
    /// Entries that were ignored, with the reason.
    pub rejected: Vec<(EntryHash, &'static str)>,
    /// Who may enter the voice rooms.
    pub rules: RoomRules,
    /// What each device had before each time it was demoted, removed or left, in the order
    /// of the log (it may be back since).
    losses: BTreeMap<DevicePublicKey, Vec<Loss>>,
    /// Who was given each epoch's secret by an entry that applied.
    holders: BTreeMap<EpochId, BTreeSet<DevicePublicKey>>,
    /// Who was sent each epoch's secret in an entry kept that did not apply (see
    /// [`Self::refuse_kept`]), each with whether it was no member then.
    exposed: BTreeMap<EpochId, BTreeMap<DevicePublicKey, bool>>,
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

    /// Every device given or sent the secret of epoch `id`, whether or not the entry
    /// applied; `judged` leaves out those sent it while members, in an entry that did not
    /// apply, as the judgment of a `Rotate` does (see [`Self::leaked_as_judged`]).
    fn reached(&self, id: &EpochId, judged: bool) -> BTreeSet<DevicePublicKey> {
        let given = self.holders.get(id).into_iter().flatten().copied();
        let sent = self.exposed.get(id).into_iter().flatten();
        let sent = sent
            .filter(|(_, outside)| !judged || **outside)
            .map(|(k, _)| *k);
        given.chain(sent).collect()
    }

    /// Whether every device given or sent the secret of the epoch numbered `number` is
    /// still a member: a proof made with its key then comes from a member, whichever epoch
    /// it is. False for an epoch this state does not have.
    pub fn epoch_kept(&self, number: u32) -> bool {
        let mut epochs = self
            .epochs
            .iter()
            .filter(|e| epoch_number(&e.id) == number)
            .peekable();
        epochs.peek().is_some()
            && epochs.all(|e| {
                self.holders.contains_key(&e.id)
                    && self.reached(&e.id, false).iter().all(|k| self.is_member(k))
            })
    }

    /// Whether `key` was given or sent the secret of any epoch, that is, ever had a group key.
    pub fn ever_held(&self, key: &DevicePublicKey) -> bool {
        self.holders.values().any(|h| h.contains(key))
            || self.exposed.values().any(|e| e.contains_key(key))
    }

    /// Devices that hold the current epoch's secret without being members: someone left
    /// or was removed, a rotation was discarded, or an entry that sent it did not apply
    /// (see [`Self::refuse_kept`]). The members should start a new epoch.
    pub fn current_secret_leaked_to(&self) -> Vec<DevicePublicKey> {
        self.leaked(false)
    }

    /// [`Self::current_secret_leaked_to`] as the judgment of a `Rotate` by a member that
    /// does not manage counts it: an entry that did not apply counts only for a device it
    /// was sent to while no member.
    pub(crate) fn leaked_as_judged(&self) -> Vec<DevicePublicKey> {
        self.leaked(true)
    }

    /// Devices given the current epoch's secret by an entry that applied that are no
    /// longer members (someone left or was removed, a rotation was discarded): a member
    /// that does not manage may rotate for those in versions before entries that did not
    /// apply counted, so it rotates only for them; a leak through an entry that did not
    /// apply is mended by a manager.
    pub fn current_secret_left_with(&self) -> Vec<DevicePublicKey> {
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

    fn leaked(&self, judged: bool) -> Vec<DevicePublicKey> {
        let Some(current) = self.current_epoch() else {
            return Vec::new();
        };
        self.reached(&current.id, judged)
            .into_iter()
            .filter(|k| !self.is_member(k))
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

    /// Notes what `device` had before entry `entry` demotes it, removes it or has it leave.
    fn note_loss(&mut self, device: &DevicePublicKey, entry: EntryHash) {
        let loss = Loss {
            entry,
            ladder: self.ladder.clone(),
            managers: self.managers,
        };
        self.losses.entry(*device).or_default().push(loss);
    }

    /// The entries that demoted, removed or had leave `device`, in the order they applied.
    pub(crate) fn losses_of(&self, device: &DevicePublicKey) -> impl Iterator<Item = &EntryHash> {
        self.losses
            .get(device)
            .into_iter()
            .flatten()
            .map(|l| &l.entry)
    }

    /// How many times `device` was demoted, removed or left.
    pub(crate) fn loss_count(&self, device: &DevicePublicKey) -> usize {
        self.losses.get(device).map_or(0, Vec::len)
    }

    /// The entries that demoted, removed or had leave a member that managed the group just
    /// before, with that member.
    pub(crate) fn manager_losses(&self) -> impl Iterator<Item = (&DevicePublicKey, &EntryHash)> {
        self.losses.iter().flat_map(|(device, losses)| {
            losses
                .iter()
                .filter(move |l| {
                    l.ladder
                        .iter()
                        .position(|k| k == device)
                        .is_some_and(|rank| l.managers.include(rank))
                })
                .map(move |l| (device, &l.entry))
        })
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
        #[cfg(any(test, feature = "counts"))]
        crate::work::applied();
        match self.check_and_apply(hash, entry, rotation_needed, before) {
            Ok(Applied::Yes) => self.applied.push(hash),
            Ok(Applied::Waits) => {}
            Ok(Applied::LetIn(join)) => {
                self.applied.push(join);
                self.applied.push(hash);
            }
            Err(reason) => self.refuse_kept(hash, entry, reason),
        }
    }

    /// Notes why `entry`, which the log keeps, does not apply here (a change that lost a
    /// race, or one a loss of its author outranks). Its author could make it, so its boxes
    /// went to their devices all the same: a device sent the secret of an epoch this group
    /// has in an `Add` or `Welcome` (two managers that let in two devices with one invite, a
    /// no and a yes at once, a full group, a manager moved down that welcomed a member)
    /// counts as holding it once it is no member, so the members start a new epoch if that
    /// is the current one (see [`Self::current_secret_leaked_to`]), and proofs under it say
    /// nothing more ([`Self::epoch_kept`]). It is not counted as given the key: a member's
    /// box may open to nothing, and it would never be given one again.
    pub(crate) fn refuse_kept(&mut self, hash: EntryHash, entry: &Entry, reason: &'static str) {
        if let Body::Add { member, epoch, .. } | Body::Welcome { member, epoch, .. } = &entry.body
            && self.epoch(epoch).is_some()
        {
            let outside = !self.is_member(member);
            let was = self.exposed.entry(*epoch).or_default().entry(*member);
            *was.or_default() |= outside;
        }
        self.rejected.push((hash, reason));
    }

    /// Applies a room rule, once every other entry applied: rules are checked against the
    /// group as it is, the same on every member whatever order the rules arrived in. An
    /// author that was demoted, removed or left since still counts, for the rules it signed
    /// before (that the entry ending its rights is signed on, directly or not), with the place
    /// it had then. A rule signed apart from such an entry is outranked by it, as any other
    /// entry needing a place: its author could otherwise sign one on entries from before it
    /// lost its place, dated as it likes. `stand` tells where the rule stands from the
    /// author's losses.
    pub(crate) fn apply_rule(&mut self, hash: EntryHash, entry: &Entry, stand: RuleStand) {
        match self.check_rule(entry, stand) {
            Ok(()) => self.applied.push(hash),
            Err(reason) => self.rejected.push((hash, reason)),
        }
    }

    /// Whether room rule `entry` applies here, at the level the group judges at (see
    /// [`Level`]), and what it changes if so.
    fn check_rule(&mut self, entry: &Entry, stand: RuleStand) -> Result<(), &'static str> {
        match self.level {
            Level::Two | Level::Three => self.check_rule_at_two(entry, stand),
        }
    }

    /// [`Self::check_rule`] as [`Level::Two`] judges it.
    fn check_rule_at_two(&mut self, entry: &Entry, stand: RuleStand) -> Result<(), &'static str> {
        let author = entry.author;
        if stand.apart {
            return Err(OUTRANKED);
        }
        // The ladder and managers the author's rights are judged on: those it had before its
        // first loss after the rule, or those of now.
        let lost_later = stand
            .first_after
            .and_then(|i| self.losses.get(&author)?.get(i));
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
            // Every loss here is before it: it is the last entry.
            trial.check_rule(entry, RuleStand::default())
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

    /// Whether `rotation` gives its key to every member here, but those whose key nothing
    /// can be sealed to ([`DevicePublicKey::is_weak`]): what a `Rotate` written here holds
    /// from level three.
    pub(crate) fn rotation_for_all(&self, rotation: &Rotation) -> bool {
        self.members
            .keys()
            .filter(|k| !k.is_weak())
            .all(|k| rotation.sealed.iter().any(|(to, _)| to == k))
    }

    /// A copy to try one more entry on, as [`Self::apply`] would apply it: everything but the
    /// lists of the entries that applied and were refused, which only grow, and the
    /// picture, neither of which applying reads.
    pub(crate) fn to_try_on(&self) -> Self {
        #[cfg(any(test, feature = "counts"))]
        crate::work::copied();
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
            level: self.level,
            epochs: self.epochs.clone(),
            used_tickets: self.used_tickets.clone(),
            open_invites: self.open_invites.clone(),
            welcomers: self.welcomers.clone(),
            applied: Vec::new(),
            rejected: Vec::new(),
            rules: self.rules.clone(),
            losses: self.losses.clone(),
            holders: self.holders.clone(),
            exposed: self.exposed.clone(),
            keyed: self.keyed.clone(),
            waiting_joins: self.waiting_joins.clone(),
        }
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
            level: self.level,
            epochs: self.epochs.clone(),
            used_tickets: self.used_tickets.clone(),
            open_invites: BTreeMap::new(),
            welcomers: self.welcomers.clone(),
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
            exposed: if with_holders {
                self.exposed.clone()
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

    /// Whether `entry` applies here, at the level the group judges at (see [`Level`]),
    /// and what it changes if so.
    fn check_and_apply(
        &mut self,
        hash: EntryHash,
        entry: &Entry,
        rotation_needed: Option<bool>,
        before: &dyn Fn(&EntryHash) -> bool,
    ) -> Result<Applied, &'static str> {
        match self.level {
            Level::Two => self.check_and_apply_at_two(hash, entry, rotation_needed, before),
            Level::Three => self.check_and_apply_at_three(hash, entry, rotation_needed, before),
        }
    }

    /// [`Self::check_and_apply`] as [`Level::Three`] judges it: as level two, but for
    /// invites, which name who lets their device in, and the `Welcome`s that let a device in
    /// with one. (A `Rotate` that leaves out a member its author saw is not kept at all: see
    /// the log's judgment.)
    fn check_and_apply_at_three(
        &mut self,
        hash: EntryHash,
        entry: &Entry,
        rotation_needed: Option<bool>,
        before: &dyn Fn(&EntryHash) -> bool,
    ) -> Result<Applied, &'static str> {
        if self.creator.is_some() {
            match &entry.body {
                Body::Invite { .. } => return Err(INVITE_NAMES_NOBODY),
                body @ Body::Unknown { .. } => {
                    if let Some(invite) = NamedInvite::of(body) {
                        return self
                            .apply_named_invite(entry, invite)
                            .map(|()| Applied::Yes);
                    }
                }
                Body::Welcome { member, .. } if self.is_manager(&entry.author) => {
                    // The join it would let in, as level two picks it: its device may have
                    // been invited by someone who named who lets it in.
                    if let Some(join) = self.join_to_let_in(entry, member, before)
                        && !self.may_let_in(&entry.author, &join.ticket)
                    {
                        return Err(WELCOME_NOT_NAMED);
                    }
                }
                _ => {}
            }
        }
        self.check_and_apply_at_two(hash, entry, rotation_needed, before)
    }

    /// Notes a [`NamedInvite`] as level two notes a [`Body::Invite`], with the managers it
    /// names.
    fn apply_named_invite(
        &mut self,
        entry: &Entry,
        invite: NamedInvite,
    ) -> Result<(), &'static str> {
        self.require_manager(&entry.author)?;
        let ticket = &invite.ticket;
        if ticket.issuer != entry.author
            || Some(ticket.group_id) != self.group_id
            || !ticket.signature_valid()
        {
            return Err("invalid ticket");
        }
        if !self.used_tickets.contains(&ticket.ticket_id) {
            self.open_invites
                .insert(ticket.ticket_id, (invite.token, ticket.expires_ms));
            let mut named = invite.welcomers;
            named.push(ticket.issuer);
            self.welcomers.insert(ticket.ticket_id, named);
        }
        Ok(())
    }

    /// Whether `device` may let in a device that joins with `ticket`: anyone who manages
    /// the group, unless the ticket's invite was written as a [`NamedInvite`] (from level
    /// three), which leaves that to its issuer and the managers it names.
    pub fn may_let_in(&self, device: &DevicePublicKey, ticket: &Ticket) -> bool {
        self.welcomers
            .get(&ticket.ticket_id)
            .is_none_or(|named| named.contains(device))
    }

    /// Tokens of the invites still open at `now_ms` that `device` may let their devices in
    /// with ([`Self::may_let_in`]): where it answers them.
    pub fn invite_tokens_for(&self, device: &DevicePublicKey, now_ms: u64) -> Vec<InviteToken> {
        self.open_invites
            .iter()
            .filter(|(_, (_, expires))| now_ms <= *expires)
            .filter(|(id, _)| {
                self.welcomers
                    .get(*id)
                    .is_none_or(|named| named.contains(device))
            })
            .map(|(_, (token, _))| *token)
            .collect()
    }

    /// [`Self::check_and_apply`] as [`Level::Two`] judges it.
    fn check_and_apply_at_two(
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
                self.note_loss(member, hash);
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
                // Not checked against the join's time, which its author chooses: the
                // manager that lets it in checks the ticket against its own clock.
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
                    let join = self
                        .join_to_let_in(entry, member, before)
                        .ok_or(NO_JOIN_TO_WELCOME)?;
                    self.require_room_for_one_more()?;
                    self.used_tickets.insert(join.ticket.ticket_id);
                    self.open_invites.remove(&join.ticket.ticket_id);
                    self.welcomers.remove(&join.ticket.ticket_id);
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
                self.note_loss(member, hash);
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
            Body::Level { level } => {
                self.require_manager(&author)?;
                let level = Level::numbered(*level).ok_or(LEVEL_UNKNOWN)?;
                if level <= self.level {
                    return Err(LEVEL_NOT_HIGHER);
                }
                self.level = level;
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
                self.welcomers.remove(ticket_id);
                self.used_tickets.insert(*ticket_id);
            }
            Body::Leave => {
                self.require_member(&author)?;
                self.note_loss(&author, hash);
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

    /// The `Join` of `member`, no member, that `Welcome` entry `welcome` lets in: the one it
    /// is signed on directly, the one its manager saw; else the first before it in the order.
    fn join_to_let_in(
        &self,
        welcome: &Entry,
        member: &DevicePublicKey,
        before: &dyn Fn(&EntryHash) -> bool,
    ) -> Option<WaitingJoin> {
        if self.is_member(member) {
            return None;
        }
        let usable = |j: &&WaitingJoin| self.ticket_usable(&j.ticket);
        let joins = || self.waiting_joins.get(member).into_iter().flatten();
        joins()
            .filter(usable)
            .find(|j| welcome.parents.contains(&j.hash))
            .or_else(|| joins().filter(usable).find(|j| before(&j.hash)))
            .cloned()
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
