//! Long-run lab helper (`lab-soak`): runs the core like the application, writes numbered
//! direct messages to every friend and numbered lines in a group's room at a steady pace, and
//! prints what happens as lines that a script can check afterwards: connections coming and
//! going, every room line from the others, and what each chat holds. Never needed outside
//! tests.

use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use fogata_core::{
    Core, CoreEvent, FIRST_ROOM, Store,
    direct::{Outgoing, model::Body},
};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_net::RoomChange;

use crate::chat::NetArgs;

#[derive(clap::Args)]
pub struct SoakArgs {
    /// Milliseconds between two rounds of messages (one to each friend, one in the room).
    #[arg(long, default_value_t = 5000)]
    every: u64,
    /// Rounds to send before only listening (none: for ever).
    #[arg(long)]
    rounds: Option<u64>,
    /// Group whose first room to enter and talk in.
    #[arg(long)]
    group: Option<String>,
    /// A file to attach to every `attach_every`th message.
    #[arg(long)]
    attach: Option<PathBuf>,
    #[arg(long, default_value_t = 10)]
    attach_every: u64,
    /// Print every chat with its items and stop, without connecting to anyone.
    #[arg(long)]
    dump: bool,
    #[command(flatten)]
    net: NetArgs,
}

fn now() -> u64 {
    fogata_core::now_ms()
}

/// The number in a message this helper wrote: `soak <n>`.
fn number(text: &str) -> Option<u64> {
    text.strip_prefix("soak ")?.parse().ok()
}

pub async fn run(store: Store, args: SoakArgs) -> Result<()> {
    let dir = store.dir().to_path_buf();
    drop(store);
    let mut options = args.net.options();
    if args.dump {
        options.use_dht = false;
        options.use_mdns = false;
        options.bootstrap.clear();
        options.peers.clear();
    }
    let (core, mut events) = Core::start(dir, options)?;
    let me = core.me();
    let friends: Vec<(DevicePublicKey, String)> = core
        .friends()
        .await?
        .into_iter()
        .map(|f| (f.person.device, f.person.name))
        .collect();
    if args.dump {
        for (device, name) in &friends {
            let chat = core.direct_chat(*device, usize::MAX).await?;
            for view in chat.items {
                let item = view.item;
                let Body::Message { text, .. } = item.body else {
                    continue;
                };
                let who = if item.mine { "mine" } else { "theirs" };
                println!(
                    "item {name} {who} delivered={} seq={} sent={} {text}",
                    item.delivered, item.seq, item.sent_ms
                );
                for file in view.files {
                    let path = file.path.map(|p| p.display().to_string());
                    println!(
                        "file {name} {who} {text} size={} local={:?} path={}",
                        file.file.size,
                        file.file.local,
                        path.unwrap_or_else(|| "-".into())
                    );
                }
            }
        }
        core.shutdown().await?;
        return Ok(());
    }

    let name = core
        .profile()
        .await?
        .map(|p| p.name)
        .unwrap_or_else(|| "anon".into());
    let group = match &args.group {
        Some(local) => {
            let group = core.group_id(local)?;
            core.join_room(group).await?;
            Some(group)
        }
        None => None,
    };
    // Numbering goes on from what this device wrote before it last stopped.
    let mut next = 1;
    for (device, _) in &friends {
        let chat = core.direct_chat(*device, usize::MAX).await?;
        for view in chat.items {
            if let (true, Body::Message { text, .. }) = (view.item.mine, &view.item.body) {
                next = next.max(number(text).map_or(0, |n| n + 1));
            }
        }
    }
    let boot = now();
    println!(
        "T {boot} start me={} name={name} friends={} next={next}",
        me.fingerprint(),
        friends.len()
    );

    let mut tick = tokio::time::interval(Duration::from_millis(args.every.max(1)));
    // After a pause (a stopped process, a sleeping computer) it goes on at its pace, as a
    // person would, instead of writing every missed round in the same millisecond.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut stats = tokio::time::interval(Duration::from_secs(10));
    let mut sent = 0;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = tick.tick(), if args.rounds.is_none_or(|r| sent < r) => {
                let files = match &args.attach {
                    Some(path) if next % args.attach_every.max(1) == 0 => vec![Outgoing {
                        path: path.clone(),
                        video: None,
                    }],
                    _ => Vec::new(),
                };
                for (device, who) in &friends {
                    match core.send_direct(*device, &format!("soak {next}"), files.clone()).await {
                        Ok(()) => println!("T {} dm-sent {who} {next}", now()),
                        Err(e) => println!("T {} dm-error {who} {next} {e:#}", now()),
                    }
                }
                if let Some(group) = group
                    && let Err(e) = core.say(group, &format!("room {name} {boot} {next}")).await
                {
                    println!("T {} room-error {next} {e:#}", now());
                }
                next += 1;
                sent += 1;
            }
            _ = stats.tick() => {
                for (device, who) in &friends {
                    let chat = core.direct_chat(*device, usize::MAX).await?;
                    let (mut mine, mut delivered, mut theirs) = (0, 0, 0);
                    for view in &chat.items {
                        if let Body::Message { .. } = view.item.body {
                            if view.item.mine {
                                mine += 1;
                                delivered += u32::from(view.item.delivered);
                            } else {
                                theirs += 1;
                            }
                        }
                    }
                    println!(
                        "T {} stat {who} mine={mine} delivered={delivered} theirs={theirs} connected={}",
                        now(),
                        chat.connected
                    );
                }
            }
            event = events.recv() => {
                let Some(event) = event else { break };
                show(&friends, group, me, event);
            }
        }
    }
    if let Some(group) = group {
        core.leave_room(group).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    core.shutdown().await?;
    Ok(())
}

fn show(
    friends: &[(DevicePublicKey, String)],
    group: Option<GroupId>,
    me: DevicePublicKey,
    event: CoreEvent,
) {
    let friend = |device: &DevicePublicKey| {
        friends
            .iter()
            .find(|(d, _)| d == device)
            .map_or_else(|| device.short_fingerprint(), |(_, n)| n.clone())
    };
    let t = now();
    match event {
        CoreEvent::FriendOnline { device } => println!("T {t} friend-on {}", friend(&device)),
        CoreEvent::FriendOffline { device } => println!("T {t} friend-off {}", friend(&device)),
        CoreEvent::MemberConnected {
            group: g,
            device,
            path,
        } if Some(g) == group => println!("T {t} member-on {} {path}", friend(&device)),
        CoreEvent::MemberDisconnected { group: g, device } if Some(g) == group => {
            println!("T {t} member-off {}", friend(&device));
        }
        CoreEvent::Room {
            group: g,
            voice,
            change,
        } if Some(g) == group && voice == FIRST_ROOM => match change {
            RoomChange::Chat(line) if line.sender != me => {
                println!("T {t} room-in {} sent={}", line.text, line.timestamp_ms);
            }
            RoomChange::Joined { device, .. } if device != me => {
                println!("T {t} room-joined {}", friend(&device));
            }
            RoomChange::Left { device, on_purpose } if device != me => {
                println!(
                    "T {t} room-left {} on_purpose={on_purpose}",
                    friend(&device)
                );
            }
            _ => {}
        },
        CoreEvent::DirectMessage { friend: from, text } => {
            println!("T {t} dm-in {} {text}", friend(&from));
        }
        CoreEvent::Notice(text) => println!("T {t} notice {text}"),
        CoreEvent::Stopped => println!("T {t} stopped"),
        _ => {}
    }
}
