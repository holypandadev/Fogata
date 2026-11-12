//! The core's groups: what the screens show of them (the list, invitations, activity and
//! members) and what changes them: making and joining groups, members and managers, names,
//! pictures and limits, voice rooms and who may be in them, invites, and leaving.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId, names};
use fogata_net::{GroupSetup, Path, RoomView};
use fogata_state::{Body, GroupLog, VoiceRoomId};

use crate::{
    ActivityView, ActivityWhat, Core, CoreEvent, GroupView, InvitationView, JoinAsk, KeptOutView,
    MemberView, Person, RoomSummary, UndoneKind, VoiceRoomView, direct, invite, now_ms, person,
    shares, store, store::DeclineLeave,
};

/// Whether a device seen at capability level `seen` (the network's `caps`;
/// [`fogata_net::caps::FIRST_LEVEL`] when never seen) is known to be too old for a group
/// judged at `level`. A device never seen saying its level is not known to be: it may be
/// new.
fn too_old_to_join(seen: u32, level: fogata_state::Level) -> bool {
    level > fogata_state::Level::default()
        && seen > fogata_net::caps::FIRST_LEVEL
        && !fogata_net::caps::judges_at(seen, level)
}

/// A line of a group's activity as told by its log, or a record this device saw arrive.
enum Slot<L, S> {
    Line(L),
    Seen(S),
}

/// The page `skip..skip + take` of `lines` (the newest first) with `seen` (in any order)
/// put among them by time, and whether more follow: a record goes before the first line
/// that is not newer than it, and records of the same time keep their order. Nothing is
/// made of the slots outside the page.
fn page_by_time<L, S>(
    lines: Vec<L>,
    line_ms: impl Fn(&L) -> u64,
    mut seen: Vec<S>,
    seen_ms: impl Fn(&S) -> u64,
    skip: usize,
    take: usize,
) -> (Vec<Slot<L, S>>, bool) {
    seen.sort_by_key(|s| std::cmp::Reverse(seen_ms(s)));
    let mut all = Vec::with_capacity(lines.len() + seen.len());
    let mut seen = seen.into_iter().peekable();
    for line in lines {
        let at = line_ms(&line);
        while let Some(record) = seen.next_if(|r| seen_ms(r) >= at) {
            all.push(Slot::Seen(record));
        }
        all.push(Slot::Line(line));
    }
    all.extend(seen.map(Slot::Seen));
    let more = all.len() > skip.saturating_add(take);
    (all.into_iter().skip(skip).take(take).collect(), more)
}

impl Core {
    /// Every group, live rooms first, then by name.
    pub async fn groups(&self) -> Result<Vec<GroupView>> {
        let me = self.me();
        let groups = self.inner.node.groups().await?;
        let book = self.inner.node.friends().await?;
        let mut out = Vec::new();
        for group in groups {
            let id = group.group_id;
            let state = &group.state;
            if !state.is_member(&me)
                || self.inner.is_call(id)
                || self.inner.direct.is_chat_group(&id)
            {
                continue;
            }
            let live = self.inner.node.rooms(id).await?;
            let in_room: HashSet<DevicePublicKey> = live
                .iter()
                .flat_map(|(_, r)| r.participants.iter().map(|(device, _, _)| *device))
                .collect();
            let summary = |room: &RoomView| RoomSummary {
                started_ms: room.started_ms,
                participants: room
                    .participants
                    .iter()
                    .map(|(device, nick, _)| {
                        self.person_with(*device, Some(nick), book.as_ref(), state)
                    })
                    .collect(),
                joined: room.joined,
            };
            let now = now_ms();
            let voice_rooms: Vec<VoiceRoomView> = state
                .rooms
                .iter()
                .map(|voice| VoiceRoomView {
                    id: voice.id,
                    name: voice.name.clone(),
                    room: live
                        .iter()
                        .find(|(v, _)| *v == voice.id)
                        .map(|(_, room)| summary(room)),
                    only: state
                        .rules
                        .only
                        .get(&voice.id)
                        .map(|only| only.iter().copied().collect()),
                    barrier: state.barrier(&voice.id, &me, now),
                })
                .collect();
            let order = |voice: &VoiceRoomId| state.rooms.iter().position(|r| r.id == *voice);
            let mut kept = state.kept_out_now(now);
            kept.retain(|(voice, _, _)| order(voice).is_some());
            kept.sort_by_key(|(voice, member, _)| (order(voice), *member));
            let kept_out = kept
                .into_iter()
                .map(|(voice, member, k)| KeptOutView {
                    voice,
                    member: self.person_with(member, None, book.as_ref(), state),
                    by: self.person_with(k.by, None, book.as_ref(), state),
                    until_ms: k.until_ms,
                })
                .collect();
            let room = live
                .iter()
                .find(|(_, r)| r.joined)
                .or_else(|| live.iter().find(|(_, r)| !r.participants.is_empty()))
                .map(|(_, room)| summary(room));
            // Before the lock below: a person's name takes it too.
            let level_behind: Vec<Person> =
                if state.is_manager(&me) && state.level < fogata_state::Level::CURRENT {
                    self.behind_level(state, fogata_state::Level::CURRENT)
                        .into_iter()
                        .map(|device| self.person_with(device, None, book.as_ref(), state))
                        .collect()
                } else {
                    Vec::new()
                };
            let raise_anyway = self.level_waited(id, !level_behind.is_empty(), now);
            let st = self.inner.lock();
            let online = st
                .online
                .get(&id)
                .map(|m| {
                    m.keys()
                        .filter(|d| state.is_member(d) && st.shows_online(d, &in_room))
                        .count()
                })
                .unwrap_or(0);
            out.push(GroupView {
                id,
                name: state.name.clone(),
                local_name: self
                    .inner
                    .store
                    .group_name(&id)
                    .unwrap_or_else(|| state.name.clone()),
                icon: state.icon,
                members: state.members.len(),
                online,
                manager: state.is_manager(&me),
                rank: state.rank(&me).unwrap_or(usize::MAX),
                managers: state.managers.first(),
                // Only a picture this version draws; anything else shows the icon.
                picture: group
                    .picture
                    .clone()
                    .filter(|p| fogata_state::is_record_picture(p)),
                banner: group.banner,
                max_members: state.max_members,
                max_in_room: state.max_in_room,
                voice_rooms,
                room,
                last_room_ms: st.meta.last_room.get(&hex::encode(id.as_bytes())).copied(),
                open_invites: state.open_invite_tokens(now).len(),
                outranks: state.below(&me).to_vec(),
                kept_out,
                level_behind,
                raise_anyway,
                level_ahead: group.level_ahead,
            });
        }
        out.sort_by(|a, b| {
            b.room
                .is_some()
                .cmp(&a.room.is_some())
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }

    /// Groups friends added this device to, waiting for the person to join or decline.
    pub async fn group_invitations(&self) -> Result<Vec<InvitationView>> {
        let book = self.inner.node.friends().await?;
        let mut out = Vec::new();
        for stored in self.inner.store.groups()? {
            if !stored.pending {
                continue;
            }
            let Ok(log) = self.inner.store.load_log(&stored) else {
                continue;
            };
            let by = stored
                .invited_by
                .as_deref()
                .and_then(|h| hex::decode(h).ok())
                .and_then(|b| DevicePublicKey::from_bytes(&b).ok());
            let state = log.state();
            out.push(InvitationView {
                id: log.group_id(),
                name: state.name.clone(),
                by: match by {
                    Some(device) => self.person_with(device, None, book.as_ref(), &state),
                    None => person(self.me(), "?"),
                },
                members: state.members.len(),
            });
        }
        Ok(out)
    }

    /// Joins a group a friend added this device to: it runs from now on.
    pub async fn accept_group_invitation(&self, group: GroupId) -> Result<()> {
        let id = hex::encode(group.as_bytes());
        let stored = self
            .inner
            .store
            .groups()?
            .into_iter()
            .find(|g| g.group_id == id && g.pending)
            .context("no such invitation")?;
        let log = self.inner.store.load_log(&stored)?;
        self.inner.store.set_pending(&id, None)?;
        self.inner.node.add_group(GroupSetup::Member(log)).await?;
        let _ = self.inner.events.send(CoreEvent::GroupsChanged);
        Ok(())
    }

    /// Declines a group a friend added this device to: its log is forgotten, and it is not
    /// offered here again unless a member adds this device again. Members online now get
    /// this device's signed `Leave`, so they stop counting it as a member; others get it
    /// when they next offer the group.
    pub async fn decline_group_invitation(&self, group: GroupId) -> Result<()> {
        let id = hex::encode(group.as_bytes());
        let (leave, members) =
            self.inner
                .store
                .decline_group(&id, &self.inner.identity, now_ms())?;
        if let Some(DeclineLeave { leave, admission }) = leave {
            let _ = self
                .inner
                .node
                .give_entries(members, vec![leave], Some(admission))
                .await;
        }
        let _ = self.inner.events.send(CoreEvent::GroupsChanged);
        Ok(())
    }

    /// The group stored under `local_name` here.
    pub fn group_id(&self, local_name: &str) -> Result<GroupId> {
        let stored = self.inner.store.group(local_name)?;
        hex::decode(&stored.group_id)
            .ok()
            .and_then(|b| GroupId::from_bytes(&b).ok())
            .context("stored group id is damaged")
    }

    pub(crate) async fn log(&self, group: GroupId) -> Result<GroupLog> {
        self.inner
            .node
            .group(group)
            .await?
            .context("this device does not run that group")
    }

    /// What the members of `group` did, the newest first: `take` lines after the first
    /// `skip`, and whether there are more. Built from the log, the same on every member,
    /// with the changes of the banner and picture this device saw arrive.
    pub async fn group_activity(
        &self,
        group: GroupId,
        skip: usize,
        take: usize,
    ) -> Result<(Vec<ActivityView>, bool)> {
        use fogata_state::{Did, UndoneBy};
        let log = self.log(group).await?;
        let book = self.inner.node.friends().await?;
        let shared = log.shared_state();
        let person =
            |device: DevicePublicKey| self.person_with(device, None, book.as_ref(), &shared);
        // The records this device saw, put among the rest by time. Only the page's lines are
        // made into views: each names its people, whose profiles (pictures and all) are
        // copied, and a long log has thousands of lines.
        let seen: Vec<(DevicePublicKey, store::RecordSeen)> = self
            .inner
            .store
            .record_history(&group)
            .into_iter()
            .filter_map(|r| {
                let author = hex::decode(&r.author)
                    .ok()
                    .and_then(|b| DevicePublicKey::from_bytes(&b).ok())?;
                Some((author, r))
            })
            .collect();
        let lines = log.activity().into_iter().rev().collect();
        let (page, more) = page_by_time(lines, |a| a.time_ms, seen, |(_, r)| r.time_ms, skip, take);
        let view = |slot: Slot<fogata_state::Activity, (DevicePublicKey, store::RecordSeen)>| {
            let a = match slot {
                Slot::Seen((author, r)) => {
                    let taken_away = r.taken_away;
                    return ActivityView {
                        by: person(author),
                        at_ms: r.time_ms,
                        what: match r.kind {
                            store::RecordKind::Banner => ActivityWhat::Banner { taken_away },
                            store::RecordKind::Picture => ActivityWhat::Picture { taken_away },
                        },
                        undone: None,
                        seen_here: true,
                    };
                }
                Slot::Line(a) => a,
            };
            let what = match a.did {
                Did::Created { name } => ActivityWhat::Created { name },
                Did::Renamed { name } => ActivityWhat::Renamed { name },
                Did::SetIcon => ActivityWhat::Icon,
                Did::SetPicture { taken_away } => ActivityWhat::Picture { taken_away },
                Did::Added { member } => ActivityWhat::Added {
                    member: person(member),
                },
                Did::Joined { invited_by } => ActivityWhat::Joined {
                    invited_by: person(invited_by),
                },
                Did::LetIn { member } => ActivityWhat::LetIn {
                    member: person(member),
                },
                Did::Removed { member } => ActivityWhat::Removed {
                    member: person(member),
                },
                Did::Left => ActivityWhat::Left,
                Did::MovedUp { member, place } => ActivityWhat::MovedUp {
                    member: person(member),
                    place,
                },
                Did::MovedDown { member } => ActivityWhat::MovedDown {
                    member: person(member),
                },
                Did::Managers { managers } => ActivityWhat::Managers {
                    first: managers.first(),
                },
                Did::Limits { members, room } => ActivityWhat::Limits { members, room },
                Did::RoomOpened { name } => ActivityWhat::RoomOpened { name },
                Did::RoomRenamed { from, name } => ActivityWhat::RoomRenamed { from, name },
                Did::RoomClosed { name } => ActivityWhat::RoomClosed { name },
                Did::Invited => ActivityWhat::Invited,
                Did::Revoked => ActivityWhat::Revoked,
                Did::RoomAccess { room, members } => ActivityWhat::RoomAccess {
                    room,
                    members: members.map(|m| m.into_iter().map(person).collect()),
                },
                Did::KeptOut {
                    room,
                    member,
                    until_ms,
                } => ActivityWhat::KeptOut {
                    room,
                    member: person(member),
                    until_ms,
                },
                Did::LetBack { room, member } => ActivityWhat::LetBack {
                    room,
                    member: person(member),
                },
            };
            let undone = a.undone.map(|u| match u {
                UndoneBy::Removed { by } => (UndoneKind::Removed, Some(person(by))),
                UndoneBy::Left => (UndoneKind::Left, None),
                UndoneBy::MovedDown { by } => (UndoneKind::MovedDown, Some(person(by))),
            });
            ActivityView {
                by: person(a.author),
                at_ms: a.time_ms,
                what,
                undone,
                seen_here: false,
            }
        };
        Ok((page.into_iter().map(view).collect(), more))
    }

    pub async fn members(&self, group: GroupId) -> Result<Vec<MemberView>> {
        let me = self.me();
        let log = self.log(group).await?;
        let book = self.inner.node.friends().await?;
        let state = log.state();
        let lacking = state.lacking_current_secret();
        let in_room: HashSet<DevicePublicKey> = self
            .inner
            .node
            .rooms(group)
            .await?
            .iter()
            .flat_map(|(_, r)| r.participants.iter().map(|(device, _, _)| *device))
            .collect();
        let (online, away): (HashMap<DevicePublicKey, Path>, HashSet<DevicePublicKey>) = {
            let st = self.inner.lock();
            let online: HashMap<DevicePublicKey, Path> = st
                .online
                .get(&group)
                .map(|m| {
                    m.iter()
                        .filter(|(d, _)| st.shows_online(d, &in_room))
                        .map(|(d, p)| (*d, p.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let away = online
                .keys()
                .filter(|d| st.shows_away(d, in_room.contains(*d)))
                .copied()
                .collect();
            (online, away)
        };
        let mut out: Vec<MemberView> = state
            .members
            .keys()
            .map(|device| MemberView {
                person: self.person_with(*device, None, book.as_ref(), &state),
                rank: state.rank(device).unwrap_or(usize::MAX),
                manager: state.is_manager(device),
                me: *device == me,
                online: online.get(device).cloned(),
                away: away.contains(device),
                waiting_for_key: lacking.contains(device),
            })
            .collect();
        out.sort_by_key(|m| m.rank);
        Ok(out)
    }

    /// Creates a group with an icon and these friends in it; they get it the next time they
    /// are online while this device is.
    pub async fn create_group(
        &self,
        name: &str,
        icon: u8,
        friends: &[DevicePublicKey],
    ) -> Result<GroupId> {
        let identity = &self.inner.identity;
        let mut log = GroupLog::create(identity, name, now_ms())?;
        if icon != 0 {
            log.append(identity, Body::SetIcon { icon }, now_ms())?;
        }
        let book = self.inner.node.friends().await?;
        for device in friends {
            let nick = book
                .as_ref()
                .and_then(|b| b.friends.get(device))
                .map(|f| f.profile.name.clone())
                .context("only friends can be added when making a group")?;
            log.add(identity, *device, &nick, now_ms())?;
        }
        self.inner.store.save_group_log(&log, &log.state().name)?;
        let id = log.group_id();
        self.inner.node.add_group(GroupSetup::Member(log)).await?;
        let _ = self.inner.events.send(CoreEvent::GroupsChanged);
        Ok(id)
    }

    /// A single-use invite to a group, with this device's current addresses (managers). The
    /// invite is noted in the group's log, so every manager can be found by whoever uses it.
    pub async fn invite(&self, group: GroupId, days: u64) -> Result<String> {
        let addrs = self.inner.node.addresses().await?;
        let mut text = None;
        self.change_group(group, |log, id| {
            text = Some(invite::make(log, id, days, addrs, now_ms())?);
            Ok(())
        })
        .await?;
        text.context("no invite was made")
    }

    /// Starts joining a group with an invite; [`CoreEvent::Joined`] follows when a member
    /// accepts it. `local_name` is the name to keep it under here (default: the group's own,
    /// made unique) and `nick` the name the members see (default: the profile name). Returns
    /// the group's id and name.
    pub async fn join_group(
        &self,
        invite_text: &str,
        local_name: Option<&str>,
        nick: Option<&str>,
    ) -> Result<(GroupId, String)> {
        let invite = invite::decode(invite_text)?;
        let group = invite.group_id();
        if self.inner.lock().joining.contains_key(&group) {
            bail!("this device is already joining that group");
        }
        // Chats' ids are random, so a chat under this group's id was made to take its
        // place: it is let go (its messages stay; the next one starts a chat group anew).
        if direct::friend_of(&self.inner, &group).is_some() {
            tracing::warn!("a chat's group had the id of a group joined with an invite");
            direct::on_removed(&self.inner, group).await;
        }
        if let Some(log) = self.inner.node.group(group).await? {
            if log.state().is_member(&self.me()) {
                bail!("this device is already in that group");
            }
            // A group this device left, still running to deliver its leave.
            self.inner.node.drop_group(group).await?;
        }
        let local = match local_name.map(names::tidy).filter(|n| !n.is_empty()) {
            Some(name) if self.inner.store.groups()?.iter().any(|g| g.name == name) => {
                bail!("a group called \"{name}\" already exists here");
            }
            Some(name) => name,
            None => self.inner.store.free_group_name(&invite.name)?,
        };
        let nick = match nick {
            Some(nick) => names::tidy(nick),
            None => self
                .profile()
                .await?
                .map(|p| p.name)
                .unwrap_or_else(|| "anon".into()),
        };
        self.inner.lock().joining.insert(group, local.clone());
        // Kept until a manager answers, so the join goes on if this device starts again.
        let pending = store::PendingJoin {
            group_id: hex::encode(group.as_bytes()),
            invite: invite_text.trim().to_string(),
            name: local,
            nick: nick.clone(),
        };
        if let Err(e) = self.inner.store.note_join(group, Some(pending)) {
            tracing::warn!("cannot keep the join for later: {e:#}");
        }
        self.inner
            .node
            .add_group(GroupSetup::Joining {
                group_id: group,
                ticket: invite.ticket.clone(),
                rendezvous: invite.rendezvous(),
                name: nick,
            })
            .await?;
        self.inner
            .node
            .dial(invite.addrs.clone(), Some(group))
            .await?;
        Ok((group, invite.name))
    }

    /// Applies a change made here to a group's log and sends it to the members.
    async fn change_group(
        &self,
        group: GroupId,
        change: impl FnOnce(&mut GroupLog, &DeviceIdentity) -> Result<()>,
    ) -> Result<()> {
        let mut log = self.log(group).await?;
        change(&mut log, &self.inner.identity)?;
        // On disk before it is told done, after any write of the group that waits.
        let (store, saved) = (self.inner.store.clone(), log.clone());
        self.inner
            .writer
            .then(move || store.save_group_log(&saved, &saved.state().name))
            .await??;
        self.inner.node.merge_group(log).await?;
        Ok(())
    }

    /// Adds a friend to a group (managers). Not a friend whose Fogata is known to be too
    /// old for the level the group went up to: its copy would refuse the group from there
    /// on and never get in, while the group would count it in (see the network's `caps`).
    /// A friend whose level is not known yet is added; the network does not send it the
    /// group until it lists that level, and tells why.
    pub async fn add_to_group(&self, group: GroupId, friend: DevicePublicKey) -> Result<()> {
        let nick = self
            .inner
            .node
            .friends()
            .await?
            .and_then(|b| b.friends.get(&friend).map(|f| f.profile.name.clone()))
            .context("not a friend")?;
        let seen = self.inner.node.level_seen(&friend);
        self.change_group(group, |log, id| {
            let level = log.level();
            anyhow::ensure!(
                !too_old_to_join(seen, level),
                "{nick} must update Fogata before they can join this group"
            );
            log.add(id, friend, &nick, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Notes whether group `group`, managed here, waits to go up to this version's level
    /// (`waiting`: members hold it back), and since when, kept across restarts; returns
    /// whether it has waited [`crate::LEVEL_WAIT_MS`] by `now`, so that a manager may raise
    /// it anyway. A group that stops waiting starts again from nothing.
    fn level_waited(&self, group: GroupId, waiting: bool, now: u64) -> bool {
        let key = hex::encode(group.as_bytes());
        let (since, changed) = {
            let mut st = self.inner.lock();
            let wait = &mut st.meta.level_wait;
            if waiting {
                let had = wait.contains_key(&key);
                (Some(*wait.entry(key).or_insert(now)), !had)
            } else {
                (None, wait.remove(&key).is_some())
            }
        };
        if changed {
            self.inner.save_meta();
        }
        since.is_some_and(|since| now.saturating_sub(since) >= crate::LEVEL_WAIT_MS)
    }

    /// The members other than this device whose copy was never seen judging at `level` (see
    /// the network's `caps`): those that keep the group from going up to it.
    fn behind_level(
        &self,
        state: &fogata_state::GroupState,
        level: fogata_state::Level,
    ) -> Vec<DevicePublicKey> {
        let me = self.me();
        state
            .members
            .keys()
            .filter(|device| {
                **device != me
                    && !fogata_net::caps::judges_at(self.inner.node.level_seen(device), level)
            })
            .copied()
            .collect()
    }

    /// Removes a member below this device on the ladder and makes a new group key without
    /// it; a manager's open invites it could answer end with it (see
    /// [`GroupLog::cancel_invites_lost_since`]).
    pub async fn remove_from_group(&self, group: GroupId, member: DevicePublicKey) -> Result<()> {
        self.change_group(group, |log, id| {
            let before = log.shared_state();
            log.remove(id, member, now_ms())?;
            log.cancel_invites_lost_since(id, &before, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Moves a member below this device on the ladder up to just below it.
    pub async fn promote(&self, group: GroupId, member: DevicePublicKey) -> Result<()> {
        self.change_group(group, |log, id| {
            log.append(id, Body::Promote { member }, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Moves a member below this device on the ladder down to the end; when that ends its
    /// managing, the open invites it could answer end too.
    pub async fn demote(&self, group: GroupId, member: DevicePublicKey) -> Result<()> {
        self.change_group(group, |log, id| {
            let before = log.shared_state();
            log.append(id, Body::Demote { member }, now_ms())?;
            log.cancel_invites_lost_since(id, &before, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Who manages the group: the first `first` on the ladder, or everyone (`None`)
    /// (managers; never leaving out this device or those above it). The open invites those
    /// left out could answer end with it.
    pub async fn set_managers(&self, group: GroupId, first: Option<u8>) -> Result<()> {
        self.change_group(group, |log, id| {
            let before = log.shared_state();
            log.append(id, Body::SetManagers { first }, now_ms())?;
            log.cancel_invites_lost_since(id, &before, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Raises a group to `level` of judgment (managers; see [`fogata_state::Level`]): the one
    /// place a group's level goes up. Only once every other member's copy was seen judging
    /// at it, as the capability level it lists says (see the network's `caps`): a copy below
    /// it would stop at the `Level` entry, and wait on everything signed after it.
    pub async fn raise_group_level(
        &self,
        group: GroupId,
        level: fogata_state::Level,
    ) -> Result<()> {
        self.change_group(group, |log, id| {
            let state = log.state();
            let behind: Vec<String> = self
                .behind_level(&state, level)
                .iter()
                .filter_map(|device| state.members.get(device).map(|m| m.name.clone()))
                .collect();
            drop(state);
            anyhow::ensure!(
                behind.is_empty(),
                "not every member's Fogata judges at level {} yet: {}",
                level.number(),
                behind.join(", ")
            );
            log.raise_level(id, level, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Raises a group to the level of judgment this version gives although some members
    /// were never seen at it (managers): only once it has waited [`crate::LEVEL_WAIT_MS`]
    /// for them, as the group's settings show. Their copies stop at the `Level` entry, keep
    /// the group as it was there, and tell their person that it needs a newer Fogata (see
    /// [`GroupLog::level_ahead`]); they take part again once updated. `behind` is who the
    /// person confirmed leaving behind: the raise does not happen when that changed
    /// meanwhile (one more member behind, say), so nobody is left behind unnamed.
    pub async fn raise_group_level_anyway(
        &self,
        group: GroupId,
        behind: Vec<DevicePublicKey>,
    ) -> Result<()> {
        let level = fogata_state::Level::CURRENT;
        let since = self
            .inner
            .lock()
            .meta
            .level_wait
            .get(&hex::encode(group.as_bytes()))
            .copied();
        anyhow::ensure!(
            since.is_some_and(|s| now_ms().saturating_sub(s) >= crate::LEVEL_WAIT_MS),
            "this group has not waited long enough for its members to update"
        );
        self.change_group(group, |log, id| {
            let state = log.state();
            let mut now_behind = self.behind_level(&state, level);
            let mut confirmed = behind;
            now_behind.sort_unstable();
            confirmed.sort_unstable();
            anyhow::ensure!(
                now_behind == confirmed,
                "who has to update changed; look at the group's members again"
            );
            drop(state);
            log.raise_level(id, level, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Changes a group's icon for everyone (managers).
    pub async fn set_group_icon(&self, group: GroupId, icon: u8) -> Result<()> {
        self.change_group(group, |log, id| {
            log.append(id, Body::SetIcon { icon }, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Renames a group for everyone (managers).
    pub async fn rename_group(&self, group: GroupId, name: &str) -> Result<()> {
        let name = name.to_string();
        self.change_group(group, |log, id| {
            log.append(id, Body::Rename { name }, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Sets a group's picture for everyone, a PNG, JPEG, GIF or WebP of at most 550 KiB, or
    /// none to show the icon (managers). It goes apart from the log, only the newest kept, to the members whose
    /// version keeps such records; earlier versions show the icon instead of a picture set
    /// this way (see [`GroupLog::set_picture`]).
    pub async fn set_group_picture(&self, group: GroupId, picture: Option<Vec<u8>>) -> Result<()> {
        if picture
            .as_ref()
            .is_some_and(|p| p.len() > fogata_state::MAX_RECORD_PICTURE_BYTES)
        {
            bail!("that picture is too big for a group");
        }
        if picture
            .as_ref()
            .is_some_and(|p| !fogata_state::is_record_picture(p))
        {
            bail!("a group's picture is a PNG, JPEG, GIF or WebP");
        }
        let picture = picture.unwrap_or_default();
        Ok(self.inner.node.set_group_picture(group, picture).await?)
    }

    /// Chooses a group's banner for everyone (managers): a PNG, JPEG, GIF or WebP of at most
    /// 550 KiB, or none to take it away. It goes apart from the log, to the members whose
    /// version keeps banners.
    pub async fn set_group_banner(&self, group: GroupId, picture: Option<Vec<u8>>) -> Result<()> {
        let picture = picture.unwrap_or_default();
        if picture.len() > fogata_state::MAX_BANNER_BYTES {
            bail!("that picture is too big for a banner");
        }
        if !picture.is_empty() && !fogata_state::is_banner_picture(&picture) {
            bail!("a banner is a PNG, JPEG, GIF or WebP");
        }
        Ok(self.inner.node.set_group_banner(group, picture).await?)
    }

    /// A group's banner, when it has one.
    pub async fn group_banner(&self, group: GroupId) -> Result<Option<Vec<u8>>> {
        let log = self.log(group).await?;
        Ok(log.banner().map(|b| b.picture.clone()))
    }

    /// The most members a group takes and the most people in each voice room (managers).
    pub async fn set_group_limits(&self, group: GroupId, members: u8, room: u8) -> Result<()> {
        self.change_group(group, |log, id| {
            log.append(id, Body::SetLimits { members, room }, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Opens a voice room called `name`, or renames room `id` (managers). Returns its id.
    pub async fn set_voice_room(
        &self,
        group: GroupId,
        id: Option<VoiceRoomId>,
        name: &str,
    ) -> Result<VoiceRoomId> {
        let name = name.trim().to_string();
        let id = match id {
            Some(id) => id,
            None => {
                let mut fresh = [0u8; 8];
                fogata_crypto::random_bytes(&mut fresh)?;
                fresh
            }
        };
        self.change_group(group, |log, me| {
            log.append(me, Body::SetRoom { id, name }, now_ms())?;
            Ok(())
        })
        .await?;
        Ok(id)
    }

    /// Who may enter a voice room: everyone in the group (`None`), or only `members` and
    /// the managers (managers).
    pub async fn set_room_access(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        members: Option<Vec<DevicePublicKey>>,
    ) -> Result<()> {
        self.change_group(group, |log, me| {
            log.set_room_access(me, voice, members, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Takes a member below this device on the ladder out of a voice room now; it may come
    /// back at once.
    pub async fn remove_from_room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        member: DevicePublicKey,
    ) -> Result<()> {
        let state = self.log(group).await?.state();
        if !state.outranks(&self.me(), &member) {
            bail!("only someone above a member on the ladder takes it out of a room");
        }
        Ok(self
            .inner
            .node
            .remove_from_room(group, voice, member)
            .await?)
    }

    /// Keeps a member below this device on the ladder out of a voice room for `minutes`, or
    /// until someone above it lets it back (`None`); it leaves the room now if it is in.
    pub async fn keep_out_of_room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        member: DevicePublicKey,
        minutes: Option<u32>,
    ) -> Result<()> {
        let now = now_ms();
        let until = minutes.map(|m| now + u64::from(m.max(1)) * 60_000);
        self.change_group(group, |log, me| {
            log.keep_out(me, voice, member, until, now)?;
            Ok(())
        })
        .await
    }

    /// Lets a member below this device back into a voice room before its time out runs out.
    pub async fn let_back_into_room(
        &self,
        group: GroupId,
        voice: VoiceRoomId,
        member: DevicePublicKey,
    ) -> Result<()> {
        self.change_group(group, |log, me| {
            log.let_back(me, voice, member, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Closes a voice room; the first one stays (managers).
    pub async fn remove_voice_room(&self, group: GroupId, id: VoiceRoomId) -> Result<()> {
        if self
            .inner
            .node
            .room(group, id)
            .await?
            .is_some_and(|r| !r.participants.is_empty())
        {
            bail!("people are in that room");
        }
        self.change_group(group, |log, me| {
            log.append(me, Body::RemoveRoom { id }, now_ms())?;
            Ok(())
        })
        .await
    }

    /// Cancels every invite of a group not used yet, so none of them lets anyone in
    /// (managers).
    pub async fn cancel_invites(&self, group: GroupId) -> Result<()> {
        self.change_group(group, |log, me| {
            let open: Vec<[u8; 16]> = log.state().open_invites.keys().copied().collect();
            for ticket_id in open {
                log.append(me, Body::Revoke { ticket_id }, now_ms())?;
            }
            Ok(())
        })
        .await
    }

    /// Devices waiting for this manager to let them in with an invite, oldest first.
    pub async fn join_asks(&self) -> Result<Vec<JoinAsk>> {
        let asks: Vec<_> = self
            .inner
            .lock()
            .join_asks
            .iter()
            .map(|((group, device), (name, since))| (*group, *device, name.clone(), *since))
            .collect();
        let mut out = Vec::new();
        for (group, device, name, since_ms) in asks {
            let Ok(log) = self.log(group).await else {
                continue;
            };
            out.push(JoinAsk {
                group,
                group_name: log.state().name,
                device,
                name,
                since_ms,
            });
        }
        out.sort_by_key(|a| a.since_ms);
        Ok(out)
    }

    /// Lets a device waiting to join in, or says no, which also cancels the invite it
    /// showed (managers).
    pub async fn answer_join(
        &self,
        group: GroupId,
        device: DevicePublicKey,
        accept: bool,
    ) -> Result<()> {
        Ok(self.inner.node.answer_join(group, device, accept).await?)
    }

    /// Leaves a group; the members learn it at once if any is connected, otherwise the next
    /// time this device starts.
    pub async fn leave_group(&self, group: GroupId) -> Result<()> {
        let leave = self.change_group(group, |log, id| {
            // A manager's invites end first: one that left keeps the log it had.
            log.cancel_invites_before_leaving(id, now_ms())?;
            log.append(id, Body::Leave, now_ms())?;
            Ok(())
        });
        shares::leaving(&self.inner, group, leave).await
    }
}

#[cfg(test)]
mod tests {
    use fogata_state::Level;

    use super::{Slot, page_by_time, too_old_to_join};

    /// The slots of a page as letters (lines) and digits (records), by their names.
    fn page(
        lines: &[(char, u64)],
        seen: &[(char, u64)],
        skip: usize,
        take: usize,
    ) -> (String, bool) {
        let (slots, more) =
            page_by_time(lines.to_vec(), |l| l.1, seen.to_vec(), |s| s.1, skip, take);
        let names = slots
            .into_iter()
            .map(|s| match s {
                Slot::Line((name, _)) | Slot::Seen((name, _)) => name,
            })
            .collect();
        (names, more)
    }

    /// A record goes before the first line not newer than it, even among lines whose
    /// authors' clocks disagree; records of the same time keep their order; and pages read
    /// one after another give the whole list, saying whether more follow.
    #[test]
    fn records_go_among_the_lines_by_time_and_pages_add_up() {
        let lines = [('a', 50), ('b', 40), ('c', 45), ('d', 10), ('e', 10)];
        let seen = [('1', 10), ('2', 60), ('3', 42), ('4', 42), ('5', 5)];
        let whole = "2a34bc1de5";
        assert_eq!(
            page(&lines, &seen, 0, usize::MAX),
            (whole.to_string(), false)
        );
        for take in 1..=whole.len() + 1 {
            let mut read = String::new();
            let mut skip = 0;
            loop {
                let (part, more) = page(&lines, &seen, skip, take);
                assert!(part.len() <= take);
                read.push_str(&part);
                skip += take;
                assert_eq!(more, skip < whole.len(), "take {take}, skip {skip}");
                if !more {
                    break;
                }
            }
            assert_eq!(read, whole, "take {take}");
        }
        assert_eq!(page(&lines, &seen, 20, 5), (String::new(), false));
        assert_eq!(page(&[], &seen, 0, 2), ("23".to_string(), true));
        assert_eq!(page(&lines, &[], 3, 9), ("de".to_string(), false));
    }

    /// Only a friend known to be too old is kept out: one never seen saying its level may
    /// be new, and a group never raised is judged alike by every copy.
    #[test]
    fn only_a_friend_known_to_be_too_old_is_kept_out() {
        assert!(too_old_to_join(2, Level::Three));
        assert!(!too_old_to_join(3, Level::Three));
        assert!(!too_old_to_join(
            fogata_net::caps::FIRST_LEVEL,
            Level::Three
        ));
        assert!(!too_old_to_join(2, Level::Two));
    }
}
