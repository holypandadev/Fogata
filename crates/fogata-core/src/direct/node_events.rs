//! What the network node tells the direct chats: friends connecting and going, the chat
//! groups offered, messages and receipts that come, groups that change, and the timer that
//! sends again what waits and lets go of what expired.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
    time::Instant,
};

use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_friends::FriendBook;
use fogata_proto::DirectPayload;
use fogata_state::GroupLog;

use super::{
    ChatsState, Entry as ChatEntry, TICK, Taken,
    fetch::fetch_by_themselves,
    fits, flush,
    model::{self, Body, Item, ItemId},
    received, room_for, save_group, save_profile,
    store::Record,
    take_group, timer_words, undo_pin,
};
use crate::{CoreEvent, Inner, now_ms};

/// The friend whose chat runs over `group`, if it is a chat's group.
pub(crate) fn friend_of(inner: &Inner, group: &GroupId) -> Option<DevicePublicKey> {
    inner.direct.lock().by_group.get(group).copied()
}

/// A member of a group connected. Returns whether the group is a chat's. Told again of a
/// friend already connected (its path changed, or a resync read the members anew after the
/// event), the screens only look again: what waits went when it connected, and goes again
/// in its time (see `flush_waiting`), not twice at once.
pub(crate) async fn on_member_connected(
    inner: &Arc<Inner>,
    group: GroupId,
    device: DevicePublicKey,
) -> bool {
    {
        let mut chats = inner.direct.lock();
        match chats.by_group.get(&group) {
            Some(friend) if *friend == device => {
                if !chats.connected.insert(device) {
                    drop(chats);
                    inner.emit(CoreEvent::DirectChanged { friend: device });
                    return true;
                }
                // Everything waiting goes now.
                chats.sent.clear();
            }
            Some(_) => return true,
            None => return false,
        }
    }
    flush(inner, device).await;
    fetch_by_themselves(inner, device).await;
    inner.emit(CoreEvent::DirectChanged { friend: device });
    true
}

/// A member of a group went. Returns whether the group is a chat's.
pub(crate) fn on_member_disconnected(
    inner: &Inner,
    group: GroupId,
    device: DevicePublicKey,
) -> bool {
    let mut chats = inner.direct.lock();
    match chats.by_group.get(&group) {
        Some(friend) if *friend == device => {
            chats.connected.remove(&device);
            chats.takes_pins.remove(&device);
            chats.takes_unsends.remove(&device);
            drop(chats);
            inner.emit(CoreEvent::DirectChanged { friend: device });
            true
        }
        Some(_) => true,
        None => false,
    }
}

/// The node dropped events (see [`fogata_net::NodeEvent::Resync`]): which friends' chats
/// were connected is forgotten, to be read anew, and given back. A friend that went and
/// came back meanwhile, whose events were dropped, is then told connected as new, and is
/// sent what waits at once, as on any connection.
pub(crate) fn take_connected(inner: &Inner) -> HashSet<DevicePublicKey> {
    std::mem::take(&mut inner.direct.lock().connected)
}

/// The node dropped events (see [`fogata_net::NodeEvent::Resync`]): friends whose chat was
/// connected `before` it and whose chat's group is not among `connected` any more went.
pub(crate) fn forget_gone(
    inner: &Inner,
    before: &HashSet<DevicePublicKey>,
    connected: &HashSet<(GroupId, DevicePublicKey)>,
) {
    let gone: Vec<(GroupId, DevicePublicKey)> = {
        let chats = inner.direct.lock();
        chats
            .by_group
            .iter()
            .filter(|(g, f)| before.contains(*f) && !connected.contains(&(**g, **f)))
            .map(|(g, f)| (*g, *f))
            .collect()
    };
    for (group, friend) in gone {
        on_member_disconnected(inner, group, friend);
    }
}

/// A friend sent the log of a group for the chat between the two. Taken when the chat has
/// none, or when its id is lower than the one the chat has (both made one at once).
pub(crate) async fn on_offered(inner: &Arc<Inner>, log: GroupLog, by: DevicePublicKey) {
    let group = log.group_id();
    // A chat's id is random: one this device knows as anything else (a group, an
    // invitation, a group it left, one it is joining) was chosen to take that place.
    if !fits(inner, &log, &by)
        || inner.is_call(group)
        || inner.store.group_name(&group).is_some()
        || inner.lock().joining.contains_key(&group)
    {
        tracing::debug!("a chat's group from {} that does not fit", by.fingerprint());
        return;
    }
    let take = {
        let chats = inner.direct.lock();
        if chats.by_group.get(&group).is_some_and(|f| *f != by) {
            return;
        }
        match chats.chats.get(&by) {
            Some(entry) if entry.ended_ms.is_some() => false,
            Some(entry) => entry
                .group_id()
                .is_none_or(|ours| group.as_bytes() <= ours.as_bytes()),
            None => true,
        }
    };
    if take {
        take_group(inner, by, log, false).await;
    }
}

/// Messages of a chat from the friend: new items are kept, receipts mark this device's own
/// as delivered, and the friend is told what is kept here. A pin of a message this device
/// no longer has (its time ran out here before the pin came) is answered with an unpin, so
/// that the friend does not keep it pinned, alone, for good: there it goes on its own time.
pub(crate) async fn on_messages(
    inner: &Arc<Inner>,
    group: GroupId,
    from: DevicePublicKey,
    payloads: Vec<Vec<u8>>,
) {
    let now = now_ms();
    let mut receipt = Vec::new();
    let mut news = Vec::new();
    let mut pins = false;
    let mut undone = false;
    let saved = {
        let direct = &inner.direct;
        let mut chats = direct.lock();
        if chats.by_group.get(&group) != Some(&from) {
            return;
        }
        let ChatsState {
            chats: entries,
            sent,
            takes_pins,
            takes_unsends,
            taken,
            ..
        } = &mut *chats;
        let Some(entry) = entries.get_mut(&from) else {
            return;
        };
        if entry.ended_ms.is_some() {
            return;
        }
        let taken = taken
            .entry(from)
            .or_insert_with(|| Taken::of(&entry.chat, now));
        // Past the day's room, this item and those after it wait on the friend's device.
        let mut full = false;
        let mut records = Vec::new();
        let mut delivered = Vec::new();
        // The friend's messages it deleted for both, and this device's own deletions that
        // reached the friend: both go from here once what came is saved.
        let mut gone = Vec::new();
        for bytes in payloads {
            let Ok(payload) = DirectPayload::decode(&bytes) else {
                continue;
            };
            match payload {
                DirectPayload::Receipt { ids } => {
                    for id in ids {
                        if let Some(item) = entry.chat.get_mut(&id)
                            && item.mine
                            && !item.delivered
                        {
                            if item.pin_target().is_some() {
                                takes_pins.insert(from);
                            }
                            if item.unsend_target().is_some() {
                                takes_unsends.insert(from);
                                gone.push(id);
                            }
                            item.delivered = true;
                            sent.remove(&id);
                            delivered.push(id);
                        }
                    }
                }
                // Only a message of the friend's own, and only that message: nothing of the
                // deletion is kept, and it is confirmed whether or not the message is here.
                DirectPayload::Unsend { id, target, .. } => {
                    takes_unsends.insert(from);
                    receipt.push(id);
                    if entry
                        .chat
                        .get(&target)
                        .is_some_and(|i| i.is_message() && !i.mine)
                    {
                        gone.push(target);
                    }
                }
                payload => {
                    let id = match &payload {
                        DirectPayload::Message { id, .. } | DirectPayload::Timer { id, .. } => *id,
                        DirectPayload::Pin { id, .. } => {
                            takes_pins.insert(from);
                            *id
                        }
                        DirectPayload::Receipt { .. } | DirectPayload::Unsend { .. } => continue,
                    };
                    // Kept or not, it is confirmed, so it is not sent again.
                    receipt.push(id);
                    // One removed here (deleted, or its time ran out) is not kept again:
                    // it comes again only because the receipt for it did not reach the
                    // friend.
                    if entry.chat.contains(&id) || entry.chat.forgot(&id) {
                        continue;
                    }
                    let Some(item) = received(payload, now) else {
                        tracing::debug!("a direct message that is not kept");
                        continue;
                    };
                    if full || !room_for(&entry.chat, taken, now, &item) {
                        if !full {
                            tracing::info!("a friend's messages wait: their room here is used");
                        }
                        full = true;
                        receipt.pop();
                        continue;
                    }
                    // A pin only of a message here: one already gone has nothing to keep, and
                    // the friend is told to unpin it.
                    if let Some(target) = item.pin_target() {
                        if !entry.chat.get(target).is_some_and(Item::is_message) {
                            tracing::debug!("a pin of a message that is not here");
                            if let Some(undo) = undo_pin(&item, now)
                                && let Some(undo) = entry.chat.add(undo)
                            {
                                records.push(Record::Item { item: undo });
                                undone = true;
                            }
                            continue;
                        }
                        pins = true;
                    }
                    // After everything here, in the order the friend sent them.
                    let Some(item) = entry.chat.add(item) else {
                        continue;
                    };
                    if let Body::Message { .. } = item.body {
                        news.push(model::preview(&item, "", timer_words));
                    }
                    records.push(Record::Item { item });
                }
            }
        }
        if !delivered.is_empty() {
            records.push(Record::Delivered { ids: delivered });
        }
        // What could not be saved is not confirmed: the friend sends it again, and it is
        // saved then.
        let saved = direct.save(&from, entry, &records, None);
        if !gone.is_empty() {
            remove_items(inner, &from, entry, sent, &gone);
        }
        saved
    };
    if !saved.await.unwrap_or(false) {
        receipt.clear();
    }
    // An unpin lets its message go if its time has passed.
    if pins {
        expire(inner, now);
    }
    // Appearing offline, this device keeps what comes and says nothing back: a receipt
    // would show the friend it is online. The friend sends it again until this device
    // shows again.
    let hidden = inner.lock().appear_offline;
    if !receipt.is_empty() && !hidden {
        let receipt = DirectPayload::Receipt { ids: receipt }.encode();
        if let Ok(Err(e)) = inner.node.send_direct(group, from, vec![receipt]).await {
            tracing::debug!("a receipt did not go: {e}");
        }
    }
    // Unpins go like any item of this device's, again until they arrive; while this device
    // appears offline, once it shows.
    if undone && !hidden {
        flush(inner, from).await;
    }
    if !news.is_empty() {
        fetch_by_themselves(inner, from).await;
    }
    inner.emit(CoreEvent::DirectChanged { friend: from });
    if let Some(text) = news.pop() {
        inner.emit(CoreEvent::DirectMessage { friend: from, text });
    }
}

/// A group's log changed. Returns whether it is a chat's group; one that no longer fits a
/// chat of the two (someone else in it) stops being the chat's group.
pub(crate) async fn on_group_changed(inner: &Arc<Inner>, group: GroupId) -> bool {
    let Some(friend) = inner.direct.lock().by_group.get(&group).copied() else {
        return false;
    };
    let Ok(Some(log)) = inner.node.group(group).await else {
        return true;
    };
    if fits(inner, &log, &friend) {
        let mut chats = inner.direct.lock();
        if let Some(entry) = chats.chats.get_mut(&friend)
            && entry.group_id() == Some(group)
        {
            entry.group = Some(log.clone());
            save_group(&inner.direct, &friend, &log);
        }
    } else {
        tracing::info!("a chat's group no longer fits the two friends; it is let go");
        forget_group(inner, friend, group).await;
    }
    true
}

/// This device is no longer a member of a group. Returns whether it was a chat's group,
/// which is then let go (a new one is made when someone writes).
pub(crate) async fn on_removed(inner: &Arc<Inner>, group: GroupId) -> bool {
    let Some(friend) = inner.direct.lock().by_group.get(&group).copied() else {
        return false;
    };
    forget_group(inner, friend, group).await;
    true
}

async fn forget_group(inner: &Arc<Inner>, friend: DevicePublicKey, group: GroupId) {
    {
        let mut chats = inner.direct.lock();
        chats.by_group.remove(&group);
        chats.connected.remove(&friend);
        if let Some(entry) = chats.chats.get_mut(&friend)
            && entry.group_id() == Some(group)
        {
            entry.group = None;
            inner
                .direct
                .then_on_disk(move |store| store.forget_group(&friend));
        }
    }
    inner.shared.clear_group(group);
    let _ = inner.node.drop_group(group).await;
    inner.emit(CoreEvent::DirectChanged { friend });
}

/// The friends changed. A chat with someone who is no longer a friend can only be read
/// (and its group stops); with a friend again, it goes on. The friend's profile is kept
/// with the chat as it changes.
pub(crate) async fn on_friends_changed(inner: &Arc<Inner>, book: &FriendBook) {
    let now = now_ms();
    let mut stop = Vec::new();
    let mut resume = Vec::new();
    let mut changed = Vec::new();
    {
        let direct = &inner.direct;
        let mut chats = direct.lock();
        let ChatsState {
            chats: entries,
            connected,
            ..
        } = &mut *chats;
        for (friend, entry) in entries.iter_mut() {
            match (book.friends.get(friend), entry.ended_ms) {
                (None, None) => {
                    if entry.chat.is_empty() && entry.group.is_none() {
                        continue;
                    }
                    entry.ended_ms = Some(now);
                    direct.journal(friend, entry, &[Record::Ended { ms: Some(now) }]);
                    connected.remove(friend);
                    if let Some(group) = entry.group_id() {
                        stop.push(group);
                    }
                    changed.push(*friend);
                }
                (Some(_), Some(_)) => {
                    entry.ended_ms = None;
                    direct.journal(friend, entry, &[Record::Ended { ms: None }]);
                    if let Some(log) = entry.group.clone() {
                        resume.push((*friend, log));
                    }
                    changed.push(*friend);
                }
                _ => {}
            }
            if let Some(f) = book.friends.get(friend)
                && entry
                    .profile
                    .as_ref()
                    .is_none_or(|p| p.version_ms < f.profile.version_ms)
            {
                entry.profile = Some(f.profile.clone());
                save_profile(direct, friend, &f.profile);
            }
        }
    }
    for group in stop {
        inner.shared.clear_group(group);
        let _ = inner.node.drop_group(group).await;
    }
    for (friend, log) in resume {
        if fits(inner, &log, &friend) {
            take_group(inner, friend, log, false).await;
        }
    }
    for friend in changed {
        inner.emit(CoreEvent::DirectChanged { friend });
    }
}

/// At start: the friends' profiles go with their chats, the groups of chats with friends
/// run, and every chat is checked for what ran out. Then, while the core runs, chats are
/// checked again now and then.
pub(crate) fn start(inner: &Arc<Inner>) {
    let weak = Arc::downgrade(inner);
    tokio::spawn(async move {
        {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            if let Ok(Some(book)) = inner.node.friends().await {
                on_friends_changed(&inner, &book).await;
                let groups: Vec<(DevicePublicKey, GroupLog)> = {
                    let chats = inner.direct.lock();
                    chats
                        .chats
                        .iter()
                        .filter(|(f, e)| e.ended_ms.is_none() && book.friends.contains_key(*f))
                        .filter_map(|(f, e)| Some((*f, e.group.clone()?)))
                        .collect()
                };
                for (friend, log) in groups {
                    if fits(&inner, &log, &friend) {
                        take_group(&inner, friend, log, false).await;
                    } else {
                        let group = log.group_id();
                        forget_group(&inner, friend, group).await;
                    }
                }
            }
        }
        tick(weak).await;
    });
    seal_files(inner);
}

/// Seals, one by one and away from the async threads, the attachments kept plain before the
/// data folder was sealed. Each takes its plain one's place under the chats' lock, which
/// removing attachments holds too, so a file removed meanwhile does not come back.
fn seal_files(inner: &Arc<Inner>) {
    if inner.direct.store.key().is_none() {
        return;
    }
    let weak = Arc::downgrade(inner);
    tokio::task::spawn_blocking(move || {
        let Some(plain) = weak.upgrade().map(|inner| inner.direct.store.plain_files()) else {
            return;
        };
        if plain.is_empty() {
            return;
        }
        let count = plain.len();
        for path in plain {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let swap = |sealed: &std::path::Path| {
                let _chats = inner.direct.lock();
                if path.exists() {
                    std::fs::rename(sealed, &path)
                } else {
                    std::fs::remove_file(sealed)
                }
            };
            if let Err(e) = inner.direct.store.seal_file(&path, swap) {
                tracing::warn!("an attachment stays plain until the next start: {e:#}");
            }
        }
        tracing::info!("sealed {count} attachments kept from before");
    });
}

/// Sends again what waits for a receipt every [`TICK`], starts again what comes by itself
/// and did not come, and removes what ran out every third time.
async fn tick(weak: Weak<Inner>) {
    for count in 0u64.. {
        {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            if count.is_multiple_of(3) {
                expire(&inner, now_ms());
            }
            flush_waiting(&inner).await;
            fetch_waiting(&inner).await;
        }
        tokio::time::sleep(TICK).await;
    }
}

/// Sends what waits for a receipt in every chat whose friend is connected.
pub(crate) async fn flush_waiting(inner: &Arc<Inner>) {
    let waiting: Vec<DevicePublicKey> = {
        let chats = inner.direct.lock();
        chats
            .connected
            .iter()
            .filter(|f| {
                chats
                    .chats
                    .get(*f)
                    .is_some_and(|e| e.chat.waiting().next().is_some())
            })
            .copied()
            .collect()
    };
    for friend in waiting {
        flush(inner, friend).await;
    }
}

/// Starts again what comes by itself and did not come yet, in every chat whose friend is
/// connected: one that did not come is tried again once its wait is over (`retry_after` in
/// fetch.rs).
async fn fetch_waiting(inner: &Arc<Inner>) {
    let connected: Vec<DevicePublicKey> = inner.direct.lock().connected.iter().copied().collect();
    for friend in connected {
        fetch_by_themselves(inner, friend).await;
    }
}

/// Removes the items whose time ran out, with their files, and writes their chats again so
/// nothing of them stays on the disk.
pub(crate) fn expire(inner: &Inner, now: u64) {
    let direct = &inner.direct;
    direct.forget_old_charges(now);
    let mut chats = direct.lock();
    let ChatsState {
        chats: entries,
        sent,
        ..
    } = &mut *chats;
    let mut changed = Vec::new();
    for (friend, entry) in entries.iter_mut() {
        let ids = entry.chat.expired(now);
        if ids.is_empty() {
            continue;
        }
        remove_items(inner, friend, entry, sent, &ids);
        changed.push(*friend);
    }
    drop(chats);
    for friend in changed {
        inner.emit(CoreEvent::DirectChanged { friend });
    }
}

/// Removes items `ids` from `friend`'s chat here, with the files no other item there refers
/// to, and writes the chat again so nothing of them stays on the disk. Returns what went.
pub(crate) fn remove_items(
    inner: &Inner,
    friend: &DevicePublicKey,
    entry: &mut ChatEntry,
    sent: &mut HashMap<ItemId, Instant>,
    ids: &[ItemId],
) -> Vec<Item> {
    let direct = &inner.direct;
    for id in ids {
        sent.remove(id);
    }
    let gone = entry.chat.remove(ids);
    entry
        .chat
        .forget(gone.iter().filter(|i| !i.mine).map(|i| i.id));
    let mut files = HashSet::new();
    for file in gone.iter().flat_map(Item::files) {
        if files.insert(file.id) && !entry.chat.refers_to(&file.id) {
            let names = gone
                .iter()
                .flat_map(Item::files)
                .filter(|f| f.id == file.id)
                .map(|f| f.name.as_str());
            direct.store.remove_file(friend, &file.id, names);
            if let Some(group) = entry.group_id() {
                inner.shared.remove(group, file.id);
            }
        }
    }
    // The copies put aside of a damaged journal still hold what just disappeared.
    let damaged = *friend;
    direct.rewrite(friend, move |store| store.drop_damaged(&damaged, None));
    gone
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Instant};

    use fogata_crypto::GroupId;

    use super::*;
    use crate::{
        Inner,
        direct::model::{DAY_MS, ItemId},
    };

    /// A friend's chat told connected twice (its path changed, say) sends what waits once,
    /// not again at once: only a connection after it went sends it all again, and a resync,
    /// after which it may have gone and come back unseen.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_friend_told_connected_twice_is_sent_what_waits_once() {
        let dir =
            std::env::temp_dir().join(format!("fogata-connected-twice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = crate::NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, _events) = crate::Core::start(dir.clone(), options).unwrap();
        let inner = core.inner.clone();
        let friend = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let group = GroupId::random().unwrap();
        let item: ItemId = [7; 16];
        let sent_now = |inner: &Inner| {
            inner.direct.lock().sent.insert(item, Instant::now());
        };
        inner.direct.lock().by_group.insert(group, friend);

        assert!(on_member_connected(&inner, group, friend).await);
        assert!(inner.direct.lock().connected.contains(&friend));
        assert!(
            inner.direct.lock().sent.is_empty(),
            "what waits goes as it connects"
        );
        // Sent a moment ago, as it connected.
        sent_now(&inner);
        assert!(on_member_connected(&inner, group, friend).await);
        assert!(
            inner.direct.lock().sent.contains_key(&item),
            "what was sent a moment ago went again"
        );
        // Gone and back: what waits goes again.
        assert!(on_member_disconnected(&inner, group, friend));
        assert!(on_member_connected(&inner, group, friend).await);
        assert!(inner.direct.lock().sent.is_empty());
        // A resync reads the members anew as new: a friend still connected (it may have
        // gone and come back, its events dropped) is sent what waits at once.
        sent_now(&inner);
        let before = take_connected(&inner);
        assert!(before.contains(&friend));
        assert!(on_member_connected(&inner, group, friend).await);
        assert!(
            inner.direct.lock().sent.is_empty(),
            "nothing went again after a resync"
        );
        forget_gone(&inner, &before, &HashSet::from([(group, friend)]));
        assert!(inner.direct.lock().connected.contains(&friend));
        // The node has no such member: on a whole resync the friend went.
        crate::node_events::resync(&inner).await;
        assert!(!inner.direct.lock().connected.contains(&friend));
        // Another group's member is no chat's.
        let other = GroupId::random().unwrap();
        assert!(!on_member_connected(&inner, other, friend).await);
        core.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A friend's message deleted here, or whose time ran out, that the friend sends again
    /// (its receipt did not get there: this device appeared offline, say) is confirmed and
    /// not kept again, also after a restart.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_friends_message_removed_here_does_not_come_back_when_sent_again() {
        let dir = std::env::temp_dir().join(format!("fogata-forgotten-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = || crate::NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let friend = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let group = GroupId::random().unwrap();
        let message = |id: ItemId, ttl_ms| {
            DirectPayload::Message {
                id,
                sent_ms: now_ms(),
                text: "hello".into(),
                files: Vec::new(),
                ttl_ms,
            }
            .encode()
        };
        let holds = |inner: &Inner, id: &ItemId| {
            inner
                .direct
                .lock()
                .chats
                .get(&friend)
                .is_some_and(|e| e.chat.contains(id))
        };
        let (deleted, expiring): (ItemId, ItemId) = ([1; 16], [2; 16]);

        let (core, _events) = crate::Core::start(dir.clone(), options()).unwrap();
        let inner = core.inner.clone();
        {
            let mut chats = inner.direct.lock();
            chats.chats.insert(friend, Default::default());
            chats.by_group.insert(group, friend);
        }
        let payloads = vec![message(deleted, 0), message(expiring, DAY_MS)];
        on_messages(&inner, group, friend, payloads.clone()).await;
        assert!(holds(&inner, &deleted) && holds(&inner, &expiring));
        core.delete_direct_message(friend, deleted).unwrap();
        expire(&inner, now_ms() + 2 * DAY_MS);
        assert!(!holds(&inner, &deleted) && !holds(&inner, &expiring));
        on_messages(&inner, group, friend, payloads.clone()).await;
        assert!(!holds(&inner, &deleted), "a deleted message came back");
        assert!(!holds(&inner, &expiring), "an expired message came back");
        // A new one is still kept.
        on_messages(&inner, group, friend, vec![message([3; 16], 0)]).await;
        assert!(holds(&inner, &[3; 16]));
        inner.direct.written().await.unwrap();
        core.shutdown().await.unwrap();
        drop((inner, core, _events));

        // Remembered on the disk.
        let (core, _events) = crate::Core::start(dir.clone(), options()).unwrap();
        let inner = core.inner.clone();
        inner.direct.lock().by_group.insert(group, friend);
        assert!(holds(&inner, &[3; 16]));
        on_messages(&inner, group, friend, payloads).await;
        assert!(!holds(&inner, &deleted) && !holds(&inner, &expiring));
        core.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
