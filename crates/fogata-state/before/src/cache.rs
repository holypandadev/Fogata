//! What a group log works out from its entries: the common order, the group as the losses
//! settle it (room rules left for last), and the state with the room rules on top. Each is kept until the entries
//! change. A new entry signed on everything the log had is carried forward on what was
//! worked out (it goes last and outranks nothing); any other change works the order out
//! again from the first place it can move, and the group from the start, when asked.

use std::{
    cell::OnceCell,
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock, PoisonError},
};

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupSecret};

use crate::{
    budget::{Budget, Entries, PAST_RUN},
    entry::{Body, EntryHash, secret_context},
    log::GroupLog,
    state::GroupState,
};

/// The group as the entries before an entry make it, and the runs they make (see
/// [`Budget`]): what the entry is judged on.
pub(crate) type View = (GroupState, Budget);

/// An order of entries, with the place of each.
#[derive(Clone, Debug, Default)]
pub(crate) struct Order {
    pub(crate) hashes: Vec<EntryHash>,
    pub(crate) place: HashMap<EntryHash, usize>,
}

impl Order {
    fn of(hashes: Vec<EntryHash>) -> Self {
        let place = hashes.iter().enumerate().map(|(i, h)| (*h, i)).collect();
        Self { hashes, place }
    }

    /// Replaces everything from place `from` on with `tail`.
    fn replace_from(&mut self, from: usize, tail: Vec<EntryHash>) {
        for hash in self.hashes.drain(from..) {
            self.place.remove(&hash);
        }
        for hash in tail {
            self.place.insert(hash, self.hashes.len());
            self.hashes.push(hash);
        }
    }

    fn push(&mut self, hash: EntryHash) {
        self.place.insert(hash, self.hashes.len());
        self.hashes.push(hash);
    }
}

/// The common order of every entry ([`GroupLog::ordered`]), and the entries left out of it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Full {
    pub(crate) order: Order,
    /// Entries with a parent not known, or not placed.
    pub(crate) waiting: BTreeSet<EntryHash>,
    /// How many entries in the order are room rules.
    pub(crate) rules: usize,
    /// Entries in the order that no other entry in it names as parent
    /// ([`GroupLog::placed_heads`]): an entry signed on all of them comes after every
    /// entry of the order.
    pub(crate) tips: Vec<EntryHash>,
    /// Who came in when, and each device's run (see [`crate::budget`]), after the order.
    pub(crate) budget: Arc<Budget>,
}

/// The entries of the common order that are not spare ([`GroupLog::spare`]): the live ones
/// (part of a run, see [`Budget::live`]), and every entry before one of them.
#[derive(Debug, Default)]
pub(crate) struct Needed {
    pub(crate) hashes: HashSet<EntryHash>,
    /// Those that no other of them names as parent ([`GroupLog::needed_heads`]).
    pub(crate) tips: Vec<EntryHash>,
}

/// The group without its room rules, once the losses have settled what is outranked.
#[derive(Clone, Debug)]
pub(crate) struct Settled {
    pub(crate) state: Arc<GroupState>,
    /// Entries outranked, each with the loss that outranks it.
    pub(crate) outranked: BTreeMap<EntryHash, EntryHash>,
    /// Whether the rounds reached one that changed nothing (they stop after a round per loss
    /// otherwise): only then does an entry signed on everything apply on top.
    pub(crate) still: bool,
}

/// What a log worked out, each part filled when first asked for. Copies of a log share what
/// was worked out until one of them changes.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cache {
    full: OnceLock<Arc<Full>>,
    settled: OnceLock<Arc<Settled>>,
    state: OnceLock<Arc<GroupState>>,
    /// The live entries of the order that no live entry names as parent, all of them: new
    /// entries are signed on the first few ([`GroupLog::heads`]).
    pub(crate) heads: OnceLock<Vec<EntryHash>>,
    /// The entries of the common order that are not spare (see [`GroupLog::spare`]).
    needed: OnceLock<Arc<Needed>>,
    /// Whether each `Rotate` judged so far was needed in its author's view (see
    /// [`GroupLog::rotation_needed`]). That depends only on the entry and what came before
    /// it, which its hash fixes, so it is kept when the log changes, and shared with copies.
    pub(crate) judged: Arc<Mutex<BTreeMap<EntryHash, bool>>>,
    /// The group as entries signed on each set of parents saw it, and the runs (see
    /// [`GroupLog::insert`]): they depend only on those parents, which their hashes fix.
    pub(crate) views: Arc<Mutex<HashMap<Vec<EntryHash>, Arc<View>>>>,
    /// Boxes this device opened, by entry and device, and what each gave: opening one costs
    /// a key agreement, and a box never changes.
    opened: Opened,
}

impl Cache {
    /// Nothing worked out; what never changes for an entry (rotations judged, boxes opened)
    /// kept.
    pub(crate) fn kept(&self) -> Self {
        Self {
            judged: self.judged.clone(),
            views: self.views.clone(),
            opened: self.opened.clone(),
            ..Self::default()
        }
    }
}

type Opened = Arc<Mutex<HashMap<(EntryHash, DevicePublicKey), Option<Arc<GroupSecret>>>>>;

/// The common order as a graph of places, to walk ancestors and descendants of losses.
struct Graph {
    parents: Vec<Vec<usize>>,
    children: Vec<Vec<usize>>,
}

impl Graph {
    fn of(log: &GroupLog, order: &Order) -> Self {
        let mut parents = vec![Vec::new(); order.hashes.len()];
        let mut children = vec![Vec::new(); order.hashes.len()];
        for (i, hash) in order.hashes.iter().enumerate() {
            for parent in &log.entries[hash].parents {
                // Every parent of an entry in the order is in it.
                if let Some(&p) = order.place.get(parent) {
                    parents[i].push(p);
                    children[p].push(i);
                }
            }
        }
        Self { parents, children }
    }

    /// The places reached from `from` through `next`, never past `bound` (an ancestor comes
    /// before an entry in the order, a descendant after it).
    fn reach(next: &[Vec<usize>], from: usize, within: impl Fn(usize) -> bool) -> Vec<bool> {
        let mut seen = vec![false; next.len()];
        let mut todo = vec![from];
        while let Some(at) = todo.pop() {
            for &other in &next[at] {
                if within(other) && !seen[other] {
                    seen[other] = true;
                    todo.push(other);
                }
            }
        }
        seen
    }
}

impl GroupLog {
    pub(crate) fn full(&self) -> &Full {
        self.cache.full.get_or_init(|| {
            let all: Vec<EntryHash> = self.entries.keys().copied().collect();
            let (hashes, budget) = self.order_after(&[], |_| None, all);
            let order = Order::of(hashes);
            let waiting = self
                .entries
                .keys()
                .filter(|h| !order.place.contains_key(*h))
                .copied()
                .collect();
            let rules = order
                .hashes
                .iter()
                .filter(|h| self.entries[*h].body.is_room_rule())
                .count();
            let tips = self.tips_in(&order.hashes);
            Arc::new(Full {
                order,
                waiting,
                rules,
                tips,
                budget: Arc::new(budget),
            })
        })
    }

    pub(crate) fn settled_cache(&self) -> &Settled {
        self.cache.settled.get_or_init(|| {
            let full = self.full();
            Arc::new(self.settle(&full.order, &full.budget.over))
        })
    }

    pub(crate) fn state_cache(&self) -> &Arc<GroupState> {
        self.cache.state.get_or_init(|| {
            let settled = &self.settled_cache().state;
            let full = self.full();
            if full.rules == 0 {
                return settled.clone();
            }
            let mut state = GroupState::clone(settled);
            for hash in &full.order.hashes {
                let entry = &self.entries[hash];
                if !entry.body.is_room_rule() {
                    continue;
                }
                if full.budget.over.contains(hash) {
                    state.refuse(*hash, PAST_RUN);
                } else {
                    state.apply_rule(*hash, entry);
                }
            }
            Arc::new(state)
        })
    }

    /// The entries that are not spare (see [`Needed`]), worked out from the last entry of
    /// the order back: an entry is needed if it is live or named by a later one that is
    /// needed.
    pub(crate) fn needed_cache(&self) -> &Arc<Needed> {
        self.cache.needed.get_or_init(|| {
            let live = &self.full().budget.live;
            let mut hashes = HashSet::new();
            let mut named = HashSet::new();
            let mut tips = Vec::new();
            for hash in self.full().order.hashes.iter().rev() {
                let entry = &self.entries[hash];
                let is_named = named.contains(hash);
                if !is_named && !live.contains(hash) {
                    continue;
                }
                if !is_named {
                    tips.push(*hash);
                }
                hashes.insert(*hash);
                named.extend(entry.parents.iter().copied());
            }
            tips.reverse();
            Arc::new(Needed { hashes, tips })
        })
    }

    /// What entries are signed on.
    pub(crate) fn heads_cache(&self) -> &Vec<EntryHash> {
        self.cache.heads.get_or_init(|| {
            let full = self.full();
            self.live_tips(&full.order.hashes, &full.budget.live)
        })
    }

    /// The entries of `ordered` that no other names as parent.
    fn tips_in(&self, ordered: &[EntryHash]) -> Vec<EntryHash> {
        let referenced: HashSet<&EntryHash> = ordered
            .iter()
            .flat_map(|h| self.entries[h].parents.iter())
            .collect();
        ordered
            .iter()
            .filter(|h| !referenced.contains(h))
            .copied()
            .collect()
    }

    /// Forgets everything worked out: the entries changed in a way not carried forward.
    pub(crate) fn work_out_again(&mut self) {
        *self.cache = self.cache.kept();
    }

    /// Entry `hash` was taken out of the log: one that waited changes nothing placed;
    /// any other, everything.
    pub(crate) fn forgot(&mut self, hash: &EntryHash) {
        match self.cache.full.get_mut() {
            Some(full) if full.waiting.contains(hash) => {
                Arc::make_mut(full).waiting.remove(hash);
            }
            _ => self.work_out_again(),
        }
    }

    /// Carries what was worked out forward over new entry `hash`, just added. What cannot be
    /// carried forward is dropped, to be worked out again when asked for.
    pub(crate) fn note_new(&mut self, hash: EntryHash) {
        self.cache.needed.take();
        let heads = self.cache.heads.take();
        let state = self.cache.state.take();
        let settled = self.cache.settled.take();
        let Some(mut full) = self.cache.full.take() else {
            *self.cache = self.cache.kept();
            return;
        };
        let entry = &self.entries[&hash];
        // An entry that waited for this one may now go anywhere after it.
        if full
            .waiting
            .iter()
            .any(|w| self.entries[w].parents.contains(&hash))
        {
            *self.cache = self.cache.kept();
            return;
        }
        if entry
            .parents
            .iter()
            .any(|p| !full.order.place.contains_key(p))
        {
            // It waits too: nothing placed changes.
            Arc::make_mut(&mut full).waiting.insert(hash);
            self.cache.full = full.into();
            self.cache.settled = settled.map_or_else(OnceLock::new, OnceLock::from);
            self.cache.state = state.map_or_else(OnceLock::new, OnceLock::from);
            self.cache.heads = heads.map_or_else(OnceLock::new, OnceLock::from);
            return;
        }
        // The state with the room rules is the settled one when there are none: let go of it
        // here, so that carrying the settled one forward changes it in place.
        drop(state);
        let on_top = full.tips.iter().all(|h| entry.parents.contains(h));
        let f = Arc::make_mut(&mut full);
        // Whether it went last, everything before staying where it was.
        let last = if on_top {
            // Every entry of the order is before it.
            f.order.push(hash);
            Arc::make_mut(&mut f.budget).place(self, &hash);
            true
        } else {
            let from = entry
                .parents
                .iter()
                .map(|p| f.order.place[p] + 1)
                .max()
                .unwrap_or(0);
            let mut was = f.order.hashes[from..].to_vec();
            self.place(&mut f.order, &mut f.budget, hash);
            was.push(hash);
            f.order.hashes[from..] == was[..]
        };
        if last {
            f.tips.retain(|h| !entry.parents.contains(h));
            f.tips.push(hash);
        } else {
            f.tips = self.tips_in(&f.order.hashes);
        }
        let budget = f.budget.clone();
        if entry.body.is_room_rule() {
            // Rules apply on top of the group as the rest settles it, which a rule changes
            // in nothing (no entry is signed on a new one yet, so the rest keep their order
            // and ancestors) but the runs: one that goes last changes only the state with
            // the rules; one before others may end a run they were past, or fill one.
            f.rules += 1;
            self.cache.full = full.into();
            if last {
                self.cache.settled = settled.map_or_else(OnceLock::new, OnceLock::from);
            }
            return;
        }
        // Applied on top of the group as it settled, when it goes last and no loss before it
        // outranks it nor does it, if a loss, outrank any of them: then every round of the
        // losses comes out as before, with it applied last.
        if last
            && (on_top || self.outranks_nothing_new(&f.order, &hash))
            && let Some(mut settled) = settled.filter(|s| s.still)
        {
            let s = Arc::make_mut(&mut settled);
            let state = Arc::make_mut(&mut s.state);
            if budget.over.contains(&hash) {
                state.refuse(hash, PAST_RUN);
            } else {
                let needed = state.rotation_to_judge(entry).then(|| {
                    if on_top {
                        // The group as it saw it is the group now.
                        self.rotation_needed_on(&hash, state)
                    } else {
                        self.rotation_needed(&hash)
                    }
                });
                let before = |h: &EntryHash| self.is_before(&f.order.place, &entry.parents, h);
                state.apply(hash, entry, needed, &before);
            }
            // It takes the place of its parents among the live entries no other names, if it
            // is live itself (see `Budget::live`).
            if let Some(mut tips) = heads {
                if budget.live.contains(&hash) {
                    tips.retain(|h| !entry.parents.contains(h));
                    tips.push(hash);
                }
                self.cache.heads = tips.into();
            }
            self.cache.settled = settled.into();
        }
        self.cache.full = full.into();
    }

    /// Whether entry `hash`, last in the common `order`, neither stands apart from a loss of
    /// its author (or, for a join, of its ticket's issuer) nor, if a loss itself, from an
    /// entry of the device that lost: every such entry is one of its ancestors. Room rules,
    /// which no loss outranks, do not count.
    fn outranks_nothing_new(&self, order: &Order, hash: &EntryHash) -> bool {
        let entry = &self.entries[hash];
        let lost = match &entry.body {
            Body::Remove { member, .. } | Body::Demote { member } => Some(*member),
            Body::Leave => Some(entry.author),
            _ => None,
        };
        let mut devices = vec![entry.author];
        if let Body::Join { ticket, .. } = &entry.body {
            devices.push(ticket.issuer);
        }
        let check: Vec<usize> = order
            .hashes
            .iter()
            .enumerate()
            .filter(|(_, h)| *h != hash)
            .filter(|(_, h)| !self.entries[*h].body.is_room_rule())
            .filter(|(_, h)| {
                let other = &self.entries[*h];
                let by_lost = lost.is_some_and(|d| {
                    other.author == d
                        || matches!(&other.body, Body::Join { ticket, .. } if ticket.issuer == d)
                });
                let loss_of_author = match &other.body {
                    Body::Remove { member, .. } | Body::Demote { member } => {
                        devices.contains(member)
                    }
                    Body::Leave => devices.contains(&other.author),
                    _ => false,
                };
                by_lost || loss_of_author
            })
            .map(|(i, _)| i)
            .collect();
        let Some(&low) = check.iter().min() else {
            return true;
        };
        // Its ancestors back to the first entry to check.
        let mut seen = vec![false; order.hashes.len()];
        let mut todo = vec![*hash];
        while let Some(at) = todo.pop() {
            for parent in &self.entries[&at].parents {
                if let Some(&i) = order.place.get(parent)
                    && i >= low
                    && !seen[i]
                {
                    seen[i] = true;
                    todo.push(*parent);
                }
            }
        }
        check.iter().all(|i| seen[*i])
    }

    /// Puts entry `hash`, whose parents are all in `order` and which no entry names as parent,
    /// in its place: everything up to its last parent stays, what comes after is ordered
    /// again with it.
    /// `budget` is what the order made of the runs, before and after.
    fn place(&self, order: &mut Order, budget: &mut Arc<Budget>, hash: EntryHash) {
        let entry = &self.entries[&hash];
        let from = entry
            .parents
            .iter()
            .map(|p| order.place[p] + 1)
            .max()
            .unwrap_or(0);
        if from == order.hashes.len() {
            order.push(hash);
            Arc::make_mut(budget).place(self, &hash);
            return;
        }
        let rest = order.hashes[from..].iter().copied().chain([hash]).collect();
        let (tail, after) = self.order_after(
            &order.hashes[..from],
            |h| order.place.get(h).copied().filter(|i| *i < from),
            rest,
        );
        order.replace_from(from, tail);
        *budget = Arc::new(after);
    }

    /// The common order of `rest` after `before`, entries already in order (`placed` gives
    /// the place in it of an entry that is one of them): an entry comes after its parents; among entries
    /// ready at the same time, those of the devices that came in last go first, then the
    /// smallest hash. An entry with a parent neither placed nor in `rest` is left out, and
    /// so is every entry after it.
    ///
    /// The result is the order the whole log would have from where `before` ends: an entry
    /// is ranked against the others when its last parent is placed, with the comings in up
    /// to there; and who came in when and each device's run after the whole order (see
    /// [`Budget`]). An entry past its author's run counts in neither.
    pub(crate) fn order_after(
        &self,
        before: &[EntryHash],
        placed: impl Fn(&EntryHash) -> Option<usize>,
        rest: Vec<EntryHash>,
    ) -> (Vec<EntryHash>, Budget) {
        let in_rest: HashSet<EntryHash> = rest.iter().copied().collect();
        let mut waiting: HashMap<EntryHash, usize> = HashMap::new();
        let mut children: HashMap<EntryHash, Vec<EntryHash>> = HashMap::new();
        // Entries ready once `before` is placed, by the place of their last parent in it.
        let mut ready_at: BTreeMap<usize, Vec<EntryHash>> = BTreeMap::new();
        let mut ready_first = Vec::new();
        for hash in &rest {
            let entry = &self.entries[hash];
            if entry
                .parents
                .iter()
                .any(|p| placed(p).is_none() && !in_rest.contains(p))
            {
                continue;
            }
            let mut left = 0;
            let mut last = None;
            for p in &entry.parents {
                match placed(p) {
                    Some(at) => last = last.max(Some(at)),
                    None => {
                        left += 1;
                        children.entry(*p).or_default().push(*hash);
                    }
                }
            }
            waiting.insert(*hash, left);
            if left == 0 {
                match last {
                    Some(last) => ready_at.entry(last).or_default().push(*hash),
                    None => ready_first.push(*hash),
                }
            }
        }
        let mut budget = Budget::new(self.run_room);
        let mut ready: BTreeSet<(Reverse<u64>, EntryHash)> = BTreeSet::new();
        let key = |hash: &EntryHash, budget: &Budget| budget.key(&self.entries[hash].author, *hash);
        ready.extend(ready_first.iter().map(|h| key(h, &budget)));
        for (i, hash) in before.iter().enumerate() {
            budget.place(self, hash);
            if let Some(now_ready) = ready_at.remove(&i) {
                ready.extend(now_ready.iter().map(|h| key(h, &budget)));
            }
        }
        let mut order = Vec::with_capacity(waiting.len());
        while let Some((_, hash)) = ready.pop_first() {
            order.push(hash);
            budget.place(self, &hash);
            for child in children.get(&hash).into_iter().flatten() {
                if let Some(n) = waiting.get_mut(child) {
                    *n -= 1;
                    if *n == 0 {
                        ready.insert(key(child, &budget));
                    }
                }
            }
        }
        (order, budget)
    }

    /// The group, room rules and all, that the entries `before` make on their own, every
    /// entry before each of them among them: what a log of just those works out, without
    /// copying them into one.
    pub(crate) fn state_of(&self, before: Vec<EntryHash>) -> GroupState {
        self.view_of(before).0
    }

    /// [`Self::state_of`], and the runs those entries make (see [`Budget`]).
    pub(crate) fn view_of(&self, before: Vec<EntryHash>) -> View {
        let (hashes, budget) = self.order_after(&[], |_| None, before);
        let order = Order::of(hashes);
        let mut state = GroupState::clone(&self.settle(&order, &budget.over).state);
        for hash in &order.hashes {
            let entry = &self.entries[hash];
            if !entry.body.is_room_rule() {
                continue;
            }
            if budget.over.contains(hash) {
                state.refuse(*hash, PAST_RUN);
            } else {
                state.apply_rule(*hash, entry);
            }
        }
        (state, budget)
    }

    /// The group without its room rules after the entries of `order`, the common order (see
    /// [`GroupLog::state`]): the rules in it apply later, on top, and no loss outranks them.
    /// `over` are the entries of `order` past their author's run, which never apply.
    fn settle(&self, order: &Order, over: &Entries) -> Settled {
        let hashes = &order.hashes;
        let graph = OnceCell::new();
        let mut outranked = BTreeMap::new();
        let mut state = self.apply_in_order(order, &outranked, over);
        // Losses come from what applied, and what applies depends on what was refused: go on
        // until nothing changes. A loss is only ever signed by someone above, so that comes
        // within a round for each loss; past that, the last round stands, the same everywhere.
        // Those past their run never apply, so they do not count.
        let losses = hashes
            .iter()
            .filter(|h| {
                matches!(
                    self.entries[*h].body,
                    Body::Remove { .. } | Body::Leave | Body::Demote { .. }
                ) && !over.contains(*h)
            })
            .count();
        let mut still = false;
        for _ in 0..=losses {
            let next = self.outranked(order, &graph, &state, over);
            if next == outranked {
                still = true;
                break;
            }
            outranked = next;
            state = self.apply_in_order(order, &outranked, over);
        }
        Settled {
            state: Arc::new(state),
            outranked,
            still,
        }
    }

    fn apply_in_order(
        &self,
        order: &Order,
        outranked: &BTreeMap<EntryHash, EntryHash>,
        over: &Entries,
    ) -> GroupState {
        let mut state = GroupState::default();
        for hash in &order.hashes {
            let entry = &self.entries[hash];
            if entry.body.is_room_rule() {
                continue;
            }
            if over.contains(hash) {
                state.refuse(*hash, PAST_RUN);
            } else if outranked.contains_key(hash) {
                state.refuse(*hash, "signed apart from its author's removal");
            } else {
                let needed = state
                    .rotation_to_judge(entry)
                    .then(|| self.rotation_needed(hash));
                let before = |h: &EntryHash| self.is_before(&order.place, &entry.parents, h);
                state.apply(*hash, entry, needed, &before);
            }
        }
        state
    }

    /// Whether `target` is one of `parents` or before one of them, all placed in the order
    /// whose places `place` gives: the walk back stops at `target`'s place, since nothing
    /// before it in the order comes after it in the graph.
    pub(crate) fn is_before(
        &self,
        place: &HashMap<EntryHash, usize>,
        parents: &[EntryHash],
        target: &EntryHash,
    ) -> bool {
        let Some(&low) = place.get(target) else {
            return false;
        };
        let mut seen = HashSet::new();
        let mut todo = parents.to_vec();
        while let Some(hash) = todo.pop() {
            if hash == *target {
                return true;
            }
            match place.get(&hash) {
                Some(&at) if at > low && seen.insert(hash) => {
                    todo.extend(self.entries[&hash].parents.iter().copied());
                }
                _ => {}
            }
        }
        false
    }

    /// Entries that a removal, demotion or leave applied in `state` outranks (see
    /// [`GroupLog::state`]), each with the first such loss in the order. An entry's
    /// ancestors and descendants are all in the order, room rules among them, so the walks
    /// stay there.
    fn outranked(
        &self,
        order: &Order,
        graph: &OnceCell<Graph>,
        state: &GroupState,
        over: &Entries,
    ) -> BTreeMap<EntryHash, EntryHash> {
        // The device that lost something, the entry, and whether it lost only its place.
        let losses: Vec<(DevicePublicKey, EntryHash, bool)> = state
            .applied
            .iter()
            .filter_map(|h| match &self.entries[h].body {
                Body::Remove { member, .. } => Some((*member, *h, false)),
                Body::Leave => Some((self.entries[h].author, *h, false)),
                Body::Demote { member } => Some((*member, *h, true)),
                _ => None,
            })
            .collect();
        let mut out = BTreeMap::new();
        if losses.is_empty() {
            return out;
        }
        // What each device signed, and the joins with each device's invites: the only
        // entries a loss of that device can outrank.
        let mut signed: BTreeMap<DevicePublicKey, Vec<usize>> = BTreeMap::new();
        let mut invited: BTreeMap<DevicePublicKey, Vec<usize>> = BTreeMap::new();
        for (i, hash) in order.hashes.iter().enumerate() {
            let entry = &self.entries[hash];
            if entry.body.is_room_rule() || over.contains(hash) {
                continue;
            }
            signed.entry(entry.author).or_default().push(i);
            if let Body::Join { ticket, .. } = &entry.body {
                invited.entry(ticket.issuer).or_default().push(i);
            }
        }
        for (device, loss, place_only) in losses {
            let at = order.place[&loss];
            let suspects: Vec<usize> = signed
                .get(&device)
                .into_iter()
                .flatten()
                .filter(|i| !place_only || self.entries[&order.hashes[**i]].body.needs_rank())
                .chain(invited.get(&device).into_iter().flatten())
                .filter(|i| **i != at)
                .copied()
                .collect();
            // Most who lose something signed nothing else: no need to walk the log for them.
            let (Some(&low), Some(&high)) = (suspects.iter().min(), suspects.iter().max()) else {
                continue;
            };
            let graph = graph.get_or_init(|| Graph::of(self, order));
            let before = Graph::reach(&graph.parents, at, |i| i >= low);
            let after = Graph::reach(&graph.children, at, |i| i <= high);
            for i in suspects {
                if !before[i] && !after[i] {
                    out.entry(order.hashes[i]).or_insert(loss);
                }
            }
        }
        out
    }

    /// The secret in the box entry `hash` holds for `identity`, if it opens: opened once
    /// per entry and device.
    pub(crate) fn open_box(
        &self,
        hash: &EntryHash,
        epoch: &crate::entry::EpochId,
        sealed: &fogata_crypto::Sealed,
        identity: &DeviceIdentity,
    ) -> Option<Arc<GroupSecret>> {
        let key = (*hash, identity.public_key());
        let lock = || {
            self.cache
                .opened
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
        };
        if let Some(opened) = lock().get(&key) {
            return opened.clone();
        }
        let opened = sealed
            .open(identity, &secret_context(&self.group_id(), epoch))
            .ok()
            .and_then(|bytes| GroupSecret::from_bytes(&bytes).ok())
            .map(Arc::new);
        lock().insert(key, opened.clone());
        opened
    }
}
