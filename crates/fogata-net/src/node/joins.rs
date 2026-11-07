//! Joining a group with an invite: the signed join sent to the managers, a manager's answer,
//! the log sent to a device let in part after part, refusals that go along with a revoke,
//! and the log this device was sent when it was let in, judged away from the loop.

use std::collections::{BTreeMap, VecDeque};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_proto::MemberRequest;
use fogata_state::{GroupLog, Ticket};
use libp2p::PeerId;

use super::{Node, Pending, judging::Job, now_ms};
use crate::{NodeEvent, group::NotLetIn, short_peer};

/// Times a part of a log that did not arrive is sent again.
const PART_TRIES: u8 = 3;
/// Most parts, and bytes, of a long log taken from one manager that let this device in.
pub(super) const MAX_ADMITTED_PARTS: u32 = 64;
pub(super) const MAX_ADMITTED_BYTES: usize = 16 * 1024 * 1024;
/// Most bytes of logs in parts held at once, from every peer of every group being joined:
/// any peer found under an invite's key may say it let this device in, so what they send
/// is bounded as a whole too.
pub(super) const MAX_ADMITTED_BYTES_ALL: usize = 2 * MAX_ADMITTED_BYTES;

impl Node {
    /// Joining device: shows our invite ticket to a peer that may be a member.
    pub(super) fn send_join(&mut self, peer: PeerId, group: GroupId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let Some(ticket) = slot.ctx.joining().map(Ticket::encode) else {
            return;
        };
        if slot.join_sent.insert(peer) {
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
    /// and the rest come with the next sync.
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
            return;
        }
        let (new, refused) = match judged.and_then(|log| slot.ctx.take_judged(log)) {
            Some(new) => (new, BTreeMap::new()),
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
        node::{Node, start, tests::inspect},
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
}
