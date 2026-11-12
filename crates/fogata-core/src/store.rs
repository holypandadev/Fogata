//! Files kept on this device: the device identity, the profile and friends, the list of its
//! groups and each group's log, and a few times for the screens (when a friend was last
//! seen, when a group last had a room, where friends and members were last reached). Nothing
//! said or shared in a room; direct chats are kept apart, in `direct/` (see
//! [`crate::direct::store`]). The identity is secret, the friends file holds friend secrets
//! and the logs hold epoch keys sealed to it. On Unix the files are created readable by their
//! owner only.
//!
//! When the system keychain keeps a data key for the folder ([`crate::vault`]), the identity
//! and the friends file are sealed with it, as are direct chats' journals, pins, profiles and
//! attachments; the logs need not be, as what is secret in them is sealed to the identity.
//! What was judged of each log is kept beside it, sealed (see [`read_judged`]).
//! A folder is sealed the first time an application that may seal opens it: the plain files
//! are copied to `before-sealing/` first, each is sealed in its place and read back. The
//! copies of the direct chats go once all of it is sealed; the others (and all of them when
//! sealing was cut short) at a later start that opens the folder whole, so a keychain that
//! loses the key after giving it back once does not take the account with it. The identity
//! and the friends file go first, so that copies of Fogata from before sealing refuse the
//! folder from then on (theirs reads the friends file as written by a later version) rather
//! than change it.
//!
//! Every file is replaced whole through a temporary file, and a change that reads a file and
//! writes it back holds the folder's lock, so two changes at once never undo one another. A
//! file found damaged is put aside as `.damaged`, never deleted, and what it held is taken
//! from what can still be read: the start of a log, the logs for the list of groups, the copy
//! of the friends file made before its last change.
//!
//! Sealing the folder and the files set aside are in sealing.rs, the groups and their logs
//! in groups.rs, and what was judged of each log in judged.rs.

mod groups;
mod judged;
mod sealing;

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{
    DeviceIdentity, DevicePublicKey, GroupId,
    at_rest::{DataKey, Form, sealed_form},
};
use fogata_friends::{Animation, FriendBook, Profile};
use fogata_state::{Entry, GroupLog};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub use self::groups::{DeclineLeave, DeclinedGroup, Offered, StoredGroup};
#[cfg(test)]
pub(crate) use self::judged::tests as judged_tests;
pub(crate) use self::judged::{JUDGED_SUFFIX, read_judged, write_judged};
use self::sealing::SealPhase;
pub use self::sealing::{BACKUP_DIR, REFUSED_DIR, SEALING_FILE, SET_ASIDE_FILE, Sealing};
use crate::vault::{self, Keychain};

/// Where this device keeps its MLS state, one file per group.
const MLS_DIR: &str = "mls";

/// The file in a data folder that says which layout of it the build that last opened it
/// writes: its files and folders and what they hold ([`LAYOUT`]).
pub(crate) const LAYOUT_FILE: &str = "layout";

/// The layout of the data folder this build reads and writes. It goes up only when a build
/// older than the one that raises it would misread what the newer one writes (a file moved
/// or written in another form), not for new files an older build leaves alone (those are
/// read with care: `levels.json`, `.bak` and `.damaged` copies). A folder of a layout above
/// this build's is refused rather than misread: an older build installed over a newer one
/// by hand. A folder with no marker is of layout 1, from before there was one.
pub(crate) const LAYOUT: u32 = 1;

/// The layout the data folder `dir` says it has: 1 with no marker (from before there was
/// one), `None` when the marker does not read (cut short).
fn layout_of(dir: &Path) -> Result<Option<u32>> {
    let path = dir.join(LAYOUT_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text.trim().parse::<u32>().ok()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Some(1)),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// Refuses the data folder `dir` when a newer build wrote it ([`LAYOUT`]). Nothing is
/// written: it is marked as this build's once a core runs in it ([`Store::mark_layout`]).
fn check_layout(dir: &Path) -> Result<()> {
    if let Some(found) = layout_of(dir)?
        && found > LAYOUT
    {
        bail!(
            "the data in {} was written by a newer version of Fogata (data layout {found}; \
             this version reads layout {LAYOUT}): install the newer version again",
            dir.display()
        );
    }
    Ok(())
}

/// A secret of the folder that is sealed and does not open with the folder's key (a bit
/// flipped on the disk, say): sealing cannot be checked to its end while it is there.
#[derive(Debug)]
pub(crate) struct DoesNotOpen(pub(crate) PathBuf);

impl std::fmt::Display for DoesNotOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} does not open with its key", self.0.display())
    }
}

impl std::error::Error for DoesNotOpen {}

/// Default data directory for the current platform. On Windows it is under
/// `%LOCALAPPDATA%`, which stays on this computer: `%APPDATA%` roams with the profile on
/// computers in a domain, and the folder holds downloads in progress and every attachment.
/// An error when the system names no such folder: the data never lands in whatever folder
/// Fogata was started from.
pub fn default_data_dir() -> Result<PathBuf> {
    data_dir_from(&|name| std::env::var_os(name))
}

/// [`default_data_dir`] with the environment variables `var` gives. Only absolute folders
/// count: an empty or relative value is as good as none.
fn data_dir_from(var: &dyn Fn(&str) -> Option<std::ffi::OsString>) -> Result<PathBuf> {
    let dir = |name: &str| var(name).map(PathBuf::from).filter(|dir| dir.is_absolute());
    let found = if cfg!(target_os = "windows") {
        let under = |name| dir(name).map(|dir| dir.join("Fogata"));
        local_data_dir(under("LOCALAPPDATA"), under("APPDATA"))
    } else if cfg!(target_os = "macos") {
        dir("HOME").map(|home| home.join("Library/Application Support/Fogata"))
    } else {
        dir("XDG_DATA_HOME")
            .map(|data| data.join("fogata"))
            .or_else(|| dir("HOME").map(|home| home.join(".local/share/fogata")))
    };
    found.with_context(|| {
        let names = if cfg!(target_os = "windows") {
            "LOCALAPPDATA and APPDATA name"
        } else if cfg!(target_os = "macos") {
            "HOME names"
        } else {
            "XDG_DATA_HOME and HOME name"
        };
        format!("no folder to keep Fogata's data in: the system gives none ({names} no full path)")
    })
}

/// The data folder at `local`, where a folder made at `roaming` by an earlier copy is
/// moved the first time. When it cannot be moved (a copy still runs from it, or the two
/// are on different drives) it stays in use where it is, and moving is tried again at the
/// next start.
fn local_data_dir(local: Option<PathBuf>, roaming: Option<PathBuf>) -> Option<PathBuf> {
    let Some(local) = local else {
        return roaming;
    };
    let Some(roaming) = roaming.filter(|r| !local.exists() && r.is_dir()) else {
        return Some(local);
    };
    if let Some(parent) = local.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        tracing::warn!("cannot create {}: {e}", parent.display());
    }
    match fs::rename(&roaming, &local) {
        Ok(()) => {
            tracing::info!(
                "data folder moved from {} to {}",
                roaming.display(),
                local.display()
            );
            Some(local)
        }
        Err(e) => {
            tracing::warn!(
                "data folder stays at {}, not moved to {}: {e}",
                roaming.display(),
                local.display()
            );
            Some(roaming)
        }
    }
}

#[derive(Debug)]
pub struct Store {
    dir: PathBuf,
    /// Held while a file is read, changed and written back. Every `Store` of the same folder
    /// in this process shares it.
    lock: Arc<Mutex<()>>,
    /// The friends file was damaged and its copy from before its last change took its place.
    friends_restored: std::sync::atomic::AtomicBool,
    keychain: Keychain,
    /// The folder's data key once looked for: `None` while its data is plain.
    key: OnceLock<Result<Option<DataKey>, String>>,
    /// This store holds the folder's `running.lock` ([`Self::hold_running`]).
    running: std::sync::atomic::AtomicBool,
    /// Plain files were copied to [`BACKUP_DIR`] by this store.
    backed_up: std::sync::atomic::AtomicBool,
    /// Where sealing the folder stands, once its key was looked for: as this store left it
    /// when it sealed, or as another copy of Fogata that runs on the folder noted it.
    phase: Mutex<Option<SealPhase>>,
    /// The folder was found sealed whole by an earlier start ([`SealPhase::Done`]).
    done_before: std::sync::atomic::AtomicBool,
    /// Why the keychain kept no key for this folder, which holds nothing yet and whose
    /// person did not choose to keep it unencrypted ([`vault::NoKeychain`]).
    refused: Mutex<Option<String>>,
}

/// A join with an invite that waits for a manager to let this device in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingJoin {
    /// The group's id in hex.
    pub group_id: String,
    /// The invite as it was given.
    pub invite: String,
    /// The name to keep the group under here.
    pub name: String,
    /// The name the members see.
    pub nick: String,
}

/// The folder's lock, held.
type Held<'a> = MutexGuard<'a, ()>;

/// Temporary files older than this were left by a write that never finished.
const STALE_TEMP: Duration = Duration::from_secs(600);

/// Times kept for the screens, in milliseconds since the Unix epoch, by device or group id in
/// hex.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Meta {
    #[serde(default)]
    pub last_seen: BTreeMap<String, u64>,
    #[serde(default)]
    pub last_room: BTreeMap<String, u64>,
    /// The UDP and TCP port this device listens on, picked at random once and kept: a
    /// firewall can then let it in with one rule, while no port marks a device as Fogata.
    #[serde(default)]
    pub port: Option<u16>,
    /// Where friends and members were reached (an address without a peer id): which device,
    /// and when last. Dialled again at start, before the DHT finds anyone.
    #[serde(default)]
    pub reached: BTreeMap<String, Reached>,
    /// Since when each group this device manages (by group id in hex) has waited to go up
    /// to the level of judgment this version gives, held back by members never seen at it:
    /// past [`crate::LEVEL_WAIT_MS`] a manager may raise it anyway.
    #[serde(default)]
    pub level_wait: BTreeMap<String, u64>,
}

/// A device reached at an address, and when last.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Reached {
    /// Device key in hex.
    pub device: String,
    pub at_ms: u64,
}

/// Addresses kept in [`Meta::reached`], at most, and for how long.
const MAX_REACHED: usize = 32;
const REACHED_FOR_MS: u64 = 30 * 24 * 3600 * 1000;

impl Meta {
    /// Notes that friend or member `device` (in hex) was reached at `address` at `now_ms`,
    /// forgetting the oldest addresses past [`MAX_REACHED`] and those not reached for a
    /// month.
    pub fn note_reached(&mut self, address: String, device: String, now_ms: u64) {
        self.reached.insert(
            address,
            Reached {
                device,
                at_ms: now_ms,
            },
        );
        self.reached
            .retain(|_, r| now_ms.saturating_sub(r.at_ms) < REACHED_FOR_MS);
        while self.reached.len() > MAX_REACHED {
            let oldest = self
                .reached
                .iter()
                .min_by_key(|(_, r)| r.at_ms)
                .map(|(a, _)| a.clone())
                .expect("not empty");
            self.reached.remove(&oldest);
        }
    }

    /// The addresses to dial at start with the device reached at each, newest first,
    /// without those not reached for a month.
    pub fn reached_addresses(&self, now_ms: u64) -> Vec<(String, String)> {
        let mut out: Vec<(&String, &Reached)> = self
            .reached
            .iter()
            .filter(|(_, r)| now_ms.saturating_sub(r.at_ms) < REACHED_FOR_MS)
            .collect();
        out.sort_by_key(|(_, r)| std::cmp::Reverse(r.at_ms));
        out.into_iter()
            .map(|(a, r)| (a.clone(), r.device.clone()))
            .collect()
    }
}

/// The most changes of a group's banner and picture kept in its history here.
pub const MAX_RECORDS_SEEN: usize = 200;

/// Which of a group's records apart from its log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordKind {
    Banner,
    Picture,
}

/// A change of a group's banner or picture as this device saw it arrive. Only the newest of
/// each is kept apart from the log (and the log has none of them), so who changed them before
/// is known only from what each device noted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordSeen {
    pub kind: RecordKind,
    /// The member that chose it, in hex.
    pub author: String,
    /// When, by the author's clock.
    pub time_ms: u64,
    /// It took the picture away.
    pub taken_away: bool,
    /// Its signature in hex: tells one record from another.
    pub signature: String,
}

impl RecordSeen {
    fn of<K: fogata_state::RecordKind>(
        kind: RecordKind,
        record: &fogata_state::GroupRecord<K>,
    ) -> Self {
        Self {
            kind,
            author: hex::encode(record.author.as_bytes()),
            time_ms: record.time_ms,
            taken_away: record.picture.is_empty(),
            signature: hex::encode(record.signature),
        }
    }
}

impl Store {
    /// Opens a data folder, using the keychain as this process was set to
    /// ([`vault::use_keychain`]). Nothing is asked of the keychain until a secret is read.
    pub fn open(dir: PathBuf) -> Result<Self> {
        Self::open_with(dir, vault::chosen())
    }

    /// Opens a data folder, using `keychain` for its data key.
    pub fn open_with(dir: PathBuf, keychain: Keychain) -> Result<Self> {
        check_layout(&dir)?;
        fs::create_dir_all(dir.join("logs"))
            .with_context(|| format!("cannot create {}", dir.display()))?;
        // Only this user goes in: another one could otherwise hold a lock on the file that
        // says the application runs here, and keep it from starting.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(e) = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)) {
                tracing::warn!("cannot keep {} to this user only: {e}", dir.display());
            }
        }
        let store = Self {
            lock: folder_lock(&dir),
            dir,
            friends_restored: Default::default(),
            keychain,
            key: OnceLock::new(),
            running: Default::default(),
            backed_up: Default::default(),
            phase: Mutex::new(None),
            done_before: Default::default(),
            refused: Mutex::new(None),
        };
        store.remove_stale_temps();
        Ok(store)
    }

    /// Marks the folder as of this build's layout ([`LAYOUT`]), unless it says so already:
    /// as a core starts in it, so opening a folder only to look (or to restore into it, a
    /// step that may be refused) changes nothing in it.
    pub(crate) fn mark_layout(&self) {
        let path = self.dir.join(LAYOUT_FILE);
        if path.exists() && matches!(layout_of(&self.dir), Ok(Some(LAYOUT))) {
            return;
        }
        if let Err(e) = write_private(&path, format!("{LAYOUT}\n").as_bytes()) {
            tracing::warn!("cannot write {}: {e:#}", path.display());
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Whether the friends file was damaged and its copy from before its last change took its
    /// place: that change (a friend added or removed, say) is missing.
    pub fn friends_restored(&self) -> bool {
        self.friends_restored.load(Ordering::Relaxed)
    }

    /// Locks the folder for a running device, until the returned file is dropped or the
    /// process ends however it ends. Two copies of one device on the same folder (the
    /// application and the command line, say) would each take the other's connections and
    /// write over each other's files; the second one is refused.
    pub fn hold_running(&self) -> Result<fs::File> {
        // A store of this process may be sealing the folder, holding the lock meanwhile: wait
        // for it, or this copy would be refused as if another one ran.
        let looking = key_lock(&self.dir);
        let _looking = looking.lock().unwrap_or_else(PoisonError::into_inner);
        let path = self.dir.join("running.lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        match take_lock(&file) {
            Ok(()) => {
                self.running.store(true, Ordering::Relaxed);
                Ok(file)
            }
            Err(fs::TryLockError::WouldBlock) => bail!(
                "Fogata is already running with the data in {}; close it first",
                self.dir.display()
            ),
            Err(fs::TryLockError::Error(e)) => {
                // A file system without locks: run all the same.
                tracing::debug!("cannot lock {}: {e}", path.display());
                self.running.store(true, Ordering::Relaxed);
                Ok(file)
            }
        }
    }

    fn lock(&self) -> Held<'_> {
        self.lock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Removes the temporary files of writes a crash cut short. Only old ones: a write going
    /// on in another process keeps its own.
    fn remove_stale_temps(&self) {
        // Direct chats write theirs in `direct/` and in each chat's folder there.
        let direct = self.dir.join("direct");
        let chats: Vec<PathBuf> = fs::read_dir(&direct)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        for dir in [
            self.dir.clone(),
            self.dir.join("logs"),
            self.dir.join(MLS_DIR),
            direct,
        ]
        .into_iter()
        .chain(chats)
        {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let stale = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > STALE_TEMP);
                if stale && is_temp_name(&entry.file_name().to_string_lossy()) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    fn meta_path(&self) -> PathBuf {
        self.dir.join("meta.json")
    }

    /// The screen times, the port and where peers were reached. Each part is read on its
    /// own, so one that cannot be read (damaged, or in another version's form) loses only
    /// itself: above all the port stays, which a firewall rule may name.
    pub fn meta(&self) -> Meta {
        let Some(value) = fs::read(self.meta_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            return Meta::default();
        };
        fn part<T: serde::de::DeserializeOwned + Default>(v: &serde_json::Value, k: &str) -> T {
            v.get(k)
                .and_then(|p| serde_json::from_value(p.clone()).ok())
                .unwrap_or_default()
        }
        Meta {
            last_seen: part(&value, "last_seen"),
            last_room: part(&value, "last_room"),
            level_wait: part(&value, "level_wait"),
            port: part(&value, "port"),
            reached: value
                .get("reached")
                .and_then(|r| r.as_object())
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|(address, r)| {
                            Some((address.clone(), serde_json::from_value(r.clone()).ok()?))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn save_meta(&self, meta: &Meta) -> Result<()> {
        let _held = self.lock();
        write_private(&self.meta_path(), &serde_json::to_vec_pretty(meta)?)
    }

    fn member_profiles_path(&self) -> PathBuf {
        self.dir.join("members.json")
    }

    /// The profiles last met of members that are not friends, as each signed it: their
    /// names and pictures show while they are away too. One that does not read is left out.
    pub fn member_profiles(&self) -> Vec<Profile> {
        let Ok(bytes) = fs::read(self.member_profiles_path()) else {
            return Vec::new();
        };
        let list: Vec<String> = serde_json::from_slice(&bytes).unwrap_or_default();
        list.iter()
            .filter_map(|text| hex::decode(text).ok())
            .filter_map(|bytes| Profile::decode(&bytes).ok())
            .collect()
    }

    pub fn save_member_profiles<'a>(
        &self,
        profiles: impl IntoIterator<Item = &'a Profile>,
    ) -> Result<()> {
        let list: Vec<String> = profiles
            .into_iter()
            .map(|p| hex::encode(p.encode()))
            .collect();
        let _held = self.lock();
        write_private(
            &self.member_profiles_path(),
            &serde_json::to_vec_pretty(&list)?,
        )
    }

    fn levels_path(&self) -> PathBuf {
        self.dir.join("levels.json")
    }

    /// The highest capability level each friend and member was seen at (see the network's
    /// `caps`), by device key in hex. Kept in a file of its own, so that older versions,
    /// which do not know it, read the rest of the folder as before.
    pub fn levels(&self) -> Vec<(fogata_crypto::DevicePublicKey, u32)> {
        let Ok(bytes) = fs::read(self.levels_path()) else {
            return Vec::new();
        };
        let map: BTreeMap<String, u32> = serde_json::from_slice(&bytes).unwrap_or_default();
        map.iter()
            .filter_map(|(device, level)| {
                let device = hex::decode(device).ok()?;
                Some((
                    fogata_crypto::DevicePublicKey::from_bytes(&device).ok()?,
                    *level,
                ))
            })
            .collect()
    }

    /// Notes that `device` was seen at capability `level`; a lower one never replaces a
    /// higher one.
    pub fn note_level(&self, device: &fogata_crypto::DevicePublicKey, level: u32) -> Result<()> {
        let _held = self.lock();
        let mut map: BTreeMap<String, u32> = fs::read(self.levels_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let at = map.entry(hex::encode(device.as_bytes())).or_default();
        if *at >= level {
            return Ok(());
        }
        *at = level;
        write_private(&self.levels_path(), &serde_json::to_vec_pretty(&map)?)
    }

    fn animations_path(&self) -> PathBuf {
        self.dir.join("animations.bin")
    }

    /// The animations last met of friends' and members' pictures and banners, as each signed
    /// them. One that does not read is left out.
    pub fn animations(&self) -> Vec<Animation> {
        let Ok(bytes) = fs::read(self.animations_path()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut rest = bytes.as_slice();
        while let Some((len, tail)) = rest.split_first_chunk::<4>() {
            let len = u32::from_be_bytes(*len) as usize;
            let Some((record, tail)) = tail.split_at_checked(len) else {
                break;
            };
            if let Ok(animation) = Animation::decode(record) {
                out.push(animation);
            }
            rest = tail;
        }
        out
    }

    pub fn save_animations<'a>(
        &self,
        animations: impl IntoIterator<Item = &'a Animation>,
    ) -> Result<()> {
        self.save_animations_holding(animations, |_, _| Ok(()))
    }

    /// [`Self::save_animations`], `hold` asked first for room on the disk for the new file
    /// (of so many bytes, in the data folder) while it is written beside the old one: the
    /// others' animations are a copy of what the network gives again, and must not take
    /// the room the disk keeps for the group logs and journals (see `shares::hold_disk`).
    /// When it is refused, the file stays as it was.
    pub fn save_animations_holding<'a, R>(
        &self,
        animations: impl IntoIterator<Item = &'a Animation>,
        hold: impl FnOnce(u64, &Path) -> Result<R>,
    ) -> Result<()> {
        let mut bytes = Vec::new();
        for animation in animations {
            let record = animation.encode();
            bytes.extend_from_slice(&(record.len() as u32).to_be_bytes());
            bytes.extend_from_slice(&record);
        }
        let _room = hold(bytes.len() as u64, &self.dir)?;
        let _held = self.lock();
        write_private(&self.animations_path(), &bytes)
    }

    /// Joins with an invite still waiting for a manager, so they go on after a restart.
    pub fn joins(&self) -> Vec<PendingJoin> {
        fs::read(self.dir.join("joins.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Keeps `join` among the joins waiting (`None` takes the one of `group` out).
    pub fn note_join(&self, group: GroupId, join: Option<PendingJoin>) -> Result<()> {
        let _held = self.lock();
        let mut joins = self.joins();
        let before = joins.len();
        joins.retain(|j| j.group_id != hex::encode(group.as_bytes()));
        if join.is_none() && joins.len() == before {
            return Ok(());
        }
        joins.extend(join);
        write_private(
            &self.dir.join("joins.json"),
            &serde_json::to_vec_pretty(&joins)?,
        )
    }

    fn identity_path(&self) -> PathBuf {
        self.dir.join("identity.key")
    }

    fn groups_path(&self) -> PathBuf {
        self.dir.join("groups.json")
    }

    fn friends_path(&self) -> PathBuf {
        self.dir.join("friends.bin")
    }

    /// The profile and friends as they were before their last change.
    fn friends_copy_path(&self) -> PathBuf {
        self.dir.join("friends.bin.bak")
    }

    /// Whether a profile was made on this device. A sealed friends file is not opened, so the
    /// keychain is not asked for its key here; it is in a folder that has a key and where
    /// the friends file is plain or refused, as looking for the key decides whether that is
    /// the account's own (and takes it back from [`REFUSED_DIR`] when it is).
    pub fn has_profile(&self) -> Result<bool> {
        let keyed = || vault::key_name(&self.dir).is_some();
        match fs::read(self.friends_path()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if !(keyed() && self.friends_refused().is_some()) {
                    return Ok(false);
                }
                self.data_key()?;
                Ok(fs::symlink_metadata(self.friends_path()).is_ok())
            }
            Ok(bytes) if sealed_form(&bytes).is_some() => Ok(true),
            Ok(bytes) if !keyed() && FriendBook::decode(&bytes).is_ok() => Ok(true),
            _ => Ok(self.friends()?.is_some()),
        }
    }

    /// A friends file (or its copy) that sealing refused and moved to [`REFUSED_DIR`].
    fn friends_refused(&self) -> Option<PathBuf> {
        let mut names: Vec<_> = fs::read_dir(self.dir.join(REFUSED_DIR))
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("friends.bin"))
            })
            .collect();
        names.sort();
        names.into_iter().next()
    }

    /// Where the profile and friends made on this device were kept when they could not be
    /// used, while the folder has no friends file: refused by sealing ([`REFUSED_DIR`]), or
    /// in the folder itself, damaged with their copy (set aside as `friends.bin.damaged`)
    /// or left as a copy alone. Never deleted: the start screen says where they are, rather
    /// than look like a new device.
    pub fn friends_kept(&self) -> Option<PathBuf> {
        if fs::symlink_metadata(self.friends_path()).is_ok() {
            return None;
        }
        if self.friends_refused().is_some() {
            return Some(self.dir.join(REFUSED_DIR));
        }
        let kept = fs::read_dir(&self.dir).ok()?.flatten().any(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("friends.bin.") && !is_temp_name(&name)
        });
        kept.then(|| self.dir.clone())
    }

    /// Whether sealing refused plain files in this folder that are still kept in
    /// [`REFUSED_DIR`].
    pub fn holds_refused(&self) -> bool {
        fs::read_dir(self.dir.join(REFUSED_DIR)).is_ok_and(|mut d| d.next().is_some())
    }

    /// Profile and friends, if a profile was made on this device.
    pub fn friends(&self) -> Result<Option<FriendBook>> {
        let key = self.data_key()?;
        let held = self.lock();
        self.read_friends(&held, key.as_ref())
    }

    /// When the file is damaged, the copy from before its last change takes its place; when
    /// that is damaged too, the device goes on without a profile. The damaged file is put
    /// aside, never deleted.
    fn read_friends(&self, _held: &Held, key: Option<&DataKey>) -> Result<Option<FriendBook>> {
        let path = self.friends_path();
        let damage = match fs::read(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => anyhow::Error::from(e),
            Ok(bytes) => match decode_book(key, &bytes) {
                Ok(book) => return Ok(Some(book)),
                Err(e)
                    if e.downcast_ref::<fogata_friends::FriendError>()
                        == Some(&fogata_friends::FriendError::NewerVersion) =>
                {
                    bail!(
                        "{} was saved by a newer version of Fogata; update Fogata to open it",
                        path.display()
                    )
                }
                Err(e) => e,
            },
        };
        let aside = aside_path(&path);
        let cannot_put_aside = || {
            format!(
                "{} cannot be read ({damage:#}) nor put aside",
                path.display()
            )
        };
        let copy = fs::read(self.friends_copy_path())
            .ok()
            .and_then(|bytes| Some((decode_book(key, &bytes).ok()?, bytes)));
        let Some((book, bytes)) = copy else {
            fs::rename(&path, &aside).with_context(cannot_put_aside)?;
            // Its copy goes aside too: the first profile made next would write its own copy
            // over it.
            let copy = self.friends_copy_path();
            if fs::symlink_metadata(&copy).is_ok() {
                let copy_aside = aside_path(&path);
                if let Err(e) = fs::rename(&copy, &copy_aside) {
                    tracing::warn!("cannot put {} aside: {e}", copy.display());
                }
            }
            tracing::error!(
                "{} could not be read ({damage:#}) and its copy neither: it is kept as {}, \
                 and this device goes on without a profile",
                path.display(),
                aside.display()
            );
            return Ok(None);
        };
        copy_aside(&path, &aside).with_context(cannot_put_aside)?;
        // A copy left plain by sealing cut short is sealed as it takes the file's place.
        let bytes = match key {
            Some(key) if sealed_form(&bytes).is_none() => key.seal_whole("friends", &bytes)?,
            _ => bytes,
        };
        if let Err(e) = write_private(&path, &bytes) {
            // The damaged file is still in its place; the next read puts it aside again.
            let _ = fs::remove_file(&aside);
            return Err(e);
        }
        tracing::warn!(
            "{} could not be read ({damage:#}): it is kept as {}, and the copy from before \
             its last change is used",
            path.display(),
            aside.display()
        );
        self.friends_restored.store(true, Ordering::Relaxed);
        Ok(Some(book))
    }

    /// Profile and friends, or an error telling how to make a profile.
    pub fn require_friends(&self) -> Result<FriendBook> {
        self.friends()?
            .context("make your profile first: fogata profile --name <your name>")
    }

    /// Saves the profile and friends. What the file held is copied first, if it still reads,
    /// so that a damaged file can fall back on it.
    pub fn save_friends(&self, book: &FriendBook) -> Result<()> {
        let key = self.data_key()?;
        let _held = self.lock();
        let path = self.friends_path();
        let new = book.encode();
        let new_on_disk = to_disk(key.as_ref(), "friends", &new)?;
        match fs::read(&path) {
            Ok(old) => match from_disk(key.as_ref(), "friends", &old) {
                Ok(plain) if *plain == *new => return Ok(()),
                Ok(plain) if FriendBook::decode(&plain).is_ok() => {
                    write_private(&self.friends_copy_path(), &old)?;
                }
                _ => {}
            },
            // The first profile is its own copy; one left by a profile that could not be
            // read is set aside rather than written over.
            Err(_) => {
                let copy = self.friends_copy_path();
                if fs::symlink_metadata(&copy).is_ok() {
                    fs::rename(&copy, aside_path(&path))
                        .with_context(|| format!("cannot put {} aside", copy.display()))?;
                }
                write_private(&copy, &new_on_disk)?
            }
        }
        write_private(&path, &new_on_disk)
    }

    fn log_path(&self, group_id: &GroupId) -> PathBuf {
        self.dir
            .join("logs")
            .join(format!("{}.log", hex::encode(group_id.as_bytes())))
    }

    /// What was judged of the group's log, beside it (see [`read_judged`]).
    fn judged_path(&self, group_id: &GroupId) -> PathBuf {
        with_suffix(&self.log_path(group_id), JUDGED_SUFFIX)
    }

    /// The group's room rules as earlier versions kept them, in a file of their own: read
    /// with the log, and gone once the log is written again with them in it.
    fn rules_path(&self, group_id: &GroupId) -> PathBuf {
        self.dir
            .join("logs")
            .join(format!("{}.rules", hex::encode(group_id.as_bytes())))
    }

    /// The group's banner, in a file of its own.
    fn banner_path(&self, group_id: &GroupId) -> PathBuf {
        self.dir
            .join("logs")
            .join(format!("{}.banner", hex::encode(group_id.as_bytes())))
    }

    /// Forgets the banner and picture kept of a group, and their changes seen here, for one
    /// this device left or was removed from.
    pub fn forget_group_pictures(&self, group_id: &GroupId) {
        let _ = fs::remove_file(self.banner_path(group_id));
        let _ = fs::remove_file(self.picture_path(group_id));
        let _ = fs::remove_file(self.history_path(group_id));
    }

    /// The changes of the group's banner and picture this device saw (see [`RecordSeen`]).
    fn history_path(&self, group_id: &GroupId) -> PathBuf {
        self.dir
            .join("logs")
            .join(format!("{}.history", hex::encode(group_id.as_bytes())))
    }

    /// The changes of group `id`'s banner and picture this device saw, the oldest first.
    pub fn record_history(&self, id: &GroupId) -> Vec<RecordSeen> {
        fs::read(self.history_path(id))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Notes the banner and picture records of `log` this device had not seen yet.
    fn note_records(&self, log: &GroupLog) -> Result<()> {
        let mut seen = Vec::new();
        if let Some(banner) = log.banner_record() {
            seen.push(RecordSeen::of(RecordKind::Banner, banner));
        }
        if let Some(picture) = log.picture_record() {
            seen.push(RecordSeen::of(RecordKind::Picture, picture));
        }
        let mut history = self.record_history(&log.group_id());
        let before = history.len();
        for record in seen {
            if !history.iter().any(|h| h.signature == record.signature) {
                history.push(record);
            }
        }
        if history.len() == before {
            return Ok(());
        }
        // The oldest go, but never the newest of a kind: it would be noted again as new.
        while history.len() > MAX_RECORDS_SEEN {
            let Some(at) = (0..history.len()).find(|i| {
                history[i + 1..]
                    .iter()
                    .any(|later| later.kind == history[*i].kind)
            }) else {
                break;
            };
            history.remove(at);
        }
        write_private(
            &self.history_path(&log.group_id()),
            &serde_json::to_vec(&history)?,
        )
    }

    /// The group's picture record, in a file of its own like the banner.
    fn picture_path(&self, group_id: &GroupId) -> PathBuf {
        self.dir
            .join("logs")
            .join(format!("{}.picture", hex::encode(group_id.as_bytes())))
    }

    /// This device's MLS state for a group, in a file of its own written whole, so that what
    /// a newer state leaves out (secrets of past epochs, say) goes from the disk with the
    /// file it replaces.
    fn mls_path(&self, group_id: &GroupId) -> PathBuf {
        self.dir
            .join(MLS_DIR)
            .join(format!("{}.bin", hex::encode(group_id.as_bytes())))
    }

    /// This device's MLS state for the group, if it keeps one (see
    /// `fogata_mls::GroupMember::to_bytes`).
    pub fn mls_state(&self, group_id: &GroupId) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let key = self.data_key()?;
        let path = self.mls_path(group_id);
        let bytes = match fs::read(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            read => {
                Zeroizing::new(read.with_context(|| format!("cannot read {}", path.display()))?)
            }
        };
        let context = mls_context(&hex::encode(group_id.as_bytes()));
        Ok(Some(
            from_disk(key.as_ref(), &context, &bytes)
                .with_context(|| format!("{} is damaged", path.display()))?,
        ))
    }

    /// Keeps `state` as this device's MLS state for the group, sealed with the folder's key
    /// when it has one, under a context that names the group.
    pub fn save_mls_state(&self, group_id: &GroupId, state: &[u8]) -> Result<()> {
        let key = self.data_key()?;
        let dir = self.dir.join(MLS_DIR);
        fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let context = mls_context(&hex::encode(group_id.as_bytes()));
        write_private(
            &self.mls_path(group_id),
            &to_disk(key.as_ref(), &context, state)?,
        )
    }

    /// Forgets this device's MLS state for the group.
    pub fn remove_mls_state(&self, group_id: &GroupId) -> Result<()> {
        let path = self.mls_path(group_id);
        match fs::remove_file(&path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            removed => removed.with_context(|| format!("cannot remove {}", path.display())),
        }
    }

    /// Loads this device's identity, creating it on first use.
    pub fn identity(&self) -> Result<(DeviceIdentity, bool)> {
        // An account moved to another computer is never used here again, nor replaced.
        crate::transfer::refuse_if_moved(&self.dir)?;
        let key = self.data_key()?;
        let _held = self.lock();
        let path = self.identity_path();
        if path.exists() {
            let bytes =
                fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
            let identity = read_identity(key.as_ref(), &bytes)
                .with_context(|| format!("{} is damaged", path.display()))?;
            return Ok((identity, false));
        }
        // Others know this device by the key its profile and groups were made with: a new one
        // would silently make it a stranger to all of them.
        let has_logs = fs::read_dir(self.dir.join("logs")).is_ok_and(|mut d| {
            d.any(|e| e.is_ok_and(|e| e.path().extension().is_some_and(|x| x == "log")))
        });
        if self.friends_path().exists() || has_logs {
            if self.dir.join(REFUSED_DIR).join("identity.key").exists() {
                bail!(
                    "{} was found unencrypted in this folder, whose data is encrypted, so it \
                     was put there by something other than Fogata and was moved to {}: put \
                     the encrypted file back from a copy, or move the folder away to start as \
                     a new device",
                    path.display(),
                    self.dir.join(REFUSED_DIR).display()
                );
            }
            bail!(
                "{} is missing, but this folder holds a profile or groups made with it: put \
                 the file back from a copy, or move the folder away to start as a new device",
                path.display()
            );
        }
        let identity = DeviceIdentity::generate()?;
        write_identity(key.as_ref(), &path, &identity)?;
        Ok((identity, true))
    }

    /// This device's key, if it has an identity, without making one.
    fn own_key(&self) -> Option<DevicePublicKey> {
        let key = self.data_key().ok()?;
        let bytes = fs::read(self.identity_path()).ok()?;
        Some(read_identity(key.as_ref(), &bytes)?.public_key())
    }

    /// Replaces this device's identity with a new one. Only while it has no profile and no
    /// groups: afterwards others know the device by its key.
    pub fn new_identity(&self) -> Result<DeviceIdentity> {
        let key = self.data_key()?;
        let held = self.lock();
        if self.read_friends(&held, key.as_ref())?.is_some()
            || !self.read_groups(&held)?.groups.is_empty()
        {
            bail!("this device is already known by its key; it cannot get another one");
        }
        let identity = DeviceIdentity::generate()?;
        write_identity(key.as_ref(), &self.identity_path(), &identity)?;
        Ok(identity)
    }
}

/// A log as its file holds it: each entry in the common order, after its length, and then
/// the entry that tells of a level of the group above this version's, if any
/// ([`GroupLog::entries_to_store`]).
pub(crate) fn encode_log(log: &GroupLog) -> Vec<u8> {
    encode_entries(log.entries_to_store())
}

/// Entries one after another, each after its length.
fn encode_entries(entries: Vec<&Entry>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries {
        let encoded = entry.encode();
        bytes.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&encoded);
    }
    bytes
}

/// `base`, or `base (2)` and so on: a name none of `groups` has.
fn free_name(groups: &[StoredGroup], base: &str) -> String {
    let base = if base.trim().is_empty() {
        "group"
    } else {
        base.trim()
    };
    let mut name = base.to_string();
    let mut n = 2;
    while groups.iter().any(|g| g.name == name) {
        name = format!("{base} ({n})");
        n += 1;
    }
    name
}

/// The entries of a log file that can be read, and whether some of it could not. Past a part
/// that does not read, the next entry is looked for a byte at a time, so one damaged entry
/// (or length) does not lose the ones after it. Each entry is signed: what is found is
/// checked as any entry is when it goes into a log.
pub(crate) fn decode_entries(bytes: &[u8]) -> (Vec<Entry>, bool) {
    let mut entries = Vec::new();
    let mut damaged = false;
    let mut at = 0;
    while at < bytes.len() {
        match entry_at(&bytes[at..]) {
            Some((entry, used)) => {
                entries.push(entry);
                at += used;
            }
            None => {
                damaged = true;
                at += 1;
            }
        }
    }
    (entries, damaged)
}

/// The entry at the start of `bytes`, after its length, and the bytes the two take.
fn entry_at(bytes: &[u8]) -> Option<(Entry, usize)> {
    let (len, tail) = bytes.split_at_checked(4)?;
    let len = u32::from_be_bytes(len.try_into().expect("4 bytes")) as usize;
    let (entry, _) = tail.split_at_checked(len)?;
    Some((Entry::decode(entry).ok()?, 4 + len))
}

/// `path` with `suffix` added to its name.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Where to put a damaged file aside: `path` with `.damaged` added to its name, or
/// `.damaged.2` and so on when an earlier one is there, even a link to nothing.
fn aside_path(path: &Path) -> PathBuf {
    std::iter::once(with_suffix(path, ".damaged"))
        .chain((2..).map(|n| with_suffix(path, &format!(".damaged.{n}"))))
        .find(|p| fs::symlink_metadata(p).is_err())
        .expect("some name is free")
}

/// Copies `from` to `to`, a new file only this user reads: never through a link, nor over
/// anything, left at `to`.
fn copy_aside(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut source = fs::File::open(from)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut out = options.open(to)?;
    let copied = std::io::copy(&mut source, &mut out).and_then(|_| out.sync_all());
    if copied.is_err() {
        let _ = fs::remove_file(to);
    }
    copied
}

/// Plain copies of damaged files made before the folder was sealed are sealed under this.
pub(crate) const DAMAGED: &str = "damaged";

/// What the MLS state of the group named `group` (in hex) is sealed for: a group's file put
/// in another's place does not open.
pub(crate) fn mls_context(group: &str) -> String {
    format!("{MLS_DIR}/{group}")
}

/// What a file holds: `bytes` opened with `key` when they are sealed, else as they are. A
/// sealed file without a key does not read.
pub(crate) fn from_disk(
    key: Option<&DataKey>,
    context: &str,
    bytes: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    match (sealed_form(bytes), key) {
        (None, _) => Ok(Zeroizing::new(bytes.to_vec())),
        (Some(Form::Whole | Form::Named), Some(key)) => Ok(key.open_whole(context, bytes)?),
        (Some(_), Some(_)) => bail!("not sealed as expected"),
        (Some(_), None) => bail!("sealed, and this folder has no key"),
    }
}

/// What the file at `path` holds, when it is there and holds something: an error when that
/// is not sealed.
pub(crate) fn sealed_bytes(path: &Path) -> Result<Option<Zeroizing<Vec<u8>>>> {
    let bytes = match fs::read(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        read => Zeroizing::new(read.with_context(|| format!("cannot read {}", path.display()))?),
    };
    if bytes.is_empty() {
        return Ok(None);
    }
    if sealed_form(&bytes).is_none() {
        bail!("{} is not sealed", path.display());
    }
    Ok(Some(bytes))
}

/// `plain` as a file holds it: sealed with `key` when the folder has one.
pub(crate) fn to_disk(key: Option<&DataKey>, context: &str, plain: &[u8]) -> Result<Vec<u8>> {
    Ok(match key {
        Some(key) => key.seal_whole(context, plain)?,
        None => plain.to_vec(),
    })
}

fn decode_book(key: Option<&DataKey>, bytes: &[u8]) -> Result<FriendBook> {
    Ok(FriendBook::decode(&from_disk(key, "friends", bytes)?)?)
}

/// An identity saved as its secret in hex, sealed or not.
fn read_identity(key: Option<&DataKey>, bytes: &[u8]) -> Option<DeviceIdentity> {
    let plain = from_disk(key, "identity", bytes).ok()?;
    let text = std::str::from_utf8(&plain).ok()?;
    let bytes: Zeroizing<Vec<u8>> = Zeroizing::new(hex::decode(text.trim()).ok()?);
    let bytes: &[u8; 32] = bytes.as_slice().try_into().ok()?;
    Some(DeviceIdentity::from_secret_bytes(bytes))
}

fn write_identity(key: Option<&DataKey>, path: &Path, identity: &DeviceIdentity) -> Result<()> {
    let secret = identity.secret_bytes();
    let text = Zeroizing::new(hex::encode(secret.as_slice()));
    write_private(path, &to_disk(key, "identity", text.as_bytes())?)
}

/// How long a running lock found held is tried again before another copy of Fogata is said
/// to run on the folder ([`take_lock`]).
const LOCK_LINGERS: Duration = Duration::from_millis(500);

/// Takes the lock on `file`, a folder's `running.lock`, trying again for [`LOCK_LINGERS`]
/// while it is held. A lock this process has just let go of can still look held for a few
/// milliseconds: a program that another thread starts at that moment gets a copy of every
/// open file until it begins to run, and the lock goes only once the last copy is closed.
/// Without waiting, a start right after a restore or a move would be refused as if another
/// copy of Fogata ran; one that really runs holds the lock far longer than this.
fn take_lock(file: &fs::File) -> Result<(), fs::TryLockError> {
    let started = std::time::Instant::now();
    loop {
        match file.try_lock() {
            Err(fs::TryLockError::WouldBlock) if started.elapsed() < LOCK_LINGERS => {
                std::thread::sleep(Duration::from_millis(5));
            }
            taken => return taken,
        }
    }
}

/// The lock of the folder `dir`, the same for every `Store` of it in this process.
fn folder_lock(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let key = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(key)
        .or_default()
        .clone()
}

/// Held while a `Store` of the folder `dir` in this process looks for its key (and may seal
/// it), and by [`Store::hold_running`] while it takes the folder's running lock.
fn key_lock(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let key = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(key)
        .or_default()
        .clone()
}

/// Whether `name` is one of [`write_private`]'s temporary files: `name.pid.n.tmp`.
pub(crate) fn is_temp_name(name: &str) -> bool {
    let mut parts = name.rsplit('.');
    let number =
        |p: Option<&str>| p.is_some_and(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    parts.next() == Some("tmp")
        && number(parts.next())
        && number(parts.next())
        && parts.next().is_some()
}

/// Removes `dir`, a folder of plain copies made while sealing, if it is there.
fn remove_copies(dir: &Path) {
    if !dir.exists() {
        return;
    }
    match fs::remove_dir_all(dir) {
        Ok(()) => tracing::info!("removed the plain copies in {}", dir.display()),
        Err(e) => tracing::warn!("cannot remove {}: {e}", dir.display()),
    }
}

/// Writes a file readable by its owner only (on Unix), through a temporary file of its own,
/// so a crash leaves either the old contents or the new ones and two writes at once never
/// share one. The contents and the rename are flushed to the disk before it returns.
pub fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
    #[cfg(test)]
    faults::check().with_context(|| format!("cannot write {}", path.display()))?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut name = path
        .file_name()
        .with_context(|| format!("cannot write {}", path.display()))?
        .to_owned();
    name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = path.with_file_name(name);
    let written = write_new(&tmp, contents)
        .with_context(|| format!("cannot write {}", tmp.display()))
        .and_then(|()| {
            fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))
        });
    if written.is_err()
        && let Err(e) = fs::remove_file(&tmp)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!("cannot remove {}: {e}", tmp.display());
    }
    written?;
    sync_folder(path);
    Ok(())
}

/// Writes made to fail in tests, as a full disk or a computer that stops half way would:
/// on this thread, every write after the first `n` allowed fails.
#[cfg(test)]
pub(crate) mod faults {
    use std::cell::Cell;

    thread_local! {
        static LEFT: Cell<Option<usize>> = const { Cell::new(None) };
        static TRIED: Cell<usize> = const { Cell::new(0) };
    }

    /// Lets the next `n` writes through and fails every one after; `None` lets all through.
    pub(crate) fn fail_after(n: Option<usize>) {
        LEFT.set(n);
        TRIED.set(0);
    }

    /// The writes tried since [`fail_after`].
    pub(crate) fn tried() -> usize {
        TRIED.get()
    }

    pub(crate) fn check() -> std::io::Result<()> {
        TRIED.set(TRIED.get() + 1);
        match LEFT.get() {
            Some(0) => Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "no room left (made to fail)",
            )),
            Some(n) => {
                LEFT.set(Some(n - 1));
                Ok(())
            }
            None => Ok(()),
        }
    }
}

fn write_new(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// Flushes the folder of `path` to the disk, so that a rename in it lasts through a power
/// cut. Some file systems refuse it; the rename is made all the same. On Windows a folder
/// cannot be flushed like this, and NTFS keeps renames in its journal.
fn sync_folder(path: &Path) {
    #[cfg(unix)]
    {
        let dir = match path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        if let Err(e) = fs::File::open(dir).and_then(|d| d.sync_all()) {
            tracing::debug!("cannot flush {}: {e}", dir.display());
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use fogata_state::{Body, GroupBanner, GroupPicture};

    use super::*;

    #[test]
    fn a_roaming_data_folder_moves_to_the_local_one_and_still_opens() {
        let base = std::env::temp_dir().join(format!("fogata-local-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let roaming = base.join("Roaming").join("Fogata");
        let local = base.join("Local").join("Fogata");
        // Nothing yet: the local folder, made by whoever opens it.
        assert_eq!(
            local_data_dir(Some(local.clone()), Some(roaming.clone())),
            Some(local.clone())
        );
        assert!(!local.exists());
        // A folder an earlier copy made, with what it keeps: moved whole, and it opens.
        let store = Store::open(roaming.clone()).unwrap();
        store
            .save_meta(&Meta {
                port: Some(39756),
                ..Meta::default()
            })
            .unwrap();
        drop(store);
        fs::write(roaming.join("app.json"), b"{}").unwrap();
        assert_eq!(
            local_data_dir(Some(local.clone()), Some(roaming.clone())),
            Some(local.clone())
        );
        assert!(!roaming.exists());
        assert!(local.join("app.json").exists());
        assert_eq!(Store::open(local.clone()).unwrap().meta().port, Some(39756));
        // Once there, it stays there, whatever comes back at the old place.
        fs::create_dir_all(&roaming).unwrap();
        assert_eq!(
            local_data_dir(Some(local.clone()), Some(roaming.clone())),
            Some(local.clone())
        );
        // Without the local place, the roaming one.
        assert_eq!(local_data_dir(None, Some(roaming.clone())), Some(roaming));
        assert_eq!(local_data_dir(None, None), None);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn no_data_folder_without_an_absolute_one_from_the_system() {
        let names = ["HOME", "XDG_DATA_HOME", "LOCALAPPDATA", "APPDATA"];
        let none = data_dir_from(&|_| None).unwrap_err().to_string();
        assert!(
            none.starts_with("no folder to keep Fogata's data in"),
            "{none}"
        );
        // Empty or relative: the folder Fogata was started from would get the data.
        for value in ["", "fogata-data", "home/ana"] {
            assert!(data_dir_from(&|name| names.contains(&name).then(|| value.into())).is_err());
        }
        let absolute = std::env::temp_dir().join("fogata-no-such-home");
        let dir = data_dir_from(&|name| {
            (name == "HOME" || name == "LOCALAPPDATA").then(|| absolute.clone().into())
        })
        .unwrap();
        assert!(dir.is_absolute() && dir.starts_with(&absolute), "{dir:?}");
        // A relative XDG_DATA_HOME is passed over for HOME.
        let dir = data_dir_from(&|name| match name {
            "XDG_DATA_HOME" | "APPDATA" => Some("data".into()),
            "HOME" | "LOCALAPPDATA" => Some(absolute.clone().into()),
            _ => None,
        })
        .unwrap();
        assert!(dir.starts_with(&absolute), "{dir:?}");
    }

    #[test]
    fn a_data_folder_that_cannot_move_stays_in_use() {
        let base = std::env::temp_dir().join(format!("fogata-stuck-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let roaming = base.join("Roaming").join("Fogata");
        fs::create_dir_all(&roaming).unwrap();
        // The local place's parent is a file: nothing can go under it.
        fs::write(base.join("Local"), b"").unwrap();
        let local = base.join("Local").join("Fogata");
        assert_eq!(
            local_data_dir(Some(local), Some(roaming.clone())),
            Some(roaming.clone())
        );
        assert!(roaming.is_dir());
        fs::remove_dir_all(&base).unwrap();
    }

    /// A data folder is marked with the layout this build writes as a core runs in it;
    /// one with no marker (from before there was one) or a marker cut short opens; one a
    /// newer layout wrote is refused, not misread. Opening it only to look writes nothing.
    #[test]
    fn a_data_folder_of_a_newer_layout_is_refused() {
        let dir = std::env::temp_dir().join(format!("fogata-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let marker = dir.join(LAYOUT_FILE);
        let read = || fs::read_to_string(&marker).unwrap();
        let store = Store::open(dir.clone()).unwrap();
        assert!(!marker.exists());
        store.mark_layout();
        assert_eq!(read(), format!("{LAYOUT}\n"));
        drop(store);
        // From before the marker, and cut short.
        for before in [None, Some("")] {
            match before {
                None => fs::remove_file(&marker).unwrap(),
                Some(text) => fs::write(&marker, text).unwrap(),
            }
            Store::open(dir.clone()).unwrap().mark_layout();
            assert_eq!(read(), format!("{LAYOUT}\n"));
        }
        fs::write(&marker, format!("{}\n", LAYOUT + 1)).unwrap();
        let refused = Store::open(dir.clone()).err().unwrap();
        assert!(refused.to_string().contains("newer version"), "{refused:#}");
        // Left as it is, for the newer build.
        assert_eq!(read(), format!("{}\n", LAYOUT + 1));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_part_of_meta_that_cannot_be_read_loses_only_itself() {
        let dir = std::env::temp_dir().join(format!("fogata-store-meta-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        // An earlier form of `reached`, and a time that is not a number.
        fs::write(
            dir.join("meta.json"),
            br#"{"port": 39756, "last_seen": {"ab": 5, "cd": "x"},
                "reached": {"/ip4/192.0.2.1/udp/1/quic-v1": 7,
                            "/ip4/192.0.2.2/udp/2/quic-v1": {"device": "ab", "at_ms": 9}}}"#,
        )
        .unwrap();
        let meta = store.meta();
        assert_eq!(meta.port, Some(39756));
        assert_eq!(meta.reached.len(), 1);
        assert_eq!(meta.reached["/ip4/192.0.2.2/udp/2/quic-v1"].at_ms, 9);
        assert!(meta.last_seen.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn capability_levels_are_kept_and_only_raised() {
        let dir = std::env::temp_dir().join(format!("fogata-levels-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        assert!(store.levels().is_empty());
        let device = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        store.note_level(&device, 3).unwrap();
        store.note_level(&device, 2).unwrap();
        assert_eq!(store.levels(), vec![(device, 3)]);
        // A damaged file reads as nothing known, and is written again whole.
        fs::write(dir.join("levels.json"), b"{not json").unwrap();
        assert!(store.levels().is_empty());
        store.note_level(&device, 2).unwrap();
        assert_eq!(store.levels(), vec![(device, 2)]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reached_addresses_are_kept_newest_first_for_a_month() {
        let mut meta = Meta::default();
        let address = |i| format!("/ip4/192.0.2.{i}/udp/30000/quic-v1");
        for i in 0..40u64 {
            meta.note_reached(address(i), format!("d{i}"), 1000 + i);
        }
        let addresses = meta.reached_addresses(2000);
        assert_eq!(addresses.len(), MAX_REACHED);
        assert_eq!(addresses[0], (address(39), "d39".to_string()));
        assert!(!addresses.iter().any(|(a, _)| *a == address(0)));
        // Reached again, it comes first.
        meta.note_reached(address(20), "d20".into(), 3000);
        assert_eq!(meta.reached_addresses(3000)[0].0, address(20));
        assert_eq!(meta.reached_addresses(3000 + REACHED_FOR_MS).len(), 0);
    }

    #[test]
    fn old_temporary_files_go_when_the_folder_opens() {
        let dir = std::env::temp_dir().join(format!("fogata-temps-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let chat = dir.join("direct").join("ab12");
        fs::create_dir_all(&chat).unwrap();
        fs::create_dir_all(dir.join("logs")).unwrap();
        let old = [
            dir.join("groups.json.7.0.tmp"),
            dir.join("logs").join("0a1b.log.7.1.tmp"),
            dir.join("direct").join("settings.json.7.2.tmp"),
            chat.join("journal.7.3.tmp"),
        ];
        let fresh = chat.join("journal.8.0.tmp");
        let kept = chat.join("journal");
        let long_ago = std::time::SystemTime::now() - STALE_TEMP * 2;
        for path in old.iter().chain([&kept]) {
            fs::write(path, b"x").unwrap();
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(long_ago)
                .unwrap();
        }
        fs::write(&fresh, b"x").unwrap();
        Store::open(dir.clone()).unwrap();
        assert!(old.iter().all(|p| !p.exists()));
        // One that may still be on its way, and what is not a temporary file, stay.
        assert!(fresh.exists() && kept.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn temporary_files_are_told_apart() {
        assert!(is_temp_name("groups.json.123.4.tmp"));
        assert!(is_temp_name("0a1b.log.9.0.tmp"));
        assert!(!is_temp_name("notes.tmp"));
        assert!(!is_temp_name("12.3.tmp"));
        assert!(!is_temp_name("groups.json.x.4.tmp"));
        assert!(!is_temp_name("groups.json"));
    }

    #[test]
    fn banners_and_pictures_stored_dated_far_ahead_are_not_read_back() {
        let dir = std::env::temp_dir().join(format!("fogata-store-ahead-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let log = GroupLog::create(&me, "Home", 1).unwrap();
        store.save_group_log(&log, "Home").unwrap();
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.resize(100, 1);
        let read = || store.load_log(&store.group("Home").unwrap()).unwrap();
        // Written as a version that took them from the network did, before records dated
        // more than a day ahead were refused there: they would show over every banner and
        // picture chosen for as long.
        let ahead = crate::now_ms() + 2 * fogata_state::MAX_RECORD_AHEAD_MS;
        let id = log.group_id();
        let banner = GroupBanner::sign(&me, id, b"GIF89a".to_vec(), ahead).unwrap();
        let picture = GroupPicture::sign(&me, id, png.clone(), ahead).unwrap();
        fs::write(store.banner_path(&id), banner.encode()).unwrap();
        fs::write(store.picture_path(&id), picture.encode()).unwrap();
        let log = read();
        assert!(log.banner_record().is_none() && log.picture_record().is_none());
        // Dated now, they are.
        let now = crate::now_ms();
        let banner = GroupBanner::sign(&me, id, b"GIF89a".to_vec(), now).unwrap();
        let picture = GroupPicture::sign(&me, id, png, now).unwrap();
        fs::write(store.banner_path(&id), banner.encode()).unwrap();
        fs::write(store.picture_path(&id), picture.encode()).unwrap();
        let log = read();
        assert!(log.banner_record().is_some() && log.picture_record().is_some());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_banners_and_pictures_seen_are_noted_once_and_the_oldest_go() {
        let dir = std::env::temp_dir().join(format!("fogata-store-seen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (me, _) = store.identity().unwrap();
        let mut log = GroupLog::create(&me, "Home", 1).unwrap();
        store.save_group_log(&log, "Home").unwrap();
        assert!(store.record_history(&log.group_id()).is_empty());
        let png = |fill: u8| {
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.resize(100, fill);
            png
        };
        log.set_picture(&me, png(1), 10).unwrap();
        store.save_log("Home", &log).unwrap();
        // Saved again unchanged: noted once.
        store.save_log("Home", &log).unwrap();
        log.set_banner(&me, b"GIF89a".to_vec(), 11).unwrap();
        log.set_picture(&me, Vec::new(), 12).unwrap();
        store.save_log("Home", &log).unwrap();
        let seen = store.record_history(&log.group_id());
        let kinds: Vec<(RecordKind, bool)> = seen.iter().map(|r| (r.kind, r.taken_away)).collect();
        assert_eq!(
            kinds,
            [
                (RecordKind::Picture, false),
                (RecordKind::Banner, false),
                (RecordKind::Picture, true)
            ]
        );
        assert!(
            seen.iter()
                .all(|r| r.author == hex::encode(me.public_key().as_bytes()))
        );
        for i in 0..MAX_RECORDS_SEEN {
            log.set_picture(&me, png(i as u8), 100 + i as u64).unwrap();
            store.save_log("Home", &log).unwrap();
        }
        // The banner, still the one shown, stays; the oldest pictures went.
        let seen = store.record_history(&log.group_id());
        assert_eq!(seen.len(), MAX_RECORDS_SEEN);
        assert_eq!(seen[0].kind, RecordKind::Banner);
        assert!(
            seen[1..]
                .iter()
                .all(|r| r.kind == RecordKind::Picture && !r.taken_away)
        );
        // Gone with the group's pictures when the group is.
        store.forget_group_pictures(&log.group_id());
        assert!(store.record_history(&log.group_id()).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Reads every group log under the folders in `FOGATA_REPLAY_LOGS` (separated by `:`,
    /// searched to any depth) and fails if one has an entry ignored only for a rule that
    /// versions before it did not have: such a log would read differently on older copies.
    /// Run by hand on copies of real data folders:
    /// `FOGATA_REPLAY_LOGS=<dir> cargo test -p fogata-core replay -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn replay_logs_under_the_newer_rules() {
        fn logs(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    logs(&path, out);
                } else if path.extension().is_some_and(|e| e == "log")
                    && path.parent().is_some_and(|p| p.ends_with("logs"))
                {
                    out.push(path);
                }
            }
        }
        let mut found = Vec::new();
        for dir in std::env::var("FOGATA_REPLAY_LOGS")
            .unwrap_or_default()
            .split(':')
        {
            logs(Path::new(dir), &mut found);
        }
        let mut differ = 0;
        for path in &found {
            let (entries, _) = decode_entries(&fs::read(path).unwrap());
            let Some(group) = entries.first().map(|e| e.group_id) else {
                continue;
            };
            let mut log = GroupLog::empty(group);
            log.insert_all(entries);
            let state = log.state();
            let newer: Vec<_> = state
                .rejected
                .iter()
                .filter(|(_, r)| {
                    [
                        fogata_state::ROTATE_NOT_NEEDED,
                        fogata_state::WELCOME_NOT_MANAGER,
                    ]
                    .contains(r)
                })
                .collect();
            println!(
                "{}: {} entries, {} epochs, {} ignored by newer rules",
                path.display(),
                log.len(),
                state.epochs.len(),
                newer.len()
            );
            // The entries that appointed and demoted admins read as moves on the ladder.
            let moves = log
                .entries()
                .filter(|e| matches!(e.body, Body::Promote { .. } | Body::Demote { .. }))
                .count();
            let mut refused: BTreeMap<&str, usize> = BTreeMap::new();
            for (_, reason) in &state.rejected {
                *refused.entry(reason).or_default() += 1;
            }
            println!(
                "  {} members on the ladder, managers {:?}, {moves} moves, refused {refused:?}",
                state.ladder.len(),
                state.managers
            );
            differ += newer.len();
        }
        println!("{} logs read", found.len());
        assert_eq!(differ, 0);
    }

    #[test]
    fn entries_this_version_cannot_read_are_kept() {
        let dir = std::env::temp_dir().join(format!("fogata-store-unread-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (identity, _) = store.identity().unwrap();
        fs::write(dir.join("groups.json"), br#"[{"old": "secret"}]"#).unwrap();
        let log = GroupLog::create(&identity, "Home", 1).unwrap();
        store.save_log("Home", &log).unwrap();
        assert_eq!(store.groups().unwrap().len(), 1);
        let text = fs::read_to_string(dir.join("groups.json")).unwrap();
        assert!(text.contains("\"old\""), "{text}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn room_rules_an_earlier_version_kept_apart_move_into_the_log() {
        let dir = std::env::temp_dir().join(format!("fogata-store-rules-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = Store::open(dir.clone()).unwrap();
        let (identity, _) = store.identity().unwrap();
        let leo = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let mut log = GroupLog::create(&identity, "Home", 1).unwrap();
        log.add(&identity, leo, "Leo", 2).unwrap();
        // As an earlier version kept it: the log in its file, the rule, which nothing is
        // signed on, in a file of its own.
        store.save_log("Home", &log).unwrap();
        let rule = Entry::sign(
            &identity,
            log.group_id(),
            log.heads(),
            3,
            Body::KeepOut {
                room: fogata_state::FIRST_ROOM,
                member: leo,
                until_ms: None,
            },
        );
        let rules = store.rules_path(&log.group_id());
        fs::write(&rules, encode_entries(vec![&rule])).unwrap();
        let kept_out = |log: &GroupLog| {
            log.state()
                .barrier(&fogata_state::FIRST_ROOM, &leo, 10)
                .is_some()
        };
        let read = store.load_log(&store.group("Home").unwrap()).unwrap();
        assert!(kept_out(&read));
        // Written again, the log holds the rule, and its old file goes.
        store.save_group_log(&read, "Home").unwrap();
        assert!(!rules.exists());
        let (entries, damaged) =
            decode_entries(&fs::read(store.log_path(&log.group_id())).unwrap());
        assert!(!damaged);
        assert!(entries.iter().any(|e| e.hash() == rule.hash()));
        assert!(kept_out(
            &store.load_log(&store.group("Home").unwrap()).unwrap()
        ));
        fs::remove_dir_all(&dir).unwrap();
    }
}
