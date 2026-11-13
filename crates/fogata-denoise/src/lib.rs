//! Noise suppression for the microphone's sound in calls: RNNoise, in its Rust form
//! (nnnoiseless). A small neural network looks at each 10 ms of sound in 22 bands, tells
//! speech from noise and lowers the bands that hold noise, so a fan, a hum, typing or
//! traffic fade while the voice stays as loud as it was.
//!
//! WebRTC hands over its processed microphone sound in 10 ms frames at 16, 32 or 48 kHz;
//! RNNoise works on 10 ms frames at 48 kHz, so the other two rates are resampled on the
//! way in and out (about 1 ms more delay each way). RNNoise itself gives each frame back
//! one frame (10 ms) later: the price of looking at 20 ms at a time.
//!
//! RNNoise takes a sound that starts out of silence for speech for a while: after as
//! little as 10 ms of nothing (the microphone opening again, the sound server dropping a
//! moment) a steady noise came back whole for up to a second. So frames that hold
//! nothing go past it as they are: it never hears the gap, and goes on from what it knew
//! of the noise before it. The frame it was holding back when the gap began (the 10 ms
//! before it) is dropped rather than played after the gap.
//!
//! [`Denoiser::process`] is told on every frame whether to suppress, so the setting
//! applies from the next frame. While off it only keeps the last frames, so that turning
//! it on starts from a network that has already heard them; either way the change
//! cross-fades over one frame.

#![forbid(unsafe_code)]

use nnnoiseless::DenoiseState;

mod resample;

use resample::Resampler;

/// Samples in one 10 ms frame at 48 kHz, what RNNoise takes at a time.
const FRAME: usize = DenoiseState::FRAME_SIZE;

/// Frames kept while off and replayed when turned on: RNNoise's pitch analysis looks
/// 36 ms back.
const HISTORY: usize = 4;

/// A frame whose every sample is under this (16-bit values: about −78 dBFS) holds
/// nothing: digital silence, or its last bit of dither.
const NOTHING: f32 = 4.0;

/// The rates WebRTC processes the microphone's sound at.
const RATES: [usize; 3] = [16000, 32000, 48000];

/// One channel of microphone sound being cleaned. Not shared between channels.
pub struct Denoiser {
    rnn: Box<DenoiseState<'static>>,
    /// Samples a second of the frames, 0 before the first.
    rate: usize,
    /// The last frame went through RNNoise.
    running: bool,
    /// The last frame held nothing and went past RNNoise.
    gap: bool,
    /// The last [`HISTORY`] frames as they came in, oldest first.
    history: Vec<f32>,
    /// To 48 kHz and back, for 16 and 32 kHz.
    up: Option<Resampler>,
    down: Option<Resampler>,
    wide_in: [f32; FRAME],
    wide_out: [f32; FRAME],
    /// What RNNoise gave back for the last frame, at the frame's rate.
    clean: Vec<f32>,
    /// A frame as it came in, while fading out of RNNoise.
    raw: Vec<f32>,
}

impl std::fmt::Debug for Denoiser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Denoiser")
            .field("rate", &self.rate)
            .field("running", &self.running)
            .finish_non_exhaustive()
    }
}

impl Default for Denoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl Denoiser {
    pub fn new() -> Self {
        Denoiser {
            rnn: DenoiseState::new(),
            rate: 0,
            running: false,
            gap: false,
            history: Vec::new(),
            up: None,
            down: None,
            wide_in: [0.0; FRAME],
            wide_out: [0.0; FRAME],
            clean: Vec::new(),
            raw: Vec::new(),
        }
    }

    /// Whether `process` takes `frames` samples at `rate`: 10 ms at 16, 32 or 48 kHz.
    pub fn accepts(rate: i32, frames: usize) -> bool {
        usize::try_from(rate).is_ok_and(|rate| RATES.contains(&rate) && frames == rate / 100)
    }

    /// Cleans one 10 ms frame in place when `on`, samples as 16-bit values in floats
    /// (−32768 to 32767, as WebRTC keeps them). A frame of any other rate or length is
    /// left as it is.
    pub fn process(&mut self, on: bool, rate: usize, frame: &mut [f32]) {
        if !RATES.contains(&rate) || frame.len() != rate / 100 {
            self.running = false;
            return;
        }
        if rate != self.rate {
            self.start(rate);
        }
        // A gap: its frames go past RNNoise as they are, on or off, and a change of the
        // switch waits for the sound to come back.
        if frame.iter().all(|x| x.abs() < NOTHING) {
            self.gap = true;
            return;
        }
        let after_gap = std::mem::take(&mut self.gap);
        match (on, self.running) {
            (false, false) => self.remember(frame),
            (true, true) => {
                self.denoise(frame);
                if after_gap {
                    // What RNNoise held back is the sound from before the gap; this
                    // frame's own comes with the next one, as ever.
                    frame.fill(0.0);
                } else {
                    frame.copy_from_slice(&self.clean);
                }
            }
            (true, false) => {
                // The frames heard while off, so the network, its pitch analysis and its
                // overlap start where the sound is; what they give back is not used.
                self.reset_resamplers();
                let history = std::mem::take(&mut self.history);
                for old in history.chunks(frame.len()) {
                    self.denoise(old);
                }
                self.history = history;
                self.denoise(frame);
                cross_fade(frame, &self.clean);
                self.running = true;
            }
            (false, true) => {
                self.raw.copy_from_slice(frame);
                self.denoise(frame);
                frame.copy_from_slice(&self.clean);
                cross_fade(frame, &self.raw);
                let raw = std::mem::take(&mut self.raw);
                self.remember(&raw);
                self.raw = raw;
                self.running = false;
            }
        }
    }

    /// Forgets everything heard: a new rate.
    fn start(&mut self, rate: usize) {
        self.rate = rate;
        self.running = false;
        self.history = vec![0.0; HISTORY * rate / 100];
        self.clean = vec![0.0; rate / 100];
        self.raw = vec![0.0; rate / 100];
        self.up = (rate != 48000).then(|| Resampler::new(rate, 48000));
        self.down = (rate != 48000).then(|| Resampler::new(48000, rate));
    }

    fn reset_resamplers(&mut self) {
        if let Some(up) = &mut self.up {
            up.reset();
        }
        if let Some(down) = &mut self.down {
            down.reset();
        }
    }

    fn remember(&mut self, frame: &[f32]) {
        let len = frame.len();
        self.history.copy_within(len.., 0);
        let end = self.history.len();
        self.history[end - len..].copy_from_slice(frame);
    }

    /// RNNoise over one frame at the frame's rate; the result, one frame late, goes to
    /// `clean`.
    fn denoise(&mut self, frame: &[f32]) {
        match &mut self.up {
            Some(up) => up.process(frame, &mut self.wide_in),
            None => self.wide_in.copy_from_slice(frame),
        }
        self.rnn.process_frame(&mut self.wide_out, &self.wide_in);
        match &mut self.down {
            Some(down) => down.process(&self.wide_out, &mut self.clean),
            None => self.clean.copy_from_slice(&self.wide_out),
        }
    }
}

/// Goes from what `frame` holds to `to` over the frame, along a raised cosine.
fn cross_fade(frame: &mut [f32], to: &[f32]) {
    let n = frame.len() as f32;
    for (i, (a, b)) in frame.iter_mut().zip(to).enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::PI * (i as f32 + 0.5) / n).cos();
        *a = *a * (1.0 - w) + *b * w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A noise that is the same on every run.
    fn noise(n: usize, seed: &mut u64, level: f32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((*seed >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 2.0 * level
            })
            .collect()
    }

    /// A fan's noise, the same on every run: white noise through two low-pass filters
    /// of one pole at about 1 kHz. RNNoise lowers it by 40 dB and more at every rate
    /// (white noise up to 16 or 24 kHz, by a few dB only).
    struct Fan {
        seed: u64,
        a: f32,
        y: [f32; 2],
    }

    impl Fan {
        fn new(rate: usize) -> Self {
            Fan {
                seed: 7,
                a: 1.0 - (-2.0 * std::f32::consts::PI * 1000.0 / rate as f32).exp(),
                y: [0.0; 2],
            }
        }

        fn frame(&mut self, n: usize) -> Vec<f32> {
            noise(n, &mut self.seed, 12000.0)
                .into_iter()
                .map(|x| {
                    self.y[0] += self.a * (x - self.y[0]);
                    self.y[1] += self.a * (self.y[0] - self.y[1]);
                    self.y[1]
                })
                .collect()
        }
    }

    fn rms(xs: &[f32]) -> f32 {
        (xs.iter().map(|x| x * x).sum::<f32>() / xs.len() as f32).sqrt()
    }

    fn db(ratio: f32) -> f32 {
        20.0 * ratio.log10()
    }

    /// Steady noise alone, at every rate: much lower once RNNoise has heard a second of it.
    #[test]
    fn steady_noise_is_lowered() {
        for rate in RATES {
            let mut d = Denoiser::new();
            let mut seed = 7;
            let len = rate / 100;
            let mut before = Vec::new();
            let mut after = Vec::new();
            for i in 0..300 {
                let mut frame = noise(len, &mut seed, 1000.0);
                if i >= 100 {
                    before.extend_from_slice(&frame);
                }
                d.process(true, rate, &mut frame);
                assert!(frame.iter().all(|x| x.is_finite()));
                if i >= 100 {
                    after.extend_from_slice(&frame);
                }
            }
            let lowered = db(rms(&before) / rms(&after));
            assert!(lowered > 10.0, "{rate} Hz: only {lowered:.1} dB lower");
        }
    }

    /// A gap of silence in a fan's noise, as a microphone that opens again or a sound
    /// server that drops a moment leaves: the gap stays quiet, and the noise that
    /// follows is as low as before it from its first 100 ms on, whether the gap holds
    /// nothing at all or a last bit of dither. (RNNoise, hearing the gap, let the noise
    /// back whole for up to a second after as little as 20 ms of nothing.)
    #[test]
    fn noise_stays_lowered_after_a_gap_of_silence() {
        for rate in RATES {
            for gap in [1, 2, 5, 20, 100, 300] {
                for dither in [0.0, 1.0] {
                    let mut d = Denoiser::new();
                    let mut fan = Fan::new(rate);
                    let mut seed = 5;
                    let len = rate / 100;
                    for _ in 0..300 {
                        d.process(true, rate, &mut fan.frame(len));
                    }
                    for _ in 0..gap {
                        let mut frame = noise(len, &mut seed, dither);
                        d.process(true, rate, &mut frame);
                        let loudest = frame.iter().fold(0.0f32, |m, x| m.max(x.abs()));
                        assert!(loudest < 30.0, "{rate} Hz: {loudest} in a gap");
                    }
                    for tenth in 0..3 {
                        let mut before = Vec::new();
                        let mut after = Vec::new();
                        for _ in 0..10 {
                            let mut frame = fan.frame(len);
                            before.extend_from_slice(&frame);
                            d.process(true, rate, &mut frame);
                            assert!(frame.iter().all(|x| x.is_finite()));
                            after.extend_from_slice(&frame);
                        }
                        let lowered = db(rms(&before) / rms(&after).max(1e-3));
                        assert!(
                            lowered > 30.0,
                            "{rate} Hz, {} ms after a gap of {gap} frames \
                             (dither {dither}): only {lowered:.1} dB lower",
                            tenth * 100
                        );
                    }
                }
            }
        }
    }

    /// The fan's noise alone is lowered that much once RNNoise has heard a second of it,
    /// at every rate: what the test of the gap compares with.
    #[test]
    fn a_fans_noise_is_lowered_by_30_db() {
        for rate in RATES {
            let mut d = Denoiser::new();
            let mut fan = Fan::new(rate);
            let len = rate / 100;
            let mut before = Vec::new();
            let mut after = Vec::new();
            for i in 0..300 {
                let mut frame = fan.frame(len);
                if i >= 100 {
                    before.extend_from_slice(&frame);
                }
                d.process(true, rate, &mut frame);
                if i >= 100 {
                    after.extend_from_slice(&frame);
                }
            }
            let lowered = db(rms(&before) / rms(&after).max(1e-3));
            assert!(lowered > 30.0, "{rate} Hz: only {lowered:.1} dB lower");
        }
    }

    /// Off, nothing changes, at any rate, and odd frames are left alone.
    #[test]
    fn off_leaves_the_sound_alone() {
        let mut d = Denoiser::new();
        let mut seed = 3;
        for rate in RATES {
            for _ in 0..20 {
                let frame = noise(rate / 100, &mut seed, 3000.0);
                let mut out = frame.clone();
                d.process(false, rate, &mut out);
                assert_eq!(out, frame);
            }
        }
        let frame = noise(441, &mut seed, 3000.0);
        let mut out = frame.clone();
        d.process(true, 44100, &mut out);
        assert_eq!(out, frame);
    }

    /// Turning it on and off in the middle of a tone: no jump bigger than the tone's own
    /// steps, so no click.
    #[test]
    fn switching_does_not_click() {
        for rate in RATES {
            let mut d = Denoiser::new();
            let len = rate / 100;
            let mut out = Vec::new();
            let mut t = 0usize;
            for i in 0..200 {
                let mut frame: Vec<f32> = (0..len)
                    .map(|k| {
                        let s = (t + k) as f32 / rate as f32;
                        8000.0 * (2.0 * std::f32::consts::PI * 220.0 * s).sin()
                    })
                    .collect();
                t += len;
                d.process((i / 25) % 2 == 1, rate, &mut frame);
                out.extend_from_slice(&frame);
            }
            // The largest step of a 220 Hz tone at 8000 is 8000 · 2π · 220 / rate.
            let tone_step = 8000.0 * 2.0 * std::f32::consts::PI * 220.0 / rate as f32;
            let worst = out
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0, f32::max);
            assert!(
                worst < 2.5 * tone_step,
                "{rate} Hz: step {worst} vs {tone_step}"
            );
        }
    }
}
