//! A group's room on standard input and output: everything said in a group is said in its
//! room, which exists only while someone is in it. Runs on [`fogata_core::Core`], like the
//! application, so every group and friend is online meanwhile.

use std::{path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use fogata_core::{Core, CoreEvent, NetOptions, Store};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId};
use fogata_friends::FriendBook;
use fogata_net::{GroupSetup, Multiaddr, NodeConfig, Path, RoomChange};

use crate::{friends, input};

#[derive(clap::Args)]
pub struct ChatArgs {
    /// Group whose room to enter (as listed by `fogata group list`).
    group: String,
    /// Name shown to the others (defaults to the profile name).
    #[arg(long)]
    nick: Option<String>,
    /// Stay online in the group without entering its room; shows who is in it.
    #[arg(long)]
    watch: bool,
    /// Where `/get` saves files.
    #[arg(long, default_value = ".")]
    downloads: PathBuf,
    /// Let in at once every device that shows an invite of this device (for tests).
    #[arg(long)]
    accept_joins: bool,
    #[command(flatten)]
    net: NetArgs,
}

/// Network options shared by the commands that start a node.
#[derive(clap::Args)]
pub struct NetArgs {
    /// UDP/TCP port to listen on (0 = any free one).
    #[arg(long, default_value_t = 0)]
    port: u16,
    /// Do not use the public DHT to find members.
    #[arg(long)]
    no_dht: bool,
    /// Do not look for members on the local network.
    #[arg(long)]
    no_mdns: bool,
    /// Help other members coordinate a hole punch, but never carry their traffic.
    #[arg(long)]
    no_bridge: bool,
    /// DHT bootstrap peer (replaces the public ones). Repeatable.
    #[arg(long = "bootstrap")]
    bootstrap: Vec<Multiaddr>,
    /// Address of a member to dial directly. Repeatable.
    #[arg(long = "peer")]
    peers: Vec<Multiaddr>,
}

impl NetArgs {
    pub fn use_dht(&self) -> bool {
        !self.no_dht
    }

    pub fn options(self) -> NetOptions {
        NetOptions {
            listen_port: self.port,
            bootstrap: self.bootstrap,
            use_dht: !self.no_dht,
            use_mdns: !self.no_mdns,
            bridge: !self.no_bridge,
            peers: self.peers,
            hidden: false,
        }
    }

    /// For the test commands that run a node by themselves.
    pub fn node_config(
        self,
        identity: DeviceIdentity,
        groups: Vec<GroupSetup>,
        friends: Option<FriendBook>,
    ) -> NodeConfig {
        let options = self.options();
        NodeConfig {
            identity: Arc::new(identity),
            groups,
            friends,
            listen_port: options.listen_port,
            bootstrap: options.bootstrap,
            use_dht: options.use_dht,
            use_mdns: options.use_mdns,
            bridge: options.bridge,
            peers: options.peers,
            remembered: Vec::new(),
            hidden: options.hidden,
        }
    }
}

pub async fn run(store: Store, args: ChatArgs) -> Result<()> {
    let dir = store.dir().to_path_buf();
    let use_dht = args.net.use_dht();
    let local_name = args.group.clone();
    let ChatArgs {
        nick,
        watch,
        downloads,
        accept_joins,
        net,
        ..
    } = args;
    let (core, mut events) = Core::start(dir, net.options())?;
    let group = core.group_id(&local_name)?;
    let me = core.me();
    let Some(view) = core.groups().await?.into_iter().find(|g| g.id == group) else {
        anyhow::bail!("this device is no longer a member of \"{local_name}\"");
    };

    if watch {
        println!(
            "Group \"{}\" (device {}), watching its room. /join to enter, /help for commands.",
            view.name,
            me.fingerprint()
        );
    } else {
        let shown = match &nick {
            Some(nick) => nick.clone(),
            None => core
                .profile()
                .await?
                .map(|p| p.name)
                .unwrap_or_else(|| "anon".into()),
        };
        println!(
            "Room of \"{}\" as {shown} (device {}). Type a message and press Enter; /help for commands.",
            view.name,
            me.fingerprint()
        );
        core.join_room_as(group, nick.clone()).await?;
    }
    println!("Times are UTC.");
    if use_dht {
        say!("· looking for members on the DHT; this can take up to a minute");
    }

    let session = Session {
        core,
        group,
        nick,
        downloads,
        accept_joins,
        started_ms: fogata_core::now_ms(),
    };
    let mut lines = input::lines();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            line = lines.recv() => {
                let Some(line) = line else { break };
                if !session.command(line.trim()).await? {
                    break;
                }
            }
            event = events.recv() => {
                let Some(event) = event else { break };
                if !session.show(event).await? {
                    break;
                }
            }
        }
    }
    let core = session.core;
    if core
        .room(group, fogata_core::FIRST_ROOM)
        .await?
        .is_some_and(|r| r.joined)
    {
        core.leave_room(group).await?;
        // Give the goodbye a moment to reach the others.
        tokio::time::sleep(Duration::from_millis(800)).await;
    }
    Ok(())
}

struct Session {
    core: Core,
    group: GroupId,
    nick: Option<String>,
    downloads: PathBuf,
    accept_joins: bool,
    /// Lines this device said since then are already on screen.
    started_ms: u64,
}

impl Session {
    /// Handles a typed line. Returns false to stop.
    async fn command(&self, line: &str) -> Result<bool> {
        let (core, group, me) = (&self.core, self.group, self.core.me());
        match line {
            "" => {}
            "/quit" => return Ok(false),
            "/help" => say!(
                "· /room  /join  /leave  /share <file>  /files  /get <n>  /stop <n>  /members  /invite  /friends  /accept <who>  /decline <who>  /quit"
            ),
            "/room" => match core.room(group, fogata_core::FIRST_ROOM).await? {
                None => say!("· no room is open; /join starts one"),
                Some(room) => {
                    let since = fogata_core::now_ms().saturating_sub(room.started_ms) / 60_000;
                    say!("· room open for {since} min:");
                    for (person, away) in room.participants {
                        let note = if away { " (away)" } else { "" };
                        let who = if person.device == me {
                            "you".to_string()
                        } else {
                            format!("{} [{}]", person.name, person.device.fingerprint())
                        };
                        say!("    {who}{note}");
                    }
                }
            },
            "/join" => core.join_room_as(group, self.nick.clone()).await?,
            "/leave" => core.leave_room(group).await?,
            "/files" => self.list_files().await?,
            "/friends" => {
                let online: Vec<_> = core
                    .friends()
                    .await?
                    .into_iter()
                    .filter(|f| f.path.is_some())
                    .collect();
                if online.is_empty() {
                    say!("· no friend is online");
                }
                for friend in online {
                    say!(
                        "· {} [{}] {}",
                        friend.person.name,
                        friend.person.short,
                        friends::presence_text(&friend.presence)
                    );
                }
            }
            "/members" => {
                for member in core.members(group).await? {
                    let status = match (member.me, &member.online) {
                        (true, _) => "this device".to_string(),
                        (false, Some(path)) => format!("online, {path}"),
                        (false, None) => "not connected".to_string(),
                    };
                    say!(
                        "· #{} {} [{}] ({status})",
                        member.rank + 1,
                        member.person.name,
                        member.person.device.fingerprint()
                    );
                }
            }
            "/invite" => match core.invite(group, 7).await {
                Ok(link) => {
                    say!("· single-use invite with this device's addresses, valid 7 days:");
                    println!("{link}");
                }
                Err(e) => say!("· {e}"),
            },
            _ if line.starts_with("/share ") => {
                let path = PathBuf::from(line["/share ".len()..].trim());
                if let Err(e) = core.share_file(group, path, None).await {
                    say!("· {e:#}");
                }
            }
            _ if line.starts_with("/get ") || line.starts_with("/stop ") => {
                let (verb, which) = line.split_once(' ').unwrap_or_default();
                let files = core
                    .room(group, fogata_core::FIRST_ROOM)
                    .await?
                    .map(|r| r.files)
                    .unwrap_or_default();
                let Some(file) = which
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|n| n.checked_sub(1))
                    .and_then(|n| files.get(n))
                else {
                    say!("· no file {which}; see /files");
                    return Ok(true);
                };
                if verb == "/stop" {
                    core.stop_sharing(group, file.info.id).await?;
                } else if file.mine {
                    say!("· you already have {}", file.info.name);
                } else {
                    let dest = core.download(group, file.info.id, &self.downloads).await?;
                    say!(
                        "· getting {} ({}) into {}",
                        file.info.name,
                        size_text(file.info.size),
                        dest.display()
                    );
                }
            }
            _ if line.starts_with("/accept ") || line.starts_with("/decline ") => {
                let (verb, who) = line.split_once(' ').unwrap_or_default();
                let (incoming, _) = core.requests().await?;
                let candidates = incoming
                    .iter()
                    .map(|r| (r.person.device, r.person.name.as_str()));
                match friends::find(candidates, who) {
                    Ok(device) => {
                        core.answer_request(device, verb == "/accept").await?;
                        say!("· done");
                    }
                    Err(e) => say!("· {e}"),
                }
            }
            text => {
                if let Err(e) = core.say(group, text).await {
                    say!("· {e:#}");
                }
            }
        }
        Ok(true)
    }

    async fn list_files(&self) -> Result<()> {
        let files = self
            .core
            .room(self.group, fogata_core::FIRST_ROOM)
            .await?
            .map(|r| r.files)
            .unwrap_or_default();
        if files.is_empty() {
            say!("· no files in the room; /share <file> shares one");
        }
        for (n, file) in files.iter().enumerate() {
            let holders: Vec<&str> = file.holders.iter().map(|p| p.name.as_str()).collect();
            let mut note = String::new();
            if file.mine {
                note.push_str(&format!(", yours, sent {} time(s)", file.sent));
            }
            if let Some(got) = file.downloading {
                note.push_str(&format!(", {} received", size_text(got)));
            }
            say!(
                "· {}  {} ({})  from {}{note}",
                n + 1,
                file.info.name,
                size_text(file.info.size),
                holders.join(", ")
            );
        }
        Ok(())
    }

    /// A device as shown in this room: its name here and its fingerprint.
    async fn who(&self, device: &DevicePublicKey) -> String {
        let name = self
            .core
            .members(self.group)
            .await
            .ok()
            .and_then(|m| m.into_iter().find(|m| m.person.device == *device))
            .map(|m| m.person.name)
            .unwrap_or_else(|| "?".into());
        format!("{name} [{}]", device.fingerprint())
    }

    /// Prints an event. Returns false to stop.
    async fn show(&self, event: CoreEvent) -> Result<bool> {
        let (group, me) = (self.group, self.core.me());
        match event {
            CoreEvent::Room {
                group: g, change, ..
            } if g == group => match change {
                RoomChange::Opened { started_ms } => {
                    say!(
                        "· the room is open, since {} UTC",
                        crate::clock_at(started_ms)
                    );
                }
                RoomChange::Joined { device, .. } => {
                    if device == me {
                        say!("· you are in the room");
                    } else {
                        say!("+ {} joined the room", self.who(&device).await);
                    }
                }
                RoomChange::Left { device, on_purpose } => {
                    let how = if on_purpose { "left" } else { "dropped out of" };
                    if device != me {
                        say!("- {} {how} the room", self.who(&device).await);
                    }
                }
                RoomChange::Away { device } => say!("~ {} seems away", self.who(&device).await),
                RoomChange::Back { device } => say!("~ {} is back", self.who(&device).await),
                RoomChange::Chat(line) => {
                    if line.sender != me || line.timestamp_ms < self.started_ms {
                        println!(
                            "{} {} [{}]: {}",
                            crate::clock_at(line.timestamp_ms),
                            line.nick,
                            line.sender.fingerprint(),
                            line.text
                        );
                    }
                }
                RoomChange::FileShared { file, by } => {
                    let who = if by == me {
                        "you share".to_string()
                    } else {
                        format!("{} shares", self.who(&by).await)
                    };
                    say!(
                        "· {who} {} ({}); /files to see, /get to download",
                        file.name,
                        size_text(file.size)
                    );
                }
                RoomChange::FileGone { file } => {
                    say!("· {} is gone: nobody who has it is in the room", file.name);
                }
                RoomChange::Closed => say!("· the room closed: nobody is in it"),
            },
            CoreEvent::MemberConnected {
                group: g,
                device,
                path,
            } if g == group => {
                let path = match path {
                    Path::ThroughMember {
                        member: Some(member),
                        ..
                    } => format!("through {}", self.who(&member).await),
                    path => path.to_string(),
                };
                say!("+ {} connected ({path})", self.who(&device).await);
            }
            CoreEvent::MemberDisconnected { group: g, device } if g == group => {
                say!("- {} disconnected", self.who(&device).await);
            }
            CoreEvent::Removed { group: g, .. } if g == group => {
                say!("· this device is no longer a member of the group; leaving");
                return Ok(false);
            }
            CoreEvent::Download {
                group: g,
                received,
                total,
                ..
            } if g == group => {
                let quarter = received * 4 / total.max(1);
                if (1..4).contains(&quarter) && received * 20 / total.max(1) == quarter * 5 {
                    say!("· {}%", quarter * 25);
                }
            }
            CoreEvent::DownloadDone { group: g, path, .. } if g == group => {
                say!("· saved {} (checked); you share it now too", path.display());
            }
            CoreEvent::DownloadFailed {
                group: g, reason, ..
            } if g == group => {
                say!("· download failed: {reason}");
            }
            CoreEvent::FriendRequest { profile } => {
                say!(
                    "· {} wants to be friends: /accept {} or /decline {}",
                    friends::describe(&profile.name, &profile.device),
                    profile.name,
                    profile.name
                );
            }
            CoreEvent::FriendOnline { device } => {
                say!("· friend {} is online", self.friend_name(&device).await);
            }
            CoreEvent::FriendOffline { device } => {
                say!("· friend {} went offline", self.friend_name(&device).await);
            }
            CoreEvent::FriendAdded { profile } => {
                say!("· {} and you are friends now", profile.name);
            }
            CoreEvent::GroupInvited { name, by, .. } => {
                say!(
                    "· {} added you to the group \"{name}\"; join it with: fogata group accept \"{name}\"",
                    self.friend_name(&by).await
                );
            }
            CoreEvent::InvitationDeclined { name, who, .. } => {
                say!("· {who} declined the invitation to the group \"{name}\"");
            }
            CoreEvent::JoinAsked {
                group: g,
                device,
                name,
            } if g == group => {
                if self.accept_joins {
                    self.core.answer_join(group, device, true).await?;
                    say!("· let {name} [{}] in", device.fingerprint());
                } else {
                    say!(
                        "· {name} [{}] shows an invite and waits; let them in from the app",
                        device.fingerprint()
                    );
                }
            }
            CoreEvent::Address(address) => say!("· other peers see this device at {address}"),
            CoreEvent::HardNat => say!(
                "· this network changes the public port for each destination; direct connections may need a member in between"
            ),
            CoreEvent::Info(text) => say!("· {text}"),
            CoreEvent::Notice(text) => say!("! {text}"),
            _ => {}
        }
        Ok(true)
    }

    async fn friend_name(&self, device: &DevicePublicKey) -> String {
        let name = self
            .core
            .friends()
            .await
            .ok()
            .and_then(|f| f.into_iter().find(|f| f.person.device == *device))
            .map(|f| f.person.name)
            .unwrap_or_else(|| "?".into());
        format!("{name} [{}]", device.short_fingerprint())
    }
}

fn size_text(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} B"),
        b if b < 1024 * 1024 => format!("{:.1} KiB", b as f64 / 1024.0),
        b if b < 1024 * 1024 * 1024 => format!("{:.1} MiB", b as f64 / (1024.0 * 1024.0)),
        b => format!("{:.1} GiB", b as f64 / (1024.0 * 1024.0 * 1024.0)),
    }
}
