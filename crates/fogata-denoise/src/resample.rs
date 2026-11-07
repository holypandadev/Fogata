//! Rate changes between 16 or 32 kHz and 48 kHz, one 10 ms frame at a time: a polyphase
//! low-pass filter (a Kaiser-windowed sinc of [`SPAN`] taps: 2 ms between 16 and 48 kHz,
//! 1 ms between 32 and 48 kHz), so no frame waits for the next.

/// Taps of the low-pass filter, at the rate between the two steps: 48 kHz between 16 and
/// 48 kHz (2 ms), 96 kHz between 32 and 48 kHz (1 ms). The sound is delayed by half that.
const SPAN: usize = 96;

/// Kaiser window shape: about 80 dB of attenuation outside the pass band.
const BETA: f64 = 8.0;

/// Changes the rate of a stream from `from` to `to` (one of them 48 kHz, the other 16 or
/// 32 kHz).
#[derive(Debug)]
pub(crate) struct Resampler {
    /// Up by `up`, then down by `down`.
    up: usize,
    down: usize,
    /// Taps per phase.
    taps: usize,
    /// Filter by phase: `phases[p * taps + j]`.
    phases: Vec<f32>,
    /// The last `taps - 1` input samples, then the frame being changed.
    buf: Vec<f32>,
}

impl Resampler {
    pub(crate) fn new(from: usize, to: usize) -> Self {
        let g = gcd(from, to);
        let (up, down) = (to / g, from / g);
        let taps = SPAN.div_ceil(up.max(1)).max(2);
        let len = taps * up;
        // Pass up to 90 % of the lower rate's half, at the rate between the two steps.
        let cutoff = 0.45 * from.min(to) as f64 / (from * up) as f64;
        let centre = (len - 1) as f64 / 2.0;
        let norm = bessel_i0(BETA);
        let proto: Vec<f64> = (0..len)
            .map(|k| {
                let x = k as f64 - centre;
                let sinc = if x == 0.0 {
                    2.0 * cutoff
                } else {
                    (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
                };
                let r = x / (centre + 0.5);
                let window = bessel_i0(BETA * (1.0 - r * r).max(0.0).sqrt()) / norm;
                sinc * window
            })
            .collect();
        // Each phase passes a steady level as it is.
        let mut phases = vec![0.0; len];
        for p in 0..up {
            let sum: f64 = (0..taps).map(|j| proto[p + j * up]).sum();
            for j in 0..taps {
                phases[p * taps + j] = (proto[p + j * up] / sum) as f32;
            }
        }
        Resampler {
            up,
            down,
            taps,
            phases,
            buf: vec![0.0; taps - 1],
        }
    }

    /// Forgets the samples heard.
    pub(crate) fn reset(&mut self) {
        self.buf.clear();
        self.buf.resize(self.taps - 1, 0.0);
    }

    /// `input.len() * up` must equal `output.len() * down`.
    pub(crate) fn process(&mut self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len() * self.up, output.len() * self.down);
        let keep = self.taps - 1;
        self.buf.extend_from_slice(input);
        for (n, out) in output.iter_mut().enumerate() {
            let t = n * self.down;
            let (i, p) = (t / self.up, t % self.up);
            let newest = keep + i;
            let filter = &self.phases[p * self.taps..(p + 1) * self.taps];
            *out = filter
                .iter()
                .enumerate()
                .map(|(j, h)| h * self.buf[newest - j])
                .sum();
        }
        let end = self.buf.len();
        self.buf.copy_within(end - keep.., 0);
        self.buf.truncate(keep);
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The modified Bessel function of the first kind, order 0.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let half = x / 2.0;
    for k in 1..50 {
        term *= (half / k as f64) * (half / k as f64);
        sum += term;
        if term < sum * 1e-12 {
            break;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: usize, hz: f64, frames: usize) -> Vec<f32> {
        (0..frames * rate / 100)
            .map(|i| {
                (10000.0 * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin()) as f32
            })
            .collect()
    }

    fn run(r: &mut Resampler, from: usize, to: usize, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        let mut frame = vec![0.0; to / 100];
        for chunk in input.chunks(from / 100) {
            r.process(chunk, &mut frame);
            out.extend_from_slice(&frame);
        }
        out
    }

    fn rms(xs: &[f32]) -> f64 {
        (xs.iter().map(|&x| x as f64 * x as f64).sum::<f64>() / xs.len() as f64).sqrt()
    }

    /// A tone in the pass band keeps its level both ways, and a tone the lower rate cannot
    /// hold is removed on the way down.
    #[test]
    fn levels() {
        for low in [16000, 32000] {
            for (from, to) in [(low, 48000), (48000, low)] {
                let mut r = Resampler::new(from, to);
                let out = run(&mut r, from, to, &tone(from, 1000.0, 50));
                let steady = &out[out.len() / 2..];
                let ratio = 20.0 * (rms(steady) / (10000.0 / 2f64.sqrt())).log10();
                assert!(ratio.abs() < 0.1, "{from}->{to}: {ratio:.2} dB");
            }
            let mut r = Resampler::new(48000, low);
            let high = low as f64 * 0.6;
            let out = run(&mut r, 48000, low, &tone(48000, high, 50));
            let left = 20.0 * (rms(&out[out.len() / 2..]) / 7071.0).log10();
            assert!(left < -60.0, "{high} Hz to {low}: {left:.1} dB");
        }
    }

    /// Up then down gives the same tone back, about 2 ms later.
    #[test]
    fn round_trip() {
        for low in [16000, 32000] {
            let mut up = Resampler::new(low, 48000);
            let mut down = Resampler::new(48000, low);
            let hz = 100.0;
            let wide = run(&mut up, low, 48000, &tone(low, hz, 50));
            let back = run(&mut down, 48000, low, &wide);
            // The tone that fits best, by its sine and cosine parts.
            let w = 2.0 * std::f64::consts::PI * hz / low as f64;
            let range = low / 10..low / 2;
            let (mut s, mut c) = (0.0, 0.0);
            for i in range.clone() {
                s += back[i] as f64 * (w * i as f64).sin();
                c += back[i] as f64 * (w * i as f64).cos();
            }
            let n = range.len() as f64 / 2.0;
            let (s, c) = (s / n, c / n);
            let err: Vec<f32> = range
                .clone()
                .map(|i| back[i] - (s * (w * i as f64).sin() + c * (w * i as f64).cos()) as f32)
                .collect();
            let level = (s * s + c * c).sqrt();
            let snr = 20.0 * (level / 2f64.sqrt() / rms(&err)).log10();
            let delay_ms = (-c).atan2(s) / w / low as f64 * 1000.0;
            assert!(
                (level / 10000.0 - 1.0).abs() < 0.01,
                "{low}: level {level:.0}"
            );
            assert!(snr > 60.0, "{low}: {snr:.1} dB");
            assert!(delay_ms > 0.0 && delay_ms < 3.0, "{low}: {delay_ms:.2} ms");
        }
    }
}
