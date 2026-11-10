//! Frozen bytes of every kind of plaintext sealed in a group or direct message
//! (`tests/data/payloads.txt`): see `frozen/mod.rs` for what a change of them means and how
//! to write them anew on purpose. The wire messages that carry them are frozen in
//! `fogata-net`.
//!
//! A payload that leaves out what an older version did not send (the first voice room, no
//! recent events, no video) is frozen in both forms.

mod frozen;

use fogata_proto::{DirectFile, DirectFileKind, DirectPayload, GroupPayload, WireVideo, WireVoice};
use frozen::{case, frozen};

/// The name of each kind of group payload. A kind added fails to build here: freeze it in
/// [`group_payloads`] too.
fn group_kind(payload: &GroupPayload) -> &'static str {
    match payload {
        GroupPayload::RoomJoin { .. } => "group-room-join",
        GroupPayload::RoomHere { .. } => "group-room-here",
        GroupPayload::RoomLeave { .. } => "group-room-leave",
        GroupPayload::RoomChat { .. } => "group-room-chat",
        GroupPayload::RoomUnshare { .. } => "group-room-unshare",
        GroupPayload::RoomShare { .. } => "group-room-share",
        GroupPayload::RoomRemove { .. } => "group-room-remove",
    }
}

fn group_payloads() -> Vec<(String, GroupPayload)> {
    const FIRST: [u8; 8] = [0; 8];
    const OTHER: [u8; 8] = [5; 8];
    let video = || WireVideo {
        thumb: b"\xff\xd8\xff\xe0 small".to_vec(),
        width: 1280,
        height: 720,
        duration_ms: 72_000,
    };
    [
        (
            "-first-room",
            GroupPayload::RoomJoin {
                voice: FIRST,
                room: [1; 16],
                started_ms: 1_700_000_000_000,
                nick: "Ana".into(),
            },
        ),
        (
            "",
            GroupPayload::RoomJoin {
                voice: OTHER,
                room: [1; 16],
                started_ms: 1_700_000_000_000,
                nick: "Ana".into(),
            },
        ),
        (
            "-older",
            GroupPayload::RoomHere {
                voice: FIRST,
                room: [1; 16],
                started_ms: 1_700_000_000_000,
                nick: "Ana".into(),
                recent: Vec::new(),
            },
        ),
        (
            "",
            GroupPayload::RoomHere {
                voice: OTHER,
                room: [1; 16],
                started_ms: 1_700_000_000_000,
                nick: "Ana".into(),
                recent: [[7u8; 16], [8; 16]].concat(),
            },
        ),
        (
            "-first-room",
            GroupPayload::RoomLeave {
                voice: FIRST,
                room: [1; 16],
            },
        ),
        (
            "",
            GroupPayload::RoomLeave {
                voice: OTHER,
                room: [1; 16],
            },
        ),
        (
            "",
            GroupPayload::RoomChat {
                voice: OTHER,
                room: [1; 16],
                nick: "Ana".into(),
                text: "hola, ¿qué tal? \u{1f525}".into(),
            },
        ),
        (
            "",
            GroupPayload::RoomUnshare {
                voice: OTHER,
                room: [1; 16],
                id: [2; 32],
            },
        ),
        (
            "-older",
            GroupPayload::RoomShare {
                voice: FIRST,
                room: [1; 16],
                id: [2; 32],
                name: "notes.pdf".into(),
                size: 120_000,
                video: None,
            },
        ),
        (
            "",
            GroupPayload::RoomShare {
                voice: OTHER,
                room: [1; 16],
                id: [2; 32],
                name: "weekend.mp4".into(),
                size: 48_000_000,
                video: Some(video()),
            },
        ),
        (
            "",
            GroupPayload::RoomRemove {
                voice: OTHER,
                room: [1; 16],
                member: [3; 32],
            },
        ),
    ]
    .into_iter()
    .map(|(form, payload)| (format!("{}{form}", group_kind(&payload)), payload))
    .collect()
}

/// The name of each kind of direct payload, as [`group_kind`].
fn direct_kind(payload: &DirectPayload) -> &'static str {
    match payload {
        DirectPayload::Message { .. } => "direct-message",
        DirectPayload::Timer { .. } => "direct-timer",
        DirectPayload::Receipt { .. } => "direct-receipt",
        DirectPayload::Pin { .. } => "direct-pin",
        DirectPayload::Unsend { .. } => "direct-unsend",
    }
}

fn direct_payloads() -> Vec<(String, DirectPayload)> {
    [
        (
            "-older",
            DirectPayload::Message {
                id: [1; 16],
                sent_ms: 1_700_000_000_000,
                text: "hi".into(),
                files: Vec::new(),
                ttl_ms: 0,
            },
        ),
        (
            "",
            DirectPayload::Message {
                id: [1; 16],
                sent_ms: 1_700_000_000_000,
                text: "look".into(),
                files: vec![
                    DirectFile {
                        id: [2; 32],
                        name: "weekend.mp4".into(),
                        size: 48_000_000,
                        kind: DirectFileKind::Video,
                        video: Some(WireVideo {
                            thumb: b"\xff\xd8\xff\xe0 small".to_vec(),
                            width: 1280,
                            height: 720,
                            duration_ms: 72_000,
                        }),
                        voice: None,
                    },
                    DirectFile {
                        id: [3; 32],
                        name: "cat.png".into(),
                        size: 30_000,
                        kind: DirectFileKind::Picture,
                        video: None,
                        voice: None,
                    },
                    DirectFile {
                        id: [4; 32],
                        name: "Voice message.ogg".into(),
                        size: 60_000,
                        kind: DirectFileKind::File,
                        video: None,
                        voice: Some(WireVoice {
                            duration_ms: 18_000,
                            bars: (0..40).collect(),
                        }),
                    },
                ],
                ttl_ms: 86_400_000,
            },
        ),
        (
            "",
            DirectPayload::Timer {
                id: [5; 16],
                sent_ms: 1_700_000_000_001,
                ms: 3_600_000,
            },
        ),
        (
            "",
            DirectPayload::Receipt {
                ids: vec![[6; 16], [7; 16]],
            },
        ),
        (
            "",
            DirectPayload::Pin {
                id: [8; 16],
                sent_ms: 1_700_000_000_002,
                target: [1; 16],
                pinned: true,
            },
        ),
        (
            "",
            DirectPayload::Unsend {
                id: [9; 16],
                sent_ms: 1_700_000_000_003,
                target: [1; 16],
            },
        ),
    ]
    .into_iter()
    .map(|(form, payload)| (format!("{}{form}", direct_kind(&payload)), payload))
    .collect()
}

#[test]
fn every_payload_is_written_and_read_as_frozen() {
    let mut cases = Vec::new();
    for (name, payload) in group_payloads() {
        cases.push(case(name, &payload, GroupPayload::encode, |b| {
            GroupPayload::decode(b).ok()
        }));
    }
    for (name, payload) in direct_payloads() {
        cases.push(case(name, &payload, DirectPayload::encode, |b| {
            DirectPayload::decode(b).ok()
        }));
    }
    frozen("payloads.txt", &cases);
}
