//! Fogata command-line client.

#![forbid(unsafe_code)]

/// Prints a line prefixed with the current UTC time, so that test logs from several
/// machines can be lined up.
macro_rules! say {
    ($($arg:tt)*) => {
        println!("{} {}", $crate::clock(), format_args!($($arg)*))
    };
}

/// Current UTC time as HH:MM:SS.
pub(crate) fn clock() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    clock_at(ms)
}

/// A time in milliseconds since the Unix epoch as HH:MM:SS UTC.
pub(crate) fn clock_at(unix_ms: u64) -> String {
    let secs = unix_ms / 1000 % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

mod bridge;
mod chat;
mod disk;
mod friends;
mod groups;
mod input;
#[cfg(feature = "lab")]
mod lab;
mod network;
mod probe;
mod recovery;
mod release;
#[cfg(feature = "lab")]
mod soak;
mod transfer;

use std::{io::IsTerminal, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
#[cfg(feature = "lab")]
use fogata_net::Multiaddr;

use fogata_core::{Store, store};

#[derive(Parser)]
#[command(name = "fogata", version, about = "Peer-to-peer group chat")]
struct Cli {
    /// Where the identity and groups are stored.
    #[arg(long, global = true, env = "FOGATA_DATA_DIR")]
    data_dir: Option<PathBuf>,

    /// Print network details.
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show this device's identity, creating it on first use.
    Id,
    /// Show or change your profile: the name and avatar friends and groups see.
    Profile(friends::ProfileArgs),
    /// Friends and friend requests. Without a subcommand, lists them.
    Friends {
        #[command(subcommand)]
        command: Option<friends::FriendsCommand>,
    },
    /// Manage groups.
    #[command(subcommand)]
    Group(groups::GroupCommand),
    /// Enter a group's room and chat.
    Chat(chat::ChatArgs),
    /// Move this account to another computer, or bring one here, with a short code.
    #[command(subcommand)]
    Transfer(transfer::TransferCommand),
    /// Copies of this account that restore it without the system keychain.
    #[command(subcommand)]
    Recovery(recovery::RecoveryCommand),
    /// Make, sign, check and publish pre-releases.
    Release(release::ReleaseArgs),
    /// See and install new versions that came from friends and members.
    #[command(subcommand)]
    Update(release::UpdateCommand),
    /// Check this network (router, UDP, IPv6) and explain what it means for reaching friends.
    Network {
        #[command(flatten)]
        net: chat::NetArgs,
    },
    /// Carry a local WebRTC engine's media to members through a local TURN endpoint.
    #[command(hide = true)]
    MediaBridge(bridge::BridgeArgs),
    /// Measure how real-time packets travel to a member.
    #[command(hide = true)]
    MediaProbe(probe::ProbeArgs),
    /// Run a DHT node for the network lab.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabDht {
        #[arg(long)]
        port: u16,
        #[arg(long = "peer")]
        peers: Vec<Multiaddr>,
        /// Write every announcement taken to this file, one per line.
        #[arg(long)]
        records: Option<PathBuf>,
        /// Write the protocols each peer lists in identify to this file, one peer per line.
        #[arg(long)]
        identified: Option<PathBuf>,
        /// Answer every lookup with this many made-up peers, new ones each time.
        #[arg(long, requires = "lie_at")]
        lies: Option<usize>,
        /// The addresses the made-up peers are said to be at.
        #[arg(long = "lie-at")]
        lie_at: Vec<Multiaddr>,
    },
    /// Network lab only: a peer that tells everyone it meets a false address of theirs.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabLiar {
        #[arg(long = "dial")]
        dial: Vec<Multiaddr>,
        /// The address it says the others have.
        #[arg(long)]
        says: Multiaddr,
    },
    /// Network lab only: a stranger that keeps many connections open to a device and says
    /// something cheap to it every half minute.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabFlood {
        #[arg(long)]
        dial: Multiaddr,
        /// The address to listen and dial from.
        #[arg(long)]
        listen: Multiaddr,
        #[arg(long, default_value_t = 32)]
        connections: usize,
    },
    /// Network lab only: a machine that keeps many TCP connections open to a device without
    /// ever saying anything on them.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabSilent {
        #[arg(long)]
        dial: std::net::SocketAddr,
        /// An address to connect from; each one keeps its own connections.
        #[arg(long = "from")]
        from: Vec<std::net::IpAddr>,
        #[arg(long, default_value_t = 512)]
        per_address: usize,
    },
    /// Network lab only: a neighbour that answers every mDNS query with made-up peers.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabMdnsFlood {
        /// The address of the interface on the local network.
        #[arg(long)]
        interface: std::net::Ipv4Addr,
        /// Responses to each query.
        #[arg(long, default_value_t = 4)]
        answers: usize,
        /// Made-up peers in each response.
        #[arg(long, default_value_t = 20)]
        per_answer: usize,
        /// Responses a second that answer no query.
        #[arg(long, default_value_t = 50)]
        unasked: usize,
    },
    /// Network lab only: open the sealed local addresses in a lab DHT node's records with
    /// this device's secrets.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabOpen { records: PathBuf },
    /// Network lab only: put a release folder into the data folder as it is, unchecked.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabPlantRelease { from: PathBuf },
    /// Network lab only: sign a manifest's bytes as they are, with no check.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabSignRaw {
        #[arg(long)]
        key: PathBuf,
        body: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Network lab only: as a stranger, ask the devices dialled for releases.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabUpdateProbe {
        #[command(flatten)]
        net: chat::NetArgs,
        #[arg(long, default_value = "linux-flatpak")]
        platform: String,
    },
    /// Lab only: write numbered messages to every friend and in a room at a steady pace, and
    /// print what happens, for long runs on a misbehaving network.
    #[cfg(feature = "lab")]
    #[command(hide = true)]
    LabSoak(soak::SoakArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = if cli.verbose {
        "warn,fogata=debug,fogata_net=debug"
    } else {
        "error"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();

    // A folder the application sealed opens with its key from the system keychain; the
    // command line never seals one itself, so test folders put nothing in the keychain.
    fogata_core::vault::use_keychain(fogata_core::vault::Keychain::Open(std::sync::Arc::new(
        fogata_core::vault::SystemKeychain,
    )));
    let store = || match cli.data_dir.clone() {
        Some(dir) => Store::open(dir),
        None => Store::open(
            store::default_data_dir().context("no data folder; give one with --data-dir")?,
        ),
    };

    match cli.command {
        Command::Id => {
            let (identity, created) = store()?.identity()?;
            if created {
                println!("Created a new identity for this device.");
            }
            println!(
                "Device key: {}",
                hex::encode(identity.public_key().as_bytes())
            );
            println!("Short form: {}", identity.public_key().fingerprint());
        }
        Command::Profile(args) => friends::profile(store()?, args)?,
        Command::Friends { command } => friends::run(store()?, command, cli.verbose).await?,
        Command::Group(command) => groups::run(store()?, command).await?,
        Command::Chat(args) => chat::run(store()?, args).await?,
        Command::Transfer(command) => transfer::run(store()?.dir(), command).await?,
        Command::Recovery(command) => recovery::run(store()?.dir(), command)?,
        Command::Release(command) => release::release(store, command).await?,
        Command::Update(command) => release::update(store()?, command).await?,
        #[cfg(feature = "lab")]
        Command::LabPlantRelease { from } => release::plant(store()?, &from)?,
        #[cfg(feature = "lab")]
        Command::LabSignRaw { key, body, out } => release::sign_raw(&key, &body, &out)?,
        #[cfg(feature = "lab")]
        Command::LabUpdateProbe { net, platform } => release::probe(net, platform).await?,
        Command::Network { net } => network::run(store()?, net).await?,
        Command::MediaProbe(args) => probe::run(store()?, args).await?,
        Command::MediaBridge(args) => bridge::run(store()?, args).await?,
        #[cfg(feature = "lab")]
        Command::LabDht {
            port,
            peers,
            records,
            identified,
            lies,
            lie_at,
        } => {
            let lies = lies.map(|count| (count, lie_at));
            fogata_net::run_lab_dht(port, peers, records, identified, lies, |addr| {
                println!("{addr}")
            })
            .await?;
        }
        #[cfg(feature = "lab")]
        Command::LabLiar { dial, says } => {
            fogata_net::run_lab_liar(dial, says, |line| say!("{line}")).await?;
        }
        #[cfg(feature = "lab")]
        Command::LabFlood {
            dial,
            listen,
            connections,
        } => {
            fogata_net::run_lab_flood(dial, listen, connections, |line| say!("{line}")).await?;
        }
        #[cfg(feature = "lab")]
        Command::LabSilent {
            dial,
            from,
            per_address,
        } => {
            fogata_net::run_lab_silent(dial, from, per_address, |line| say!("{line}")).await?;
        }
        #[cfg(feature = "lab")]
        Command::LabMdnsFlood {
            interface,
            answers,
            per_answer,
            unasked,
        } => {
            fogata_net::run_lab_mdns_flood(interface, answers, per_answer, unasked, |line| {
                say!("{line}")
            })
            .await?;
        }
        #[cfg(feature = "lab")]
        Command::LabOpen { records } => lab::open_records(store()?, &records)?,
        #[cfg(feature = "lab")]
        Command::LabSoak(args) => soak::run(store()?, args).await?,
    }
    Ok(())
}

#[cfg(test)]
mod lab_tests {
    use clap::CommandFactory;

    /// Every subcommand and long option of the program, at any depth.
    fn names(command: &clap::Command, out: &mut Vec<String>) {
        out.push(command.get_name().to_string());
        out.extend(
            command
                .get_arguments()
                .filter_map(|a| a.get_long().map(str::to_string)),
        );
        for sub in command.get_subcommands() {
            names(sub, out);
        }
    }

    fn lab_names() -> Vec<String> {
        let mut all = Vec::new();
        names(&super::Cli::command(), &mut all);
        // `lab` itself, or `lab-…`: not a name that only begins the same way (`label`).
        all.into_iter()
            .filter(|n| n == "lab" || n.starts_with("lab-"))
            .collect()
    }

    /// The program that ships has none of the network lab's tools or switches (every
    /// `lab` or `lab-*` subcommand, every `--lab` or `--lab-*` option): they plant
    /// releases, sign anything and flood peers. tools/check.sh runs the tests with the
    /// feature and without it.
    #[cfg(not(feature = "lab"))]
    #[test]
    fn the_program_that_ships_has_no_lab_tools() {
        assert_eq!(lab_names(), Vec::<String>::new());
    }

    /// The lab's own copy has them, so the test above sees them when they are there.
    #[cfg(feature = "lab")]
    #[test]
    fn the_labs_copy_has_its_tools() {
        let found = lab_names();
        for name in ["lab-dht", "lab-plant-release", "lab-sign-raw", "lab"] {
            assert!(found.iter().any(|n| n == name), "{name} missing: {found:?}");
        }
    }
}
