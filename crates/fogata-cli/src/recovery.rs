//! `fogata recovery`: a copy of this account that is restored without the system keychain,
//! with a recovery key the person keeps.

use std::{
    io::{BufRead, IsTerminal},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Result, bail};
use clap::Subcommand;
use fogata_core::{
    recovery::{self, NewKey, RecoveryKey, RestoreOptions},
    vault::{Keychain, SystemKeychain},
};
use zeroize::Zeroizing;

#[derive(Subcommand)]
pub enum RecoveryCommand {
    /// Turn on copies of this account, or change their key: shows the recovery key once.
    /// Copies made before keep opening with the key they were made for.
    On {
        /// Change the key without asking when copies are on already.
        #[arg(long)]
        yes: bool,
        /// Take the new key without asking for two of its groups back (for scripts).
        #[arg(long, hide = true)]
        unchecked: bool,
    },
    /// Make a copy of this account at FILE. No other copy of Fogata may run on this folder.
    Export {
        file: PathBuf,
        /// Put the files sent in chats in the copy too.
        #[arg(long)]
        attachments: bool,
    },
    /// How many bytes the files sent in chats would add to a copy.
    Size,
    /// Restore the copy in FILE here. The recovery key is read from the input, so it stays
    /// out of the shell's history and the list of processes.
    Restore {
        file: PathBuf,
        /// Replace the account this folder holds; it is kept aside, not deleted.
        #[arg(long)]
        replace: bool,
        /// Without asking.
        #[arg(long)]
        yes: bool,
        /// Keep the account restored unencrypted when the system keychain cannot keep the key
        /// that encrypts it.
        #[arg(long)]
        unencrypted: bool,
    },
    /// Whether copies are on, and whether an account a restore replaced is kept here.
    Status,
    /// Delete the account a restore replaced, kept in this folder.
    ForgetReplaced,
}

pub fn run(dir: &Path, command: RecoveryCommand) -> Result<()> {
    match command {
        RecoveryCommand::On { yes, unchecked } => {
            if recovery::is_on(dir)
                && !yes
                && !ask(
                    "Copies are on already. Make a new recovery key? Copies made before open \
                     only with the old one.",
                )
            {
                bail!("nothing changed");
            }
            let new = NewKey::generate()?;
            println!(
                "Your recovery key. Write it down or print it, and keep it apart from the copies:"
            );
            println!();
            println!("    {}", &new.text()[..]);
            println!();
            println!(
                "Fogata does not keep it anywhere: without it no copy opens. Anyone with a \
                 copy and this key has the account."
            );
            if !unchecked {
                confirm_written(&new.text())?;
            }
            // Only now: until the person showed they have it, copies stay as they were.
            recovery::turn_on(dir, &new)?;
            println!("Copies are on, for the recovery key named {}.", new.id());
            Ok(())
        }
        RecoveryCommand::Export { file, attachments } => {
            let made = recovery::export(dir, &file, attachments)?;
            println!(
                "Made {} for the recovery key named {} ({} files, {} bytes){}.",
                file.display(),
                recovery::key_id(dir)?.unwrap_or_default(),
                made.files,
                made.bytes,
                if attachments {
                    ""
                } else {
                    ", without the files sent in chats"
                }
            );
            Ok(())
        }
        RecoveryCommand::Size => {
            println!(
                "The files sent in chats would add {} bytes to a copy.",
                recovery::attachments_bytes(dir)?
            );
            Ok(())
        }
        RecoveryCommand::Restore {
            file,
            replace,
            yes,
            unencrypted,
        } => {
            let info = recovery::inspect(&file, dir)?;
            println!("A copy made on {}.", info.made_on);
            if info.this_key == Some(false) {
                println!("It was made for another recovery key than this folder's.");
            }
            let key = read_key()?;
            let copy = recovery::open_copy(&file, &key)?;
            println!(
                "It holds {}'s account (device {}), {} files.",
                copy.name, copy.fingerprint, copy.files
            );
            let here = recovery::account_in(dir)?;
            let question = match &here {
                Some(here) => {
                    let name = here
                        .name
                        .clone()
                        .unwrap_or_else(|| "an account that does not open here".into());
                    let device = here
                        .fingerprint
                        .as_ref()
                        .map(|f| format!(" (device {f})"))
                        .unwrap_or_default();
                    println!("This folder holds {name}{device}.");
                    if !replace {
                        bail!(
                            "restoring the copy replaces it: run it again with --replace (it is \
                             kept aside, not deleted)"
                        );
                    }
                    format!(
                        "Replace {name} here with {}'s account from the copy? The one here is \
                         kept aside.",
                        copy.name
                    )
                }
                None => format!("Restore {}'s account here?", copy.name),
            };
            if !yes
                && !ask(&format!(
                    "{question} Only if no other computer uses {}'s account any more: if one \
                     does, move the account instead (fogata transfer).",
                    copy.name
                ))
            {
                bail!("nothing changed");
            }
            let options = restore_options(replace, unencrypted);
            let restored = match recovery::restore(dir, &file, &key, &options) {
                Err(e) if e.downcast_ref::<fogata_core::vault::NoKeychain>().is_some() => {
                    bail!(
                        "{e}: nothing changed. Run it again with --unencrypted to keep the account \
                         unencrypted until a keychain can keep its key"
                    )
                }
                other => other?,
            };
            println!(
                "Restored {}'s account ({} files).{}",
                restored.name,
                restored.files,
                if restored.replaced {
                    " The account this folder held is kept aside: `fogata recovery \
                     forget-replaced` deletes it."
                } else {
                    ""
                }
            );
            Ok(())
        }
        RecoveryCommand::Status => {
            println!(
                "Copies are {}.",
                if recovery::is_on(dir) { "on" } else { "off" }
            );
            if recovery::replaced_kept(dir) {
                println!("An account a restore replaced is kept in this folder.");
            }
            Ok(())
        }
        RecoveryCommand::ForgetReplaced => {
            if recovery::forget_replaced(dir)? {
                println!("Deleted the account the restore replaced.");
            } else {
                println!("No account a restore replaced is kept here.");
            }
            Ok(())
        }
    }
}

/// How the command line restores: sealed with a new key of the system keychain, as the
/// application seals an account that moves in, though the command line opens folders without
/// sealing them otherwise.
fn restore_options(replace: bool, unencrypted: bool) -> RestoreOptions {
    RestoreOptions {
        replace,
        keychain: Some(Keychain::Seal(Arc::new(SystemKeychain))),
        free_space: Some(crate::disk::free_space),
        unencrypted,
    }
}

/// The recovery key, read from the input; asked again while it does not read as one when the
/// input is a terminal.
fn read_key() -> Result<RecoveryKey> {
    let terminal = std::io::stdin().is_terminal();
    loop {
        println!("Recovery key:");
        let mut line = Zeroizing::new(String::new());
        if std::io::stdin().lock().read_line(&mut line)? == 0 {
            bail!("no recovery key was given");
        }
        match RecoveryKey::parse(&line) {
            Ok(key) => return Ok(key),
            Err(problem) if terminal => println!("{problem}"),
            Err(problem) => bail!("{problem}"),
        }
    }
}

/// Asks for two groups of the key, picked at random, so that the person checks what they
/// wrote down; an error when the input ends first.
fn confirm_written(key: &str) -> Result<()> {
    let groups: Vec<&str> = key.split(' ').collect();
    let mut pick = [0u8; 2];
    fogata_crypto::random_bytes(&mut pick)?;
    let first = pick[0] as usize % (groups.len() - 1);
    let second = (first + 1 + pick[1] as usize % (groups.len() - 2)) % (groups.len() - 1);
    for at in [first.min(second), first.max(second)] {
        loop {
            println!(
                "To check what you wrote down, type group {} of the key:",
                at + 1
            );
            let mut line = Zeroizing::new(String::new());
            if std::io::stdin().lock().read_line(&mut line)? == 0 {
                bail!("nothing changed: the new key was not checked");
            }
            if line.trim().eq_ignore_ascii_case(groups[at]) {
                break;
            }
            println!("That is not it: look again at the key above.");
        }
    }
    println!("Good.");
    Ok(())
}

fn ask(question: &str) -> bool {
    println!("{question} [y/N]");
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).is_ok() && matches!(line.trim(), "y" | "Y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The command line opens folders without sealing them, but what it restores is sealed
    /// with the system keychain, as an account that moves in is; a copy there is no room for
    /// is refused first; and unencrypted is asked of the restore, not written in the folder.
    #[test]
    fn a_restore_seals_with_the_system_keychain() {
        for replace in [false, true] {
            let options = restore_options(replace, replace);
            assert_eq!(options.replace, replace);
            assert_eq!(options.unencrypted, replace);
            assert!(matches!(options.keychain, Some(Keychain::Seal(_))));
            let free_space = options.free_space.expect("the disk is asked");
            #[cfg(unix)]
            assert!(free_space(&std::env::temp_dir()).is_some());
            #[cfg(not(unix))]
            let _ = free_space;
        }
    }
}
