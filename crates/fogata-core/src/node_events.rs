//! What the network node tells the core: its events pumped into the core's state one at a
//! time (a panic in one loses that one only), the state read again when events were lost,
//! and the friends and profiles that changed saved a moment later.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak, atomic::Ordering},
    time::Instant,
};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_friends::{Animation, FriendBook, Profile};
use fogata_net::{GroupSetup, NodeEvent, NodeEvents, Path, RoomChange};

use crate::{
    CoreEvent, Inner, calls, direct, now_ms, saves,
    store::{DeclineLeave, Offered},
    writer,
};

/// Most animations of other devices kept: a few for each friend and member.
const MAX_ANIMATIONS: usize = 512;

/// How long [`Core::shutdown`](crate::Core::shutdown) waits for the node's last events to be handled.
pub(crate) const PUMP_DRAIN: std::time::Duration = std::time::Duration::from_secs(10);

/// Turns the node's events into saved files and screen events. The node's events end when it
/// stops: on purpose, or when an internal error stopped it, which the screens are told. A
/// panic while one event is handled is logged and loses that event only.
pub(crate) async fn pump<T: Pumped<E>, E>(target: Weak<T>, mut events: impl EventSource<E>) {
    use futures::FutureExt as _;
    while let Some(event) = events.next().await {
        let Some(target) = target.upgrade() else {
            return;
        };
        // A panic while one event is handled loses that event only: the pump going with it
        // would leave the app waiting for events that never come, and no `Stopped`.
        if std::panic::AssertUnwindSafe(target.handle(event))
            .catch_unwind()
            .await
            .is_err()
        {
            tracing::error!("an event of the network node could not be handled; it is skipped");
        }
    }
    if let Some(target) = target.upgrade() {
        target.ended();
    }
}

/// The node dropped events this core fell far behind on: the groups, the members connected,
/// the friends and the voice rooms are read anew from it, and the screens look again (they
/// read the rooms anew on `GroupsChanged`). Groups offered and friend requests are never
/// dropped (see fogata-net's events.rs).
pub(crate) async fn resync(inner: &Arc<Inner>) {
    tracing::warn!("the network node dropped events; reading the groups and friends anew");
    let Some(groups) = read_again(RESYNC_TRIES, || inner.node.groups()).await else {
        // A node that stopped ends the events, and the screens are told (`Pumped::ended`).
        tracing::error!("the network node did not say its groups; what is shown may be old");
        return;
    };
    let mut online: HashMap<GroupId, HashMap<DevicePublicKey, Path>> = HashMap::new();
    let mut connected = HashSet::new();
    let before = direct::take_connected(inner);
    for group in groups.iter().map(|g| g.group_id) {
        Box::pin(handle(inner, NodeEvent::GroupChanged(group))).await;
        for (device, path) in inner.node.members(group).await.unwrap_or_default() {
            connected.insert((group, device));
            if !direct::on_member_connected(inner, group, device).await {
                online.entry(group).or_default().insert(device, path);
            }
        }
    }
    direct::forget_gone(inner, &before, &connected);
    rooms_anew(inner, groups.iter().map(|g| g.group_id)).await;
    let friends = inner.node.friends_online().await.unwrap_or_default();
    {
        let mut state = inner.lock();
        state.online = online;
        state.friends_online = friends.into_iter().collect();
    }
    Box::pin(handle(inner, NodeEvent::FriendsChanged)).await;
    calls::resync(inner).await;
    inner.emit(CoreEvent::JoinAsksChanged);
    inner.emit(CoreEvent::GroupsChanged);
}

/// The voice rooms of `groups` as the node knows them now, for what this core keeps of the
/// room events it may have missed: when a room was last seen open (`last_room`, which an
/// opening dropped would have set). A call's rooms are read by `calls::resync`, and a
/// chat's group has none.
async fn rooms_anew(inner: &Arc<Inner>, groups: impl Iterator<Item = GroupId>) {
    let mut changed = false;
    for group in groups {
        if inner.is_call(group) || direct::friend_of(inner, &group).is_some() {
            continue;
        }
        let Ok(rooms) = inner.node.rooms(group).await else {
            continue;
        };
        let Some(started_ms) = rooms.iter().map(|(_, room)| room.started_ms).max() else {
            continue;
        };
        let mut state = inner.lock();
        let last = state
            .meta
            .last_room
            .entry(hex::encode(group.as_bytes()))
            .or_default();
        if *last < started_ms {
            *last = started_ms;
            changed = true;
        }
    }
    if changed {
        inner.save_meta();
    }
}

/// How long [`read_again`] waits after each failed try: an answer lost to a panic in the
/// node's turn comes at the next.
const RESYNC_TRIES: [std::time::Duration; 3] = [
    std::time::Duration::from_millis(500),
    std::time::Duration::from_secs(2),
    std::time::Duration::from_secs(5),
];

/// What `read` gives, tried again after each wait of `tries` while it fails.
async fn read_again<T, E, F: std::future::Future<Output = std::result::Result<T, E>>>(
    tries: [std::time::Duration; 3],
    mut read: impl FnMut() -> F,
) -> Option<T> {
    for wait in tries {
        if let Ok(read) = read().await {
            return Some(read);
        }
        tokio::time::sleep(wait).await;
    }
    read().await.ok()
}

/// What [`pump`] hands the node's events to: the core, or a test's.
pub(crate) trait Pumped<E>: Send + Sync + 'static {
    fn handle(self: Arc<Self>, event: E) -> impl std::future::Future<Output = ()> + Send;

    /// The events ended while this still runs.
    fn ended(&self);
}

impl Pumped<NodeEvent> for Inner {
    async fn handle(self: Arc<Self>, event: NodeEvent) {
        handle(&self, event).await
    }

    fn ended(&self) {
        if !self.stopping.load(Ordering::Relaxed) {
            tracing::error!("the network node stopped by itself; Fogata must be restarted");
            self.emit(CoreEvent::Stopped);
        }
    }
}

/// Where [`pump`] takes events from, in order: the node's, or a test's.
pub(crate) trait EventSource<E> {
    fn next(&mut self) -> impl std::future::Future<Output = Option<E>> + Send;
}

impl EventSource<NodeEvent> for NodeEvents {
    fn next(&mut self) -> impl std::future::Future<Output = Option<NodeEvent>> + Send {
        self.recv()
    }
}

/// Writes the friend book now, or with the rest of the burst of changes it belongs to (see
/// saves.rs).
fn save_friends_soon(inner: &Arc<Inner>, book: FriendBook) {
    let (when, book) = {
        let mut state = inner.lock();
        let when = state.friends_saves.changed(Instant::now());
        if when == saves::When::Now {
            (when, Some(book))
        } else {
            state.unsaved_book = Some(book);
            (when, None)
        }
    };
    match when {
        saves::When::Now => {
            if let Some(book) = book {
                save_friends(inner, book);
            }
        }
        saves::When::In(wait) => {
            let inner = Arc::downgrade(inner);
            tokio::spawn(async move {
                tokio::time::sleep(wait).await;
                if let Some(inner) = inner.upgrade() {
                    save_waiting_friends(&inner);
                }
            });
        }
        saves::When::Waiting => {}
    }
}

/// Writes the friend book if a write waits.
pub(crate) fn save_waiting_friends(inner: &Inner) {
    let book = {
        let mut state = inner.lock();
        state.friends_saves.take_waiting(Instant::now());
        state.unsaved_book.take()
    };
    if let Some(book) = book {
        save_friends(inner, book);
    }
}

fn save_friends(inner: &Inner, book: FriendBook) {
    let store = inner.store.clone();
    inner.writer.soon(writer::File::Friends, move || {
        if let Err(e) = store.save_friends(&book) {
            tracing::warn!("cannot save friends: {e:#}");
        }
    });
}

/// Forgets the animations and profiles of people no longer friends nor members of any group
/// here, in a moment: a burst of changes (a group left, its members going) is looked at once.
fn forget_gone_soon(inner: &Arc<Inner>) {
    if std::mem::replace(&mut inner.lock().forgetting, true) {
        return;
    }
    let inner = Arc::downgrade(inner);
    tokio::spawn(async move {
        tokio::time::sleep(FORGET_GONE_AFTER).await;
        if let Some(inner) = inner.upgrade() {
            inner.lock().forgetting = false;
            forget_gone(&inner).await;
        }
    });
}

/// How long [`forget_gone_soon`] waits.
const FORGET_GONE_AFTER: std::time::Duration = std::time::Duration::from_secs(2);

async fn forget_gone(inner: &Arc<Inner>) {
    let (Ok(Some(book)), Ok(logs)) = (inner.node.friends().await, inner.node.groups().await) else {
        return;
    };
    // Not those of a group this device left, which runs on only to deliver the leave.
    let me = inner.identity.public_key();
    let members: HashSet<DevicePublicKey> = logs
        .iter()
        .filter(|group| group.state.is_member(&me))
        .flat_map(|group| group.state.members.keys().copied())
        .collect();
    let (animations, profiles) = {
        let mut state = inner.lock();
        let before = (state.animations.len(), state.profiles.len());
        state
            .animations
            .retain(|(device, _), _| members.contains(device) || book.friends.contains_key(device));
        state.profiles.retain(|device, _| members.contains(device));
        (
            (state.animations.len() < before.0)
                .then(|| state.animations.values().cloned().collect::<Vec<_>>()),
            state.profiles.len() < before.1,
        )
    };
    if let Some(animations) = animations {
        save_animations(inner, animations, false);
    }
    if profiles {
        save_profiles(inner);
    }
}

/// Writes the members' profiles now, or with the rest of the burst (see saves.rs).
fn save_profiles_soon(inner: &Arc<Inner>) {
    let when = inner.lock().profiles_saves.changed(Instant::now());
    match when {
        saves::When::Now => save_profiles(inner),
        saves::When::In(wait) => {
            let inner = Arc::downgrade(inner);
            tokio::spawn(async move {
                tokio::time::sleep(wait).await;
                if let Some(inner) = inner.upgrade() {
                    save_waiting_profiles(&inner);
                }
            });
        }
        saves::When::Waiting => {}
    }
}

/// Writes the members' profiles if a write waits.
pub(crate) fn save_waiting_profiles(inner: &Inner) {
    if inner.lock().profiles_saves.take_waiting(Instant::now()) {
        save_profiles(inner);
    }
}

fn save_profiles(inner: &Inner) {
    let profiles: Vec<Profile> = inner.lock().profiles.values().cloned().collect();
    let store = inner.store.clone();
    inner.writer.soon(writer::File::Profiles, move || {
        if let Err(e) = store.save_member_profiles(&profiles) {
            tracing::warn!("cannot save the members' profiles: {e:#}");
        }
    });
}

/// Writes the others' animations, kept for the next start. One more of them (up to
/// [`MAX_ANIMATIONS`] of half a megabyte each) is written only while the disk has room for
/// it past what it keeps free, as a download is: when it has not, the file stays as it was
/// and the animations are shown from memory, and from the network again after a restart.
/// Fewer (the animations of those gone) are always written: that frees room in the end, and
/// leaves nothing of theirs behind.
fn save_animations(inner: &Arc<Inner>, animations: Vec<Animation>, more: bool) {
    let store = inner.store.clone();
    let weak = Arc::downgrade(inner);
    inner.writer.soon(writer::File::Animations, move || {
        let saved = if more {
            store.save_animations_holding(&animations, |bytes, dir| {
                // Written as the core goes (its last writes): unchecked then, as before.
                let Some(inner) = weak.upgrade() else {
                    return Ok(None);
                };
                crate::shares::hold_disk(
                    &inner,
                    bytes,
                    dir,
                    "not enough free space for the animations",
                )
                .map(Some)
            })
        } else {
            store.save_animations(&animations)
        };
        if let Err(e) = saved {
            tracing::warn!("cannot save the animations: {e:#}");
        }
    });
}

async fn handle(inner: &Arc<Inner>, event: NodeEvent) {
    let now = now_ms();
    match event {
        NodeEvent::Listening(_) => {}
        NodeEvent::ExternalAddress(address) => inner.emit(CoreEvent::Address(address)),
        NodeEvent::Reached { address, device } => handle_reached(inner, address, device),
        NodeEvent::PortChangesPerDestination => inner.emit(CoreEvent::HardNat),
        NodeEvent::Info(text) => inner.emit(CoreEvent::Info(text)),
        NodeEvent::OtherSession => inner.emit(CoreEvent::OtherSession),
        NodeEvent::Resync => resync(inner).await,
        NodeEvent::SignalsLost { group, from } => {
            inner.emit(CoreEvent::SignalsLost { group, from })
        }
        NodeEvent::LevelSeen { device, level } => handle_level_seen(inner, device, level),
        NodeEvent::MemberConnected {
            group,
            device,
            path,
            ..
        } => handle_member_connected(inner, group, device, path).await,
        NodeEvent::MemberDisconnected { group, device, .. } => {
            if direct::on_member_disconnected(inner, group, device) {
                return;
            }
            {
                let mut state = inner.lock();
                if let Some(online) = state.online.get_mut(&group) {
                    online.remove(&device);
                }
                state.forget_away_if_gone(&device);
            }
            inner.emit(CoreEvent::MemberDisconnected { group, device });
            inner.emit(CoreEvent::GroupsChanged);
        }
        NodeEvent::Room {
            group,
            voice,
            change,
        } => {
            let call = inner.is_call(group);
            if call {
                match &change {
                    RoomChange::Left { device, .. } => calls::on_left(inner, group, *device).await,
                    RoomChange::Joined { device, .. } => {
                        calls::on_joined(inner, group, *device).await
                    }
                    _ => {}
                }
            }
            if !call && matches!(change, RoomChange::Opened { .. } | RoomChange::Closed) {
                inner
                    .lock()
                    .meta
                    .last_room
                    .insert(hex::encode(group.as_bytes()), now);
                inner.save_meta();
            }
            let home = matches!(
                change,
                RoomChange::Opened { .. }
                    | RoomChange::Closed
                    | RoomChange::Joined { .. }
                    | RoomChange::Left { .. }
            );
            inner.emit(CoreEvent::Room {
                group,
                voice,
                change,
            });
            if home {
                inner.emit(CoreEvent::GroupsChanged);
            }
        }
        // A call's group is never saved, and a chat's is kept with its chat.
        NodeEvent::GroupChanged(group) => {
            if direct::on_group_changed(inner, group).await {
                return;
            }
            if inner.is_call(group) {
                calls::on_group_changed(inner, group).await;
            } else {
                inner.save_group(group).await;
            }
            // A member may have gone.
            forget_gone_soon(inner);
            inner.emit(CoreEvent::GroupsChanged);
        }
        NodeEvent::JoinAsked {
            group,
            device,
            name,
        } => {
            inner
                .lock()
                .join_asks
                .insert((group, device), (name.clone(), now));
            inner.emit(CoreEvent::JoinAsked {
                group,
                device,
                name,
            });
        }
        NodeEvent::JoinAnswered { group, device } => {
            inner.lock().join_asks.remove(&(group, device));
            inner.emit(CoreEvent::JoinAsksChanged);
        }
        NodeEvent::JoinWaiting { group } => {
            let name = inner
                .lock()
                .joining
                .get(&group)
                .cloned()
                .unwrap_or_default();
            inner.emit(CoreEvent::JoinWaiting { group, name });
        }
        NodeEvent::JoinDeclined { group } => {
            let Some(name) = inner.lock().joining.remove(&group) else {
                return;
            };
            inner.forget_join(group);
            let _ = inner.node.drop_group(group).await;
            inner.emit(CoreEvent::JoinDeclined { group, name });
        }
        NodeEvent::MustUpdateToJoin {
            group,
            device,
            name,
        } => inner.emit(CoreEvent::MustUpdateToJoin {
            group,
            device,
            name,
        }),
        NodeEvent::UpdateToJoin { group } => {
            let Some(name) = inner.lock().joining.get(&group).cloned() else {
                return;
            };
            inner.emit(CoreEvent::UpdateToJoin { group, name });
        }
        NodeEvent::JoinExpired { group } => {
            let Some(name) = inner.lock().joining.remove(&group) else {
                return;
            };
            inner.forget_join(group);
            let _ = inner.node.drop_group(group).await;
            inner.emit(CoreEvent::Notice(format!(
                "The invitation to “{name}” ran out before someone in the group let you in. \
                 Ask for a new one."
            )));
        }
        NodeEvent::Joined { group, name } => {
            if !inner.is_call(group) {
                // The log the manager sent replaces an invitation kept under the same id:
                // the group is joined, and runs at the next start too.
                inner.save_group(group).await;
                let store = inner.store.clone();
                inner.writer.after(move || {
                    // Kept, the invitation would be tried again at the next start.
                    if let Err(e) = store.set_pending(&hex::encode(group.as_bytes()), None) {
                        tracing::warn!("cannot forget the invitation of a group joined: {e:#}");
                    }
                });
            }
            inner.lock().joining.remove(&group);
            inner.forget_join(group);
            inner.emit(CoreEvent::Joined { group, name });
            inner.emit(CoreEvent::GroupsChanged);
        }
        NodeEvent::Removed { group, log } => {
            if direct::on_removed(inner, group).await {
                return;
            }
            inner.shared.clear_group(group);
            let name = log.shared_state().name.clone();
            // The log stays, to tell this group if this device is added again; its pictures
            // and its members' go.
            let store = inner.store.clone();
            let saved = name.clone();
            inner.writer.after(move || {
                if let Err(e) = store.save_group_log(&log, &saved) {
                    tracing::warn!("cannot save group {group:?}: {e:#}");
                }
                store.forget_group_pictures(&group);
            });
            forget_gone_soon(inner);
            inner.lock().online.remove(&group);
            inner.emit(CoreEvent::Removed { group, name });
            inner.emit(CoreEvent::GroupsChanged);
        }
        NodeEvent::MemberProfile { profile } => {
            let newer = {
                let mut state = inner.lock();
                let newer = state
                    .profiles
                    .get(&profile.device)
                    .is_none_or(|p| p.version_ms < profile.version_ms);
                if newer {
                    state.profiles.insert(profile.device, profile);
                }
                newer
            };
            if newer {
                // Kept for the next start: shown while the member is away too.
                save_profiles_soon(inner);
                inner.emit(CoreEvent::GroupsChanged);
            }
        }
        NodeEvent::Animation { animation } => {
            if animation.device == inner.identity.public_key() {
                return;
            }
            let key = (animation.device, animation.of);
            let device = animation.device;
            let saved = {
                let mut state = inner.lock();
                if state
                    .animations
                    .get(&key)
                    .is_some_and(|a| a.version_ms >= animation.version_ms)
                {
                    return;
                }
                state.animations.insert(key, animation);
                // Bounded: the oldest go first past it.
                while state.animations.len() > MAX_ANIMATIONS {
                    let oldest = state
                        .animations
                        .iter()
                        .min_by_key(|(_, a)| a.version_ms)
                        .map(|(k, _)| *k);
                    if let Some(oldest) = oldest {
                        state.animations.remove(&oldest);
                    }
                }
                state.animations.values().cloned().collect::<Vec<_>>()
            };
            // Kept for the next start: shown while the friend or member is away too.
            save_animations(inner, saved, true);
            inner.emit(CoreEvent::Animation { device });
        }
        NodeEvent::Signal {
            group,
            from,
            payload,
        } => inner.emit(CoreEvent::Signal {
            group,
            from,
            payload,
        }),
        NodeEvent::FriendRequest { profile } => {
            inner.emit(CoreEvent::FriendRequest { profile });
            inner.emit(CoreEvent::FriendsChanged);
        }
        NodeEvent::FriendRequestSent { device, name } => {
            inner.emit(CoreEvent::FriendRequestSent { device, name });
            inner.emit(CoreEvent::FriendsChanged);
        }
        NodeEvent::FriendRequestFailed(reason) => {
            inner.emit(CoreEvent::FriendRequestFailed(reason))
        }
        NodeEvent::FriendAdded { profile } => {
            inner.emit(CoreEvent::FriendAdded { profile });
            inner.emit(CoreEvent::FriendsChanged);
        }
        NodeEvent::FriendOnline {
            device,
            path,
            hidden,
            ..
        } => {
            {
                let mut state = inner.lock();
                state.friends_online.insert(device, path);
                if hidden {
                    state.hidden.insert(device);
                } else {
                    state.hidden.remove(&device);
                    state
                        .meta
                        .last_seen
                        .insert(hex::encode(device.as_bytes()), now);
                }
            }
            inner.save_meta();
            inner.emit(CoreEvent::FriendOnline { device });
            inner.emit(CoreEvent::FriendsChanged);
        }
        NodeEvent::FriendOffline { device } => {
            {
                let mut state = inner.lock();
                state.friends_online.remove(&device);
                state.friend_rooms.remove(&device);
                state.forget_away_if_gone(&device);
                // One that appeared offline was last seen when it hid.
                if !state.hidden.contains(&device) {
                    state
                        .meta
                        .last_seen
                        .insert(hex::encode(device.as_bytes()), now);
                }
            }
            inner.save_meta();
            inner.emit(CoreEvent::FriendOffline { device });
            inner.emit(CoreEvent::FriendsChanged);
            calls::on_offline(inner, device).await;
        }
        NodeEvent::Call { from, message } => calls::on_call(inner, from, message).await,
        NodeEvent::CallUndelivered { to, call, refused } => {
            calls::on_undelivered(inner, to, call, refused).await
        }
        NodeEvent::FriendStatus {
            device,
            in_room,
            group,
            hidden,
            away,
        } => {
            let (seen, away) = {
                let mut state = inner.lock();
                // Hiding looks like going offline, and showing again like coming back.
                let seen = state.hidden.contains(&device) != hidden;
                if seen {
                    state
                        .meta
                        .last_seen
                        .insert(hex::encode(device.as_bytes()), now);
                }
                if hidden {
                    state.hidden.insert(device);
                } else {
                    state.hidden.remove(&device);
                }
                if in_room && !hidden {
                    state.friend_rooms.insert(device, group);
                } else {
                    state.friend_rooms.remove(&device);
                }
                // A friend that is a member too shows as away in the groups' lists.
                let away = if away {
                    state.away.insert(device)
                } else {
                    state.away.remove(&device)
                };
                (seen, away)
            };
            if seen {
                inner.save_meta();
            }
            if away {
                inner.emit(CoreEvent::GroupsChanged);
            }
            inner.emit(CoreEvent::FriendsChanged);
        }
        NodeEvent::MemberPresence {
            device,
            hidden,
            away,
        } => {
            let changed = {
                let mut state = inner.lock();
                let hidden = if hidden {
                    state.hidden.insert(device)
                } else {
                    state.hidden.remove(&device)
                };
                let away = if away {
                    state.away.insert(device)
                } else {
                    state.away.remove(&device)
                };
                hidden || away
            };
            if changed {
                inner.emit(CoreEvent::GroupsChanged);
                inner.emit(CoreEvent::FriendsChanged);
            }
        }
        NodeEvent::FriendsChanged => {
            // A friend may have gone.
            forget_gone_soon(inner);
            // The book is read once for all of these, and handed to the write, not copied
            // (it carries every friend's picture and banner). A burst of changes is one
            // event by then: the node's queue keeps only the latest.
            if let Ok(Some(book)) = inner.node.friends().await {
                direct::on_friends_changed(inner, &book).await;
                calls::on_friends_changed(inner, &book).await;
                save_friends_soon(inner, book);
            }
            inner.emit(CoreEvent::FriendsChanged);
        }
        NodeEvent::DirectOffered { log, by } => direct::on_offered(inner, log, by).await,
        NodeEvent::Typing {
            group,
            voice,
            from,
            typing,
        } => match (direct::friend_of(inner, &group), voice) {
            (Some(friend), _) if friend == from => {
                inner.emit(CoreEvent::DirectTyping { friend, typing })
            }
            (None, Some(voice)) => inner.emit(CoreEvent::Typing {
                group,
                voice,
                from,
                typing,
            }),
            _ => {}
        },
        NodeEvent::Direct {
            group,
            from,
            payloads,
        } => direct::on_messages(inner, group, from, payloads).await,
        NodeEvent::TakenOut {
            group,
            voice,
            by,
            barrier,
        } => {
            inner.shared.clear_group(group);
            inner.emit(CoreEvent::TakenOut {
                group,
                voice,
                by,
                barrier,
            });
            inner.emit(CoreEvent::GroupsChanged);
        }
        NodeEvent::InvitationDeclined { group, device } => {
            let Ok(Some(log)) = inner.node.group(group).await else {
                return;
            };
            let book = inner.node.friends().await.ok().flatten();
            let who = book
                .and_then(|b| b.friends.get(&device).map(|f| f.profile.name.clone()))
                .or_else(|| inner.lock().profiles.get(&device).map(|p| p.name.clone()))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| device.short_fingerprint());
            inner.emit(CoreEvent::InvitationDeclined {
                group,
                name: log.state().name,
                device,
                who,
            });
        }
        NodeEvent::GroupOffered { log, by } => {
            let group = log.group_id();
            let name = log.state().name;
            // Being joined with an invite: only the log a manager sends counts.
            if inner.lock().joining.contains_key(&group) {
                tracing::info!("a friend offered a group this device is joining; ignored");
                return;
            }
            match inner.store.save_offered_group(&log, &by) {
                // Listed with the others either way; announced once a minute at most.
                Ok(Offered::Pending(_)) if inner.offer_noticed(by, now) => {
                    inner.emit(CoreEvent::GroupInvited { group, name, by })
                }
                Ok(Offered::Pending(_)) => {}
                // Already accepted here (an older copy of the log): nothing to ask.
                Ok(Offered::Accepted(_)) => {}
                // Its removal here was undone: it never stopped being a member, and runs the
                // group again at once.
                Ok(Offered::Restored(name)) => match inner.store.load_log_of(&group) {
                    Ok(log) => {
                        if let Err(e) = inner.node.add_group(GroupSetup::Member(log)).await {
                            tracing::warn!("cannot run group {group:?} again: {e:#}");
                            return;
                        }
                        inner.emit(CoreEvent::Joined { group, name });
                    }
                    Err(e) => tracing::warn!("cannot read group {group:?}: {e:#}"),
                },
                // Declined here: the friend gets the leave, so it stops offering the group.
                Ok(Offered::Declined(leave)) => {
                    if let Some(leave) = leave {
                        let DeclineLeave { leave, admission } = *leave;
                        let _ = inner
                            .node
                            .give_entries(vec![by], vec![leave], Some(admission))
                            .await;
                    }
                    return;
                }
                Err(e) => tracing::warn!("cannot save group {group:?}: {e:#}"),
            }
            inner.emit(CoreEvent::GroupsChanged);
        }
    }
}

/// Keeps an address a device was reached at, for the next start.
fn handle_reached(inner: &Arc<Inner>, address: fogata_net::Multiaddr, device: DevicePublicKey) {
    inner.lock().meta.note_reached(
        address.to_string(),
        hex::encode(device.as_bytes()),
        now_ms(),
    );
    inner.save_meta();
}

/// Keeps the capability level a device's copy said it judges at.
fn handle_level_seen(inner: &Arc<Inner>, device: DevicePublicKey, level: u32) {
    let store = inner.store.clone();
    inner.writer.after(move || {
        if let Err(e) = store.note_level(&device, level) {
            tracing::warn!("cannot keep a capability level: {e:#}");
        }
    });
}

/// A member of a group connected, or its path changed.
async fn handle_member_connected(
    inner: &Arc<Inner>,
    group: GroupId,
    device: DevicePublicKey,
    path: Path,
) {
    if direct::on_member_connected(inner, group, device).await {
        return;
    }
    inner
        .lock()
        .online
        .entry(group)
        .or_default()
        .insert(device, path.clone());
    inner.emit(CoreEvent::MemberConnected {
        group,
        device,
        path,
    });
    inner.emit(CoreEvent::GroupsChanged);
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use fogata_crypto::GroupId;
    use fogata_state::FIRST_ROOM;

    use super::*;
    use crate::{Core, NetOptions};

    impl EventSource<u32> for tokio::sync::mpsc::UnboundedReceiver<u32> {
        fn next(&mut self) -> impl std::future::Future<Output = Option<u32>> + Send {
            self.recv()
        }
    }

    /// A panic while one event is handled loses that event only: the ones after it are
    /// handled, and the end of the events is still told, so the app is never left waiting.
    #[tokio::test]
    async fn the_events_go_on_past_one_that_panics() {
        #[derive(Default)]
        struct Seen {
            events: Mutex<Vec<u32>>,
            ended: AtomicBool,
        }
        impl Pumped<u32> for Seen {
            async fn handle(self: Arc<Self>, event: u32) {
                assert_ne!(event, 1, "an event that cannot be handled");
                self.events.lock().unwrap().push(event);
            }
            fn ended(&self) {
                self.ended.store(true, Ordering::Relaxed);
            }
        }
        let seen = Arc::new(Seen::default());
        let (send, events) = tokio::sync::mpsc::unbounded_channel();
        for event in [0, 1, 2] {
            send.send(event).unwrap();
        }
        drop(send);
        let pumped = tokio::spawn(pump(Arc::downgrade(&seen), events)).await;
        assert!(pumped.is_ok(), "the events stopped at the panic");
        assert_eq!(*seen.events.lock().unwrap(), [0, 2]);
        assert!(seen.ended.load(Ordering::Relaxed));
    }

    /// Reading the rooms anew after the node dropped events keeps when a room was last seen
    /// open, which a dropped opening would have set: the room's start, never earlier than
    /// what was kept.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_resync_keeps_when_a_room_was_last_open() {
        let dir = std::env::temp_dir().join(format!("fogata-rooms-anew-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, _events) = Core::start(dir.clone(), options).unwrap();
        let inner = core.inner.clone();
        let group = core.create_group("g", 0, &[]).await.unwrap();
        let quiet = core.create_group("q", 0, &[]).await.unwrap();
        let key = |g: &GroupId| hex::encode(g.as_bytes());
        // A room opened here.
        inner
            .node
            .join_room(group, FIRST_ROOM, "Ana".into())
            .await
            .unwrap();
        let started = loop {
            if let Some(room) = inner.node.room(group, FIRST_ROOM).await.unwrap() {
                break room.started_ms;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        };
        // The opening told, then taken back as if it had been dropped.
        while !inner.lock().meta.last_room.contains_key(&key(&group)) {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        inner.lock().meta.last_room.remove(&key(&group));
        rooms_anew(&inner, [group, quiet].into_iter()).await;
        assert_eq!(
            inner.lock().meta.last_room.get(&key(&group)),
            Some(&started)
        );
        assert_eq!(inner.lock().meta.last_room.get(&key(&quiet)), None);
        // Later than the room's start: kept.
        inner.lock().meta.last_room.insert(key(&group), started + 5);
        rooms_anew(&inner, [group].into_iter()).await;
        assert_eq!(
            inner.lock().meta.last_room.get(&key(&group)),
            Some(&(started + 5))
        );
        core.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// What the core shows for each of the node's events it takes without asking the network
    /// anything: written down before `handle` was split, to be shown the same after. The
    /// events about a group are of one this device is not in, as a group's can be on their
    /// way while it goes.
    #[tokio::test(flavor = "multi_thread")]
    async fn each_node_event_is_shown_as_before() {
        use fogata_crypto::DeviceIdentity;
        use fogata_friends::{Animated, Avatar};
        use fogata_net::PeerId;
        let dir = std::env::temp_dir().join(format!("fogata-events-shown-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, mut events) = Core::start(dir.clone(), options).unwrap();
        let inner = core.inner.clone();
        // The kinds of what waits, past what the node says of its addresses meanwhile.
        let shown = |events: &mut crate::CoreEvents| {
            let mut kinds = Vec::new();
            while let Ok(event) = events.try_recv() {
                let kind = format!("{event:?}");
                let kind = kind
                    .split(|c: char| !c.is_alphanumeric())
                    .next()
                    .unwrap_or_default();
                if !matches!(kind, "Address" | "Info") {
                    kinds.push(kind.to_string());
                }
            }
            kinds
        };
        // What the start shows goes first.
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if shown(&mut events).is_empty() {
                break;
            }
        }
        let ana = DeviceIdentity::generate().unwrap();
        let device = ana.public_key();
        let group = fogata_state::GroupLog::create(&ana, "g", 1)
            .unwrap()
            .group_id();
        let profile = Profile::sign(&ana, "Ana", Avatar::Preset(0), 5).unwrap();
        fn animation(identity: &DeviceIdentity) -> Animation {
            Animation::sign(identity, Animated::Photo, &[], Vec::new(), 5).unwrap()
        }
        let peer = PeerId::random();
        let address: fogata_net::Multiaddr = "/ip4/198.51.100.7/udp/4001/quic-v1".parse().unwrap();
        let cases = vec![
            (NodeEvent::PortChangesPerDestination, vec!["HardNat"]),
            (NodeEvent::OtherSession, vec!["OtherSession"]),
            (
                NodeEvent::SignalsLost {
                    group,
                    from: device,
                },
                vec!["SignalsLost"],
            ),
            (
                NodeEvent::MemberConnected {
                    group,
                    device,
                    peer,
                    path: Path::Direct,
                    address,
                },
                vec!["MemberConnected", "GroupsChanged"],
            ),
            (
                NodeEvent::MemberDisconnected {
                    group,
                    device,
                    peer,
                },
                vec!["MemberDisconnected", "GroupsChanged"],
            ),
            (
                NodeEvent::TakenOut {
                    group,
                    voice: FIRST_ROOM,
                    by: None,
                    barrier: None,
                },
                vec!["TakenOut", "GroupsChanged"],
            ),
            (
                NodeEvent::JoinAsked {
                    group,
                    device,
                    name: "Ana".into(),
                },
                vec!["JoinAsked"],
            ),
            (
                NodeEvent::JoinAnswered { group, device },
                vec!["JoinAsksChanged"],
            ),
            (NodeEvent::JoinWaiting { group }, vec!["JoinWaiting"]),
            (NodeEvent::JoinDeclined { group }, vec![]),
            (
                NodeEvent::MustUpdateToJoin {
                    group,
                    device,
                    name: "Ana".into(),
                },
                vec!["MustUpdateToJoin"],
            ),
            (NodeEvent::UpdateToJoin { group }, vec![]),
            (NodeEvent::JoinExpired { group }, vec![]),
            (
                NodeEvent::MemberProfile {
                    profile: profile.clone(),
                },
                vec!["GroupsChanged"],
            ),
            (
                NodeEvent::MemberProfile {
                    profile: profile.clone(),
                },
                vec![],
            ),
            (
                NodeEvent::Animation {
                    animation: animation(&inner.identity),
                },
                vec![],
            ),
            (
                NodeEvent::Animation {
                    animation: animation(&ana),
                },
                vec!["Animation"],
            ),
            (
                NodeEvent::Animation {
                    animation: animation(&ana),
                },
                vec![],
            ),
            (
                NodeEvent::Signal {
                    group,
                    from: device,
                    payload: vec![1],
                },
                vec!["Signal"],
            ),
            (
                NodeEvent::FriendRequest {
                    profile: profile.clone(),
                },
                vec!["FriendRequest", "FriendsChanged"],
            ),
            (
                NodeEvent::FriendRequestSent {
                    device,
                    name: "Ana".into(),
                },
                vec!["FriendRequestSent", "FriendsChanged"],
            ),
            (
                NodeEvent::FriendRequestFailed("no".into()),
                vec!["FriendRequestFailed"],
            ),
            (
                NodeEvent::FriendAdded {
                    profile: profile.clone(),
                },
                vec!["FriendAdded", "FriendsChanged"],
            ),
            (
                NodeEvent::FriendOnline {
                    device,
                    peer,
                    path: Path::Direct,
                    hidden: false,
                },
                vec!["FriendOnline", "FriendsChanged"],
            ),
            (
                NodeEvent::FriendStatus {
                    device,
                    in_room: false,
                    group: None,
                    hidden: false,
                    away: true,
                },
                vec!["GroupsChanged", "FriendsChanged"],
            ),
            (
                NodeEvent::MemberPresence {
                    device,
                    hidden: false,
                    away: false,
                },
                vec!["GroupsChanged", "FriendsChanged"],
            ),
            (
                NodeEvent::MemberPresence {
                    device,
                    hidden: false,
                    away: false,
                },
                vec![],
            ),
            (
                NodeEvent::FriendOffline { device },
                vec!["FriendOffline", "FriendsChanged"],
            ),
            (
                NodeEvent::Typing {
                    group,
                    voice: Some(FIRST_ROOM),
                    from: device,
                    typing: true,
                },
                vec!["Typing"],
            ),
            (
                NodeEvent::Typing {
                    group,
                    voice: None,
                    from: device,
                    typing: true,
                },
                vec![],
            ),
            (NodeEvent::InvitationDeclined { group, device }, vec![]),
        ];
        let mut found = Vec::new();
        let mut expected = Vec::new();
        for (event, shows) in cases {
            let name = format!("{event:?}");
            let name = name
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or_default()
                .to_string();
            handle(&inner, event).await;
            found.push((name.clone(), shown(&mut events)));
            expected.push((
                name,
                shows.into_iter().map(String::from).collect::<Vec<_>>(),
            ));
        }
        assert_eq!(found, expected);
        // What the events kept.
        let state = inner.lock();
        assert!(state.join_asks.is_empty());
        assert!(state.profiles.contains_key(&device));
        assert!(!state.friends_online.contains_key(&device));
        assert!(!state.away.contains(&device));
        drop(state);
        core.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A read that fails (an answer lost to a panic in the node's turn) is tried again, a
    /// few times, and then given up.
    #[tokio::test]
    async fn a_resync_reads_again_what_failed() {
        let tries = [std::time::Duration::from_millis(1); 3];
        let count = std::sync::atomic::AtomicU32::new(0);
        let read = || {
            let n = count.fetch_add(1, Ordering::Relaxed);
            async move { if n < 2 { Err(()) } else { Ok(n) } }
        };
        assert_eq!(read_again(tries, read).await, Some(2));
        let never = || async { Err::<(), ()>(()) };
        assert_eq!(read_again(tries, never).await, None);
    }
}
