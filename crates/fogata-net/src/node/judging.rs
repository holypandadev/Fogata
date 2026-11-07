//! Logs taken whole, judged away from the node's loop. A group a friend offers, a direct
//! chat's group a friend offers, a removal shown to this device by a member it no longer
//! verifies and the log a manager lets a joining device in with are each judged at once
//! ([`fogata_state::GroupLog::insert_all`]): every entry signed on parents nobody else is
//! signed on is judged on the group worked out for those parents, about as costly as
//! working out the whole group. 4096 entries written to cost the most (an offer's bound)
//! take 12 to 13 seconds in a release build; on the loop, nothing would reach this device or
//! leave it meanwhile.
//!
//! - Each is checked first for whatever costs little, on the loop, as before: its bounds,
//!   the sender's entry budget, a removal's signature.
//! - Then it is judged on a blocking thread, on a log of its own or a copy of the group's,
//!   so nothing the loop does meanwhile changes what is judged, and a job whose verdict is
//!   never taken (the node stopped, the group went) leaves nothing behind: the copy goes
//!   with it.
//! - What it brings is taken on the loop once it is judged, as if it had come then: a
//!   group that runs here by then takes the entries through the bounded path, as any batch
//!   from a member ([`fogata_state::GroupLog::insert_some`]), so they merge with whatever
//!   it took meanwhile. A removal or a joining device's log is taken whole, the judged copy
//!   in place of the log, when nothing came meanwhile that the copy lacks
//!   ([`crate::group::GroupContext::take_judged`]); otherwise through the bounded path.
//! - [`MAX_JUDGING`] jobs run at once and [`MAX_WAITING`] wait, a few of each peer; a job
//!   past them is refused, as an offer past the sender's budget is, and comes again later.
//!   An offer or a removal again from the same peer for the same group replaces the one
//!   still waiting; another peer's waits as a job of its own.

use std::collections::VecDeque;

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_state::{Entry, EntryHash, GroupLog};
use libp2p::PeerId;
use tokio::sync::mpsc;

use super::Node;
use crate::group::{AdmittedTrial, RemovalTrial};

/// Jobs judged at once, each on a thread of its own.
pub(super) const MAX_JUDGING: usize = 2;
/// Jobs waiting for their turn, from everyone together, and from one peer.
pub(super) const MAX_WAITING: usize = 8;
const MAX_WAITING_PER_PEER: usize = 2;
/// Bytes of the jobs waiting, from everyone together: an offer holds up to 4 MiB.
const MAX_WAITING_BYTES: usize = 16 << 20;

/// What is judged.
pub(super) enum Job {
    /// A group a friend offered that does not run here.
    Group {
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
        declined: Option<EntryHash>,
    },
    /// A direct chat's group a friend offered that does not run here.
    Direct {
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
    },
    /// A removal of this device, shown by a peer that is no member here.
    Removal(RemovalTrial),
    /// The log a manager let this joining device in with.
    Admitted {
        trial: AdmittedTrial,
        entries: Vec<Vec<u8>>,
    },
}

impl Job {
    fn kind(&self) -> u8 {
        match self {
            Job::Group { .. } => 0,
            Job::Direct { .. } => 1,
            Job::Removal(_) => 2,
            Job::Admitted { .. } => 3,
        }
    }

    fn bytes(&self) -> usize {
        match self {
            Job::Group { entries, .. }
            | Job::Direct { entries, .. }
            | Job::Admitted { entries, .. } => entries.iter().map(Vec::len).sum(),
            Job::Removal(trial) => trial.bytes(),
        }
    }

    /// Judges it: the costly part.
    fn judge(self, group: GroupId) -> Verdict {
        match self {
            Job::Group {
                by,
                entries,
                declined,
            } => {
                let log = whole(group, &entries);
                Verdict::Group {
                    by,
                    entries,
                    declined,
                    log,
                }
            }
            Job::Direct { by, entries } => {
                let log = whole(group, &entries);
                Verdict::Direct { by, entries, log }
            }
            Job::Removal(trial) => Verdict::Removal(trial.judge()),
            Job::Admitted { trial, entries } => Verdict::Admitted {
                log: trial.judge(),
                entries,
            },
        }
    }
}

/// `entries` as a log of their own, with its group worked out, so that reading it on the
/// loop costs nothing.
fn whole(group: GroupId, entries: &[Vec<u8>]) -> GroupLog {
    let mut log = GroupLog::empty(group);
    log.insert_all(entries.iter().filter_map(|bytes| Entry::decode(bytes).ok()));
    drop(log.state());
    log
}

/// What a job came to.
pub(super) enum Verdict {
    Group {
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
        declined: Option<EntryHash>,
        log: GroupLog,
    },
    Direct {
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
        log: GroupLog,
    },
    /// The entries to take, if they remove this device, and the log with them.
    Removal(Option<(Vec<Vec<u8>>, GroupLog)>),
    /// The log with the entries sent, if it is the invited group's.
    Admitted {
        log: Option<GroupLog>,
        entries: Vec<Vec<u8>>,
    },
    /// Judging it failed (a panic): the peer that sent it is let go.
    Failed,
}

/// A job judged: from whom, for which group, and what it came to.
pub(super) struct Judged {
    pub(super) peer: PeerId,
    pub(super) group: GroupId,
    pub(super) verdict: Verdict,
}

/// One job, with what it is told apart by.
struct Waiting {
    peer: PeerId,
    group: GroupId,
    job: Job,
}

/// The jobs waiting, those being judged, and their verdicts on the way back to the loop.
pub(super) struct Judging {
    waiting: VecDeque<Waiting>,
    /// The peers of the jobs being judged, one entry each.
    running: Vec<PeerId>,
    done_tx: mpsc::Sender<Judged>,
    done: mpsc::Receiver<Judged>,
}

impl Default for Judging {
    fn default() -> Self {
        // A job sends its verdict once, and is counted as running until the loop takes it:
        // there is always room.
        let (done_tx, done) = mpsc::channel(MAX_JUDGING);
        Self {
            waiting: VecDeque::new(),
            running: Vec::new(),
            done_tx,
            done,
        }
    }
}

impl Judging {
    /// Whether a job from `peer` would be taken now (see [`Self::add`]), checked before
    /// what it costs the sender is charged.
    pub(super) fn has_room(&self, peer: PeerId) -> bool {
        self.waiting.len() < MAX_WAITING
            && self.waiting.iter().filter(|w| w.peer == peer).count() < MAX_WAITING_PER_PEER
    }

    /// Adds a job of `peer` for `group`, and starts it if its turn came. One still waiting
    /// from the same peer, of the same kind for the same group, is replaced in its place in
    /// line: the newer holds what that one did. Another peer's is not, a removal of this
    /// device included: what one peer shows (a bare removal, without the entries before
    /// it) may remove nothing where another's (the whole) does, and the peer that showed
    /// the whole was told it was taken and does not show it again. Returns whether it was
    /// taken; one refused changes nothing.
    pub(super) fn add(&mut self, peer: PeerId, group: GroupId, job: Job) -> bool {
        let same = |w: &Waiting| w.peer == peer && w.group == group && w.job.kind() == job.kind();
        let bytes: usize = self.waiting.iter().map(|w| w.job.bytes()).sum();
        if let Some(older) = self.waiting.iter_mut().find(|w| same(w)) {
            if bytes - older.job.bytes() + job.bytes() > MAX_WAITING_BYTES {
                return false;
            }
            *older = Waiting { peer, group, job };
            return true;
        }
        if !self.has_room(peer) || bytes + job.bytes() > MAX_WAITING_BYTES {
            return false;
        }
        self.waiting.push_back(Waiting { peer, group, job });
        self.start();
        true
    }

    /// Starts the jobs waiting while fewer than [`MAX_JUDGING`] run.
    fn start(&mut self) {
        while self.running.len() < MAX_JUDGING {
            let Some(Waiting { peer, group, job }) = self.waiting.pop_front() else {
                return;
            };
            self.running.push(peer);
            let done = self.done_tx.clone();
            tokio::task::spawn_blocking(move || {
                let verdict =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.judge(group)))
                        .unwrap_or(Verdict::Failed);
                // Fails only once the node is gone, which no longer wants it.
                let _ = done.try_send(Judged {
                    peer,
                    group,
                    verdict,
                });
            });
        }
    }

    /// The next job judged, for the node's loop to wait on.
    pub(super) async fn next(&mut self) -> Judged {
        match self.done.recv().await {
            Some(judged) => judged,
            // The sender kept here never goes.
            None => std::future::pending().await,
        }
    }

    /// Counts `judged`, taken by the loop, as no longer running, and starts the next.
    pub(super) fn finished(&mut self, judged: &Judged) {
        if let Some(at) = self.running.iter().position(|p| *p == judged.peer) {
            self.running.swap_remove(at);
        }
        self.start();
    }

    /// Jobs waiting or being judged.
    #[cfg(test)]
    pub(super) fn busy(&self) -> usize {
        self.waiting.len() + self.running.len()
    }
}

impl Node {
    /// Takes what a job judged away from the loop brings (see the module's doc).
    pub(super) fn on_judged(&mut self, judged: Judged) {
        let Judged {
            peer,
            group,
            verdict,
        } = judged;
        match verdict {
            Verdict::Group {
                by,
                entries,
                declined,
                log,
            } => self.group_offer_judged(peer, by, entries, declined, log),
            Verdict::Direct { by, entries, log } => {
                self.direct_offer_judged(peer, by, entries, log)
            }
            Verdict::Removal(Some((entries, log))) => {
                self.removal_judged(peer, group, entries, log)
            }
            Verdict::Removal(None) => {}
            Verdict::Admitted { log, entries } => {
                let taken = log.is_some();
                self.admitted_judged(group, peer, entries, taken, log)
            }
            Verdict::Failed => {
                tracing::error!(
                    %peer,
                    "judging a log a peer sent failed with an internal error; the peer is \
                     disconnected"
                );
                let _ = self.swarm.disconnect_peer_id(peer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use fogata_crypto::DeviceIdentity;
    use fogata_proto::MemberRequest;
    use fogata_state::Body;

    use super::*;
    use crate::{
        NodeConfig,
        group::GroupSetup,
        node::{start, tests::inspect},
    };

    fn direct() -> Job {
        Job::Direct {
            by: DeviceIdentity::generate().unwrap().public_key(),
            entries: Vec::new(),
        }
    }

    /// So many run at once and so many wait, a few of each peer; a newer job of a peer for
    /// the same group takes the place of the one waiting; past that, jobs are refused until
    /// those running are taken and the ones waiting start.
    #[tokio::test(flavor = "multi_thread")]
    async fn jobs_wait_their_turn_within_bounds() {
        let mut judging = Judging::default();
        let group = GroupId::random().unwrap();
        for _ in 0..MAX_JUDGING {
            assert!(judging.add(PeerId::random(), group, direct()));
        }
        assert_eq!(judging.running.len(), MAX_JUDGING);
        let leo = PeerId::random();
        assert!(judging.add(leo, group, direct()));
        assert!(judging.add(leo, group, direct()));
        assert_eq!(
            judging.waiting.len(),
            1,
            "the newer one replaces the one waiting"
        );
        assert!(judging.add(leo, GroupId::random().unwrap(), direct()));
        assert!(!judging.has_room(leo));
        assert!(!judging.add(leo, GroupId::random().unwrap(), direct()));
        while judging.waiting.len() < MAX_WAITING {
            assert!(judging.add(PeerId::random(), group, direct()));
        }
        assert!(!judging.add(PeerId::random(), group, direct()));
        assert_eq!(judging.busy(), MAX_JUDGING + MAX_WAITING);
        // Every one is judged and taken in turn.
        for left in (0..MAX_JUDGING + MAX_WAITING).rev() {
            let judged = judging.next().await;
            assert!(matches!(judged.verdict, Verdict::Direct { .. }));
            judging.finished(&judged);
            assert_eq!(judging.busy(), left);
        }
    }

    /// A removal of this device that a member shows with the entries before it is not
    /// pushed out of line by the same removal another peer shows bare: that one alone
    /// removes nothing here, and the member that showed the whole was told it was taken.
    /// The other peer's waits as a job of its own, within its own share.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_removal_waiting_is_not_replaced_by_another_peer_s() {
        use std::sync::Arc;

        use crate::group::{GroupContext, GroupSetup};

        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let leo = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, leo.public_key(), "leo", 2).unwrap();
        let group = log.group_id();
        let at_leo = GroupContext::new(leo.clone(), GroupSetup::Member(log.clone()));
        let rename = log
            .append(&ana, fogata_state::Body::Rename { name: "h".into() }, 3)
            .unwrap();
        let removal = log.remove(&ana, leo.public_key(), 4).unwrap();
        let whole = [rename.encode(), removal.encode()];
        let bare = [removal.encode()];
        let mut judging = Judging::default();
        for _ in 0..MAX_JUDGING {
            assert!(judging.add(PeerId::random(), group, direct()));
        }
        let (shown_by, bare_by) = (PeerId::random(), PeerId::random());
        let trial = |entries: &[Vec<u8>]| Job::Removal(at_leo.removal_trial(entries).unwrap());
        assert!(judging.add(shown_by, group, trial(&whole)));
        assert!(judging.add(bare_by, group, trial(&bare)));
        let waiting: Vec<PeerId> = judging.waiting.iter().map(|w| w.peer).collect();
        assert_eq!(waiting, [shown_by, bare_by]);
        // A newer one of the other peer's replaces its own.
        assert!(judging.add(bare_by, group, trial(&bare)));
        assert_eq!(judging.waiting.len(), 2);
        let mut removed_by = Vec::new();
        while judging.busy() > 0 {
            let judged = judging.next().await;
            if let Verdict::Removal(Some(_)) = judged.verdict {
                removed_by.push(judged.peer);
            }
            judging.finished(&judged);
        }
        assert_eq!(removed_by, [shown_by]);
    }

    /// A removal of this device shown by a peer that is no member here, with as many
    /// entries along as it may bring, each costly to judge (two members writing apart, every
    /// entry on parents no other is signed on): the node answers meanwhile, the removal is
    /// judged away from its loop, and taken once it is.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_removal_costly_to_judge_is_taken_while_the_node_goes_on_answering() {
        const ANSWERS_WITHIN: Duration = Duration::from_millis(250);
        let ana = DeviceIdentity::generate().unwrap();
        let leo = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, leo.public_key(), "leo", 2).unwrap();
        let group = log.group_id();
        // A long history first, which every entry along is judged on.
        let mut history = Vec::new();
        let mut parents = log.heads();
        for i in 0..1500 {
            let body = Body::Rename {
                name: format!("h{i}"),
            };
            let entry = Entry::sign(&ana, group, parents, 3 + i, body);
            parents = vec![entry.hash()];
            history.push(entry);
        }
        log.insert_all(history);
        let at_leo = log.clone();
        let mut written = Vec::new();
        let mut last = [log.heads(), log.heads()];
        for i in 0..crate::group::MAX_REMOVAL_ENTRIES - 1 {
            let by: &DeviceIdentity = [&ana, &leo][i % 2];
            let body = Body::Rename {
                name: format!("n{i}"),
            };
            let entry = Entry::sign(by, group, last[i % 2].clone(), 2_000 + i as u64, body);
            last[i % 2] = vec![entry.hash()];
            written.push(entry);
        }
        log.insert_all(written);
        log.remove(&ana, leo.public_key(), 3_000).unwrap();
        let shown: Vec<Vec<u8>> = log
            .entries_in_order()
            .into_iter()
            .filter(|e| at_leo.get(&e.hash()).is_none())
            .map(Entry::encode)
            .collect();
        assert_eq!(shown.len(), crate::group::MAX_REMOVAL_ENTRIES);
        // What judging it costs: what the loop would have been held up for.
        let ctx = crate::group::GroupContext::new(leo.clone(), GroupSetup::Member(at_leo.clone()));
        let started = Instant::now();
        assert!(ctx.my_removal(&shown).is_some());
        let judging = started.elapsed();
        assert!(judging > 4 * ANSWERS_WITHIN, "judged in {judging:?}");
        let (node, _events) = start(NodeConfig {
            identity: leo,
            groups: vec![GroupSetup::Member(at_leo)],
            friends: None,
            listen_port: 0,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers: Vec::new(),
            remembered: Vec::new(),
            hidden: false,
        })
        .unwrap();
        let push = MemberRequest::Push {
            group_id: group.as_bytes().to_vec(),
            entries: shown.into_iter().map(Into::into).collect(),
        };
        let started = Instant::now();
        let answer = inspect(&node, move |n| n.on_request(PeerId::random(), push)).await;
        let mut slowest = started.elapsed();
        assert!(
            matches!(answer, fogata_proto::MemberResponse::Ack),
            "{answer:?}"
        );
        let deadline = Instant::now() + Duration::from_secs(60) + 20 * judging;
        loop {
            let started = Instant::now();
            let member = inspect(&node, move |n| {
                n.groups.get(&group).is_some_and(|s| s.ctx.i_am_member())
            })
            .await;
            slowest = slowest.max(started.elapsed());
            if !member {
                break;
            }
            assert!(Instant::now() < deadline, "the removal was never taken");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            slowest < ANSWERS_WITHIN,
            "the node took {slowest:?} to answer while judging a removal of {judging:?}"
        );
        node.shutdown().await.unwrap();
    }
}
