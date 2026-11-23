//! How much one device may make every member keep before another member has seen it.
//!
//! Every member keeps every entry it takes, so what a device writes is limited by what
//! the other members do. A device's run is the entries it wrote that no entry of another
//! member is signed on yet, directly or through others; an entry of another member takes
//! out of it every entry it is signed on, and no other, so an entry written long before
//! takes out nothing written since. A run holds at most [`MAX_RUN_ENTRIES`] entries that
//! count, [`MAX_RUN_BYTES`] of them: those of every kind but the group's `Create`, the
//! kinds that never apply here and `Join`s. MLS Commits and Resyncs count like the rest,
//! whether or not they apply.
//!
//! Bringing a device in and taking it out count in the run of the member that brought it
//! in: the author of its `Add`, or the manager whose `Welcome` let it in with its `Join`. A
//! join is part of no run (anyone holding a ticket can sign one, under any key): the first
//! entry signed on it brings it in and counts it, and, when that is a `Welcome` naming its
//! device, counts as the device's coming in. And its removal or leave counts there too, as the common order counts who came in
//! when, without judging rights. A removal or leave is never held back by a
//! run, so that nobody's keeps anyone in: it may fill one past its room, and that member
//! brings nobody else in until its run has room again. Such an entry leaves a run only
//! through an entry of a device that came in before it: a device never takes out the
//! entries that brought it in, nor those of devices brought in after it, so a member alone
//! cannot bring devices of its own in and out without end.
//!
//! Entries that are part of no run (those past one, those of a kind that never applies
//! here, and joins nobody let in) are spare: nothing is signed on them, and a device lets them go. An entry signed on
//! some anyway, directly or through others like them, makes every member keep them: they
//! count, entries and bytes, with it in the run it counts in, those that no entry of a run
//! placed before it had brought in, and it is past that run if they do not fit, a removal
//! or leave too; honest devices never sign on them.
//!
//! Whether an entry is past its author's run is worked out from the common order alone, as
//! each entry is placed, never from what applied, so the rounds of the losses change
//! nothing of it; and it is worked out the same on the entries before it, as its author saw
//! them, to judge it ([`crate::GroupLog::insert`]). An entry past its run applies to
//! nothing, takes no part in who came in when nor in anyone's run, and nothing is signed on
//! it, so a device lets it go like any spare entry. Nor does one signed on an entry of its
//! own author past its run count: a device cannot make the others keep entries past its run
//! by signing on them.
//!
//! From level five, runs alone no longer bound a log: two devices that sign on each other's
//! entries end each other's runs, and could write without end. So the whole log has a
//! ceiling too, [`CEILING_ENTRIES`] entries that count, [`CEILING_BYTES`] of them, all
//! authors together, counted as runs count them, in the common order. It holds every entry
//! of a log ordered at level five (one where a `Level` entry kept names it), wherever it
//! goes in the order, as the order itself is worked out at that level for every entry: an
//! entry that would pass it is past it as one past its run is. A log under the ceiling when
//! it is raised (every honest group for years) keeps and applies the same; one past it keeps
//! what fits, in the order, and what does not applies to nothing.
//!
//! What gets a full group out goes a little past it: a removal or leave of a device that
//! is in, and what it brings in (the entries part of no run it is signed on), up to
//! [`SLACK`] past the ceiling, so that a removal written while the log filled up, on an
//! entry of its author that the order then put past the ceiling, still applies; and the
//! first `Level` entry in the order to name a level, and the first [`Moved`] (one its
//! author wrote where it saw the group at five, as its first member, which is part of its
//! run, so that every copy keeps it and signs on it; any other is a kind its author's level
//! did not know), up to [`RESERVE`] past that, for themselves alone: whatever filled the slack, the
//! group can still be raised and moved. Later `Level` entries naming no higher level are
//! held like the rest, so that a manager cannot fill the log with them. A log at five
//! therefore never holds more than [`CEILING_ENTRIES`], [`SLACK`] and [`RESERVE`] together:
//! under what every release takes from a friend.

use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::{BuildHasherDefault, Hasher},
};

use fogata_crypto::DevicePublicKey;

use crate::{
    entry::{Body, Entry, EntryHash, Moved, NamedInvite},
    log::GroupLog,
    state::Level,
};

/// Most entries that count a device writes in one run: as many as a node takes from one
/// device at once (3.6). A device that alone writes in its group, rotating its key every
/// week, reaches it after 78 years.
pub const MAX_RUN_ENTRIES: u32 = 4096;
/// Most bytes of the entries that count a device writes in one run: as many as a node takes
/// from one device at once. A device that alone writes in a group of 20 and rotates its key
/// every week (3 KB) reaches it after 26 years.
pub const MAX_RUN_BYTES: usize = 4 << 20;

/// Why an entry past its author's run is refused, or applies to nothing.
pub const PAST_RUN: &str = "its author wrote too much that no other member signed on yet";

/// Most entries that count a log holds from level five, all authors together (see the
/// module): 512 under the most a friend's offer may hold in every release
/// ([`MAX_RUN_ENTRIES`]), for the [`SLACK`] and [`RESERVE`] past it. An
/// honest group of 20 that rotates its key every week and changes a device a week reaches
/// it after about 10 years.
pub const CEILING_ENTRIES: u32 = 3584;
/// Most bytes of the entries that count a log holds from level five: half a MiB under the
/// most a friend's offer may hold in every release ([`MAX_RUN_BYTES`]).
pub const CEILING_BYTES: usize = 3584 << 10;

/// Why an entry past the ceiling of its log is refused, or applies to nothing.
pub const PAST_CEILING: &str = "the group's log is full: it moves to a new one";

/// How far past the ceiling a removal or leave goes, with what it brings in (see the
/// module).
pub(crate) const SLACK: RunRoom = RunRoom {
    entries: 256,
    bytes: 256 << 10,
};

/// How far past [`SLACK`] the first raise to a level and a move go, for themselves alone
/// (see the module): a few small entries. The ceiling, the slack and this leave 192 entries
/// and 192 KiB under what a friend's offer holds.
pub(crate) const RESERVE: RunRoom = RunRoom {
    entries: 64,
    bytes: 64 << 10,
};

/// How many entries that count, and bytes of them: what a device writes in a run at most,
/// or what one entry adds to its run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RunRoom {
    pub(crate) entries: u32,
    pub(crate) bytes: usize,
}

pub(crate) const RUN_ROOM: RunRoom = RunRoom {
    entries: MAX_RUN_ENTRIES,
    bytes: MAX_RUN_BYTES,
};

/// The ceiling of a log from level five ([`CEILING_ENTRIES`], [`CEILING_BYTES`]).
pub(crate) const CEILING: RunRoom = RunRoom {
    entries: CEILING_ENTRIES,
    bytes: CEILING_BYTES,
};

/// What an entry that adds nothing to its run adds.
const NOTHING: RunRoom = RunRoom {
    entries: 0,
    bytes: 0,
};

impl Default for RunRoom {
    fn default() -> Self {
        RUN_ROOM
    }
}

/// Hashes an entry's hash by its first bytes: a SHA-256 digest is spread evenly already,
/// and nobody can choose it.
#[derive(Default)]
pub(crate) struct ByDigest(u64);

impl Hasher for ByDigest {
    fn write(&mut self, bytes: &[u8]) {
        let mut first = [0; 8];
        let n = bytes.len().min(8);
        first[..n].copy_from_slice(&bytes[..n]);
        self.0 = self.0.rotate_left(17) ^ u64::from_le_bytes(first);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// A set of entries by hash.
pub(crate) type Entries = HashSet<EntryHash, BuildHasherDefault<ByDigest>>;

/// What an entry is for its author's run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// The root: never past a run, and part of it.
    Exempt,
    /// A kind that never applies here (one this version does not know, and the MLS kinds
    /// other than Commits and Resyncs), and a `Join`, which applies only with the `Welcome`
    /// of a manager signed on it: never signed on otherwise, so let go of like any spare
    /// entry; part of no run, and counted in the run of an entry signed on it (see the
    /// module).
    Inert,
    /// A removal of a device that is in, a leave of a device that is in: counted in the run
    /// of the device that brought it in, if any, and never past a run, so that nobody's run
    /// keeps anyone in.
    Out,
    /// Everything else.
    Counted,
}

/// Where an entry counts: the run it is in, and whether it brings a device in or takes one
/// out (see the module).
#[derive(Clone, Copy, Debug)]
struct Payer {
    device: DevicePublicKey,
    /// For a device brought in or taken out: how many devices had come in before it. Only
    /// an entry of one of those takes it out of its run.
    churn: Option<u64>,
}

/// The run of one device: its entries that no entry of another member is signed on yet,
/// directly or through others.
#[derive(Clone, Debug, Default)]
struct Run {
    /// Each with what it adds to the run (itself if it counts, and the entries part of no
    /// run it brought in), for those that add something; those that add nothing are there
    /// too, since an entry signed on one of them is signed on what is before it.
    entries: HashMap<EntryHash, Option<RunRoom>, BuildHasherDefault<ByDigest>>,
    counted: u32,
    bytes: usize,
}

/// When each device came in, as the common order goes (see [`GroupLog::ordered`]); and each
/// device's run (see the module).
#[derive(Clone, Default)]
pub(crate) struct Budget {
    room: RunRoom,
    /// The level the order is worked out at: from level three, a [`NamedInvite`] is part of
    /// its author's run, as the invite it is, where level two knows no such kind.
    level: Level,
    came_in: BTreeMap<DevicePublicKey, u64>,
    /// Who brought each device in, the last time it came in: the author of its `Add`, or
    /// the issuer of the ticket it joined with; none for the creator.
    brought_in_by: BTreeMap<DevicePublicKey, DevicePublicKey>,
    inside: BTreeSet<DevicePublicKey>,
    count: u64,
    runs: HashMap<DevicePublicKey, Run>,
    /// Entries of the order that are part of a run: all but those past a run and those of
    /// a kind that never applies. New entries are signed on them, applied or not.
    pub(crate) live: Entries,
    /// Entries past their author's run, or past the ceiling.
    pub(crate) over: Entries,
    /// Those of them past the ceiling (see the module).
    past_ceiling: Entries,
    /// Entries part of no run that an entry of a run is signed on, directly or through
    /// others like them: they count in the run of the first such entry in the order.
    brought: Entries,
    /// Entries of a run that count in another run than their author's, or bring a device in
    /// or take one out (see the module).
    payers: HashMap<EntryHash, Payer, BuildHasherDefault<ByDigest>>,
    /// The most the log holds from level five (see the module).
    ceiling: RunRoom,
    /// What the entries of a run placed so far count, all together, entries and bytes:
    /// never less, since every member keeps them.
    total_entries: u32,
    total_bytes: usize,
    /// The highest level a `Level` entry of a run placed so far names: only one naming a
    /// higher one gets past the ceiling.
    highest_named: u32,
    /// Whether a move part of a run was placed already: only the first goes into the
    /// reserve.
    moved: bool,
}

impl std::fmt::Debug for Budget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Budget")
            .field("live", &self.live.len())
            .field("over", &self.over.len())
            .field("total", &(self.total_entries, self.total_bytes))
            .finish_non_exhaustive()
    }
}

impl Budget {
    pub(crate) fn new(room: RunRoom, ceiling: RunRoom, level: Level) -> Self {
        Self {
            room,
            level,
            ceiling,
            ..Self::default()
        }
    }

    /// The level the order is worked out at.
    pub(crate) fn level(&self) -> Level {
        self.level
    }

    /// Why entry `hash`, one of [`Self::over`], applies to nothing.
    pub(crate) fn why_over(&self, hash: &EntryHash) -> &'static str {
        if self.past_ceiling.contains(hash) {
            PAST_CEILING
        } else {
            PAST_RUN
        }
    }

    /// What the entries that count in the log hold, all together, and the most they may
    /// from level five.
    pub(crate) fn fill(&self) -> (RunRoom, RunRoom) {
        let total = RunRoom {
            entries: self.total_entries,
            bytes: self.total_bytes,
        };
        (total, self.ceiling)
    }

    /// Whether `entry` is one that gets a full group out, which goes past the ceiling (see
    /// the module): a removal or leave of a device that is in, or the first `Level` entry
    /// to name a level.
    fn gets_out(&self, entry: &Entry, kind: Kind) -> bool {
        match entry.body {
            Body::Level { level } => level > self.highest_named,
            _ => kind == Kind::Out,
        }
    }

    fn kind(&self, log: &GroupLog, entry: &Entry) -> Kind {
        match &entry.body {
            Body::Create { .. } => Kind::Exempt,
            body @ Body::Unknown { .. }
                if self.level >= Level::Three && NamedInvite::of(body).is_some() =>
            {
                Kind::Counted
            }
            // A move is one where its author saw the group at five, and could move it
            // there: any other `Moved` is a kind its author's level did not know.
            body @ Body::Unknown { .. }
                if self.level >= Level::Five
                    && Moved::of(body).is_some()
                    && log.moves(&entry.hash()) =>
            {
                Kind::Counted
            }
            Body::Unknown { .. }
            | Body::MlsKeyPackage { .. }
            | Body::MlsJoin { .. }
            | Body::Join { .. } => Kind::Inert,
            Body::Remove { member, .. } if self.inside.contains(member) => Kind::Out,
            Body::Leave if self.inside.contains(&entry.author) => Kind::Out,
            _ => Kind::Counted,
        }
    }

    /// The device a removal or leave takes out.
    fn going(entry: &Entry) -> DevicePublicKey {
        match &entry.body {
            Body::Remove { member, .. } => *member,
            _ => entry.author,
        }
    }

    /// The run `entry` counts in (see the module).
    fn payer_of(&self, log: &GroupLog, entry: &Entry) -> Payer {
        let churn = Some(self.count);
        match (&entry.body, self.kind(log, entry)) {
            (Body::Add { .. }, _) => Payer {
                device: entry.author,
                churn,
            },
            (_, Kind::Out) => match self.brought_in_by.get(&Self::going(entry)) {
                Some(by) => Payer { device: *by, churn },
                None => Payer {
                    device: entry.author,
                    churn: None,
                },
            },
            _ => Payer {
                device: entry.author,
                churn: None,
            },
        }
    }

    /// The run entry `hash` of `log`, placed already, counts in.
    fn payer(&self, log: &GroupLog, hash: &EntryHash) -> Payer {
        self.payers.get(hash).copied().unwrap_or(Payer {
            device: log.entries[hash].author,
            churn: None,
        })
    }
    /// What `entry`, `bytes` long, placed next, adds to the run it counts in, and the
    /// entries part of no run it brings in (see the module); why not if that is past the
    /// run, or past the log's ceiling. `log` holds its parents.
    fn charge(
        &self,
        log: &GroupLog,
        entry: &Entry,
        bytes: usize,
    ) -> Result<(RunRoom, Vec<EntryHash>), &'static str> {
        let kind = self.kind(log, entry);
        if matches!(kind, Kind::Exempt | Kind::Inert) {
            return Ok((NOTHING, Vec::new()));
        }
        let at_five = self.level >= Level::Five;
        let gets_out = at_five && self.gets_out(entry, kind);
        // One signed on an entry of its own author past its run is past it too; one signed
        // on one past the ceiling is past the ceiling, but for an entry that gets the group
        // out, which brings that one in (see the module).
        for p in &entry.parents {
            if self.over.contains(p) && log.entries.get(p).is_some_and(|e| e.author == entry.author)
            {
                if !self.past_ceiling.contains(p) {
                    return Err(PAST_RUN);
                }
                if !gets_out {
                    return Err(PAST_CEILING);
                }
            }
        }
        let (cost, brings) = self.charge_run(log, entry, kind, bytes).ok_or(PAST_RUN)?;
        if at_five {
            let past = |more: RunRoom| {
                self.total_entries.saturating_add(cost.entries)
                    > self.ceiling.entries.saturating_add(more.entries)
                    || self.total_bytes.saturating_add(cost.bytes)
                        > self.ceiling.bytes.saturating_add(more.bytes)
            };
            // A raise or a move goes into the reserve alone: what it brings in, only into
            // the slack.
            let slack_and_reserve = RunRoom {
                entries: SLACK.entries + RESERVE.entries,
                bytes: SLACK.bytes + RESERVE.bytes,
            };
            let first_move = Moved::of(&entry.body).is_some() && !self.moved;
            let held = if first_move || (gets_out && kind != Kind::Out) {
                past(slack_and_reserve) || (!brings.is_empty() && past(SLACK))
            } else if gets_out {
                past(SLACK)
            } else {
                past(NOTHING)
            };
            if held {
                return Err(PAST_CEILING);
            }
        }
        Ok((cost, brings))
    }

    /// [`Self::charge`] for the run alone: `None` if past it.
    fn charge_run(
        &self,
        log: &GroupLog,
        entry: &Entry,
        kind: Kind,
        bytes: usize,
    ) -> Option<(RunRoom, Vec<EntryHash>)> {
        let payer = self.payer_of(log, entry).device;
        let mut left = self.runs.get(&payer).map_or(self.room, |run| RunRoom {
            entries: self.room.entries.saturating_sub(run.counted),
            bytes: self.room.bytes.saturating_sub(run.bytes),
        });
        let mut cost = RunRoom { entries: 1, bytes };
        if kind == Kind::Out {
            // A removal or leave is never held back by a run, so that nobody's keeps anyone
            // in; of a device nobody brought in (the creator), it counts nowhere.
            if !self.brought_in_by.contains_key(&Self::going(entry)) {
                cost = NOTHING;
            }
            left.entries = left.entries.saturating_sub(cost.entries);
            left.bytes = left.bytes.saturating_sub(cost.bytes);
        } else {
            left.entries = left.entries.checked_sub(cost.entries)?;
            left.bytes = left.bytes.checked_sub(cost.bytes)?;
        }
        // The entries part of no run before it, back to those of a run or brought in
        // already, whose own were brought in with them; stopped once past the room.
        let mut brings = Vec::new();
        let mut todo: Vec<EntryHash> = entry.parents.clone();
        let mut seen = Entries::default();
        while let Some(hash) = todo.pop() {
            if self.live.contains(&hash) || self.brought.contains(&hash) || !seen.insert(hash) {
                continue;
            }
            let Some(before) = log.entries.get(&hash) else {
                continue;
            };
            let len = log.encoded_len(&hash);
            left.entries = left.entries.checked_sub(1)?;
            left.bytes = left.bytes.checked_sub(len)?;
            cost.entries += 1;
            cost.bytes += len;
            brings.push(hash);
            todo.extend(before.parents.iter().copied());
        }
        Some((cost, brings))
    }

    /// Whether `entry` may be written next, on top of the entries this was worked out from:
    /// not past its author's run.
    pub(crate) fn allows(&self, log: &GroupLog, entry: &Entry) -> Result<(), &'static str> {
        self.charge(log, entry, entry.encode().len()).map(drop)
    }

    /// Notes entry `hash` of `log`, placed next in the order.
    pub(crate) fn place(&mut self, log: &GroupLog, hash: &EntryHash) {
        let entry = &log.entries[hash];
        let bytes = log.encoded_len(hash);
        let (cost, brings) = match self.charge(log, entry, bytes) {
            Ok(charged) => charged,
            Err(why) => {
                self.over.insert(*hash);
                if why == PAST_CEILING {
                    self.past_ceiling.insert(*hash);
                }
                return;
            }
        };
        let kind = self.kind(log, entry);
        if let Body::Level { level } = entry.body
            && kind != Kind::Inert
        {
            self.highest_named = self.highest_named.max(level);
        }
        if kind == Kind::Counted && Moved::of(&entry.body).is_some() {
            self.moved = true;
        }
        if kind != Kind::Inert {
            let mut payer = self.payer_of(log, entry);
            // A `Welcome` that brings in a join of the device it names lets that device in:
            // it counts as its coming in, in the run of its own payer. Any other entry that
            // brings a join in only pays for it.
            let welcomed = match &entry.body {
                Body::Welcome { member, .. } => Some(*member),
                _ => None,
            };
            let joins: Vec<&Entry> = brings
                .iter()
                .map(|h| &log.entries[h])
                .filter(|e| {
                    matches!(e.body, Body::Join { .. })
                        && welcomed == Some(e.author)
                        && !self.inside.contains(&e.author)
                })
                .collect();
            if !joins.is_empty() {
                payer.churn = Some(self.count);
            }
            for join in joins {
                if self.inside.insert(join.author) {
                    self.brought_in_by.insert(join.author, payer.device);
                    self.came_in.insert(join.author, self.count);
                    self.count += 1;
                }
            }
            self.end_runs_before(log, entry);
            let run = self.runs.entry(payer.device).or_default();
            let adds = cost != NOTHING;
            run.entries.insert(*hash, adds.then_some(cost));
            run.counted += cost.entries;
            run.bytes += cost.bytes;
            self.total_entries = self.total_entries.saturating_add(cost.entries);
            self.total_bytes = self.total_bytes.saturating_add(cost.bytes);
            if payer.device != entry.author || payer.churn.is_some() {
                self.payers.insert(*hash, payer);
            }
            self.brought.extend(brings);
            self.live.insert(*hash);
        }
        self.pass(entry);
    }

    /// Whether `hash`, an entry of `log` placed already, could lead to entries of another
    /// device's run than `author`'s: one of a run itself, or one that is part of none.
    fn leads_to_a_run(&self, log: &GroupLog, hash: &EntryHash, author: &DevicePublicKey) -> bool {
        if !self.live.contains(hash) {
            return true;
        }
        let of = self.payer(log, hash).device;
        of != *author
            && self
                .runs
                .get(&of)
                .is_some_and(|r| r.entries.contains_key(hash))
    }

    /// Takes out of the other devices' runs every entry in them that `entry`, which is
    /// part of a run, is signed on, directly or not: its author saw those; but an entry that
    /// brings a device in or takes one out only if its author came in before it (see the
    /// module). It goes back only through entries of a run and entries that are part of
    /// none: an entry part of a run took out of the others' runs everything before it when
    /// it was placed, and so did, for its own author, every entry before one no longer in
    /// its run.
    fn end_runs_before(&mut self, log: &GroupLog, entry: &Entry) {
        let author = entry.author;
        let came_in = self.came_in.get(&author).copied();
        let mut todo: Vec<EntryHash> = entry
            .parents
            .iter()
            .filter(|p| self.leads_to_a_run(log, p, &author))
            .copied()
            .collect();
        let mut seen = Entries::default();
        while let Some(hash) = todo.pop() {
            if !seen.insert(hash) {
                continue;
            }
            let Some(before) = log.entries.get(&hash) else {
                continue;
            };
            if self.live.contains(&hash) {
                if before.author == author {
                    continue;
                }
                let payer = self.payer(log, &hash);
                // Its own: it stays, and so may what is before it in other runs.
                let kept = payer.device == author
                    || payer
                        .churn
                        .is_some_and(|before| came_in.is_none_or(|c| c >= before));
                let Some(run) = self.runs.get_mut(&payer.device) else {
                    continue;
                };
                if kept {
                    if !run.entries.contains_key(&hash) {
                        continue;
                    }
                } else {
                    match run.entries.remove(&hash) {
                        Some(Some(cost)) => {
                            run.counted -= cost.entries;
                            run.bytes -= cost.bytes;
                        }
                        Some(None) => {}
                        None => continue,
                    }
                }
            }
            // Every entry before one placed is placed.
            todo.extend(before.parents.iter().copied());
        }
    }

    /// Notes who comes in or goes with `entry`, without judging rights (a device not in yet
    /// counts as the newest, one that comes back counts from then).
    fn pass(&mut self, entry: &Entry) {
        let comes_in = match &entry.body {
            Body::Create { .. } => Some(entry.author),
            Body::Add { member, .. } => {
                if !self.inside.contains(member) {
                    self.brought_in_by.insert(*member, entry.author);
                }
                Some(*member)
            }
            Body::Remove { member, .. } => {
                self.inside.remove(member);
                None
            }
            Body::Leave => {
                self.inside.remove(&entry.author);
                None
            }
            _ => None,
        };
        if let Some(device) = comes_in
            && self.inside.insert(device)
        {
            self.came_in.insert(device, self.count);
            self.count += 1;
        }
    }

    /// Whether `device` is in, as the common order counts who came in, for tests.
    #[cfg(test)]
    pub(crate) fn counts_in(&self, device: &DevicePublicKey) -> bool {
        self.inside.contains(device)
    }

    /// How many entries that count, and bytes of them, `device`'s run holds, for tests.
    #[cfg(test)]
    pub(crate) fn run_of(&self, device: &DevicePublicKey) -> RunRoom {
        self.runs.get(device).map_or(NOTHING, |run| RunRoom {
            entries: run.counted,
            bytes: run.bytes,
        })
    }

    /// Where an entry of `author` goes among those ready with it: those of the devices that
    /// came in last first; a device not in yet counts as the newest.
    pub(crate) fn key(
        &self,
        author: &DevicePublicKey,
        hash: EntryHash,
    ) -> (Reverse<u64>, EntryHash) {
        (
            Reverse(self.came_in.get(author).copied().unwrap_or(u64::MAX)),
            hash,
        )
    }
}
