//! Frozen bytes of every kind of group log entry, of an invite ticket and of the records a
//! manager signs apart from the log (`tests/data/log.txt`): see
//! `fogata-proto/tests/frozen/mod.rs` for what a change of them means and how to write them
//! anew on purpose.
//!
//! Everything is built from fixed keys, ids and times, and signed: ed25519 signs the same
//! bytes alike, so the signatures are frozen too, and each one read back must still verify.
//! The sealed boxes of epoch secrets are random where they are made; here they are fixed
//! bytes of the right lengths, which the encoding does not look into.

#[path = "../../fogata-proto/tests/frozen/mod.rs"]
mod frozen;

use fogata_crypto::{DeviceIdentity, DevicePublicKey, GroupId, Sealed, codec::Writer};
use fogata_state::{
    Body, Changes, Entry, EpochLink, GroupBanner, GroupPicture, NamedInvite, Rotation, Ticket,
};
use frozen::{case, frozen};

/// When everything here was signed.
const TIME_MS: u64 = 1_700_000_000_000;

fn identity(byte: u8) -> DeviceIdentity {
    DeviceIdentity::from_secret_bytes(&[byte; 32])
}

fn key(byte: u8) -> DevicePublicKey {
    identity(byte).public_key()
}

fn group() -> GroupId {
    GroupId::from_bytes(&[1; 16]).unwrap()
}

fn sealed(byte: u8) -> Sealed {
    Sealed {
        ephemeral: *key(byte).as_bytes(),
        nonce: [byte; 24],
        // An epoch secret and its authentication tag.
        ciphertext: vec![byte; 32 + 16],
    }
}

fn rotation(epoch: u8, members: &[u8]) -> Rotation {
    Rotation {
        epoch: [epoch; 16],
        commitment: [epoch.wrapping_add(1); 32],
        sealed: members.iter().map(|m| (key(*m), sealed(*m))).collect(),
    }
}

/// An invite ticket of the admin (`identity(2)`), signed as [`Ticket::issue`] signs one,
/// but with a fixed id.
fn ticket() -> Ticket {
    let issuer = identity(2);
    let mut ticket = Ticket {
        group_id: group(),
        ticket_id: [30; 16],
        issuer: issuer.public_key(),
        expires_ms: TIME_MS + 86_400_000,
        signature: [0; 64],
    };
    let mut w = Writer::default();
    w.raw(b"fogata/v1/ticket")
        .group(&ticket.group_id)
        .raw(&ticket.ticket_id)
        .key(&ticket.issuer)
        .u64(ticket.expires_ms);
    ticket.signature = issuer.sign(&w.into_bytes());
    assert!(
        ticket.signature_valid(),
        "the ticket is not signed as one is"
    );
    ticket
}

/// The name of each kind of entry. A kind added fails to build here: freeze it in
/// [`bodies`] too.
fn kind(body: &Body) -> &'static str {
    match body {
        Body::Create { .. } => "entry-create",
        Body::Rename { .. } => "entry-rename",
        Body::Promote { .. } => "entry-promote",
        Body::Demote { .. } => "entry-demote",
        Body::SetManagers { .. } => "entry-set-managers",
        Body::Add { .. } => "entry-add",
        Body::Join { .. } => "entry-join",
        Body::Welcome { .. } => "entry-welcome",
        Body::Remove { .. } => "entry-remove",
        Body::Leave => "entry-leave",
        Body::Rotate { .. } => "entry-rotate",
        Body::SetIcon { .. } => "entry-set-icon",
        Body::Invite { .. } => "entry-invite",
        Body::SetPicture { .. } => "entry-set-picture",
        Body::SetLimits { .. } => "entry-set-limits",
        Body::SetRoom { .. } => "entry-set-room",
        Body::RemoveRoom { .. } => "entry-remove-room",
        Body::Revoke { .. } => "entry-revoke",
        Body::RoomAccess { .. } => "entry-room-access",
        Body::KeepOut { .. } => "entry-keep-out",
        Body::LetBack { .. } => "entry-let-back",
        Body::MlsKeyPackage { .. } => "entry-mls-key-package",
        Body::MlsJoin { .. } => "entry-mls-join",
        Body::Commit { .. } => "entry-commit",
        Body::Resync { .. } => "entry-resync",
        Body::Level { .. } => "entry-level",
        Body::Unknown { .. } => "entry-unknown",
    }
}

/// One body of each kind, and of each form a kind has, with the device that signs it.
fn bodies() -> Vec<(&'static str, u8, Body)> {
    let png = b"\x89PNG\r\n\x1a\n tiny".to_vec();
    vec![
        (
            "",
            2,
            Body::Create {
                name: "Fogata".into(),
                start: rotation(10, &[2]),
            },
        ),
        (
            "",
            2,
            Body::Rename {
                name: "Fogata \u{1f525}".into(),
            },
        ),
        ("", 2, Body::Promote { member: key(3) }),
        ("", 2, Body::Demote { member: key(3) }),
        ("-everyone", 2, Body::SetManagers { first: None }),
        ("", 2, Body::SetManagers { first: Some(2) }),
        (
            "",
            2,
            Body::Add {
                member: key(3),
                name: "Ana".into(),
                epoch: [10; 16],
                secret: sealed(3),
            },
        ),
        (
            "",
            4,
            Body::Join {
                ticket: ticket(),
                name: "Luis".into(),
            },
        ),
        (
            "",
            2,
            Body::Welcome {
                member: key(4),
                epoch: [10; 16],
                secret: sealed(4),
            },
        ),
        (
            "",
            2,
            Body::Remove {
                member: key(3),
                rotation: rotation(11, &[2, 4]),
            },
        ),
        ("", 4, Body::Leave),
        (
            "",
            2,
            Body::Rotate {
                rotation: rotation(12, &[2, 3, 4]),
            },
        ),
        ("", 2, Body::SetIcon { icon: 7 }),
        (
            "",
            2,
            Body::Invite {
                ticket: ticket(),
                token: [31; 16],
            },
        ),
        ("", 2, Body::SetPicture { picture: Some(png) }),
        ("-none", 2, Body::SetPicture { picture: None }),
        (
            "",
            2,
            Body::SetLimits {
                members: 50,
                room: 12,
            },
        ),
        (
            "",
            2,
            Body::SetRoom {
                id: [5; 8],
                name: "Music".into(),
            },
        ),
        ("", 2, Body::RemoveRoom { id: [5; 8] }),
        (
            "",
            2,
            Body::Revoke {
                ticket_id: [30; 16],
            },
        ),
        (
            "-everyone",
            2,
            Body::RoomAccess {
                room: [5; 8],
                members: None,
            },
        ),
        (
            "",
            2,
            Body::RoomAccess {
                room: [5; 8],
                members: Some(vec![key(3), key(4)]),
            },
        ),
        (
            "-until-let-back",
            2,
            Body::KeepOut {
                room: [5; 8],
                member: key(3),
                until_ms: None,
            },
        ),
        (
            "",
            2,
            Body::KeepOut {
                room: [5; 8],
                member: key(3),
                until_ms: Some(TIME_MS + 600_000),
            },
        ),
        (
            "",
            2,
            Body::LetBack {
                room: [5; 8],
                member: key(3),
            },
        ),
        (
            "",
            3,
            Body::MlsKeyPackage {
                key_package: vec![40; 370],
            },
        ),
        (
            "",
            4,
            Body::MlsJoin {
                ticket: ticket(),
                name: "Luis".into(),
                key_package: vec![41; 370],
            },
        ),
        (
            "-first",
            2,
            Body::Commit {
                link: EpochLink {
                    epoch: 0,
                    prev: None,
                },
                changes: Changes::new([key(3), key(4)], []),
                payload: [42; 32],
                welcome: Some([43; 32]),
            },
        ),
        (
            "",
            2,
            Body::Commit {
                link: EpochLink {
                    epoch: 1,
                    prev: Some([44; 32]),
                },
                changes: Changes::new([], [key(3)]),
                payload: [45; 32],
                welcome: None,
            },
        ),
        (
            "",
            4,
            Body::Resync {
                link: EpochLink {
                    epoch: 2,
                    prev: Some([46; 32]),
                },
                payload: [47; 32],
            },
        ),
        ("", 2, Body::Level { level: 2 }),
        // The raise to level four, in the encoding every level reads.
        ("-four", 2, Body::Level { level: 4 }),
        // An invite that names who lets its device in (level three): a kind level two does
        // not know, so read as one.
        (
            "-named-invite",
            2,
            NamedInvite {
                ticket: ticket(),
                token: [31; 16],
                welcomers: vec![key(3)],
            }
            .body(),
        ),
        // A kind from a newer version, and an MLS kind in an encoding this version does
        // not write: both kept whole, as they came.
        (
            "",
            2,
            Body::Unknown {
                kind: 200,
                bytes: vec![48; 20],
            },
        ),
        (
            "-damaged-key-package",
            3,
            Body::Unknown {
                kind: 20,
                bytes: vec![0, 0, 0],
            },
        ),
    ]
}

#[test]
fn every_entry_and_record_is_written_and_read_as_frozen() {
    let mut cases = Vec::new();
    for (form, author, body) in bodies() {
        let name = format!("{}{form}", kind(&body));
        let parents = if matches!(body, Body::Create { .. }) {
            Vec::new()
        } else {
            vec![[50; 32], [51; 32]]
        };
        let entry = Entry::sign(&identity(author), group(), parents, TIME_MS, body);
        cases.push(case(name, &entry, Entry::encode, |b| {
            Entry::decode(b).ok().filter(Entry::signature_valid)
        }));
    }
    cases.push(case("ticket".into(), &ticket(), Ticket::encode, |b| {
        Ticket::decode(b).ok().filter(Ticket::signature_valid)
    }));
    let admin = identity(2);
    let gif = b"GIF89a tiny".to_vec();
    for (name, picture) in [("banner", gif.clone()), ("banner-none", Vec::new())] {
        let banner = GroupBanner::sign(&admin, group(), picture, TIME_MS).unwrap();
        // Decoding checks the signature.
        cases.push(case(name.into(), &banner, GroupBanner::encode, |b| {
            GroupBanner::decode(b).ok()
        }));
    }
    for (name, picture) in [("group-picture", gif), ("group-picture-none", Vec::new())] {
        let picture = GroupPicture::sign(&admin, group(), picture, TIME_MS).unwrap();
        cases.push(case(name.into(), &picture, GroupPicture::encode, |b| {
            GroupPicture::decode(b).ok()
        }));
    }
    frozen("log.txt", &cases);
}
