//! Direct messages between two friends. Each chat runs over a group of just the two of them,
//! so it has a group's encryption, signatures and checks of who is in it: its messages are
//! sealed with the group's key and signed by their author, and only the two devices can
//! read or write them. What is said is kept on both devices and goes when both are online;
//! meanwhile it waits on the device that wrote it, and nothing is stored anywhere else.
//!
//! The chat's group is made by whichever of the two first writes, and its log goes to the
//! other over their friend connection. Two made at once settle on the one with the lowest
//! id. Messages carry their own id and time and are sent again until the other device says
//! it keeps them (a receipt), so none is lost to a connection that drops, and none is kept
//! twice. Attachments are fetched from the friend's device over the file protocol, as room
//! files are: pictures as they arrive, videos and files when they are opened.
//!
//! Either of the two can pin a message: the pin travels as an item of its own, and the last
//! pin or unpin of a message counts on both devices. A pinned message does not disappear;
//! once unpinned its own time counts again. Pins go only to a copy of the app that reads
//! them ([`PINS_PROTOCOL`]); one from before pins would skip them without a receipt, so
//! they wait here until it is updated.
//!
//! Each can also delete one of its own messages for both: the deletion travels the same
//! way, to a copy that reads it ([`UNSEND_PROTOCOL`]), and goes from here once it arrived;
//! the friend's device deletes the message as it gets it.
//!
//! Writing in a chat is in sending.rs, fetching attachments in fetch.rs, and what the node
//! tells the chats in node_events.rs.

mod fetch;
pub mod model;
mod node_events;
mod sending;
pub mod store;

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use fogata_crypto::{DevicePublicKey, GroupId, names, random_bytes};
use fogata_friends::{FriendBook, Profile};
use fogata_net::{
    NodeHandle, StreamProtocol, TYPING_PROTOCOL,
    files::{self, SharedFiles},
};
use fogata_proto::{DirectFileKind, DirectPayload};
use fogata_state::{GroupLog, MAX_CHAT_CHARS, MAX_THUMB_BYTES, fair_name};

use self::fetch::{Coming, FETCHES, Limit, fetch};
pub use self::fetch::{FILES_DAILY_BYTES, MIN_CHARGE, VOICE_DAILY_BYTES};
use self::node_events::expire;
pub(crate) use self::node_events::{
    flush_waiting, forget_gone, friend_of, on_friends_changed, on_group_changed,
    on_member_connected, on_member_disconnected, on_messages, on_offered, on_removed, start,
    take_connected,
};
pub use self::sending::{Outgoing, VOICE_NAME, VoiceNote};
use self::sending::{fair_voice, payload};
use self::{
    model::{
        Attachment, Body, Chat, FileId, FileKind, Item, ItemId, Local, MAX_FILE_BYTES,
        MAX_FILE_NAME_BYTES, MAX_FILES, Usage, VideoInfo, VoiceInfo, valid_timer,
    },
    store::{DirectStore, LIMITS, Record, Settings, Split},
};
use crate::{Core, CoreEvent, Inner, Person, now_ms, person, person_of, writer::Writer};

/// Marks a device that takes pins in direct chats ([`DirectPayload::Pin`]). No stream of it
/// is ever opened: it only shows among the protocols a device says it takes when it connects
/// (identify), so pins go only to devices that read them. Copies from before it that take
/// typing signals ([`TYPING_PROTOCOL`]) read pins too, which came first.
pub const PINS_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/direct-pins/1");

/// Marks a device that takes deletions for both in direct chats ([`DirectPayload::Unsend`]),
/// as [`PINS_PROTOCOL`] marks one that takes pins.
pub const UNSEND_PROTOCOL: StreamProtocol = StreamProtocol::new("/fogata/direct-unsend/1");

/// Tells the devices `node` connects to, from now on, that this one takes pins
/// ([`PINS_PROTOCOL`]) and deletions for both ([`UNSEND_PROTOCOL`]). Called as the node
/// starts, so the first exchange of what each side takes carries it. Streams of them are
/// taken and dropped.
pub(crate) fn advertise_pins(node: &NodeHandle) {
    for protocol in [PINS_PROTOCOL, UNSEND_PROTOCOL] {
        if let Ok(mut incoming) = node.streams().accept(protocol) {
            let node = node.background();
            tokio::spawn(async move { while node.next_stream(&mut incoming).await.is_some() {} });
        }
    }
}

/// How long a message sent and not yet confirmed waits before it goes again.
const RESEND_AFTER: Duration = Duration::from_secs(15);

/// How often messages that wait for a receipt are sent again; every third time, chats are
/// also checked for messages whose time ran out (which writes a chat's journal again).
const TICK: Duration = Duration::from_secs(20);

/// Most messages sealed in one request; more go in further ones.
const BATCH: usize = 32;

/// Name endings shown as pictures and as videos in the chat.
const PICTURES: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const VIDEOS: [&str; 6] = ["mp4", "m4v", "mov", "webm", "mkv", "avi"];

/// The words for a change of the timer, in a chat's last line.
pub fn timer_words(ms: u64) -> String {
    match ms {
        0 => "Disappearing messages off".to_string(),
        ms if ms == model::DAY_MS => "Disappearing messages: 24 hours".to_string(),
        ms => format!("Disappearing messages: {} days", ms / model::DAY_MS),
    }
}

/// A chat in the list of chats.
#[derive(Clone, Debug)]
pub struct ChatSummary {
    pub friend: Person,
    /// The friend shows as online now.
    pub online: bool,
    /// Online, but nobody used the friend's computer for a while.
    pub away: bool,
    /// The last line and whether this device wrote it.
    pub last: Option<(String, bool)>,
    /// The last line is a voice message alone, this long.
    pub last_voice_ms: Option<u64>,
    /// When the last line reached this device (or was written here), by this device's
    /// clock: chats with something new come first, whatever the friend's clock says.
    pub last_ms: u64,
    pub unread: usize,
    pub timer_ms: u64,
    /// The two stopped being friends then: the chat can only be read.
    pub ended_ms: Option<u64>,
}

/// An attachment as the chat shows it.
#[derive(Clone, Debug)]
pub struct FileView {
    pub file: Attachment,
    /// Bytes fetched so far, while it comes.
    pub received: Option<u64>,
    /// Where it is, once here.
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ItemView {
    pub item: Item,
    pub files: Vec<FileView>,
    /// A message pinned now: it does not disappear meanwhile.
    pub pinned: bool,
}

/// A chat's page.
#[derive(Clone, Debug)]
pub struct ChatView {
    pub friend: Person,
    pub online: bool,
    /// Online, but nobody used the friend's computer for a while.
    pub away: bool,
    /// When the friend was last seen online, while offline.
    pub last_seen_ms: Option<u64>,
    /// The chat's group is connected: what is written goes at once.
    pub connected: bool,
    /// The newest items, oldest first.
    pub items: Vec<ItemView>,
    /// Older items are there too.
    pub more: bool,
    /// The messages pinned now, the most recently pinned first, wherever they are in the
    /// chat.
    pub pinned: Vec<ItemView>,
    pub timer_ms: u64,
    pub ended_ms: Option<u64>,
    pub usage: Usage,
    /// The chat's history on this device's disk was changed, or part of it lost, since
    /// this device last wrote it: found when it was read at this start.
    pub altered: bool,
    /// When it was read: what changed since is asked for with it
    /// ([`Core::direct_chat_changes`]).
    pub version: u64,
}

/// Older items of a chat, before one a screen holds ([`Core::direct_chat_older`]).
#[derive(Clone, Debug)]
pub struct OlderItems {
    /// Oldest first.
    pub items: Vec<ItemView>,
    /// Older items are there too.
    pub more: bool,
}

/// What changed in a chat since a view of it was read ([`Core::direct_chat_changes`]).
#[derive(Clone, Debug)]
pub struct ChatChanges {
    /// The chat now. Its items are those that came or changed since the view was read,
    /// oldest first, unless `whole`; its pinned messages are there only when
    /// `pinned_changed`; `more` says whether items are there before the oldest the view
    /// holds.
    pub view: ChatView,
    /// Items the view may hold that no longer show: gone, or a pin whose message went.
    pub removed: Vec<ItemId>,
    /// What changed is not known: the items are the chat's from the oldest the view held
    /// on, or its newest, to take in place of the view's.
    pub whole: bool,
    /// The messages pinned are given, and may have changed.
    pub pinned_changed: bool,
}

/// Which items of a chat a view of it reads.
enum Pick {
    /// The newest, at most this many.
    Newest(usize),
    /// What changed since stamp `since` from item `from` on, or among the newest `count`
    /// when the view holds none.
    Changes {
        since: u64,
        from: Option<(u64, ItemId)>,
        count: usize,
    },
}

/// Whether `item` shows in its chat: not a pin whose message is gone (an unpin on its way
/// to the friend), nor a deletion for both, which waits for the friend unseen.
fn shows(chat: &Chat, item: &Item) -> bool {
    item.pin_target().is_none_or(|t| chat.contains(t)) && item.unsend_target().is_none()
}

/// What direct messages take on this device.
#[derive(Clone, Debug)]
pub struct StorageView {
    pub total: Usage,
    pub chats: Vec<(Person, Usage)>,
    pub settings: Settings,
}

/// A chat with what goes with it here.
#[derive(Default)]
struct Entry {
    chat: Chat,
    ended_ms: Option<u64>,
    group: Option<GroupLog>,
    /// The friend's profile as last known, for when the two are no longer friends.
    profile: Option<Profile>,
    /// The chat's history on the disk was not as this device last wrote it, when read at
    /// this start (see [`store::Loaded::altered`]).
    altered: bool,
}

impl Entry {
    fn group_id(&self) -> Option<GroupId> {
        self.group.as_ref().map(GroupLog::group_id)
    }
}

#[derive(Default)]
struct ChatsState {
    chats: HashMap<DevicePublicKey, Entry>,
    by_group: HashMap<GroupId, DevicePublicKey>,
    settings: Settings,
    /// Friends whose chat's group is connected now.
    connected: HashSet<DevicePublicKey>,
    /// Attachments being fetched, with the bytes so far.
    downloads: HashMap<(DevicePublicKey, FileId), u64>,
    /// When each message was last sent, while it waits for its receipt.
    sent: HashMap<ItemId, Instant>,
    /// Friends whose device, connected now, sent a pin or confirmed one: it takes them,
    /// whatever it said when it connected.
    takes_pins: HashSet<DevicePublicKey>,
    /// The same for deletions for both.
    takes_unsends: HashSet<DevicePublicKey>,
    /// What each friend's items took in the chat's journal in the last day, once counted
    /// ([`Taken`]).
    taken: HashMap<DevicePublicKey, Taken>,
    /// Each friend's turns for attachments that come by themselves
    /// (`FETCHES_PER_FRIEND` in fetch.rs).
    turns: HashMap<DevicePublicKey, Arc<tokio::sync::Semaphore>>,
    /// Attachments that did not come by themselves: when they may be tried again by
    /// themselves, and how many times they did not come.
    retry: HashMap<(DevicePublicKey, FileId), (Instant, u32)>,
    /// Attachments being sent: a copy in the chat's folder is not removed while a send uses
    /// it (see `Copies` in sending.rs).
    copying: InUse,
}

impl ChatsState {
    /// Notes how much of `file` came from `friend` so far, or that its download ended
    /// (`None`): the messages with it show it changed.
    fn set_download(&mut self, friend: DevicePublicKey, file: FileId, got: Option<u64>) {
        match got {
            Some(got) => self.downloads.insert((friend, file), got),
            None => self.downloads.remove(&(friend, file)),
        };
        if let Some(entry) = self.chats.get_mut(&friend) {
            entry.chat.touch_file(&file);
        }
    }
}

/// Attachments being sent, by how many sends use them, with the copies a removal left for
/// them in the meantime.
#[derive(Default)]
struct InUse {
    sends: HashMap<(DevicePublicKey, FileId), usize>,
    /// The names of the copies of each file in use that a removal left (an earlier message
    /// with the file deleted, or whose time ran out, the chat's files cleared): the send's
    /// message keeps them, or they go as it ends.
    left: HashMap<(DevicePublicKey, FileId), BTreeSet<String>>,
}

impl InUse {
    /// Removes the copies of `file` under `names` from the chat's folder, or, while a send
    /// uses the file, leaves them to it: a message about to be sent with the file found its
    /// copy here, and would refer to one just gone.
    fn remove<'a>(
        &mut self,
        store: &DirectStore,
        friend: &DevicePublicKey,
        file: &FileId,
        names: impl IntoIterator<Item = &'a str>,
    ) {
        if self.uses(friend, file) {
            self.leave(friend, file, names);
        } else {
            store.remove_file(friend, file, names);
        }
    }

    /// Leaves the copies of `file`, which a send uses, under `names` to it.
    fn leave<'a>(
        &mut self,
        friend: &DevicePublicKey,
        file: &FileId,
        names: impl IntoIterator<Item = &'a str>,
    ) {
        tracing::debug!("a copy of a file a send uses stays until it ends");
        self.left
            .entry((*friend, *file))
            .or_default()
            .extend(names.into_iter().map(str::to_string));
    }

    fn uses(&self, friend: &DevicePublicKey, file: &FileId) -> bool {
        self.sends.contains_key(&(*friend, *file))
    }
}

/// The chats of this device. What changes in them is written on the files' thread
/// ([`Writer`]), in the order it happened, asked for while the chats are held: reading a
/// chat never waits for the disk.
pub(crate) struct Chats {
    store: Arc<DirectStore>,
    state: Arc<Mutex<ChatsState>>,
    writer: Arc<Writer>,
    /// Writes of records asked for, and of those done, counted in the order they were
    /// asked for (with the chats held), so what waits for them knows when none is left.
    asked: AtomicU64,
    done: Arc<AtomicU64>,
    /// Held while a chat's items are handed to the node, so two sends at once (a new
    /// message and the tick, say) hand them over in the order they were taken.
    sending: tokio::sync::Mutex<()>,
    /// Turns for attachments that come by themselves, of every chat ([`FETCHES`]).
    turns: Arc<tokio::sync::Semaphore>,
}

/// What a chat's records that could not be written undo, with the chats held.
type Undo = Box<dyn FnOnce(&mut ChatsState) + Send>;

impl std::fmt::Debug for Chats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chats").finish_non_exhaustive()
    }
}

impl Chats {
    /// Reads every chat kept in the data folder, sealed with `key` when it has one; what
    /// changes is written by `writer`.
    pub(crate) fn open(
        data_dir: &Path,
        key: Option<fogata_crypto::at_rest::DataKey>,
        writer: Arc<Writer>,
    ) -> Self {
        let store = DirectStore::open_sealed(data_dir, key);
        // Copies made to play attachments last until the core stops.
        store.forget_copies();
        let mut state = ChatsState {
            settings: store.settings(),
            ..Default::default()
        };
        for (friend, loaded) in store.chats() {
            let entry = Entry {
                chat: loaded.chat,
                ended_ms: loaded.ended_ms,
                profile: loaded
                    .profile
                    .and_then(|bytes| Profile::decode(&bytes).ok())
                    .filter(|p| p.device == friend),
                group: loaded.group,
                altered: loaded.altered,
            };
            if let Some(group) = entry.group_id() {
                state.by_group.insert(group, friend);
            }
            state.chats.insert(friend, entry);
        }
        Self {
            store: Arc::new(store),
            state: Arc::new(Mutex::new(state)),
            writer,
            asked: Default::default(),
            done: Default::default(),
            sending: tokio::sync::Mutex::new(()),
            turns: Arc::new(tokio::sync::Semaphore::new(FETCHES)),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ChatsState> {
        lock_chats(&self.state)
    }

    /// Counts `file` as in use by a send to `friend` (see `Copies` in sending.rs).
    fn start_using(&self, friend: &DevicePublicKey, file: &FileId) {
        *self
            .lock()
            .copying
            .sends
            .entry((*friend, *file))
            .or_default() += 1;
    }

    /// Ends a send's use of files `using`, of which it copied `made` into the chat's folder.
    /// A copy it made that no item of the chat keeps and no other send uses goes, and so
    /// does one a removal left to it meanwhile that no item says is here.
    fn stop_using(&self, friend: &DevicePublicKey, using: &[FileId], made: &[Attachment]) {
        let mut chats = self.lock();
        let ChatsState {
            chats: entries,
            copying,
            ..
        } = &mut *chats;
        let mut left = Vec::new();
        for file in using {
            let key = (*friend, *file);
            if let Some(n) = copying.sends.get_mut(&key) {
                *n -= 1;
                if *n == 0 {
                    copying.sends.remove(&key);
                    left.extend(
                        copying
                            .left
                            .remove(&key)
                            .into_iter()
                            .flatten()
                            .map(|name| (*file, name)),
                    );
                }
            }
        }
        let files = || {
            entries
                .get(friend)
                .into_iter()
                .flat_map(|e| e.chat.items())
                .flat_map(Item::files)
        };
        for (file, name) in left {
            let here = files().any(|f| f.id == file && f.name == name && f.local == Local::Here);
            if !here {
                tracing::debug!("a copy of a file removed while a send used it goes");
                self.store.remove_file(friend, &file, [name.as_str()]);
            }
        }
        for made in made {
            let kept = files().any(|f| f.id == made.id && f.name == made.name);
            if !kept && !copying.uses(friend, &made.id) {
                tracing::debug!("a copy of a file a send did not keep goes");
                self.store
                    .remove_file(friend, &made.id, [made.name.as_str()]);
            }
        }
    }

    /// Serves an attachment kept here under the chat's group.
    fn share(
        &self,
        shared: &SharedFiles,
        group: GroupId,
        friend: &DevicePublicKey,
        file: &Attachment,
    ) {
        shared.insert_kept(
            group,
            file.id,
            self.store.file_path(friend, file),
            self.store.key().cloned(),
        );
    }

    /// Whether `group` is a chat's group, which is neither listed nor saved as a group.
    pub(crate) fn is_chat_group(&self, group: &GroupId) -> bool {
        self.lock().by_group.contains_key(group)
    }

    /// Adds records to a chat's journal, writing it again whole when it grew enough or an
    /// earlier write failed. `entry` already holds what they say; they are written in turn,
    /// after what was asked for before (see [`Chats`]).
    fn journal(&self, friend: &DevicePublicKey, entry: &Entry, records: &[Record]) {
        drop(self.save(friend, entry, records, None));
    }

    /// As [`Self::journal`]; what is returned says, once they are written, whether the
    /// records are on the disk. Those that are not are undone with `undo`, before anything
    /// asked for after them is written.
    fn save(
        &self,
        friend: &DevicePublicKey,
        entry: &Entry,
        records: &[Record],
        undo: Option<Undo>,
    ) -> tokio::sync::oneshot::Receiver<bool> {
        let (done, kept) = tokio::sync::oneshot::channel();
        let split = Split::of(records, &entry.chat);
        let (store, state, friend) = (self.store.clone(), self.state.clone(), *friend);
        let counted = self.count();
        self.writer.after(move || {
            // A journal behind what its chat holds is only brought up to date whole.
            let lagging = store.lagging(&friend);
            let saved = match store.append_split(&friend, split) {
                Ok(true) => {
                    let whole = rewrite(&store, &state, &friend);
                    !lagging || whole
                }
                Ok(false) => true,
                Err(e) => {
                    tracing::warn!("cannot save a chat: {e:#}");
                    false
                }
            };
            if !saved && let Some(undo) = undo {
                undo(&mut lock_chats(&state));
            }
            counted();
            let _ = done.send(saved);
        });
        kept
    }

    /// Counts a write of records asked for now, with the chats held; what is returned
    /// counts it done.
    fn count(&self) -> impl FnOnce() + Send + 'static + use<> {
        let at = self.asked.fetch_add(1, Ordering::Relaxed) + 1;
        let done = self.done.clone();
        move || done.store(at, Ordering::Release)
    }

    /// How many writes of records were asked for so far; with the chats held.
    fn asked(&self) -> u64 {
        self.asked.load(Ordering::Relaxed)
    }

    /// Waits until the writes of records asked for, up to `asked`, are done.
    async fn written_up_to(&self, asked: u64) -> Result<()> {
        if self.done.load(Ordering::Acquire) >= asked {
            return Ok(());
        }
        self.written().await
    }

    /// Writes a chat's journal again whole, in turn, as the chat is then; then does `after`.
    fn rewrite(&self, friend: &DevicePublicKey, after: impl FnOnce(&DirectStore) + Send + 'static) {
        let (store, state, friend) = (self.store.clone(), self.state.clone(), *friend);
        let counted = self.count();
        self.writer.after(move || {
            rewrite(&store, &state, &friend);
            after(&store);
            counted();
        });
    }

    /// Runs `job` on the chats' files in turn, after the records asked for before it.
    fn then_on_disk(&self, job: impl FnOnce(&DirectStore) + Send + 'static) {
        let store = self.store.clone();
        self.writer.after(move || job(&store));
    }

    /// Waits until everything asked for before is written.
    async fn written(&self) -> Result<()> {
        self.writer.then(|| ()).await
    }

    /// Waits, blocking, until everything asked for before is written.
    #[cfg(test)]
    fn settle(&self) {
        let (done, wait) = std::sync::mpsc::channel();
        self.writer.after(move || {
            let _ = done.send(());
        });
        let _ = wait.recv();
    }
}

fn lock_chats(state: &Mutex<ChatsState>) -> std::sync::MutexGuard<'_, ChatsState> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps the friend's profile with the chat, in turn.
fn save_profile(direct: &Chats, friend: &DevicePublicKey, profile: &Profile) {
    let (friend, bytes) = (*friend, profile.encode());
    direct.then_on_disk(move |store| {
        if let Err(e) = store.save_profile(&friend, &bytes) {
            tracing::warn!("cannot keep a friend's profile with the chat: {e:#}");
        }
    });
}

/// Keeps the chat's group, in turn.
fn save_group(direct: &Chats, friend: &DevicePublicKey, log: &GroupLog) {
    let (friend, log) = (*friend, log.clone());
    direct.then_on_disk(move |store| {
        if let Err(e) = store.save_group(&friend, &log) {
            tracing::warn!("cannot save a chat's group: {e:#}");
        }
    });
}

/// Writes a chat's journal and pins again whole with what the chat holds now, if it is
/// still here; returns whether they were written. Only the copy is made with the chats
/// held. On the files' thread.
fn rewrite(store: &DirectStore, state: &Mutex<ChatsState>, friend: &DevicePublicKey) -> bool {
    let Some((chat, ended_ms)) = lock_chats(state)
        .chats
        .get(friend)
        .map(|e| (e.chat.clone(), e.ended_ms))
    else {
        return true;
    };
    store
        .rewrite(friend, &chat, ended_ms)
        .inspect_err(|e| tracing::warn!("cannot save a chat: {e:#}"))
        .is_ok()
}

/// Whether `log` can be the group of a chat between this device and `friend`: the two of
/// them and nobody else, made by one of them, with a key this device can read.
fn fits(inner: &Inner, log: &GroupLog, friend: &DevicePublicKey) -> bool {
    let me = inner.identity.public_key();
    let state = log.state();
    state.members.len() == 2
        && state.is_member(&me)
        && state.is_member(friend)
        && state.creator.is_some_and(|c| c == me || c == *friend)
        && log.current_keys(&inner.identity).is_some()
}

fn file_kind(name: &str, video: bool) -> FileKind {
    let ending = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if video || VIDEOS.contains(&ending.as_str()) {
        FileKind::Video
    } else if PICTURES.contains(&ending.as_str()) {
        FileKind::Picture
    } else {
        FileKind::File
    }
}

/// How a file the friend sent shows: as a picture or a video only if its name says so too.
/// What shows as a video is played, and what plays a file is chosen by its name: one sent as
/// a video under another name (a playlist, say) would be read by whatever player takes it.
fn received_kind(sent: DirectFileKind, name: &str) -> FileKind {
    match (sent, file_kind(name, false)) {
        (DirectFileKind::Picture, FileKind::Picture) => FileKind::Picture,
        (DirectFileKind::Video, FileKind::Video) => FileKind::Video,
        _ => FileKind::File,
    }
}

/// A file's name as it travels: its last part, without control characters, marks that hide
/// or turn text around, or path separators, as it shows (no padding of blank characters).
fn clean_file_name(path: &Path) -> String {
    let name: String = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .filter(|c| !c.is_control() && !fogata_state::hiding_mark(*c) && !matches!(c, '/' | '\\'))
        .collect();
    let mut name = names::tidy(&name);
    let mut end = name.len().min(MAX_FILE_NAME_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name.truncate(end);
    match name.trim() {
        "" => "file".to_string(),
        name => name.to_string(),
    }
}

/// An item from the friend, checked: text and names within bounds and without hiding marks
/// in names, names that show something (kept as they show), sizes that can be files, a
/// known timer. `None` for one that is not kept.
/// How far past the time it arrived the friend may date a change of the timer or a pin. The
/// last change by its author's time counts on both devices, so one dated years ahead would
/// win over every later one, made here or there: a timer no one could change, a message
/// no one could unpin and that would never disappear. A clock this far off is wrong anyway.
const MAX_AHEAD_MS: u64 = 24 * 3600 * 1000;

fn far_ahead(sent_ms: u64, now: u64) -> bool {
    sent_ms > now.saturating_add(MAX_AHEAD_MS)
}

/// This device's unpin of what the friend's `pin` pins, when that message is not here: dated
/// after the pin, so that it counts after it on both devices. `None` for an unpin, or when
/// no id can be made.
fn undo_pin(pin: &Item, now: u64) -> Option<Item> {
    let Body::Pin {
        target,
        pinned: true,
    } = pin.body
    else {
        return None;
    };
    Some(Item {
        id: new_id().ok()?,
        mine: true,
        sent_ms: now.max(pin.sent_ms.saturating_add(1)),
        got_ms: now,
        seq: 0,
        at_ms: 0,
        body: Body::Pin {
            target,
            pinned: false,
        },
        ttl_ms: 0,
        delivered: false,
    })
}

pub(crate) fn received(payload: DirectPayload, now: u64) -> Option<Item> {
    let item = |id, sent_ms, body, ttl_ms| Item {
        id,
        mine: false,
        sent_ms,
        got_ms: now,
        seq: 0,
        at_ms: 0,
        body,
        ttl_ms,
        delivered: true,
    };
    match payload {
        DirectPayload::Message {
            id,
            sent_ms,
            text,
            files,
            ttl_ms,
        } => {
            let text = names::shown_text(&text);
            let fine = text.chars().count() <= MAX_CHAT_CHARS
                && (!text.trim().is_empty() || !files.is_empty())
                && files.len() <= MAX_FILES
                && valid_timer(ttl_ms)
                && files.iter().all(|f| {
                    !names::tidy(&f.name).is_empty()
                        && f.name.len() <= MAX_FILE_NAME_BYTES
                        && fair_name(&f.name, MAX_FILE_NAME_BYTES)
                        && !f.name.contains(['/', '\\'])
                        && f.size <= MAX_FILE_BYTES
                        && f.video
                            .as_ref()
                            .is_none_or(|v| v.thumb.len() <= MAX_THUMB_BYTES)
                });
            if !fine {
                return None;
            }
            let files = files
                .into_iter()
                .map(|f| (received_kind(f.kind, &f.name), f))
                .map(|(kind, f)| Attachment {
                    id: f.id,
                    // A voice message is a file; one that does not look like one shows as
                    // the file it is.
                    voice: f
                        .voice
                        .filter(|v| {
                            f.kind == DirectFileKind::File && fair_voice(&f.name, f.size, v)
                        })
                        .map(|v| VoiceInfo {
                            duration_ms: v.duration_ms,
                            bars: v.bars,
                            played: false,
                        }),
                    kind,
                    name: names::tidy(&f.name),
                    size: f.size,
                    // Only a video shows a first picture: one sent with anything else would
                    // only take room in the chat's journal.
                    video: f
                        .video
                        .filter(|_| kind == FileKind::Video)
                        .map(|v| VideoInfo {
                            thumb: v.thumb,
                            width: v.width,
                            height: v.height,
                            duration_ms: v.duration_ms,
                        }),
                    local: Local::Missing,
                })
                .collect();
            Some(item(id, sent_ms, Body::Message { text, files }, ttl_ms))
        }
        DirectPayload::Timer { id, sent_ms, ms } => (valid_timer(ms) && !far_ahead(sent_ms, now))
            .then(|| item(id, sent_ms, Body::Timer { ms }, 0)),
        DirectPayload::Pin {
            id,
            sent_ms,
            target,
            pinned,
        } => (!far_ahead(sent_ms, now)).then(|| item(id, sent_ms, Body::Pin { target, pinned }, 0)),
        DirectPayload::Unsend {
            id,
            sent_ms,
            target,
        } => Some(item(id, sent_ms, Body::Unsend { target }, 0)),
        DirectPayload::Receipt { .. } => None,
    }
}

/// Most bytes of a friend's items (messages with what describes their files, timers and
/// pins, as the chat's journal keeps them) this device keeps in a day: past them the
/// friend's next items are not confirmed, and the friend's device sends them again until
/// they fit. Each chat's journal and what it holds in memory grow no faster than this. 16 MiB
/// is some eighteen of the largest messages (ten videos, each with its first picture), or
/// thousands of messages of text.
pub const RECORDS_DAILY_BYTES: u64 = 16 << 20;

/// Most a friend's items kept in a chat may take in all ([`Item::weight`]): past it, as past
/// the day's room, the friend's next items wait on the friend's device, until messages here
/// disappear or are deleted. 256 MiB is sixteen days at the day's most, a million messages
/// of text, or some three thousand videos each with the largest first picture: what bounds
/// the chat's journal and what it holds in memory.
pub const RECORDS_MAX_BYTES: u64 = 256 << 20;

/// Whether the friend's `item`, arriving at `now`, fits in what `chat` keeps of the friend's
/// in all and in the day's room (`taken`), which counts it when it does.
fn room_for(chat: &Chat, taken: &mut Taken, now: u64, item: &Item) -> bool {
    chat.theirs().saturating_add(item.weight()) <= RECORDS_MAX_BYTES
        && taken.fits(now, record_bytes(item))
}

/// What a friend's items took in the chat's journal in the last day: when each arrived here
/// and its bytes, and their sum.
#[derive(Debug, Default)]
struct Taken {
    items: VecDeque<(u64, u64)>,
    bytes: u64,
}

impl Taken {
    /// Counts the friend's items in `chat` that arrived in the day before `now`.
    fn of(chat: &Chat, now: u64) -> Self {
        let mut taken = Taken::default();
        let mut items: Vec<&Item> = chat
            .items()
            .iter()
            .filter(|i| !i.mine && i.got_ms.saturating_add(model::DAY_MS) > now)
            .collect();
        items.sort_by_key(|i| i.got_ms);
        for item in items {
            taken.items.push_back((item.got_ms, record_bytes(item)));
            taken.bytes = taken.bytes.saturating_add(record_bytes(item));
        }
        taken
    }

    /// Whether an item of `bytes` arriving at `now` fits in the day's room, and counts it
    /// when it does.
    fn fits(&mut self, now: u64, bytes: u64) -> bool {
        while let Some(&(at, old)) = self.items.front()
            && at.saturating_add(model::DAY_MS) <= now
        {
            self.items.pop_front();
            self.bytes = self.bytes.saturating_sub(old);
        }
        if self.bytes.saturating_add(bytes) > RECORDS_DAILY_BYTES {
            return false;
        }
        self.items.push_back((now, bytes));
        self.bytes = self.bytes.saturating_add(bytes);
        true
    }
}

/// What an item takes in the chat's journal.
fn record_bytes(item: &Item) -> u64 {
    serde_json::to_vec(&Record::Item { item: item.clone() }).map_or(0, |b| b.len() as u64)
}

fn new_id() -> Result<ItemId> {
    let mut id = [0u8; 16];
    random_bytes(&mut id)?;
    Ok(id)
}

impl Core {
    /// The chats with something in them, the newest first.
    pub async fn direct_chats(&self) -> Result<Vec<ChatSummary>> {
        let book = self.inner.node.friends().await?;
        let shown = self.inner.shown_online();
        let away = self.inner.shown_away();
        let chats = self.inner.direct.lock();
        let mut out: Vec<ChatSummary> = chats
            .chats
            .iter()
            .filter(|(_, e)| !e.chat.is_empty())
            .map(|(friend, entry)| {
                // A pin or a deletion is not news: the last line is the last message or timer.
                let last = entry.chat.items().iter().rev().find(|i| !i.is_aside());
                let person = chat_person(friend, entry, book.as_ref());
                ChatSummary {
                    online: shown.contains(friend),
                    away: away.contains(friend),
                    last: last.map(|i| (model::preview(i, &person.name, timer_words), i.mine)),
                    last_voice_ms: last.and_then(model::voice_preview),
                    friend: person,
                    last_ms: last.map_or(0, |i| i.got_ms.max(i.at_ms)),
                    unread: entry.chat.unread(),
                    timer_ms: entry.chat.timer(),
                    ended_ms: entry.ended_ms,
                }
            })
            .collect();
        out.sort_by_key(|c| std::cmp::Reverse(c.last_ms));
        Ok(out)
    }

    /// A chat's newest `limit` items, and what goes with the chat. A friend never written
    /// with gives an empty chat.
    pub async fn direct_chat(&self, friend: DevicePublicKey, limit: usize) -> Result<ChatView> {
        Ok(self.chat_view(friend, Pick::Newest(limit)).await?.view)
    }

    /// The items of a chat just before the item at `before` (its `seq` and id) in the chat's
    /// order, whether that one is still there or not: `count` of them, and more to reach
    /// `seq` `down_to` when given, oldest first. The place of an item never changes, so the
    /// items before it are the same however many came, changed or went after it.
    pub fn direct_chat_older(
        &self,
        friend: DevicePublicKey,
        before: (u64, ItemId),
        count: usize,
        down_to: Option<u64>,
    ) -> Result<OlderItems> {
        let chats = self.inner.direct.lock();
        let Some(entry) = chats.chats.get(&friend) else {
            return Ok(OlderItems {
                items: Vec::new(),
                more: false,
            });
        };
        let chat = &entry.chat;
        let end = chat.place(before);
        let mut start = end.saturating_sub(count);
        if let Some(seq) = down_to {
            start = start.min(chat.place((seq, [0; 16])));
        }
        let view = self.item_view(&chats, &friend, chat);
        Ok(OlderItems {
            items: chat.items()[start..end]
                .iter()
                .filter(|i| shows(chat, i))
                .map(view)
                .collect(),
            more: start > 0,
        })
    }

    /// What changed in a chat since a view of it read at `since` (its `version`) that holds
    /// the items from `from` (the oldest it holds) on, or none: what came or changed among
    /// them, what went, and what goes with the chat. Only when the chat no longer knows what
    /// changed since then (it was read again, or many went) does it give the items from
    /// `from` on whole; a view that holds none gets at most the newest `count`.
    pub async fn direct_chat_changes(
        &self,
        friend: DevicePublicKey,
        since: u64,
        from: Option<(u64, ItemId)>,
        count: usize,
    ) -> Result<ChatChanges> {
        self.chat_view(friend, Pick::Changes { since, from, count })
            .await
    }

    async fn chat_view(&self, friend: DevicePublicKey, pick: Pick) -> Result<ChatChanges> {
        let book = self.inner.node.friends().await?;
        let in_book = book
            .as_ref()
            .is_some_and(|b| b.friends.contains_key(&friend));
        let online = self.inner.shown_online().contains(&friend);
        let away = self.inner.shown_away().contains(&friend);
        let last_seen_ms = self
            .inner
            .lock()
            .meta
            .last_seen
            .get(&hex::encode(friend.as_bytes()))
            .copied();
        let chats = self.inner.direct.lock();
        let empty = Entry::default();
        let entry = match chats.chats.get(&friend) {
            Some(entry) => entry,
            None if in_book => &empty,
            None => bail!("no chat with that device"),
        };
        let chat = &entry.chat;
        // With the chats held: whatever changes after this has a later stamp.
        let version = model::version();
        let view = self.item_view(&chats, &friend, chat);
        let items = chat.items();
        let newest = |count: usize| items.len().saturating_sub(count);
        let (start, items, removed, whole, pinned_changed) = match pick {
            Pick::Newest(limit) => {
                let start = newest(limit);
                let items = items[start..]
                    .iter()
                    .filter(|i| shows(chat, i))
                    .map(&view)
                    .collect();
                (start, items, Vec::new(), true, true)
            }
            Pick::Changes { since, from, count } => {
                let start = from.map_or_else(|| newest(count), |from| chat.place(from));
                if chat.knows_since(since) {
                    let mut removed: Vec<ItemId> = chat.removed_since(since).copied().collect();
                    let mut changed = Vec::new();
                    for item in chat.changed_since(since, start) {
                        if shows(chat, item) {
                            changed.push(view(item));
                        } else {
                            // Shown no longer, as a pin whose message went.
                            removed.push(item.id);
                        }
                    }
                    (
                        start,
                        changed,
                        removed,
                        false,
                        chat.pins_changed_since(since),
                    )
                } else {
                    let items = items[start..]
                        .iter()
                        .filter(|i| shows(chat, i))
                        .map(&view)
                        .collect();
                    (start, items, Vec::new(), true, true)
                }
            }
        };
        let pinned = if pinned_changed {
            chat.pinned()
                .iter()
                .filter_map(|id| chat.get(id))
                .map(&view)
                .collect()
        } else {
            Vec::new()
        };
        let mut usage = chat.usage();
        usage.messages = self.inner.direct.store.journal_bytes(&friend);
        Ok(ChatChanges {
            view: ChatView {
                friend: chat_person(&friend, entry, book.as_ref()),
                online,
                away,
                last_seen_ms,
                // A friend appearing offline is not shown as connected either.
                connected: online && chats.connected.contains(&friend),
                items,
                more: start > 0,
                pinned,
                timer_ms: chat.timer().max(if chat.is_empty() {
                    chats.settings.timer_ms
                } else {
                    0
                }),
                ended_ms: entry.ended_ms,
                usage,
                altered: entry.altered,
                version,
            },
            removed,
            whole,
            pinned_changed,
        })
    }

    /// How an item of the chat with `friend` shows, with its files as they stand here.
    fn item_view<'a>(
        &'a self,
        chats: &'a ChatsState,
        friend: &'a DevicePublicKey,
        chat: &Chat,
    ) -> impl Fn(&Item) -> ItemView + 'a {
        let pinned = chat.pinned();
        move |item: &Item| ItemView {
            item: item.clone(),
            files: item
                .files()
                .iter()
                .map(|f| FileView {
                    file: f.clone(),
                    received: chats.downloads.get(&(*friend, f.id)).copied(),
                    path: (f.local == Local::Here)
                        .then(|| self.inner.direct.store.file_path(friend, f)),
                })
                .collect(),
            pinned: pinned.contains(&item.id),
        }
    }

    /// Marks everything in a chat as read.
    pub fn mark_direct_read(&self, friend: DevicePublicKey) {
        let direct = &self.inner.direct;
        let mut chats = direct.lock();
        let Some(entry) = chats.chats.get_mut(&friend) else {
            return;
        };
        let last = entry.chat.last_seq();
        if entry.chat.read_seq >= last {
            return;
        }
        entry.chat.read_seq = last;
        direct.journal(&friend, entry, &[Record::Read { seq: last }]);
        drop(chats);
        self.inner.emit(CoreEvent::DirectChanged { friend });
    }

    /// Notes the friend's voice message with this file as played here: its dot goes.
    pub fn mark_direct_played(&self, friend: DevicePublicKey, file: FileId) {
        let direct = &self.inner.direct;
        let mut chats = direct.lock();
        let Some(entry) = chats.chats.get_mut(&friend) else {
            return;
        };
        if !entry.chat.set_played(&file) {
            return;
        }
        direct.journal(&friend, entry, &[Record::Played { file }]);
        drop(chats);
        self.inner.emit(CoreEvent::DirectChanged { friend });
    }

    /// Fetches an attachment from the friend: one not fetched yet, or removed here. Progress
    /// and the end come as events.
    pub async fn fetch_direct_file(&self, friend: DevicePublicKey, file: FileId) -> Result<()> {
        fetch(&self.inner, friend, file, true).await
    }

    /// Removes a chat's pictures, videos and files from this device; the messages stay, and
    /// the friend keeps its copies.
    pub fn clear_direct_files(&self, friend: DevicePublicKey) -> Result<()> {
        let direct = &self.inner.direct;
        let mut chats = direct.lock();
        let ChatsState {
            chats: entries,
            copying,
            ..
        } = &mut *chats;
        let entry = entries.get_mut(&friend).context("no such chat")?;
        let here: Vec<Attachment> = entry
            .chat
            .items()
            .iter()
            .flat_map(Item::files)
            .filter(|f| f.local == Local::Here)
            .cloned()
            .collect();
        direct.remove_files(&self.inner.shared, copying, &friend, entry, &here);
        drop(chats);
        self.inner.emit(CoreEvent::DirectChanged { friend });
        Ok(())
    }

    /// Deletes message `item` of the chat from this device, with the files no other message
    /// refers to; the friend keeps its copy. One of this device's own not sent yet is never
    /// sent.
    pub fn delete_direct_message(&self, friend: DevicePublicKey, item: ItemId) -> Result<()> {
        let mut chats = self.inner.direct.lock();
        let ChatsState {
            chats: entries,
            sent,
            copying,
            ..
        } = &mut *chats;
        let entry = entries.get_mut(&friend).context("no such chat")?;
        ensure!(
            entry.chat.get(&item).is_some_and(Item::is_message),
            "no such message"
        );
        node_events::remove_items(&self.inner, &friend, entry, sent, copying, &[item]);
        drop(chats);
        self.inner.emit(CoreEvent::DirectChanged { friend });
        Ok(())
    }

    /// Deletes this device's own message `item` for both: here at once, with the files no
    /// other message refers to, and on the friend's device when the deletion gets there. It
    /// waits here, like a message, until it does; a copy of the friend's from before such
    /// deletions gets it once it is updated.
    pub async fn unsend_direct(&self, friend: DevicePublicKey, item: ItemId) -> Result<()> {
        {
            let direct = &self.inner.direct;
            let mut chats = direct.lock();
            let ChatsState {
                chats: entries,
                sent,
                copying,
                ..
            } = &mut *chats;
            let entry = entries.get_mut(&friend).context("no such chat")?;
            ensure!(
                entry.ended_ms.is_none(),
                "only a chat with a friend sends a deletion"
            );
            ensure!(
                entry
                    .chat
                    .get(&item)
                    .is_some_and(|i| i.is_message() && i.mine),
                "only your own message can be deleted for both"
            );
            let now = now_ms();
            let unsend = Item {
                id: new_id()?,
                mine: true,
                sent_ms: now,
                got_ms: now,
                seq: 0,
                at_ms: 0,
                body: Body::Unsend { target: item },
                ttl_ms: 0,
                delivered: false,
            };
            let records: Vec<Record> = entry
                .chat
                .add(unsend)
                .map(|item| Record::Item { item })
                .into_iter()
                .collect();
            direct.journal(&friend, entry, &records);
            node_events::remove_items(&self.inner, &friend, entry, sent, copying, &[item]);
        }
        flush(&self.inner, friend).await;
        self.inner.emit(CoreEvent::DirectChanged { friend });
        Ok(())
    }

    /// Deletes a chat that can only be read, as after the two stopped being friends: its
    /// messages and files, here only.
    pub async fn delete_direct_chat(&self, friend: DevicePublicKey) -> Result<()> {
        let direct = &self.inner.direct;
        let removed = {
            let mut chats = direct.lock();
            let entry = chats.chats.get(&friend).context("no such chat")?;
            ensure!(
                entry.ended_ms.is_some(),
                "only a chat with someone who is no longer a friend can be deleted"
            );
            if let Some(group) = entry.group_id() {
                self.inner.shared.clear_group(group);
                chats.by_group.remove(&group);
            }
            chats.chats.remove(&friend);
            chats.connected.remove(&friend);
            // After what was asked for before, so nothing of the chat is written again.
            let store = direct.store.clone();
            direct.writer.then(move || store.remove_chat(&friend))
        };
        let removed = removed.await;
        self.inner.emit(CoreEvent::DirectChanged { friend });
        removed?
    }

    /// Copies an attachment kept here into `dir`, under its own name made safe (as room
    /// downloads are, see [`crate::saved_name`]), and returns where it went. The copy (of up
    /// to the whole room for direct messages, opened as it goes when sealed) is made off the
    /// core's own threads, which also keep the connections and calls going.
    pub async fn save_direct_file(
        &self,
        friend: DevicePublicKey,
        file: FileId,
        dir: &Path,
    ) -> Result<PathBuf> {
        let attachment = self.kept_attachment(friend, file)?;
        // The copy leaves the disk the room the core's own files need, as a room download
        // into the same folder does.
        let room = crate::shares::hold_disk(
            &self.inner,
            attachment.size,
            dir,
            "not enough free space for this file",
        )?;
        let (inner, dir) = (self.inner.clone(), dir.to_path_buf());
        tokio::task::spawn_blocking(move || {
            let _room = room;
            let (mut kept, _) = inner
                .direct
                .store
                .open_file(&friend, &attachment)
                .with_context(|| format!("cannot save {}", attachment.name))?;
            crate::downloads::save_received(&mut kept, &dir, &attachment.name)
                .with_context(|| format!("cannot save {}", attachment.name))
        })
        .await?
    }

    /// An attachment kept here, read whole into memory (for a picture on the screen), if it
    /// is at most `max_bytes`.
    pub async fn direct_file_bytes(
        &self,
        friend: DevicePublicKey,
        file: FileId,
        max_bytes: u64,
    ) -> Result<Vec<u8>> {
        let attachment = self.kept_attachment(friend, file)?;
        ensure!(attachment.size <= max_bytes, "too big to show");
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let (mut kept, size) = inner.direct.store.open_file(&friend, &attachment)?;
            ensure!(size <= max_bytes, "too big to show");
            let mut bytes = Vec::with_capacity(size as usize);
            std::io::Read::read_to_end(&mut kept, &mut bytes)?;
            Ok(bytes)
        })
        .await?
    }

    /// Where an attachment kept here can be played from by the system's player: its own file,
    /// or, when it is sealed, a plain copy for this player alone, which goes with
    /// [`Self::forget_direct_file_copy`] and at the next start. The copy leaves the disk the
    /// room the core's own files need, as a download does.
    pub async fn direct_file_to_play(
        &self,
        friend: DevicePublicKey,
        file: FileId,
    ) -> Result<PathBuf> {
        let attachment = self.kept_attachment(friend, file)?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            inner
                .direct
                .store
                .open_to_play_holding(&friend, &attachment, |size, dir| {
                    crate::shares::hold_disk(
                        &inner,
                        size,
                        dir,
                        "not enough free space for this file",
                    )
                })
        })
        .await?
    }

    /// Whether an attachment of the chat with `friend` is kept here: no longer once its
    /// message is deleted or ran out, or its file was removed.
    pub fn direct_file_kept(&self, friend: DevicePublicKey, file: FileId) -> bool {
        self.kept_attachment(friend, file).is_ok()
    }

    /// Removes the plain copy made to play an attachment at `copy`, once its player let it
    /// go; any other path is left alone.
    pub fn forget_direct_file_copy(&self, copy: &Path) {
        self.inner.direct.store.forget_copy(copy);
    }

    fn kept_attachment(&self, friend: DevicePublicKey, file: FileId) -> Result<Attachment> {
        let chats = self.inner.direct.lock();
        let entry = chats.chats.get(&friend).context("no such chat")?;
        entry
            .chat
            .items()
            .iter()
            .flat_map(Item::files)
            .find(|f| f.id == file && f.local == Local::Here)
            .cloned()
            .context("that file is not on this computer")
    }

    /// What direct messages take here, in all and by chat, and the settings.
    pub async fn direct_storage(&self) -> Result<StorageView> {
        let book = self.inner.node.friends().await?;
        let chats = self.inner.direct.lock();
        let mut total = Usage::default();
        let mut by_chat = Vec::new();
        for (friend, entry) in &chats.chats {
            if entry.chat.is_empty() {
                continue;
            }
            let mut usage = entry.chat.usage();
            usage.messages = self.inner.direct.store.journal_bytes(friend);
            total.add(&usage);
            by_chat.push((chat_person(friend, entry, book.as_ref()), usage));
        }
        by_chat.sort_by_key(|(_, u)| std::cmp::Reverse(u.attachments()));
        Ok(StorageView {
            total,
            chats: by_chat,
            settings: chats.settings.clone(),
        })
    }

    /// Removes at once what ran out by `now_ms`, and waits until the chats are written
    /// without it; the core also does it by itself every few seconds.
    pub async fn expire_direct(&self, now_ms: u64) {
        expire(&self.inner, now_ms);
        let _ = self.inner.direct.written().await;
    }

    pub fn direct_settings(&self) -> Settings {
        self.inner.direct.lock().settings.clone()
    }

    /// Changes the settings of direct messages; a smaller room frees space at once.
    pub fn set_direct_settings(&self, settings: Settings) -> Result<()> {
        ensure!(
            LIMITS.contains(&settings.limit),
            "that is not one of the sizes"
        );
        ensure!(
            valid_timer(settings.timer_ms),
            "that is not one of the timers"
        );
        self.inner.direct.store.save_settings(&settings)?;
        self.inner.direct.lock().settings = settings;
        free_space(&self.inner, None, None);
        Ok(())
    }
}

impl Inner {
    /// Refuses what has to reach `friend` unless both show as online now: nothing new waits
    /// for a friend who is away, or goes from a device that appears offline.
    fn ensure_both_online(&self, friend: &DevicePublicKey, name: &str) -> Result<()> {
        let state = self.lock();
        ensure!(
            !state.appear_offline,
            "you appear offline; go online to write"
        );
        ensure!(
            state.friends_online.contains_key(friend) && !state.hidden.contains(friend),
            "{name} is offline; you can write when you are both online"
        );
        Ok(())
    }

    /// Friends that show as online now: connected, and not appearing offline.
    fn shown_online(&self) -> HashSet<DevicePublicKey> {
        let state = self.lock();
        state
            .friends_online
            .keys()
            .filter(|d| !state.hidden.contains(*d))
            .copied()
            .collect()
    }

    /// Friends that show as online now and are away from their computers.
    fn shown_away(&self) -> HashSet<DevicePublicKey> {
        let state = self.lock();
        state
            .friends_online
            .keys()
            .filter(|d| state.shows_away(d, state.friend_rooms.contains_key(*d)))
            .copied()
            .collect()
    }
}

/// Notes in the chat with `friend` that a call the two were in ended, as a line kept on this
/// device only (each device notes the call as it saw it): this device started it (`mine`),
/// the friend did, or `starter` names who did. It lasts as the chat's messages do. Nothing
/// is noted with someone who is not a friend.
pub(crate) fn note_call(
    inner: &Inner,
    book: &FriendBook,
    friend: DevicePublicKey,
    mine: bool,
    starter: Option<String>,
    lasted_ms: u64,
) {
    let Some(profile) = book.friends.get(&friend).map(|f| &f.profile) else {
        return;
    };
    let Ok(id) = new_id() else {
        return;
    };
    let now = now_ms();
    {
        let direct = &inner.direct;
        let mut chats = direct.lock();
        let entry = chats.chats.entry(friend).or_default();
        if entry.ended_ms.is_some() {
            return;
        }
        if entry.profile.is_none() {
            entry.profile = Some(profile.clone());
            save_profile(direct, &friend, profile);
        }
        let item = Item {
            id,
            mine,
            sent_ms: now,
            got_ms: now,
            seq: 0,
            at_ms: 0,
            body: Body::Call { lasted_ms, starter },
            ttl_ms: entry.chat.timer(),
            delivered: true,
        };
        let records: Vec<Record> = entry
            .chat
            .add(item)
            .map(|item| Record::Item { item })
            .into_iter()
            .collect();
        direct.journal(&friend, entry, &records);
    }
    inner.emit(CoreEvent::DirectChanged { friend });
}

/// The friend of a chat as the chat shows it: from the friends' list, or as last known.
fn chat_person(friend: &DevicePublicKey, entry: &Entry, book: Option<&FriendBook>) -> Person {
    if let Some(f) = book.and_then(|b| b.friends.get(friend)) {
        return person_of(&f.profile);
    }
    match &entry.profile {
        Some(profile) => person_of(profile),
        None => person(*friend, &friend.short_fingerprint()),
    }
}

impl Chats {
    /// Writes again the journals of the chats with charges of attachments that came by
    /// themselves more than two days before `now`, without any older than a day: when and
    /// how big a friend's voice messages and pictures were stays on the disk no longer than
    /// it counts, and a day at most past that. Returns the chats written again.
    fn forget_old_charges(&self, now: u64) -> Vec<DevicePublicKey> {
        let mut chats = self.lock();
        let mut written = Vec::new();
        for (friend, entry) in chats.chats.iter_mut() {
            if entry
                .chat
                .oldest_charge()
                .is_some_and(|at| at.saturating_add(2 * model::DAY_MS) <= now)
            {
                entry.chat.prune_charges(now);
                self.rewrite(friend, |_| {});
                written.push(*friend);
            }
        }
        written
    }

    /// Removes attachments kept here: their files, and they are no longer served. The
    /// messages stay, saying where each went.
    fn remove_files(
        &self,
        shared: &SharedFiles,
        copying: &mut InUse,
        friend: &DevicePublicKey,
        entry: &mut Entry,
        files: &[Attachment],
    ) {
        let mut records = Vec::new();
        let mut done = HashSet::new();
        for file in files {
            if !done.insert(file.id) {
                continue;
            }
            entry.chat.set_local(&file.id, Local::Removed);
            let names = entry.chat.names_of(&file.id);
            copying.remove(
                &self.store,
                friend,
                &file.id,
                names.iter().map(String::as_str),
            );
            if let Some(group) = entry.group_id() {
                shared.remove(group, file.id);
            }
            records.push(Record::File {
                file: file.id,
                local: Local::Removed,
            });
        }
        self.journal(friend, entry, &records);
    }

    /// Removes the oldest attachments until what is kept fits the room set for them, as
    /// [`model::to_free`] chooses; `keep` (one that just came, or is being sent) stays, and
    /// with `only` none but that chat's go. Returns the chats changed.
    fn free_space(
        &self,
        shared: &SharedFiles,
        keep: Option<(&DevicePublicKey, &FileId)>,
        only: Option<&DevicePublicKey>,
    ) -> HashSet<DevicePublicKey> {
        let mut chats = self.lock();
        let mut changed = HashSet::new();
        let Some(limit) = chats.settings.limit else {
            return changed;
        };
        let list: Vec<(DevicePublicKey, &Chat, u64)> = chats
            .chats
            .iter()
            .map(|(f, e)| (*f, &e.chat, self.store.journal_bytes(f)))
            .collect();
        let gone = model::to_free(&list, limit, keep, only).unwrap_or_default();
        let ChatsState {
            chats: entries,
            copying,
            ..
        } = &mut *chats;
        for (friend, file) in gone {
            // A file a send uses stays: it is about to be the send's, newest of all.
            if copying.uses(&friend, &file) {
                continue;
            }
            let Some(entry) = entries.get_mut(&friend) else {
                continue;
            };
            let attachment = entry
                .chat
                .items()
                .iter()
                .flat_map(Item::files)
                .find(|f| f.id == file)
                .cloned();
            if let Some(attachment) = attachment {
                tracing::info!("an old attachment was removed here to free space");
                self.remove_files(shared, copying, &friend, entry, &[attachment]);
                changed.insert(friend);
            }
        }
        changed
    }
}

/// Frees space as [`Chats::free_space`] does (from chat `only` alone, when given), and
/// tells the screens.
fn free_space(
    inner: &Inner,
    keep: Option<(&DevicePublicKey, &FileId)>,
    only: Option<&DevicePublicKey>,
) {
    for friend in inner.direct.free_space(&inner.shared, keep, only) {
        inner.emit(CoreEvent::DirectChanged { friend });
    }
}

/// Makes the chat's group if it has none (this device writes first) and runs it.
async fn ensure_group(inner: &Arc<Inner>, friend: DevicePublicKey, name: &str) -> Result<()> {
    if inner
        .direct
        .lock()
        .chats
        .get(&friend)
        .is_some_and(|e| e.group.is_some())
    {
        return Ok(());
    }
    let identity = &inner.identity;
    let now = now_ms();
    let mut log = GroupLog::create(identity, "Direct", now)?;
    let shown: String = name
        .chars()
        .take(fogata_state::MAX_MEMBER_NAME_CHARS)
        .collect();
    log.add(identity, friend, &shown, now)?;
    take_group(inner, friend, log, true).await;
    Ok(())
}

/// Makes `log` the chat's group: kept, its files served under it, and run by the node. Two
/// made at once settle on the one with the lowest id, and one made here (`made_here`) gives
/// way to any the chat took from the friend meanwhile.
async fn take_group(inner: &Arc<Inner>, friend: DevicePublicKey, log: GroupLog, made_here: bool) {
    let group = log.group_id();
    let old = {
        let mut chats = inner.direct.lock();
        let entry = chats.chats.entry(friend).or_default();
        if entry
            .group_id()
            .is_some_and(|ours| made_here || ours.as_bytes() < group.as_bytes())
        {
            return;
        }
        let old = entry.group.replace(log.clone());
        let here: Vec<Attachment> = entry
            .chat
            .items()
            .iter()
            .flat_map(Item::files)
            .filter(|f| f.local == Local::Here)
            .cloned()
            .collect();
        for file in &here {
            inner.direct.share(&inner.shared, group, &friend, file);
        }
        save_group(&inner.direct, &friend, &log);
        let old = old.map(|o| o.group_id()).filter(|o| *o != group);
        if let Some(old) = old {
            chats.by_group.remove(&old);
            chats.connected.remove(&friend);
        }
        chats.by_group.insert(group, friend);
        old
    };
    if let Some(old) = old {
        inner.shared.clear_group(old);
        let _ = inner.node.drop_group(old).await;
    }
    if let Err(e) = inner.node.add_direct(log).await {
        tracing::warn!("cannot run a chat's group: {e}");
    }
}

/// Whether the friend's device, connected now, takes pins: it says so when it connects, or
/// it sent or confirmed one.
async fn takes_pins(inner: &Inner, friend: DevicePublicKey) -> bool {
    if inner.direct.lock().takes_pins.contains(&friend) {
        return true;
    }
    let Ok(Some(peer)) = inner.node.member_peer(friend).await else {
        return false;
    };
    inner.node.advertises(&peer, &PINS_PROTOCOL) || inner.node.advertises(&peer, &TYPING_PROTOCOL)
}

/// Whether the friend's device, connected now, takes deletions for both: it says so when it
/// connects, or it sent or confirmed one.
async fn takes_unsends(inner: &Inner, friend: DevicePublicKey) -> bool {
    if inner.direct.lock().takes_unsends.contains(&friend) {
        return true;
    }
    let Ok(Some(peer)) = inner.node.member_peer(friend).await else {
        return false;
    };
    inner.node.advertises(&peer, &UNSEND_PROTOCOL)
}

/// What the friend's device takes of what not every copy reads.
#[derive(Clone, Copy)]
struct Takes {
    pins: bool,
    unsends: bool,
}

/// This device's items to send the friend now: not confirmed, and pins and deletions for
/// both only if the friend's device takes them; none while all of them were sent a moment
/// ago. Once one is due, those
/// sent a moment ago go again with it, in order: one may have gone into a connection that
/// was already gone (this device asleep, say), and an item must not reach the friend before
/// those written before it.
fn due<'a>(
    chat: &'a Chat,
    sent: &HashMap<ItemId, Instant>,
    now: Instant,
    takes: Takes,
) -> Vec<&'a Item> {
    let waiting: Vec<&Item> = chat
        .waiting()
        .filter(|i| takes.pins || i.pin_target().is_none())
        .filter(|i| takes.unsends || i.unsend_target().is_none())
        .collect();
    let any_due = waiting
        .iter()
        .any(|i| sent.get(&i.id).is_none_or(|at| now - *at >= RESEND_AFTER));
    if any_due { waiting } else { Vec::new() }
}

/// Sends the friend this device's messages it has not confirmed yet, if the chat's group is
/// connected; one sent a moment ago waits for its receipt a little longer. Pins wait while
/// the friend's device does not take them, and go once it does.
async fn flush(inner: &Arc<Inner>, friend: DevicePublicKey) {
    // Appearing offline, nothing goes: a friend would see this device online by what it gets.
    // It all goes again once this device shows ([`Core::set_hidden`]).
    if inner.lock().appear_offline {
        return;
    }
    let _sending = inner.direct.sending.lock().await;
    let takes = Takes {
        pins: takes_pins(inner, friend).await,
        unsends: takes_unsends(inner, friend).await,
    };
    let (group, payloads, asked) = {
        let mut chats = inner.direct.lock();
        if !chats.connected.contains(&friend) {
            return;
        }
        let ChatsState {
            chats: entries,
            sent,
            ..
        } = &mut *chats;
        let Some(entry) = entries.get(&friend) else {
            return;
        };
        let Some(group) = entry.group_id() else {
            return;
        };
        let now = Instant::now();
        let due = due(&entry.chat, sent, now, takes);
        for item in &due {
            sent.insert(item.id, now);
        }
        if !takes.unsends && entry.chat.waiting().any(|i| i.unsend_target().is_some()) {
            tracing::debug!("deletions wait for a copy of the friend's that takes them");
        }
        if !takes.pins {
            let held = entry
                .chat
                .waiting()
                .filter(|i| i.pin_target().is_some())
                .count();
            if held > 0 {
                tracing::debug!(held, "pins wait for a copy of the friend's that takes them");
            }
        }
        let pins = due.iter().filter(|i| i.pin_target().is_some()).count();
        if !due.is_empty() {
            tracing::debug!(items = due.len(), pins, "direct items sent");
        }
        let payloads: Vec<(ItemId, Vec<u8>)> = due
            .into_iter()
            .filter_map(|item| Some((item.id, payload(item)?.encode())))
            .collect();
        (group, payloads, inner.direct.asked())
    };
    if payloads.is_empty() {
        return;
    }
    // Only what is on the disk goes: the records of these items were asked for before
    // they were taken. One whose record could not be written is no longer in the chat.
    if inner.direct.written_up_to(asked).await.is_err() {
        return;
    }
    let payloads: Vec<Vec<u8>> = {
        let chats = inner.direct.lock();
        let Some(entry) = chats.chats.get(&friend) else {
            return;
        };
        payloads
            .into_iter()
            .filter(|(id, _)| entry.chat.contains(id))
            .map(|(_, payload)| payload)
            .collect()
    };
    for batch in payloads.chunks(BATCH) {
        match inner.node.send_direct(group, friend, batch.to_vec()).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::debug!("direct messages not sent now: {e}");
                return;
            }
            Err(_) => return,
        }
    }
}

/// `fetching`, given up past `limit`; `coming` hears of the bytes as they come.
async fn within<T>(
    limit: Limit,
    coming: &Coming,
    fetching: impl std::future::Future<Output = Result<T, files::FileError>>,
) -> Result<T, String> {
    let quiet = match limit {
        Limit::Total(limit) => {
            return match tokio::time::timeout(limit, fetching).await {
                Ok(fetched) => fetched.map_err(|e| e.to_string()),
                Err(_) => Err(format!(
                    "it took more than {} seconds to come",
                    limit.as_secs()
                )),
            };
        }
        Limit::Quiet(quiet) => quiet,
    };
    let mut fetching = std::pin::pin!(fetching);
    loop {
        tokio::select! {
            fetched = &mut fetching => return fetched.map_err(|e| e.to_string()),
            () = tokio::time::sleep_until(coming.last() + quiet) => {
                if coming.last() + quiet <= tokio::time::Instant::now() {
                    return Err(format!(
                        "nothing of it came for {} seconds",
                        quiet.as_secs()
                    ));
                }
            }
        }
    }
}

impl Chats {
    /// Whether attachments kept from before the data folder was sealed are still plain:
    /// being sealed in the background (`seal_files` in node_events.rs), or left so by a start
    /// that could not.
    pub(crate) fn holds_plain_files(&self) -> bool {
        self.store.holds_plain_file()
    }
}

#[cfg(test)]
mod tests {
    use fogata_proto::{DirectFile, WireVideo};

    use super::*;
    use super::{
        fetch::journals,
        model::{Charge, Pool},
    };

    #[test]
    fn kinds_come_from_the_name_or_a_video_preview() {
        assert_eq!(file_kind("lake.JPG", false), FileKind::Picture);
        assert_eq!(file_kind("weekend.mp4", false), FileKind::Video);
        assert_eq!(file_kind("clip.bin", true), FileKind::Video);
        assert_eq!(file_kind("budget.pdf", false), FileKind::File);
        assert_eq!(file_kind("no-extension", false), FileKind::File);
    }

    #[test]
    fn names_that_travel_are_made_safe() {
        assert_eq!(clean_file_name(Path::new("/tmp/a\nb.txt")), "ab.txt");
        assert_eq!(
            clean_file_name(Path::new("photo\u{202E}gnp.exe")),
            "photognp.exe"
        );
        assert_eq!(clean_file_name(Path::new("/")), "file");
        assert_eq!(clean_file_name(Path::new("/tmp/\u{3164}\u{2800}")), "file");
        assert_eq!(
            clean_file_name(Path::new("lake\u{3164}\u{3164}.jpg\u{3164}")),
            "lake .jpg"
        );
        // At most 255 bytes, never cut inside a character.
        let long = clean_file_name(Path::new(&"\u{1F600}".repeat(100)));
        assert_eq!(long.len(), 252);
        assert_eq!(long.chars().count(), 63);
    }

    #[test]
    fn what_shows_as_a_picture_or_a_video_goes_by_its_name_too() {
        use DirectFileKind as Sent;
        assert_eq!(received_kind(Sent::Picture, "lake.JPG"), FileKind::Picture);
        assert_eq!(received_kind(Sent::Video, "weekend.mp4"), FileKind::Video);
        assert_eq!(received_kind(Sent::Video, "weekend.m3u8"), FileKind::File);
        assert_eq!(received_kind(Sent::Picture, "setup.exe"), FileKind::File);
        assert_eq!(received_kind(Sent::Picture, "weekend.mp4"), FileKind::File);
        assert_eq!(received_kind(Sent::File, "lake.jpg"), FileKind::File);
        let sent = DirectPayload::Message {
            id: [1; 16],
            sent_ms: 5,
            text: String::new(),
            files: vec![DirectFile {
                id: [2; 32],
                name: "setup.exe".into(),
                size: 10,
                kind: DirectFileKind::Picture,
                video: None,
                voice: None,
            }],
            ttl_ms: 0,
        };
        assert_eq!(received(sent, 1).unwrap().files()[0].kind, FileKind::File);
    }

    #[test]
    fn a_timer_or_a_pin_dated_far_ahead_is_not_kept() {
        let now = 1_000_000;
        let timer = |sent_ms| DirectPayload::Timer {
            id: [3; 16],
            sent_ms,
            ms: model::DAY_MS,
        };
        let pin = |sent_ms| DirectPayload::Pin {
            id: [4; 16],
            sent_ms,
            target: [1; 16],
            pinned: true,
        };
        for far in [u64::MAX, now + MAX_AHEAD_MS + 1] {
            assert!(received(timer(far), now).is_none());
            assert!(received(pin(far), now).is_none());
        }
        // A clock some hours ahead is still taken as it is.
        let ahead = now + 3 * 3600 * 1000;
        assert_eq!(received(timer(ahead), now).unwrap().sent_ms, ahead);
        assert_eq!(received(pin(ahead), now).unwrap().sent_ms, ahead);
    }

    #[test]
    fn only_a_video_keeps_its_first_picture() {
        let sent = |kind, name: &str| DirectPayload::Message {
            id: [1; 16],
            sent_ms: 5,
            text: String::new(),
            files: vec![DirectFile {
                id: [2; 32],
                name: name.into(),
                size: 10,
                kind,
                video: Some(WireVideo {
                    thumb: vec![7; 1000],
                    width: 4,
                    height: 3,
                    duration_ms: 9,
                }),
                voice: None,
            }],
            ttl_ms: 0,
        };
        let kept = |kind, name| received(sent(kind, name), 1).unwrap().files()[0].clone();
        assert!(kept(DirectFileKind::Video, "weekend.mp4").video.is_some());
        for (kind, name) in [
            (DirectFileKind::File, "notes.txt"),
            (DirectFileKind::Picture, "lake.jpg"),
            (DirectFileKind::Video, "weekend.m3u8"),
        ] {
            assert!(kept(kind, name).video.is_none(), "{name}");
        }
    }

    #[test]
    fn a_friends_items_take_a_bounded_room_in_all() {
        let item = |n: u32, text: String| Item {
            id: {
                let mut id = [0; 16];
                id[..4].copy_from_slice(&n.to_be_bytes());
                id
            },
            mine: false,
            sent_ms: u64::from(n),
            got_ms: u64::from(n),
            seq: 0,
            at_ms: 0,
            body: Body::Message {
                text,
                files: vec![],
            },
            ttl_ms: 0,
            delivered: true,
        };
        // A chat holding nearly all the room: the next item waits, whatever the day.
        let mut chat = Chat::default();
        let big = "x".repeat(4 << 20);
        let mut n = 0;
        while chat.theirs() + item(n, big.clone()).weight() <= RECORDS_MAX_BYTES {
            chat.add(item(n, big.clone()));
            n += 1;
        }
        let now = 100 * model::DAY_MS;
        let mut taken = Taken::default();
        assert!(!room_for(&chat, &mut taken, now, &item(n, big.clone())));
        assert_eq!(taken.bytes, 0, "an item that waits is not counted");
        // Small ones still fit, up to the room in all.
        assert!(room_for(&chat, &mut taken, now, &item(n, "hi".into())));
        // Items gone make room again; this device's own do not count.
        let gone = chat.remove(&[item(0, String::new()).id]);
        assert_eq!(gone.len(), 1);
        assert!(room_for(&chat, &mut taken, now, &item(n, big.clone())));
        let mut mine = item(n + 1, big.clone());
        mine.mine = true;
        let before = chat.theirs();
        chat.add(mine);
        assert_eq!(chat.theirs(), before);
        // A weight is about what a record takes.
        let one = item(1, "hi Leo".into());
        let record = record_bytes(&one);
        assert!(
            one.weight() >= record / 2 && one.weight() <= record * 2,
            "{record}"
        );
    }

    #[test]
    fn a_friends_items_take_a_bounded_room_a_day() {
        let now = 10 * model::DAY_MS;
        let mut taken = Taken::default();
        let half = RECORDS_DAILY_BYTES / 2;
        assert!(taken.fits(now - model::DAY_MS, half));
        assert!(taken.fits(now - 1000, half));
        // A day after the first, its room is free again.
        assert!(!taken.fits(now - 500, 10));
        assert!(taken.fits(now, half));
        assert!(!taken.fits(now, 1));
        // What a chat holds counts once read back: the friend's items of the last day.
        let mut chat = Chat::default();
        let item = |n: u8, got_ms, mine| Item {
            id: [n; 16],
            mine,
            sent_ms: got_ms,
            got_ms,
            seq: 0,
            at_ms: 0,
            body: Body::Message {
                text: "hi".into(),
                files: vec![],
            },
            ttl_ms: 0,
            delivered: true,
        };
        chat.add(item(1, now - model::DAY_MS, false));
        chat.add(item(2, now - 10, false));
        chat.add(item(3, now - 5, true));
        let taken = Taken::of(&chat, now);
        assert_eq!(taken.items.len(), 1);
        assert_eq!(taken.bytes, record_bytes(chat.get(&[2; 16]).unwrap()));
    }

    #[test]
    fn what_comes_from_the_friend_is_checked() {
        let message = |text: &str, name: &str, ttl_ms: u64| DirectPayload::Message {
            id: [1; 16],
            sent_ms: 5,
            text: text.into(),
            files: vec![DirectFile {
                id: [2; 32],
                name: name.into(),
                size: 10,
                kind: DirectFileKind::Picture,
                video: None,
                voice: None,
            }],
            ttl_ms,
        };
        let item = received(message("hi", "lake.jpg", 0), 100).unwrap();
        assert_eq!((item.mine, item.got_ms), (false, 100));
        assert_eq!(item.files()[0].local, Local::Missing);
        assert!(received(message("hi", "photo\u{202E}gnp.exe", 0), 1).is_none());
        assert!(received(message("hi", "../x", 0), 1).is_none());
        assert!(received(message("hi", "\u{3164}\u{FFA0}", 0), 1).is_none());
        let padded = received(message("hi", "\u{2800}lake.jpg\u{3164}", 0), 1).unwrap();
        assert_eq!(padded.files()[0].name, "lake.jpg");
        assert!(received(message("hi", "a.jpg", 5), 1).is_none());
        assert!(received(message(&"x".repeat(MAX_CHAT_CHARS + 1), "a.jpg", 0), 1).is_none());
        let empty = DirectPayload::Message {
            id: [1; 16],
            sent_ms: 5,
            text: "  ".into(),
            files: vec![],
            ttl_ms: 0,
        };
        assert!(received(empty, 1).is_none());
        let odd_timer = DirectPayload::Timer {
            id: [3; 16],
            sent_ms: 1,
            ms: 12,
        };
        assert!(received(odd_timer, 1).is_none());
        // What this device sends reads back as the same item on the other side.
        let mine = Item {
            id: [4; 16],
            mine: true,
            sent_ms: 9,
            got_ms: 9,
            seq: 1,
            at_ms: 9,
            body: Body::Timer { ms: model::DAY_MS },
            ttl_ms: 0,
            delivered: false,
        };
        let back = received(payload(&mine).unwrap(), 20).unwrap();
        assert_eq!((back.id, back.sent_ms, back.body), (mine.id, 9, mine.body));
    }

    /// What every chat's messages take is known without asking the disk, which the chats'
    /// lock is held for: with hundreds of chats, every attachment that comes would otherwise
    /// hold a thread up for as many reads. It follows what is written.
    #[test]
    fn what_messages_take_is_known_without_asking_the_disk() {
        let dir = std::env::temp_dir().join(format!("fogata-direct-sizes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chats = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        let friends: Vec<DevicePublicKey> = (0..50)
            .map(|_| {
                fogata_crypto::DeviceIdentity::generate()
                    .unwrap()
                    .public_key()
            })
            .collect();
        let item = |id: u8| Item {
            id: [id; 16],
            mine: false,
            sent_ms: 100,
            got_ms: 100,
            seq: u64::from(id),
            at_ms: 100,
            body: Body::Message {
                text: format!("message {id}"),
                files: Vec::new(),
            },
            ttl_ms: 0,
            delivered: true,
        };
        for friend in &friends {
            let mut state = chats.lock();
            let entry = state.chats.entry(*friend).or_default();
            let added = entry.chat.add(item(1)).unwrap();
            chats.journal(friend, entry, &[Record::Item { item: added }]);
        }
        chats.settle();
        let on_disk = |friend: &DevicePublicKey| {
            let dir = chats.store.chat_dir(friend);
            ["journal", "pins"]
                .iter()
                .map(|f| std::fs::metadata(dir.join(f)).map_or(0, |m| m.len()))
                .sum::<u64>()
        };
        let read = || {
            chats
                .store
                .sizes_read
                .load(std::sync::atomic::Ordering::Relaxed)
        };
        let before = read();
        let total = journals(&chats.lock(), &chats.store);
        assert_eq!(read(), before, "the disk was asked");
        assert_eq!(total, friends.iter().map(on_disk).sum::<u64>());
        // One chat grows: the total follows.
        {
            let mut state = chats.lock();
            let entry = state.chats.get_mut(&friends[0]).unwrap();
            let added = entry.chat.add(item(2)).unwrap();
            chats.journal(&friends[0], entry, &[Record::Item { item: added }]);
        }
        chats.settle();
        let before = read();
        let total = journals(&chats.lock(), &chats.store);
        assert_eq!(read(), before, "the disk was asked");
        assert_eq!(total, friends.iter().map(on_disk).sum::<u64>());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Two friends whose messages take the whole room for direct messages do not push out
    /// what a third chat keeps, nor their own chats' pinned files and the files sent to them:
    /// only the files they sent that are not pinned go.
    #[test]
    fn a_friend_s_messages_do_not_push_out_what_other_chats_keep() {
        let dir = std::env::temp_dir().join(format!("fogata-direct-share-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chats = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        let key = || {
            fogata_crypto::DeviceIdentity::generate()
                .unwrap()
                .public_key()
        };
        let (ana, leo, bea) = (key(), key(), key());
        let file = |id: u8| Attachment {
            id: [id; 32],
            name: format!("photo{id}.jpg"),
            size: 400,
            kind: FileKind::Picture,
            video: None,
            local: Local::Here,
            voice: None,
        };
        let item = |id: u8, mine: bool, body: Body| Item {
            id: [id; 16],
            mine,
            sent_ms: u64::from(id),
            got_ms: u64::from(id),
            seq: u64::from(id),
            at_ms: u64::from(id),
            body,
            ttl_ms: 0,
            delivered: true,
        };
        let with = |id: u8| Body::Message {
            text: String::new(),
            files: vec![file(id)],
        };
        // Each chat: a file the friend sent, one sent to the friend, and a pinned one the
        // friend sent; Ana and Leo write a lot besides.
        for (friend, base, chatty) in [(ana, 10, true), (leo, 20, true), (bea, 30, false)] {
            chats.store.files_dir(&friend).unwrap();
            let mut items = vec![
                item(base, false, with(base)),
                item(base + 1, true, with(base + 1)),
                item(base + 2, false, with(base + 2)),
                item(
                    base + 3,
                    true,
                    Body::Pin {
                        target: [base + 2; 16],
                        pinned: true,
                    },
                ),
            ];
            if chatty {
                let text = "messages ".repeat(3_000);
                let files = Vec::new();
                items.push(item(base + 4, false, Body::Message { text, files }));
            }
            let mut state = chats.lock();
            let entry = state.chats.entry(friend).or_default();
            let mut records = Vec::new();
            for item in items {
                for f in item.files() {
                    std::fs::write(chats.store.file_path(&friend, f), [0; 400]).unwrap();
                }
                entry.chat.insert(item.clone());
                records.push(Record::Item { item });
            }
            chats.journal(&friend, entry, &records);
        }
        chats.settle();
        let [of_ana, of_leo] = [ana, leo].map(|f| chats.store.journal_bytes(&f));
        // Each of the two takes more than half the room with its messages alone.
        let limit = of_ana.min(of_leo) * 2 - 2;
        chats.lock().settings.limit = Some(limit);
        chats.free_space(&SharedFiles::default(), None, None);
        chats.settle();
        let here =
            |friend: &DevicePublicKey, id: u8| chats.store.file_path(friend, &file(id)).exists();
        assert!(!here(&ana, 10) && !here(&leo, 20), "what they sent goes");
        assert!(
            here(&ana, 11) && here(&ana, 12),
            "Ana's chat keeps the rest"
        );
        assert!(
            here(&leo, 21) && here(&leo, 22),
            "Leo's chat keeps the rest"
        );
        assert!(
            here(&bea, 30) && here(&bea, 31) && here(&bea, 32),
            "Bea's chat keeps all"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn space_goes_from_the_oldest_attachments_on_disk_too() {
        let dir = std::env::temp_dir().join(format!("fogata-direct-space-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chats = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        let friend = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let file = |id: u8| Attachment {
            id: [id; 32],
            name: format!("photo{id}.jpg"),
            size: 400,
            kind: FileKind::Picture,
            video: None,
            local: Local::Here,
            voice: None,
        };
        let item = |id: u8, at: u64| Item {
            id: [id; 16],
            mine: false,
            sent_ms: at,
            got_ms: at,
            seq: u64::from(id),
            at_ms: at,
            body: Body::Message {
                text: String::new(),
                files: vec![file(id)],
            },
            ttl_ms: 0,
            delivered: true,
        };
        chats.store.files_dir(&friend).unwrap();
        {
            let mut state = chats.lock();
            let entry = state.chats.entry(friend).or_default();
            let mut records = Vec::new();
            // The newest first: the order they arrived in does not count, their time does.
            for (id, at) in [(3, 300), (1, 100), (2, 200)] {
                std::fs::write(chats.store.file_path(&friend, &file(id)), [id; 400]).unwrap();
                entry.chat.insert(item(id, at));
                records.push(Record::Item { item: item(id, at) });
            }
            chats.journal(&friend, entry, &records);
        }
        // The messages count against the room too, and stay.
        chats.settle();
        let messages = chats.store.journal_bytes(&friend);
        assert!(messages > 0);
        chats.lock().settings.limit = Some(900 + messages);
        let shared = SharedFiles::default();
        // What just came stays even when it is the oldest.
        let changed = chats.free_space(&shared, Some((&friend, &[1; 32])), None);
        assert_eq!(changed.into_iter().collect::<Vec<_>>(), [friend]);
        let gone = |id: u8| !chats.store.file_path(&friend, &file(id)).exists();
        assert!(!gone(1) && gone(2) && !gone(3));
        // Then the oldest goes first.
        chats.settle();
        let messages = chats.store.journal_bytes(&friend);
        chats.lock().settings.limit = Some(500 + messages);
        chats.free_space(&shared, None, None);
        assert!(gone(1) && gone(2) && !gone(3));
        // Without room for more than the messages, every attachment goes; they stay.
        chats.settle();
        chats.lock().settings.limit = Some(chats.store.journal_bytes(&friend));
        chats.free_space(&shared, None, None);
        assert!(gone(3));
        // The messages stay, and say so after a restart as well.
        chats.settle();
        let again = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        let state = again.lock();
        let kept = &state.chats[&friend].chat;
        assert_eq!(kept.items().len(), 3);
        let local: Vec<Local> = kept.items().iter().map(|i| i.files()[0].local).collect();
        assert_eq!(local, [Local::Removed; 3]);
        drop(state);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A slow disk does not hold up reading a chat: what changes in it is written on the
    /// files' thread, in turn, not with the chats held.
    #[test]
    fn reading_a_chat_does_not_wait_for_the_disk() {
        let dir = std::env::temp_dir().join(format!("fogata-direct-slow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chats = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        chats
            .store
            .slow_ms
            .store(500, std::sync::atomic::Ordering::Relaxed);
        let friend = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let item = |id: u8| Item {
            id: [id; 16],
            mine: true,
            sent_ms: 100,
            got_ms: 100,
            seq: 0,
            at_ms: 0,
            body: Body::Message {
                text: format!("message {id}"),
                files: Vec::new(),
            },
            ttl_ms: 0,
            delivered: false,
        };
        let (taken, held) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| {
                let mut state = chats.lock();
                let entry = state.chats.entry(friend).or_default();
                for id in 1..=2 {
                    let added = entry.chat.add(item(id)).unwrap();
                    if id == 1 {
                        taken.send(()).unwrap();
                    }
                    chats.journal(&friend, entry, &[Record::Item { item: added }]);
                }
            });
            held.recv().unwrap();
            let at = Instant::now();
            let items = chats.lock().chats[&friend].chat.items().len();
            let waited = at.elapsed();
            assert_eq!(items, 2);
            assert!(waited < Duration::from_millis(250), "{waited:?}");
        });
        // Both are on the disk once written, in order.
        chats.settle();
        let again = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        let texts: Vec<String> = again.lock().chats[&friend]
            .chat
            .items()
            .iter()
            .map(|i| match &i.body {
                Body::Message { text, .. } => text.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(texts, ["message 1", "message 2"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn old_charges_do_not_stay_in_the_journal() {
        let dir =
            std::env::temp_dir().join(format!("fogata-direct-charges-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let chats = Chats::open(&dir, None, Arc::new(Writer::start().unwrap()));
        let friend = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let now = now_ms();
        let charge = |at_ms, file| Charge {
            at_ms,
            bytes: 1 << 20,
            file: Some(file),
        };
        let journal = dir
            .join("direct")
            .join(hex::encode(friend.as_bytes()))
            .join("journal");
        let holds = |file: [u8; 32]| {
            let bytes = std::fs::read(&journal).unwrap();
            let hex = hex::encode(file);
            bytes.windows(hex.len()).any(|w| w == hex.as_bytes())
        };
        {
            let mut state = chats.lock();
            let entry = state.chats.entry(friend).or_default();
            let (old, new) = (
                charge(now - 3 * model::DAY_MS, [1; 32]),
                charge(now - 1000, [2; 32]),
            );
            entry.chat.charge(Pool::Voice, old);
            entry.chat.charge(Pool::Files, new);
            let records = [
                Record::charge(Pool::Voice, old),
                Record::charge(Pool::Files, new),
            ];
            chats.journal(&friend, entry, &records);
        }
        chats.settle();
        assert!(holds([1; 32]) && holds([2; 32]));
        // One a day old or less stays; nothing is written again for it.
        assert!(chats.forget_old_charges(now - 2 * model::DAY_MS).is_empty());
        assert_eq!(chats.forget_old_charges(now), [friend]);
        chats.settle();
        assert!(!holds([1; 32]) && holds([2; 32]));
        assert!(chats.forget_old_charges(now).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pins_wait_for_a_device_that_takes_them() {
        const NONE: Takes = Takes {
            pins: false,
            unsends: false,
        };
        const ALL: Takes = Takes {
            pins: true,
            unsends: true,
        };
        let mut chat = Chat::default();
        let mine = |id: u8, body| Item {
            id: [id; 16],
            mine: true,
            sent_ms: 10,
            got_ms: 10,
            seq: 0,
            at_ms: 0,
            body,
            ttl_ms: 0,
            delivered: false,
        };
        chat.add(mine(
            1,
            Body::Message {
                text: "hi".into(),
                files: vec![],
            },
        ));
        chat.add(mine(
            2,
            Body::Pin {
                target: [1; 16],
                pinned: true,
            },
        ));
        let now = Instant::now();
        let mut sent = HashMap::new();
        let ids = |due: Vec<&Item>| due.iter().map(|i| i.id[0]).collect::<Vec<_>>();
        assert_eq!(ids(due(&chat, &sent, now, NONE)), [1]);
        assert_eq!(ids(due(&chat, &sent, now, ALL)), [1, 2]);
        // One sent a moment ago waits for its receipt a little longer, alone...
        sent.insert([1; 16], now);
        assert_eq!(ids(due(&chat, &sent, now, NONE)), Vec::<u8>::new());
        // ...but goes again before one written after it, so the two arrive in order.
        assert_eq!(ids(due(&chat, &sent, now, ALL)), [1, 2]);
        sent.insert([2; 16], now);
        assert_eq!(ids(due(&chat, &sent, now, ALL)), Vec::<u8>::new());
        assert_eq!(ids(due(&chat, &sent, now + RESEND_AFTER, ALL)), [1, 2]);
    }

    #[test]
    fn timer_lines_say_the_time_in_words() {
        assert_eq!(timer_words(0), "Disappearing messages off");
        assert_eq!(
            timer_words(model::DAY_MS),
            "Disappearing messages: 24 hours"
        );
        assert_eq!(
            timer_words(7 * model::DAY_MS),
            "Disappearing messages: 7 days"
        );
    }

    /// A file a send found kept here (for an earlier message) stays while the send runs,
    /// whatever removes that earlier message's copy meanwhile: deleting it, its time running
    /// out, the chat's files cleared, or room made for others. The send's message keeps it
    /// then; one that is not sent lets it go.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_a_send_uses_stays_until_the_send_ends() {
        let dir = std::env::temp_dir().join(format!("fogata-send-uses-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = crate::NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, _events) = crate::Core::start(dir.clone(), options).unwrap();
        let inner = core.inner.clone();
        let direct = &inner.direct;
        let friend = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        direct.store.files_dir(&friend).unwrap();
        let file = |id: u8| Attachment {
            id: [id; 32],
            name: format!("photo{id}.jpg"),
            size: 400,
            kind: FileKind::Picture,
            video: None,
            local: Local::Here,
            voice: None,
        };
        let item = |id: u8, file_id: u8, ttl_ms: u64| Item {
            id: [id; 16],
            mine: false,
            sent_ms: 1,
            got_ms: 1,
            seq: u64::from(id),
            at_ms: 1,
            body: Body::Message {
                text: String::new(),
                files: vec![file(file_id)],
            },
            ttl_ms,
            delivered: true,
        };
        let add = |item: Item| {
            let mut chats = direct.lock();
            let entry = chats.chats.entry(friend).or_default();
            for f in item.files() {
                std::fs::write(direct.store.file_path(&friend, f), [0; 400]).unwrap();
            }
            entry.chat.insert(item.clone());
            direct.journal(&friend, entry, &[Record::Item { item }]);
        };
        let here = |id: u8| direct.store.file_path(&friend, &file(id)).exists();
        let local = |id: u8| {
            direct.lock().chats[&friend]
                .chat
                .items()
                .iter()
                .flat_map(Item::files)
                .find(|f| f.id == [id; 32])
                .map(|f| f.local)
        };

        // Deleted while a send uses its file: the copy stays, and the send's message keeps it.
        add(item(1, 1, 0));
        direct.start_using(&friend, &[1; 32]);
        core.delete_direct_message(friend, [1; 16]).unwrap();
        assert!(
            here(1),
            "the copy a send uses went with the earlier message"
        );
        add(item(2, 1, 0));
        direct.stop_using(&friend, &[[1; 32]], &[]);
        assert!(here(1), "the copy the send's message refers to went");

        // Its time ran out while a send used it, and the send failed: the copy goes then.
        add(item(3, 3, 1000));
        direct.start_using(&friend, &[3; 32]);
        expire(&inner, 10_000);
        assert!(
            here(3),
            "the copy a send uses went with the earlier message"
        );
        direct.stop_using(&friend, &[[3; 32]], &[]);
        assert!(!here(3), "a copy no message keeps stayed");

        // The chat's files cleared while a send used one: it stays until the send ends, and
        // goes if the send did not keep it.
        add(item(4, 4, 0));
        direct.start_using(&friend, &[4; 32]);
        core.clear_direct_files(friend).unwrap();
        assert!(here(4) && !here(1));
        assert_eq!(local(4), Some(Local::Removed));
        direct.stop_using(&friend, &[[4; 32]], &[]);
        assert!(!here(4), "a copy no message says is here stayed");

        // Room made for others leaves a file a send uses alone.
        add(item(5, 5, 0));
        direct.start_using(&friend, &[5; 32]);
        direct.lock().settings.limit = Some(1);
        free_space(&inner, None, None);
        assert!(here(5));
        assert_eq!(local(5), Some(Local::Here));
        direct.stop_using(&friend, &[[5; 32]], &[]);
        assert!(here(5), "its message still keeps it");
        free_space(&inner, None, None);
        assert!(!here(5));

        core.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
