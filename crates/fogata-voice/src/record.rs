//! Encoding what the microphone records: 20 ms at a time to Opus, keeping each 20 ms's
//! level for the bars.

use opus::{Application, Bitrate, Channels, Encoder};

use crate::{BARS, BITRATE, Error, FRAME, MAX_MS, RATE, level, ogg};

/// Most samples a message takes: 15 minutes.
const MAX_SAMPLES: u64 = MAX_MS * RATE as u64 / 1000;

/// Largest packet the encoder may make: Opus's own limit for one frame.
const PACKET_BYTES: usize = 1275;

/// A message being recorded.
pub struct Recorder {
    encoder: Encoder,
    /// Samples taken that do not make a whole packet yet.
    pending: Vec<f32>,
    /// Where a packet is encoded before it is kept at its own size: kept at the most a
    /// packet may take, 15 minutes of them would hold about 57 MB instead of about 3.
    encoded: Vec<u8>,
    packets: Vec<Vec<u8>>,
    /// The level of each 20 ms taken.
    levels: Vec<u8>,
    /// Samples taken.
    samples: u64,
    /// Samples the decoder gives before the sound: the encoder's look-ahead.
    pre_skip: u16,
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("samples", &self.samples)
            .field("packets", &self.packets.len())
            .finish_non_exhaustive()
    }
}

/// A finished message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording {
    /// The Ogg Opus file.
    pub file: Vec<u8>,
    pub duration_ms: u64,
    /// [`BARS`] levels from its start to its end, the loudest at 255.
    pub bars: Vec<u8>,
}

fn opus_error(_: opus::Error) -> Error {
    Error("the voice encoder failed")
}

impl Recorder {
    /// A new message: mono at 48 kHz, tuned for voice, near [`BITRATE`].
    pub fn new() -> Result<Self, Error> {
        let mut encoder =
            Encoder::new(RATE, Channels::Mono, Application::Voip).map_err(opus_error)?;
        encoder
            .set_bitrate(Bitrate::Bits(BITRATE))
            .map_err(opus_error)?;
        encoder.set_vbr(true).map_err(opus_error)?;
        // Kept near the rate, so 15 minutes stay well under what a friend's copy reads.
        encoder.set_vbr_constraint(true).map_err(opus_error)?;
        let lookahead = encoder.get_lookahead().map_err(opus_error)?;
        Ok(Recorder {
            encoder,
            pending: Vec::with_capacity(FRAME),
            encoded: vec![0; PACKET_BYTES],
            packets: Vec::new(),
            levels: Vec::new(),
            samples: 0,
            pre_skip: u16::try_from(lookahead.max(0)).unwrap_or(0),
        })
    }

    /// Takes 48 kHz mono samples (from -1 to 1); returns how many it took, fewer once the
    /// message is 15 minutes long.
    pub fn push(&mut self, pcm: &[f32]) -> Result<usize, Error> {
        let room = usize::try_from(MAX_SAMPLES - self.samples).unwrap_or(usize::MAX);
        let taken = &pcm[..pcm.len().min(room)];
        for &s in taken {
            self.pending.push(if s.is_finite() {
                s.clamp(-1.0, 1.0)
            } else {
                0.0
            });
            if self.pending.len() == FRAME {
                self.encode_pending()?;
            }
        }
        self.samples += taken.len() as u64;
        Ok(taken.len())
    }

    fn encode_pending(&mut self) -> Result<(), Error> {
        let squares: f32 = self.pending.iter().map(|s| s * s).sum();
        self.levels
            .push(level((squares / self.pending.len().max(1) as f32).sqrt()));
        self.pending.resize(FRAME, 0.0);
        self.encode_frame()?;
        self.pending.clear();
        Ok(())
    }

    /// Encodes the frame in `pending` into a packet of its own size.
    fn encode_frame(&mut self) -> Result<(), Error> {
        let n = self
            .encoder
            .encode_float(&self.pending, &mut self.encoded)
            .map_err(opus_error)?;
        self.packets.push(self.encoded[..n].to_vec());
        Ok(())
    }

    /// The message reached 15 minutes: nothing more goes in.
    pub fn full(&self) -> bool {
        self.samples >= MAX_SAMPLES
    }

    pub fn duration_ms(&self) -> u64 {
        self.samples * 1000 / RATE as u64
    }

    /// The level of each 20 ms recorded so far.
    pub fn levels(&self) -> &[u8] {
        &self.levels
    }

    /// The finished file, in a stream numbered `serial`; `None` when nothing was taken.
    pub fn finish(mut self, serial: u32) -> Result<Option<Recording>, Error> {
        if self.samples == 0 {
            return Ok(None);
        }
        if !self.pending.is_empty() {
            self.encode_pending()?;
            // A level for a part of a frame would count its silence.
            self.levels.pop();
        }
        // The decoder gives the look-ahead first: frames of silence after the end cover it.
        let end = u64::from(self.pre_skip) + self.samples;
        while ((self.packets.len() * FRAME) as u64) < end {
            self.pending.clear();
            self.pending.resize(FRAME, 0.0);
            self.encode_frame()?;
        }
        self.pending.clear();
        let file = ogg::write(&self.packets, self.pre_skip, self.samples, serial);
        Ok(Some(Recording {
            file,
            duration_ms: self.duration_ms(),
            bars: bars(&self.levels, BARS),
        }))
    }
}

/// Levels a bar spans, from the loudest down: 36 dB.
const BAR_RANGE: u32 = 36 * 255 / 60;

/// `n` bars from `levels` from start to end: the loudest level of each stretch, the
/// loudest bar at 255 and 36 dB under it at 0, so a voice's syllables and pauses show.
/// Fewer levels than bars repeat.
pub fn bars(levels: &[u8], n: usize) -> Vec<u8> {
    if levels.is_empty() {
        return vec![0; n];
    }
    let raw: Vec<u8> = (0..n)
        .map(|i| {
            let from = i * levels.len() / n;
            let to = ((i + 1) * levels.len() / n).max(from + 1);
            levels[from..to.min(levels.len())]
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
        })
        .collect();
    let top = u32::from(raw.iter().copied().max().unwrap_or(0));
    if top == 0 {
        return raw;
    }
    let floor = top.saturating_sub(BAR_RANGE);
    let span = top - floor;
    raw.iter()
        .map(|&b| (u32::from(b).saturating_sub(floor) * 255 / span) as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hertz: f32, seconds: f32, amp: f32) -> Vec<f32> {
        (0..(seconds * RATE as f32) as usize)
            .map(|i| amp * (2.0 * std::f32::consts::PI * hertz * i as f32 / RATE as f32).sin())
            .collect()
    }

    #[test]
    fn a_recording_reads_back_with_its_length() {
        let mut r = Recorder::new().unwrap();
        let tone = sine(440.0, 2.345, 0.5);
        // In uneven pieces, as a device hands them.
        for chunk in tone.chunks(441) {
            assert_eq!(r.push(chunk).unwrap(), chunk.len());
        }
        let rec = r.finish(5).unwrap().unwrap();
        assert_eq!(rec.duration_ms, 2345);
        assert_eq!(rec.bars.len(), BARS);
        let read = ogg::read(&rec.file).unwrap();
        assert_eq!(read.samples, tone.len() as u64);
        assert_eq!(read.duration_ms(), 2345);
        // 24 kbit/s near enough: 2.3 s in about 7 kB.
        assert!(
            (4_000..12_000).contains(&rec.file.len()),
            "{}",
            rec.file.len()
        );
    }

    /// Each packet is kept at its own size: a long recording holds about what its file
    /// will, not the most a packet may take for each 20 ms.
    #[test]
    fn packets_are_kept_at_their_own_size() {
        let mut r = Recorder::new().unwrap();
        r.push(&sine(300.0, 10.0, 0.4)).unwrap();
        let bytes: usize = r.packets.iter().map(Vec::len).sum();
        let held: usize = r.packets.iter().map(Vec::capacity).sum();
        assert!(held <= bytes + bytes / 4, "{held} held for {bytes} bytes");
    }

    #[test]
    fn nothing_recorded_is_no_file() {
        assert_eq!(Recorder::new().unwrap().finish(1).unwrap(), None);
    }

    #[test]
    fn recording_stops_at_15_minutes() {
        let mut r = Recorder::new().unwrap();
        let second = vec![0.01f32; RATE as usize];
        let mut taken = 0;
        for _ in 0..15 * 60 {
            taken += r.push(&second).unwrap();
        }
        assert!(r.full());
        assert_eq!(r.push(&second).unwrap(), 0);
        assert_eq!(taken as u64, MAX_SAMPLES);
        let rec = r.finish(9).unwrap().unwrap();
        assert_eq!(rec.duration_ms, MAX_MS);
        let read = ogg::read(&rec.file).unwrap();
        assert_eq!(read.duration_ms(), MAX_MS);
        assert!(rec.file.len() < crate::MAX_FILE_BYTES);
    }

    #[test]
    fn bars_follow_the_loudness() {
        let mut levels = vec![10u8; 100];
        levels[50..60].fill(200);
        levels[70..80].fill(160);
        let b = bars(&levels, 10);
        assert_eq!(b.len(), 10);
        assert_eq!(b[5], 255);
        // 40 levels under the loudest are about 9 dB: well up the bar.
        assert!((150..190).contains(&b[7]), "{}", b[7]);
        assert_eq!(b[0], 0);
        // Fewer levels than bars; a quiet message is as tall as a loud one.
        assert_eq!(bars(&[100, 200], 4), vec![88, 88, 255, 255]);
        assert_eq!(bars(&[20, 30], 2), vec![170, 255]);
        assert_eq!(bars(&[], 3), vec![0, 0, 0]);
        assert_eq!(level(1.0), 255);
        assert_eq!(level(0.0), 0);
        assert_eq!(level(f32::NAN), 0);
        assert!((120..135).contains(&level(0.032)));
    }
}
