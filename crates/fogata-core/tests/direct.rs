//! Direct chats between two friends in one process: messages both ways with receipts and
//! typing indicators, messages written while the other is away that go once both are online
//! (also after a restart), the disappearing timer, pinned messages, attachments fetched by
//! themselves or when asked, and a chat that can only be read once the two stop being
//! friends. The devices find each other by dialling each other's loopback address.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use fogata_core::CoreEvents;
use fogata_core::{
    Core, CoreEvent, NetOptions, Store,
    direct::{
        ChatView, Outgoing, VoiceNote,
        model::{Body, DAY_MS, Local, MAX_PINS},
    },
};
use fogata_crypto::DevicePublicKey;
use fogata_friends::Friend;
use fogata_net::Multiaddr;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and many
/// at once crowd it until a pin or a receipt waits for the next round.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn wait_for(events: &mut CoreEvents, what: &str, mut done: impl FnMut(&CoreEvent) -> bool) {
    let found = timeout(Duration::from_secs(60), async {
        while let Some(event) = events.recv().await {
            if done(&event) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(found, "timed out waiting for {what}");
}

/// Polls a chat until `check` holds, for up to 60 s: what waits for a receipt goes again
/// at a tick every 20 s once 15 s went by since it last went, so up to 35 s after.
async fn chat_until(
    core: &Core,
    friend: DevicePublicKey,
    what: &str,
    check: impl Fn(&ChatView) -> bool,
) -> ChatView {
    let found = timeout(Duration::from_secs(60), async {
        loop {
            if let Ok(chat) = core.direct_chat(friend, 100).await
                && check(&chat)
            {
                return chat;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    found.unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

fn texts(chat: &ChatView) -> Vec<String> {
    chat.items
        .iter()
        .filter_map(|i| match &i.item.body {
            Body::Message { text, .. } => Some(text.clone()),
            Body::Timer { .. } | Body::Pin { .. } | Body::Unsend { .. } | Body::Call { .. } => None,
        })
        .collect()
}

/// A voice message's file of `seconds` of a tone, as the application records one.
fn ogg(seconds: usize) -> Vec<u8> {
    let mut recorder = fogata_voice::Recorder::new().unwrap();
    let pcm: Vec<f32> = (0..48_000 * seconds)
        .map(|i| 0.3 * (i as f32 * 0.05).sin())
        .collect();
    recorder.push(&pcm).unwrap();
    recorder.finish(1).unwrap().unwrap().file
}

/// Two data folders whose profiles are already friends.
fn friends(base: &Path) -> (PathBuf, PathBuf) {
    let _ = std::fs::remove_dir_all(base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    let (store_a, store_b) = (
        Store::open(dir_a.clone()).unwrap(),
        Store::open(dir_b.clone()).unwrap(),
    );
    let mut book_a = store_a.friends().unwrap().unwrap();
    let mut book_b = store_b.friends().unwrap().unwrap();
    let friend = |book: &fogata_friends::FriendBook| Friend {
        profile: book.profile.clone(),
        secret: book.secret.clone(),
        since_ms: 2,
        delivered: true,
        banner: None,
        bio: None,
    };
    book_a
        .friends
        .insert(book_b.profile.device, friend(&book_b));
    book_b
        .friends
        .insert(book_a.profile.device, friend(&book_a));
    store_a.save_friends(&book_a).unwrap();
    store_b.save_friends(&book_b).unwrap();
    (dir_a, dir_b)
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Only the loopback address of the other device, when given.
fn alone(port: u16, peer: Option<u16>) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers: peer
            .map(|p| {
                format!("/ip4/127.0.0.1/udp/{p}/quic-v1")
                    .parse::<Multiaddr>()
                    .unwrap()
            })
            .into_iter()
            .collect(),
        ..Default::default()
    }
}

/// Options for Ana and Bea: each listens on a loopback port of its own and dials the other's.
fn dialling() -> (NetOptions, NetOptions) {
    let port_a = free_port();
    let port_b = loop {
        let port = free_port();
        if port != port_a {
            break port;
        }
    };
    (alone(port_a, Some(port_b)), alone(port_b, Some(port_a)))
}

/// Starts a core on a folder another core just let go of: its lock goes with the last of
/// its tasks.
async fn start_again(dir: &Path, options: NetOptions) -> (Core, CoreEvents) {
    for _ in 0..50 {
        match Core::start(dir.to_path_buf(), options.clone()) {
            Ok(started) => return started,
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("the folder stayed locked");
}

/// Waits until `core` shows `friend` online: only then can it write to it.
#[cfg(unix)]
async fn sees_online(core: &Core, friend: DevicePublicKey, what: &str) {
    let seen = timeout(Duration::from_secs(60), async {
        loop {
            let friends = core.friends().await.unwrap();
            if friends.iter().any(|f| {
                f.person.device == friend
                    && !matches!(f.presence, fogata_core::Presence::Offline { .. })
            }) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(seen.is_ok(), "timed out waiting for {what}");
}

async fn stop(core: Core) {
    core.shutdown().await.unwrap();
    drop(core);
}

#[tokio::test(flavor = "multi_thread")]
async fn friends_write_both_ways_and_hear_back() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-talk-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    ana.send_direct(bea.me(), "hi Bea", Vec::new())
        .await
        .unwrap();
    wait_for(&mut events_b, "Ana's message", |e| {
        matches!(e, CoreEvent::DirectMessage { friend, text } if *friend == ana.me() && text == "hi Bea")
    })
    .await;
    let chats = bea.direct_chats().await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!((chats[0].friend.name.as_str(), chats[0].unread), ("Ana", 1));
    assert_eq!(chats[0].last, Some(("hi Bea".to_string(), false)));
    // Bea's device said it keeps it.
    chat_until(&ana, bea.me(), "the message delivered", |c| {
        c.items.last().is_some_and(|i| i.item.delivered)
    })
    .await;

    bea.send_direct(ana.me(), "how are you", Vec::new())
        .await
        .unwrap();
    bea.mark_direct_read(ana.me());
    assert_eq!(bea.direct_chats().await.unwrap()[0].unread, 0);
    let chat = chat_until(&ana, bea.me(), "Bea's answer", |c| texts(c).len() == 2).await;
    assert_eq!(texts(&chat), ["hi Bea", "how are you"]);
    assert!(chat.connected);
    // Bea writes again: Ana sees her typing, then stop. Nothing of it is kept. "Typing"
    // goes only to who takes it on the chat's connection now, at most once in two seconds:
    // said again until Ana sees it.
    let typing = |e: &CoreEvent| matches!(e, CoreEvent::DirectTyping { friend, typing: true } if *friend == bea.me());
    let seen = timeout(Duration::from_secs(60), async {
        loop {
            bea.direct_typing(ana.me(), true).await.unwrap();
            let shown = timeout(Duration::from_millis(2_500), async {
                while let Some(e) = events_a.recv().await {
                    if typing(&e) {
                        return;
                    }
                }
            })
            .await;
            if shown.is_ok() {
                return;
            }
        }
    })
    .await;
    assert!(seen.is_ok(), "timed out waiting for Bea typing");
    bea.direct_typing(ana.me(), false).await.unwrap();
    wait_for(
        &mut events_a,
        "Bea stopped typing",
        |e| matches!(e, CoreEvent::DirectTyping { friend, typing: false } if *friend == bea.me()),
    )
    .await;
    assert_eq!(
        texts(&chat_until(&ana, bea.me(), "the chat", |_| true).await).len(),
        2
    );
    // The chat's group is no group to list.
    assert!(ana.groups().await.unwrap().is_empty());
    assert!(bea.groups().await.unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_written_unless_both_are_online() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-wait-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    // Apart, neither can write to the other: nothing waits for a friend who is away.
    let (ana, mut events_a) = Core::start(dir_a.clone(), alone(port_a, None)).unwrap();
    let (bea, _) = Core::start(dir_b.clone(), alone(port_b, None)).unwrap();
    let (bea_key, ana_key) = (bea.me(), ana.me());
    let refused = ana
        .send_direct(bea_key, "one", Vec::new())
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("Bea is offline"),
        "{refused:#}"
    );
    assert!(
        bea.send_direct(ana_key, "are you there?", Vec::new())
            .await
            .is_err()
    );
    assert!(ana.set_direct_timer(bea_key, DAY_MS).await.is_err());
    assert!(ana.direct_chats().await.unwrap().is_empty());
    assert!(bea.direct_chats().await.unwrap().is_empty());
    stop(bea).await;

    // Bea comes back, dialling Ana: now both write, and everything goes.
    let (bea, mut events_b) = start_again(&dir_b, alone(port_b, Some(port_a))).await;
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    ana.send_direct(bea_key, "one", Vec::new()).await.unwrap();
    ana.send_direct(bea_key, "two", Vec::new()).await.unwrap();
    bea.send_direct(ana_key, "are you there?", Vec::new())
        .await
        .unwrap();
    for (core, friend) in [(&bea, ana_key), (&ana, bea_key)] {
        chat_until(core, friend, "all three delivered", |c| {
            texts(c).len() == 3 && c.items.iter().all(|i| !i.item.mine || i.item.delivered)
        })
        .await;
    }

    // Appearing offline, Ana cannot write either; the chat stays to be read.
    ana.set_hidden(true).await.unwrap();
    let refused = ana
        .send_direct(bea_key, "three", Vec::new())
        .await
        .unwrap_err();
    assert!(
        refused.to_string().contains("appear offline"),
        "{refused:#}"
    );
    assert_eq!(
        texts(&ana.direct_chat(bea_key, 100).await.unwrap()).len(),
        3
    );
    // And Bea, who sees her offline, cannot write to her.
    let hidden = timeout(Duration::from_secs(20), async {
        loop {
            match bea.send_direct(ana_key, "hello?", Vec::new()).await {
                Err(e) if e.to_string().contains("Ana is offline") => return,
                Err(e) => panic!("{e:#}"),
                Ok(()) => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
    })
    .await;
    assert!(hidden.is_ok(), "Bea still writes to Ana appearing offline");
    ana.set_hidden(false).await.unwrap();

    // Kept on disk: after a restart Bea still has them, and can read them while Ana is away.
    stop(ana).await;
    drop(events_a);
    stop(bea).await;
    let (bea, _events_b) = start_again(&dir_b, alone(port_b, None)).await;
    let chat = bea.direct_chat(ana_key, 100).await.unwrap();
    assert!(texts(&chat).contains(&"one".to_string()));
    assert!(
        bea.send_direct(ana_key, "still there?", Vec::new())
            .await
            .is_err()
    );
    drop(bea);
    let _ = std::fs::remove_dir_all(&base);
}

/// A device appearing offline shows no sign of being online through its direct chats: what
/// waits for a receipt is not sent again when the friend connects, and a friend appearing
/// offline is not shown as connected. It all goes once the device shows.
#[tokio::test(flavor = "multi_thread")]
async fn appearing_offline_sends_nothing_through_the_chats() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-hidden-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a.clone(), alone(port_a, None)).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), alone(port_b, Some(port_a))).unwrap();
    let (bea_key, ana_key) = (bea.me(), ana.me());
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    ana.send_direct(bea_key, "one", Vec::new()).await.unwrap();
    chat_until(&ana, bea_key, "the first one delivered", |c| {
        c.items.iter().all(|i| !i.item.mine || i.item.delivered) && c.connected
    })
    .await;

    // A message of Ana's waits for its receipt: Bea's device shows it but cannot keep it
    // (where its chat's journal goes is taken by a folder), so it does not confirm it.
    let journal = dir_b
        .join("direct")
        .join(hex::encode(ana_key.as_bytes()))
        .join("journal");
    let kept = std::fs::read(&journal).unwrap();
    std::fs::remove_file(&journal).unwrap();
    std::fs::create_dir(&journal).unwrap();
    ana.send_direct(bea_key, "two", Vec::new()).await.unwrap();
    chat_until(&bea, ana_key, "Ana's second one shown", |c| {
        texts(c).len() == 2
    })
    .await;
    // Bea, appearing offline, keeps what comes and answers nothing.
    bea.set_hidden(true).await.unwrap();
    std::fs::remove_dir(&journal).unwrap();
    std::fs::write(&journal, kept).unwrap();
    assert!(
        ana.direct_chat(bea_key, 100)
            .await
            .unwrap()
            .items
            .iter()
            .any(|i| i.item.mine && !i.item.delivered),
        "Bea confirmed what she could not keep"
    );
    // Ana appearing offline too, Bea's view shows her neither online nor connected.
    ana.set_hidden(true).await.unwrap();
    chat_until(&bea, ana_key, "Ana not shown as connected", |c| {
        !c.online && !c.connected
    })
    .await;

    // Bea restarts and shows: Ana's chat connects again, and nothing is sent to her.
    stop(bea).await;
    drop(events_b);
    // Past the 15 s after which what waits for a receipt may go again, then a while with Bea
    // back: nothing goes to her.
    tokio::time::sleep(Duration::from_secs(16)).await;
    let (bea, _events_b) = start_again(&dir_b, alone(port_b, Some(port_a))).await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    let chat = ana.direct_chat(bea_key, 100).await.unwrap();
    assert!(
        chat.items.iter().any(|i| i.item.mine && !i.item.delivered),
        "what Ana wrote went to Bea while she appears offline"
    );

    // Ana shows again: it goes at once.
    ana.set_hidden(false).await.unwrap();
    chat_until(&ana, bea_key, "the message delivered", |c| {
        c.items.iter().all(|i| !i.item.mine || i.item.delivered)
    })
    .await;
    stop(ana).await;
    stop(bea).await;
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_timer_is_the_same_for_both_and_messages_disappear() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-timer-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    ana.send_direct(bea.me(), "forever", Vec::new())
        .await
        .unwrap();
    ana.set_direct_timer(bea.me(), DAY_MS).await.unwrap();
    assert!(ana.set_direct_timer(bea.me(), 5).await.is_err());
    chat_until(&bea, ana.me(), "the timer on Bea's side", |c| {
        c.timer_ms == DAY_MS
    })
    .await;
    bea.send_direct(ana.me(), "this goes", Vec::new())
        .await
        .unwrap();
    let chat = chat_until(&ana, bea.me(), "Bea's message", |c| texts(c).len() == 2).await;
    let fleeting = chat.items.last().unwrap().item.clone();
    assert_eq!(fleeting.ttl_ms, DAY_MS);
    // A copy of a damaged journal put aside on Bea's side holds the message too: it goes
    // with it.
    let aside = dir_b
        .join("direct")
        .join(hex::encode(ana.me().as_bytes()))
        .join("journal.damaged");
    std::fs::write(&aside, b"this goes").unwrap();
    // A day later it is gone on both, with the line about the timer and the first message
    // still there.
    let later = fleeting.got_ms.max(fleeting.sent_ms) + DAY_MS + 60_000;
    ana.expire_direct(later).await;
    bea.expire_direct(later).await;
    assert!(!aside.exists());
    let chat = ana.direct_chat(bea.me(), 100).await.unwrap();
    assert_eq!(texts(&chat), ["forever"]);
    assert!(
        chat.items
            .iter()
            .any(|i| matches!(i.item.body, Body::Timer { ms } if ms == DAY_MS))
    );
    assert_eq!(
        texts(&bea.direct_chat(ana.me(), 100).await.unwrap()),
        ["forever"]
    );
    // Nothing of it stays in the journal on disk.
    let journal = std::fs::read(
        dir_b
            .join("direct")
            .join(hex::encode(ana.me().as_bytes()))
            .join("journal"),
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&journal).contains("this goes"));
    let _ = std::fs::remove_dir_all(&base);
}

fn pinned(chat: &ChatView) -> Vec<String> {
    chat.pinned
        .iter()
        .filter_map(|i| match &i.item.body {
            Body::Message { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_deleted_for_both_goes_from_both() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-unsend-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a.clone(), options_a).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    ana.send_direct(bea.me(), "stays", Vec::new())
        .await
        .unwrap();
    ana.send_direct(bea.me(), "oops", Vec::new()).await.unwrap();
    chat_until(&bea, ana.me(), "Ana's messages on Bea's side", |c| {
        texts(c).len() == 2
    })
    .await;
    let chat = ana.direct_chat(bea.me(), 100).await.unwrap();
    let oops = chat
        .items
        .iter()
        .find(|i| matches!(&i.item.body, Body::Message { text, .. } if text == "oops"))
        .unwrap()
        .item
        .id;
    // Only one's own message is deleted for both.
    let theirs = bea.direct_chat(ana.me(), 100).await.unwrap().items[0]
        .item
        .id;
    assert!(bea.unsend_direct(ana.me(), theirs).await.is_err());

    ana.unsend_direct(bea.me(), oops).await.unwrap();
    assert_eq!(
        texts(&ana.direct_chat(bea.me(), 100).await.unwrap()),
        ["stays"]
    );
    chat_until(&bea, ana.me(), "the deletion on Bea's side", |c| {
        texts(c) == ["stays"]
    })
    .await;
    // Once it arrived, nothing of the deletion stays on Ana's side, where it waited with
    // the pins.
    let pins = dir_a
        .join("direct")
        .join(hex::encode(bea.me().as_bytes()))
        .join("pins");
    let confirmed = timeout(Duration::from_secs(60), async {
        while pins.exists() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(confirmed.is_ok(), "the deletion stayed on Ana's side");
    ana.shutdown().await.unwrap();
    bea.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_deleted_here_goes_from_here_only_and_for_good() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-delete-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a.clone(), options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    bea.send_direct(ana.me(), "keep", Vec::new()).await.unwrap();
    bea.send_direct(ana.me(), "delete", Vec::new())
        .await
        .unwrap();
    ana.send_direct(bea.me(), "mine", Vec::new()).await.unwrap();
    let chat = chat_until(&ana, bea.me(), "Bea's messages", |c| texts(c).len() == 3).await;
    let id = |text: &str| {
        chat.items
            .iter()
            .find(|i| matches!(&i.item.body, Body::Message { text: t, .. } if t == text))
            .unwrap()
            .item
            .id
    };
    // The friend's message and this device's own.
    ana.delete_direct_message(bea.me(), id("delete")).unwrap();
    ana.delete_direct_message(bea.me(), id("mine")).unwrap();
    assert_eq!(
        texts(&ana.direct_chat(bea.me(), 100).await.unwrap()),
        ["keep"]
    );
    assert!(ana.delete_direct_message(bea.me(), id("delete")).is_err());
    // Bea keeps her copy.
    let chat = chat_until(&bea, ana.me(), "Ana's message on Bea's side", |c| {
        texts(c).len() == 3
    })
    .await;
    assert_eq!(texts(&chat).len(), 3);

    // Gone after a restart too.
    ana.shutdown().await.unwrap();
    drop(ana);
    let mut started = None;
    for _ in 0..50 {
        match Core::start(dir_a.clone(), options_a.clone()) {
            Ok(s) => {
                started = Some(s);
                break;
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    let (ana, _events) = started.expect("the folder stayed locked");
    assert_eq!(
        texts(&ana.direct_chat(bea.me(), 100).await.unwrap()),
        ["keep"]
    );
    ana.shutdown().await.unwrap();
    bea.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pinned_message_stays_on_both_until_it_is_unpinned() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-pins-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    ana.set_direct_timer(bea.me(), DAY_MS).await.unwrap();
    chat_until(&bea, ana.me(), "the timer on Bea's side", |c| {
        c.timer_ms == DAY_MS
    })
    .await;
    bea.send_direct(ana.me(), "this stays", Vec::new())
        .await
        .unwrap();
    bea.send_direct(ana.me(), "this goes", Vec::new())
        .await
        .unwrap();
    let chat = chat_until(&ana, bea.me(), "Bea's messages", |c| texts(c).len() == 2).await;
    let kept = chat.items.iter().find(|i| i.item.is_message()).unwrap();
    let id = kept.item.id;
    assert!(!kept.pinned);
    ana.pin_direct(bea.me(), id, true).await.unwrap();
    // Pinned on both, with a line for it, which is not the chat's last line.
    let chat = chat_until(&bea, ana.me(), "the pin on Bea's side", |c| {
        pinned(c) == ["this stays"]
    })
    .await;
    assert!(chat.items.iter().any(|i| matches!(
        i.item.body,
        Body::Pin { target, pinned: true } if target == id && !i.item.mine
    )));
    assert_eq!(
        bea.direct_chats().await.unwrap()[0].last,
        Some(("this goes".to_string(), true))
    );
    assert_eq!(
        pinned(&ana.direct_chat(bea.me(), 100).await.unwrap()),
        ["this stays"]
    );

    // A day later only the pinned one is left, on both.
    let later = chat.items.iter().map(|i| i.item.got_ms).max().unwrap() + DAY_MS + 60_000;
    ana.expire_direct(later).await;
    bea.expire_direct(later).await;
    for (core, friend) in [(&ana, bea.me()), (&bea, ana.me())] {
        let chat = core.direct_chat(friend, 100).await.unwrap();
        assert_eq!(texts(&chat), ["this stays"]);
        assert!(chat.items.iter().any(|i| i.pinned));
    }

    // Unpinned by Bea: on both it goes as soon as its time is counted again.
    bea.pin_direct(ana.me(), id, false).await.unwrap();
    chat_until(&ana, bea.me(), "the unpin on Ana's side", |c| {
        c.pinned.is_empty()
    })
    .await;
    chat_until(&bea, ana.me(), "the unpin delivered", |c| {
        c.items.iter().all(|i| !i.item.mine || i.item.delivered)
    })
    .await;
    ana.expire_direct(later).await;
    bea.expire_direct(later).await;
    for (core, friend) in [(&ana, bea.me()), (&bea, ana.me())] {
        let chat = core.direct_chat(friend, 100).await.unwrap();
        assert!(texts(&chat).is_empty());
        assert!(chat.items.iter().all(|i| i.item.pin_target().is_none()));
    }
    assert!(
        !dir_b
            .join("direct")
            .join(hex::encode(ana.me().as_bytes()))
            .join("pins")
            .exists()
    );

    // At most MAX_PINS: one more is refused until one is unpinned.
    ana.set_direct_timer(bea.me(), 0).await.unwrap();
    for n in 0..=MAX_PINS {
        ana.send_direct(bea.me(), &format!("note {n}"), Vec::new())
            .await
            .unwrap();
    }
    // In the order written (several in one millisecond may sit in any order).
    let chat = ana.direct_chat(bea.me(), 200).await.unwrap();
    let ids: Vec<_> = (0..=MAX_PINS)
        .map(|n| {
            chat.items
                .iter()
                .find(|i| matches!(&i.item.body, Body::Message { text, .. } if *text == format!("note {n}")))
                .unwrap()
                .item
                .id
        })
        .collect();
    for id in &ids[..MAX_PINS] {
        ana.pin_direct(bea.me(), *id, true).await.unwrap();
    }
    assert!(ana.pin_direct(bea.me(), ids[MAX_PINS], true).await.is_err());
    ana.pin_direct(bea.me(), ids[0], false).await.unwrap();
    ana.pin_direct(bea.me(), ids[MAX_PINS], true).await.unwrap();
    let chat = chat_until(&bea, ana.me(), "the pins on Bea's side", |c| {
        c.pinned.len() == MAX_PINS && c.pinned[0].item.id == ids[MAX_PINS]
    })
    .await;
    assert!(chat.pinned.iter().all(|i| i.item.id != ids[0]));
    let _ = std::fs::remove_dir_all(&base);
}

/// Bea's message ran out on her side (each device counts from when it got it, so Ana, who
/// got it later, still has it) before Ana pinned it: Bea's device answers with an unpin, so
/// Ana does not keep it pinned, alone, for good; on Ana's side it then goes on its own time.
#[tokio::test(flavor = "multi_thread")]
async fn a_pin_of_a_message_gone_on_the_other_side_is_undone() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-late-pin-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    bea.set_direct_timer(ana.me(), DAY_MS).await.unwrap();
    chat_until(&ana, bea.me(), "the timer on Ana's side", |c| {
        c.timer_ms == DAY_MS
    })
    .await;
    bea.send_direct(ana.me(), "this is gone", Vec::new())
        .await
        .unwrap();
    let chat = chat_until(&ana, bea.me(), "Bea's message", |c| texts(c).len() == 1).await;
    let message = chat.items.iter().find(|i| i.item.is_message()).unwrap();
    let (id, got) = (message.item.id, message.item.got_ms);
    // A day on, on Bea's side only.
    let mine = bea.direct_chat(ana.me(), 100).await.unwrap();
    let written = mine.items.iter().map(|i| i.item.got_ms).max().unwrap();
    bea.expire_direct(written + DAY_MS).await;
    assert!(texts(&bea.direct_chat(ana.me(), 100).await.unwrap()).is_empty());

    ana.pin_direct(bea.me(), id, true).await.unwrap();
    let chat = chat_until(&ana, bea.me(), "Bea's unpin on Ana's side", |c| {
        c.items.iter().any(|i| {
            !i.item.mine
                && matches!(i.item.body, Body::Pin { target, pinned: false } if target == id)
        })
    })
    .await;
    assert!(chat.pinned.is_empty());
    chat_until(&bea, ana.me(), "the unpin delivered", |c| {
        c.items.iter().all(|i| !i.item.mine || i.item.delivered)
    })
    .await;
    // Ana's copy goes on its own time, and nothing of it is left on either side.
    ana.expire_direct(got + DAY_MS).await;
    bea.expire_direct(got + DAY_MS).await;
    for (core, friend) in [(&ana, bea.me()), (&bea, ana.me())] {
        let chat = core.direct_chat(friend, 100).await.unwrap();
        assert!(texts(&chat).is_empty());
        assert!(chat.items.iter().all(|i| i.item.pin_target().is_none()));
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn pictures_come_by_themselves_and_files_when_asked() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-files-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let picture = base.join("lake.jpg");
    let document = base.join("budget.pdf");
    std::fs::write(&picture, jpeg(7, 299_996)).unwrap();
    std::fs::write(&document, vec![9u8; 220_000]).unwrap();
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    let files = vec![
        Outgoing {
            path: picture.clone(),
            video: None,
        },
        Outgoing {
            path: document.clone(),
            video: None,
        },
    ];
    ana.send_direct(bea.me(), "here they are", files)
        .await
        .unwrap();
    // The picture comes by itself; the document waits to be opened.
    let chat = chat_until(&bea, ana.me(), "the picture here", |c| {
        c.items
            .last()
            .is_some_and(|i| i.files.first().is_some_and(|f| f.file.local == Local::Here))
    })
    .await;
    let item = chat.items.last().unwrap();
    assert_eq!(item.files[1].file.local, Local::Missing);
    let kept = item.files[0].path.clone().unwrap();
    assert_eq!(
        std::fs::read(&kept).unwrap(),
        std::fs::read(&picture).unwrap()
    );
    assert_eq!(chat.usage.pictures, 300_000);
    let pdf = item.files[1].file.id;
    // Asked for while the disk has no room for it and what it keeps free: refused at once,
    // as a room download is, and asked for again once there is room.
    bea.use_free_space(|_| Some(0));
    let full = bea.fetch_direct_file(ana.me(), pdf).await.unwrap_err();
    assert!(
        format!("{full:#}").contains("not enough free space"),
        "{full:#}"
    );
    let chat = bea.direct_chat(ana.me(), 100).await.unwrap();
    assert_eq!(
        chat.items.last().unwrap().files[1].file.local,
        Local::Missing
    );
    // Nor is a copy kept of a file sent while the disk has no room for it.
    let plan = base.join("plan.txt");
    std::fs::write(&plan, vec![5u8; 1000]).unwrap();
    let plan = vec![Outgoing {
        path: plan,
        video: None,
    }];
    let full = bea.send_direct(ana.me(), "plan", plan).await.unwrap_err();
    assert!(
        format!("{full:#}").contains("not enough free space"),
        "{full:#}"
    );
    // Nor of a voice message recorded here: it is not sent, and nothing of it is kept.
    let files_here = || {
        std::fs::read_dir(dir_b.join("direct"))
            .unwrap()
            .flatten()
            .filter_map(|chat| std::fs::read_dir(chat.path().join("files")).ok())
            .map(|files| files.count())
            .sum::<usize>()
    };
    let before = (
        files_here(),
        bea.direct_chat(ana.me(), 100).await.unwrap().items.len(),
    );
    let voice = VoiceNote {
        file: ogg(2),
        duration_ms: 2_000,
        bars: vec![120; 40],
    };
    let full = bea.send_direct_voice(ana.me(), voice).await.unwrap_err();
    assert!(
        format!("{full:#}").contains("not enough free space"),
        "{full:#}"
    );
    assert_eq!(
        (
            files_here(),
            bea.direct_chat(ana.me(), 100).await.unwrap().items.len()
        ),
        before
    );
    bea.use_free_space(|_| None);
    bea.fetch_direct_file(ana.me(), pdf).await.unwrap();
    chat_until(&bea, ana.me(), "the document here", |c| {
        c.items.last().unwrap().files[1].file.local == Local::Here
    })
    .await;

    // Cleared here, the messages stay and say so; asked for again, it comes back.
    bea.clear_direct_files(ana.me()).unwrap();
    let chat = bea.direct_chat(ana.me(), 100).await.unwrap();
    let item = chat.items.last().unwrap();
    assert!(item.files.iter().all(|f| f.file.local == Local::Removed));
    assert!(!kept.exists());
    assert_eq!(chat.usage.pictures + chat.usage.files, 0);
    bea.fetch_direct_file(ana.me(), pdf).await.unwrap();
    chat_until(&bea, ana.me(), "the document back", |c| {
        c.items.last().unwrap().files[1].file.local == Local::Here
    })
    .await;
    // Saved elsewhere under its own name, once the disk has room for it there: as a room
    // download, it leaves what the disk keeps free, and nothing of it is written before.
    let downloads = base.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    bea.use_free_space(|_| Some(0));
    let full = bea
        .save_direct_file(ana.me(), pdf, &downloads)
        .await
        .unwrap_err();
    assert!(
        format!("{full:#}").contains("not enough free space"),
        "{full:#}"
    );
    assert_eq!(std::fs::read_dir(&downloads).unwrap().count(), 0);
    bea.use_free_space(|_| None);
    let saved = bea
        .save_direct_file(ana.me(), pdf, &downloads)
        .await
        .unwrap();
    assert_eq!(saved, downloads.join("budget.pdf"));
    let storage = bea.direct_storage().await.unwrap();
    assert_eq!(storage.total.files, 220_000);

    // A picture sent again while the copy of an earlier message is here: it shows at once
    // from that copy, rather than waiting for a fetch that never comes.
    for text in ["again", "and again"] {
        let picture = vec![Outgoing {
            path: picture.clone(),
            video: None,
        }];
        ana.send_direct(bea.me(), text, picture).await.unwrap();
        chat_until(&bea, ana.me(), text, |c| {
            texts(c).last().is_some_and(|t| t == text)
                && c.items.last().unwrap().files[0].file.local == Local::Here
        })
        .await;
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn voice_messages_come_by_themselves_and_stay_played() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-voice-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    let file = ogg(2);
    let note = |bars: usize| VoiceNote {
        file: file.clone(),
        duration_ms: 2_000,
        bars: vec![120; bars],
    };
    assert!(ana.send_direct_voice(bea.me(), note(3)).await.is_err());
    ana.send_direct_voice(bea.me(), note(40)).await.unwrap();
    let mine = ana.direct_chat(bea.me(), 10).await.unwrap();
    let sent = &mine.items.last().unwrap().files[0];
    assert!(sent.file.voice.as_ref().is_some_and(|v| v.played));
    // Bea's comes by itself, though files do not.
    let chat = chat_until(&bea, ana.me(), "the voice message here", |c| {
        c.items
            .last()
            .is_some_and(|i| i.files.first().is_some_and(|f| f.file.local == Local::Here))
    })
    .await;
    let got = &chat.items.last().unwrap().files[0];
    let voice = got.file.voice.clone().unwrap();
    assert_eq!(
        (voice.duration_ms, voice.bars.len(), voice.played),
        (2_000, 40, false)
    );
    assert_eq!(got.file.name, "Voice message.ogg");
    let id = got.file.id;
    assert_eq!(
        bea.direct_file_bytes(ana.me(), id, 1 << 22).await.unwrap(),
        file
    );
    assert!(bea.direct_file_kept(ana.me(), id));
    assert!(!bea.direct_file_kept(bea.me(), id));
    assert_eq!(chat.usage.files, file.len() as u64);
    let list = bea.direct_chats().await.unwrap();
    assert_eq!(list[0].last_voice_ms, Some(2_000));

    // Played, it stays played after a restart.
    bea.mark_direct_played(ana.me(), id);
    stop(bea).await;
    let (bea, _events_b) = start_again(&dir_b, options_b.clone()).await;
    let chat = bea.direct_chat(ana.me(), 10).await.unwrap();
    let voice = chat.items.last().unwrap().files[0]
        .file
        .voice
        .clone()
        .unwrap();
    assert!(voice.played);
    // Its file removed, it is no longer kept, though the message stays.
    bea.clear_direct_files(ana.me()).unwrap();
    assert!(!bea.direct_file_kept(ana.me(), id));
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// What comes for a voice message and is not one shows as a file: thrown away when files do
/// not come by themselves (it comes when asked for), kept when they do.
#[tokio::test(flavor = "multi_thread")]
async fn a_voice_message_that_is_not_one_comes_as_a_file() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-notvoice-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let junk = |n: u8| VoiceNote {
        file: vec![n; 40_000],
        duration_ms: 5_000,
        bars: vec![9; 40],
    };

    // Files do not come by themselves: it is a file to ask for, and its bytes are gone.
    ana.send_direct_voice(bea.me(), junk(1)).await.unwrap();
    let chat = chat_until(&bea, ana.me(), "the first one a file", |c| {
        c.items
            .last()
            .is_some_and(|i| i.files.first().is_some_and(|f| f.file.voice.is_none()))
    })
    .await;
    let first = chat.items.last().unwrap().files[0].clone();
    assert_eq!(first.file.local, Local::Missing);
    assert!(first.path.is_none());
    // Asked for, it comes, as the file it is.
    bea.fetch_direct_file(ana.me(), first.file.id)
        .await
        .unwrap();
    chat_until(&bea, ana.me(), "the first one asked for", |c| {
        c.items.last().unwrap().files[0].file.local == Local::Here
    })
    .await;

    // Files come by themselves: it is kept as a file.
    let mut settings = bea.direct_settings();
    settings.auto_files = true;
    bea.set_direct_settings(settings).unwrap();
    ana.send_direct_voice(bea.me(), junk(2)).await.unwrap();
    chat_until(&bea, ana.me(), "the second one kept as a file", |c| {
        c.items.last().is_some_and(|i| {
            i.files.first().is_some_and(|f| {
                f.file.voice.is_none() && f.file.local == Local::Here && f.file.size == 40_000
            })
        })
    })
    .await;

    // A real voice message of a second that says it lasts ten minutes is not that one.
    ana.send_direct_voice(
        bea.me(),
        VoiceNote {
            file: ogg(1),
            duration_ms: 600_000,
            bars: vec![9; 40],
        },
    )
    .await
    .unwrap();
    let long = *blake3::hash(&ogg(1)).as_bytes();
    chat_until(&bea, ana.me(), "the long one a file", |c| {
        c.items
            .iter()
            .flat_map(|i| &i.files)
            .any(|f| f.file.id == long && f.file.voice.is_none() && f.file.local == Local::Here)
    })
    .await;
    // Nor is a file Bea sent Ana, sent back as a voice message: kept here already, it is
    // checked all the same.
    let document = base.join("budget.pdf");
    std::fs::write(&document, vec![3u8; 30_000]).unwrap();
    let outgoing = vec![Outgoing {
        path: document,
        video: None,
    }];
    bea.send_direct(ana.me(), "", outgoing).await.unwrap();
    ana.send_direct_voice(
        bea.me(),
        VoiceNote {
            file: vec![3u8; 30_000],
            duration_ms: 5_000,
            bars: vec![9; 40],
        },
    )
    .await
    .unwrap();
    let doc = *blake3::hash(&[3u8; 30_000]).as_bytes();
    let chat = chat_until(
        &bea,
        ana.me(),
        "the document back as a voice message",
        |c| {
            c.items
                .iter()
                .filter(|i| !i.item.mine)
                .flat_map(|i| &i.files)
                .any(|f| f.file.id == doc && f.file.local == Local::Here)
        },
    )
    .await;
    assert!(
        chat.items
            .iter()
            .flat_map(|i| &i.files)
            .all(|f| f.file.id != doc || f.file.voice.is_none()),
        "a document shows as a voice message"
    );

    // It stays a file after a restart.
    stop(bea).await;
    let (bea, _events_b) = start_again(&dir_b, options_b.clone()).await;
    let chat = bea.direct_chat(ana.me(), 10).await.unwrap();
    assert!(
        chat.items
            .iter()
            .flat_map(|i| &i.files)
            .all(|f| f.file.voice.is_none())
    );
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// A burst of voice messages, here files that are not voice messages at all: every one
/// started counts against the friend's daily allowance (64 MiB), on its way or not, kept or
/// not; the one past it waits to be asked for, also after a restart.
#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_voice_messages_stops_at_the_daily_allowance() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-burst-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Ana online", online).await;
    wait_for(&mut events_b, "Bea online", online).await;
    // 17 of 4 MiB, the largest a voice message may be: 16 fill the allowance.
    let size = 4 << 20;
    for n in 0..17u8 {
        let note = VoiceNote {
            file: vec![n; size],
            duration_ms: 60_000,
            bars: vec![9; 40],
        };
        ana.send_direct_voice(bea.me(), note).await.unwrap();
    }
    let settled = |c: &ChatView| {
        let files: Vec<_> = c.items.iter().flat_map(|i| &i.files).collect();
        files.len() == 17 && files.iter().filter(|f| f.file.voice.is_none()).count() == 16
    };
    let chat = chat_until(&bea, ana.me(), "16 tried and found not voice", settled).await;
    // Give the last one time to start, were it going to.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let chat = bea.direct_chat(ana.me(), 100).await.unwrap_or(chat);
    let waiting: Vec<_> = chat
        .items
        .iter()
        .flat_map(|i| &i.files)
        .filter(|f| f.file.voice.is_some())
        .collect();
    assert_eq!(waiting.len(), 1, "one waits");
    assert_eq!(waiting[0].file.local, Local::Missing);
    let last = waiting[0].file.id;
    // Still waiting after a restart: the allowance is in the journal.
    stop(bea).await;
    let (bea, mut events_b) = start_again(&dir_b, options_b.clone()).await;
    wait_for(&mut events_b, "Ana online again", online).await;
    // Given time to start, it does not.
    tokio::time::sleep(Duration::from_secs(5)).await;
    let chat = bea.direct_chat(ana.me(), 100).await.unwrap();
    let file = chat
        .items
        .iter()
        .flat_map(|i| &i.files)
        .find(|f| f.file.id == last)
        .unwrap();
    assert_eq!(file.file.local, Local::Missing);
    // Asked for, it comes.
    bea.fetch_direct_file(ana.me(), last).await.unwrap();
    chat_until(&bea, ana.me(), "the last one asked for", |c| {
        c.items
            .iter()
            .flat_map(|i| &i.files)
            .any(|f| f.file.id == last && f.file.local == Local::Here)
    })
    .await;
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// A picture's file: a JPEG's first bytes, then `size` bytes more of its own.
fn jpeg(n: u8, size: usize) -> Vec<u8> {
    let mut bytes = b"\xff\xd8\xff\xe0".to_vec();
    bytes.extend((0..size).map(|i| (i as u8).wrapping_mul(31).wrapping_add(n)));
    bytes
}

/// A burst of a friend's pictures that come by themselves makes room in the friend's own
/// chat only: what another chat keeps stays. A "picture" that does not start as one shows
/// as the file it is, and is not kept while files do not come by themselves.
#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_pictures_makes_room_in_its_own_chat_only() {
    use fogata_core::direct::{
        model::{Attachment, Chat, FileKind, Item},
        store::{DirectStore, Record},
    };
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-pictures-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    // Bea keeps 1 MB of attachments, 400 KB of them a picture Sofía sent long ago, and
    // some 20 KB for the chats' messages, which count too.
    std::fs::create_dir_all(dir_b.join("direct")).unwrap();
    std::fs::write(
        dir_b.join("direct").join("settings.json"),
        r#"{"limit":1020000,"auto_pictures":true,"auto_files":false,"timer_ms":0}"#,
    )
    .unwrap();
    let sofia = fogata_crypto::DeviceIdentity::generate()
        .unwrap()
        .public_key();
    let store = DirectStore::open(&dir_b);
    let beach = jpeg(0, 400_000);
    let old = Attachment {
        id: *blake3::hash(&beach).as_bytes(),
        name: "beach.jpg".into(),
        size: beach.len() as u64,
        kind: FileKind::Picture,
        video: None,
        local: Local::Here,
        voice: None,
    };
    store.files_dir(&sofia).unwrap();
    std::fs::write(store.file_path(&sofia, &old), &beach).unwrap();
    let item = Item {
        id: [1; 16],
        mine: false,
        sent_ms: 1_000,
        got_ms: 1_000,
        seq: 1,
        at_ms: 1_000,
        body: Body::Message {
            text: String::new(),
            files: vec![old.clone()],
        },
        ttl_ms: 0,
        delivered: true,
    };
    let mut chat = Chat::default();
    chat.insert(item.clone());
    store
        .append(&sofia, &[Record::Item { item }], &chat)
        .unwrap();

    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    // Four pictures of about 300 KB, and one that is not a picture at all.
    for n in 1..=5u8 {
        let path = base.join(format!("photo{n}.jpg"));
        let bytes = if n == 5 {
            vec![n; 100_000]
        } else {
            jpeg(n, 299_000)
        };
        std::fs::write(&path, bytes).unwrap();
        let picture = vec![Outgoing { path, video: None }];
        ana.send_direct(bea.me(), "", picture).await.unwrap();
    }
    let settled = |c: &ChatView| {
        let files: Vec<_> = c.items.iter().flat_map(|i| &i.files).collect();
        files.len() == 5
            && files.iter().all(|f| {
                f.received.is_none()
                    && (f.file.local != Local::Missing || f.file.kind == FileKind::File)
            })
    };
    chat_until(&bea, ana.me(), "the pictures settled", settled).await;
    // Given time to make more room or keep more, nothing changes.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let chat = bea.direct_chat(ana.me(), 100).await.unwrap();
    let files: Vec<_> = chat.items.iter().flat_map(|i| &i.files).collect();
    // Sofía's picture stays; Ana's chat holds what fits beside it.
    let kept = bea.direct_chat(sofia, 10).await.unwrap();
    assert_eq!(
        kept.items[0].files[0].file.local,
        Local::Here,
        "Sofía's picture went"
    );
    assert!(store.file_path(&sofia, &old).exists());
    let here = files.iter().filter(|f| f.file.local == Local::Here).count();
    assert_eq!(
        here,
        2,
        "{:?}",
        files.iter().map(|f| f.file.local).collect::<Vec<_>>()
    );
    assert!(chat.usage.pictures <= 600_000);
    // The one that is not a picture is a file, not kept.
    let odd = files.iter().find(|f| f.file.name == "photo5.jpg").unwrap();
    assert_eq!(
        (odd.file.kind, odd.file.local),
        (FileKind::File, Local::Missing)
    );
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// A burst of a friend's pictures comes a few at a time: no more than four of the friend's
/// attachments that come by themselves are on their way at once, the others wait for their
/// turn, and all come.
#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_pictures_comes_a_few_at_a_time() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-turns-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    // Three messages of ten pictures of 2 MB.
    for m in 0..3u8 {
        let pictures = (0..10u8)
            .map(|n| {
                let path = base.join(format!("photo{m}-{n}.jpg"));
                std::fs::write(&path, jpeg(m * 10 + n, 2 << 20)).unwrap();
                Outgoing { path, video: None }
            })
            .collect();
        ana.send_direct(bea.me(), "", pictures).await.unwrap();
    }
    let (ana_key, mut most, mut here) = (ana.me(), 0, 0);
    let all_here = timeout(Duration::from_secs(120), async {
        loop {
            if let Ok(chat) = bea.direct_chat(ana_key, 100).await {
                let files: Vec<_> = chat.items.iter().flat_map(|i| &i.files).collect();
                let coming = files
                    .iter()
                    .filter(|f| f.received.is_some_and(|got| got > 0))
                    .count();
                most = most.max(coming);
                here = files.iter().filter(|f| f.file.local == Local::Here).count();
                if files.len() == 30 && files.iter().all(|f| f.file.local == Local::Here) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(
        all_here.is_ok(),
        "{here} of 30 pictures came, {most} of them at once"
    );
    assert!(most <= 4, "{most} pictures came at once");
    assert!(most >= 1);
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// A voice message that cannot start to come by itself (here the chat's folder for files
/// cannot be made) costs nothing of the friend's daily allowance: once the folder can be
/// made, the next one comes by itself, however many could not.
#[tokio::test(flavor = "multi_thread")]
async fn a_voice_message_that_does_not_start_is_not_charged() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-refund-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    // A file where Bea's chat with Ana keeps its files: no folder can be made there.
    let chat = dir_b.join("direct").join(hex::encode(ana.me().as_bytes()));
    std::fs::create_dir_all(&chat).unwrap();
    std::fs::write(chat.join("files"), b"in the way").unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    // 17 of 4 MiB: more than the allowance, were they charged.
    for n in 0..17u8 {
        let note = VoiceNote {
            file: vec![n; 4 << 20],
            duration_ms: 60_000,
            bars: vec![9; 40],
        };
        ana.send_direct_voice(bea.me(), note).await.unwrap();
    }
    chat_until(&bea, ana.me(), "the 17 here", |c| {
        c.items.iter().flat_map(|i| &i.files).count() == 17
    })
    .await;
    // Each of the 17 tries to start and cannot; only then can the folder be made.
    tokio::time::sleep(Duration::from_secs(2)).await;
    std::fs::remove_file(chat.join("files")).unwrap();
    // The next one comes by itself.
    let file = ogg(1);
    let note = VoiceNote {
        file: file.clone(),
        duration_ms: 1000,
        bars: vec![9; 40],
    };
    ana.send_direct_voice(bea.me(), note).await.unwrap();
    let id = *blake3::hash(&file).as_bytes();
    chat_until(&bea, ana.me(), "the last one here by itself", |c| {
        c.items
            .iter()
            .flat_map(|i| &i.files)
            .any(|f| f.file.id == id && f.file.local == Local::Here)
    })
    .await;
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// The same file sent under names that end differently is one file on each side: each
/// message shows it under its own ending, and clearing the chat's files leaves nothing of it.
#[tokio::test(flavor = "multi_thread")]
async fn the_same_file_under_another_ending_is_one_file() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-endings-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a.clone(), options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let bytes = jpeg(9, 50_000);
    for name in ["lake.jpg", "lake.jpeg"] {
        let path = base.join(name);
        std::fs::write(&path, &bytes).unwrap();
        let picture = vec![Outgoing { path, video: None }];
        ana.send_direct(bea.me(), name, picture).await.unwrap();
    }
    let chat = chat_until(&bea, ana.me(), "both here", |c| {
        let files: Vec<_> = c.items.iter().flat_map(|i| &i.files).collect();
        files.len() == 2 && files.iter().all(|f| f.file.local == Local::Here)
    })
    .await;
    for f in chat.items.iter().flat_map(|i| &i.files) {
        let path = f.path.clone().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{}", path.display());
    }
    let files_of = |dir: &Path, friend: DevicePublicKey| {
        let files = dir
            .join("direct")
            .join(hex::encode(friend.as_bytes()))
            .join("files");
        std::fs::read_dir(files)
            .map(|d| d.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    for (core, dir, friend) in [(&bea, &dir_b, ana.me()), (&ana, &dir_a, bea.me())] {
        core.clear_direct_files(friend).unwrap();
        assert_eq!(files_of(dir, friend), Vec::<std::ffi::OsString>::new());
    }
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

/// A send that fails part way (here a later file is too big) leaves no copy of the files
/// before it in the chat's folder: nothing would ever remove one, and the room for direct
/// messages does not count it.
#[tokio::test(flavor = "multi_thread")]
async fn a_send_that_fails_leaves_no_copy_behind() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-failed-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    std::fs::create_dir_all(dir_a.join("direct")).unwrap();
    std::fs::write(
        dir_a.join("direct").join("settings.json"),
        r#"{"limit":100000,"auto_pictures":true,"auto_files":false,"timer_ms":0}"#,
    )
    .unwrap();
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a.clone(), options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let (small, big) = (base.join("notes.txt"), base.join("film.bin"));
    std::fs::write(&small, vec![7u8; 1000]).unwrap();
    std::fs::write(&big, vec![9u8; 200_000]).unwrap();
    let files = dir_a
        .join("direct")
        .join(hex::encode(bea.me().as_bytes()))
        .join("files");
    let kept = || {
        std::fs::read_dir(&files)
            .map(|d| d.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let both = vec![
        Outgoing {
            path: small.clone(),
            video: None,
        },
        Outgoing {
            path: big,
            video: None,
        },
    ];
    let failed = ana.send_direct(bea.me(), "two", both).await.unwrap_err();
    assert!(format!("{failed:#}").contains("bigger"), "{failed:#}");
    assert_eq!(kept(), Vec::<std::ffi::OsString>::new());
    // The same file sent alone is kept, and stays.
    let one = vec![Outgoing {
        path: small,
        video: None,
    }];
    ana.send_direct(bea.me(), "one", one).await.unwrap();
    assert_eq!(kept().len(), 1);
    stop(bea).await;
    stop(ana).await;
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_chat_with_someone_no_longer_a_friend_can_only_be_read() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-ended-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    ana.send_direct(bea.me(), "bye", Vec::new()).await.unwrap();
    chat_until(&bea, ana.me(), "the message", |c| texts(c).len() == 1).await;

    ana.remove_friend(bea.me()).await.unwrap();
    for (core, friend) in [(&ana, bea.me()), (&bea, ana.me())] {
        let chat = chat_until(core, friend, "the chat read-only", |c| c.ended_ms.is_some()).await;
        assert_eq!(texts(&chat), ["bye"]);
        assert!(
            core.send_direct(friend, "hello?", Vec::new())
                .await
                .is_err()
        );
    }
    bea.delete_direct_chat(ana.me()).await.unwrap();
    assert!(bea.direct_chats().await.unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

/// Makes a chat's folder in `dir` read-only (`true`) or writable again. Returns whether
/// writing in it now fails, which it does not for a user that ignores permissions (root).
#[cfg(unix)]
fn read_only(dir: &Path, friend: DevicePublicKey, on: bool) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let chat = dir.join("direct").join(hex::encode(friend.as_bytes()));
    let journal = chat.join("journal");
    let (folder, file) = if on { (0o500, 0o400) } else { (0o700, 0o600) };
    std::fs::set_permissions(&journal, std::fs::Permissions::from_mode(file)).unwrap();
    std::fs::set_permissions(&chat, std::fs::Permissions::from_mode(folder)).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .is_err()
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn what_cannot_be_saved_is_neither_confirmed_nor_sent() {
    let _one = ONE_AT_A_TIME.lock().await;
    let base = std::env::temp_dir().join(format!("fogata-direct-unsaved-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (port_a, port_b) = (free_port(), free_port());
    let (ana, _events_a) = Core::start(dir_a.clone(), alone(port_a, None)).unwrap();
    let (bea, _events_b) = Core::start(dir_b.clone(), alone(port_b, Some(port_a))).unwrap();
    let (ana_key, bea_key) = (ana.me(), bea.me());
    sees_online(&ana, bea_key, "Bea online").await;
    ana.send_direct(bea_key, "one", Vec::new()).await.unwrap();
    chat_until(&ana, bea_key, "the first one delivered", |c| {
        c.items.iter().any(|i| i.item.delivered)
    })
    .await;

    // Bea's device cannot write its chat: it shows what comes but does not confirm it, so
    // Ana's device keeps sending it.
    if !read_only(&dir_b, ana_key, true) {
        eprintln!("permissions do not apply to this user; skipped");
        let _ = std::fs::remove_dir_all(&base);
        return;
    }
    ana.send_direct(bea_key, "two", Vec::new()).await.unwrap();
    chat_until(&bea, ana_key, "the second one shown", |c| {
        texts(c).len() == 2
    })
    .await;
    // Given time to come, no receipt does.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let chat = ana.direct_chat(bea_key, 100).await.unwrap();
    assert!(!chat.items.last().unwrap().item.delivered);
    // Once it can, it saves the chat and confirms.
    read_only(&dir_b, ana_key, false);
    chat_until(&ana, bea_key, "the second one delivered", |c| {
        c.items.last().is_some_and(|i| i.item.delivered)
    })
    .await;
    stop(bea).await;
    let (bea, _events_b) = start_again(&dir_b, alone(port_b, Some(port_a))).await;
    let chat = bea.direct_chat(ana_key, 100).await.unwrap();
    assert_eq!(texts(&chat), ["one", "two"]);

    // What waits for a receipt when both go away goes once both are back, after restarts
    // too: Bea's device shows it but cannot keep it, so Ana's keeps it waiting.
    read_only(&dir_b, ana_key, true);
    sees_online(&ana, bea_key, "Bea back").await;
    ana.send_direct(bea_key, "three", Vec::new()).await.unwrap();
    chat_until(&bea, ana_key, "the third one shown", |c| {
        texts(c).len() == 3
    })
    .await;
    stop(bea).await;
    stop(ana).await;
    read_only(&dir_b, ana_key, false);
    let (ana, _events_a) = start_again(&dir_a, alone(port_a, None)).await;
    let chat = ana.direct_chat(bea_key, 100).await.unwrap();
    assert!(!chat.items.last().unwrap().item.delivered);
    let (bea, _events_b) = start_again(&dir_b, alone(port_b, Some(port_a))).await;
    chat_until(&ana, bea_key, "the third one delivered", |c| {
        c.items.last().is_some_and(|i| i.item.delivered)
    })
    .await;
    let chat = bea.direct_chat(ana_key, 100).await.unwrap();
    assert_eq!(texts(&chat), ["one", "two", "three"]);

    // Ana's device cannot write its chat: what she writes is refused and not sent.
    sees_online(&ana, bea_key, "Bea back again").await;
    read_only(&dir_a, bea_key, true);
    assert!(ana.send_direct(bea_key, "four", Vec::new()).await.is_err());
    assert_eq!(
        texts(&ana.direct_chat(bea_key, 100).await.unwrap()),
        ["one", "two", "three"]
    );
    read_only(&dir_a, bea_key, false);
    ana.send_direct(bea_key, "five", Vec::new()).await.unwrap();
    let chat = chat_until(&bea, ana_key, "the fifth one", |c| texts(c).len() == 4).await;
    assert_eq!(texts(&chat), ["one", "two", "three", "five"]);
    let _ = std::fs::remove_dir_all(&base);
}

/// How long writing to a friend takes, and how long reading a chat waits meanwhile: 200
/// messages written one after another while another task reads the chat. Run by hand on a
/// real disk: `FOGATA_MEASURE_DIR=<folder> cargo test -p fogata-core --test direct
/// writing_and_reading_at_once -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn writing_and_reading_at_once() {
    use std::time::Instant;
    let _one = ONE_AT_A_TIME.lock().await;
    let root = std::env::var_os("FOGATA_MEASURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let base = root.join(format!("fogata-direct-measure-{}", std::process::id()));
    let (dir_a, dir_b) = friends(&base);
    let (options_a, options_b) = dialling();
    let (ana, mut events_a) = Core::start(dir_a, options_a.clone()).unwrap();
    let (bea, mut events_b) = Core::start(dir_b, options_b.clone()).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let (ana, bea_key) = (std::sync::Arc::new(ana), bea.me());
    ana.send_direct(bea_key, "hi", Vec::new()).await.unwrap();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reader = {
        let (ana, done) = (ana.clone(), done.clone());
        tokio::spawn(async move {
            let (mut waits, mut held) = (Vec::new(), Vec::new());
            while !done.load(std::sync::atomic::Ordering::Relaxed) {
                let at = Instant::now();
                ana.direct_chat(bea_key, 50).await.unwrap();
                waits.push(at.elapsed());
                // Only the chats held: what the disk can hold up.
                let at = Instant::now();
                ana.direct_settings();
                held.push(at.elapsed());
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            (waits, held)
        })
    };
    let mut sends = Vec::new();
    for n in 0..200 {
        let at = Instant::now();
        ana.send_direct(bea_key, &format!("message {n}"), Vec::new())
            .await
            .unwrap();
        sends.push(at.elapsed());
    }
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let (mut waits, mut held) = reader.await.unwrap();
    let summary = |times: &mut Vec<Duration>| {
        times.sort();
        let at = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
        format!(
            "n={} median={:?} p99={:?} max={:?}",
            times.len(),
            at(0.5),
            at(0.99),
            times[times.len() - 1]
        )
    };
    println!("send: {}", summary(&mut sends));
    println!("read meanwhile: {}", summary(&mut waits));
    println!("chats held meanwhile: {}", summary(&mut held));
    stop(bea).await;
    let _ = std::fs::remove_dir_all(&base);
}
