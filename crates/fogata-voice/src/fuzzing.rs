//! Entry points for fuzzing what reads and plays a friend's voice message, and what turns a
//! microphone's sound into one (see fogata-core's `fuzzing`). Each takes any bytes and must
//! neither panic nor take long; besides that, each checks what must hold: a file read writes
//! and reads back the same, a stretch made from what each step says it needs is the stretch
//! made from all of the sound, and what the recorder writes, the reader takes.

use crate::{
    BARS, MAX_FILE_BYTES, MAX_READ_MS, Player, RATE, Recorder,
    ogg::{self, MAX_PACKET},
    resample::Linear,
    wsola::{HOP, Wsola},
};

/// Messages longer than this play only in part: the whole of 15 minutes would make a run
/// slow, and a long message plays as a short one does, a stretch at a time.
const PLAYED_WHOLE_MS: u64 = 30_000;

/// The speeds a run plays at, the first byte of the input choosing them: those the
/// application offers, and odd ones a bug could pass.
const SPEEDS: [f64; 8] = [1.0, 1.5, 2.0, 0.25, 4.0, 0.0, f64::NAN, 1e9];

/// Fixes the checksum of each page whose layout holds, so damaged pages get past it and
/// what they carry is read.
fn reseal(file: &mut [u8]) {
    let mut at = 0;
    while at + 27 <= file.len() {
        if &file[at..at + 4] != b"OggS" {
            break;
        }
        let segments = usize::from(file[at + 26]);
        let Some(table) = file.get(at + 27..at + 27 + segments) else {
            break;
        };
        let end = at + 27 + segments + table.iter().map(|&l| usize::from(l)).sum::<usize>();
        if end > file.len() {
            break;
        }
        file[at + 22..at + 26].fill(0);
        let sum = ogg::crc(&file[at..end]);
        file[at + 22..at + 26].copy_from_slice(&sum.to_le_bytes());
        at = end;
    }
}

/// A file read as a friend's message is, as it came and with its checksums fixed: one that
/// reads keeps to the caps, and written again reads back the same.
pub fn ogg_read(data: &[u8]) {
    let _ = ogg::packet_samples(data);
    let mut fixed = data.to_vec();
    reseal(&mut fixed);
    for bytes in [data, &fixed] {
        let Ok(read) = ogg::read(bytes) else {
            continue;
        };
        assert!(read.duration_ms() <= MAX_READ_MS);
        assert!(read.samples > 0);
        let packets: Vec<Vec<u8>> = (0..read.packets())
            .map(|i| read.packet(i).to_vec())
            .collect();
        for p in &packets {
            assert!(p.len() <= MAX_PACKET);
            assert!(ogg::packet_samples(p).is_ok());
        }
        if packets.is_empty() {
            continue;
        }
        let again = ogg::write(&packets, read.pre_skip, read.samples, 7);
        let back = ogg::read(&again).expect("a file read writes a file that reads");
        assert_eq!(
            (back.packets(), back.pre_skip, back.samples),
            (read.packets(), read.pre_skip, read.samples)
        );
        for (i, p) in packets.iter().enumerate() {
            assert_eq!(back.packet(i), p.as_slice());
        }
    }
}

pub fn ogg_seeds() -> Vec<Vec<u8>> {
    let tone = |hertz: f32, seconds: f32, amp: f32| -> Vec<f32> {
        (0..(seconds * RATE as f32) as usize)
            .map(|i| amp * (2.0 * std::f32::consts::PI * hertz * i as f32 / RATE as f32).sin())
            .collect()
    };
    let record = |pcm: &[f32]| -> Vec<u8> {
        let mut r = Recorder::new().expect("an encoder");
        r.push(pcm).expect("encoded");
        r.finish(0x0f0a_9a7a)
            .expect("finished")
            .expect("a message")
            .file
    };
    vec![
        record(&tone(220.0, 0.6, 0.4)),
        record(&tone(440.0, 1.3, 0.2)),
        record(&vec![0.0; RATE as usize / 2]),
    ]
}

/// A message played as the application plays it: the first byte chooses the speeds it plays
/// at, the rest is the file. It ends within the steps its length allows at the slowest
/// speed, never hands out more than asked, and only sound that is a number.
pub fn voice_play(data: &[u8]) {
    let Some((&choice, file)) = data.split_first() else {
        return;
    };
    let mut fixed = file.to_vec();
    reseal(&mut fixed);
    let Ok(mut player) = Player::new(&fixed) else {
        return;
    };
    let speeds = [
        SPEEDS[usize::from(choice & 7)],
        SPEEDS[usize::from((choice >> 3) & 7)],
    ];
    let whole = player.duration_ms() <= PLAYED_WHOLE_MS;
    let mut out = vec![0.0f32; 441];
    for (round, &speed) in speeds.iter().enumerate() {
        if round > 0 {
            player.rewind();
        }
        player.set_speed(speed);
        // At 0.25× a message lasts four times as long; a step hands out at least a sample
        // until the end, so this many always reach it.
        let cap = if whole {
            player.samples() * 4 / out.len() as u64 + 64
        } else {
            u64::from(RATE) * 2 / out.len() as u64
        };
        let mut steps = 0;
        let mut handed = 0u64;
        loop {
            let n = player.fill(&mut out);
            assert!(n <= out.len());
            assert!(out[..n].iter().all(|s| s.is_finite()));
            handed += n as u64;
            if n < out.len() {
                assert!(player.ended());
                assert_eq!(player.position(), player.samples());
                break;
            }
            steps += 1;
            if steps >= cap {
                assert!(!whole, "still playing after {steps} steps");
                break;
            }
        }
        assert!(handed <= player.samples() * 4 + 2 * HOP as u64);
        let _ = (player.position_ms(), player.duration_ms());
    }
}

pub fn voice_play_seeds() -> Vec<Vec<u8>> {
    ogg_seeds()
        .into_iter()
        .zip([0x08u8, 0x11, 0x3a])
        .map(|(file, choice)| [&[choice][..], &file].concat())
        .collect()
}

/// Sound as floats from `bytes`, four bytes a sample; what is not a number or out of range
/// is kept, as a broken device could send it.
fn samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&c| f32::from_le_bytes(c))
        .collect()
}

/// WSOLA steps: the speed (8 bytes) and the place it starts (4 bytes), then the sound. Each
/// step hands out [`HOP`] samples, and stretching from only what [`Wsola::needs`] says gives
/// what stretching from all of the sound does.
pub fn wsola_step(data: &[u8]) {
    let Some((head, rest)) = data.split_at_checked(12) else {
        return;
    };
    let speed = f64::from_le_bytes(head[..8].try_into().expect("8 bytes"));
    let start = u64::from(u32::from_le_bytes(head[8..].try_into().expect("4 bytes")));
    let input = samples(rest);
    let finite = input.iter().all(|s| s.is_finite());
    let end = start + input.len() as u64;
    let mut whole = Wsola::new(speed, start);
    let mut windowed = Wsola::new(speed, start);
    let (mut a, mut b) = (Vec::new(), Vec::new());
    // A step goes at least a quarter of a hop: this many reach the end of the sound.
    for _ in 0..input.len() / (HOP / 4) + 4 {
        if whole.next_start() >= end {
            break;
        }
        let before = a.len();
        // All of the sound, where it is.
        whole.step(&input, start, &mut a);
        assert_eq!(a.len() - before, HOP);
        let (from, to) = windowed.needs();
        assert!(from <= to);
        let lo = from.clamp(start, end);
        let hi = to.clamp(lo, end);
        windowed.step(
            &input[(lo - start) as usize..(hi - start) as usize],
            lo,
            &mut b,
        );
        assert!(whole.position() <= whole.next_start() + crate::wsola::SEEK as u64);
    }
    if finite {
        assert!(a.iter().all(|s| s.is_finite()));
    }
    assert_eq!(a.len(), b.len());
    assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
}

pub fn wsola_step_seeds() -> Vec<Vec<u8>> {
    let voice: Vec<u8> = (0..4800)
        .flat_map(|i| {
            let t = i as f32 / RATE as f32;
            let s: f32 = (1..=4)
                .map(|k| (2.0 * std::f32::consts::PI * 140.0 * k as f32 * t).sin() / k as f32)
                .sum();
            (0.2 * s).to_le_bytes()
        })
        .collect();
    [1.5f64, 2.0, 1.0, 0.25]
        .iter()
        .zip([0u32, 0, 960, 7])
        .map(|(speed, start)| [&speed.to_le_bytes()[..], &start.to_le_bytes(), &voice].concat())
        .collect()
}

/// Rate changes: the two rates (4 bytes each), the size of the pieces the sound comes in (2
/// bytes), then the sound. In pieces or whole, it makes as many samples, give or take one,
/// and never more than its rates allow.
pub fn resample(data: &[u8]) {
    let Some((head, rest)) = data.split_at_checked(10) else {
        return;
    };
    let from = u32::from_le_bytes(head[..4].try_into().expect("4 bytes"));
    let to = u32::from_le_bytes(head[4..8].try_into().expect("4 bytes"));
    let piece = usize::from(u16::from_le_bytes([head[8], head[9]])).max(1);
    let input = samples(rest);
    let mut whole = Vec::new();
    let mut r = Linear::new(from, to);
    r.push(&input, &mut whole);
    r.push(&[], &mut whole);
    let mut pieces = Vec::new();
    let mut r = Linear::new(from, to);
    for chunk in input.chunks(piece) {
        r.push(chunk, &mut pieces);
    }
    assert!(whole.len().abs_diff(pieces.len()) <= 1);
    // The steepest change of rate: 8 kHz to 384 kHz.
    assert!(whole.len() <= input.len() * 48 + 1);
    // Between two samples near the largest a float holds, the line between them can
    // overflow; sound is far below that.
    if input.iter().all(|s| s.abs() <= 1e38) {
        assert!(whole.iter().all(|s| s.is_finite()));
    }
}

pub fn resample_seeds() -> Vec<Vec<u8>> {
    let sound: Vec<u8> = (0..2000)
        .flat_map(|i| ((i as f32 * 0.03).sin() * 0.5).to_le_bytes())
        .collect();
    [
        (44_100u32, 48_000u32, 441u16),
        (48_000, 44_100, 480),
        (16_000, 48_000, 1),
        (8, 0, 7),
    ]
    .iter()
    .map(|(from, to, piece)| {
        [
            &from.to_le_bytes()[..],
            &to.to_le_bytes(),
            &piece.to_le_bytes(),
            &sound,
        ]
        .concat()
    })
    .collect()
}

/// A recording: the size of the pieces the sound comes in (2 bytes), then the sound, which
/// may be loud, not a number or endless. What the recorder writes, the reader takes, with
/// the length recorded.
pub fn recorder(data: &[u8]) {
    let Some((head, rest)) = data.split_at_checked(2) else {
        return;
    };
    let piece = usize::from(u16::from_le_bytes([head[0], head[1]])).max(1);
    let input = samples(rest);
    let mut r = Recorder::new().expect("an encoder");
    let mut taken = 0;
    for chunk in input.chunks(piece) {
        taken += r.push(chunk).expect("encoded");
    }
    assert_eq!(taken, input.len());
    assert_eq!(r.duration_ms(), input.len() as u64 * 1000 / u64::from(RATE));
    let Some(rec) = r.finish(1).expect("finished") else {
        assert!(input.is_empty());
        return;
    };
    assert_eq!(rec.bars.len(), BARS);
    assert!(rec.file.len() <= MAX_FILE_BYTES);
    let read = ogg::read(&rec.file).expect("what the recorder writes, the reader takes");
    assert_eq!(read.samples, input.len() as u64);
}

pub fn recorder_seeds() -> Vec<Vec<u8>> {
    let sound = |amp: f32| -> Vec<u8> {
        (0..2400)
            .flat_map(|i| ((i as f32 * 0.05).sin() * amp).to_le_bytes())
            .collect()
    };
    vec![
        [&960u16.to_le_bytes()[..], &sound(0.3)].concat(),
        [&441u16.to_le_bytes()[..], &sound(3.0)].concat(),
        [
            &1u16.to_le_bytes()[..],
            &f32::NAN.to_le_bytes(),
            &sound(0.01),
        ]
        .concat(),
    ]
}
