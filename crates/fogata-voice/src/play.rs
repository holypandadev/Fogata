//! Playing a message: decoded as it plays, so a long one never sits decoded in memory,
//! and stretched in time when faster than 1×.

use opus::{Channels, Decoder};

use crate::{
    Error, RATE,
    ogg::{self, Opus},
    wsola::Wsola,
};

/// Decoded sound kept before what plays now: what a piece may look back to.
const KEEP: u64 = RATE as u64;

/// A message playing, or ready to.
pub struct Player {
    opus: Opus,
    decoder: Decoder,
    /// The next packet to decode.
    next: usize,
    /// Decoded samples still to drop: the pre-skip.
    skip: usize,
    /// Where a packet is decoded: the most one may hold, 120 ms. Kept, so playing, which
    /// runs on the output's audio thread, never waits for memory.
    frame: Vec<f32>,
    /// Decoded sound from sample `base` of the message on.
    buf: Vec<f32>,
    base: u64,
    /// Where 1× playing is, in samples of the message.
    at: u64,
    speed: f64,
    /// Faster than 1×.
    stretch: Option<Wsola>,
    /// Stretched sound made and not handed out yet.
    ready: Vec<f32>,
    /// A packet did not decode: the message ends there.
    broken: bool,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("samples", &self.opus.samples)
            .field("at", &self.position())
            .field("speed", &self.speed)
            .finish_non_exhaustive()
    }
}

impl Player {
    /// Reads a message's file (strictly: it may come from anyone) to play from its start.
    pub fn new(file: &[u8]) -> Result<Self, Error> {
        let opus = ogg::read(file)?;
        let decoder =
            Decoder::new(RATE, Channels::Mono).map_err(|_| Error("the voice decoder failed"))?;
        Ok(Player {
            skip: usize::from(opus.pre_skip),
            frame: vec![0.0; 5760],
            opus,
            decoder,
            next: 0,
            buf: Vec::new(),
            base: 0,
            at: 0,
            speed: 1.0,
            stretch: None,
            ready: Vec::new(),
            broken: false,
        })
    }

    /// The message's length in samples.
    pub fn samples(&self) -> u64 {
        self.opus.samples
    }

    pub fn duration_ms(&self) -> u64 {
        self.opus.duration_ms()
    }

    /// How far into the message playing is, in samples.
    pub fn position(&self) -> u64 {
        if self.ended() {
            return self.opus.samples;
        }
        match &self.stretch {
            Some(w) => w.position().min(self.opus.samples),
            None => self.at.min(self.opus.samples),
        }
    }

    pub fn position_ms(&self) -> u64 {
        self.position() * 1000 / u64::from(RATE)
    }

    /// Everything played.
    pub fn ended(&self) -> bool {
        let end = self.opus.samples;
        match &self.stretch {
            Some(w) => w.next_start() >= end && self.ready.is_empty(),
            None => self.at >= end,
        }
    }

    /// Goes back to the start.
    pub fn rewind(&mut self) {
        let _ = self.decoder.reset_state();
        self.next = 0;
        self.skip = usize::from(self.opus.pre_skip);
        self.buf.clear();
        self.base = 0;
        self.at = 0;
        self.ready.clear();
        self.stretch = (self.speed != 1.0).then(|| Wsola::new(self.speed, 0));
    }

    /// Plays at `speed` (1, 1.5, 2...) from where it is; one that is not a number or not above
    /// 0 is 1×, and speeds are kept between 0.25× and 4×.
    pub fn set_speed(&mut self, speed: f64) {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed.clamp(0.25, 4.0)
        } else {
            1.0
        };
        if speed == self.speed {
            return;
        }
        let at = self.position();
        self.speed = speed;
        self.ready.clear();
        self.at = at;
        self.stretch = (speed != 1.0).then(|| Wsola::new(speed, at));
    }

    /// Decodes until sample `to` (exclusive) of the message is in `buf`, or the end.
    fn decode_to(&mut self, to: u64) {
        let end = self.opus.samples;
        while self.base + (self.buf.len() as u64) < to.min(end)
            && self.next < self.opus.packets()
            && !self.broken
        {
            let packet = self.opus.packet(self.next);
            self.next += 1;
            let Ok(n) = self.decoder.decode_float(packet, &mut self.frame, false) else {
                self.broken = true;
                break;
            };
            let n = n.min(self.frame.len());
            let dropped = self.skip.min(n);
            self.skip -= dropped;
            let room = end.saturating_sub(self.base + self.buf.len() as u64) as usize;
            let take = (n - dropped).min(room);
            self.buf
                .extend_from_slice(&self.frame[dropped..dropped + take]);
        }
        if self.broken || self.next >= self.opus.packets() {
            // What could not be decoded is cut: the message ends where its sound does.
            self.opus.samples = self.opus.samples.min(self.base + self.buf.len() as u64);
        }
    }

    /// Drops decoded sound well before `from`.
    fn forget_before(&mut self, from: u64) {
        let keep_from = from.saturating_sub(KEEP);
        if keep_from > self.base + KEEP {
            let n = ((keep_from - self.base) as usize).min(self.buf.len());
            self.buf.drain(..n);
            self.base += n as u64;
        }
    }

    /// Fills `out` with 48 kHz mono sound; returns how much it filled, less than all of it
    /// only at the end.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        let mut filled = 0;
        while filled < out.len() {
            if self.stretch.is_none() {
                self.decode_to(self.at + (out.len() - filled) as u64);
                let from = self.at.saturating_sub(self.base) as usize;
                let available = self.buf.len().saturating_sub(from);
                let n = available.min(out.len() - filled);
                if n == 0 {
                    break;
                }
                out[filled..filled + n].copy_from_slice(&self.buf[from..from + n]);
                filled += n;
                self.at += n as u64;
                self.forget_before(self.at);
                continue;
            }
            if self.ready.is_empty() {
                let Some(w) = self.stretch.as_ref() else {
                    break;
                };
                if w.next_start() >= self.opus.samples {
                    break;
                }
                let (from, to) = w.needs();
                self.decode_to(to);
                if from >= self.opus.samples {
                    break;
                }
                let start = from.max(self.base);
                let lo = (start - self.base) as usize;
                let hi = (to.saturating_sub(self.base) as usize).min(self.buf.len());
                let Some(w) = self.stretch.as_mut() else {
                    break;
                };
                w.step(&self.buf[lo..hi.max(lo)], start, &mut self.ready);
                self.forget_before(from);
            }
            let n = self.ready.len().min(out.len() - filled);
            out[filled..filled + n].copy_from_slice(&self.ready[..n]);
            self.ready.drain(..n);
            filled += n;
        }
        filled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FRAME, Recorder};

    fn sine(hertz: f32, samples: usize, amp: f32) -> Vec<f32> {
        (0..samples)
            .map(|i| amp * (2.0 * std::f32::consts::PI * hertz * i as f32 / RATE as f32).sin())
            .collect()
    }

    fn record(pcm: &[f32]) -> Vec<u8> {
        let mut r = Recorder::new().unwrap();
        r.push(pcm).unwrap();
        r.finish(3).unwrap().unwrap().file
    }

    fn play_all(player: &mut Player) -> Vec<f32> {
        let mut out = vec![];
        let mut chunk = vec![0.0; 441];
        loop {
            let n = player.fill(&mut chunk);
            out.extend_from_slice(&chunk[..n]);
            if n < chunk.len() {
                break;
            }
        }
        out
    }

    /// The correlation of `a` and `b` at the shift (up to 10 ms) that matches them best.
    fn best_correlation(a: &[f32], b: &[f32]) -> f64 {
        let n = a.len().min(b.len()) - 480;
        (0..480)
            .map(|shift| {
                let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
                for i in 0..n {
                    let (x, y) = (f64::from(a[i]), f64::from(b[i + shift]));
                    ab += x * y;
                    aa += x * x;
                    bb += y * y;
                }
                ab / (aa.sqrt() * bb.sqrt()).max(1e-12)
            })
            .fold(f64::MIN, f64::max)
    }

    #[test]
    fn plays_back_what_was_recorded() {
        // A tone that glides, so a shift in time shows.
        let pcm: Vec<f32> = (0..RATE as usize * 3)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                0.4 * (2.0 * std::f32::consts::PI * (300.0 * t + 40.0 * t * t)).sin()
            })
            .collect();
        let mut player = Player::new(&record(&pcm)).unwrap();
        assert_eq!(player.duration_ms(), 3000);
        let out = play_all(&mut player);
        assert_eq!(out.len(), pcm.len());
        assert!(player.ended());
        // Opus is lossy; at 24 kbit/s a clean tone still matches closely, in place.
        let c = best_correlation(&pcm[FRAME..], &out[FRAME..]);
        assert!(c > 0.9, "{c}");
    }

    #[test]
    fn faster_is_shorter_and_keeps_the_pitch() {
        let pcm = sine(440.0, RATE as usize * 4, 0.5);
        let file = record(&pcm);
        for speed in [1.5, 2.0] {
            let mut player = Player::new(&file).unwrap();
            player.set_speed(speed);
            let out = play_all(&mut player);
            let ratio = out.len() as f64 / pcm.len() as f64;
            assert!((ratio - 1.0 / speed).abs() < 0.01, "{speed}: {ratio}");
            let middle = &out[4800..out.len() - 4800];
            let ups = middle
                .windows(2)
                .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
                .count();
            let hertz = ups as f64 * f64::from(RATE) / middle.len() as f64;
            assert!((hertz - 440.0).abs() < 5.0, "{speed}: {hertz}");
            assert!(player.ended());
            assert_eq!(player.position(), player.samples());
        }
    }

    /// Played faster, a voice comes out as stretching all of it at once would make it:
    /// the player keeps decoded what each piece is matched against.
    #[test]
    fn faster_voice_plays_as_stretched_whole() {
        let pcm: Vec<f32> = (0..RATE as usize * 2)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let note: f32 = (1..=6)
                    .map(|k| (2.0 * std::f32::consts::PI * 140.0 * k as f32 * t).sin() / k as f32)
                    .sum();
                0.25 * note * (0.55 - 0.45 * (2.0 * std::f32::consts::PI * 4.0 * t).cos())
            })
            .collect();
        let file = record(&pcm);
        let decoded = play_all(&mut Player::new(&file).unwrap());
        for speed in [1.5, 2.0] {
            let mut whole = Wsola::new(speed, 0);
            let mut expected = vec![];
            while whole.next_start() < decoded.len() as u64 {
                whole.step(&decoded, 0, &mut expected);
            }
            let mut player = Player::new(&file).unwrap();
            player.set_speed(speed);
            let out = play_all(&mut player);
            assert_eq!(out.len(), expected.len(), "{speed}");
            let differ = out
                .iter()
                .zip(&expected)
                .filter(|(a, b)| (*a - *b).abs() > 1e-6)
                .count();
            assert_eq!(differ, 0, "{speed}: {differ} samples differ");
        }
    }

    /// Once it plays, what playing keeps is reused: no buffer is made anew or grows, so
    /// the output's audio thread does not wait for memory.
    #[test]
    fn playing_reuses_what_it_keeps() {
        let file = record(&sine(180.0, RATE as usize * 6, 0.3));
        for speed in [1.0, 1.5, 2.0] {
            let mut player = Player::new(&file).unwrap();
            player.set_speed(speed);
            let mut out = vec![0.0f32; 441];
            for _ in 0..RATE as usize * 2 / out.len() {
                player.fill(&mut out);
            }
            let kept = |p: &Player| {
                (
                    p.frame.as_ptr(),
                    p.buf.as_ptr(),
                    p.buf.capacity(),
                    p.ready.capacity(),
                )
            };
            let before = kept(&player);
            for _ in 0..RATE as usize / out.len() {
                assert_eq!(player.fill(&mut out), out.len());
                assert_eq!(kept(&player), before, "{speed}×");
            }
        }
    }

    #[test]
    fn the_speed_can_change_while_playing() {
        let pcm = sine(330.0, RATE as usize * 2, 0.5);
        let mut player = Player::new(&record(&pcm)).unwrap();
        let mut half = vec![0.0; RATE as usize / 2];
        assert_eq!(player.fill(&mut half), half.len());
        assert_eq!(player.position_ms(), 500);
        player.set_speed(2.0);
        let rest = play_all(&mut player);
        // The last 1.5 s at 2×: 0.75 s.
        let seconds = rest.len() as f64 / f64::from(RATE);
        assert!((seconds - 0.75).abs() < 0.03, "{seconds}");
        player.set_speed(1.0);
        assert!(player.ended());
        // Again from the start.
        player.rewind();
        assert_eq!(player.position(), 0);
        assert_eq!(play_all(&mut player).len(), pcm.len());
    }

    /// Packets changed at random under good checksums reach the decoder: playing them at
    /// any speed never panics, never runs past the length and always ends.
    #[test]
    fn damaged_packets_play_or_end_without_harm() {
        let good = record(&sine(250.0, RATE as usize, 0.4));
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut played = 0;
        for round in 0..300 {
            let mut f = good.clone();
            // Past the two header pages.
            let from = 120;
            for _ in 0..1 + next() % 6 {
                let at = from + (next() as usize) % (f.len() - from);
                f[at] = next() as u8;
            }
            crate::ogg::tests::reseal_lenient(&mut f);
            let Ok(mut player) = Player::new(&f) else {
                continue;
            };
            played += 1;
            player.set_speed([1.0, 1.5, 2.0][round % 3]);
            let out = play_all(&mut player);
            assert!(out.len() as u64 <= player.samples() + 960);
            assert!(out.iter().all(|s| s.is_finite()));
            assert!(player.ended());
        }
        assert!(played > 0);
    }

    #[test]
    fn odd_speeds_play_at_1x_or_within_bounds() {
        let pcm = sine(330.0, RATE as usize, 0.5);
        let file = record(&pcm);
        for (speed, ratio) in [(f64::NAN, 1.0), (-1.0, 1.0), (0.0, 1.0), (100.0, 0.25)] {
            let mut player = Player::new(&file).unwrap();
            player.set_speed(speed);
            let out = play_all(&mut player);
            let got = out.len() as f64 / pcm.len() as f64;
            assert!((got - ratio).abs() < 0.02, "{speed}: {got}");
        }
    }

    #[test]
    fn a_damaged_file_does_not_play() {
        let mut file = record(&sine(200.0, 9600, 0.3));
        let n = file.len();
        file[n - 5] ^= 1;
        assert!(Player::new(&file).is_err());
    }
}
