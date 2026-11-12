//! Copies of the account that are restored without the system keychain (fogata-core's
//! `recovery`): turning them on with a recovery key the person keeps, making one, and
//! restoring one. Making and restoring a copy, and deleting the account a restore replaced,
//! need no core running on the folder: the screens stop it first
//! ([`super::app::stop_core`]) and start it again after.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, bail};
use flutter_rust_bridge::frb;
use fogata_core::{
    recovery::{self, CopyProblem, KeyProblem, NewKey, RecoveryKey, Refused, RestoreOptions},
    vault::NoKeychain,
};
use zeroize::Zeroizing;

use super::app::{blocking, use_keychain};

/// The recovery key made last and not taken yet: shown to the person once, and kept here
/// (never on the disk) until they typed two of its groups back and copies are turned on
/// with it ([`recovery_turn_on`]). Wiped from memory when it goes.
static NEW_KEY: Mutex<Option<NewKey>> = Mutex::new(None);

/// Where the folder stands with copies.
#[derive(Clone, Debug)]
pub struct UiRecoveryState {
    pub is_on: bool,
    /// The short name of the recovery key copies are made for, to compare with the one
    /// shown when it was made.
    pub key_id: Option<String>,
    /// The folder keeps the account a restore replaced.
    pub replaced_kept: bool,
}

/// A recovery key just made, to show once.
#[derive(Clone, Debug)]
pub struct UiNewRecoveryKey {
    /// The key as the person writes it down: groups of four, apart.
    pub text: String,
    /// Its short name.
    pub id: String,
}

/// Why typed text is not a recovery key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiKeyProblem {
    /// Not 53 letters and digits.
    NotAKey,
    /// 53 letters and digits that are not a key: one of them is wrong.
    Mistyped,
}

/// Why a copy does not open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiCopyProblem {
    /// Not a copy of a Fogata account.
    NotACopy,
    /// Made by a newer version of Fogata.
    NewerVersion,
    /// Made for another recovery key than the one typed.
    OtherKey,
    /// Damaged, or changed since it was made.
    Damaged,
    /// It opens with the key, but was not made by a computer that held the account.
    Unproven,
}

/// What a copy says before its key is typed, or why it does not read.
#[derive(Clone, Debug)]
pub struct UiCopyInfo {
    /// The day it was made, `YYYY-MM-DD`.
    pub made_on: String,
    /// Whether it was made for the recovery key this folder has now; `None` when copies are
    /// not on here.
    pub this_key: Option<bool>,
    pub problem: Option<UiCopyProblem>,
}

/// What a copy holds, read with its key.
#[derive(Clone, Debug)]
pub struct UiCopySummary {
    pub made_on: String,
    /// The account's name, and its device's fingerprint as people compare it.
    pub name: String,
    pub fingerprint: String,
    pub files: u32,
    pub bytes: i64,
    /// The files sent in chats came along.
    pub attachments: bool,
}

/// A copy opened with a key: what it holds, or why it does not open.
#[derive(Clone, Debug)]
pub struct UiCopyOpened {
    pub summary: Option<UiCopySummary>,
    pub key_problem: Option<UiKeyProblem>,
    pub problem: Option<UiCopyProblem>,
}

/// The account a data folder holds, as far as it reads.
#[derive(Clone, Debug)]
pub struct UiAccountHere {
    /// Its name, when its profile reads.
    pub name: Option<String>,
    /// Its device's fingerprint, when its identity reads.
    pub fingerprint: Option<String>,
}

/// Why a restore did not start: nothing changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiRestoreRefusal {
    /// The folder holds an account (`name`, when it opens), and replacing it was not asked.
    AccountHere,
    /// The account an earlier restore replaced is still kept in the folder.
    ReplacedKept,
    /// The disk has not the room the copy takes (`need` and `free` say how much).
    NoRoom,
    /// The copy is in the data folder it would go into.
    CopyInFolder,
    /// The system keychain cannot keep the key that encrypts what is restored: the person
    /// may choose to keep it unencrypted, and restore again.
    NoKeychain,
}

/// A copy restored.
#[derive(Clone, Debug)]
pub struct UiRestored {
    pub name: String,
    pub files: u32,
    /// The account the folder held was replaced, and is kept aside.
    pub replaced: bool,
}

/// How a restore went: restored, refused before anything changed, or a copy that does not
/// open.
#[derive(Clone, Debug)]
pub struct UiRestoreOutcome {
    pub restored: Option<UiRestored>,
    pub refused: Option<UiRestoreRefusal>,
    pub key_problem: Option<UiKeyProblem>,
    pub problem: Option<UiCopyProblem>,
    /// The name of the account the folder holds, for [`UiRestoreRefusal::AccountHere`].
    pub name: Option<String>,
    /// Bytes for [`UiRestoreRefusal::NoRoom`].
    pub need: i64,
    pub free: i64,
}

/// A copy made.
#[derive(Clone, Debug)]
pub struct UiExported {
    pub files: u32,
    pub bytes: i64,
}

fn key_problem(problem: KeyProblem) -> UiKeyProblem {
    match problem {
        KeyProblem::NotAKey => UiKeyProblem::NotAKey,
        KeyProblem::Mistyped => UiKeyProblem::Mistyped,
    }
}

/// The error of type `T` in `e` or anything it says it came of.
fn find<T: std::error::Error + Send + Sync + 'static>(e: &anyhow::Error) -> Option<&T> {
    e.downcast_ref::<T>()
        .or_else(|| e.chain().find_map(|c| c.downcast_ref::<T>()))
}

/// Why a copy does not open, when that is what `e` says.
fn copy_problem(e: &anyhow::Error) -> Option<UiCopyProblem> {
    find::<CopyProblem>(e).map(|p| match p {
        CopyProblem::NotACopy => UiCopyProblem::NotACopy,
        CopyProblem::NewerVersion => UiCopyProblem::NewerVersion,
        CopyProblem::OtherKey => UiCopyProblem::OtherKey,
        CopyProblem::Damaged => UiCopyProblem::Damaged,
        CopyProblem::Unproven => UiCopyProblem::Unproven,
    })
}

/// Where the folder stands with copies.
pub async fn recovery_state(data_dir: String) -> Result<UiRecoveryState> {
    use_keychain();
    blocking(move || {
        let dir = Path::new(&data_dir);
        let on = recovery::is_on(dir);
        Ok(UiRecoveryState {
            is_on: on,
            key_id: if on { recovery::key_id(dir)? } else { None },
            replaced_kept: recovery::replaced_kept(dir),
        })
    })
    .await?
}

/// Makes a new recovery key, to show once. Nothing changes until copies are turned on with
/// it ([`recovery_turn_on`]); a key made before and not taken goes.
#[frb(sync)]
pub fn recovery_new_key() -> Result<UiNewRecoveryKey> {
    let key = NewKey::generate()?;
    let made = UiNewRecoveryKey {
        // Moved out of the wiped string, not copied: the bridge takes this one to the screens.
        text: std::mem::take(&mut *key.text()),
        id: key.id(),
    };
    *NEW_KEY.lock().unwrap_or_else(|e| e.into_inner()) = Some(key);
    Ok(made)
}

/// The person did not take the key shown: it goes from memory.
#[frb(sync)]
pub fn recovery_forget_new_key() {
    NEW_KEY.lock().unwrap_or_else(|e| e.into_inner()).take();
}

/// Turns copies on, or changes their key, with the key made last, named `id`, once the
/// person showed they wrote it down. The key goes from memory once taken.
pub async fn recovery_turn_on(data_dir: String, id: String) -> Result<()> {
    use_keychain();
    let key = take_new_key(&id)?;
    blocking(move || {
        let turned = recovery::turn_on(Path::new(&data_dir), &key);
        if turned.is_err() {
            // Kept for another try, unless a newer key was made meanwhile.
            let mut held = NEW_KEY.lock().unwrap_or_else(|e| e.into_inner());
            if held.is_none() {
                *held = Some(key);
            }
        }
        turned
    })
    .await?
}

/// The key made last, if it is the one named `id`: a key shown before another was made is
/// never taken in its place.
fn take_new_key(id: &str) -> Result<NewKey> {
    let mut held = NEW_KEY.lock().unwrap_or_else(|e| e.into_inner());
    match held.as_ref() {
        Some(key) if key.id() == id => Ok(held.take().expect("checked")),
        Some(_) => bail!("this recovery key is not the one shown last: make a new one"),
        None => bail!("no recovery key is waiting: make a new one"),
    }
}

/// Why `text` is not a recovery key, as said while it is typed; `None` for one that reads.
#[frb(sync)]
pub fn recovery_key_problem(text: String) -> Option<UiKeyProblem> {
    recovery::check_key(&text).err().map(key_problem)
}

/// The name a copy made now is offered under.
#[frb(sync)]
pub fn recovery_copy_name() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    recovery::copy_name(now)
}

/// Whether `path` is inside the data folder, where a copy is never kept (it would go into
/// the next copy, and to another computer with a move), and from where none is restored.
#[frb(sync)]
pub fn recovery_in_data_folder(data_dir: String, path: String) -> bool {
    in_folder(Path::new(&data_dir), Path::new(&path))
}

fn in_folder(dir: &Path, path: &Path) -> bool {
    // A file not made yet is where its folder is.
    let at = std::fs::canonicalize(path).ok().or_else(|| {
        let parent = path.parent()?;
        Some(std::fs::canonicalize(parent).ok()?.join(path.file_name()?))
    });
    at.zip(std::fs::canonicalize(dir).ok())
        .is_some_and(|(at, dir)| at.starts_with(dir))
}

/// The bytes the files sent in chats would add to a copy.
pub async fn recovery_attachments_bytes(data_dir: String) -> Result<i64> {
    blocking(move || recovery::attachments_bytes(Path::new(&data_dir)))
        .await?
        .map(|b| b as i64)
}

/// Makes a copy of the account at `to`, with the files sent in chats when `attachments`,
/// written straight into it: under the macOS sandbox the save dialog lets the application
/// write that file alone, not one of its own beside it. No core may run on the folder.
pub async fn recovery_export(
    data_dir: String,
    to: String,
    attachments: bool,
) -> Result<UiExported> {
    use_keychain();
    let made =
        blocking(move || recovery::export_into(Path::new(&data_dir), Path::new(&to), attachments))
            .await??;
    Ok(UiExported {
        files: made.files,
        bytes: made.bytes as i64,
    })
}

/// What the copy at `copy` says before its key is typed.
pub async fn recovery_inspect(data_dir: String, copy: String) -> Result<UiCopyInfo> {
    use_keychain();
    let read = blocking(move || recovery::inspect(Path::new(&copy), Path::new(&data_dir))).await?;
    match read {
        Ok(info) => Ok(UiCopyInfo {
            made_on: info.made_on,
            this_key: info.this_key,
            problem: None,
        }),
        Err(e) => match copy_problem(&e) {
            Some(problem) => Ok(UiCopyInfo {
                made_on: String::new(),
                this_key: None,
                problem: Some(problem),
            }),
            None => Err(e),
        },
    }
}

/// Opens the copy at `copy` with the recovery key `key` and says what it holds; nothing is
/// written.
pub async fn recovery_open(copy: String, key: String) -> Result<UiCopyOpened> {
    let mut opened = UiCopyOpened {
        summary: None,
        key_problem: None,
        problem: None,
    };
    // Wiped once read, as the key it reads as is.
    let key = Zeroizing::new(key);
    let key = match RecoveryKey::parse(&key) {
        Ok(key) => key,
        Err(problem) => {
            opened.key_problem = Some(key_problem(problem));
            return Ok(opened);
        }
    };
    let read = blocking(move || recovery::open_copy(Path::new(&copy), &key)).await?;
    match read {
        Ok(s) => {
            opened.summary = Some(UiCopySummary {
                made_on: s.made_on,
                name: s.name,
                fingerprint: s.fingerprint,
                files: s.files,
                bytes: s.bytes as i64,
                attachments: s.attachments,
            })
        }
        Err(e) => opened.problem = Some(copy_problem(&e).ok_or(e)?),
    }
    Ok(opened)
}

/// The account the data folder holds, if anything of one is there: what a restore would
/// replace.
pub async fn recovery_account_in(data_dir: String) -> Result<Option<UiAccountHere>> {
    use_keychain();
    let here = blocking(move || recovery::account_in(Path::new(&data_dir))).await??;
    Ok(here.map(|h| UiAccountHere {
        name: h.name,
        fingerprint: h.fingerprint,
    }))
}

/// Restores the copy at `copy`, opened with `key`, into the data folder: replacing the
/// account there, kept aside, only when `replace`; sealed with a new key of the keychain
/// this process uses, or kept unencrypted when it cannot keep one and the person chose so
/// (`unencrypted`). No core may run on the folder.
pub async fn recovery_restore(
    data_dir: String,
    copy: String,
    key: String,
    replace: bool,
    unencrypted: bool,
) -> Result<UiRestoreOutcome> {
    use_keychain();
    // Wiped once read, as the key it reads as is.
    let key = Zeroizing::new(key);
    let key = match RecoveryKey::parse(&key) {
        Ok(key) => key,
        Err(problem) => {
            let mut out = outcome();
            out.key_problem = Some(key_problem(problem));
            return Ok(out);
        }
    };
    let options = restore_options(replace, unencrypted);
    let restored = blocking(move || {
        recovery::restore(Path::new(&data_dir), &PathBuf::from(copy), &key, &options)
    })
    .await?;
    restore_outcome(restored)
}

/// How the application restores: with the keychain the process chose, as everything else
/// it opens, and the disk asked for room first.
fn restore_options(replace: bool, unencrypted: bool) -> RestoreOptions {
    RestoreOptions {
        replace,
        keychain: None,
        free_space: Some(crate::disk::free_space),
        unencrypted,
    }
}

fn outcome() -> UiRestoreOutcome {
    UiRestoreOutcome {
        restored: None,
        refused: None,
        key_problem: None,
        problem: None,
        name: None,
        need: 0,
        free: 0,
    }
}

/// A restore's result as the screens word it: the refusals and the copy's problems typed,
/// any other failure an error.
fn restore_outcome(restored: Result<recovery::Restored>) -> Result<UiRestoreOutcome> {
    let mut out = outcome();
    let e = match restored {
        Ok(r) => {
            out.restored = Some(UiRestored {
                name: r.name,
                files: r.files,
                replaced: r.replaced,
            });
            return Ok(out);
        }
        Err(e) => e,
    };
    if let Some(refused) = find::<Refused>(&e) {
        out.refused = Some(match refused {
            Refused::AccountHere(name) => {
                out.name = name.clone();
                UiRestoreRefusal::AccountHere
            }
            Refused::ReplacedKept => UiRestoreRefusal::ReplacedKept,
            Refused::NoRoom { need, free } => {
                out.need = *need as i64;
                out.free = *free as i64;
                UiRestoreRefusal::NoRoom
            }
            Refused::CopyInFolder => UiRestoreRefusal::CopyInFolder,
        });
    } else if find::<NoKeychain>(&e).is_some() {
        out.refused = Some(UiRestoreRefusal::NoKeychain);
    } else if let Some(problem) = copy_problem(&e) {
        out.problem = Some(problem);
    } else {
        return Err(e);
    }
    Ok(out)
}

/// Deletes the account a restore replaced, kept in the folder; whether there was one. No
/// core may run on the folder.
pub async fn recovery_forget_replaced(data_dir: String) -> Result<bool> {
    use_keychain();
    blocking(move || recovery::forget_replaced(Path::new(&data_dir))).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::app::RUNTIME;
    use anyhow::Context;

    /// The key shown is kept in memory only until copies are turned on with it, and only
    /// the one shown last is taken: one shown before another was made, or forgotten, never.
    /// Both tests of the held key run in this one, which owns it.
    #[test]
    fn the_key_shown_is_held_until_it_is_taken() {
        let first = recovery_new_key().unwrap();
        assert_eq!(recovery_key_problem(first.text.clone()), None);
        let second = recovery_new_key().unwrap();
        assert_ne!(first.id, second.id);
        // The first went when the second was made.
        assert!(take_new_key(&first.id).is_err());
        let taken = take_new_key(&second.id).unwrap();
        assert_eq!(taken.id(), second.id);
        // Taken once only.
        assert!(take_new_key(&second.id).is_err());

        let third = recovery_new_key().unwrap();
        recovery_forget_new_key();
        assert!(take_new_key(&third.id).is_err());

        // A turn-on of a key that is not waiting changes nothing on the disk.
        let dir = std::env::temp_dir().join(format!(
            "fogata-ffi-recovery-{}-{}",
            std::process::id(),
            third.id.replace(' ', "")
        ));
        let turned = RUNTIME.block_on(recovery_turn_on(
            dir.to_string_lossy().into_owned(),
            third.id.clone(),
        ));
        assert!(turned.is_err());
        assert!(!dir.exists());
    }

    #[test]
    fn typed_keys_are_told_apart() {
        assert_eq!(
            recovery_key_problem("not a key".into()),
            Some(UiKeyProblem::NotAKey)
        );
        let key = NewKey::generate().unwrap();
        let text = key.text().to_string();
        // One character changed: the check character tells it.
        let mut wrong: Vec<char> = text.chars().collect();
        wrong[0] = if wrong[0] == 'A' { 'B' } else { 'A' };
        let wrong: String = wrong.into_iter().collect();
        assert_eq!(recovery_key_problem(wrong), Some(UiKeyProblem::Mistyped));
        // Spaces, dashes and small letters are taken.
        assert_eq!(
            recovery_key_problem(text.to_lowercase().replace(' ', "-")),
            None
        );
    }

    /// Each refusal comes back typed, with what the screens word it with, even under the
    /// words added on the way; any other failure stays an error.
    #[test]
    fn refusals_are_typed() {
        let refused = |e: anyhow::Error| restore_outcome(Err(e)).unwrap();

        let out = refused(anyhow::Error::new(Refused::AccountHere(Some("Ana".into()))));
        assert_eq!(out.refused, Some(UiRestoreRefusal::AccountHere));
        assert_eq!(out.name.as_deref(), Some("Ana"));

        let out = refused(anyhow::Error::new(Refused::NoRoom { need: 10, free: 3 }));
        assert_eq!(out.refused, Some(UiRestoreRefusal::NoRoom));
        assert_eq!((out.need, out.free), (10, 3));

        let out = refused(anyhow::Error::new(Refused::ReplacedKept));
        assert_eq!(out.refused, Some(UiRestoreRefusal::ReplacedKept));
        let out = refused(anyhow::Error::new(Refused::CopyInFolder));
        assert_eq!(out.refused, Some(UiRestoreRefusal::CopyInFolder));

        let no_keychain: Result<()> = Err(NoKeychain("locked".into()).into());
        let out = refused(no_keychain.context("cannot seal").unwrap_err());
        assert_eq!(out.refused, Some(UiRestoreRefusal::NoKeychain));

        let damaged: Result<()> = Err(CopyProblem::Damaged.into());
        let out = refused(damaged.context("the copy is cut short").unwrap_err());
        assert_eq!(out.problem, Some(UiCopyProblem::Damaged));
        assert_eq!(out.refused, None);
        let out = refused(anyhow::Error::new(CopyProblem::OtherKey));
        assert_eq!(out.problem, Some(UiCopyProblem::OtherKey));

        assert!(restore_outcome(Err(anyhow::anyhow!("cannot read the copy"))).is_err());
        let ok = restore_outcome(Ok(recovery::Restored {
            name: "Ana".into(),
            device: "00".into(),
            files: 7,
            replaced: true,
        }))
        .unwrap();
        let restored = ok.restored.unwrap();
        assert_eq!((restored.name.as_str(), restored.files), ("Ana", 7));
        assert!(restored.replaced);
    }

    /// The application restores with the keychain the process chose, asks the disk for
    /// room, and passes the person's choices on.
    #[test]
    fn a_restore_uses_the_chosen_keychain() {
        for choice in [false, true] {
            let options = restore_options(choice, !choice);
            assert_eq!((options.replace, options.unencrypted), (choice, !choice));
            assert!(options.keychain.is_none());
            assert!(options.free_space.is_some());
        }
    }

    #[test]
    fn a_copy_is_never_in_the_data_folder() {
        let dir = std::env::temp_dir().join(format!("fogata-ffi-in-folder-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("direct")).unwrap();
        assert!(in_folder(&dir, &dir.join("Fogata.fogata-copy")));
        assert!(in_folder(&dir, &dir.join("direct/x.fogata-copy")));
        assert!(!in_folder(
            &dir,
            &std::env::temp_dir().join("Fogata.fogata-copy")
        ));
        // A folder whose name starts like it is another folder.
        let beside = PathBuf::from(format!("{}-beside", dir.display()));
        assert!(!in_folder(&dir, &beside.join("x")));
        std::fs::remove_dir_all(&dir).unwrap();
        let name = recovery_copy_name();
        assert!(name.starts_with("Fogata-") && name.ends_with(".fogata-copy"));
    }
}
