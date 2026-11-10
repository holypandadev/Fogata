//! Files this device shares in a group's room. Each is served from a copy made when it is
//! shared (see [`files::Copies`]), hashed as it is copied, so what members get by its id is
//! what the person shared; and it is served only once its room took it, unless this device
//! left the room, or stopped sharing it, meanwhile.

use std::path::{Path as FsPath, PathBuf};

use anyhow::{Context, Result, bail};
use fogata_crypto::GroupId;
use fogata_net::{FileInfo, files};

use crate::{
    Core, Inner, downloads::MAX_SHARED_NAME, downloads::SPACE_KEPT_FREE, downloads::fair_file_name,
};

/// The folder of the data folder the copies are kept in.
pub(crate) const COPIES_DIR: &str = "sharing";

impl Core {
    /// Shares a file in a group's room while this device is in it: a copy of it as it is
    /// now, which goes when it stops being shared.
    pub async fn share_file(
        &self,
        group: GroupId,
        path: PathBuf,
        video: Option<fogata_state::VideoPreview>,
    ) -> Result<FileInfo> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .context("that is not a file")?;
        // Shown to the others as a room takes it: a name it would refuse (too long, as
        // Windows and macOS allow, or with marks that hide in text) would leave the file
        // shared with nobody, the sharer not told.
        let name = fair_file_name(&name, MAX_SHARED_NAME);
        let cannot_read = || format!("cannot read {}", path.display());
        let size = tokio::fs::metadata(&path)
            .await
            .with_context(cannot_read)?
            .len();
        let room = room_for_copy(&self.inner, size)?;
        let copied = {
            let (inner, from) = (self.inner.clone(), path.clone());
            tokio::task::spawn_blocking(move || {
                let copied = inner.copies.copy(&from, room.most());
                drop(room);
                copied
            })
            .await?
        };
        let (copy, id, size) = match copied {
            Ok(copied) => copied,
            Err(e) if e.kind() == std::io::ErrorKind::StorageFull => bail!("{e}"),
            // Room was held for it as it was when shared.
            Err(e) if e.kind() == std::io::ErrorKind::FileTooLarge => bail!(
                "{} grew while it was being shared: share it once it is finished",
                path.display()
            ),
            Err(e) => return Err(e).with_context(cannot_read),
        };
        // A room lists so many files of each person, and would leave this one out unsaid.
        let me = self.me();
        let shared_here = self
            .inner
            .node
            .rooms(group)
            .await?
            .into_iter()
            .filter(|(_, room)| room.joined)
            .flat_map(|(_, room)| room.files)
            .filter(|file| file.holders.contains(&me) && file.info.id != id)
            .count();
        if shared_here >= fogata_state::MAX_SHARES_EACH {
            bail!(
                "you share {} files in this room already: stop sharing some first",
                fogata_state::MAX_SHARES_EACH
            );
        }
        let info = FileInfo {
            id,
            name,
            size,
            video,
        };
        announce(&self.inner, group, info.clone(), copy, path).await?;
        Ok(info)
    }

    /// Stops sharing a file in a group's room.
    pub async fn stop_sharing(&self, group: GroupId, file: [u8; 32]) -> Result<()> {
        self.inner.shared.remove(group, file);
        let unshared = self.inner.node.unshare_file(group, file).await;
        // A share of it that its room took meanwhile is not served either.
        self.inner.shared.remove(group, file);
        Ok(unshared?)
    }
}

/// Tells `group`'s room this device shares `info`, read from `copy`, and serves it once the
/// room took it: unless the room was left, or the file stopped being shared, meanwhile, as
/// a step of those that comes after this one's stops serving it (see
/// [`files::SharedFiles::ticket`]). Then the room hears it is not shared after all, unless
/// another share of the same file went.
async fn announce(
    inner: &Inner,
    group: GroupId,
    info: FileInfo,
    copy: std::sync::Arc<files::Copy>,
    shown: PathBuf,
) -> Result<()> {
    let id = info.id;
    let ticket = inner.shared.ticket(group, id);
    inner.node.share_file(group, info).await?;
    if inner.shared.insert_copy(ticket, copy, shown) {
        return Ok(());
    }
    if !inner.shared.contains(group, id) {
        let _ = inner.node.unshare_file(group, id).await;
    }
    bail!("the file was not shared: you left the room, or stopped sharing it, meanwhile")
}

/// Shares from here a room file downloaded to `target` too, so that it stays when the first
/// sharer leaves: a copy of it, made now, if it is still the file and the disk has room.
pub(crate) async fn share_download(
    inner: &std::sync::Arc<Inner>,
    group: GroupId,
    info: &FileInfo,
    target: &FsPath,
) {
    let shared = async {
        let room = room_for_copy(inner, info.size)?;
        let (copy, id, _) = {
            let (inner, from) = (inner.clone(), target.to_path_buf());
            tokio::task::spawn_blocking(move || {
                let copied = inner.copies.copy(&from, room.most());
                drop(room);
                copied
            })
            .await??
        };
        if id != info.id {
            bail!("it changed since it came");
        }
        announce(inner, group, info.clone(), copy, target.to_path_buf()).await
    };
    if let Err(e) = shared.await {
        tracing::debug!("a download is not shared from here: {e:#}");
    }
}

/// Room on the disk held for a copy being made or a room file being fetched: what those
/// under way may take is counted together, so two of them never each take nearly all that
/// is free and leave nothing for the group logs, journals and lists written meanwhile.
/// Given back when dropped, once the file is made (it then takes its room itself) or
/// failed; moved into the work that writes it, so a caller that stops waiting does not give
/// it back while the writing goes on.
pub(crate) struct Room {
    inner: std::sync::Arc<Inner>,
    disk: Disk,
    held: u64,
}

/// Which disk a folder is on, for the room held on each: a download to one disk is not
/// refused for what copies to another hold. Unix: the device of the folder (or of the
/// nearest folder above it that is there yet); Windows: its drive (or share); elsewhere,
/// one for all.
pub(crate) type Disk = String;

pub(crate) fn disk_of(dir: &FsPath) -> Disk {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        dir.ancestors()
            .find_map(|d| std::fs::metadata(d).ok())
            .map(|m| m.dev().to_string())
            .unwrap_or_default()
    }
    #[cfg(windows)]
    {
        let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
        match dir.components().next() {
            Some(std::path::Component::Prefix(prefix)) => {
                prefix.as_os_str().to_string_lossy().to_lowercase()
            }
            _ => String::new(),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = dir;
        String::new()
    }
}

impl Room {
    /// What the file may take: what is held for it.
    pub(crate) fn most(&self) -> u64 {
        self.held
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        let mut copying = self.inner.copying.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(held) = copying.get_mut(&self.disk) {
            *held = held.saturating_sub(self.held);
            if *held == 0 {
                copying.remove(&self.disk);
            }
        }
    }
}

/// The room held for a copy of a file of `size` bytes: a sealed copy takes a little more
/// than the file, a tag for each part.
fn held_for(size: u64) -> u64 {
    size.saturating_add(size / 1024).saturating_add(64 * 1024)
}

/// Holds `bytes` of the disk of `dir`, leaving [`SPACE_KEPT_FREE`] and what the copies and
/// fetches under way on that disk hold (`refused` says why when they do not fit). Read and
/// held at once.
pub(crate) fn hold_disk(
    inner: &std::sync::Arc<Inner>,
    bytes: u64,
    dir: &FsPath,
    refused: &str,
) -> Result<Room> {
    let free_space = inner.lock().free_space;
    let disk = disk_of(dir);
    let mut copying = inner.copying.lock().unwrap_or_else(|e| e.into_inner());
    let held = copying.get(&disk).copied().unwrap_or(0);
    match free_space.map(|free_space| free_space(dir)) {
        Some(Some(free)) => {
            let most = free.saturating_sub(SPACE_KEPT_FREE).saturating_sub(held);
            if bytes > most {
                bail!("{refused}");
            }
        }
        // Not checked: what is held is still counted, for when the system answers again.
        Some(None) => tracing::debug!(
            "the free space of {} is not known; {bytes} bytes held unchecked",
            dir.display()
        ),
        None => tracing::debug!("no free space check: {bytes} bytes held unchecked"),
    }
    *copying.entry(disk.clone()).or_default() = held.saturating_add(bytes);
    Ok(Room {
        inner: inner.clone(),
        disk,
        held: bytes,
    })
}

/// Holds room for a copy of a file of `size` bytes, sealed or not (see [`hold_disk`]).
fn room_for_copy(inner: &std::sync::Arc<Inner>, size: u64) -> Result<Room> {
    hold_disk(
        inner,
        held_for(size),
        inner.copies.dir(),
        "not enough free space to share this file",
    )
}

/// Stops serving the files this device shares in `group`'s rooms around `leaving`, a step
/// that takes it out of them: before, so they stop at once, and after, so that a share its
/// room took meanwhile is not served either.
pub(crate) async fn leaving<T>(
    inner: &Inner,
    group: GroupId,
    leaving: impl Future<Output = T>,
) -> T {
    inner.shared.clear_group(group);
    let left = leaving.await;
    inner.shared.clear_group(group);
    left
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use fogata_crypto::GroupId;
    use fogata_net::{Multiaddr, NodeHandle, PeerId, files::FILE_PROTOCOL};
    use futures::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::time::timeout;

    use super::held_for;
    use crate::{Core, CoreEvent, NetOptions};

    /// One test at a time: each starts devices that talk over this computer's network.
    static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn options(port: u16, peers: Vec<Multiaddr>) -> NetOptions {
        NetOptions {
            listen_port: port,
            use_dht: false,
            use_mdns: false,
            peers,
            ..Default::default()
        }
    }

    async fn wait_for(
        events: &mut crate::CoreEvents,
        what: &str,
        mut done: impl FnMut(&CoreEvent) -> bool,
    ) {
        let found = timeout(Duration::from_secs(60), async {
            while let Some(event) = events.recv().await {
                if done(&event) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        assert!(found, "timed out waiting for {what}");
    }

    /// Ana and Bea, two cores in `base`, in the room of a group of Ana's that Bea joined;
    /// with the peer Bea's node reaches Ana's at.
    struct Room {
        ana: Core,
        bea: Core,
        group: GroupId,
        ana_peer: PeerId,
        _events: (crate::CoreEvents, crate::CoreEvents),
    }

    async fn in_a_room(base: &Path) -> Room {
        let _ = std::fs::remove_dir_all(base);
        let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
        Core::create_profile(&dir_a, "Ana", None).unwrap();
        Core::create_profile(&dir_b, "Bea", None).unwrap();
        let port_a = std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let (ana, mut events_a) = Core::start(dir_a, options(port_a, Vec::new())).unwrap();
        let group = ana.create_group("crew", 0, &[]).await.unwrap();
        let invite = ana.invite(group, 1).await.unwrap();
        let to_ana: Multiaddr = format!("/ip4/127.0.0.1/udp/{port_a}/quic-v1")
            .parse()
            .unwrap();
        let (bea, mut events_b) = Core::start(dir_b, options(0, vec![to_ana])).unwrap();
        bea.join_group(&invite, None, None).await.unwrap();
        let who = bea.me();
        wait_for(
            &mut events_a,
            "the ask to join",
            |e| matches!(e, CoreEvent::JoinAsked { device, .. } if *device == who),
        )
        .await;
        ana.answer_join(group, who, true).await.unwrap();
        wait_for(
            &mut events_b,
            "the join",
            |e| matches!(e, CoreEvent::Joined { group: g, .. } if *g == group),
        )
        .await;
        ana.join_room(group).await.unwrap();
        bea.join_room(group).await.unwrap();
        // Each side checks the other's proof of membership on its own: Bea may have Ana's
        // while Ana is still checking Bea's, and until then Ana serves Bea nothing (the
        // stream is dropped). Both must have the other's.
        let ana_peer = timeout(Duration::from_secs(60), async {
            loop {
                let ana_has_bea = matches!(ana.inner.node.member_peer(bea.me()).await, Ok(Some(_)));
                if let Ok(Some(peer)) = bea.inner.node.member_peer(ana.me()).await
                    && ana_has_bea
                {
                    return peer;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("Ana and Bea never reached each other");
        Room {
            ana,
            bea,
            group,
            ana_peer,
            _events: (events_a, events_b),
        }
    }

    /// What a member's own program gets asking `peer` for file `id` of `group` from
    /// `offset`, read as it comes and never checked: `None` when the file is not there,
    /// else the bytes that came. `between` runs once the first `after` bytes came.
    async fn ask_raw(
        node: &NodeHandle,
        group: GroupId,
        peer: PeerId,
        id: [u8; 32],
        offset: u64,
        after: usize,
        between: impl FnOnce(),
    ) -> Option<Vec<u8>> {
        let mut stream = node
            .streams()
            .open_stream(peer, FILE_PROTOCOL)
            .await
            .unwrap();
        stream.write_all(group.as_bytes()).await.unwrap();
        stream.write_all(&id).await.unwrap();
        stream.write_all(&offset.to_be_bytes()).await.unwrap();
        stream.flush().await.unwrap();
        let mut status = [0u8; 1];
        stream.read_exact(&mut status).await.ok()?;
        if status[0] != 0 {
            return None;
        }
        let mut size = [0u8; 8];
        stream.read_exact(&mut size).await.unwrap();
        let mut got = Vec::new();
        let mut buf = vec![0u8; 4096];
        let mut between = Some(between);
        let _ = timeout(Duration::from_secs(30), async {
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
                if got.len() >= after
                    && let Some(between) = between.take()
                {
                    between();
                }
            }
        })
        .await;
        Some(got)
    }

    impl Room {
        async fn ask(&self, id: [u8; 32], offset: u64) -> Option<Vec<u8>> {
            let node = &self.bea.inner.node;
            ask_raw(node, self.group, self.ana_peer, id, offset, 0, || {}).await
        }
    }

    /// Ana alone, started in `base`, with a group of hers.
    async fn alone(base: &Path) -> (Core, crate::CoreEvents, GroupId, PathBuf) {
        let _ = std::fs::remove_dir_all(base);
        let dir = base.join("ana");
        Core::create_profile(&dir, "Ana", None).unwrap();
        let (ana, events) = Core::start(dir.clone(), options(0, Vec::new())).unwrap();
        let group = ana.create_group("crew", 0, &[]).await.unwrap();
        (ana, events, group, dir)
    }

    /// A large file being copied to share does not keep the data folder held once the core
    /// stops: the copy stops with it, and the folder is free in a moment.
    #[tokio::test(flavor = "multi_thread")]
    async fn stopping_while_a_file_is_copied_lets_go_of_the_folder() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-share-stop-{}", std::process::id()));
        let (ana, events, group, dir) = alone(&base).await;
        let film = base.join("film.mkv");
        // Takes no room on the disk: it is read as zeros.
        std::fs::File::create(&film)
            .unwrap()
            .set_len(512 << 20)
            .unwrap();
        let ana = std::sync::Arc::new(ana);
        let sharing = {
            let ana = ana.clone();
            tokio::spawn(async move { ana.share_file(group, film, None).await })
        };
        tokio::time::sleep(Duration::from_millis(300)).await;
        ana.shutdown().await.unwrap();
        drop((ana, events));
        let shared = timeout(Duration::from_secs(5), sharing).await;
        let free = timeout(Duration::from_secs(5), async {
            loop {
                let store = crate::store::Store::open(dir.clone()).unwrap();
                if store.hold_running().is_ok() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(free.is_ok(), "the data folder was still held");
        assert!(shared.expect("the copy went on").unwrap().is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A file that grows while it is shared is refused as such, not for want of room.
    /// Whether it grows while its copy is made is up to the threads' turns: on a busy
    /// computer the writer may get none until the copy is done, and the share then goes
    /// on to say the room is not joined. Such a try says nothing either way and is made
    /// again; any other answer fails.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_growing_while_shared_is_refused_as_such() {
        use std::io::Write as _;
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-share-grow-{}", std::process::id()));
        let (ana, _events, group, _) = alone(&base).await;
        let recording = base.join("recording.mkv");
        let mut said = String::new();
        for _ in 0..20 {
            std::fs::write(&recording, vec![7u8; 4 << 20]).unwrap();
            let writing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let writer = {
                let (writing, recording) = (writing.clone(), recording.clone());
                std::thread::spawn(move || {
                    let mut file = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&recording)
                        .unwrap();
                    let chunk = vec![9u8; 64 * 1024];
                    // At most 64 MiB more.
                    for _ in 0..1024 {
                        if !writing.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        file.write_all(&chunk).unwrap();
                    }
                })
            };
            // Shared once it is growing.
            while std::fs::metadata(&recording).unwrap().len() == 4 << 20 && !writer.is_finished() {
                std::thread::yield_now();
            }
            let refused = ana
                .share_file(group, recording.clone(), None)
                .await
                .unwrap_err();
            writing.store(false, std::sync::atomic::Ordering::Relaxed);
            writer.join().unwrap();
            said = format!("{refused:#}");
            if !said.contains("join the room first") {
                break;
            }
        }
        assert!(said.contains("grew while it was being shared"), "{said}");
        ana.shutdown().await.unwrap();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Ana in a room with Bea, sharing a file of `bytes` from `base` under `name`: the
    /// room, the file's id and where it is.
    async fn sharing(base: &Path, name: &str, bytes: &[u8]) -> (Room, [u8; 32], PathBuf) {
        let room = in_a_room(base).await;
        let path = base.join(name);
        std::fs::write(&path, bytes).unwrap();
        let info = room
            .ana
            .share_file(room.group, path.clone(), None)
            .await
            .unwrap();
        assert_eq!(room.ask(info.id, 0).await.as_deref(), Some(bytes));
        (room, info.id, path)
    }

    fn write_over(path: &Path, bytes: &[u8]) {
        use std::io::Write as _;
        let mut same = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        same.write_all(bytes).unwrap();
    }

    /// A shared file written over in place (the same file, other bytes of the same length)
    /// is served as it was shared, also from the middle.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_shared_file_written_over_is_served_as_it_was() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-shared-over-{}", std::process::id()));
        let (room, id, path) = sharing(&base, "notes.txt", b"PUBLIC-V1").await;
        write_over(&path, b"SECRET-V2");
        assert_eq!(room.ask(id, 0).await.as_deref(), Some(&b"PUBLIC-V1"[..]));
        assert_eq!(room.ask(id, 3).await.as_deref(), Some(&b"LIC-V1"[..]));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// One replaced by another file is served as it was shared.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_shared_file_replaced_is_served_as_it_was() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-shared-repl-{}", std::process::id()));
        let (room, id, path) = sharing(&base, "plan.txt", b"PLAN-ONE!").await;
        let other = base.join("other.txt");
        std::fs::write(&other, b"REPLACED!").unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert_eq!(room.ask(id, 0).await.as_deref(), Some(&b"PLAN-ONE!"[..]));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// One made a link to another file is served as it was shared: the link is not
    /// followed.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_shared_file_made_a_link_is_served_as_it_was() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-shared-link-{}", std::process::id()));
        let (room, id, path) = sharing(&base, "song.txt", b"SONG-FREE").await;
        let secret = base.join("secret.txt");
        std::fs::write(&secret, b"SYMLINK!!").unwrap();
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&secret, &path).unwrap();
        assert_eq!(room.ask(id, 0).await.as_deref(), Some(&b"SONG-FREE"[..]));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// One changed while it is on its way goes whole as it was shared.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_shared_file_changed_on_its_way_goes_as_it_was() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-shared-way-{}", std::process::id()));
        let bytes: Vec<u8> = (0..16u32 << 20).map(|i| (i % 251) as u8).collect();
        let (room, id, path) = sharing(&base, "film.mkv", &bytes).await;
        let node = &room.bea.inner.node;
        let got = ask_raw(node, room.group, room.ana_peer, id, 0, 1 << 20, move || {
            write_over(&path, &vec![b'X'; 16 << 20]);
        })
        .await
        .unwrap();
        assert_eq!(got.len(), bytes.len());
        assert!(got == bytes, "other bytes went on its way");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A share the room refuses (this device left it) leaves the file unserved, before,
    /// while and after it is tried, also to a member that knew its id; one shared again in
    /// the room is served again. Two shares of the same file at once both go.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_share_the_room_refuses_is_never_served() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-refused-{}", std::process::id()));
        let room = in_a_room(&base).await;
        let (ana, group) = (&room.ana, room.group);
        let path = base.join("notes.txt");
        std::fs::write(&path, b"PUBLIC-V1").unwrap();
        let info = ana.share_file(group, path.clone(), None).await.unwrap();
        assert!(room.ask(info.id, 0).await.is_some());
        ana.leave_room(group).await.unwrap();
        assert_eq!(room.ask(info.id, 0).await, None, "served out of the room");

        // Asked for while the share is tried, and after.
        let asking = async {
            let mut served = 0;
            for _ in 0..20 {
                served += usize::from(room.ask(info.id, 0).await.is_some());
            }
            served
        };
        let (refused, served) = tokio::join!(ana.share_file(group, path.clone(), None), asking);
        assert!(refused.is_err(), "shared out of the room");
        assert_eq!(served, 0, "served while a refused share was tried");
        assert_eq!(
            room.ask(info.id, 0).await,
            None,
            "served after a refused share"
        );

        // Back in the room it is not served until shared again.
        ana.join_room(group).await.unwrap();
        assert_eq!(
            room.ask(info.id, 0).await,
            None,
            "served before it was shared again"
        );
        let (one, two) = tokio::join!(
            ana.share_file(group, path.clone(), None),
            ana.share_file(group, path.clone(), None)
        );
        assert_eq!(one.unwrap().id, info.id);
        assert_eq!(two.unwrap().id, info.id);
        assert!(room.ask(info.id, 0).await.is_some());
        ana.stop_sharing(group, info.id).await.unwrap();
        assert_eq!(
            room.ask(info.id, 0).await,
            None,
            "served once no longer shared"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The copies kept for sharing, by name.
    fn copies(core: &Core) -> Vec<String> {
        std::fs::read_dir(core.inner.copies.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    async fn no_copies_soon(core: &Core) {
        let gone = timeout(Duration::from_secs(10), async {
            while !copies(core).is_empty() {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(gone.is_ok(), "copies left: {:?}", copies(core));
    }

    /// A file's copy is kept while it is shared, once however often it is shared, sealed
    /// when the data folder is; it goes once the file stops being shared, or its room is
    /// left, and what a run left goes when the next starts. Entering another voice room of
    /// the group stops what was shared in the last.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_copy_lasts_as_long_as_its_share() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-copy-life-{}", std::process::id()));
        let (room, id, path) = sharing(&base, "notes.txt", b"PUBLIC-V1").await;
        let (ana, group) = (&room.ana, room.group);
        ana.share_file(group, path.clone(), None).await.unwrap();
        assert_eq!(copies(ana).len(), 1);
        let copy = ana.inner.copies.dir().join(&copies(ana)[0]);
        let on_disk = std::fs::read(&copy).unwrap();
        if ana.inner.store.data_key().unwrap().is_some() {
            assert!(!on_disk.windows(9).any(|w| w == b"PUBLIC-V1"), "kept plain");
        }
        ana.stop_sharing(group, id).await.unwrap();
        no_copies_soon(ana).await;

        ana.share_file(group, path.clone(), None).await.unwrap();
        ana.leave_room(group).await.unwrap();
        no_copies_soon(ana).await;

        ana.join_room(group).await.unwrap();
        // In once the room is open here again.
        let shared = timeout(Duration::from_secs(30), async {
            while ana.share_file(group, path.clone(), None).await.is_err() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(shared.is_ok(), "never in the room again");
        assert!(room.ask(id, 0).await.is_some());
        let other = ana.set_voice_room(group, None, "Gaming").await.unwrap();
        ana.join_voice_room(group, other, None).await.unwrap();
        assert_eq!(
            room.ask(id, 0).await,
            None,
            "served from the last voice room"
        );
        no_copies_soon(ana).await;

        // Left by a run that stopped: gone when the next starts.
        let stray = ana.inner.copies.dir().join("left.copy");
        std::fs::write(&stray, b"old").unwrap();
        let dir = ana.inner.store.dir().to_path_buf();
        let Room { ana, bea, .. } = room;
        ana.shutdown().await.unwrap();
        drop((ana, bea));
        // Its lock goes with the last of its tasks.
        let mut started = None;
        for _ in 0..50 {
            match Core::start(dir.clone(), options(0, Vec::new())) {
                Ok(core) => {
                    started = Some(core);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        let (ana, _events) = started.expect("the folder stayed locked");
        assert!(copies(&ana).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Leaving the room and going straight back in: in it again, and sharing there. The
    /// leave and the join can be sealed in the same millisecond, and a room takes no join
    /// that is not later than the device's last leave.
    #[tokio::test(flavor = "multi_thread")]
    async fn going_straight_back_into_the_room_lets_this_device_share_there() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-back-in-{}", std::process::id()));
        let room = in_a_room(&base).await;
        let (ana, group) = (&room.ana, room.group);
        let path = base.join("notes.txt");
        std::fs::write(&path, b"PUBLIC-V1").unwrap();
        for _ in 0..5 {
            ana.leave_room(group).await.unwrap();
            ana.join_room(group).await.unwrap();
            let shared = timeout(Duration::from_secs(5), async {
                while ana.share_file(group, path.clone(), None).await.is_err() {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await;
            assert!(shared.is_ok(), "out of the room it went back into");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A file is shared only with room on the disk for its copy, leaving some to spare;
    /// one refused leaves nothing behind.
    #[tokio::test(flavor = "multi_thread")]
    async fn sharing_needs_room_for_the_copy() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-copy-room-{}", std::process::id()));
        let room = in_a_room(&base).await;
        let (ana, group) = (&room.ana, room.group);
        ana.use_free_space(|_| Some(crate::downloads::SPACE_KEPT_FREE + held_for(9) - 1));
        let path = base.join("notes.txt");
        std::fs::write(&path, b"PUBLIC-V1").unwrap();
        let refused = ana.share_file(group, path.clone(), None).await.unwrap_err();
        assert!(
            refused.to_string().contains("not enough free space"),
            "{refused:#}"
        );
        assert!(copies(ana).is_empty());
        ana.use_free_space(|_| Some(crate::downloads::SPACE_KEPT_FREE + held_for(9)));
        let info = ana.share_file(group, path, None).await.unwrap();
        assert!(room.ask(info.id, 0).await.is_some());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// A room file being fetched holds its room too: while it comes, a file that would fit
    /// alone is not copied to share, and once it came the room is there again.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_fetch_under_way_holds_its_room_against_copies() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-fetch-room-{}", std::process::id()));
        let room = in_a_room(&base).await;
        let (ana, group) = (&room.ana, room.group);
        const SIZE: u64 = 4 << 20;
        ana.use_free_space(|_| Some(crate::downloads::SPACE_KEPT_FREE + held_for(SIZE) * 3 / 2));
        let path = base.join("notes.bin");
        std::fs::write(&path, vec![1; SIZE as usize]).unwrap();
        let fetching = super::hold_disk(&ana.inner, SIZE, &base, "full").unwrap();
        let refused = ana.share_file(group, path.clone(), None).await.unwrap_err();
        assert!(
            refused.to_string().contains("not enough free space"),
            "{refused:#}"
        );
        drop(fetching);
        ana.share_file(group, path, None).await.unwrap();
        assert!(ana.inner.copying.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }

    /// What is held on one disk does not count against another: a download to a second
    /// disk is not refused for a copy being made on the first.
    #[tokio::test(flavor = "multi_thread")]
    async fn room_held_on_one_disk_leaves_another_alone() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-two-disks-{}", std::process::id()));
        let room = in_a_room(&base).await;
        let ana = &room.ana;
        const SIZE: u64 = 4 << 20;
        ana.use_free_space(|_| Some(crate::downloads::SPACE_KEPT_FREE + SIZE * 3 / 2));
        // Nearly all of another disk's room is held.
        ana.inner
            .copying
            .lock()
            .unwrap()
            .insert("another disk".into(), SIZE);
        let here = super::hold_disk(&ana.inner, SIZE, &base, "full").unwrap();
        // This disk's own holds still count.
        assert!(super::hold_disk(&ana.inner, SIZE, &base, "full").is_err());
        drop(here);
        let copying = ana.inner.copying.lock().unwrap().clone();
        assert_eq!(copying.len(), 1, "{copying:?}");
        assert_eq!(copying["another disk"], SIZE);
        assert_eq!(
            super::disk_of(&base),
            super::disk_of(&base.join("not there yet"))
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Two large files shared at once on a disk with room for one: the room each copy
    /// takes is held together, so the second is refused rather than both filling the disk.
    #[tokio::test(flavor = "multi_thread")]
    async fn copies_made_at_once_share_the_room_on_the_disk() {
        let _one = ONE_AT_A_TIME.lock().await;
        let base = std::env::temp_dir().join(format!("fogata-copy-both-{}", std::process::id()));
        let room = in_a_room(&base).await;
        let (ana, group) = (&room.ana, room.group);
        const SIZE: u64 = 8 << 20;
        // Room for one copy and a half, past what is kept free.
        ana.use_free_space(|_| Some(crate::downloads::SPACE_KEPT_FREE + held_for(SIZE) * 3 / 2));
        let paths: Vec<PathBuf> = (0..2u8)
            .map(|n| {
                let path = base.join(format!("film{n}.mkv"));
                std::fs::write(&path, vec![n; SIZE as usize]).unwrap();
                path
            })
            .collect();
        let (first, second) = tokio::join!(
            ana.share_file(group, paths[0].clone(), None),
            ana.share_file(group, paths[1].clone(), None),
        );
        let refused = [&first, &second]
            .iter()
            .filter(|r| {
                r.as_ref()
                    .is_err_and(|e| e.to_string().contains("not enough free space"))
            })
            .count();
        assert_eq!(
            (first.is_ok() as u8 + second.is_ok() as u8, refused),
            (1, 1),
            "one shared, one refused"
        );
        // Once made, the copy takes its room on the disk itself: nothing stays held.
        assert!(ana.inner.copying.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
