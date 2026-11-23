//! Measurements of what ten groups' rooms make a device hold, run by hand (`--ignored`).

use super::tests::{record, spill_dir};
use super::*;
use fogata_crypto::DeviceIdentity;
use fogata_state::{RoomEvent, RoomTracker};
use std::sync::Arc;

/// A room as a device keeps it, with its latest events.
type FloodedRoom = (RoomTracker<Arc<WireMessage>>, Vec<Arc<WireMessage>>);

/// The ten rooms of a group whose every room has eight devices of a member that floods
/// it: each device shares as many videos with the biggest picture as the room takes from
/// it, and one says 500 of the heaviest lines; numbered on from `n`. With `lean`, kept as
/// the node keeps them now: each record once, shared with the room's latest events, and
/// a share's picture in its record only; else as before, the latest events copied and
/// each picture kept read as well. Each room's latest events go with it.
fn flooded_group(n: &mut u32, lean: bool) -> Vec<FloodedRoom> {
    use fogata_state::{
        EVENT_SLACK_BYTES, MAX_CHAT_CHARS, MAX_IN_ROOM, MAX_SHARES_EACH, MAX_THUMB_BYTES,
    };
    let key = || DeviceIdentity::generate().unwrap().public_key();
    let devices: Vec<DevicePublicKey> = (0..MAX_IN_ROOM).map(|_| key()).collect();
    let me = key();
    let heavy = "\u{1}".repeat(MAX_CHAT_CHARS);
    let thumb = vec![9; MAX_THUMB_BYTES];
    (0..10u8)
        .map(|v| {
            let mut t = RoomTracker::new(me)
                .weighing(|record: &Arc<WireMessage>| crate::group::room_record_weight(record))
                .keeping_pictures(!lean);
            t.set_room_for(MAX_IN_ROOM, 10);
            let mut recent: VecDeque<RecentEvent> = VecDeque::new();
            let mut send = |t: &mut RoomTracker<Arc<WireMessage>>, sender, input, weight| {
                *n += 1;
                let mut message_id = [0u8; 16];
                message_id[..4].copy_from_slice(&n.to_be_bytes());
                let mut wire = record(0, 0);
                wire.message_id = message_id.to_vec();
                wire.ciphertext = vec![5; weight - crate::group::room_record_weight(&wire)];
                let wire = Arc::new(wire);
                // As apply_room keeps it among the latest: the same message, or before a
                // copy of it.
                let kept = if lean {
                    wire.clone()
                } else {
                    Arc::new((*wire).clone())
                };
                keep_recent(&mut recent, [v; 16], message_id, &kept);
                let event = RoomEvent {
                    room: [v; 16],
                    sender,
                    timestamp_ms: 1_000 + u64::from(*n),
                    message_id,
                    input,
                };
                t.apply(event, wire, 1_000);
            };
            for (d, device) in devices.iter().enumerate() {
                let join = RoomInput::Join {
                    started_ms: 1_000,
                    nick: "m".into(),
                };
                send(&mut t, *device, join, 300);
                for i in 0..MAX_SHARES_EACH as u32 {
                    let mut id = [0u8; 32];
                    id[..4].copy_from_slice(&(d as u32 * 1_000 + i).to_be_bytes());
                    let file = FileInfo {
                        id,
                        name: format!("f{i:03}"),
                        size: 1,
                        video: Some(VideoPreview {
                            thumb: thumb.clone(),
                            width: 2,
                            height: 2,
                            duration_ms: 1,
                        }),
                    };
                    let weight = MAX_THUMB_BYTES.div_ceil(3) * 4 + 4 + EVENT_SLACK_BYTES;
                    send(&mut t, *device, RoomInput::Share(file), weight);
                }
            }
            for _ in 0..500 {
                let chat = RoomInput::Chat {
                    nick: "m".into(),
                    text: heavy.clone(),
                };
                let weight = 6 * MAX_CHAT_CHARS + 1 + EVENT_SLACK_BYTES;
                send(&mut t, devices[0], chat, weight);
            }
            let recent = recent.into_iter().filter_map(|e| e.wire).collect();
            (t, recent)
        })
        .collect()
}

/// This process's resident memory, in bytes.
fn resident() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").unwrap();
        let pages: u64 = statm.split_whitespace().nth(1).unwrap().parse().unwrap();
        pages * 4096
    }
    #[cfg(not(target_os = "linux"))]
    {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        let kib: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
        kib * 1024
    }
}

/// What the rooms of ten groups, each flooded as [`flooded_group`] does, make a device
/// keep: with every group's own bounds only (`bounded` false), or also within the bound
/// on the chat of all its rooms ([`fit_chat`], every slot of the node, here after each
/// group came); kept `lean` as the node does now, or as before. Linux and macOS, one at
/// a time, in a release build:
/// `cargo test --release -p fogata-net --lib device_room_memory -- --ignored --nocapture`.
fn measure_device_room_memory(bounded: bool, lean: bool) {
    let before = resident();
    let mut n = 0;
    let mut rooms = Vec::new();
    for _ in 0..10 {
        rooms.extend(flooded_group(&mut n, lean));
        if bounded {
            fit_chat(rooms.iter_mut().map(|(t, _)| t), ROOM_CHAT_BYTES);
        }
    }
    let after = resident();
    let chat: usize = rooms
        .iter()
        .flat_map(|(t, _)| t.chat_weights())
        .map(|l| l.3)
        .sum();
    let shares: usize = rooms.iter().map(|(t, _)| t.share_bytes()).sum();
    let records: usize = rooms.iter().map(|(t, _)| t.snapshot().len()).sum();
    let pictures: usize = rooms
        .iter()
        .filter_map(|(t, _)| t.room())
        .flat_map(|r| r.files.values())
        .filter_map(|f| f.info.video.as_ref())
        .map(|v| v.thumb.len())
        .sum();
    println!(
        "10 groups{}{}: {records} records kept; chat {} MiB, shares {} MiB as counted, \
         pictures read {} MiB; {} MiB resident",
        if bounded { ", chat bounded" } else { "" },
        if lean { ", kept once" } else { "" },
        chat >> 20,
        shares >> 20,
        pictures >> 20,
        after.saturating_sub(before) >> 20
    );
}

#[test]
#[ignore]
fn measure_device_room_memory_lean() {
    measure_device_room_memory(true, true);
}

#[test]
#[ignore]
fn measure_device_room_memory_bounded() {
    measure_device_room_memory(true, false);
}

#[test]
#[ignore]
fn measure_device_room_memory_per_group_only() {
    measure_device_room_memory(false, false);
}

/// What ten groups of one room each make a device hold of their shares, in each of which
/// a member that floods it has eight devices sharing as many videos with the biggest
/// picture as the room takes from each (47): every share in memory (`on_disk` false),
/// or past [`ROOM_SHARE_BYTES`] on disk ([`fit_shares`], every slot of the node, here
/// after each group came). Kept as the node keeps them, with each room's latest events.
/// Also how long the slowest part of a room's answer takes to read back. Linux and
/// macOS, one at a time, in a release build:
/// `cargo test --release -p fogata-net --lib device_share_memory -- --ignored --nocapture`.
fn measure_device_share_memory(on_disk: bool) {
    use fogata_state::{EVENT_SLACK_BYTES, MAX_IN_ROOM, MAX_SHARES_EACH, MAX_THUMB_BYTES};
    let key = || DeviceIdentity::generate().unwrap().public_key();
    let dir = spill_dir("measure");
    let mut spill = on_disk.then(|| Spill::open(dir.clone()).unwrap());
    let me = key();
    let thumb = vec![9; MAX_THUMB_BYTES];
    let mut n = 0u32;
    let before = resident();
    let mut rooms: Vec<(RoomTracker<Kept>, Vec<Arc<WireMessage>>)> = Vec::new();
    for g in 0..10u8 {
        let mut t = RoomTracker::new(me)
            .weighing(Kept::weight)
            .keeping_pictures(false);
        t.set_room_for(MAX_IN_ROOM, 1);
        let mut recent: VecDeque<RecentEvent> = VecDeque::new();
        let mut send = |t: &mut RoomTracker<Kept>, sender, input, weight| {
            n += 1;
            let mut message_id = [0u8; 16];
            message_id[..4].copy_from_slice(&n.to_be_bytes());
            let mut wire = record(0, 0);
            wire.message_id = message_id.to_vec();
            wire.ciphertext = vec![5; weight - crate::group::room_record_weight(&wire)];
            let wire = Arc::new(wire);
            keep_recent(&mut recent, [g; 16], message_id, &wire);
            let event = RoomEvent {
                room: [g; 16],
                sender,
                timestamp_ms: 1_000 + u64::from(n),
                message_id,
                input,
            };
            t.apply(event, Kept::Inline(wire), 1_000);
        };
        for d in 0..u32::from(MAX_IN_ROOM) {
            let device = key();
            let join = RoomInput::Join {
                started_ms: 1_000,
                nick: "m".into(),
            };
            send(&mut t, device, join, 300);
            for i in 0..MAX_SHARES_EACH as u32 {
                let mut id = [0u8; 32];
                id[..4].copy_from_slice(&(d * 1_000 + i).to_be_bytes());
                let file = FileInfo {
                    id,
                    name: format!("f{i:03}"),
                    size: 1,
                    video: Some(VideoPreview {
                        thumb: thumb.clone(),
                        width: 2,
                        height: 2,
                        duration_ms: 1,
                    }),
                };
                let weight = MAX_THUMB_BYTES.div_ceil(3) * 4 + 4 + EVENT_SLACK_BYTES;
                send(&mut t, device, RoomInput::Share(file), weight);
            }
        }
        let recent = recent.into_iter().filter_map(|e| e.wire).collect();
        rooms.push((t, recent));
        if let Some(spill) = spill.as_mut() {
            fit_shares(rooms.iter_mut().map(|(t, _)| t), spill, ROOM_SHARE_BYTES);
        }
    }
    let after = resident();
    let listed: usize = rooms.iter().map(|(t, _)| t.share_records().count()).sum();
    let counted: usize = rooms.iter().map(|(t, _)| t.share_bytes()).sum();
    let (mut held, mut moved) = (0, 0);
    for (kept, weight) in rooms
        .iter()
        .flat_map(|(t, _)| t.share_records())
        .map(|(.., k)| (k, k.weight()))
    {
        match kept {
            Kept::Inline(_) => held += weight,
            Kept::Spilled { .. } => moved += weight,
        }
    }
    let file = spill.as_ref().map_or(0, |s| s.bytes().0);
    // Each part of the first room's answer, read back, the slowest.
    let mut slowest = (Duration::ZERO, 0, 0);
    let mut from = Some(0);
    while let Some(at) = from {
        let started = Instant::now();
        let (part, more) = rooms_part(std::iter::once(&rooms[0].0), spill.as_ref(), at).unwrap();
        let took = started.elapsed();
        if took > slowest.0 {
            slowest = (took, part.len(), part.iter().map(wire_size).sum::<usize>());
        }
        from = more;
    }
    println!(
        "10 one-room groups{}: {listed} shares, {} MiB as counted; messages {} MiB in \
         memory, {} MiB on disk (file {} MiB); {} MiB resident; slowest part of an \
         answer ({} shares, {} KiB) read in {:?}",
        if on_disk {
            ", on disk past the bound"
        } else {
            ""
        },
        counted >> 20,
        held >> 20,
        moved >> 20,
        file >> 20,
        after.saturating_sub(before) >> 20,
        slowest.1,
        slowest.2 >> 10,
        slowest.0,
    );
    drop(spill);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore]
fn measure_device_share_memory_in_memory() {
    measure_device_share_memory(false);
}

#[test]
#[ignore]
fn measure_device_share_memory_on_disk() {
    measure_device_share_memory(true);
}
