//! Files of a room fetched from the members that hold them: saved to the downloads folder
//! under a name of their own that never replaces another file, played or previewed from a
//! temporary copy, asked again of another holder or later a few times when it fails.

use std::{
    collections::HashSet,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{DevicePublicKey, GroupId, names};
use fogata_net::{RoomView, files};
use fogata_state::VoiceRoomId;

use crate::{
    Core, CoreEvent, Inner,
    active_files::{INERT_ENDING, acts_on_its_own},
    direct, shares,
};

/// Rounds over a file's holders that waited because each said it sends more only later
/// (see [`LaterRounds`]): a few, each waiting longer, and an hour in all at most.
const LATER_ROUNDS: u32 = 5;
const LATER_FIRST: std::time::Duration = std::time::Duration::from_secs(2);
const LATER_MOST: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The waits of a download whose holders say they send more later: as long as they ask,
/// and at least twice the one before; past [`LATER_ROUNDS`] or [`LATER_MOST`] in all the
/// download stops, rather than wait without end on a holder that always says later.
#[derive(Debug, Default)]
pub(crate) struct LaterRounds {
    rounds: u32,
    waited: std::time::Duration,
}

impl LaterRounds {
    /// The wait before the next round, when the holders asked to wait `asked`; `None` once
    /// the download has waited enough.
    pub(crate) fn next(&mut self, asked: std::time::Duration) -> Option<std::time::Duration> {
        // Asked to wait past what is left of the most: no use waiting at all.
        if self.rounds >= LATER_ROUNDS || asked > LATER_MOST.saturating_sub(self.waited) {
            return None;
        }
        let grows = LATER_FIRST * 2u32.pow(self.rounds);
        let wait = asked.max(grows).min(LATER_MOST - self.waited);
        self.rounds += 1;
        self.waited += wait;
        Some(wait)
    }
}

/// Pauses before each further round over a file's holders, when none of them could send it
/// in the round before.
const FETCH_RETRIES: [std::time::Duration; 2] = [
    std::time::Duration::from_secs(2),
    std::time::Duration::from_secs(5),
];

/// How a fetched file takes its place.
#[derive(Clone, Copy)]
enum Place {
    /// In place of whatever is there: a folder of the application's own.
    Replace,
    /// Never over anything there, under the next free name then: a folder people use.
    New,
}

/// A video of a room being fetched to play: where it went, or why not, once it ends.
pub(crate) type Playing =
    futures::future::Shared<tokio::sync::oneshot::Receiver<std::result::Result<PathBuf, String>>>;

/// Space a room file fetched here leaves free on its disk.
pub(crate) const SPACE_KEPT_FREE: u64 = 256 * 1024 * 1024;

/// The memory the previews being fetched may take together, in KiB: each holds its file
/// whole, and a room with many pictures asks for them all at once. Past it, a preview
/// waits for one before it to end.
pub(crate) const PREVIEWS_KIB: u32 = 48 * 1024;

/// [`Core::fetch_to_play`]'s fetch: the video at its place in `dir`, unless it is there
/// already. Given up, and what came of it removed, once this device is in no room of
/// `group`: it left, was taken out, or the group went.
async fn fetch_video(
    inner: &Arc<Inner>,
    group: GroupId,
    entry: &fogata_net::RoomFile,
    dir: &FsPath,
) -> std::result::Result<PathBuf, String> {
    let file = entry.info.id;
    let target = dir.join(direct::store::kept_name(&file, &entry.info.name));
    let fetching = async {
        // One fetched before is taken again only if it is still that file.
        if std::fs::metadata(&target).is_ok_and(|m| m.len() == entry.info.size) {
            let kept = target.clone();
            let hashed = tokio::task::spawn_blocking(move || files::hash_file(&kept)).await;
            if hashed.is_ok_and(|hashed| hashed.is_ok_and(|(id, _)| id == file)) {
                return Ok(target.clone());
            }
        }
        let _room = shares::hold_disk(
            inner,
            entry.info.size,
            dir,
            "not enough free space for this file",
        )
        .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fetch_from_holders(inner, group, entry, &target, Place::Replace).await
    };
    tokio::select! {
        fetched = fetching => fetched,
        () = out_of_room(inner, group) => {
            let part = files::part_path(&target, &file);
            inner.lock().part_from.remove(&part);
            // Once the work the fetch started on it has ended, which nothing else can start
            // meanwhile.
            if let Err(e) = files::remove_part(&part).await {
                tracing::debug!("cannot remove a video that stopped coming: {e}");
            }
            Err("you left the room".into())
        }
    }
}

/// How often a video fetched to play looks whether this device is still in its room.
const ROOM_CHECK: std::time::Duration = std::time::Duration::from_millis(500);

/// Ends once this device is in no room of `group`, whichever way it went out; it looks at
/// once when this device leaves a room on purpose.
async fn out_of_room(inner: &Inner, group: GroupId) {
    let left = inner.lock().left_room.clone();
    loop {
        tokio::select! {
            () = tokio::time::sleep(ROOM_CHECK) => {}
            () = left.notified() => {}
        }
        match inner.node.rooms(group).await {
            Ok(rooms) if rooms.iter().any(|(_, room)| room.joined) => {}
            _ => return,
        }
    }
}

/// Fetches a room's file into `target` from whoever has it, sending progress events, and
/// returns where it went. When no holder can send it, it tries again a little later, while
/// the file is still in the room; each try goes on from what arrived before.
async fn fetch_from_holders(
    inner: &Arc<Inner>,
    group: GroupId,
    entry: &fogata_net::RoomFile,
    target: &FsPath,
    place: Place,
) -> std::result::Result<PathBuf, String> {
    let (file, total) = (entry.info.id, entry.info.size);
    let _fetching = Fetching::start(inner, (group, file));
    let mut holders = entry.holders.clone();
    let part = files::part_path(target, &file);
    // Holders whose download stopped before the end are asked again only after the others;
    // one that sent something that is not the file, not again.
    let (mut cut, mut wrong) = (Vec::new(), HashSet::new());
    let mut result = Err("nobody connected has it".to_string());
    // Holders that sent this device all they send it in a day are asked again when the
    // first of them can send more: the download waits, it does not fail.
    let mut later: Option<std::time::Duration> = None;
    let mut later_rounds = LaterRounds::default();
    let mut round = 0;
    'rounds: while round <= FETCH_RETRIES.len() {
        let wait = match later.take() {
            Some(asked) => match later_rounds.next(asked) {
                Some(wait) => Some(wait),
                None => {
                    result = Err("its holders send it only later: try again then".to_string());
                    break;
                }
            },
            None if round > 0 => Some(FETCH_RETRIES[round - 1]),
            None => None,
        };
        if let Some(wait) = wait {
            tokio::time::sleep(wait).await;
            let rooms = inner.node.rooms(group).await.unwrap_or_default();
            match find_file(rooms, file) {
                // Whoever has it now.
                Some(listed) => holders = listed.holders,
                None => {
                    result = Err("that file is no longer in the room".to_string());
                    break;
                }
            }
        }
        let listed: Vec<DevicePublicKey> = holders.iter().copied().collect();
        for holder in &ask_order(&listed, &cut, &wrong) {
            let Ok(Some(peer)) = inner.node.member_peer(*holder).await else {
                continue;
            };
            go_on_from(inner, &part, *holder).await;
            let mut last_step = 0;
            let progress = |got: u64| {
                inner.lock().downloads.insert((group, file), got);
                let step = got * 20 / total.max(1);
                if step > last_step {
                    last_step = step;
                    let _ = inner.events.send(CoreEvent::Download {
                        group,
                        file,
                        received: got,
                        total,
                    });
                }
            };
            let fetched = match place {
                Place::Replace => {
                    files::fetch(&inner.node, group, peer, file, total, target, progress)
                        .await
                        .map(|()| target.to_path_buf())
                }
                Place::New => {
                    files::fetch_new(&inner.node, group, peer, file, total, target, progress).await
                }
            };
            match fetched {
                Ok(path) => {
                    inner.lock().part_from.remove(&part);
                    result = Ok(path);
                    break 'rounds;
                }
                Err(files::FileError::Later(wait)) => {
                    tracing::debug!("{} sends more later", holder.fingerprint());
                    later = Some(later.map_or(wait, |l| l.min(wait)));
                    result = Err(files::FileError::Later(wait).to_string());
                }
                Err(e) => {
                    tracing::debug!("download from {} stopped: {e}", holder.fingerprint());
                    if matches!(e, files::FileError::Damaged) {
                        wrong.insert(*holder);
                    } else if !cut.contains(holder) {
                        cut.push(*holder);
                    }
                    result = Err(e.to_string());
                }
            }
        }
        if later.is_none() {
            round += 1;
        }
    }
    result
}

/// A fetch of a room's file counted while it runs ([`State::fetching`](crate::State::fetching)), however it ends:
/// given up too, as a video fetched to play is once its room is left.
pub(crate) struct Fetching<'a> {
    pub(crate) inner: &'a Inner,
    pub(crate) key: (GroupId, [u8; 32]),
}

/// The order a room file's holders are asked in: those whose download stopped before (`cut`)
/// after the others, in the order they stopped, and never one that sent something that was
/// not the file (`wrong`).
fn ask_order(
    holders: &[DevicePublicKey],
    cut: &[DevicePublicKey],
    wrong: &HashSet<DevicePublicKey>,
) -> Vec<DevicePublicKey> {
    holders
        .iter()
        .filter(|h| !wrong.contains(h) && !cut.contains(h))
        .chain(
            cut.iter()
                .filter(|h| holders.contains(h) && !wrong.contains(h)),
        )
        .copied()
        .collect()
}

/// Lets a download from `holder` go on from the `.part` at `part` only if that holder sent
/// what it holds: one that another holder (or nobody known, as after a restart) left is
/// cut away, so a holder that sent wrong bytes and stopped does not spoil the download from
/// the next. The `.part` is then noted as `holder`'s.
async fn go_on_from(inner: &Inner, part: &FsPath, holder: DevicePublicKey) {
    let theirs = inner.lock().part_from.get(part) == Some(&holder);
    // One that a download here, or the work of one given up, still writes is left to it,
    // still another's: this download then fails, and is tried again later.
    if !theirs {
        match files::remove_free_part(part).await {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => tracing::warn!("cannot remove a partial download: {e}"),
        }
    }
    inner.lock().part_from.insert(part.to_path_buf(), holder);
}

/// A file listed in one of a group's voice rooms, with who has it.
fn find_file(rooms: Vec<(VoiceRoomId, RoomView)>, file: [u8; 32]) -> Option<fogata_net::RoomFile> {
    rooms
        .into_iter()
        .flat_map(|(_, room)| room.files)
        .find(|f| f.info.id == file)
}

/// `dir/name`, or `dir/name (2)` and so on if that exists, with `name` made safe as
/// [`saved_name`] does.
pub fn free_file_name(dir: &FsPath, name: &str) -> PathBuf {
    free_file_name_except(dir, name, &HashSet::new())
}

/// The name a file someone else gave the name `name` is saved under. Only the last part of
/// `name` is used, without anything that could reach outside the folder. Marks that hide in
/// a name or turn it around are left out too: `photo\u{202E}gnp.exe` would show as
/// `photoexe.png`. So are the characters Windows refuses in a name (`< > " | ? *`),
/// everywhere, so a file keeps its name from one system to another, and dots at the start,
/// which would hide the file, or make it one a program reads for its settings (`.bashrc`).
/// A file that acts on its own when a file manager shows it or when it is opened
/// (`link.url`, `start.desktop`) gets [`INERT_ENDING`] added (`link.url.txt`), so it opens as
/// the text it is.
pub fn saved_name(name: &str) -> String {
    // The last part after either system's separator, so a name saves alike everywhere.
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let clean: String = last
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'))
        .collect();
    let mut clean = fair_name(&clean, MAX_SAVED_NAME);
    if acts_on_its_own(&clean) {
        clean = format!(
            "{}{INERT_ENDING}",
            fair_name(&clean, MAX_SAVED_NAME - INERT_ENDING.len())
        );
    }
    if windows_device_name(&clean) {
        format!("_{clean}")
    } else {
        clean
    }
}

/// `name` as [`fair_file_name`] makes it, at most `max` bytes long, without dots and spaces
/// at the start nor at the end: Windows drops those at the end, so the file would land
/// under another name than the one shown. "file" when nothing is left.
fn fair_name(name: &str, max: usize) -> String {
    let fair = fair_file_name(name, max);
    match fair.trim_matches(['.', ' ']) {
        "" => "file".to_string(),
        kept => kept.to_string(),
    }
}

/// [`free_file_name`], also leaving out the paths in `taken`.
fn free_file_name_except(dir: &FsPath, name: &str, taken: &HashSet<PathBuf>) -> PathBuf {
    let clean = saved_name(name);
    // A link counts as taken, even one to nothing.
    let free = |p: &PathBuf| std::fs::symlink_metadata(p).is_err() && !taken.contains(p);
    let first = dir.join(&clean);
    if free(&first) {
        return first;
    }
    let (stem, ext) = match clean.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (clean.clone(), String::new()),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(free)
        .expect("some name is free")
}

/// Saves what `source` reads (a kept attachment, through its seal when it has one, or a
/// file of a room) into `dir` as `name` (see [`free_file_name`]), or the next free name then:
/// never over a file, nor through a link, that is there or appeared meanwhile. The copy is
/// marked as downloaded (see [`mark_as_downloaded`]). Returns where it went.
pub fn save_received(
    source: &mut impl std::io::Read,
    dir: &FsPath,
    name: &str,
) -> std::io::Result<PathBuf> {
    for _ in 0..100 {
        let dest = free_file_name(dir, name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)
        {
            Ok(mut out) => {
                return match std::io::copy(source, &mut out).and_then(|_| out.sync_all()) {
                    Ok(()) => {
                        drop(out);
                        mark_or_warn(&dest);
                        Ok(dest)
                    }
                    Err(e) => {
                        drop(out);
                        let _ = std::fs::remove_file(&dest);
                        Err(e)
                    }
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("no free name to save it under"))
}

/// Whether Windows takes `name` for a device rather than a file (`nul.mp4`, `COM1.txt`):
/// whatever is written there goes nowhere, and the download fails. Left out everywhere, so a
/// file keeps its name from one system to another.
fn windows_device_name(name: &str) -> bool {
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let upper = base.to_ascii_uppercase();
    if matches!(upper.as_str(), "CONIN$" | "CONOUT$") {
        return true;
    }
    let mut chars = upper.chars();
    match (
        chars.next(),
        chars.next(),
        chars.next(),
        chars.next(),
        chars.next(),
    ) {
        (Some(a), Some(b), Some(c), None, _) => matches!(
            [a, b, c],
            ['C', 'O', 'N'] | ['P', 'R', 'N'] | ['A', 'U', 'X'] | ['N', 'U', 'L']
        ),
        (Some(a), Some(b), Some(c), Some(d), None) => {
            matches!([a, b, c], ['C', 'O', 'M'] | ['L', 'P', 'T'])
                && (d.is_ascii_digit() || matches!(d, '\u{00B9}' | '\u{00B2}' | '\u{00B3}'))
        }
        _ => false,
    }
}

/// Longest name of a file shared in a room, in bytes, as the room takes it.
pub(crate) const MAX_SHARED_NAME: usize = 255;
/// Longest name a download is saved under, in bytes: file systems take 255 at most, and a
/// download also needs room for its `.part` ending while it arrives and for a " (2)" that
/// tells it from a file of the same name. On Windows shorter: a path there takes 260
/// characters unless the system allows long ones, and the downloads folder comes before
/// the name.
#[cfg(not(windows))]
const MAX_SAVED_NAME: usize = 255 - 24;
#[cfg(windows)]
const MAX_SAVED_NAME: usize = 120;
/// Longest extension kept when a name is shortened, dot included.
const MAX_EXTENSION: usize = 16;

/// `name` without control characters nor marks that hide in text or turn it around, as it
/// shows (no padding of blank characters), and at most `max` bytes long, cut where a
/// character ends and keeping its extension; "file" when nothing is left.
pub(crate) fn fair_file_name(name: &str, max: usize) -> String {
    let clean: String = name
        .chars()
        .filter(|c| !c.is_control() && !fogata_state::hiding_mark(*c))
        .collect();
    let clean = names::tidy(&clean);
    let clean = match clean.trim_matches('.') {
        "" => return "file".to_string(),
        _ => clean,
    };
    if clean.len() <= max {
        return clean;
    }
    let (stem, extension) = match clean.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() < MAX_EXTENSION => {
            (stem, format!(".{ext}"))
        }
        _ => (clean.as_str(), String::new()),
    };
    let mut cut = max.saturating_sub(extension.len());
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{extension}", &stem[..cut])
}

/// Marks a file that came from someone else as downloaded from the internet, so that
/// Windows asks before running it, as it does for a browser's downloads. Nothing happens
/// elsewhere: macOS marks what the application writes by itself (`LSFileQuarantineEnabled`).
/// Fails where the disk keeps no such marks (FAT and exFAT drives, some network shares):
/// the file is there, and nothing will ask before running it.
pub fn mark_as_downloaded(path: &FsPath) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut stream = path.as_os_str().to_owned();
        stream.push(":Zone.Identifier");
        std::fs::write(stream, "[ZoneTransfer]\r\nZoneId=3\r\n")
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(())
    }
}

/// [`mark_as_downloaded`], noting in the log when the mark could not be written: the file is
/// kept, as a browser keeps it then.
pub(crate) fn mark_or_warn(path: &FsPath) {
    if let Err(e) = mark_as_downloaded(path) {
        tracing::warn!(
            "{} is saved without the mark that it was downloaded: {e}",
            path.display()
        );
    }
}

impl Core {
    /// Starts downloading a file of a group's room into `dir`, from whoever in the room has
    /// it; progress and the result come as events. Returns where it will be saved.
    pub async fn download(&self, group: GroupId, file: [u8; 32], dir: &FsPath) -> Result<PathBuf> {
        let entry = self.room_file(group, file).await?;
        let room = shares::hold_disk(
            &self.inner,
            entry.info.size,
            dir,
            "not enough free space for this file",
        )?;
        let dest = {
            let mut state = self.inner.lock();
            // Clicked twice: one download, not two copies.
            if let Some(dest) = state.saving_files.get(&(group, file)) {
                return Ok(dest.clone());
            }
            let dest = free_file_name_except(dir, &entry.info.name, &state.saving);
            state.saving.insert(dest.clone());
            state.saving_files.insert((group, file), dest.clone());
            dest
        };
        let inner = self.inner.clone();
        let target = dest.clone();
        tokio::spawn(async move {
            let fetched = fetch_from_holders(&inner, group, &entry, &target, Place::New).await;
            // Made, or given up: the file takes its room itself, or none.
            drop(room);
            {
                let mut state = inner.lock();
                state.saving.remove(&target);
                state.saving_files.remove(&(group, file));
            }
            match fetched {
                Ok(target) => {
                    mark_or_warn(&target);
                    let _ = inner.events.send(CoreEvent::DownloadDone {
                        group,
                        file,
                        path: target.clone(),
                    });
                    shares::share_download(&inner, group, &entry.info, &target).await;
                }
                Err(reason) => {
                    let _ = inner.events.send(CoreEvent::DownloadFailed {
                        group,
                        file,
                        reason,
                    });
                }
            }
        });
        Ok(dest)
    }

    /// Fetches a video of the room to play it, into `dir` under its id, unless it is already
    /// there: this device does not start sharing it, and only progress events are sent.
    /// Returns where it is. Asked again into the same folder while it comes, the same fetch
    /// is waited for; it stops, and what came of it goes, once this device is in no room of
    /// the group.
    pub async fn fetch_to_play(
        &self,
        group: GroupId,
        file: [u8; 32],
        dir: &FsPath,
    ) -> Result<PathBuf> {
        let entry = self.room_file(group, file).await?;
        let key = (group, file, dir.to_path_buf());
        let playing = {
            let mut state = self.inner.lock();
            match state.playing.get(&key) {
                Some(playing) => playing.clone(),
                None => {
                    let (done, playing) = tokio::sync::oneshot::channel();
                    let playing = futures::FutureExt::shared(playing);
                    state.playing.insert(key.clone(), playing.clone());
                    let inner = self.inner.clone();
                    let key = key.clone();
                    tokio::spawn(async move {
                        let fetched = fetch_video(&inner, group, &entry, &key.2).await;
                        inner.lock().playing.remove(&key);
                        let _ = done.send(fetched);
                    });
                    playing
                }
            }
        };
        match playing.clone().await {
            Ok(fetched) => fetched.map_err(anyhow::Error::msg),
            Err(_) => {
                // The fetch went without a word: the next ask starts another.
                let mut state = self.inner.lock();
                if state.playing.get(&key).is_some_and(|p| p.ptr_eq(&playing)) {
                    state.playing.remove(&key);
                }
                bail!("the video stopped coming")
            }
        }
    }

    /// Lets the room downloads check the free space of their disk with `free_space`, as the
    /// system tells it (this crate makes no system calls): one that would not fit is not
    /// started.
    pub fn use_free_space(&self, free_space: fn(&FsPath) -> Option<u64>) {
        self.inner.lock().free_space = Some(free_space);
    }

    /// Fetches a file of the room into memory for a preview, if it is at most `max_bytes`:
    /// nothing is kept on disk, this device does not start sharing it, and no download
    /// events are sent.
    pub async fn fetch_preview(
        &self,
        group: GroupId,
        file: [u8; 32],
        max_bytes: u64,
    ) -> Result<Vec<u8>> {
        let entry = self.room_file(group, file).await?;
        anyhow::ensure!(entry.info.size <= max_bytes, "too big for a preview");
        let _turn = self.preview_turn(entry.info.size).await?;
        let mut last = anyhow::anyhow!("nobody connected has it");
        for holder in &entry.holders {
            let Ok(Some(peer)) = self.inner.node.member_peer(*holder).await else {
                continue;
            };
            match files::fetch_to_memory(&self.inner.node, group, peer, file, entry.info.size).await
            {
                Ok(bytes) => return Ok(bytes),
                Err(e) => last = e.into(),
            }
        }
        Err(last)
    }

    /// Holds the memory of a preview of `size` bytes among the previews being fetched
    /// ([`PREVIEWS_KIB`]), waiting for it if need be; one bigger than all of it holds all.
    async fn preview_turn(&self, size: u64) -> Result<tokio::sync::SemaphorePermit<'_>> {
        let kib = size.div_ceil(1024).clamp(1, u64::from(PREVIEWS_KIB)) as u32;
        Ok(self.inner.previews.acquire_many(kib).await?)
    }

    /// A file listed in one of a group's voice rooms, with who has it.
    async fn room_file(&self, group: GroupId, file: [u8; 32]) -> Result<fogata_net::RoomFile> {
        find_file(self.inner.node.rooms(group).await?, file)
            .context("that file is no longer in the room")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use fogata_crypto::DeviceIdentity;

    use super::*;

    /// Previews being fetched hold their memory together: past what they may take, the
    /// next waits for one to end; one bigger than all of it still goes, alone.
    #[tokio::test(flavor = "multi_thread")]
    async fn previews_share_a_memory_budget() {
        let dir = std::env::temp_dir().join(format!("fogata-previews-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = crate::NetOptions {
            listen_port: 0,
            use_dht: false,
            use_mdns: false,
            ..Default::default()
        };
        let (core, _events) = Core::start(dir.clone(), options).unwrap();
        let ten = 10 << 20;
        let wait = std::time::Duration::from_millis(200);
        let held: Vec<_> = futures::future::join_all((0..4).map(|_| core.preview_turn(ten)))
            .await
            .into_iter()
            .map(Result::unwrap)
            .collect();
        assert!(
            tokio::time::timeout(wait, core.preview_turn(ten))
                .await
                .is_err(),
            "a fifth 10 MB preview did not wait"
        );
        // Small ones still fit beside them.
        let small = tokio::time::timeout(wait, core.preview_turn(100_000)).await;
        assert!(small.is_ok_and(|turn| turn.is_ok()));
        drop(held);
        let huge = tokio::time::timeout(wait, core.preview_turn(u64::MAX)).await;
        assert!(huge.is_ok_and(|turn| turn.is_ok()));
        core.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A holder that always says "later" makes a download wait a few times, longer each
    /// time, and then stop: never without end, and never past an hour in all.
    #[test]
    fn a_download_waits_for_later_a_few_times_and_then_stops() {
        let second = std::time::Duration::from_secs(1);
        let mut rounds = LaterRounds::default();
        let mut waits = Vec::new();
        while let Some(wait) = rounds.next(second) {
            waits.push(wait.as_secs());
        }
        assert_eq!(waits, [2, 4, 8, 16, 32]);
        // Holders asking for a day, past the most: it stops at once, rather than wait an
        // hour for nothing.
        let mut rounds = LaterRounds::default();
        let day = std::time::Duration::from_secs(24 * 60 * 60);
        assert_eq!(rounds.next(day), None);
        // Asking for what is left: waited, then no more.
        let mut rounds = LaterRounds::default();
        assert_eq!(rounds.next(LATER_MOST), Some(LATER_MOST));
        assert_eq!(rounds.next(second), None);
    }

    #[test]
    fn a_holder_that_stopped_is_asked_last_and_one_that_sent_another_file_never() {
        let key = || DeviceIdentity::generate().unwrap().public_key();
        let (ana, leo, sofia) = (key(), key(), key());
        let holders = [ana, leo, sofia];
        let none = HashSet::new();
        assert_eq!(ask_order(&holders, &[], &none), holders);
        assert_eq!(ask_order(&holders, &[ana], &none), [leo, sofia, ana]);
        assert_eq!(
            ask_order(&holders, &[leo, ana], &HashSet::from([sofia])),
            [leo, ana]
        );
        // One that is no longer a holder is not asked.
        assert_eq!(ask_order(&[leo], &[ana], &none), [leo]);
    }

    #[test]
    fn a_saved_copy_never_replaces_a_file_nor_goes_through_a_link() {
        let dir = std::env::temp_dir().join(format!("fogata-copy-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let from = dir.join("kept");
        std::fs::write(&from, "theirs").unwrap();
        let saves = dir.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("plan.pdf"), "mine").unwrap();
        let to =
            save_received(&mut std::fs::File::open(&from).unwrap(), &saves, "plan.pdf").unwrap();
        assert_eq!(to, saves.join("plan (2).pdf"));
        assert_eq!(
            std::fs::read_to_string(saves.join("plan.pdf")).unwrap(),
            "mine"
        );
        assert_eq!(std::fs::read_to_string(&to).unwrap(), "theirs");
        #[cfg(unix)]
        {
            // A link to nothing counts as taken: writing there would make the file it names.
            let outside = dir.join("outside");
            std::os::unix::fs::symlink(&outside, saves.join("photo.png")).unwrap();
            let to = save_received(
                &mut std::fs::File::open(&from).unwrap(),
                &saves,
                "photo.png",
            )
            .unwrap();
            assert_eq!(to, saves.join("photo (2).png"));
            assert!(!outside.exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// On Windows a received file carries the mark of a download, and one that acts by
    /// itself (here a shortcut to a share) is saved as text, which Explorer only shows.
    #[cfg(windows)]
    #[test]
    fn a_received_file_is_marked_as_downloaded_and_a_shortcut_saved_as_text() {
        let dir = std::env::temp_dir().join(format!("fogata-marked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let from = dir.join("kept");
        std::fs::write(&from, "[InternetShortcut]\r\nURL=file://server/share\r\n").unwrap();
        let saves = dir.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        for (name, saved) in [
            ("invoice.url", "invoice.url.txt"),
            ("Report.LNK. ", "Report.LNK.txt"),
            ("plan.pdf", "plan.pdf"),
        ] {
            let to = save_received(&mut std::fs::File::open(&from).unwrap(), &saves, name).unwrap();
            assert_eq!(to, saves.join(saved), "{name}");
            let mut mark = to.as_os_str().to_owned();
            mark.push(":Zone.Identifier");
            assert_eq!(
                std::fs::read_to_string(mark).unwrap(),
                "[ZoneTransfer]\r\nZoneId=3\r\n",
                "{name}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn downloads_stay_in_their_folder() {
        let dir = std::env::temp_dir().join(format!("fogata-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_file_name(&dir, "../../etc/passwd"), dir.join("passwd"));
        assert_eq!(free_file_name(&dir, ".."), dir.join("file"));
        assert_eq!(free_file_name(&dir, "a\nb.txt"), dir.join("ab.txt"));
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        assert_eq!(free_file_name(&dir, "notes.txt"), dir.join("notes (2).txt"));
        // A name made to show as something else keeps its real ending.
        assert_eq!(
            free_file_name(&dir, "photo\u{202E}gnp.exe"),
            dir.join("photognp.exe")
        );
        // Names Windows keeps for devices, whatever their ending.
        for (shared, saved) in [
            ("nul.mp4", "_nul.mp4"),
            ("CON", "_CON"),
            ("com1.txt", "_com1.txt"),
            ("Lpt9.tar.gz", "_Lpt9.tar.gz"),
            ("aux .png", "_aux .png"),
            ("console.txt", "console.txt"),
            ("com10.txt", "com10.txt"),
            ("conin$", "_conin$"),
            ("CONOUT$.log", "_CONOUT$.log"),
        ] {
            assert_eq!(free_file_name(&dir, shared), dir.join(saved), "{shared}");
        }
        // Dots and spaces at the end, which Windows would drop.
        for (shared, saved) in [
            ("report.txt.", "report.txt"),
            ("notes . .", "notes"),
            ("nul. ", "_nul"),
            (". .", "file"),
        ] {
            assert_eq!(free_file_name(&dir, shared), dir.join(saved), "{shared}");
        }
        // Characters Windows refuses in a name.
        for (shared, saved) in [
            ("what?.png", "what.png"),
            ("a|b.txt", "ab.txt"),
            ("<draft> \"final\" *v2*.docx", "draft final v2.docx"),
            ("???", "file"),
        ] {
            assert_eq!(free_file_name(&dir, shared), dir.join(saved), "{shared}");
        }
        // Two downloads of the same name at once land in two places.
        let saving = HashSet::from([dir.join("clip.mp4")]);
        assert_eq!(
            free_file_name_except(&dir, "clip.mp4", &saving),
            dir.join("clip (2).mp4")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_file_that_acts_on_its_own_is_saved_as_text() {
        use crate::active_files::ACTIVE_ENDINGS;
        for ending in ACTIVE_ENDINGS {
            let upper = ending.to_ascii_uppercase();
            for (shared, saved) in [
                (format!("a.{ending}"), format!("a.{ending}.txt")),
                (format!("Link.{upper}"), format!("Link.{upper}.txt")),
                (format!("x.{ending}."), format!("x.{ending}.txt")),
                (format!("x.{ending} "), format!("x.{ending}.txt")),
                (format!("x.{ending} . ."), format!("x.{ending}.txt")),
                (
                    format!("invoice.pdf.{ending}"),
                    format!("invoice.pdf.{ending}.txt"),
                ),
                (format!(".{ending}"), ending.to_string()),
                (format!("..x.{ending}"), format!("x.{ending}.txt")),
                (format!("nul.{ending}"), format!("_nul.{ending}.txt")),
            ] {
                assert_eq!(saved_name(&shared), saved, "{shared}");
            }
        }
        for (shared, saved) in [
            ("desktop.ini", "desktop.ini.txt"),
            ("Desktop.INI", "Desktop.INI.txt"),
            ("autorun.inf.", "autorun.inf.txt"),
            ("C:\\Users\\Public\\desktop.ini", "desktop.ini.txt"),
            ("photo.jpg.lnk", "photo.jpg.lnk.txt"),
            ("photo\u{202E}knl.jpg", "photoknl.jpg"),
            ("start\u{200B}.desktop\u{3164}", "start.desktop.txt"),
            ("a.url:Zone.Identifier", "a.urlZone.Identifier"),
        ] {
            assert_eq!(saved_name(shared), saved, "{shared}");
        }
        // Another of the same name: the number goes before the ending that makes it text.
        let dir = std::env::temp_dir().join(format!("fogata-active-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("share.URL.txt"), b"x").unwrap();
        assert_eq!(
            free_file_name(&dir, "share.URL"),
            dir.join("share.URL (2).txt")
        );
        // A name of the longest length still ends as text.
        let long = format!("{}.lnk", "写".repeat(81));
        let saved = saved_name(&long);
        assert!(saved.len() <= MAX_SAVED_NAME, "{}", saved.len());
        assert!(saved.ends_with(".lnk.txt"), "{saved}");
        // Saved through the core, it lands as text.
        let to = save_received(&mut &b"[InternetShortcut]"[..], &dir, "open me.url").unwrap();
        assert_eq!(to, dir.join("open me.url.txt"));
        assert_eq!(std::fs::read(&to).unwrap(), b"[InternetShortcut]");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_name_never_starts_with_a_dot() {
        for (shared, saved) in [
            (".bashrc", "bashrc"),
            (".profile", "profile"),
            ("..profile", "profile"),
            (". hidden.txt", "hidden.txt"),
            (".directory", "directory"),
            ("/home/ana/.ssh", "ssh"),
            ("...", "file"),
            (". .", "file"),
            (".nul", "_nul"),
        ] {
            assert_eq!(saved_name(shared), saved, "{shared}");
        }
    }

    #[test]
    fn ordinary_names_are_saved_as_they_are() {
        for name in [
            "photo.jpg",
            "IMG_2041.HEIC",
            "Screenshot 2026-11-06 at 10.30.12.png",
            "plan.pdf",
            "backup.tar.gz",
            "project.zip",
            "report final (v2).docx",
            "budget.xlsx",
            "slides.pptx",
            "notes.txt",
            "song.mp3",
            "voice note.ogg",
            "clip.mp4",
            "setup.exe",
            "my.url.txt",
            "desktop",
            "theme park.png",
            "한글 문서.hwp",
            "Café meeting 10.30.MP4",
        ] {
            assert_eq!(saved_name(name), name, "{name}");
        }
    }

    #[test]
    fn a_download_with_the_longest_name_can_be_saved() {
        let dir = std::env::temp_dir().join(format!("fogata-long-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 81 characters of three bytes: 247 bytes with the extension, which a file system
        // takes, but not with the `.part` ending a download needs while it arrives.
        let name = format!("{}.jpg", "写".repeat(81));
        for _ in 0..3 {
            let saved = free_file_name(&dir, &name);
            let file_name = saved.file_name().unwrap().to_str().unwrap().to_string();
            assert!(file_name.ends_with(".jpg"), "{file_name}");
            assert!(file_name.starts_with("写写写"));
            let mut part = saved.clone().into_os_string();
            part.push(".0a1b2c3d.part");
            std::fs::write(&part, b"x").unwrap();
            std::fs::rename(&part, &saved).unwrap();
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_shared_name_fits_what_a_room_takes() {
        assert_eq!(fair_file_name("notes\u{200F}.txt", 255), "notes.txt");
        assert_eq!(fair_file_name("a\tb.pdf", 255), "ab.pdf");
        assert_eq!(fair_file_name("\u{202E}", 255), "file");
        // Windows and macOS take 255 characters, which can be far more bytes.
        let long = format!("{}.mp4", "日本".repeat(60));
        let fair = fair_file_name(&long, 255);
        assert!(fair.len() <= 255 && fair.ends_with(".mp4"), "{fair}");
        let no_extension = "é".repeat(200);
        assert_eq!(fair_file_name(&no_extension, 255).len(), 254);
    }

    #[test]
    fn a_file_name_keeps_what_shows() {
        for blank in [
            "\u{3164}",
            "\u{2800}\u{115F}",
            "\u{0301}\u{FE0F}",
            "\u{3000}.",
        ] {
            assert_eq!(fair_file_name(blank, 255), "file", "{blank:?}");
        }
        assert_eq!(
            fair_file_name("photo\u{3164}\u{3164}\u{3164}\u{3164}.exe", 255),
            "photo .exe"
        );
        assert_eq!(
            fair_file_name("\u{2800}notes.txt\u{3164}", 255),
            "notes.txt"
        );
        assert_eq!(fair_file_name("한글 문서.hwp", 255), "한글 문서.hwp");
        // A folder that is not there: every name in it is free.
        let dir = std::env::temp_dir().join(format!("fogata-blank-{}", std::process::id()));
        assert_eq!(free_file_name(&dir, "\u{3164}\u{3164}"), dir.join("file"));
    }
}
