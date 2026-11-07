//! Fuzzing the decoders of everything that comes from outside: other devices, links people
//! paste, and files on the disk. Each target takes any bytes, reads them as Fogata does and
//! must neither panic nor take long; its seeds are well-formed examples to start from.
//!
//! The targets run two ways: as long runs of damaged copies of the seeds (the ignored tests
//! of this module, `tools/fuzz.sh`), and under libFuzzer (`fuzz/`, `tools/fuzz.sh
//! --libfuzzer`), which builds this crate with the `fuzzing` feature.

use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use fogata_crypto::{
    CHALLENGE_LEN, DeviceIdentity, GroupKeys, GroupSecret, MembershipProof, Sealed, SealedProof,
    at_rest::{
        CHAIN_START, DataKey, Form, SealReader, SealWriter, chain_header, chain_link,
        records_header,
    },
    codec::{Reader, Writer},
    group_tag, group_tag_matches,
};
pub use fogata_friends::fuzzing::friend_sealed;
use fogata_friends::{
    Animated, Animation, Avatar, Banner, Bio, FriendBook, FriendCode, FriendLink, FriendSecret,
    Introduction, Profile,
};
pub use fogata_media::fuzzing::{parts, turn};
use fogata_mls::{ChainLink, Followed, GroupMember};
use fogata_net::fuzzing::{device, encoded, framed, sample_log};
pub use fogata_net::fuzzing::{
    friend_wire, log, member_wire, node_friend, node_join, node_request, room_event, sealed_name,
    typing,
};
use fogata_proto::{DirectFile, DirectFileKind, DirectPayload, WireVideo, WireVoice};
use fogata_state::{
    Body, Changes, Entry, EntryHash, EpochLink, GroupBanner, GroupLog, GroupPicture, Ticket,
    Verdict, payload_digest,
};
pub use fogata_voice::fuzzing::{ogg_read, recorder, resample, voice_play, wsola_step};

use crate::{
    direct::{self, model::Chat, store::Record},
    invite,
    store::decode_entries,
};

/// A GIF of one pixel.
const ONE_PIXEL_GIF: &[u8] =
    b"GIF89a\x01\x00\x01\x00\x00\x00\x00,\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02D\x01\x00;";

/// The clock the runs read: fixed, so the same input always does the same.
const NOW_MS: u64 = 1_800_000_000_000;

/// A decoder to fuzz: its name, what reads the bytes, and well-formed inputs.
#[derive(Debug)]
pub struct Target {
    pub name: &'static str,
    pub run: fn(&[u8]),
    pub seeds: fn() -> Vec<Vec<u8>>,
}

/// Every target. `fuzz/fuzz_targets/` has one file for each, of the same name.
pub const TARGETS: &[Target] = &[
    Target {
        name: "entry",
        run: entry,
        seeds: entry_seeds,
    },
    Target {
        name: "log",
        run: log,
        seeds: fogata_net::fuzzing::log_seeds,
    },
    Target {
        name: "proof",
        run: proof,
        seeds: proof_seeds,
    },
    Target {
        name: "member_wire",
        run: member_wire,
        seeds: fogata_net::fuzzing::member_wire_seeds,
    },
    Target {
        name: "node_request",
        run: node_request,
        seeds: fogata_net::fuzzing::node_request_seeds,
    },
    Target {
        name: "node_join",
        run: node_join,
        seeds: fogata_net::fuzzing::node_join_seeds,
    },
    Target {
        name: "node_friend",
        run: node_friend,
        seeds: fogata_net::fuzzing::node_friend_seeds,
    },
    Target {
        name: "friend_wire",
        run: friend_wire,
        seeds: fogata_net::fuzzing::friend_wire_seeds,
    },
    Target {
        name: "friend_sealed",
        run: friend_sealed,
        seeds: fogata_friends::fuzzing::friend_sealed_seeds,
    },
    Target {
        name: "room_event",
        run: room_event,
        seeds: fogata_net::fuzzing::room_event_seeds,
    },
    Target {
        name: "parts",
        run: parts,
        seeds: fogata_media::fuzzing::parts_seeds,
    },
    Target {
        name: "turn",
        run: turn,
        seeds: fogata_media::fuzzing::turn_seeds,
    },
    Target {
        name: "direct_payload",
        run: direct_payload,
        seeds: direct_payload_seeds,
    },
    Target {
        name: "typing",
        run: typing,
        seeds: fogata_net::fuzzing::typing_seeds,
    },
    Target {
        name: "sealed_name",
        run: sealed_name,
        seeds: fogata_net::fuzzing::sealed_name_seeds,
    },
    Target {
        name: "invite",
        run: invite_text,
        seeds: invite_seeds,
    },
    Target {
        name: "records",
        run: records,
        seeds: records_seeds,
    },
    Target {
        name: "at_rest",
        run: at_rest,
        seeds: at_rest_seeds,
    },
    Target {
        name: "mls",
        run: mls,
        seeds: mls_seeds,
    },
    Target {
        name: "ogg_read",
        run: ogg_read,
        seeds: fogata_voice::fuzzing::ogg_seeds,
    },
    Target {
        name: "voice_play",
        run: voice_play,
        seeds: fogata_voice::fuzzing::voice_play_seeds,
    },
    Target {
        name: "wsola_step",
        run: wsola_step,
        seeds: fogata_voice::fuzzing::wsola_step_seeds,
    },
    Target {
        name: "resample",
        run: resample,
        seeds: fogata_voice::fuzzing::resample_seeds,
    },
    Target {
        name: "recorder",
        run: recorder,
        seeds: fogata_voice::fuzzing::recorder_seeds,
    },
];

/// A group log entry and an invite ticket; what reads must write and read back the same.
pub fn entry(data: &[u8]) {
    // Each reads from one encoding only: what is signed and hashed is what came.
    if let Ok(entry) = Entry::decode(data) {
        assert_eq!(entry.encode(), data);
        assert_eq!(Entry::decode(&entry.encode()).as_ref(), Ok(&entry));
        let _ = (
            entry.signature_valid(),
            entry.hash(),
            entry.body.is_room_rule(),
        );
    }
    if let Ok(ticket) = Ticket::decode(data) {
        assert_eq!(ticket.encode(), data);
        assert_eq!(Ticket::decode(&ticket.encode()).as_ref(), Ok(&ticket));
        let _ = ticket.signature_valid();
    }
}

fn entry_seeds() -> Vec<Vec<u8>> {
    let log = sample_log();
    let me = device(0);
    let mut out = encoded(&log);
    out.push(log.issue_ticket(&me, NOW_MS).expect("a ticket").encode());
    for body in [
        Body::Unknown {
            kind: 200,
            bytes: vec![1, 2, 3],
        },
        Body::Level { level: 3 },
    ] {
        out.push(Entry::sign(&me, log.group_id(), log.heads(), NOW_MS, body).encode());
    }
    out
}

/// The group keys of the runs' proofs.
fn keys() -> GroupKeys {
    GroupKeys::derive(
        fogata_crypto::GroupId::from_bytes(&[9; 16]).expect("16 bytes"),
        0,
        &GroupSecret::from_bytes(&[9; 32]).expect("32 bytes"),
    )
}

const PROVER: &[u8] = b"prover";
const VERIFIER: &[u8] = b"verifier";
const CHALLENGE: [u8; CHALLENGE_LEN] = [3; CHALLENGE_LEN];

/// What proves membership: a sealed proof (first byte 0: its epoch, nonce and ciphertext),
/// a box sealed to this device (1), or a group tag (2).
pub fn proof(data: &[u8]) {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    match mode % 3 {
        0 => {
            let Some((epoch, rest)) = rest.split_at_checked(4) else {
                return;
            };
            let Some((nonce, ciphertext)) = rest.split_at_checked(24) else {
                return;
            };
            let sealed = SealedProof {
                epoch: u32::from_be_bytes(epoch.try_into().expect("4 bytes")),
                nonce: nonce.try_into().expect("24 bytes"),
                ciphertext: ciphertext.to_vec(),
            };
            let _ = MembershipProof::open(&sealed, &keys(), PROVER, VERIFIER, &CHALLENGE);
        }
        1 => {
            let mut r = Reader::new(rest);
            if let Ok(sealed) = r.sealed() {
                let _ = sealed.open(&device(0), b"fuzz");
            }
        }
        _ => {
            let _ = group_tag_matches(&keys(), VERIFIER, PROVER, &CHALLENGE, rest);
        }
    }
}

fn proof_seeds() -> Vec<Vec<u8>> {
    let keys = keys();
    let sealed = MembershipProof::create(&keys, &device(1), PROVER, VERIFIER, &CHALLENGE)
        .seal(&keys, PROVER, VERIFIER, &CHALLENGE)
        .expect("sealed");
    let mut first = vec![0u8];
    first.extend_from_slice(&sealed.epoch.to_be_bytes());
    first.extend_from_slice(&sealed.nonce);
    first.extend_from_slice(&sealed.ciphertext);
    let mut w = Writer::default();
    w.sealed(&Sealed::seal(&device(0).public_key(), b"fuzz", b"hi").expect("sealed"));
    let boxed = [&[1u8][..], &w.into_bytes()].concat();
    let tag = [&[2u8][..], &group_tag(&keys, VERIFIER, PROVER, &CHALLENGE)].concat();
    vec![first, boxed, tag]
}

/// Messages of a direct chat, one after the other, each after its length (2 bytes), taken
/// in as the friend's messages are, then what the chat shows of them.
pub fn direct_payload(data: &[u8]) {
    let mut chat = Chat::default();
    let mut rest = data;
    while let Some((len, tail)) = rest.split_at_checked(2) {
        let len = usize::from(u16::from_be_bytes([len[0], len[1]]));
        let Some((bytes, tail)) = tail.split_at_checked(len) else {
            break;
        };
        rest = tail;
        let Ok(payload) = DirectPayload::decode(bytes) else {
            continue;
        };
        assert_eq!(
            DirectPayload::decode(&payload.encode()).as_ref(),
            Ok(&payload)
        );
        if let DirectPayload::Receipt { ids } = &payload {
            for id in ids {
                if let Some(item) = chat.get_mut(id) {
                    item.delivered = true;
                }
            }
            continue;
        }
        let Some(item) = direct::received(payload, NOW_MS) else {
            continue;
        };
        if let Some(target) = item.pin_target()
            && !chat.get(target).is_some_and(|i| i.is_message())
        {
            continue;
        }
        if let Some(item) = chat.add(item) {
            let _ = direct::model::preview(&item, "Bea", |ms| format!("{ms}"));
        }
    }
    let _ = (
        chat.pinned(),
        chat.unread(),
        chat.usage(),
        chat.next_expiry(),
    );
    let _ = chat.expired(NOW_MS + 86_400_000);
}

fn direct_payload_seeds() -> Vec<Vec<u8>> {
    let payloads = [
        DirectPayload::Message {
            id: [1; 16],
            sent_ms: NOW_MS,
            text: "hi".into(),
            files: vec![DirectFile {
                id: [2; 32],
                name: "clip.mp4".into(),
                size: 1000,
                kind: DirectFileKind::Video,
                video: Some(WireVideo {
                    thumb: vec![0xff, 0xd8, 0xff],
                    width: 640,
                    height: 360,
                    duration_ms: 4000,
                }),
                voice: None,
            }],
            ttl_ms: 0,
        },
        DirectPayload::Message {
            id: [6; 16],
            sent_ms: NOW_MS,
            text: String::new(),
            files: vec![DirectFile {
                id: [7; 32],
                name: "Voice message.ogg".into(),
                size: 60_000,
                kind: DirectFileKind::File,
                video: None,
                voice: Some(WireVoice {
                    duration_ms: 18_000,
                    bars: vec![128; fogata_proto::VOICE_BARS],
                }),
            }],
            ttl_ms: 0,
        },
        DirectPayload::Timer {
            id: [3; 16],
            sent_ms: NOW_MS,
            ms: 3_600_000,
        },
        DirectPayload::Pin {
            id: [4; 16],
            sent_ms: NOW_MS,
            target: [1; 16],
            pinned: true,
        },
        DirectPayload::Receipt {
            ids: vec![[1; 16], [5; 16]],
        },
    ];
    let mut together = Vec::new();
    for payload in &payloads {
        let bytes = payload.encode();
        together.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        together.extend_from_slice(&bytes);
    }
    vec![together]
}

/// Text people paste: an invite to a group, a friend link or a friend code.
pub fn invite_text(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(invite) = invite::decode(text) {
        let back = invite::decode(&invite::encode(&invite)).expect("reads back");
        assert_eq!(back.ticket, invite.ticket);
    }
    if let Ok(link) = FriendLink::decode(text) {
        let back = FriendLink::decode(&link.encode()).expect("reads back");
        assert_eq!(back.device, link.device);
    }
    if let Ok(code) = FriendCode::parse(text) {
        assert!(FriendCode::parse(&code.to_string()).is_ok_and(|c| c == code));
    }
}

fn invite_seeds() -> Vec<Vec<u8>> {
    let mut log = sample_log();
    let me = device(0);
    let address: fogata_net::Multiaddr = "/ip4/203.0.113.7/udp/24000/quic-v1"
        .parse()
        .expect("an address");
    let text = invite::make(&mut log, &me, 7, vec![address.clone()], NOW_MS).expect("an invite");
    let link = FriendLink::new(
        me.public_key(),
        "Ana",
        b"peer".to_vec(),
        vec![address.to_vec()],
        NOW_MS,
    )
    .expect("a link");
    vec![
        text.into_bytes(),
        link.encode().into_bytes(),
        FriendCode::generate(1_800_000_000)
            .expect("a code")
            .to_string()
            .into_bytes(),
    ]
}

/// Signed records: a group banner or picture (first byte 0), an animation (1), a profile (2), a
/// friend's banner (3), a bio (4), or a whole friends file (5). What reads must write and
/// read back the same.
pub fn records(data: &[u8]) {
    let Some((&kind, rest)) = data.split_first() else {
        return;
    };
    match kind % 6 {
        0 => {
            if let Ok(banner) = GroupBanner::decode(rest) {
                assert_eq!(GroupBanner::decode(&banner.encode()).as_ref(), Ok(&banner));
                let _ = (banner.verify(), banner.digest(), banner.replaces(&banner));
            }
            if let Ok(picture) = GroupPicture::decode(rest) {
                assert_eq!(
                    GroupPicture::decode(&picture.encode()).as_ref(),
                    Ok(&picture)
                );
                let _ = (
                    picture.verify(),
                    picture.digest(),
                    picture.replaces(&picture),
                );
            }
        }
        1 => {
            if let Ok(animation) = Animation::decode(rest) {
                assert_eq!(
                    Animation::decode(&animation.encode()).as_ref(),
                    Ok(&animation)
                );
                let _ = (animation.verify(), animation.animates(b"\x89PNG"));
            }
        }
        2 => {
            if let Ok(profile) = Profile::decode(rest) {
                assert_eq!(Profile::decode(&profile.encode()).as_ref(), Ok(&profile));
                let _ = profile.verify();
            }
        }
        3 => {
            if let Ok(banner) = Banner::decode(rest) {
                assert_eq!(Banner::decode(&banner.encode()).as_ref(), Ok(&banner));
                let _ = banner.verify();
            }
        }
        4 => {
            if let Ok(bio) = Bio::decode(rest) {
                assert_eq!(Bio::decode(&bio.encode()).as_ref(), Ok(&bio));
                let _ = bio.verify();
            }
        }
        _ => {
            if let Ok(book) = FriendBook::decode(rest) {
                let again = FriendBook::decode(&book.encode()).expect("reads back");
                assert_eq!(again.encode(), book.encode());
            }
        }
    }
}

fn records_seeds() -> Vec<Vec<u8>> {
    let (me, bea, cid, dan) = (device(0), device(1), device(2), device(3));
    let png = b"\x89PNG\r\n\x1a\n0000".to_vec();
    let group = sample_log().group_id();
    let intro = |identity: &DeviceIdentity, name: &str| Introduction {
        profile: Profile::sign(identity, name, Avatar::Preset(1), 3).expect("a profile"),
        secret: FriendSecret::from_bytes(&[identity.public_key().as_bytes()[0]; 32])
            .expect("32 bytes"),
    };
    let mut book = FriendBook::new(&me, "Ana", 1).expect("a book");
    book.request_received(intro(&bea, "Bea"), 2);
    book.answer(&bea.public_key(), true, 3).expect("answered");
    book.request_received(intro(&cid, "Cid"), 4);
    book.request_sent(dan.public_key(), "Dan", 5).expect("sent");
    let png_picture = png.clone();
    book.set_banner(&me, 0xFF3D21AD, Some(png.clone()), 6)
        .expect("a banner");
    let records = [
        GroupBanner::sign(&me, group, png.clone(), NOW_MS)
            .expect("a banner")
            .encode(),
        Animation::sign(&me, Animated::Photo, &png, ONE_PIXEL_GIF.to_vec(), 7)
            .expect("an animation")
            .encode(),
        Profile::sign(&me, "Ana", Avatar::Photo(png.clone()), 8)
            .expect("a profile")
            .encode(),
        Banner::sign(&me, 0x112233FF, Some(png), 9)
            .expect("a banner")
            .encode(),
        Bio::sign(&me, "Hi,\nI'm Ana", 10).expect("a bio").encode(),
        book.encode().to_vec(),
        // Numbered 6, read as kind 0.
        GroupPicture::sign(&me, group, png_picture, NOW_MS)
            .expect("a picture")
            .encode(),
    ];
    records
        .into_iter()
        .enumerate()
        .map(|(kind, record)| [&[kind as u8][..], &record].concat())
        .collect()
}

/// The key and context of the runs' sealed files.
fn data_key() -> DataKey {
    DataKey::from_bytes(&[3; 32])
}
const CONTEXT: &str = "fuzz";
/// The chat whose records the `at_rest` target reads.
const CHAT: &str = "chat";

/// Files from the disk: a whole sealed file (first byte 0), a chat's journal sealed (1) or
/// plain (2), a sealed attachment (3) read whole and from the places the next bytes name,
/// or a group log (4).
pub fn at_rest(data: &[u8]) {
    let Some((&kind, rest)) = data.split_first() else {
        return;
    };
    let key = data_key();
    match kind % 5 {
        0 => {
            let _ = key.open_whole(CONTEXT, rest);
        }
        1 => {
            let _ = direct::store::decode_records(Some(&key), CONTEXT, CHAT, rest);
        }
        2 => {
            let _ = direct::store::decode_records(None, CONTEXT, CHAT, rest);
        }
        3 => {
            let len = rest.len() as u64;
            if let Ok(mut reader) = SealReader::new(&key, CONTEXT, Cursor::new(rest), len) {
                let plain = reader.len();
                let mut all = Vec::new();
                if reader.read_to_end(&mut all).is_ok() {
                    assert_eq!(all.len() as u64, plain);
                }
                for at in rest.iter().take(8) {
                    let to = plain * u64::from(*at) / 255;
                    if reader.seek(SeekFrom::Start(to)).is_ok() {
                        let mut buf = [0u8; 100];
                        let _ = reader.read(&mut buf);
                    }
                }
            }
        }
        _ => {
            let (entries, _) = decode_entries(rest);
            for entry in entries {
                assert_eq!(Entry::decode(&entry.encode()).as_ref(), Ok(&entry));
            }
        }
    }
}

fn at_rest_seeds() -> Vec<Vec<u8>> {
    let key = data_key();
    let whole = key
        .seal_whole(CONTEXT, b"what a whole file holds")
        .expect("sealed");
    let item = direct::received(
        DirectPayload::Message {
            id: [1; 16],
            sent_ms: NOW_MS,
            text: "hi".into(),
            files: vec![],
            ttl_ms: 0,
        },
        NOW_MS,
    )
    .expect("an item");
    let records = [
        Record::Item { item },
        Record::Delivered { ids: vec![[2; 16]] },
        Record::Read { seq: 3 },
        Record::Gone { ids: vec![[4; 16]] },
        Record::Ended { ms: Some(NOW_MS) },
    ];
    // As a chain, and as records sealed each on their own (the earlier form).
    let mut chain = chain_header().to_vec();
    let mut older = records_header().to_vec();
    let mut plain = Vec::new();
    let mut before = CHAIN_START;
    for record in &records {
        let json = serde_json::to_vec(record).expect("json");
        let boxed = key
            .seal_link(&format!("{CONTEXT}/{CHAT}"), &before, &json)
            .expect("sealed");
        before = chain_link(&boxed).expect("a tag");
        chain.extend_from_slice(&(boxed.len() as u32).to_be_bytes());
        chain.extend_from_slice(&boxed);
        let boxed = key.seal(Form::Records, CONTEXT, &json).expect("sealed");
        older.extend_from_slice(&(boxed.len() as u32).to_be_bytes());
        older.extend_from_slice(&boxed);
        plain.extend_from_slice(&(json.len() as u32).to_be_bytes());
        plain.extend_from_slice(&json);
    }
    let mut writer = SealWriter::new(&key, CONTEXT, Vec::new()).expect("a writer");
    let contents: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    writer.write_all(&contents).expect("written");
    let stream = writer.finish().expect("finished");
    let log = framed(&encoded(&sample_log()));
    [
        (0u8, whole),
        (
            0,
            key.named("data-0123")
                .expect("a short name")
                .seal_whole(CONTEXT, b"what a whole file holds")
                .expect("sealed"),
        ),
        (1, chain),
        (1, older),
        (2, plain),
        (3, stream),
        (4, log),
    ]
    .into_iter()
    .map(|(kind, file)| [&[kind][..], &file].concat())
    .collect()
}

/// A group on MLS for the `mls` target, made once: Ana's log with Bea's KeyPackage and the
/// first Commit, the MLS messages beside it, Ana's state after it, and the state of a
/// device (Cid) holding a KeyPackage and waiting to be added.
struct MlsFixture {
    log: GroupLog,
    first: EntryHash,
    first_changes: Changes,
    first_payload: Vec<u8>,
    first_welcome: Vec<u8>,
    ana: Vec<u8>,
    cid: Vec<u8>,
    /// Bea's KeyPackage, a Commit of hers on epoch 1, Cid's KeyPackage, and a Welcome for
    /// Cid from Ana.
    bea_package: Vec<u8>,
    bea_update: Vec<u8>,
    cid_package: Vec<u8>,
    cid_welcome: Vec<u8>,
}

fn mls_fixture() -> &'static MlsFixture {
    static FIXTURE: std::sync::OnceLock<MlsFixture> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(|| {
        let (ana, bea, cid) = (device(0), device(1), device(2));
        let mut log = GroupLog::create(&ana, "friends", NOW_MS - 10_000).expect("created");
        log.add(&ana, bea.public_key(), "Bea", NOW_MS - 9_000)
            .expect("added");
        let group = log.group_id();
        let mut bea_mls = GroupMember::new(&bea, group);
        let bea_package = bea_mls.key_package().expect("a key package");
        let entry = Entry::sign(
            &bea,
            group,
            log.heads(),
            NOW_MS - 8_000,
            Body::MlsKeyPackage {
                key_package: bea_package.clone(),
            },
        );
        log.insert(entry).expect("inserted");
        let mut ana_mls = GroupMember::new(&ana, group);
        ana_mls.start().expect("started");
        let proposed = ana_mls
            .commit(&[(bea.public_key(), &bea_package)], &[])
            .expect("a commit");
        let first_welcome = proposed.welcome.clone().expect("a welcome");
        let entry = Entry::sign(
            &ana,
            group,
            log.heads(),
            NOW_MS - 7_000,
            Body::Commit {
                link: EpochLink {
                    epoch: 0,
                    prev: None,
                },
                changes: proposed.changes.clone(),
                payload: payload_digest(&proposed.payload),
                welcome: Some(payload_digest(&first_welcome)),
            },
        );
        let first = entry.hash();
        log.insert(entry).expect("inserted");
        let link = ChainLink {
            entry: first,
            author: ana.public_key(),
            digest: payload_digest(&proposed.payload),
            changes: Some(&proposed.changes),
            payload: Some(&proposed.payload),
            welcome: Some(&first_welcome),
        };
        assert_eq!(
            ana_mls.follow(&[link], NOW_MS).expect("followed"),
            Followed::UpToDate
        );
        assert_eq!(
            bea_mls.follow(&[link], NOW_MS).expect("followed"),
            Followed::UpToDate
        );
        let bea_update = bea_mls.commit(&[], &[]).expect("an update").payload;
        let cid_mls = GroupMember::new(&cid, group);
        let cid_package = cid_mls.key_package().expect("a key package");
        let cid_welcome = GroupMember::from_bytes(&ana, &ana_mls.to_bytes())
            .expect("reads")
            .commit(&[(cid.public_key(), &cid_package)], &[])
            .expect("a commit")
            .welcome
            .expect("a welcome");
        MlsFixture {
            log,
            first,
            first_changes: proposed.changes.clone(),
            first_payload: proposed.payload,
            first_welcome,
            ana: ana_mls.to_bytes().to_vec(),
            cid: cid_mls.to_bytes().to_vec(),
            bea_package,
            bea_update,
            cid_package,
            cid_welcome,
        }
    })
}

/// What groups on MLS read from outside: a log entry, read and put in a log whose chain
/// is then worked out (first byte 0); a KeyPackage (1); this device's state, as its file
/// holds it (2); a Commit of Bea's, followed by Ana (3); a Welcome for Cid (4).
pub fn mls(data: &[u8]) {
    let Some((&kind, rest)) = data.split_first() else {
        return;
    };
    let fixture = mls_fixture();
    match kind % 5 {
        0 => {
            if let Ok(entry) = Entry::decode(rest) {
                assert_eq!(Entry::decode(&entry.encode()).as_ref(), Ok(&entry));
                let mut log = fixture.log.clone();
                if log.insert(entry).is_ok() {
                    let _ = log.mls_chain(|_| Verdict::Unknown);
                }
            }
        }
        1 => {
            let _ = fogata_mls::read_key_package(rest, &device(1).public_key());
        }
        2 => {
            if let Ok(member) = GroupMember::from_bytes(&device(0), rest) {
                let _ = member.to_bytes();
            }
        }
        3 => {
            let mut ana = GroupMember::from_bytes(&device(0), &fixture.ana).expect("reads");
            let none = Changes::default();
            let links = [
                ChainLink {
                    entry: fixture.first,
                    author: device(0).public_key(),
                    digest: payload_digest(&fixture.first_payload),
                    changes: Some(&fixture.first_changes),
                    payload: Some(&fixture.first_payload),
                    welcome: Some(&fixture.first_welcome),
                },
                ChainLink {
                    entry: [7; 32],
                    author: device(1).public_key(),
                    digest: payload_digest(rest),
                    changes: Some(&none),
                    payload: Some(rest),
                    welcome: None,
                },
            ];
            let _ = ana.follow(&links, NOW_MS);
        }
        _ => {
            let mut cid = GroupMember::from_bytes(&device(2), &fixture.cid).expect("reads");
            let changes = Changes::new([device(2).public_key()], []);
            let link = ChainLink {
                entry: [8; 32],
                author: device(0).public_key(),
                digest: [9; 32],
                changes: Some(&changes),
                payload: None,
                welcome: Some(rest),
            };
            let _ = cid.follow(&[link], NOW_MS);
        }
    }
}

fn mls_seeds() -> Vec<Vec<u8>> {
    let fixture = mls_fixture();
    let mut entries: Vec<Vec<u8>> = fixture
        .log
        .entries()
        .filter(|e| e.body.is_mls())
        .map(Entry::encode)
        .collect();
    let ana = device(0);
    let group = fixture.log.group_id();
    let ticket = fixture.log.issue_ticket(&ana, NOW_MS).expect("a ticket");
    for body in [
        Body::MlsJoin {
            ticket,
            name: "Cid".into(),
            key_package: fixture.cid_package.clone(),
        },
        Body::Commit {
            link: EpochLink {
                epoch: 1,
                prev: Some(fixture.first),
            },
            changes: Changes::default(),
            payload: payload_digest(&fixture.bea_update),
            welcome: None,
        },
        Body::Resync {
            link: EpochLink {
                epoch: 1,
                prev: Some(fixture.first),
            },
            payload: payload_digest(b"resync"),
        },
    ] {
        entries.push(Entry::sign(&ana, group, fixture.log.heads(), NOW_MS, body).encode());
    }
    let mut seeds: Vec<Vec<u8>> = entries
        .into_iter()
        .map(|e| [&[0u8][..], &e].concat())
        .collect();
    for (kind, bytes) in [
        (1u8, &fixture.bea_package),
        (2, &fixture.ana),
        (3, &fixture.bea_update),
        (4, &fixture.cid_welcome),
        (4, &fixture.first_welcome),
    ] {
        seeds.push([&[kind][..], bytes].concat());
    }
    seeds
}

#[cfg(test)]
mod runs {
    //! Long runs of damaged inputs against each target, ignored by default: `tools/fuzz.sh`
    //! runs them for as long as it is told. `FOGATA_FUZZ_SECS` sets how long each runs (60 s
    //! by default), `FOGATA_FUZZ_SEED` the seed of its damage (else the clock's, printed);
    //! an input that panics or takes too long is saved under `FOGATA_FUZZ_OUT` (the
    //! workspace's `target/fuzz-found` by default).

    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        path::PathBuf,
        time::{Duration, Instant, SystemTime},
    };

    use super::*;

    /// One input taking longer than this is a finding, as a panic is.
    const SLOW: Duration = Duration::from_secs(5);
    /// Inputs grow no larger than this.
    const MAX_LEN: usize = 256 * 1024;
    /// Damaged inputs kept to damage further.
    const MAX_POOL: usize = 4096;

    /// xorshift64*.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }

        fn byte(&mut self) -> u8 {
            self.next() as u8
        }
    }

    /// Damages `bytes` once to a few times: bits flipped, bytes and lengths changed, parts
    /// cut, copied, moved in from another input of `pool`, or plain noise.
    fn damage(rng: &mut Rng, bytes: &mut Vec<u8>, pool: &[Vec<u8>]) {
        for _ in 0..1 + rng.below(4) {
            let at = rng.below(bytes.len() + 1);
            match rng.below(11) {
                0 if at < bytes.len() => bytes[at] ^= 1 << rng.below(8),
                1 if at < bytes.len() => bytes[at] = rng.byte(),
                2 if at < bytes.len() => {
                    bytes[at] = [0, 1, 0x7f, 0x80, 0xff, 0x9f, 0xbf, 0x40, 0x20][rng.below(9)];
                }
                3 => bytes.truncate(at),
                4 => {
                    let extra: Vec<u8> = (0..rng.below(64)).map(|_| rng.byte()).collect();
                    bytes.splice(at..at, extra);
                }
                5 if at + 4 <= bytes.len() => {
                    let big = [u32::MAX, 0x8000_0000, 0x0100_0000, 0, 0xffff, 0x7fff_ffff];
                    bytes[at..at + 4].copy_from_slice(&big[rng.below(big.len())].to_be_bytes());
                }
                6 => {
                    let end = (at + rng.below(64)).min(bytes.len());
                    bytes.drain(at..end);
                }
                7 => {
                    let end = (at + 1 + rng.below(256)).min(bytes.len());
                    let copy = bytes[at.min(end)..end].to_vec();
                    let to = rng.below(bytes.len() + 1);
                    bytes.splice(to..to, copy);
                }
                8 if !pool.is_empty() => {
                    let other = &pool[rng.below(pool.len())];
                    let from = rng.below(other.len() + 1);
                    let end = (from + rng.below(512)).min(other.len());
                    let cut = (at + rng.below(512)).min(bytes.len());
                    bytes.splice(at..cut, other[from..end].iter().copied());
                }
                9 if rng.below(16) == 0 => {
                    *bytes = (0..rng.below(300)).map(|_| rng.byte()).collect();
                }
                _ if at < bytes.len() => {
                    let n = bytes[at];
                    bytes[at] = if rng.below(2) == 0 {
                        n.wrapping_add(1 + rng.byte() % 8)
                    } else {
                        n.wrapping_sub(1 + rng.byte() % 8)
                    };
                }
                _ => {}
            }
        }
        bytes.truncate(MAX_LEN);
    }

    fn found_dir() -> PathBuf {
        std::env::var_os("FOGATA_FUZZ_OUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fuzz-found")
            })
    }

    /// Runs `target` against `count` damaged inputs, or for `time` when it is given.
    fn run(target: &Target, seed: u64, count: u64, time: Option<Duration>) -> u64 {
        let mut rng = Rng(seed | 1);
        let mut pool = (target.seeds)();
        for input in &pool {
            (target.run)(input);
        }
        let deadline = time.map(|t| Instant::now() + t);
        let mut done = 0u64;
        while deadline.map_or(done < count, |d| Instant::now() < d) {
            for _ in 0..64 {
                let mut input = pool[rng.below(pool.len())].clone();
                damage(&mut rng, &mut input, &pool);
                let started = Instant::now();
                let outcome = catch_unwind(AssertUnwindSafe(|| (target.run)(&input)));
                let took = started.elapsed();
                if outcome.is_err() || took > SLOW {
                    let dir = found_dir().join(target.name);
                    let _ = std::fs::create_dir_all(&dir);
                    let path = dir.join(format!("{seed}-{done}"));
                    let _ = std::fs::write(&path, &input);
                    panic!(
                        "{}: input {done} of seed {seed} {} ({took:?}); saved as {}",
                        target.name,
                        if outcome.is_err() {
                            "panicked"
                        } else {
                            "was slow"
                        },
                        path.display()
                    );
                }
                if pool.len() < MAX_POOL && rng.below(16) == 0 {
                    pool.push(input);
                }
                done += 1;
            }
        }
        done
    }

    fn long_run(name: &str) {
        let target = TARGETS.iter().find(|t| t.name == name).expect("a target");
        let secs = std::env::var("FOGATA_FUZZ_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        let seed = std::env::var("FOGATA_FUZZ_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(1, |d| d.as_nanos() as u64)
            });
        eprintln!("{name}: seed {seed}, {secs} s");
        let done = run(target, seed, 0, Some(Duration::from_secs(secs)));
        eprintln!("{name}: {done} inputs, none failed");
    }

    #[test]
    fn every_target_has_a_libfuzzer_target_and_takes_damaged_seeds() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/fuzz_targets");
        for target in TARGETS {
            assert!(
                dir.join(format!("{}.rs", target.name)).is_file(),
                "fuzz/fuzz_targets/{}.rs",
                target.name
            );
            assert!(
                LONG_RUNS.contains(&target.name),
                "no long run of {}",
                target.name
            );
            run(target, 7, 256, None);
        }
    }

    /// Writes the seeds of every target as files, one folder per target, into
    /// `FOGATA_FUZZ_SEEDS`: where libFuzzer starts from.
    #[test]
    #[ignore = "writes files; tools/fuzz.sh runs it"]
    fn write_seeds() {
        let dir = PathBuf::from(std::env::var_os("FOGATA_FUZZ_SEEDS").expect("FOGATA_FUZZ_SEEDS"));
        for target in TARGETS {
            let dir = dir.join(target.name);
            std::fs::create_dir_all(&dir).expect("a folder");
            for (n, seed) in (target.seeds)().into_iter().enumerate() {
                std::fs::write(dir.join(format!("seed-{n}")), seed).expect("written");
            }
        }
    }

    macro_rules! long_runs {
        ($($name:ident),* $(,)?) => {
            /// The targets with a long run below.
            const LONG_RUNS: &[&str] = &[$(stringify!($name)),*];
            $(
                #[test]
                #[ignore = "long; tools/fuzz.sh runs it"]
                fn $name() {
                    long_run(stringify!($name));
                }
            )*
        };
    }

    long_runs!(
        entry,
        log,
        proof,
        member_wire,
        node_request,
        node_join,
        node_friend,
        friend_wire,
        friend_sealed,
        room_event,
        parts,
        turn,
        direct_payload,
        typing,
        sealed_name,
        invite,
        records,
        at_rest,
        mls,
        ogg_read,
        voice_play,
        wsola_step,
        resample,
        recorder,
    );
}
