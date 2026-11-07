//! Rings played on instruments rather than on the sounds' mallet: a bar of four chords, a
//! low note under each and a tune over them, played with a little of a player's unevenness
//! in time and touch. The instruments are made from how real ones sound: a marimba's bar
//! (its first three modes, tuned about 1:4:10, under a soft mallet), a vibraphone's (the
//! same, longer, with the slow tremolo of its fans) over a soft pad, a nylon string plucked
//! by a finger (Karplus-Strong, in tune to a fraction of a sample, through a guitar's body)
//! and an electric piano's tine (a pure note bent by its pickup, more the harder it is hit).

use std::f64::consts::{PI, TAU};

use super::{Biquad, Sound, Variant};

/// A beat, in seconds. A song is a bar of eight, and the ring comes back every bar
/// ([`super::RING_EVERY`]).
pub(super) const BEAT: f64 = 0.4;
pub(super) const SONG_EVERY: f64 = 8.0 * BEAT;

/// How far a note may fall from the beat, in seconds, either way.
const UNEVEN: f64 = 0.010;
/// How far a note's touch may differ from the written one, as a fraction.
const UNEVEN_TOUCH: f64 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Instrument {
    Marimba,
    Vibraphone,
    /// A soft, sustained chord under the vibraphone.
    Pad,
    Guitar,
    WarmGuitar,
    Harp,
    FeltPiano,
    Keys,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Bass,
    Chord,
    Tune,
    /// Played on the pad, whatever the song's instrument.
    Pad,
}

/// A note of a song: its beat, its key (MIDI: 69 is A4, 440 Hz), how many beats it is
/// held, how hard it is played (0 to 1) and its part.
type Event = (f64, u8, f64, f64, Part);

pub(super) struct Song {
    instrument: Instrument,
    events: &'static [Event],
    /// Where the song's unevenness starts, so it is the same every time.
    seed: u64,
}

pub(super) fn song(sound: Sound, variant: Variant) -> Option<Song> {
    let (instrument, events, seed) = match (sound, variant) {
        (Sound::Ring, Variant::D) => (Instrument::Marimba, &WOOD[..], 11),
        (Sound::Ring, Variant::E) => (Instrument::Vibraphone, &GLOW[..], 23),
        (Sound::Ring, Variant::F) => (Instrument::Guitar, &PORCH[..], 37),
        (Sound::Ring, Variant::G) => (Instrument::Keys, &KEYS[..], 41),
        // F's song, the same in every note and its unevenness, on other instruments.
        (Sound::Ring, Variant::H) => (Instrument::WarmGuitar, &PORCH[..], 37),
        (Sound::Ring, Variant::I) => (Instrument::Harp, &PORCH[..], 37),
        (Sound::Ring, Variant::J) => (Instrument::FeltPiano, &PORCH[..], 37),
        _ => return None,
    };
    Some(Song {
        instrument,
        events,
        seed,
    })
}

use Part::{Bass, Chord, Pad, Tune};

/// On a marimba, A, F#m, D, E: a tune up the chord and back down, a dyad after each low
/// note.
const WOOD: [Event; 25] = [
    (0.0, 57, 2.0, 0.75, Bass),
    (2.0, 54, 2.0, 0.75, Bass),
    (4.0, 50, 2.0, 0.75, Bass),
    (6.0, 52, 2.0, 0.75, Bass),
    (1.0, 61, 1.0, 0.45, Chord),
    (1.0, 64, 1.0, 0.45, Chord),
    (3.0, 61, 1.0, 0.45, Chord),
    (3.0, 66, 1.0, 0.45, Chord),
    (5.0, 62, 1.0, 0.45, Chord),
    (5.0, 66, 1.0, 0.45, Chord),
    (7.0, 64, 1.0, 0.45, Chord),
    (7.0, 68, 1.0, 0.45, Chord),
    (0.0, 73, 0.5, 0.75, Tune),
    (0.5, 76, 0.5, 0.7, Tune),
    (1.0, 81, 1.0, 0.85, Tune),
    (2.0, 78, 0.5, 0.75, Tune),
    (2.5, 76, 0.5, 0.7, Tune),
    (3.0, 73, 1.0, 0.8, Tune),
    (4.0, 74, 0.5, 0.75, Tune),
    (4.5, 78, 0.5, 0.7, Tune),
    (5.0, 81, 0.5, 0.85, Tune),
    (5.5, 78, 0.5, 0.7, Tune),
    (6.0, 80, 0.5, 0.75, Tune),
    (6.5, 76, 0.5, 0.7, Tune),
    (7.0, 71, 1.0, 0.8, Tune),
];

/// On a vibraphone over a pad, A, F#m, Bm, E7: long notes that ring into each other.
const GLOW: [Event; 27] = [
    (0.0, 45, 2.0, 0.6, Pad),
    (0.0, 57, 2.0, 0.6, Pad),
    (0.0, 61, 2.0, 0.6, Pad),
    (0.0, 64, 2.0, 0.6, Pad),
    (2.0, 42, 2.0, 0.6, Pad),
    (2.0, 54, 2.0, 0.6, Pad),
    (2.0, 57, 2.0, 0.6, Pad),
    (2.0, 61, 2.0, 0.6, Pad),
    (4.0, 47, 2.0, 0.6, Pad),
    (4.0, 54, 2.0, 0.6, Pad),
    (4.0, 59, 2.0, 0.6, Pad),
    (4.0, 62, 2.0, 0.6, Pad),
    (6.0, 40, 2.0, 0.6, Pad),
    (6.0, 56, 2.0, 0.6, Pad),
    (6.0, 59, 2.0, 0.6, Pad),
    (6.0, 62, 2.0, 0.6, Pad),
    (0.0, 76, 1.5, 0.8, Tune),
    (1.0, 78, 1.0, 0.65, Tune),
    (1.5, 76, 1.0, 0.7, Tune),
    (2.0, 73, 2.0, 0.8, Tune),
    (3.5, 69, 1.0, 0.65, Tune),
    (4.0, 71, 1.0, 0.7, Tune),
    (4.5, 74, 1.0, 0.7, Tune),
    (5.0, 78, 1.5, 0.8, Tune),
    (6.0, 76, 1.5, 0.8, Tune),
    (7.0, 74, 1.0, 0.65, Tune),
    (7.5, 71, 1.5, 0.7, Tune),
];

/// On a nylon-string guitar, A, E, F#m, D: the low string, two strings together, then the
/// tune on the top ones.
const PORCH: [Event; 20] = [
    (0.0, 45, 2.0, 0.75, Bass),
    (0.5, 57, 1.5, 0.5, Chord),
    (0.5, 64, 1.5, 0.5, Chord),
    (1.0, 73, 1.0, 0.8, Tune),
    (1.5, 76, 0.7, 0.75, Tune),
    (2.0, 40, 2.0, 0.75, Bass),
    (2.5, 59, 1.5, 0.5, Chord),
    (2.5, 68, 1.5, 0.5, Chord),
    (3.0, 74, 1.0, 0.8, Tune),
    (3.5, 71, 0.7, 0.75, Tune),
    (4.0, 42, 2.0, 0.75, Bass),
    (4.5, 57, 1.5, 0.5, Chord),
    (4.5, 61, 1.5, 0.5, Chord),
    (5.0, 73, 1.0, 0.8, Tune),
    (5.5, 69, 0.7, 0.75, Tune),
    (6.0, 50, 2.0, 0.75, Bass),
    (6.5, 57, 1.5, 0.5, Chord),
    (6.5, 66, 1.5, 0.5, Chord),
    (7.0, 69, 1.0, 0.8, Tune),
    (7.5, 71, 0.7, 0.75, Tune),
];

/// On an electric piano, Amaj7, F#m7, Dmaj7, E7: each chord held, touched again softly on
/// the off-beat before the next, and a tune over them.
const KEYS: [Event; 39] = [
    (0.0, 45, 1.8, 0.7, Bass),
    (2.0, 42, 1.8, 0.7, Bass),
    (4.0, 50, 1.8, 0.7, Bass),
    (6.0, 40, 1.8, 0.7, Bass),
    (0.0, 61, 1.3, 0.55, Chord),
    (0.0, 64, 1.3, 0.55, Chord),
    (0.0, 68, 1.3, 0.55, Chord),
    (1.5, 61, 0.4, 0.35, Chord),
    (1.5, 64, 0.4, 0.35, Chord),
    (1.5, 68, 0.4, 0.35, Chord),
    (2.0, 57, 1.3, 0.55, Chord),
    (2.0, 61, 1.3, 0.55, Chord),
    (2.0, 64, 1.3, 0.55, Chord),
    (3.5, 57, 0.4, 0.35, Chord),
    (3.5, 61, 0.4, 0.35, Chord),
    (3.5, 64, 0.4, 0.35, Chord),
    (4.0, 54, 1.3, 0.55, Chord),
    (4.0, 57, 1.3, 0.55, Chord),
    (4.0, 61, 1.3, 0.55, Chord),
    (5.5, 54, 0.4, 0.35, Chord),
    (5.5, 57, 0.4, 0.35, Chord),
    (5.5, 61, 0.4, 0.35, Chord),
    (6.0, 56, 1.3, 0.55, Chord),
    (6.0, 59, 1.3, 0.55, Chord),
    (6.0, 62, 1.3, 0.55, Chord),
    (7.5, 56, 0.4, 0.35, Chord),
    (7.5, 59, 0.4, 0.35, Chord),
    (7.5, 62, 0.4, 0.35, Chord),
    (0.0, 73, 1.0, 0.8, Tune),
    (1.0, 71, 0.5, 0.65, Tune),
    (1.5, 73, 0.5, 0.7, Tune),
    (2.0, 76, 1.5, 0.85, Tune),
    (3.5, 73, 0.5, 0.65, Tune),
    (4.0, 78, 1.0, 0.85, Tune),
    (5.0, 76, 0.5, 0.65, Tune),
    (5.5, 73, 0.5, 0.7, Tune),
    (6.0, 71, 1.0, 0.8, Tune),
    (7.0, 74, 0.5, 0.65, Tune),
    (7.5, 71, 0.5, 0.7, Tune),
];

/// How loud each part is in the mix on `instrument`: the tune over the low notes, the
/// chords and the pad, whose notes sound all the time, under both (about 8 to 10 dB under
/// the whole). A plucked string's chords die away sooner than the others', so they are
/// plucked louder, and its low strings ring fuller, so they are plucked softer.
const fn part_level(instrument: Instrument, part: Part) -> f64 {
    match (instrument, part) {
        (_, Part::Tune) => 1.0,
        (_, Part::Pad) => 0.3,
        (Instrument::Guitar, Part::Bass) => 0.6,
        (Instrument::WarmGuitar, Part::Bass) => 0.45,
        (Instrument::Harp, Part::Bass) => 0.6,
        (_, Part::Bass) => 0.7,
        (Instrument::Guitar, Part::Chord) => 0.9,
        (Instrument::WarmGuitar, Part::Chord) => 0.6,
        (Instrument::Harp, Part::Chord) => 0.7,
        (Instrument::FeltPiano, Part::Chord) => 0.55,
        (Instrument::Marimba, Part::Chord) => 0.6,
        (_, Part::Chord) => 0.4,
    }
}

/// How much longer than written a note is let ring on `instrument`, in seconds: a harp's
/// strings and a piano under its pedal ring on into the next notes.
const fn linger(instrument: Instrument) -> f64 {
    match instrument {
        Instrument::Harp => BEAT,
        Instrument::FeltPiano => 0.5 * BEAT,
        Instrument::WarmGuitar => 0.25 * BEAT,
        _ => 0.0,
    }
}

impl Song {
    /// The song once, mono, at `rate` samples per second, with the tails of its last notes.
    pub(super) fn play(&self, rate: f64) -> Vec<f64> {
        let mut uneven = Noise::new(self.seed);
        let mut noise = Noise::new(self.seed ^ 0x5eed);
        let mut out: Vec<f64> = Vec::new();
        for &(beat, key, hold, touch, part) in self.events {
            // Nothing starts before the bar does.
            let late = if beat == 0.0 {
                uneven.next().abs()
            } else {
                uneven.next()
            };
            let at = beat * BEAT + late * UNEVEN;
            let touch = (touch * (1.0 + UNEVEN_TOUCH * uneven.next())).clamp(0.05, 1.0);
            let instrument = if part == Part::Pad {
                Instrument::Pad
            } else {
                self.instrument
            };
            let note = play_note(
                instrument,
                key_hertz(key),
                touch,
                hold * BEAT + linger(self.instrument),
                at,
                rate,
                &mut noise,
            );
            let first = (at * rate) as usize;
            if out.len() < first + note.len() {
                out.resize(first + note.len(), 0.0);
            }
            let level = part_level(self.instrument, part) * touch;
            for (slot, s) in out[first..].iter_mut().zip(&note) {
                *slot += level * s;
            }
        }
        out
    }
}

fn key_hertz(key: u8) -> f64 {
    440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0)
}

/// One note on `instrument`, held for `hold` seconds, starting `at` seconds into the song,
/// at about full scale for a full `touch`: it rises without a click and ends at rest.
pub(super) fn play_note(
    instrument: Instrument,
    hertz: f64,
    touch: f64,
    hold: f64,
    at: f64,
    rate: f64,
    noise: &mut Noise,
) -> Vec<f64> {
    let mut note = match instrument {
        Instrument::Marimba => marimba(hertz, touch, rate, noise),
        Instrument::Vibraphone => vibraphone(hertz, touch, hold, at, rate),
        Instrument::Pad => pad(hertz, hold, rate),
        Instrument::Guitar => strung(&NYLON, hertz, touch, hold, rate, noise),
        Instrument::WarmGuitar => strung(&WARM_NYLON, hertz, touch, hold, rate, noise),
        Instrument::Harp => strung(&HARP, hertz, touch, hold, rate, noise),
        Instrument::FeltPiano => felt_piano(hertz, touch, hold, rate, noise),
        Instrument::Keys => keys(hertz, touch, hold, rate),
    };
    let fade = ((0.02 * rate) as usize).min(note.len());
    let start = note.len() - fade;
    for (k, s) in note[start..].iter_mut().enumerate() {
        *s *= 0.5 + 0.5 * (PI * k as f64 / fade as f64).cos();
    }
    note
}

/// A raised-cosine rise over `seconds`.
fn rise(t: f64, seconds: f64) -> f64 {
    if t < seconds {
        0.5 - 0.5 * (PI * t / seconds).cos()
    } else {
        1.0
    }
}

/// 1 while a note is held, then falling to a third every `release` seconds, as under a
/// damper.
fn damper(t: f64, hold: f64, release: f64) -> f64 {
    if t < hold {
        1.0
    } else {
        (-(t - hold) / release).exp()
    }
}

/// How much of a mode at `hertz` a soft mallet sets ringing: its felt, in contact for a
/// few milliseconds, barely moves what is much higher.
fn felt(hertz: f64) -> f64 {
    1.0 / (1.0 + (hertz / 3_000.0).powi(2))
}

/// Modes of a bar: multiple of the note, level, and how much sooner than the note it
/// fades. A marimba's are tuned about 1:4:10, a vibraphone's exactly.
const MARIMBA_MODES: [(f64, f64, f64); 3] =
    [(1.0, 1.0, 1.0), (3.97, 0.25, 0.22), (9.83, 0.06, 0.07)];
const VIBRAPHONE_MODES: [(f64, f64, f64); 3] =
    [(1.0, 1.0, 1.0), (4.0, 0.18, 0.2), (10.0, 0.035, 0.05)];

/// A bar's modes, each `(hertz, level, seconds to fall to a third)`, under a mallet that
/// brings out the higher ones the harder it strikes.
fn bar_modes(
    modes: &[(f64, f64, f64)],
    hertz: f64,
    touch: f64,
    ring: f64,
    rate: f64,
) -> Vec<(f64, f64, f64)> {
    let hard = 0.4 + 0.6 * touch;
    modes
        .iter()
        .enumerate()
        .filter(|(_, (multiple, _, _))| hertz * multiple < 0.45 * rate)
        .map(|(k, &(multiple, level, sooner))| {
            let level = if k == 0 { level } else { level * hard };
            (
                hertz * multiple,
                level * felt(hertz * multiple),
                ring * sooner,
            )
        })
        .collect()
}

fn marimba(hertz: f64, touch: f64, rate: f64, noise: &mut Noise) -> Vec<f64> {
    // Low bars ring longer, over their resonators.
    let ring = (0.3 * (440.0 / hertz).powf(0.8)).min(1.2);
    let modes = bar_modes(&MARIMBA_MODES, hertz, touch, ring, rate);
    // The mallet's soft knock on the wood: 6 ms of muffled noise.
    let knock_len = (0.006 * rate) as usize;
    let muffle = 1.0 - (-TAU * 1_500.0 / rate).exp();
    let mut low = 0.0;
    let knock: Vec<f64> = (0..knock_len)
        .map(|i| {
            low += muffle * (noise.next() - low);
            0.15 * touch * low * (PI * i as f64 / knock_len as f64).sin()
        })
        .collect();
    let len = (ring * 6.0 * rate) as usize;
    (0..len)
        .map(|i| {
            let t = i as f64 / rate;
            let tone: f64 = modes
                .iter()
                .map(|&(h, level, fall)| level * (-t / fall).exp() * (TAU * h * t).sin())
                .sum();
            rise(t, 0.002) * tone + knock.get(i).copied().unwrap_or(0.0)
        })
        .collect()
}

/// How fast and how deep the vibraphone's fans sway its notes.
const TREMOLO_HERTZ: f64 = 3.6;
const TREMOLO_DEPTH: f64 = 0.3;

fn vibraphone(hertz: f64, touch: f64, hold: f64, at: f64, rate: f64) -> Vec<f64> {
    let ring = (1.1 * (440.0 / hertz).powf(0.4)).min(2.5);
    let modes = bar_modes(&VIBRAPHONE_MODES, hertz, touch, ring, rate);
    let len = ((ring * 6.0).min(hold + 0.7) * rate) as usize;
    (0..len)
        .map(|i| {
            let t = i as f64 / rate;
            // The fans turn all the time, so the sway goes on from note to note.
            let sway = 1.0 - TREMOLO_DEPTH * (0.5 - 0.5 * (TAU * TREMOLO_HERTZ * (at + t)).cos());
            let tone: f64 = modes
                .iter()
                .enumerate()
                .map(|(k, &(h, level, fall))| {
                    let sway = if k == 0 { sway } else { 1.0 };
                    sway * level * (-t / fall).exp() * (TAU * h * t).sin()
                })
                .sum();
            rise(t, 0.003) * damper(t, hold, 0.1) * tone
        })
        .collect()
}

/// A soft pad: two copies of each note a few cents apart, each with harmonics that fall
/// away quickly, swelling in and fading out slowly.
fn pad(hertz: f64, hold: f64, rate: f64) -> Vec<f64> {
    const RELEASE: f64 = 0.6;
    let len = ((hold + RELEASE) * rate) as usize;
    (0..len)
        .map(|i| {
            let t = i as f64 / rate;
            let mut tone = 0.0;
            for (copy, detune) in [(0.0, 0.9965), (1.3, 1.0035)] {
                for k in 1..=6 {
                    let h = hertz * detune * k as f64;
                    if h >= 0.45 * rate {
                        break;
                    }
                    tone += (k as f64).powf(-1.8) * (TAU * h * t + copy * k as f64).sin();
                }
            }
            let fall = if t < hold {
                1.0
            } else {
                0.5 + 0.5 * (PI * (t - hold) / RELEASE).cos()
            };
            // The second copy starts out of phase, so the rise brings the pad in from rest.
            rise(t, 0.35) * fall * 0.5 * tone
        })
        .collect()
}

/// How a plucked string sounds.
struct Strung {
    /// How round the pluck is: above about this many hertz (for the softest touch, and
    /// added for a full one) its shape is smoothed away.
    round: (f64, f64),
    /// Where it is plucked, as a fraction of the string from its end.
    at: f64,
    /// How long its low A rings, in seconds to fall 60 dB; higher strings ring shorter.
    sustain: f64,
    /// Whether the string loses its highs faster on each pass, as a thick or soft string
    /// does.
    dark: bool,
    /// What it is strung on: resonances, each in hertz, sharpness and level against the
    /// string.
    body: &'static [(f64, f64, f64)],
    /// How long the pluck takes to let go of the string, in seconds.
    rise: f64,
    /// How long it rings on once let go, in seconds to fall 60 dB.
    stop: f64,
    /// Above about this many hertz the sound is softened, or not at all if 0.
    tone: f64,
}

/// A nylon string plucked with a fingertip on a guitar: its body's air, and its top.
const NYLON: Strung = Strung {
    round: (500.0, 500.0),
    at: 0.18,
    sustain: 3.0,
    dark: false,
    body: &[(100.0, 8.0, 0.6), (205.0, 10.0, 0.4), (390.0, 8.0, 0.25)],
    rise: 0.0015,
    stop: 0.15,
    tone: 0.0,
};

/// The same, plucked with the soft side of the thumb further from the bridge, on a deeper,
/// fuller body, the strings darker and ringing longer.
const WARM_NYLON: Strung = Strung {
    round: (280.0, 320.0),
    at: 0.27,
    sustain: 4.0,
    dark: true,
    body: &[
        (95.0, 6.0, 0.9),
        (190.0, 7.0, 0.6),
        (280.0, 6.0, 0.4),
        (420.0, 6.0, 0.2),
    ],
    rise: 0.003,
    stop: 0.25,
    tone: 3_000.0,
};

/// A harp's string, plucked softly with the fleshy finger near its middle, ringing long on
/// a light soundboard.
const HARP: Strung = Strung {
    round: (350.0, 350.0),
    at: 0.35,
    sustain: 5.0,
    dark: true,
    body: &[(150.0, 4.0, 0.35), (320.0, 5.0, 0.25)],
    rise: 0.0025,
    stop: 0.4,
    tone: 4_000.0,
};

fn strung(
    how: &Strung,
    hertz: f64,
    touch: f64,
    hold: f64,
    rate: f64,
    noise: &mut Noise,
) -> Vec<f64> {
    // The string goes round a line one period long: each sample is the mean of the
    // string around a period before (half a sample either side, or a whole one weighted
    // to the middle for a dark string), read between samples, which keeps it in tune to a
    // fraction of a sample and loses a little of its highs on each pass, as a string does.
    let period = rate / hertz;
    let whole = (period.floor() as usize).max(2);
    // Low strings ring longer; once let go, a hand stops them.
    let sustain = (how.sustain * (110.0 / hertz).sqrt()).clamp(0.9, how.sustain + 0.5);
    let keep = |seconds: f64| 10f64.powf(-3.0 / (seconds * hertz));
    let (ringing, stopped) = (keep(sustain), keep(how.stop));
    // The pluck: the string's shape when the finger lets go, one period of noise, made
    // round (darker the softer the finger, and smooth across the period's ends) and without
    // what the plucking point cannot set moving.
    let noise: Vec<f64> = (0..whole).map(|_| noise.next()).collect();
    let notch = ((how.at * whole as f64).round() as usize).max(1) % whole.max(1);
    let mut plucked: Vec<f64> = (0..whole)
        .map(|i| noise[i] - noise[(i + whole - notch) % whole])
        .collect();
    let dark = (-TAU * (how.round.0 + how.round.1 * touch) / rate).exp();
    for _ in 0..3 {
        // Twice round the period first, so its end runs smoothly into its start.
        let mut s = 0.0;
        for _ in 0..2 {
            for x in &plucked {
                s = (1.0 - dark) * *x + dark * s;
            }
        }
        for x in &mut plucked {
            s = (1.0 - dark) * *x + dark * s;
            *x = s;
        }
    }
    let mean = plucked.iter().sum::<f64>() / whole.max(1) as f64;
    // Every pluck as strong, whatever shape the noise gave it.
    let strength = (plucked.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / whole as f64)
        .sqrt()
        .max(1e-9);
    let pluck: Vec<f64> = plucked
        .iter()
        .map(|x| 0.35 * (x - mean) / strength)
        .collect();

    let len = (sustain.min(hold + 0.4) * rate) as usize;
    let mut string: Vec<f64> = Vec::with_capacity(len);
    for i in 0..len {
        // Before the note, the string holds the pluck's shape.
        let back = |k: usize| {
            if i >= k {
                string[i - k]
            } else {
                pluck[(i + 3 * whole - k) % whole]
            }
        };
        let read = |late: f64| {
            let k = late.floor();
            let between = late - k;
            (1.0 - between) * back(k as usize) + between * back(k as usize + 1)
        };
        // The hand comes down on the string over about 20 ms.
        let t = i as f64 / rate;
        let loss = if t < hold {
            ringing
        } else {
            stopped + (ringing - stopped) * (-(t - hold) / 0.02).exp()
        };
        string.push(if how.dark {
            loss * (0.25 * read(period - 1.0) + 0.5 * read(period) + 0.25 * read(period + 1.0))
        } else {
            loss * 0.5 * (read(period - 0.5) + read(period + 0.5))
        });
    }
    for (i, s) in string.iter_mut().enumerate() {
        *s *= rise(i as f64 / rate, how.rise);
    }
    // Through the body: the string, and the body's resonances it sets ringing.
    let mut body: Vec<Biquad> = how
        .body
        .iter()
        .map(|&(h, q, _)| Biquad::band_pass(h, q, rate))
        .collect();
    let soften = if how.tone > 0.0 {
        (-TAU * how.tone / rate).exp()
    } else {
        0.0
    };
    let mut soft = 0.0;
    string
        .iter()
        .map(|&s| {
            s + body
                .iter_mut()
                .zip(how.body)
                .map(|(b, &(_, _, level))| level * b.next(s))
                .sum::<f64>()
        })
        .map(|s| {
            soft = (1.0 - soften) * s + soften * soft;
            0.6 * touch * soft
        })
        .collect()
}

/// A felt piano: an upright whose hammers have a layer of felt before the strings, so even
/// a firm touch sounds soft. Each note is three strings a fraction of a cent apart, their
/// partials a little sharp as a piano's are, falling quickly at first and then ringing on;
/// the felt keeps the partials over about 1 to 1.6 kHz down, and knocks softly.
fn felt_piano(hertz: f64, touch: f64, hold: f64, rate: f64, noise: &mut Noise) -> Vec<f64> {
    // How sharp the partials get: more on short, high strings.
    let stiff = 0.000_15 * hertz / 110.0;
    let ring = (2.5 * (220.0 / hertz).powf(0.6)).clamp(0.8, 5.0);
    let felt = 1_000.0 + 600.0 * touch;
    let partials: Vec<(f64, f64, f64)> = (1..=12)
        .map(|k| {
            let k = f64::from(k);
            (
                k * hertz * (1.0 + stiff * k * k).sqrt(),
                k.powi(-1) / (1.0 + (k * hertz / felt).powi(4)),
                1.0 / (1.0 + 0.05 * k * k),
            )
        })
        .filter(|&(h, _, _)| h < 0.45 * rate && h < 6_000.0)
        .collect();
    // The felt's knock on the strings: 15 ms of low, muffled noise.
    let knock_len = (0.015 * rate) as usize;
    let muffle = 1.0 - (-TAU * 400.0 / rate).exp();
    let mut low = 0.0;
    let knock: Vec<f64> = (0..knock_len)
        .map(|i| {
            low += muffle * (noise.next() - low);
            0.25 * touch * low * (PI * i as f64 / knock_len as f64).sin()
        })
        .collect();
    let len = ((ring * 5.0).min(hold + 0.8) * rate) as usize;
    (0..len)
        .map(|i| {
            let t = i as f64 / rate;
            let mut tone = 0.0;
            for &(h, level, sooner) in &partials {
                let fall = 0.55 * (-t / (0.15 * ring * sooner)).exp()
                    + 0.45 * (-t / (ring * sooner)).exp();
                // Three strings: in tune, and a third of a cent either side.
                let strings = (TAU * h * t).sin()
                    + (TAU * h * 1.000_2 * t).sin()
                    + (TAU * h * 0.999_8 * t).sin();
                tone += level * fall * strings / 3.0;
            }
            rise(t, 0.004) * damper(t, hold, 0.12) * tone + knock.get(i).copied().unwrap_or(0.0)
        })
        .collect()
}

/// An electric piano's tine: a pure note that rings long, a short high ping on the strike
/// in the lower notes, and the pickup's bend, which adds the note's even harmonics, more
/// the harder the key is hit.
fn keys(hertz: f64, touch: f64, hold: f64, rate: f64) -> Vec<f64> {
    let ring = (1.6 * (440.0 / hertz).sqrt()).min(3.5);
    let len = ((ring * 6.0).min(hold + 0.5) * rate) as usize;
    let bend = 0.2 + 0.4 * touch;
    let ping = if hertz * 7.0 < 4_000.0 {
        0.06 * touch
    } else {
        0.0
    };
    (0..len)
        .map(|i| {
            let t = i as f64 / rate;
            let level = (-t / ring).exp() * damper(t, hold, 0.08) * rise(t, 0.002);
            let tine = level * (TAU * hertz * t).sin()
                + ping * (-t / 0.015).exp() * rise(t, 0.002) * (TAU * 7.0 * hertz * t).sin();
            // The bend's x² would sit off zero by half the level squared: taken away.
            tine + bend * (tine * tine - 0.5 * level * level)
        })
        .collect()
}

/// A repeatable stream of noise (xorshift), from -1 to 1.
pub(super) struct Noise(u64);

impl Noise {
    pub(super) fn new(seed: u64) -> Self {
        Noise(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub(super) fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        let r = x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11;
        r as f64 / (1u64 << 52) as f64 - 1.0
    }
}
