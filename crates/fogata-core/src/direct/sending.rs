//! Writing in a direct chat: messages, voice messages and attachments written and sealed
//! for the friend, a chat's timer, pins, and typing signals.

use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use fogata_crypto::{DevicePublicKey, names};
use fogata_net::files;
use fogata_proto::{
    DirectFile, DirectFileKind, DirectPayload, MAX_VOICE_BYTES, MAX_VOICE_MS, VOICE_BARS,
    WireVideo, WireVoice,
};
use fogata_state::{MAX_CHAT_CHARS, MAX_THUMB_BYTES, VideoPreview};

use super::{
    ChatsState, Entry, Undo, clean_file_name, ensure_group, file_kind, flush, free_space,
    model::{
        Attachment, Body, FileKind, Item, ItemId, Local, MAX_FILES, MAX_PINS, VideoInfo, VoiceInfo,
        valid_timer,
    },
    new_id,
    node_events::expire,
    store::Record,
};
use crate::{Core, CoreEvent, Inner, now_ms};

/// The copies of attachments a send uses in the chat's folder, and those it made there. As
/// it ends, whether the message was kept or not, a copy it made that no item of the chat
/// keeps and no other send is using goes: a send that failed (the disk full, the friendship
/// ended meanwhile, a later file too big) left a full copy of each earlier file there,
/// which nothing would ever remove and the room for direct messages did not count.
struct Copies {
    inner: std::sync::Arc<Inner>,
    friend: DevicePublicKey,
    using: Vec<super::model::FileId>,
    made: Vec<Attachment>,
}

impl Copies {
    fn new(inner: &std::sync::Arc<Inner>, friend: DevicePublicKey) -> Self {
        Self {
            inner: inner.clone(),
            friend,
            using: Vec::new(),
            made: Vec::new(),
        }
    }

    /// Counts the file as in use by this send, before its copy is looked for: another send
    /// that fails meanwhile then leaves it.
    fn using(&mut self, file: &super::model::FileId) {
        *self
            .inner
            .direct
            .lock()
            .copying
            .entry((self.friend, *file))
            .or_default() += 1;
        self.using.push(*file);
    }
}

impl Drop for Copies {
    fn drop(&mut self) {
        let mut chats = self.inner.direct.lock();
        for file in &self.using {
            let key = (self.friend, *file);
            if let Some(n) = chats.copying.get_mut(&key) {
                *n -= 1;
                if *n == 0 {
                    chats.copying.remove(&key);
                }
            }
        }
        for made in &self.made {
            let kept = chats.chats.get(&self.friend).is_some_and(|e| {
                e.chat
                    .items()
                    .iter()
                    .flat_map(Item::files)
                    .any(|f| f.id == made.id && f.name == made.name)
            });
            if !kept && !chats.copying.contains_key(&(self.friend, made.id)) {
                tracing::debug!("a copy of a file a send did not keep goes");
                self.inner
                    .direct
                    .store
                    .remove_file(&self.friend, &made.id, [made.name.as_str()]);
            }
        }
    }
}

/// A file to send, with a video's first picture and length as this device read them.
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub path: PathBuf,
    pub video: Option<VideoPreview>,
}

/// A voice message recorded here, to send: an Ogg Opus file with its length and levels.
#[derive(Clone, Debug)]
pub struct VoiceNote {
    pub file: Vec<u8>,
    pub duration_ms: u64,
    pub bars: Vec<u8>,
}

/// The name a voice message's file goes under. Copies from before voice messages show it
/// as a file of that name.
pub const VOICE_NAME: &str = "Voice message.ogg";

/// Whether what a friend says of a voice message can be one: an Ogg file (the name says
/// what plays it) within a voice message's size, of a length from a moment to 15 minutes
/// (and the second the last packet may run over), with its bars.
pub(super) fn fair_voice(name: &str, size: u64, voice: &WireVoice) -> bool {
    name.to_ascii_lowercase().ends_with(".ogg")
        && size > 0
        && size <= MAX_VOICE_BYTES
        && (1..=MAX_VOICE_MS + 1000).contains(&voice.duration_ms)
        && voice.bars.len() == VOICE_BARS
}

fn wire_kind(kind: FileKind) -> DirectFileKind {
    match kind {
        FileKind::Picture => DirectFileKind::Picture,
        FileKind::Video => DirectFileKind::Video,
        FileKind::File => DirectFileKind::File,
    }
}

/// What travels for an item of this device; a call's line stays here.
pub(super) fn payload(item: &Item) -> Option<DirectPayload> {
    Some(match &item.body {
        Body::Message { text, files } => DirectPayload::Message {
            id: item.id,
            sent_ms: item.sent_ms,
            text: text.clone(),
            files: files
                .iter()
                .map(|f| DirectFile {
                    id: f.id,
                    name: f.name.clone(),
                    size: f.size,
                    kind: wire_kind(f.kind),
                    video: f.video.as_ref().map(|v| WireVideo {
                        thumb: v.thumb.clone(),
                        width: v.width,
                        height: v.height,
                        duration_ms: v.duration_ms,
                    }),
                    voice: f.voice.as_ref().map(|v| WireVoice {
                        duration_ms: v.duration_ms,
                        bars: v.bars.clone(),
                    }),
                })
                .collect(),
            ttl_ms: item.ttl_ms,
        },
        Body::Timer { ms } => DirectPayload::Timer {
            id: item.id,
            sent_ms: item.sent_ms,
            ms: *ms,
        },
        Body::Pin { target, pinned } => DirectPayload::Pin {
            id: item.id,
            sent_ms: item.sent_ms,
            target: *target,
            pinned: *pinned,
        },
        Body::Unsend { target } => DirectPayload::Unsend {
            id: item.id,
            sent_ms: item.sent_ms,
            target: *target,
        },
        Body::Call { .. } => return None,
    })
}

impl Core {
    /// Writes to a friend: `text`, files, or both, while both show as online. It is kept here
    /// at once and goes now, or again until the friend's device says it keeps it. Files are
    /// copied into the chat's folder, so they can be fetched later.
    pub async fn send_direct(
        &self,
        friend: DevicePublicKey,
        text: &str,
        outgoing: Vec<Outgoing>,
    ) -> Result<()> {
        let book = self
            .inner
            .node
            .friends()
            .await?
            .context("this device has no profile")?;
        let name = book
            .friends
            .get(&friend)
            .map(|f| f.profile.name.clone())
            .context("only friends can be written to")?;
        self.inner.ensure_both_online(&friend, &name)?;
        let text = names::shown_text(text);
        let text = text.trim();
        ensure!(
            !text.is_empty() || !outgoing.is_empty(),
            "write something first"
        );
        ensure!(
            text.chars().count() <= MAX_CHAT_CHARS,
            "a message is at most {MAX_CHAT_CHARS} characters"
        );
        ensure!(
            outgoing.len() <= MAX_FILES,
            "at most {MAX_FILES} files go in one message"
        );
        let limit = self.inner.direct.lock().settings.limit;
        let dir = self.inner.direct.store.files_dir(&friend)?;
        let mut copies = Copies::new(&self.inner, friend);
        let mut files = Vec::new();
        for out in outgoing {
            let name = clean_file_name(&out.path);
            let hashed = {
                let path = out.path.clone();
                tokio::task::spawn_blocking(move || files::hash_file(&path)).await?
            };
            let (id, size) = hashed.with_context(|| format!("cannot read {name}"))?;
            if let Some(limit) = limit
                && size > limit
            {
                bail!(
                    "{name} is bigger than the space for direct messages; make it bigger in Settings, Storage"
                );
            }
            let file = Attachment {
                id,
                kind: file_kind(&name, out.video.is_some()),
                name,
                size,
                video: out
                    .video
                    .filter(|v| v.thumb.len() <= MAX_THUMB_BYTES)
                    .map(|v| VideoInfo {
                        thumb: v.thumb,
                        width: v.width,
                        height: v.height,
                        duration_ms: v.duration_ms,
                    }),
                local: Local::Here,
                voice: None,
            };
            debug_assert!(
                self.inner
                    .direct
                    .store
                    .file_path(&friend, &file)
                    .starts_with(&dir)
            );
            copies.using(&file.id);
            if !self.inner.direct.store.has_copy(&friend, &file) {
                // Kept here already under a name that ends otherwise: the same file.
                let here = self
                    .inner
                    .direct
                    .lock()
                    .chats
                    .get(&friend)
                    .and_then(|e| e.chat.kept(&file.id).cloned());
                let (inner, from, kept) = (self.inner.clone(), out.path.clone(), file.clone());
                tokio::task::spawn_blocking(move || match here {
                    Some(here) => inner.direct.store.link_copy(&friend, &here, &kept),
                    None => inner.direct.store.keep_copy(&friend, &kept, &from),
                })
                .await?
                .with_context(|| format!("cannot keep a copy of {}", file.name))?;
                copies.made.push(file.clone());
            }
            files.push(file);
        }
        self.write_message(friend, &name, text, files).await
    }

    /// Sends a voice message recorded here to a friend, while both show as online, as a
    /// message with its file alone. It goes as [`Core::send_direct`]'s do.
    pub async fn send_direct_voice(&self, friend: DevicePublicKey, voice: VoiceNote) -> Result<()> {
        let book = self
            .inner
            .node
            .friends()
            .await?
            .context("this device has no profile")?;
        let name = book
            .friends
            .get(&friend)
            .map(|f| f.profile.name.clone())
            .context("only friends can be written to")?;
        self.inner.ensure_both_online(&friend, &name)?;
        ensure!(
            (1..=MAX_VOICE_MS + 1000).contains(&voice.duration_ms)
                && voice.bars.len() == VOICE_BARS
                && voice.file.len() as u64 <= MAX_VOICE_BYTES,
            "that is not a voice message"
        );
        let size = voice.file.len() as u64;
        if let Some(limit) = self.inner.direct.lock().settings.limit
            && size > limit
        {
            bail!(
                "the voice message is bigger than the space for direct messages; make it bigger in Settings, Storage"
            );
        }
        self.inner.direct.store.files_dir(&friend)?;
        let file = Attachment {
            id: *blake3::hash(&voice.file).as_bytes(),
            name: VOICE_NAME.to_string(),
            size,
            kind: FileKind::File,
            video: None,
            local: Local::Here,
            voice: Some(VoiceInfo {
                duration_ms: voice.duration_ms,
                bars: voice.bars,
                // This device's own voice messages show no dot.
                played: true,
            }),
        };
        let mut copies = Copies::new(&self.inner, friend);
        copies.using(&file.id);
        if !self.inner.direct.store.has_copy(&friend, &file) {
            let (inner, kept, bytes) = (self.inner.clone(), file.clone(), voice.file);
            tokio::task::spawn_blocking(move || {
                inner.direct.store.keep_bytes(&friend, &kept, &bytes)
            })
            .await?
            .context("cannot keep the voice message")?;
            copies.made.push(file.clone());
        }
        self.write_message(friend, &name, "", vec![file]).await
    }

    /// Writes a message of `text` and `files` (kept here already) to `friend`, named
    /// `name`, and sends it.
    async fn write_message(
        &self,
        friend: DevicePublicKey,
        name: &str,
        text: &str,
        files: Vec<Attachment>,
    ) -> Result<()> {
        let now = now_ms();
        let direct = &self.inner.direct;
        let saved = {
            let mut chats = direct.lock();
            let default_timer = chats.settings.timer_ms;
            let entry = chats.chats.entry(friend).or_default();
            ensure!(
                entry.ended_ms.is_none(),
                "you are no longer friends with {name}"
            );
            let mut records = Vec::new();
            // A new chat starts with the timer chosen for new chats, for both.
            if entry.chat.is_empty() && default_timer != 0 {
                let item = Item {
                    id: new_id()?,
                    mine: true,
                    sent_ms: now,
                    got_ms: now,
                    seq: 0,
                    at_ms: 0,
                    body: Body::Timer { ms: default_timer },
                    ttl_ms: 0,
                    delivered: false,
                };
                records.extend(entry.chat.add(item).map(|item| Record::Item { item }));
            }
            let item = Item {
                id: new_id()?,
                mine: true,
                sent_ms: now,
                got_ms: now,
                seq: 0,
                at_ms: 0,
                body: Body::Message {
                    text: text.to_string(),
                    files: files.clone(),
                },
                ttl_ms: entry.chat.timer(),
                delivered: false,
            };
            records.extend(entry.chat.add(item).map(|item| Record::Item { item }));
            // What this device writes counts as read.
            let read_before = entry.chat.read_seq;
            entry.chat.read_seq = entry.chat.last_seq();
            records.push(Record::Read {
                seq: entry.chat.read_seq,
            });
            // What cannot be kept here is not sent: it would be gone from this device after
            // a restart while the friend has it.
            let ids: Vec<ItemId> = records
                .iter()
                .filter_map(|r| match r {
                    Record::Item { item } => Some(item.id),
                    _ => None,
                })
                .collect();
            let read_now = entry.chat.read_seq;
            let undo: Undo = Box::new(move |chats: &mut ChatsState| {
                if let Some(entry) = chats.chats.get_mut(&friend) {
                    entry.chat.remove(&ids);
                    if entry.chat.read_seq == read_now {
                        entry.chat.read_seq = read_before;
                    }
                }
            });
            direct.save(&friend, entry, &records, Some(undo))
        };
        if !saved.await.unwrap_or(false) {
            bail!("the message was not sent: it could not be saved on this computer");
        }
        if let Some(group) = direct.lock().chats.get(&friend).and_then(Entry::group_id) {
            for file in &files {
                direct.share(&self.inner.shared, group, &friend, file);
            }
        }
        for file in &files {
            free_space(&self.inner, Some((&friend, &file.id)), None);
        }
        ensure_group(&self.inner, friend, name).await?;
        flush(&self.inner, friend).await;
        self.inner.emit(CoreEvent::DirectChanged { friend });
        Ok(())
    }

    /// Changes a chat's disappearing timer for both people (0 is off). The change goes in the
    /// chat as a line; messages written after it disappear after `ms`.
    pub async fn set_direct_timer(&self, friend: DevicePublicKey, ms: u64) -> Result<()> {
        ensure!(valid_timer(ms), "that is not one of the timers");
        let book = self
            .inner
            .node
            .friends()
            .await?
            .context("this device has no profile")?;
        let name = book
            .friends
            .get(&friend)
            .map(|f| f.profile.name.clone())
            .context("only a chat with a friend can change")?;
        self.inner.ensure_both_online(&friend, &name)?;
        {
            let direct = &self.inner.direct;
            let mut chats = direct.lock();
            let entry = chats.chats.entry(friend).or_default();
            ensure!(
                entry.ended_ms.is_none(),
                "you are no longer friends with {name}"
            );
            if entry.chat.timer() == ms && !entry.chat.is_empty() {
                return Ok(());
            }
            let now = now_ms();
            let item = Item {
                id: new_id()?,
                mine: true,
                sent_ms: entry.chat.timer_time(now),
                got_ms: now,
                seq: 0,
                at_ms: 0,
                body: Body::Timer { ms },
                ttl_ms: 0,
                delivered: false,
            };
            let mut records: Vec<Record> = entry
                .chat
                .add(item)
                .map(|item| Record::Item { item })
                .into_iter()
                .collect();
            entry.chat.read_seq = entry.chat.last_seq();
            records.push(Record::Read {
                seq: entry.chat.read_seq,
            });
            direct.journal(&friend, entry, &records);
        }
        ensure_group(&self.inner, friend, &name).await?;
        flush(&self.inner, friend).await;
        self.inner.emit(CoreEvent::DirectChanged { friend });
        Ok(())
    }

    /// Pins a message of a chat for both people, or unpins it. While pinned it does not
    /// disappear; unpinned, its own time counts again, and if that has passed it goes now.
    /// A message only pins while both still have it: one of the friend's whose time is over
    /// by when the friend wrote it is refused here, and the friend's device undoes a pin of
    /// a message it no longer has.
    pub async fn pin_direct(
        &self,
        friend: DevicePublicKey,
        message: ItemId,
        pinned: bool,
    ) -> Result<()> {
        let book = self
            .inner
            .node
            .friends()
            .await?
            .context("this device has no profile")?;
        let name = book
            .friends
            .get(&friend)
            .map(|f| f.profile.name.clone())
            .context("only a chat with a friend can change")?;
        self.inner.ensure_both_online(&friend, &name)?;
        {
            let direct = &self.inner.direct;
            let mut chats = direct.lock();
            let entry = chats.chats.get_mut(&friend).context("no such chat")?;
            ensure!(
                entry.ended_ms.is_none(),
                "you are no longer friends with {name}"
            );
            let now = now_ms();
            let target = entry
                .chat
                .get(&message)
                .filter(|i| i.is_message())
                .context("that message is no longer here")?;
            ensure!(
                !pinned || !target.gone_from_its_author(now),
                "that message has already disappeared for {name}"
            );
            let now_pinned = entry.chat.pinned();
            if now_pinned.contains(&message) == pinned {
                return Ok(());
            }
            ensure!(
                !pinned || now_pinned.len() < MAX_PINS,
                "a chat keeps at most {MAX_PINS} pinned messages; unpin one first"
            );
            let item = Item {
                id: new_id()?,
                mine: true,
                sent_ms: entry.chat.pin_time(&message, now),
                got_ms: now,
                seq: 0,
                at_ms: 0,
                body: Body::Pin {
                    target: message,
                    pinned,
                },
                ttl_ms: 0,
                delivered: false,
            };
            let mut records: Vec<Record> = entry
                .chat
                .add(item)
                .map(|item| Record::Item { item })
                .into_iter()
                .collect();
            entry.chat.read_seq = entry.chat.last_seq();
            records.push(Record::Read {
                seq: entry.chat.read_seq,
            });
            direct.journal(&friend, entry, &records);
        }
        if !pinned {
            expire(&self.inner, now_ms());
        }
        ensure_group(&self.inner, friend, &name).await?;
        flush(&self.inner, friend).await;
        self.inner.emit(CoreEvent::DirectChanged { friend });
        Ok(())
    }

    /// Tells the friend, if connected to the chat now, that this device is typing in the
    /// chat with it, or stopped. Nothing is kept; a chat that has no group yet says nothing.
    pub async fn direct_typing(&self, friend: DevicePublicKey, typing: bool) -> Result<()> {
        let group = self
            .inner
            .direct
            .lock()
            .chats
            .get(&friend)
            .and_then(Entry::group_id);
        if let Some(group) = group {
            self.inner.node.typing(group, typing).await?;
        }
        Ok(())
    }
}
