//! Profile and friends commands.

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use fogata_core::{Core, CoreEvent, Presence, Store};
use fogata_crypto::DevicePublicKey;
use fogata_friends::{Avatar, FriendBook, FriendCode, FriendLink, LINK_PREFIX, Profile};
use qrcode::{QrCode, render::unicode::Dense1x2};
use tokio::{sync::mpsc::UnboundedReceiver, time::Instant};

use crate::{chat::NetArgs, groups::now_ms, input};

/// How long `invite` waits for a request, and `add` for the answer once the request arrived.
const WAIT: Duration = Duration::from_secs(10 * 60);
/// How long `add` looks for the other device.
const FIND: Duration = Duration::from_secs(3 * 60);

#[derive(clap::Args)]
pub struct ProfileArgs {
    /// Your name, as friends and group members see it.
    #[arg(long)]
    name: Option<String>,
    /// One of the preset avatars, 0 to 4.
    #[arg(long, conflicts_with = "photo")]
    avatar: Option<u8>,
    /// A PNG or JPEG picture of at most 64 KiB.
    #[arg(long)]
    photo: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum FriendsCommand {
    /// List friends and friend requests.
    List,
    /// Show a link and QR code, or a short code, for one person to send a friend request,
    /// and wait for the request.
    Invite {
        /// Show a short code to read out instead of a link.
        #[arg(long)]
        code: bool,
        /// Accept the first request without asking (for scripted tests).
        #[arg(long, hide = true)]
        accept_first: bool,
        #[command(flatten)]
        net: NetArgs,
    },
    /// Send a friend request with a link or code from the other device, and wait for the
    /// answer.
    Add {
        /// The link or code; read from the keyboard when left out, so it shows neither in
        /// the list of processes nor in the shell's history.
        link_or_code: Option<String>,
        #[command(flatten)]
        net: NetArgs,
    },
    /// Accept a friend request. The other device learns it the next time both are online.
    Accept { who: String },
    /// Decline a friend request. The other device is not told.
    Decline { who: String },
    /// Forget a friend, or cancel a friend request.
    Remove { who: String },
    /// Stay online for friends: see who is online and answer friend requests.
    Online {
        #[command(flatten)]
        net: NetArgs,
    },
}

/// A device with its name and fingerprint, as shown to people.
pub fn describe(name: &str, device: &DevicePublicKey) -> String {
    format!("{name} ({})", device.fingerprint_grouped())
}

/// What a friend's presence says, for people: a room's group only appears when this device
/// is a member of it.
pub fn presence_text(presence: &Presence) -> String {
    match presence {
        Presence::Online => "is online".to_string(),
        Presence::Away => "is away".to_string(),
        Presence::InRoom {
            group: Some((_, name)),
        } => format!("is in the room of \"{name}\""),
        Presence::InRoom { group: None } => "is in a room".to_string(),
        Presence::Offline {
            last_seen_ms: Some(at),
        } => format!("was last seen at {} UTC", crate::clock_at(*at)),
        Presence::Offline { last_seen_ms: None } => "is offline".to_string(),
    }
}

/// Finds one device among `candidates` by name or by the start of its fingerprint.
pub fn find<'a>(
    candidates: impl Iterator<Item = (DevicePublicKey, &'a str)>,
    who: &str,
) -> Result<DevicePublicKey> {
    let wanted = who.trim().to_lowercase();
    let print: String = who
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_uppercase();
    let found: Vec<DevicePublicKey> = candidates
        .filter(|(device, name)| {
            name.to_lowercase() == wanted
                || (print.len() >= 4 && device.fingerprint().starts_with(&print))
        })
        .map(|(device, _)| device)
        .collect();
    match found.as_slice() {
        [one] => Ok(*one),
        [] => bail!("nobody called \"{who}\" here (see `fogata friends`)"),
        _ => bail!("several match \"{who}\"; give the fingerprint instead"),
    }
}

pub fn profile(store: Store, args: ProfileArgs) -> Result<()> {
    let (identity, _) = store.identity()?;
    let avatar = match (args.avatar, args.photo) {
        (Some(n), _) => Some(Avatar::Preset(n)),
        (None, Some(path)) => {
            Some(Avatar::Photo(std::fs::read(&path).with_context(|| {
                format!("cannot read {}", path.display())
            })?))
        }
        (None, None) => None,
    };
    let book = match store.friends()? {
        Some(mut book) => {
            if args.name.is_some() || avatar.is_some() {
                book.set_profile(&identity, args.name.as_deref(), avatar, now_ms())?;
                store.save_friends(&book)?;
                println!("Profile updated. Friends see it the next time they connect.");
            }
            book
        }
        None => {
            let name = args
                .name
                .context("choose your name first: fogata profile --name <your name>")?;
            let mut book = FriendBook::new(&identity, &name, now_ms())?;
            if avatar.is_some() {
                book.set_profile(&identity, None, avatar, now_ms())?;
            }
            store.save_friends(&book)?;
            println!("Profile created.");
            book
        }
    };
    show_profile(&book.profile);
    Ok(())
}

fn show_profile(profile: &Profile) {
    let avatar = match &profile.avatar {
        Avatar::Preset(n) => format!("preset {n}"),
        Avatar::Photo(bytes) => format!("picture, {} KiB", bytes.len().div_ceil(1024)),
    };
    println!("Name:        {}", profile.name);
    println!("Avatar:      {avatar}");
    println!("Fingerprint: {}", profile.device.fingerprint_grouped());
}

pub async fn run(store: Store, command: Option<FriendsCommand>, verbose: bool) -> Result<()> {
    let mut book = store.require_friends()?;
    let now = now_ms();
    match command.unwrap_or(FriendsCommand::List) {
        FriendsCommand::List => list(&book),
        FriendsCommand::Accept { who } => {
            let device = find(incoming(&book), &who)?;
            book.answer(&device, true, now);
            store.save_friends(&book)?;
            println!(
                "Accepted {}. They learn it the next time both devices are online (for example with `fogata friends online`).",
                describe(book.name_of(&device).unwrap_or("?"), &device)
            );
        }
        FriendsCommand::Decline { who } => {
            let device = find(incoming(&book), &who)?;
            book.answer(&device, false, now);
            store.save_friends(&book)?;
            println!("Declined.");
        }
        FriendsCommand::Remove { who } => {
            let device = find(everyone(&book), &who)?;
            let name = book.name_of(&device).unwrap_or("?").to_string();
            let was_friend = book.friends.contains_key(&device);
            book.remove(&device, now)?;
            store.save_friends(&book)?;
            println!("Forgot {}.", describe(&name, &device));
            if was_friend {
                println!(
                    "This device has a new friend secret, so they can no longer find it. Your other friends get the new one the next time you are both online."
                );
            }
        }
        FriendsCommand::Invite {
            code,
            accept_first,
            net,
        } => {
            let mode = Mode::Invite { accept_first };
            let session = Session::start(store, &book, net, mode, verbose)?;
            session.invite(code).await?;
        }
        FriendsCommand::Add { link_or_code, net } => {
            let link_or_code = match link_or_code {
                Some(text) => text,
                None => crate::input::secret_line("Link or code from the other device:")?,
            };
            let text = link_or_code.trim();
            if text.starts_with(LINK_PREFIX) {
                FriendLink::decode(text)?;
            } else if text.starts_with("fogata:") {
                bail!("this is a group invite; use `fogata group join`");
            } else {
                FriendCode::parse(text)?;
            }
            let session = Session::start(store, &book, net, Mode::Add, verbose)?;
            session.add(text).await?;
        }
        FriendsCommand::Online { net } => {
            let session = Session::start(store, &book, net, Mode::Online, verbose)?;
            session.online().await?;
        }
    }
    Ok(())
}

fn incoming(book: &FriendBook) -> impl Iterator<Item = (DevicePublicKey, &str)> {
    book.incoming
        .iter()
        .map(|(device, request)| (*device, request.profile.name.as_str()))
}

fn everyone(book: &FriendBook) -> impl Iterator<Item = (DevicePublicKey, &str)> {
    let friends = book
        .friends
        .iter()
        .map(|(device, friend)| (*device, friend.profile.name.as_str()));
    let outgoing = book
        .outgoing
        .iter()
        .map(|(device, request)| (*device, request.name.as_str()));
    friends.chain(incoming(book)).chain(outgoing)
}

/// Friends, for picking one by name or fingerprint.
pub fn friends_of(book: &FriendBook) -> impl Iterator<Item = (DevicePublicKey, &str)> {
    book.friends
        .iter()
        .map(|(device, friend)| (*device, friend.profile.name.as_str()))
}

fn list(book: &FriendBook) {
    println!(
        "You are {}.",
        describe(&book.profile.name, &book.profile.device)
    );
    if book.friends.is_empty() && book.incoming.is_empty() && book.outgoing.is_empty() {
        println!("No friends yet. Make an invite with: fogata friends invite");
        return;
    }
    if !book.friends.is_empty() {
        println!("Friends ({}):", book.friends.len());
        for (device, friend) in &book.friends {
            let note = if friend.delivered {
                ""
            } else {
                "  (they get your acceptance the next time both are online)"
            };
            println!(
                "  {:<20} {}{note}",
                friend.profile.name,
                device.short_fingerprint()
            );
        }
    }
    if !book.incoming.is_empty() {
        println!("Requests to answer ({}):", book.incoming.len());
        for (device, request) in &book.incoming {
            println!(
                "  {:<20} {}  (fogata friends accept|decline {})",
                request.profile.name,
                device.fingerprint_grouped(),
                device.short_fingerprint().replace('·', "")
            );
        }
    }
    if !book.outgoing.is_empty() {
        println!("Requests sent ({}):", book.outgoing.len());
        for (device, request) in &book.outgoing {
            println!(
                "  {:<20} {}  waiting for them to accept",
                request.name,
                device.short_fingerprint()
            );
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Invite { accept_first: bool },
    Add,
    Online,
}

/// The core running for the friends commands, with friend requests answered on standard
/// input. Every group is online too, so a friend added to a group gets it from here.
struct Session {
    core: Core,
    events: fogata_core::CoreEvents,
    lines: UnboundedReceiver<String>,
    stdin_open: bool,
    prompts: VecDeque<Profile>,
    mode: Mode,
    use_dht: bool,
    verbose: bool,
    /// Presence last shown, to print only changes.
    shown: HashMap<DevicePublicKey, Presence>,
}

/// What a session waits for.
enum Goal {
    /// A friend added during the session comes online.
    NewFriendOnline,
    Never,
}

impl Session {
    fn start(
        store: Store,
        book: &FriendBook,
        net: NetArgs,
        mode: Mode,
        verbose: bool,
    ) -> Result<Self> {
        let use_dht = net.use_dht();
        println!(
            "You are {}.",
            describe(&book.profile.name, &book.profile.device)
        );
        let (core, events) = Core::start(store.dir().to_path_buf(), net.options())?;
        Ok(Self {
            core,
            events,
            lines: input::lines(),
            stdin_open: true,
            prompts: VecDeque::new(),
            mode,
            use_dht,
            verbose,
            shown: HashMap::new(),
        })
    }

    async fn invite(mut self, code: bool) -> Result<()> {
        if code {
            let code = self.core.friend_code().await?.text;
            println!("Friend code for one person, valid for 10 minutes:");
            println!();
            println!("    {code}");
            println!();
            println!("On their device: fogata friends add {code}");
        } else {
            // Wait a moment so that the link carries an address others confirmed.
            if self.use_dht {
                say!("· finding this device's public address");
                let deadline = Instant::now() + Duration::from_secs(10);
                while let Ok(Some(event)) =
                    tokio::time::timeout_at(deadline, self.events.recv()).await
                {
                    if matches!(event, CoreEvent::Address(_)) {
                        break;
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            let link = self.core.friend_link().await?.text;
            println!("Friend link for one person, valid for 10 minutes:");
            print_qr(&link);
            println!("{link}");
        }
        say!("· waiting for a friend request (Ctrl+C to stop)");
        self.run(Goal::NewFriendOnline, Some(WAIT)).await
    }

    async fn add(mut self, text: &str) -> Result<()> {
        match FriendLink::decode(text) {
            Ok(link) => say!("· looking for {}", describe(&link.name, &link.device)),
            Err(_) => say!(
                "· looking for the device showing {}",
                FriendCode::parse(text)?
            ),
        }
        self.core.add_friend(text).await?;
        self.run(Goal::NewFriendOnline, Some(FIND)).await
    }

    async fn online(mut self) -> Result<()> {
        say!("· online for friends; answer requests with y or n, /remove <who>, /quit to stop");
        self.run(Goal::Never, None).await
    }

    fn prompt(&self) {
        if let Some(profile) = self.prompts.front() {
            say!(
                "? {} wants to be friends. Check that their device shows this fingerprint. Accept? [y/N]",
                describe(&profile.name, &profile.device)
            );
        }
    }

    async fn run(&mut self, goal: Goal, limit: Option<Duration>) -> Result<()> {
        let mut deadline = limit.map(|d| Instant::now() + d);
        let mut added: Vec<DevicePublicKey> = Vec::new();
        loop {
            let sleep = async {
                match deadline {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                _ = sleep => {
                    if self.mode == Mode::Add && !added.is_empty() {
                        say!("· no answer yet; it arrives the next time both devices are online");
                        break;
                    }
                    match self.mode {
                        Mode::Add => bail!(
                            "the other device was not found; check the link or code and that it is still shown there"
                        ),
                        _ => {
                            say!("· no friend request arrived");
                            return Ok(());
                        }
                    }
                }
                line = self.lines.recv(), if self.stdin_open => {
                    let Some(line) = line else {
                        self.stdin_open = false;
                        continue;
                    };
                    let line = line.trim().to_lowercase();
                    if line == "/quit" {
                        break;
                    }
                    if let Some(who) = line.strip_prefix("/remove ") {
                        let friends = self.core.friends().await?;
                        let candidates = friends.iter().map(|f| (f.person.device, f.person.name.as_str()));
                        match find(candidates, who) {
                            Ok(device) => {
                                let name = self.name_of(&device).await;
                                self.core.remove_friend(device).await?;
                                say!("· forgot {name}");
                            }
                            Err(e) => say!("· {e}"),
                        }
                        continue;
                    }
                    if let Some(profile) = self.prompts.pop_front() {
                        let accept = matches!(line.as_str(), "y" | "yes");
                        self.core.answer_request(profile.device, accept).await?;
                        if !accept {
                            say!("· declined");
                        }
                        self.prompt();
                    }
                }
                event = self.events.recv() => {
                    let Some(event) = event else { break };
                    match event {
                        CoreEvent::FriendsChanged => self.show_presence().await?,
                        CoreEvent::FriendRequest { profile } => {
                            if let Mode::Invite { accept_first: true } = self.mode
                                && added.is_empty()
                            {
                                say!("· accepting {}", describe(&profile.name, &profile.device));
                                self.core.answer_request(profile.device, true).await?;
                                continue;
                            }
                            self.prompts.push_back(profile);
                            if self.prompts.len() == 1 {
                                self.prompt();
                            }
                            // Give the person time to answer.
                            deadline = deadline.map(|_| Instant::now() + WAIT);
                        }
                        CoreEvent::FriendRequestSent { device, name } => {
                            say!(
                                "· request delivered to {}; waiting for them to accept",
                                describe(&name, &device)
                            );
                            say!("  (if you stop now, their answer arrives the next time both devices are online)");
                            added.push(device);
                            deadline = deadline.map(|_| Instant::now() + WAIT);
                        }
                        CoreEvent::FriendRequestFailed(reason) => {
                            if self.mode == Mode::Add {
                                bail!("{reason}");
                            }
                            say!("· {reason}");
                        }
                        CoreEvent::FriendAdded { profile } => {
                            say!("+ {} and you are friends now", profile.name);
                            added.push(profile.device);
                        }
                        CoreEvent::FriendOnline { device } => {
                            let name = self.name_of(&device).await;
                            let path = self
                                .core
                                .friends()
                                .await?
                                .into_iter()
                                .find(|f| f.person.device == device)
                                .and_then(|f| f.path)
                                .map(|p| format!(" ({p})"))
                                .unwrap_or_default();
                            say!("+ {name} is online{path}");
                            self.shown.insert(device, Presence::Online);
                            if matches!(goal, Goal::NewFriendOnline) && added.contains(&device) {
                                break;
                            }
                        }
                        CoreEvent::FriendOffline { device } => {
                            let name = self.name_of(&device).await;
                            say!("- {name} went offline");
                            self.shown.remove(&device);
                        }
                        CoreEvent::GroupInvited { name, by, .. } => {
                            let who = self.name_of(&by).await;
                            say!("+ {who} added you to the group \"{name}\"; join with: fogata group accept \"{name}\"");
                        }
                        CoreEvent::InvitationDeclined { name, who, .. } => {
                            say!("- {who} declined the invitation to the group \"{name}\"");
                        }
                        CoreEvent::Address(addr) if self.verbose => {
                            say!("· other peers see this device at {addr}");
                        }
                        CoreEvent::Info(text) if self.verbose => say!("· {text}"),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Prints friends that entered or left a room since last shown.
    async fn show_presence(&mut self) -> Result<()> {
        for friend in self.core.friends().await? {
            let device = friend.person.device;
            let shown = self.shown.get(&device);
            let entering = matches!(friend.presence, Presence::InRoom { .. })
                && shown != Some(&friend.presence);
            let leaving = matches!(shown, Some(Presence::InRoom { .. }))
                && matches!(friend.presence, Presence::Online | Presence::Away);
            if entering || leaving {
                say!(
                    "· {} [{}] {}",
                    friend.person.name,
                    friend.person.short,
                    presence_text(&friend.presence)
                );
            }
            if friend.path.is_some() {
                self.shown.insert(device, friend.presence);
            }
        }
        Ok(())
    }

    async fn name_of(&self, device: &DevicePublicKey) -> String {
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

/// Prints `text` as a QR code drawn with block characters, light on a dark terminal.
fn print_qr(text: &str) {
    let Ok(code) = QrCode::new(text.as_bytes()) else {
        return;
    };
    let image = code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Light)
        .light_color(Dense1x2::Dark)
        .quiet_zone(true)
        .build();
    println!("{image}");
}
