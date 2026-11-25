//! Measurements of what many chats and their sealing cost the direct store, run by hand (`--ignored`).

use super::tests::item;
use super::*;
use crate::direct::model::{Body, FileKind, Item, Local};
use fogata_crypto::DeviceIdentity;
use std::io::Seek;

/// What a start costs with 20 sealed chats of 10,000 messages each: every chat read
/// whole (as before summaries were kept, and as a folder's first start without them
/// still does), and a start from the summaries, with one chat read when it is opened;
/// the time, and what this process holds more once done, by `ps`.
/// `cargo test --release -p fogata-core --lib cost_of_many_chats -- --ignored --nocapture`.
#[test]
#[ignore]
fn cost_of_many_chats() {
    use std::time::Instant;
    let dir = std::env::var_os("FOGATA_COST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("fogata-cost-chats-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let key = DataKey::generate().unwrap();
    let mut chat = Chat::default();
    for n in 0..10_000u32 {
        let mut id = [0u8; 16];
        id[..4].copy_from_slice(&n.to_be_bytes());
        chat.insert(Item {
            id,
            mine: n % 2 == 0,
            sent_ms: u64::from(n) * 1000,
            got_ms: u64::from(n) * 1000,
            seq: u64::from(n),
            at_ms: u64::from(n) * 1000,
            body: Body::Message {
                text: format!("message {n}: see you at eight in the square, I'll bring the guitar"),
                files: Vec::new(),
            },
            ttl_ms: 0,
            delivered: true,
        });
    }
    let store = DirectStore::open_sealed(&dir, Some(key.clone()));
    for _ in 0..20 {
        let friend = DeviceIdentity::generate().unwrap().public_key();
        store.rewrite(&friend, &chat, None).unwrap();
    }
    drop(chat);
    let rss_kb = || {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<u64>()
            .unwrap()
    };
    let ms = |start: Instant| start.elapsed().as_secs_f64() * 1000.0;
    let open = || {
        super::super::Chats::open(
            &dir,
            Some(key.clone()),
            std::sync::Arc::new(crate::writer::Writer::start().unwrap()),
        )
    };
    // Every chat read whole, as before summaries were kept (and as the first start of a
    // folder without them still does).
    let before = rss_kb();
    let start = Instant::now();
    let chats = DirectStore::open_sealed(&dir, Some(key.clone())).chats();
    let read_all = ms(start);
    let held_all = rss_kb().saturating_sub(before) / 1024;
    assert_eq!(chats.len(), 20);
    assert!(chats.iter().all(|(_, l)| l.chat.items().len() == 10_000));
    drop(chats);
    // The first start of such a folder reads each chat and keeps its summary.
    let start = Instant::now();
    let first = open();
    let first_ms = ms(start);
    first.settle();
    drop(first);
    // Then a start reads the summaries only, and a chat when it is opened.
    let before = rss_kb();
    let start = Instant::now();
    let chats = open();
    let start_ms = ms(start);
    let held_start = rss_kb().saturating_sub(before) / 1024;
    assert_eq!(
        chats.store.loads.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    let friend = *chats.lock().chats.keys().next().unwrap();
    let start = Instant::now();
    assert_eq!(chats.lock().chats[&friend].chat.items().len(), 10_000);
    let opened_ms = ms(start);
    // Another read back from its journal's end, whole (as a search would) and by a page.
    let other = *chats.lock().chats.keys().nth(1).unwrap();
    let start = Instant::now();
    assert_eq!(chats.store.scan_back(&other, |_| true).unwrap(), 10_000);
    let back_ms = ms(start);
    let start = Instant::now();
    let (page, more) = chats.store.page_back(&other, None, 200).unwrap();
    assert!(page.len() == 200 && more);
    let page_ms = ms(start);
    eprintln!(
        "20 sealed chats of 10,000 messages: all read whole in {read_all:.0} ms, holding \
         {held_all} MB more; a first start that keeps their summaries {first_ms:.0} ms; a \
         start from the summaries {start_ms:.1} ms, holding {held_start} MB more; one chat \
         read when opened {opened_ms:.1} ms; one read back whole {back_ms:.1} ms, its \
         newest 200 {page_ms:.1} ms"
    );
    drop(chats);
    fs::remove_dir_all(&dir).unwrap();
}

/// What sealing costs: a journal of 10,000 messages written whole, read back and added
/// to, plain and sealed, and a large attachment kept, read, and sealed in its place.
/// `cargo test --release -p fogata-core --lib cost_of_sealing -- --ignored --nocapture`,
/// in `FOGATA_COST_DIR` (on a real disk) when set.
#[test]
#[ignore]
fn cost_of_sealing() {
    use std::time::Instant;
    let base = std::env::var_os("FOGATA_COST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("fogata-cost-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let friend = DeviceIdentity::generate().unwrap().public_key();
    let mut chat = Chat::default();
    for n in 0..10_000u32 {
        let mut id = [0u8; 16];
        id[..4].copy_from_slice(&n.to_be_bytes());
        chat.insert(Item {
            id,
            mine: n % 2 == 0,
            sent_ms: u64::from(n) * 1000,
            got_ms: u64::from(n) * 1000,
            seq: u64::from(n),
            at_ms: u64::from(n) * 1000,
            body: Body::Message {
                text: format!("message {n}: see you at eight in the square, I'll bring the guitar"),
                files: Vec::new(),
            },
            ttl_ms: 0,
            delivered: true,
        });
    }
    let ms = |start: Instant| start.elapsed().as_secs_f64() * 1000.0;
    for key in [None, Some(DataKey::generate().unwrap())] {
        let what = if key.is_some() { "sealed" } else { "plain" };
        let dir = base.join(what);
        let store = DirectStore::open_sealed(&dir, key.clone());
        let start = Instant::now();
        store.rewrite(&friend, &chat, None).unwrap();
        let write = ms(start);
        let size = fs::metadata(store.journal_path(&friend)).unwrap().len();
        let start = Instant::now();
        let loaded = DirectStore::open_sealed(&dir, key.clone())
            .load(&friend)
            .unwrap();
        let read = ms(start);
        assert_eq!(loaded.chat.items().len(), 10_000);
        let mut grown = chat.clone();
        let start = Instant::now();
        for n in 0..100u8 {
            let record = Record::Item {
                item: Item {
                    id: [n; 16],
                    ..item(n, true, vec![])
                },
            };
            record.apply(&mut grown, &mut None);
            store.append(&friend, &[record], &grown).unwrap();
        }
        let append = ms(start) / 100.0;
        eprintln!(
            "journal of 10,000 messages, {what}: {size} bytes, written whole in {write:.1} ms, \
             read in {read:.1} ms, one message added in {append:.2} ms"
        );
    }

    let mb: usize = std::env::var("FOGATA_COST_MB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(512);
    let bytes: Vec<u8> = (0..mb << 20).map(|i| (i as u32 % 251) as u8).collect();
    let source = base.join("big.mkv");
    fs::write(&source, &bytes).unwrap();
    let file = Attachment {
        id: *blake3::hash(&bytes).as_bytes(),
        name: "big.mkv".into(),
        size: bytes.len() as u64,
        kind: FileKind::Video,
        video: None,
        local: Local::Here,
        voice: None,
    };
    drop(bytes);
    let mbs = |start: Instant| mb as f64 / start.elapsed().as_secs_f64();
    for key in [None, Some(DataKey::generate().unwrap())] {
        let what = if key.is_some() { "sealed" } else { "plain" };
        let store = DirectStore::open_sealed(&base.join(what), key.clone());
        store.files_dir(&friend).unwrap();
        let start = Instant::now();
        store.keep_copy(&friend, &file, &source).unwrap();
        let keep = mbs(start);
        let start = Instant::now();
        let (mut kept, _) = store.open_file(&friend, &file).unwrap();
        io::copy(&mut kept, &mut io::sink()).unwrap();
        let read = mbs(start);
        let start = Instant::now();
        let (mut kept, size) = store.open_file(&friend, &file).unwrap();
        kept.seek(io::SeekFrom::Start(size / 2)).unwrap();
        let mut piece = vec![0u8; 64 * 1024];
        kept.read_exact(&mut piece).unwrap();
        let seek = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "attachment of {mb} MiB, {what}: kept at {keep:.0} MiB/s, read at {read:.0} \
             MiB/s, 64 KiB from its middle in {seek:.2} ms"
        );
    }
    // Sealing one kept from before, in its place, read back to check it.
    let store = DirectStore::open_sealed(&base.join("plain"), Some(DataKey::generate().unwrap()));
    let path = store.file_path(&friend, &file);
    let start = Instant::now();
    store
        .seal_file(&path, |sealed| fs::rename(sealed, &path))
        .unwrap();
    eprintln!(
        "attachment of {mb} MiB sealed in its place (sealed, then read back) at {:.0} MiB/s",
        mbs(start)
    );
    fs::remove_dir_all(&base).unwrap();
}
