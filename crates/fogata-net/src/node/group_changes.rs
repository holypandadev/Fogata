//! What a change to a group's log brings: members removed let go (and what the relay gave
//! them), keys and rooms brought up to date, the change pushed to the other members, and
//! the group's banner and picture sent to those that keep them.

use std::time::{Duration, Instant};

use fogata_crypto::GroupId;
use fogata_proto::{MemberRequest, SharedBytes};
use fogata_state::{EntryHash, GroupLog};
use libp2p::{
    Multiaddr, PeerId,
    swarm::dial_opts::{DialOpts, PeerCondition},
};

use super::{Node, Pending, SEARCH_HARD, now_ms};
use crate::{NodeEvent, Path};

/// How long after a peer the relay no longer serves was disconnected it is dialled again
/// ([`Node::let_go`]).
const REDIAL_LET_GO: Duration = Duration::from_secs(3);
/// How long after a call ends here its people keep what the relay gave them, unless it
/// serves them again by then (another call, a group): enough to go back into the call, not
/// the hour a slot lasts or the day a way through lasts. A call still going on between
/// others through this device then needs another way.
pub(super) const CALL_GRACE: Duration = Duration::from_secs(5 * 60);

/// A banner's or picture record's digest as kept for [`Pending::Banner`] and
/// [`Pending::Picture`]; all zeros for none.
pub(super) fn banner_key(digest: &[u8]) -> [u8; 32] {
    digest.try_into().unwrap_or_default()
}

impl Node {
    /// Disconnects a peer just removed from a group that is neither a member of a group here
    /// nor a friend. One still here as a friend or through a direct chat, and no longer a
    /// member of a group the relay serves, loses what the relay gave it: a slot lasts an hour
    /// and a way through for as long as it is used, and both live on the connections they
    /// came over, so those close if it had either, and it is dialled again a moment later
    /// (once the closed connections are gone at both ends), as the friend it still is, on a
    /// connection that carries neither.
    pub(super) fn let_go(&mut self, peer: PeerId) {
        if !self.members.contains_key(&peer) && !self.is_friend_peer(&peer) {
            let _ = self.swarm.disconnect_peer_id(peer);
            return;
        }
        if crate::locked(&self.relay_peers).contains(&peer) {
            return;
        }
        self.close_relayed_connections(peer);
        if !self.relay_users.remove(&peer) {
            return;
        }
        let addresses: Vec<Multiaddr> = self
            .connections
            .values()
            .filter(|c| c.peer == peer && !matches!(c.path, Path::ThroughMember { .. }))
            .map(|c| c.address.clone())
            .collect();
        tracing::debug!(%peer, "no longer served by the relay: connected anew");
        let _ = self.swarm.disconnect_peer_id(peer);
        if !addresses.is_empty() {
            self.relay_redial
                .insert(peer, (Instant::now() + REDIAL_LET_GO, addresses));
        }
    }

    /// Dials again the peers [`Self::let_go`] disconnected, once it is time, and lets go the
    /// people of a call that ended (see [`CALL_GRACE`]).
    pub(super) fn redial_let_go(&mut self) {
        let now = Instant::now();
        let over: Vec<PeerId> = self
            .after_call
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(peer, _)| *peer)
            .collect();
        for peer in over {
            self.after_call.remove(&peer);
            if !crate::locked(&self.relay_peers).contains(&peer) {
                self.let_go(peer);
            }
        }
        let due: Vec<PeerId> = self
            .relay_redial
            .iter()
            .filter(|(_, (at, _))| *at <= now)
            .map(|(peer, _)| *peer)
            .collect();
        for peer in due {
            let Some((_, addresses)) = self.relay_redial.remove(&peer) else {
                continue;
            };
            self.wanted.want(peer);
            self.dial_opts(
                DialOpts::peer_id(peer)
                    .addresses(addresses)
                    .condition(PeerCondition::Disconnected)
                    .build(),
            );
        }
    }

    /// Forgets that `peer` is a member of `group`; a peer left in no group stops being a
    /// member at all.
    pub(super) fn drop_member(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        if !slot.members.remove(&peer) {
            return;
        }
        slot.rosters.remove(&peer);
        slot.roster_refused.remove(&peer);
        slot.roster_heard.remove(&peer);
        slot.banner_peers.remove(&peer);
        slot.banner_refused.remove(&peer);
        slot.picture_peers.remove(&peer);
        slot.picture_refused.remove(&peer);
        // Someone just went, maybe to another network: look for them at every slot for a
        // while, since this side has to learn the new address to punch through to it.
        slot.quiet_lookups = 0;
        slot.searching_until = Some(Instant::now() + SEARCH_HARD);
        slot.last_lookup = None;
        let in_other = self.groups.values().any(|s| s.members.contains(&peer));
        let device = if in_other {
            self.members.get(&peer).map(|m| m.device)
        } else {
            self.members.remove(&peer).map(|m| m.device)
        };
        self.publish_members();
        if let Some(device) = device {
            self.emit(NodeEvent::MemberDisconnected {
                group,
                device,
                peer,
            });
        }
    }

    /// Spreads new log entries of `group` to its members, drops peers that stopped being
    /// members and reports the change. When this device is no longer a member, the node stops
    /// running the group.
    pub(super) fn after_group_change(
        &mut self,
        group: GroupId,
        mut new: Vec<EntryHash>,
        from: Option<PeerId>,
    ) {
        // Someone left or a rotation was discarded: the member admitted first rotates now.
        // A manager that was not given the current key makes a new one.
        let one_above = self.manager_above_here(group);
        if let Some(slot) = self.groups.get_mut(&group) {
            new.extend(slot.ctx.rotate_if_leaked(now_ms(), false));
            new.extend(slot.ctx.mend_key(now_ms(), one_above));
        }
        if new.is_empty() {
            return;
        }
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        // Held once for every member it goes to (see `SharedBytes`).
        let entries: Vec<SharedBytes> = slot
            .ctx
            .encoded(new.iter().copied())
            .into_iter()
            .map(SharedBytes::from)
            .collect();
        let peers: Vec<PeerId> = slot
            .members
            .iter()
            .copied()
            .filter(|p| Some(*p) != from)
            .collect();
        let removed: Vec<PeerId> = slot
            .members
            .iter()
            .filter(|p| {
                self.members
                    .get(p)
                    .is_some_and(|m| !slot.ctx.is_member_device(&m.device))
            })
            .copied()
            .collect();
        // A member that takes this device's leave has it: the group may stop running here.
        let kind = if slot.ctx.my_leave().is_some_and(|l| new.contains(&l)) {
            Pending::Leave(group)
        } else {
            Pending::Push
        };
        for peer in peers.iter().filter(|_| !entries.is_empty()) {
            self.request(
                *peer,
                MemberRequest::Push {
                    group_id: group.as_bytes().to_vec(),
                    entries: entries.clone(),
                },
                kind,
            );
        }
        for peer in removed {
            self.drop_member(group, peer);
            // The log that tells it is on its way to it: disconnecting now would drop it,
            // and the device would go on as a member until it next connected.
            if self.outbox.contains_key(&peer) {
                self.farewell.insert(peer);
            } else {
                self.let_go(peer);
            }
        }
        self.emit(NodeEvent::GroupChanged(group));
        self.rooms_follow_log(group);
        self.forget_settled_joins(group);
        self.group_changed_for_friends(group);
        let slot = self.groups.get_mut(&group).expect("checked above");
        if slot.ctx.i_am_member() {
            // Back in (added again): no leave to deliver any more.
            slot.leaving = None;
        } else if slot.ctx.joining().is_none() {
            // Left: the group is gone for the application at once, but runs on here until a
            // member has the leave (when any member is left to have it).
            if let Some(leave) = slot.ctx.my_leave().filter(|_| slot.ctx.member_count() > 0) {
                let told = slot.leaving.replace(leave).is_some();
                let taken = from.is_some() && slot.ctx.signed_on_by_others(&new, &leave);
                let log = slot.ctx.log().clone();
                if !told {
                    self.emit(NodeEvent::Removed { group, log });
                }
                if taken {
                    self.leave_taken(group);
                }
                return;
            }
            let peers: Vec<PeerId> = slot.members.iter().copied().collect();
            for peer in peers {
                self.drop_member(group, peer);
            }
            let slot = self.groups.remove(&group).expect("checked above");
            self.publish_members();
            self.provided.1 = None;
            self.emit(NodeEvent::Removed {
                group,
                log: slot.ctx.log().clone(),
            });
        }
    }

    /// A member has this device's leave of `group`: the group, left here already, stops
    /// running.
    pub(super) fn leave_taken(&mut self, group: GroupId) {
        if self.groups.get(&group).is_some_and(|s| s.leaving.is_some()) {
            tracing::debug!("a member has this device's leave");
            self.drop_group(group);
        }
    }

    /// The group's banner changed here: saved like the log, and sent on to the members
    /// that take banners, but not back to `from`.
    pub(super) fn banner_changed(&mut self, group: GroupId, from: Option<PeerId>) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        if let Some(banner) = slot.ctx.encoded_banner() {
            let digest = banner_key(&slot.ctx.banner_digest());
            let peers: Vec<PeerId> = slot
                .banner_peers
                .iter()
                .filter(|p| {
                    slot.members.contains(p)
                        && Some(**p) != from
                        && slot.banner_refused.get(p) != Some(&digest)
                })
                .copied()
                .collect();
            for peer in peers {
                self.request(
                    peer,
                    MemberRequest::GroupBanner {
                        group_id: group.as_bytes().to_vec(),
                        banner: banner.clone(),
                    },
                    Pending::Banner(group, digest),
                );
            }
        }
        self.emit(NodeEvent::GroupChanged(group));
    }

    /// The group's picture record changed here: saved like the log, and sent on to the
    /// members that take such records, but not back to `from`.
    pub(super) fn picture_changed(&mut self, group: GroupId, from: Option<PeerId>) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        if let Some(picture) = slot.ctx.encoded_picture() {
            let digest = banner_key(&slot.ctx.picture_digest());
            let peers: Vec<PeerId> = slot
                .picture_peers
                .iter()
                .filter(|p| {
                    slot.members.contains(p)
                        && Some(**p) != from
                        && slot.picture_refused.get(p) != Some(&digest)
                })
                .copied()
                .collect();
            for peer in peers {
                self.request(
                    peer,
                    MemberRequest::GroupPicture {
                        group_id: group.as_bytes().to_vec(),
                        picture: picture.clone(),
                    },
                    Pending::Picture(group, digest),
                );
            }
        }
        self.emit(NodeEvent::GroupChanged(group));
    }

    /// Entries `peer` pushed for `group`: taken as any batch a member sends.
    pub(super) fn take_pushed(&mut self, peer: PeerId, group: GroupId, entries: &[Vec<u8>]) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        let taken = slot.ctx.insert_encoded(entries);
        self.refused_entries(&taken.refused);
        self.after_group_change(group, taken.new, Some(peer));
    }

    /// A removal of this device `peer` showed, judged: `log` is the group's log with it,
    /// taken whole when nothing came meanwhile (the member that shows it does so once);
    /// otherwise its `entries` are taken as any batch, and the rest come again later.
    pub(super) fn removal_judged(
        &mut self,
        peer: PeerId,
        group: GroupId,
        entries: Vec<Vec<u8>>,
        log: GroupLog,
    ) {
        let Some(slot) = self.groups.get_mut(&group) else {
            return;
        };
        match slot.ctx.take_judged(log) {
            Some(new) => self.after_group_change(group, new, Some(peer)),
            None => self.take_pushed(peer, group, &entries),
        }
    }
}
