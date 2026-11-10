//! The member protocol, group by group: the requests members send each other and their
//! answers. Its parts are beside it: proofs of membership (proofs.rs), what a peer says
//! before proving itself (early.rs), joining with an invite (joins.rs), what a change to a
//! group's log brings (group_changes.rs), the heartbeat (heartbeat.rs), other ways to reach
//! members (member_reach.rs), requests on their way (outbox.rs), this device's presence
//! (presence.rs) and group messages spread to a group's members (spread.rs).

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use fogata_crypto::{GroupId, MembershipProof, SealedProof};
use fogata_proto::{MemberRequest, MemberResponse, SharedBytes, WireProof};
use fogata_state::{Entry, EntryHash, Ticket};
use libp2p::{PeerId, request_response};

use super::early::early_profile;
use super::member_reach::lan_quic;
use super::proofs::MAX_TAGS;
use super::spread::Forward;
use super::{Node, Pending, WaitingJoin, judging::Job, limits, now_ms};
use crate::{NodeEvent, Path, short_peer};

/// Local addresses a member sends are dialled again at most this often: each dial sends
/// packets to every one of them, and a member could otherwise point this device at any
/// address, again and again.
const LAN_REDIAL: Duration = Duration::from_secs(60);
/// Most events of a group's rooms a member asks for in parts: a little more than its rooms
/// can hold (500 chat lines in each of 10 voice rooms, and the join and 100 files of each of
/// 20 members, each in one room at a time). Each one is checked like any group message.
const MAX_ROOM_EVENTS: u32 = 8192;
/// Most members kept from one member's roster.
const MAX_ROSTER: usize = 256;
/// Most peers of one roster this device is not connected to that it tries to reach at once,
/// through the member that sent it: more than a group holds.
const MAX_ROSTER_DIALS: usize = 32;

pub(super) fn bytes(hashes: impl IntoIterator<Item = EntryHash>) -> Vec<serde_bytes::ByteBuf> {
    hashes
        .into_iter()
        .map(|h| serde_bytes::ByteBuf::from(h.to_vec()))
        .collect()
}

fn hashes(list: &[serde_bytes::ByteBuf]) -> Vec<EntryHash> {
    list.iter()
        .filter_map(|h| <[u8; 32]>::try_from(h.as_slice()).ok())
        .collect()
}

/// About what a sealed group message takes in an answer.
pub(super) fn wire_bytes(message: &fogata_proto::WireMessage) -> usize {
    message.ciphertext.len()
        + message.signature.len()
        + message.nonce.len()
        + message.sender.len()
        + message.message_id.len()
        + message.group_id.len()
        + 16
}

fn sorted(mut list: Vec<EntryHash>) -> Vec<EntryHash> {
    list.sort_unstable();
    list
}

pub(super) fn group_of(bytes: &[u8]) -> Option<GroupId> {
    GroupId::from_bytes(bytes).ok()
}

/// Whether `request` may only add events for the application, which it is refused while
/// the application is far behind (see events.rs): group and direct messages. Everything
/// else keeps the group and its members going. A call signal is not refused: its sender
/// would not send it again, and the call would wait for it; few and small as they are,
/// they give way on the application's side, which then has the call agree anew.
fn adds_events(request: &MemberRequest) -> bool {
    matches!(
        request,
        MemberRequest::Deliver(_) | MemberRequest::Spread { .. } | MemberRequest::Direct { .. }
    )
}

impl Node {
    // ----- Requests and answers -----

    pub(super) fn on_member_event(
        &mut self,
        event: request_response::Event<MemberRequest, MemberResponse>,
    ) {
        match event {
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            } => {
                let response = self.on_request(peer, request);
                let _ = self
                    .swarm
                    .behaviour_mut()
                    .member
                    .send_response(channel, response);
            }
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
                ..
            } => {
                if let Some((to, kind)) = self.pending.remove(&request_id) {
                    self.request_done(to);
                    self.on_response(peer, kind, response);
                }
            }
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                let Some((to, kind)) = self.pending.remove(&request_id) else {
                    return;
                };
                // Those after a direct chat's request that did not get there wait for the
                // application to send everything not confirmed again, in its order.
                if matches!(kind, Pending::Direct)
                    && let Some(outbox) = self.outbox.get_mut(&to)
                {
                    outbox.direct.clear();
                }
                self.request_done(to);
                match kind {
                    Pending::AdmittedPart(group) => self.part_lost(group, peer),
                    Pending::Refusal(group) => self.send_refusal_again(peer, group),
                    // Lost with a connection that closed while another stays (both ends
                    // dialled at once, and one was let go): asked again at once on the one
                    // left. The other end may have this device's proof already and ask
                    // nothing more, and nothing else asks again: it would stay unchecked
                    // here, every request of it refused, for as long as it is connected.
                    Pending::Challenge(group, _)
                        if matches!(
                            error,
                            request_response::OutboundFailure::ConnectionClosed
                                | request_response::OutboundFailure::Io(_)
                        ) && self.swarm.is_connected(&peer) =>
                    {
                        self.request_dropped(to, kind);
                        self.challenge_with(peer, group, None);
                    }
                    // Lost on the way (its connection closed while another stays, or no
                    // answer in time): let go as one that never went, so that an invite is
                    // shown again, and a room asked for again, at the next heartbeat.
                    kind => self.request_dropped(to, kind),
                }
            }
            _ => {}
        }
    }

    /// The device of member `peer`, when an ask of its for the log or a room's past is
    /// answered now (see `limits::SERVED_BYTES_BURST`).
    fn may_serve(&mut self, peer: PeerId) -> Option<fogata_crypto::DevicePublicKey> {
        let device = self.members.get(&peer)?.device;
        if self.limits.may_serve(device, Instant::now()) {
            Some(device)
        } else {
            tracing::debug!("{} asked for more than it may now", short_peer(&peer));
            None
        }
    }

    /// Whether a group's banner or picture record of `bytes` from member `peer` is looked at
    /// now (see `limits::GROUP_RECORD_BYTES_BURST`).
    fn take_group_record(&mut self, peer: PeerId, bytes: usize) -> bool {
        let Some(device) = self.members.get(&peer).map(|m| m.device) else {
            return false;
        };
        let taken = self.limits.take_group_record(device, bytes, Instant::now());
        if !taken {
            tracing::debug!(
                "{} sent more group records than it may now",
                short_peer(&peer)
            );
        }
        taken
    }

    /// Charges member `device` for an answer of about `bytes` made for it.
    fn charge_served(&mut self, device: fogata_crypto::DevicePublicKey, bytes: usize) {
        self.limits.charge_served(device, bytes, Instant::now());
    }

    pub(super) fn on_request(&mut self, peer: PeerId, request: MemberRequest) -> MemberResponse {
        // A stranger past its address's budget gets an answer that costs nothing: a challenge
        // the same decoy as one that names no group of this device, anything else a refusal.
        if self.stranger_over_budget(&peer) {
            return match request {
                MemberRequest::Challenge { .. } => SealedProof::decoy(0)
                    .map(|d| MemberResponse::Proof(WireProof::from(&d)))
                    .unwrap_or(MemberResponse::Refused),
                _ => MemberResponse::Refused,
            };
        }
        // The application is far behind (see events.rs): what would only add to its events
        // is refused. A direct message is sent again by its sender; group lines and what a
        // room said meanwhile are asked for again once the application is not behind (see
        // `room_heartbeat`), in the group the message was of.
        if adds_events(&request) && self.events.full() {
            let group = match &request {
                MemberRequest::Deliver(message) | MemberRequest::Spread { message, .. } => {
                    group_of(&message.group_id)
                }
                _ => None,
            };
            if let Some(slot) = group.and_then(|g| self.groups.get_mut(&g)) {
                slot.room_again.get_or_insert_default();
            }
            return MemberResponse::Refused;
        }
        match request {
            MemberRequest::Challenge { challenge, tags } => {
                // Whoever asks gets an answer of the same shape: a proof sealed with the
                // epoch's key, or random bytes when this device cannot answer. Nobody without
                // a group's key can name it, gets a device key or finds out whether this
                // device is in a group; only a named group gets a challenge back.
                let decoy = |epoch: u32| {
                    SealedProof::decoy(epoch)
                        .map(|d| MemberResponse::Proof(WireProof::from(&d)))
                        .unwrap_or(MemberResponse::Refused)
                };
                let Ok(challenge) = <[u8; 32]>::try_from(challenge.as_slice()) else {
                    return decoy(0);
                };
                // The newest epoch both sides know, so a member who missed a rotation can
                // still connect and catch up.
                let Some((group, keys)) = self.named_group(&peer, &challenge, &tags) else {
                    return decoy(0);
                };
                let slot = &self.groups[&group];
                // An older key, or the current one while someone who left still has it,
                // is no sign the asker is still in: it proves itself first, and once it
                // has, it is asked to ask again.
                if !slot.ctx.key_kept_in(keys.epoch()) && !slot.members.contains(&peer) {
                    let slot = self.groups.get_mut(&group).expect("checked above");
                    slot.held_back.insert(peer);
                    self.interesting.insert(peer);
                    self.hinted.entry(peer).or_default().insert(group);
                    let older = !slot
                        .ctx
                        .known_epochs()
                        .iter()
                        .take(MAX_TAGS)
                        .any(|n| *n == keys.epoch());
                    self.challenge_with(peer, group, older.then_some(keys.epoch()));
                    return decoy(keys.epoch());
                }
                let local = self.local_peer().to_bytes();
                let proof = MembershipProof::create(
                    &keys,
                    &self.identity,
                    &local,
                    &peer.to_bytes(),
                    &challenge,
                );
                let Ok(sealed) = proof.seal(&keys, &local, &peer.to_bytes(), &challenge) else {
                    return decoy(keys.epoch());
                };
                // Membership is proved both ways, also under the epoch the asker named: it
                // may be older than the newest few (a member that missed many rotations).
                self.interesting.insert(peer);
                self.hinted.entry(peer).or_default().insert(group);
                self.challenge_with(peer, group, Some(keys.epoch()));
                MemberResponse::Proof(WireProof::from(&sealed))
            }
            MemberRequest::Deliver(wire) => {
                let Some(group) = group_of(&wire.group_id) else {
                    return MemberResponse::Refused;
                };
                if !self.is_member_of(&group, &peer) || !self.take_group_message(peer, &wire) {
                    return MemberResponse::Refused;
                }
                self.receive(peer, wire, Forward::Flooded);
                MemberResponse::Ack
            }
            MemberRequest::Spread { message, reached } => {
                let Some(group) = group_of(&message.group_id) else {
                    return MemberResponse::Refused;
                };
                if !self.is_member_of(&group, &peer) || !self.take_group_message(peer, &message) {
                    return MemberResponse::Refused;
                }
                self.receive(peer, message, Forward::Spread(reached));
                MemberResponse::Ack
            }
            MemberRequest::RoomEvents { group_id, ids } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                if !self.is_member_of(&group, &peer) {
                    return MemberResponse::Refused;
                }
                let Some(device) = self.may_serve(peer) else {
                    return MemberResponse::Refused;
                };
                let messages = self.recent_events(group, &ids);
                self.charge_served(device, messages.iter().map(wire_bytes).sum());
                MemberResponse::RoomLog {
                    messages,
                    more: None,
                }
            }
            MemberRequest::Roster {
                group_id,
                heads,
                banner,
                picture,
            } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                if !slot.members.contains(&peer) {
                    return MemberResponse::Refused;
                }
                let Some(device) = self.may_serve(peer) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                // It counts this device in now: an ask of this device's it refused before
                // goes again.
                if slot.roster_refused.remove(&peer) {
                    self.ask_roster(group, peer);
                } else {
                    slot.roster_heard.insert(peer);
                }
                // Banners only to versions that say they keep them. Each costs this device
                // up to a few hundred KiB of upload: charged to the member, and sent once
                // per record however often it asks (see `RecordSent`).
                let mut served = 0;
                if let Some(banner) = banner
                    && let Some(slot) = self.groups.get_mut(&group)
                {
                    slot.banner_peers.insert(peer);
                    if banner.as_slice() != slot.ctx.banner_digest() {
                        served += self.send_banner_to(group, &[peer]);
                    }
                }
                // Picture records the same way.
                if let Some(picture) = picture
                    && let Some(slot) = self.groups.get_mut(&group)
                {
                    slot.picture_peers.insert(peer);
                    if picture.as_slice() != slot.ctx.picture_digest() {
                        served += self.send_picture_to(group, &[peer]);
                    }
                }
                let Some(slot) = self.groups.get(&group) else {
                    return MemberResponse::Refused;
                };
                let heads_digest = slot.ctx.heads_digest();
                // The same log as here: it has this device's leave.
                if slot.leaving.is_some()
                    && heads.as_ref().is_some_and(|h| h.as_slice() == heads_digest)
                {
                    self.leave_taken(group);
                    return MemberResponse::Refused;
                }
                // One sync at a time with each member: the answer asks for the next.
                if heads.is_some_and(|h| h.as_slice() != heads_digest) {
                    served += self.request_sync(peer, group);
                }
                let Some(slot) = self.groups.get(&group) else {
                    return MemberResponse::Refused;
                };
                let peers = slot
                    .members
                    .iter()
                    .filter(|p| **p != peer)
                    .filter(|p| {
                        self.members
                            .get(p)
                            .is_some_and(|m| !matches!(m.path, Path::ThroughMember { .. }))
                    })
                    .map(|p| serde_bytes::ByteBuf::from(p.to_bytes()))
                    .collect::<Vec<_>>();
                served += peers.iter().map(|p| p.len()).sum::<usize>();
                self.charge_served(device, served);
                MemberResponse::Roster { peers }
            }
            MemberRequest::Sync { group_id, have } => {
                let Some(slot) = group_of(&group_id).and_then(|g| self.groups.get(&g)) else {
                    return MemberResponse::Refused;
                };
                if !slot.members.contains(&peer) {
                    return MemberResponse::Refused;
                }
                let Some(device) = self.may_serve(peer) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = group_of(&group_id).and_then(|g| self.groups.get(&g)) else {
                    return MemberResponse::Refused;
                };
                let have = hashes(&have);
                if let Some(leave) = slot.leaving
                    && have.contains(&leave)
                {
                    let group = slot.ctx.group_id();
                    self.leave_taken(group);
                    return MemberResponse::Refused;
                }
                let (entries, want) = slot.ctx.diff(&have);
                let served = entries.iter().map(|e| e.len()).sum::<usize>() + want.len() * 32;
                self.charge_served(device, served);
                MemberResponse::SyncReply {
                    entries: entries
                        .into_iter()
                        .map(serde_bytes::ByteBuf::from)
                        .collect(),
                    want: bytes(want),
                }
            }
            MemberRequest::GroupBanner { group_id, banner } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                if !slot.members.contains(&peer) {
                    return MemberResponse::Refused;
                }
                slot.banner_peers.insert(peer);
                // Past the member's budget it is not looked at, but not refused either: an
                // earlier version that hears a refusal would not send this banner again
                // while connected. It comes again with the next roster that differs.
                if !self.take_group_record(peer, banner.len()) {
                    return MemberResponse::Ack;
                }
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                // Only a newer one is taken: an older one is dropped before its signature is
                // checked over up to half a megabyte, which reading its date does not cost.
                match slot.ctx.banner_may_be_newer(&banner) {
                    Err(_) => return MemberResponse::Refused,
                    Ok(false) => return MemberResponse::Ack,
                    Ok(true) => {}
                }
                let Ok(banner) = fogata_state::GroupBanner::decode(&banner) else {
                    return MemberResponse::Refused;
                };
                if slot.ctx.take_banner(banner) {
                    self.banner_changed(group, Some(peer));
                }
                MemberResponse::Ack
            }
            MemberRequest::GroupPicture { group_id, picture } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                if !slot.members.contains(&peer) {
                    return MemberResponse::Refused;
                }
                slot.picture_peers.insert(peer);
                // As a banner past the budget is.
                if !self.take_group_record(peer, picture.len()) {
                    return MemberResponse::Ack;
                }
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                // As an older banner is.
                match slot.ctx.picture_may_be_newer(&picture) {
                    Err(_) => return MemberResponse::Refused,
                    Ok(false) => return MemberResponse::Ack,
                    Ok(true) => {}
                }
                let Ok(picture) = fogata_state::GroupPicture::decode(&picture) else {
                    return MemberResponse::Refused;
                };
                if slot.ctx.take_picture(picture) {
                    self.picture_changed(group, Some(peer));
                }
                MemberResponse::Ack
            }
            MemberRequest::Push { group_id, entries } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                let entries: Vec<Vec<u8>> = entries.iter().map(SharedBytes::to_vec).collect();
                if !self.groups.contains_key(&group) {
                    return MemberResponse::Refused;
                }
                // A removal shown by a peer that is no member here is judged away from the
                // loop (see judging.rs): refused before it pays when too many wait.
                let member = self.groups[&group].members.contains(&peer);
                if !member && !self.judging.has_room(peer) {
                    return MemberResponse::Refused;
                }
                let entries = self.within_entry_budget(peer, entries);
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                if !member {
                    let Some(trial) = slot.ctx.removal_trial(&entries) else {
                        return MemberResponse::Refused;
                    };
                    return if self.judging.add(peer, group, Job::Removal(trial)) {
                        MemberResponse::Ack
                    } else {
                        MemberResponse::Refused
                    };
                }
                self.take_pushed(peer, group, &entries);
                MemberResponse::Ack
            }
            MemberRequest::JoinRequest { ticket } => {
                let Ok(ticket) = Ticket::decode(&ticket) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&ticket.group_id) else {
                    return MemberResponse::Refused;
                };
                if slot.ctx.joining().is_some() || !slot.ctx.i_am_manager() {
                    return MemberResponse::Refused;
                }
                if !slot.ctx.ticket_genuine(&ticket) {
                    return MemberResponse::Refused;
                }
                if slot.ctx.ticket_revoked(&ticket.ticket_id) {
                    // The answer itself proves nothing; the signed `Revoke` goes along.
                    let revoke = slot.ctx.refusal_of(&ticket);
                    self.send_refusal(peer, ticket.group_id, revoke);
                    return MemberResponse::JoinDeclined;
                }
                // A ticket already used is offered again only to show its joiner, now a
                // member, the log it missed (checked against its signed `Join` next).
                if !slot.ctx.ticket_acceptable(&ticket, now_ms())
                    && !slot.ctx.ticket_used(&ticket.ticket_id)
                {
                    return MemberResponse::Refused;
                }
                let heads = slot.ctx.heads();
                slot.join_offers
                    .insert(peer, (ticket.ticket_id, heads.clone()));
                MemberResponse::JoinOffer {
                    heads: bytes(heads),
                }
            }
            MemberRequest::JoinCommit { entry, binding } => {
                let Ok(entry) = Entry::decode(&entry) else {
                    return MemberResponse::Refused;
                };
                // A signed `Join` can be passed on by anyone the joining device asked: the
                // log goes only to the peer its author bound it to, which is this one.
                let local = self.local_peer().to_bytes();
                if !binding.is_some_and(|b| entry.join_bound(&b, &peer.to_bytes(), &local)) {
                    tracing::debug!(
                        "{} handed over a join not bound to its connection",
                        short_peer(&peer)
                    );
                    return MemberResponse::Refused;
                }
                let group = entry.group_id;
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                let Some((ticket_id, heads)) = slot.join_offers.remove(&peer) else {
                    return MemberResponse::Refused;
                };
                // Let in while it was away: it only needs the log. Its join is signed now on
                // the heads just offered, so an old one copied from the log does not count.
                let fresh = sorted(entry.parents.clone()) == sorted(heads.clone())
                    && slot.ctx.log().get(&entry.hash()).is_none();
                if fresh
                    && entry.signature_valid()
                    && slot.ctx.joined_with(&entry.author, &ticket_id)
                {
                    let mut parts = slot.ctx.encoded_parts();
                    if parts.len() <= 1 {
                        let entries = parts
                            .pop()
                            .unwrap_or_default()
                            .into_iter()
                            .map(serde_bytes::ByteBuf::from)
                            .collect();
                        return MemberResponse::JoinAccepted { entries };
                    }
                    // Too long for one answer: it goes in parts, as to a device let in now.
                    self.admit(group, peer);
                    return MemberResponse::JoinWaiting;
                }
                let Some(name) = slot.ctx.join_valid(&entry, &ticket_id, &heads, now_ms()) else {
                    return MemberResponse::Refused;
                };
                let fogata_state::Body::Join { ticket, .. } = &entry.body else {
                    return MemberResponse::Refused;
                };
                // An invite does not let anyone in by itself: a manager says yes first, so a
                // link passed on to someone else is seen for what it is.
                let device = entry.author;
                let ticket = ticket.clone();
                let Some(asked) = slot.wait_for_answer(
                    device,
                    WaitingJoin {
                        peer,
                        entry,
                        ticket,
                    },
                ) else {
                    return MemberResponse::Refused;
                };
                if asked {
                    self.emit(NodeEvent::JoinAsked {
                        group,
                        device,
                        name,
                    });
                }
                MemberResponse::JoinWaiting
            }
            MemberRequest::RoomSync { group_id, from } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                if !self.is_member_of(&group, &peer) {
                    return MemberResponse::Refused;
                }
                let Some(device) = self.may_serve(peer) else {
                    return MemberResponse::Refused;
                };
                let (messages, more) = self.room_snapshot(group, from);
                self.charge_served(device, messages.iter().map(wire_bytes).sum());
                MemberResponse::RoomLog { messages, more }
            }
            MemberRequest::Addresses { addresses } => {
                // Kept for a peer that may still be proving its membership (both ends verify
                // each other at once), but only ever dialled for members.
                if !self.swarm.is_connected(&peer) {
                    return MemberResponse::Refused;
                }
                let addresses = lan_quic(addresses);
                let changed = self.member_lan.get(&peer) != Some(&addresses);
                self.member_lan.insert(peer, addresses);
                // A first try that only knew public addresses may have failed.
                if changed
                    && self
                        .lan_redialled
                        .get(&peer)
                        .is_none_or(|t| t.elapsed() >= LAN_REDIAL)
                {
                    self.lan_redialled.insert(peer, Instant::now());
                    self.quic_tried.remove(&peer);
                }
                self.want_datagrams(peer);
                MemberResponse::Ack
            }
            MemberRequest::Signal { group_id, payload } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                if !self.is_member_of(&group, &peer) {
                    return MemberResponse::Refused;
                }
                let Some(from) = self.members.get(&peer).map(|m| m.device) else {
                    return MemberResponse::Refused;
                };
                // Calls are set up in a voice room, by its participants, with a few small
                // signals: none goes to a device in none of the group's rooms, and a member
                // that sends large or many is not heard. Nothing of a call with someone kept
                // out of the voice room it is for. A member not in this device's room is
                // heard only a few times now and then: its join may still be on its way, and
                // the application keeps those few until it comes.
                let Some(in_my_room) = self
                    .groups
                    .get(&group)
                    .and_then(|s| s.joined())
                    .map(|(_, room)| room.is_participant(&from))
                else {
                    return MemberResponse::Refused;
                };
                let now = Instant::now();
                if payload.len() > limits::MAX_SIGNAL_BYTES
                    || self.kept_out_of_my_room(&group, &from)
                    || !self.limits.take_signal(peer, now)
                    || (!in_my_room && !self.limits.take_early_signal(peer, now))
                {
                    tracing::debug!(%peer, bytes = payload.len(), "call signal dropped");
                    return MemberResponse::Refused;
                }
                self.emit(NodeEvent::Signal {
                    group,
                    from,
                    payload,
                });
                MemberResponse::Ack
            }
            MemberRequest::Admitted {
                group_id,
                entries,
                part,
            } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self
                    .groups
                    .get_mut(&group)
                    .filter(|s| s.ctx.joining().is_some() && s.join_committed.contains(&peer))
                else {
                    return MemberResponse::Refused;
                };
                let Some((index, count)) = part else {
                    slot.parts_in.remove(&peer);
                    self.on_admitted(group, peer, entries);
                    return MemberResponse::Ack;
                };
                if self.take_admitted_part(group, peer, index, count, entries) {
                    MemberResponse::Ack
                } else {
                    MemberResponse::Refused
                }
            }
            MemberRequest::JoinDeclined { group_id, revoke } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                if slot.ctx.joining().is_none() {
                    return MemberResponse::Refused;
                }
                // Only the manager who wrote the invite can sign its `Revoke`: anyone else who
                // learnt the group's id, another member among them, could otherwise end the
                // join. Another manager's no reaches this device once the issuer confirms it.
                if !revoke.is_some_and(|r| slot.ctx.refused_by_issuer(&r)) {
                    tracing::info!(
                        "{} said a manager refused the join, without the inviting manager's \
                         signature; still waiting",
                        short_peer(&peer)
                    );
                    // Shown the invite again later, it may have the signed one by then.
                    slot.join_sent.remove(&peer);
                    return MemberResponse::Refused;
                }
                self.emit(NodeEvent::JoinDeclined { group });
                MemberResponse::Ack
            }
            MemberRequest::Direct { group_id, messages } => {
                self.on_direct_request(peer, &group_id, messages)
            }
            MemberRequest::OtherSession => self.on_other_session(peer),
            MemberRequest::FriendAsk { sealed } => self.on_member_friend_ask(peer, &sealed),
            MemberRequest::KeyProblem { group_id, epoch } => {
                let Some(group) = group_of(&group_id) else {
                    return MemberResponse::Refused;
                };
                if !self.is_member_of(&group, &peer) {
                    return MemberResponse::Refused;
                }
                let Some(device) = self.members.get(&peer).map(|m| m.device) else {
                    return MemberResponse::Refused;
                };
                let Some(slot) = self.groups.get_mut(&group) else {
                    return MemberResponse::Refused;
                };
                let new = slot.ctx.mend_reported_key(&device, epoch, now_ms());
                self.after_group_change(group, new, None);
                MemberResponse::Ack
            }
            MemberRequest::Presence { hidden, away } => {
                let Some(member) = self.members.get(&peer) else {
                    return self.keep_early(peer, |e| e.presence = Some((hidden, away)));
                };
                let device = member.device;
                let away = away && !hidden;
                self.emit(NodeEvent::MemberPresence {
                    device,
                    hidden,
                    away,
                });
                MemberResponse::Ack
            }
            MemberRequest::Profile { profile, animation } => {
                let Some(member) = self.members.get(&peer) else {
                    // Checked now: only what can count waits, within a bound in all.
                    let held = self
                        .early
                        .iter()
                        .filter(|(p, _)| **p != peer)
                        .map(|(_, e)| e.bytes())
                        .sum();
                    let Some(profile) = early_profile(profile, animation, held) else {
                        return MemberResponse::Refused;
                    };
                    return self.keep_early(peer, |e| e.profile = Some(profile));
                };
                let device = member.device;
                match fogata_friends::Profile::decode(&profile) {
                    Ok(profile) if profile.device == device => {
                        // At most one every few seconds from each (see limits.rs).
                        if let Some(profile) = self.limits.member_profile(profile, Instant::now()) {
                            self.emit(NodeEvent::MemberProfile { profile });
                        }
                        // Its picture's animation, signed by the same device; a bad one
                        // leaves the still picture.
                        if let Some(animation) = animation
                            .and_then(|a| fogata_friends::Animation::decode(&a).ok())
                            .filter(|a| {
                                a.device == device && a.of == fogata_friends::Animated::Photo
                            })
                            && let Some(animation) =
                                self.limits.animation(animation, Instant::now())
                        {
                            self.emit(NodeEvent::Animation { animation });
                        }
                        MemberResponse::Ack
                    }
                    _ => MemberResponse::Refused,
                }
            }
        }
    }

    pub(super) fn on_response(&mut self, peer: PeerId, kind: Pending, response: MemberResponse) {
        // Whatever the answer, the next sync with that member may go.
        if let Pending::Sync(group) = kind
            && let Some(slot) = self.groups.get_mut(&group)
        {
            slot.syncing.remove(&peer);
        }
        match (kind, response) {
            (Pending::Challenge(group, challenge), MemberResponse::Proof(wire)) => {
                self.on_proof(peer, group, challenge, &wire)
            }
            (Pending::Challenge(group, challenge), _) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.forget_challenge(&peer, &challenge);
                }
                self.drop_early_unless_asked(peer);
            }
            (Pending::Roster(group), MemberResponse::Roster { peers }) => {
                let listed: HashSet<PeerId> = peers
                    .iter()
                    .take(MAX_ROSTER)
                    .filter_map(|bytes| PeerId::from_bytes(bytes).ok())
                    .collect();
                let Some(slot) = self.groups.get_mut(&group) else {
                    return;
                };
                // Only a member's list counts, and only so many new peers of it are tried
                // at once: each is a dial through that member, which it could answer slowly.
                if !slot.members.contains(&peer) {
                    return;
                }
                slot.rosters.insert(peer, (listed.clone(), Instant::now()));
                let mut dials = 0;
                for other in listed {
                    if !self.swarm.is_connected(&other) {
                        if dials == MAX_ROSTER_DIALS {
                            continue;
                        }
                        dials += 1;
                    }
                    self.reach_through(peer, other, group);
                }
            }
            (Pending::Sync(group), MemberResponse::SyncReply { entries, want }) => {
                if !self.groups.contains_key(&group) {
                    return;
                }
                let entries: Vec<Vec<u8>> = entries.into_iter().map(|e| e.into_vec()).collect();
                let sent = entries.len();
                let entries = self.within_entry_budget(peer, entries);
                let Some(slot) = self.groups.get_mut(&group) else {
                    return;
                };
                let taken = slot.ctx.insert_encoded(&entries);
                let (new, refused) = (taken.new, taken.refused);
                // An answer holds a part of what this device lacks: while one brings new
                // entries, all of them within the member's budget and none refused, the next
                // is asked for at once rather than at the next roster.
                let again = !new.is_empty() && entries.len() == sent && refused.is_empty();
                // As much as one message carries: the rest goes at the next sync.
                let want = hashes(&want);
                let wanted: Vec<SharedBytes> = slot
                    .ctx
                    .encoded_part(want)
                    .into_iter()
                    .map(SharedBytes::from)
                    .collect();
                self.refused_entries(&refused);
                self.after_group_change(group, new, Some(peer));
                if !wanted.is_empty() {
                    self.request(
                        peer,
                        MemberRequest::Push {
                            group_id: group.as_bytes().to_vec(),
                            entries: wanted,
                        },
                        Pending::Push,
                    );
                }
                if again {
                    self.request_sync(peer, group);
                }
            }
            (Pending::Join(group), MemberResponse::JoinOffer { heads }) => {
                let Some(slot) = self.groups.get(&group) else {
                    return;
                };
                if let Some(entry) = slot.ctx.sign_join(hashes(&heads), now_ms()) {
                    if let Some(slot) = self.groups.get_mut(&group) {
                        slot.join_committed.insert(peer);
                    }
                    let binding = entry.join_binding(
                        &self.identity,
                        &self.local_peer().to_bytes(),
                        &peer.to_bytes(),
                    );
                    self.request(
                        peer,
                        MemberRequest::JoinCommit {
                            entry: entry.encode(),
                            binding: Some(serde_bytes::ByteBuf::from(binding.to_vec())),
                        },
                        Pending::JoinCommit(group),
                    );
                }
            }
            // Not opened while the application is far behind: asked for again later, the
            // room whole of one member (see `room_heartbeat`), events by id of this one.
            // The member gave it and may give it again: it is one to ask once more, however
            // many stalls came, so the room is never left unasked.
            (Pending::RoomSync(group, _), MemberResponse::RoomLog { .. }) if self.events.full() => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.room_again.get_or_insert_default().remove(&peer);
                    slot.room_asked.remove(&peer);
                }
            }
            (Pending::RoomEvents(group), MemberResponse::RoomLog { .. }) if self.events.full() => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.room_asked.remove(&peer);
                }
            }
            (Pending::RoomSync(group, from), MemberResponse::RoomLog { messages, more }) => {
                // The room came whole from a member: nothing to ask again.
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.room_again = None;
                }
                let all = self.open_room_log(peer, group, messages, false);
                // The rest comes in further parts; each one must move on.
                if let Some(next) = more.filter(|n| all && *n > from && *n <= MAX_ROOM_EVENTS) {
                    self.request(
                        peer,
                        MemberRequest::RoomSync {
                            group_id: group.as_bytes().to_vec(),
                            from: next,
                        },
                        Pending::RoomSync(group, next),
                    );
                }
            }
            (Pending::RoomEvents(group), MemberResponse::RoomLog { messages, .. }) => {
                self.open_room_log(peer, group, messages, true);
            }
            (Pending::RoomSync(group, _), _) => {
                // Probably asked before the member had verified us; ask again later.
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.room_asked.remove(&peer);
                }
            }
            (Pending::Join(group) | Pending::JoinCommit(group), MemberResponse::Refused) => {
                // Try again later, with this or another member; one that refused the signed
                // join does not let this device in.
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.join_sent.remove(&peer);
                    slot.stop_join_with(&peer);
                }
            }
            (Pending::JoinCommit(group), MemberResponse::JoinWaiting) => {
                // This manager asks its person; it answers with `Admitted` or `JoinDeclined`.
                // The others are still asked, so any manager can let the device in. Only
                // while still joining: the answer can come after the `Admitted` that let
                // this device in, and would say it still waits.
                if self
                    .groups
                    .get(&group)
                    .is_some_and(|s| s.ctx.joining().is_some())
                {
                    self.emit(NodeEvent::JoinWaiting { group });
                }
            }
            (Pending::Join(group), MemberResponse::JoinDeclined) => {
                // Unsigned, so anyone could say it: an updated manager sends its signed
                // `Revoke` along (`MemberRequest::JoinDeclined`); an older one is asked
                // again later, and the join ends when the invite runs out.
                tracing::debug!(
                    "{} answered the invite with an unsigned no",
                    short_peer(&peer)
                );
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.join_sent.remove(&peer);
                }
            }
            (Pending::JoinCommit(group), MemberResponse::JoinAccepted { entries }) => {
                self.on_admitted(group, peer, entries);
            }
            (Pending::Roster(group), MemberResponse::Refused) => self.roster_refused(group, peer),
            (Pending::Leave(group), MemberResponse::Ack) => {
                if self.is_member_of(&group, &peer) {
                    self.leave_taken(group);
                }
            }
            (Pending::Refusal(group), _) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.refusing.remove(&peer);
                }
            }
            (Pending::AdmittedPart(group), response) => {
                self.part_answered(group, peer, matches!(response, MemberResponse::Ack));
            }
            (Pending::OtherSession(device, check), response) => {
                let answered = matches!(response, MemberResponse::Ack);
                self.on_other_session_answer(peer, device, check, answered);
            }
            // An earlier version takes smaller banners: it keeps the one it has, and is not
            // sent this one again at every roster.
            (Pending::Banner(group, digest), response) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.banner_answered(peer, digest);
                    if matches!(response, MemberResponse::Refused) {
                        tracing::debug!("{} refused the group's banner", short_peer(&peer));
                        slot.banner_refused.insert(peer, digest);
                    } else {
                        slot.banner_refused.remove(&peer);
                    }
                }
            }
            (Pending::Picture(group, digest), response) => {
                if let Some(slot) = self.groups.get_mut(&group) {
                    slot.picture_answered(peer, digest);
                    if matches!(response, MemberResponse::Refused) {
                        tracing::debug!("{} refused the group's picture", short_peer(&peer));
                        slot.picture_refused.insert(peer, digest);
                    } else {
                        slot.picture_refused.remove(&peer);
                    }
                }
            }
            (Pending::FriendAsk(device, fresh), response) => {
                self.member_ask_answered(device, fresh, matches!(response, MemberResponse::Ack))
            }
            // Refused while the member was still checking this device, say.
            (Pending::Presence(sent), response) => {
                self.presence_done(peer, sent, matches!(response, MemberResponse::Ack))
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fogata_crypto::DeviceIdentity;
    use fogata_state::{Body, GroupLog};
    use libp2p::Multiaddr;

    use super::*;
    use crate::{
        Command, NodeConfig,
        group::GroupSetup,
        node::{
            start,
            tests::{add_member, inspect, lone_member},
        },
    };

    /// A member asking for the log and the rooms' past again and again is answered up to
    /// its budget, and refused after it, while another member is still answered.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_asking_over_and_over_is_answered_up_to_its_budget() {
        let (node, group) = lone_member();
        let (answered, other) = inspect(&node, move |n| {
            let (bea, cai) = (PeerId::random(), PeerId::random());
            add_member(n, group, bea);
            add_member(n, group, cai);
            let asks = |group: GroupId| {
                [
                    MemberRequest::Sync {
                        group_id: group.as_bytes().to_vec(),
                        have: Vec::new(),
                    },
                    MemberRequest::RoomSync {
                        group_id: group.as_bytes().to_vec(),
                        from: 0,
                    },
                    MemberRequest::Roster {
                        group_id: group.as_bytes().to_vec(),
                        heads: None,
                        banner: None,
                        picture: None,
                    },
                ]
            };
            let mut answered = 0;
            for _ in 0..10_000 {
                for ask in asks(group) {
                    if !matches!(n.on_request(bea, ask), MemberResponse::Refused) {
                        answered += 1;
                    }
                }
            }
            let other = asks(group)
                .into_iter()
                .all(|ask| !matches!(n.on_request(cai, ask), MemberResponse::Refused));
            (answered, other)
        })
        .await;
        // Each answer counts at least `SERVED_LEAST`: about 8192 of them in a burst, and a
        // few more for the time the asks take.
        let most = 32 * 1024 * 1024 / limits::SERVED_LEAST + 1024;
        assert!(answered <= most, "{answered} answers");
        assert!(answered >= most / 2, "only {answered} answers");
        assert!(other);
        node.shutdown().await.unwrap();
    }

    /// The member requests of `kind` to `peer` on their way or waiting their turn.
    fn requests_of(n: &Node, peer: PeerId, kind: impl Fn(&Pending) -> bool) -> usize {
        let on_its_way = n
            .pending
            .values()
            .filter(|(p, k)| *p == peer && kind(k))
            .count();
        let waiting = n
            .outbox
            .get(&peer)
            .map_or(0, |o| o.waiting.iter().filter(|(_, k)| kind(k)).count());
        on_its_way + waiting
    }

    /// A member that asks for rosters again and again, each time naming a banner, a picture
    /// record and heads other than this device's, is sent the banner and the picture once
    /// and one sync at a time, not one of each per ask: each banner is up to a few hundred
    /// KiB of this device's upload. Once a sync is answered, the next ask gets the next.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_asking_rosters_over_and_over_is_sent_the_banner_once() {
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = PeerId::random();
            add_member(n, group, bea);
            let slot = n.groups.get_mut(&group).unwrap();
            slot.ctx.set_banner(Vec::new(), now_ms()).unwrap();
            slot.ctx.set_picture(Vec::new(), now_ms()).unwrap();
            let ask = |i: u8| MemberRequest::Roster {
                group_id: group.as_bytes().to_vec(),
                heads: Some(serde_bytes::ByteBuf::from(vec![i; 16])),
                banner: Some(serde_bytes::ByteBuf::from(vec![i; 16])),
                picture: Some(serde_bytes::ByteBuf::from(vec![i; 16])),
            };
            for i in 0..100 {
                assert!(matches!(
                    n.on_request(bea, ask(i)),
                    MemberResponse::Roster { .. }
                ));
            }
            let counts = |n: &Node| {
                (
                    requests_of(n, bea, |k| matches!(k, Pending::Banner(..))),
                    requests_of(n, bea, |k| matches!(k, Pending::Picture(..))),
                    requests_of(n, bea, |k| matches!(k, Pending::Sync(_))),
                )
            };
            let before = counts(n);
            // The sync is answered, with nothing: the next ask syncs again, and the banner,
            // answered too, is still not sent again.
            let digest =
                super::super::group_changes::banner_key(&n.groups[&group].ctx.banner_digest());
            n.on_response(
                bea,
                Pending::Sync(group),
                MemberResponse::SyncReply {
                    entries: Vec::new(),
                    want: Vec::new(),
                },
            );
            n.on_response(bea, Pending::Banner(group, digest), MemberResponse::Ack);
            n.on_request(bea, ask(200));
            (before, counts(n))
        })
        .await;
        assert_eq!(found, ((1, 1, 1), (1, 1, 2)));
        node.shutdown().await.unwrap();
    }

    /// While the application is far behind, a member's group and direct messages and call
    /// signals are refused, for it to send them again, and room events asked for are not
    /// opened but the room is asked for again later (see `ask_rooms_again`); the rest of the
    /// member protocol goes on.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_adds_no_events_while_the_application_is_far_behind() {
        use fogata_proto::{MemberResponse, WireMessage};
        let (node, group) = lone_member();
        let found = inspect(&node, move |n| {
            let bea = PeerId::random();
            add_member(n, group, bea);
            let wire = || WireMessage {
                version: 1,
                group_id: group.as_bytes().to_vec(),
                epoch: 0,
                sender: vec![0; 32],
                timestamp_ms: 0,
                message_id: vec![0; 16],
                nonce: vec![0; 24],
                ciphertext: vec![0; 64],
                signature: vec![0; 64],
            };
            let answers = |n: &mut Node| {
                let deliver = n.on_request(bea, MemberRequest::Deliver(wire()));
                let presence = n.on_request(
                    bea,
                    MemberRequest::Presence {
                        hidden: false,
                        away: false,
                    },
                );
                n.groups
                    .get_mut(&group)
                    .unwrap()
                    .room_asked
                    .insert(bea, Instant::now());
                n.on_response(
                    bea,
                    Pending::RoomSync(group, 0),
                    MemberResponse::RoomLog {
                        messages: Vec::new(),
                        more: None,
                    },
                );
                (
                    matches!(deliver, MemberResponse::Ack),
                    matches!(presence, MemberResponse::Ack),
                    n.groups[&group].room_again.is_some(),
                )
            };
            let before = answers(n);
            for _ in 0..crate::events::HARD_LIMIT {
                n.events.send(NodeEvent::OtherSession);
            }
            (before, answers(n))
        })
        .await;
        assert_eq!(found, ((true, true, false), (false, true, true)));
        node.shutdown().await.unwrap();
    }

    /// A member sends this device message after message of log entries signed with its own
    /// key that do not verify, and of group messages; meanwhile the node must go on answering.
    /// Prints how long it took to answer at worst. Ignored by default: it is meant for release
    /// builds, where a signature check costs what it costs on a real device
    /// (`cargo test --release -p fogata-net -- --ignored flooding`).
    #[ignore]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_member_flooding_entries_and_messages_leaves_the_node_answering() {
        use fogata_proto::{MemberRequest, WireMessage};
        let ana = Arc::new(DeviceIdentity::generate().unwrap());
        let bea = Arc::new(DeviceIdentity::generate().unwrap());
        let mut log = GroupLog::create(&ana, "g", 1).unwrap();
        log.add(&ana, bea.public_key(), "bea", 2).unwrap();
        let group = log.group_id();
        let config = |identity: Arc<DeviceIdentity>, port: u16, peers: Vec<Multiaddr>| NodeConfig {
            identity,
            groups: vec![GroupSetup::Member(log.clone())],
            friends: None,
            listen_port: port,
            bootstrap: Vec::new(),
            use_dht: false,
            use_mdns: false,
            bridge: false,
            peers,
            remembered: Vec::new(),
            hidden: false,
        };
        let port = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let at: Multiaddr = format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
            .parse()
            .unwrap();
        let (node_a, _events_a) = start(config(ana.clone(), port, Vec::new())).unwrap();
        let (node_b, mut events_b) = start(config(bea.clone(), 0, vec![at])).unwrap();
        let ana_peer = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                match events_b.recv().await {
                    Some(NodeEvent::MemberConnected { device, peer, .. })
                        if device == ana.public_key() =>
                    {
                        return peer;
                    }
                    Some(_) => {}
                    None => panic!("Bea's node stopped"),
                }
            }
        })
        .await
        .expect("Bea reached Ana");
        // As many of the smallest entries as a message carries, in Bea's name, each with a
        // signature that does not verify.
        let heads = log.heads();
        let entries: Vec<fogata_proto::SharedBytes> = (0..24_000u64)
            .map(|i| {
                let mut bytes =
                    Entry::sign(&bea, group, heads.clone(), 10 + i, Body::Leave).encode();
                let n = bytes.len();
                bytes[n - 1] ^= 1;
                bytes.into()
            })
            .collect();
        let message = WireMessage {
            version: 1,
            group_id: group.as_bytes().to_vec(),
            epoch: 0,
            sender: bea.public_key().as_bytes().to_vec(),
            timestamp_ms: 1,
            message_id: vec![1; 16],
            nonce: vec![0; 24],
            ciphertext: vec![0; 150 * 1024],
            signature: vec![0; 64],
        };
        let flood = async {
            for round in 0..10u8 {
                let push = MemberRequest::Push {
                    group_id: group.as_bytes().to_vec(),
                    entries: entries.clone(),
                };
                node_b
                    .commands
                    .send(Command::Request(ana_peer, Box::new(push)))
                    .await
                    .unwrap();
                for k in 0..200u8 {
                    let mut wire = message.clone();
                    wire.message_id = vec![round, k, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1];
                    let spread = MemberRequest::Spread {
                        message: wire,
                        reached: Vec::new(),
                    };
                    node_b
                        .commands
                        .send(Command::Request(ana_peer, Box::new(spread)))
                        .await
                        .unwrap();
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        };
        let watch = async {
            let mut worst = Duration::ZERO;
            let until = Instant::now() + Duration::from_secs(12);
            while Instant::now() < until {
                let asked = Instant::now();
                node_a.addresses().await.unwrap();
                worst = worst.max(asked.elapsed());
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            worst
        };
        let ((), worst) = tokio::join!(flood, watch);
        eprintln!("longest wait for an answer while flooded: {worst:?}");
        assert!(worst < Duration::from_secs(1), "{worst:?}");
        let kept = node_a.group(group).await.unwrap().unwrap();
        assert_eq!(kept.len(), log.len());
    }

    /// A challenge lost with a connection that closed while another stays (both ends
    /// dialled at once, one let go) is asked again at once on the one left: the other end
    /// may have this device's proof already and ask nothing more, and nothing else asks
    /// again, so it stayed unchecked here, every request of it refused, as long as it was
    /// connected.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_challenge_lost_with_a_closed_connection_is_asked_again_on_the_other() {
        use libp2p::{multiaddr::Protocol, swarm::ConnectionId};
        let (ana, group) = lone_member();
        let (bea, _events) = start(NodeConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            groups: Vec::new(),
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
        let bea_peer = inspect(&bea, |n| n.local_peer()).await;
        let (ana_peer, at) = loop {
            let found = inspect(&ana, |n| {
                let at = n
                    .swarm
                    .listeners()
                    .find(|a| a.to_string().starts_with("/ip4/127.0.0.1/udp/"))
                    .cloned();
                (n.local_peer(), at)
            })
            .await;
            if let (peer, Some(at)) = found {
                break (peer, at);
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        };
        bea.dial(vec![at.with(Protocol::P2p(ana_peer))], None)
            .await
            .unwrap();
        let started = std::time::Instant::now();
        while !inspect(&ana, move |n| n.swarm.is_connected(&bea_peer)).await {
            assert!(started.elapsed().as_secs() < 30, "Bea never connected");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let (lost, asked_again) = inspect(&ana, move |n| {
            let challenged = |n: &Node| {
                n.pending
                    .iter()
                    .find(|(_, (p, k))| *p == bea_peer && matches!(k, Pending::Challenge(..)))
                    .map(|(id, _)| *id)
            };
            if challenged(n).is_none() {
                n.challenge_with(bea_peer, group, None);
            }
            let lost = challenged(n).expect("a challenge on its way");
            n.on_member_event(request_response::Event::OutboundFailure {
                peer: bea_peer,
                connection_id: ConnectionId::new_unchecked(1),
                request_id: lost,
                error: request_response::OutboundFailure::ConnectionClosed,
            });
            let again = challenged(n);
            (lost, again)
        })
        .await;
        assert!(
            asked_again.is_some_and(|id| id != lost),
            "the challenge was not asked again"
        );
        ana.shutdown().await.unwrap();
        bea.shutdown().await.unwrap();
    }

    /// A banner or picture record older than the one kept is dropped before its signature is
    /// checked: one whose signature is broken is answered as any older one, while a newer
    /// one is checked and refused, and a newer one well signed is taken, as before.
    #[tokio::test(flavor = "multi_thread")]
    async fn an_older_record_is_dropped_before_its_signature_is_checked() {
        use fogata_state::{GroupBanner, GroupPicture};
        let (node, group) = lone_member();
        let answers = inspect(&node, move |n| {
            let bea = PeerId::random();
            add_member(n, group, bea);
            let now = now_ms();
            let gif = b"GIF89a".to_vec();
            let slot = n.groups.get_mut(&group).unwrap();
            slot.ctx.set_banner(gif.clone(), now).unwrap();
            slot.ctx.set_picture(gif.clone(), now).unwrap();
            let identity = n.identity.clone();
            // Forged: dated as signed, but the date changed after.
            let forged_banner = |time_ms: u64| {
                let mut banner = GroupBanner::sign(&identity, group, gif.clone(), 1).unwrap();
                banner.time_ms = time_ms;
                MemberRequest::GroupBanner {
                    group_id: group.as_bytes().to_vec(),
                    banner: banner.encode(),
                }
            };
            let forged_picture = |time_ms: u64| {
                let mut picture = GroupPicture::sign(&identity, group, gif.clone(), 1).unwrap();
                picture.time_ms = time_ms;
                MemberRequest::GroupPicture {
                    group_id: group.as_bytes().to_vec(),
                    picture: picture.encode(),
                }
            };
            let newer = MemberRequest::GroupBanner {
                group_id: group.as_bytes().to_vec(),
                banner: GroupBanner::sign(&identity, group, Vec::new(), now + 1)
                    .unwrap()
                    .encode(),
            };
            let damaged = MemberRequest::GroupBanner {
                group_id: group.as_bytes().to_vec(),
                banner: vec![1, 2, 3],
            };
            let answers = [
                forged_banner(now - 1),
                forged_picture(now - 1),
                forged_banner(now + 1),
                forged_picture(now + 1),
                damaged,
                newer,
            ]
            .map(|request| matches!(n.on_request(bea, request), MemberResponse::Ack));
            let taken = n.groups[&group]
                .ctx
                .log()
                .banner_record()
                .is_some_and(|b| b.time_ms == now + 1 && b.picture.is_empty());
            (answers, taken)
        })
        .await;
        assert_eq!(answers, ([true, true, false, false, false, true], true));
        node.shutdown().await.unwrap();
    }
}
