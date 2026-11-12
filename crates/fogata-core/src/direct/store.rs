//! Direct messages on disk, in `direct/` of the data folder: a folder per friend, named
//! after its device key in hex, with the chat's journal, the log of the chat's two-person
//! group, the friend's profile as last known (the chat keeps its name and picture after the
//! two stop being friends) and the attachments kept here, under `files/` by their id. The
//! settings are in `direct/settings.json`.
//!
//! The journal is a list of records, each a length and a JSON object, added at the end as
//! things happen: an item, items delivered or read, an attachment fetched or removed, items
//! gone. It is written again whole, through a temporary file, when items go (so what
//! disappeared does not stay in the file) and when it has grown well past what it holds. A
//! damaged journal keeps the records before the damage and a copy is put aside, as group
//! logs do. Every file is readable by its owner only.
//!
//! Pins and unpins, and their deliveries, go in a second journal, `pins`, of the same form
//! and written the same way: a copy of the app from before pins reads the chat's journal
//! as it always did (to it an unknown record would be damage) and leaves `pins` alone.
//!
//! In a folder with a data key ([`crate::vault`]) the journal and the pins start with a
//! sealed file's header and each record is a box of its own, so adding one at the end stays
//! cheap, bound to the record before it and to its chat: records taken out of a file's start
//! or middle, repeated, put in another order or moved to another chat do not read (the
//! records before them do, as with any damage). How the two end is kept sealed in `tails`
//! (see [`Tails`]), so records cut off their end, or an older copy put back, show too, as a
//! chat whose history changed ([`Loaded::altered`]). Files of records sealed each on its own,
//! as they were before, are read and written again as chains. The friend's profile is sealed
//! whole, and attachments as streams, read as they go
//! (the screens get a picture's bytes, or a copy of a video in `direct/open/` to play, which
//! goes when the next one is opened and at every start). The chat's group's log is not
//! sealed: what is secret in it is sealed to the device's identity.

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{
    DevicePublicKey,
    at_rest::{
        BOX_OVERHEAD, CHAIN_START, DataKey, Form, HEADER_LEN, SealReader, SealWriter, TAG_LEN,
        chain_header, chain_link, sealed_form, stream_len,
    },
};
use fogata_net::files::{Kept, kept_context, open_kept};
use fogata_state::GroupLog;
use serde::{Deserialize, Serialize};

use super::model::{
    Attachment, Charge, Chat, DAY_MS, FileId, Item, ItemId, Local, Pool, Summary, hex_array,
    hex_list, hex_option,
};
use crate::active_files::active_ending;
use crate::store::{
    DAMAGED, JUDGED_SUFFIX, decode_entries, encode_log, from_disk, read_judged, sealed_bytes,
    to_disk, write_judged, write_private,
};

/// The rooms for pictures, videos and files the settings offer, in bytes; `None` is no
/// limit.
pub const LIMITS: [Option<u64>; 6] = [
    Some(500_000_000),
    Some(1_000_000_000),
    Some(2_000_000_000),
    Some(5_000_000_000),
    Some(10_000_000_000),
    None,
];

const DEFAULT_LIMIT: Option<u64> = Some(2_000_000_000);

/// The longest record read back; anything longer is damage. A message's record is well
/// under it even with ten videos' first pictures.
const MAX_RECORD_BYTES: usize = 4 << 20;

/// The record count of a journal that a failed write left behind what its chat holds.
const LAGGING: usize = usize::MAX;

/// Direct messages' settings on this device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Room for the pictures, videos and files of every chat together, in bytes; `None` for
    /// no limit. Past it the oldest go first.
    #[serde(default = "default_limit")]
    pub limit: Option<u64>,
    /// Pictures up to 10 MB are fetched as they arrive.
    #[serde(default = "yes")]
    pub auto_pictures: bool,
    /// Videos and files are fetched as they arrive too, not only when opened.
    #[serde(default)]
    pub auto_files: bool,
    /// The disappearing timer of new chats, 0 for off.
    #[serde(default)]
    pub timer_ms: u64,
}

fn default_limit() -> Option<u64> {
    DEFAULT_LIMIT
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            auto_pictures: true,
            auto_files: false,
            timer_ms: 0,
        }
    }
}

/// One change to a chat, as the journal keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "r", rename_all = "snake_case")]
pub enum Record {
    Item {
        item: super::model::Item,
    },
    /// This device's items reached the friend.
    Delivered {
        #[serde(with = "hex_list")]
        ids: Vec<ItemId>,
    },
    /// Everything up to `seq` was read here.
    Read {
        seq: u64,
    },
    /// Every attachment of the chat with this file id stands so here now.
    File {
        #[serde(with = "hex_array")]
        file: FileId,
        local: Local,
    },
    /// Items that disappeared, or were taken out.
    Gone {
        #[serde(with = "hex_list")]
        ids: Vec<ItemId>,
    },
    /// The two stopped being friends at `ms` (or are friends again: `None`); the chat can
    /// only be read meanwhile.
    Ended {
        ms: Option<u64>,
    },
    /// The voice messages with this file id were played here.
    Played {
        #[serde(with = "hex_array")]
        file: FileId,
    },
    /// What came for the voice messages with this file id was not one: they are files.
    NotVoice {
        #[serde(with = "hex_array")]
        file: FileId,
    },
    /// A friend's voice message, file `file`, started to come by itself at `at_ms`: `bytes`
    /// charged to the friend's daily allowance of voice messages.
    VoiceCharge {
        at_ms: u64,
        bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none", with = "hex_option")]
        file: Option<FileId>,
    },
    /// As [`Record::VoiceCharge`], for a picture, video or file.
    FileCharge {
        at_ms: u64,
        bytes: u64,
        #[serde(with = "hex_array")]
        file: FileId,
    },
    /// What came for the pictures with this file id was not one: they are files.
    NotPicture {
        #[serde(with = "hex_array")]
        file: FileId,
    },
}

impl Record {
    /// The record of a charge.
    pub fn charge(pool: Pool, charge: Charge) -> Record {
        match (pool, charge.file) {
            (Pool::Files, Some(file)) => Record::FileCharge {
                at_ms: charge.at_ms,
                bytes: charge.bytes,
                file,
            },
            _ => Record::VoiceCharge {
                at_ms: charge.at_ms,
                bytes: charge.bytes,
                file: charge.file,
            },
        }
    }
}

/// A chat read back from its folder.
#[derive(Debug, Default)]
pub struct Loaded {
    pub chat: Chat,
    pub ended_ms: Option<u64>,
    /// The chat's group, if it has one.
    pub group: Option<GroupLog>,
    /// The friend's profile as last known, encoded.
    pub profile: Option<Vec<u8>>,
    /// The journal or the pins are not as this device last wrote them: records were cut
    /// off their end, a record does not read, or a file was put back as an older copy.
    pub altered: bool,
}

impl Record {
    pub fn apply(&self, chat: &mut Chat, ended: &mut Option<u64>) {
        match self {
            Record::Item { item } => {
                chat.insert(item.clone());
            }
            Record::Delivered { ids } => {
                for id in ids {
                    if let Some(item) = chat.get_mut(id) {
                        item.delivered = true;
                    }
                }
            }
            Record::Read { seq } => chat.read_seq = chat.read_seq.max(*seq),
            Record::File { file, local } => {
                chat.set_local(file, *local);
            }
            // What was removed is remembered, unless it was this device's own: a journal
            // written whole keeps only the ids of the friend's, whose items are not here.
            Record::Gone { ids } => {
                let mine: HashSet<ItemId> = chat
                    .remove(ids)
                    .into_iter()
                    .filter(|i| i.mine)
                    .map(|i| i.id)
                    .collect();
                chat.forget(ids.iter().filter(|id| !mine.contains(*id)).copied());
            }
            Record::Ended { ms } => *ended = *ms,
            Record::Played { file } => {
                chat.set_played(file);
            }
            Record::NotVoice { file } => {
                chat.drop_voice(file);
            }
            Record::NotPicture { file } => {
                chat.drop_picture(file);
            }
            Record::VoiceCharge { at_ms, bytes, file } => {
                chat.charge(
                    Pool::Voice,
                    Charge {
                        at_ms: *at_ms,
                        bytes: *bytes,
                        file: *file,
                    },
                );
            }
            Record::FileCharge { at_ms, bytes, file } => {
                chat.charge(
                    Pool::Files,
                    Charge {
                        at_ms: *at_ms,
                        bytes: *bytes,
                        file: Some(*file),
                    },
                );
            }
        }
    }
}

/// Records to add to a chat, split between its journal and its pins as they go there, so
/// they can be written after the chat changed again.
#[derive(Debug, Default)]
pub struct Split {
    journal: Vec<Record>,
    pins: Vec<Record>,
    /// How many records the two gain.
    added: usize,
    /// The items the chat held.
    items: usize,
}

impl Split {
    /// `records`, of which `chat` already holds what they say.
    pub fn of(records: &[Record], chat: &Chat) -> Self {
        let is_pin = |id: &ItemId| chat.get(id).is_some_and(Item::is_aside);
        let mut split = Split {
            items: chat.items().len(),
            ..Default::default()
        };
        for record in records {
            match record {
                Record::Item { item } if item.is_aside() => {
                    split.pins.push(record.clone());
                }
                Record::Delivered { ids } if ids.iter().any(is_pin) => {
                    let (of_pins, rest): (Vec<ItemId>, Vec<ItemId>) =
                        ids.iter().partition(|id| is_pin(id));
                    split.pins.push(Record::Delivered { ids: of_pins });
                    if !rest.is_empty() {
                        split.journal.push(Record::Delivered { ids: rest });
                        split.added += 1;
                    }
                }
                // What went may have been either.
                Record::Gone { .. } => {
                    split.journal.push(record.clone());
                    split.pins.push(record.clone());
                    split.added += 1;
                }
                _ => split.journal.push(record.clone()),
            }
            split.added += 1;
        }
        split
    }
}

/// The folder of direct messages.
#[derive(Debug)]
pub struct DirectStore {
    dir: PathBuf,
    /// Records in each journal, to know when it is worth writing again whole; [`LAGGING`]
    /// after a write failed, or while a chat's journal or pins are plain in a folder with a
    /// key (sealing them was cut short).
    records: Mutex<HashMap<DevicePublicKey, usize>>,
    /// The folder's data key, when its data is sealed.
    key: Option<DataKey>,
    /// How each chat's sealed journal and pins end, as last written (see [`Tails`]).
    tails: Mutex<HashMap<DevicePublicKey, Tails>>,
    /// Bytes of each chat's journal and pins on the disk, as read after each write here:
    /// what the room for direct messages counts of them, known without asking the disk
    /// while the chats are held.
    sizes: Mutex<HashMap<DevicePublicKey, u64>>,
    /// Milliseconds each addition of records takes longer: a slow disk, for tests.
    #[cfg(test)]
    pub(crate) slow_ms: std::sync::atomic::AtomicU64,
    /// How many times the size of a journal and its pins was asked of the disk, for tests.
    #[cfg(test)]
    pub(crate) sizes_read: std::sync::atomic::AtomicUsize,
    /// Every attachment was found sealed: with a key, what is kept from then on is sealed,
    /// so the attachments are not read again to say so (see [`Self::holds_plain_file`]).
    all_sealed: std::sync::atomic::AtomicBool,
    /// How many times the attachments were read to tell whether one is plain, for tests.
    #[cfg(test)]
    pub(crate) plain_looks: std::sync::atomic::AtomicUsize,
    /// The disk takes no hard links (FAT, exFAT, some network shares), for tests.
    #[cfg(test)]
    pub(crate) no_links: std::sync::atomic::AtomicBool,
}

/// How a sealed journal or pins ends: how many records it has, and the last one's tag,
/// which each record after it is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Tail {
    records: usize,
    #[serde(with = "hex_array")]
    tag: [u8; TAG_LEN],
}

impl Tail {
    /// Whether a file whose records have `tags` is the one this ended, perhaps with records
    /// added after it (a write that ended before its tail was kept).
    fn found_in(tail: Option<&Tail>, tags: &[[u8; TAG_LEN]]) -> bool {
        match tail {
            None => true,
            Some(t) => t.records == 0 || tags.get(t.records - 1) == Some(&t.tag),
        }
    }

    fn of(tags: &[[u8; TAG_LEN]]) -> Option<Tail> {
        tags.last().map(|tag| Tail {
            records: tags.len(),
            tag: *tag,
        })
    }
}

/// How a chat's sealed journal and pins end, kept sealed in the chat's folder (`tails`) and
/// written after them, so records cut off their end, or a file put back as an older copy,
/// show. While the two are written again whole, how they will end is kept too: a write cut
/// short then reads as either. The data folder could still be put back whole, the tails
/// with it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Tails {
    journal: Option<Tail>,
    pins: Option<Tail>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next: Option<(Option<Tail>, Option<Tail>)>,
}

impl DirectStore {
    /// The direct messages of a data folder whose data is plain.
    pub fn open(data_dir: &Path) -> Self {
        Self::open_sealed(data_dir, None)
    }

    /// The direct messages of a data folder, sealed with `key` when it has one.
    pub fn open_sealed(data_dir: &Path, key: Option<DataKey>) -> Self {
        Self {
            dir: data_dir.join("direct"),
            records: Mutex::new(HashMap::new()),
            key,
            tails: Mutex::new(HashMap::new()),
            sizes: Mutex::new(HashMap::new()),
            #[cfg(test)]
            slow_ms: Default::default(),
            #[cfg(test)]
            sizes_read: Default::default(),
            all_sealed: Default::default(),
            #[cfg(test)]
            plain_looks: Default::default(),
            #[cfg(test)]
            no_links: Default::default(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The data key that attachments kept here may be sealed with.
    pub fn key(&self) -> Option<&DataKey> {
        self.key.as_ref()
    }

    fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }

    /// The settings; the defaults when none were saved or they cannot be read.
    pub fn settings(&self) -> Settings {
        fs::read(self.settings_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.make_dir(&self.dir)?;
        write_private(&self.settings_path(), &serde_json::to_vec_pretty(settings)?)
    }

    /// A friend's chat folder.
    pub fn chat_dir(&self, friend: &DevicePublicKey) -> PathBuf {
        self.dir.join(hex::encode(friend.as_bytes()))
    }

    fn journal_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend).join("journal")
    }

    fn pins_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend).join("pins")
    }

    fn group_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend).join("group.log")
    }

    /// What was judged of the chat's group log, beside it (see [`read_judged`]).
    fn judged_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend)
            .join(format!("group.log{JUDGED_SUFFIX}"))
    }

    fn profile_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend).join("profile")
    }

    fn tails_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend).join(TAILS)
    }

    fn summary_path(&self, friend: &DevicePublicKey) -> PathBuf {
        self.chat_dir(friend).join(SUMMARY)
    }

    /// Keeps `summary`, the summary of the chat as its journal and pins hold it now (just
    /// written), with how the two end, so it is only taken while they still end so. Sealed
    /// in a folder with a key. Nothing is kept while the two lag behind the chat.
    pub fn write_summary(&self, friend: &DevicePublicKey, summary: &Summary) -> Result<()> {
        let records = self
            .records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(friend)
            .copied();
        let Some(records) = records.filter(|&n| n != LAGGING) else {
            return Ok(());
        };
        let kept = KeptSummary {
            summary: summary.clone(),
            journal: end_of(&self.journal_path(friend))?,
            pins: end_of(&self.pins_path(friend))?,
            records,
        };
        let json = zeroize::Zeroizing::new(serde_json::to_vec(&kept)?);
        let bytes = match &self.key {
            Some(key) => key.seal_whole(&summary_context(&chat_name(friend)), &json)?,
            None => json.to_vec(),
        };
        write_private(&self.summary_path(friend), &bytes)
    }

    /// The summary kept for a chat, if it was written for its journal and pins as they are
    /// now; otherwise (none, from another chat, damaged, older or newer than the two, the
    /// two not as their tails say, or in a form that is read whole to be written again)
    /// `None`, and the chat is to be read whole.
    pub fn summary(&self, friend: &DevicePublicKey) -> Option<Summary> {
        self.kept_summary(friend).map(|(kept, _)| kept.summary)
    }

    /// [`Self::summary`], taken in place of reading the chat: the journal and pins are then
    /// added to as if they had been read.
    pub fn resume(&self, friend: &DevicePublicKey) -> Option<Summary> {
        let (kept, tails) = self.kept_summary(friend)?;
        if let Some(tails) = tails {
            self.tails
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(*friend, tails);
        }
        self.count(friend, kept.records);
        self.drop_damaged(friend, Some(DAMAGED_KEPT));
        Some(kept.summary)
    }

    /// The summary kept for a chat when it stands for the chat as it is on the disk, with the
    /// tails the two end with when sealed.
    fn kept_summary(&self, friend: &DevicePublicKey) -> Option<(KeptSummary, Option<Tails>)> {
        let bytes = fs::read(self.summary_path(friend)).ok()?;
        let plain = match &self.key {
            Some(key) => key
                .open_whole(&summary_context(&chat_name(friend)), &bytes)
                .ok()?,
            None if sealed_form(&bytes).is_some() => return None,
            None => zeroize::Zeroizing::new(bytes),
        };
        let kept: KeptSummary = serde_json::from_slice(&plain).ok()?;
        let (journal, pins) = (self.journal_path(friend), self.pins_path(friend));
        if end_of(&journal).ok()? != kept.journal || end_of(&pins).ok()? != kept.pins {
            return None;
        }
        if self.key.is_none() {
            return Some((kept, None));
        }
        // Sealed, both are chains ending as their tails say, no rewrite cut short.
        for (path, end) in [(&journal, &kept.journal), (&pins, &kept.pins)] {
            if end.len > 0 && chain_form(path) != Some(Form::Chain) {
                return None;
            }
        }
        let tails = self.read_tails(friend).ok()??;
        let ends = |tail: Option<Tail>, end: &End| match tail {
            Some(t) => t.records > 0 && t.tag == end.last,
            None => end.len <= HEADER_LEN as u64,
        };
        if tails.next.is_some()
            || !ends(tails.journal, &kept.journal)
            || !ends(tails.pins, &kept.pins)
        {
            return None;
        }
        Some((kept, Some(tails)))
    }

    /// The tails kept for a chat: `Ok(None)` when there are none (a folder from before
    /// they were kept, or plain), an error when they do not read.
    fn read_tails(&self, friend: &DevicePublicKey) -> Result<Option<Tails>> {
        let Some(key) = self.key.as_ref() else {
            return Ok(None);
        };
        let bytes = match fs::read(self.tails_path(friend)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let plain = key.open_whole(&tails_context(&chat_name(friend)), &bytes)?;
        Ok(Some(serde_json::from_slice(&plain)?))
    }

    /// Keeps a chat's tails, sealed (only in a folder with a key).
    fn write_tails(&self, friend: &DevicePublicKey, tails: &Tails) -> Result<()> {
        let Some(key) = self.key.as_ref() else {
            return Ok(());
        };
        let json = zeroize::Zeroizing::new(serde_json::to_vec(tails)?);
        let sealed = key.seal_whole(&tails_context(&chat_name(friend)), &json)?;
        write_private(&self.tails_path(friend), &sealed)?;
        self.tails
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(*friend, tails.clone());
        Ok(())
    }

    /// Where an attachment is kept here: its id in hex, with the ending of its name when
    /// that is a plain one, so the system's players know what it is (see [`kept_name`]). One
    /// kept under the name used before is moved to this one.
    pub fn file_path(&self, friend: &DevicePublicKey, file: &Attachment) -> PathBuf {
        let dir = self.chat_dir(friend).join("files");
        let path = dir.join(kept_name(&file.id, &file.name));
        if let Some(before) = kept_name_before(&file.id, &file.name) {
            let before = dir.join(before);
            if fs::symlink_metadata(&path).is_err()
                && fs::symlink_metadata(&before).is_ok()
                && let Err(e) = fs::rename(&before, &path)
            {
                tracing::warn!("cannot move {}: {e}", before.display());
            }
        }
        path
    }

    /// The chats kept here, each with its friend.
    pub fn chats(&self) -> Vec<(DevicePublicKey, Loaded)> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(friend) = hex::decode(&name)
                .ok()
                .and_then(|b| DevicePublicKey::from_bytes(&b).ok())
            else {
                continue;
            };
            if !entry.path().is_dir() {
                continue;
            }
            match self.load(&friend) {
                Ok(loaded) => out.push((friend, loaded)),
                Err(e) => tracing::warn!("chat {name} not loaded: {e:#}"),
            }
        }
        out
    }

    /// Reads a chat back: its journal, then its pins. A damaged journal keeps what came
    /// before the damage, a copy of it is put aside, and it is written again from what was
    /// kept.
    pub fn load(&self, friend: &DevicePublicKey) -> Result<Loaded> {
        let mut loaded = Loaded::default();
        let mut count = 0;
        let mut damaged = false;
        let mut plain = false;
        let mut unchained = false;
        let chat = chat_name(friend);
        let mut ends = [None, None];
        for (at, (path, file)) in [
            (self.journal_path(friend), JOURNAL),
            (self.pins_path(friend), PINS),
        ]
        .into_iter()
        .enumerate()
        {
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
                Err(e) => {
                    return Err(e).with_context(|| format!("cannot read {}", path.display()));
                }
            };
            plain |= !bytes.is_empty() && sealed_form(&bytes).is_none();
            unchained |= sealed_form(&bytes) == Some(Form::Records);
            let (records, tags, broken) = read_records(self.key.as_ref(), file, &chat, &bytes)
                .with_context(|| format!("cannot read {}", path.display()))?;
            ends[at] = Some(tags);
            for record in &records {
                record.apply(&mut loaded.chat, &mut loaded.ended_ms);
            }
            count += records.len();
            if broken {
                damaged = true;
                let mut name = path.file_name().unwrap_or_default().to_owned();
                name.push(".damaged");
                let aside = path.with_file_name(name);
                // Without the copy, what follows the damage is lost at the next write.
                let copied = aside.exists() || {
                    fs::copy(&path, &aside)
                        .inspect_err(|e| {
                            tracing::warn!("cannot copy {} aside: {e}", path.display())
                        })
                        .is_ok()
                };
                tracing::warn!(
                    "{} is damaged: the {} records before the damage are kept{}",
                    path.display(),
                    records.len(),
                    if copied {
                        format!(", and a copy is in {}", aside.display())
                    } else {
                        String::new()
                    }
                );
            }
        }
        loaded.chat.settle();
        loaded.chat.prune_charges(crate::now_ms());
        // Sealed, the files must end as this device last wrote them. A folder from before
        // that was kept has nothing to compare with: what is there now counts from here.
        let chained = self.key.is_some() && !plain && !unchained;
        let kept = if chained {
            self.read_tails(friend)
        } else {
            Ok(None)
        };
        let [journal, pins] = ends.map(|tags| tags.unwrap_or_default());
        loaded.altered = damaged
            || match &kept {
                Ok(Some(t)) => {
                    // Each as written, or as it was to be written again.
                    let ends_as = |tags: &[[u8; TAG_LEN]],
                                   was: Option<&Tail>,
                                   next: Option<&Tail>| {
                        Tail::found_in(was, tags) || t.next.is_some() && Tail::found_in(next, tags)
                    };
                    let next = t.next.unwrap_or_default();
                    !ends_as(&journal, t.journal.as_ref(), next.0.as_ref())
                        || !ends_as(&pins, t.pins.as_ref(), next.1.as_ref())
                }
                Ok(None) => false,
                Err(_) => chained,
            };
        if loaded.altered && !damaged {
            tracing::warn!("chat {chat}: its journal or pins are not as last written here");
        }
        if chained && !damaged {
            let now = Tails {
                journal: Tail::of(&journal),
                pins: Tail::of(&pins),
                next: None,
            };
            if kept.as_ref().ok().and_then(Option::as_ref) != Some(&now)
                && let Err(e) = self.write_tails(friend, &now)
            {
                tracing::warn!("chat {chat}: its tails not kept: {e:#}");
            }
            self.tails
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(*friend, now);
        }
        if damaged {
            self.rewrite(friend, &loaded.chat, loaded.ended_ms)?;
        } else if unchained {
            // Written again as chains now; until that works, nothing is added to them.
            if let Err(e) = self.rewrite(friend, &loaded.chat, loaded.ended_ms) {
                tracing::warn!("chat {chat} not written again as a chain: {e:#}");
                self.count(friend, LAGGING);
            }
        } else if plain && self.key.is_some() {
            // Written whole, sealed, at the next change.
            self.count(friend, LAGGING);
        } else {
            self.count(friend, count);
        }
        self.drop_damaged(friend, Some(DAMAGED_KEPT));
        self.measure(friend);
        let group_path = self.group_path(friend);
        if let Ok(bytes) = fs::read(&group_path) {
            let (entries, _) = decode_entries(&bytes);
            if let Some(first) = entries.first() {
                let (path, context) = (self.judged_path(friend), judged_context(&chat));
                let group = first.group_id;
                loaded.group = Some(read_judged(
                    self.key.as_ref(),
                    &path,
                    &context,
                    group,
                    entries,
                ));
            }
        }
        loaded.profile = fs::read(self.profile_path(friend))
            .ok()
            .and_then(|bytes| from_disk(self.key.as_ref(), PROFILE, &bytes).ok())
            .map(|plain| plain.to_vec());
        Ok(loaded)
    }

    fn count(&self, friend: &DevicePublicKey, records: usize) {
        if let Ok(mut map) = self.records.lock() {
            map.insert(*friend, records);
        }
    }

    fn make_dir(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(0o700)) {
                tracing::warn!("cannot keep {} to this user only: {e}", dir.display());
            }
        }
        Ok(())
    }

    /// Adds records at the end of a chat's journal, those about pins at the end of its
    /// pins, and flushes them to the disk. `chat` already holds what they say. Returns
    /// whether the two should be written again whole ([`Self::rewrite`]): they have grown
    /// well past what `chat` holds, or an earlier write failed, so they lack what came since.
    pub fn append(
        &self,
        friend: &DevicePublicKey,
        records: &[Record],
        chat: &Chat,
    ) -> Result<bool> {
        self.append_split(friend, Split::of(records, chat))
    }

    /// [`Self::append`] of records split already, as they were when `chat` held them.
    pub fn append_split(&self, friend: &DevicePublicKey, split: Split) -> Result<bool> {
        if self.lagging(friend) {
            return Ok(true);
        }
        if split.added == 0 {
            return Ok(false);
        }
        let added = self.add_records(friend, &split);
        self.measure(friend);
        if added.is_err() {
            self.records
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(*friend, LAGGING);
        }
        let added = added?;
        let mut map = self.records.lock().unwrap_or_else(|e| e.into_inner());
        let count = map.entry(*friend).or_default();
        *count += added;
        Ok(*count > split.items * 2 + 64)
    }

    /// Whether a write of the chat's journal or pins failed since they were last written
    /// whole.
    pub fn lagging(&self, friend: &DevicePublicKey) -> bool {
        self.records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(friend)
            .is_some_and(|&n| n == LAGGING)
    }

    /// Writes the records to the journal and the pins; returns how many records the two
    /// gained.
    fn add_records(&self, friend: &DevicePublicKey, split: &Split) -> Result<usize> {
        #[cfg(test)]
        std::thread::sleep(Duration::from_millis(
            self.slow_ms.load(std::sync::atomic::Ordering::Relaxed),
        ));
        let dir = self.chat_dir(friend);
        self.make_dir(&dir)?;
        let chat = chat_name(friend);
        let mut tails = self
            .tails
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(friend)
            .cloned()
            .unwrap_or_default();
        for (path, file, records) in [
            (self.journal_path(friend), JOURNAL, &split.journal),
            (self.pins_path(friend), PINS, &split.pins),
        ] {
            if !records.is_empty() {
                let last = append_records(
                    &path,
                    self.key.as_ref(),
                    &chain_context(file, &chat),
                    records,
                )?;
                let tail = if file == JOURNAL {
                    &mut tails.journal
                } else {
                    &mut tails.pins
                };
                *tail = Some(Tail {
                    records: tail.map_or(0, |t| t.records) + records.len(),
                    tag: last,
                });
            }
        }
        self.write_tails(friend, &tails)?;
        Ok(split.added)
    }

    /// Writes a chat's journal and its pins again whole, with only what it holds now.
    pub fn rewrite(
        &self,
        friend: &DevicePublicKey,
        chat: &Chat,
        ended_ms: Option<u64>,
    ) -> Result<()> {
        let written = self.write_whole(friend, chat, ended_ms);
        self.measure(friend);
        written
    }

    fn write_whole(
        &self,
        friend: &DevicePublicKey,
        chat: &Chat,
        ended_ms: Option<u64>,
    ) -> Result<()> {
        let dir = self.chat_dir(friend);
        self.make_dir(&dir)?;
        let (pins, items): (Vec<&Item>, Vec<&Item>) =
            chat.items().iter().partition(|item| item.is_aside());
        // The friend's items removed here first, so one sent again is still not kept again
        // after a restart (an older build reads them as removing what is not there).
        let forgotten: Vec<ItemId> = chat.forgotten().copied().collect();
        let mut records: Vec<Record> = Vec::new();
        if !forgotten.is_empty() {
            records.push(Record::Gone { ids: forgotten });
        }
        records.extend(
            items
                .into_iter()
                .map(|item| Record::Item { item: item.clone() }),
        );
        records.push(Record::Read { seq: chat.read_seq });
        // Charges older than a day count no more: nothing of them is kept.
        let now = crate::now_ms();
        let fresh = |c: &&Charge| c.at_ms.saturating_add(DAY_MS) > now;
        records.extend(
            chat.voice_charges
                .iter()
                .filter(fresh)
                .map(|c| Record::charge(Pool::Voice, *c)),
        );
        records.extend(
            chat.file_charges
                .iter()
                .filter(fresh)
                .map(|c| Record::charge(Pool::Files, *c)),
        );
        if ended_ms.is_some() {
            records.push(Record::Ended { ms: ended_ms });
        }
        let key = self.key.as_ref();
        let chat = chat_name(friend);
        let mut journal = Records::whole(key, chain_context(JOURNAL, &chat));
        for record in &records {
            journal.push(record)?;
        }
        let mut pinned = Records::whole(key, chain_context(PINS, &chat));
        for item in &pins {
            pinned.push(&Record::Item {
                item: (*item).clone(),
            })?;
        }
        // How the two will end is kept first, so a write cut short reads as either.
        let then = (journal.tail(), pinned.tail());
        let was = self
            .tails
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(friend)
            .cloned()
            .unwrap_or_default();
        self.write_tails(
            friend,
            &Tails {
                next: Some(then),
                ..was
            },
        )?;
        write_private(&self.journal_path(friend), &journal.bytes)?;
        let mut count = records.len();
        let path = self.pins_path(friend);
        if pins.is_empty() {
            if let Err(e) = fs::remove_file(&path)
                && e.kind() != io::ErrorKind::NotFound
            {
                return Err(e).with_context(|| format!("cannot remove {}", path.display()));
            }
        } else {
            write_private(&path, &pinned.bytes)?;
            count += pins.len();
        }
        self.write_tails(
            friend,
            &Tails {
                journal: then.0,
                pins: then.1,
                next: None,
            },
        )?;
        self.count(friend, count);
        Ok(())
    }

    /// Removes the copies of a damaged journal or pins put aside in the chat's folder, which
    /// keep messages that have disappeared since: all of them, or only those older than
    /// `older_than`.
    pub fn drop_damaged(&self, friend: &DevicePublicKey, older_than: Option<Duration>) {
        for entry in fs::read_dir(self.chat_dir(friend))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("journal.damaged") && !name.starts_with("pins.damaged") {
                continue;
            }
            let old = older_than.is_none_or(|age| {
                entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|at| at.elapsed().ok())
                    .is_some_and(|since| since > age)
            });
            if old && let Err(e) = fs::remove_file(entry.path()) {
                tracing::warn!("cannot remove {}: {e}", entry.path().display());
            }
        }
    }

    /// Keeps the log of the chat's group.
    pub fn save_group(&self, friend: &DevicePublicKey, log: &GroupLog) -> Result<()> {
        let dir = self.chat_dir(friend);
        self.make_dir(&dir)?;
        write_private(&self.group_path(friend), &encode_log(log))?;
        let context = judged_context(&chat_name(friend));
        write_judged(self.key.as_ref(), &self.judged_path(friend), &context, log);
        Ok(())
    }

    /// Forgets the chat's group (the friend made another one, or it no longer fits).
    pub fn forget_group(&self, friend: &DevicePublicKey) {
        let _ = fs::remove_file(self.group_path(friend));
        let _ = fs::remove_file(self.judged_path(friend));
    }

    /// Keeps the friend's profile, encoded, when it changed.
    pub fn save_profile(&self, friend: &DevicePublicKey, profile: &[u8]) -> Result<()> {
        let key = self.key.as_ref();
        if fs::read(self.profile_path(friend))
            .ok()
            .and_then(|old| from_disk(key, PROFILE, &old).ok())
            .is_some_and(|old| *old == *profile)
        {
            return Ok(());
        }
        let dir = self.chat_dir(friend);
        self.make_dir(&dir)?;
        write_private(&self.profile_path(friend), &to_disk(key, PROFILE, profile)?)
    }

    /// The folder for a chat's attachments, made if needed.
    pub fn files_dir(&self, friend: &DevicePublicKey) -> Result<PathBuf> {
        let dir = self.chat_dir(friend).join("files");
        self.make_dir(&self.chat_dir(friend))?;
        self.make_dir(&dir)?;
        Ok(dir)
    }

    /// Whether a whole copy of an attachment is kept here: of its size, or sealed.
    pub fn has_copy(&self, friend: &DevicePublicKey, file: &Attachment) -> bool {
        fs::metadata(self.file_path(friend, file)).is_ok_and(|m| {
            m.len() == file.size || (self.key.is_some() && m.len() == stream_len(file.size))
        })
    }

    /// Keeps a copy of the file at `from` as `file`'s, sealed when the folder has a key,
    /// readable by its owner only; only if the bytes copied are that file's (its id): the
    /// file at `from` may have changed since it was hashed, and what is served under an id
    /// is that file. Blocking.
    pub fn keep_copy(
        &self,
        friend: &DevicePublicKey,
        file: &Attachment,
        from: &Path,
    ) -> Result<()> {
        let dest = self.file_path(friend, file);
        let tmp = temp_next_to(&dest);
        let kept = (|| {
            let out = new_private(&tmp)?;
            let mut hasher = blake3::Hasher::new();
            let mut source = fogata_net::files::open_without_waiting(from)?;
            if !source.metadata()?.is_file() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file").into());
            }
            let mut buf = vec![0u8; 64 * 1024];
            let mut copy = |write: &mut dyn io::Write| -> io::Result<()> {
                loop {
                    let n = io::Read::read(&mut source, &mut buf)?;
                    if n == 0 {
                        return Ok(());
                    }
                    hasher.update(&buf[..n]);
                    write.write_all(&buf[..n])?;
                }
            };
            match &self.key {
                Some(key) => {
                    let mut writer = SealWriter::new(key, &kept_context(&file.id), out)?;
                    copy(&mut writer)?;
                    writer.finish()?.sync_all()?;
                }
                None => {
                    let mut out = out;
                    copy(&mut out)?;
                    out.sync_all()?;
                }
            }
            if hasher.finalize().as_bytes() != &file.id {
                bail!("it changed while it was being sent; send it again");
            }
            fs::rename(&tmp, &dest)?;
            Ok(())
        })();
        if kept.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        kept
    }

    /// Keeps `bytes` as `file`'s copy (a voice message recorded here), sealed when the folder
    /// has a key, readable by its owner only. Blocking.
    pub fn keep_bytes(
        &self,
        friend: &DevicePublicKey,
        file: &Attachment,
        bytes: &[u8],
    ) -> Result<()> {
        let dest = self.file_path(friend, file);
        let tmp = temp_next_to(&dest);
        let kept = (|| {
            let mut out = new_private(&tmp)?;
            match &self.key {
                Some(key) => {
                    let mut writer = SealWriter::new(key, &kept_context(&file.id), out)?;
                    io::Write::write_all(&mut writer, bytes)?;
                    writer.finish()?.sync_all()?;
                }
                None => {
                    io::Write::write_all(&mut out, bytes)?;
                    out.sync_all()?;
                }
            }
            fs::rename(&tmp, &dest)
        })();
        if kept.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        Ok(kept?)
    }

    /// An attachment kept here, to read as its plaintext, with its size. Blocking.
    pub fn open_file(
        &self,
        friend: &DevicePublicKey,
        file: &Attachment,
    ) -> io::Result<(Kept, u64)> {
        open_kept(&self.file_path(friend, file), &file.id, self.key.as_ref())
    }

    /// Copies an attachment kept here, as its plaintext, to `dest`, where nothing may be (not
    /// even a link). A copy that stops halfway (a full disk, a damaged part) is removed: a
    /// part of it is no use to anyone, and takes the room the disk keeps free. Blocking.
    fn copy_out(&self, friend: &DevicePublicKey, file: &Attachment, dest: &Path) -> Result<()> {
        let (mut kept, _) = self.open_file(friend, file)?;
        let mut out = new_private(dest)?;
        let copied = io::copy(&mut kept, &mut out).and_then(|_| out.sync_all());
        if let Err(e) = copied {
            drop(out);
            let _ = fs::remove_file(dest);
            return Err(e.into());
        }
        Ok(())
    }

    /// Where an attachment kept here can be played from: its own file when it is plain, else
    /// a plain copy in `direct/open/` made for this player alone, under a name of its own
    /// (another player may have the same one open), which goes with [`Self::forget_copy`].
    /// Blocking.
    pub fn open_to_play(&self, friend: &DevicePublicKey, file: &Attachment) -> Result<PathBuf> {
        self.open_to_play_holding(friend, file, |_, _| Ok(()))
    }

    /// [`Self::open_to_play`], `hold` asked first for room on the disk for a plain copy of
    /// `size` bytes in `dir` when one is made (see `shares::hold_disk`): what it holds is
    /// held until the copy is made, or not.
    pub fn open_to_play_holding<R>(
        &self,
        friend: &DevicePublicKey,
        file: &Attachment,
        hold: impl FnOnce(u64, &Path) -> Result<R>,
    ) -> Result<PathBuf> {
        let (kept, _) = self.open_file(friend, file)?;
        if let Kept::Plain(_) = kept {
            return Ok(self.file_path(friend, file));
        }
        drop(kept);
        let _room = hold(file.size, &self.dir)?;
        let dir = self.dir.join(OPEN_DIR);
        self.make_dir(&dir)?;
        let mut tag = [0u8; 8];
        fogata_crypto::random_bytes(&mut tag)?;
        let dest = dir.join(format!(
            "{}-{}",
            hex::encode(tag),
            kept_name(&file.id, &file.name)
        ));
        self.copy_out(friend, file, &dest)?;
        Ok(dest)
    }

    /// Removes one plain copy made to play an attachment, once its player let it go; any
    /// other path is left alone.
    pub fn forget_copy(&self, copy: &Path) {
        let dir = self.dir.join(OPEN_DIR);
        if copy.parent() != Some(dir.as_path()) || copy.file_name().is_none() {
            return;
        }
        if let Err(e) = fs::remove_file(copy)
            && e.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!("cannot remove {}: {e}", copy.display());
        }
    }

    /// Removes the plain copies made to play attachments.
    pub fn forget_copies(&self) {
        let dir = self.dir.join(OPEN_DIR);
        if let Err(e) = fs::remove_dir_all(&dir)
            && e.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!("cannot remove {}: {e}", dir.display());
        }
    }

    /// The attachments kept here that are plain while the folder has a key, by chat: they
    /// were kept before it was sealed.
    pub fn plain_files(&self) -> Vec<PathBuf> {
        let Some(key) = &self.key else {
            return Vec::new();
        };
        let Ok(chats) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for chat in chats.flatten() {
            let Ok(files) = fs::read_dir(chat.path().join("files")) else {
                continue;
            };
            for entry in files.flatten() {
                let path = entry.path();
                let Some(id) = file_id_of(&path) else {
                    continue;
                };
                if path.is_file()
                    && open_kept(&path, &id, Some(key))
                        .is_ok_and(|(kept, _)| matches!(kept, Kept::Plain(_)))
                {
                    out.push(path);
                }
            }
        }
        out
    }

    /// Whether an attachment kept here is plain while the folder has a key: one that does
    /// not start as a sealed file does. Only the start of each is read, and the first plain
    /// one ends the look; [`Self::plain_files`] tells them apart for sealing.
    pub fn holds_plain_file(&self) -> bool {
        use std::sync::atomic::Ordering;
        if self.key.is_none() || self.all_sealed.load(Ordering::Relaxed) {
            return false;
        }
        #[cfg(test)]
        self.plain_looks.fetch_add(1, Ordering::Relaxed);
        let Ok(chats) = fs::read_dir(&self.dir) else {
            return false;
        };
        // A file that could not be read now (busy, say) is looked at again next time: the
        // answer is kept only once every attachment was read and found sealed.
        let unread = std::cell::Cell::new(false);
        let plain = chats.flatten().any(|chat| {
            let Ok(files) = fs::read_dir(chat.path().join("files")) else {
                return false;
            };
            files.flatten().any(|entry| {
                let path = entry.path();
                if file_id_of(&path).is_none() || !path.is_file() {
                    return false;
                }
                let mut start = [0u8; HEADER_LEN];
                let read = fs::File::open(&path).and_then(|mut f| {
                    let mut n = 0;
                    while n < start.len() {
                        match f.read(&mut start[n..])? {
                            0 => break,
                            got => n += got,
                        }
                    }
                    Ok(n)
                });
                if read.is_err() {
                    unread.set(true);
                }
                read.is_ok_and(|n| sealed_form(&start[..n]).is_none())
            })
        });
        if !plain && !unread.get() {
            self.all_sealed.store(true, Ordering::Relaxed);
        }
        plain
    }

    /// Seals an attachment kept plain at `path` into a file next to it, checks it reads back
    /// as what the plain one holds, and hands it to `swap`, which puts it in the plain one's
    /// place (or removes it, when the plain one went meanwhile). Blocking.
    pub fn seal_file(&self, path: &Path, swap: impl FnOnce(&Path) -> io::Result<()>) -> Result<()> {
        let (Some(key), Some(id)) = (&self.key, file_id_of(path)) else {
            bail!("{} cannot be sealed", path.display());
        };
        let context = kept_context(&id);
        let tmp = temp_next_to(path);
        let sealed = (|| -> Result<()> {
            let mut writer = SealWriter::new(key, &context, new_private(&tmp)?)?;
            let mut hasher = blake3::Hasher::new();
            let mut plain = fs::File::open(path)?;
            let mut buf = vec![0u8; 256 * 1024];
            loop {
                let n = plain.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                writer.write_all(&buf[..n])?;
            }
            writer.finish()?.sync_all()?;
            let len = fs::metadata(&tmp)?.len();
            let mut back = SealReader::new(key, &context, fs::File::open(&tmp)?, len)?;
            let mut again = blake3::Hasher::new();
            io::copy(&mut back, &mut again)?;
            if again.finalize() != hasher.finalize() {
                bail!("{} did not read back once sealed", path.display());
            }
            Ok(swap(&tmp)?)
        })();
        if sealed.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        sealed
    }

    /// Makes the file kept as `kept`'s reachable as `copy`'s too, when the two have the
    /// same id (the same file) under names that end differently, so each message shows and
    /// plays it under its own ending: a hard link, or a copy where the disk has none.
    /// Blocking.
    pub fn link_copy(
        &self,
        friend: &DevicePublicKey,
        kept: &Attachment,
        copy: &Attachment,
    ) -> Result<()> {
        self.link_copy_holding(friend, kept, copy, |_, _| Ok(()))
    }

    /// [`Self::link_copy`], `hold` asked first for room on the disk for a copy of so many
    /// bytes in the chat's folder when one is made (a link takes none).
    pub fn link_copy_holding<R>(
        &self,
        friend: &DevicePublicKey,
        kept: &Attachment,
        copy: &Attachment,
        hold: impl FnOnce(u64, &Path) -> Result<R>,
    ) -> Result<()> {
        let (from, to) = (self.file_path(friend, kept), self.file_path(friend, copy));
        if kept.id != copy.id || from == to || fs::symlink_metadata(&to).is_ok() {
            return Ok(());
        }
        #[cfg(test)]
        let linked = !self.no_links.load(std::sync::atomic::Ordering::Relaxed)
            && fs::hard_link(&from, &to).is_ok();
        #[cfg(not(test))]
        let linked = fs::hard_link(&from, &to).is_ok();
        if linked {
            return Ok(());
        }
        let _room = hold(
            fs::metadata(&from).map_or(kept.size, |m| m.len()),
            to.parent().unwrap_or(&to),
        )?;
        let tmp = temp_next_to(&to);
        let copied = (|| {
            let mut out = new_private(&tmp)?;
            io::copy(&mut fs::File::open(&from)?, &mut out)?;
            out.sync_all()?;
            fs::rename(&tmp, &to)
        })();
        if copied.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        Ok(copied?)
    }

    /// Removes the file kept for attachment id `file`, under each of `names` it is known by
    /// in its chat (see [`Self::link_copy`]).
    pub fn remove_file<'a>(
        &self,
        friend: &DevicePublicKey,
        file: &FileId,
        names: impl IntoIterator<Item = &'a str>,
    ) {
        let dir = self.chat_dir(friend).join("files");
        let paths: std::collections::BTreeSet<PathBuf> = names
            .into_iter()
            .flat_map(|name| [Some(kept_name(file, name)), kept_name_before(file, name)])
            .flatten()
            .map(|kept| dir.join(kept))
            .collect();
        for path in paths {
            if let Err(e) = fs::remove_file(&path)
                && e.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!("cannot remove {}: {e}", path.display());
            }
        }
    }

    /// The size of a chat's journal and pins, for what its messages take, as last written.
    pub fn journal_bytes(&self, friend: &DevicePublicKey) -> u64 {
        let known = self
            .sizes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(friend)
            .copied();
        known.unwrap_or_else(|| self.measure(friend))
    }

    /// Reads the bytes of a chat's journal and pins from the disk, and keeps them for
    /// [`Self::journal_bytes`]: after every write of the two, on the thread that wrote.
    fn measure(&self, friend: &DevicePublicKey) -> u64 {
        #[cfg(test)]
        self.sizes_read
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let bytes = [self.journal_path(friend), self.pins_path(friend)]
            .iter()
            .map(|path| fs::metadata(path).map_or(0, |m| m.len()))
            .sum();
        self.sizes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(*friend, bytes);
        bytes
    }

    /// Removes a chat's folder: its messages, files, group and the friend's profile.
    pub fn remove_chat(&self, friend: &DevicePublicKey) -> Result<()> {
        let dir = self.chat_dir(friend);
        if let Ok(mut map) = self.records.lock() {
            map.remove(friend);
        }
        if let Ok(mut map) = self.tails.lock() {
            map.remove(friend);
        }
        let removed = fs::remove_dir_all(&dir);
        self.sizes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(friend);
        match removed {
            Err(e) if e.kind() != io::ErrorKind::NotFound => {
                Err(e).with_context(|| format!("cannot remove {}", dir.display()))
            }
            _ => Ok(()),
        }
    }
}

/// Opens the journal or pins at `path` to read and write, made private if new. It is not
/// opened for appending: Windows lets only a handle that can write anywhere cut the file
/// shorter, as a failed write needs, so what is added is written at the end by seeking there.
fn open_records(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Adds `records` at the end of the journal or pins at `path`, made private if new, and
/// flushes them to the disk. Sealed with `key`, a new file starts with the chain's header,
/// and the first record added is bound to the last one there, whose tag ends the file; a
/// file of records of the earlier form is not added to (it is written again whole).
/// Returns the tag the file now ends with (nothing when plain).
fn append_records(
    path: &Path,
    key: Option<&DataKey>,
    context: &str,
    records: &[Record],
) -> Result<[u8; TAG_LEN]> {
    let mut file = open_records(path).with_context(|| format!("cannot open {}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("cannot read {}", path.display()))?
        .len();
    let mut out = match key {
        Some(_) if len == 0 => Records::whole(key, context.to_owned()),
        Some(_) => {
            let mut header = [0u8; HEADER_LEN];
            let mut before = CHAIN_START;
            file.seek(SeekFrom::Start(0))
                .and_then(|_| file.read_exact(&mut header))
                .and_then(|()| {
                    if len > HEADER_LEN as u64 {
                        file.seek(SeekFrom::End(-(TAG_LEN as i64)))?;
                        file.read_exact(&mut before)?;
                    }
                    Ok(())
                })
                .with_context(|| format!("cannot read {}", path.display()))?;
            if sealed_form(&header) != Some(Form::Chain) {
                bail!("{} is not a chain of records", path.display());
            }
            Records::after(key, context.to_owned(), before)
        }
        None => Records::after(None, context.to_owned(), CHAIN_START),
    };
    for record in records {
        out.push(record)?;
    }
    let written = file
        .seek(SeekFrom::End(0))
        .and_then(|_| file.write_all(&out.bytes))
        .and_then(|()| file.sync_data());
    if written.is_err() {
        // A full disk can take part of the records. Cut it off: records added behind a
        // broken one would be lost with it the next time the file is read. Should that fail
        // too, the chat is written again whole (it lags), which replaces the file.
        if let Err(e) = file.set_len(len) {
            tracing::warn!(
                "cannot cut {} back after a failed write: {e}",
                path.display()
            );
        }
    }
    written.with_context(|| format!("cannot write {}", path.display()))?;
    Ok(out.before)
}

/// The name a file is kept under: its id in hex, with the ending of its name `name` when that
/// is a plain one (one to eight letters and digits), as no ending that a file system treats
/// in its own way (a `:` starts a stream on Windows) gets through, and none of a file that
/// acts on its own (see [`crate::active_files`]): a player needs no such ending, and the
/// folder may be shown.
pub(crate) fn kept_name(id: &[u8; 32], name: &str) -> String {
    match plain_ending(name).filter(|e| !active_ending(e)) {
        Some(ending) => format!("{}.{ending}", hex::encode(id)),
        None => hex::encode(id),
    }
}

/// The name a file was kept under before endings of files that act on their own were left
/// out, when it is not [`kept_name`]'s.
fn kept_name_before(id: &[u8; 32], name: &str) -> Option<String> {
    plain_ending(name)
        .filter(|e| active_ending(e))
        .map(|ending| format!("{}.{ending}", hex::encode(id)))
}

/// The ending of `name` in lowercase, if it is one to eight letters and digits.
fn plain_ending(name: &str) -> Option<String> {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| (1..=8).contains(&e.len()) && e.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// Where plain copies of sealed attachments are made to play them.
const OPEN_DIR: &str = "open";

/// How long the copy of a damaged journal stays, if nothing rewrites the chat meanwhile.
const DAMAGED_KEPT: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// The id of an attachment kept at `path`: its name up to the ending, in hex.
fn file_id_of(path: &Path) -> Option<FileId> {
    let name = path.file_name()?.to_str()?;
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    hex::decode(stem).ok()?.try_into().ok()
}

/// A temporary file next to `path`, of the form [`write_private`] uses, so a crash's
/// leftovers go at the next start.
fn temp_next_to(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    path.with_file_name(name)
}

/// A new file readable by its owner only (on Unix).
fn new_private(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// The journal and the pins, by name; records of the earlier form were sealed for these.
const JOURNAL: &str = "journal";
const PINS: &str = "pins";
const PROFILE: &str = "profile";
const TAILS: &str = "tails";
const SUMMARY: &str = "summary";

/// What a chat's summary is sealed for: the chat, so it cannot be moved to another one.
fn summary_context(chat: &str) -> String {
    format!("{SUMMARY}/{chat}")
}

/// How a journal or pins file ended when a summary was written: its length and its last
/// bytes (a sealed one's last tag), none when it is not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct End {
    len: u64,
    #[serde(with = "hex_array")]
    last: [u8; TAG_LEN],
}

fn end_of(path: &Path) -> io::Result<End> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(End {
                len: 0,
                last: [0; TAG_LEN],
            });
        }
        Err(e) => return Err(e),
    };
    let len = file.metadata()?.len();
    let mut last = [0u8; TAG_LEN];
    let take = len.min(TAG_LEN as u64) as usize;
    file.seek(SeekFrom::Start(len - take as u64))?;
    file.read_exact(&mut last[TAG_LEN - take..])?;
    Ok(End { len, last })
}

/// The sealed form a journal or pins file starts with, if any.
fn chain_form(path: &Path) -> Option<Form> {
    let mut header = [0u8; HEADER_LEN];
    fs::File::open(path).ok()?.read_exact(&mut header).ok()?;
    sealed_form(&header)
}

/// A chat's summary as kept, with how its journal and pins ended then and the records the
/// two held.
#[derive(Serialize, Deserialize)]
struct KeptSummary {
    summary: Summary,
    journal: End,
    pins: End,
    records: usize,
}

/// What a chat's tails are sealed for: the chat, so they cannot be moved to another one.
fn tails_context(chat: &str) -> String {
    format!("{TAILS}/{chat}")
}

/// What is judged of a chat's group log is sealed for: the chat (see [`read_judged`]).
fn judged_context(chat: &str) -> String {
    format!("judged/direct/{chat}")
}

/// A chat's name in the folder: its friend's device key in hex.
fn chat_name(friend: &DevicePublicKey) -> String {
    hex::encode(friend.as_bytes())
}

/// What the records of a chat's journal or pins (`file`) are sealed for: the file and the
/// chat, so they cannot be moved to another chat either.
fn chain_context(file: &str, chat: &str) -> String {
    format!("{file}/{chat}")
}

/// A chat's journal or pins (`file`) as they are on the disk (`bytes`, sealed with `key` or
/// plain), in the form a folder without a key keeps them: each record after its length.
/// Records past damage are left out, as loading leaves them.
pub(crate) fn plain_records(
    key: Option<&DataKey>,
    file: &str,
    chat: &str,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    let (records, _) = decode_records(key, file, chat, bytes)?;
    let mut plain = Records::whole(None, chain_context(file, chat));
    for record in &records {
        plain.push(record)?;
    }
    Ok(plain.bytes)
}

/// Records in the form [`plain_records`] gives, as the file holds them in a folder sealed
/// with `key` (or plain without one). Records that do not read are refused.
pub(crate) fn sealed_records(
    key: Option<&DataKey>,
    file: &str,
    chat: &str,
    plain: &[u8],
) -> Result<Vec<u8>> {
    let (records, damaged) = decode_records(None, file, chat, plain)?;
    if damaged {
        bail!("the records of {file} do not read");
    }
    let mut sealed = Records::whole(key, chain_context(file, chat));
    for record in &records {
        sealed.push(record)?;
    }
    Ok(sealed.bytes)
}

/// The names of the files of a chat's folder that are sealed in a folder with a key: the
/// journal and the pins (chains), the friend's profile (whole), and how the two end.
pub(crate) const CHAT_CHAINS: [&str; 2] = [JOURNAL, PINS];
pub(crate) const CHAT_PROFILE: &str = PROFILE;
pub(crate) const CHAT_TAILS: &str = TAILS;
pub(crate) const CHAT_SUMMARY: &str = SUMMARY;
pub(crate) const CHAT_OPEN: &str = OPEN_DIR;

/// Records written one after the other for a journal or pins: each after its length, sealed
/// as a chain for `context` when there is a key.
struct Records<'a> {
    key: Option<&'a DataKey>,
    context: String,
    /// What the next record is bound to: the tag of the last one.
    before: [u8; TAG_LEN],
    bytes: Vec<u8>,
    /// Records pushed.
    count: usize,
}

impl<'a> Records<'a> {
    /// A whole file: the chain's header when there is a key, then the records.
    fn whole(key: Option<&'a DataKey>, context: String) -> Self {
        let mut records = Self::after(key, context, CHAIN_START);
        if key.is_some() {
            records.bytes.extend_from_slice(&chain_header());
        }
        records
    }

    /// Records that go after those of a file whose last record left `before`.
    fn after(key: Option<&'a DataKey>, context: String, before: [u8; TAG_LEN]) -> Self {
        Self {
            key,
            context,
            before,
            bytes: Vec::new(),
            count: 0,
        }
    }

    /// How a whole file of these records ends.
    fn tail(&self) -> Option<Tail> {
        (self.count > 0).then_some(Tail {
            records: self.count,
            tag: self.before,
        })
    }

    fn push(&mut self, record: &Record) -> Result<()> {
        let json = zeroize::Zeroizing::new(serde_json::to_vec(record)?);
        let bytes = match self.key {
            Some(key) => {
                let sealed = key.seal_link(&self.context, &self.before, &json)?;
                self.before = chain_link(&sealed).expect("a box ends with its tag");
                sealed
            }
            None => json.to_vec(),
        };
        self.bytes
            .extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        self.bytes.extend_from_slice(&bytes);
        self.count += 1;
        Ok(())
    }
}

/// The records of a chat's journal or pins (`file`), sealed or plain, up to the first one
/// that cannot be read, and whether one could not. Sealed records without a key do not read
/// at all.
pub(crate) fn decode_records(
    key: Option<&DataKey>,
    file: &str,
    chat: &str,
    bytes: &[u8],
) -> Result<(Vec<Record>, bool)> {
    read_records(key, file, chat, bytes).map(|(records, _, broken)| (records, broken))
}

/// [`decode_records`], with the tag of each record read when they are a sealed chain.
fn read_records(
    key: Option<&DataKey>,
    file: &str,
    chat: &str,
    bytes: &[u8],
) -> Result<(Vec<Record>, Vec<[u8; TAG_LEN]>, bool)> {
    let (key, mut rest) = match sealed_form(bytes) {
        None => (None, bytes),
        Some(form @ (Form::Records | Form::Chain)) => match key {
            Some(key) => (Some((key, form)), &bytes[HEADER_LEN..]),
            None => bail!("sealed, and this folder has no key"),
        },
        Some(_) => return Ok((Vec::new(), Vec::new(), true)),
    };
    let context = chain_context(file, chat);
    let mut before = CHAIN_START;
    let max = MAX_RECORD_BYTES + if key.is_some() { BOX_OVERHEAD } else { 0 };
    let mut out = Vec::new();
    let mut tags = Vec::new();
    while !rest.is_empty() {
        let Some((len, tail)) = rest.split_at_checked(4) else {
            return Ok((out, tags, true));
        };
        let len = u32::from_be_bytes(len.try_into().expect("4 bytes")) as usize;
        if len > max {
            return Ok((out, tags, true));
        }
        let Some((record, tail)) = tail.split_at_checked(len) else {
            return Ok((out, tags, true));
        };
        let opened = match key {
            Some((key, Form::Chain)) => match key.open_link(&context, &before, record) {
                Ok(plain) => {
                    before = chain_link(record).expect("a box ends with its tag");
                    tags.push(before);
                    Some(plain)
                }
                Err(_) => return Ok((out, tags, true)),
            },
            Some((key, _)) => match key.open(Form::Records, file, record) {
                Ok(plain) => Some(plain),
                Err(_) => return Ok((out, tags, true)),
            },
            None => None,
        };
        let record = opened.as_deref().map_or(record, |plain| &plain[..]);
        let Ok(record) = serde_json::from_slice(record) else {
            tags.pop();
            return Ok((out, tags, true));
        };
        out.push(record);
        rest = tail;
    }
    Ok((out, tags, false))
}

/// Seals what is plain among the direct chats of the data folder `data_dir`: each chat's
/// journal, pins and friend's profile, and copies of a damaged journal or pins put aside.
/// `back_up` gets each plain file before it is sealed. Each is read back once sealed.
pub(crate) fn seal_plain(
    data_dir: &Path,
    key: &DataKey,
    back_up: &dyn Fn(&Path, &[u8]) -> Result<()>,
) -> Result<()> {
    let Ok(chats) = fs::read_dir(data_dir.join("direct")) else {
        return Ok(());
    };
    for chat in chats.flatten() {
        let dir = chat.path();
        if !dir.is_dir() {
            continue;
        }
        let chat = chat.file_name().to_string_lossy().into_owned();
        // A plain summary holds the chat's last line: it goes, and is made again sealed.
        let summary = dir.join(SUMMARY);
        if read_plain(&summary)?.is_some() {
            fs::remove_file(&summary)
                .with_context(|| format!("cannot remove {}", summary.display()))?;
        }
        for name in [JOURNAL, PINS] {
            let path = dir.join(name);
            let Some(plain) = read_plain(&path)? else {
                continue;
            };
            back_up(&path, &plain)?;
            let (records, damaged) = decode_records(None, name, &chat, &plain)?;
            if damaged {
                let aside = dir.join(format!("{name}.damaged"));
                if !aside.exists() {
                    write_private(&aside, &key.seal_whole(DAMAGED, &plain)?)?;
                }
            }
            let mut sealed = Records::whole(Some(key), chain_context(name, &chat));
            for record in &records {
                sealed.push(record)?;
            }
            write_private(&path, &sealed.bytes)?;
            let back = fs::read(&path)
                .ok()
                .and_then(|b| decode_records(Some(key), name, &chat, &b).ok());
            if back != Some((records, false)) {
                write_private(&path, &plain)?;
                bail!("{} did not read back once sealed", path.display());
            }
        }
        for entry in fs::read_dir(&dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let context = if name == PROFILE {
                PROFILE
            } else if name.starts_with("journal.damaged") || name.starts_with("pins.damaged") {
                DAMAGED
            } else {
                continue;
            };
            let path = entry.path();
            let Some(plain) = read_plain(&path)? else {
                continue;
            };
            back_up(&path, &plain)?;
            write_private(&path, &key.seal_whole(context, &plain)?)?;
            let back = fs::read(&path)
                .ok()
                .and_then(|b| key.open_whole(context, &b).ok());
            if back.as_ref().map(|b| &b[..]) != Some(&plain[..]) {
                write_private(&path, &plain)?;
                bail!("{} did not read back once sealed", path.display());
            }
        }
    }
    Ok(())
}

/// Checks that what [`seal_plain`] seals in the direct chats of the data folder `data_dir`
/// is sealed and opens with `key`: each journal and pins as a chain whose records open (a
/// damaged end, as a crash cut short leaves, is set right when the chat is read), each
/// friend's profile whole; copies of a damaged journal or pins only need to be sealed.
pub(crate) fn check_sealed(data_dir: &Path, key: &DataKey) -> Result<()> {
    let Ok(chats) = fs::read_dir(data_dir.join("direct")) else {
        return Ok(());
    };
    for chat in chats.flatten() {
        let dir = chat.path();
        if !dir.is_dir() {
            continue;
        }
        let chat = chat.file_name().to_string_lossy().into_owned();
        for name in [JOURNAL, PINS] {
            let path = dir.join(name);
            let Some(bytes) = sealed_bytes(&path)? else {
                continue;
            };
            let (records, damaged) = decode_records(Some(key), name, &chat, &bytes)?;
            if damaged && records.is_empty() && bytes.len() > HEADER_LEN {
                return Err(crate::store::DoesNotOpen(path).into());
            }
        }
        for entry in fs::read_dir(&dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if name == PROFILE {
                if let Some(bytes) = sealed_bytes(&path)? {
                    key.open_whole(PROFILE, &bytes)
                        .map_err(|_| crate::store::DoesNotOpen(path.clone()))?;
                }
            } else if name.starts_with("journal.damaged") || name.starts_with("pins.damaged") {
                sealed_bytes(&path)?;
            }
        }
    }
    Ok(())
}

/// What the file at `path` holds, if it is there, holds something and is not sealed.
fn read_plain(path: &Path) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>> {
    match fs::read(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        Ok(bytes) if bytes.is_empty() || sealed_form(&bytes).is_some() => Ok(None),
        Ok(bytes) => Ok(Some(zeroize::Zeroizing::new(bytes))),
    }
}

#[cfg(test)]
mod tests {
    use fogata_crypto::DeviceIdentity;

    use std::{io::Seek, time::SystemTime};

    use super::*;
    use crate::direct::model::{Body, FileKind, Item, Local};

    fn item(id: u8, mine: bool, files: Vec<Attachment>) -> Item {
        Item {
            id: [id; 16],
            mine,
            sent_ms: u64::from(id) * 10,
            got_ms: u64::from(id) * 10,
            seq: u64::from(id),
            at_ms: u64::from(id) * 10,
            body: Body::Message {
                text: format!("message {id}"),
                files,
            },
            ttl_ms: 0,
            delivered: false,
        }
    }

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-direct-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// A small random number source, the same every run for a seed.
    struct Dice(u64);

    impl Dice {
        fn next(&mut self, below: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % below.max(1)
        }

        fn chance(&mut self, in_ten: u64) -> bool {
            self.next(10) < in_ten
        }
    }

    /// A chat changed as the core changes it, its records written as the core writes them
    /// (added, or the two written whole when they grew or something went), and its summary
    /// kept after them.
    struct Written<'a> {
        store: &'a DirectStore,
        friend: DevicePublicKey,
        chat: Chat,
        ended: Option<u64>,
    }

    impl Written<'_> {
        fn add(&mut self, records: &[Record]) {
            for record in records {
                record.apply(&mut self.chat, &mut self.ended);
            }
            let whole = self
                .store
                .append(&self.friend, records, &self.chat)
                .unwrap()
                || records.iter().any(|r| matches!(r, Record::Gone { .. }));
            if whole {
                self.store
                    .rewrite(&self.friend, &self.chat, self.ended)
                    .unwrap();
            }
            self.store
                .write_summary(&self.friend, &Summary::of(&self.chat, self.ended))
                .unwrap();
        }

        /// The summary of the chat read whole from the disk.
        fn read_whole(&self, dir: &Path, key: Option<&DataKey>) -> Summary {
            let loaded = DirectStore::open_sealed(dir, key.cloned())
                .load(&self.friend)
                .unwrap();
            Summary::of(&loaded.chat, loaded.ended_ms)
        }
    }

    /// Random changes of a chat, as the core makes them: messages both ways with files,
    /// deliveries, reads, files that come or go, played voice messages, files that were not
    /// what they said, charges, pins and unpins, deletions, the two no longer friends and
    /// again. After each, the summary kept is the one the chat read whole gives, plain and
    /// sealed.
    #[test]
    fn the_summary_kept_is_always_the_chat_s_read_whole() {
        let now = crate::now_ms();
        for (seed, key) in [
            (7, None),
            (11, Some(DataKey::generate().unwrap())),
            (23, Some(DataKey::generate().unwrap())),
        ] {
            let dir = folder(&format!("summary-{seed}"));
            let store = DirectStore::open_sealed(&dir, key.clone());
            let mut chat = Written {
                store: &store,
                friend: DeviceIdentity::generate().unwrap().public_key(),
                chat: Chat::default(),
                ended: None,
            };
            let mut dice = Dice(seed);
            let mut ids: Vec<ItemId> = Vec::new();
            let mut files: Vec<FileId> = Vec::new();
            for step in 0..300u64 {
                let pick = |dice: &mut Dice, list: &[[u8; 16]]| {
                    (!list.is_empty()).then(|| list[dice.next(list.len() as u64) as usize])
                };
                let records = match dice.next(12) {
                    0..=3 => {
                        let mut id = [0u8; 16];
                        id[..8].copy_from_slice(&step.to_be_bytes());
                        id[8] = 1;
                        let mine = dice.chance(5);
                        let attached: Vec<Attachment> = (0..dice.next(3))
                            .map(|n| {
                                let file = if !files.is_empty() && dice.chance(3) {
                                    files[dice.next(files.len() as u64) as usize]
                                } else {
                                    let mut f = [0u8; 32];
                                    f[..8].copy_from_slice(&step.to_be_bytes());
                                    f[8] = n as u8;
                                    files.push(f);
                                    f
                                };
                                Attachment {
                                    id: file,
                                    name: format!(
                                        "f{}.{}",
                                        dice.next(3),
                                        ["jpg", "ogg", "pdf"][n as usize % 3]
                                    ),
                                    size: 1 + dice.next(20 << 20),
                                    kind: [FileKind::Picture, FileKind::File, FileKind::Video]
                                        [dice.next(3) as usize],
                                    video: None,
                                    local: [Local::Missing, Local::Here, Local::Removed]
                                        [dice.next(3) as usize],
                                    voice: dice.chance(2).then(|| super::super::model::VoiceInfo {
                                        duration_ms: 1000,
                                        bars: vec![1; 40],
                                        played: false,
                                    }),
                                }
                            })
                            .collect();
                        ids.push(id);
                        vec![Record::Item {
                            item: Item {
                                id,
                                mine,
                                sent_ms: now - 1000 + step,
                                got_ms: now - 1000 + step,
                                seq: chat.chat.next_seq(),
                                at_ms: now - 1000 + step,
                                body: Body::Message {
                                    text: format!("message {step}"),
                                    files: attached,
                                },
                                ttl_ms: if dice.chance(3) { model_day() } else { 0 },
                                delivered: !mine,
                            },
                        }]
                    }
                    4 => pick(&mut dice, &ids)
                        .map(|id| vec![Record::Delivered { ids: vec![id] }])
                        .unwrap_or_default(),
                    5 => vec![Record::Read {
                        seq: dice.next(chat.chat.next_seq()),
                    }],
                    6 if !files.is_empty() => vec![Record::File {
                        file: files[dice.next(files.len() as u64) as usize],
                        local: [Local::Missing, Local::Here, Local::Removed][dice.next(3) as usize],
                    }],
                    7 if !files.is_empty() => {
                        let file = files[dice.next(files.len() as u64) as usize];
                        vec![
                            [
                                Record::Played { file },
                                Record::NotVoice { file },
                                Record::NotPicture { file },
                            ][dice.next(3) as usize]
                                .clone(),
                        ]
                    }
                    8 => vec![Record::FileCharge {
                        at_ms: now - dice.next(1000),
                        bytes: dice.next(1 << 20),
                        file: [step as u8; 32],
                    }],
                    9 => pick(&mut dice, &ids)
                        .map(|target| {
                            let mut id = [0u8; 16];
                            id[..8].copy_from_slice(&step.to_be_bytes());
                            id[8] = 2;
                            vec![Record::Item {
                                item: Item {
                                    id,
                                    mine: dice.chance(5),
                                    sent_ms: now + step,
                                    got_ms: now,
                                    seq: chat.chat.next_seq(),
                                    at_ms: now,
                                    body: Body::Pin {
                                        target,
                                        pinned: dice.chance(7),
                                    },
                                    ttl_ms: 0,
                                    delivered: dice.chance(5),
                                },
                            }]
                        })
                        .unwrap_or_default(),
                    10 => pick(&mut dice, &ids)
                        .map(|id| {
                            ids.retain(|i| *i != id);
                            vec![Record::Gone { ids: vec![id] }]
                        })
                        .unwrap_or_default(),
                    _ => vec![Record::Ended {
                        ms: dice.chance(5).then_some(now),
                    }],
                };
                if records.is_empty() {
                    continue;
                }
                chat.add(&records);
                assert_eq!(
                    store.summary(&chat.friend),
                    Some(chat.read_whole(&dir, key.as_ref())),
                    "seed {seed}, step {step}: {records:?}"
                );
            }
            assert!(chat.chat.items().len() > 20);
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    fn model_day() -> u64 {
        DAY_MS
    }

    /// A summary that does not stand for the journal and pins as they are is not taken, and
    /// the chat is read whole: records added after it (a stop between the two writes), the
    /// journal cut short or put back older, the summary of an earlier state or another
    /// chat's, a damaged one, the tails not as the summary says.
    #[test]
    fn a_summary_that_does_not_fit_the_journal_is_not_taken() {
        for key in [None, Some(DataKey::generate().unwrap())] {
            let dir = folder(&format!("summary-stale-{}", key.is_some()));
            let store = DirectStore::open_sealed(&dir, key.clone());
            let friend = DeviceIdentity::generate().unwrap().public_key();
            let other = DeviceIdentity::generate().unwrap().public_key();
            let mut chat = Written {
                store: &store,
                friend,
                chat: Chat::default(),
                ended: None,
            };
            let mut other_chat = Written {
                store: &store,
                friend: other,
                chat: Chat::default(),
                ended: None,
            };
            for n in 1..=3 {
                chat.add(&[Record::Item {
                    item: item(n, false, vec![]),
                }]);
                other_chat.add(&[Record::Item {
                    item: item(n, false, vec![]),
                }]);
            }
            let summary_path = store.summary_path(&friend);
            let journal_path = store.journal_path(&friend);
            let kept = fs::read(&summary_path).unwrap();
            let journal = fs::read(&journal_path).unwrap();
            assert!(store.summary(&friend).is_some());
            // Records added, the stop before the summary.
            let record = Record::Item {
                item: item(4, false, vec![]),
            };
            record.apply(&mut chat.chat, &mut None);
            store.append(&friend, &[record], &chat.chat).unwrap();
            assert_eq!(
                store.summary(&friend),
                None,
                "a summary older than the journal"
            );
            // The journal cut short, or put back as an older copy, under a newer summary.
            store
                .write_summary(&friend, &Summary::of(&chat.chat, None))
                .unwrap();
            assert!(store.summary(&friend).is_some());
            let whole = fs::read(&journal_path).unwrap();
            fs::write(&journal_path, &whole[..whole.len() - 3]).unwrap();
            assert_eq!(store.summary(&friend), None, "a journal cut short");
            fs::write(&journal_path, &journal).unwrap();
            assert_eq!(store.summary(&friend), None, "an older journal");
            fs::write(&journal_path, &whole).unwrap();
            assert!(store.summary(&friend).is_some());
            // An older summary, another chat's, a damaged one, none.
            fs::write(&summary_path, &kept).unwrap();
            assert_eq!(store.summary(&friend), None, "an older summary");
            fs::copy(store.summary_path(&other), &summary_path).unwrap();
            assert_eq!(store.summary(&friend), None, "another chat's summary");
            fs::write(&summary_path, b"{").unwrap();
            assert_eq!(store.summary(&friend), None, "a damaged summary");
            fs::remove_file(&summary_path).unwrap();
            assert_eq!(store.summary(&friend), None, "no summary");
            // Written again from the chat read whole, as a start does, it is taken.
            let opened = DirectStore::open_sealed(&dir, key.clone());
            let loaded = opened.load(&friend).unwrap();
            assert_eq!(loaded.chat.items().len(), 4);
            opened
                .write_summary(&friend, &Summary::of(&loaded.chat, None))
                .unwrap();
            assert_eq!(
                DirectStore::open_sealed(&dir, key.clone()).resume(&friend),
                Some(Summary::of(&loaded.chat, None))
            );
            if key.is_some() {
                // Sealed: tails that say otherwise (another end kept) are not passed over.
                let tails = store.tails_path(&friend);
                let kept_tails = fs::read(&tails).unwrap();
                fs::copy(store.tails_path(&other), &tails).unwrap();
                assert_eq!(store.summary(&friend), None, "tails of another chat");
                fs::write(&tails, &kept_tails).unwrap();
                assert!(store.summary(&friend).is_some());
            } else {
                // Plain, a summary sealed is no summary.
                fs::write(
                    &summary_path,
                    DataKey::generate().unwrap().seal_whole("x", b"{}").unwrap(),
                )
                .unwrap();
                assert_eq!(store.summary(&friend), None);
            }
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// What a failed write leaves is cut off the journal with the handle that wrote it, which
    /// Windows allows only to a handle not opened for appending.
    #[test]
    fn a_journal_handle_can_cut_the_file_and_is_not_an_appending_one() {
        use std::io::Write;
        let dir = folder("cut");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("journal");
        let mut file = open_records(&path).unwrap();
        file.write_all(b"abcdef").unwrap();
        // Not in append mode: a write lands where the handle is.
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(b"X").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"Xbcdef");
        file.set_len(3).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"Xbc");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The ending of a name that a peer gives is kept only when it is a plain one: a `:` in it
    /// would start a stream of the file on Windows, and a path separator would leave the folder.
    #[test]
    fn only_a_plain_ending_is_kept() {
        let id = [7u8; 32];
        let hex = hex::encode(id);
        assert_eq!(kept_name(&id, "Meeting 10.30.MP4"), format!("{hex}.mp4"));
        for odd in [
            "clip.mp4:30",
            "clip.m:p4",
            "clip.mp4 ",
            "clip.a/b",
            "clip.",
            "clip",
        ] {
            assert_eq!(kept_name(&id, odd), hex, "{odd:?}");
        }
        assert_eq!(kept_name(&id, "clip.mp4:30").find(':'), None);
    }

    /// A file that acts on its own when a file manager shows it is kept without its ending,
    /// whatever its case; one kept with it before moves to the name kept now.
    #[test]
    fn a_file_that_acts_on_its_own_is_kept_without_its_ending() {
        let id = [7u8; 32];
        let hex = hex::encode(id);
        for name in [
            "share.url",
            "a.LNK",
            "start.Desktop",
            "run.command",
            "x.webloc",
        ] {
            assert_eq!(kept_name(&id, name), hex, "{name}");
            assert!(kept_name_before(&id, name).is_some(), "{name}");
        }
        assert_eq!(kept_name_before(&id, "lake.jpg"), None);
        assert_eq!(kept_name_before(&id, "odd.p/ng"), None);

        let dir = folder("active");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let link = Attachment {
            id,
            name: "share.URL".into(),
            size: 3,
            kind: FileKind::File,
            video: None,
            local: Local::Here,
            voice: None,
        };
        let files = store.chat_dir(&friend).join("files");
        fs::create_dir_all(&files).unwrap();
        let before = files.join(format!("{hex}.url"));
        fs::write(&before, b"old").unwrap();
        let path = store.file_path(&friend, &link);
        assert_eq!(path, files.join(&hex));
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert!(!before.exists());
        // One left under the name used before goes when the file is removed.
        fs::write(&before, b"old").unwrap();
        store.remove_file(&friend, &id, ["share.URL"]);
        assert!(!before.exists());
        assert!(!path.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_journal_reads_back_what_happened() {
        let dir = folder("journal");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let picture = Attachment {
            id: [5; 32],
            name: "lake.JPG".into(),
            size: 1_200_000,
            kind: FileKind::Picture,
            video: None,
            local: Local::Missing,
            voice: None,
        };
        let mut chat = Chat::default();
        let records = vec![
            Record::Item {
                item: item(1, true, vec![]),
            },
            Record::Item {
                item: item(2, false, vec![picture.clone()]),
            },
            Record::Delivered { ids: vec![[1; 16]] },
            Record::Read { seq: 2 },
            Record::File {
                file: [5; 32],
                local: Local::Here,
            },
            Record::Ended { ms: Some(99) },
        ];
        let mut ended = None;
        for r in &records {
            r.apply(&mut chat, &mut ended);
        }
        assert!(!store.append(&friend, &records, &chat).unwrap());
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat, chat);
        assert_eq!(loaded.ended_ms, Some(99));
        assert!(loaded.chat.get(&[1; 16]).unwrap().delivered);
        assert_eq!(loaded.chat.items()[1].files()[0].local, Local::Here);
        assert_eq!(loaded.chat.unread(), 0);
        assert_eq!(
            store.file_path(&friend, &picture),
            store
                .chat_dir(&friend)
                .join("files")
                .join(format!("{}.jpg", hex::encode([5u8; 32])))
        );
        // A name whose ending is not a plain one gets none.
        let odd = Attachment {
            name: "x.p/ng".into(),
            ..picture
        };
        assert!(store.file_path(&friend, &odd).extension().is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The attachments a friend's chat fetched by themselves stay counted across a restart,
    /// and when the journal is written whole again.
    #[test]
    fn charges_last() {
        let dir = folder("charges");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let now = crate::now_ms();
        let records = [
            Record::Item {
                item: item(1, false, vec![]),
            },
            // As a journal from before files were noted has it.
            Record::VoiceCharge {
                at_ms: now - 2_000,
                bytes: 4 << 20,
                file: None,
            },
            Record::VoiceCharge {
                at_ms: now - 1_000,
                bytes: 3 << 20,
                file: Some([7; 32]),
            },
            Record::FileCharge {
                at_ms: now - 1_000,
                bytes: 9 << 20,
                file: [8; 32],
            },
        ];
        for r in &records {
            r.apply(&mut chat, &mut None);
        }
        store.append(&friend, &records, &chat).unwrap();
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat.voice_charged(now), 7 << 20);
        assert_eq!(loaded.chat.charged(Pool::Files, now), 9 << 20);
        assert!(loaded.chat.charged_for(Pool::Voice, &[7; 32], now));
        assert_eq!(loaded.chat, chat);
        store.rewrite(&friend, &chat, None).unwrap();
        let again = store.load(&friend).unwrap().chat;
        assert_eq!(again.voice_charged(now), 7 << 20);
        assert_eq!(again.charged(Pool::Files, now), 9 << 20);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn going_items_leave_the_file_when_it_is_written_again() {
        let dir = folder("rewrite");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let records = [
            Record::Item {
                item: item(1, true, vec![]),
            },
            Record::Item {
                item: item(2, true, vec![]),
            },
        ];
        for r in &records {
            r.apply(&mut chat, &mut None);
        }
        store.append(&friend, &records, &chat).unwrap();
        chat.remove(&[[1; 16]]);
        store
            .append(&friend, &[Record::Gone { ids: vec![[1; 16]] }], &chat)
            .unwrap();
        store.rewrite(&friend, &chat, None).unwrap();
        let bytes = fs::read(store.journal_path(&friend)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("message 1"));
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_damaged_journal_keeps_what_came_before() {
        let dir = folder("damaged");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let first = [Record::Item {
            item: item(1, false, vec![]),
        }];
        first[0].apply(&mut chat, &mut None);
        store.append(&friend, &first, &chat).unwrap();
        // A crash in the middle of the next record.
        let path = store.journal_path(&friend);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(&[0, 0, 0, 40, b'{']);
        fs::write(&path, &bytes).unwrap();
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat, chat);
        assert!(path.with_file_name("journal.damaged").exists());
        // Written again, it reads cleanly.
        assert!(
            !decode_records(None, JOURNAL, "", &fs::read(&path).unwrap())
                .unwrap()
                .1
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The copy of a damaged journal keeps messages that disappear meanwhile: it goes when the
    /// chat is written again for what disappeared, and after a week anyway.
    #[test]
    fn the_copy_of_a_damaged_journal_does_not_stay() {
        let dir = folder("damaged-copy");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let first = [Record::Item {
            item: item(1, false, vec![]),
        }];
        first[0].apply(&mut chat, &mut None);
        store.append(&friend, &first, &chat).unwrap();
        let path = store.journal_path(&friend);
        let damage = |name: &str| {
            let mut bytes = fs::read(&path).unwrap();
            bytes.extend_from_slice(&[0, 0, 0, 40, b'{']);
            fs::write(&path, &bytes).unwrap();
            store.load(&friend).unwrap();
            path.with_file_name(name)
        };
        // A copy of today stays when the chat is read again, an old one does not.
        let copy = damage("journal.damaged");
        assert!(copy.exists());
        store.load(&friend).unwrap();
        assert!(copy.exists());
        let old = SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        fs::File::options()
            .write(true)
            .open(&copy)
            .unwrap()
            .set_modified(old)
            .unwrap();
        store.load(&friend).unwrap();
        assert!(!copy.exists());

        // Written again for what disappeared, it goes with the journal's own.
        let copy = damage("journal.damaged");
        fs::write(path.with_file_name("pins.damaged"), b"x").unwrap();
        assert!(copy.exists());
        store.drop_damaged(&friend, None);
        assert!(!copy.exists());
        assert!(!path.with_file_name("pins.damaged").exists());
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A full disk can take part of a record. The part must not stay, or every record
    /// added after it would be lost with it the next time the journal is read.
    /// Linux only: macOS lets small appending writes past `ulimit -f`, so no write fails
    /// there and the full disk cannot be played.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_write_a_full_disk_cuts_short_leaves_no_part_behind() {
        const NAME: &str =
            "direct::store::tests::a_write_a_full_disk_cuts_short_leaves_no_part_behind";
        let friend = DeviceIdentity::from_secret_bytes(&[7; 32]).public_key();
        if let Some(dir) = std::env::var_os("FOGATA_TEST_FULL_DISK") {
            // In a process whose files cannot grow past a few KiB, as on a full disk.
            let dir = PathBuf::from(dir);
            let store = DirectStore::open(&dir);
            let mut chat = Chat::default();
            let mut kept = 0;
            for id in 1..=200 {
                let record = [Record::Item {
                    item: item(id, true, vec![]),
                }];
                record[0].apply(&mut chat, &mut None);
                if store.append(&friend, &record, &chat).is_err() {
                    break;
                }
                kept = id;
            }
            assert!(kept > 0 && kept < 200, "kept {kept}");
            // The journal now lacks what the chat holds: the next write is a whole one.
            assert!(store.lagging(&friend));
            assert!(store.append(&friend, &[], &chat).unwrap());
            fs::write(dir.join("kept"), kept.to_string()).unwrap();
            return;
        }
        let dir = folder("full");
        fs::create_dir_all(&dir).unwrap();
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg(
                r#"ulimit -f 4 && trap '' XFSZ && exec "$0" --exact "$1" --test-threads=1 --quiet"#,
            )
            .arg(std::env::current_exe().unwrap())
            .arg(NAME)
            .env("FOGATA_TEST_FULL_DISK", &dir)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        let kept: usize = fs::read_to_string(dir.join("kept"))
            .unwrap()
            .parse()
            .unwrap();
        let store = DirectStore::open(&dir);
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat.items().len(), kept);
        let path = store.journal_path(&friend);
        assert!(!path.with_file_name("journal.damaged").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pins_go_in_their_own_file_that_older_copies_leave_alone() {
        let dir = folder("pins");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let pin = Item {
            id: [9; 16],
            mine: true,
            sent_ms: 30,
            got_ms: 30,
            seq: 3,
            at_ms: 30,
            body: Body::Pin {
                target: [1; 16],
                pinned: true,
            },
            ttl_ms: 0,
            delivered: false,
        };
        let mut chat = Chat::default();
        let records = vec![
            Record::Item {
                item: item(1, false, vec![]),
            },
            Record::Item {
                item: item(2, true, vec![]),
            },
            Record::Item { item: pin.clone() },
        ];
        for r in &records {
            r.apply(&mut chat, &mut None);
        }
        store.append(&friend, &records, &chat).unwrap();
        // Both reach the friend: one receipt, its parts in each file.
        let delivered = Record::Delivered {
            ids: vec![[2; 16], [9; 16]],
        };
        delivered.apply(&mut chat, &mut None);
        store.append(&friend, &[delivered], &chat).unwrap();
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat, chat);
        assert_eq!(loaded.chat.pinned(), vec![[1; 16]]);
        assert!(loaded.chat.get(&[9; 16]).unwrap().delivered);
        // The journal alone, as a copy from before pins reads it: whole, with no pin.
        let (journal, damaged) = decode_records(
            None,
            JOURNAL,
            "",
            &fs::read(store.journal_path(&friend)).unwrap(),
        )
        .unwrap();
        assert!(!damaged);
        assert_eq!(journal.len(), 3);
        assert!(journal.iter().all(|r| match r {
            Record::Item { item } => item.pin_target().is_none(),
            Record::Delivered { ids } => ids == &[[2; 16]],
            _ => false,
        }));
        // Written again whole: still apart; with no pins left, the file goes.
        store.rewrite(&friend, &chat, None).unwrap();
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        assert!(store.pins_path(&friend).exists());
        chat.remove(&[[9; 16]]);
        store.rewrite(&friend, &chat, None).unwrap();
        assert!(!store.pins_path(&friend).exists());
        assert_eq!(store.load(&friend).unwrap().chat.items(), chat.items());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn pins_keep_only_the_last_change_of_each_message() {
        let dir = folder("pins-compact");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let first = [Record::Item {
            item: item(1, false, vec![]),
        }];
        first[0].apply(&mut chat, &mut None);
        store.append(&friend, &first, &chat).unwrap();
        // Pinned and unpinned many times: the file is written again whole on its way, with
        // the last change only.
        let mut whole = false;
        for n in 0..200u64 {
            let pin = Item {
                id: [(n % 250) as u8 + 2; 16],
                mine: true,
                sent_ms: 1_000 + n,
                got_ms: 1_000 + n,
                seq: 0,
                at_ms: 0,
                body: Body::Pin {
                    target: [1; 16],
                    pinned: n % 2 == 0,
                },
                ttl_ms: 0,
                delivered: false,
            };
            let pin = chat.add(pin).unwrap();
            let records = [Record::Item { item: pin }];
            if store.append(&friend, &records, &chat).unwrap() {
                store.rewrite(&friend, &chat, None).unwrap();
                whole = true;
            }
        }
        assert!(whole);
        assert_eq!(chat.items().len(), 2);
        // Written whole now: the pins hold one record, and it reads back the same.
        store.rewrite(&friend, &chat, None).unwrap();
        let (pins, damaged) =
            decode_records(None, PINS, "", &fs::read(store.pins_path(&friend)).unwrap()).unwrap();
        assert!(!damaged);
        assert_eq!(pins.len(), 1);
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn damaged_pins_keep_what_came_before() {
        let dir = folder("pins-damaged");
        let store = DirectStore::open(&dir);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let pin = Item {
            id: [9; 16],
            mine: false,
            sent_ms: 30,
            got_ms: 30,
            seq: 2,
            at_ms: 30,
            body: Body::Pin {
                target: [1; 16],
                pinned: true,
            },
            ttl_ms: 0,
            delivered: true,
        };
        let records = [
            Record::Item {
                item: item(1, false, vec![]),
            },
            Record::Item { item: pin },
        ];
        for r in &records {
            r.apply(&mut chat, &mut None);
        }
        store.append(&friend, &records, &chat).unwrap();
        let path = store.pins_path(&friend);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(&[0, 0, 0, 40, b'{']);
        fs::write(&path, &bytes).unwrap();
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat, chat);
        assert!(path.with_file_name("pins.damaged").exists());
        assert!(
            !decode_records(None, JOURNAL, "", &fs::read(&path).unwrap())
                .unwrap()
                .1
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_sealed_journal_reads_back_and_shows_nothing_of_what_it_holds() {
        let dir = folder("sealed");
        let key = DataKey::generate().unwrap();
        let store = DirectStore::open_sealed(&dir, Some(key.clone()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let pin = Item {
            id: [9; 16],
            mine: true,
            sent_ms: 30,
            got_ms: 30,
            seq: 3,
            at_ms: 30,
            body: Body::Pin {
                target: [1; 16],
                pinned: true,
            },
            ttl_ms: 0,
            delivered: false,
        };
        let mut chat = Chat::default();
        let records = vec![
            Record::Item {
                item: item(1, false, vec![]),
            },
            Record::Item {
                item: item(2, true, vec![]),
            },
            Record::Item { item: pin },
            Record::Read { seq: 2 },
            // A call's line, kept here only, with the name of the one who started it.
            Record::Item {
                item: Item {
                    id: [11; 16],
                    mine: false,
                    sent_ms: 40,
                    got_ms: 40,
                    seq: 4,
                    at_ms: 40,
                    body: Body::Call {
                        lasted_ms: 4_500_000,
                        starter: Some("Marta".into()),
                    },
                    ttl_ms: 0,
                    delivered: true,
                },
            },
        ];
        for r in &records {
            r.apply(&mut chat, &mut None);
        }
        store.append(&friend, &records, &chat).unwrap();
        store.save_profile(&friend, b"Marta's profile").unwrap();
        let loaded = store.load(&friend).unwrap();
        assert_eq!(loaded.chat, chat);
        assert!(matches!(
            loaded.chat.get(&[11; 16]).unwrap().body,
            Body::Call {
                lasted_ms: 4_500_000,
                ..
            }
        ));
        assert_eq!(loaded.profile.as_deref(), Some(&b"Marta's profile"[..]));
        for path in [
            store.journal_path(&friend),
            store.pins_path(&friend),
            store.profile_path(&friend),
        ] {
            let bytes = fs::read(&path).unwrap();
            assert!(sealed_form(&bytes).is_some(), "{}", path.display());
            assert!(!String::from_utf8_lossy(&bytes).contains("message"));
            assert!(!String::from_utf8_lossy(&bytes).contains("Marta"));
        }
        // More at the end, then written whole.
        let more = [Record::Item {
            item: item(4, true, vec![]),
        }];
        more[0].apply(&mut chat, &mut None);
        store.append(&friend, &more, &chat).unwrap();
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        store.rewrite(&friend, &chat, None).unwrap();
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        // Cut short in the middle of a record: what came before stays, and the copy put
        // aside is the sealed file.
        let path = store.journal_path(&friend);
        let mut bytes = fs::read(&path).unwrap();
        let whole = bytes.len();
        bytes.extend_from_slice(&[0, 0, 0, 90, 1, 2, 3]);
        fs::write(&path, &bytes).unwrap();
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        let aside = fs::read(path.with_file_name("journal.damaged")).unwrap();
        assert!(sealed_form(&aside).is_some());
        assert_eq!(fs::read(&path).unwrap().len(), whole);
        // Another key reads nothing, and no key cannot read it at all.
        let other = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        assert!(other.load(&friend).unwrap().chat.items().is_empty());
        assert!(DirectStore::open(&dir).load(&friend).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The records of a sealed journal or pins, each with its length, after the header.
    fn sealed_records(bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut rest = &bytes[HEADER_LEN..];
        let mut out = Vec::new();
        while !rest.is_empty() {
            let len = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
            out.push(rest[..4 + len].to_vec());
            rest = &rest[4 + len..];
        }
        out
    }

    #[test]
    fn sealed_records_moved_on_the_disk_do_not_read() {
        let dir = folder("chain");
        let store = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let mut after = Vec::new();
        // Added one at a time, each to the file as it is then.
        for id in 1..=4 {
            let record = [Record::Item {
                item: item(id, id % 2 == 0, vec![]),
            }];
            record[0].apply(&mut chat, &mut None);
            store.append(&friend, &record, &chat).unwrap();
            after.push(chat.clone());
        }
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        let path = store.journal_path(&friend);
        let whole = fs::read(&path).unwrap();
        assert_eq!(sealed_form(&whole), Some(Form::Chain));
        let records = sealed_records(&whole);
        assert_eq!(records.len(), 4);
        let header = &whole[..HEADER_LEN];
        let [a, b, c, d] = [&records[0], &records[1], &records[2], &records[3]];
        // Taken out at the start or in the middle, repeated, or in another order: what
        // comes before reads, nothing after it does.
        for (changed, kept) in [
            (vec![b, c, d], 0),
            (vec![a, c, d], 1),
            (vec![a, b, b, c, d], 2),
            (vec![a, c, b, d], 1),
        ] {
            let bytes: Vec<u8> = [header]
                .into_iter()
                .chain(changed.iter().map(|r| &r[..]))
                .flatten()
                .copied()
                .collect();
            fs::write(&path, &bytes).unwrap();
            let _ = fs::remove_file(path.with_file_name("journal.damaged"));
            let loaded = store.load(&friend).unwrap().chat;
            let expected = if kept == 0 {
                Chat::default()
            } else {
                after[kept - 1].clone()
            };
            assert_eq!(loaded.items(), expected.items(), "{kept} kept");
            assert!(path.with_file_name("journal.damaged").exists());
        }
        // Another chat's journal does not read as this one's.
        let other = DeviceIdentity::generate().unwrap().public_key();
        fs::create_dir_all(store.chat_dir(&other)).unwrap();
        fs::write(store.journal_path(&other), &whole).unwrap();
        assert!(store.load(&other).unwrap().chat.items().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_journal_cut_short_or_put_back_shows_as_changed() {
        let dir = folder("tails");
        let key = DataKey::generate().unwrap();
        let store = DirectStore::open_sealed(&dir, Some(key.clone()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let mut add = |store: &DirectStore, id: u8| {
            let record = [Record::Item {
                item: item(id, true, vec![]),
            }];
            record[0].apply(&mut chat, &mut None);
            store.append(&friend, &record, &chat).unwrap();
        };
        for id in 1..=3 {
            add(&store, id);
        }
        let path = store.journal_path(&friend);
        let older = fs::read(&path).unwrap();
        for id in 4..=6 {
            add(&store, id);
        }
        let whole = fs::read(&path).unwrap();
        // As written: read again by a new start, nothing changed.
        let again = || DirectStore::open_sealed(&dir, Some(key.clone()));
        let loaded = again().load(&friend).unwrap();
        assert_eq!(loaded.chat.items().len(), 6);
        assert!(!loaded.altered);
        // The last record cut off: every record left reads, and the chat says so.
        let records = sealed_records(&whole);
        let cut = whole.len() - records.last().unwrap().len();
        fs::write(&path, &whole[..cut]).unwrap();
        let loaded = again().load(&friend).unwrap();
        assert_eq!(loaded.chat.items().len(), 5);
        assert!(loaded.altered);
        // Read again, what is there now counts: it is not told twice.
        assert!(!again().load(&friend).unwrap().altered);
        // An older copy of the whole file put back.
        let store = again();
        store.load(&friend).unwrap();
        add(&store, 7);
        fs::write(&path, &older).unwrap();
        let loaded = again().load(&friend).unwrap();
        assert_eq!(loaded.chat.items().len(), 3);
        assert!(loaded.altered);
        // A write whose tails were not kept (the computer stopped in between) is no change:
        // the file holds what the tails said, and more after it.
        let store = again();
        store.load(&friend).unwrap();
        let tails = fs::read(store.tails_path(&friend)).unwrap();
        add(&store, 8);
        fs::write(store.tails_path(&friend), &tails).unwrap();
        assert!(!again().load(&friend).unwrap().altered);
        // Tails that do not read are a change too.
        fs::write(again().tails_path(&friend), b"not tails").unwrap();
        assert!(again().load(&friend).unwrap().altered);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_chat_written_again_whole_reads_as_either_while_it_is_written() {
        let dir = folder("tails-rewrite");
        let key = DataKey::generate().unwrap();
        let store = DirectStore::open_sealed(&dir, Some(key.clone()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        for id in 1..=4 {
            let record = [Record::Item {
                item: item(id, true, vec![]),
            }];
            record[0].apply(&mut chat, &mut None);
            store.append(&friend, &record, &chat).unwrap();
        }
        let before = fs::read(store.journal_path(&friend)).unwrap();
        chat.remove(&[[1; 16], [2; 16]]);
        store.rewrite(&friend, &chat, None).unwrap();
        let again = || DirectStore::open_sealed(&dir, Some(key.clone()));
        assert!(!again().load(&friend).unwrap().altered);
        // Stopped after the tails said what comes, before the journal was written: the old
        // journal reads as it was. Put back once the rewrite is done, it is a change.
        let store = again();
        store.load(&friend).unwrap();
        let mut tails = store.read_tails(&friend).unwrap().unwrap();
        let done = tails.clone();
        tails.next = Some((tails.journal, tails.pins));
        tails.journal = Tail::of(&tags(&key, &friend, &before));
        store.write_tails(&friend, &tails).unwrap();
        fs::write(store.journal_path(&friend), &before).unwrap();
        assert!(!again().load(&friend).unwrap().altered);
        let store = again();
        store.load(&friend).unwrap();
        store.write_tails(&friend, &done).unwrap();
        fs::write(store.journal_path(&friend), &before).unwrap();
        assert!(again().load(&friend).unwrap().altered);
        let _ = fs::remove_dir_all(&dir);
    }

    /// The tags of a sealed journal's records.
    fn tags(key: &DataKey, friend: &DevicePublicKey, bytes: &[u8]) -> Vec<[u8; TAG_LEN]> {
        read_records(Some(key), JOURNAL, &chat_name(friend), bytes)
            .unwrap()
            .1
    }

    #[test]
    fn a_folder_from_before_tails_counts_from_its_first_start() {
        let dir = folder("tails-first");
        let key = DataKey::generate().unwrap();
        let store = DirectStore::open_sealed(&dir, Some(key.clone()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        for id in 1..=3 {
            let record = [Record::Item {
                item: item(id, true, vec![]),
            }];
            record[0].apply(&mut chat, &mut None);
            store.append(&friend, &record, &chat).unwrap();
        }
        fs::remove_file(store.tails_path(&friend)).unwrap();
        let again = || DirectStore::open_sealed(&dir, Some(key.clone()));
        let loaded = again().load(&friend).unwrap();
        assert_eq!(loaded.chat.items().len(), 3);
        assert!(!loaded.altered);
        assert!(again().tails_path(&friend).exists());
        // A plain folder keeps none.
        let plain = folder("tails-plain");
        let store = DirectStore::open(&plain);
        store
            .append(&friend, &[Record::Read { seq: 1 }], &chat)
            .unwrap();
        assert!(!store.tails_path(&friend).exists());
        assert!(!DirectStore::open(&plain).load(&friend).unwrap().altered);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&plain);
    }

    #[test]
    fn records_sealed_each_on_their_own_are_written_again_as_a_chain() {
        let dir = folder("unchained");
        let key = DataKey::generate().unwrap();
        let store = DirectStore::open_sealed(&dir, Some(key.clone()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        // A journal as it was written before chains.
        let mut older = fogata_crypto::at_rest::records_header().to_vec();
        for record in [
            Record::Item {
                item: item(1, false, vec![]),
            },
            Record::Item {
                item: item(2, true, vec![]),
            },
            Record::Read { seq: 2 },
        ] {
            record.apply(&mut chat, &mut None);
            let json = serde_json::to_vec(&record).unwrap();
            let sealed = key.seal(Form::Records, JOURNAL, &json).unwrap();
            older.extend_from_slice(&(sealed.len() as u32).to_be_bytes());
            older.extend_from_slice(&sealed);
        }
        fs::create_dir_all(store.chat_dir(&friend)).unwrap();
        let path = store.journal_path(&friend);
        fs::write(&path, &older).unwrap();
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        assert_eq!(sealed_form(&fs::read(&path).unwrap()), Some(Form::Chain));
        assert!(!path.with_file_name("journal.damaged").exists());
        // It goes on as a chain.
        let more = [Record::Item {
            item: item(3, true, vec![]),
        }];
        more[0].apply(&mut chat, &mut None);
        assert!(!store.append(&friend, &more, &chat).unwrap());
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        assert_eq!(sealed_records(&fs::read(&path).unwrap()).len(), 4);
        // An earlier-form file is never added to: written whole at the next change.
        fs::write(&path, &older).unwrap();
        let fresh = DirectStore::open_sealed(&dir, Some(key));
        assert!(fresh.append(&friend, &more, &chat).is_err());
        assert!(fresh.lagging(&friend));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_plain_journal_in_a_sealed_folder_is_written_whole_sealed_at_the_next_change() {
        let dir = folder("plain-left");
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        let first = [Record::Item {
            item: item(1, false, vec![]),
        }];
        first[0].apply(&mut chat, &mut None);
        DirectStore::open(&dir)
            .append(&friend, &first, &chat)
            .unwrap();
        let store = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        let next = [Record::Item {
            item: item(2, true, vec![]),
        }];
        next[0].apply(&mut chat, &mut None);
        // Not added to the plain file: the two are written whole, sealed.
        assert!(store.append(&friend, &next, &chat).unwrap());
        store.rewrite(&friend, &chat, None).unwrap();
        assert!(sealed_form(&fs::read(store.journal_path(&friend)).unwrap()).is_some());
        assert_eq!(store.load(&friend).unwrap().chat, chat);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// An attachment's copy is kept only when the bytes copied are the file its id names:
    /// one changed after it was hashed is not kept, sealed or plain, and leaves nothing.
    #[test]
    fn a_copy_is_kept_only_of_the_file_hashed() {
        for key in [None, Some(DataKey::generate().unwrap())] {
            let dir = folder("kept-hashed");
            let store = DirectStore::open_sealed(&dir, key);
            let friend = DeviceIdentity::generate().unwrap().public_key();
            fs::create_dir_all(&dir).unwrap();
            let source = dir.join("plan.pdf");
            fs::write(&source, b"SECRET-V2").unwrap();
            let file = Attachment {
                id: *blake3::hash(b"PUBLIC-V1").as_bytes(),
                name: "plan.pdf".into(),
                size: 9,
                kind: FileKind::File,
                video: None,
                local: Local::Here,
                voice: None,
            };
            let files = store.files_dir(&friend).unwrap();
            assert!(store.keep_copy(&friend, &file, &source).is_err());
            assert_eq!(fs::read_dir(&files).unwrap().count(), 0);
            fs::write(&source, b"PUBLIC-V1").unwrap();
            store.keep_copy(&friend, &file, &source).unwrap();
            let mut kept = Vec::new();
            store
                .open_file(&friend, &file)
                .unwrap()
                .0
                .read_to_end(&mut kept)
                .unwrap();
            assert_eq!(kept, b"PUBLIC-V1");
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// Each video played from a plain copy has a copy of its own: closing one player, or
    /// a second view of the same video, never takes the copy another player reads.
    #[test]
    fn each_video_played_has_a_copy_of_its_own() {
        let dir = folder("copies");
        let store = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        fs::create_dir_all(&dir).unwrap();
        let video = |name: &str, bytes: &[u8]| {
            let source = dir.join(name);
            fs::write(&source, bytes).unwrap();
            let file = Attachment {
                id: *blake3::hash(bytes).as_bytes(),
                name: name.into(),
                size: bytes.len() as u64,
                kind: FileKind::Video,
                video: None,
                local: Local::Here,
                voice: None,
            };
            store.files_dir(&friend).unwrap();
            store.keep_copy(&friend, &file, &source).unwrap();
            file
        };
        let (a, b) = (
            video("a.mp4", b"first video"),
            video("b.mp4", b"second video"),
        );
        let play_a = store.open_to_play(&friend, &a).unwrap();
        let play_b = store.open_to_play(&friend, &b).unwrap();
        let again = store.open_to_play(&friend, &a).unwrap();
        assert_ne!(play_a, again);
        assert!(play_a.exists() && play_b.exists() && again.exists());
        assert_eq!(again.extension().unwrap(), "mp4");
        assert_eq!(fs::read(&again).unwrap(), b"first video");
        // The first player closes: its copy goes, the others stay.
        store.forget_copy(&play_a);
        assert!(!play_a.exists() && play_b.exists() && again.exists());
        // Only a copy goes: nothing elsewhere is removed this way.
        let kept = store.file_path(&friend, &b);
        store.forget_copy(&kept);
        store.forget_copy(&dir.join("b.mp4"));
        assert!(kept.exists() && dir.join("b.mp4").exists());
        // At the next start, those left go too.
        store.forget_copies();
        assert!(!play_b.exists() && !again.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A plain copy to play is made only once the disk has room for it (as the core holds
    /// it, see `shares::hold_disk`), and one that stops halfway leaves nothing behind.
    #[test]
    fn a_copy_to_play_needs_room_and_leaves_nothing_half_made() {
        let dir = folder("play-room");
        let store = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        fs::create_dir_all(&dir).unwrap();
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let source = dir.join("trip.mp4");
        fs::write(&source, &bytes).unwrap();
        let file = Attachment {
            id: *blake3::hash(&bytes).as_bytes(),
            name: "trip.mp4".into(),
            size: bytes.len() as u64,
            kind: FileKind::Video,
            video: None,
            local: Local::Here,
            voice: None,
        };
        store.files_dir(&friend).unwrap();
        store.keep_copy(&friend, &file, &source).unwrap();
        let copies = || {
            fs::read_dir(dir.join(OPEN_DIR))
                .map(|d| d.count())
                .unwrap_or(0)
        };
        // No room: refused before anything is written, for the whole file.
        let mut asked = None;
        let refused = store.open_to_play_holding(&friend, &file, |size, _| {
            asked = Some(size);
            Err::<(), _>(anyhow::anyhow!("not enough free space for this file"))
        });
        assert!(format!("{:#}", refused.unwrap_err()).contains("not enough free space"));
        assert_eq!(asked, Some(file.size));
        assert_eq!(copies(), 0);
        // A part of the kept file damaged past the first chunk: the copy stops there, and
        // what was written of it goes.
        let kept = store.file_path(&friend, &file);
        let mut sealed = fs::read(&kept).unwrap();
        let at = sealed.len() - 1000;
        sealed[at] ^= 0x55;
        fs::write(&kept, &sealed).unwrap();
        assert!(
            store
                .open_to_play_holding(&friend, &file, |_, _| Ok(()))
                .is_err()
        );
        assert_eq!(copies(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn attachments_are_kept_sealed_and_read_as_they_were() {
        let dir = folder("attachments");
        let key = DataKey::generate().unwrap();
        let store = DirectStore::open_sealed(&dir, Some(key));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let bytes: Vec<u8> = (0..200_000u32).map(|i| (i % 249) as u8).collect();
        let source = dir.join("holiday.mp4");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&source, &bytes).unwrap();
        let file = Attachment {
            id: *blake3::hash(&bytes).as_bytes(),
            name: "holiday.mp4".into(),
            size: bytes.len() as u64,
            kind: FileKind::Video,
            video: None,
            local: Local::Here,
            voice: None,
        };
        store.files_dir(&friend).unwrap();
        assert!(!store.has_copy(&friend, &file));
        store.keep_copy(&friend, &file, &source).unwrap();
        assert!(store.has_copy(&friend, &file));
        let kept = fs::read(store.file_path(&friend, &file)).unwrap();
        assert!(sealed_form(&kept).is_some());
        assert_eq!(kept.len() as u64, stream_len(file.size));
        let (mut reader, size) = store.open_file(&friend, &file).unwrap();
        assert_eq!(size, file.size);
        let mut back = Vec::new();
        reader.read_to_end(&mut back).unwrap();
        assert_eq!(back, bytes);
        // Saved elsewhere, and played from a plain copy that goes when asked.
        let saved = dir.join("saved.mp4");
        store.copy_out(&friend, &file, &saved).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), bytes);
        let play = store.open_to_play(&friend, &file).unwrap();
        assert!(play.starts_with(store.dir().join(OPEN_DIR)));
        assert_eq!(play.extension().unwrap(), "mp4");
        assert_eq!(fs::read(&play).unwrap(), bytes);
        store.forget_copies();
        assert!(!play.exists());
        assert!(store.plain_files().is_empty());

        // One kept before the folder was sealed: read as it is, and sealed in its place.
        let old = Attachment {
            id: *blake3::hash(b"an older picture").as_bytes(),
            name: "photo.png".into(),
            size: 16,
            kind: FileKind::Picture,
            video: None,
            local: Local::Here,
            voice: None,
        };
        let old_path = store.file_path(&friend, &old);
        fs::write(&old_path, b"an older picture").unwrap();
        assert_eq!(store.open_to_play(&friend, &old).unwrap(), old_path);
        assert_eq!(store.plain_files(), vec![old_path.clone()]);
        store
            .seal_file(&old_path, |sealed| fs::rename(sealed, &old_path))
            .unwrap();
        assert!(sealed_form(&fs::read(&old_path).unwrap()).is_some());
        let (mut reader, _) = store.open_file(&friend, &old).unwrap();
        let mut back = Vec::new();
        reader.read_to_end(&mut back).unwrap();
        assert_eq!(back, b"an older picture");
        assert!(store.plain_files().is_empty());
        // A plain file that only starts like a sealed one is still read as it is.
        let tricky_bytes = b" fogata\x03 and then anything".to_vec();
        let tricky = Attachment {
            id: *blake3::hash(&tricky_bytes).as_bytes(),
            name: "note.txt".into(),
            size: tricky_bytes.len() as u64,
            kind: FileKind::File,
            video: None,
            local: Local::Here,
            voice: None,
        };
        fs::write(store.file_path(&friend, &tricky), &tricky_bytes).unwrap();
        let (mut reader, _) = store.open_file(&friend, &tricky).unwrap();
        let mut back = Vec::new();
        reader.read_to_end(&mut back).unwrap();
        assert_eq!(back, tricky_bytes);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Once every attachment was found sealed, asking again reads none of them: what comes
    /// later is kept sealed. While one is plain, each asking looks again.
    #[test]
    fn attachments_found_sealed_are_not_read_again() {
        use std::sync::atomic::Ordering;
        let dir = std::env::temp_dir().join(format!("fogata-plain-once-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let files = store.files_dir(&friend).unwrap();
        let bytes = b"a picture".repeat(100);
        let id = *blake3::hash(&bytes).as_bytes();
        // Kept plain before the folder was sealed.
        fs::write(files.join(hex::encode(id)), &bytes).unwrap();
        assert!(store.holds_plain_file());
        assert!(store.holds_plain_file());
        assert_eq!(store.plain_looks.load(Ordering::Relaxed), 2);
        // Sealed now: told once, then not looked at again.
        fs::remove_file(files.join(hex::encode(id))).unwrap();
        let file = Attachment {
            id,
            name: "photo.jpg".into(),
            size: bytes.len() as u64,
            kind: FileKind::Picture,
            video: None,
            local: Local::Here,
            voice: None,
        };
        store.keep_bytes(&friend, &file, &bytes).unwrap();
        assert!(!store.holds_plain_file());
        assert!(!store.holds_plain_file());
        assert!(!store.holds_plain_file());
        assert_eq!(store.plain_looks.load(Ordering::Relaxed), 3);
        // One that cannot be read leaves the answer open: looked at again next time.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let other = DeviceIdentity::generate().unwrap().public_key();
            let others = store.files_dir(&other).unwrap();
            let fresh = DirectStore::open_sealed(&dir, store.key.clone());
            let locked = others.join(hex::encode([9u8; 32]));
            fs::write(&locked, b"x").unwrap();
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
            if fs::File::open(&locked).is_err() {
                assert!(!fresh.holds_plain_file());
                assert!(!fresh.holds_plain_file());
                assert_eq!(fresh.plain_looks.load(Ordering::Relaxed), 2);
            }
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What looking whether attachments are still plain costs once a folder is sealed
    /// (`Core::at_rest`, once per start of the application), with 5 000 attachments in 50
    /// chats: `cargo test --release -p fogata-core --lib cost_of_telling_plain_files --
    /// --ignored --nocapture`, in `FOGATA_COST_DIR` (on a real disk) when set.
    #[test]
    #[ignore]
    fn cost_of_telling_plain_files() {
        use std::time::Instant;
        let dir = std::env::var_os("FOGATA_COST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("fogata-cost-plain-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = DirectStore::open_sealed(&dir, Some(DataKey::generate().unwrap()));
        for chat in 0..50u32 {
            let friend = DeviceIdentity::generate().unwrap().public_key();
            store.files_dir(&friend).unwrap();
            for n in 0..100u32 {
                let bytes = [chat.to_be_bytes(), n.to_be_bytes()].concat().repeat(1000);
                let file = Attachment {
                    id: *blake3::hash(&bytes).as_bytes(),
                    name: "photo.jpg".into(),
                    size: bytes.len() as u64,
                    kind: FileKind::Picture,
                    video: None,
                    local: Local::Here,
                    voice: None,
                };
                store.keep_bytes(&friend, &file, &bytes).unwrap();
            }
        }
        let start = Instant::now();
        assert!(!store.holds_plain_file());
        let took = start.elapsed();
        println!("5 000 sealed attachments looked at in {took:?}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What reading every chat at a start costs, sealed: 20 chats of 10,000 messages each,
    /// as the core reads them as it starts (the time, and what this process holds more
    /// once they are read, by `ps`).
    /// `cargo test --release -p fogata-core --lib cost_of_many_chats -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cost_of_many_chats() {
        use std::time::Instant;
        let dir = std::env::var_os("FOGATA_COST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("fogata-cost-chats-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let key = DataKey::generate().unwrap();
        let mut chat = Chat::default();
        for n in 0..10_000u32 {
            let mut id = [0u8; 16];
            id[..4].copy_from_slice(&n.to_be_bytes());
            chat.insert(Item {
                id,
                mine: n % 2 == 0,
                sent_ms: u64::from(n) * 1000,
                got_ms: u64::from(n) * 1000,
                seq: u64::from(n),
                at_ms: u64::from(n) * 1000,
                body: Body::Message {
                    text: format!(
                        "message {n}: see you at eight in the square, I'll bring the guitar"
                    ),
                    files: Vec::new(),
                },
                ttl_ms: 0,
                delivered: true,
            });
        }
        let store = DirectStore::open_sealed(&dir, Some(key.clone()));
        for _ in 0..20 {
            let friend = DeviceIdentity::generate().unwrap().public_key();
            store.rewrite(&friend, &chat, None).unwrap();
        }
        drop(chat);
        let rss_kb = || {
            let out = std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<u64>()
                .unwrap()
        };
        let before = rss_kb();
        let start = Instant::now();
        let chats = DirectStore::open_sealed(&dir, Some(key)).chats();
        let took = start.elapsed();
        let after = rss_kb();
        assert_eq!(chats.len(), 20);
        assert!(chats.iter().all(|(_, l)| l.chat.items().len() == 10_000));
        eprintln!(
            "20 sealed chats of 10,000 messages read in {:.0} ms; the process holds {} MB more",
            took.as_secs_f64() * 1000.0,
            after.saturating_sub(before) / 1024
        );
        drop(chats);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What sealing costs: a journal of 10,000 messages written whole, read back and added
    /// to, plain and sealed, and a large attachment kept, read, and sealed in its place.
    /// `cargo test --release -p fogata-core --lib cost_of_sealing -- --ignored --nocapture`,
    /// in `FOGATA_COST_DIR` (on a real disk) when set.
    #[test]
    #[ignore]
    fn cost_of_sealing() {
        use std::time::Instant;
        let base = std::env::var_os("FOGATA_COST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("fogata-cost-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut chat = Chat::default();
        for n in 0..10_000u32 {
            let mut id = [0u8; 16];
            id[..4].copy_from_slice(&n.to_be_bytes());
            chat.insert(Item {
                id,
                mine: n % 2 == 0,
                sent_ms: u64::from(n) * 1000,
                got_ms: u64::from(n) * 1000,
                seq: u64::from(n),
                at_ms: u64::from(n) * 1000,
                body: Body::Message {
                    text: format!(
                        "message {n}: see you at eight in the square, I'll bring the guitar"
                    ),
                    files: Vec::new(),
                },
                ttl_ms: 0,
                delivered: true,
            });
        }
        let ms = |start: Instant| start.elapsed().as_secs_f64() * 1000.0;
        for key in [None, Some(DataKey::generate().unwrap())] {
            let what = if key.is_some() { "sealed" } else { "plain" };
            let dir = base.join(what);
            let store = DirectStore::open_sealed(&dir, key.clone());
            let start = Instant::now();
            store.rewrite(&friend, &chat, None).unwrap();
            let write = ms(start);
            let size = fs::metadata(store.journal_path(&friend)).unwrap().len();
            let start = Instant::now();
            let loaded = DirectStore::open_sealed(&dir, key.clone())
                .load(&friend)
                .unwrap();
            let read = ms(start);
            assert_eq!(loaded.chat.items().len(), 10_000);
            let mut grown = chat.clone();
            let start = Instant::now();
            for n in 0..100u8 {
                let record = Record::Item {
                    item: Item {
                        id: [n; 16],
                        ..item(n, true, vec![])
                    },
                };
                record.apply(&mut grown, &mut None);
                store.append(&friend, &[record], &grown).unwrap();
            }
            let append = ms(start) / 100.0;
            eprintln!(
                "journal of 10,000 messages, {what}: {size} bytes, written whole in {write:.1} ms, \
                 read in {read:.1} ms, one message added in {append:.2} ms"
            );
        }

        let mb: usize = std::env::var("FOGATA_COST_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(512);
        let bytes: Vec<u8> = (0..mb << 20).map(|i| (i as u32 % 251) as u8).collect();
        let source = base.join("big.mkv");
        fs::write(&source, &bytes).unwrap();
        let file = Attachment {
            id: *blake3::hash(&bytes).as_bytes(),
            name: "big.mkv".into(),
            size: bytes.len() as u64,
            kind: FileKind::Video,
            video: None,
            local: Local::Here,
            voice: None,
        };
        drop(bytes);
        let mbs = |start: Instant| mb as f64 / start.elapsed().as_secs_f64();
        for key in [None, Some(DataKey::generate().unwrap())] {
            let what = if key.is_some() { "sealed" } else { "plain" };
            let store = DirectStore::open_sealed(&base.join(what), key.clone());
            store.files_dir(&friend).unwrap();
            let start = Instant::now();
            store.keep_copy(&friend, &file, &source).unwrap();
            let keep = mbs(start);
            let start = Instant::now();
            let (mut kept, _) = store.open_file(&friend, &file).unwrap();
            io::copy(&mut kept, &mut io::sink()).unwrap();
            let read = mbs(start);
            let start = Instant::now();
            let (mut kept, size) = store.open_file(&friend, &file).unwrap();
            kept.seek(io::SeekFrom::Start(size / 2)).unwrap();
            let mut piece = vec![0u8; 64 * 1024];
            kept.read_exact(&mut piece).unwrap();
            let seek = start.elapsed().as_secs_f64() * 1000.0;
            eprintln!(
                "attachment of {mb} MiB, {what}: kept at {keep:.0} MiB/s, read at {read:.0} \
                 MiB/s, 64 KiB from its middle in {seek:.2} ms"
            );
        }
        // Sealing one kept from before, in its place, read back to check it.
        let store =
            DirectStore::open_sealed(&base.join("plain"), Some(DataKey::generate().unwrap()));
        let path = store.file_path(&friend, &file);
        let start = Instant::now();
        store
            .seal_file(&path, |sealed| fs::rename(sealed, &path))
            .unwrap();
        eprintln!(
            "attachment of {mb} MiB sealed in its place (sealed, then read back) at {:.0} MiB/s",
            mbs(start)
        );
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn settings_start_from_the_defaults() {
        let dir = folder("settings");
        let store = DirectStore::open(&dir);
        assert_eq!(store.settings(), Settings::default());
        assert_eq!(store.settings().limit, Some(2_000_000_000));
        let changed = Settings {
            limit: None,
            auto_pictures: false,
            auto_files: true,
            timer_ms: 7 * crate::direct::model::DAY_MS,
        };
        store.save_settings(&changed).unwrap();
        assert_eq!(store.settings(), changed);
        // Missing parts take their defaults.
        fs::write(store.settings_path(), br#"{"auto_files": true}"#).unwrap();
        let read = store.settings();
        assert_eq!(
            (read.limit, read.auto_pictures, read.auto_files),
            (DEFAULT_LIMIT, true, true)
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_chat_keeps_its_group_and_the_friends_profile() {
        let dir = folder("group");
        let store = DirectStore::open(&dir);
        let me = DeviceIdentity::generate().unwrap();
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let mut log = GroupLog::create(&me, "Direct", 1).unwrap();
        log.add(&me, friend, "Marta", 2).unwrap();
        store.save_group(&friend, &log).unwrap();
        store.save_profile(&friend, b"profile").unwrap();
        let loaded = store.load(&friend).unwrap();
        assert_eq!(
            loaded.group.unwrap().hashes_in_order(),
            log.hashes_in_order()
        );
        assert_eq!(loaded.profile.as_deref(), Some(&b"profile"[..]));
        assert_eq!(store.chats().len(), 1);
        store.remove_chat(&friend).unwrap();
        assert!(store.chats().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A chat's group the friend made costly to judge is judged once, not each time the chat
    /// is read, sealed or not; what was judged of it counts only sealed with the folder's key.
    #[test]
    fn a_chat_group_costly_to_judge_is_judged_once() {
        use crate::store::judged_tests::{assert_same, costly_log, judged_from_nothing, timed};
        let friend = DeviceIdentity::generate().unwrap();
        let (log, _) = costly_log(&friend, 600);
        for (what, key) in [
            ("plain", None),
            ("sealed", Some(DataKey::generate().unwrap())),
        ] {
            let dir = folder(&format!("costly-group-{what}"));
            let store = DirectStore::open_sealed(&dir, key.clone());
            store.save_group(&friend.public_key(), &log).unwrap();
            let (_, judging) = judged_from_nothing(&store.group_path(&friend.public_key()));
            // The best of a few reads: what a read costs, without the waits of a computer
            // busy with the other tests (one slow read on CI took 2.4 times less than judging).
            let (loaded, mut took) = timed(|| store.load(&friend.public_key()).unwrap());
            for _ in 0..2 {
                took = took.min(timed(|| store.load(&friend.public_key()).unwrap()).1);
            }
            assert_same(&loaded.group.unwrap(), &log);
            assert!(
                took * 5 < judging,
                "{what}: read in {took:?}, judged in {judging:?}"
            );
            if let Some(key) = &key {
                // Left plain in a sealed folder, they count for nothing.
                let judged = store.judged_path(&friend.public_key());
                let sealed = fs::read(&judged).unwrap();
                let context = judged_context(&chat_name(&friend.public_key()));
                let bytes = key.open_whole(&context, &sealed).unwrap();
                fs::write(&judged, &bytes[..]).unwrap();
                let loaded = store.load(&friend.public_key()).unwrap();
                assert_same(&loaded.group.unwrap(), &log);
                // The log is judged whole, and what was judged kept sealed again.
                assert_ne!(fs::read(&judged).unwrap(), bytes.to_vec());
            }
            fs::remove_dir_all(&dir).unwrap();
        }
    }
}
