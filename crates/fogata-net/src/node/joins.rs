//! Joining a group with an invite: the signed join sent to the managers, a manager's answer,
//! the log sent to a device let in part after part, refusals that go along with a revoke,
//! and the log this device was sent when it was let in, judged away from the loop.

use std::collections::{BTreeMap, VecDeque};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_proto::MemberRequest;
use fogata_state::{GroupLog, Ticket};
use libp2p::PeerId;

use super::{BehindTold, Node, Pending, judging::Job, now_ms};
use crate::{NodeEvent, caps, group::NotLetIn, short_peer};

/// Times a part of a log that did not arrive is sent again.
const PART_TRIES: u8 = 3;
/// Most parts, and bytes, of a long log taken from one manager that let this device in.
pub(super) const MAX_ADMITTED_PARTS: u32 = 64;
pub(super) const MAX_ADMITTED_BYTES: usize = 16 * 1024 * 1024;
/// Most bytes of logs in parts held at once, from every peer of every group being joined:
/// any peer found under an invite's key may say it let this device in, so what they send
/// is bounded as a whole too.
pub(super) const MAX_ADMITTED_BYTES_ALL: usize = 2 * MAX_ADMITTED_BYTES;

/// Whether a device whose connection lists capability level `listed` judges at `level`, a
/// group's: a copy below it refuses the `Level` entry that raised the group, and waits on
/// everything signed after it, the `Welcome` that would let it in included. Such a device
/// never gets in, while the group counts it a member and gives it its keys, so it is not let
/// in. A group no `Level` entry raised is judged alike by every copy. `None` when its
/// connection has not shown what it speaks (see `Node::shown_level`): such a device is let in
/// as before, so that none that would get in is kept out.
///
/// Only what this manager does is decided here; what any copy takes of the log is not.
pub(super) fn judges_level(level: fogata_state::Level, listed: Option<u32>) -> Option<bool> {
    if level <= fogata_state::Level::default() {
        return Some(true);
    }
    listed.map(|listed| caps::judges_at(listed, level))
}

/// Whether a log a manager let this device in with carries a `Level` entry, signed, that
/// raises the group above every level this version judges at: why it could not be taken,
/// and only an update of Fogata lets this device in. A manager's word only (nothing of the
/// group is known yet), so it is only told; the join goes on.
pub(super) fn raised_past_this_version(entries: &[Vec<u8>]) -> bool {
    entries
        .iter()
        .filter_map(|bytes| fogata_state::Entry::decode(bytes).ok())
        .filter(|entry| {
            matches!(entry.body, fogata_state::Body::Level { level }
                if fogata_state::Level::numbered(level).is_none()
                    && level > fogata_state::Level::CURRENT.number())
        })
        .take(1)
        .any(|entry| entry.signature_valid())
}

impl Node {
    /// Manager: tells the application, once for `told`, that `device` (called `name`) must
    /// update Fogata before it can be let into `group`.
    pub(super) fn tell_behind(
        &mut self,
        group: GroupId,
        told: BehindTold,
        device: DevicePublicKey,
        name: String,
    ) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        tracing::debug!(
            "{} judges below the group's level: not let in",
            device.fingerprint()
        );
        if slot.behind_told.insert(told) {
            self.emit(NodeEvent::MustUpdateToJoin {
                group,
                device,
                name,
            });
        }
    }

    /// Joining device: shows our invite ticket to a peer that may be a member.
    pub(super) fn send_join(&mut self, peer: PeerId, group: GroupId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(ticket) = slot.ctx.joining().map(Ticket::encode) else {
            return;
        };
        if slot.join_sent.insert(peer) {
            // The manager lets in no copy too old for the group's level: it is told this
            // one's (see `judges_level`), which a device lists only to peers it chose.
            self.show_protocols(peer);
            self.request(
                peer,
                MemberRequest::JoinRequest { ticket },
                Pending::Join(group),
            );
        }
    }

    /// A joining device got the log from a manager that let it in. Whether it is the
    /// invited group's is judged on the whole log, away from the loop (see judging.rs), and
    /// it is taken once it is ([`Self::admitted_judged`]).
    pub(super) fn on_admitted(
        &mut self,
        group: GroupId,
        peer: PeerId,
        entries: Vec<serde_bytes::ByteBuf>,
    ) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let entries: Vec<Vec<u8>> = entries.into_iter().map(|e| e.into_vec()).collect();
        let Some(trial) = slot.ctx.admitted_trial(&entries) else {
            self.admitted_judged(group, peer, entries, true, None);
            return;
        };
        if !self
            .judging
            .add(peer, group, Job::Admitted { trial, entries })
        {
            // Asked again later, of this manager or another.
            if let Some(slot) = self.groups.get_mut(&group) {
                slot.join_sent.remove(&peer);
            }
        }
    }

    /// A part of a long log from a manager on `peer` that let this device in: kept until
    /// they are all here, then taken as a whole. Returns whether it was kept. A part sent
    /// again (its answer was lost) takes the place of the one held, and counts once; what
    /// one peer holds is bounded, and so is what all of them hold: past that, the peers
    /// holding the most give way first, and when that is not enough the part is refused.
    pub(super) fn take_admitted_part(
        &mut self,
        group: GroupId,
        peer: PeerId,
        index: u32,
        count: u32,
        entries: Vec<serde_bytes::ByteBuf>,
    ) -> bool {
        let size: usize = entries.iter().map(|e| e.len()).sum();
        let Some(slot) = self.groups.get_mut(&group) else {
            return false;
        };
        if count > MAX_ADMITTED_PARTS || index >= count {
            slot.parts_in.remove(&peer);
            return false;
        }
        let parts = slot.parts_in.entry(peer).or_default();
        if parts.count != count {
            *parts = super::PartsIn {
                count,
                ..Default::default()
            };
        }
        let replaced = parts
            .parts
            .get(&index)
            .map_or(0, |p| p.iter().map(|e| e.len()).sum());
        let held = parts.bytes - replaced + size;
        if held > MAX_ADMITTED_BYTES || !self.room_for_parts(group, peer, held) {
            if let Some(slot) = self.groups.get_mut(&group) {
                slot.parts_in.remove(&peer);
            }
            return false;
        }
        let slot = self.groups.get_mut(&group).expect("checked above");
        let parts = slot.parts_in.get_mut(&peer).expect("kept above");
        parts.parts.insert(index, entries);
        parts.bytes = held;
        if parts.parts.len() as u32 == count
            && let Some(parts) = slot.parts_in.remove(&peer)
        {
            let entries = parts.parts.into_values().flatten().collect();
            self.on_admitted(group, peer, entries);
        }
        true
    }

    /// Whether `peer` may hold `held` bytes of parts for `group`, within what all peers may
    /// hold together. When they would hold too much, those holding more than `peer` would
    /// are made to give way, the most first, so that no one peer keeps the others out.
    fn room_for_parts(&mut self, group: GroupId, peer: PeerId, held: usize) -> bool {
        let mut others: Vec<(usize, GroupId, PeerId)> = self
            .groups
            .iter()
            .flat_map(|(g, s)| {
                s.parts_in
                    .iter()
                    .map(move |(p, parts)| (parts.bytes, *g, *p))
            })
            .filter(|(_, g, p)| (*g, *p) != (group, peer))
            .collect();
        let mut total: usize = others.iter().map(|(b, _, _)| b).sum::<usize>() + held;
        others.sort_unstable_by_key(|o| std::cmp::Reverse(o.0));
        for (bytes, g, p) in others {
            if total <= MAX_ADMITTED_BYTES_ALL || bytes <= held {
                break;
            }
            tracing::debug!(
                "dropped {bytes} bytes of a log in parts from {}",
                short_peer(&p)
            );
            if let Some(slot) = self.groups.get_mut(&g) {
                slot.parts_in.remove(&p);
            }
            total -= bytes;
        }
        total <= MAX_ADMITTED_BYTES_ALL
    }

    /// Forgets the parts of logs held for joins that ended, and the managers they were
    /// asked of.
    pub(super) fn forget_ended_join(&mut self, group: GroupId) {
        if let Some(slot) = self
            .groups
            .get_mut(&group)
            .filter(|s| s.ctx.joining().is_none())
        {
            slot.parts_in.clear();
            slot.join_committed.clear();
        }
    }

    /// The log a manager on `peer` let this device in with, judged (`taken`: it is the
    /// invited group's, or it no longer had to be). `judged` is the group's log with it,
    /// taken whole when nothing came meanwhile; otherwise `entries` are taken as any batch,
    /// and the rest come with the next sync, once this device is in.
    pub(super) fn admitted_judged(
        &mut self,
        group: GroupId,
        peer: PeerId,
        entries: Vec<Vec<u8>>,
        taken: bool,
        judged: Option<GroupLog>,
    ) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        if !taken {
            tracing::debug!("{peer} sent a log that is not the invited group's");
            slot.join_sent.remove(&peer);
            slot.stop_join_with(&peer);
            if !slot.update_told && raised_past_this_version(&entries) {
                slot.update_told = true;
                self.emit(NodeEvent::UpdateToJoin { group });
            }
            return;
        }
        let was_judged = judged.is_some();
        let (new, refused) = match judged.and_then(|log| slot.ctx.take_judged(log)) {
            Some(new) => (new, BTreeMap::new()),
            None if was_judged && slot.ctx.joining().is_some() => {
                // Judged on the log as it was: with what was taken meanwhile, the entries
                // may let this device in as its invite does not allow (an `Add` waiting
                // here for a parent they bring). Not taken; the manager is asked again,
                // and its log is judged on this one.
                slot.join_sent.remove(&peer);
                return;
            }
            None => {
                let taken = slot.ctx.insert_encoded(&entries);
                (taken.new, taken.refused)
            }
        };
        self.refused_entries(&refused);
        self.forget_ended_join(group);
        let slot = self.groups.get_mut(&group).expect("checked above");
        if slot.ctx.joining().is_none() && slot.ctx.current().is_some() {
            let name = slot.ctx.name().to_string();
            self.provided.1 = None;
            self.emit(NodeEvent::GroupChanged(group));
            self.emit(NodeEvent::Joined { group, name });
            // Now a member: prove it to everyone we are connected to.
            let peers: Vec<PeerId> = self
                .interesting
                .iter()
                .filter(|p| self.swarm.is_connected(p))
                .copied()
                .collect();
            for p in peers {
                self.challenge(p, group);
            }
        } else {
            slot.join_sent.remove(&peer);
            self.after_group_change(group, new, Some(peer));
        }
    }

    /// A manager's answer to a device waiting to join with an invite: in, with the log and
    /// its key, or out, and the invite stops working.
    pub(super) fn answer_join(&mut self, group: GroupId, device: DevicePublicKey, accept: bool) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(waiting) = slot.join_waiting.remove(&device) else {
            return;
        };
        if accept {
            let ticket_id = waiting.ticket.ticket_id;
            // The group may have gone up a level since the device asked.
            if judges_level(slot.ctx.log().level(), waiting.level) == Some(false) {
                let name = match &waiting.entry.body {
                    fogata_state::Body::Join { name, .. } => fogata_crypto::names::tidy(name),
                    _ => String::new(),
                };
                let told = BehindTold::Invite(ticket_id);
                self.tell_behind(group, told, device, name);
                self.emit(NodeEvent::JoinAnswered { group, device });
                return;
            }
            let new = match slot.ctx.accept_join(waiting.entry, &ticket_id, now_ms()) {
                Ok(new) => new,
                Err(why) => {
                    self.info(match why {
                        NotLetIn::Invalid => "that invite no longer works, or the group is full",
                        NotLetIn::NoKeyYet => {
                            "this device has no group key yet: the new member gets in once it has one"
                        }
                        NotLetIn::WroteTooMuch => {
                            "this device wrote more than the others have seen yet: the new member \
                             can be let in once another member is online"
                        }
                        NotLetIn::SignedOnSpare => {
                            "that device's request was not made on this group as it is: it has to ask again"
                        }
                    });
                    self.emit(NodeEvent::JoinAnswered { group, device });
                    return;
                }
            };
            self.admit(group, waiting.peer);
            self.info(format!(
                "a new member joined with an invite ({})",
                short_peer(&waiting.peer)
            ));
            self.after_group_change(group, new, Some(waiting.peer));
        } else {
            let new = slot.ctx.revoke(waiting.ticket.ticket_id, now_ms());
            let revoke = slot.ctx.refusal_of(&waiting.ticket);
            self.send_refusal(waiting.peer, group, revoke);
            self.after_group_change(group, new, None);
        }
        self.emit(NodeEvent::JoinAnswered { group, device });
    }

    /// Sends a device let in the log: in one message, or, when it is long, in parts of at
    /// most `SYNC_BYTES` one after the other, each sent once the one before arrived, so that
    /// each arrives within a request's time also over a slow link. A message lost on the way
    /// (its connection closed while another stays, or no answer in time) is sent again, as
    /// the device asks no more while it is connected.
    pub(super) fn admit(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let mut parts: VecDeque<Vec<serde_bytes::ByteBuf>> = slot
            .ctx
            .encoded_parts()
            .into_iter()
            .map(|p| p.into_iter().map(serde_bytes::ByteBuf::from).collect())
            .collect();
        if parts.is_empty() {
            parts.push_back(Vec::new());
        }
        let count = parts.len() as u32;
        slot.admitting.insert(
            peer,
            super::Admitting {
                parts,
                next: 0,
                count,
                tries: 0,
            },
        );
        self.send_part(group, peer);
    }

    /// Sends the next part of the log to a device let in, if one is left: a log of one part
    /// goes whole, as a single message.
    fn send_part(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(admitting) = slot.admitting.get(&peer) else {
            return;
        };
        let Some(entries) = admitting.parts.front().cloned() else {
            slot.admitting.remove(&peer);
            return;
        };
        let part = (admitting.count > 1).then_some((admitting.next, admitting.count));
        self.request(
            peer,
            MemberRequest::Admitted {
                group_id: group.as_bytes().to_vec(),
                entries,
                part,
            },
            Pending::AdmittedPart(group),
        );
    }

    /// A device let in answered a part of the log: on to the next, or, refused, no more.
    pub(super) fn part_answered(&mut self, group: GroupId, peer: PeerId, taken: bool) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(admitting) = slot.admitting.get_mut(&peer) else {
            return;
        };
        if !taken {
            slot.admitting.remove(&peer);
            return;
        }
        admitting.parts.pop_front();
        admitting.next += 1;
        admitting.tries = 0;
        self.send_part(group, peer);
    }

    /// A part of the log did not reach a device let in: tried again a few times, then the
    /// device gets the log when it shows its invite again.
    pub(super) fn part_lost(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(admitting) = slot.admitting.get_mut(&peer) else {
            return;
        };
        admitting.tries += 1;
        if admitting.tries > PART_TRIES {
            slot.admitting.remove(&peer);
            return;
        }
        self.send_part(group, peer);
    }

    /// Tells a device that showed an invite that it was cancelled, with the `Revoke` that
    /// did it when there is one.
    pub(super) fn send_refusal(&mut self, peer: PeerId, group: GroupId, revoke: Option<Vec<u8>>) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        slot.refusing.insert(peer, (revoke, 0));
        self.send_refusal_again(peer, group);
    }

    /// Sends the refusal on its way to `peer` once more, if it was lost fewer than
    /// [`PART_TRIES`] times (as `Admitted`, see `admit`); after that the device learns it
    /// when it next shows its invite.
    pub(super) fn send_refusal_again(&mut self, peer: PeerId, group: GroupId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some((revoke, tries)) = slot.refusing.get_mut(&peer) else {
            return;
        };
        if *tries > PART_TRIES {
            slot.refusing.remove(&peer);
            return;
        }
        *tries += 1;
        let revoke = revoke.clone();
        self.request(
            peer,
            MemberRequest::JoinDeclined {
                group_id: group.as_bytes().to_vec(),
                revoke: revoke.map(serde_bytes::ByteBuf::from),
            },
            Pending::Refusal(group),
        );
    }

    /// Forgets joins waiting for an answer that are settled: the device got in through
    /// another manager, or its invite was used, cancelled or ran out. A device whose invite
    /// another manager cancelled is told so here.
    pub(super) fn forget_settled_joins(&mut self, group: GroupId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let settled: Vec<DevicePublicKey> = slot
            .join_waiting
            .iter()
            .filter(|(device, w)| {
                slot.ctx.is_member_device(device)
                    || !slot.ctx.ticket_acceptable(&w.ticket, now_ms())
            })
            .map(|(device, _)| *device)
            .collect();
        let mut refused = Vec::new();
        for device in &settled {
            let Some(waiting) = slot.join_waiting.remove(device) else {
                continue;
            };
            if !slot.ctx.is_member_device(device)
                && slot.ctx.ticket_revoked(&waiting.ticket.ticket_id)
            {
                refused.push((waiting.peer, slot.ctx.refusal_of(&waiting.ticket)));
            }
        }
        for (peer, revoke) in refused {
            self.send_refusal(peer, group, revoke);
        }
        for device in settled {
            self.emit(NodeEvent::JoinAnswered { group, device });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::{DeviceIdentity, GroupId};
    use fogata_proto::{MemberRequest, MemberResponse};
    use fogata_state::GroupLog;
    use libp2p::{PeerId, core::ConnectedPoint, swarm::ConnectionId};
    use serde_bytes::ByteBuf;

    use super::{MAX_ADMITTED_BYTES, MAX_ADMITTED_BYTES_ALL};
    use crate::{
        NodeConfig, NodeHandle,
        group::GroupSetup,
        node::{Node, Pending, start, tests::inspect},
    };

    /// A node joining a group with an invite, and the group.
    fn joining_node() -> (NodeHandle, GroupId) {
        let admin = DeviceIdentity::generate().unwrap();
        let log = GroupLog::create(&admin, "g", 1).unwrap();
        let ticket = log.issue_ticket(&admin, u64::MAX / 2).unwrap();
        let group = ticket.group_id;
        let (node, _events) = start(NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: vec![GroupSetup::Joining {
                group_id: group,
                ticket,
                rendezvous: [3; 32],
                name: "bea".into(),
            }],
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
        (node, group)
    }

    /// A part of a log `bytes` long (in entries that are no entries, which is all the
    /// node looks at before it has them all).
    fn part(group: GroupId, index: u32, count: u32, bytes: usize) -> MemberRequest {
        MemberRequest::Admitted {
            group_id: group.as_bytes().to_vec(),
            entries: vec![ByteBuf::from(vec![0u8; bytes])],
            part: Some((index, count)),
        }
    }

    /// What is held of logs in parts, in every group, counted entry by entry.
    fn held(n: &Node) -> usize {
        n.groups
            .values()
            .flat_map(|s| s.parts_in.values())
            .flat_map(|p| p.parts.values())
            .flatten()
            .map(|e| e.len())
            .sum()
    }

    /// `peer`, made a manager this device sent its signed join to.
    fn committed(n: &mut Node, group: GroupId, peer: PeerId) {
        n.groups
            .get_mut(&group)
            .unwrap()
            .join_committed
            .insert(peer);
    }

    /// What a manager's answers to this device's join leave: a refusal of the invite shown
    /// or of the signed join lets go of that manager for now, an unsigned no only of the
    /// invite shown to it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_managers_no_to_a_join_lets_go_of_it_as_before() {
        let (node, group) = joining_node();
        let found = inspect(&node, move |n| {
            let (ana, cai, dan) = (PeerId::random(), PeerId::random(), PeerId::random());
            for peer in [ana, cai, dan] {
                committed(n, group, peer);
                n.groups.get_mut(&group).unwrap().join_sent.insert(peer);
            }
            n.on_response(ana, Pending::Join(group), MemberResponse::Refused);
            n.on_response(cai, Pending::JoinCommit(group), MemberResponse::Refused);
            n.on_response(dan, Pending::Join(group), MemberResponse::JoinDeclined);
            let slot = &n.groups[&group];
            [ana, cai, dan].map(|p| {
                (
                    slot.join_sent.contains(&p),
                    slot.join_committed.contains(&p),
                )
            })
        })
        .await;
        assert_eq!(found, [(false, false), (false, false), (false, true)]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parts_sent_again_bigger_count_against_the_bound() {
        let (node, group) = joining_node();
        let (refused, held) = inspect(&node, move |n| {
            let peer = PeerId::random();
            committed(n, group, peer);
            // Every part small first, then each one again, as large as a message carries.
            for index in 0..63 {
                n.on_request(peer, part(group, index, 64, 1));
            }
            let mut refused = false;
            for index in 0..63 {
                let answer = n.on_request(peer, part(group, index, 64, 4 << 20));
                refused |= matches!(answer, MemberResponse::Refused);
            }
            (refused, held(n))
        })
        .await;
        assert!(refused, "the parts sent again went past the bound");
        assert!(held <= MAX_ADMITTED_BYTES, "{held} bytes held");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_part_sent_again_the_same_is_kept() {
        let (node, group) = joining_node();
        let answers = inspect(&node, move |n| {
            let peer = PeerId::random();
            committed(n, group, peer);
            (0..8)
                .map(|_| n.on_request(peer, part(group, 0, 2, 3 << 20)))
                .map(|a| matches!(a, MemberResponse::Ack))
                .collect::<Vec<_>>()
        })
        .await;
        assert!(answers.into_iter().all(|ack| ack));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn all_peers_together_hold_a_bounded_amount_and_none_keeps_the_others_out() {
        let (node, group) = joining_node();
        let (held, honest) = inspect(&node, move |n| {
            // Many peers, each holding as much as one may.
            for _ in 0..8 {
                let peer = PeerId::random();
                committed(n, group, peer);
                for index in 0..4 {
                    n.on_request(peer, part(group, index, 64, 4 << 20));
                }
            }
            let held = held(n);
            // A manager with a shorter log still gets its parts kept.
            let honest = PeerId::random();
            committed(n, group, honest);
            let answer = n.on_request(honest, part(group, 0, 2, 1 << 20));
            (held, matches!(answer, MemberResponse::Ack))
        })
        .await;
        assert!(held <= MAX_ADMITTED_BYTES_ALL, "{held} bytes held");
        assert!(honest);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parts_go_with_the_peer_that_sent_them() {
        let (node, group) = joining_node();
        let held = inspect(&node, move |n| {
            let peer = PeerId::random();
            committed(n, group, peer);
            let address: libp2p::Multiaddr = "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap();
            let point = ConnectedPoint::Dialer {
                address,
                role_override: libp2p::core::Endpoint::Dialer,
                port_use: libp2p::core::transport::PortUse::Reuse,
            };
            n.on_connection_established(peer, ConnectionId::new_unchecked(77), &point);
            n.on_request(peer, part(group, 0, 2, 1 << 20));
            assert!(held(n) > 0);
            n.on_connection_closed(peer, ConnectionId::new_unchecked(77), 0);
            held(n)
        })
        .await;
        assert_eq!(held, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parts_go_when_the_join_ends() {
        let (node, group) = joining_node();
        let held = inspect(&node, move |n| {
            let peer = PeerId::random();
            committed(n, group, peer);
            n.on_request(peer, part(group, 0, 2, 1 << 20));
            assert!(held(n) > 0);
            // Let in by another manager: the join is over.
            let slot = n.groups.get_mut(&group).unwrap();
            slot.ctx.end_join_for_tests();
            n.forget_ended_join(group);
            held(n)
        })
        .await;
        assert_eq!(held, 0);
    }

    /// Two logs sent at once are judged each on the log as it was. A manager the invite
    /// does not name sends one with an `Add` of the joining device whose parent, the
    /// `Rotate` that makes the key it seals, comes in the other: each alone lets nobody
    /// in, and the one taken second is not added to the first unjudged, where the `Add`
    /// would apply.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_log_judged_on_an_older_one_is_not_added_unjudged_while_joining() {
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let zed = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        log.raise_level(&ana, fogata_state::Level::Three, 3)
            .unwrap();
        // Ana is first: her invite names nobody else.
        let (ticket, token) = log.invite(&ana, u64::MAX / 2, 4).unwrap();
        let mut fork = log.clone();
        let rotate = fork.rotate(&bea, 5).unwrap();
        let add = fork.add(&bea, zed.public_key(), "zed", 6).unwrap();
        let with = |extra: &fogata_state::Entry| -> Vec<Vec<u8>> {
            log.entries()
                .chain([extra])
                .map(fogata_state::Entry::encode)
                .collect()
        };
        let (adds, rotates) = (with(&add), with(&rotate));
        let group = log.group_id();
        let (node, _events) = start(NodeConfig {
            identity: zed.clone(),
            groups: vec![GroupSetup::Joining {
                group_id: group,
                ticket,
                rendezvous: fogata_state::invite_rendezvous(&token),
                name: "zed".into(),
            }],
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
        let zed_key = zed.public_key();
        let (joining, member) = inspect(&node, move |n| {
            let slot = &n.groups[&group];
            let first = slot.ctx.admitted_trial(&adds).unwrap();
            let second = slot.ctx.admitted_trial(&rotates).unwrap();
            let judged = first.judge();
            assert!(judged.is_some());
            n.admitted_judged(group, PeerId::random(), adds, true, judged);
            assert!(n.groups[&group].ctx.joining().is_some());
            let judged = second.judge();
            assert!(judged.is_some());
            n.admitted_judged(group, PeerId::random(), rotates, true, judged);
            let ctx = &n.groups[&group].ctx;
            (
                ctx.joining().is_some(),
                ctx.log().state().is_member(&zed_key),
            )
        })
        .await;
        assert!(joining);
        assert!(!member);
    }

    /// A signed join passed on by someone else (any peer the joining device asked, which
    /// stands between it and the managers) does not get that peer the log: the join is
    /// bound to the joining device's own connection, and waits for an answer only there.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_join_passed_on_by_another_peer_does_not_get_it_the_log() {
        use fogata_state::{Body, Entry};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let (ticket, _) = log.invite(&ana, u64::MAX / 2, 2).unwrap();
        let group = log.group_id();
        let (node, _events) = start(NodeConfig {
            identity: ana,
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
        })
        .unwrap();
        let bea_key = bea.public_key();
        let (relayed, unbound, waiting) = inspect(&node, move |n| {
            let ana_peer = n.local_peer().to_bytes();
            let (bea_peer, mallory) = (PeerId::random(), PeerId::random());
            // Both show the invite and are offered the same heads.
            let offer = |n: &mut Node, peer: PeerId| {
                let ask = MemberRequest::JoinRequest {
                    ticket: ticket.encode(),
                };
                match n.on_request(peer, ask) {
                    MemberResponse::JoinOffer { heads } => heads
                        .iter()
                        .map(|h| <[u8; 32]>::try_from(h.as_slice()).unwrap())
                        .collect::<Vec<_>>(),
                    _ => panic!("no offer"),
                }
            };
            let heads = offer(n, mallory);
            offer(n, bea_peer);
            let body = Body::Join {
                ticket: ticket.clone(),
                name: "Bea".into(),
            };
            let entry = Entry::sign(&bea, group, heads, 2_000, body);
            let commit = |binding: Option<[u8; 64]>| MemberRequest::JoinCommit {
                entry: entry.encode(),
                binding: binding.map(|b| ByteBuf::from(b.to_vec())),
            };
            let bound = entry.join_binding(&bea, &bea_peer.to_bytes(), &ana_peer);
            // Mallory passes Bea's join on, as Bea bound it, and with none.
            let relayed = n.on_request(mallory, commit(Some(bound)));
            offer(n, mallory);
            let unbound = n.on_request(mallory, commit(None));
            // Bea hands over her own.
            let own = n.on_request(bea_peer, commit(Some(bound)));
            assert!(matches!(own, MemberResponse::JoinWaiting));
            let waiting = n.groups[&group].join_waiting.get(&bea_key).map(|w| w.peer);
            (relayed, unbound, waiting == Some(bea_peer))
        })
        .await;
        assert!(matches!(relayed, MemberResponse::Refused));
        assert!(matches!(unbound, MemberResponse::Refused));
        assert!(waiting, "the join waits on Bea's own connection");
    }

    /// A group gone up to a level lets in no device whose copy does not judge at it: such a
    /// copy refuses the log from the raise on and would never get in, while the group would
    /// count it a member. Its join is refused, which leaves the invite good, the manager is
    /// told once that it must update, and an updated copy is then let in as before. A device
    /// that shows this one nothing of what it speaks is let in as before.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_copy_below_the_group_s_level_is_not_let_in_and_its_manager_is_told() {
        use std::collections::HashSet;

        use fogata_state::{Body, Entry, Level};

        use crate::{NodeEvent, caps};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let cai = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.raise_level(&ana, Level::Three, 2).unwrap();
        let (ticket, _) = log.invite(&ana, u64::MAX / 2, 3).unwrap();
        let (other, _) = log.invite(&ana, u64::MAX / 2, 4).unwrap();
        let group = log.group_id();
        let (node, mut events) = start(NodeConfig {
            identity: ana,
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
        })
        .unwrap();
        let bea_key = bea.public_key();
        let answers = inspect(&node, move |n| {
            let ana_peer = n.local_peer().to_bytes();
            let (bea_peer, cai_peer) = (PeerId::random(), PeerId::random());
            let lists = |n: &mut Node, peer: PeerId, level: Option<u32>| {
                let shown: HashSet<_> = level.map(caps::caps_protocol).into_iter().collect();
                crate::locked(&n.advertised).insert(peer, shown);
            };
            let join = |n: &mut Node,
                        who: &DeviceIdentity,
                        peer: PeerId,
                        ticket: &fogata_state::Ticket| {
                let ask = MemberRequest::JoinRequest {
                    ticket: ticket.encode(),
                };
                let heads = match n.on_request(peer, ask) {
                    MemberResponse::JoinOffer { heads } => heads
                        .iter()
                        .map(|h| <[u8; 32]>::try_from(h.as_slice()).unwrap())
                        .collect::<Vec<_>>(),
                    _ => panic!("no offer"),
                };
                let body = Body::Join {
                    ticket: ticket.clone(),
                    name: "Bea".into(),
                };
                let entry = Entry::sign(who, group, heads, 4_000, body);
                let binding = entry.join_binding(who, &peer.to_bytes(), &ana_peer);
                let commit = MemberRequest::JoinCommit {
                    entry: entry.encode(),
                    binding: Some(ByteBuf::from(binding.to_vec())),
                };
                let answer = n.on_request(peer, commit);
                let waits = n.groups[&group]
                    .join_waiting
                    .contains_key(&who.public_key());
                (matches!(answer, MemberResponse::JoinWaiting), waits)
            };
            // The previous version: twice, as it keeps asking.
            lists(n, bea_peer, Some(2));
            let old = [
                join(n, &bea, bea_peer, &ticket),
                join(n, &bea, bea_peer, &ticket),
            ];
            // Updated.
            lists(n, bea_peer, Some(caps::CAPS_LEVEL));
            let updated = join(n, &bea, bea_peer, &ticket);
            // Showing nothing of what it speaks.
            lists(n, cai_peer, None);
            let unshown = join(n, &cai, cai_peer, &other);
            (old, updated, unshown)
        })
        .await;
        let (old, updated, unshown) = answers;
        assert_eq!(old, [(false, false), (false, false)]);
        assert_eq!(updated, (true, true));
        assert_eq!(unshown, (true, true));
        let mut told = 0;
        while let Ok(event) = events.try_recv() {
            if let NodeEvent::MustUpdateToJoin {
                group: g,
                device,
                name,
            } = event
            {
                assert_eq!((g, device, name.as_str()), (group, bea_key, "Bea"));
                told += 1;
            }
        }
        assert_eq!(told, 1, "the manager is told once");
    }

    /// A device that asked before the group went up a level, and whose copy does not judge
    /// at the new one, is not let in by its manager's yes either: the manager is told.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_yes_after_the_group_went_up_lets_no_older_copy_in() {
        use fogata_state::{Body, Entry, Level};

        use crate::NodeEvent;
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let (ticket, _) = log.invite(&ana, u64::MAX / 2, 2).unwrap();
        let group = log.group_id();
        let entry = Entry::sign(
            &bea,
            group,
            log.heads(),
            3,
            Body::Join {
                ticket: ticket.clone(),
                name: "Bea".into(),
            },
        );
        log.raise_level(&ana, Level::Three, 4).unwrap();
        let (node, mut events) = start(NodeConfig {
            identity: ana,
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
        })
        .unwrap();
        let bea_key = bea.public_key();
        let member = inspect(&node, move |n| {
            let slot = n.groups.get_mut(&group).unwrap();
            let waiting = super::super::WaitingJoin {
                peer: PeerId::random(),
                entry,
                ticket,
                level: Some(2),
            };
            slot.wait_for_answer(bea_key, waiting).unwrap();
            n.answer_join(group, bea_key, true);
            n.groups[&group].ctx.is_member_device(&bea_key)
        })
        .await;
        assert!(!member);
        let mut told = false;
        while let Ok(event) = events.try_recv() {
            told |=
                matches!(event, NodeEvent::MustUpdateToJoin { device, .. } if device == bea_key);
        }
        assert!(told);
    }

    /// A group at the level every copy judges at lets in whatever a device's copy says.
    #[test]
    fn a_group_never_raised_is_judged_by_every_copy() {
        use fogata_state::Level;
        assert_eq!(super::judges_level(Level::Two, None), Some(true));
        assert_eq!(super::judges_level(Level::Two, Some(1)), Some(true));
        assert_eq!(super::judges_level(Level::Three, None), None);
        assert_eq!(super::judges_level(Level::Three, Some(2)), Some(false));
        assert_eq!(super::judges_level(Level::Three, Some(3)), Some(true));
        assert_eq!(super::judges_level(Level::Three, Some(4)), Some(true));
    }

    /// A joining device tells its person to update when the log it was let in with went up
    /// to a level this version does not know, signed; not for a level it knows, nor for one
    /// whose signature does not hold.
    #[test]
    fn a_log_raised_past_this_version_is_told() {
        use fogata_state::{Body, Entry, Level};

        use super::raised_past_this_version;
        let ana = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        let raise = log.raise_level(&ana, Level::Three, 2).unwrap();
        let mut entries: Vec<Vec<u8>> = log.entries().map(Entry::encode).collect();
        assert!(!raised_past_this_version(&entries));
        let past = Level::CURRENT.number() + 1;
        let later = Entry::sign(
            &ana,
            log.group_id(),
            vec![raise.hash()],
            3,
            Body::Level { level: past },
        );
        let mut forged = later.clone();
        forged.time_ms += 1;
        entries.push(forged.encode());
        assert!(!raised_past_this_version(&entries));
        entries.pop();
        entries.push(later.encode());
        assert!(raised_past_this_version(&entries));
    }
}
