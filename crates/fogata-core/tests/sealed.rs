//! Data folders sealed with a key from the keychain (a keychain in memory here, so the
//! computer's own is never touched): two friends write and send files with everything
//! sealed on both disks, and a folder an earlier version left plain is sealed at its first
//! start, its attachments in the background, and its plain copies go at the next start.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};

use fogata_core::CoreEvents;
use fogata_core::{
    AtRest, Core, CoreEvent, NetOptions, Store,
    direct::{
        ChatView, Outgoing,
        model::{Attachment, Body, Chat, FileKind, Item, Local},
        store::{DirectStore, Record},
    },
    store::BACKUP_DIR,
    vault::{self, Keychain, MemoryKeyStore},
};
use fogata_crypto::{DeviceIdentity, DevicePublicKey, at_rest::sealed_form};
use fogata_friends::{Friend, FriendBook};
use fogata_net::Multiaddr;
use tokio::time::timeout;

/// One test at a time: each starts devices that talk over this computer's network, and
/// several at once on a busy computer leave the protocol's timers too little room.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Every core of this test process seals its folder with keys kept here.
fn keychain() -> Arc<MemoryKeyStore> {
    static KEYS: OnceLock<Arc<MemoryKeyStore>> = OnceLock::new();
    let keys = KEYS.get_or_init(Default::default).clone();
    vault::use_keychain(Keychain::Seal(keys.clone()));
    keys
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

async fn chat_until(
    core: &Core,
    friend: DevicePublicKey,
    what: &str,
    check: impl Fn(&ChatView) -> bool,
) -> ChatView {
    timeout(Duration::from_secs(30), async {
        loop {
            if let Ok(chat) = core.direct_chat(friend, 100).await
                && check(&chat)
            {
                return chat;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

fn free_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

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

async fn start_again(dir: &Path, options: NetOptions) -> (Core, CoreEvents) {
    for _ in 0..50 {
        match Core::start(dir.to_path_buf(), options.clone()) {
            Ok(started) => return started,
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("the folder stayed locked");
}

fn befriend(dir_a: &Path, dir_b: &Path) {
    let (store_a, store_b) = (
        Store::open(dir_a.to_path_buf()).unwrap(),
        Store::open(dir_b.to_path_buf()).unwrap(),
    );
    let mut book_a = store_a.friends().unwrap().unwrap();
    let mut book_b = store_b.friends().unwrap().unwrap();
    let friend = |book: &FriendBook| Friend {
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
}

/// Every file under `dir`, but the plain copies set aside while sealing.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let path = e.path();
            // The running copy's lock holds nothing, and Windows refuses to read it.
            if path.ends_with(BACKUP_DIR) || path.ends_with("running.lock") {
                continue;
            }
            if path.is_dir() {
                todo.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

fn shows(dir: &Path, needle: &[u8]) -> Option<PathBuf> {
    files_under(dir).into_iter().find(|path| {
        std::fs::read(path)
            .unwrap()
            .windows(needle.len())
            .any(|w| w == needle)
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn sealed_friends_write_and_send_files() {
    let _one = ONE_AT_A_TIME.lock().await;
    keychain();
    let base = std::env::temp_dir().join(format!("fogata-sealed-talk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (dir_a, dir_b) = (base.join("ana"), base.join("bea"));
    Core::create_profile(&dir_a, "Ana", None).unwrap();
    Core::create_profile(&dir_b, "Bea", None).unwrap();
    befriend(&dir_a, &dir_b);
    for dir in [&dir_a, &dir_b] {
        assert!(dir.join(vault::SEALED_FILE).exists());
        for name in ["identity.key", "friends.bin", "friends.bin.bak"] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert!(sealed_form(&bytes).is_some(), "{name}");
        }
        assert!(!dir.join(BACKUP_DIR).exists());
    }

    // A JPEG's first bytes: only a file that starts as a picture is kept as one.
    let picture_bytes: Vec<u8> = b"\xff\xd8\xff"
        .iter()
        .copied()
        .chain((0..300_000u32).map(|i| (i % 241) as u8))
        .collect();
    let video_bytes: Vec<u8> = (0..700_000u32).map(|i| (i % 239) as u8).collect();
    let (picture, video) = (base.join("lake.jpg"), base.join("river.mp4"));
    std::fs::write(&picture, &picture_bytes).unwrap();
    std::fs::write(&video, &video_bytes).unwrap();

    let (port_a, port_b) = (free_port(), free_port());
    let (ana, mut events_a) = Core::start(dir_a.clone(), alone(port_a, None)).unwrap();
    let (bea, mut events_b) = Core::start(dir_b.clone(), alone(port_b, Some(port_a))).unwrap();
    let online = |e: &CoreEvent| matches!(e, CoreEvent::FriendOnline { .. });
    wait_for(&mut events_a, "Bea online", online).await;
    wait_for(&mut events_b, "Ana online", online).await;

    let files = vec![
        Outgoing {
            path: picture.clone(),
            video: None,
        },
        Outgoing {
            path: video.clone(),
            video: None,
        },
    ];
    ana.send_direct(bea.me(), "the lake photos", files)
        .await
        .unwrap();
    let chat = chat_until(&bea, ana.me(), "the picture here", |c| {
        c.items
            .last()
            .is_some_and(|i| i.files.first().is_some_and(|f| f.file.local == Local::Here))
    })
    .await;
    let item = chat.items.last().unwrap();
    let (picture_id, video_id) = (item.files[0].file.id, item.files[1].file.id);
    assert_eq!(
        bea.direct_file_bytes(ana.me(), picture_id, 1 << 20)
            .await
            .unwrap(),
        picture_bytes
    );
    assert!(
        bea.direct_file_bytes(ana.me(), picture_id, 1000)
            .await
            .is_err()
    );
    bea.fetch_direct_file(ana.me(), video_id).await.unwrap();
    chat_until(&bea, ana.me(), "the video here", |c| {
        c.items.last().unwrap().files[1].file.local == Local::Here
    })
    .await;
    let play = bea.direct_file_to_play(ana.me(), video_id).await.unwrap();
    assert_eq!(std::fs::read(&play).unwrap(), video_bytes);
    bea.forget_direct_file_copy(&play);
    assert!(!play.exists());
    let downloads = base.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let saved = bea
        .save_direct_file(ana.me(), video_id, &downloads)
        .await
        .unwrap();
    assert_eq!(std::fs::read(saved).unwrap(), video_bytes);
    bea.send_direct(ana.me(), "how lovely, café time", Vec::new())
        .await
        .unwrap();
    chat_until(&ana, bea.me(), "Bea's answer", |c| {
        c.items.iter().any(|i| match &i.item.body {
            Body::Message { text, .. } => text == "how lovely, café time",
            _ => false,
        })
    })
    .await;

    // Nothing written or sent shows in either folder: the journals, the kept copies and the
    // files fetched are all sealed.
    for dir in [&dir_a, &dir_b] {
        for needle in [
            &b"the lake photos"[..],
            b"qu\xc3\xa9 bonito",
            &picture_bytes[..4096],
        ] {
            assert_eq!(shows(dir, needle), None);
        }
    }

    // Read back after a restart.
    ana.shutdown().await.unwrap();
    drop(ana);
    let (ana, _) = start_again(&dir_a, alone(port_a, None)).await;
    let chat = ana.direct_chat(bea.me(), 100).await.unwrap();
    assert_eq!(chat.items.len(), 2);
    assert_eq!(
        ana.direct_file_bytes(bea.me(), picture_id, 1 << 20)
            .await
            .unwrap(),
        picture_bytes
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// A folder as an earlier version left it: plain, with a chat and a picture kept. Its
/// identity, the friend, the picture, its bytes and where it is kept.
fn plain_leo(
    dir: &Path,
) -> (
    DeviceIdentity,
    DevicePublicKey,
    Attachment,
    Vec<u8>,
    PathBuf,
) {
    let dir = dir.to_path_buf();
    let store = Store::open_with(dir.clone(), Keychain::Off).unwrap();
    let (me, _) = store.identity().unwrap();
    store
        .save_friends(&FriendBook::new(&me, "Leo", 1).unwrap())
        .unwrap();
    let friend = DeviceIdentity::generate().unwrap().public_key();
    let picture: Vec<u8> = (0..150_000u32).map(|i| (i % 233) as u8).collect();
    let file = Attachment {
        id: *blake3::hash(&picture).as_bytes(),
        name: "beach.png".into(),
        size: picture.len() as u64,
        kind: FileKind::Picture,
        video: None,
        local: Local::Here,
        voice: None,
    };
    let chats = DirectStore::open(&dir);
    chats.files_dir(&friend).unwrap();
    std::fs::write(chats.file_path(&friend, &file), &picture).unwrap();
    let mut chat = Chat::default();
    let record = Record::Item {
        item: Item {
            id: [1; 16],
            mine: true,
            sent_ms: 1,
            got_ms: 1,
            seq: 1,
            at_ms: 1,
            body: Body::Message {
                text: "look at the beach".into(),
                files: vec![file.clone()],
            },
            ttl_ms: 0,
            delivered: true,
        },
    };
    record.apply(&mut chat, &mut None);
    chats.append(&friend, &[record], &chat).unwrap();
    drop((store, chats));
    let kept = DirectStore::open(&dir).file_path(&friend, &file);

    (me, friend, file, picture, kept)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plain_folder_is_sealed_at_its_first_start() {
    let _one = ONE_AT_A_TIME.lock().await;
    let keys = keychain();
    let base = std::env::temp_dir().join(format!("fogata-sealed-migrate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("leo");

    let (me, friend, file, picture, kept) = plain_leo(&dir);

    let (core, _) = Core::start(dir.clone(), alone(free_port(), None)).unwrap();
    assert_eq!(core.me(), me.public_key());
    assert!(core.store().sealed());
    let view = core.direct_chat(friend, 100).await.unwrap();
    assert_eq!(view.items.len(), 1);
    assert_eq!(
        core.direct_file_bytes(friend, file.id, 1 << 20)
            .await
            .unwrap(),
        picture
    );
    // The picture is sealed in the background.
    timeout(Duration::from_secs(20), async {
        while sealed_form(&std::fs::read(&kept).unwrap()).is_none() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the picture was not sealed");
    assert_eq!(
        core.direct_file_bytes(friend, file.id, 1 << 20)
            .await
            .unwrap(),
        picture
    );
    assert_eq!(shows(&dir, b"look at the beach"), None);
    assert_eq!(shows(&dir, &picture[..4096]), None);
    assert_eq!(core.at_rest(), AtRest::Sealed);
    // The chats' plain copies went once everything read back sealed; the identity's waits
    // for a start that opens the folder whole with the key the keychain gives again.
    assert!(!dir.join(BACKUP_DIR).join("direct").exists());
    assert!(dir.join(BACKUP_DIR).join("identity.key").exists());
    core.shutdown().await.unwrap();
    drop(core);

    let (core, _) = start_again(&dir, alone(free_port(), None)).await;
    assert!(!dir.join(BACKUP_DIR).exists());
    assert_eq!(core.direct_chat(friend, 100).await.unwrap().items.len(), 1);
    core.shutdown().await.unwrap();
    assert!(!keys.names().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

/// The folder says it is not all sealed while it is not: its plain copies cannot be made
/// (`before-sealing` is a file), or an attachment cannot be sealed (its folder cannot be
/// written). Each start goes on, and the first that seals it all says so.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_folder_not_all_sealed_says_so_until_it_is() {
    use std::os::unix::fs::PermissionsExt;
    let _one = ONE_AT_A_TIME.lock().await;
    keychain();
    let base =
        std::env::temp_dir().join(format!("fogata-sealed-unfinished-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let dir = base.join("leo");
    let (me, friend, file, picture, kept) = plain_leo(&dir);
    let identity = std::fs::read(dir.join("identity.key")).unwrap();

    std::fs::write(dir.join(BACKUP_DIR), b"not a folder").unwrap();
    let files = kept.parent().unwrap().to_path_buf();
    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o500)).unwrap();
    let (core, _) = Core::start(dir.clone(), alone(free_port(), None)).unwrap();
    assert_eq!(core.me(), me.public_key());
    assert_ne!(core.at_rest(), AtRest::Sealed);
    assert_eq!(core.at_rest(), AtRest::Unfinished);
    assert_eq!(std::fs::read(dir.join("identity.key")).unwrap(), identity);
    assert_eq!(core.direct_chat(friend, 100).await.unwrap().items.len(), 1);
    core.shutdown().await.unwrap();
    drop(core);

    // The copies can be made now; the picture still cannot be sealed.
    std::fs::remove_file(dir.join(BACKUP_DIR)).unwrap();
    let (core, _) = start_again(&dir, alone(free_port(), None)).await;
    assert!(core.store().sealed());
    assert_ne!(
        sealed_form(&std::fs::read(dir.join("identity.key")).unwrap()),
        None
    );
    // Given time to be sealed in the background, the picture is not.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(sealed_form(&std::fs::read(&kept).unwrap()), None);
    assert_eq!(core.at_rest(), AtRest::Unfinished);
    core.shutdown().await.unwrap();
    drop(core);

    std::fs::set_permissions(&files, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (core, _) = start_again(&dir, alone(free_port(), None)).await;
    timeout(Duration::from_secs(20), async {
        while sealed_form(&std::fs::read(&kept).unwrap()).is_none() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the picture was not sealed");
    assert_eq!(core.at_rest(), AtRest::Sealed);
    assert_eq!(
        core.direct_file_bytes(friend, file.id, 1 << 20)
            .await
            .unwrap(),
        picture
    );
    core.shutdown().await.unwrap();
    let _ = std::fs::remove_dir_all(&base);
}
