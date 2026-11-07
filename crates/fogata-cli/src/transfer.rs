//! `fogata transfer`: moving this device's account to another computer, or bringing one
//! here, with a short code.

use std::{io::BufRead, path::Path};

use anyhow::{Result, bail};
use clap::Subcommand;
use fogata_core::transfer::{
    self, Failure, MovedState, TransferEvent, TransferOptions, TransferState,
};

use crate::chat::NetArgs;

#[derive(Subcommand)]
pub enum TransferCommand {
    /// Show a code here and bring the account of the computer where it is typed. Replaces the
    /// profile this folder has.
    Receive {
        #[command(flatten)]
        net: NetArgs,
        /// What this computer is called on the other one.
        #[arg(long)]
        name: Option<String>,
        /// Minutes the code stays valid.
        #[arg(long, default_value_t = 10, hide = true)]
        minutes: u64,
        /// Replace the profile this folder has, and take the account that comes, without
        /// asking.
        #[arg(long)]
        yes: bool,
    },
    /// Move this account to the computer showing the code. This computer stops using it.
    /// Without CODE it is asked for, so it stays out of the shell's history and the list of
    /// processes.
    Send {
        code: Option<String>,
        #[command(flatten)]
        net: NetArgs,
        #[arg(long)]
        name: Option<String>,
        /// Move without asking.
        #[arg(long)]
        yes: bool,
        /// Seconds to look for the other computer.
        #[arg(long, default_value_t = 180, hide = true)]
        seconds: u64,
    },
    /// Whether this account moved away, or a move into this folder waits to be finished.
    Status,
    /// After the account moved away: delete what is left of it here.
    Forget,
    /// The account is "moving" but the other computer never got it: use it here again.
    Keep {
        /// Without asking. If the other computer has it, there are two of you.
        #[arg(long)]
        yes: bool,
    },
    /// Throw away a move into this folder that the other computer did not finish.
    Discard,
}

/// What this computer is called, for the other one.
fn computer_name(given: Option<String>) -> String {
    given
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "another computer".into())
}

pub async fn run(dir: &Path, command: TransferCommand) -> Result<()> {
    match command {
        TransferCommand::Receive {
            net,
            name,
            minutes,
            yes,
        } => {
            let store = fogata_core::Store::open(dir.to_path_buf())?;
            if store.has_profile()? {
                let me = store
                    .friends()
                    .ok()
                    .flatten()
                    .map_or_else(|| "this profile".to_string(), |b| b.profile.name);
                if !yes
                    && !ask(&format!(
                        "The account that comes replaces {me} here, with its friends, groups \
                         and chats. Go on?"
                    ))
                {
                    bail!("nothing changed");
                }
            }
            drop(store);
            let mut options = TransferOptions::new(net.options(), computer_name(name));
            options.code_lifetime_ms = minutes * 60 * 1000;
            options.free_space = Some(crate::disk::free_space);
            let moving = transfer::receive(dir.to_path_buf(), options)?;
            follow(moving, false, yes).await
        }
        TransferCommand::Send {
            code,
            net,
            name,
            yes,
            seconds,
        } => {
            let code = match code {
                Some(code) => code,
                None => read_line("Code shown on the new computer:")?,
            };
            let mut options = TransferOptions::new(net.options(), computer_name(name));
            options.no_contact_ms = seconds * 1000;
            let moving = transfer::send(dir.to_path_buf(), &code, options)?;
            follow(moving, true, yes).await
        }
        TransferCommand::Status => {
            match transfer::state(dir) {
                TransferState::None => println!("No move here."),
                TransferState::Moved(m) if m.state == MovedState::Moved => println!(
                    "This account ({}) moved to “{}”. `fogata transfer forget` deletes what is left here.",
                    m.name,
                    named(&m.to)
                ),
                TransferState::Moved(m) => println!(
                    "This account ({}) is moving to “{}”, which has not confirmed yet. Show a code there and run `fogata transfer send CODE` to finish.",
                    m.name,
                    named(&m.to)
                ),
                TransferState::Staged(i) => println!(
                    "{}'s account from “{}” came whole and waits for that computer to finish. Run `fogata transfer receive` and type the code there.",
                    i.name,
                    named(&i.from)
                ),
            }
            Ok(())
        }
        TransferCommand::Forget => {
            transfer::forget(dir)?;
            println!("Deleted what was left of the account here.");
            Ok(())
        }
        TransferCommand::Keep { yes } => {
            if !yes
                && !ask(
                    "Use the account here again? Only if the other computer does not have it: \
                     with both, friends and groups would see two of you.",
                )
            {
                bail!("nothing changed");
            }
            transfer::keep_this_computer(dir)?;
            println!("This computer uses the account again.");
            Ok(())
        }
        TransferCommand::Discard => {
            if transfer::discard_incoming(dir)? {
                println!("Thrown away.");
            } else {
                println!("It was already being put in place: this folder holds the account now.");
            }
            Ok(())
        }
    }
}

/// The other computer's name, or words for it when it gave none that shows.
fn named(computer: &str) -> &str {
    if computer.is_empty() {
        "the other computer"
    } else {
        computer
    }
}

/// Follows a move to its end, `sending` on the computer with the account. Both computers ask
/// before the account goes, unless `yes`.
async fn follow(mut moving: transfer::Transfer, sending: bool, yes: bool) -> Result<()> {
    while let Some(event) = moving.events.recv().await {
        match event {
            TransferEvent::Code { code, .. } => {
                say!("Code: {code}");
                say!("Type it on the other computer: fogata transfer send {code}");
            }
            TransferEvent::Looking => say!("Looking for the other computer..."),
            TransferEvent::WrongCodeTried => say!("Someone tried a wrong code."),
            TransferEvent::Matched {
                computer,
                summary,
                lan,
                replaces,
            } => {
                let computer = named(&computer);
                say!(
                    "Matched “{computer}”{}: {} with {} friends, {} groups, {} chats, {} files, {} bytes",
                    if lan { " on this network" } else { "" },
                    summary.profile.name,
                    summary.friends,
                    summary.groups,
                    summary.chats,
                    summary.files,
                    summary.bytes
                );
                if let Some(name) = replaces {
                    say!("It replaces the profile {name} there.");
                }
                let question = if sending {
                    format!("Move this account to “{computer}”?")
                } else {
                    format!(
                        "Bring {}'s account here from “{computer}”?",
                        summary.profile.name
                    )
                };
                moving.confirm(yes || ask(&question));
            }
            TransferEvent::Progress { done, total } => {
                say!("{}%", done * 100 / total.max(1));
            }
            TransferEvent::Finishing => say!("Finishing on both computers..."),
            TransferEvent::Done { computer, .. } => {
                say!("Done ({}).", named(&computer));
                return Ok(());
            }
            TransferEvent::Failed(failure) => {
                if failure == Failure::Unconfirmed {
                    bail!("{failure}: run `fogata transfer status` on both computers");
                }
                bail!("{failure}");
            }
        }
    }
    bail!("the move stopped")
}

fn read_line(prompt: &str) -> Result<String> {
    println!("{prompt}");
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn ask(question: &str) -> bool {
    println!("{question} [y/N]");
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).is_ok() && matches!(line.trim(), "y" | "Y" | "yes")
}
