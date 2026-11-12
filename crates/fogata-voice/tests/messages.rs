//! A voice message as the application makes and plays one, through this crate's public
//! calls only: the microphone's sound at its device's rate brought to 48 kHz, recorded,
//! written as a file, read back as a friend's copy reads it, played, and brought to the
//! speaker's rate. And what a friend's copy must take of anything recorded here: the
//! longest, loudest message stays within what the reader allows.

use fogata_voice::{BARS, FRAME, Linear, MAX_FILE_BYTES, MAX_MS, Player, RATE, Recorder, ogg};

/// A voice-like sound at `rate`: a gliding tone with a few harmonics, louder and softer
/// four times a second.
fn voice(rate: u32, seconds: f32) -> Vec<f32> {
    (0..(seconds * rate as f32) as usize)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let pitch = 2.0 * std::f32::consts::PI * (160.0 * t + 15.0 * t * t);
            let note: f32 = (1..=4).map(|k| (pitch * k as f32).sin() / k as f32).sum();
            0.3 * note * (0.6 - 0.4 * (2.0 * std::f32::consts::PI * 4.0 * t).cos())
        })
        .collect()
}

fn rms(samples: &[f32]) -> f64 {
    let squares: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (squares / samples.len().max(1) as f64).sqrt()
}

fn play_all(player: &mut Player) -> Vec<f32> {
    let mut out = Vec::new();
    // What an output device asks for at a time.
    let mut chunk = vec![0.0; 512];
    loop {
        let n = player.fill(&mut chunk);
        out.extend_from_slice(&chunk[..n]);
        if n < chunk.len() {
            return out;
        }
    }
}

/// Noise from -1 to 1 that is the same at every run.
fn noise(samples: usize) -> Vec<f32> {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    (0..samples)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        })
        .collect()
}

#[test]
fn a_message_from_a_44_1_khz_microphone_plays_on_a_44_1_khz_speaker() {
    let device = 44_100;
    let spoken = voice(device, 3.0);
    // The microphone hands its sound in pieces of its own size.
    let mut to_48k = Linear::new(device, RATE);
    let mut recorder = Recorder::new().unwrap();
    let mut at_48k = Vec::new();
    for piece in spoken.chunks(441) {
        at_48k.clear();
        to_48k.push(piece, &mut at_48k);
        assert_eq!(recorder.push(&at_48k).unwrap(), at_48k.len());
    }
    let message = recorder.finish(77).unwrap().unwrap();
    assert!(
        (2_990..=3_000).contains(&message.duration_ms),
        "{}",
        message.duration_ms
    );
    assert_eq!(message.bars.len(), BARS);
    // Louder and softer four times a second: the bars rise and fall.
    assert_eq!(message.bars.iter().copied().max(), Some(255));
    assert!(message.bars.iter().any(|&b| b < 200), "{:?}", message.bars);

    // A friend's copy reads the file as it came.
    let read = ogg::read(&message.file).unwrap();
    assert_eq!(read.duration_ms(), message.duration_ms);
    assert_eq!(read.channels, 1);
    let mut player = Player::new(&message.file).unwrap();
    assert_eq!(player.duration_ms(), message.duration_ms);
    let played = play_all(&mut player);
    assert!(player.ended());
    assert_eq!(played.len() as u64, read.samples);

    let mut to_device = Linear::new(RATE, device);
    let mut heard = Vec::new();
    to_device.push(&played, &mut heard);
    // As long as what was said, within a frame, and as loud within 1.5 dB: Opus is lossy,
    // and the rate changes smooth a little, but a voice comes through whole.
    let frame_at_device = FRAME * device as usize / RATE as usize;
    assert!(
        heard.len().abs_diff(spoken.len()) <= frame_at_device,
        "{} played for {} said",
        heard.len(),
        spoken.len()
    );
    let skip = frame_at_device;
    let db = 20.0 * (rms(&heard[skip..]) / rms(&spoken[skip..])).log10();
    assert!(db.abs() < 1.5, "{db} dB");
    assert!(heard.iter().all(|s| s.is_finite()));

    // At 2x it plays in half the time, as loud.
    let mut player = Player::new(&message.file).unwrap();
    player.set_speed(2.0);
    let fast = play_all(&mut player);
    let ratio = fast.len() as f64 / played.len() as f64;
    assert!((ratio - 0.5).abs() < 0.01, "{ratio}");
    let db = 20.0 * (rms(&fast) / rms(&played)).log10();
    assert!(db.abs() < 1.5, "{db} dB at 2x");
}

/// The longest message the recorder makes, of the sound that costs Opus the most (loud
/// noise), is one a friend's copy reads: under the size it reads at most and as long as it
/// lets a message be. Its rate is the recorder's; its caps are the reader's.
#[test]
fn the_longest_loudest_message_is_one_a_friend_reads() {
    let mut recorder = Recorder::new().unwrap();
    let second = noise(RATE as usize);
    while !recorder.full() {
        recorder.push(&second).unwrap();
    }
    assert_eq!(recorder.push(&second).unwrap(), 0);
    let message = recorder.finish(1).unwrap().unwrap();
    assert_eq!(message.duration_ms, MAX_MS);
    assert!(
        message.file.len() < MAX_FILE_BYTES,
        "{} bytes for 15 minutes",
        message.file.len()
    );
    let read = ogg::read(&message.file).unwrap();
    assert_eq!(read.duration_ms(), MAX_MS);
    // Every packet is one the reader takes as voice.
    for i in 0..read.packets() {
        assert_eq!(ogg::packet_samples(read.packet(i)), Ok(FRAME));
    }
    assert!(Player::new(&message.file).is_ok());
}

/// A message cut short anywhere, as a transfer that stopped leaves it, is refused or plays
/// to its end with sound that is all numbers: never a panic.
#[test]
fn a_message_cut_anywhere_is_refused_or_plays() {
    let mut recorder = Recorder::new().unwrap();
    recorder.push(&voice(RATE, 0.5)).unwrap();
    let file = recorder.finish(3).unwrap().unwrap().file;
    for len in 0..file.len() {
        let Ok(mut player) = Player::new(&file[..len]) else {
            continue;
        };
        let out = play_all(&mut player);
        assert!(out.len() as u64 <= player.samples());
        assert!(out.iter().all(|s| s.is_finite()));
        assert!(player.ended());
    }
    assert!(Player::new(&file).is_ok());
}
