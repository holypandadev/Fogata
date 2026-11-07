//! Runs a node with the local TURN endpoint, so that a WebRTC engine on this device can call
//! members over Fogata's own connections.

use std::{net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use fogata_media::bridge::{self, BridgeConfig};
use fogata_net::{GroupSetup, NodeEvent};

use fogata_core::Store;

use crate::chat::NetArgs;

#[derive(clap::Args)]
pub struct BridgeArgs {
    /// Group whose members can be called.
    group: String,
    /// Local UDP port for the WebRTC engine's TURN client.
    #[arg(long, default_value_t = 3478)]
    turn_port: u16,
    /// TURN user name and password; random ones (printed) by default. A connection for a
    /// member authenticates as this user, a dot and the member's device key in hex (printed
    /// as the member connects). Any program on this device can reach the endpoint, and with
    /// them it could send into members' calls.
    #[arg(long)]
    turn_user: Option<String>,
    #[arg(long)]
    turn_pass: Option<String>,
    /// Test only: share of outgoing media datagrams to drop (0 to 1).
    #[cfg(feature = "lab")]
    #[arg(long, default_value_t = 0.0)]
    lab_loss: f64,
    /// Test only: delay added to outgoing media datagrams, in milliseconds.
    #[cfg(feature = "lab")]
    #[arg(long, default_value_t = 0)]
    lab_delay_ms: u64,
    /// Test only: random extra delay of up to this many milliseconds.
    #[cfg(feature = "lab")]
    #[arg(long, default_value_t = 0)]
    lab_jitter_ms: u64,
    /// Stop after this many seconds (0 = run until stopped).
    #[arg(long, default_value_t = 0)]
    seconds: u64,
    #[command(flatten)]
    net: NetArgs,
}

/// `bytes` random bytes in hex.
fn random_hex(bytes: usize) -> Result<String> {
    let mut out = vec![0u8; bytes];
    fogata_crypto::random_bytes(&mut out)?;
    Ok(hex::encode(out))
}

/// What `--lab-loss`, `--lab-delay-ms` and `--lab-jitter-ms` ask for; none without the lab
/// feature.
#[cfg(feature = "lab")]
fn impairment(args: &BridgeArgs) -> Option<bridge::Impairment> {
    (args.lab_loss > 0.0 || args.lab_delay_ms > 0 || args.lab_jitter_ms > 0).then_some(
        bridge::Impairment {
            loss: args.lab_loss,
            delay: Duration::from_millis(args.lab_delay_ms),
            jitter: Duration::from_millis(args.lab_jitter_ms),
        },
    )
}

#[cfg(not(feature = "lab"))]
fn impairment(_: &BridgeArgs) -> Option<bridge::Impairment> {
    None
}

/// The TURN user name and password: those given, or random ones.
fn credentials(args: &BridgeArgs) -> Result<(String, String)> {
    Ok((
        args.turn_user.clone().map_or_else(|| random_hex(8), Ok)?,
        args.turn_pass.clone().map_or_else(|| random_hex(16), Ok)?,
    ))
}

pub async fn run(store: Store, args: BridgeArgs) -> Result<()> {
    let (turn_user, turn_pass) = credentials(&args)?;
    let impairment = impairment(&args);
    let (identity, _) = store.identity()?;
    let log = store.load_log(&store.group(&args.group)?)?;
    let (node, mut events) = fogata_net::start(args.net.node_config(
        identity,
        vec![GroupSetup::Member(log)],
        None,
    ))?;
    let listen = SocketAddr::from(([127, 0, 0, 1], args.turn_port));
    let bridge = bridge::start(
        node.clone(),
        BridgeConfig {
            listen,
            username: turn_user.clone(),
            password: turn_pass.clone(),
            impairment,
            streams_only: false,
            unanswered: None,
        },
    )
    .await
    .context("starting the local TURN endpoint")?;
    let relay = bridge::relay_address(&node.local_peer());
    say!(
        "· TURN for the local WebRTC engine: turn:{listen}?transport=udp pass={turn_pass} (relay address {relay}); a connection for a member takes the user printed with it"
    );

    let run = async {
        while let Some(event) = events.recv().await {
            match event {
                NodeEvent::MemberConnected { device, path, .. } => {
                    say!(
                        "+ member {} connected ({path}), TURN user {}",
                        device.fingerprint(),
                        bridge::member_user(&turn_user, device.as_bytes())
                    );
                }
                NodeEvent::MemberDisconnected { device, .. } => {
                    say!("- member {} disconnected", device.fingerprint());
                }
                NodeEvent::Info(text) => say!("· {text}"),
                _ => {}
            }
        }
    };
    if args.seconds > 0 {
        let _ = tokio::time::timeout(Duration::from_secs(args.seconds), run).await;
    } else {
        run.await;
    }
    bridge.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn args(extra: &[&str]) -> BridgeArgs {
        #[derive(Parser)]
        struct Cli {
            #[command(flatten)]
            bridge: BridgeArgs,
        }
        let mut line = vec!["fogata", "g"];
        line.extend(extra);
        Cli::try_parse_from(line).unwrap().bridge
    }

    /// Any program on this device can reach the TURN endpoint: without credentials given,
    /// each run makes its own, which nobody can guess, rather than a known pair.
    #[test]
    fn turn_credentials_are_random_unless_given() {
        let (user, pass) = credentials(&args(&[])).unwrap();
        let (user2, pass2) = credentials(&args(&[])).unwrap();
        assert_eq!((user.len(), pass.len()), (16, 32));
        assert!(user != user2 && pass != pass2);
        assert!(pass.chars().all(|c| c.is_ascii_hexdigit()));
        let given = args(&["--turn-user", "me", "--turn-pass", "secret"]);
        assert_eq!(
            credentials(&given).unwrap(),
            ("me".to_string(), "secret".to_string())
        );
    }
}
