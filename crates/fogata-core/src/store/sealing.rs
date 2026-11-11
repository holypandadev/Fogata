//! Sealing a data folder with the data key the system keychain keeps: finding the key,
//! sealing the plain files one by one after copying them aside, telling how the folder's
//! secrets are kept, and the files found damaged set aside rather than lost.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{PoisonError, atomic::Ordering},
};

use anyhow::{Context, Result, bail};
use fogata_crypto::at_rest::{DataKey, Form, sealed_form};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[cfg(test)]
use super::faults;
use super::{
    DAMAGED, DoesNotOpen, MLS_DIR, Store, aside_path, copy_aside, key_lock, mls_context,
    remove_copies, sealed_bytes, take_lock, with_suffix, write_private,
};
use crate::vault::{self, Keychain};

/// Where the plain files of a folder being sealed are copied first, until the next start.
pub const BACKUP_DIR: &str = "before-sealing";

/// Where sealing a plain folder stands ([`SealPhase`]). It belongs to this computer: a move
/// does not take it. It is sealed with the folder's key ([`SEALING_CONTEXT`]) when sealing
/// was begun by this version or it ended; earlier versions wrote it plain, and read a sealed
/// one as none (they then check the whole folder again, and note it plain).
pub const SEALING_FILE: &str = "sealing.json";

/// What [`SEALING_FILE`] is sealed for.
const SEALING_CONTEXT: &str = "sealing";

/// Where the plain secrets found in a folder that keeps them sealed are moved, never read
/// nor sealed (see [`PlainSecrets`]). It belongs to this computer: a move does not take it.
pub const REFUSED_DIR: &str = "refused";

/// A secret of the folder that the person chose to set aside, read at the next start
/// ([`Store::set_aside_at_next_start`]). It belongs to this computer: a move does not take
/// it.
pub const SET_ASIDE_FILE: &str = "set-aside.json";

/// What [`SET_ASIDE_FILE`] is sealed for: a note written without the folder's key would put
/// a plain copy someone left in [`BACKUP_DIR`] in place of a secret they damaged.
const SET_ASIDE_CONTEXT: &str = "set-aside";

/// How far sealing a folder got, kept in [`SEALING_FILE`]: the folder counts as sealed only
/// once every secret in it was found sealed and opening with its key, and until then its
/// plain copies in [`BACKUP_DIR`] stay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub(super) enum SealPhase {
    /// Begun, and not ended: the start that began it stopped half way, or is still at it.
    Pending,
    /// Something could not be sealed, did not read back, or could not be checked (why).
    Failed { why: String },
    /// Every secret was found sealed and opening with the folder's key.
    Done,
}

/// Where sealing a folder stood when it was last noted, and whether that note was sealed with
/// the folder's key: one that is not could have been written by anyone who can write the
/// folder.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Noted {
    phase: SealPhase,
    by_key: bool,
}

/// What a start does with the plain secrets (the identity, the friends file and its copies,
/// the recovery key, the MLS state) it finds in a folder that has a key.
///
/// Anyone who can write the folder but not read the keychain (a synced or backed-up copy, a
/// computer left alone) can put plain files there. Sealing them as a folder being sealed
/// does would take them in as the account's own: a recovery key of theirs would get every
/// copy made from then on, the identity's secret and the chats in it. So a plain secret is
/// sealed only while sealing is under way as a note sealed with the folder's key says,
/// which only a copy of Fogata that holds the key writes (as it makes the key, before the
/// folder is marked sealed). Once sealing ended, or when it is noted only in the clear
/// (as earlier versions did), a plain secret was put there by someone else, and is refused:
/// moved to [`REFUSED_DIR`]; except while the folder is as an earlier version left it when
/// its sealing was cut short ([`PlainSecrets::Resume`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlainSecrets {
    /// Sealing is under way, as noted with the key: plain secrets are the account's own.
    Seal,
    /// Sealing was begun by an earlier version, noted in the clear (or not at all), and
    /// neither the identity nor the friends file is sealed yet: they and the MLS state are
    /// sealed (taking them over gives nothing more than putting a whole other folder in this
    /// one's place would), and a plain recovery key is refused: copies of what is sealed here
    /// must not go to a key someone put next to it. Its person turns copies on again.
    SealButRecovery,
    /// Sealing was begun by an earlier version, noted in the clear as begun or failed, and
    /// cut short after the identity was sealed: the plain copy that sealing made of the
    /// identity ([`BACKUP_DIR`]) is still there, and is the identity sealed here. Sealing
    /// goes on ([`Take::UnlessItsCopyDiffers`]): a plain secret that sealing had copied
    /// aside is sealed only as that copy holds it, one it never reached (it stopped before)
    /// is copied and sealed now. A plain recovery key is sealed only as its copy holds it.
    ///
    /// Someone who can write the folder could leave such a folder too, but only while the
    /// account's identity lies in it in the clear (until the start after sealing ended),
    /// where they could read it, and the friends file and the MLS state beside it, anyway.
    Resume,
    /// Every plain secret is refused.
    Refuse,
}

impl PlainSecrets {
    /// What is done with a plain secret found: the recovery key, or another (`recovery`).
    fn take(self, recovery: bool) -> Take {
        match self {
            Self::Seal => Take::Yes,
            Self::SealButRecovery if recovery => Take::No,
            Self::SealButRecovery => Take::Yes,
            Self::Resume if recovery => Take::IfItsCopyIsTheSame,
            Self::Resume => Take::UnlessItsCopyDiffers,
            Self::Refuse => Take::No,
        }
    }
}

/// Whether a plain secret found is sealed as the account's own, or refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Take {
    Yes,
    No,
    /// When the plain copy sealing made of it in [`BACKUP_DIR`] holds the same, or sealing
    /// made none: it never got to it.
    UnlessItsCopyDiffers,
    /// Only when the plain copy sealing made of it holds the same.
    IfItsCopyIsTheSame,
}

/// How a data folder's secrets would be kept, once asked ([`Store::sealing`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sealing {
    /// Encrypted with a key of the system keychain.
    Sealed,
    /// Being encrypted with a key of the system keychain, and not all of it is yet (why):
    /// what could not be sealed (a full disk, say) or checked, or a start that stopped half
    /// way. Tried again at each start.
    Unfinished(String),
    /// Unencrypted: as a copy run for tests is set to leave it, as its person chose, or as
    /// it already was (it is encrypted at the first start a keychain keeps a key).
    Plain,
    /// The folder holds nothing yet, and the keychain kept no key for it (why): nothing
    /// secret is written in it until a keychain keeps one or its person chooses to keep it
    /// unencrypted ([`vault::allow_plain`]).
    NoKeychain(String),
}

impl Store {
    /// The folder's data key (`Some(None)` while its data is plain) if it was looked for
    /// already: a log read before that (by the command line, say) is judged whole rather
    /// than ask the keychain.
    pub(super) fn known_key(&self) -> Option<Option<&DataKey>> {
        match self.key.get() {
            Some(Ok(key)) => Some(key.as_ref()),
            _ => None,
        }
    }

    /// The folder's data key, `None` while its data is plain. The first call looks for it in
    /// the keychain, and seals the folder when it may: it is plain or was left half sealed,
    /// the keychain may seal, and no other copy of Fogata runs on the folder.
    pub fn data_key(&self) -> Result<Option<DataKey>> {
        self.key
            .get_or_init(|| self.find_key().map_err(|e| format!("{e:#}")))
            .clone()
            .map_err(anyhow::Error::msg)
    }

    fn find_key(&self) -> Result<Option<DataKey>> {
        let looking = key_lock(&self.dir);
        let _looking = looking.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(key) = vault::open(&self.dir, &self.keychain)? {
            // Sealing that a crash or a full disk cut short goes on; while another copy runs
            // on the folder, it says how far it got.
            match self.alone()? {
                Some(_running) => self.seal_rest(&key),
                None => {
                    *self.phase.lock().unwrap_or_else(PoisonError::into_inner) =
                        self.read_phase(&key).map(|n| n.phase)
                }
            }
            return Ok(Some(key));
        }
        let Some(keys) = self.keychain.store() else {
            return Ok(None);
        };
        // Another copy running on the folder writes its files as they are. A folder that
        // holds nothing of an account yet gets nothing from this one meanwhile: an identity
        // made now would be written in the clear without its person choosing so.
        let Some(_running) = self.alone()? else {
            if self.holds_nothing() && self.may_seal() && !vault::plain_allowed(&self.dir) {
                bail!(
                    "Fogata is already running with the data in {}; close it first",
                    self.dir.display()
                );
            }
            return Ok(None);
        };
        // Secrets sealed with a key whose `sealed.json` is gone must not get a new key.
        if let Some(key) = vault::recover(&self.dir, keys.as_ref())? {
            self.seal_rest(&key);
            return Ok(Some(key));
        }
        let Keychain::Seal(keys) = &self.keychain else {
            return Ok(None);
        };
        // Sealing is noted as begun, with the key, before the folder is marked sealed: the
        // plain secrets of a folder that has a key are taken as its own only so.
        let begun = |key: &DataKey| self.write_phase(&SealPhase::Pending, Some(key));
        let key = match vault::try_create_with(&self.dir, keys.as_ref(), begun) {
            Ok(Ok(key)) => key,
            // A folder that holds nothing of an account yet gets nothing secret written in
            // the clear until its person chooses so: an identity made now would stay readable
            // by anyone who copies the folder.
            Ok(Err(why)) if self.holds_nothing() && !vault::plain_allowed(&self.dir) => {
                *self.refused.lock().unwrap_or_else(PoisonError::into_inner) = Some(why.clone());
                return Err(vault::NoKeychain(why).into());
            }
            Ok(Err(_)) => return Ok(None),
            Err(e) if self.holds_nothing() => return Err(e),
            // The key was kept but `sealed.json` could not be written (a full disk, say):
            // the keychain forgot the key again, and the account runs as it is, plain, and
            // says so, until a start can seal it.
            Err(e) => {
                tracing::warn!("cannot seal the data in {}: {e:#}", self.dir.display());
                *self.phase.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some(SealPhase::Failed {
                        why: format!("{e:#}"),
                    });
                return Ok(None);
            }
        };
        // Chosen while no keychain kept a key; encrypted now.
        let _ = fs::remove_file(self.dir.join(vault::PLAIN_FILE));
        tracing::info!("sealing the data in {}", self.dir.display());
        self.seal_rest(&key);
        Ok(Some(key))
    }

    /// Seals what is still plain. What could not be sealed now (a full disk, say) still
    /// reads, and the next start goes on with it. [`SEALING_FILE`] says so from before the
    /// first file is sealed, and the folder counts as sealed ([`Store::sealed`]) only once
    /// every secret in it was found sealed and opening with `key`, which is checked once,
    /// when sealing ends; later starts refuse the plain secrets they find ([`PlainSecrets`]).
    /// Then the plain copies of the chats go at once: they would keep, in the clear, what
    /// disappears from the chats meanwhile. The identity, the friends file and the MLS state
    /// keep theirs until a later start opens the folder whole ([`Store::drop_backup`]): a
    /// keychain that gave the key back once may still lose it (a reset keychain, an item
    /// tied to the program's signature), and without those copies the account would be gone.
    fn seal_rest(&self, key: &DataKey) {
        self.set_aside_marked(key);
        self.take_back_refused(key);
        let noted = self.read_phase(key);
        let done = noted.as_ref().is_some_and(|n| n.phase == SealPhase::Done);
        let plain = match &noted {
            Some(Noted {
                phase: SealPhase::Done,
                ..
            }) => PlainSecrets::Refuse,
            Some(Noted { by_key: true, .. }) => PlainSecrets::Seal,
            _ if self.holds_no_sealed_account() => PlainSecrets::SealButRecovery,
            // An earlier version noted sealing in the clear as begun, and stopped: its plain
            // secrets are taken as the account's own as far as its copies say so.
            Some(Noted { by_key: false, .. }) if self.cut_short_here(key) => PlainSecrets::Resume,
            _ => PlainSecrets::Refuse,
        };
        self.done_before.store(done, Ordering::Relaxed);
        // Only a note sealed with the key lets later starts seal plain secrets: one written
        // by a start that refused some, or that found sealing noted in the clear, stays in
        // the clear until sealing ends.
        let noting = (plain == PlainSecrets::Seal).then_some(key);
        if !done && let Err(e) = self.write_phase(&SealPhase::Pending, noting) {
            tracing::warn!("cannot note that sealing began: {e:#}");
        }
        let sealed = self
            .seal_plain(key, plain)
            .and_then(|()| if done { Ok(()) } else { self.check_sealed(key) });
        let phase = match sealed {
            Ok(()) if done && noted.as_ref().is_some_and(|n| n.by_key) => SealPhase::Done,
            // Ended now, or noted in the clear by an earlier version: noted with the key.
            Ok(()) => match self.write_phase(&SealPhase::Done, Some(key)) {
                Ok(()) => SealPhase::Done,
                Err(_) if done => SealPhase::Done,
                Err(e) => SealPhase::Failed {
                    why: format!("{e:#}"),
                },
            },
            Err(e) => {
                let failed = SealPhase::Failed {
                    why: format!("{e:#}"),
                };
                if let Err(e) = self.write_phase(&failed, noting) {
                    tracing::debug!("cannot note that sealing failed: {e:#}");
                }
                failed
            }
        };
        match &phase {
            SealPhase::Done => self.remove_backup_of_chats(),
            SealPhase::Failed { why } => tracing::warn!(
                "the data in {} is not all sealed yet: {why}",
                self.dir.display()
            ),
            SealPhase::Pending => {}
        }
        *self.phase.lock().unwrap_or_else(PoisonError::into_inner) = Some(phase);
    }

    /// Where sealing the folder stood when it was last noted; `None` when it never was (a
    /// plain folder, or one sealed by a version before [`SEALING_FILE`]) or it does not read.
    /// A note sealed with anything but `key` is none.
    fn read_phase(&self, key: &DataKey) -> Option<Noted> {
        let bytes = fs::read(self.dir.join(SEALING_FILE)).ok()?;
        if sealed_form(&bytes).is_some() {
            let plain = key.open_whole(SEALING_CONTEXT, &bytes).ok()?;
            return Some(Noted {
                phase: serde_json::from_slice(&plain).ok()?,
                by_key: true,
            });
        }
        Some(Noted {
            phase: serde_json::from_slice(&bytes).ok()?,
            by_key: false,
        })
    }

    /// Notes `phase`, sealed with `key` when there is one, else in the clear.
    fn write_phase(&self, phase: &SealPhase, key: Option<&DataKey>) -> Result<()> {
        let json = Zeroizing::new(serde_json::to_vec_pretty(phase)?);
        let bytes = match key {
            Some(key) => key.seal_whole(SEALING_CONTEXT, &json)?,
            None => json.to_vec(),
        };
        write_private(&self.dir.join(SEALING_FILE), &bytes)
    }

    /// Whether neither the identity nor the friends file (nor its copy) is there sealed:
    /// sealing, which seals them first, did not get past them yet.
    fn holds_no_sealed_account(&self) -> bool {
        [
            self.identity_path(),
            self.friends_path(),
            self.friends_copy_path(),
        ]
        .iter()
        .all(|p| fs::read(p).map_or(true, |b| sealed_form(&b).is_none()))
    }

    /// Checks that every secret of the folder is sealed and opens with `key`: the identity,
    /// the friends file and its copy, the MLS state and the direct chats' journals, pins and
    /// profiles. Copies of damaged files only need to be sealed: being damaged, they may not
    /// open. Attachments are sealed in the background, and told apart when asked
    /// ([`crate::direct::store::DirectStore::holds_plain_file`]).
    fn check_sealed(&self, key: &DataKey) -> Result<()> {
        let opens = |path: &Path, context: &str| -> Result<()> {
            let Some(bytes) = sealed_bytes(path)? else {
                return Ok(());
            };
            key.open_whole(context, &bytes)
                .map_err(|_| DoesNotOpen(path.to_path_buf()))?;
            Ok(())
        };
        opens(&self.identity_path(), "identity")?;
        opens(&self.friends_path(), "friends")?;
        opens(&self.friends_copy_path(), "friends")?;
        opens(
            &self.dir.join(crate::recovery::KEY_FILE),
            crate::recovery::KEY_CONTEXT,
        )?;
        for entry in fs::read_dir(&self.dir)?.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("friends.bin.damaged")
            {
                sealed_bytes(&entry.path())?;
            }
        }
        for entry in fs::read_dir(self.dir.join(MLS_DIR))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(group) = name.strip_suffix(".bin") {
                opens(&entry.path(), &mls_context(group))?;
            }
        }
        crate::direct::store::check_sealed(&self.dir, key)
    }

    /// `Some` when no other copy of Fogata runs on the folder, with the folder's running
    /// lock held until it is dropped (nothing more when this store holds it already).
    fn alone(&self) -> Result<Option<Option<fs::File>>> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(Some(None));
        }
        let path = self.dir.join("running.lock");
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("cannot open {}", path.display()))?;
        match take_lock(&file) {
            Ok(()) => Ok(Some(Some(file))),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(_)) => Ok(Some(None)),
        }
    }

    /// Seals the folder's plain secrets with `key`, the identity and the friends file first;
    /// each is copied to [`BACKUP_DIR`] before and read back after. Those `plain` says are
    /// not the account's own are refused instead. It runs while the key is first looked for,
    /// which every reader of these files waits for.
    fn seal_plain(&self, key: &DataKey, plain: PlainSecrets) -> Result<()> {
        let take = plain.take(false);
        self.name_key(key, &self.identity_path(), "identity")?;
        self.name_key(key, &self.friends_path(), "friends")?;
        self.seal_whole(key, &self.identity_path(), "identity", take)?;
        self.seal_whole(key, &self.friends_path(), "friends", take)?;
        self.seal_whole(key, &self.friends_copy_path(), "friends", take)?;
        self.seal_whole(
            key,
            &self.dir.join(crate::recovery::KEY_FILE),
            crate::recovery::KEY_CONTEXT,
            plain.take(true),
        )?;
        for entry in fs::read_dir(&self.dir)?.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with("friends.bin.damaged")
            {
                self.seal_whole(key, &entry.path(), DAMAGED, take)?;
            }
        }
        for entry in fs::read_dir(self.dir.join(MLS_DIR))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(group) = name.strip_suffix(".bin") {
                self.seal_whole(key, &entry.path(), &mls_context(group), take)?;
            }
        }
        crate::direct::store::seal_plain(&self.dir, key, &|path, bytes| self.back_up(path, bytes))
    }

    /// Seals the file at `path`, if it is there and plain, as one box for `context`; or,
    /// when `take` says it is not the account's own, moves it to [`REFUSED_DIR`] as it is.
    fn seal_whole(&self, key: &DataKey, path: &Path, context: &str, take: Take) -> Result<()> {
        let plain = match fs::read(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            read => {
                Zeroizing::new(read.with_context(|| format!("cannot read {}", path.display()))?)
            }
        };
        if sealed_form(&plain).is_some() {
            return Ok(());
        }
        if !self.takes(path, &plain, take) {
            return self.refuse(path);
        }
        self.back_up(path, &plain)?;
        write_private(path, &key.seal_whole(context, &plain)?)?;
        let back = fs::read(path)
            .ok()
            .and_then(|b| key.open_whole(context, &b).ok());
        if back.as_ref().map(|b| &b[..]) != Some(&plain[..]) {
            write_private(path, &plain)?;
            bail!("{} did not read back once sealed", path.display());
        }
        Ok(())
    }

    /// Whether the plain secret at `path`, holding `plain`, is sealed as the account's own.
    fn takes(&self, path: &Path, plain: &[u8], take: Take) -> bool {
        match take {
            Take::Yes => true,
            Take::No => false,
            Take::UnlessItsCopyDiffers | Take::IfItsCopyIsTheSame => match self.copy_of(path) {
                Some(copy) => *copy == *plain,
                None => take == Take::UnlessItsCopyDiffers,
            },
        }
    }

    /// The plain copy sealing made of the file at `path` in [`BACKUP_DIR`], if it is there.
    fn copy_of(&self, path: &Path) -> Option<Zeroizing<Vec<u8>>> {
        let relative = path.strip_prefix(&self.dir).ok()?;
        let copy = self.dir.join(BACKUP_DIR).join(relative);
        // Only a file of its own: never read through a link someone left there.
        if !fs::symlink_metadata(&copy).ok()?.is_file() {
            return None;
        }
        fs::read(copy).ok().map(Zeroizing::new)
    }

    /// Whether the folder is as an earlier version left it when it stopped sealing it after
    /// the identity: the identity is sealed, opens with `key`, and its plain copy made just
    /// before is still in [`BACKUP_DIR`]. Only a copy of Fogata that sealed this identity
    /// leaves that copy (or someone who could read it, who has the account already); the
    /// start after sealing ended removes it ([`Store::drop_backup`]).
    fn cut_short_here(&self, key: &DataKey) -> bool {
        let Ok(sealed) = fs::read(self.identity_path()) else {
            return false;
        };
        if sealed_form(&sealed).is_none() {
            return false;
        }
        let Ok(identity) = key.open_whole("identity", &sealed) else {
            return false;
        };
        self.copy_of(&self.identity_path())
            .is_some_and(|copy| *copy == *identity)
    }

    /// Brings back, sealed, the account's own secrets that a start of an earlier version
    /// (0.1.0-beta.8) refused in a folder whose sealing an older one had cut short: it took
    /// them for files someone put there, and the account came up without its friends. A
    /// refused file is taken back by the rule that start should have used
    /// ([`PlainSecrets::Resume`]): only while the identity's plain copy of that sealing is
    /// still there ([`Store::cut_short_here`]), only when its copy holds the same (or it has
    /// none: sealing never reached it), and only where nothing is now (a profile made since
    /// stays). The friends file and its copy, the MLS state and the recovery key (only as
    /// its copy holds it) are taken back; anything else stays refused.
    fn take_back_refused(&self, key: &DataKey) {
        let refused = self.dir.join(REFUSED_DIR);
        if !refused.is_dir() || !self.cut_short_here(key) {
            return;
        }
        let mut back = vec![
            (self.friends_path(), "friends".to_string(), false),
            (self.friends_copy_path(), "friends".to_string(), false),
            (
                self.dir.join(crate::recovery::KEY_FILE),
                crate::recovery::KEY_CONTEXT.to_string(),
                true,
            ),
        ];
        for entry in fs::read_dir(refused.join(MLS_DIR))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(group) = name.strip_suffix(".bin") {
                back.push((
                    self.dir.join(MLS_DIR).join(&name),
                    mls_context(group),
                    false,
                ));
            }
        }
        let mut friends_kept_since = false;
        for (path, context, recovery) in back {
            // The friends file's copy goes with it: beside one made since, it would hold
            // another profile than the one it is the copy of.
            if path == self.friends_copy_path() && friends_kept_since {
                continue;
            }
            let take = PlainSecrets::Resume.take(recovery);
            match self.take_back(key, &path, &context, take) {
                Ok(true) => {}
                Ok(false) if path == self.friends_path() => {
                    friends_kept_since =
                        fs::symlink_metadata(&path).is_ok() && refused.join("friends.bin").exists();
                }
                Ok(false) => {}
                Err(e) => tracing::warn!("{} was not taken back: {e:#}", path.display()),
            }
        }
        // Nothing left in it: it goes, and nothing says files were refused any more.
        for dir in [refused.join(MLS_DIR), refused] {
            let _ = fs::remove_dir(dir);
        }
    }

    /// Takes the refused file for `path` back to it, sealed for `context`, when `take` says
    /// it is the account's own and nothing is at `path`; it is removed from [`REFUSED_DIR`]
    /// once its sealed form reads back, which a start that stops in between finds and ends.
    /// Whether it is back.
    fn take_back(&self, key: &DataKey, path: &Path, context: &str, take: Take) -> Result<bool> {
        let relative = path
            .strip_prefix(&self.dir)
            .with_context(|| format!("{} is not in the data folder", path.display()))?;
        let from = self.dir.join(REFUSED_DIR).join(relative);
        if !fs::symlink_metadata(&from).is_ok_and(|m| m.is_file()) {
            return Ok(false);
        }
        let plain = Zeroizing::new(
            fs::read(&from).with_context(|| format!("cannot read {}", from.display()))?,
        );
        if sealed_form(&plain).is_some() || !self.takes(path, &plain, take) {
            return Ok(false);
        }
        let opens_as_plain = |path: &Path| {
            fs::read(path)
                .ok()
                .and_then(|b| key.open_whole(context, &b).ok())
                .is_some_and(|b| *b == *plain)
        };
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            // Taken back by a start that stopped before it removed the refused one.
            Ok(_) if opens_as_plain(path) => return remove_taken_back(&from).map(|()| true),
            // Something else is there now: it stays, and so does the refused file.
            _ => return Ok(false),
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        write_private(path, &key.seal_whole(context, &plain)?)?;
        if !opens_as_plain(path) {
            let _ = fs::remove_file(path);
            bail!("{} did not read back once sealed", path.display());
        }
        tracing::warn!(
            "{} was refused by an earlier start, though it was the account's own as sealing \
             left it: taken back, sealed",
            path.display()
        );
        remove_taken_back(&from).map(|()| true)
    }

    /// Moves the plain secret at `path` to [`REFUSED_DIR`], under its path in the folder (with
    /// `.2` and so on added when an earlier one is there): it is nothing of the account's.
    fn refuse(&self, path: &Path) -> Result<()> {
        let relative = path
            .strip_prefix(&self.dir)
            .with_context(|| format!("{} is not in the data folder", path.display()))?;
        let first = self.dir.join(REFUSED_DIR).join(relative);
        if let Some(parent) = first.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(
                    self.dir.join(REFUSED_DIR),
                    fs::Permissions::from_mode(0o700),
                );
            }
        }
        let to = std::iter::once(first.clone())
            .chain((2..).map(|n| with_suffix(&first, &format!(".{n}"))))
            .find(|p| fs::symlink_metadata(p).is_err())
            .expect("some name is free");
        fs::rename(path, &to).with_context(|| format!("cannot set {} aside", path.display()))?;
        tracing::warn!(
            "{} was plain in a folder that keeps it sealed, so it was put there without the \
             folder's key: refused, and moved to {}",
            path.display(),
            to.display()
        );
        Ok(())
    }

    /// Seals again the file at `path` if it was sealed without the name of its key, as
    /// folders were sealed at first, so it can be found again without `sealed.json`.
    fn name_key(&self, key: &DataKey, path: &Path, context: &str) -> Result<()> {
        let Ok(bytes) = fs::read(path) else {
            return Ok(());
        };
        if sealed_form(&bytes) != Some(Form::Whole) || key.name().is_none() {
            return Ok(());
        }
        // One that does not open is left for its reader to report.
        let Ok(plain) = key.open_whole(context, &bytes) else {
            return Ok(());
        };
        let named = key.seal_whole(context, &plain)?;
        write_private(path, &named)?;
        if fs::read(path)
            .ok()
            .and_then(|b| key.open_whole(context, &b).ok())
            .as_deref()
            .map(|b| &b[..])
            != Some(&plain[..])
        {
            write_private(path, &bytes)?;
            bail!("{} did not read back once sealed again", path.display());
        }
        Ok(())
    }

    /// Copies a plain file of the folder, `bytes`, to [`BACKUP_DIR`] before it is sealed.
    fn back_up(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let relative = path
            .strip_prefix(&self.dir)
            .with_context(|| format!("{} is not in the data folder", path.display()))?;
        let copy = self.dir.join(BACKUP_DIR).join(relative);
        let mut made = self.dir.clone();
        for part in Path::new(BACKUP_DIR)
            .join(relative)
            .parent()
            .into_iter()
            .flatten()
        {
            made.push(part);
            if !made.exists() {
                #[cfg(test)]
                faults::check()?;
                fs::create_dir(&made)
                    .with_context(|| format!("cannot create {}", made.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&made, fs::Permissions::from_mode(0o700));
                }
            }
        }
        write_private(&copy, bytes)?;
        self.backed_up.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Removes the plain copies left by sealing (those of the identity, the friends file and
    /// the MLS state, and all of them when sealing was cut short), at a start after the one
    /// that found every secret sealed ([`SealPhase::Done`]), once the folder opened whole
    /// with the key the keychain gave again.
    pub fn drop_backup(&self) {
        if self.backed_up.load(Ordering::Relaxed)
            || !self.done_before.load(Ordering::Relaxed)
            || !self.sealed()
        {
            return;
        }
        self.remove_backup();
    }

    fn remove_backup(&self) {
        remove_copies(&self.dir.join(BACKUP_DIR));
    }

    /// Removes the plain copies of the direct chats made while sealing.
    fn remove_backup_of_chats(&self) {
        remove_copies(&self.dir.join(BACKUP_DIR).join("direct"));
    }

    /// Whether the folder holds nothing of an account: no identity and no friends file.
    fn holds_nothing(&self) -> bool {
        [self.identity_path(), self.friends_path()]
            .iter()
            .all(|p| fs::symlink_metadata(p).is_err())
    }

    /// Asks the keychain for the folder's key (making one for a folder that may be sealed),
    /// and says how its secrets are kept. Nothing secret is written.
    pub fn sealing(&self) -> Result<Sealing> {
        match self.data_key() {
            Ok(Some(_)) if self.sealed() => Ok(Sealing::Sealed),
            Ok(_) => Ok(match self.unfinished() {
                Some(why) => Sealing::Unfinished(why),
                None => Sealing::Plain,
            }),
            Err(e) => match self
                .refused
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
            {
                Some(why) => Ok(Sealing::NoKeychain(why)),
                None => Err(e),
            },
        }
    }

    /// Whether the folder's data is sealed with a key from the keychain: every secret in it
    /// was found sealed and opening with the key ([`SealPhase::Done`]).
    pub fn sealed(&self) -> bool {
        matches!(self.key.get(), Some(Ok(Some(_))))
            && *self.phase.lock().unwrap_or_else(PoisonError::into_inner) == Some(SealPhase::Done)
    }

    /// The secret that keeps sealing from being checked to its end, sealed and not opening
    /// with the folder's key, if that is what keeps it.
    pub fn damaged(&self) -> Option<PathBuf> {
        self.unfinished()?;
        let Some(Some(key)) = self.known_key() else {
            return None;
        };
        let checked = self.check_sealed(key).err()?;
        checked
            .chain()
            .find_map(|e| e.downcast_ref::<DoesNotOpen>())
            .map(|d| d.0.clone())
    }

    /// Notes that the damaged secret [`Store::damaged`] names goes aside at the next start,
    /// before anything reads it: put aside as `.damaged` (still sealed, never deleted), and
    /// in its place the plain copy made of it before sealing, when there is one, which that
    /// start then seals. Sealing can then be checked to its end, and the plain copies go as
    /// usual. What the copy holds is the file as it was when the folder was sealed.
    pub fn set_aside_at_next_start(&self, path: &Path) -> Result<()> {
        if self.damaged().as_deref() != Some(path) {
            bail!("{} is not the damaged file", path.display());
        }
        let relative = path
            .strip_prefix(&self.dir)
            .with_context(|| format!("{} is not in the data folder", path.display()))?;
        // Without its plain copy the account would be left with no identity at all.
        if path == self.identity_path() && !self.dir.join(BACKUP_DIR).join(relative).is_file() {
            bail!("the identity does not open, and no copy of it from before sealing is left");
        }
        let Some(Some(key)) = self.known_key() else {
            bail!("the folder's key is not known");
        };
        let note = serde_json::to_vec(&relative.to_string_lossy())?;
        write_private(
            &self.dir.join(SET_ASIDE_FILE),
            &key.seal_whole(SET_ASIDE_CONTEXT, &note)?,
        )
    }

    /// Does what [`Store::set_aside_at_next_start`] noted, once: as the store opens, before
    /// anything reads the folder's secrets.
    fn set_aside_marked(&self, key: &DataKey) {
        let marker = self.dir.join(SET_ASIDE_FILE);
        let Ok(bytes) = fs::read(&marker) else {
            return;
        };
        match self.set_aside(key, &bytes) {
            Ok(()) => {}
            Err(e) => tracing::warn!("a damaged file was not set aside: {e:#}"),
        }
        // Done, or nothing to do: the person is asked again if the file is still damaged.
        let _ = fs::remove_file(&marker);
    }

    /// What [`Store::set_aside_marked`] does with the marker's `bytes`. Nothing changes
    /// unless the file named still does not open with `key` now: one written again since
    /// (a friends file saved meanwhile, a chat that took new records) is not damaged any
    /// more. The damaged file is kept aside first (a link, or a copy), and only then is the
    /// plain copy from before sealing put in its place, sealed, in one step, so the folder never
    /// lacks the file. Only the identity and the friends file are taken back so: an MLS
    /// state from before sealing would bring back an epoch, and keys, the group left behind,
    /// and chats keep no plain copy once sealed; those are only set aside.
    fn set_aside(&self, key: &DataKey, bytes: &[u8]) -> Result<()> {
        let note = key
            .open_whole(SET_ASIDE_CONTEXT, bytes)
            .context("the note was not written with the folder's key")?;
        let relative: String = serde_json::from_slice(&note)?;
        let relative = Path::new(&relative);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || relative.starts_with(BACKUP_DIR)
        {
            bail!("{} is not in the data folder", relative.display());
        }
        let path = self.dir.join(relative);
        let still_damaged = self.check_sealed(key).err().is_some_and(|e| {
            e.chain()
                .filter_map(|e| e.downcast_ref::<DoesNotOpen>())
                .any(|d| d.0 == path)
        });
        if !still_damaged {
            bail!("{} opens with its key now", path.display());
        }
        let copy = self.dir.join(BACKUP_DIR).join(relative);
        let takes_back = [
            self.identity_path(),
            self.friends_path(),
            self.friends_copy_path(),
        ]
        .contains(&path);
        let plain = if takes_back {
            fs::read(&copy).ok().map(Zeroizing::new)
        } else {
            None
        };
        // Sealed as it is put in place: a later start refuses a plain secret.
        let context = if path == self.identity_path() {
            "identity"
        } else {
            "friends"
        };
        if path == self.identity_path() && plain.is_none() {
            bail!("the identity does not open, and no copy of it from before sealing is left");
        }
        let aside = aside_path(&path);
        match &plain {
            Some(plain) => {
                // Kept aside before anything replaces it, then replaced in one step.
                if fs::hard_link(&path, &aside).is_err() {
                    copy_aside(&path, &aside)
                        .with_context(|| format!("cannot keep {} aside", path.display()))?;
                }
                write_private(&path, &key.seal_whole(context, plain)?)?;
                tracing::warn!(
                    "{} did not open with its key: kept as {}, and taken back from before \
                     sealing",
                    path.display(),
                    aside.display()
                );
            }
            None => {
                fs::rename(&path, &aside)
                    .with_context(|| format!("cannot set {} aside", path.display()))?;
                tracing::warn!(
                    "{} did not open with its key: set aside as {}",
                    path.display(),
                    aside.display()
                );
            }
        }
        Ok(())
    }

    /// Why the folder is not all sealed, when sealing it began (or it has a key) and did not
    /// end: what failed, or that a start stopped half way.
    pub fn unfinished(&self) -> Option<String> {
        if self.sealed() {
            return None;
        }
        match &*self.phase.lock().unwrap_or_else(PoisonError::into_inner) {
            Some(SealPhase::Failed { why }) => Some(why.clone()),
            Some(SealPhase::Pending) => Some("sealing stopped half way".into()),
            _ if matches!(self.key.get(), Some(Ok(Some(_)))) => {
                Some("sealing was not checked to the end".into())
            }
            _ => None,
        }
    }

    /// Whether this process seals a plain folder when the keychain keeps a key for it
    /// ([`Keychain::Seal`]): a plain folder then had no keychain to keep one.
    pub fn may_seal(&self) -> bool {
        matches!(self.keychain, Keychain::Seal(_))
    }
}

/// Removes the refused plain file at `path` once it is back in its place, sealed: written
/// over first, as an account move does with what it leaves behind.
fn remove_taken_back(path: &Path) -> Result<()> {
    crate::transfer::wipe(path)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };

    use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId, at_rest::DataKey};
    use fogata_friends::FriendBook;

    use super::*;
    use crate::{
        direct::{
            model::{Body, Chat, Item},
            store::{DirectStore, Record},
        },
        vault::{KeyStore, MemoryKeyStore},
    };

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-seal-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn message(id: u8, text: &str) -> Record {
        Record::Item {
            item: Item {
                id: [id; 16],
                mine: id.is_multiple_of(2),
                sent_ms: u64::from(id),
                got_ms: u64::from(id),
                seq: u64::from(id),
                at_ms: u64::from(id),
                body: Body::Message {
                    text: text.into(),
                    files: Vec::new(),
                },
                ttl_ms: 0,
                delivered: true,
            },
        }
    }

    /// A plain folder as an earlier version left it: an identity, a profile with its
    /// copy, and a chat with a journal and the friend's profile.
    fn plain_folder(dir: &Path) -> (DevicePublicKey, DevicePublicKey, Chat) {
        let store = Store::open_with(dir.to_path_buf(), Keychain::Off).unwrap();
        let (me, _) = store.identity().unwrap();
        let book = FriendBook::new(&me, "Marta", 1).unwrap();
        store.save_friends(&book).unwrap();
        let mut renamed = book.clone();
        renamed.set_profile(&me, Some("Marta R"), None, 2).unwrap();
        store.save_friends(&renamed).unwrap();
        let friend = DeviceIdentity::generate().unwrap().public_key();
        let chats = DirectStore::open(dir);
        let mut chat = Chat::default();
        let records = [message(1, "hi Leo"), message(2, "how are you")];
        for r in &records {
            r.apply(&mut chat, &mut None);
        }
        chats.append(&friend, &records, &chat).unwrap();
        chats.save_profile(&friend, b"profile of Leo").unwrap();
        (me.public_key(), friend, chat)
    }

    /// A secret that does not open with the key keeps sealing from its end, and its
    /// plain copy in the clear: named, it goes aside at the next start (kept, sealed),
    /// its plain copy from before sealing takes its place sealed, and sealing ends.
    /// A file marked to set aside that opens again by the next start (the friends file
    /// saved meanwhile) is left as it is: nothing newer is replaced by the old copy.
    #[test]
    fn a_marked_file_that_opens_again_is_left_as_it_is() {
        let dir = folder("healed");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let open = || Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let store = open();
        store.identity().unwrap();
        let copy = store.friends_copy_path();
        drop(store);
        let mut bytes = fs::read(&copy).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 1;
        fs::write(&copy, &bytes).unwrap();
        fs::remove_file(dir.join(SEALING_FILE)).unwrap();
        let store = open();
        let (me, _) = store.identity().unwrap();
        store.set_aside_at_next_start(&copy).unwrap();
        // Saved twice before the next start: the copy is whole, and newer.
        let mut book = store.friends().unwrap().unwrap();
        book.set_profile(&me, Some("Marta Healed"), None, 99)
            .unwrap();
        store.save_friends(&book).unwrap();
        store.save_friends(&book).unwrap();
        let healed = fs::read(&copy).unwrap();
        drop(store);
        let store = open();
        store.identity().unwrap();
        assert_eq!(fs::read(&copy).unwrap(), healed, "replaced by the old copy");
        let name = copy.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!copy.with_file_name(format!("{name}.damaged")).exists());
        assert!(!dir.join(SET_ASIDE_FILE).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A group's MLS state set aside is not taken back from before sealing: that would
    /// bring back an epoch, and its keys, the group left behind.
    #[test]
    fn a_damaged_mls_state_is_set_aside_without_taking_an_old_one_back() {
        let dir = folder("mls-aside");
        plain_folder(&dir);
        let group = GroupId::from_bytes(&[7; 16]).unwrap();
        {
            let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
            store.save_mls_state(&group, b"epoch one").unwrap();
        }
        let keys = Arc::new(MemoryKeyStore::default());
        let open = || Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let store = open();
        store.identity().unwrap();
        assert!(store.sealed());
        let state = store.mls_path(&group);
        drop(store);
        let mut bytes = fs::read(&state).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 1;
        fs::write(&state, &bytes).unwrap();
        fs::remove_file(dir.join(SEALING_FILE)).unwrap();
        let store = open();
        store.identity().unwrap();
        assert_eq!(store.damaged(), Some(state.clone()));
        store.set_aside_at_next_start(&state).unwrap();
        drop(store);
        let store = open();
        store.identity().unwrap();
        assert!(!state.exists(), "an old MLS state was taken back");
        let name = state.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            fs::read(state.with_file_name(format!("{name}.damaged"))).unwrap(),
            bytes
        );
        assert!(store.sealed(), "{:?}", store.unfinished());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_secret_set_aside_lets_sealing_end() {
        let dir = folder("damaged");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let open = || Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let store = open();
        store.identity().unwrap();
        assert!(store.sealed());
        let copy = store.friends_copy_path();
        drop(store);
        // A bit flipped in the sealed copy of the friends file, and a start that checks
        // the folder whole again.
        let mut bytes = fs::read(&copy).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 1;
        fs::write(&copy, &bytes).unwrap();
        fs::remove_file(dir.join(SEALING_FILE)).unwrap();
        let store = open();
        store.identity().unwrap();
        assert!(!store.sealed());
        assert_eq!(store.damaged(), Some(copy.clone()));
        let relative = copy.strip_prefix(&dir).unwrap();
        assert!(dir.join(BACKUP_DIR).join(relative).is_file());
        // Only the file named is set aside.
        assert!(
            store
                .set_aside_at_next_start(&store.identity_path())
                .is_err()
        );
        store.set_aside_at_next_start(&copy).unwrap();
        drop(store);
        let store = open();
        store.identity().unwrap();
        assert!(store.sealed(), "{:?}", store.unfinished());
        assert_eq!(store.damaged(), None);
        assert!(!dir.join(SET_ASIDE_FILE).exists());
        // Kept aside as it was, and the copy back, sealed, opening with the key.
        let name = copy.file_name().unwrap().to_string_lossy().into_owned();
        let aside = copy.with_file_name(format!("{name}.damaged"));
        assert_eq!(fs::read(&aside).unwrap(), bytes);
        assert!(sealed_form(&fs::read(&copy).unwrap()).is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    /// A note to set a damaged secret aside that was not written with the folder's key
    /// (someone who damaged the sealed identity and left a plain one of theirs as its copy
    /// from before sealing) changes nothing: the identity stays as it is, and its "copy"
    /// is never taken in.
    #[test]
    fn a_note_to_set_aside_written_without_the_key_is_ignored() {
        let dir = folder("note-planted");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let open = || Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let store = open();
        store.identity().unwrap();
        assert!(store.sealed());
        let identity = store.identity_path();
        drop(store);
        // The start after the one that sealed the folder drops the copies made before.
        let store = open();
        store.identity().unwrap();
        store.drop_backup();
        drop(store);
        assert!(!dir.join(BACKUP_DIR).exists());
        let mut damaged = fs::read(&identity).unwrap();
        let middle = damaged.len() / 2;
        damaged[middle] ^= 1;
        fs::write(&identity, &damaged).unwrap();
        let theirs = hex::encode(DeviceIdentity::generate().unwrap().secret_bytes());
        fs::create_dir_all(dir.join(BACKUP_DIR)).unwrap();
        fs::write(dir.join(BACKUP_DIR).join("identity.key"), &theirs).unwrap();
        fs::write(
            dir.join(SET_ASIDE_FILE),
            serde_json::to_vec("identity.key").unwrap(),
        )
        .unwrap();
        let store = open();
        assert!(store.identity().is_err());
        assert_eq!(fs::read(&identity).unwrap(), damaged);
        assert!(!dir.join(SET_ASIDE_FILE).exists());
        drop(store);
        let _ = fs::remove_dir_all(&dir);
    }

    /// A damaged secret is set aside under another name when a link is left where it
    /// would go: nothing is written where the link points.
    #[cfg(unix)]
    #[test]
    fn a_secret_is_never_set_aside_through_a_link() {
        use crate::store::with_suffix;

        let dir = folder("aside-link");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let open = || Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let store = open();
        store.identity().unwrap();
        let copy = store.friends_copy_path();
        drop(store);
        let mut bytes = fs::read(&copy).unwrap();
        let middle = bytes.len() / 2;
        bytes[middle] ^= 1;
        fs::write(&copy, &bytes).unwrap();
        fs::remove_file(dir.join(SEALING_FILE)).unwrap();
        let store = open();
        store.identity().unwrap();
        store.set_aside_at_next_start(&copy).unwrap();
        drop(store);
        let outside = folder("aside-link-outside");
        fs::create_dir_all(&outside).unwrap();
        let target = outside.join("written");
        std::os::unix::fs::symlink(&target, with_suffix(&copy, ".damaged")).unwrap();
        let store = open();
        store.identity().unwrap();
        assert!(store.sealed(), "{:?}", store.unfinished());
        assert!(!target.exists(), "written through the link");
        assert_eq!(fs::read(with_suffix(&copy, ".damaged.2")).unwrap(), bytes);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&outside);
    }

    fn files_of(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut todo = vec![dir.to_path_buf()];
        while let Some(d) = todo.pop() {
            for e in fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    todo.push(e.path());
                } else {
                    out.insert(e.path(), fs::read(e.path()).unwrap());
                }
            }
        }
        out
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn a_folder_says_whether_it_is_sealed_or_had_no_keychain() {
        // A keychain that keeps the key: sealed.
        let dir = folder("state-sealed");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys)).unwrap();
        store.identity().unwrap();
        assert!(store.sealed() && store.may_seal());
        // None that answers: plain, though it would have been sealed.
        let dir = folder("state-none");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        keys.unreachable.store(true, Ordering::Relaxed);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys)).unwrap();
        store.identity().unwrap();
        assert!(!store.sealed() && store.may_seal());
        // Set to leave plain folders plain (tests): plain, as asked.
        let dir = folder("state-plain");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Open(keys)).unwrap();
        store.identity().unwrap();
        assert!(!store.sealed() && !store.may_seal());
        for name in ["state-sealed", "state-none", "state-plain"] {
            let _ = fs::remove_dir_all(folder(name));
        }
    }

    #[test]
    fn a_plain_folder_is_sealed_and_reads_back_whole() {
        let dir = folder("migrate");
        let (me, friend, chat) = plain_folder(&dir);
        let plain = files_of(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        // Asking whether there is a profile does not need the keychain.
        keys.unreachable.store(true, Ordering::Relaxed);
        assert!(store.has_profile().unwrap());
        keys.unreachable.store(false, Ordering::Relaxed);

        let (identity, made) = store.identity().unwrap();
        assert!(!made);
        assert_eq!(identity.public_key(), me);
        assert!(store.sealed());
        assert_eq!(store.friends().unwrap().unwrap().profile.name, "Marta R");
        let key = store.data_key().unwrap().unwrap();
        let chats = DirectStore::open_sealed(&dir, Some(key));
        let loaded = chats.load(&friend).unwrap();
        assert_eq!(loaded.chat, chat);
        assert_eq!(loaded.profile.as_deref(), Some(&b"profile of Leo"[..]));

        // Every secret is sealed, and nothing said in the chats shows anywhere in the
        // folder: their plain copies made while sealing went once all of it read back
        // sealed, and do not keep, days later, what disappeared from them meanwhile.
        let chat_dir = dir.join("direct").join(hex::encode(friend.as_bytes()));
        for path in [
            dir.join("identity.key"),
            dir.join("friends.bin"),
            dir.join("friends.bin.bak"),
            chat_dir.join("journal"),
            chat_dir.join("profile"),
        ] {
            let bytes = fs::read(&path).unwrap();
            assert!(sealed_form(&bytes).is_some(), "{}", path.display());
            assert_ne!(bytes, plain[&path], "{}", path.display());
        }
        assert!(!dir.join(BACKUP_DIR).join("direct").exists());
        let identity_hex = hex::encode(*identity.secret_bytes());
        let shown = |what: &[u8]| -> Vec<PathBuf> {
            files_of(&dir)
                .into_iter()
                .filter(|(_, bytes)| contains(bytes, what))
                .map(|(path, _)| path)
                .collect()
        };
        assert_eq!(shown(b"hi Leo"), Vec::<PathBuf>::new());
        // The identity keeps its plain copy until a later start opens the folder whole,
        // not the one that sealed it.
        assert_eq!(
            shown(identity_hex.as_bytes()),
            vec![dir.join(BACKUP_DIR).join("identity.key")]
        );
        store.drop_backup();
        assert!(dir.join(BACKUP_DIR).join("identity.key").exists());
        let again = Store::open_with(dir.clone(), Keychain::Open(keys.clone())).unwrap();
        assert_eq!(again.identity().unwrap().0.public_key(), me);
        again.drop_backup();
        assert!(!dir.join(BACKUP_DIR).exists());
        assert_eq!(shown(identity_hex.as_bytes()), Vec::<PathBuf>::new());
        assert_eq!(keys.names().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A keychain that gives each key back once, when it is read after being kept, and
    /// then loses it (reset, or an item tied to a program's signature that changed).
    #[derive(Debug, Default)]
    struct Forgetful(MemoryKeyStore);

    impl KeyStore for Forgetful {
        fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
            let got = self.0.get(name)?;
            self.0.delete(name)?;
            Ok(got)
        }
        fn set(&self, name: &str, secret: &str) -> Result<(), String> {
            self.0.set(name, secret)
        }
        fn delete(&self, name: &str) -> Result<(), String> {
            self.0.delete(name)
        }
    }

    #[test]
    fn a_keychain_that_loses_the_key_leaves_the_identity_and_friends_in_their_copies() {
        let dir = folder("forgetful");
        let (me, _, _) = plain_folder(&dir);
        let plain = files_of(&dir);
        let keys = Arc::new(Forgetful::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), me);
        assert!(store.sealed());
        // The start that sealed it ends: its own copies stay.
        store.drop_backup();
        drop(store);
        assert!(keys.0.names().is_empty(), "the keychain kept the key");

        // The next start cannot open the folder, and the account is still in the plain
        // copies; the chats' copies went, as their messages may disappear.
        let next = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let error = next.identity().map(drop).unwrap_err();
        assert!(format!("{error:#}").contains("does not hold"), "{error:#}");
        next.drop_backup();
        let backup = dir.join(BACKUP_DIR);
        for name in ["identity.key", "friends.bin", "friends.bin.bak"] {
            assert_eq!(
                fs::read(backup.join(name)).unwrap(),
                plain[&dir.join(name)],
                "{name}"
            );
        }
        assert!(!backup.join("direct").exists());
        for (path, bytes) in files_of(&dir) {
            assert!(!contains(&bytes, b"hi Leo"), "{}", path.display());
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mls_state_is_sealed_for_its_group_and_goes_whole() {
        let dir = folder("mls");
        let groups = [
            GroupId::from_bytes(&[1; 16]).unwrap(),
            GroupId::from_bytes(&[2; 16]).unwrap(),
        ];
        let secret = b"the secrets of an MLS epoch".to_vec();
        // A plain folder keeps it plain, and seals it with the rest.
        let plain = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        plain.identity().unwrap();
        assert_eq!(plain.mls_state(&groups[0]).unwrap(), None);
        plain.save_mls_state(&groups[0], &secret).unwrap();
        let path = dir
            .join(MLS_DIR)
            .join(format!("{}.bin", hex::encode([1u8; 16])));
        assert_eq!(fs::read(&path).unwrap(), secret);
        drop(plain);
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(
            store.mls_state(&groups[0]).unwrap().map(|s| s.to_vec()),
            Some(secret.to_vec())
        );
        assert!(store.sealed());
        let sealed = fs::read(&path).unwrap();
        assert!(sealed_form(&sealed).is_some());
        assert!(!contains(&sealed, &secret));
        // Written again, the file holds only the new state; another group's file in its
        // place does not open.
        store.save_mls_state(&groups[0], b"newer").unwrap();
        store.save_mls_state(&groups[1], b"other group").unwrap();
        assert_eq!(
            store.mls_state(&groups[0]).unwrap().map(|s| s.to_vec()),
            Some(b"newer".to_vec())
        );
        let other = dir
            .join(MLS_DIR)
            .join(format!("{}.bin", hex::encode([2u8; 16])));
        fs::copy(&other, &path).unwrap();
        assert!(store.mls_state(&groups[0]).is_err());
        store.remove_mls_state(&groups[0]).unwrap();
        store.remove_mls_state(&groups[0]).unwrap();
        assert_eq!(store.mls_state(&groups[0]).unwrap(), None);
        assert_eq!(
            store.mls_state(&groups[1]).unwrap().map(|s| s.to_vec()),
            Some(b"other group".to_vec())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sealing_cut_short_goes_on_at_the_next_start() {
        let dir = folder("cut");
        let (me, friend, chat) = plain_folder(&dir);
        let plain = files_of(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let first = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let key = first.data_key().unwrap().unwrap();
        // As if the process had stopped once the identity and the friends file were
        // sealed: the rest still plain, the copies made so far still aside, and sealing
        // noted as begun.
        first.write_phase(&SealPhase::Pending, Some(&key)).unwrap();
        drop(first);
        let chat_dir = dir.join("direct").join(hex::encode(friend.as_bytes()));
        let backup = dir.join(BACKUP_DIR);
        for path in [dir.join("friends.bin.bak"), chat_dir.join("journal")] {
            fs::write(&path, &plain[&path]).unwrap();
        }
        fs::create_dir_all(&backup).unwrap();
        fs::write(
            backup.join("identity.key"),
            &plain[&dir.join("identity.key")],
        )
        .unwrap();
        assert!(sealed_form(&fs::read(chat_dir.join("journal")).unwrap()).is_none());

        // The next start finishes the work, also one that only opens sealed folders
        // (the folder is sealed already), and what was plain reads the same.
        let store = Store::open_with(dir.clone(), Keychain::Open(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), me);
        let key = store.data_key().unwrap();
        for path in [dir.join("friends.bin.bak"), chat_dir.join("journal")] {
            assert!(
                sealed_form(&fs::read(&path).unwrap()).is_some(),
                "{}",
                path.display()
            );
        }
        assert_eq!(
            DirectStore::open_sealed(&dir, key)
                .load(&friend)
                .unwrap()
                .chat,
            chat
        );
        // Sealed whole now: the plain copies of the chats are gone; the others, of this
        // start and the one before, go at the next start that opens the folder whole.
        assert!(!backup.join("direct").exists());
        assert!(backup.join("identity.key").exists());
        store.drop_backup();
        assert!(backup.exists());
        let next = Store::open_with(dir.clone(), Keychain::Open(keys.clone())).unwrap();
        assert_eq!(next.identity().unwrap().0.public_key(), me);
        next.drop_backup();
        assert!(!backup.exists());
        assert_eq!(keys.names().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The secrets of a folder made by [`plain_folder`] and given an MLS state.
    fn secrets(dir: &Path, friend: &DevicePublicKey) -> Vec<PathBuf> {
        let chat_dir = dir.join("direct").join(hex::encode(friend.as_bytes()));
        vec![
            dir.join("identity.key"),
            dir.join("friends.bin"),
            dir.join("friends.bin.bak"),
            dir.join(MLS_DIR)
                .join(format!("{}.bin", hex::encode([3u8; 16]))),
            chat_dir.join("journal"),
            chat_dir.join("profile"),
        ]
    }

    fn is_sealed(path: &Path) -> bool {
        sealed_form(&fs::read(path).unwrap()).is_some()
    }

    /// [`plain_folder`] with the MLS state of a group too.
    fn plain_folder_with_mls(dir: &Path) -> (DevicePublicKey, DevicePublicKey, Chat) {
        let made = plain_folder(dir);
        let store = Store::open_with(dir.to_path_buf(), Keychain::Off).unwrap();
        let group = GroupId::from_bytes(&[3; 16]).unwrap();
        store
            .save_mls_state(&group, b"the secrets of an epoch")
            .unwrap();
        made
    }

    /// Opens the folder as the application does, and checks it holds the account whole.
    fn opens_whole(
        dir: &Path,
        keys: &Arc<MemoryKeyStore>,
        made: &(DevicePublicKey, DevicePublicKey, Chat),
    ) -> Store {
        let (me, friend, chat) = made;
        let store = Store::open_with(dir.to_path_buf(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), *me);
        assert_eq!(store.friends().unwrap().unwrap().profile.name, "Marta R");
        let group = GroupId::from_bytes(&[3; 16]).unwrap();
        assert_eq!(
            store.mls_state(&group).unwrap().map(|s| s.to_vec()),
            Some(b"the secrets of an epoch".to_vec())
        );
        let chats = DirectStore::open_sealed(dir, store.data_key().unwrap());
        assert_eq!(chats.load(friend).unwrap().chat, *chat);
        store
    }

    /// A folder whose plain copies cannot be made (`before-sealing` is a file) is not
    /// sealed, and says so, start after start, until they can be.
    #[test]
    fn a_folder_whose_copies_cannot_be_made_says_it_is_not_sealed() {
        let dir = folder("no-copies");
        let made = plain_folder_with_mls(&dir);
        let plain = files_of(&dir);
        fs::write(dir.join(BACKUP_DIR), b"not a folder").unwrap();
        let keys = Arc::new(MemoryKeyStore::default());
        for _ in 0..2 {
            let store = opens_whole(&dir, &keys, &made);
            // Nothing is sealed without its copy: the identity is as it was.
            let identity = dir.join("identity.key");
            assert_eq!(fs::read(&identity).unwrap(), plain[&identity]);
            assert!(!store.sealed());
            assert!(
                matches!(store.sealing().unwrap(), Sealing::Unfinished(_)),
                "{:?}",
                store.sealing()
            );
            store.drop_backup();
        }
        // Once the copies can be made, the next start seals it all.
        fs::remove_file(dir.join(BACKUP_DIR)).unwrap();
        let store = opens_whole(&dir, &keys, &made);
        assert!(store.sealed());
        assert_eq!(store.sealing().unwrap(), Sealing::Sealed);
        for path in secrets(&dir, &made.1) {
            assert!(is_sealed(&path), "{}", path.display());
        }
        store.drop_backup();
        assert!(dir.join(BACKUP_DIR).join("identity.key").exists());
        drop(store);
        opens_whole(&dir, &keys, &made).drop_backup();
        assert!(!dir.join(BACKUP_DIR).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A folder sealed by a version that noted nothing of how far sealing got, maybe cut
    /// short (a chat left plain): its first start checks it all, ends the work and says
    /// it is sealed; its copies go at the start after.
    #[test]
    fn a_folder_sealed_before_sealing_was_noted_is_checked_at_its_next_start() {
        let dir = folder("unnoted");
        let made = plain_folder_with_mls(&dir);
        let plain = files_of(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        drop(opens_whole(&dir, &keys, &made));
        let journal = secrets(&dir, &made.1)[4].clone();
        fs::write(&journal, &plain[&journal]).unwrap();
        fs::remove_file(dir.join(SEALING_FILE)).unwrap();
        let store = opens_whole(&dir, &keys, &made);
        assert!(store.sealed());
        let key = store.data_key().unwrap().unwrap();
        assert_eq!(
            store.read_phase(&key),
            Some(Noted {
                phase: SealPhase::Done,
                by_key: true
            })
        );
        assert!(secrets(&dir, &made.1).iter().all(|p| is_sealed(p)));
        store.drop_backup();
        assert!(dir.join(BACKUP_DIR).join("identity.key").exists());
        drop(store);
        opens_whole(&dir, &keys, &made).drop_backup();
        assert!(!dir.join(BACKUP_DIR).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The plain recovery key of the folders below: what it holds does not matter here.
    const RECOVERY: &[u8] = b"the recovery key of the account";

    /// The folder [`plain_folder_with_mls`] makes, with a recovery key, as 0.1.0-beta.7 or
    /// earlier left it when sealing it stopped after the identity (a full disk, a computer
    /// that stopped): the identity sealed, with its plain copy aside, the friends file, its
    /// copy, the recovery key, the MLS state and a chat's journal still plain, and sealing
    /// noted as `phase` in the clear, as those versions wrote the note. `copied` names the
    /// plain files sealing had copied aside before it stopped. Returns the plain files.
    fn cut_short_by_an_older_version(
        dir: &Path,
        keys: &Arc<MemoryKeyStore>,
        phase: &SealPhase,
        copied: &[&Path],
    ) -> (
        (DevicePublicKey, DevicePublicKey, Chat),
        BTreeMap<PathBuf, Vec<u8>>,
    ) {
        let made = plain_folder_with_mls(dir);
        fs::write(dir.join(crate::recovery::KEY_FILE), RECOVERY).unwrap();
        let plain = files_of(dir);
        drop(
            Store::open_with(dir.to_path_buf(), Keychain::Seal(keys.clone()))
                .unwrap()
                .data_key(),
        );
        let backup = dir.join(BACKUP_DIR);
        fs::remove_dir_all(&backup).unwrap();
        let identity = dir.join("identity.key");
        let mut left_plain = secrets(dir, &made.1);
        left_plain.retain(|p| *p != identity && !p.ends_with("profile"));
        left_plain.push(dir.join(crate::recovery::KEY_FILE));
        for path in &left_plain {
            fs::write(path, &plain[path]).unwrap();
        }
        for path in std::iter::once(identity.as_path()).chain(copied.iter().copied()) {
            let copy = backup.join(path.strip_prefix(dir).unwrap());
            fs::create_dir_all(copy.parent().unwrap()).unwrap();
            fs::write(copy, &plain[path]).unwrap();
        }
        // Exactly what those versions' `write_phase` wrote.
        fs::write(
            dir.join(SEALING_FILE),
            serde_json::to_vec_pretty(phase).unwrap(),
        )
        .unwrap();
        assert!(is_sealed(&identity));
        (made, plain)
    }

    /// The paths of the folder's friends file, its copy and the group's MLS state.
    fn account_files(dir: &Path) -> [PathBuf; 3] {
        [
            dir.join("friends.bin"),
            dir.join("friends.bin.bak"),
            dir.join(MLS_DIR)
                .join(format!("{}.bin", hex::encode([3u8; 16]))),
        ]
    }

    /// A folder whose sealing an older version cut short, noted in the clear as begun or
    /// failed, is sealed whole at its first start by this one: the plain secrets that
    /// sealing had copied aside are taken as those copies hold them, and those it never
    /// reached (no copy) are copied and sealed now. The account opens whole, with its
    /// friends; a recovery key without its copy is refused (copies are turned on again).
    #[test]
    fn a_folder_whose_sealing_an_older_version_cut_short_is_sealed_whole() {
        for (n, (phase, copied_all)) in [
            (SealPhase::Pending, true),
            (SealPhase::Pending, false),
            (
                SealPhase::Failed {
                    why: "No space left on device (os error 28)".into(),
                },
                false,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let dir = folder(&format!("older-cut-{n}"));
            let keys = Arc::new(MemoryKeyStore::default());
            let files = account_files(&dir);
            let recovery = dir.join(crate::recovery::KEY_FILE);
            let mut copied: Vec<&Path> = vec![&files[0]];
            if copied_all {
                copied.extend([files[1].as_path(), files[2].as_path(), recovery.as_path()]);
            }
            let (made, _) = cut_short_by_an_older_version(&dir, &keys, &phase, &copied);
            // Asked first, as the application does: the profile is there.
            let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
            assert!(store.has_profile().unwrap(), "{n}");
            drop(store);
            let store = opens_whole(&dir, &keys, &made);
            assert!(store.sealed(), "{n}: {:?}", store.unfinished());
            for path in secrets(&dir, &made.1) {
                assert!(is_sealed(&path), "{n}: {}", path.display());
            }
            assert_eq!(store.friends_kept(), None, "{n}");
            let refused = dir.join(REFUSED_DIR);
            if copied_all {
                assert!(is_sealed(&recovery), "{n}");
                assert!(!refused.exists(), "{n}");
                assert!(!store.holds_refused());
            } else {
                assert!(!recovery.exists(), "{n}");
                assert_eq!(
                    fs::read(refused.join(crate::recovery::KEY_FILE)).unwrap(),
                    RECOVERY
                );
                assert!(store.holds_refused());
            }
            let key = store.data_key().unwrap().unwrap();
            assert_eq!(
                store.read_phase(&key),
                Some(Noted {
                    phase: SealPhase::Done,
                    by_key: true
                }),
                "{n}"
            );
            drop(store);
            // The starts after: the same account, and the plain copies go.
            opens_whole(&dir, &keys, &made).drop_backup();
            assert!(!dir.join(BACKUP_DIR).exists(), "{n}");
            opens_whole(&dir, &keys, &made);
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// The same folder, with plain files someone put there: one that is not what sealing
    /// copied aside is refused, and so is every plain secret once the identity's plain copy
    /// is gone (the start after sealing ended), or is not this identity's. A plain note
    /// alone never takes a plain secret in.
    #[test]
    fn plain_secrets_planted_beside_a_plain_note_are_still_refused() {
        let theirs = {
            let other = folder("planted-note-theirs");
            plain_folder(&other);
            let mut book =
                FriendBook::decode(&fs::read(other.join("friends.bin")).unwrap()).unwrap();
            book.profile.name = "Someone else".into();
            let bytes = book.encode().to_vec();
            fs::remove_dir_all(&other).unwrap();
            bytes
        };
        let pending = serde_json::to_vec_pretty(&SealPhase::Pending).unwrap();
        // Planted where sealing had copied the file aside.
        let dir = folder("planted-note-copied");
        let keys = Arc::new(MemoryKeyStore::default());
        let files = account_files(&dir);
        let (made, _) =
            cut_short_by_an_older_version(&dir, &keys, &SealPhase::Pending, &[&files[0]]);
        fs::write(&files[0], &theirs).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), made.0);
        assert_eq!(
            fs::read(dir.join(REFUSED_DIR).join("friends.bin")).unwrap(),
            theirs
        );
        // The rest, which sealing never reached, is sealed; the folder says where the
        // friends file is kept, rather than look like a new device's.
        assert!(is_sealed(&files[1]) && is_sealed(&files[2]));
        assert!(store.friends().unwrap().is_none());
        assert_eq!(store.friends_kept(), Some(dir.join(REFUSED_DIR)));
        // A profile made then sets the copy aside rather than write over it.
        let copy = fs::read(&files[1]).unwrap();
        let (me, _) = store.identity().unwrap();
        store
            .save_friends(&FriendBook::new(&me, "Marta new", 9).unwrap())
            .unwrap();
        assert_eq!(fs::read(dir.join("friends.bin.damaged")).unwrap(), copy);
        drop(store);
        fs::remove_dir_all(&dir).unwrap();

        // Sealed whole, and started again: the identity's plain copy is gone. A plain note
        // and plain secrets with copies of their own put back are refused.
        let dir = folder("planted-note-later");
        let made = plain_folder_with_mls(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        drop(opens_whole(&dir, &keys, &made));
        opens_whole(&dir, &keys, &made).drop_backup();
        assert!(!dir.join(BACKUP_DIR).exists());
        let files = account_files(&dir);
        let before: Vec<_> = files.iter().map(|p| fs::read(p).unwrap()).collect();
        fs::create_dir_all(dir.join(BACKUP_DIR)).unwrap();
        for path in &files[..2] {
            fs::write(path, &theirs).unwrap();
            fs::write(
                dir.join(BACKUP_DIR).join(path.file_name().unwrap()),
                &theirs,
            )
            .unwrap();
        }
        fs::write(dir.join(SEALING_FILE), &pending).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        store.identity().unwrap();
        assert_eq!(store.friends().unwrap().map(|b| b.encode()), None);
        for name in ["friends.bin", "friends.bin.bak"] {
            assert_eq!(fs::read(dir.join(REFUSED_DIR).join(name)).unwrap(), theirs);
        }
        assert_eq!(store.friends_kept(), Some(dir.join(REFUSED_DIR)));
        assert_eq!(fs::read(&files[2]).unwrap(), before[2]);
        drop(store);
        fs::remove_dir_all(&dir).unwrap();

        // A plain copy of another identity beside the sealed one proves nothing either.
        let dir = folder("planted-note-identity");
        let keys = Arc::new(MemoryKeyStore::default());
        let files = account_files(&dir);
        cut_short_by_an_older_version(&dir, &keys, &SealPhase::Pending, &[&files[0]]);
        fs::write(&files[0], &theirs).unwrap();
        fs::write(dir.join(BACKUP_DIR).join("friends.bin"), &theirs).unwrap();
        fs::write(
            dir.join(BACKUP_DIR).join("identity.key"),
            hex::encode(DeviceIdentity::generate().unwrap().secret_bytes()),
        )
        .unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        store.identity().unwrap();
        assert_eq!(
            fs::read(dir.join(REFUSED_DIR).join("friends.bin")).unwrap(),
            theirs
        );
        assert!(
            dir.join(REFUSED_DIR)
                .join(files[2].strip_prefix(&dir).unwrap())
                .exists()
        );
        drop(store);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// What 0.1.0-beta.8 did with a folder whose sealing an older version cut short: it
    /// refused its plain secrets (moved them to `refused/`), noted sealing as done with the
    /// key, and the account came up without its friends. Done again from those files.
    fn refused_by_beta8(dir: &Path, key: &DataKey, plain: &[PathBuf]) {
        for path in plain {
            if !path.exists() || is_sealed(path) {
                continue;
            }
            let to = dir.join(REFUSED_DIR).join(path.strip_prefix(dir).unwrap());
            fs::create_dir_all(to.parent().unwrap()).unwrap();
            fs::rename(path, to).unwrap();
        }
        let store = Store::open_with(dir.to_path_buf(), Keychain::Off).unwrap();
        store.write_phase(&SealPhase::Done, Some(key)).unwrap();
    }

    /// Someone who started 0.1.0-beta.8 on such a folder has its secrets in `refused/`:
    /// asking whether there is a profile takes them back, sealed, as sealing had copied them
    /// aside (or never reached them), and the account opens whole. A start that stopped
    /// between putting one back and removing it from `refused/` is ended; a friends file
    /// made since stays as it is, with the refused one kept; one that differs from its copy
    /// stays refused.
    #[test]
    fn secrets_refused_by_the_previous_version_are_taken_back() {
        let files_of_account = |dir: &Path| {
            let mut all = account_files(dir).to_vec();
            all.push(dir.join(crate::recovery::KEY_FILE));
            all
        };
        let key_of = |dir: &Path, keys: &Arc<MemoryKeyStore>| {
            vault::open(dir, &Keychain::Seal(keys.clone()))
                .unwrap()
                .unwrap()
        };
        for copied_all in [true, false] {
            let dir = folder(&format!("taken-back-{copied_all}"));
            let keys = Arc::new(MemoryKeyStore::default());
            let files = files_of_account(&dir);
            let copied: Vec<&Path> = if copied_all {
                files.iter().map(PathBuf::as_path).collect()
            } else {
                vec![&files[0]]
            };
            let (made, _) =
                cut_short_by_an_older_version(&dir, &keys, &SealPhase::Pending, &copied);
            refused_by_beta8(&dir, &key_of(&dir, &keys), &files);
            let refused = dir.join(REFUSED_DIR);
            assert!(refused.join("friends.bin").exists());
            // As the application starts: asking first.
            let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
            assert!(store.has_profile().unwrap(), "{copied_all}");
            drop(store);
            let store = opens_whole(&dir, &keys, &made);
            assert!(store.sealed());
            for path in &files[..3] {
                assert!(is_sealed(path), "{}", path.display());
            }
            if copied_all {
                assert!(is_sealed(&files[3]));
                assert!(!refused.exists());
                assert!(!store.holds_refused());
            } else {
                // A recovery key sealing never copied aside stays refused, and is told.
                assert!(!files[3].exists());
                assert_eq!(
                    fs::read(refused.join(crate::recovery::KEY_FILE)).unwrap(),
                    RECOVERY
                );
                assert!(store.holds_refused());
                assert!(!refused.join("friends.bin").exists());
                assert!(!refused.join(MLS_DIR).exists());
            }
            drop(store);
            opens_whole(&dir, &keys, &made).drop_backup();
            assert!(!dir.join(BACKUP_DIR).exists());
            opens_whole(&dir, &keys, &made);
            fs::remove_dir_all(&dir).unwrap();
        }

        // Stopped after the friends file was put back, before it left `refused/`.
        let dir = folder("taken-back-stopped");
        let keys = Arc::new(MemoryKeyStore::default());
        let files = files_of_account(&dir);
        let copied: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        let (made, plain) =
            cut_short_by_an_older_version(&dir, &keys, &SealPhase::Pending, &copied);
        let key = key_of(&dir, &keys);
        refused_by_beta8(&dir, &key, &files);
        fs::write(
            &files[0],
            key.seal_whole("friends", &plain[&files[0]]).unwrap(),
        )
        .unwrap();
        let store = opens_whole(&dir, &keys, &made);
        assert!(!dir.join(REFUSED_DIR).exists());
        assert!(!store.holds_refused());
        drop(store);
        fs::remove_dir_all(&dir).unwrap();

        // A friends file made since stays, and so does the refused one (and its copy,
        // which goes with it), told.
        let dir = folder("taken-back-made-since");
        let keys = Arc::new(MemoryKeyStore::default());
        let files = files_of_account(&dir);
        let copied: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        let (made, _) = cut_short_by_an_older_version(&dir, &keys, &SealPhase::Pending, &copied);
        let key = key_of(&dir, &keys);
        refused_by_beta8(&dir, &key, &files);
        let since = FriendBook::new(&DeviceIdentity::generate().unwrap(), "Marta again", 5)
            .unwrap()
            .encode();
        let since = key.seal_whole("friends", &since).unwrap();
        fs::write(&files[0], &since).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), made.0);
        assert_eq!(fs::read(&files[0]).unwrap(), since);
        assert_eq!(
            store.friends().unwrap().unwrap().profile.name,
            "Marta again"
        );
        let refused = dir.join(REFUSED_DIR);
        assert!(refused.join("friends.bin").exists());
        assert!(refused.join("friends.bin.bak").exists());
        assert!(!files[1].exists());
        assert!(is_sealed(&files[2]) && is_sealed(&files[3]));
        assert!(store.holds_refused());
        assert_eq!(store.friends_kept(), None);
        drop(store);
        fs::remove_dir_all(&dir).unwrap();

        // One that is not what sealing copied aside stays refused, and the folder says
        // where the profile is kept.
        let dir = folder("taken-back-differs");
        let keys = Arc::new(MemoryKeyStore::default());
        let files = files_of_account(&dir);
        let copied: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        cut_short_by_an_older_version(&dir, &keys, &SealPhase::Pending, &copied);
        refused_by_beta8(&dir, &key_of(&dir, &keys), &files);
        for name in ["friends.bin", "friends.bin.bak"] {
            fs::write(dir.join(REFUSED_DIR).join(name), b"someone else's").unwrap();
        }
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert!(!store.has_profile().unwrap());
        assert!(!files[0].exists() && !files[1].exists());
        assert_eq!(store.friends_kept(), Some(dir.join(REFUSED_DIR)));
        // The MLS state, as its copy holds it, came back.
        assert!(is_sealed(&files[2]));
        drop(store);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Sealing stopped at each of its writes in turn, as by a full disk or a computer
    /// that stops: the folder never says it is sealed while a secret in it is plain,
    /// each secret already sealed has its plain copy aside (putting the copies back
    /// gives the account as it was), and the next start ends the work.
    #[test]
    fn sealing_stopped_at_each_write_never_says_sealed_before_it_is() {
        for n in 0.. {
            let dir = folder(&format!("stop-{n}"));
            let made = plain_folder_with_mls(&dir);
            let plain = files_of(&dir);
            let keys = Arc::new(MemoryKeyStore::default());
            faults::fail_after(Some(n));
            let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
            let key = store.data_key();
            let tried = faults::tried();
            faults::fail_after(None);
            assert!(key.is_ok(), "{n}: {key:?}");
            let secrets = secrets(&dir, &made.1);
            let all_sealed = secrets.iter().all(|p| is_sealed(p));
            if store.sealed() {
                assert!(all_sealed, "{n}: said sealed with a secret plain");
                assert_eq!(store.sealing().unwrap(), Sealing::Sealed);
            } else {
                assert!(
                    matches!(store.sealing().unwrap(), Sealing::Unfinished(_)),
                    "{n}: {:?}",
                    store.sealing()
                );
                for path in secrets.iter().filter(|p| is_sealed(p)) {
                    let copy = dir.join(BACKUP_DIR).join(path.strip_prefix(&dir).unwrap());
                    assert_eq!(
                        fs::read(&copy).ok().as_ref(),
                        Some(&plain[path]),
                        "{n}: {}",
                        copy.display()
                    );
                }
                // Put back, the copies give the account as it was.
                let back = folder(&format!("stop-{n}-back"));
                let files = files_of(&dir);
                let copies = dir.join(BACKUP_DIR);
                let (aside, kept): (Vec<_>, Vec<_>) = files
                    .iter()
                    .partition(|(path, _)| path.starts_with(&copies));
                for (path, bytes) in kept.into_iter().chain(aside) {
                    let rel = path
                        .strip_prefix(&copies)
                        .or_else(|_| path.strip_prefix(&dir))
                        .unwrap();
                    if [vault::SEALED_FILE, SEALING_FILE]
                        .iter()
                        .any(|f| rel == Path::new(f))
                    {
                        continue;
                    }
                    let to = back.join(rel);
                    fs::create_dir_all(to.parent().unwrap()).unwrap();
                    fs::write(&to, bytes).unwrap();
                }
                for path in &secrets {
                    let rel = path.strip_prefix(&dir).unwrap();
                    assert_eq!(
                        fs::read(back.join(rel)).unwrap(),
                        plain[path],
                        "{n}: {}",
                        rel.display()
                    );
                }
                let restored = Store::open_with(back.clone(), Keychain::Off).unwrap();
                assert_eq!(restored.identity().unwrap().0.public_key(), made.0, "{n}");
                fs::remove_dir_all(&back).unwrap();
            }
            // The plain copies stay through the start that sealed it all.
            store.drop_backup();
            assert!(
                dir.join(BACKUP_DIR).join("identity.key").exists()
                    || !is_sealed(&dir.join("identity.key")),
                "{n}"
            );
            drop(store);
            // The next start ends the work, and the one after removes the copies.
            let next = opens_whole(&dir, &keys, &made);
            assert!(next.sealed(), "{n}");
            assert!(secrets.iter().all(|p| is_sealed(p)), "{n}");
            next.drop_backup();
            drop(next);
            opens_whole(&dir, &keys, &made).drop_backup();
            assert!(!dir.join(BACKUP_DIR).exists(), "{n}");
            assert_eq!(keys.names().len(), 1, "{n}");
            fs::remove_dir_all(&dir).unwrap();
            // Every write was stopped in turn: the copies, the seals, the records.
            if tried <= n {
                assert!(n > 2 * secrets.len(), "only {n} writes");
                eprintln!("sealing stopped at each of its {n} writes");
                break;
            }
        }
    }

    /// What [`sealing_stopped_at_each_write_never_says_sealed_before_it_is`] checks, on a
    /// real disk that fills up, with room for less and less of it each time. Run alone,
    /// with `FOGATA_SMALL_DISK` naming a small file system of its own, which it fills (a
    /// tmpfs of 1 MiB, say, mounted in a namespace of its own: `unshare -rm`).
    #[test]
    #[ignore]
    fn sealing_on_a_full_disk() {
        use std::io::Write as _;
        let Some(disk) = std::env::var_os("FOGATA_SMALL_DISK").map(PathBuf::from) else {
            panic!("FOGATA_SMALL_DISK names no small disk");
        };
        let filler = disk.join("filler");
        let mut stopped = 0;
        for room in 0..64u64 {
            let dir = disk.join("folder");
            let _ = fs::remove_dir_all(&dir);
            let _ = fs::remove_file(&filler);
            let made = plain_folder_with_mls(&dir);
            // Fill the disk, then give back `room` pages of it.
            let mut file = fs::File::create(&filler).unwrap();
            let page = [0u8; 4096];
            while file.write_all(&page).is_ok() {}
            let _ = file.sync_all();
            let full = fs::metadata(&filler).unwrap().len();
            file.set_len(full.saturating_sub(room * 4096)).unwrap();
            file.sync_all().unwrap();
            drop(file);
            let keys = Arc::new(MemoryKeyStore::default());
            let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
            let opened = store.identity();
            let secrets = secrets(&dir, &made.1);
            let sealed_at_once = store.sealed();
            if sealed_at_once {
                assert!(
                    secrets.iter().all(|p| is_sealed(p)),
                    "{room}: said sealed with a secret plain"
                );
            } else {
                stopped += 1;
                assert!(
                    matches!(store.sealing(), Ok(Sealing::Unfinished(_))),
                    "{room}: {:?}",
                    store.sealing()
                );
            }
            drop(store);
            fs::remove_file(&filler).unwrap();
            // With room again, it opens whole and ends sealing.
            assert!(opened.is_ok(), "{room}: {opened:?}");
            let next = opens_whole(&dir, &keys, &made);
            assert!(next.sealed(), "{room}");
            assert!(secrets.iter().all(|p| is_sealed(p)), "{room}");
            if sealed_at_once && stopped > 0 {
                break;
            }
        }
        assert!(stopped > 0, "the disk never ran out");
        eprintln!("sealing stopped by a full disk {stopped} times");
        let _ = fs::remove_dir_all(disk.join("folder"));
    }

    #[test]
    fn a_sealed_folder_never_opens_without_its_key_and_is_left_as_it_is() {
        let dir = folder("refused");
        let (_, friend, _) = plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        store.data_key().unwrap();
        assert!(!dir.join(BACKUP_DIR).join("direct").exists());
        let before = files_of(&dir);
        let name = keys.names().pop().unwrap();

        let refused = |keychain: Keychain, says: &str| {
            let store = Store::open_with(dir.clone(), keychain).unwrap();
            for error in [
                store.identity().map(drop).unwrap_err(),
                store.friends().map(drop).unwrap_err(),
                store.new_identity().map(drop).unwrap_err(),
            ] {
                assert!(format!("{error:#}").contains(says), "{error:#}");
            }
            // Groups, which are not sealed, still list.
            assert!(store.groups().unwrap().is_empty());
        };
        // No keychain in use, one that cannot be reached, one without the key, one with
        // another.
        refused(Keychain::Off, "not in use");
        keys.unreachable.store(true, Ordering::Relaxed);
        refused(Keychain::Seal(keys.clone()), "cannot be reached");
        keys.unreachable.store(false, Ordering::Relaxed);
        let other = DataKey::generate().unwrap();
        keys.set(&name, &hex::encode(other.as_bytes())).unwrap();
        refused(Keychain::Seal(keys.clone()), "does not open");
        keys.delete(&name).unwrap();
        refused(Keychain::Seal(keys.clone()), "does not hold");
        // A chat's journal does not read without the key either.
        assert!(DirectStore::open(&dir).load(&friend).is_err());
        assert_eq!(files_of(&dir), before, "a refused folder was changed");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copies_from_before_sealing_refuse_a_sealed_folder() {
        let dir = folder("older");
        plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        Store::open_with(dir.clone(), Keychain::Seal(keys))
            .unwrap()
            .data_key()
            .unwrap();
        // Earlier copies read the friends file as written by a later version, and the
        // identity as damaged: they stop there, before reading or writing anything else.
        assert_eq!(
            FriendBook::decode(&fs::read(dir.join("friends.bin")).unwrap()).unwrap_err(),
            fogata_friends::FriendError::NewerVersion
        );
        let identity = fs::read(dir.join("identity.key")).unwrap();
        let earlier = |bytes: &[u8]| -> Option<[u8; 32]> {
            let text = String::from_utf8(bytes.to_vec()).ok()?;
            hex::decode(text.trim()).ok()?.try_into().ok()
        };
        assert!(earlier(&identity).is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_a_keychain_the_folder_stays_plain_until_one_keeps_its_key() {
        let dir = folder("fallback");
        let (me, _, _) = plain_folder(&dir);
        let keys = Arc::new(MemoryKeyStore::default());
        keys.unreachable.store(true, Ordering::Relaxed);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), me);
        assert!(!store.sealed());
        assert!(!dir.join(vault::SEALED_FILE).exists());
        assert!(!dir.join(BACKUP_DIR).exists());
        assert!(sealed_form(&fs::read(dir.join("identity.key")).unwrap()).is_none());
        // Saved plain meanwhile.
        let mut book = store.friends().unwrap().unwrap();
        book.set_profile(&store.identity().unwrap().0, Some("Sofía"), None, 3)
            .unwrap();
        store.save_friends(&book).unwrap();
        assert!(sealed_form(&fs::read(dir.join("friends.bin")).unwrap()).is_none());

        // Another copy running on the folder: it stays as it is too.
        keys.unreachable.store(false, Ordering::Relaxed);
        let running = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let lock = running.hold_running().unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert!(store.data_key().unwrap().is_none());
        assert!(keys.names().is_empty());
        drop(lock);

        // Once the keychain can keep a key, the next start seals it.
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert!(store.data_key().unwrap().is_some());
        assert_eq!(store.friends().unwrap().unwrap().profile.name, "Sofía");
        assert!(sealed_form(&fs::read(dir.join("friends.bin")).unwrap()).is_some());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A new folder another copy of Fogata (or the command line) runs on gets no key
    /// made here, and nothing secret written in the clear either: this copy fails until
    /// the other one closes, and then seals it from its first file.
    #[test]
    fn a_new_folder_another_copy_runs_on_gets_nothing_plain() {
        let dir = folder("new-and-running");
        let keys = Arc::new(MemoryKeyStore::default());
        let running = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let lock = running.hold_running().unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let error = store.identity().map(drop).unwrap_err();
        assert!(
            format!("{error:#}").contains("already running"),
            "{error:#}"
        );
        assert!(!dir.join("identity.key").exists());
        assert!(keys.names().is_empty());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert!(store.sealing().is_err());
        // A folder whose person chose to keep it unencrypted stays as it is meanwhile.
        vault::allow_plain(&dir).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.sealing().unwrap(), Sealing::Plain);
        fs::remove_file(dir.join(vault::PLAIN_FILE)).unwrap();
        drop(lock);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        store.identity().unwrap();
        assert!(sealed_form(&fs::read(dir.join("identity.key")).unwrap()).is_some());
        keys.delete(&vault::key_name(&dir).unwrap()).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_folder_writes_nothing_plain_until_its_person_chooses_or_a_keychain_keeps_a_key() {
        let secrets = |dir: &Path| {
            ["identity.key", "friends.bin", vault::SEALED_FILE]
                .into_iter()
                .filter(|f| dir.join(f).exists())
                .collect::<Vec<_>>()
        };
        // No keychain to reach, one that is locked, one that gives another value back.
        let keys = Arc::new(MemoryKeyStore::default());
        for (case, flag) in [
            ("absent", &keys.unreachable),
            ("locked", &keys.locked),
            ("garbled", &keys.garbles),
        ] {
            let dir = folder(&format!("no-keychain-{case}"));
            flag.store(true, Ordering::Relaxed);
            let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
            assert!(
                matches!(store.sealing().unwrap(), Sealing::NoKeychain(_)),
                "{case}"
            );
            let error = store.identity().map(drop).unwrap_err();
            assert!(
                error.downcast_ref::<vault::NoKeychain>().is_some()
                    || format!("{error:#}").contains("cannot keep the key"),
                "{case}: {error:#}"
            );
            assert!(store.friends().is_err(), "{case}");
            assert_eq!(secrets(&dir), Vec::<&str>::new(), "{case}");
            assert!(keys.names().is_empty(), "{case}: a key stayed");
            // Asked again, the same.
            let again = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
            assert!(matches!(again.sealing().unwrap(), Sealing::NoKeychain(_)));
            flag.store(false, Ordering::Relaxed);
            fs::remove_dir_all(&dir).unwrap();
        }

        // The keychain works again: the folder is sealed from its first file.
        let dir = folder("no-keychain-back");
        keys.locked.store(true, Ordering::Relaxed);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert!(store.identity().is_err());
        keys.locked.store(false, Ordering::Relaxed);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.sealing().unwrap(), Sealing::Sealed);
        store.identity().unwrap();
        assert!(sealed_form(&fs::read(dir.join("identity.key")).unwrap()).is_some());
        keys.delete(&vault::key_name(&dir).unwrap()).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        // The person chose to keep it unencrypted: plain, and sealed at the first start
        // a keychain keeps a key, which forgets the choice.
        let dir = folder("no-keychain-chosen");
        keys.locked.store(true, Ordering::Relaxed);
        vault::allow_plain(&dir).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.sealing().unwrap(), Sealing::Plain);
        let (me, _) = store.identity().unwrap();
        assert!(sealed_form(&fs::read(dir.join("identity.key")).unwrap()).is_none());
        keys.locked.store(false, Ordering::Relaxed);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.sealing().unwrap(), Sealing::Sealed);
        assert_eq!(store.identity().unwrap().0.public_key(), me.public_key());
        assert!(sealed_form(&fs::read(dir.join("identity.key")).unwrap()).is_some());
        assert!(!dir.join(vault::PLAIN_FILE).exists());
        // Sealed, a keychain that lost the key is an error; no new key is made.
        let name = keys.names().pop().unwrap();
        keys.delete(&name).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert!(store.sealing().is_err());
        assert!(store.identity().is_err());
        assert!(keys.names().is_empty());
        fs::remove_dir_all(&dir).unwrap();

        // A folder left plain before (it holds an identity) goes on plain, as it was.
        let dir = folder("no-keychain-old");
        Store::open_with(dir.clone(), Keychain::Off)
            .unwrap()
            .identity()
            .unwrap();
        keys.unreachable.store(true, Ordering::Relaxed);
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.sealing().unwrap(), Sealing::Plain);
        keys.unreachable.store(false, Ordering::Relaxed);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_new_folder_is_sealed_from_its_first_file() {
        let dir = folder("new");
        let keys = Arc::new(MemoryKeyStore::default());
        let store = Store::open_with(dir.clone(), Keychain::Seal(keys)).unwrap();
        let (identity, made) = store.identity().unwrap();
        assert!(made);
        assert!(sealed_form(&fs::read(dir.join("identity.key")).unwrap()).is_some());
        let book = FriendBook::new(&identity, "Ana", 1).unwrap();
        store.save_friends(&book).unwrap();
        // Saving the same again writes nothing new, though each seal differs.
        let sealed = fs::read(dir.join("friends.bin")).unwrap();
        store.save_friends(&book).unwrap();
        assert_eq!(fs::read(dir.join("friends.bin")).unwrap(), sealed);
        assert!(sealed_form(&fs::read(dir.join("friends.bin.bak")).unwrap()).is_some());
        assert!(!dir.join(BACKUP_DIR).exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_sealed_folder_without_its_sealed_file_keeps_its_key() {
        let dir = folder("lost-sealed-file");
        let keys = Arc::new(MemoryKeyStore::default());
        let first = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let (identity, _) = first.identity().unwrap();
        let sealed_identity = fs::read(dir.join("identity.key")).unwrap();
        let name = keys.names().pop().unwrap();
        let sealed_file = fs::read(dir.join(vault::SEALED_FILE)).unwrap();

        // The identity names its key: it is found again.
        fs::remove_file(dir.join(vault::SEALED_FILE)).unwrap();
        let again = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(
            again.identity().unwrap().0.public_key(),
            identity.public_key()
        );
        assert_eq!(keys.names(), std::slice::from_ref(&name));
        assert_eq!(vault::key_name(&dir), Some(name.clone()));
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), sealed_identity);

        // A keychain without that key, or with another key under its name: an error that
        // names it, no new key, nothing overwritten.
        fs::remove_file(dir.join(vault::SEALED_FILE)).unwrap();
        let other = Arc::new(MemoryKeyStore::default());
        other.set("data-other", "00").unwrap();
        let lost = Store::open_with(dir.clone(), Keychain::Seal(other.clone())).unwrap();
        let error = format!("{:#}", lost.identity().unwrap_err());
        assert!(error.contains(&format!("does not hold {name}")), "{error}");
        assert!(error.contains(vault::SEALED_FILE), "{error}");
        let wrong = Arc::new(MemoryKeyStore::default());
        wrong
            .set(&name, &hex::encode(DataKey::generate().unwrap().as_bytes()))
            .unwrap();
        let lost = Store::open_with(dir.clone(), Keychain::Seal(wrong.clone())).unwrap();
        let error = format!("{:#}", lost.identity().unwrap_err());
        assert!(
            error.contains(&format!("{name} does not open it")),
            "{error}"
        );
        assert_eq!(wrong.names(), std::slice::from_ref(&name));
        assert_eq!(other.names(), ["data-other".to_string()]);
        assert!(!dir.join(vault::SEALED_FILE).exists());
        assert_eq!(fs::read(dir.join("identity.key")).unwrap(), sealed_identity);

        // Put back, the folder opens again.
        fs::write(dir.join(vault::SEALED_FILE), sealed_file).unwrap();
        let back = Store::open_with(dir.clone(), Keychain::Seal(keys)).unwrap();
        assert_eq!(
            back.identity().unwrap().0.public_key(),
            identity.public_key()
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A keychain that cannot list what it holds, as the system's cannot.
    #[derive(Debug, Default)]
    struct UnlistedKeyStore(MemoryKeyStore);

    impl KeyStore for UnlistedKeyStore {
        fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
            self.0.get(name)
        }

        fn set(&self, name: &str, secret: &str) -> Result<(), String> {
            self.0.set(name, secret)
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            self.0.delete(name)
        }
    }

    #[test]
    fn a_keychain_that_cannot_list_finds_the_key_of_a_folder_without_its_sealed_file() {
        let dir = folder("lost-sealed-file-unlisted");
        let keys = Arc::new(UnlistedKeyStore::default());
        let first = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        let (identity, _) = first.identity().unwrap();
        let name = keys.0.names().pop().unwrap();

        fs::remove_file(dir.join(vault::SEALED_FILE)).unwrap();
        let again = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(
            again.identity().unwrap().0.public_key(),
            identity.public_key()
        );
        assert_eq!(keys.0.names(), std::slice::from_ref(&name));
        assert_eq!(vault::key_name(&dir), Some(name));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn files_sealed_without_the_name_of_their_key_get_it_at_the_next_start() {
        let dir = folder("unnamed");
        let (me, _, _) = plain_folder(&dir);
        let keys = Arc::new(UnlistedKeyStore::default());
        let key = Store::open_with(dir.clone(), Keychain::Seal(keys.clone()))
            .unwrap()
            .data_key()
            .unwrap()
            .unwrap();
        // As folders were sealed at first: the same box, without the key's name.
        let unnamed = DataKey::from_bytes(key.as_bytes());
        for (file, context) in [("identity.key", "identity"), ("friends.bin", "friends")] {
            let path = dir.join(file);
            let plain = key.open_whole(context, &fs::read(&path).unwrap()).unwrap();
            fs::write(&path, unnamed.seal_whole(context, &plain).unwrap()).unwrap();
            assert_eq!(
                fogata_crypto::at_rest::key_name(&fs::read(&path).unwrap()),
                None
            );
        }

        let store = Store::open_with(dir.clone(), Keychain::Seal(keys.clone())).unwrap();
        assert_eq!(store.identity().unwrap().0.public_key(), me);
        let name = vault::key_name(&dir).unwrap();
        for file in ["identity.key", "friends.bin"] {
            let bytes = fs::read(dir.join(file)).unwrap();
            assert_eq!(fogata_crypto::at_rest::key_name(&bytes), Some(&name[..]));
        }
        fs::remove_file(dir.join(vault::SEALED_FILE)).unwrap();
        let again = Store::open_with(dir.clone(), Keychain::Open(keys)).unwrap();
        assert_eq!(again.identity().unwrap().0.public_key(), me);
        assert!(again.friends().unwrap().is_some());
        assert_eq!(vault::key_name(&dir), Some(name));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// A keychain as slow to keep a key as a Mac's can be on the first start.
    #[derive(Debug, Default)]
    struct SlowKeyStore(MemoryKeyStore);

    impl KeyStore for SlowKeyStore {
        fn get(&self, name: &str) -> Result<Option<Zeroizing<String>>, String> {
            self.0.get(name)
        }

        fn set(&self, name: &str, secret: &str) -> Result<(), String> {
            std::thread::sleep(Duration::from_millis(600));
            self.0.set(name, secret)
        }

        fn delete(&self, name: &str) -> Result<(), String> {
            self.0.delete(name)
        }
    }

    #[test]
    fn the_app_starts_while_another_of_its_stores_seals_the_folder() {
        // The application reads the folder through several stores as it starts; one of
        // them sealing it held the running lock, and the node's start was refused as if
        // another copy of Fogata ran on the folder.
        let dir = folder("starting");
        Store::open_with(dir.clone(), Keychain::Off)
            .unwrap()
            .identity()
            .unwrap();
        let keys = Arc::new(SlowKeyStore::default());
        let sealing = {
            let dir = dir.clone();
            std::thread::spawn(move || {
                Store::open_with(dir, Keychain::Seal(keys))
                    .unwrap()
                    .identity()
                    .unwrap()
            })
        };
        std::thread::sleep(Duration::from_millis(150));
        let running = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let lock = running.hold_running().unwrap();
        sealing.join().unwrap();
        assert!(dir.join(vault::SEALED_FILE).exists());
        // A second copy is still refused.
        let other = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        assert!(other.hold_running().is_err());
        drop(lock);
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The running lock let go of is taken again at once while another thread starts
    /// programs: each of them holds a copy of the lock for a moment as it starts, and the
    /// next start of Fogata was refused as if another copy ran (a restore's, say).
    #[cfg(unix)]
    #[test]
    fn the_running_lock_is_taken_again_while_another_thread_starts_programs() {
        use std::sync::atomic::AtomicBool;

        let dir = folder("taken-again");
        fs::create_dir_all(&dir).unwrap();
        let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let starting = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    std::process::Command::new("true").status().unwrap();
                }
            })
        };
        let started = std::time::Instant::now();
        let mut refused = None;
        while started.elapsed() < Duration::from_secs(2) {
            drop(store.hold_running().unwrap());
            if let Err(e) = store.hold_running() {
                refused = Some(format!("{e:#}"));
                break;
            }
        }
        stop.store(true, Ordering::Relaxed);
        starting.join().unwrap();
        assert_eq!(refused, None);
        // Another copy that really runs is still refused.
        let lock = store.hold_running().unwrap();
        let other = Store::open_with(dir.clone(), Keychain::Off).unwrap();
        assert!(other.hold_running().is_err());
        drop(lock);
        fs::remove_dir_all(&dir).unwrap();
    }
}
