//! The member heartbeat, every other punch slot: joins tried again, rosters asked for,
//! presence sent where it was lost, and managers told when this device's box of the
//! current key does not open.

use std::time::{Duration, Instant};

use fogata_crypto::GroupId;
use fogata_proto::MemberRequest;
use libp2p::PeerId;

use super::{Node, Pending, now_ms};
use crate::NodeEvent;

/// How long past its expiry, by this device's clock, an invite is still shown to managers:
/// theirs decide, and may be behind.
const JOIN_GRACE_MS: u64 = 24 * 3600 * 1000;

/// The capability level that brought [`MemberRequest::KeyProblem`]; older managers are not
/// sent it.
const KEY_PROBLEM_LEVEL: u32 = 2;
/// How often a manager is told again that this device's box of the current key does not
/// open.
const KEY_PROBLEM_AGAIN: Duration = Duration::from_secs(60);

/// An ask for a member's roster of `group`, with this device's digests of the log heads,
/// the banner and the picture: the member starts a sync, or sends what it has of the
/// others, when they differ from its own.
fn roster_ask(slot: &super::GroupSlot, group: GroupId) -> MemberRequest {
    MemberRequest::Roster {
        group_id: group.as_bytes().to_vec(),
        heads: Some(serde_bytes::ByteBuf::from(slot.ctx.heads_digest())),
        banner: Some(serde_bytes::ByteBuf::from(slot.ctx.banner_digest())),
        picture: Some(serde_bytes::ByteBuf::from(slot.ctx.picture_digest())),
    }
}

impl Node {
    /// Every heartbeat: relay slots at members, rosters, keys for members a rotation missed,
    /// a new key when someone who left still holds the current one, and invites shown again
    /// to peers that may be members.
    pub(super) fn member_heartbeat(&mut self) {
        let peers: Vec<PeerId> = self.members.keys().copied().collect();
        for peer in peers {
            self.reserve_slot(peer);
            // A QUIC link for media that was not due when it was wanted.
            self.want_datagrams(peer);
        }
        for peer in std::mem::take(&mut self.presence.lost) {
            if self.members.contains_key(&peer) {
                self.send_presence(peer);
            }
        }
        let groups: Vec<GroupId> = self.groups.keys().copied().collect();
        for group in groups {
            let slot = self.groups.get_mut(&group).expect("listed above");
            slot.forget_sent();
            // Two people have nobody to reach through each other: a direct chat's group
            // asks for no rosters.
            let members: Vec<PeerId> = if slot.direct {
                Vec::new()
            } else {
                slot.members.iter().copied().collect()
            };
            // Managers hand the current key to members a rotation missed, and make a new one
            // when they were not given it.
            let one_above = self.manager_above_here(group);
            let levels = crate::locked(&self.levels).clone();
            let slot = self.groups.get_mut(&group).expect("listed above");
            let mut new = slot.ctx.welcome_lacking(now_ms());
            new.extend(slot.ctx.mend_key(now_ms(), one_above));
            // Once every member's copy judges at this version's level, the group goes up.
            new.extend(
                slot.ctx
                    .raise_level_when_all_judge(now_ms(), one_above, |d| {
                        levels.get(d).copied().unwrap_or(crate::caps::FIRST_LEVEL)
                    }),
            );
            // A week-old key is replaced (not a call's: it lasts as long as the call).
            if !slot.call {
                new.extend(slot.ctx.rotate_if_old(now_ms()));
            }
            // The member expected to rotate may be offline: after a couple of heartbeats,
            // whoever sees the leak rotates.
            slot.leak_heartbeats = if slot.ctx.secret_leaked() {
                slot.leak_heartbeats + 1
            } else {
                0
            };
            new.extend(
                slot.ctx
                    .rotate_if_leaked(now_ms(), slot.leak_heartbeats >= 2),
            );
            new.extend(slot.ctx.confirm_revokes(now_ms()));
            let joining = slot.ctx.joining().is_some();
            let ask = roster_ask(slot, group);
            for peer in members {
                self.request(peer, ask.clone(), Pending::Roster(group));
            }
            self.after_group_change(group, new, None);
            self.tell_key_problem(group);
            self.forget_settled_joins(group);
            self.forget_ended_join(group);
            let ran_out = self
                .groups
                .get(&group)
                .and_then(|s| s.ctx.joining())
                .is_some_and(|t| now_ms() > t.expires_ms.saturating_add(JOIN_GRACE_MS));
            if ran_out {
                self.emit(NodeEvent::JoinExpired { group });
            } else if joining {
                let peers: Vec<PeerId> = self
                    .interesting
                    .iter()
                    .filter(|p| self.swarm.is_connected(p))
                    .copied()
                    .collect();
                for peer in peers {
                    self.send_join(peer, group);
                }
            }
        }
    }

    /// Whether a member connected now in `group` manages it and is above this device: the
    /// one to go first when several could do the same.
    pub(super) fn manager_above_here(&self, group: GroupId) -> bool {
        let Some(slot) = self.groups.get(&group) else {
            return false;
        };
        let mine = slot.ctx.rank_of(&self.identity.public_key());
        slot.members.iter().any(|p| {
            self.members.get(p).is_some_and(|m| {
                slot.ctx.is_manager_device(&m.device) && slot.ctx.rank_of(&m.device) < mine
            })
        })
    }

    /// A member whose box of the current key does not open tells the manager connected now
    /// that knows the request and is highest on the ladder, at most once a minute: each one
    /// told makes a new key.
    pub(super) fn tell_key_problem(&mut self, group: GroupId) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let Some(epoch) = slot.ctx.broken_epoch() else {
            return;
        };
        let told_lately = slot
            .key_told
            .values()
            .any(|(e, at)| *e == epoch && at.elapsed() < KEY_PROBLEM_AGAIN);
        if told_lately {
            return;
        }
        let manager: Option<PeerId> = slot
            .members
            .iter()
            .filter_map(|p| {
                let device = self.members.get(p)?.device;
                slot.ctx
                    .is_manager_device(&device)
                    .then(|| (slot.ctx.rank_of(&device), *p))
            })
            .filter(|(_, p)| self.peer_level(p).is_some_and(|l| l >= KEY_PROBLEM_LEVEL))
            .min()
            .map(|(_, p)| p);
        if let Some(peer) = manager {
            if let Some(slot) = self.groups.get_mut(&group) {
                slot.key_told.insert(peer, (epoch, Instant::now()));
            }
            self.request(
                peer,
                MemberRequest::KeyProblem {
                    group_id: group.as_bytes().to_vec(),
                    epoch,
                },
                Pending::KeyProblem,
            );
        }
    }

    /// `peer`, a member here, refused this device's ask for its roster: it had not counted
    /// this device in yet. Asked again at once when it has asked for this device's roster
    /// since (it counts this device in now), or else when it does. Until then this device
    /// would not learn of the members `peer` reaches until the next heartbeat, nor sync
    /// with it.
    pub(super) fn roster_refused(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self
            .groups
            .get_mut(&group)
            .filter(|s| s.members.contains(&peer))
        else {
            return;
        };
        if slot.roster_heard.remove(&peer) {
            self.ask_roster(group, peer);
        } else {
            slot.roster_refused.insert(peer);
        }
    }

    /// Asks member `peer` again for its roster, as at a heartbeat, and for its rooms: what
    /// was asked for while it had not counted this device in yet.
    pub(super) fn ask_roster(&mut self, group: GroupId, peer: PeerId) {
        let Some(slot) = self.groups.get(&group) else {
            return;
        };
        let ask = roster_ask(slot, group);
        self.request(peer, ask, Pending::Roster(group));
        self.ask_room(group, peer);
    }
}
