//! The noise suppressor as calls and voice messages use it, through its public calls only:
//! 10 ms frames at the rates WebRTC hands over, samples as 16-bit values in floats, cleaned
//! in place. What its callers rely on: the frames it takes, how late the voice comes out,
//! a voice kept while the noise around it goes, silence left silent, and only numbers out
//! of whatever loud sound comes in.

use fogata_denoise::Denoiser;

const RATES: [usize; 3] = [16_000, 32_000, 48_000];

/// Of every 700 ms, the 400 ms a voice speaks; the rest is a pause.
fn speaking(rate: usize, i: usize) -> bool {
    i % (rate * 7 / 10) < rate * 4 / 10
}

/// A voice-like sound, as 16-bit values: a 140 Hz note with harmonics, louder and softer
/// five times a second, said for 400 ms and then still for 300 ms.
fn voice(rate: usize, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            if !speaking(rate, i) {
                return 0.0;
            }
            let t = i as f32 / rate as f32;
            let note: f32 = (1..=8)
                .map(|k| (2.0 * std::f32::consts::PI * 140.0 * k as f32 * t).sin() / k as f32)
                .sum();
            6000.0 * note * (0.6 - 0.4 * (2.0 * std::f32::consts::PI * 5.0 * t).cos())
        })
        .collect()
}

/// White noise of `level` (16-bit values), the same at every run.
fn noise(len: usize, seed: &mut u64, level: f32) -> Vec<f32> {
    (0..len)
        .map(|_| {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            ((*seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * level
        })
        .collect()
}

/// Runs `input` through a new suppressor, on, 10 ms at a time.
fn cleaned(rate: usize, input: &[f32]) -> Vec<f32> {
    let mut denoiser = Denoiser::new();
    let mut out = input.to_vec();
    for frame in out.chunks_mut(rate / 100) {
        denoiser.process(true, rate, frame);
    }
    out
}

/// How many samples later `out` follows `clean` (up to 30 ms), and how much of `out` is
/// `clean` scaled at that lag: (lag, gain, energy of what is left over).
fn best_match(clean: &[f32], out: &[f32], rate: usize) -> (usize, f64, f64) {
    let span = clean.len() - rate * 30 / 1000;
    (0..rate * 30 / 1000)
        .map(|lag| {
            let (mut co, mut cc) = (0.0, 0.0);
            for i in 0..span {
                co += f64::from(clean[i]) * f64::from(out[i + lag]);
                cc += f64::from(clean[i]) * f64::from(clean[i]);
            }
            let gain = co / cc;
            let left: f64 = (0..span)
                .map(|i| f64::from(out[i + lag]) - gain * f64::from(clean[i]))
                .map(|r| r * r)
                .sum();
            (lag, gain, left)
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .unwrap()
}

#[test]
fn it_takes_10_ms_at_the_rates_webrtc_uses_and_leaves_any_other_frame_alone() {
    for rate in RATES {
        assert!(Denoiser::accepts(rate as i32, rate / 100));
        assert!(!Denoiser::accepts(rate as i32, rate / 100 + 1));
        assert!(!Denoiser::accepts(rate as i32, rate / 50));
    }
    for (rate, frames) in [
        (44_100, 441),
        (8_000, 80),
        (96_000, 960),
        (-48_000, 480),
        (0, 0),
    ] {
        assert!(!Denoiser::accepts(rate, frames), "{rate} {frames}");
    }
    // What it does not take passes as it came, even on.
    let mut denoiser = Denoiser::new();
    let mut seed = 5;
    let odd = noise(441, &mut seed, 3000.0);
    let mut frame = odd.clone();
    denoiser.process(true, 44_100, &mut frame);
    assert_eq!(frame, odd);
}

/// A voice in steady noise: once the suppressor has heard a second, the noise in its
/// pauses is much lower and the voice about as loud, one frame (10 ms) later, and
/// up to 1 ms later again each way at 16 and 32 kHz.
#[test]
fn the_noise_around_a_voice_goes_and_the_voice_stays() {
    let mut found = Vec::new();
    for rate in RATES {
        let warm = rate;
        let len = 4 * rate;
        let clean = voice(rate, len);
        let mut seed = 11;
        let hiss = noise(len, &mut seed, 1500.0);
        let noisy: Vec<f32> = clean.iter().zip(&hiss).map(|(v, n)| v + n).collect();
        let out = cleaned(rate, &noisy);
        assert!(out.iter().all(|x| x.is_finite()));

        let (lag, gain, _) = best_match(&clean[warm..], &out[warm..], rate);
        // The noise in the pauses, 50 ms in from either end, as it came and as it went out.
        let edge = rate / 20;
        let in_pause =
            |i: usize| (0..=edge).all(|d| !speaking(rate, i - d) && !speaking(rate, i + d));
        let pauses: Vec<usize> = (warm..len - lag - edge).filter(|&i| in_pause(i)).collect();
        let before: f64 = pauses.iter().map(|&i| f64::from(hiss[i]).powi(2)).sum();
        let after: f64 = pauses
            .iter()
            .map(|&i| f64::from(out[i + lag]).powi(2))
            .sum();
        let lowered = 10.0 * (before / after).log10();
        let ms = lag as f64 * 1000.0 / rate as f64;
        eprintln!("{rate} Hz: {ms:.2} ms late, voice x{gain:.2}, noise {lowered:.1} dB lower");
        found.push((rate, lag, ms, gain, lowered));
    }
    for (rate, lag, ms, gain, lowered) in found {
        assert!(
            lowered > 10.0,
            "{rate} Hz: noise only {lowered:.1} dB lower"
        );
        // As loud within about 3 dB.
        assert!((0.7..1.4).contains(&gain), "{rate} Hz: voice x{gain:.2}");
        // RNNoise's own frame, within a few samples of its filters; the rate changes add
        // their half of a filter each way.
        let late = if rate == 48_000 {
            9.9..10.1
        } else {
            10.0..13.0
        };
        assert!(
            late.contains(&ms),
            "{rate} Hz: {ms:.2} ms late ({lag} samples)"
        );
    }
}

#[test]
fn silence_stays_silent() {
    for rate in RATES {
        let out = cleaned(rate, &vec![0.0; 2 * rate]);
        let loudest = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(loudest < 1.0, "{rate} Hz: {loudest}");
    }
}

/// The loudest sound a microphone can give, in every shape, comes out as numbers within
/// what 16 bits hold, on and off and switched between.
#[test]
fn the_loudest_sound_comes_out_as_numbers() {
    for rate in RATES {
        let len = rate / 100;
        let shapes: Vec<Vec<f32>> = vec![
            // Full scale, clipped: a square wave.
            (0..2 * rate)
                .map(|i| if (i / 40) % 2 == 0 { 32767.0 } else { -32768.0 })
                .collect(),
            // Each sample the opposite of the one before.
            (0..2 * rate)
                .map(|i| if i % 2 == 0 { 32767.0 } else { -32768.0 })
                .collect(),
            // Held at the top.
            vec![32767.0; 2 * rate],
            // Noise at full scale.
            noise(2 * rate, &mut 3, 32767.0),
        ];
        for (n, shape) in shapes.iter().enumerate() {
            let mut denoiser = Denoiser::new();
            for (i, chunk) in shape.chunks(len).enumerate() {
                let mut frame = chunk.to_vec();
                denoiser.process((i / 7) % 3 != 0, rate, &mut frame);
                assert!(
                    frame.iter().all(|x| x.is_finite() && x.abs() <= 65_536.0),
                    "{rate} Hz, shape {n}, frame {i}: {:?}",
                    &frame[..4]
                );
            }
        }
    }
}
