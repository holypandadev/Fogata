//! A copy of the account that is restored without the system keychain: for when the keychain
//! that keeps the data folder's key is lost, or the disk dies.
//!
//! Turning it on ([`NewKey`], then [`turn_on`] once the person wrote it down) makes an X25519
//! pair: the secret half is the recovery key, shown to the person once and never written
//! anywhere (fogata-crypto's `recovery`). The folder keeps the public half and a key derived
//! from the secret one that proves a copy was made by a computer with the account
//! (`recovery.bin`, sealed with the folder's data key as the identity is, and sealed again by
//! a move). A copy ([`export`]) is then made without the recovery key, at any time: it holds
//! what a move moves (the same list and the same exclusions, attachments only when asked
//! for), each file in the form a folder without a key keeps it, encrypted for the recovery
//! key. Only restoring ([`restore`]) asks for the key.
//!
//! A copy is the header in the clear and frames, each its length (4 bytes, big-endian) and
//! the frame sealed (see fogata-crypto's `recovery`). A frame carries a kind byte, then a
//! message in JSON ([`Part`]) or a piece of a file: first the proof that the copy was made
//! with the account (a small frame, checked before anything large is read), then the list of
//! every file with its size and BLAKE3 ([`Manifest`]), then each file in that order as its
//! path, its pieces of 256 KiB and its end, and last an end that counts the files. Nothing may
//! follow it.
//!
//! Restoring goes the way an account that moved here goes in (`transfer`): the copy is
//! written into `incoming/`, sealed with a new key of this computer's keychain, checked (the
//! list, every BLAKE3, the identity opens, the profile is its own and signed by it, every
//! group's log reads), and only then put in place of the folder's files under the same
//! record (`incoming.json`), so a computer that stops half way finishes it or throws it away
//! at the next start, never leaving half of each. An account the folder held is replaced
//! only when the person says so, and is then kept whole in `replaced/` with its key, until
//! the person forgets it ([`forget_replaced`]).
//!
//! The account restored is the same identity, as a move's is: only one computer should use
//! it, and the screens say so before a copy is restored.

use std::{
    collections::HashSet,
    fmt, fs,
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use fogata_crypto::{
    DevicePublicKey,
    at_rest::DataKey,
    at_rest::sealed_form,
    recovery::{
        CopyHeader, CopyOpener, CopySealer, HEADER_LEN, PROOF_LEN, RecoveryPublic, copy_proof,
        proves,
    },
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub use fogata_crypto::recovery::{CopyProblem, KeyProblem, RecoveryKey};

use crate::{
    Store, now_ms,
    store::write_private,
    transfer::{
        self, INCOMING_DIR, Incoming, MAX_BYTES, MAX_FILES, MoveKey, REPLACED_DIR, StageState,
        files::{self, Item, Source},
        wire::CHUNK,
    },
    vault::{self, Keychain},
};

/// What the data folder keeps of the recovery pair: the public half and the key that proves
/// copies, sealed with the folder's data key.
pub const KEY_FILE: &str = "recovery.bin";
/// The context `recovery.bin` is sealed under.
pub(crate) const KEY_CONTEXT: &str = "recovery";
/// How the last restore into this folder went: whether the account it replaced is kept in
/// `replaced/`. Never moved nor copied.
pub(crate) const RESTORED_FILE: &str = "restored.json";
/// The ending of a copy's file name.
pub const COPY_ENDING: &str = "fogata-copy";

/// Longest frame of a piece of a file: the piece, its kind and the tag.
const MAX_FRAME: usize = CHUNK + 1024;
/// Longest first frame, the copy's proof: read before the copy is known to be the account's.
const MAX_START: usize = 1024;
/// Longest frame of the list of files: far above what 500,000 files need. Read only once the
/// copy proved it was made with the account.
const MAX_MANIFEST: usize = 256 * 1024 * 1024;
const KEY_VERSION: u8 = 1;
const MS_A_DAY: u64 = 24 * 60 * 60 * 1000;

/// The first frame of a copy: what it holds.
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    /// The account's device key in hex.
    device: String,
    /// The account's name, as its profile has it.
    name: String,
    /// The version of Fogata that made it.
    build: String,
    made_ms: u64,
    attachments: bool,
    files: Vec<Listed>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Listed {
    path: String,
    len: u64,
    blake3: String,
}

/// A message of a copy, between the pieces of its files.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Part {
    /// [`copy_proof`] of the header, in hex.
    Start {
        proof: String,
    },
    Manifest(Manifest),
    File {
        path: String,
    },
    FileEnd,
    End {
        files: u32,
    },
}

const MESSAGE: u8 = 0;
const DATA: u8 = 1;

/// Why a restore did not start: nothing changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The folder holds an account (its name, when it opens), and the person did not say to
    /// replace it.
    AccountHere(Option<String>),
    /// The account a restore replaced before is still kept in `replaced/`.
    ReplacedKept,
    /// The disk has not the room the copy takes once restored.
    NoRoom { need: u64, free: u64 },
    /// The copy is in the data folder it would be restored into, which a restore empties.
    CopyInFolder,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::AccountHere(Some(name)) => write!(
                f,
                "this folder holds the account of {name}; restoring the copy replaces it"
            ),
            Refused::AccountHere(None) => f.write_str(
                "this folder holds an account (it does not open here); restoring the copy \
                 replaces it",
            ),
            Refused::ReplacedKept => f.write_str(
                "the account a restore replaced is still kept in this folder: forget it first",
            ),
            Refused::NoRoom { need, free } => write!(
                f,
                "the copy needs {} MB on this disk once restored, and {} MB are free",
                need.div_ceil(1 << 20),
                free / (1 << 20)
            ),
            Refused::CopyInFolder => f.write_str(
                "the copy is inside the data folder it would be restored into: move it out \
                 first",
            ),
        }
    }
}

impl std::error::Error for Refused {}

/// Whether recovery is on for the account of `dir`.
pub fn is_on(dir: &Path) -> bool {
    dir.join(KEY_FILE).is_file()
}

/// What the folder keeps of the recovery pair.
struct FolderKey {
    public: RecoveryPublic,
    auth: Zeroizing<[u8; 32]>,
}

/// The recovery pair the folder of `store` keeps, if any. In a sealed folder it must be
/// sealed with the folder's key: one written there by anything without that key (another
/// public half, so that copies go to someone else) is refused.
fn folder_key(store: &Store) -> Result<Option<FolderKey>> {
    let path = store.dir().join(KEY_FILE);
    let read = || match fs::read(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        bytes => bytes
            .map(|b| Some(Zeroizing::new(b)))
            .with_context(|| format!("cannot read {}", path.display())),
    };
    if read()?.is_none() {
        return Ok(None);
    }
    // Read again once the key was looked for: the start that looks for it seals the file,
    // or refuses it and moves it away.
    let key = store.data_key()?;
    let Some(bytes) = read()? else {
        return Ok(None);
    };
    let refused = || {
        anyhow::anyhow!(
            "the recovery key this folder keeps does not check out: turn copies on again"
        )
    };
    if key.is_some() && sealed_form(&bytes).is_none() {
        return Err(refused());
    }
    let plain =
        crate::store::from_disk(key.as_ref(), KEY_CONTEXT, &bytes).map_err(|_| refused())?;
    match plain.split_first() {
        Some((&KEY_VERSION, rest)) if rest.len() == 64 => Ok(Some(FolderKey {
            public: RecoveryPublic(rest[..32].try_into().expect("32 bytes")),
            auth: Zeroizing::new(rest[32..].try_into().expect("32 bytes")),
        })),
        Some((v, _)) if *v > KEY_VERSION => bail!(
            "{} was saved by a newer version of Fogata; update Fogata to use it",
            path.display()
        ),
        _ => Err(refused()),
    }
}

/// A short name of a recovery key, from its public half, for the person to compare with the
/// one shown when it was made: `XXXX XXXX`.
fn key_name(public: &RecoveryPublic) -> String {
    let id = fogata_crypto::crockford_encode(&public.fingerprint()[..5]);
    format!("{} {}", &id[..4], &id[4..])
}

/// The short name of the recovery key copies of `dir` are made for, if copies are on.
pub fn key_id(dir: &Path) -> Result<Option<String>> {
    let store = Store::open(dir.to_path_buf())?;
    Ok(folder_key(&store)?.map(|k| key_name(&k.public)))
}

/// A new recovery key, not in use until [`turn_on`] takes it: the person writes it down and
/// shows they have it first. Wiped from memory once dropped.
#[derive(Debug)]
pub struct NewKey(RecoveryKey);

impl NewKey {
    pub fn generate() -> Result<Self> {
        Ok(Self(RecoveryKey::generate()?))
    }

    /// The key as the person writes it down.
    pub fn text(&self) -> Zeroizing<String> {
        self.0.text()
    }

    /// Its short name ([`key_id`]).
    pub fn id(&self) -> String {
        key_name(&self.0.public())
    }
}

/// Turns recovery on for the account of `dir`, or changes its key, with `key`, which the
/// person wrote down: copies are made for it from now on. Copies made before keep opening
/// with the key they were made for.
pub fn turn_on(dir: &Path, key: &NewKey) -> Result<()> {
    turn_on_with(dir, key, &vault::chosen())
}

pub(crate) fn turn_on_with(dir: &Path, key: &NewKey, keychain: &Keychain) -> Result<()> {
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    transfer::refuse_if_moved(dir)?;
    if !store.has_profile()? {
        bail!("there is no account here to keep a copy of");
    }
    let mut plain = Zeroizing::new(Vec::with_capacity(65));
    plain.push(KEY_VERSION);
    plain.extend_from_slice(&key.0.public().0);
    plain.extend_from_slice(&key.0.auth_key()[..]);
    let data_key = store.data_key()?;
    write_private(
        &dir.join(KEY_FILE),
        &crate::store::to_disk(data_key.as_ref(), KEY_CONTEXT, &plain)?,
    )
}

/// Whether `text` reads as a recovery key, before anything is opened with it.
pub fn check_key(text: &str) -> Result<(), KeyProblem> {
    RecoveryKey::parse(text).map(|_| ())
}

/// The file name a copy made at `at_ms` is offered under: `Fogata-YYYY-MM-DD.fogata-copy`.
pub fn copy_name(at_ms: u64) -> String {
    format!("Fogata-{}.{COPY_ENDING}", date((at_ms / MS_A_DAY) as u32))
}

/// The day `day` (counted from 1 January 1970) as `YYYY-MM-DD`.
fn date(day: u32) -> String {
    // From the days to the civil date, with years that start in March so that the leap day
    // is the last of its year.
    let z = i64::from(day) + 719_468;
    let era = z.div_euclid(146_097);
    let of_era = z - era * 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let m = (5 * of_year + 2) / 153;
    let d = of_year - (153 * m + 2) / 5 + 1;
    let month = if m < 10 { m + 3 } else { m - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{d:02}")
}

/// What a copy says before its key is typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyInfo {
    /// The day it was made, `YYYY-MM-DD`.
    pub made_on: String,
    /// Whether it was made for the recovery key `dir` holds now; `None` when recovery is not
    /// on there.
    pub this_key: Option<bool>,
}

/// Reads the header of the copy at `copy`, for the screens to say what it is before the key
/// is asked for.
pub fn inspect(copy: &Path, dir: &Path) -> Result<CopyInfo> {
    inspect_with(copy, dir, &vault::chosen())
}

pub(crate) fn inspect_with(copy: &Path, dir: &Path, keychain: &Keychain) -> Result<CopyInfo> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    fs::File::open(copy)
        .with_context(|| format!("cannot read {}", copy.display()))?
        .take(HEADER_LEN as u64)
        .read_to_end(&mut header)?;
    let header = CopyHeader::decode(&header)?;
    let this_key = Store::open_with(dir.to_path_buf(), keychain.clone())
        .ok()
        .and_then(|store| folder_key(&store).ok().flatten())
        .map(|key| key.public.fingerprint() == header.fingerprint);
    Ok(CopyInfo {
        made_on: date(header.day),
        this_key,
    })
}

/// Whether the file at `rel` is an attachment of a chat, which a copy leaves out unless asked.
fn is_attachment(rel: &str) -> bool {
    rel.starts_with("direct/") && rel.contains("/files/")
}

/// The bytes the attachments of `dir` add to a copy, for the person to choose before it is
/// made.
pub fn attachments_bytes(dir: &Path) -> Result<u64> {
    Ok(files::list(dir)?
        .iter()
        .filter(|i| is_attachment(&i.rel))
        .map(|i| i.len)
        .sum())
}

/// A copy made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    pub files: u32,
    /// The size of the copy's file.
    pub bytes: u64,
}

/// Makes a copy of the account of `dir` at `to`, with the attachments of its chats when
/// `attachments`. It is written whole beside `to` first and takes its name only at the end,
/// so a copy cut short never looks like one. No core may run on the folder, as for a move:
/// nothing changes while it is read.
pub fn export(dir: &Path, to: &Path, attachments: bool) -> Result<Exported> {
    export_with(dir, to, attachments, &vault::chosen())
}

/// Makes a copy of the account of `dir` at `to` as [`export`] does, but writes it straight
/// into `to`, with no file of its own beside it: for an application whose system lets it
/// write only the file the person chose (the macOS sandbox, after a save dialog). A link at
/// `to` is never written through, and a copy that fails half way is removed. One cut short
/// all the same (the computer stopped) still never passes for a copy: it lacks the end that
/// closes every copy, so restoring it is refused as damaged ([`CopyProblem::Damaged`]).
pub fn export_into(dir: &Path, to: &Path, attachments: bool) -> Result<Exported> {
    export_into_with(dir, to, attachments, &vault::chosen())
}

pub(crate) fn export_with(
    dir: &Path,
    to: &Path,
    attachments: bool,
    keychain: &Keychain,
) -> Result<Exported> {
    export_in(dir, to, attachments, keychain, Writing::Beside)
}

pub(crate) fn export_into_with(
    dir: &Path,
    to: &Path,
    attachments: bool,
    keychain: &Keychain,
) -> Result<Exported> {
    export_in(dir, to, attachments, keychain, Writing::InPlace)
}

/// Where a copy is written while it is made.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Writing {
    /// Beside its name, under a name of its own, and renamed when whole.
    Beside,
    /// Straight into its name.
    InPlace,
}

fn export_in(
    dir: &Path,
    to: &Path,
    attachments: bool,
    keychain: &Keychain,
    writing: Writing,
) -> Result<Exported> {
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    let _running = store.hold_running()?;
    transfer::recover_held(dir, keychain)?;
    transfer::refuse_if_moved(dir)?;
    transfer::refuse_if_staged(dir)?;
    if !store.has_profile()? {
        bail!("there is no account here to keep a copy of");
    }
    let recovery = folder_key(&store)?.context("recovery is not turned on for this account")?;
    // A copy in the data folder would go into the next copy, and to another computer with a
    // move.
    let parent = match to.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    if fs::canonicalize(&parent)
        .ok()
        .zip(fs::canonicalize(dir).ok())
        .is_some_and(|(parent, dir)| parent.starts_with(dir))
    {
        bail!("a copy cannot be kept in the data folder itself");
    }
    let key = store.data_key()?;
    let device = hex::encode(store.identity()?.0.public_key().as_bytes());
    let name = store.require_friends()?.profile.name;
    let items: Vec<Item> = files::leave_out_unreadable(dir, files::list(dir)?, key.as_ref())
        .into_iter()
        .filter(|i| attachments || !is_attachment(&i.rel))
        .collect();
    // The list goes first, so every file is read once for its size and digest, and again as
    // it is written.
    let mut listed = Vec::with_capacity(items.len());
    for item in &items {
        let (len, digest) = read_item(dir, item, key.as_ref(), |_| Ok(()))?;
        listed.push(Listed {
            path: item.rel.clone(),
            len,
            blake3: digest,
        });
    }
    let manifest = Manifest {
        device,
        name,
        build: env!("CARGO_PKG_VERSION").to_string(),
        made_ms: now_ms(),
        attachments,
        files: listed,
    };
    let bytes = match writing {
        Writing::Beside => {
            write_beside(dir, to, &parent, &items, manifest, &recovery, key.as_ref())?
        }
        Writing::InPlace => {
            write_in_place(dir, to, &parent, &items, manifest, &recovery, key.as_ref())?
        }
    };
    tracing::info!("made a copy of the account ({} files)", items.len());
    Ok(Exported {
        files: items.len() as u32,
        bytes,
    })
}

/// Writes the copy beside `to`, under a new name of its own, and gives it the name `to` only
/// when whole; its size.
fn write_beside(
    dir: &Path,
    to: &Path,
    parent: &Path,
    items: &[Item],
    manifest: Manifest,
    recovery: &FolderKey,
    key: Option<&DataKey>,
) -> Result<u64> {
    // A name of its own, made new: never a file or a link someone else left beside `to`
    // (in a shared folder, say), which would be written through, or removed on a failure.
    let mut tmp_name = to
        .file_name()
        .context("a copy needs a file name")?
        .to_owned();
    let mut id = [0u8; 8];
    fogata_crypto::random_bytes(&mut id)?;
    tmp_name.push(format!(".{}.tmp", hex::encode(id)));
    let tmp = to.with_file_name(tmp_name);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&tmp)
        .with_context(|| format!("cannot write {}", tmp.display()))?;
    let written = write_copy(file, dir, items, manifest, recovery, key).and_then(|bytes| {
        fs::rename(&tmp, to).with_context(|| format!("cannot write {}", to.display()))?;
        sync_folder(parent);
        Ok(bytes)
    });
    written.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        if disk_full(&e) {
            e.context(format!(
                "no room on the disk for the copy at {}: nothing was written there, and the \
                 account is as it was",
                to.display()
            ))
        } else {
            e
        }
    })
}

/// Writes the copy straight into `to`, made or emptied, never through a link there; removes
/// it if the copy fails. Its size.
fn write_in_place(
    dir: &Path,
    to: &Path,
    parent: &Path,
    items: &[Item],
    manifest: Manifest,
    recovery: &FolderKey,
    key: Option<&DataKey>,
) -> Result<u64> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    // Where the system cannot refuse a link as it opens, it is looked for first.
    #[cfg(not(unix))]
    if fs::symlink_metadata(to).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("cannot write {}: it is a link", to.display());
    }
    // A link there is refused as it opens, and then nothing of ours is there to remove.
    let file = options
        .open(to)
        .with_context(|| format!("cannot write {}", to.display()))?;
    #[cfg(unix)]
    {
        // A file kept by someone else before (in a shared folder) keeps its mode as it opens:
        // the copy is for this user alone.
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    let written = write_copy(file, dir, items, manifest, recovery, key).inspect(|_| {
        sync_folder(parent);
    });
    written.map_err(|e| {
        let _ = fs::remove_file(to);
        if disk_full(&e) {
            e.context(format!(
                "no room on the disk for the copy at {}: it was not made, and the account is \
                 as it was",
                to.display()
            ))
        } else {
            e
        }
    })
}

/// Whether `e` comes of a disk with no room left.
fn disk_full(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|e| e.downcast_ref::<io::Error>())
        .any(|e| e.kind() == io::ErrorKind::StorageFull)
}

/// Flushes `dir` to the disk, so that a rename in it lasts through a power cut.
fn sync_folder(dir: &Path) {
    #[cfg(unix)]
    if let Err(e) = fs::File::open(dir).and_then(|d| d.sync_all()) {
        tracing::debug!("cannot flush {}: {e}", dir.display());
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Reads the file `item` of `dir` as plain, handing each piece of at most [`CHUNK`] bytes to
/// `piece`; its size and BLAKE3.
fn read_item(
    dir: &Path,
    item: &Item,
    key: Option<&DataKey>,
    mut piece: impl FnMut(&[u8]) -> Result<()>,
) -> Result<(u64, String)> {
    let mut hash = blake3::Hasher::new();
    let mut len = 0u64;
    match files::open(dir, item, key)? {
        Source::Whole(plain) => {
            for chunk in plain.chunks(CHUNK) {
                hash.update(chunk);
                len += chunk.len() as u64;
                piece(chunk)?;
            }
        }
        Source::Reader(mut reader) => {
            let mut buf = Zeroizing::new(vec![0u8; CHUNK]);
            loop {
                let mut got = 0;
                while got < CHUNK {
                    match reader.read(&mut buf[got..])? {
                        0 => break,
                        n => got += n,
                    }
                }
                if got == 0 {
                    break;
                }
                hash.update(&buf[..got]);
                len += got as u64;
                piece(&buf[..got])?;
                if got < CHUNK {
                    break;
                }
            }
        }
    }
    Ok((len, hash.finalize().to_hex().to_string()))
}

/// Writes the whole copy to `file`; its size.
fn write_copy(
    file: fs::File,
    dir: &Path,
    items: &[Item],
    manifest: Manifest,
    recovery: &FolderKey,
    key: Option<&DataKey>,
) -> Result<u64> {
    let mut out = Writer {
        io: BufWriter::new(file),
        sealer: CopySealer::new(&recovery.public, (now_ms() / MS_A_DAY) as u32)?,
        written: HEADER_LEN as u64,
    };
    out.io.write_all(out.sealer.header())?;
    let proof = copy_proof(&recovery.auth, out.sealer.header());
    out.message(&Part::Start {
        proof: hex::encode(proof),
    })?;
    let first = Part::Manifest(manifest);
    out.message(&first)?;
    let Part::Manifest(manifest) = first else {
        unreachable!("the list")
    };
    for (item, listed) in items.iter().zip(&manifest.files) {
        out.message(&Part::File {
            path: item.rel.clone(),
        })?;
        let mut piece = Zeroizing::new(Vec::with_capacity(CHUNK + 1));
        let (len, digest) = read_item(dir, item, key, |data| {
            piece.clear();
            piece.push(DATA);
            piece.extend_from_slice(data);
            out.frame(&piece)
        })?;
        if len != listed.len || digest != listed.blake3 {
            bail!("{} changed while the copy was made", item.rel);
        }
        out.message(&Part::FileEnd)?;
    }
    out.message(&Part::End {
        files: items.len() as u32,
    })?;
    let file = out.io.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;
    Ok(out.written)
}

struct Writer {
    io: BufWriter<fs::File>,
    sealer: CopySealer,
    written: u64,
}

impl Writer {
    fn frame(&mut self, plain: &[u8]) -> Result<()> {
        #[cfg(test)]
        crate::store::faults::check()?;
        let sealed = self.sealer.seal(plain)?;
        self.io.write_all(&(sealed.len() as u32).to_be_bytes())?;
        self.io.write_all(&sealed)?;
        self.written += 4 + sealed.len() as u64;
        Ok(())
    }

    fn message(&mut self, part: &Part) -> Result<()> {
        let mut plain = Zeroizing::new(vec![MESSAGE]);
        serde_json::to_writer(&mut *plain, part)?;
        self.frame(&plain)
    }
}

/// What a copy holds, read with its key before it is restored: for the person to see whose
/// account it is before it replaces anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopySummary {
    /// The day it was made, `YYYY-MM-DD`.
    pub made_on: String,
    /// The account's name.
    pub name: String,
    /// The account's device key in hex, and its fingerprint as people compare it.
    pub device: String,
    pub fingerprint: String,
    pub files: u32,
    pub bytes: u64,
    pub attachments: bool,
}

/// Opens the copy at `copy` with `key` and says what it holds; nothing is written.
pub fn open_copy(copy: &Path, key: &RecoveryKey) -> Result<CopySummary> {
    let mut reader = CopyReader::open(copy, key)?;
    let manifest = reader.manifest()?;
    let day = CopyHeader::decode(reader.opener.header())?.day;
    let device = DevicePublicKey::from_bytes(&hex::decode(&manifest.device)?)?;
    Ok(CopySummary {
        made_on: date(day),
        name: manifest.name.clone(),
        fingerprint: device.fingerprint_grouped(),
        device: manifest.device.clone(),
        files: manifest.files.len() as u32,
        bytes: manifest.files.iter().map(|f| f.len).sum(),
        attachments: manifest.attachments,
    })
}

/// The account a data folder holds, as far as it reads, for the person to see what a restore
/// would replace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountHere {
    /// Its name, when its profile reads.
    pub name: Option<String>,
    /// Its device's fingerprint, when its identity reads.
    pub fingerprint: Option<String>,
}

/// The account `dir` holds, if anything of one is there (see [`restore`]).
pub fn account_in(dir: &Path) -> Result<Option<AccountHere>> {
    account_in_with(dir, &vault::chosen())
}

pub(crate) fn account_in_with(dir: &Path, keychain: &Keychain) -> Result<Option<AccountHere>> {
    if !dir.exists() || !holds_account(dir)? {
        return Ok(None);
    }
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    let fingerprint = if dir.join("identity.key").exists() {
        store
            .identity()
            .ok()
            .map(|(id, _)| id.public_key().fingerprint_grouped())
    } else {
        None
    };
    Ok(Some(AccountHere {
        name: store.friends().ok().flatten().map(|b| b.profile.name),
        fingerprint,
    }))
}

/// A copy restored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    /// The account's name.
    pub name: String,
    /// The account's device key in hex.
    pub device: String,
    pub files: u32,
    /// Whether an account the folder held was replaced, and is kept in `replaced/`.
    pub replaced: bool,
}

/// How a copy is restored.
#[derive(Clone, Debug, Default)]
pub struct RestoreOptions {
    /// Replace the account the folder holds; it is kept whole in `replaced/`.
    pub replace: bool,
    /// The keychain the restored account is sealed with; `None` for the one this process uses
    /// ([`vault::use_keychain`]). One that may seal makes a new key for it, as a move into
    /// this computer does.
    pub keychain: Option<Keychain>,
    /// The free space on the disk of a folder, as the system tells it (see
    /// `TransferOptions::free_space`): a copy there is no room for is refused before
    /// anything is written.
    pub free_space: Option<fn(&Path) -> Option<u64>>,
    /// Write the account restored unencrypted when the keychain cannot keep a key for it,
    /// as the person chose ([`vault::allow_plain`] for a folder, but only for this restore:
    /// nothing says so in the folder if it does not happen).
    pub unencrypted: bool,
}

/// Restores the copy at `copy`, opened with `key`, into the data folder `dir`. A folder that
/// holds anything of an account is refused ([`Refused::AccountHere`]) unless
/// `options.replace`; that account is then kept whole in `replaced/`. Any failure before the
/// copy is checked whole leaves the folder as it was; once it is being put in place, it is
/// finished, now or at the next start. No core may run on the folder.
pub fn restore(
    dir: &Path,
    copy: &Path,
    key: &RecoveryKey,
    options: &RestoreOptions,
) -> Result<Restored> {
    let keychain = options.keychain.clone().unwrap_or_else(vault::chosen);
    restore_in(dir, copy, key, &keychain, options)
}

#[cfg(test)]
pub(crate) fn restore_with(
    dir: &Path,
    copy: &Path,
    key: &RecoveryKey,
    replace: bool,
    keychain: &Keychain,
) -> Result<Restored> {
    let options = RestoreOptions {
        replace,
        ..Default::default()
    };
    restore_in(dir, copy, key, keychain, &options)
}

fn restore_in(
    dir: &Path,
    copy: &Path,
    key: &RecoveryKey,
    keychain: &Keychain,
    options: &RestoreOptions,
) -> Result<Restored> {
    let (replace, free_space) = (options.replace, options.free_space);
    // The folder is emptied into `replaced/`, which goes when nothing of an account was
    // there: a copy kept inside it would go too.
    if fs::canonicalize(copy)
        .ok()
        .zip(fs::canonicalize(dir).ok())
        .is_some_and(|(copy, dir)| copy.starts_with(dir))
    {
        bail!(Refused::CopyInFolder);
    }
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    let _running = store.hold_running()?;
    transfer::recover_held(dir, keychain)?;
    transfer::refuse_if_moved(dir)?;
    transfer::refuse_if_staged(dir)?;
    refuse_if_kept(dir)?;
    if dir.join(REPLACED_DIR).exists() {
        bail!(
            "{} is still there from an earlier move",
            dir.join(REPLACED_DIR).display()
        );
    }
    // A device key alone, which nobody knows yet, is replaced without asking and goes with
    // the folder's own key rather than being kept aside.
    let replaces = holds_account(dir)?;
    if replaces && !replace {
        let name = store.friends().ok().flatten().map(|b| b.profile.name);
        bail!(Refused::AccountHere(name));
    }
    let own_port = store.meta().port;
    // The copy opens with the key and its list reads, before anything here changes.
    let mut reader = CopyReader::open(copy, key)?;
    let manifest = reader.manifest()?;
    let total = manifest.files.iter().map(|f| f.len).sum::<u64>();
    if let Some(free) = free_space.and_then(|free_space| free_space(dir))
        && free < transfer::room_needed(total)
    {
        bail!(Refused::NoRoom {
            need: transfer::room_needed(total),
            free
        });
    }
    transfer::discard(dir, keychain)?;
    let stage = dir.join(INCOMING_DIR);
    fs::create_dir(&stage).with_context(|| format!("cannot create {}", stage.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&stage, fs::Permissions::from_mode(0o700));
    }
    let mut id = [0u8; 16];
    fogata_crypto::random_bytes(&mut id)?;
    let mut record = Incoming {
        state: StageState::Receiving,
        transfer: hex::encode(id),
        from: String::new(),
        name: String::new(),
        device: manifest.device.clone(),
        key: MoveKey::default(),
        restore: Some(replaces),
    };
    let taken = transfer::write_stage(dir, &record)
        .and_then(|()| {
            take(
                dir,
                &stage,
                keychain,
                &mut reader,
                &manifest,
                own_port,
                options.unencrypted,
            )
        })
        .and_then(|()| transfer::check_stage(&stage, keychain, &manifest.device))
        .and_then(|profile| {
            record.name = profile.name;
            record.state = StageState::Commit;
            transfer::write_stage(dir, &record)
        });
    if let Err(e) = taken {
        if let Err(e) = transfer::discard(dir, keychain) {
            tracing::warn!("what was restored goes at the next start: {e:#}");
        }
        if disk_full(&e) {
            return Err(e.context(format!(
                "no room on the disk to restore the copy in {}: nothing changed there",
                dir.display()
            )));
        }
        return Err(e);
    }
    // From here on the copy is restored, now or at the next start.
    transfer::put_in_place(dir, &record, keychain)
        .context("the copy is put in place when Fogata starts next")?;
    if let Err(e) = transfer::remove_replaced(dir, keychain) {
        tracing::warn!("what the restore replaced goes at the next start: {e:#}");
    }
    tracing::info!("restored a copy of the account");
    Ok(Restored {
        name: record.name,
        device: manifest.device,
        files: manifest.files.len() as u32,
        replaced: replaces,
    })
}

/// Whether `dir` holds anything of an account worth keeping: a profile even damaged, groups,
/// MLS states, chats, the plain copies sealing left, or an identity with any of them:
/// anything a move would take but the settings a new folder starts with. A profile that does
/// not read is still an account: its identity, groups and chats are there. A data key alone
/// (`sealed.json`, and how far sealing got) is not: the application makes one for a new
/// folder the first time it opens it, before any account, and a folder whose keychain was
/// lost still holds the identity sealed. Nor is an identity alone ([`account_files`]).
fn holds_account(dir: &Path) -> Result<bool> {
    Ok(account_files(dir)? == Here::Account)
}

/// What a data folder holds of an account.
#[derive(Debug, PartialEq, Eq)]
enum Here {
    Nothing,
    /// Only a device key, which nobody knows yet.
    FreshIdentity,
    Account,
}

/// What `dir` holds of an account. A device key with nothing else of an account (no profile,
/// friends, groups, chats nor a copy sealing left) is the one the welcome page makes as it
/// opens, to show its fingerprint before a profile is made: no friend nor group knows it, so
/// a restore replaces it without asking, and deletes it rather than keep it aside.
fn account_files(dir: &Path) -> Result<Here> {
    if fs::symlink_metadata(dir.join(crate::store::BACKUP_DIR)).is_ok() {
        return Ok(Here::Account);
    }
    let rels: Vec<String> = files::list(dir)?
        .into_iter()
        .map(|i| i.rel)
        .filter(|rel| !matches!(rel.as_str(), "meta.json" | "app.json" | vault::PLAIN_FILE))
        .collect();
    Ok(match rels.as_slice() {
        [] => Here::Nothing,
        [only] if only == "identity.key" => Here::FreshIdentity,
        _ => Here::Account,
    })
}

/// A copy being read: its frames, opened in order.
struct CopyReader {
    io: BufReader<fs::File>,
    opener: CopyOpener,
}

/// A copy that does not read as it was written.
fn damaged(why: &str) -> anyhow::Error {
    anyhow::Error::new(CopyProblem::Damaged).context(why.to_string())
}

impl CopyReader {
    fn open(copy: &Path, key: &RecoveryKey) -> Result<Self> {
        let file =
            fs::File::open(copy).with_context(|| format!("cannot read {}", copy.display()))?;
        let mut io = BufReader::new(file);
        let mut header = Vec::with_capacity(HEADER_LEN);
        (&mut io).take(HEADER_LEN as u64).read_to_end(&mut header)?;
        let opener = CopyOpener::new(&header, key)?;
        let mut reader = Self { io, opener };
        // Before anything large is read: the copy was made with the account.
        let proof = match reader.part(MAX_START)? {
            Part::Start { proof } => hex::decode(proof).unwrap_or_default(),
            _ => return Err(damaged("the copy does not start with its proof")),
        };
        if proof.len() != PROOF_LEN || !proves(&key.auth_key(), reader.opener.header(), &proof) {
            bail!(CopyProblem::Unproven);
        }
        Ok(reader)
    }

    /// The next frame, opened; `None` at the end of the file, between frames.
    fn next(&mut self, max: usize) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let mut len = [0u8; 4];
        let mut got = 0;
        while got < len.len() {
            match self.io.read(&mut len[got..])? {
                0 if got == 0 => return Ok(None),
                0 => return Err(damaged("the copy is cut short")),
                n => got += n,
            }
        }
        let len = u32::from_be_bytes(len) as usize;
        if len > max {
            return Err(damaged("a frame of the copy is too long"));
        }
        let mut sealed = vec![0u8; len];
        self.io
            .read_exact(&mut sealed)
            .map_err(|_| damaged("the copy is cut short"))?;
        Ok(Some(self.opener.open(&sealed)?))
    }

    /// The next frame, which must be there.
    fn frame(&mut self, max: usize) -> Result<Zeroizing<Vec<u8>>> {
        self.next(max)?
            .ok_or_else(|| damaged("the copy is cut short"))
    }

    fn part(&mut self, max: usize) -> Result<Part> {
        let frame = self.frame(max)?;
        match frame.split_first() {
            Some((&MESSAGE, json)) => {
                serde_json::from_slice(json).map_err(|_| damaged("a malformed part of the copy"))
            }
            _ => Err(damaged("a piece of a file out of place")),
        }
    }

    /// The list that starts the copy, checked: names a folder takes, each once, and no more
    /// files nor bytes than a move takes.
    fn manifest(&mut self) -> Result<Manifest> {
        let Part::Manifest(manifest) = self.part(MAX_MANIFEST)? else {
            return Err(damaged("the copy does not start with its list"));
        };
        check_manifest(&manifest)?;
        Ok(manifest)
    }
}

/// A copy's list as a restore takes it: names a folder takes, each once, and no more files
/// nor bytes than a move takes, counted without overflowing.
fn check_manifest(manifest: &Manifest) -> Result<()> {
    let total = manifest
        .files
        .iter()
        .try_fold(0u64, |sum, f| sum.checked_add(f.len));
    if manifest.files.len() > MAX_FILES as usize || total.is_none_or(|t| t > MAX_BYTES) {
        return Err(damaged("the copy lists more than an account holds"));
    }
    if manifest.device.len() != 64 || hex::decode(&manifest.device).is_err() {
        return Err(damaged("the copy names no account"));
    }
    let mut seen = HashSet::new();
    for listed in &manifest.files {
        files::check_path(&listed.path)?;
        if !seen.insert(listed.path.to_lowercase())
            || blake3::Hash::from_hex(&listed.blake3).is_err()
        {
            return Err(damaged("the copy's list does not read"));
        }
    }
    Ok(())
}

/// Writes every file of the copy into `stage`, sealed with a new key of this computer's
/// keychain (or plain without one, where the person chose so), as a move does.
fn take(
    dir: &Path,
    stage: &Path,
    keychain: &Keychain,
    reader: &mut CopyReader,
    manifest: &Manifest,
    own_port: Option<u16>,
    unencrypted: bool,
) -> Result<()> {
    let key = match keychain {
        Keychain::Seal(keys) => match vault::try_create(stage, keys.as_ref())? {
            Ok(key) => Some(key),
            Err(why) if !unencrypted && !vault::may_stay_plain(dir) => {
                bail!(vault::NoKeychain(why))
            }
            Err(_) => None,
        },
        _ => None,
    };
    for listed in &manifest.files {
        match reader.part(MAX_FRAME)? {
            Part::File { path } if path == listed.path => {}
            _ => return Err(damaged("the copy's files are not as listed")),
        }
        let kind = files::check_path(&listed.path)?;
        let mut sink = files::create(stage, &listed.path, kind, key.as_ref())?;
        let mut hash = blake3::Hasher::new();
        let mut len = 0u64;
        loop {
            let frame = reader.frame(MAX_FRAME)?;
            match frame.split_first() {
                Some((&DATA, data)) if !data.is_empty() => {
                    len += data.len() as u64;
                    if len > listed.len {
                        return Err(damaged("a file of the copy is longer than listed"));
                    }
                    hash.update(data);
                    sink.write(data)?;
                }
                Some((&MESSAGE, json))
                    if matches!(serde_json::from_slice(json), Ok(Part::FileEnd)) =>
                {
                    break;
                }
                _ => return Err(damaged("a file of the copy does not end")),
            }
        }
        if len != listed.len || hash.finalize().to_hex().as_str() != listed.blake3 {
            return Err(damaged("a file of the copy is not as listed"));
        }
        sink.finish(key.as_ref(), |rel, bytes| {
            transfer::adjust(rel, bytes, own_port)
        })?;
    }
    match reader.part(MAX_FRAME)? {
        Part::End { files } if files as usize == manifest.files.len() => {}
        _ => return Err(damaged("the copy does not end as it should")),
    }
    if !io::BufRead::fill_buf(&mut reader.io)?.is_empty() {
        return Err(damaged("something follows the end of the copy"));
    }
    Ok(())
}

/// Notes in `dir` that a copy was restored there, keeping (`kept`) the account it replaced
/// in `replaced/`.
pub(crate) fn note_restored(dir: &Path, kept: bool) -> Result<()> {
    write_private(
        &dir.join(RESTORED_FILE),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "at_ms": now_ms(),
            "replaced": kept,
        }))?,
    )
}

/// Whether `replaced/` in `dir` holds the account a restore replaced, kept until the person
/// forgets it.
pub(crate) fn keeps_replaced(dir: &Path) -> bool {
    dir.join(REPLACED_DIR).exists()
        && fs::read(dir.join(RESTORED_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|v| v.get("replaced").and_then(|r| r.as_bool()))
            // A record that does not read keeps what it may have said to keep.
            .unwrap_or(dir.join(RESTORED_FILE).exists())
}

/// Errors while `replaced/` keeps an account a restore replaced: what would replace the
/// account again would have to throw it away.
pub(crate) fn refuse_if_kept(dir: &Path) -> Result<()> {
    if keeps_replaced(dir) {
        bail!(Refused::ReplacedKept);
    }
    Ok(())
}

/// Whether the folder `dir` keeps the account a restore replaced.
pub fn replaced_kept(dir: &Path) -> bool {
    keeps_replaced(dir)
}

/// Deletes the account a restore replaced, kept in `replaced/` of `dir`, with its key in
/// the keychain. Whether there was one.
pub fn forget_replaced(dir: &Path) -> Result<bool> {
    forget_replaced_with(dir, &vault::chosen())
}

pub(crate) fn forget_replaced_with(dir: &Path, keychain: &Keychain) -> Result<bool> {
    let store = Store::open_with(dir.to_path_buf(), keychain.clone())?;
    let _running = store.hold_running()?;
    if !keeps_replaced(dir) {
        return Ok(false);
    }
    // The record goes first: a deletion cut short then goes on at the next start, as what a
    // move replaced does.
    transfer::remove_if_there(&dir.join(RESTORED_FILE))?;
    transfer::remove_replaced(dir, keychain)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use fogata_crypto::{DeviceIdentity, DevicePublicKey};
    use fogata_friends::FriendBook;
    use fogata_state::GroupLog;

    use super::*;
    use crate::{
        direct::{
            model::{Attachment, Body, Chat, FileKind, Item, Local},
            store::{DirectStore, Record},
        },
        store::faults,
        vault::MemoryKeyStore,
    };

    fn keychain() -> (Arc<MemoryKeyStore>, Keychain) {
        let keys = Arc::new(MemoryKeyStore::default());
        (keys.clone(), Keychain::Seal(keys))
    }

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fogata-recovery-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An account with a profile, a group with its MLS state, and a chat with a picture.
    struct Account {
        identity: DeviceIdentity,
        manager: DeviceIdentity,
        log: GroupLog,
        friend: DevicePublicKey,
        picture: Attachment,
    }

    fn account(dir: &Path, keychain: &Keychain, name: &str) -> Account {
        let store = Store::open_with(dir.to_path_buf(), keychain.clone()).unwrap();
        let (identity, _) = store.identity().unwrap();
        store
            .save_friends(&FriendBook::new(&identity, name, 1).unwrap())
            .unwrap();
        let manager = DeviceIdentity::generate().unwrap();
        let mut log = GroupLog::create(&manager, "Home", 1).unwrap();
        log.add(&manager, identity.public_key(), name, 2).unwrap();
        store.save_group_log(&log, "Home").unwrap();
        store
            .save_mls_state(&log.group_id(), format!("{name}'s MLS state").as_bytes())
            .unwrap();
        let chats = DirectStore::open_sealed(dir, store.data_key().unwrap());
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let picture = Attachment {
            id: [5; 32],
            name: "lake.jpg".into(),
            size: 9,
            kind: FileKind::Picture,
            video: None,
            local: Local::Here,
            voice: None,
        };
        let item = Item {
            id: [1; 16],
            mine: true,
            sent_ms: 10,
            got_ms: 10,
            seq: 1,
            at_ms: 10,
            body: Body::Message {
                text: format!("hello, I'm {name}"),
                files: vec![picture.clone()],
            },
            ttl_ms: 0,
            delivered: false,
        };
        let records = vec![Record::Item { item }];
        let mut chat = Chat::default();
        let mut ended = None;
        for r in &records {
            r.apply(&mut chat, &mut ended);
        }
        chats.append(&friend, &records, &chat).unwrap();
        chats.files_dir(&friend).unwrap();
        chats.keep_bytes(&friend, &picture, b"a picture").unwrap();
        Account {
            identity,
            manager,
            log,
            friend,
            picture,
        }
    }

    /// What the account of `dir` holds, as the application reads it.
    #[derive(Debug, PartialEq)]
    struct Seen {
        device: DevicePublicKey,
        name: String,
        groups: Vec<(String, Vec<fogata_state::EntryHash>, Option<Vec<u8>>)>,
        chat: Chat,
        picture: Option<Vec<u8>>,
    }

    fn seen(dir: &Path, keychain: &Keychain, account: &Account) -> Seen {
        let store = Store::open_with(dir.to_path_buf(), keychain.clone()).unwrap();
        let groups = store
            .groups()
            .unwrap()
            .iter()
            .map(|g| {
                let log = store.load_log(g).unwrap();
                let mls = store
                    .mls_state(&log.group_id())
                    .unwrap()
                    .map(|s| s.to_vec());
                (g.name.clone(), log.hashes_in_order(), mls)
            })
            .collect();
        let chats = DirectStore::open_sealed(dir, store.data_key().unwrap());
        let picture = chats
            .open_file(&account.friend, &account.picture)
            .ok()
            .map(|(mut r, _)| {
                let mut bytes = Vec::new();
                r.read_to_end(&mut bytes).unwrap();
                bytes
            });
        Seen {
            device: store.identity().unwrap().0.public_key(),
            name: store.friends().unwrap().unwrap().profile.name,
            groups,
            chat: chats.load(&account.friend).unwrap().chat,
            picture,
        }
    }

    /// Every file under `dir` with what it holds, but the lock a store takes.
    fn files_in(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut left = vec![dir.to_path_buf()];
        while let Some(at) = left.pop() {
            for entry in fs::read_dir(&at).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    left.push(path);
                } else if entry.file_name() != "running.lock" {
                    out.insert(
                        path.strip_prefix(dir).unwrap().to_path_buf(),
                        fs::read(&path).unwrap(),
                    );
                }
            }
        }
        out
    }

    /// Turns copies on for `dir`; the key.
    fn on(dir: &Path, keychain: &Keychain) -> RecoveryKey {
        let new = NewKey::generate().unwrap();
        turn_on_with(dir, &new, keychain).unwrap();
        RecoveryKey::parse(&new.text()).unwrap()
    }

    /// An account with recovery on, its key, and a copy of it with or without attachments.
    fn exported(
        base: &Path,
        keychain: &Keychain,
        attachments: bool,
    ) -> (PathBuf, Account, RecoveryKey, PathBuf) {
        let dir = base.join("ana");
        let ana = account(&dir, keychain, "Ana");
        let key = on(&dir, keychain);
        let copy = base.join(copy_name(now_ms()));
        let made = export_with(&dir, &copy, attachments, keychain).unwrap();
        assert_eq!(made.bytes, fs::metadata(&copy).unwrap().len());
        (dir, ana, key, copy)
    }

    #[test]
    fn a_copy_restores_the_same_account_into_an_empty_folder() {
        let base = folder("round");
        let (keys, keychain) = keychain();
        let (dir, ana, key, copy) = exported(&base, &keychain, false);
        assert!(copy.to_string_lossy().ends_with(".fogata-copy"));
        assert!(!dir.join("incoming").exists());
        assert_eq!(
            inspect_with(&copy, &dir, &keychain).unwrap().this_key,
            Some(true),
            "made for this folder's key"
        );
        let before = seen(&dir, &keychain, &ana);
        let into = base.join("new");
        let restored = restore_with(&into, &copy, &key, false, &keychain).unwrap();
        assert_eq!(restored.name, "Ana");
        assert!(!restored.replaced);
        let after = seen(&into, &keychain, &ana);
        // Everything but the picture, which a copy leaves out unless asked.
        assert_eq!(after.picture, None);
        assert_eq!(
            Seen {
                picture: None,
                ..before
            },
            after
        );
        assert_eq!(after.device, ana.identity.public_key());
        // Sealed with a key of its own, and the copy's records gone.
        let name = vault::key_name(&into).unwrap();
        assert_ne!(Some(name.clone()), vault::key_name(&dir));
        assert!(keys.names().contains(&name));
        for gone in ["incoming", "incoming.json", "replaced", "moved-in.json"] {
            assert!(!into.join(gone).exists(), "{gone}");
        }
        assert!(is_on(&into), "the public key came along");
        // With the attachments, the picture comes too.
        let copy = base.join("with.fogata-copy");
        export_with(&dir, &copy, true, &keychain).unwrap();
        assert!(attachments_bytes(&dir).unwrap() > 0);
        let into = base.join("newer");
        restore_with(&into, &copy, &key, false, &keychain).unwrap();
        assert_eq!(seen(&into, &keychain, &ana), seen(&dir, &keychain, &ana));
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_copy_changed_cut_out_of_order_or_for_another_key_is_refused_and_nothing_changes() {
        let base = folder("refused");
        let (keys, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, true);
        let bytes = fs::read(&copy).unwrap();
        // The frames: where each starts.
        let mut starts = Vec::new();
        let mut at = HEADER_LEN;
        while at < bytes.len() {
            starts.push(at);
            at += 4 + u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        }
        assert_eq!(at, bytes.len());
        assert!(starts.len() > 6);
        let frame =
            |i: usize| bytes[starts[i]..*starts.get(i + 1).unwrap_or(&bytes.len())].to_vec();
        let mut variants: Vec<(&str, Vec<u8>)> = Vec::new();
        for at in [
            0,
            12,
            40,
            HEADER_LEN - 1,
            HEADER_LEN + 2,
            HEADER_LEN + 9,
            bytes.len() / 2,
            bytes.len() - 1,
        ] {
            let mut changed = bytes.clone();
            changed[at] ^= 0x20;
            variants.push(("a byte changed", changed));
        }
        for len in [
            10,
            HEADER_LEN,
            starts[3],
            starts[3] + 7,
            *starts.last().unwrap(),
            bytes.len() - 1,
        ] {
            variants.push(("cut short", bytes[..len].to_vec()));
        }
        let mut swapped = bytes[..starts[2]].to_vec();
        swapped.extend(frame(3));
        swapped.extend(frame(2));
        swapped.extend_from_slice(&bytes[starts[4]..]);
        variants.push(("frames in another order", swapped));
        let mut repeated = bytes[..starts[3]].to_vec();
        repeated.extend(frame(2));
        repeated.extend_from_slice(&bytes[starts[3]..]);
        variants.push(("a frame repeated", repeated));
        let mut dropped = bytes[..starts[2]].to_vec();
        dropped.extend_from_slice(&bytes[starts[3]..]);
        variants.push(("a frame left out", dropped));
        let mut more = bytes.clone();
        more.extend(frame(starts.len() - 1));
        variants.push(("more after the end", more));
        let other = RecoveryKey::generate().unwrap();
        // Into an empty folder and over another account (told to replace it).
        let empty = base.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let bea = base.join("bea");
        account(&bea, &keychain, "Bea");
        for (into, replace) in [(&empty, false), (&bea, true)] {
            let files = files_in(into);
            let names = {
                let mut n = keys.names();
                n.sort();
                n
            };
            let unchanged = |what: &str| {
                assert_eq!(files_in(into), files, "{what}");
                let mut n = keys.names();
                n.sort();
                assert_eq!(n, names, "{what}: a key left in the keychain");
            };
            for (what, variant) in &variants {
                let path = base.join("variant.fogata-copy");
                fs::write(&path, variant).unwrap();
                let e = restore_with(into, &path, &key, replace, &keychain).unwrap_err();
                assert!(e.downcast_ref::<CopyProblem>().is_some(), "{what}: {e:#}");
                unchanged(what);
            }
            let e = restore_with(into, &copy, &other, replace, &keychain).unwrap_err();
            assert_eq!(
                e.downcast_ref::<CopyProblem>(),
                Some(&CopyProblem::OtherKey)
            );
            unchanged("another key");
            // And the copy as it was goes in.
            restore_with(into, &copy, &key, replace, &keychain).unwrap();
        }
        fs::remove_dir_all(&base).unwrap();
    }

    /// The restore with every write made to fail in turn (a full disk, a computer that
    /// stops), then the next start: the folder holds the account it had or the one restored
    /// whole, never some of each, and the one it had is kept whole when replaced.
    #[test]
    fn a_restore_stopped_at_any_write_leaves_one_account_or_the_other() {
        let base = folder("faults");
        let (_, keychain) = keychain();
        let (_, ana, key, copy) = exported(&base, &keychain, true);
        let mut said_no_room = 0;
        for n in 0.. {
            let dir = base.join(format!("bea-{n}"));
            let bea = account(&dir, &keychain, "Bea");
            let had = seen(&dir, &keychain, &bea);
            faults::fail_after(Some(n));
            let result = restore_with(&dir, &copy, &key, true, &keychain);
            let tried = faults::tried();
            faults::fail_after(None);
            // The next start: however the restore ended, it let go of the folder.
            {
                let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
                let _running = store
                    .hold_running()
                    .unwrap_or_else(|e| panic!("{n}: the restore kept the folder: {e:#}"));
                transfer::recover_held(&dir, &keychain).unwrap();
            }
            let device = Store::open_with(dir.clone(), keychain.clone())
                .unwrap()
                .identity()
                .unwrap()
                .0
                .public_key();
            if device == ana.identity.public_key() {
                assert_eq!(seen(&dir, &keychain, &ana).name, "Ana", "{n}");
                assert_eq!(
                    seen(&dir, &keychain, &ana).picture.as_deref(),
                    Some(&b"a picture"[..]),
                    "{n}"
                );
                assert_eq!(seen(&dir.join(REPLACED_DIR), &keychain, &bea), had, "{n}");
                assert!(keeps_replaced(&dir), "{n}");
            } else {
                assert!(result.is_err(), "{n}");
                assert_eq!(seen(&dir, &keychain, &bea), had, "{n}");
                assert!(!dir.join(REPLACED_DIR).exists(), "{n}");
            }
            // A full disk while the copy's files are written says so, and that nothing
            // changed: and nothing did.
            if let Err(e) = &result
                && e.to_string()
                    .starts_with("no room on the disk to restore the copy in")
            {
                assert_ne!(device, ana.identity.public_key(), "{n}");
                said_no_room += 1;
            }
            for gone in ["incoming", "incoming.json"] {
                assert!(!dir.join(gone).exists(), "{n}: {gone}");
            }
            if tried <= n {
                assert!(result.is_ok());
                assert!(n > 10, "only {n} writes");
                break;
            }
        }
        assert!(said_no_room > 0, "a full disk never said so");
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn an_account_is_replaced_only_when_asked_and_then_kept() {
        let base = folder("replace");
        let (keys, keychain) = keychain();
        let (_, ana, key, copy) = exported(&base, &keychain, false);
        let dir = base.join("bea");
        let bea = account(&dir, &keychain, "Bea");
        let had = seen(&dir, &keychain, &bea);
        let bea_key = vault::key_name(&dir).unwrap();
        let e = restore_with(&dir, &copy, &key, false, &keychain).unwrap_err();
        assert_eq!(
            e.downcast_ref::<Refused>(),
            Some(&Refused::AccountHere(Some("Bea".into())))
        );
        assert_eq!(seen(&dir, &keychain, &bea), had);
        let restored = restore_with(&dir, &copy, &key, true, &keychain).unwrap();
        assert!(restored.replaced);
        assert_eq!(
            seen(&dir, &keychain, &ana).device,
            ana.identity.public_key()
        );
        // The account it had is kept whole, with its key, through the next starts.
        for _ in 0..2 {
            transfer::recover(&dir);
            let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
            let _running = store.hold_running().unwrap();
            transfer::recover_held(&dir, &keychain).unwrap();
        }
        assert!(replaced_kept(&dir));
        assert_eq!(seen(&dir.join(REPLACED_DIR), &keychain, &bea), had);
        assert!(keys.names().contains(&bea_key));
        // Neither a restore nor a move replaces the account again while it is kept.
        let e = restore_with(&dir, &copy, &key, true, &keychain).unwrap_err();
        assert_eq!(e.downcast_ref::<Refused>(), Some(&Refused::ReplacedKept));
        assert!(refuse_if_kept(&dir).is_err());
        // Nor do copies or moves take it along.
        assert!(
            files::list(&dir)
                .unwrap()
                .iter()
                .all(|i| !i.rel.starts_with("replaced") && i.rel != RESTORED_FILE)
        );
        // Forgotten, it goes with its key.
        assert!(forget_replaced_with(&dir, &keychain).unwrap());
        assert!(!dir.join(REPLACED_DIR).exists());
        assert!(!keys.names().contains(&bea_key));
        assert!(!forget_replaced_with(&dir, &keychain).unwrap());
        assert_eq!(seen(&dir, &keychain, &ana).name, "Ana");
        fs::remove_dir_all(&base).unwrap();
    }

    /// A new key is not taken until the person wrote it down: copies stay for the key they
    /// were for meanwhile.
    #[test]
    fn a_new_key_changes_nothing_until_it_is_taken() {
        let base = folder("new-key");
        let (_, keychain) = keychain();
        let (dir, _, key, _) = exported(&base, &keychain, false);
        let before = fs::read(dir.join(KEY_FILE)).unwrap();
        let new = NewKey::generate().unwrap();
        assert_eq!(fs::read(dir.join(KEY_FILE)).unwrap(), before);
        let copy = base.join("still.fogata-copy");
        export_with(&dir, &copy, false, &keychain).unwrap();
        open_copy(&copy, &key).unwrap();
        turn_on_with(&dir, &new, &keychain).unwrap();
        export_with(&dir, &copy, false, &keychain).unwrap();
        let e = open_copy(&copy, &key).unwrap_err();
        assert_eq!(
            e.downcast_ref::<CopyProblem>(),
            Some(&CopyProblem::OtherKey)
        );
        open_copy(&copy, &RecoveryKey::parse(&new.text()).unwrap()).unwrap();
        fs::remove_dir_all(&base).unwrap();
    }

    /// Someone who knows only the public half of Ana's key (from a backup of her folder, say)
    /// makes a copy of an account of theirs for it: it opens with Ana's key, but restores
    /// nothing, as it was not made with her account.
    #[test]
    fn a_copy_made_with_the_public_half_alone_is_refused() {
        let base = folder("forged");
        let (_, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        let summary = open_copy(&copy, &key).unwrap();
        assert_eq!(summary.name, "Ana");
        let mallory = base.join("mallory");
        account(&mallory, &keychain, "Ana");
        let store = Store::open_with(mallory.clone(), keychain.clone()).unwrap();
        let mut plain = vec![KEY_VERSION];
        plain.extend_from_slice(&key.public().0);
        plain.extend_from_slice(&[7; 32]);
        let data_key = store.data_key().unwrap();
        write_private(
            &mallory.join(KEY_FILE),
            &crate::store::to_disk(data_key.as_ref(), KEY_CONTEXT, &plain).unwrap(),
        )
        .unwrap();
        drop(store);
        let forged = base.join("forged.fogata-copy");
        export_with(&mallory, &forged, false, &keychain).unwrap();
        for e in [
            open_copy(&forged, &key).unwrap_err(),
            restore_with(&base.join("new"), &forged, &key, false, &keychain).unwrap_err(),
        ] {
            assert_eq!(
                e.downcast_ref::<CopyProblem>(),
                Some(&CopyProblem::Unproven)
            );
        }
        assert!(!base.join("new").join("identity.key").exists());
        fs::remove_dir_all(&base).unwrap();
    }

    /// What copies are made for cannot be changed by anything that writes in a sealed folder
    /// without its key: a key file put there in the clear, or one sealed with another
    /// folder's key, makes no copy.
    #[test]
    fn a_key_file_swapped_in_a_sealed_folder_makes_no_copy() {
        let base = folder("swapped");
        let (_, keychain) = keychain();
        let (dir, _, key, _) = exported(&base, &keychain, false);
        let theirs = RecoveryKey::generate().unwrap();
        let mut plain = vec![KEY_VERSION];
        plain.extend_from_slice(&theirs.public().0);
        plain.extend_from_slice(&theirs.auth_key()[..]);
        let other = base.join("other");
        account(&other, &keychain, "Leo");
        let other_key = Store::open_with(other.clone(), keychain.clone())
            .unwrap()
            .data_key()
            .unwrap();
        let ours = fs::read(dir.join(KEY_FILE)).unwrap();
        for swapped in [
            plain.clone(),
            crate::store::to_disk(other_key.as_ref(), KEY_CONTEXT, &plain).unwrap(),
        ] {
            fs::write(dir.join(KEY_FILE), &swapped).unwrap();
            let to = base.join("copy.fogata-copy");
            assert!(export_with(&dir, &to, false, &keychain).is_err());
            assert!(!to.exists());
        }
        fs::write(dir.join(KEY_FILE), &ours).unwrap();
        let to = base.join("copy.fogata-copy");
        export_with(&dir, &to, false, &keychain).unwrap();
        assert_eq!(open_copy(&to, &key).unwrap().name, "Ana");
        fs::remove_dir_all(&base).unwrap();
    }

    /// The same swap made while Fogata is closed, with a plain key file (and a plain
    /// identity) that the next start would have sealed with the folder's own key: it is
    /// refused and moved aside as it was, unsealed, and no copy is made for it, whatever
    /// the note of how far sealing got says (as left, rewritten in the clear, or gone).
    #[test]
    fn plain_secrets_planted_in_a_sealed_folder_are_refused_after_a_restart() {
        let base = folder("planted");
        let (_, keychain) = keychain();
        let (dir, ana, key, _) = exported(&base, &keychain, false);
        let theirs = RecoveryKey::generate().unwrap();
        let mut planted_key = vec![KEY_VERSION];
        planted_key.extend_from_slice(&theirs.public().0);
        planted_key.extend_from_slice(&theirs.auth_key()[..]);
        let planted_identity = hex::encode(DeviceIdentity::generate().unwrap().secret_bytes());
        let ours = fs::read(dir.join(KEY_FILE)).unwrap();
        let identity = dir.join("identity.key");
        let sealed_identity = fs::read(&identity).unwrap();
        let note = dir.join(crate::store::SEALING_FILE);
        let noted = fs::read(&note).unwrap();
        let refused = dir.join(crate::store::REFUSED_DIR);
        for left in [
            Some(noted.clone()),
            Some(br#"{"phase":"pending"}"#.to_vec()),
            None,
        ] {
            match &left {
                Some(bytes) => fs::write(&note, bytes).unwrap(),
                None => fs::remove_file(&note).unwrap(),
            }
            fs::write(dir.join(KEY_FILE), &planted_key).unwrap();
            fs::write(&identity, &planted_identity).unwrap();
            let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
            let refusal = store.identity().unwrap_err().to_string();
            assert!(refusal.contains("unencrypted"), "{refusal}");
            assert!(!is_on(&dir));
            let to = base.join("copy.fogata-copy");
            assert!(export_with(&dir, &to, false, &keychain).is_err());
            assert!(!to.exists());
            assert_eq!(
                fs::read(refused.join(KEY_FILE)).unwrap(),
                planted_key,
                "{left:?}"
            );
            assert_eq!(
                fs::read(refused.join("identity.key")).unwrap(),
                planted_identity.as_bytes()
            );
            drop(store);
            fs::remove_dir_all(&refused).unwrap();
            // The account's own files put back, it is the account it was.
            fs::write(&identity, &sealed_identity).unwrap();
            fs::write(dir.join(KEY_FILE), &ours).unwrap();
            let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
            assert_eq!(
                store.identity().unwrap().0.public_key(),
                ana.identity.public_key()
            );
            let to = base.join("copy.fogata-copy");
            export_with(&dir, &to, false, &keychain).unwrap();
            assert_eq!(open_copy(&to, &key).unwrap().name, "Ana");
            assert!(open_copy(&to, &theirs).is_err());
            fs::remove_file(&to).unwrap();
            assert!(!refused.exists());
        }
        fs::remove_dir_all(&base).unwrap();
    }

    /// A plain folder with copies on is sealed whole at the first start a keychain keeps a
    /// key, its recovery key with it: copies go on for the same key.
    #[test]
    fn a_plain_folder_with_copies_on_keeps_them_once_sealed() {
        let base = folder("sealed-later");
        let dir = base.join("ana");
        account(&dir, &Keychain::Off, "Ana");
        let key = on(&dir, &Keychain::Off);
        assert!(sealed_form(&fs::read(dir.join(KEY_FILE)).unwrap()).is_none());
        let (_, keychain) = keychain();
        let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
        assert!(store.data_key().unwrap().is_some());
        assert!(store.sealed());
        drop(store);
        assert!(sealed_form(&fs::read(dir.join(KEY_FILE)).unwrap()).is_some());
        let to = base.join("copy.fogata-copy");
        export_with(&dir, &to, false, &keychain).unwrap();
        assert_eq!(open_copy(&to, &key).unwrap().name, "Ana");
        fs::remove_dir_all(&base).unwrap();
    }

    /// An identity whose profile is gone or does not read still has its groups, chats and
    /// key: a restore refuses it unless told to replace it, and then keeps it whole with its
    /// key. A folder only opened before (settings, the folder for logs) holds no account.
    #[test]
    fn an_identity_without_a_profile_that_reads_is_an_account_too() {
        let base = folder("no-profile");
        let (keys, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        for (n, damage) in ["gone", "unreadable"].into_iter().enumerate() {
            let dir = base.join(format!("bea-{n}"));
            account(&dir, &keychain, "Bea");
            let bea_key = vault::key_name(&dir).unwrap();
            let _ = fs::remove_file(dir.join("friends.bin.bak"));
            match damage {
                "gone" => fs::remove_file(dir.join("friends.bin")).unwrap(),
                _ => fs::write(dir.join("friends.bin"), b"not a profile").unwrap(),
            }
            let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
            assert!(!store.has_profile().unwrap(), "{damage}");
            drop(store);
            let before = files_in(&dir);
            let e = restore_with(&dir, &copy, &key, false, &keychain).unwrap_err();
            assert!(
                matches!(
                    e.downcast_ref::<Refused>(),
                    Some(Refused::AccountHere(None))
                ),
                "{damage}: {e:#}"
            );
            assert_eq!(files_in(&dir), before, "{damage}");
            let restored = restore_with(&dir, &copy, &key, true, &keychain).unwrap();
            assert!(restored.replaced, "{damage}");
            assert!(keeps_replaced(&dir), "{damage}");
            assert!(keys.names().contains(&bea_key), "{damage}");
            let kept = files_in(&dir.join(REPLACED_DIR));
            for (path, bytes) in &before {
                assert_eq!(kept.get(path), Some(bytes), "{damage}: {}", path.display());
            }
        }
        // A folder only opened holds nothing to keep.
        let fresh = base.join("fresh");
        Store::open_with(fresh.clone(), keychain.clone()).unwrap();
        assert!(
            !restore_with(&fresh, &copy, &key, false, &keychain)
                .unwrap()
                .replaced
        );
        assert!(!fresh.join(REPLACED_DIR).exists());
        fs::remove_dir_all(&base).unwrap();
    }

    /// A folder the application opened before any account (to show its welcome page, say)
    /// holds a data key, the settings and its own folders, but no account: a copy is restored
    /// there without being told to replace anything, and nothing of it is kept aside.
    #[test]
    fn a_folder_opened_before_any_account_takes_a_copy_unasked() {
        let base = folder("fresh-opened");
        let (keys, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        for (n, opened) in [keychain.clone(), Keychain::Off].into_iter().enumerate() {
            let dir = base.join(format!("new-{n}"));
            // What a start without an account leaves: the store's folders and lock, the data
            // key the first look for it makes (with a keychain that may seal), the settings
            // and the window's and the log's folders.
            {
                let store = Store::open_with(dir.clone(), opened.clone()).unwrap();
                let _running = store.hold_running().unwrap();
                store.data_key().unwrap();
                let mut meta = store.meta();
                meta.port = Some(40_123);
                store.save_meta(&meta).unwrap();
                assert!(!store.has_profile().unwrap());
            }
            fs::write(dir.join("app.json"), br#"{"theme":"dark"}"#).unwrap();
            fs::create_dir_all(dir.join("window")).unwrap();
            fs::create_dir_all(dir.join("diagnostics")).unwrap();
            fs::write(dir.join("diagnostics/fogata-2026-11-04.log"), b"started").unwrap();
            let fresh_key = vault::key_name(&dir);
            assert_eq!(fresh_key.is_some(), n == 0);
            let restored = restore_with(&dir, &copy, &key, false, &keychain).unwrap();
            assert!(!restored.replaced, "{n}");
            assert!(!dir.join(REPLACED_DIR).exists(), "{n}");
            assert!(!keeps_replaced(&dir), "{n}");
            assert_eq!(restored.name, "Ana");
            // The new folder's key, which sealed nothing, goes; the account has its own.
            if let Some(fresh) = fresh_key {
                assert!(!keys.names().contains(&fresh));
                assert_ne!(vault::key_name(&dir), Some(fresh));
            }
        }
        fs::remove_dir_all(&base).unwrap();
    }

    /// Where the keychain keeps no key, a copy is restored unencrypted only when asked, and
    /// asking leaves nothing in the folder that says so when the restore does not happen.
    #[test]
    fn unencrypted_is_for_one_restore_only() {
        let base = folder("unencrypted");
        let (keys, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        keys.locked
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let dir = base.join("new");
        let options = |unencrypted| RestoreOptions {
            keychain: Some(keychain.clone()),
            unencrypted,
            ..Default::default()
        };
        let e = restore(&dir, &copy, &key, &options(false)).unwrap_err();
        assert!(e.downcast_ref::<vault::NoKeychain>().is_some(), "{e:#}");
        let other = RecoveryKey::generate().unwrap();
        assert!(restore(&dir, &copy, &other, &options(true)).is_err());
        for gone in [vault::PLAIN_FILE, "identity.key", "incoming"] {
            assert!(!dir.join(gone).exists(), "{gone}");
        }
        restore(&dir, &copy, &key, &options(true)).unwrap();
        assert!(vault::key_name(&dir).is_none());
        let identity = fs::read(dir.join("identity.key")).unwrap();
        assert!(sealed_form(&identity).is_none(), "plain, as asked");
        fs::remove_dir_all(&base).unwrap();
    }

    /// The bytes of a key from its text, as anything that wrote it out would hold them.
    fn key_bytes(text: &str) -> Vec<u8> {
        const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let mut out = Vec::new();
        let (mut acc, mut bits) = (0u32, 0);
        for c in text.bytes().filter(|c| *c != b' ').take(52) {
            acc = (acc << 5) | CROCKFORD.iter().position(|x| *x == c).unwrap() as u32;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                out.push((acc >> bits) as u8);
                acc &= (1 << bits) - 1;
            }
        }
        out.truncate(32);
        out
    }

    /// What a log line or a file would hold of the key.
    fn forms_of(text: &str) -> Vec<Vec<u8>> {
        let bytes = key_bytes(text);
        let joined: String = text.chars().filter(|c| *c != ' ').collect();
        let mut forms = vec![
            text.as_bytes().to_vec(),
            joined.as_bytes().to_vec(),
            joined.to_lowercase().into_bytes(),
            hex::encode(&bytes).into_bytes(),
            bytes.clone(),
        ];
        // Any four groups in a row.
        for group in text.split(' ').collect::<Vec<_>>().windows(4) {
            forms.push(group.join(" ").into_bytes());
        }
        forms
    }

    fn holds(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[derive(Clone, Default)]
    struct Lines(Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for Lines {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_recovery_key_is_written_nowhere_nor_logged() {
        let base = folder("nowhere");
        let (_, keychain) = keychain();
        let lines = Lines::default();
        let writer = lines.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let mut texts = Vec::new();
        tracing::subscriber::with_default(subscriber, || {
            let dir = base.join("ana");
            account(&dir, &keychain, "Ana");
            // Turned on, then changed: neither key is kept.
            for _ in 0..2 {
                let new = NewKey::generate().unwrap();
                turn_on_with(&dir, &new, &keychain).unwrap();
                texts.push(new.text());
            }
            let key = RecoveryKey::parse(&texts[1]).unwrap();
            let copy = base.join("copy.fogata-copy");
            export_with(&dir, &copy, true, &keychain).unwrap();
            // A wrong key, a key mistyped, then the right one.
            let other = RecoveryKey::generate().unwrap();
            let into = base.join("new");
            assert!(restore_with(&into, &copy, &other, false, &keychain).is_err());
            assert!(check_key(&texts[1][1..]).is_err());
            restore_with(&into, &copy, &key, false, &keychain).unwrap();
            tracing::info!("the end");
        });
        let logged = lines.0.lock().unwrap().clone();
        assert!(holds(&logged, b"the end"), "the log was taken");
        let mut written = files_in(&base);
        written.insert(PathBuf::from("log"), logged);
        for text in &texts {
            for form in forms_of(text) {
                for (path, bytes) in &written {
                    assert!(!holds(bytes, &form), "{} holds the key", path.display());
                }
            }
        }
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn the_header_holds_nothing_of_the_account() {
        let base = folder("header");
        let (_, keychain) = keychain();
        let (dir, ana, key, copy) = exported(&base, &keychain, true);
        let bytes = fs::read(&copy).unwrap();
        let header = CopyHeader::decode(&bytes).unwrap();
        // Only these, in this order.
        let public = key.public();
        let mut expected = b"FOGATACOPY\x01".to_vec();
        expected.extend_from_slice(&public.fingerprint());
        expected.extend_from_slice(&header.ephemeral);
        expected.extend_from_slice(&((now_ms() / MS_A_DAY) as u32).to_be_bytes());
        assert_eq!(&bytes[..HEADER_LEN], &expected[..]);
        // Nothing of the account reads anywhere in the copy.
        let device = ana.identity.public_key();
        let book = Store::open_with(dir.clone(), keychain.clone())
            .unwrap()
            .friends()
            .unwrap()
            .unwrap();
        for clear in [
            &b"Ana"[..],
            b"hello",
            b"Home",
            b"identity.key",
            b"friends.bin",
            b"a picture",
            device.as_bytes(),
            hex::encode(device.as_bytes()).as_bytes(),
            ana.log.group_id().as_bytes(),
            hex::encode(ana.log.group_id().as_bytes()).as_bytes(),
            &public.0,
            &book.profile.signature,
        ] {
            assert!(!holds(&bytes, clear), "{}", String::from_utf8_lossy(clear));
        }
        let _ = ana.manager;
        fs::remove_dir_all(&base).unwrap();
    }

    /// A copy cut short while it is written (a full disk) leaves no file under its name, and
    /// an older copy there stays as it was.
    #[test]
    fn a_copy_cut_short_is_never_taken_for_one() {
        let base = folder("export-cut");
        let (_, keychain) = keychain();
        let (dir, _, _, copy) = exported(&base, &keychain, true);
        let older = fs::read(&copy).unwrap();
        // What someone else left beside it under the name a copy being written once took: a
        // file, and a link to a file elsewhere. Neither is written through nor removed.
        let tmp = copy.with_file_name(format!(
            "{}.tmp",
            copy.file_name().unwrap().to_string_lossy()
        ));
        fs::write(&tmp, b"not ours").unwrap();
        let elsewhere = base.join("elsewhere");
        fs::write(&elsewhere, b"theirs").unwrap();
        #[cfg(unix)]
        let link = {
            let link = copy.with_file_name("Fogata-link.fogata-copy.tmp");
            std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
            link
        };
        let around = || -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&base)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        let before = around();
        for n in [0, 1, 3, 6] {
            faults::fail_after(Some(n));
            let made = export_with(&dir, &copy, true, &keychain);
            faults::fail_after(None);
            assert!(made.is_err(), "{n}");
            assert_eq!(fs::read(&copy).unwrap(), older, "{n}");
            assert_eq!(around(), before, "{n}: something left or removed beside it");
            assert_eq!(fs::read(&tmp).unwrap(), b"not ours", "{n}");
        }
        #[cfg(unix)]
        {
            let to = copy.with_file_name("Fogata-link.fogata-copy");
            export_with(&dir, &to, true, &keychain).unwrap();
            assert_eq!(fs::read(&elsewhere).unwrap(), b"theirs");
            assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        }
        let fresh = base.join("fresh.fogata-copy");
        faults::fail_after(Some(2));
        let full = export_with(&dir, &fresh, true, &keychain).unwrap_err();
        faults::fail_after(None);
        // Says what failed, and that nothing changed.
        assert!(
            full.to_string()
                .starts_with("no room on the disk for the copy at"),
            "{full:#}"
        );
        assert!(!fresh.exists());
        // Not in the data folder itself.
        assert!(export_with(&dir, &dir.join("x.fogata-copy"), false, &keychain).is_err());
        fs::remove_dir_all(&base).unwrap();
    }

    /// A copy whose list adds up past what a number holds, or past what an account holds, is
    /// refused before anything is written; so is one there is no room for on the disk, and
    /// one kept inside the folder it would be restored into, which a restore empties.
    #[test]
    fn a_copy_too_large_for_the_disk_or_inside_the_folder_is_refused() {
        let listed = |len| Listed {
            path: "logs/x.log".into(),
            len,
            blake3: blake3::hash(b"").to_hex().to_string(),
        };
        let mut manifest = Manifest {
            device: "ab".repeat(32),
            name: "Ana".into(),
            build: String::new(),
            made_ms: 0,
            attachments: false,
            files: vec![listed(u64::MAX), listed(2)],
        };
        manifest.files[1].path = "logs/y.log".into();
        assert!(check_manifest(&manifest).is_err());
        manifest.files[0].len = 1;
        check_manifest(&manifest).unwrap();
        manifest.files[0].len = MAX_BYTES;
        assert!(check_manifest(&manifest).is_err());

        let base = folder("room");
        let (_, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        let into = base.join("new");
        fs::create_dir_all(&into).unwrap();
        let options = RestoreOptions {
            keychain: Some(keychain.clone()),
            free_space: Some(|_| Some(1024)),
            ..Default::default()
        };
        let e = restore(&into, &copy, &key, &options).unwrap_err();
        assert!(
            matches!(
                e.downcast_ref::<Refused>(),
                Some(Refused::NoRoom { free: 1024, .. })
            ),
            "{e:#}"
        );
        assert!(!into.join("identity.key").exists());
        assert!(!into.join("incoming").exists());
        let inside = into.join("Fogata-copy.fogata-copy");
        fs::copy(&copy, &inside).unwrap();
        let options = RestoreOptions {
            keychain: Some(keychain.clone()),
            ..Default::default()
        };
        let e = restore(&into, &inside, &key, &options).unwrap_err();
        assert_eq!(e.downcast_ref::<Refused>(), Some(&Refused::CopyInFolder));
        assert!(inside.exists());
        fs::remove_file(&inside).unwrap();
        restore(&into, &copy, &key, &options).unwrap();
        fs::remove_dir_all(&base).unwrap();
    }

    /// Once the account moved away, what is left of it here is deleted on the person's word;
    /// another person's account a restore replaced, kept here, is not part of it.
    #[test]
    fn what_is_left_after_a_move_does_not_take_a_kept_account() {
        let base = folder("forget");
        let (keys, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        let dir = base.join("bea");
        account(&dir, &keychain, "Bea");
        let bea_key = vault::key_name(&dir).unwrap();
        restore_with(&dir, &copy, &key, true, &keychain).unwrap();
        fs::write(
            dir.join(transfer::MOVED_FILE),
            br#"{"state":"moved","transfer":"","to":"","name":"Ana","at_ms":0}"#,
        )
        .unwrap();
        assert!(transfer::forget(&dir).is_err());
        assert!(keeps_replaced(&dir));
        assert!(dir.join(REPLACED_DIR).join("identity.key").exists());
        assert!(keys.names().contains(&bea_key));
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_copy_is_named_after_its_day() {
        assert_eq!(copy_name(0), "Fogata-1970-01-01.fogata-copy");
        assert_eq!(date(19_782), "2024-02-29");
        assert_eq!(date(20_761), "2026-11-04");
        assert_eq!(date(11_016), "2000-02-29");
    }

    /// A member's account restored from a copy made before the group started a new epoch:
    /// the log it gets from the members brings the new epoch's secret, sealed to the same
    /// device, so it reads and writes in the group again by itself. (MLS does not run in the
    /// application yet: fogata-mls's `an_older_copy_put_back_catches_up_unless_it_committed`
    /// says what it will do then.)
    #[test]
    fn a_member_restored_from_an_older_copy_gets_the_group_s_newer_epoch() {
        let base = folder("epoch");
        let (_, keychain) = keychain();
        let (dir, ana, key, copy) = exported(&base, &keychain, false);
        // After the copy, the group goes on to a new epoch.
        let mut current = ana.log.clone();
        current.rotate(&ana.manager, 10).unwrap();
        let epoch = current.state().current_epoch().unwrap().id;
        assert_ne!(Some(epoch), ana.log.state().current_epoch().map(|e| e.id));
        let into = base.join("restored");
        restore_with(&into, &copy, &key, false, &keychain).unwrap();
        let store = Store::open_with(into.clone(), keychain.clone()).unwrap();
        let group = store.group("Home").unwrap();
        let mut restored = store.load_log(&group).unwrap();
        let me = store.identity().unwrap().0;
        assert!(restored.state().is_member(&me.public_key()));
        // What a member's sync hands over.
        restored.insert_all(current.entries().cloned().collect::<Vec<_>>());
        let secrets = restored.secrets(&me);
        assert!(secrets.contains_key(&epoch));
        assert_eq!(
            secrets[&epoch].as_bytes(),
            current.secrets(&ana.manager)[&epoch].as_bytes()
        );
        let _ = dir;
        fs::remove_dir_all(&base).unwrap();
    }

    /// A copy written straight into the file chosen, as the sandboxed application makes it:
    /// it restores as any copy; one that fails leaves nothing there; a link there is never
    /// written through.
    #[test]
    fn a_copy_written_in_place_restores_and_a_failed_one_leaves_nothing() {
        let base = folder("in-place");
        let (_, keychain) = keychain();
        let (dir, ana, key, _) = exported(&base, &keychain, false);
        let to = base.join("chosen.fogata-copy");
        // A file the save dialog said to replace.
        fs::write(&to, b"an older file").unwrap();
        let made = export_into_with(&dir, &to, true, &keychain).unwrap();
        assert_eq!(made.bytes, fs::metadata(&to).unwrap().len());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&to).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let names = || -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&base)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        };
        let into = base.join("restored");
        restore_with(&into, &to, &key, false, &keychain).unwrap();
        assert_eq!(seen(&into, &keychain, &ana), seen(&dir, &keychain, &ana));
        // Failing at any frame: nothing is left where it was going, nor beside it.
        let before = names();
        for n in [0, 1, 3, 6] {
            faults::fail_after(Some(n));
            let made = export_into_with(&dir, &to, true, &keychain);
            faults::fail_after(None);
            assert!(made.is_err(), "{n}");
            assert!(
                fs::symlink_metadata(&to).is_err(),
                "{n}: something left at the copy"
            );
            let mut left = names();
            left.push("chosen.fogata-copy".into());
            left.sort();
            assert_eq!(left, before, "{n}: something left beside it");
        }
        // Out of room: said so.
        faults::fail_after(Some(2));
        let full = export_into_with(&dir, &to, true, &keychain).unwrap_err();
        faults::fail_after(None);
        assert!(
            full.to_string()
                .starts_with("no room on the disk for the copy at"),
            "{full:#}"
        );
        assert!(!to.exists());
        #[cfg(unix)]
        {
            let elsewhere = base.join("elsewhere");
            fs::write(&elsewhere, b"theirs").unwrap();
            let link = base.join("link.fogata-copy");
            std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
            assert!(export_into_with(&dir, &link, true, &keychain).is_err());
            assert_eq!(fs::read(&elsewhere).unwrap(), b"theirs");
            assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        }
        // Not in the data folder itself.
        let inside = dir.join("x.fogata-copy");
        assert!(export_into_with(&dir, &inside, false, &keychain).is_err());
        assert!(!inside.exists());
        fs::remove_dir_all(&base).unwrap();
    }

    /// The welcome page makes a device key as it opens, to show its fingerprint: a folder
    /// with that key alone holds no account to keep. A copy goes in without being told to
    /// replace anything, and the key goes, with the folder's key in the keychain, rather than
    /// being kept aside.
    #[test]
    fn a_device_key_alone_is_replaced_unasked_and_not_kept() {
        let base = folder("fresh-identity");
        let (keys, keychain) = keychain();
        let (_, _, key, copy) = exported(&base, &keychain, false);
        for (n, made) in ["first", "another"].into_iter().enumerate() {
            let dir = base.join(format!("welcome-{n}"));
            // As the welcome page makes it (`device_fingerprint`, and "make another key").
            let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
            let (identity, _) = store.identity().unwrap();
            if made == "another" {
                store.new_identity().unwrap();
            }
            drop(store);
            fs::write(dir.join("app.json"), br#"{"theme":"dark"}"#).unwrap();
            assert_eq!(account_files(&dir).unwrap(), Here::FreshIdentity, "{made}");
            assert_eq!(account_in_with(&dir, &keychain).unwrap(), None, "{made}");
            let fresh_key = vault::key_name(&dir).expect("sealed");
            let restored = restore_with(&dir, &copy, &key, false, &keychain).unwrap();
            assert_eq!(restored.name, "Ana");
            assert!(!restored.replaced, "{made}");
            assert!(!keeps_replaced(&dir), "{made}");
            assert!(!dir.join(REPLACED_DIR).exists(), "{made}");
            assert!(
                !keys.names().contains(&fresh_key),
                "{made}: its key left behind"
            );
            let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
            assert_ne!(
                store.identity().unwrap().0.public_key(),
                identity.public_key(),
                "{made}"
            );
            // Another restore is not refused for an account kept aside.
            drop(store);
            restore_with(&dir, &copy, &key, true, &keychain).unwrap();
        }
        // A key with anything else of an account is an account.
        let dir = base.join("with-a-group");
        let store = Store::open_with(dir.clone(), keychain.clone()).unwrap();
        store.identity().unwrap();
        let manager = DeviceIdentity::generate().unwrap();
        let log = GroupLog::create(&manager, "Home", 1).unwrap();
        store.save_group_log(&log, "Home").unwrap();
        drop(store);
        assert_eq!(account_files(&dir).unwrap(), Here::Account);
        assert!(account_in_with(&dir, &keychain).unwrap().is_some());
        fs::remove_dir_all(&base).unwrap();
    }
}
