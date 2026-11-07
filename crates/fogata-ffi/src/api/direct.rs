//! Direct messages with friends, for the screens: the list of chats, a chat's page, writing,
//! the disappearing timer, pinned messages, attachments and what they take on this device.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use fogata_core::{
    VideoPreview,
    direct::{
        ChatSummary, ChatView, FileView, ItemView, Outgoing, StorageView,
        model::{Body, FileKind, Local, Usage},
        store::Settings,
    },
};

use super::{
    app::{device, file_id, run},
    types::{UiPerson, UiVideo, hex_id, ms, person},
};

/// A chat in the list of chats.
#[derive(Clone, Debug)]
pub struct UiDirectChat {
    pub friend: UiPerson,
    pub online: bool,
    /// Online, but nobody used the friend's computer for a while.
    pub away: bool,
    /// The last line, and whether this device wrote it.
    pub last: Option<String>,
    pub last_mine: bool,
    /// The last line is a voice message alone, this long.
    pub last_voice_ms: Option<i64>,
    /// When the last line arrived here, or was written here: the list goes newest first.
    pub last_ms: i64,
    pub unread: u32,
    /// The disappearing timer, 0 when off.
    pub timer_ms: i64,
    /// Since when the two are no longer friends: the chat can only be read.
    pub ended_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiFileKind {
    Picture,
    Video,
    File,
}

/// Where an attachment stands on this device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiFileState {
    /// Not fetched: comes when opened (or by itself, for a picture).
    Missing,
    /// Coming now.
    Coming,
    Here,
    /// Removed here to free space, or with the chat's files cleared.
    Removed,
}

#[derive(Clone, Debug)]
pub struct UiDirectFile {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub kind: UiFileKind,
    pub video: Option<UiVideo>,
    /// A voice message.
    pub voice: Option<UiVoice>,
    pub state: UiFileState,
    /// Bytes fetched so far, while it comes.
    pub received: i64,
    /// Where it is, once here.
    pub path: Option<String>,
}

/// A voice message as its sender recorded it.
#[derive(Clone, Debug)]
pub struct UiVoice {
    pub duration_ms: i64,
    /// Its levels from start to end, 0 to 255.
    pub bars: Vec<u8>,
    /// Played here (always so for this device's own).
    pub played: bool,
}

#[derive(Clone, Debug)]
pub struct UiDirectItem {
    pub id: String,
    /// Written on this device.
    pub mine: bool,
    /// When it shows in the chat: when written, but never before the item above it nor
    /// after it arrived. Items come in the order they arrived, so this only grows.
    pub at_ms: i64,
    pub text: String,
    pub files: Vec<UiDirectFile>,
    /// The disappearing timer changed to this (0 is off); `None` for a message.
    pub timer_ms: Option<i64>,
    /// This device's message reached the friend.
    pub delivered: bool,
    /// When it disappears from this device, 0 for never; a pinned message does not
    /// disappear meanwhile.
    pub expires_ms: i64,
    /// A message pinned now.
    pub pinned: bool,
    /// A message pinned or unpinned; `None` for a message or a change of the timer.
    pub pin: Option<UiDirectPin>,
    /// A call the two were in, which ended; `None` for anything else. `mine` says this
    /// device started it.
    pub call: Option<UiDirectCall>,
}

/// A call that ended, as a line of the chat.
#[derive(Clone, Debug)]
pub struct UiDirectCall {
    /// From when it was answered until the last moment two or more people were in it.
    pub lasted_ms: i64,
    /// Who started it, when that was neither of the two.
    pub starter: Option<String>,
}

/// A message pinned or unpinned, as a line of the chat.
#[derive(Clone, Debug)]
pub struct UiDirectPin {
    /// The message's id.
    pub target: String,
    pub pinned: bool,
}

/// What a chat takes on this device.
#[derive(Clone, Debug)]
pub struct UiDirectUsage {
    pub messages: i64,
    pub pictures: i64,
    pub videos: i64,
    pub files: i64,
}

/// A chat's page.
#[derive(Clone, Debug)]
pub struct UiDirectPage {
    pub friend: UiPerson,
    pub online: bool,
    /// Online, but nobody used the friend's computer for a while.
    pub away: bool,
    pub last_seen_ms: Option<i64>,
    /// What is written goes at once.
    pub connected: bool,
    /// Oldest first.
    pub items: Vec<UiDirectItem>,
    /// Older items are there too.
    pub more: bool,
    /// The messages pinned now, the most recently pinned first.
    pub pinned: Vec<UiDirectItem>,
    pub timer_ms: i64,
    pub ended_ms: Option<i64>,
    pub usage: UiDirectUsage,
    /// The chat's history on this computer was changed, or part of it lost, since Fogata
    /// last wrote it (found when it started).
    pub altered: bool,
}

#[derive(Clone, Debug)]
pub struct UiDirectSettings {
    /// Room for pictures, videos and files, in bytes; 0 for no limit.
    pub limit: i64,
    /// Pictures up to 10 MB come by themselves.
    pub auto_pictures: bool,
    /// Videos and files come by themselves too.
    pub auto_files: bool,
    /// The disappearing timer of new chats, 0 for off.
    pub timer_ms: i64,
}

#[derive(Clone, Debug)]
pub struct UiDirectChatUsage {
    pub friend: UiPerson,
    pub usage: UiDirectUsage,
}

/// What direct messages take on this device, and their settings.
#[derive(Clone, Debug)]
pub struct UiDirectStorage {
    pub total: UiDirectUsage,
    pub chats: Vec<UiDirectChatUsage>,
    pub settings: UiDirectSettings,
}

/// A file to send, with a video's first picture and length as this device read them.
#[derive(Clone, Debug)]
pub struct UiDirectOutgoing {
    pub path: String,
    pub video: Option<UiVideo>,
}

fn usage(u: &Usage) -> UiDirectUsage {
    UiDirectUsage {
        messages: ms(u.messages),
        pictures: ms(u.pictures),
        videos: ms(u.videos),
        files: ms(u.files),
    }
}

fn settings(s: &Settings) -> UiDirectSettings {
    UiDirectSettings {
        limit: s.limit.map_or(0, ms),
        auto_pictures: s.auto_pictures,
        auto_files: s.auto_files,
        timer_ms: ms(s.timer_ms),
    }
}

fn summary(c: ChatSummary) -> UiDirectChat {
    let (last, last_mine) = match c.last {
        Some((text, mine)) => (Some(text), mine),
        None => (None, false),
    };
    UiDirectChat {
        friend: person(&c.friend),
        online: c.online,
        away: c.away,
        last,
        last_mine,
        last_voice_ms: c.last_voice_ms.map(ms),
        last_ms: ms(c.last_ms),
        unread: u32::try_from(c.unread).unwrap_or(u32::MAX),
        timer_ms: ms(c.timer_ms),
        ended_ms: c.ended_ms.map(ms),
    }
}

fn file(f: &FileView) -> UiDirectFile {
    UiDirectFile {
        id: hex_id(&f.file.id),
        name: f.file.name.clone(),
        size: ms(f.file.size),
        kind: match f.file.kind {
            FileKind::Picture => UiFileKind::Picture,
            FileKind::Video => UiFileKind::Video,
            FileKind::File => UiFileKind::File,
        },
        video: f.file.video.as_ref().map(|v| UiVideo {
            thumb: v.thumb.clone(),
            width: v.width,
            height: v.height,
            duration_ms: ms(v.duration_ms),
        }),
        voice: f.file.voice.as_ref().map(|v| UiVoice {
            duration_ms: ms(v.duration_ms),
            bars: v.bars.clone(),
            played: v.played,
        }),
        state: match (f.file.local, f.received) {
            (Local::Here, _) => UiFileState::Here,
            (_, Some(_)) => UiFileState::Coming,
            (Local::Missing, None) => UiFileState::Missing,
            (Local::Removed, None) => UiFileState::Removed,
        },
        received: f.received.map_or(0, ms),
        path: f.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
    }
}

fn item(view: &ItemView) -> UiDirectItem {
    let item = &view.item;
    let (text, timer_ms, pin) = match &item.body {
        Body::Message { text, .. } => (text.clone(), None, None),
        Body::Timer { ms: t } => (String::new(), Some(ms(*t)), None),
        Body::Pin { target, pinned } => (
            String::new(),
            None,
            Some(UiDirectPin {
                target: hex_id(target),
                pinned: *pinned,
            }),
        ),
        Body::Call { .. } => (String::new(), None, None),
    };
    let call = match &item.body {
        Body::Call { lasted_ms, starter } => Some(UiDirectCall {
            lasted_ms: ms(*lasted_ms),
            starter: starter.clone(),
        }),
        _ => None,
    };
    UiDirectItem {
        id: hex_id(&item.id),
        mine: item.mine,
        at_ms: ms(item.at_ms),
        text,
        files: view.files.iter().map(file).collect(),
        timer_ms,
        delivered: item.delivered,
        expires_ms: item.expires_ms().map_or(0, ms),
        pinned: view.pinned,
        pin,
        call,
    }
}

fn page(view: ChatView) -> UiDirectPage {
    UiDirectPage {
        friend: person(&view.friend),
        online: view.online,
        away: view.away,
        last_seen_ms: view.last_seen_ms.map(ms),
        connected: view.connected,
        items: view.items.iter().map(item).collect(),
        more: view.more,
        pinned: view.pinned.iter().map(item).collect(),
        timer_ms: ms(view.timer_ms),
        ended_ms: view.ended_ms.map(ms),
        usage: usage(&view.usage),
        altered: view.altered,
    }
}

fn storage(view: StorageView) -> UiDirectStorage {
    UiDirectStorage {
        total: usage(&view.total),
        chats: view
            .chats
            .iter()
            .map(|(p, u)| UiDirectChatUsage {
                friend: person(p),
                usage: usage(u),
            })
            .collect(),
        settings: settings(&view.settings),
    }
}

fn non_negative(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// The chats with something in them, the newest first.
pub async fn direct_chats() -> Result<Vec<UiDirectChat>> {
    run(|core| async move {
        Ok(core
            .direct_chats()
            .await?
            .into_iter()
            .map(summary)
            .collect())
    })
    .await
}

/// A chat's newest `limit` items, and what goes with the chat.
pub async fn direct_chat(device_id: String, limit: u32) -> Result<UiDirectPage> {
    let friend = device(&device_id)?;
    run(|core| async move { Ok(page(core.direct_chat(friend, limit as usize).await?)) }).await
}

/// Writes to a friend: text, files, or both. It goes when both are online.
pub async fn send_direct(
    device_id: String,
    text: String,
    files: Vec<UiDirectOutgoing>,
) -> Result<()> {
    let friend = device(&device_id)?;
    let files = files
        .into_iter()
        .map(|f| Outgoing {
            path: PathBuf::from(f.path),
            video: f.video.map(|v| VideoPreview {
                thumb: v.thumb,
                width: v.width,
                height: v.height,
                duration_ms: non_negative(v.duration_ms),
            }),
        })
        .collect();
    run(|core| async move { core.send_direct(friend, &text, files).await }).await
}

/// Changes a chat's disappearing timer for both people: 0 (off), 24 hours, 7 or 90 days.
pub async fn set_direct_timer(device_id: String, timer_ms: i64) -> Result<()> {
    let friend = device(&device_id)?;
    run(|core| async move { core.set_direct_timer(friend, non_negative(timer_ms)).await }).await
}

/// Pins a message of a chat for both people, or unpins it (`pinned` false). A pinned message
/// does not disappear; a chat keeps at most 50.
pub async fn pin_direct(device_id: String, item_id: String, pinned: bool) -> Result<()> {
    let friend = device(&device_id)?;
    let message: [u8; 16] = hex::decode(&item_id)?
        .try_into()
        .map_err(|_| anyhow!("not a message id"))?;
    run(|core| async move { core.pin_direct(friend, message, pinned).await }).await
}

/// Tells the friend, if connected now, that this device is typing in the chat with it
/// (`typing`), or stopped.
pub async fn direct_typing(device_id: String, typing: bool) -> Result<()> {
    let friend = device(&device_id)?;
    run(|core| async move { core.direct_typing(friend, typing).await }).await
}

/// Marks everything in a chat as read.
pub async fn mark_direct_read(device_id: String) -> Result<()> {
    let friend = device(&device_id)?;
    run(|core| async move {
        core.mark_direct_read(friend);
        Ok(())
    })
    .await
}

/// Notes the friend's voice message with this file as played: its dot goes.
pub async fn mark_direct_played(device_id: String, file: String) -> Result<()> {
    let (friend, file) = (device(&device_id)?, file_id(&file)?);
    run(|core| async move {
        core.mark_direct_played(friend, file);
        Ok(())
    })
    .await
}

/// Fetches an attachment from the friend: one not fetched yet, or removed here.
pub async fn fetch_direct_file(device_id: String, file: String) -> Result<()> {
    let (friend, file) = (device(&device_id)?, file_id(&file)?);
    run(|core| async move { core.fetch_direct_file(friend, file).await }).await
}

/// Copies an attachment kept here into `dir` under its own name; returns where it went.
pub async fn save_direct_file(device_id: String, file: String, dir: String) -> Result<String> {
    let (friend, file) = (device(&device_id)?, file_id(&file)?);
    run(|core| async move {
        let path = core.save_direct_file(friend, file, Path::new(&dir))?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

/// A picture kept here, read whole for the screen, if it is at most `max_bytes`: kept
/// files may be sealed, so the screens read them through the core.
pub async fn direct_file_bytes(device_id: String, file: String, max_bytes: u32) -> Result<Vec<u8>> {
    let (friend, file) = (device(&device_id)?, file_id(&file)?);
    run(|core| async move { core.direct_file_bytes(friend, file, max_bytes.into()).await }).await
}

/// Whether an attachment of the chat with `device_id` is still kept here: no longer once its
/// message is deleted or ran out, or its file was removed. For one the page read does not
/// hold.
pub async fn direct_file_kept(device_id: String, file: String) -> Result<bool> {
    let (friend, file) = (device(&device_id)?, file_id(&file)?);
    run(|core| async move { Ok(core.direct_file_kept(friend, file)) }).await
}

/// Where a video kept here can be played from by the system's player: its own file, or a
/// plain copy of a sealed one, made for this player alone, which goes with
/// [`forget_direct_file_copy`].
pub async fn direct_file_to_play(device_id: String, file: String) -> Result<String> {
    let (friend, file) = (device(&device_id)?, file_id(&file)?);
    run(|core| async move {
        let path = core.direct_file_to_play(friend, file).await?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

/// Removes the plain copy made to play a video at `path`, once its player let it go; any
/// other path is left alone.
pub async fn forget_direct_file_copy(path: String) -> Result<()> {
    run(|core| async move {
        core.forget_direct_file_copy(Path::new(&path));
        Ok(())
    })
    .await
}

/// Removes a chat's pictures, videos and files from this device; the messages stay.
pub async fn clear_direct_files(device_id: String) -> Result<()> {
    let friend = device(&device_id)?;
    run(|core| async move { core.clear_direct_files(friend) }).await
}

/// Deletes a chat with someone who is no longer a friend, here only.
pub async fn delete_direct_chat(device_id: String) -> Result<()> {
    let friend = device(&device_id)?;
    run(|core| async move { core.delete_direct_chat(friend).await }).await
}

/// What direct messages take on this device, and their settings.
pub async fn direct_storage() -> Result<UiDirectStorage> {
    run(|core| async move { Ok(storage(core.direct_storage().await?)) }).await
}

/// Changes the settings of direct messages; a smaller room frees space at once.
pub async fn set_direct_settings(settings: UiDirectSettings) -> Result<()> {
    let settings = Settings {
        limit: match settings.limit {
            0 => None,
            n if n > 0 => Some(n as u64),
            _ => return Err(anyhow!("that is not one of the sizes")),
        },
        auto_pictures: settings.auto_pictures,
        auto_files: settings.auto_files,
        timer_ms: non_negative(settings.timer_ms),
    };
    run(|core| async move { core.set_direct_settings(settings) }).await
}
