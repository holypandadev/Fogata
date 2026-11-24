//! Groups that moved to a new log (a `Moved` entry, from level five; see
//! [`fogata_state::Moved`]): the first member moving a full group, members following it,
//! and what the old log keeps doing meanwhile.
//!
//! - The first member writes the new log and the `Moved` entry on its live copy
//!   ([`Node::move_group`]), runs the new group at once and sends the entry to the members,
//!   as any other.
//! - A member whose copy applies a move asks a member of the old group for the new log's
//!   start ([`MemberRequest::MovedLog`]), the entries before and with the one the move names,
//!   one member at a time, of those at capability level five. Judged away from the loop
//!   (see judging.rs), the start is taken only if it gives the old group as the move saw it
//!   ([`fogata_state::GroupLog::follows`]): the node then runs the new group, which brings
//!   the rest of its log as any group does, and tells the application
//!   ([`NodeEvent::GroupMoved`]). A start that is not that group is the mover's own doing
//!   (what the move names cannot be changed on the way): it is refused for good and told
//!   once ([`NodeEvent::MoveRefused`]). A member that answers with nothing usable is not
//!   asked again until every other was.
//! - The old group keeps running, writing nothing, so its log reaches members that were
//!   away and this device answers their asks for the new log; the application stops it
//!   after a while.
//! - The first member writes again in the new log who entries written apart from the move
//!   took out or brought in ([`fogata_state::GroupLog::again_after_move`]), each time the
//!   old log changes.

use std::{collections::HashSet, time::Instant};

use fogata_crypto::GroupId;
use fogata_proto::{MemberRequest, MemberResponse};
use fogata_state::{Entry, GroupLog, Level, NOT_THE_SAME_GROUP};
use libp2p::PeerId;

use super::{Node, Pending, judging::Job, members::group_of, now_ms};
use crate::{NodeEvent, group::GroupSetup};

/// Most entries, and bytes, of a new log's start taken from members: those of a group a
/// friend offers, which every version takes. A start holds a few entries for every member
/// and the group's picture.
const MAX_START_ENTRIES: usize = 4096;
const MAX_START_BYTES: usize = 4 * 1024 * 1024;

/// An ask for a new log's start with no answer for this long is given up, and another
/// member asked.
const ASK_FOR: std::time::Duration = std::time::Duration::from_secs(120);

/// Following a group's move, kept with the old group.
#[derive(Default)]
pub(super) struct Following {
    /// The member asked now, and what it gave so far.
    asking: Option<Asking>,
    /// The start is being judged (see judging.rs).
    judging: bool,
    /// Members that gave nothing usable: asked again only once every other one was.
    tried: HashSet<PeerId>,
    /// The new log is not the group as the move saw it: refused for good, and told once.
    refused: bool,
    /// The old log's heads (their digest) when the first member last wrote again in the new
    /// log what was written apart from the move: done again only once they change.
    again_for: Option<Vec<u8>>,
}

struct Asking {
    peer: PeerId,
    to: GroupId,
    /// The entry the next part starts at.
    from: u32,
    entries: Vec<Vec<u8>>,
    bytes: usize,
    since: Instant,
}

/// What judging a new log's start came to.
pub(super) enum StartVerdict {
    /// It is the group as the move saw it: the new log, made of it.
    Follows(Box<GroupLog>),
    /// It is not: the mover's own doing.
    NotTheGroup,
    /// It lacks what the move names: the member that sent it gave nothing usable.
    Unusable,
}

/// Judges the start `entries` of the log `old` moved to (see the module's doc).
pub(super) fn judge_start(old: &GroupLog, to: GroupId, entries: &[Vec<u8>]) -> StartVerdict {
    let mut new = GroupLog::empty(to);
    new.insert_all(entries.iter().filter_map(|bytes| Entry::decode(bytes).ok()));
    match old.follows(&new) {
        Ok(()) => {
            drop(new.state());
            StartVerdict::Follows(Box::new(new))
        }
        Err(NOT_THE_SAME_GROUP) => StartVerdict::NotTheGroup,
        Err(_) => StartVerdict::Unusable,
    }
}

impl Node {
    /// Moves `group` to a new log, as its first member (see the module's doc). Returns the
    /// new group's id.
    pub(super) fn move_group(&mut self, group: GroupId) -> Result<GroupId, String> {
        let slot = self.groups.get_mut(&group).ok_or("no such group")?;
        let (new, moved) = slot.ctx.move_group(now_ms())?;
        let to = new.group_id();
        self.after_group_change(group, vec![moved], None);
        self.add_group(GroupSetup::Member(new.clone()));
        self.emit(NodeEvent::GroupMoved {
            from: group,
            log: new,
        });
        Ok(to)
    }

    /// Answers a member's ask for the start of the log its group moved to.
    pub(super) fn on_moved_log_request(
        &mut self,
        peer: PeerId,
        group_id: Vec<u8>,
        from: u32,
    ) -> MemberResponse {
        let Some(group) = group_of(&group_id) else {
            return MemberResponse::Refused;
        };
        if !self.is_member_of(&group, &peer) {
            return MemberResponse::Refused;
        }
        let Some((_, moved)) = self.groups.get(&group).and_then(|s| s.ctx.moved()) else {
            return MemberResponse::Refused;
        };
        if !self.groups.contains_key(&moved.to) {
            return MemberResponse::Refused;
        }
        let Some(device) = self.may_serve(peer) else {
            return MemberResponse::Refused;
        };
        let (entries, more) =
            self.groups[&moved.to]
                .ctx
                .start_for_move(&moved.setup, &device, from);
        if entries.is_empty() {
            return MemberResponse::Refused;
        }
        self.charge_served(device, entries.iter().map(Vec::len).sum());
        MemberResponse::MovedLog {
            entries: entries.into_iter().map(Into::into).collect(),
            more,
        }
    }

    /// For each group here that moved: asks a member for the new log while this device
    /// does not run it, and as the first member of the new log, writes again there what
    /// was written apart from the move. At each heartbeat, and when a group's log changes.
    pub(super) fn follow_moves(&mut self) {
        let moved: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, s)| s.ctx.moved().is_some())
            .map(|(g, _)| *g)
            .collect();
        for group in moved {
            self.follow_move(group);
        }
    }

    /// [`Self::follow_moves`] for one group, when it moved.
    pub(super) fn follow_move(&mut self, group: GroupId) {
        let Some((_, moved)) = self.groups.get(&group).and_then(|s| s.ctx.moved()) else {
            return;
        };
        if self.groups.contains_key(&moved.to) {
            self.again_after_move(group, moved.to);
        } else {
            self.ask_for_moved_log(group, moved.to);
        }
    }

    /// See [`Self::follow_moves`].
    fn again_after_move(&mut self, group: GroupId, to: GroupId) {
        let Some(old) = self.groups.get_mut(&group) else {
            return;
        };
        let heads = old.ctx.heads_digest();
        if old.following.again_for.as_ref() == Some(&heads) {
            return;
        }
        old.following.again_for = Some(heads);
        // The new group's slot out of the map for a moment, so the old log is read where it
        // is, not copied: nothing else runs meanwhile.
        let Some(mut new) = self.groups.remove(&to) else {
            return;
        };
        let written = new
            .ctx
            .again_after_move(self.groups[&group].ctx.log(), now_ms());
        self.groups.insert(to, new);
        self.after_group_change(to, written, None);
    }

    /// See [`Self::follow_moves`].
    fn ask_for_moved_log(&mut self, group: GroupId, to: GroupId) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let following = &slot.following;
        if following.refused || following.judging {
            return;
        }
        if following
            .asking
            .as_ref()
            .is_some_and(|a| a.since.elapsed() < ASK_FOR)
        {
            return;
        }
        // Members whose copy judges at five: only they know the ask.
        let able: Vec<PeerId> = slot
            .members
            .iter()
            .filter(|p| {
                self.peer_level(p)
                    .is_some_and(|l| crate::caps::judges_at(l, Level::Five))
            })
            .copied()
            .collect();
        let slot = self.groups.get_mut(&group).expect("checked above");
        if let Some(asking) = slot.following.asking.take() {
            slot.following.tried.insert(asking.peer);
        }
        let mut untried = able.iter().filter(|p| !slot.following.tried.contains(*p));
        let Some(peer) = untried.next().copied() else {
            // Every one was asked: each may answer next time.
            slot.following.tried.clear();
            return;
        };
        slot.following.asking = Some(Asking {
            peer,
            to,
            from: 0,
            entries: Vec::new(),
            bytes: 0,
            since: Instant::now(),
        });
        self.request(
            peer,
            MemberRequest::MovedLog {
                group_id: group.as_bytes().to_vec(),
                from: 0,
            },
            Pending::MovedLog(group, 0),
        );
    }

    /// A member answered this device's ask for part `from` of the start of the log
    /// `group` moved to.
    pub(super) fn on_moved_log_answer(
        &mut self,
        peer: PeerId,
        group: GroupId,
        from: u32,
        response: MemberResponse,
    ) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(asking) = slot
            .following
            .asking
            .as_mut()
            .filter(|a| a.peer == peer && a.from == from)
        else {
            return;
        };
        let MemberResponse::MovedLog { entries, more } = response else {
            self.moved_log_lost(group, peer);
            return;
        };
        asking.bytes += entries.iter().map(|e| e.len()).sum::<usize>();
        asking
            .entries
            .extend(entries.into_iter().map(|e| e.into_vec()));
        let too_much = asking.entries.len() > MAX_START_ENTRIES || asking.bytes > MAX_START_BYTES;
        match more {
            _ if too_much => self.moved_log_lost(group, peer),
            // Each part takes the ask further, or it would never end.
            Some(next) if next > from => {
                asking.from = next;
                asking.since = Instant::now();
                self.request(
                    peer,
                    MemberRequest::MovedLog {
                        group_id: group.as_bytes().to_vec(),
                        from: next,
                    },
                    Pending::MovedLog(group, next),
                );
            }
            Some(_) => self.moved_log_lost(group, peer),
            None => {
                let Some(asking) = slot.following.asking.take() else {
                    return;
                };
                let job = Job::Moved {
                    old: slot.ctx.log().clone(),
                    to: asking.to,
                    entries: asking.entries,
                };
                if self.judging.add(peer, group, job) {
                    slot.following.judging = true;
                }
            }
        }
    }

    /// The ask for the new log of `group` to `peer` came to nothing (refused, lost, too
    /// much): another member is asked at the next heartbeat.
    pub(super) fn moved_log_lost(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        if slot
            .following
            .asking
            .as_ref()
            .is_some_and(|a| a.peer == peer)
        {
            slot.following.asking = None;
            slot.following.tried.insert(peer);
        }
    }

    /// Takes what judging the start `peer` sent of the log `group` moved to came to.
    pub(super) fn start_judged(&mut self, peer: PeerId, group: GroupId, verdict: StartVerdict) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        slot.following.judging = false;
        match verdict {
            // What the member sent does not let this device in yet: it was let into the old
            // group apart from the move, and the first member has not added it here again.
            StartVerdict::Follows(new) if !new.state().is_member(&slot.me) => {
                slot.following.tried.insert(peer);
            }
            StartVerdict::Follows(new) => {
                let to = new.group_id();
                // Still the move here, and the new group not run yet.
                let still = slot.ctx.moved().is_some_and(|(_, m)| m.to == to);
                if !still || self.groups.contains_key(&to) {
                    return;
                }
                tracing::info!("{group:?} moved to {to:?}: following it");
                self.add_group(GroupSetup::Member((*new).clone()));
                self.emit(NodeEvent::GroupMoved {
                    from: group,
                    log: *new,
                });
            }
            StartVerdict::NotTheGroup => {
                if !slot.following.refused {
                    slot.following.refused = true;
                    tracing::warn!(
                        "{group:?} moved to a log that is not the group as it was; staying"
                    );
                    self.emit(NodeEvent::MoveRefused { group });
                }
            }
            StartVerdict::Unusable => {
                slot.following.tried.insert(peer);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc, time::Duration};

    use fogata_crypto::{DeviceIdentity, DevicePublicKey};
    use fogata_state::Moved;

    use super::*;
    use crate::{
        NodeConfig, NodeEvents, NodeHandle, Path,
        caps::{CAPS_LEVEL, caps_protocol},
        node::{Member, start, tests::inspect},
    };

    fn node(identity: &Arc<DeviceIdentity>, log: &GroupLog) -> (NodeHandle, NodeEvents) {
        start(NodeConfig {
            identity: identity.clone(),
            groups: vec![GroupSetup::Member(log.clone())],
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

    /// `peer`, the device `device` at this version's level, a verified member of `group`.
    fn member(n: &mut Node, group: GroupId, peer: PeerId, device: DevicePublicKey) {
        n.members.insert(
            peer,
            Member {
                device,
                path: Path::Direct,
                address: "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap(),
                told: Path::Direct,
                worse_since: None,
            },
        );
        n.groups.get_mut(&group).unwrap().members.insert(peer);
        crate::locked(&n.advertised).insert(peer, HashSet::from([caps_protocol(CAPS_LEVEL)]));
    }

    /// Ana (first) and Bea, at level five.
    fn group() -> (Arc<DeviceIdentity>, Arc<DeviceIdentity>, GroupLog) {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "friends", 1).unwrap();
        log.add(&ana, bea.public_key(), "Bea", 2).unwrap();
        log.raise_level(&ana, Level::Five, 3).unwrap();
        (ana, bea, log)
    }

    /// Bea's node, holding `moved` (a copy of the group with its move): it asks Ana for the
    /// new log, and the ask is what it waits on.
    async fn bea_gets_the_move(
        bea: &NodeHandle,
        group: GroupId,
        ana_peer: PeerId,
        ana: DevicePublicKey,
        moved: GroupLog,
    ) {
        let asked = inspect(bea, move |n| {
            member(n, group, ana_peer, ana);
            n.add_group(GroupSetup::Member(moved));
            let asked = n.groups[&group].following.asking.as_ref().map(|a| a.peer);
            let pending = n.pending.values().any(|(p, k)| {
                *p == ana_peer && matches!(k, Pending::MovedLog(g, 0) if *g == group)
            });
            (asked, pending)
        })
        .await;
        assert_eq!(asked, (Some(ana_peer), true));
    }

    /// Bea's node takes `answer` from Ana to its ask (asked again first if a failed dial
    /// let the first go).
    async fn bea_takes(bea: &NodeHandle, group: GroupId, ana_peer: PeerId, answer: MemberResponse) {
        inspect(bea, move |n| {
            let slot = n.groups.get_mut(&group).unwrap();
            slot.following.tried.clear();
            let to = slot.ctx.moved().unwrap().1.to;
            slot.following.asking = Some(Asking {
                peer: ana_peer,
                to,
                from: 0,
                entries: Vec::new(),
                bytes: 0,
                since: Instant::now(),
            });
            n.on_response(ana_peer, Pending::MovedLog(group, 0), answer);
        })
        .await;
    }

    /// Waits until Bea's node runs `to`, or judging what she got came to nothing.
    async fn runs(node: &NodeHandle, group: GroupId, to: GroupId) -> bool {
        for _ in 0..200 {
            let (runs, judging) = inspect(node, move |n| {
                (
                    n.groups.contains_key(&to),
                    n.groups[&group].following.judging,
                )
            })
            .await;
            if runs || !judging {
                return runs;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("still judging");
    }

    fn moved_told(events: &mut NodeEvents) -> (Option<GroupId>, Option<GroupId>) {
        let (mut moved, mut refused) = (None, None);
        while let Ok(event) = events.try_recv() {
            match event {
                NodeEvent::GroupMoved { log, .. } => moved = Some(log.group_id()),
                NodeEvent::MoveRefused { group } => refused = Some(group),
                _ => {}
            }
        }
        (moved, refused)
    }

    /// The first member moves the group; another member that gets the move asks it for the
    /// new log, gets its start, judges it the group it was and runs it, as a member; it is
    /// told once. A peer that is no member, or asks about a group that did not move, gets
    /// nothing.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_follows_the_group_to_its_new_log() {
        let (ana, bea, log) = group();
        let x = log.group_id();
        let (ana_node, mut ana_events) = node(&ana, &log);
        let (bea_node, mut bea_events) = node(&bea, &log);
        let (ana_peer, bea_peer) = (PeerId::random(), PeerId::random());
        let bea_key = bea.public_key();
        let (to, moved, refused) = inspect(&ana_node, move |n| {
            member(n, x, bea_peer, bea_key);
            // Not moved yet: nothing to give.
            let ask = || MemberRequest::MovedLog {
                group_id: x.as_bytes().to_vec(),
                from: 0,
            };
            let before = n.on_request(bea_peer, ask());
            let to = n.move_group(x).unwrap();
            // Moved once only.
            assert!(n.move_group(x).is_err());
            let stranger = n.on_request(PeerId::random(), ask());
            let log = n.groups[&x].ctx.log().clone();
            (to, log, (before, stranger))
        })
        .await;
        assert!(matches!(refused.0, MemberResponse::Refused));
        assert!(matches!(refused.1, MemberResponse::Refused));
        assert_eq!(moved_told(&mut ana_events).0, Some(to));

        bea_gets_the_move(&bea_node, x, ana_peer, ana.public_key(), moved).await;
        let answer = inspect(&ana_node, move |n| {
            n.on_request(
                bea_peer,
                MemberRequest::MovedLog {
                    group_id: x.as_bytes().to_vec(),
                    from: 0,
                },
            )
        })
        .await;
        let MemberResponse::MovedLog { entries, more } = &answer else {
            panic!("no start");
        };
        assert!(!entries.is_empty());
        assert_eq!(*more, None);
        bea_takes(&bea_node, x, ana_peer, answer).await;
        assert!(runs(&bea_node, x, to).await);
        let (member, still) = inspect(&bea_node, move |n| {
            (
                n.groups[&to].ctx.i_am_member(),
                n.groups[&x].ctx.moved().map(|(_, m)| m.to),
            )
        })
        .await;
        assert!(member);
        // The old group runs on, for members that were away.
        assert_eq!(still, Some(to));
        assert_eq!(moved_told(&mut bea_events), (Some(to), None));
        // Followed once: a heartbeat asks for nothing more.
        let asked = inspect(&bea_node, move |n| {
            n.follow_moves();
            n.groups[&x].following.asking.is_some()
        })
        .await;
        assert!(!asked);
        ana_node.shutdown().await.unwrap();
        bea_node.shutdown().await.unwrap();
    }

    /// A start that lacks the entry the move names is the sender's fault: the member that
    /// sent it is asked only once every other one was, and the move still counts.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_start_that_lacks_its_last_entry_is_asked_for_elsewhere() {
        let (ana, bea, log) = group();
        let x = log.group_id();
        let (ana_node, _) = node(&ana, &log);
        let (bea_node, mut bea_events) = node(&bea, &log);
        let (ana_peer, bea_peer) = (PeerId::random(), PeerId::random());
        let bea_key = bea.public_key();
        let (to, moved, answer) = inspect(&ana_node, move |n| {
            member(n, x, bea_peer, bea_key);
            let to = n.move_group(x).unwrap();
            let answer = n.on_request(
                bea_peer,
                MemberRequest::MovedLog {
                    group_id: x.as_bytes().to_vec(),
                    from: 0,
                },
            );
            (to, n.groups[&x].ctx.log().clone(), answer)
        })
        .await;
        let MemberResponse::MovedLog { mut entries, more } = answer else {
            panic!("no start");
        };
        // The start is by time: its last entry is the one the move names.
        entries.pop();
        bea_gets_the_move(&bea_node, x, ana_peer, ana.public_key(), moved).await;
        bea_takes(
            &bea_node,
            x,
            ana_peer,
            MemberResponse::MovedLog { entries, more },
        )
        .await;
        assert!(!runs(&bea_node, x, to).await);
        let (tried, refused) = inspect(&bea_node, move |n| {
            let f = &n.groups[&x].following;
            (f.tried.contains(&ana_peer), f.refused)
        })
        .await;
        assert!(tried && !refused);
        assert_eq!(moved_told(&mut bea_events), (None, None));
        ana_node.shutdown().await.unwrap();
        bea_node.shutdown().await.unwrap();
    }

    /// A first member that moves the group to a log that is not the group as it was (by
    /// hand, past the checks of `move_to`): its members refuse it for good, are told once,
    /// and stay where they are.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_move_to_another_group_is_refused_for_good() {
        let (ana, bea, mut log) = group();
        let x = log.group_id();
        let (bea_node, mut bea_events) = node(&bea, &log);
        let mut other = GroupLog::create(&ana, "not friends", 10).unwrap();
        other.raise_level(&ana, Level::CURRENT, 11).unwrap();
        other.add(&ana, bea.public_key(), "Bea", 12).unwrap();
        let [setup] = other.heads()[..] else {
            unreachable!()
        };
        let body = Moved {
            to: other.group_id(),
            setup,
        }
        .body();
        log.append(&ana, body, 13).unwrap();
        assert!(log.moved().is_some());
        let ana_peer = PeerId::random();
        bea_gets_the_move(&bea_node, x, ana_peer, ana.public_key(), log).await;
        let mut start: Vec<Entry> = other.entries_in_order().into_iter().cloned().collect();
        start.sort_by_key(|e| (e.time_ms, e.hash()));
        let answer = MemberResponse::MovedLog {
            entries: start.iter().map(|e| e.encode().into()).collect(),
            more: None,
        };
        bea_takes(&bea_node, x, ana_peer, answer).await;
        assert!(!runs(&bea_node, x, other.group_id()).await);
        let refused = inspect(&bea_node, move |n| {
            let refused = n.groups[&x].following.refused;
            // Nothing more is asked.
            n.follow_moves();
            (refused, n.groups[&x].following.asking.is_none())
        })
        .await;
        assert_eq!(refused, (true, true));
        assert_eq!(moved_told(&mut bea_events), (None, Some(x)));
        bea_node.shutdown().await.unwrap();
    }

    /// What a member wrote in the old log apart from the move (Bea adds Dan on her copy
    /// before she gets the move) reaches the first member, which adds Dan again in the new
    /// log, once; the old group writes nothing at its heartbeats.
    #[tokio::test(flavor = "multi_thread")]
    async fn the_first_member_writes_again_what_was_written_apart() {
        let (ana, bea, log) = group();
        let x = log.group_id();
        let mut beas = log.clone();
        let dan = DeviceIdentity::generate().unwrap().public_key();
        let add = beas.add(&bea, dan, "Dan", 5).unwrap();
        let (ana_node, _) = node(&ana, &log);
        let bea_peer = PeerId::random();
        let bea_key = bea.public_key();
        let (in_new, again, quiet) = inspect(&ana_node, move |n| {
            member(n, x, bea_peer, bea_key);
            let to = n.move_group(x).unwrap();
            let pushed = n.on_request(
                bea_peer,
                MemberRequest::Push {
                    group_id: x.as_bytes().to_vec(),
                    entries: vec![add.encode().into()],
                },
            );
            assert!(matches!(pushed, MemberResponse::Ack));
            let in_new = n.groups[&to].ctx.log().state().is_member(&dan);
            let len = n.groups[&to].ctx.log().len();
            n.follow_moves();
            let again = n.groups[&to].ctx.log().len() == len;
            let heads = n.groups[&x].ctx.heads();
            n.member_heartbeat();
            (in_new, again, n.groups[&x].ctx.heads() == heads)
        })
        .await;
        assert!(in_new, "Dan is added again in the new log");
        assert!(again, "and only once");
        assert!(quiet, "the old group writes nothing");
        ana_node.shutdown().await.unwrap();
    }

    /// A device let into the old group apart from the move (Bea adds Dan before she gets
    /// it) follows too, once the first member added it again in the new log: what it is sent
    /// is the start and the rest after it, so it is a member of what it takes.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_device_let_in_apart_from_the_move_follows_once_added_again() {
        let (ana, bea, log) = group();
        let x = log.group_id();
        let dan = Arc::new(DeviceIdentity::generate().unwrap());
        let mut beas = log.clone();
        beas.add(&bea, dan.public_key(), "Dan", 5).unwrap();
        let (ana_node, _) = node(&ana, &log);
        let (dan_peer, ana_peer) = (PeerId::random(), PeerId::random());
        let (dan_key, beas_log) = (dan.public_key(), beas.clone());
        let (to, moved, answer) = inspect(&ana_node, move |n| {
            let to = n.move_group(x).unwrap();
            // Bea's copy, apart from the move, reaches Ana.
            n.add_group(GroupSetup::Member(beas_log));
            member(n, x, dan_peer, dan_key);
            let answer = n.on_request(
                dan_peer,
                MemberRequest::MovedLog {
                    group_id: x.as_bytes().to_vec(),
                    from: 0,
                },
            );
            (to, n.groups[&x].ctx.log().clone(), answer)
        })
        .await;
        let (dan_node, mut dan_events) = node(&dan, &beas);
        bea_gets_the_move(&dan_node, x, ana_peer, ana.public_key(), moved).await;
        bea_takes(&dan_node, x, ana_peer, answer).await;
        assert!(runs(&dan_node, x, to).await);
        let member = inspect(&dan_node, move |n| n.groups[&to].ctx.i_am_member()).await;
        assert!(member);
        assert_eq!(moved_told(&mut dan_events), (Some(to), None));
        ana_node.shutdown().await.unwrap();
        dan_node.shutdown().await.unwrap();
    }
}
