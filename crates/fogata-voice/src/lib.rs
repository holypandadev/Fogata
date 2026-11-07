//! Voice messages: what a microphone records, as Opus in an Ogg file that other players
//! read too (RFC 7845), the levels the message shows as bars, and playing it back at 1×,
//! 1.5× or 2× without raising the voice.
//!
//! - [`ogg`] writes the file and reads one from a friend: strictly, as untrusted input,
//!   with caps on its size, its packets and its length.
//! - [`Recorder`] takes 48 kHz mono sound, encodes it 20 ms at a time and keeps its levels.
//! - [`Player`] decodes a file as it plays and stretches it in time ([`wsola`]).
//! - [`Linear`] changes the rate of sound between a device's and 48 kHz.

#![forbid(unsafe_code)]

#[cfg(feature = "fuzzing")]
pub mod fuzzing;
pub mod ogg;
mod play;
mod record;
mod resample;
pub mod wsola;

pub use play::Player;
pub use record::{Recorder, Recording};
pub use resample::Linear;

/// Samples a second of everything here: what Opus works at.
pub const RATE: u32 = 48_000;

/// Samples in one encoded packet: 20 ms.
pub const FRAME: usize = 960;

/// What the voice is encoded at, in bits a second, as variable bit rate kept near it.
pub const BITRATE: i32 = 24_000;

/// Longest message: 15 minutes.
pub const MAX_MS: u64 = 15 * 60 * 1000;

/// Longest message read from a friend: 15 minutes and a second, as the last packet may
/// end a little after the end.
pub const MAX_READ_MS: u64 = MAX_MS + 1000;

/// Biggest file read: 15 minutes at 32 kbit/s with the file's own bytes, rounded up. A
/// message recorded here at 24 kbit/s is about 2.8 MB at most.
pub const MAX_FILE_BYTES: usize = 4 << 20;

/// Bars a message shows, from its start to its end.
pub const BARS: usize = 40;

/// What can be wrong with a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Error {}

/// A level from 0 to 255 for `rms` (of samples from -1 to 1): -60 dB and below is 0, full
/// scale 255, in steps of equal loudness.
pub fn level(rms: f32) -> u8 {
    if rms.is_nan() || rms <= 0.0 {
        return 0;
    }
    let db = 20.0 * rms.log10();
    ((db + 60.0) / 60.0 * 255.0).clamp(0.0, 255.0) as u8
}
