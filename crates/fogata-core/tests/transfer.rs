//! Moving an account to another data folder ("computer") in one process, over the loopback
//! interface only (no DHT, no mDNS): the moved account talks to its friend and its group as
//! the same person, the old folder stops for good; wrong codes, a third computer answering
//! on the rendezvous point, moves cut short, lost last steps and many large attachments.
//! Every folder is sealed with keys of a keychain kept in memory.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use fogata_core::CoreEvents;
use fogata_core::{
    Core, CoreEvent, NetOptions, Store,
    direct::Outgoing,
    transfer::{self, Failure, MovedState, TransferEvent, TransferOptions, TransferState},
    vault::{self, Keychain, MemoryKeyStore},
};
use fogata_crypto::at_rest::{SealReader, SealWriter};
use fogata_friends::{Friend, TransferCode};
use fogata_net::Multiaddr;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn keys() -> Arc<MemoryKeyStore> {
    static KEYS: OnceLock<Arc<MemoryKeyStore>> = OnceLock::new();
    let keys = KEYS.get_or_init(Default::default).clone();
    vault::use_keychain(Keychain::Seal(keys.clone()));
    keys
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn to(port: u16) -> Multiaddr {
    format!("/ip4/127.0.0.1/udp/{port}/quic-v1")
        .parse()
        .unwrap()
}

fn net(port: u16, peers: &[u16]) -> NetOptions {
    NetOptions {
        listen_port: port,
        use_dht: false,
        use_mdns: false,
        peers: peers.iter().map(|p| to(*p)).collect(),
        ..Default::default()
    }
}

fn base(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("fogata-transfer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    base
}

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

async fn eventually<F: std::future::Future<Output = bool>>(
    what: &str,
    seconds: u64,
    check: impl Fn() -> F,
) {
    let found = timeout(Duration::from_secs(seconds), async {
        while !check().await {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await;
    assert!(found.is_ok(), "timed out waiting for {what}");
}

/// Two data folders whose profiles are already friends.
fn friends(base: &Path) -> (PathBuf, PathBuf) {
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

/// Waits until nothing in this process holds `dir`'s running lock any more: a core's or a
/// move's goes with the last of its tasks.
async fn let_go(dir: &Path) {
    let free = timeout(Duration::from_secs(30), async {
        loop {
            let lock = std::fs::OpenOptions::new()
                .write(true)
                .open(dir.join("running.lock"));
            match lock {
                Err(_) => return,
                Ok(file) if file.try_lock().is_ok() => return,
                Ok(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await;
    assert!(free.is_ok(), "{} is still held", dir.display());
}

/// Starts a core on a folder another one just let go of.
async fn start_again(dir: &Path, options: NetOptions) -> (Core, CoreEvents) {
    for _ in 0..50 {
        if let Ok(started) = Core::start(dir.to_path_buf(), options.clone()) {
            return started;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Core::start(dir.to_path_buf(), options).unwrap()
}

fn options(port: u16, peers: &[u16], computer: &str) -> TransferOptions {
    TransferOptions::new(net(port, peers), computer)
}

/// Follows a move to its end; the person says `yes` when asked (on either computer).
async fn follow(
    mut moving: transfer::Transfer,
    yes: Option<bool>,
) -> (Vec<TransferEvent>, TransferEvent) {
    let mut seen = Vec::new();
    let end = timeout(Duration::from_secs(180), async {
        while let Some(event) = moving.events.recv().await {
            if let (TransferEvent::Matched { .. }, Some(yes)) = (&event, yes) {
                moving.confirm(yes);
            }
            if matches!(event, TransferEvent::Done { .. } | TransferEvent::Failed(_)) {
                return event;
            }
            seen.push(event);
        }
        TransferEvent::Failed(Failure::Other("no end".into()))
    })
    .await
    .expect("the move ended");
    (seen, end)
}

/// The code a receiving move shows.
async fn code_of(moving: &mut transfer::Transfer) -> String {
    loop {
        match moving.events.recv().await {
            Some(TransferEvent::Code { code, .. }) => return code,
            Some(_) => {}
            None => panic!("no code"),
        }
    }
}

/// A code with the same first characters as `code` (where the two meet) and another rest.
fn same_place(code: &str) -> String {
    let chars: Vec<char> = code.chars().filter(|c| *c != '-').collect();
    for other in "0123456789ABCDEFGHJKMNPQRSTVWXYZ".chars() {
        if other == chars[6] {
            continue;
        }
        for check in "0123456789ABCDEFGHJKMNPQRSTVWXYZ".chars() {
            let mut guess = chars.clone();
            guess[6] = other;
            guess[9] = check;
            let text: String = guess.into_iter().collect();
            if TransferCode::parse(&text).is_ok() {
                return text;
            }
        }
    }
    unreachable!("some check character fits")
}

/// The records of a move in `dirs` (`moved.json`, `moved-in.json`, `incoming.json`) that
/// still hold its secret.
fn move_secrets(dirs: &[&Path]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in dirs {
        for name in ["moved.json", "moved-in.json", "incoming.json"] {
            let path = dir.join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if json["key"].as_str().is_some_and(|key| !key.is_empty()) {
                found.push(path);
            }
        }
    }
    found
}

fn sealed_key_name(dir: &Path) -> String {
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("sealed.json")).unwrap()).unwrap();
    json["key"].as_str().unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_moved_account_talks_to_its_friend_and_group_and_the_old_copy_stops() {
    let _one = ONE_AT_A_TIME.lock().await;
    let keys = keys();
    let base = base("whole");
    let (dir_a, dir_b) = friends(&base);
    let dir_new = base.join("new");
    let (port_a, port_b, port_n) = (free_port(), free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a.clone(), net(port_a, &[port_b])).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), net(port_b, &[port_a])).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;
    let me = ana.me();

    // A group of the two, and a chat with a picture in it.
    let group = ana.create_group("crew", 0, &[bea.me()]).await.unwrap();
    wait_for(
        &mut events_b,
        "the group",
        |e| matches!(e, CoreEvent::GroupInvited { group: g, .. } if *g == group),
    )
    .await;
    bea.accept_group_invitation(group).await.unwrap();
    eventually("both members connected", 60, || async {
        ana.groups().await.unwrap()[0].online == 1
    })
    .await;
    let picture = base.join("picture.bin");
    std::fs::write(
        &picture,
        (0..300_000u32).map(|i| (i * 7) as u8).collect::<Vec<_>>(),
    )
    .unwrap();
    ana.send_direct(
        bea.me(),
        "antes",
        vec![Outgoing {
            path: picture.clone(),
            video: None,
        }],
    )
    .await
    .unwrap();
    eventually("Bea has the message", 60, || async {
        bea.direct_chat(me, 50).await.is_ok_and(|c| {
            c.items
                .iter()
                .any(|i| format!("{:?}", i.item.body).contains("antes"))
        })
    })
    .await;

    // The core must stop first: a move does not start on a folder in use.
    assert!(transfer::send(dir_a.clone(), "00000-00000", options(0, &[], "x")).is_err());
    ana.shutdown().await.unwrap();
    drop(ana);
    let_go(&dir_a).await;
    let old_key = sealed_key_name(&dir_a);

    let mut receiving =
        transfer::receive(dir_new.clone(), options(port_n, &[], "Leo's desktop")).unwrap();
    let code = code_of(&mut receiving).await;
    let sending =
        transfer::send(dir_a.clone(), &code, options(0, &[port_n], "Ana's laptop")).unwrap();
    let (got, sent) = tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
    assert!(
        matches!(&sent.1, TransferEvent::Done { computer, .. } if computer == "Leo's desktop"),
        "{:?}",
        sent.1
    );
    assert!(
        matches!(&got.1, TransferEvent::Done { computer, .. } if computer == "Ana's laptop"),
        "{:?}",
        got.1
    );
    let matched = got.0.iter().find_map(|e| match e {
        TransferEvent::Matched { summary, .. } => Some(summary.clone()),
        _ => None,
    });
    let summary = matched.expect("the new computer saw what came");
    assert_eq!(
        (
            summary.profile.name.as_str(),
            summary.friends,
            summary.groups,
            summary.chats,
            summary.files
        ),
        ("Ana", 1, 1, 1, 1)
    );
    assert!(
        got.0
            .iter()
            .any(|e| matches!(e, TransferEvent::Progress { .. }))
    );

    // The old folder never runs the account again, and keeps nothing that makes it Ana.
    let moved = transfer::moved(&dir_a).unwrap();
    assert_eq!(
        (moved.state, moved.to.as_str(), moved.name.as_str()),
        (MovedState::Moved, "Leo's desktop", "Ana")
    );
    for gone in ["identity.key", "friends.bin", "friends.bin.bak", "mls"] {
        assert!(!dir_a.join(gone).exists(), "{gone} is still there");
    }
    let Err(error) = Core::start(dir_a.clone(), net(0, &[])) else {
        panic!("the old folder started");
    };
    assert!(format!("{error:#}").contains("moved to"), "{error:#}");
    assert!(Store::open(dir_a.clone()).unwrap().identity().is_err());
    assert!(Core::create_profile(&dir_a, "Another", None).is_err());
    assert!(matches!(transfer::state(&dir_a), TransferState::Moved(_)));
    // Neither computer keeps the move's secret once it is done.
    assert_eq!(move_secrets(&[&dir_a, &dir_new]), Vec::<PathBuf>::new());

    // The new folder is sealed with a key of its own, and nothing is left of the move.
    let new_key = sealed_key_name(&dir_new);
    assert_ne!(old_key, new_key);
    assert!(keys.names().contains(&new_key));
    assert!(!dir_new.join("incoming").exists());
    assert!(!dir_new.join("replaced").exists());
    let identity = std::fs::read(dir_new.join("identity.key")).unwrap();
    assert!(identity.starts_with(b" fogata"), "the identity is sealed");

    // The new folder is Ana: Bea talks to her there as before, in the chat and the group.
    let (ana2, mut events_n) = start_again(&dir_new, net(free_port(), &[port_b])).await;
    assert_eq!(ana2.me(), me);
    wait_for(&mut events_n, "Bea online from the new folder", online).await;
    assert_eq!(ana2.friends().await.unwrap().len(), 1);
    let chat = ana2.direct_chat(bea.me(), 50).await.unwrap();
    assert!(
        chat.items
            .iter()
            .any(|i| format!("{:?}", i.item.body).contains("antes"))
    );
    ana2.send_direct(bea.me(), "later", Vec::new())
        .await
        .unwrap();
    eventually("Bea has the message from the new folder", 60, || async {
        bea.direct_chat(me, 50).await.is_ok_and(|c| {
            c.items
                .iter()
                .any(|i| format!("{:?}", i.item.body).contains("later"))
        })
    })
    .await;
    eventually("the group connected from the new folder", 90, || async {
        ana2.groups()
            .await
            .is_ok_and(|g| g.len() == 1 && g[0].online == 1)
    })
    .await;
    bea.join_room(group).await.unwrap();
    ana2.join_room(group).await.unwrap();
    wait_for(&mut events_b, "Ana in the room", |e| {
        matches!(e, CoreEvent::Room { change: fogata_net::RoomChange::Joined { device, .. }, .. } if *device == me)
    })
    .await;
    ana2.say(group, "hi from here").await.unwrap();
    wait_for(&mut events_b, "Ana's line in the room", |e| {
        matches!(e, CoreEvent::Room { change: fogata_net::RoomChange::Chat(line), .. } if line.text == "hi from here")
    })
    .await;
    ana2.shutdown().await.unwrap();
    bea.shutdown().await.unwrap();
    drop((ana2, bea));

    // What is left on the old computer goes, its key too, and it starts like new.
    transfer::forget(&dir_a).unwrap();
    assert!(!keys.names().contains(&old_key));
    assert_eq!(transfer::state(&dir_a), TransferState::None);
    let _ = std::fs::remove_dir_all(&base);
}

/// A folder with a profile and nothing else, sealed.
fn alone(base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name.to_lowercase());
    Core::create_profile(&dir, name, None).unwrap();
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_code_sends_nothing_and_three_stop_the_code() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("wrong");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    let port_n = free_port();

    // A mistyped code is told at once, with nothing looked for.
    let typo = transfer::send(dir_old.clone(), "ABCDE-FGHJ", options(0, &[port_n], "x"));
    assert!(typo.is_err());
    let mut receiving = transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
    let code = code_of(&mut receiving).await;
    let mut tries = Vec::new();
    for _ in 0..3 {
        let guess = same_place(&code);
        assert_ne!(guess, code.replace('-', ""));
        let sending =
            transfer::send(dir_old.clone(), &guess, options(0, &[port_n], "old")).unwrap();
        let (_, end) = follow(sending, Some(true)).await;
        assert_eq!(end, TransferEvent::Failed(Failure::WrongCode));
        tries.push(end);
    }
    let (seen, end) = follow(receiving, None).await;
    assert_eq!(end, TransferEvent::Failed(Failure::TooManyTries));
    assert_eq!(
        seen.iter()
            .filter(|e| **e == TransferEvent::WrongCodeTried)
            .count(),
        3
    );
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, TransferEvent::Matched { .. }))
    );
    // Nothing changed on either side.
    assert!(!dir_new.join("incoming").exists());
    assert!(!dir_new.join("identity.key").exists());
    assert!(transfer::moved(&dir_old).is_none());
    let (core, _) = start_again(&dir_old, net(0, &[])).await;
    core.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_third_computer_on_the_rendezvous_gets_nothing() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("third");
    let dir_old = alone(&base, "Ana");
    let (dir_new, dir_x) = (base.join("new"), base.join("intruder"));
    let (port_n, port_x) = (free_port(), free_port());
    let mut receiving = transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
    let code = code_of(&mut receiving).await;

    // Someone who saw the rendezvous point announces there with a guess of the rest of the
    // code, and is the only one the old computer reaches: the code's one try goes to it, it
    // fails, and nothing of the account is sent.
    let mut intruder_options = options(port_x, &[], "intruder");
    intruder_options.code = Some(same_place(&code));
    let intruder = transfer::receive(dir_x.clone(), intruder_options).unwrap();
    let sending = transfer::send(dir_old.clone(), &code, options(0, &[port_x], "old")).unwrap();
    let (_, end) = follow(sending, Some(true)).await;
    assert_eq!(end, TransferEvent::Failed(Failure::WrongCode));
    intruder.cancel();
    let (seen, _) = follow(intruder, None).await;
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, TransferEvent::Matched { .. }))
    );
    assert!(!dir_x.join("incoming").exists());
    assert!(transfer::moved(&dir_old).is_none());

    // A computer showing another code altogether is passed over without using up the try:
    // the move reaches the right one.
    let port_y = free_port();
    let other = transfer::receive(dir_x.clone(), options(port_y, &[], "other")).unwrap();
    let sending =
        transfer::send(dir_old.clone(), &code, options(0, &[port_y, port_n], "old")).unwrap();
    let ((got_seen, got), (_, sent)) =
        tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
    assert!(matches!(sent, TransferEvent::Done { .. }), "{sent:?}");
    assert!(matches!(got, TransferEvent::Done { .. }), "{got:?}");
    assert!(got_seen.iter().all(|e| *e != TransferEvent::WrongCodeTried));
    other.cancel();
    let (seen, _) = follow(other, None).await;
    assert!(
        !seen
            .iter()
            .any(|e| matches!(e, TransferEvent::Matched { .. }))
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// A new computer whose disk has no room for the account says so before anything comes,
/// and the old one keeps it.
#[tokio::test(flavor = "multi_thread")]
async fn a_computer_without_room_for_the_account_takes_none_of_it() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("room");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    let port_n = free_port();
    let mut full = options(port_n, &[], "new");
    full.free_space = Some(|_| Some(1024));
    let mut receiving = transfer::receive(dir_new.clone(), full).unwrap();
    let code = code_of(&mut receiving).await;
    let sending = transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
    let ((_, got), (_, sent)) =
        tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
    assert!(
        matches!(
            &got,
            TransferEvent::Failed(Failure::NoRoom { free: 1024, .. })
        ),
        "{got:?}"
    );
    assert!(matches!(sent, TransferEvent::Failed(_)), "{sent:?}");
    assert!(!dir_new.join("incoming").exists());
    assert!(transfer::moved(&dir_old).is_none());
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_runs_out() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("expired");
    let mut opts = options(free_port(), &[], "new");
    opts.code_lifetime_ms = 1500;
    let receiving = transfer::receive(base.join("new"), opts).unwrap();
    let (_, end) = follow(receiving, None).await;
    assert_eq!(end, TransferEvent::Failed(Failure::Expired));
    let _ = std::fs::remove_dir_all(&base);
}

/// The new computer's person says no to what came up: nothing moves, and the old computer
/// keeps the account.
#[tokio::test(flavor = "multi_thread")]
async fn the_new_computer_takes_an_account_only_with_its_own_yes() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("new-yes");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    let port_n = free_port();
    let mut receiving = transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
    let code = code_of(&mut receiving).await;
    let sending = transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
    let ((_, got), (_, sent)) =
        tokio::join!(follow(receiving, Some(false)), follow(sending, Some(true)));
    assert_eq!(got, TransferEvent::Failed(Failure::Cancelled));
    assert!(
        matches!(sent, TransferEvent::Failed(Failure::Declined(_))),
        "{sent:?}"
    );
    assert!(!dir_new.join("incoming").exists());
    assert!(!dir_new.join("identity.key").exists());
    assert!(transfer::moved(&dir_old).is_none());
    let (core, _) = start_again(&dir_old, net(0, &[])).await;
    core.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

/// The end of a move is told once its folder is free: another move, or the core, starts on
/// it at once.
#[tokio::test(flavor = "multi_thread")]
async fn the_folder_is_free_when_the_end_of_a_move_is_told() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("free");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    for _ in 0..10 {
        let mut opts = options(free_port(), &[], "new");
        opts.code_lifetime_ms = 200;
        let receiving = transfer::receive(dir_new.clone(), opts).unwrap();
        let (_, end) = follow(receiving, None).await;
        assert_eq!(end, TransferEvent::Failed(Failure::Expired));
        drop(
            Store::open(dir_new.clone())
                .unwrap()
                .hold_running()
                .unwrap(),
        );

        let mut opts = options(0, &[], "old");
        opts.no_contact_ms = 200;
        let code = TransferCode::generate().unwrap().to_string();
        let sending = transfer::send(dir_old.clone(), &code, opts).unwrap();
        let (_, end) = follow(sending, None).await;
        assert_eq!(end, TransferEvent::Failed(Failure::NoContact));
        drop(
            Store::open(dir_old.clone())
                .unwrap()
                .hold_running()
                .unwrap(),
        );
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Attachments written straight into a chat's folder of `dir`, sealed with its key: `count`
/// of `size` bytes each and one of `large` bytes. Returns each one's path and contents' hash.
fn attachments(dir: &Path, count: usize, size: usize, large: usize) -> Vec<(String, blake3::Hash)> {
    let key = Store::open(dir.to_path_buf())
        .unwrap()
        .data_key()
        .unwrap()
        .unwrap();
    let chat = "ab".repeat(32);
    let files = dir.join("direct").join(&chat).join("files");
    std::fs::create_dir_all(&files).unwrap();
    let mut out = Vec::new();
    for n in 0..=count {
        let len = if n == count { large } else { size + n };
        let id = blake3::hash(&n.to_be_bytes());
        let name = format!("{}.bin", id.to_hex());
        let context = fogata_net::files::kept_context(id.as_bytes());
        let mut writer = SealWriter::new(
            &key,
            &context,
            std::fs::File::create(files.join(&name)).unwrap(),
        )
        .unwrap();
        let mut hash = blake3::Hasher::new();
        let block: Vec<u8> = (0..65_536u32).map(|i| (i as u8) ^ (n as u8)).collect();
        let mut left = len;
        while left > 0 {
            let part = &block[..left.min(block.len())];
            writer.write_all(part).unwrap();
            hash.update(part);
            left -= part.len();
        }
        writer.finish().unwrap().sync_all().unwrap();
        out.push((format!("direct/{chat}/files/{name}"), hash.finalize()));
    }
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn many_and_large_attachments_move_whole() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("large");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    let files = attachments(&dir_old, 60, 70_000, 48 * 1024 * 1024);
    let port_n = free_port();
    let mut receiving = transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
    let code = code_of(&mut receiving).await;
    let sending = transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
    let ((seen, got), (_, sent)) =
        tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
    assert!(matches!(sent, TransferEvent::Done { .. }), "{sent:?}");
    assert!(matches!(got, TransferEvent::Done { .. }), "{got:?}");
    assert!(
        seen.iter()
            .filter(|e| matches!(e, TransferEvent::Progress { .. }))
            .count()
            > 2
    );
    // Every attachment reads whole with the new folder's key, and only with it.
    let key = Store::open(dir_new.clone())
        .unwrap()
        .data_key()
        .unwrap()
        .unwrap();
    for (rel, hash) in &files {
        let path = dir_new.join(rel);
        let len = std::fs::metadata(&path).unwrap().len();
        let id: [u8; 32] = hex::decode(&rel[rel.len() - 68..rel.len() - 4])
            .unwrap()
            .try_into()
            .unwrap();
        let mut reader = SealReader::new(
            &key,
            &fogata_net::files::kept_context(&id),
            std::fs::File::open(&path).unwrap(),
            len,
        )
        .unwrap();
        let mut plain = Vec::new();
        reader.read_to_end(&mut plain).unwrap();
        assert_eq!(blake3::hash(&plain), *hash, "{rel}");
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_move_cut_short_leaves_both_as_they_were() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("cut");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    attachments(&dir_old, 4, 1000, 64 * 1024 * 1024);
    for cut_new in [true, false] {
        let port_n = free_port();
        let mut receiving =
            transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
        let code = code_of(&mut receiving).await;
        let mut sending =
            transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
        // The new computer's person says yes.
        loop {
            if let TransferEvent::Matched { .. } = receiving.events.recv().await.unwrap() {
                receiving.confirm(true);
                break;
            }
        }
        // Once files go, one side goes away (closed, crashed, network gone).
        let (cut, mut other) = if cut_new {
            loop {
                match sending.events.recv().await.unwrap() {
                    TransferEvent::Matched { .. } => sending.confirm(true),
                    TransferEvent::Progress { done, .. } if done > 0 => break,
                    _ => {}
                }
            }
            (receiving, sending)
        } else {
            let mut go = false;
            loop {
                match sending.events.recv().await.unwrap() {
                    TransferEvent::Matched { .. } => sending.confirm(true),
                    TransferEvent::Progress { done, .. } if done > 0 => go = true,
                    _ => {}
                }
                if go {
                    break;
                }
            }
            (sending, receiving)
        };
        cut.cancel();
        drop(cut);
        let end = timeout(Duration::from_secs(120), async {
            loop {
                match other.events.recv().await {
                    Some(TransferEvent::Failed(f)) => return f,
                    Some(TransferEvent::Done { .. }) => panic!("a cut move finished"),
                    Some(_) => {}
                    None => return Failure::Other("ended".into()),
                }
            }
        })
        .await
        .unwrap();
        assert!(
            matches!(end, Failure::Interrupted(_) | Failure::Declined(_)),
            "{end:?}"
        );
        // Neither side keeps anything of it: the old one runs as before, the new one throws
        // away what came (at once, or at its next start when it was the one cut).
        drop(other);
        let_go(&dir_new).await;
        let_go(&dir_old).await;
        assert_eq!(transfer::state(&dir_new), TransferState::None);
        assert!(!dir_new.join("incoming").exists(), "cut new: {cut_new}");
        assert!(!dir_new.join("identity.key").exists());
        assert!(transfer::moved(&dir_old).is_none());
        let (core, _) = start_again(&dir_old, net(0, &[])).await;
        core.shutdown().await.unwrap();
        drop(core);
        let_go(&dir_old).await;
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_last_step_is_finished_with_a_new_code_or_undone_by_hand() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("lost");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");
    let me = Store::open(dir_old.clone())
        .unwrap()
        .identity()
        .unwrap()
        .0
        .public_key();

    for finish in [true, false] {
        let port_n = free_port();
        let mut receiving =
            transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
        let code = code_of(&mut receiving).await;
        let mut lose = options(0, &[port_n], "old");
        lose.lose_commit = true;
        let sending = transfer::send(dir_old.clone(), &code, lose).unwrap();
        let ((_, got), (_, sent)) =
            tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
        assert_eq!(sent, TransferEvent::Failed(Failure::Unconfirmed));
        assert_eq!(got, TransferEvent::Failed(Failure::Unconfirmed));
        // The old computer stopped using the account; the new one has it but does not use
        // it yet. Neither runs it now.
        assert_eq!(transfer::moved(&dir_old).unwrap().state, MovedState::Moving);
        // Both keep the move's secret: the new computer proves with it later.
        assert_eq!(
            move_secrets(&[&dir_old, &dir_new]),
            vec![dir_old.join("moved.json"), dir_new.join("incoming.json")]
        );
        assert!(Core::start(dir_old.clone(), net(0, &[])).is_err());
        assert!(matches!(
            transfer::state(&dir_new),
            TransferState::Staged(_)
        ));
        // Showing a code again on the new computer does not take another account meanwhile.
        if finish {
            // Another port: the first round's node may still be letting go of its own.
            let port_n = free_port();
            let mut receiving =
                transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
            let code = code_of(&mut receiving).await;
            let sending =
                transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
            let ((_, got), (asked, sent)) =
                tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
            assert!(matches!(sent, TransferEvent::Done { .. }), "{sent:?}");
            // The old computer asked again, naming the computer that finishes the move.
            assert!(asked.iter().any(
                |e| matches!(e, TransferEvent::Matched { computer, .. } if computer == "new")
            ));
            assert!(matches!(got, TransferEvent::Done { .. }), "{got:?}");
            assert_eq!(transfer::moved(&dir_old).unwrap().state, MovedState::Moved);
            assert!(!dir_old.join("identity.key").exists());
            assert_eq!(transfer::state(&dir_new), TransferState::None);
            assert_eq!(move_secrets(&[&dir_old, &dir_new]), Vec::<PathBuf>::new());
            let new = Store::open(dir_new.clone()).unwrap().identity().unwrap().0;
            assert_eq!(new.public_key(), me);
            // Start over for the second round.
            transfer::forget(&dir_old).unwrap();
            let _ = std::fs::remove_dir_all(&dir_new);
            let fresh = alone(&base, "Ana");
            assert_eq!(fresh, dir_old);
        } else {
            // The new computer throws it away; the old one's person keeps it there.
            transfer::discard_incoming(&dir_new).unwrap();
            assert_eq!(transfer::state(&dir_new), TransferState::None);
            transfer::keep_this_computer(&dir_old).unwrap();
            let (core, _) = start_again(&dir_old, net(0, &[])).await;
            core.shutdown().await.unwrap();
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Dates the move recorded in `dir`'s `moved-in.json` `days` back.
fn moved_in_days_ago(dir: &Path, days: u64) {
    let path = dir.join("moved-in.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let at = json["at_ms"].as_u64().unwrap();
    json["at_ms"] = (at - days * 24 * 60 * 60 * 1000).into();
    std::fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();
}

/// The link breaks once the new computer said it took the account: before the old one heard
/// it (which stays "moving"), or after (which stopped for good, and its word that it did is
/// lost). The new computer keeps the move's secret for a week, to finish the move with an
/// old computer still waiting, and forgets it at the first start after that.
#[tokio::test(flavor = "multi_thread")]
async fn the_secret_of_a_move_whose_last_word_is_lost_goes_a_week_on() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("last-word");
    for old_heard in [false, true] {
        let dir_old = alone(&base, "Ana");
        let dir_new = base.join("new");
        let port_n = free_port();
        let mut taking = options(port_n, &[], "new");
        taking.lose_active = !old_heard;
        let mut receiving = transfer::receive(dir_new.clone(), taking).unwrap();
        let code = code_of(&mut receiving).await;
        let mut giving = options(0, &[port_n], "old");
        giving.lose_moved = old_heard;
        let sending = transfer::send(dir_old.clone(), &code, giving).unwrap();
        let ((_, got), (_, sent)) =
            tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
        assert!(matches!(got, TransferEvent::Done { .. }), "{got:?}");
        let state = transfer::moved(&dir_old).unwrap().state;
        if old_heard {
            assert!(matches!(sent, TransferEvent::Done { .. }), "{sent:?}");
            assert_eq!(state, MovedState::Moved);
        } else {
            assert!(matches!(sent, TransferEvent::Failed(_)), "{sent:?}");
            assert_eq!(state, MovedState::Moving);
        }
        assert_eq!(
            move_secrets(&[&dir_new]),
            vec![dir_new.join("moved-in.json")],
            "{old_heard}"
        );
        // Started within the week, the account runs here and the secret stays.
        moved_in_days_ago(&dir_new, 6);
        let (core, _) = start_again(&dir_new, net(0, &[])).await;
        core.shutdown().await.unwrap();
        drop(core);
        assert_eq!(move_secrets(&[&dir_new]).len(), 1, "{old_heard}");
        // The first start after the week forgets it, and the account runs as before.
        moved_in_days_ago(&dir_new, 2);
        let (core, _) = start_again(&dir_new, net(0, &[])).await;
        core.shutdown().await.unwrap();
        drop(core);
        assert_eq!(
            move_secrets(&[&dir_new]),
            Vec::<PathBuf>::new(),
            "{old_heard}"
        );
        let_go(&dir_new).await;
        let_go(&dir_old).await;
        let _ = std::fs::remove_dir_all(&base);
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_word_before_the_end_is_finished_or_moved_again_with_a_new_code() {
    let _one = ONE_AT_A_TIME.lock().await;
    keys();
    let base = base("lost-words");
    let dir_old = alone(&base, "Ana");
    let dir_new = base.join("new");

    // The new computer's word that all came (`Staged`), then its word that it took the
    // account (`Active`), lost on the way.
    for lost_active in [false, true] {
        let me = Store::open(dir_old.clone())
            .unwrap()
            .identity()
            .unwrap()
            .0
            .public_key();
        let port_n = free_port();
        let mut lose = options(port_n, &[], "new");
        lose.lose_staged = !lost_active;
        lose.lose_active = lost_active;
        let mut receiving = transfer::receive(dir_new.clone(), lose).unwrap();
        let code = code_of(&mut receiving).await;
        let sending = transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
        let ((_, got), (_, sent)) =
            tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
        assert!(matches!(sent, TransferEvent::Failed(_)), "{sent:?}");
        if lost_active {
            // The new computer took the account; the old one stopped using it and waits.
            assert!(matches!(got, TransferEvent::Done { .. }), "{got:?}");
            assert_eq!(transfer::moved(&dir_old).unwrap().state, MovedState::Moving);
            assert_eq!(transfer::state(&dir_new), TransferState::None);
            assert_eq!(
                move_secrets(&[&dir_old, &dir_new]),
                vec![dir_old.join("moved.json"), dir_new.join("moved-in.json")]
            );
        } else {
            // The old computer never stopped using the account; the new one keeps what came.
            assert_eq!(got, TransferEvent::Failed(Failure::Unconfirmed));
            assert!(transfer::moved(&dir_old).is_none());
            assert!(matches!(
                transfer::state(&dir_new),
                TransferState::Staged(_)
            ));
        }
        // A code shown again on the new computer, typed on the old one, ends the move.
        let port_n = free_port();
        let mut receiving =
            transfer::receive(dir_new.clone(), options(port_n, &[], "new")).unwrap();
        let code = code_of(&mut receiving).await;
        let sending = transfer::send(dir_old.clone(), &code, options(0, &[port_n], "old")).unwrap();
        let ((_, got), (_, sent)) =
            tokio::join!(follow(receiving, Some(true)), follow(sending, Some(true)));
        assert!(matches!(sent, TransferEvent::Done { .. }), "{sent:?}");
        assert!(matches!(got, TransferEvent::Done { .. }), "{got:?}");
        assert_eq!(transfer::moved(&dir_old).unwrap().state, MovedState::Moved);
        assert!(!dir_old.join("identity.key").exists());
        assert_eq!(transfer::state(&dir_new), TransferState::None);
        assert_eq!(move_secrets(&[&dir_old, &dir_new]), Vec::<PathBuf>::new());
        let new = Store::open(dir_new.clone()).unwrap().identity().unwrap().0;
        assert_eq!(new.public_key(), me);
        // Start over for the next round, once neither folder is held.
        let_go(&dir_new).await;
        let_go(&dir_old).await;
        transfer::forget(&dir_old).unwrap();
        let _ = std::fs::remove_dir_all(&dir_new);
        assert_eq!(alone(&base, "Ana"), dir_old);
    }
    let _ = std::fs::remove_dir_all(&base);
}
