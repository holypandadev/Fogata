//! Direct chats on the node. Each runs over a group of just the two friends, made by either
//! of them: its log travels over their friend connection (`FriendRequest::Direct`), and the
//! membership proof, the log's sync and the sealed messages go over the same connection as
//! for any group. A direct chat's group is never announced or looked up on the DHT, nor
//! offered as a group: the two find each other as friends. Its messages go to the other
//! person only, never flooded, and the application keeps them and says when they arrived.

use fogata_crypto::{DevicePublicKey, GroupId, SealedMessage};
use fogata_proto::{
    FriendRequest, FriendResponse, MAX_DIRECT_BATCH, MAX_DIRECT_PAYLOAD_BYTES, MemberRequest,
    MemberResponse, WireMessage,
};
use fogata_state::{Entry, GroupLog};
use libp2p::PeerId;

use super::{GroupSlot, Node, friends::FriendPending, judging::Job};
use crate::{NodeEvent, group::GroupSetup};

/// Most entries in the log of a direct chat's group offered by a friend: it takes two (the
/// group made and the friend added), and a few more if a key is ever renewed.
const MAX_DIRECT_ENTRIES: usize = 64;

impl Node {
    /// Runs a direct chat's group, or adds what this copy of its log brings to it, and
    /// reaches the friend at once if it is online.
    pub(super) fn add_direct(&mut self, log: GroupLog) {
        let group = log.group_id();
        match self.groups.get_mut(&group) {
            Some(slot) => {
                let new = slot.ctx.merge(&log);
                self.after_group_change(group, new, None);
            }
            None => {
                let mut slot = GroupSlot::new(self.identity.clone(), GroupSetup::Member(log));
                slot.direct = true;
                self.groups.insert(group, slot);
            }
        }
        let others = self.others_in(group);
        for peer in self.friend_peers(&others) {
            if let Some(f) = self.friends.as_mut() {
                f.group_shared.retain(|(p, g)| !(*p == peer && *g == group));
            }
            if let Some(&device) = self.friends.as_ref().and_then(|f| f.online_device(&peer)) {
                self.offer_direct(peer, device);
            }
        }
    }

    /// The members of `group` other than this device.
    fn others_in(&self, group: GroupId) -> Vec<DevicePublicKey> {
        let me = self.identity.public_key();
        self.groups
            .get(&group)
            .map(|slot| {
                slot.ctx
                    .log()
                    .state()
                    .members
                    .keys()
                    .filter(|d| **d != me)
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A friend is online on `peer`: sends it, once per connection, the log of every direct
    /// chat's group it is in and has not proved yet (it may not have it), and asks it to
    /// prove it.
    pub(super) fn offer_direct(&mut self, peer: PeerId, device: DevicePublicKey) {
        let groups: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, slot)| {
                slot.direct
                    && slot.ctx.i_am_member()
                    && slot.ctx.is_member_device(&device)
                    && !slot.members.contains(&peer)
            })
            .map(|(id, _)| *id)
            .collect();
        for group in groups {
            let first = self
                .friends
                .as_mut()
                .is_some_and(|f| f.group_shared.insert((peer, group)));
            if first {
                let entries = self.groups[&group]
                    .ctx
                    .encoded(self.groups[&group].ctx.hashes())
                    .into_iter()
                    .map(serde_bytes::ByteBuf::from)
                    .collect();
                self.friend_request(
                    peer,
                    FriendRequest::Direct { entries },
                    FriendPending::Direct(group),
                );
            }
            self.approach(peer, Some(group));
        }
    }

    /// A friend sent the log of a direct chat's group. One running here takes what it
    /// brings, from one of its two members; any other is judged away from the loop (see
    /// judging.rs) and offered to the application, which decides whether to run it
    /// ([`NodeEvent::DirectOffered`], from [`Self::direct_offer_judged`]).
    pub(super) fn on_direct_offer(
        &mut self,
        peer: PeerId,
        by: DevicePublicKey,
        entries: Vec<serde_bytes::ByteBuf>,
    ) -> FriendResponse {
        if entries.len() > MAX_DIRECT_ENTRIES {
            return FriendResponse::Refused;
        }
        let decoded: Vec<Entry> = entries
            .iter()
            .filter_map(|bytes| Entry::decode(bytes).ok())
            .collect();
        let Some(group) = decoded.first().map(|e| e.group_id) else {
            return FriendResponse::Refused;
        };
        if decoded.iter().any(|e| e.group_id != group) {
            return FriendResponse::Refused;
        }
        let bytes: Vec<Vec<u8>> = entries.into_iter().map(|e| e.into_vec()).collect();
        if self.groups.contains_key(&group) {
            return self.direct_to_running(peer, by, group, &bytes);
        }
        if self
            .judging
            .add(peer, group, Job::Direct { by, entries: bytes })
        {
            FriendResponse::Ack
        } else {
            FriendResponse::Refused
        }
    }

    /// A direct chat's log a friend sent for a group that runs here: taken from one of its
    /// two members, as any batch a member sends.
    fn direct_to_running(
        &mut self,
        peer: PeerId,
        by: DevicePublicKey,
        group: GroupId,
        entries: &[Vec<u8>],
    ) -> FriendResponse {
        let Some(slot) = self.groups.get_mut(&group) else {
            return FriendResponse::Refused;
        };
        if !slot.direct || !slot.ctx.is_member_device(&by) {
            return FriendResponse::Refused;
        }
        let taken = slot.ctx.insert_encoded(entries);
        self.refused_entries(&taken.refused);
        self.after_group_change(group, taken.new, Some(peer));
        self.approach(peer, Some(group));
        FriendResponse::Ack
    }

    /// A direct chat's group offered by the friend `by` on `peer`, judged: offered to the
    /// application if it has just the two of them as members and `by` is still a friend.
    /// One that runs here by now takes the entries as if they came now.
    pub(super) fn direct_offer_judged(
        &mut self,
        peer: PeerId,
        by: DevicePublicKey,
        entries: Vec<Vec<u8>>,
        log: GroupLog,
    ) {
        let group = log.group_id();
        if self.groups.contains_key(&group) {
            self.direct_to_running(peer, by, group, &entries);
            return;
        }
        let friend = self
            .friends
            .as_ref()
            .is_some_and(|f| f.book.friends.contains_key(&by));
        let state = log.state();
        let me = self.identity.public_key();
        if !friend || state.members.len() != 2 || !state.is_member(&me) || !state.is_member(&by) {
            return;
        }
        // The application fell far behind meanwhile: dropped, as when it was behind as the
        // offer came.
        if self.events.full() {
            return;
        }
        self.emit(NodeEvent::DirectOffered { log, by });
    }

    /// Seals `payloads` with the key of a direct chat's group and sends them to `to`, the
    /// other person, if it is connected as a member of it now.
    pub(super) fn send_direct(
        &mut self,
        group: GroupId,
        to: DevicePublicKey,
        payloads: Vec<Vec<u8>>,
    ) -> Result<(), String> {
        let Some(slot) = self.groups.get(&group).filter(|s| s.direct) else {
            return Err("no such chat".into());
        };
        let Some(peer) = slot
            .members
            .iter()
            .find(|p| self.members.get(*p).is_some_and(|m| m.device == to))
            .copied()
        else {
            return Err("the friend is not connected".into());
        };
        let keys = slot.ctx.current().cloned().ok_or("no chat key yet")?;
        let mut messages = Vec::with_capacity(payloads.len());
        for payload in payloads {
            if payload.len() > MAX_DIRECT_PAYLOAD_BYTES {
                return Err("a message is too long".into());
            }
            let at = self.seal_time();
            let sealed = SealedMessage::seal(&keys, &self.identity, at, &payload)
                .map_err(|e| e.to_string())?;
            messages.push(WireMessage::from(&sealed));
        }
        for batch in messages.chunks(MAX_DIRECT_BATCH) {
            self.request_direct(
                peer,
                MemberRequest::Direct {
                    group_id: group.as_bytes().to_vec(),
                    messages: batch.to_vec(),
                },
            );
        }
        Ok(())
    }

    /// Messages of a direct chat from its other person: each must open with the chat's key
    /// and be signed by that person. They go to the application as they are; nothing is
    /// forwarded.
    pub(super) fn on_direct_request(
        &mut self,
        peer: PeerId,
        group_id: &[u8],
        messages: Vec<WireMessage>,
    ) -> MemberResponse {
        let Ok(group) = GroupId::from_bytes(group_id) else {
            return MemberResponse::Refused;
        };
        let Some(slot) = self.groups.get(&group).filter(|s| s.direct) else {
            return MemberResponse::Refused;
        };
        let Some(from) = self.members.get(&peer).map(|m| m.device) else {
            return MemberResponse::Refused;
        };
        if !slot.members.contains(&peer)
            || from == self.identity.public_key()
            || messages.len() > MAX_DIRECT_BATCH
        {
            return MemberResponse::Refused;
        }
        let mut payloads = Vec::new();
        for wire in &messages {
            let Ok(sealed) = SealedMessage::try_from(wire) else {
                continue;
            };
            if sealed.header.sender != from || sealed.header.group_id != group {
                continue;
            }
            let Some(keys) = slot.ctx.keys(sealed.header.epoch) else {
                continue;
            };
            match sealed.open(keys) {
                Ok(payload) if payload.len() <= MAX_DIRECT_PAYLOAD_BYTES => payloads.push(payload),
                Ok(_) => {}
                Err(e) => tracing::debug!("dropping a direct message from {peer}: {e}"),
            }
        }
        if !payloads.is_empty() {
            self.emit(NodeEvent::Direct {
                group,
                from,
                payloads,
            });
        }
        MemberResponse::Ack
    }
}
