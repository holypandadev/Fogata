//! A chat with a friend as this device keeps it: its items (messages, changes of the
//! disappearing timer, and messages pinned or unpinned), where each attachment stands here,
//! and the rules for ordering them, letting them disappear and freeing space. No I/O.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

/// An item's random id, the same on both devices.
pub type ItemId = [u8; 16];

/// A file's id: the BLAKE3 hash of its contents.
pub type FileId = [u8; 32];

pub const DAY_MS: u64 = 24 * 3600 * 1000;

/// The disappearing timers a chat can have besides off (0): 24 hours, 7 days, 90 days.
pub const TIMERS_MS: [u64; 3] = [DAY_MS, 7 * DAY_MS, 90 * DAY_MS];

/// Whether `ms` is off or one of [`TIMERS_MS`].
pub fn valid_timer(ms: u64) -> bool {
    ms == 0 || TIMERS_MS.contains(&ms)
}

/// Most messages pinned in a chat. Past it a pin is refused here; when both pin at once and
/// go past it, the most recent pins count, the same on both devices.
pub const MAX_PINS: usize = 50;

/// Pictures up to this size are fetched as they arrive, when that is on.
pub const AUTO_PICTURE_BYTES: u64 = 10 * 1024 * 1024;

/// Most attachments in one message.
pub const MAX_FILES: usize = 10;

/// Longest attachment name, in bytes as UTF-8: as for a file shared in a room, and what a
/// file system takes for a name.
pub const MAX_FILE_NAME_BYTES: usize = 255;

/// Biggest size an attachment may claim: past it the number is not a file but a way to break
/// the sums of what is kept.
pub const MAX_FILE_BYTES: u64 = 1 << 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Picture,
    Video,
    File,
}

/// Where an attachment stands on this device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Local {
    /// Not fetched (yet): a video or file not opened, or a picture too big to come by
    /// itself.
    #[default]
    Missing,
    /// On this device, in the chat's folder.
    Here,
    /// Removed here to free space, or with the chat's files cleared; the other device keeps
    /// its copy, which can be asked for again.
    Removed,
}

/// What a video looks like before it is fetched, as the sender read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoInfo {
    /// A small JPEG of the first picture.
    #[serde(with = "base64_text")]
    pub thumb: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

/// A voice message as its sender recorded it, and whether it was played here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceInfo {
    pub duration_ms: u64,
    /// Its levels from start to end, 0 to 255 ([`fogata_proto::VOICE_BARS`] of them).
    #[serde(with = "base64_text")]
    pub bars: Vec<u8>,
    /// Played on this device; the friend's show a dot until then.
    #[serde(default, skip_serializing_if = "is_false")]
    pub played: bool,
}

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(with = "hex_array")]
    pub id: FileId,
    pub name: String,
    pub size: u64,
    pub kind: FileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoInfo>,
    #[serde(default)]
    pub local: Local,
    /// A voice message (an Ogg Opus file of kind `File`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<VoiceInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Message {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<Attachment>,
    },
    /// The disappearing timer changed to `ms` (0 is off), for the messages after it.
    Timer { ms: u64 },
    /// Message `target` was pinned, or unpinned. It goes with its message.
    Pin {
        #[serde(with = "hex_array")]
        target: ItemId,
        pinned: bool,
    },
    /// This device's own message `target` was deleted for both people. Kept on this device
    /// only until it reached the friend, never shown; the friend's device deletes the
    /// message as it gets it and keeps nothing of this.
    Unsend {
        #[serde(with = "hex_array")]
        target: ItemId,
    },
    /// A call the two were in ended. It lasted `lasted_ms`, from when it was answered until
    /// the last moment two or more people were in it. The item is `mine` when this device
    /// started the call; `starter` names the one who did when that was neither of the two.
    /// Kept on this device only and never sent: each device notes the call as it saw it.
    Call {
        lasted_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        starter: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    #[serde(with = "hex_array")]
    pub id: ItemId,
    /// Written on this device.
    pub mine: bool,
    /// When its author wrote it, by the author's clock.
    pub sent_ms: u64,
    /// When it reached this device, by this device's clock; for this device's own items,
    /// when it was written.
    pub got_ms: u64,
    /// Its place among what reached this device, counting up: the chat goes in this order,
    /// and what came after the last one read is unread.
    pub seq: u64,
    /// When it shows in the chat, by this device's clock: its author's time, but never
    /// before the item above it nor after it reached this device. Set as it is written or
    /// arrives; 0 in journals from before it was kept, worked out as they load
    /// ([`Chat::settle`]).
    #[serde(default)]
    pub at_ms: u64,
    pub body: Body,
    /// How long it lasts on each device once there, 0 for ever. Changes of the timer last
    /// for ever.
    #[serde(default)]
    pub ttl_ms: u64,
    /// This device's own item reached the friend.
    #[serde(default)]
    pub delivered: bool,
}

impl Item {
    /// When it disappears from this device. Each device counts from when the item reached
    /// it, so a clock that is wrong on the other device never makes messages vanish early.
    pub fn expires_ms(&self) -> Option<u64> {
        (self.ttl_ms > 0).then(|| self.got_ms.saturating_add(self.ttl_ms))
    }

    /// Whether the friend's own message is gone from the friend's device by `now_ms`, as
    /// far as this device's clock tells: the friend counts it from when it wrote it.
    pub fn gone_from_its_author(&self, now_ms: u64) -> bool {
        !self.mine && self.ttl_ms > 0 && self.sent_ms.saturating_add(self.ttl_ms) <= now_ms
    }

    /// About what it takes in its chat's journal and in memory, from what never changes
    /// once it is kept: its text (or who started a call), and each file's name and first
    /// picture, with room for the rest.
    pub fn weight(&self) -> u64 {
        let text = match &self.body {
            Body::Message { text, .. } => text.len(),
            Body::Call { starter, .. } => starter.as_ref().map_or(0, String::len),
            Body::Timer { .. } | Body::Pin { .. } | Body::Unsend { .. } => 0,
        };
        let files: u64 = self
            .files()
            .iter()
            .map(|f| {
                let thumb = f.video.as_ref().map_or(0, |v| v.thumb.len() as u64 * 4 / 3);
                ITEM_WEIGHT + f.name.len() as u64 + thumb
            })
            .sum();
        ITEM_WEIGHT + text as u64 + files
    }

    pub fn files(&self) -> &[Attachment] {
        match &self.body {
            Body::Message { files, .. } => files,
            Body::Timer { .. } | Body::Pin { .. } | Body::Unsend { .. } | Body::Call { .. } => &[],
        }
    }

    pub fn files_mut(&mut self) -> &mut [Attachment] {
        match &mut self.body {
            Body::Message { files, .. } => files,
            Body::Timer { .. } | Body::Pin { .. } | Body::Unsend { .. } | Body::Call { .. } => {
                &mut []
            }
        }
    }

    pub fn is_message(&self) -> bool {
        matches!(self.body, Body::Message { .. })
    }

    /// The message a pin or unpin is about.
    pub fn pin_target(&self) -> Option<&ItemId> {
        match &self.body {
            Body::Pin { target, .. } => Some(target),
            _ => None,
        }
    }

    /// The message this device's deletion for both is about.
    pub fn unsend_target(&self) -> Option<&ItemId> {
        match &self.body {
            Body::Unsend { target } => Some(target),
            _ => None,
        }
    }

    /// A pin, an unpin or a deletion for both: kept apart from the journal (in the chat's
    /// pins, which copies from before pins leave alone) and sent only to a device that
    /// takes it.
    pub fn is_aside(&self) -> bool {
        matches!(self.body, Body::Pin { .. } | Body::Unsend { .. })
    }

    fn order(&self) -> (u64, ItemId) {
        (self.seq, self.id)
    }
}

/// What an item or an attachment's record takes besides its text: ids, times and names of
/// fields.
const ITEM_WEIGHT: u64 = 256;

/// How many of the friend's items that were removed here (deleted, or their time ran out)
/// a chat remembers, newest kept. The friend sends an item again until a receipt for it
/// comes back, which may be long after it was removed here (this device appearing offline
/// sends none); one remembered is confirmed and not kept again. Far more than the friend
/// can send in the time it takes the two to exchange receipts, at 16 bytes each.
pub const MAX_FORGOTTEN: usize = 4096;

/// Bytes kept on this device for a chat, by kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub messages: u64,
    pub pictures: u64,
    pub videos: u64,
    pub files: u64,
}

impl Usage {
    /// Pictures, videos and files: what the limit applies to.
    pub fn attachments(&self) -> u64 {
        self.pictures
            .saturating_add(self.videos)
            .saturating_add(self.files)
    }

    pub fn add(&mut self, other: &Usage) {
        self.messages = self.messages.saturating_add(other.messages);
        self.pictures = self.pictures.saturating_add(other.pictures);
        self.videos = self.videos.saturating_add(other.videos);
        self.files = self.files.saturating_add(other.files);
    }
}

/// One chat's items in the order they are shown, and how far they were read.
///
/// They go in the order they reached this device (this device's own as they are written),
/// never by the time their author's clock gave them: a friend's clock behind would put a
/// reply above what it answers, maybe far up the chat. So an item from the friend goes after
/// everything that was here when it arrived; what waited while the two were apart comes in
/// the order the friend wrote it, which is the order the friend sends it in.
///
/// Each item carries, beside it, when it last changed as the screens show it ([`stamp`]),
/// and the chat remembers what was taken out of it lately, so a screen that holds part of
/// the chat asks only for what changed since it read it ([`Chat::changed_since`]).
#[derive(Clone, Debug)]
pub struct Chat {
    items: Vec<Item>,
    /// When each item of `items`, at the same place, last changed.
    stamps: Vec<u64>,
    ids: HashSet<ItemId>,
    /// The last item read here, by its `seq`.
    pub read_seq: u64,
    next_seq: u64,
    /// The friend's voice messages fetched by themselves in the last day, for their daily
    /// allowance: each once, whether the file then came, was not a voice message, or stopped
    /// on its way.
    pub voice_charges: Vec<Charge>,
    /// The same for the friend's pictures, videos and files fetched by themselves.
    pub file_charges: Vec<Charge>,
    /// What the friend's items take ([`Item::weight`]), all together.
    theirs: u64,
    /// The friend's items removed here, oldest first, at most [`MAX_FORGOTTEN`]: one sent
    /// again is confirmed, not kept again.
    forgotten: VecDeque<ItemId>,
    forgotten_ids: HashSet<ItemId>,
    /// Items taken out lately, with when, oldest first, at most [`MAX_REMOVED`].
    removed: VecDeque<(u64, ItemId)>,
    /// What changed is known from this stamp on: a view of the chat from before it (the
    /// chat was made or read again since, or removals it needs were let go) is read again
    /// whole.
    known_from: u64,
    /// When a pin or unpin, or any item, last came or went: the messages pinned may have
    /// changed then.
    pins_stamp: u64,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            stamps: Vec::new(),
            ids: HashSet::new(),
            read_seq: 0,
            next_seq: 0,
            voice_charges: Vec::new(),
            file_charges: Vec::new(),
            theirs: 0,
            forgotten: VecDeque::new(),
            forgotten_ids: HashSet::new(),
            removed: VecDeque::new(),
            known_from: stamp(),
            pins_stamp: 0,
        }
    }
}

/// Two chats are the same when they hold the same, whenever it changed.
impl PartialEq for Chat {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
            && self.read_seq == other.read_seq
            && self.next_seq == other.next_seq
            && self.voice_charges == other.voice_charges
            && self.file_charges == other.file_charges
            && self.theirs == other.theirs
            && self.forgotten == other.forgotten
    }
}

impl Eq for Chat {}

/// How many removals a chat remembers for the screens: a view older than the oldest of
/// them is read again whole. Far more than go between two reads of a chat on screen.
const MAX_REMOVED: usize = 1024;

/// The last stamp given (see [`stamp`]).
static STAMPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A new stamp for a change to a chat, later than every stamp and [`version`] before it.
/// One count for every chat, so a chat made or read again has only stamps later than any
/// view of the chat it replaced.
fn stamp() -> u64 {
    STAMPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

/// The last stamp given: what changes after a view read now gets a later one. Read with the
/// chats held, as they are changed.
pub fn version() -> u64 {
    STAMPS.load(std::sync::atomic::Ordering::SeqCst)
}

/// What is known of a chat without its items: what the list of chats and the figures of
/// what chats take show, and what tells when a chat has to be read for the checks that run
/// over every chat (what ran out, what waits for a receipt, what comes by itself, room to
/// free, old charges). Worked out whole from the chat ([`Summary::of`]) every time it
/// changes, and kept beside its journal, so a start reads it in place of the journal. It
/// only ever says whether to read a chat: what is then done is decided on the chat read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// How many items the chat holds.
    pub items: usize,
    /// The last line of the list: the last item that is not a pin or a deletion.
    pub last: Option<Item>,
    pub unread: usize,
    pub timer_ms: u64,
    /// The two stopped being friends then.
    pub ended_ms: Option<u64>,
    /// What its attachments take here.
    pub usage: Usage,
    /// The earliest time something in it goes ([`Chat::due_ms`]).
    pub due_ms: Option<u64>,
    /// This device's own items the friend has not got yet.
    pub waiting: usize,
    /// The oldest charge of an attachment that came by itself.
    pub oldest_charge: Option<u64>,
    /// The attachments kept here, each file once under each name: what the chat's group
    /// serves the friend.
    pub here: Vec<Named>,
    /// The friend's attachments not here, each file once, at most [`MAX_MISSING`]: those
    /// that may come by themselves. `more_missing` when there are more.
    pub missing: Vec<Missing>,
    #[serde(default)]
    pub more_missing: bool,
}

/// Most of the friend's attachments not here a [`Summary`] lists; past them the chat is
/// read to tell which come by themselves.
pub const MAX_MISSING: usize = 64;

/// A file by its id and one name it goes by in the chat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    #[serde(with = "hex_array")]
    pub id: FileId,
    pub name: String,
}

/// One of the friend's attachments not here, as far as whether it may come by itself
/// goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Missing {
    #[serde(with = "hex_array")]
    pub id: FileId,
    pub kind: FileKind,
    pub size: u64,
    pub voice: bool,
}

impl Summary {
    /// The summary of `chat`, which ended at `ended_ms`.
    pub fn of(chat: &Chat, ended_ms: Option<u64>) -> Self {
        let mut here = Vec::new();
        let mut named = HashSet::new();
        let mut missing = Vec::new();
        let mut absent = HashSet::new();
        let mut more_missing = false;
        for item in &chat.items {
            for f in item.files() {
                if f.local == Local::Here && named.insert((f.id, f.name.as_str())) {
                    here.push(Named {
                        id: f.id,
                        name: f.name.clone(),
                    });
                }
                if !item.mine && f.local == Local::Missing && absent.insert(f.id) {
                    if missing.len() < MAX_MISSING {
                        missing.push(Missing {
                            id: f.id,
                            kind: f.kind,
                            size: f.size,
                            voice: f.voice.is_some(),
                        });
                    } else {
                        more_missing = true;
                    }
                }
            }
        }
        Summary {
            items: chat.items.len(),
            last: chat.items.iter().rev().find(|i| !i.is_aside()).cloned(),
            unread: chat.unread(),
            timer_ms: chat.timer(),
            ended_ms,
            usage: chat.usage(),
            due_ms: chat.due_ms(),
            waiting: chat.waiting().count(),
            oldest_charge: chat.oldest_charge(),
            here,
            missing,
            more_missing,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items == 0
    }
}

/// An attachment of the friend's that started to come by itself: when, what it counts
/// against the day's allowance, and which file it was (none in journals from before files
/// were noted), so a file that comes again within the day is not counted again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Charge {
    pub at_ms: u64,
    pub bytes: u64,
    pub file: Option<FileId>,
}

/// The allowance a charge counts against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Voice,
    Files,
}

impl Chat {
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, id: &ItemId) -> bool {
        self.ids.contains(id)
    }

    pub fn get(&self, id: &ItemId) -> Option<&Item> {
        self.ids
            .contains(id)
            .then(|| self.items.iter().find(|i| i.id == *id))
            .flatten()
    }

    /// An item to change: it counts as changed.
    pub fn get_mut(&mut self, id: &ItemId) -> Option<&mut Item> {
        if !self.ids.contains(id) {
            return None;
        }
        let at = self.items.iter().rposition(|i| i.id == *id)?;
        self.stamps[at] = stamp();
        Some(&mut self.items[at])
    }

    /// Counts the items with an attachment of file id `file` as changed: how it comes
    /// shows with them.
    pub fn touch_file(&mut self, file: &FileId) {
        let now = stamp();
        for (item, at) in self.items.iter().zip(&mut self.stamps) {
            if item.files().iter().any(|f| f.id == *file) {
                *at = now;
            }
        }
    }

    /// Where the items from `from` (an item's `seq` and id) on start in the chat's order:
    /// the first item at or after it, whether that one is still here or not.
    pub fn place(&self, from: (u64, ItemId)) -> usize {
        self.items.partition_point(|i| i.order() < from)
    }

    /// The items from place `at` on ([`Self::place`]) that changed after stamp `since`, or
    /// came, in the chat's order.
    pub fn changed_since(&self, since: u64, at: usize) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .zip(&self.stamps)
            .skip(at)
            .filter(move |(_, s)| **s > since)
            .map(|(item, _)| item)
    }

    /// The items taken out after stamp `since`.
    pub fn removed_since(&self, since: u64) -> impl Iterator<Item = &ItemId> {
        self.removed
            .iter()
            .rev()
            .take_while(move |(s, _)| *s > since)
            .map(|(_, id)| id)
    }

    /// Whether what changed after stamp `since` is all known here: not when the chat was
    /// made (or read again) after it, nor when removals since were let go.
    pub fn knows_since(&self, since: u64) -> bool {
        since >= self.known_from && since <= version()
    }

    /// Whether the messages pinned, or any of them, may have changed after stamp `since`.
    pub fn pins_changed_since(&self, since: u64) -> bool {
        if self.pins_stamp > since {
            return true;
        }
        let pinned: HashSet<ItemId> = self.pinned().into_iter().collect();
        self.items
            .iter()
            .zip(&self.stamps)
            .any(|(item, s)| *s > since && pinned.contains(&item.id))
    }

    /// The `seq` the next item that reaches this device gets.
    pub fn next_seq(&self) -> u64 {
        self.next_seq.max(1)
    }

    /// Adds an item new here, written now or just arrived: after everything here, under the
    /// next `seq`, shown at its author's time kept between the last item's and its own
    /// arrival. Returns it as kept; `None` if the chat has it already.
    pub fn add(&mut self, mut item: Item) -> Option<Item> {
        let floor = self.items.last().map_or(0, |i| i.at_ms);
        item.seq = self.next_seq();
        item.at_ms = item.sent_ms.min(item.got_ms).max(floor);
        self.insert(item.clone()).then_some(item)
    }

    /// Puts an item in its place, by its `seq`; false if the chat has it already, or it is a
    /// pin or unpin older than the one kept for its message. Its `seq` is kept as given (from
    /// the journal) and counted, so later ones come after it.
    ///
    /// Only the last pin or unpin of each message is kept, by its author's time (ties by id),
    /// so pinning and unpinning the same message again and again does not grow the chat.
    /// The last one stays even when it is an unpin: an older change sent again later still
    /// loses to it.
    pub fn insert(&mut self, item: Item) -> bool {
        if self.ids.contains(&item.id) {
            return false;
        }
        if let Some(target) = item.pin_target() {
            let key = (item.sent_ms, item.id);
            let mut older = Vec::new();
            for other in &self.items {
                if other.pin_target() == Some(target) {
                    if (other.sent_ms, other.id) > key {
                        return false;
                    }
                    older.push(other.id);
                }
            }
            self.remove(&older);
        }
        self.ids.insert(item.id);
        if !item.mine {
            self.theirs = self.theirs.saturating_add(item.weight());
        }
        self.next_seq = self.next_seq.max(item.seq.saturating_add(1));
        // Mostly the newest: look from the end.
        let order = item.order();
        let at = self
            .items
            .iter()
            .rposition(|i| i.order() <= order)
            .map_or(0, |p| p + 1);
        let now = stamp();
        // The message a pin is about shows pinned, or no longer.
        if let Some(target) = item.pin_target() {
            self.pins_stamp = now;
            if let Some(t) = self.items.iter().position(|i| i.id == *target) {
                self.stamps[t] = now;
            }
        }
        self.items.insert(at, item);
        self.stamps.insert(at, now);
        true
    }

    /// Gives the items read back from a journal from before their time in the chat was kept
    /// the time [`Self::add`] would have given them, in the order they reached this device.
    pub fn settle(&mut self) {
        let mut floor = 0;
        for item in &mut self.items {
            if item.at_ms == 0 {
                item.at_ms = item.sent_ms.min(item.got_ms).max(floor);
            }
            floor = floor.max(item.at_ms);
        }
    }

    /// Takes items out; returns them.
    pub fn remove(&mut self, ids: &[ItemId]) -> Vec<Item> {
        let gone: HashSet<&ItemId> = ids.iter().filter(|id| self.ids.contains(*id)).collect();
        if gone.is_empty() {
            return Vec::new();
        }
        let now = stamp();
        let mut out = Vec::new();
        let (mut keep, mut stamps) = (Vec::new(), Vec::new());
        for (item, at) in std::mem::take(&mut self.items)
            .into_iter()
            .zip(std::mem::take(&mut self.stamps))
        {
            if gone.contains(&item.id) {
                out.push(item);
            } else {
                keep.push(item);
                stamps.push(at);
            }
        }
        self.items = keep;
        self.stamps = stamps;
        for item in &out {
            self.ids.remove(&item.id);
            if !item.mine {
                self.theirs = self.theirs.saturating_sub(item.weight());
            }
            self.removed.push_back((now, item.id));
        }
        while self.removed.len() > MAX_REMOVED {
            if let Some((at, _)) = self.removed.pop_front() {
                self.known_from = self.known_from.max(at);
            }
        }
        // What showed with them changes too: a pin of one says nothing now, and a message
        // a pin taken out was about may no longer show pinned.
        self.pins_stamp = now;
        let targets: HashSet<&ItemId> = out.iter().filter_map(Item::pin_target).collect();
        for (item, at) in self.items.iter().zip(&mut self.stamps) {
            if item.pin_target().is_some_and(|t| gone.contains(t)) || targets.contains(&item.id) {
                *at = now;
            }
        }
        out
    }

    /// Remembers the friend's items `ids` as removed here, so one sent again is not kept
    /// again; past [`MAX_FORGOTTEN`], the oldest are let go.
    pub fn forget(&mut self, ids: impl IntoIterator<Item = ItemId>) {
        for id in ids {
            if self.forgotten_ids.insert(id) {
                self.forgotten.push_back(id);
            }
        }
        while self.forgotten.len() > MAX_FORGOTTEN {
            if let Some(old) = self.forgotten.pop_front() {
                self.forgotten_ids.remove(&old);
            }
        }
    }

    /// Whether the friend's item `id` was here and was removed.
    pub fn forgot(&self, id: &ItemId) -> bool {
        self.forgotten_ids.contains(id)
    }

    /// The friend's items removed here, oldest first.
    pub fn forgotten(&self) -> impl Iterator<Item = &ItemId> {
        self.forgotten.iter()
    }

    /// What the friend's items here take, all together ([`Item::weight`]).
    pub fn theirs(&self) -> u64 {
        self.theirs
    }

    /// The disappearing timer now: the last change by its author's time (ties by id), the
    /// same on both devices once both have every change. Off when it never changed.
    pub fn timer(&self) -> u64 {
        self.last_timer().map_or(0, |(_, ms)| ms)
    }

    /// When the timer last changed, and to what.
    pub fn last_timer(&self) -> Option<(u64, u64)> {
        self.items
            .iter()
            .filter_map(|i| match i.body {
                Body::Timer { ms } => Some(((i.sent_ms, i.id), ms)),
                Body::Message { .. }
                | Body::Pin { .. }
                | Body::Unsend { .. }
                | Body::Call { .. } => None,
            })
            .max_by_key(|(key, _)| *key)
            .map(|((sent, _), ms)| (sent, ms))
    }

    /// The time for a change of the timer made now on this device: after every change it
    /// knows, even when this device's clock is behind the friend's.
    pub fn timer_time(&self, now_ms: u64) -> u64 {
        match self.last_timer() {
            Some((sent, _)) if sent >= now_ms => sent.saturating_add(1),
            _ => now_ms,
        }
    }

    /// The last pin or unpin of each message, by its author's time (ties by id): the same
    /// on both devices once both have every change. The chat keeps no other.
    fn last_pins(&self) -> HashMap<ItemId, ((u64, ItemId), bool)> {
        let mut last: HashMap<ItemId, ((u64, ItemId), bool)> = HashMap::new();
        for item in &self.items {
            if let Body::Pin { target, pinned } = item.body {
                let key = (item.sent_ms, item.id);
                last.entry(target)
                    .and_modify(|old| {
                        if key > old.0 {
                            *old = (key, pinned);
                        }
                    })
                    .or_insert((key, pinned));
            }
        }
        last
    }

    /// The messages pinned now, the most recently pinned first, at most [`MAX_PINS`].
    pub fn pinned(&self) -> Vec<ItemId> {
        let messages: HashSet<&ItemId> = self
            .items
            .iter()
            .filter(|i| i.is_message())
            .map(|i| &i.id)
            .collect();
        let mut pinned: Vec<((u64, ItemId), ItemId)> = self
            .last_pins()
            .into_iter()
            .filter(|(target, (_, on))| *on && messages.contains(target))
            .map(|(target, (key, _))| (key, target))
            .collect();
        pinned.sort_unstable_by(|a, b| b.cmp(a));
        pinned.truncate(MAX_PINS);
        pinned.into_iter().map(|(_, target)| target).collect()
    }

    /// The time for a pin or unpin of `target` made now on this device: after every change
    /// of that message it knows, even when this device's clock is behind the friend's.
    pub fn pin_time(&self, target: &ItemId, now_ms: u64) -> u64 {
        match self.last_pins().get(target) {
            Some(((sent, _), _)) if *sent >= now_ms => sent.saturating_add(1),
            _ => now_ms,
        }
    }

    /// Messages from the friend not read yet.
    pub fn unread(&self) -> usize {
        self.items
            .iter()
            .filter(|i| !i.mine && i.seq > self.read_seq && matches!(i.body, Body::Message { .. }))
            .count()
    }

    /// The newest item's `seq`, for marking everything read.
    pub fn last_seq(&self) -> u64 {
        self.items.iter().map(|i| i.seq).max().unwrap_or(0)
    }

    /// This device's own items the friend has not received, oldest first.
    pub fn waiting(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|i| i.mine && !i.delivered)
    }

    /// Items whose time ran out at `now_ms`: messages not pinned, and the pins of messages
    /// that go or are gone. This device's own pin or unpin stays until it reached the
    /// friend, so an unpin that lets its message go still gets there.
    pub fn expired(&self, now_ms: u64) -> Vec<ItemId> {
        let pinned: HashSet<ItemId> = self.pinned().into_iter().collect();
        let gone: HashSet<ItemId> = self
            .items
            .iter()
            .filter(|i| !pinned.contains(&i.id) && i.expires_ms().is_some_and(|at| at <= now_ms))
            .map(|i| i.id)
            .collect();
        self.items
            .iter()
            .filter(|i| match i.pin_target() {
                Some(target) => {
                    (gone.contains(target) || !self.contains(target)) && (!i.mine || i.delivered)
                }
                None => gone.contains(&i.id),
            })
            .map(|i| i.id)
            .collect()
    }

    /// The earliest time [`Self::expired`] gives something: when the first item not pinned
    /// runs out, or at once (0) for a pin whose message is gone that may go.
    pub fn due_ms(&self) -> Option<u64> {
        let orphan = self
            .items
            .iter()
            .any(|i| i.pin_target().is_some_and(|t| !self.contains(t)) && (!i.mine || i.delivered));
        if orphan {
            return Some(0);
        }
        self.next_expiry()
    }

    /// The earliest time an item here disappears; a pinned message does not.
    pub fn next_expiry(&self) -> Option<u64> {
        let pinned: HashSet<ItemId> = self.pinned().into_iter().collect();
        self.items
            .iter()
            .filter(|i| !pinned.contains(&i.id))
            .filter_map(Item::expires_ms)
            .min()
    }

    /// What the attachments kept here take, by kind; a file in several messages counts once.
    pub fn usage(&self) -> Usage {
        let mut seen = HashSet::new();
        let mut usage = Usage::default();
        for file in self.items.iter().flat_map(Item::files) {
            if file.local != Local::Here || !seen.insert(file.id) {
                continue;
            }
            let slot = match file.kind {
                FileKind::Picture => &mut usage.pictures,
                FileKind::Video => &mut usage.videos,
                FileKind::File => &mut usage.files,
            };
            *slot = slot.saturating_add(file.size);
        }
        usage
    }

    /// Sets where every attachment with file id `file` stands; returns the items changed.
    pub fn set_local(&mut self, file: &FileId, local: Local) -> Vec<ItemId> {
        let mut changed = Vec::new();
        let now = stamp();
        for (item, at) in self.items.iter_mut().zip(&mut self.stamps) {
            let mut touched = false;
            for f in item.files_mut() {
                if f.id == *file && f.local != local {
                    f.local = local;
                    touched = true;
                }
            }
            if touched {
                *at = now;
                changed.push(item.id);
            }
        }
        changed
    }

    fn charges(&self, pool: Pool) -> &Vec<Charge> {
        match pool {
            Pool::Voice => &self.voice_charges,
            Pool::Files => &self.file_charges,
        }
    }

    fn charges_mut(&mut self, pool: Pool) -> &mut Vec<Charge> {
        match pool {
            Pool::Voice => &mut self.voice_charges,
            Pool::Files => &mut self.file_charges,
        }
    }

    /// Notes an attachment of `bytes` that started to come by itself at `at_ms`, unless its
    /// file was noted in the day before; charges more than a day before go. Whether it was
    /// noted now.
    pub fn charge(&mut self, pool: Pool, charge: Charge) -> bool {
        if charge
            .file
            .is_some_and(|file| self.charged_for(pool, &file, charge.at_ms))
        {
            return false;
        }
        let charges = self.charges_mut(pool);
        charges.retain(|c| c.at_ms.saturating_add(DAY_MS) > charge.at_ms);
        charges.push(charge);
        true
    }

    /// Takes back the charge for `file` (its fetch never started); whether there was one.
    pub fn refund(&mut self, pool: Pool, file: &FileId) -> bool {
        let charges = self.charges_mut(pool);
        let before = charges.len();
        charges.retain(|c| c.file.as_ref() != Some(file));
        charges.len() != before
    }

    /// Whether `file` was charged to `pool` in the day before `now`.
    pub fn charged_for(&self, pool: Pool, file: &FileId, now: u64) -> bool {
        self.charges(pool)
            .iter()
            .any(|c| c.file.as_ref() == Some(file) && c.at_ms.saturating_add(DAY_MS) > now)
    }

    /// Bytes charged to `pool` in the day before `now`.
    pub fn charged(&self, pool: Pool, now: u64) -> u64 {
        self.charges(pool)
            .iter()
            .filter(|c| c.at_ms.saturating_add(DAY_MS) > now)
            .map(|c| c.bytes)
            .fold(0, u64::saturating_add)
    }

    /// Bytes of voice messages fetched by themselves in the day before `now`.
    pub fn voice_charged(&self, now: u64) -> u64 {
        self.charged(Pool::Voice, now)
    }

    /// Drops the charges more than a day before `now`; whether one went.
    pub fn prune_charges(&mut self, now: u64) -> bool {
        let mut gone = false;
        for pool in [Pool::Voice, Pool::Files] {
            let charges = self.charges_mut(pool);
            let before = charges.len();
            charges.retain(|c| c.at_ms.saturating_add(DAY_MS) > now);
            gone |= charges.len() != before;
        }
        gone
    }

    /// The oldest charge kept, of either pool.
    pub fn oldest_charge(&self) -> Option<u64> {
        self.voice_charges
            .iter()
            .chain(&self.file_charges)
            .map(|c| c.at_ms)
            .min()
    }

    /// Marks the voice messages with this file id as played; whether one was not yet.
    pub fn set_played(&mut self, file: &FileId) -> bool {
        let mut changed = false;
        let now = stamp();
        for (item, at) in self.items.iter_mut().zip(&mut self.stamps) {
            for f in item.files_mut() {
                if f.id == *file
                    && let Some(voice) = f.voice.as_mut()
                    && !voice.played
                {
                    voice.played = true;
                    *at = now;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Takes away what makes the attachments with this file id voice messages: what came
    /// was not one. They show as the files they are. Whether one was.
    pub fn drop_voice(&mut self, file: &FileId) -> bool {
        let mut changed = false;
        let now = stamp();
        for (item, at) in self.items.iter_mut().zip(&mut self.stamps) {
            for f in item.files_mut() {
                if f.id == *file && f.voice.take().is_some() {
                    *at = now;
                    changed = true;
                }
            }
        }
        changed
    }

    /// Makes the attachments with this file id that show as pictures files: what came was
    /// not one. Whether one was.
    pub fn drop_picture(&mut self, file: &FileId) -> bool {
        let mut changed = false;
        let now = stamp();
        for (item, at) in self.items.iter_mut().zip(&mut self.stamps) {
            for f in item.files_mut() {
                if f.id == *file && f.kind == FileKind::Picture {
                    f.kind = FileKind::File;
                    *at = now;
                    changed = true;
                }
            }
        }
        changed
    }

    /// The attachments with file id `file`, each once by name.
    pub fn copies_of(&self, file: &FileId) -> Vec<Attachment> {
        let mut names = HashSet::new();
        self.items
            .iter()
            .flat_map(Item::files)
            .filter(|f| f.id == *file && names.insert(f.name.clone()))
            .cloned()
            .collect()
    }

    /// The names the attachments with file id `file` go by.
    pub fn names_of(&self, file: &FileId) -> Vec<String> {
        self.copies_of(file).into_iter().map(|f| f.name).collect()
    }

    /// An attachment with file id `file` kept here.
    pub fn kept(&self, file: &FileId) -> Option<&Attachment> {
        self.items
            .iter()
            .flat_map(Item::files)
            .find(|f| f.id == *file && f.local == Local::Here)
    }

    /// Whether an item still here refers to file `file`, and keeps it here.
    pub fn keeps(&self, file: &FileId) -> bool {
        self.items
            .iter()
            .flat_map(Item::files)
            .any(|f| f.id == *file && f.local == Local::Here)
    }

    /// Whether any item still refers to file `file`, wherever it stands.
    pub fn refers_to(&self, file: &FileId) -> bool {
        self.items
            .iter()
            .flat_map(Item::files)
            .any(|f| f.id == *file)
    }
}

/// The attachments to remove, oldest first, so that what the chats keep here fits in
/// `limit`: each as its chat and file. Each chat comes with what its journal takes, its
/// messages, which never go. A file goes whole, from every message of its chat that has it,
/// and counts from its oldest message. `keep` (a file that just arrived, or one being sent)
/// is never chosen.
///
/// Attachments past the limit on their own go from any chat, files of pinned messages last,
/// only when the others are not enough. With `only`, they go from that chat alone, and `None`
/// comes back when they are not enough: what a friend sends by itself makes room in its own
/// chat, never in the others'.
///
/// What the messages put past the limit then is made up only by the chats that take more
/// than their share of it (the limit by the number of chats), each from its own files, by
/// what it is over and what its messages take at most, and never from a pinned message's
/// file nor one this device sent: a friend's messages never push out what other chats
/// keep, nor what the person pinned or sent. Messages that take the room this way stay past
/// it, and files stop coming by themselves until there is room again.
pub fn to_free<K: Clone + Eq + std::hash::Hash>(
    chats: &[(K, &Chat, u64)],
    limit: u64,
    keep: Option<(&K, &FileId)>,
    only: Option<&K>,
) -> Option<Vec<(K, FileId)>> {
    let attachments: u64 = chats
        .iter()
        .map(|(_, chat, _)| chat.usage().attachments())
        .fold(0, u64::saturating_add);
    let journals: u64 = chats
        .iter()
        .map(|(_, _, journal)| *journal)
        .fold(0, u64::saturating_add);
    if attachments.saturating_add(journals) <= limit {
        return Some(Vec::new());
    }
    // Each file here once per chat, at the time of its oldest message, and whether a
    // pinned message of the chat has it, or a message this device sent.
    let mut files: HashMap<(K, FileId), (u64, ItemId, u64)> = HashMap::new();
    let mut pinned_files: HashSet<(K, FileId)> = HashSet::new();
    let mut own_files: HashSet<(K, FileId)> = HashSet::new();
    for (key, chat, _) in chats {
        if only.is_some_and(|k| k != key) {
            continue;
        }
        let pinned: HashSet<ItemId> = chat.pinned().into_iter().collect();
        for item in chat.items() {
            for file in item.files() {
                if pinned.contains(&item.id) {
                    pinned_files.insert((key.clone(), file.id));
                }
                if item.mine {
                    own_files.insert((key.clone(), file.id));
                }
            }
            for file in item.files().iter().filter(|f| f.local == Local::Here) {
                let at = (item.at_ms, item.id, file.size);
                files
                    .entry((key.clone(), file.id))
                    .and_modify(|old| {
                        if (at.0, at.1) < (old.0, old.1) {
                            *old = at;
                        }
                    })
                    .or_insert(at);
            }
        }
    }
    // In the order files go: pinned last, oldest first.
    type Order = (bool, u64, ItemId, FileId);
    let mut oldest: BTreeMap<Order, (K, u64)> = BTreeMap::new();
    for ((key, file), (at, item, size)) in files {
        if keep.is_some_and(|(k, f)| *k == key && *f == file) {
            continue;
        }
        let pinned = pinned_files.contains(&(key.clone(), file));
        oldest.insert((pinned, at, item, file), (key, size));
    }
    let mut out = Vec::new();
    let mut freed: HashMap<K, u64> = HashMap::new();
    let mut over = attachments.saturating_sub(limit);
    for ((_, _, _, file), (key, size)) in &oldest {
        if over == 0 {
            break;
        }
        over = over.saturating_sub(*size);
        *freed.entry(key.clone()).or_default() += size;
        out.push((key.clone(), *file));
    }
    if over > 0 && only.is_some() {
        return None;
    }
    // What the messages put past the limit, from the chats over their share.
    let left = attachments.saturating_sub(freed.values().sum());
    let mut over = left.saturating_add(journals).saturating_sub(limit);
    if over == 0 {
        return Some(out);
    }
    let share = limit / chats.len().max(1) as u64;
    let mut taken: HashSet<(K, FileId)> = out.iter().cloned().collect();
    // The chat furthest past its share first.
    let mut past: Vec<(u64, &K)> = chats
        .iter()
        .filter(|(key, _, _)| only.is_none_or(|k| k == key))
        .map(|(key, chat, journal)| {
            let takes = chat
                .usage()
                .attachments()
                .saturating_sub(freed.get(key).copied().unwrap_or(0))
                .saturating_add(*journal);
            (takes.saturating_sub(share).min(*journal), key)
        })
        .filter(|(past, _)| *past > 0)
        .collect();
    past.sort_unstable_by_key(|(past, _)| std::cmp::Reverse(*past));
    for (past, key) in past {
        let mut past = past.min(over);
        for ((pinned, _, _, file), (k, size)) in &oldest {
            if past == 0 {
                break;
            }
            let pair = (k.clone(), *file);
            if k != key || *pinned || own_files.contains(&pair) || taken.contains(&pair) {
                continue;
            }
            past = past.saturating_sub(*size);
            over = over.saturating_sub(*size);
            taken.insert(pair.clone());
            out.push(pair);
        }
        if over == 0 {
            break;
        }
    }
    Some(out)
}

/// How long a call lasted, in words: "a few seconds", "1 minute", "25 minutes", "1 hour",
/// "2 hours 5 minutes".
pub fn call_length(ms: u64) -> String {
    let minutes = ms / 60_000;
    if minutes == 0 {
        return "a few seconds".to_string();
    }
    let count = |n: u64, one: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {one}s")
        }
    };
    let (hours, minutes) = (minutes / 60, minutes % 60);
    match (hours, minutes) {
        (0, m) => count(m, "minute"),
        (h, 0) => count(h, "hour"),
        (h, m) => format!("{} {}", count(h, "hour"), count(m, "minute")),
    }
}

/// A voice message's length as it shows: `0:18`, `12:05`, in whole seconds as the
/// recording counted them.
pub fn voice_length(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The length of the voice message a chat's last line is, when it is one alone (the list
/// says "Voice message · 0:18").
pub fn voice_preview(item: &Item) -> Option<u64> {
    match &item.body {
        Body::Message { text, files } if text.trim().is_empty() && files.len() == 1 => {
            files[0].voice.as_ref().map(|v| v.duration_ms)
        }
        _ => None,
    }
}

/// What a message shows as the chat's last line, in the list of chats with `friend` (a
/// name).
pub fn preview(item: &Item, friend: &str, timer_words: impl Fn(u64) -> String) -> String {
    match &item.body {
        Body::Timer { ms } => timer_words(*ms),
        Body::Pin { pinned: true, .. } => "Pinned a message".to_string(),
        Body::Pin { pinned: false, .. } => "Unpinned a message".to_string(),
        Body::Unsend { .. } => "Deleted a message".to_string(),
        // The list puts "You: " before this device's own lines.
        Body::Call { lasted_ms, starter } => match (starter, item.mine) {
            (None, true) => format!("Started a call that lasted {}", call_length(*lasted_ms)),
            (starter, _) => format!(
                "{} started a call that lasted {}",
                starter.as_deref().unwrap_or(friend),
                call_length(*lasted_ms)
            ),
        },
        Body::Message { text, files } => {
            let first_line = text.lines().find(|l| !l.trim().is_empty());
            match (first_line, files.first()) {
                (Some(line), _) => line.trim().to_string(),
                (None, Some(file)) if files.len() == 1 && file.voice.is_some() => format!(
                    "Voice message · {}",
                    voice_length(file.voice.as_ref().map_or(0, |v| v.duration_ms))
                ),
                (None, Some(file)) if files.len() == 1 => file.name.clone(),
                (None, Some(_)) => format!("{} files", files.len()),
                (None, None) => String::new(),
            }
        }
    }
}

/// `[u8; N]` as lowercase hex text.
pub(crate) mod hex_array {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<[u8; N], D::Error> {
        let text = String::deserialize(d)?;
        hex::decode(text)
            .map_err(D::Error::custom)?
            .try_into()
            .map_err(|_| D::Error::custom("wrong length"))
    }
}

/// `Option<[u8; N]>` as lowercase hex text, or nothing.
pub(crate) mod hex_option {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &Option<[u8; N]>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => super::hex_array::serialize(bytes, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<Option<[u8; N]>, D::Error> {
        #[derive(Deserialize)]
        struct Hex<const M: usize>(#[serde(with = "super::hex_array")] [u8; M]);
        Ok(Option::<Hex<N>>::deserialize(d)?.map(|Hex(bytes)| bytes))
    }
}

/// A list of `[u8; N]` as lowercase hex texts.
pub(crate) mod hex_list {
    use serde::{Deserialize, Deserializer, Serializer, de::Error, ser::SerializeSeq};

    pub fn serialize<S: Serializer, const N: usize>(
        list: &[[u8; N]],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(list.len()))?;
        for bytes in list {
            seq.serialize_element(&hex::encode(bytes))?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        d: D,
    ) -> Result<Vec<[u8; N]>, D::Error> {
        Vec::<String>::deserialize(d)?
            .into_iter()
            .map(|text| {
                hex::decode(text)
                    .map_err(D::Error::custom)?
                    .try_into()
                    .map_err(|_| D::Error::custom("wrong length"))
            })
            .collect()
    }
}

/// Bytes as base64 text, so a picture stays compact in JSON.
mod base64_text {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: u8, mine: bool, at: u64, files: Vec<Attachment>) -> Item {
        Item {
            id: [id; 16],
            mine,
            sent_ms: at,
            got_ms: at,
            seq: u64::from(id),
            at_ms: at,
            body: Body::Message {
                text: format!("m{id}"),
                files,
            },
            ttl_ms: 0,
            delivered: false,
        }
    }

    /// The friend's message is gone from the friend's device once its time ran out from
    /// when the friend wrote it, however late it came here; this device's own, and one
    /// without a timer, never count so.
    #[test]
    fn a_message_is_gone_from_its_author_by_when_it_was_written() {
        let mut theirs = message(1, false, 10_000, Vec::new());
        theirs.got_ms = 50_000;
        theirs.ttl_ms = 60_000;
        assert!(!theirs.gone_from_its_author(69_999));
        assert!(theirs.gone_from_its_author(70_000));
        assert_eq!(theirs.expires_ms(), Some(110_000));
        let mut mine = theirs.clone();
        mine.mine = true;
        assert!(!mine.gone_from_its_author(200_000));
        theirs.ttl_ms = 0;
        assert!(!theirs.gone_from_its_author(u64::MAX));
    }

    /// A screen holding part of a chat learns from the chat what changed in it since it
    /// read it, from the place of the oldest item it holds on: what came, what changed and
    /// what went; and that it must read it whole when the chat no longer knows.
    #[test]
    fn a_chat_says_what_changed_since_a_view_of_it() {
        let ids = |items: Vec<&Item>| items.into_iter().map(|i| i.id[0]).collect::<Vec<_>>();
        let mut chat = Chat::default();
        for id in 1..=5 {
            chat.insert(message(
                id,
                true,
                u64::from(id),
                vec![file(id, 10, FileKind::Picture, Local::Missing)],
            ));
        }
        let read = version();
        assert!(chat.knows_since(read));
        let held = chat.place((3, [3; 16]));
        assert_eq!(held, 2);
        assert!(chat.changed_since(read, held).next().is_none());
        // Something new, one held delivered, one before what is held changed (not said),
        // a file coming and one item going.
        chat.insert(message(6, false, 6, Vec::new()));
        chat.get_mut(&[4; 16]).unwrap().delivered = true;
        chat.get_mut(&[1; 16]).unwrap().delivered = true;
        chat.touch_file(&[5; 32]);
        chat.remove(&[[3; 16]]);
        let held = chat.place((3, [3; 16]));
        assert_eq!(ids(chat.changed_since(read, held).collect()), [4, 5, 6]);
        assert_eq!(chat.removed_since(read).collect::<Vec<_>>(), [&[3; 16]]);
        assert!(!chat.pins_changed_since(version()));
        // A pin shows on its message, and its going too.
        let again = version();
        chat.insert(pin(7, true, 7, 5, true));
        assert!(chat.pins_changed_since(again));
        assert_eq!(ids(chat.changed_since(again, held).collect()), [5, 7]);
        let again = version();
        chat.remove(&[[5; 16]]);
        assert_eq!(ids(chat.changed_since(again, held).collect()), [7]);
        // Past the removals remembered, an older view is read again whole.
        let old = version();
        assert!(chat.knows_since(old));
        let many: Vec<ItemId> = (0..=MAX_REMOVED as u64)
            .map(|n| {
                let mut item = message(0, true, 100 + n, Vec::new());
                item.id = (u128::from(n) + 1000).to_be_bytes();
                item.seq = 100 + n;
                chat.insert(item.clone());
                item.id
            })
            .collect();
        chat.remove(&many);
        assert!(!chat.knows_since(old));
        assert!(chat.knows_since(version()));
        // So is a view of a chat made (or read) again after it.
        assert!(!Chat::default().knows_since(old));
    }

    fn file(id: u8, size: u64, kind: FileKind, local: Local) -> Attachment {
        Attachment {
            id: [id; 32],
            name: format!("f{id}"),
            size,
            kind,
            video: None,
            local,
            voice: None,
        }
    }

    fn timer(id: u8, sent: u64, ms: u64) -> Item {
        Item {
            id: [id; 16],
            mine: false,
            sent_ms: sent,
            got_ms: sent,
            seq: u64::from(id),
            at_ms: sent,
            body: Body::Timer { ms },
            ttl_ms: 0,
            delivered: true,
        }
    }

    /// The order and the shown time of a chat's items, by their ids' first byte.
    fn shown(chat: &Chat) -> Vec<(u8, u64)> {
        chat.items().iter().map(|i| (i.id[0], i.at_ms)).collect()
    }

    /// An item from the friend written at `sent` by its clock that reaches this device at
    /// `got`.
    fn came(id: u8, sent: u64, got: u64) -> Item {
        let mut item = message(id, false, sent, vec![]);
        item.got_ms = got;
        item
    }

    #[test]
    fn items_go_in_the_order_they_came_and_never_twice() {
        let mut chat = Chat::default();
        assert!(chat.add(message(1, true, 100, vec![])).is_some());
        assert!(chat.add(came(2, 150, 200)).is_some());
        assert!(chat.add(came(2, 150, 200)).is_none());
        assert_eq!(shown(&chat), [(1, 100), (2, 150)]);
        assert_eq!(chat.items()[1].seq, 2);
        assert_eq!(chat.next_seq(), 3);
        assert_eq!(chat.remove(&[[1; 16], [9; 16]]).len(), 1);
        assert!(!chat.contains(&[1; 16]));
    }

    #[test]
    fn a_friend_with_a_clock_behind_answers_below() {
        let mut chat = Chat::default();
        chat.add(message(1, true, 12_000, vec![]));
        // Written a moment after, by a clock an hour behind: under what it answers, shown
        // at the time of the line above, not an hour earlier.
        chat.add(came(2, 12_010 - 3_600, 12_010));
        chat.add(message(3, true, 12_020, vec![]));
        chat.add(came(4, 12_030 - 3_600, 12_030));
        assert_eq!(
            shown(&chat),
            [(1, 12_000), (2, 12_000), (3, 12_020), (4, 12_020)]
        );
    }

    #[test]
    fn a_friend_with_a_clock_ahead_shows_when_it_came() {
        let mut chat = Chat::default();
        chat.add(message(1, true, 12_000, vec![]));
        chat.add(came(2, 12_010 + 3_600, 12_010));
        chat.add(message(3, true, 12_020, vec![]));
        assert_eq!(shown(&chat), [(1, 12_000), (2, 12_010), (3, 12_020)]);
    }

    #[test]
    fn what_waited_comes_after_what_is_here_in_the_friends_order() {
        let mut chat = Chat::default();
        chat.add(came(1, 9_000, 9_000));
        // Written here while the friend was away, then the friend's, written meanwhile too,
        // all at once when the two meet: below, in the order the friend wrote them, shown
        // at their own time unless that is before the line above.
        chat.add(message(2, true, 10_500, vec![]));
        for (id, sent) in [(3, 10_000), (4, 10_600), (5, 10_700)] {
            chat.add(came(id, sent, 12_000));
        }
        assert_eq!(
            shown(&chat),
            [
                (1, 9_000),
                (2, 10_500),
                (3, 10_500),
                (4, 10_600),
                (5, 10_700)
            ]
        );
        // What comes later still goes at the end.
        chat.add(message(6, true, 12_100, vec![]));
        assert_eq!(chat.items().last().unwrap().id, [6; 16]);
    }

    #[test]
    fn both_writing_at_once_each_sees_its_own_first() {
        let mut here = Chat::default();
        let mut there = Chat::default();
        here.add(message(1, true, 5_000, vec![]));
        there.add(message(2, true, 5_000, vec![]));
        here.add(came(2, 5_000, 5_050));
        there.add(came(1, 5_000, 5_050));
        assert_eq!(shown(&here), [(1, 5_000), (2, 5_000)]);
        assert_eq!(shown(&there), [(2, 5_000), (1, 5_000)]);
    }

    #[test]
    fn a_chat_kept_before_its_times_were_reads_back_in_the_order_it_came() {
        // As an older journal holds them: by their author's time, with no shown time, and
        // the friend's reply from a clock behind written above what it answers.
        let mut chat = Chat::default();
        let mut reply = came(2, 11_000, 12_010);
        reply.at_ms = 0;
        let mut first = message(1, true, 12_000, vec![]);
        first.at_ms = 0;
        let mut later = message(3, true, 12_020, vec![]);
        later.at_ms = 0;
        for item in [reply, first, later] {
            chat.insert(item);
        }
        chat.settle();
        assert_eq!(shown(&chat), [(1, 12_000), (2, 12_000), (3, 12_020)]);
        assert_eq!(chat.next_seq(), 4);
    }

    #[test]
    fn the_timer_is_the_last_change_on_both_devices() {
        let mut here = Chat::default();
        let mut there = Chat::default();
        // Both change it at once; each gets the other's change later.
        here.insert(timer(1, 500, DAY_MS));
        there.insert(timer(2, 500, 7 * DAY_MS));
        here.insert(timer(2, 500, 7 * DAY_MS));
        there.insert(timer(1, 500, DAY_MS));
        assert_eq!(here.timer(), there.timer());
        assert_eq!(here.timer(), 7 * DAY_MS);
        // A change made here after it comes after what is known, even with a clock behind.
        assert_eq!(here.timer_time(400), 501);
        assert_eq!(here.timer_time(900), 900);
        assert_eq!(Chat::default().timer(), 0);
        // A friend's time at the very end of the clock does not overflow the next one.
        here.insert(timer(3, u64::MAX, 0));
        assert_eq!(here.timer_time(900), u64::MAX);
    }

    #[test]
    fn messages_disappear_counting_from_when_they_arrived() {
        let mut chat = Chat::default();
        let mut sent = message(1, true, 1_000, vec![]);
        sent.ttl_ms = DAY_MS;
        let mut late = message(2, false, 1_000, vec![]);
        late.ttl_ms = DAY_MS;
        late.got_ms = 5_000;
        chat.insert(sent);
        chat.insert(late);
        chat.insert(timer(3, 1_000, DAY_MS));
        assert!(chat.expired(1_000 + DAY_MS - 1).is_empty());
        assert_eq!(chat.expired(1_000 + DAY_MS), vec![[1; 16]]);
        assert_eq!(chat.next_expiry(), Some(1_000 + DAY_MS));
        // A change of the timer never disappears.
        assert_eq!(chat.expired(u64::MAX).len(), 2);
    }

    fn pin(id: u8, mine: bool, sent: u64, target: u8, pinned: bool) -> Item {
        Item {
            id: [id; 16],
            mine,
            sent_ms: sent,
            got_ms: sent,
            seq: u64::from(id),
            at_ms: sent,
            body: Body::Pin {
                target: [target; 16],
                pinned,
            },
            ttl_ms: 0,
            delivered: !mine,
        }
    }

    #[test]
    fn a_pinned_message_stays_until_it_is_unpinned() {
        let mut chat = Chat::default();
        let mut kept = message(1, false, 1_000, vec![]);
        kept.ttl_ms = DAY_MS;
        let mut other = message(2, false, 1_000, vec![]);
        other.ttl_ms = DAY_MS;
        chat.insert(kept);
        chat.insert(other);
        chat.insert(pin(3, false, 2_000, 1, true));
        assert_eq!(chat.pinned(), vec![[1; 16]]);
        let later = 1_000 + DAY_MS + 5;
        assert_eq!(chat.expired(later), vec![[2; 16]]);
        assert_eq!(chat.next_expiry(), Some(1_000 + DAY_MS));
        chat.remove(&[[2; 16]]);
        assert!(chat.expired(later).is_empty());
        assert_eq!(chat.next_expiry(), None);
        // Unpinned here, and its time has passed: it goes at once (the unpin took the pin's
        // place), but this device's unpin waits until it reached the friend.
        chat.insert(pin(4, true, chat.pin_time(&[1; 16], 3_000), 1, false));
        assert!(chat.pinned().is_empty());
        assert!(!chat.contains(&[3; 16]));
        let gone = chat.expired(later);
        assert_eq!(gone, vec![[1; 16]]);
        chat.remove(&gone);
        assert!(chat.expired(later).is_empty());
        chat.get_mut(&[4; 16]).unwrap().delivered = true;
        assert_eq!(chat.expired(later), vec![[4; 16]]);
        // A change of the timer is never pinned.
        chat.insert(timer(5, 10, DAY_MS));
        chat.insert(pin(6, false, 20, 5, true));
        assert!(chat.pinned().is_empty());
    }

    #[test]
    fn the_last_pin_or_unpin_counts_on_both_devices() {
        let mut here = Chat::default();
        let mut there = Chat::default();
        for chat in [&mut here, &mut there] {
            chat.insert(message(1, false, 100, vec![]));
        }
        // Both change it at once; each gets the other's change later.
        here.insert(pin(2, true, 500, 1, true));
        there.insert(pin(3, true, 500, 1, false));
        here.insert(pin(3, false, 500, 1, false));
        there.insert(pin(2, false, 500, 1, true));
        assert_eq!(here.pinned(), there.pinned());
        assert!(here.pinned().is_empty());
        // A change made here comes after what is known, even with a clock behind.
        assert_eq!(here.pin_time(&[1; 16], 400), 501);
        assert_eq!(here.pin_time(&[1; 16], 900), 900);
        assert_eq!(here.pin_time(&[9; 16], 400), 400);
        here.insert(pin(4, false, u64::MAX, 1, true));
        assert_eq!(here.pin_time(&[1; 16], 400), u64::MAX);
        // Pins are neither unread nor the timer.
        assert_eq!(here.unread(), 1);
        assert_eq!(here.timer(), 0);
    }

    #[test]
    fn only_the_last_pin_or_unpin_of_a_message_is_kept() {
        let mut chat = Chat::default();
        chat.insert(message(1, false, 100, vec![]));
        chat.insert(message(2, false, 100, vec![]));
        chat.insert(pin(3, false, 1_000, 2, true));
        // Pinned and unpinned again and again: one change of the message stays.
        for n in 0..20u8 {
            let at = chat.pin_time(&[1; 16], 1_000 + u64::from(n));
            assert!(chat.insert(pin(10 + n, true, at, 1, n % 2 == 0)));
        }
        let pins: Vec<ItemId> = chat
            .items()
            .iter()
            .filter(|i| i.pin_target() == Some(&[1; 16]))
            .map(|i| i.id)
            .collect();
        assert_eq!(pins, [[29; 16]]);
        assert_eq!(chat.pinned(), [[2; 16]]);
        // An older change sent again, or read back from an older journal, loses to it and
        // is not kept; the other message's pin is untouched.
        assert!(!chat.insert(pin(5, false, 1_005, 1, true)));
        assert!(!chat.contains(&[5; 16]));
        assert_eq!(chat.pinned(), [[2; 16]]);
        assert!(chat.contains(&[3; 16]));
        // In whatever order they are read, the same one stays.
        let mut again = Chat::default();
        again.insert(pin(7, false, 2_000, 1, true));
        again.insert(pin(6, false, 1_500, 1, false));
        assert_eq!(again.items().len(), 1);
        assert!(again.contains(&[7; 16]));
    }

    #[test]
    fn past_the_most_pins_the_most_recent_count() {
        let mut chat = Chat::default();
        let count = MAX_PINS as u8 + 2;
        for n in 0..count {
            chat.insert(message(n, false, 10, vec![]));
            chat.insert(pin(100 + n, false, 1_000 + u64::from(n), n, true));
        }
        let pinned = chat.pinned();
        assert_eq!(pinned.len(), MAX_PINS);
        assert_eq!(pinned[0], [count - 1; 16]);
        assert!(!pinned.contains(&[0; 16]) && !pinned.contains(&[1; 16]));
    }

    #[test]
    fn unread_counts_the_friend_messages_after_the_last_read() {
        let mut chat = Chat::default();
        chat.insert(message(1, false, 1, vec![]));
        chat.insert(message(2, true, 2, vec![]));
        chat.insert(message(3, false, 3, vec![]));
        chat.insert(timer(4, 4, DAY_MS));
        assert_eq!(chat.unread(), 2);
        chat.read_seq = 1;
        assert_eq!(chat.unread(), 1);
        chat.read_seq = chat.last_seq();
        assert_eq!(chat.unread(), 0);
    }

    #[test]
    fn usage_counts_each_file_kept_here_once() {
        let mut chat = Chat::default();
        chat.insert(message(
            1,
            false,
            1,
            vec![file(1, 100, FileKind::Picture, Local::Here)],
        ));
        chat.insert(message(
            2,
            false,
            2,
            vec![file(1, 100, FileKind::Picture, Local::Here)],
        ));
        chat.insert(message(
            3,
            true,
            3,
            vec![
                file(2, 1_000, FileKind::Video, Local::Here),
                file(3, 50, FileKind::File, Local::Missing),
                file(4, 70, FileKind::File, Local::Removed),
            ],
        ));
        let usage = chat.usage();
        assert_eq!((usage.pictures, usage.videos, usage.files), (100, 1_000, 0));
        assert_eq!(usage.attachments(), 1_100);
        assert_eq!(chat.set_local(&[1; 32], Local::Removed).len(), 2);
        assert!(!chat.keeps(&[1; 32]));
        assert!(chat.refers_to(&[1; 32]));
    }

    #[test]
    fn freeing_space_takes_the_oldest_attachments_first() {
        let mut ana = Chat::default();
        ana.insert(message(
            1,
            false,
            10,
            vec![file(1, 400, FileKind::Picture, Local::Here)],
        ));
        ana.insert(message(
            3,
            false,
            30,
            vec![file(3, 400, FileKind::Video, Local::Here)],
        ));
        let mut leo = Chat::default();
        leo.insert(message(
            2,
            true,
            20,
            vec![file(2, 400, FileKind::File, Local::Here)],
        ));
        // A file sent again later counts from its first message.
        leo.insert(message(
            4,
            true,
            40,
            vec![file(2, 400, FileKind::File, Local::Here)],
        ));
        let chats = [("ana", &ana, 0), ("leo", &leo, 0)];
        assert!(to_free(&chats, 1_200, None, None).unwrap().is_empty());
        assert_eq!(
            to_free(&chats, 1_000, None, None).unwrap(),
            vec![("ana", [1; 32])]
        );
        assert_eq!(
            to_free(&chats, 500, None, None).unwrap(),
            vec![("ana", [1; 32]), ("leo", [2; 32])]
        );
        // What just arrived stays, even when it is the oldest.
        assert_eq!(
            to_free(&chats, 1_000, Some((&"ana", &[1; 32])), None).unwrap(),
            vec![("leo", [2; 32])]
        );
        assert_eq!(to_free(&chats, 0, None, None).unwrap().len(), 3);
        // A pinned message's file goes after the others, even when it is the oldest.
        ana.insert(pin(9, true, 50, 1, true));
        let chats = [("ana", &ana, 0), ("leo", &leo, 0)];
        assert_eq!(
            to_free(&chats, 1_000, None, None).unwrap(),
            vec![("leo", [2; 32])]
        );
        assert_eq!(
            to_free(&chats, 0, None, None).unwrap(),
            vec![("leo", [2; 32]), ("ana", [3; 32]), ("ana", [1; 32])]
        );
    }

    /// The messages count against the room and never go; what they put past it comes out of
    /// the chats that take more than their share of the room, from the files their friend
    /// sent that are not pinned, never from the others'.
    #[test]
    fn messages_make_room_only_in_their_own_chat() {
        let mut ana = Chat::default();
        for (id, mine, at) in [
            (1, false, 10),
            (3, false, 30),
            (5, true, 50),
            (6, false, 60),
        ] {
            ana.insert(message(
                id,
                mine,
                at,
                vec![file(id, 400, FileKind::Picture, Local::Here)],
            ));
        }
        ana.insert(pin(9, true, 70, 6, true));
        let mut leo = Chat::default();
        leo.insert(message(
            2,
            false,
            20,
            vec![file(2, 400, FileKind::File, Local::Here)],
        ));
        // Ana's messages take 200 past the room (2,000 for the two): only her chat gives.
        let chats = [("ana", &ana, 200), ("leo", &leo, 0)];
        assert_eq!(
            to_free(&chats, 2_000, None, None).unwrap(),
            vec![("ana", [1; 32])]
        );
        // However much her messages take, Leo's file stays, and so do the one sent to her
        // and her pinned one: what her chat keeps that she sent goes, oldest first.
        let chats = [("ana", &ana, 100_000), ("leo", &leo, 0)];
        assert_eq!(
            to_free(&chats, 2_000, None, None).unwrap(),
            vec![("ana", [1; 32]), ("ana", [3; 32])]
        );
        // Leo's messages take more than his share: his file goes, none of Ana's, though her
        // files alone take more than hers.
        let chats = [("ana", &ana, 0), ("leo", &leo, 100_000)];
        assert_eq!(
            to_free(&chats, 2_000, None, None).unwrap(),
            vec![("leo", [2; 32])]
        );
        // A file that came by itself does the same in its own chat only.
        assert_eq!(
            to_free(&chats, 2_000, Some((&"leo", &[2; 32])), Some(&"leo")),
            Some(Vec::new())
        );
        // Attachments past the room on their own still go from any chat, oldest first.
        let chats = [("ana", &ana, 0), ("leo", &leo, 100_000)];
        assert_eq!(
            to_free(&chats, 1_500, None, None).unwrap(),
            vec![("ana", [1; 32]), ("leo", [2; 32])]
        );
    }

    #[test]
    fn room_for_what_a_friend_sends_by_itself_comes_from_its_own_chat_only() {
        let mut ana = Chat::default();
        ana.insert(message(
            1,
            true,
            10,
            vec![file(1, 400, FileKind::Picture, Local::Here)],
        ));
        let mut leo = Chat::default();
        leo.insert(message(
            2,
            false,
            20,
            vec![file(2, 400, FileKind::File, Local::Here)],
        ));
        leo.insert(message(
            3,
            false,
            30,
            vec![file(3, 400, FileKind::File, Local::Here)],
        ));
        // Leo's newest just came: his older one goes, not Ana's older picture.
        let chats = [("ana", &ana, 0), ("leo", &leo, 0)];
        assert_eq!(
            to_free(&chats, 800, Some((&"leo", &[3; 32])), Some(&"leo")),
            Some(vec![("leo", [2; 32])])
        );
        // When his chat is not enough, nothing of the others' goes: what came does not fit.
        assert_eq!(
            to_free(&chats, 300, Some((&"leo", &[3; 32])), Some(&"leo")),
            None
        );
        // His pinned files go too, last.
        leo.insert(pin(9, true, 40, 2, true));
        leo.insert(message(
            4,
            false,
            35,
            vec![file(4, 100, FileKind::File, Local::Here)],
        ));
        let chats = [("ana", &ana, 0), ("leo", &leo, 0)];
        assert_eq!(
            to_free(&chats, 800, Some((&"leo", &[3; 32])), Some(&"leo")),
            Some(vec![("leo", [4; 32]), ("leo", [2; 32])])
        );
    }

    #[test]
    fn a_voice_message_that_is_not_one_becomes_its_file() {
        let mut chat = Chat::default();
        let mut f = file(5, 100, FileKind::File, Local::Missing);
        f.voice = Some(VoiceInfo {
            duration_ms: 1000,
            bars: vec![1; 40],
            played: false,
        });
        chat.insert(message(1, false, 10, vec![f]));
        assert!(chat.drop_voice(&[5; 32]));
        assert!(!chat.drop_voice(&[5; 32]));
        assert!(chat.items()[0].files()[0].voice.is_none());
    }

    #[test]
    fn previews_say_the_first_line_or_the_file() {
        let words = |ms: u64| format!("timer {ms}");
        let mut item = message(1, true, 1, vec![]);
        item.body = Body::Message {
            text: "\n  hi  \nhow are you".into(),
            files: vec![],
        };
        assert_eq!(preview(&item, "Bea", words), "hi");
        item.body = Body::Message {
            text: String::new(),
            files: vec![file(1, 1, FileKind::Video, Local::Missing)],
        };
        assert_eq!(preview(&item, "Bea", words), "f1");
        assert_eq!(
            preview(&timer(2, 1, DAY_MS), "Bea", words),
            format!("timer {DAY_MS}")
        );
        assert_eq!(
            preview(&pin(3, true, 1, 1, true), "Bea", words),
            "Pinned a message"
        );
    }

    #[test]
    fn items_keep_their_form_in_json() {
        let mut item = message(7, false, 5, vec![file(9, 12, FileKind::Video, Local::Here)]);
        item.files_mut()[0].video = Some(VideoInfo {
            thumb: vec![1, 2, 3],
            width: 640,
            height: 360,
            duration_ms: 72_000,
        });
        let text = serde_json::to_string(&item).unwrap();
        assert!(text.contains(&hex::encode([7u8; 16])));
        assert_eq!(serde_json::from_str::<Item>(&text).unwrap(), item);
        assert!(valid_timer(0) && valid_timer(7 * DAY_MS) && !valid_timer(5));
        let pin = pin(8, true, 9, 7, true);
        let text = serde_json::to_string(&pin).unwrap();
        assert!(text.contains(&hex::encode([7u8; 16])));
        assert_eq!(serde_json::from_str::<Item>(&text).unwrap(), pin);
        let mut call = message(9, true, 20, vec![]);
        call.body = Body::Call {
            lasted_ms: 90_000,
            starter: None,
        };
        let text = serde_json::to_string(&call).unwrap();
        assert!(!text.contains("starter"));
        assert_eq!(serde_json::from_str::<Item>(&text).unwrap(), call);
    }

    #[test]
    fn removed_items_are_remembered_up_to_a_bound() {
        let mut chat = Chat::default();
        let id = |n: usize| -> ItemId {
            let mut id = [0; 16];
            id[..8].copy_from_slice(&(n as u64).to_le_bytes());
            id
        };
        chat.forget((0..MAX_FORGOTTEN).map(id));
        chat.forget([id(0)]);
        assert!(chat.forgot(&id(0)), "the same id is remembered once");
        chat.forget([id(MAX_FORGOTTEN)]);
        assert_eq!(chat.forgotten().count(), MAX_FORGOTTEN);
        assert!(!chat.forgot(&id(0)), "the oldest is let go");
        assert!(chat.forgot(&id(1)) && chat.forgot(&id(MAX_FORGOTTEN)));
    }

    #[test]
    fn a_call_says_how_long_it_lasted_in_words() {
        let words = |ms: u64| call_length(ms);
        assert_eq!(words(0), "a few seconds");
        assert_eq!(words(59_999), "a few seconds");
        assert_eq!(words(60_000), "1 minute");
        assert_eq!(words(119_999), "1 minute");
        assert_eq!(words(25 * 60_000 + 30_000), "25 minutes");
        assert_eq!(words(3_600_000), "1 hour");
        assert_eq!(words(3_600_000 + 60_000), "1 hour 1 minute");
        assert_eq!(words(80 * 60_000), "1 hour 20 minutes");
        assert_eq!(words(2 * 3_600_000 + 59_000), "2 hours");
        assert_eq!(words(26 * 3_600_000 + 5 * 60_000), "26 hours 5 minutes");

        let mut call = message(1, true, 5, vec![]);
        call.body = Body::Call {
            lasted_ms: 300_000,
            starter: None,
        };
        // The list of chats puts "You: " before this device's own lines.
        assert_eq!(
            preview(&call, "Bea", |_| String::new()),
            "Started a call that lasted 5 minutes"
        );
        call.mine = false;
        assert_eq!(
            preview(&call, "Bea", |_| String::new()),
            "Bea started a call that lasted 5 minutes"
        );
        call.body = Body::Call {
            lasted_ms: 3_000,
            starter: Some("Sofía".into()),
        };
        assert_eq!(
            preview(&call, "Bea", |_| String::new()),
            "Sofía started a call that lasted a few seconds"
        );
        // Never unread, never sent, and no files.
        let mut chat = Chat::default();
        chat.add(call.clone()).unwrap();
        assert_eq!(chat.unread(), 0);
        assert!(call.files().is_empty() && !call.is_message());
    }
}
