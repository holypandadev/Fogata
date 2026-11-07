//! Rate changes between a sound device's rate and 48 kHz, by straight lines between
//! samples: enough for a voice, whose sound lies well under either rate's half.

/// Changes the rate of a stream of mono samples.
#[derive(Clone, Debug)]
pub struct Linear {
    /// Input samples per output sample.
    step: f64,
    /// Where the next output sample falls, counted from the last input sample of the call
    /// before (0) to the input of this call (1 on).
    t: f64,
    last: f32,
}

/// The rates a device may be at, from what telephones use to what studios do: anything
/// else is taken as the nearest, so a rate a device misreports never makes a step that
/// turns a few samples into millions, or none.
const RATES: std::ops::RangeInclusive<u32> = 8_000..=384_000;

impl Linear {
    pub fn new(from: u32, to: u32) -> Self {
        let rate = |r: u32| f64::from(r.clamp(*RATES.start(), *RATES.end()));
        Linear {
            step: rate(from) / rate(to),
            t: 1.0,
            last: 0.0,
        }
    }

    /// The rates are the same: samples pass as they are.
    pub fn same(&self) -> bool {
        self.step == 1.0
    }

    /// Changes `input` and adds it to `out`.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.same() {
            out.extend_from_slice(input);
            return;
        }
        let n = input.len();
        let at = |k: usize| if k == 0 { self.last } else { input[k - 1] };
        while self.t <= n as f64 {
            let i = self.t.floor() as usize;
            let f = (self.t - i as f64) as f32;
            let a = at(i);
            let b = if i < n { at(i + 1) } else { a };
            out.push(a + (b - a) * f);
            self.t += self.step;
        }
        self.t -= n as f64;
        if let Some(&last) = input.last() {
            self.last = last;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_follow_the_rates_in_any_pieces() {
        for (from, to) in [
            (44_100, 48_000),
            (48_000, 44_100),
            (96_000, 48_000),
            (16_000, 48_000),
        ] {
            let input: Vec<f32> = (0..from).map(|i| (i as f32 * 0.01).sin()).collect();
            let mut whole = vec![];
            Linear::new(from, to).push(&input, &mut whole);
            let mut pieces = vec![];
            let mut r = Linear::new(from, to);
            for chunk in input.chunks(333) {
                r.push(chunk, &mut pieces);
            }
            // Fractions add up differently in pieces: a sample more or less at most.
            assert!(whole.len().abs_diff(pieces.len()) <= 1);
            assert!(whole.iter().zip(&pieces).all(|(a, b)| (a - b).abs() < 1e-3));
            assert!(
                (whole.len() as i64 - i64::from(to)).abs() <= 4,
                "{from}->{to}: {}",
                whole.len()
            );
        }
    }

    #[test]
    fn rates_out_of_range_are_taken_as_the_nearest() {
        let input = vec![0.5f32; 480];
        for (from, to) in [(0, 48_000), (1, 48_000), (u32::MAX, 48_000), (48_000, 0)] {
            let mut out = vec![];
            Linear::new(from, to).push(&input, &mut out);
            assert!(
                out.len() <= input.len() * 48 + 1,
                "{from}->{to}: {}",
                out.len()
            );
            assert!(out.iter().all(|s| s.is_finite()));
        }
    }

    #[test]
    fn the_same_rate_passes_samples_through() {
        let mut out = vec![];
        Linear::new(48_000, 48_000).push(&[0.1, 0.2], &mut out);
        assert_eq!(out, vec![0.1, 0.2]);
    }
}
