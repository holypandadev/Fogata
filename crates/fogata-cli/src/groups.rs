//! Group management commands.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use fogata_core::{Core, CoreEvent, Store, invite};
use fogata_crypto::DevicePublicKey;
use fogata_state::{Body, GroupLog, GroupState};

use crate::{chat::NetArgs, friends};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Subcommand)]
pub enum GroupCommand {
    /// Create a group with this device first on its ladder.
    Create { name: String },
    /// Print a single-use invite for one new member (managers only).
    Invite {
        name: String,
        /// Days until the invite expires.
        #[arg(long, default_value_t = 7)]
        days: u64,
    },
    /// Add a friend, or any device by its key (managers only). It receives the group the next
    /// time it syncs with a member.
    Add {
        name: String,
        /// The device key in hex, as printed by `fogata id` on that device.
        #[arg(required_unless_present = "friend")]
        key: Option<String>,
        /// A friend to add, by name or fingerprint.
        #[arg(long, conflicts_with = "key")]
        friend: Option<String>,
        /// Name to show for it (defaults to the friend's name).
        #[arg(long)]
        nick: Option<String>,
    },
    /// Join a group with an invite, typed when asked (or given, for scripts). Waits until a
    /// member accepts it.
    Join(JoinArgs),
    /// Join a group a friend added you to (see `fogata group list`).
    Accept { name: String },
    /// Decline a group a friend added you to; its log is forgotten.
    Decline { name: String },
    /// List the groups of this device.
    List,
    /// Show a group's members.
    Members { name: String },
    /// Remove a member below this device on the ladder. Starts a new group key without it.
    Remove {
        name: String,
        /// Short form of the member's key, as shown by `fogata group members`.
        member: String,
    },
    /// Move a member below this device on the ladder up to just below it.
    MoveUp { name: String, member: String },
    /// Move a member below this device on the ladder down to the end.
    MoveDown { name: String, member: String },
    /// Who manages the group: `all`, or a number for the first ones on the ladder (managers
    /// only, never leaving this device out).
    Managers { name: String, first: String },
    /// Rename a group for everyone (managers only).
    Rename { name: String, new_name: String },
    /// Leave a group. The others learn it the next time this device connects.
    Leave { name: String },
}

#[derive(clap::Args)]
pub struct JoinArgs {
    /// The invite; read from the keyboard when left out, so it shows neither in the list of
    /// processes nor in the shell's history.
    invite: Option<String>,
    /// Local name for the group (defaults to the group's name).
    #[arg(long)]
    name: Option<String>,
    /// Name shown to the others (defaults to the profile name).
    #[arg(long)]
    nick: Option<String>,
    #[command(flatten)]
    net: NetArgs,
}

fn member_by_short(state: &GroupState, short: &str) -> Result<DevicePublicKey> {
    let short: String = short
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let matches: Vec<DevicePublicKey> = state
        .members
        .keys()
        .filter(|k| k.fingerprint().starts_with(&short.to_uppercase()))
        .copied()
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => bail!("no member whose key starts with {short}"),
        _ => bail!("several members match {short}; give more characters"),
    }
}

/// Lists a group's members. Names come from the profile for this device and for friends,
/// and otherwise from the log.
fn print_members(log: &GroupLog, me: &DevicePublicKey, book: Option<&fogata_friends::FriendBook>) {
    let state = log.state();
    let lacking = state.lacking_current_secret();
    println!("Group \"{}\", {} members:", state.name, state.members.len());
    if let fogata_state::Managers::First(n) = state.managers {
        println!("The first {n} manage it.");
    }
    for (rank, key) in state.ladder.iter().enumerate() {
        let member = &state.members[key];
        let place = format!("#{}", rank + 1);
        let mut tags = vec![place.as_str()];
        if rank == 0 {
            tags.push("first");
        }
        if state.managers != fogata_state::Managers::Everyone && state.is_manager(key) {
            tags.push("manages");
        }
        if key == me {
            tags.push("this device");
        }
        if lacking.contains(key) {
            tags.push("waiting for the current key");
        }
        let known = book.and_then(|b| {
            if b.profile.device == *key {
                Some(b.profile.name.as_str())
            } else {
                b.friends.get(key).map(|f| f.profile.name.as_str())
            }
        });
        let name = match (known, member.name.as_str()) {
            (Some(name), _) => name,
            (None, "") => "-",
            (None, name) => name,
        };
        let tags = if tags.is_empty() {
            String::new()
        } else {
            format!(" ({})", tags.join(", "))
        };
        println!("  {}  {name}{tags}", key.fingerprint());
    }
}

pub async fn run(store: Store, command: GroupCommand) -> Result<()> {
    // What changes the groups here holds the folder (see `friends::hold_folder`); joining
    // starts a node, which holds it itself.
    let _running = (!matches!(
        command,
        GroupCommand::Join(_) | GroupCommand::List | GroupCommand::Members { .. }
    ))
    .then(|| friends::hold_folder(&store))
    .transpose()?;
    let (identity, _) = store.identity()?;
    let me = identity.public_key();
    match command {
        GroupCommand::Create { name } => {
            let log = GroupLog::create(&identity, &name, now_ms())?;
            // Kept here under the name as it shows, which the members see too.
            let name = log.state().name;
            store.save_log(&name, &log)?;
            println!(
                "Created group \"{name}\". Invite someone with: fogata group invite \"{name}\""
            );
        }
        GroupCommand::Invite { name, days } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            let text = invite::make(&mut log, &identity, days, Vec::new(), now_ms())?;
            store.save_log(&name, &log)?;
            println!(
                "Single-use invite, valid for {days} days. Give it only to the person it is for:"
            );
            println!("{text}");
        }
        GroupCommand::Add {
            name,
            key,
            friend,
            nick,
        } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            let by_friend = friend.is_some();
            let (key, nick) = match (key, friend) {
                (Some(key), _) => {
                    let key = hex::decode(key.trim())
                        .ok()
                        .and_then(|b| DevicePublicKey::from_bytes(&b).ok())
                        .context("not a device key (64 hex characters from `fogata id`)")?;
                    (key, nick.unwrap_or_default())
                }
                (None, Some(who)) => {
                    let book = store.require_friends()?;
                    let key = friends::find(friends::friends_of(&book), &who)?;
                    let name = book.name_of(&key).unwrap_or_default().to_string();
                    (key, nick.unwrap_or(name))
                }
                (None, None) => bail!("give a device key or --friend"),
            };
            log.add(&identity, key, &nick, now_ms())?;
            store.save_log(&name, &log)?;
            let who = friends::describe(if nick.is_empty() { "-" } else { &nick }, &key);
            if by_friend {
                println!(
                    "Added {who}. They get the group the next time you are both online while the group is open here (fogata chat \"{name}\")."
                );
            } else {
                println!("Added {who}. It gets the group the next time it connects to a member.");
            }
        }
        GroupCommand::Join(args) => join(store, args).await?,
        GroupCommand::Accept { name } => {
            let group = store.group(&name)?;
            if !group.pending {
                bail!("\"{name}\" is not a pending invitation");
            }
            store.set_pending(&group.group_id, None)?;
            println!("Joined \"{name}\". Chat there with: fogata chat \"{name}\"");
        }
        GroupCommand::Decline { name } => {
            let group = store.group(&name)?;
            if !group.pending {
                bail!("\"{name}\" is not a pending invitation; leave it with: fogata group leave");
            }
            store.decline_group(&group.group_id, &identity, now_ms())?;
            println!("Declined \"{name}\".");
        }
        GroupCommand::List => {
            let groups = store.groups()?;
            if groups.is_empty() {
                println!("No groups yet. Create one with: fogata group create <name>");
            }
            for group in groups {
                let members = store
                    .load_log(&group)
                    .map(|l| l.state().members.len())
                    .unwrap_or(0);
                if group.pending {
                    println!(
                        "{}  ({members} members; a friend added you: fogata group accept \"{}\")",
                        group.name, group.name
                    );
                } else {
                    println!("{}  ({members} members)", group.name);
                }
            }
        }
        GroupCommand::Members { name } => {
            let log = store.load_log(&store.group(&name)?)?;
            print_members(&log, &me, store.friends()?.as_ref());
        }
        GroupCommand::Remove { name, member } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            let target = member_by_short(&log.state(), &member)?;
            log.remove(&identity, target, now_ms())?;
            store.save_log(&name, &log)?;
            println!(
                "Removed {}. A new group key was made for the others; they get it the next time a member who knows about this connects.",
                target.fingerprint()
            );
        }
        GroupCommand::MoveUp { name, member } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            let target = member_by_short(&log.state(), &member)?;
            log.append(&identity, Body::Promote { member: target }, now_ms())?;
            store.save_log(&name, &log)?;
            println!("Done.");
        }
        GroupCommand::MoveDown { name, member } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            let target = member_by_short(&log.state(), &member)?;
            log.append(&identity, Body::Demote { member: target }, now_ms())?;
            store.save_log(&name, &log)?;
            println!("Done.");
        }
        GroupCommand::Managers { name, first } => {
            let first = match first.as_str() {
                "all" => None,
                n => Some(n.parse::<u8>().context("give `all` or a number")?),
            };
            let mut log = store.load_log(&store.group(&name)?)?;
            log.append(&identity, Body::SetManagers { first }, now_ms())?;
            store.save_log(&name, &log)?;
            println!("Done.");
        }
        GroupCommand::Rename { name, new_name } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            log.append(&identity, Body::Rename { name: new_name }, now_ms())?;
            store.save_log(&name, &log)?;
            let new_name = log.state().name;
            println!(
                "The group is now called \"{new_name}\" for everyone (it keeps the local name \"{name}\" here)."
            );
        }
        GroupCommand::Leave { name } => {
            let mut log = store.load_log(&store.group(&name)?)?;
            log.append(&identity, Body::Leave, now_ms())?;
            store.save_log(&name, &log)?;
            println!(
                "Left \"{name}\". Run `fogata chat \"{name}\"` once more so the others learn it."
            );
        }
    }
    Ok(())
}

async fn join(store: Store, args: JoinArgs) -> Result<()> {
    let invite = match args.invite {
        Some(invite) => invite,
        None => crate::input::secret_line("Invite:")?,
    };
    let (core, mut events) = Core::start(store.dir().to_path_buf(), args.net.options())?;
    let (group, name) = core
        .join_group(&invite, args.name.as_deref(), args.nick.as_deref())
        .await?;
    say!("· looking for a member of \"{name}\" to accept the invite");
    let joined = tokio::time::timeout(Duration::from_secs(600), async {
        while let Some(event) = events.recv().await {
            match event {
                CoreEvent::Joined { group: g, .. } if g == group => return true,
                CoreEvent::Info(text) => say!("· {text}"),
                CoreEvent::Address(addr) => {
                    say!("· other peers see this device at {addr}");
                }
                _ => {}
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    if !joined {
        bail!(
            "nobody in the group accepted the invite; try again when someone who manages it is online"
        );
    }
    // The core saves the group; give it a moment before the process ends.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let local = core.store().group_name(&group).unwrap_or(name.clone());
    say!("Joined \"{name}\". Start chatting with: fogata chat \"{local}\"");
    Ok(())
}
