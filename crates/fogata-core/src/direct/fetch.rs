//! Attachments fetched from the friend's device over the file protocol: pictures and voice
//! messages as they come, within a daily allowance, videos and files when they are asked
//! for, each checked to be what it says and charged to the chat's room, and tried again
//! later when the friend was not there.

use std::{
    collections::HashSet,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_net::files;
use fogata_proto::MAX_VOICE_BYTES;

use super::{
    ChatsState, free_space,
    model::{
        AUTO_PICTURE_BYTES, Attachment, Charge, Chat, FileId, FileKind, Item, Local, Pool,
        VoiceInfo,
    },
    store::{DirectStore, Record, Settings},
    within,
};
use crate::{CoreEvent, Inner, now_ms};

/// Fetches an attachment from the friend: one the person `asked` for, or one that comes by
/// itself. A voice message or a picture is checked once here (its file must read as one);
/// one that does not becomes the file it is, and is kept only if it was asked for or files
/// come by themselves. What comes by itself is charged to the friend's daily allowance as it
/// starts, and makes room in the friend's own chat only: past either it stays to be asked
/// for.
pub(super) async fn fetch(
    inner: &Arc<Inner>,
    friend: DevicePublicKey,
    file: FileId,
    asked: bool,
) -> Result<()> {
    // The same file sent again: the copy kept for the first message serves this one too.
    let again = {
        let chats = inner.direct.lock();
        let entry = chats.chats.get(&friend).context("no such chat")?;
        let copies = || {
            entry
                .chat
                .items()
                .iter()
                .flat_map(Item::files)
                .filter(|f| f.id == file)
        };
        copies()
            .find(|f| f.local == Local::Here)
            .cloned()
            .map(|kept| {
                let again: Vec<Attachment> = copies()
                    .filter(|f| f.local != Local::Here)
                    .cloned()
                    .collect();
                (kept, again)
            })
    };
    if let Some((kept, again)) = again {
        if !again.is_empty() {
            serve_again(inner, friend, kept, again).await;
        }
        return Ok(());
    }
    let (group, attachment, dest, turns, charged) = {
        let mut chats = inner.direct.lock();
        let limit = chats.settings.limit;
        let connected = chats.connected.contains(&friend);
        let busy = chats.downloads.contains_key(&(friend, file));
        let others = if asked {
            0
        } else {
            others_usage(&chats, &inner.direct.store, &friend)
        };
        let entry = chats.chats.get_mut(&friend).context("no such chat")?;
        let copies = || {
            entry
                .chat
                .items()
                .iter()
                .flat_map(Item::files)
                .filter(|f| f.id == file)
        };
        let attachment = copies()
            .next()
            .cloned()
            .context("that file is not in the chat")?;
        // Kept here since it was looked at a moment ago: nothing to fetch.
        if copies().any(|f| f.local == Local::Here) || busy {
            return Ok(());
        }
        let group = entry.group_id().context("the chat is not connected")?;
        ensure!(
            connected,
            "files come while you are both online; try again then"
        );
        if let Some(limit) = limit
            && attachment.size > limit
        {
            bail!(
                "that file is bigger than the space for direct messages; make it bigger in Settings, Storage"
            );
        }
        // Made before anything is charged: what stops the fetch here costs nothing.
        inner.direct.store.files_dir(&friend)?;
        let mut charged = None;
        if !asked {
            // One that did not come waits a while before it is tried again by itself.
            if chats
                .retry
                .get(&(friend, file))
                .is_some_and(|(at, _)| Instant::now() < *at)
            {
                return Ok(());
            }
            let entry = chats.chats.get_mut(&friend).context("no such chat")?;
            // Room for what comes by itself is made in its own chat only: when the other
            // chats leave too little, it stays to be asked for.
            if limit.is_some_and(|limit| others.saturating_add(attachment.size) > limit) {
                tracing::info!("an attachment waits to be asked for: no room for it here");
                return Ok(());
            }
            // Charged to the friend's daily allowance, once a day for each file, at least
            // [`MIN_CHARGE`]: past it, it stays to be asked for. The charge is noted in the
            // journal once the file starts to come, and taken back if it does not.
            let now = now_ms();
            let pool = if attachment.voice.is_some() {
                Pool::Voice
            } else {
                Pool::Files
            };
            if !entry.chat.charged_for(pool, &file, now) {
                let charge = Charge {
                    at_ms: now,
                    bytes: attachment.size.max(MIN_CHARGE),
                    file: Some(file),
                };
                if charge.bytes > allowance(&entry.chat, pool, now) {
                    tracing::info!(
                        "an attachment waits to be asked for: the day's allowance is used"
                    );
                    return Ok(());
                }
                entry.chat.charge(pool, charge);
                charged = Some((pool, charge));
            }
        }
        let dest = inner.direct.store.file_path(&friend, &attachment);
        chats.downloads.insert((friend, file), 0);
        // What comes by itself waits for a turn of the friend's and one of all; what was
        // asked for goes at once.
        let turns = (!asked).then(|| {
            let mine = chats
                .turns
                .entry(friend)
                .or_insert_with(|| Arc::new(tokio::sync::Semaphore::new(FETCHES_PER_FRIEND)))
                .clone();
            (mine, inner.direct.turns.clone())
        });
        (group, attachment, dest, turns, charged)
    };
    let peer = inner.node.member_peer(friend).await.ok().flatten();
    let Some(peer) = peer else {
        inner.direct.lock().downloads.remove(&(friend, file));
        settle(inner, friend, file, charged, false);
        bail!("files come while you are both online; try again then");
    };
    // What was asked for holds its room on the disk now, so the person hears at once that
    // it does not fit; what comes by itself once its turn comes, so those waiting for one
    // hold none.
    let room = if asked {
        match room_on_disk(inner, &attachment, &dest) {
            Ok(room) => Some(room),
            Err(e) => {
                inner.direct.lock().downloads.remove(&(friend, file));
                settle(inner, friend, file, charged, false);
                return Err(e);
            }
        }
    } else {
        None
    };
    inner.emit(CoreEvent::DirectChanged { friend });
    let weak = Arc::downgrade(inner);
    tokio::spawn(async move {
        let _turns = match turns {
            Some((mine, all)) => {
                let (Ok(mine), Ok(all)) = (mine.acquire_owned().await, all.acquire_owned().await)
                else {
                    return;
                };
                Some((mine, all))
            }
            None => None,
        };
        // Held until the file is kept or not: it then takes its room itself, or none.
        let _room = match room {
            Some(room) => room,
            None => {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                match room_on_disk(&inner, &attachment, &dest) {
                    Ok(room) => room,
                    Err(_) => {
                        tracing::info!("an attachment waits to be asked for: the disk is full");
                        inner.direct.lock().downloads.remove(&(friend, file));
                        settle(&inner, friend, file, charged, false);
                        tried(&inner, friend, file, false);
                        inner.emit(CoreEvent::DirectChanged { friend });
                        return;
                    }
                }
            }
        };
        let total = attachment.size;
        let mut last_step = 0;
        let started = std::sync::atomic::AtomicBool::new(false);
        let coming = Coming::new();
        let mut progress = |got: u64| {
            started.store(true, std::sync::atomic::Ordering::Relaxed);
            coming.note();
            let Some(inner) = weak.upgrade() else {
                return;
            };
            inner.direct.lock().downloads.insert((friend, file), got);
            let step = got * 20 / total.max(1);
            if step > last_step {
                last_step = step;
                inner.emit(CoreEvent::DirectDownload {
                    friend,
                    file,
                    received: got,
                    total,
                });
            }
        };
        // A friend that sent this device all it sends in a day says when to ask again: what
        // was asked for waits until then and goes on; what comes by itself is tried again
        // then, which is no failure. Asked for, it waits a few times at most (see
        // `LaterRounds`): a friend that always says later does not hold it without end.
        let mut later_rounds = crate::downloads::LaterRounds::default();
        let (fetched, later) = loop {
            let later = std::sync::atomic::AtomicU64::new(0);
            let cut = std::sync::atomic::AtomicBool::new(false);
            let fetched = match weak.upgrade() {
                Some(inner) => {
                    let fetching = async {
                        let fetched = match inner.direct.store.key() {
                            Some(key) => {
                                let key = key.clone();
                                files::fetch_sealed(
                                    &inner.node,
                                    group,
                                    peer,
                                    file,
                                    total,
                                    &dest,
                                    &key,
                                    &mut progress,
                                )
                                .await
                            }
                            None => {
                                files::fetch(
                                    &inner.node,
                                    group,
                                    peer,
                                    file,
                                    total,
                                    &dest,
                                    &mut progress,
                                )
                                .await
                            }
                        };
                        if let Err(files::FileError::Later(wait)) = &fetched {
                            later
                                .store(wait.as_secs().max(1), std::sync::atomic::Ordering::Relaxed);
                        }
                        if let Err(files::FileError::Cut) = &fetched {
                            cut.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        fetched
                    };
                    within(fetch_limit(asked, total), &coming, fetching).await
                }
                None => return,
            };
            let later = Duration::from_secs(later.load(std::sync::atomic::Ordering::Relaxed));
            // Stopped after some of it came (the friend gave its turn to another device's
            // while this one took it slowly): what was asked for goes on from there. Each
            // such try brings more of it; what comes by itself is tried again later anyway.
            if asked && cut.load(std::sync::atomic::Ordering::Relaxed) {
                tokio::time::sleep(CUT_AGAIN_AFTER).await;
                coming.note();
                continue;
            }
            if later.is_zero() || !asked {
                break (fetched, later);
            }
            let Some(wait) = later_rounds.next(later) else {
                break (fetched, Duration::ZERO);
            };
            tokio::time::sleep(wait).await;
            // The wait is no link gone quiet.
            coming.note();
        };
        let Some(inner) = weak.upgrade() else {
            return;
        };
        let started = fetched.is_ok() || started.load(std::sync::atomic::Ordering::Relaxed);
        settle(&inner, friend, file, charged, started);
        if !asked && !later.is_zero() {
            inner
                .direct
                .lock()
                .retry
                .entry((friend, file))
                .and_modify(|(at, _)| *at = Instant::now() + later)
                .or_insert((Instant::now() + later, 0));
        } else if !asked {
            tried(&inner, friend, file, fetched.is_ok());
        }
        match fetched {
            Ok(()) if attachment.voice.is_some() => {
                fetched_voice(&inner, friend, &attachment, group, &dest, asked).await;
            }
            Ok(()) if attachment.kind == FileKind::Picture => {
                fetched_picture(&inner, friend, &attachment, group, &dest, asked).await;
            }
            Ok(()) => fetched_file(&inner, friend, &attachment, group, &dest, !asked),
            Err(e) => {
                tracing::debug!("an attachment did not come: {e}");
                inner.emit(CoreEvent::DirectDownloadFailed {
                    friend,
                    file,
                    reason: e.to_string(),
                });
            }
        }
        // Still downloading until it is kept or not: a voice message being checked would
        // otherwise be fetched again, and charged again, by the next message to arrive.
        inner.direct.lock().downloads.remove(&(friend, file));
        inner.emit(CoreEvent::DirectChanged { friend });
    });
    Ok(())
}

/// Most attachments of one friend's that are fetched by themselves at once, and of every
/// chat's together: the others wait for their turn, so a friend who sends a burst of them,
/// and sends each slowly, holds no more than these streams and files open here.
const FETCHES_PER_FRIEND: usize = 4;
pub(super) const FETCHES: usize = 16;

/// How long an attachment of `size` bytes that comes by itself may take to come: two
/// minutes, and a second for every 32 KiB. A friend who sends it slower than that gives up
/// its turn; what came stays for the next try to go on from.
fn fetch_deadline(size: u64) -> Duration {
    Duration::from_secs(120 + size / (32 << 10))
}

/// How long an attachment the person asked for may go without any of it coming: on a slow
/// link it takes as long as it takes, and gives up only when the link stops.
const ASKED_QUIET: Duration = Duration::from_secs(120);

/// How long an attachment the person asked for, which stopped coming after some of it came,
/// waits before it is asked for again (see [`files::FileError::Cut`]).
const CUT_AGAIN_AFTER: Duration = Duration::from_secs(2);

/// When a fetch gives up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Limit {
    /// Past this long in all.
    Total(Duration),
    /// When nothing came for this long.
    Quiet(Duration),
}

/// The limit of a fetch of `size` bytes: the person waits for what was `asked` for as long
/// as it comes; what comes by itself holds a turn only for its [`fetch_deadline`].
fn fetch_limit(asked: bool, size: u64) -> Limit {
    if asked {
        Limit::Quiet(ASKED_QUIET)
    } else {
        Limit::Total(fetch_deadline(size))
    }
}

/// When bytes of a fetch last came.
pub(super) struct Coming {
    since: tokio::time::Instant,
    /// Milliseconds from `since` to the last bytes.
    last_ms: std::sync::atomic::AtomicU64,
}

impl Coming {
    fn new() -> Self {
        Coming {
            since: tokio::time::Instant::now(),
            last_ms: Default::default(),
        }
    }

    fn note(&self) {
        let ms = u64::try_from(self.since.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_ms
            .fetch_max(ms, std::sync::atomic::Ordering::Relaxed);
    }

    pub(super) fn last(&self) -> tokio::time::Instant {
        self.since + Duration::from_millis(self.last_ms.load(std::sync::atomic::Ordering::Relaxed))
    }
}

/// What a voice message, picture, video or file that comes by itself is charged at least, so
/// a burst of tiny ones does not fit thousands in a day: 64 KiB, about 20 s of voice.
pub const MIN_CHARGE: u64 = 64 << 10;

/// How long an attachment that did not come by itself waits before it is tried again by
/// itself: twice as long each time, from half a minute up to an hour.
fn retry_after(failures: u32) -> Duration {
    Duration::from_secs((30u64 << failures.min(7)).min(3600))
}

/// Holds room on the disk for `attachment`'s file at `dest`, sealed or not, as room downloads
/// do: what the disk keeps free is left for the journals, the friend book and the groups'
/// state, written meanwhile.
fn room_on_disk(
    inner: &Arc<Inner>,
    attachment: &Attachment,
    dest: &Path,
) -> Result<crate::shares::Room> {
    crate::shares::hold_disk(
        inner,
        crate::shares::held_for(attachment.size),
        dest.parent().unwrap_or(dest),
        "not enough free space for this file",
    )
}

/// Notes whether `file` came: one that did not is tried again by itself only after a
/// while ([`retry_after`]).
fn tried(inner: &Inner, friend: DevicePublicKey, file: FileId, came: bool) {
    let mut chats = inner.direct.lock();
    if came {
        chats.retry.remove(&(friend, file));
    } else {
        let failures = chats.retry.get(&(friend, file)).map_or(0, |(_, n)| *n);
        let at = Instant::now() + retry_after(failures);
        chats
            .retry
            .insert((friend, file), (at, failures.saturating_add(1)));
    }
}

/// Settles the charge a fetch of `file` made, if it made one (`charged`), once the fetch
/// ended or did not start: noted in the journal when the file `started` to come, taken
/// back when it did not.
pub(super) fn settle(
    inner: &Inner,
    friend: DevicePublicKey,
    file: FileId,
    charged: Option<(Pool, Charge)>,
    started: bool,
) {
    let Some((pool, charge)) = charged else {
        return;
    };
    let direct = &inner.direct;
    let mut chats = direct.lock();
    let Some(entry) = chats.chats.get_mut(&friend) else {
        return;
    };
    if started {
        direct.journal(&friend, entry, &[Record::charge(pool, charge)]);
    } else {
        entry.chat.refund(pool, &file);
    }
}

/// What the attachments of every chat but the friend's take here, and every chat's
/// messages: what the friend's attachments that come by themselves can never push out.
fn others_usage(chats: &ChatsState, store: &DirectStore, friend: &DevicePublicKey) -> u64 {
    chats
        .chats
        .iter()
        .filter(|(f, _)| *f != friend)
        .map(|(_, e)| e.chat.usage().attachments())
        .fold(journals(chats, store), u64::saturating_add)
}

/// What every chat's journal and pins take on the disk: messages count against the room
/// for direct messages, and always stay.
pub(super) fn journals(chats: &ChatsState, store: &DirectStore) -> u64 {
    chats
        .chats
        .keys()
        .map(|friend| store.journal_bytes(friend))
        .fold(0, u64::saturating_add)
}

/// A file kept here for one message came again in others (`again`): the kept copy serves
/// them too, once it reads as what they say it is. What says it is a voice message or a
/// picture and does not read as one shows as the file it is, as when it is fetched.
async fn serve_again(
    inner: &Arc<Inner>,
    friend: DevicePublicKey,
    kept: Attachment,
    again: Vec<Attachment>,
) {
    let voice = again.iter().find_map(|f| f.voice.clone());
    let picture = again.iter().any(|f| f.kind == FileKind::Picture);
    let (not_voice, not_picture) = {
        let (inner, kept) = (inner.clone(), kept.clone());
        tokio::task::spawn_blocking(move || {
            for copy in &again {
                if let Err(e) = inner.direct.store.link_copy(&friend, &kept, copy) {
                    tracing::warn!("a file kept here does not show under another name: {e:#}");
                }
            }
            (
                voice.is_some_and(|v| !is_voice(&inner, &friend, &kept, &v)),
                picture && !is_picture(&inner, &friend, &kept),
            )
        })
        .await
        .unwrap_or((true, true))
    };
    {
        let direct = &inner.direct;
        let mut chats = direct.lock();
        let Some(entry) = chats.chats.get_mut(&friend) else {
            return;
        };
        let file = kept.id;
        let mut records = Vec::new();
        if not_voice && entry.chat.drop_voice(&file) {
            tracing::info!("a voice message from a friend is not one: it shows as a file");
            records.push(Record::NotVoice { file });
        }
        if not_picture && entry.chat.drop_picture(&file) {
            tracing::info!("a picture from a friend is not one: it shows as a file");
            records.push(Record::NotPicture { file });
        }
        if !entry.chat.set_local(&file, Local::Here).is_empty() {
            records.push(Record::File {
                file,
                local: Local::Here,
            });
        }
        direct.journal(&friend, entry, &records);
    }
    inner.emit(CoreEvent::DirectChanged { friend });
}

/// A voice message's file arrived whole. One that reads as a voice message is kept like
/// any attachment; room for one that came by itself is made in its own chat. One that does
/// not read as one becomes the file it is, kept if asked for or if files come by
/// themselves, and otherwise thrown away until asked for.
async fn fetched_voice(
    inner: &Arc<Inner>,
    friend: DevicePublicKey,
    attachment: &Attachment,
    group: GroupId,
    dest: &Path,
    asked: bool,
) {
    let fine = match attachment.voice.clone() {
        Some(voice) => {
            let (inner, kept) = (inner.clone(), attachment.clone());
            tokio::task::spawn_blocking(move || is_voice(&inner, &friend, &kept, &voice))
                .await
                .unwrap_or(false)
        }
        None => false,
    };
    if fine {
        fetched_file(inner, friend, attachment, group, dest, !asked);
        return;
    }
    tracing::info!("a voice message from a friend is not one: it shows as a file");
    {
        let direct = &inner.direct;
        let mut chats = direct.lock();
        if let Some(entry) = chats.chats.get_mut(&friend)
            && entry.chat.drop_voice(&attachment.id)
        {
            direct.journal(
                &friend,
                entry,
                &[Record::NotVoice {
                    file: attachment.id,
                }],
            );
        }
    }
    let plain = Attachment {
        voice: None,
        ..attachment.clone()
    };
    if asked || inner.direct.lock().settings.auto_files {
        fetched_file(inner, friend, &plain, group, dest, !asked);
    } else {
        throw_away(&mut inner.direct.lock(), &friend, attachment, dest);
    }
}

/// A picture's file arrived whole. One that starts as a picture the chat shows is kept like
/// any attachment. One that does not becomes the file it is, kept if asked for or if files
/// come by themselves, and otherwise thrown away until asked for.
async fn fetched_picture(
    inner: &Arc<Inner>,
    friend: DevicePublicKey,
    attachment: &Attachment,
    group: GroupId,
    dest: &Path,
    asked: bool,
) {
    let fine = {
        let (inner, kept) = (inner.clone(), attachment.clone());
        tokio::task::spawn_blocking(move || is_picture(&inner, &friend, &kept))
            .await
            .unwrap_or(false)
    };
    if fine {
        fetched_file(inner, friend, attachment, group, dest, !asked);
        return;
    }
    tracing::info!("a picture from a friend is not one: it shows as a file");
    {
        let direct = &inner.direct;
        let mut chats = direct.lock();
        if let Some(entry) = chats.chats.get_mut(&friend)
            && entry.chat.drop_picture(&attachment.id)
        {
            direct.journal(
                &friend,
                entry,
                &[Record::NotPicture {
                    file: attachment.id,
                }],
            );
        }
    }
    let plain = Attachment {
        kind: FileKind::File,
        ..attachment.clone()
    };
    if asked || inner.direct.lock().settings.auto_files {
        fetched_file(inner, friend, &plain, group, dest, !asked);
    } else {
        throw_away(&mut inner.direct.lock(), &friend, attachment, dest);
    }
}

/// How far a voice message's length may be from what its message says: a recording here
/// says its own to the millisecond, so anything further off is not that voice message.
const VOICE_LENGTH_SLACK_MS: u64 = 1000;

/// Whether the file kept as `attachment` reads as the voice message `voice` says it is: an
/// Ogg Opus file within a voice message's limits, of the length it says (within
/// [`VOICE_LENGTH_SLACK_MS`]), so the chat never shows a length the file does not have.
/// Blocking.
fn is_voice(
    inner: &Inner,
    friend: &DevicePublicKey,
    attachment: &Attachment,
    voice: &VoiceInfo,
) -> bool {
    let Ok((mut kept, size)) = inner.direct.store.open_file(friend, attachment) else {
        return false;
    };
    if size > MAX_VOICE_BYTES {
        return false;
    }
    let mut bytes = Vec::with_capacity(size as usize);
    if std::io::Read::read_to_end(
        &mut std::io::Read::take(&mut kept, MAX_VOICE_BYTES + 1),
        &mut bytes,
    )
    .is_err()
    {
        return false;
    }
    bytes.len() as u64 <= MAX_VOICE_BYTES
        && fogata_voice::ogg::read(&bytes).is_ok_and(|opus| {
            opus.duration_ms().abs_diff(voice.duration_ms) <= VOICE_LENGTH_SLACK_MS
        })
}

/// Whether `bytes` start as a picture of a kind the chat shows: PNG, JPEG, GIF, WebP or
/// BMP.
fn looks_like_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
        || bytes.starts_with(b"BM")
}

/// Whether the file kept for a picture starts as one ([`looks_like_picture`]). Blocking.
fn is_picture(inner: &Inner, friend: &DevicePublicKey, attachment: &Attachment) -> bool {
    let Ok((kept, _)) = inner.direct.store.open_file(friend, attachment) else {
        return false;
    };
    let mut start = Vec::with_capacity(12);
    std::io::Read::read_to_end(&mut std::io::Read::take(kept, 12), &mut start).is_ok()
        && looks_like_picture(&start)
}

/// Removes the file at `dest` of an attachment that came but is not kept; unless a send
/// uses the same file, found here meanwhile, which keeps it or lets it go as it ends (see
/// `InUse` in mod.rs).
fn throw_away(
    chats: &mut ChatsState,
    friend: &DevicePublicKey,
    attachment: &Attachment,
    dest: &Path,
) {
    if chats.copying.uses(friend, &attachment.id) {
        chats
            .copying
            .leave(friend, &attachment.id, [attachment.name.as_str()]);
    } else {
        let _ = std::fs::remove_file(dest);
    }
}

/// An attachment arrived whole: kept and served, unless its message went meanwhile. Room
/// is made for it; for one that came `by_itself`, in its own chat only, and when that is
/// not enough it is not kept and stays to be asked for.
fn fetched_file(
    inner: &Inner,
    friend: DevicePublicKey,
    attachment: &Attachment,
    group: GroupId,
    dest: &Path,
    by_itself: bool,
) {
    let direct = &inner.direct;
    let mut chats = direct.lock();
    if by_itself
        && let Some(limit) = chats.settings.limit
        && others_usage(&chats, &inner.direct.store, &friend).saturating_add(attachment.size)
            > limit
    {
        tracing::info!("an attachment that came by itself is not kept: no room for it here");
        throw_away(&mut chats, &friend, attachment, dest);
        return;
    }
    let Some(entry) = chats
        .chats
        .get_mut(&friend)
        .filter(|e| e.chat.refers_to(&attachment.id))
    else {
        throw_away(&mut chats, &friend, attachment, dest);
        return;
    };
    // Every message with this file shows it under its own name's ending.
    for copy in entry.chat.copies_of(&attachment.id) {
        if let Err(e) = direct.store.link_copy(&friend, attachment, &copy) {
            tracing::warn!("a file kept here does not show under another name: {e:#}");
        }
    }
    entry.chat.set_local(&attachment.id, Local::Here);
    direct.journal(
        &friend,
        entry,
        &[Record::File {
            file: attachment.id,
            local: Local::Here,
        }],
    );
    if entry.group_id() == Some(group) {
        direct.share(&inner.shared, group, &friend, attachment);
    }
    drop(chats);
    free_space(
        inner,
        Some((&friend, &attachment.id)),
        by_itself.then_some(&friend),
    );
}

/// Most bytes of a friend's voice messages that come by themselves in a day: past them the
/// friend's voice messages wait to be asked for, as files do, so a friend cannot fill this
/// computer, nor push out what other chats keep, with voice messages. 64 MiB is about 25
/// messages of 15 minutes, or 7 hours of voice at the rate a recording here takes.
pub const VOICE_DAILY_BYTES: u64 = 64 << 20;

/// Most bytes of a friend's pictures (and videos and files, when those come by themselves
/// too) that come by themselves in a day: past them they wait to be asked for. 512 MiB is
/// some fifty pictures of the largest size that comes by itself.
pub const FILES_DAILY_BYTES: u64 = 512 << 20;

/// What is left of a friend's daily allowance of attachments that come by themselves, of
/// voice messages or of the rest: [`VOICE_DAILY_BYTES`] or [`FILES_DAILY_BYTES`] less every
/// one that started to come by itself in the last 24 hours (kept in the chat's journal, so
/// a restart does not refill it), whether it then came, was not what it said, or is still
/// on its way.
fn allowance(chat: &Chat, pool: Pool, now: u64) -> u64 {
    let daily = match pool {
        Pool::Voice => VOICE_DAILY_BYTES,
        Pool::Files => FILES_DAILY_BYTES,
    };
    daily.saturating_sub(chat.charged(pool, now))
}

/// The friend's attachments in `chat` that come by themselves now (see
/// [`fetch_by_themselves`]), oldest first.
fn due_by_themselves(
    chat: &Chat,
    settings: &Settings,
    now: u64,
    waiting: &HashSet<FileId>,
) -> Vec<FileId> {
    let mut voice_left = allowance(chat, Pool::Voice, now);
    let mut files_left = allowance(chat, Pool::Files, now);
    let mut seen = HashSet::new();
    let mut due = Vec::new();
    for f in chat
        .items()
        .iter()
        .filter(|i| !i.mine)
        .flat_map(Item::files)
        .filter(|f| !waiting.contains(&f.id))
    {
        // One that started to come today already counts; each other one counts at least
        // [`MIN_CHARGE`].
        let fits = |pool, left: &mut u64| {
            if chat.charged_for(pool, &f.id, now) {
                return true;
            }
            let cost = f.size.max(MIN_CHARGE);
            let fits = cost <= *left;
            if fits && !seen.contains(&f.id) {
                *left -= cost;
            }
            fits
        };
        let wanted = f.local == Local::Missing
            && settings.limit.is_none_or(|l| f.size <= l)
            && if f.voice.is_some() {
                // Voice messages come as they arrive, to play at once, within the friend's
                // daily allowance; past it they wait to be asked for.
                fits(Pool::Voice, &mut voice_left)
            } else if if f.kind == FileKind::Picture && f.size <= AUTO_PICTURE_BYTES {
                settings.auto_pictures
            } else {
                settings.auto_files
            } {
                // So do pictures, and videos and files when that is on, within theirs.
                fits(Pool::Files, &mut files_left)
            } else {
                false
            };
        if wanted && seen.insert(f.id) {
            due.push(f.id);
        }
    }
    due
}

/// Starts fetching the attachments that come by themselves and are not here: voice
/// messages, pictures up to 10 MB, and videos and files too when that is on, each within the
/// friend's daily allowance; never one removed here, which only comes back when asked for.
pub(super) async fn fetch_by_themselves(inner: &Arc<Inner>, friend: DevicePublicKey) {
    let due = {
        let chats = inner.direct.lock();
        let Some(entry) = chats.chats.get(&friend) else {
            return;
        };
        // Those waiting to be tried again neither come now nor count.
        let at = Instant::now();
        let waiting: HashSet<FileId> = chats
            .retry
            .iter()
            .filter(|((f, _), (until, _))| *f == friend && at < *until)
            .map(|((_, file), _)| *file)
            .collect();
        due_by_themselves(&entry.chat, &chats.settings, now_ms(), &waiting)
    };
    for file in due {
        if let Err(e) = fetch(inner, friend, file, false).await {
            tracing::debug!("an attachment does not come by itself now: {e:#}");
            tried(inner, friend, file, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        time::{Duration, Instant},
    };

    use anyhow::Result;
    use fogata_net::files;
    use fogata_proto::{
        DirectFile, DirectFileKind, DirectPayload, MAX_VOICE_BYTES, MAX_VOICE_MS, VOICE_BARS,
        WireVoice,
    };

    use super::*;
    use crate::direct::{
        VOICE_NAME,
        model::{self, Attachment, Body, Charge, Chat, FileKind, Item, Local, Pool, VoiceInfo},
        payload, received,
        store::Settings,
        timer_words, within,
    };

    #[test]
    fn a_friends_voice_messages_come_by_themselves_within_a_daily_allowance() {
        const MB: u64 = 1 << 20;
        let now = 10 * model::DAY_MS;
        let voice = |n: u8, size: u64| Item {
            id: [n; 16],
            mine: false,
            sent_ms: now,
            got_ms: now,
            seq: u64::from(n),
            at_ms: now,
            body: Body::Message {
                text: String::new(),
                files: vec![Attachment {
                    id: [n; 32],
                    name: VOICE_NAME.into(),
                    size,
                    kind: FileKind::File,
                    video: None,
                    local: Local::Missing,
                    voice: Some(VoiceInfo {
                        duration_ms: 60_000,
                        bars: vec![1; VOICE_BARS],
                        played: false,
                    }),
                }],
            },
            ttl_ms: 0,
            delivered: true,
        };
        let settings = Settings {
            auto_files: false,
            ..Settings::default()
        };
        let mut chat = Chat::default();
        let charge = |at_ms, bytes| Charge {
            at_ms,
            bytes,
            file: None,
        };
        // Started in the last day: 60 MB, whether they came, were not voice messages or
        // are still on their way; 30 MB more the day before, which no longer count.
        chat.charge(Pool::Voice, charge(now - model::DAY_MS - 1, 30 * MB));
        chat.charge(Pool::Voice, charge(now - 5000, 40 * MB));
        chat.charge(Pool::Voice, charge(now - 1000, 20 * MB));
        assert_eq!(
            allowance(&chat, Pool::Voice, now),
            VOICE_DAILY_BYTES - 60 * MB
        );
        // 4 MB left: a 3 MB one comes, the next 3 MB one waits, a 1 MB one still fits.
        chat.add(voice(4, 3 * MB));
        chat.add(voice(5, 3 * MB));
        chat.add(voice(6, MB));
        assert_eq!(
            due_by_themselves(&chat, &settings, now, &HashSet::new()),
            vec![[4; 32], [6; 32]]
        );
        // A day after the last charge the allowance is whole again.
        assert_eq!(
            due_by_themselves(&chat, &settings, now + model::DAY_MS, &HashSet::new()),
            vec![[4; 32], [5; 32], [6; 32]]
        );
        // Charges more than a day old go as the next is noted.
        chat.charge(Pool::Voice, charge(now + model::DAY_MS, MB));
        assert_eq!(chat.voice_charges.len(), 1);
    }

    #[test]
    fn a_friends_pictures_come_by_themselves_within_a_daily_allowance() {
        const MB: u64 = 1 << 20;
        let now = 10 * model::DAY_MS;
        let picture = |n: u8, size: u64, kind: FileKind| Item {
            id: [n; 16],
            mine: false,
            sent_ms: now,
            got_ms: now,
            seq: u64::from(n),
            at_ms: now,
            body: Body::Message {
                text: String::new(),
                files: vec![Attachment {
                    id: [n; 32],
                    name: format!("photo{n}.jpg"),
                    size,
                    kind,
                    video: None,
                    local: Local::Missing,
                    voice: None,
                }],
            },
            ttl_ms: 0,
            delivered: true,
        };
        let settings = Settings::default();
        let mut chat = Chat::default();
        let charge = |at_ms, bytes, file| Charge {
            at_ms,
            bytes,
            file: Some(file),
        };
        // 505 MB started to come in the last day, 5 MB of it picture 1, still on its way.
        chat.charge(Pool::Files, charge(now - 5000, 500 * MB, [9; 32]));
        chat.charge(Pool::Files, charge(now - 1000, 5 * MB, [1; 32]));
        assert_eq!(
            allowance(&chat, Pool::Files, now),
            FILES_DAILY_BYTES - 505 * MB
        );
        // 7 MB left: picture 1 goes on, picture 2 of 6 MB comes, picture 3 of 2 MB waits, and
        // so does a file, as files do not come by themselves.
        chat.add(picture(1, 5 * MB, FileKind::Picture));
        chat.add(picture(2, 6 * MB, FileKind::Picture));
        chat.add(picture(3, 2 * MB, FileKind::Picture));
        chat.add(picture(4, MB, FileKind::File));
        assert_eq!(
            due_by_themselves(&chat, &settings, now, &HashSet::new()),
            vec![[1; 32], [2; 32]]
        );
        // Picture 1 counts once: noted again, it is not charged again.
        assert!(!chat.charge(Pool::Files, charge(now, 5 * MB, [1; 32])));
        // With files coming by themselves too, they share the allowance.
        let all = Settings {
            auto_files: true,
            ..Settings::default()
        };
        assert_eq!(
            due_by_themselves(&chat, &all, now + model::DAY_MS, &HashSet::new()),
            vec![[1; 32], [2; 32], [3; 32], [4; 32]]
        );
        // One waiting to be tried again neither comes nor counts.
        let waiting = HashSet::from([[1; 32]]);
        assert_eq!(
            due_by_themselves(&chat, &settings, now, &waiting),
            vec![[2; 32]]
        );
        // Voice messages have an allowance of their own.
        assert_eq!(allowance(&chat, Pool::Voice, now), VOICE_DAILY_BYTES);
    }

    #[tokio::test]
    async fn a_download_gives_up_past_its_deadline() {
        assert_eq!(fetch_deadline(0), Duration::from_secs(120));
        assert_eq!(fetch_deadline(10 << 20), Duration::from_secs(120 + 320));
        assert_eq!(
            fetch_limit(false, 10 << 20),
            Limit::Total(Duration::from_secs(440))
        );
        let coming = Coming::new();
        let slow = within(
            Limit::Total(Duration::from_millis(50)),
            &coming,
            std::future::pending::<Result<(), files::FileError>>(),
        );
        let started = Instant::now();
        assert!(slow.await.unwrap_err().contains("took more than"));
        assert!(started.elapsed() < Duration::from_secs(5));
        let quick = within(Limit::Total(Duration::from_secs(5)), &coming, async {
            Ok::<_, files::FileError>(7)
        });
        assert_eq!(quick.await, Ok(7));
    }

    /// A fetch of `pieces` pieces, one every `every`, that stops for good after `stop_at`.
    async fn trickle(
        coming: &Coming,
        pieces: u32,
        every: Duration,
        stop_at: u32,
    ) -> Result<(), files::FileError> {
        for piece in 0..pieces {
            if piece >= stop_at {
                std::future::pending::<()>().await;
            }
            tokio::time::sleep(every).await;
            coming.note();
        }
        Ok(())
    }

    #[tokio::test]
    async fn a_download_asked_for_comes_whole_over_a_slow_link_and_stops_with_the_link() {
        // What the person asked for waits while bytes come, past the deadline of what
        // comes by itself, and gives up two minutes after the last ones.
        assert_eq!(fetch_limit(true, 1 << 30), Limit::Quiet(ASKED_QUIET));
        assert_eq!(ASKED_QUIET, Duration::from_secs(120));
        // The same at a smaller scale: a piece every 20 ms for 0.8 s.
        let (quiet, every) = (Duration::from_millis(150), Duration::from_millis(20));
        let coming = Coming::new();
        let started = Instant::now();
        let fetched = within(
            Limit::Quiet(quiet),
            &coming,
            trickle(&coming, 40, every, 40),
        )
        .await;
        assert_eq!(fetched, Ok(()));
        assert!(started.elapsed() > Duration::from_millis(790));
        let coming = Coming::new();
        let fetched = within(
            Limit::Total(Duration::from_millis(400)),
            &coming,
            trickle(&coming, 40, every, 40),
        )
        .await;
        assert!(fetched.unwrap_err().contains("took more than"));
        // Nothing comes before a first byte, or after some came.
        for stop_at in [0, 10] {
            let coming = Coming::new();
            let started = Instant::now();
            let fetched = within(
                Limit::Quiet(quiet),
                &coming,
                trickle(&coming, 40, every, stop_at),
            )
            .await;
            assert!(fetched.unwrap_err().contains("nothing of it came"));
            let ran = started.elapsed();
            let sent = every * stop_at;
            assert!(ran >= sent + quiet, "{ran:?}");
            assert!(ran < sent + quiet + Duration::from_secs(2), "{ran:?}");
        }
    }

    #[test]
    fn only_what_starts_as_a_picture_is_kept_as_one() {
        for picture in [
            &b"\x89PNG\r\n\x1a\nrest"[..],
            b"\xff\xd8\xff\xe0rest",
            b"GIF87a..",
            b"GIF89a..",
            b"RIFF\x10\0\0\0WEBPVP8 ",
            b"BM\x36\0\0\0",
        ] {
            assert!(looks_like_picture(picture), "{picture:?}");
        }
        for not in [
            &b""[..],
            b"\x89PN",
            b"RIFF\x10\0\0\0WAVEfmt ",
            b"%PDF-1.7",
            b"MZ\x90\0",
            b"<svg xmlns",
        ] {
            assert!(!looks_like_picture(not), "{not:?}");
        }
    }

    /// A burst of tiny voice messages: each counts at least 64 KiB against the allowance,
    /// so no more than 1024 come by themselves in a day.
    #[test]
    fn tiny_voice_messages_count_at_least_a_little() {
        let now = 10 * model::DAY_MS;
        let mut chat = Chat::default();
        for n in 0..1500u32 {
            let mut id = [0; 32];
            id[..4].copy_from_slice(&n.to_be_bytes());
            chat.add(Item {
                id: id[..16].try_into().unwrap(),
                mine: false,
                sent_ms: now,
                got_ms: now,
                seq: 0,
                at_ms: 0,
                body: Body::Message {
                    text: String::new(),
                    files: vec![Attachment {
                        id,
                        name: VOICE_NAME.into(),
                        size: 10,
                        kind: FileKind::File,
                        video: None,
                        local: Local::Missing,
                        voice: Some(VoiceInfo {
                            duration_ms: 1000,
                            bars: vec![1; VOICE_BARS],
                            played: false,
                        }),
                    }],
                },
                ttl_ms: 0,
                delivered: true,
            });
        }
        let settings = Settings::default();
        assert_eq!(
            due_by_themselves(&chat, &settings, now, &HashSet::new()).len(),
            1024
        );
    }

    #[test]
    fn a_voice_message_is_kept_only_when_it_looks_like_one() {
        let voice = |name: &str, size: u64, kind, duration_ms, bars: usize| {
            let payload = DirectPayload::Message {
                id: [1; 16],
                sent_ms: 5,
                text: String::new(),
                files: vec![DirectFile {
                    id: [2; 32],
                    name: name.into(),
                    size,
                    kind,
                    video: None,
                    voice: Some(WireVoice {
                        duration_ms,
                        bars: vec![9; bars],
                    }),
                }],
                ttl_ms: 0,
            };
            received(payload, 1).unwrap().files()[0].voice.clone()
        };
        use DirectFileKind::{File, Picture};
        let fine = voice(VOICE_NAME, 60_000, File, 18_000, VOICE_BARS).unwrap();
        assert_eq!(
            (fine.duration_ms, fine.bars.len(), fine.played),
            (18_000, VOICE_BARS, false)
        );
        assert!(voice("Voice.OGG", 60_000, File, MAX_VOICE_MS + 1000, VOICE_BARS).is_some());
        // Anything off and it is the file it is.
        assert!(voice("voice.mp3", 60_000, File, 18_000, VOICE_BARS).is_none());
        assert!(voice(VOICE_NAME, MAX_VOICE_BYTES + 1, File, 18_000, VOICE_BARS).is_none());
        assert!(voice(VOICE_NAME, 0, File, 18_000, VOICE_BARS).is_none());
        assert!(voice(VOICE_NAME, 60_000, Picture, 18_000, VOICE_BARS).is_none());
        assert!(voice(VOICE_NAME, 60_000, File, 0, VOICE_BARS).is_none());
        assert!(voice(VOICE_NAME, 60_000, File, MAX_VOICE_MS + 1001, VOICE_BARS).is_none());
        assert!(voice(VOICE_NAME, 60_000, File, 18_000, VOICE_BARS - 1).is_none());
        assert!(voice(VOICE_NAME, 60_000, File, 18_000, 4096).is_none());
        // What this device sends carries it.
        let mine = Item {
            id: [4; 16],
            mine: true,
            sent_ms: 9,
            got_ms: 9,
            seq: 1,
            at_ms: 9,
            body: Body::Message {
                text: String::new(),
                files: vec![Attachment {
                    id: [6; 32],
                    name: VOICE_NAME.into(),
                    size: 100,
                    kind: FileKind::File,
                    video: None,
                    local: Local::Here,
                    voice: Some(VoiceInfo {
                        duration_ms: 1500,
                        bars: vec![1; VOICE_BARS],
                        played: true,
                    }),
                }],
            },
            ttl_ms: 0,
            delivered: false,
        };
        let back = received(payload(&mine).unwrap(), 10).unwrap();
        let voice = back.files()[0].voice.as_ref().unwrap();
        assert_eq!((voice.duration_ms, voice.played), (1500, false));
        assert_eq!(model::voice_preview(&back), Some(1500));
        assert_eq!(
            model::preview(&back, "Ana", timer_words),
            "Voice message · 0:01"
        );
    }
}
