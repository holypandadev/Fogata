//! Playing voice faster without raising it: WSOLA (waveform similarity overlap-add).
//! The sound is cut into 20 ms pieces, each faded in and out, laid half over the one
//! before; to go faster, pieces are taken further apart in the original than they are laid
//! in the output. Each piece is taken within 5 ms of where it nominally starts, where it
//! best continues the piece before, so the waves join in step and the voice keeps its
//! pitch.

/// Samples in a piece: 20 ms at 48 kHz.
pub const PIECE: usize = 960;

/// Samples between pieces in the output: half a piece.
pub const HOP: usize = PIECE / 2;

/// How far from its nominal start a piece may be taken: 5 ms.
pub const SEEK: usize = 240;

/// Every how many samples the similarity is measured: enough for voice, at a quarter of
/// the work.
const STRIDE: usize = 2;

/// Stretches sound in time by `1 / speed`, one [`HOP`] of output at a time.
#[derive(Debug)]
pub struct Wsola {
    speed: f64,
    /// Where the next piece nominally starts in the input.
    nominal: f64,
    /// Where the last piece was taken from.
    prev: Option<u64>,
    /// Output being overlapped: the current piece and the second half of the last.
    acc: Vec<f32>,
    window: Vec<f32>,
}

impl Wsola {
    /// Starts at sample `start` of the input, for `speed`: one that is not a number or not
    /// above 0 is 1×, and speeds are kept between 0.25× and 4×.
    pub fn new(speed: f64, start: u64) -> Self {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        };
        let window = (0..PIECE)
            .map(|i| {
                0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / PIECE as f64).cos() as f32
            })
            .collect();
        Wsola {
            speed: speed.clamp(0.25, 4.0),
            nominal: start as f64,
            prev: None,
            acc: vec![0.0; PIECE],
            window,
        }
    }

    /// Where the next piece nominally starts: how far into the input the output is.
    pub fn position(&self) -> u64 {
        self.prev.unwrap_or(self.nominal as u64)
    }

    /// Where the next piece nominally starts.
    pub fn next_start(&self) -> u64 {
        self.nominal as u64
    }

    /// The input the next [`Wsola::step`] reads: from (inclusive) and to (exclusive). It
    /// holds the places searched and what follows the last piece, which a piece is matched
    /// against: faster than 1× that lies before the places searched.
    pub fn needs(&self) -> (u64, u64) {
        let nominal = self.nominal as u64;
        let mut from = nominal.saturating_sub(SEEK as u64);
        let mut to = nominal + (SEEK + PIECE) as u64;
        if let Some(prev) = self.prev {
            from = from.min(prev + HOP as u64);
            to = to.max(prev + (HOP + PIECE) as u64);
        }
        (from, to)
    }

    /// Adds the next piece and hands out [`HOP`] samples into `out`. `input` holds the
    /// input from sample `base` on (at least what [`Wsola::needs`] says, or up to its end:
    /// past it reads as silence).
    pub fn step(&mut self, input: &[f32], base: u64, out: &mut Vec<f32>) {
        let at = |i: u64| -> f32 {
            i.checked_sub(base)
                .and_then(|i| input.get(i as usize))
                .copied()
                .unwrap_or(0.0)
        };
        let nominal = self.nominal as u64;
        let start = match self.prev {
            None => nominal,
            Some(prev) => {
                // What would follow the last piece in the input, to match.
                let target = prev + HOP as u64;
                let lowest = nominal.saturating_sub(SEEK as u64);
                let mut best = (f32::MIN, nominal);
                for a in lowest..=nominal + SEEK as u64 {
                    let mut score = 0.0f32;
                    for j in (0..HOP as u64).step_by(STRIDE) {
                        score += at(a + j) * at(target + j);
                    }
                    // Ties go to the nominal place, so silence plays at its own pace.
                    let closer = a.abs_diff(nominal) < best.1.abs_diff(nominal);
                    if score > best.0 || (score == best.0 && closer) {
                        best = (score, a);
                    }
                }
                best.1
            }
        };
        for (i, slot) in self.acc.iter_mut().enumerate() {
            *slot += self.window[i] * at(start + i as u64);
        }
        out.extend_from_slice(&self.acc[..HOP]);
        self.acc.copy_within(HOP.., 0);
        self.acc[PIECE - HOP..].fill(0.0);
        self.prev = Some(start);
        self.nominal += HOP as f64 * self.speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn sine(hertz: f64, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (0.5 * (2.0 * std::f64::consts::PI * hertz * i as f64 / RATE).sin()) as f32)
            .collect()
    }

    fn stretch(input: &[f32], speed: f64) -> Vec<f32> {
        let mut w = Wsola::new(speed, 0);
        let mut out = vec![];
        while w.next_start() < input.len() as u64 {
            w.step(input, 0, &mut out);
        }
        out
    }

    /// The pitch of a steady tone, from how often it crosses zero going up.
    fn pitch(samples: &[f32]) -> f64 {
        let ups = samples
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        ups as f64 * RATE / samples.len() as f64
    }

    fn rms(samples: &[f32]) -> f64 {
        (samples
            .iter()
            .map(|&s| f64::from(s) * f64::from(s))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
    }

    #[test]
    fn faster_keeps_the_pitch_and_shortens_by_the_speed() {
        let input = sine(440.0, 48_000 * 4);
        for speed in [1.0, 1.5, 2.0] {
            let out = stretch(&input, speed);
            let ratio = out.len() as f64 / input.len() as f64;
            assert!((ratio - 1.0 / speed).abs() < 0.01, "{speed}: {ratio}");
            // Leave out the fade in and out at the ends.
            let middle = &out[2 * PIECE..out.len() - 2 * PIECE];
            let p = pitch(middle);
            assert!((p - 440.0).abs() < 4.0, "{speed}: {p} Hz");
            // As loud as it came: the pieces join in step, not cancelling.
            assert!(
                (rms(middle) - rms(&input)).abs() < 0.02,
                "{speed}: {}",
                rms(middle)
            );
        }
    }

    #[test]
    fn several_tones_keep_their_pitch() {
        for hertz in [150.0, 220.0, 1000.0] {
            let input = sine(hertz, 48_000 * 2);
            let out = stretch(&input, 2.0);
            let p = pitch(&out[2 * PIECE..out.len() - 2 * PIECE]);
            assert!((p - hertz).abs() < hertz * 0.02, "{hertz}: {p}");
        }
    }

    #[test]
    fn odd_speeds_are_tamed() {
        let input = sine(440.0, 48_000);
        for speed in [f64::NAN, f64::INFINITY, -2.0, 0.0, 1e-300, 1e300] {
            let out = stretch(&input, speed);
            // Between 4× and 0.25× of the input's length, and always ending.
            assert!(
                out.len() >= input.len() / 5 && out.len() <= input.len() * 5,
                "{speed}: {}",
                out.len()
            );
        }
    }

    /// Something like a voice: a low note with its overtones, rising and falling in
    /// syllables four times a second.
    fn voice_like(samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let t = i as f64 / RATE;
                let note: f64 = (1..=6)
                    .map(|k| (2.0 * std::f64::consts::PI * 140.0 * k as f64 * t).sin() / k as f64)
                    .sum();
                let syllables = 0.55 - 0.45 * (2.0 * std::f64::consts::PI * 4.0 * t).cos();
                (0.25 * note * syllables) as f32
            })
            .collect()
    }

    /// Stretched one step at a time from only what [`Wsola::needs`] says, as a player
    /// that keeps little decoded does.
    fn stretch_from_needs(input: &[f32], speed: f64) -> Vec<f32> {
        let mut w = Wsola::new(speed, 0);
        let mut out = vec![];
        while w.next_start() < input.len() as u64 {
            let (from, to) = w.needs();
            let to = to.min(input.len() as u64);
            w.step(&input[from as usize..to as usize], from, &mut out);
        }
        out
    }

    /// At every speed the piece before is matched against what follows it, which lies
    /// further back than the place searched when faster than 1×: what a step needs holds
    /// it, so each piece joins in step. A voice joined out of step loses loudness.
    #[test]
    fn faster_voice_is_matched_against_what_follows_the_piece_before() {
        let input = voice_like(48_000 * 2);
        for speed in [1.5, 2.0] {
            let whole = stretch(&input, speed);
            let windowed = stretch_from_needs(&input, speed);
            assert_eq!(whole.len(), windowed.len());
            let differ = whole
                .iter()
                .zip(&windowed)
                .filter(|(a, b)| (*a - *b).abs() > 1e-6)
                .count();
            assert_eq!(differ, 0, "{speed}: {differ} samples differ");
            let energy = |s: &[f32]| s.iter().map(|&x| f64::from(x).powi(2)).sum::<f64>();
            let kept = energy(&windowed) * speed / energy(&input);
            assert!((kept - 1.0).abs() < 0.05, "{speed}: {kept}");
        }
    }

    #[test]
    fn needs_cover_what_a_step_reads() {
        let input = sine(300.0, 48_000);
        let mut whole = Wsola::new(1.5, 0);
        let mut window = Wsola::new(1.5, 0);
        let (mut a, mut b) = (vec![], vec![]);
        while whole.next_start() < input.len() as u64 {
            whole.step(&input, 0, &mut a);
            let (from, to) = window.needs();
            let to = to.min(input.len() as u64);
            window.step(&input[from as usize..to as usize], from, &mut b);
        }
        assert_eq!(a, b);
    }
}
