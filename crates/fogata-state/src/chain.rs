//! The chain of MLS epochs of a group, read from its log.
//!
//! MLS needs one Commit to start each epoch. Fogata has no server to put Commits in a line:
//! they ride in log entries ([`Body::Commit`], [`Body::Resync`]), and two members can write
//! one on the same epoch without having seen each other's, or two halves of a group can go
//! on apart for hours. Every device settles this the same way, from the log alone:
//!
//! - The first Commit (on epoch 0, the MLS group as the member that made it started it,
//!   alone) may come from any manager, so that the group does not wait for one person; two at
//!   once settle as any two Commits do.
//! - A Commit names the epoch it was made on and the Commit that started it (`prev`, which it
//!   must be signed after), so the Commits form a tree.
//! - A Commit counts only if it fits in its author's run, as every entry must (see
//!   [`crate::budget`]), and if its author was allowed to make every change in it, judged on
//!   the group as the Commit's entry saw it (see [`Chain`]: managers add; a removal is
//!   carried out by a manager or a member that stood above the removed one, a leave by
//!   anyone); if it
//!   builds on a Commit that counts; if no removal, demotion or leave of its author outranks it, as for every other
//!   entry; and if its MLS message is not known to be bad ([`Verdict`]).
//! - Among the Commits that count on one epoch, the one with the longest line of counting
//!   Commits after it wins; equal lines go to the smallest entry hash. Plain "first in the
//!   common order" would let an entry written long ago on old parents undo a long chain.
//!
//! The intents (`Add`, `Join`, `Remove`, `Leave`) stay entries of their own and decide who is
//! a member, as before; Commits only carry them out for the keys. A Commit that loses takes
//! nothing away from the intent it carried: someone commits it again on the winner.
//!
//! What a device cannot tell the same way as every other (whether the secrets sealed to it
//! in a Commit open) is never part of this: it is that device's own trouble, which it
//! mends with a [`Body::Resync`].

use std::collections::{BTreeMap, BTreeSet};

use fogata_crypto::DevicePublicKey;

use crate::{
    budget::PAST_RUN,
    entry::{Body, Entry, EntryHash},
    log::GroupLog,
    state::GroupState,
};

/// The shortest time between two Commits of a member that only update its own keys, by
/// the clocks of the entries.
pub const MIN_UPDATE_GAP_MS: u64 = 24 * 60 * 60 * 1000;

/// What the members of an epoch found checking a Commit's MLS message with what every one of
/// them has: its signature, its framing, the tree it leads to, and that it makes exactly the
/// changes its entry names. Never whether the secrets sealed to one device open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// It checks.
    Valid,
    /// It does not: it starts no epoch, nor does anything built on it.
    Invalid,
    /// Not checked (the message has not arrived, or this device was not in that epoch): it
    /// counts as far as the log can tell.
    Unknown,
}

/// One epoch of the chain: the `Commit` or `Resync` entry that started it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub entry: EntryHash,
    pub author: DevicePublicKey,
    /// The epoch it starts.
    pub epoch: u64,
    /// The devices in the group's MLS tree from that epoch on.
    pub roster: BTreeSet<DevicePublicKey>,
}

/// The group's MLS epochs, as every device with the same log works them out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chain {
    /// The winning line of Commits, the first one first.
    pub links: Vec<Link>,
    /// Commits that count, whether or not they won.
    pub counted: BTreeSet<EntryHash>,
    /// Commits that do not count, with the reason.
    pub refused: BTreeMap<EntryHash, &'static str>,
}

impl Chain {
    pub fn tip(&self) -> Option<&Link> {
        self.links.last()
    }

    /// The current MLS epoch: 0 before the first Commit.
    pub fn epoch(&self) -> u64 {
        self.tip().map_or(0, |l| l.epoch)
    }

    /// Where `entry` is in the chain, if it is a link.
    pub fn position(&self, entry: &EntryHash) -> Option<usize> {
        self.links.iter().position(|l| l.entry == *entry)
    }

    /// The devices in the tree now: those of the last link; none before the first Commit.
    pub fn roster(&self) -> BTreeSet<DevicePublicKey> {
        self.tip().map(|l| l.roster.clone()).unwrap_or_default()
    }

    /// Members of `state` the tree still lacks: a Commit should add them. Before the first
    /// Commit, every member: the manager that starts the chain is in it by itself, and adds
    /// the others.
    pub fn to_add(&self, state: &GroupState) -> Vec<DevicePublicKey> {
        let roster = self.roster();
        state
            .members
            .keys()
            .filter(|k| !roster.contains(k))
            .copied()
            .collect()
    }

    /// Devices in the tree that are no longer members of `state`: a Commit should remove
    /// them.
    pub fn to_remove(&self, state: &GroupState) -> Vec<DevicePublicKey> {
        self.roster()
            .into_iter()
            .filter(|k| !state.is_member(k))
            .collect()
    }
}

/// What a Commit's judgment needs of the group as its entry saw it.
#[derive(Debug)]
pub(crate) struct CommitView {
    members: BTreeSet<DevicePublicKey>,
    /// Each member's place on the ladder, 0 for the first.
    ranks: BTreeMap<DevicePublicKey, usize>,
    managers: BTreeSet<DevicePublicKey>,
    /// Devices that are no members because they left (not removed).
    left: BTreeSet<DevicePublicKey>,
    /// Where each removed device stood on the ladder before it was removed.
    removed_from: BTreeMap<DevicePublicKey, usize>,
    /// The Commits and Resyncs signed before it.
    commits_before: BTreeSet<EntryHash>,
}

impl CommitView {
    /// What a Commit's judgment needs of `state`, the group as its entry saw it (its
    /// entries are `log`'s), with `commits_before` the Commits and Resyncs signed before it.
    pub(crate) fn of(
        log: &GroupLog,
        state: &GroupState,
        commits_before: BTreeSet<EntryHash>,
    ) -> Self {
        let body = |h: &EntryHash| &log.get(h).expect("applied entries are in the log").body;
        // Whether each device that is no member left last, rather than was removed.
        let mut left: BTreeSet<DevicePublicKey> = BTreeSet::new();
        for h in &state.applied {
            match body(h) {
                Body::Leave => {
                    left.insert(log.get(h).expect("in the log").author);
                }
                Body::Remove { member, .. } => {
                    left.remove(member);
                }
                _ => {}
            }
        }
        left.retain(|d| !state.is_member(d));
        let removed_from = state
            .applied
            .iter()
            .filter_map(|h| match body(h) {
                Body::Remove { member, .. } if !left.contains(member) => {
                    Some((*member, state.rank_when_gone(member)?))
                }
                _ => None,
            })
            .collect();
        Self {
            members: state.members.keys().copied().collect(),
            ranks: state
                .ladder
                .iter()
                .enumerate()
                .map(|(r, k)| (*k, r))
                .collect(),
            managers: state
                .ladder
                .iter()
                .filter(|k| state.is_manager(k))
                .copied()
                .collect(),
            left,
            removed_from,
            commits_before,
        }
    }
}

/// A Commit that counts, as the chain needs it.
#[derive(Clone, Debug)]
struct Node {
    author: DevicePublicKey,
    time_ms: u64,
    prev: Option<EntryHash>,
    /// The epoch it starts.
    epoch: u64,
    roster: BTreeSet<DevicePublicKey>,
    /// Whether it only updates its author's keys.
    update: bool,
}

impl GroupLog {
    /// The group's chain of MLS epochs (see [`crate::chain`]), with `verdict` saying what
    /// this device found checking each Commit's message.
    pub fn mls_chain(&self, verdict: impl Fn(&EntryHash) -> Verdict) -> Chain {
        let order: Vec<EntryHash> = self
            .ordered()
            .into_iter()
            .filter(|h| {
                matches!(
                    self.get(h).map(|e| &e.body),
                    Some(Body::Commit { .. } | Body::Resync { .. })
                )
            })
            .collect();
        let mut chain = Chain::default();
        if order.is_empty() {
            return chain;
        }
        let outranked = self.outranked_commits(&order);
        let over = &self.full().budget.over;
        let mut nodes: BTreeMap<EntryHash, Node> = BTreeMap::new();
        for hash in &order {
            // Past its author's run it applies to nothing, and a device may let it go.
            let judged = if over.contains(hash) {
                Err(PAST_RUN)
            } else if outranked.contains(hash) {
                Err("signed apart from its author's removal")
            } else if verdict(hash) == Verdict::Invalid {
                Err("its MLS message does not check")
            } else {
                self.judge_commit(hash, &nodes)
            };
            match judged {
                Ok(node) => {
                    nodes.insert(*hash, node);
                    chain.counted.insert(*hash);
                }
                Err(reason) => {
                    chain.refused.insert(*hash, reason);
                }
            }
        }
        // The longest line of counting Commits after each, the Commit itself included.
        let mut children: BTreeMap<Option<EntryHash>, Vec<EntryHash>> = BTreeMap::new();
        for (hash, node) in &nodes {
            children.entry(node.prev).or_default().push(*hash);
        }
        let mut depth: BTreeMap<EntryHash, usize> = BTreeMap::new();
        for hash in order.iter().rev().filter(|h| nodes.contains_key(*h)) {
            let below = children
                .get(&Some(*hash))
                .into_iter()
                .flatten()
                .map(|c| depth[c])
                .max()
                .unwrap_or(0);
            depth.insert(*hash, below + 1);
        }
        let mut at = None;
        while let Some(next) = children.get(&at).and_then(|c| {
            c.iter()
                .max_by(|a, b| depth[*a].cmp(&depth[*b]).then(b.cmp(a)))
        }) {
            let node = &nodes[next];
            chain.links.push(Link {
                entry: *next,
                author: node.author,
                epoch: node.epoch,
                roster: node.roster.clone(),
            });
            at = Some(*next);
        }
        chain
    }

    /// Whether the Commit or Resync `hash` counts, given the Commits before it that do
    /// (`nodes`): see [`crate::chain`].
    fn judge_commit(
        &self,
        hash: &EntryHash,
        nodes: &BTreeMap<EntryHash, Node>,
    ) -> Result<Node, &'static str> {
        let entry = self.get(hash).expect("in the log");
        let author = entry.author;
        let (link, changes) = match &entry.body {
            Body::Commit { link, changes, .. } => (link, Some(changes)),
            Body::Resync { link, .. } => (link, None),
            _ => return Err("not a commit"),
        };
        let view = self.commit_view(hash);
        self.commit_allowed_in(entry, &view)?;
        let roster = match link.prev {
            None => BTreeSet::from([author]),
            Some(prev) => {
                let Some(before) = nodes.get(&prev) else {
                    return Err("builds on a commit that does not count");
                };
                before.roster.clone()
            }
        };
        if !roster.contains(&author) {
            return Err("author has no place in the tree");
        }
        let mut after = roster.clone();
        let update = match changes {
            None => false,
            Some(changes) => {
                for device in &changes.add {
                    if roster.contains(device) {
                        return Err("adds a device that is no member or is in the tree");
                    }
                    after.insert(*device);
                }
                for device in &changes.remove {
                    if !roster.contains(device) {
                        return Err("removes a member or a device not in the tree");
                    }
                    after.remove(device);
                }
                if changes.is_update()
                    && self
                        .last_update(link.prev, &author, nodes)
                        .is_some_and(|last| entry.time_ms.abs_diff(last) < MIN_UPDATE_GAP_MS)
                {
                    return Err("updates its keys too often");
                }
                changes.is_update()
            }
        };
        Ok(Node {
            author,
            time_ms: entry.time_ms,
            prev: link.prev,
            epoch: link.epoch + 1,
            roster: after,
            update,
        })
    }

    /// What of [`Self::judge_commit`] needs only the group as the Commit or Resync `entry`
    /// saw it (`view`), not the chain: whether its author could make it then. A device
    /// keeps no Commit that fails it (see [`GroupLog::insert`]).
    pub(crate) fn commit_allowed_in(
        &self,
        entry: &Entry,
        view: &CommitView,
    ) -> Result<(), &'static str> {
        let author = entry.author;
        let (link, changes, welcome) = match &entry.body {
            Body::Commit {
                link,
                changes,
                welcome,
                ..
            } => (link, Some(changes), welcome.is_some()),
            Body::Resync { link, .. } => (link, None, false),
            _ => return Err("not a commit"),
        };
        if !view.members.contains(&author) {
            return Err("author is not a member");
        }
        match link.prev {
            None => {
                if link.epoch != 0 || changes.is_none() {
                    return Err("only a commit starts the chain, on epoch 0");
                }
                if !view.managers.contains(&author) {
                    return Err("only a manager starts the chain");
                }
            }
            Some(prev) => {
                if !view.commits_before.contains(&prev) {
                    return Err("signed before the commit it builds on");
                }
                let prev_epoch = match self.get(&prev).map(|e| &e.body) {
                    Some(Body::Commit { link, .. } | Body::Resync { link, .. }) => link.epoch,
                    _ => return Err("builds on a commit that does not count"),
                };
                if prev_epoch + 1 != link.epoch {
                    return Err("names another epoch than the one it builds on");
                }
            }
        }
        let Some(changes) = changes else {
            return Ok(());
        };
        if welcome != !changes.add.is_empty() {
            return Err("a welcome goes with adds, and only with them");
        }
        if !changes.add.is_empty() && !view.managers.contains(&author) {
            return Err("only managers add");
        }
        let rank = view.ranks.get(&author).copied().unwrap_or(usize::MAX);
        if changes.add.iter().any(|d| !view.members.contains(d)) {
            return Err("adds a device that is no member or is in the tree");
        }
        for device in &changes.remove {
            if view.members.contains(device) {
                return Err("removes a member or a device not in the tree");
            }
            // A manager carries out any removal: a device can also be no member without a
            // removal, when what let it in was undone.
            let above = view.removed_from.get(device).is_some_and(|r| rank < *r);
            if !above && !view.left.contains(device) && !view.managers.contains(&author) {
                return Err("only a manager or someone above carries out a removal");
            }
        }
        Ok(())
    }

    /// When `author` last only updated its keys, on the line of Commits ending at `from`.
    fn last_update(
        &self,
        mut from: Option<EntryHash>,
        author: &DevicePublicKey,
        nodes: &BTreeMap<EntryHash, Node>,
    ) -> Option<u64> {
        while let Some(node) = from.and_then(|h| nodes.get(&h)) {
            if node.update && node.author == *author {
                return Some(node.time_ms);
            }
            from = node.prev;
        }
        None
    }

    /// The group as Commit `hash` saw it, worked out once per Commit.
    fn commit_view(&self, hash: &EntryHash) -> std::sync::Arc<CommitView> {
        let lock = || {
            self.commit_views
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        };
        if let Some(view) = lock().get(hash) {
            return view.clone();
        }
        let state = self.state_before(hash);
        let commits_before = self
            .ancestors(hash)
            .into_iter()
            .filter(|h| {
                matches!(
                    self.get(h).map(|e| &e.body),
                    Some(Body::Commit { .. } | Body::Resync { .. })
                )
            })
            .collect();
        let view = std::sync::Arc::new(CommitView::of(self, &state, commits_before));
        lock().insert(*hash, view.clone());
        view
    }

    /// Commits (of `order`) that a removal, demotion or leave of their author outranks: they
    /// are neither before nor after it. A demotion outranks only Commits that add or remove.
    fn outranked_commits(&self, order: &[EntryHash]) -> BTreeSet<EntryHash> {
        let state = self.state();
        let mut out = BTreeSet::new();
        for loss in &state.applied {
            let entry = self.get(loss).expect("applied entries are in the log");
            let (device, place_only) = match &entry.body {
                Body::Remove { member, .. } => (*member, false),
                Body::Leave => (entry.author, false),
                Body::Demote { member } => (*member, true),
                _ => continue,
            };
            let suspects: Vec<&EntryHash> = order
                .iter()
                .filter(|h| {
                    let commit = self.get(h).expect("in the log");
                    commit.author == device
                        && (!place_only
                            || matches!(&commit.body, Body::Commit { changes, .. } if !changes.is_update()))
                })
                .collect();
            if suspects.is_empty() {
                continue;
            }
            let (before, after) = (self.ancestors(loss), self.descendants(loss));
            for hash in suspects {
                if !before.contains(hash) && !after.contains(hash) {
                    out.insert(*hash);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use super::*;
    use crate::{
        budget::RUN_ROOM,
        entry::{Changes, Entry, EpochLink, payload_digest},
    };

    fn all_valid(_: &EntryHash) -> Verdict {
        Verdict::Valid
    }

    /// Signs a Commit by `author` on `log`'s heads and adds it, if the log keeps it (see
    /// [`refused`]).
    fn commit(
        log: &mut GroupLog,
        author: &DeviceIdentity,
        link: EpochLink,
        changes: Changes,
        time: u64,
    ) -> EntryHash {
        let entry = commit_entry(log, author, link, changes, time);
        let hash = entry.hash();
        let _ = log.insert(entry);
        hash
    }

    /// Why `log` does not keep the Commit by `author` on its heads: its author could not
    /// make it in its own view.
    fn refused(
        log: &mut GroupLog,
        author: &DeviceIdentity,
        link: EpochLink,
        changes: Changes,
        time: u64,
    ) -> &'static str {
        let entry = commit_entry(log, author, link, changes, time);
        let hash = entry.hash();
        let Err(crate::log::StateError::NotAllowed(reason)) = log.insert(entry) else {
            panic!("the Commit was kept");
        };
        assert!(log.get(&hash).is_none());
        reason
    }

    fn commit_entry(
        log: &GroupLog,
        author: &DeviceIdentity,
        link: EpochLink,
        changes: Changes,
        time: u64,
    ) -> Entry {
        let welcome = (!changes.add.is_empty()).then(|| payload_digest(&time.to_be_bytes()));
        Entry::sign(
            author,
            log.group_id(),
            log.heads(),
            time,
            Body::Commit {
                link,
                changes,
                payload: payload_digest(
                    &[&time.to_be_bytes()[..], author.public_key().as_bytes()].concat(),
                ),
                welcome,
            },
        )
    }

    fn resync(
        log: &mut GroupLog,
        author: &DeviceIdentity,
        link: EpochLink,
        time: u64,
    ) -> EntryHash {
        let entry = Entry::sign(
            author,
            log.group_id(),
            log.heads(),
            time,
            Body::Resync {
                link,
                payload: payload_digest(&time.to_be_bytes()),
            },
        );
        let hash = entry.hash();
        let _ = log.insert(entry);
        hash
    }

    fn on(epoch: u64, prev: Option<EntryHash>) -> EpochLink {
        EpochLink { epoch, prev }
    }

    /// Gives `to` what it lacks of `from`, as a sync does.
    fn sync(from: &GroupLog, to: &mut GroupLog) {
        let missing: Vec<Entry> = from
            .entries()
            .filter(|e| to.get(&e.hash()).is_none())
            .cloned()
            .collect();
        to.insert_all(missing);
    }

    fn ids(n: usize) -> Vec<DeviceIdentity> {
        (0..n)
            .map(|_| DeviceIdentity::generate().unwrap())
            .collect()
    }

    /// Ana's group with Bea and Leo, and its first Commit by Ana, adding both.
    fn three() -> (GroupLog, Vec<DeviceIdentity>, EntryHash) {
        let d = ids(4);
        let mut log = GroupLog::create(&d[0], "g", 1).unwrap();
        log.add(&d[0], d[1].public_key(), "Bea", 2).unwrap();
        log.add(&d[0], d[2].public_key(), "Leo", 3).unwrap();
        let first = commit(
            &mut log,
            &d[0],
            on(0, None),
            Changes::new([d[1].public_key(), d[2].public_key()], []),
            4,
        );
        (log, d, first)
    }

    #[test]
    fn any_manager_starts_the_chain() {
        let d = ids(3);
        let mut log = GroupLog::create(&d[0], "g", 1).unwrap();
        log.append(&d[0], Body::SetManagers { first: Some(1) }, 1)
            .unwrap();
        log.add(&d[0], d[1].public_key(), "Bea", 2).unwrap();
        assert_eq!(
            refused(&mut log, &d[1], on(0, None), Changes::default(), 3),
            "only a manager starts the chain"
        );
        let chain = log.mls_chain(all_valid);
        assert!(chain.links.is_empty());
        assert_eq!(chain.to_add(&log.state()).len(), 2);
        let ana = commit(
            &mut log,
            &d[0],
            on(0, None),
            Changes::new([d[1].public_key()], []),
            4,
        );
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.links.len(), 1);
        assert_eq!(chain.links[0].entry, ana);
        assert_eq!(chain.epoch(), 1);
        assert_eq!(
            chain.roster(),
            BTreeSet::from([d[0].public_key(), d[1].public_key()])
        );
        assert!(chain.to_add(&log.state()).is_empty());
        // Without the first: another manager starts it, and two starts settle like any two
        // Commits on one epoch.
        let mut log = GroupLog::create(&d[0], "g", 1).unwrap();
        log.add(&d[0], d[1].public_key(), "Bea", 2).unwrap();
        log.add(&d[0], d[2].public_key(), "Leo", 3).unwrap();
        log.append(&d[0], Body::SetManagers { first: Some(2) }, 4)
            .unwrap();
        let mut other = log.clone();
        let by_bea = commit(
            &mut log,
            &d[1],
            on(0, None),
            Changes::new([d[0].public_key(), d[2].public_key()], []),
            5,
        );
        let by_ana = commit(
            &mut other,
            &d[0],
            on(0, None),
            Changes::new([d[1].public_key(), d[2].public_key()], []),
            6,
        );
        sync(&other, &mut log);
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.counted.len(), 2);
        assert_eq!(chain.tip().unwrap().entry, by_bea.min(by_ana));
        assert_eq!(chain.roster().len(), 3);
    }

    #[test]
    fn managers_add_those_above_remove_and_any_member_drops_who_left() {
        let (mut log, d, first) = three();
        let (bea, leo, sofia) = (&d[1], &d[2], &d[3]);
        log.append(&d[0], Body::SetManagers { first: Some(1) }, 5)
            .unwrap();
        log.add(&d[0], sofia.public_key(), "Sofía", 5).unwrap();
        // Bea does not manage: her Commit adding Sofía is not even kept.
        assert_eq!(
            refused(
                &mut log,
                bea,
                on(1, Some(first)),
                Changes::new([sofia.public_key()], []),
                6,
            ),
            "only managers add"
        );
        // Adding a device that is no member, or one in the tree already: neither counts.
        let stranger = DeviceIdentity::generate().unwrap().public_key();
        assert_eq!(
            refused(
                &mut log,
                &d[0],
                on(1, Some(first)),
                Changes::new([stranger], []),
                7,
            ),
            "adds a device that is no member or is in the tree"
        );
        let twice = commit(
            &mut log,
            &d[0],
            on(1, Some(first)),
            Changes::new([leo.public_key()], []),
            8,
        );
        let chain = log.mls_chain(all_valid);
        assert!(chain.refused.contains_key(&twice));
        // A welcome goes with adds.
        let entry = Entry::sign(
            &d[0],
            log.group_id(),
            log.heads(),
            9,
            Body::Commit {
                link: on(1, Some(first)),
                changes: Changes::new([sofia.public_key()], []),
                payload: [1; 32],
                welcome: None,
            },
        );
        assert_eq!(
            log.insert(entry),
            Err(crate::log::StateError::NotAllowed(
                "a welcome goes with adds, and only with them"
            ))
        );
        let added = commit(
            &mut log,
            &d[0],
            on(1, Some(first)),
            Changes::new([sofia.public_key()], []),
            10,
        );
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.tip().unwrap().entry, added);
        assert_eq!(chain.roster().len(), 4);
        // Ana removes Leo; only someone who was above him carries that out, not Sofía.
        log.remove(&d[0], leo.public_key(), 11).unwrap();
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.to_remove(&log.state()), vec![leo.public_key()]);
        assert_eq!(
            refused(
                &mut log,
                sofia,
                on(2, Some(added)),
                Changes::new([], [leo.public_key()]),
                12,
            ),
            "only a manager or someone above carries out a removal"
        );
        let mut other = log.clone();
        let by_bea = commit(
            &mut other,
            bea,
            on(2, Some(added)),
            Changes::new([], [leo.public_key()]),
            12,
        );
        assert!(other.mls_chain(all_valid).counted.contains(&by_bea));
        // Sofía leaves: any member in the tree drops her.
        log.append(sofia, Body::Leave, 13).unwrap();
        let by_bea = commit(
            &mut log,
            bea,
            on(2, Some(added)),
            Changes::new([], [sofia.public_key()]),
            14,
        );
        let chain = log.mls_chain(all_valid);
        assert!(chain.counted.contains(&by_bea));
        // Removing a member, or a device not in the tree: no.
        assert_eq!(
            refused(
                &mut log,
                &d[0],
                on(3, Some(by_bea)),
                Changes::new([], [bea.public_key()]),
                15,
            ),
            "removes a member or a device not in the tree"
        );
        let done = commit(
            &mut log,
            &d[0],
            on(3, Some(by_bea)),
            Changes::new([], [leo.public_key()]),
            16,
        );
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.tip().unwrap().entry, done);
        assert_eq!(chain.epoch(), 4);
        assert!(chain.to_remove(&log.state()).is_empty());
        assert_eq!(
            chain.roster(),
            BTreeSet::from([d[0].public_key(), bea.public_key()])
        );
    }

    #[test]
    fn a_member_updates_its_keys_at_most_once_a_day() {
        let (mut log, d, first) = three();
        let day = MIN_UPDATE_GAP_MS;
        let a = commit(&mut log, &d[1], on(1, Some(first)), Changes::default(), day);
        let soon = commit(
            &mut log,
            &d[1],
            on(2, Some(a)),
            Changes::default(),
            day + 3_600_000,
        );
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.refused[&soon], "updates its keys too often");
        // Another member may, and Bea again a day later.
        let leo = commit(&mut log, &d[2], on(2, Some(a)), Changes::default(), day + 1);
        let later = commit(
            &mut log,
            &d[1],
            on(3, Some(leo)),
            Changes::default(),
            2 * day,
        );
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.tip().unwrap().entry, later);
        assert_eq!(chain.epoch(), 4);
    }

    #[test]
    fn a_commit_names_the_epoch_it_builds_on_and_comes_after_it() {
        let (mut log, d, first) = three();
        assert_eq!(
            refused(&mut log, &d[1], on(5, Some(first)), Changes::default(), 5),
            "names another epoch than the one it builds on"
        );
        // Signed apart from the Commit it names: as if written before it.
        let mut apart = GroupLog::create(&d[3], "other", 1).unwrap();
        let _ = &mut apart;
        let before_first = {
            let root = log.root().unwrap();
            let entry = Entry::sign(
                &d[2],
                log.group_id(),
                vec![root],
                6,
                Body::Commit {
                    link: on(1, Some(first)),
                    changes: Changes::default(),
                    payload: [2; 32],
                    welcome: None,
                },
            );
            log.insert(entry)
        };
        // Leo was not even in yet where it was signed.
        assert_eq!(
            before_first,
            Err(crate::log::StateError::NotAllowed("author is not a member"))
        );
        assert_eq!(
            refused(&mut log, &d[1], on(1, Some([9; 32])), Changes::default(), 7),
            "signed before the commit it builds on"
        );
        assert_eq!(log.mls_chain(all_valid).links.len(), 1);
    }

    #[test]
    fn two_commits_at_once_settle_on_the_smaller_hash_and_longer_lines_win() {
        let (log, d, first) = three();
        // Bea and Leo each update on epoch 1 without seeing the other.
        let (mut b, mut l) = (log.clone(), log.clone());
        let by_bea = commit(&mut b, &d[1], on(1, Some(first)), Changes::default(), 10);
        let by_leo = commit(&mut l, &d[2], on(1, Some(first)), Changes::default(), 11);
        let mut both = b.clone();
        sync(&l, &mut both);
        let chain = both.mls_chain(all_valid);
        assert_eq!(chain.links.len(), 2);
        assert_eq!(chain.tip().unwrap().entry, by_bea.min(by_leo));
        assert_eq!(chain.counted.len(), 3);
        // The other half goes on: two Commits after the loser outweigh the winner alone.
        let loser = by_bea.max(by_leo);
        let side = if loser == by_bea { &mut b } else { &mut l };
        let next = commit(side, &d[0], on(2, Some(loser)), Changes::default(), 20);
        let last = commit(
            side,
            &d[0],
            on(3, Some(next)),
            Changes::new([], []),
            20 + MIN_UPDATE_GAP_MS,
        );
        let side = side.clone();
        sync(&side, &mut both);
        let chain = both.mls_chain(all_valid);
        assert_eq!(
            chain.links.iter().map(|l| l.entry).collect::<Vec<_>>(),
            vec![first, loser, next, last]
        );
        // Unless its Commits turn out bad: then the line of the winner stands alone again.
        let chain = both.mls_chain(|h| {
            if *h == next {
                Verdict::Invalid
            } else {
                Verdict::Valid
            }
        });
        assert_eq!(chain.tip().unwrap().entry, by_bea.min(by_leo));
        assert_eq!(
            chain.refused[&last],
            "builds on a commit that does not count"
        );
    }

    #[test]
    fn a_removed_or_demoted_admin_commits_nothing_apart_from_that() {
        let (mut log, d, first) = three();
        log.append(
            &d[0],
            Body::Promote {
                member: d[1].public_key(),
            },
            5,
        )
        .unwrap();
        log.add(&d[0], d[3].public_key(), "Sofía", 6).unwrap();
        // Bea adds Sofía while Ana, apart, takes her admin rights away.
        let mut b = log.clone();
        let by_bea = commit(
            &mut b,
            &d[1],
            on(1, Some(first)),
            Changes::new([d[3].public_key()], []),
            7,
        );
        assert_eq!(b.mls_chain(all_valid).tip().unwrap().entry, by_bea);
        log.append(
            &d[0],
            Body::Demote {
                member: d[1].public_key(),
            },
            8,
        )
        .unwrap();
        sync(&b, &mut log);
        let chain = log.mls_chain(all_valid);
        assert_eq!(
            chain.refused[&by_bea],
            "signed apart from its author's removal"
        );
        assert_eq!(chain.tip().unwrap().entry, first);
        assert_eq!(chain.to_add(&log.state()), vec![d[3].public_key()]);
        // Her own updates still count: a demotion only takes away what needs an admin.
        let update = commit(&mut b, &d[1], on(1, Some(first)), Changes::default(), 9);
        sync(&b, &mut log);
        assert_eq!(log.mls_chain(all_valid).tip().unwrap().entry, update);
    }

    #[test]
    fn only_a_device_in_the_tree_resyncs() {
        let (mut log, d, first) = three();
        log.add(&d[0], d[3].public_key(), "Sofía", 5).unwrap();
        let in_tree = resync(&mut log, &d[2], on(1, Some(first)), 6);
        let not_yet = resync(&mut log, &d[3], on(2, Some(in_tree)), 7);
        let chain = log.mls_chain(all_valid);
        assert_eq!(chain.tip().unwrap().entry, in_tree);
        assert_eq!(chain.refused[&not_yet], "author has no place in the tree");
        // Resyncs are no updates: the same device may resync again at once.
        let again = resync(&mut log, &d[2], on(2, Some(in_tree)), 8);
        assert_eq!(log.mls_chain(all_valid).tip().unwrap().entry, again);
    }

    /// xorshift64.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
    }

    /// Several devices, each with its own copy of a group's log, doing what members do at
    /// random and meeting only now and then.
    struct World {
        rng: Rng,
        time: u64,
        devices: Vec<DeviceIdentity>,
        logs: Vec<GroupLog>,
        /// Commits whose MLS message every member found bad.
        invalid: BTreeSet<EntryHash>,
        commits: Vec<EntryHash>,
        /// Commits refused where they were written, past their author's run.
        past_run: usize,
    }

    impl World {
        fn new(seed: u64, n: usize) -> Self {
            let devices = ids(n);
            let mut log = GroupLog::create(&devices[0], "g", 1).unwrap();
            for (i, d) in devices.iter().enumerate().skip(1).take(n - 2) {
                log.add(&devices[0], d.public_key(), "m", 1 + i as u64)
                    .unwrap();
            }
            // Only the first two manage, so that some members do not.
            log.append(&devices[0], Body::SetManagers { first: Some(2) }, 10)
                .unwrap();
            let logs = vec![log; n];
            Self {
                rng: Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1),
                time: 100,
                devices,
                logs,
                invalid: BTreeSet::new(),
                commits: Vec::new(),
                past_run: 0,
            }
        }

        fn chain(&self, i: usize) -> Chain {
            self.logs[i].mls_chain(|h| {
                if self.invalid.contains(h) {
                    Verdict::Invalid
                } else {
                    Verdict::Valid
                }
            })
        }

        /// Device `i` does what its view of the chain asks of it: adds and removes if it
        /// may, else updates its keys; now and then on an older Commit than its newest, as
        /// a device that has not caught up does.
        fn commit_duty(&mut self, i: usize, stale: bool) {
            let chain = self.chain(i);
            let state = self.logs[i].state();
            let me = self.devices[i].public_key();
            let starting = chain.links.is_empty();
            if !state.is_member(&me) || !(starting || chain.roster().contains(&me)) {
                return;
            }
            let manager = state.is_manager(&me);
            let mut add = if manager {
                chain.to_add(&state)
            } else {
                vec![]
            };
            add.retain(|d| *d != me || !starting);
            let mut remove = chain.to_remove(&state);
            let mut link = on(chain.epoch(), chain.tip().map(|l| l.entry));
            if stale && chain.links.len() > 1 {
                let at = self.rng.below(chain.links.len());
                link = if at == 0 {
                    on(0, None)
                } else {
                    on(chain.links[at - 1].epoch, Some(chain.links[at - 1].entry))
                };
            }
            if link.prev.is_none() {
                if !manager {
                    return;
                }
                add = state
                    .members
                    .keys()
                    .filter(|d| **d != me)
                    .copied()
                    .collect();
                remove.clear();
            }
            let entry = commit_entry(
                &self.logs[i],
                &self.devices[i],
                link,
                Changes::new(add, remove),
                self.time,
            );
            let hash = entry.hash();
            if self.logs[i].insert(entry) == Err(crate::log::StateError::NotAllowed(PAST_RUN)) {
                self.past_run += 1;
            }
            self.commits.push(hash);
        }

        fn step(&mut self) {
            self.time += self.rng.below(6) as u64 * 3_600_000;
            let n = self.devices.len();
            let i = self.rng.below(n);
            let me = self.devices[i].public_key();
            let state = self.logs[i].state();
            match self.rng.below(100) {
                0..30 => {
                    let stale = self.rng.below(8) == 0;
                    self.commit_duty(i, stale);
                }
                30..40 => {
                    let target = self.devices[self.rng.below(n)].public_key();
                    if state.outranks(&me, &target) {
                        let _ = self.logs[i].remove(&self.devices[i], target, self.time);
                    }
                }
                // The first stays, so that someone always manages.
                40..44 if state.rank(&me).is_some_and(|r| r > 0) => {
                    let _ = self.logs[i].append(&self.devices[i], Body::Leave, self.time);
                }
                44..52 if state.is_manager(&me) => {
                    let target = self.devices[self.rng.below(n)].public_key();
                    if !state.is_member(&target) {
                        let _ = self.logs[i].add(&self.devices[i], target, "m", self.time);
                    }
                }
                52..56 => {
                    let target = self.devices[self.rng.below(n)].public_key();
                    if state.outranks(&me, &target) {
                        let body = if self.rng.below(2) == 0 {
                            Body::Promote { member: target }
                        } else {
                            Body::Demote { member: target }
                        };
                        let _ = self.logs[i].append(&self.devices[i], body, self.time);
                    }
                }
                56..60 => {
                    let chain = self.chain(i);
                    if let Some(tip) = chain.tip()
                        && tip.roster.contains(&me)
                    {
                        let hash = resync(
                            &mut self.logs[i],
                            &self.devices[i],
                            on(tip.epoch, Some(tip.entry)),
                            self.time,
                        );
                        self.commits.push(hash);
                    }
                }
                60..62 if !self.commits.is_empty() => {
                    let bad = self.commits[self.rng.below(self.commits.len())];
                    self.invalid.insert(bad);
                }
                62.. => {
                    let j = self.rng.below(n);
                    if i != j {
                        let from = self.logs[i].clone();
                        sync(&from, &mut self.logs[j]);
                        if self.rng.below(2) == 0 {
                            let back = self.logs[j].clone();
                            sync(&back, &mut self.logs[i]);
                        }
                    }
                }
                _ => {}
            }
        }

        fn sync_all(&mut self) {
            let mut all = self.logs[0].clone();
            for log in &self.logs[1..] {
                sync(log, &mut all);
            }
            for log in &mut self.logs {
                sync(&all, log);
            }
        }
    }

    /// The chain follows the fork rule: each link is, among the Commits that count on the
    /// same Commit, one with the longest line after it, the smallest hash among equals.
    fn check_fork_rule(log: &GroupLog, chain: &Chain) {
        let prev_of = |h: &EntryHash| match &log.get(h).unwrap().body {
            Body::Commit { link, .. } | Body::Resync { link, .. } => link.prev,
            _ => unreachable!(),
        };
        fn line(
            h: &EntryHash,
            counted: &BTreeSet<EntryHash>,
            prev_of: &dyn Fn(&EntryHash) -> Option<EntryHash>,
        ) -> usize {
            1 + counted
                .iter()
                .filter(|c| prev_of(c) == Some(*h))
                .map(|c| line(c, counted, prev_of))
                .max()
                .unwrap_or(0)
        }
        let mut prev = None;
        for (i, link) in chain.links.iter().enumerate() {
            assert_eq!(link.epoch, i as u64 + 1);
            assert_eq!(prev_of(&link.entry), prev);
            assert!(chain.counted.contains(&link.entry));
            assert!(!chain.refused.contains_key(&link.entry));
            let mine = line(&link.entry, &chain.counted, &prev_of);
            for other in chain
                .counted
                .iter()
                .filter(|c| prev_of(c) == prev && **c != link.entry)
            {
                let theirs = line(other, &chain.counted, &prev_of);
                assert!(mine > theirs || (mine == theirs && link.entry < *other));
            }
            prev = Some(link.entry);
        }
        // Nothing that counts is left out of the tree the rule picks from.
        let no_room = chain
            .counted
            .iter()
            .filter(|c| prev_of(c).is_none())
            .count();
        assert!(chain.counted.is_empty() || no_room > 0);
    }

    /// `FOGATA_CHAIN_SEEDS` sets how many histories run (8 by default).
    #[test]
    fn every_device_settles_on_the_same_chain_whatever_happened_apart() {
        let seeds = std::env::var("FOGATA_CHAIN_SEEDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8u64);
        let mut past_run = 0;
        for seed in 1..=seeds {
            let mut world = World::new(seed, 6);
            // In odd seeds a device writes little in a run (the first writes six entries
            // that count to start), so Commits go past it: refused where they are judged, or
            // counting for nothing in the common order.
            let run = if seed % 2 == 1 { 7 } else { RUN_ROOM.entries };
            for log in &mut world.logs {
                log.set_run_room(run, RUN_ROOM.bytes);
            }
            // The first starts the chain with everyone in.
            world.commit_duty(0, false);
            for _ in 0..140 {
                world.step();
            }
            world.sync_all();
            let chain = world.chain(0);
            for i in 1..world.logs.len() {
                assert_eq!(world.chain(i), chain, "seed {seed}: device {i} differs");
            }
            // The same entries in any order make the same chain.
            let mut entries: Vec<Entry> = world.logs[0].entries().cloned().collect();
            for _ in 0..entries.len() {
                let (a, b) = (
                    world.rng.below(entries.len()),
                    world.rng.below(entries.len()),
                );
                entries.swap(a, b);
            }
            let mut shuffled = GroupLog::empty(world.logs[0].group_id());
            shuffled.set_run_room(run, RUN_ROOM.bytes);
            shuffled.insert_all(entries);
            let again = shuffled.mls_chain(|h| {
                if world.invalid.contains(h) {
                    Verdict::Invalid
                } else {
                    Verdict::Valid
                }
            });
            assert_eq!(again, chain, "seed {seed}: another order, another chain");
            check_fork_rule(&world.logs[0], &chain);
            // Copies that let go of what is spare (Commits past their run among it) keep the
            // same chain.
            let mut trimmed = world.logs[0].clone();
            trimmed.set_spare_room(0, 0);
            trimmed.trim();
            let again = trimmed.mls_chain(|h| {
                if world.invalid.contains(h) {
                    Verdict::Invalid
                } else {
                    Verdict::Valid
                }
            });
            assert_eq!(again.links, chain.links, "seed {seed}: trimmed");
            assert_eq!(again.counted, chain.counted, "seed {seed}: trimmed");
            if run < RUN_ROOM.entries {
                past_run += world.past_run;
                // Every device writes a whole run again from here, as members that have
                // all met do before long.
                for log in &mut world.logs {
                    log.set_run_room(RUN_ROOM.entries, RUN_ROOM.bytes);
                }
            }
            // Once everyone meets and the managers do their part, the tree holds exactly the
            // members.
            let mut rounds = 0;
            loop {
                world.sync_all();
                let log = &world.logs[0];
                let (state, chain) = (log.state(), world.chain(0));
                if chain.to_add(&state).is_empty() && chain.to_remove(&state).is_empty() {
                    assert_eq!(
                        chain.roster(),
                        state.members.keys().copied().collect(),
                        "seed {seed}"
                    );
                    break;
                }
                rounds += 1;
                assert!(
                    rounds < 5,
                    "seed {seed}: the tree never matched the members"
                );
                world.time += MIN_UPDATE_GAP_MS;
                let committer = (0..world.devices.len()).find(|i| {
                    let me = world.devices[*i].public_key();
                    state.is_manager(&me)
                        && (chain.links.is_empty() || chain.roster().contains(&me))
                });
                world.commit_duty(committer.expect("a manager in the tree"), false);
            }
            for i in 1..world.logs.len() {
                assert_eq!(world.chain(i), world.chain(0), "seed {seed}");
            }
        }
        eprintln!("{past_run} Commits refused past their run");
        if seeds >= 8 {
            assert!(past_run > 0, "no Commit went past its run");
        }
    }
}
