//! Short sounds for what happens in a room and around it, made here rather than loaded
//! from files. They are one family: a soft mallet on a glassy bar (a pure note with a
//! quiet octave and double octave that fade sooner, a short shine on the strike and a
//! pitch that settles as it rings), mostly with a light chorus, a short soft room and a
//! gentle EQ, in one key, A major pentatonic, at about the same loudness. Someone coming
//! in is a fourth going up and leaving the same fourth going down; a message is two soft
//! low notes a third apart; muting is a quick step down and unmuting the same step up; sharing starts
//! with three notes up, on a warmer, rounded FM mallet, and stops with three down;
//! watching a screen is two light high notes; a new notice is a two-note chime; a call
//! that ends falls to the low tonic; a friend calling plays a short song on a soft felt
//! piano (a bar of 3.2 s: A, E, F#m, D), and calling a friend is a slow soft rise up the
//! chord.
//!
//! Each sound can also be made other ways ([`Variant`]), to compare them by ear; [`chosen`]
//! says which one each sound plays.

use std::{
    collections::VecDeque,
    error::Error,
    f64::consts::{PI, TAU},
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

mod songs;

use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Sound {
    Join,
    Leave,
    Message,
    Mute,
    Unmute,
    ShareStart,
    ShareStop,
    WatchStart,
    WatchStop,
    Ring,
    Ringback,
    Notice,
    CallEnded,
}

#[cfg(test)]
impl Sound {
    const ALL: [Sound; 13] = [
        Sound::Join,
        Sound::Leave,
        Sound::Message,
        Sound::Mute,
        Sound::Unmute,
        Sound::ShareStart,
        Sound::ShareStop,
        Sound::WatchStart,
        Sound::WatchStop,
        Sound::Ring,
        Sound::Ringback,
        Sound::Notice,
        Sound::CallEnded,
    ];
}

/// The ways a sound can be made. A, B and C are the same for every sound: the glassy bar
/// alone; with a slightly softer strike, a chorus, a short room and a gentle EQ; and a
/// warmer voice, a rounded FM mallet, with them. The others are other notes ([`notes`]) or
/// rings played on instruments ([`songs::song`]), for the sounds that have them.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the ones no sound plays stay to be heard side by side"
    )
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Variant {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
}

/// The variant each sound plays.
const fn chosen(sound: Sound) -> Variant {
    match sound {
        Sound::Join => Variant::B,
        Sound::Leave => Variant::B,
        Sound::Message => Variant::G,
        Sound::Mute => Variant::B,
        Sound::Unmute => Variant::B,
        Sound::ShareStart => Variant::C,
        Sound::ShareStop => Variant::B,
        Sound::WatchStart => Variant::B,
        Sound::WatchStop => Variant::B,
        Sound::Ring => Variant::J,
        Sound::Ringback => Variant::D,
        Sound::Notice => Variant::B,
        Sound::CallEnded => Variant::B,
    }
}

/// The variants a sound has.
#[cfg(test)]
fn variants(sound: Sound) -> &'static [Variant] {
    use Variant::*;
    match sound {
        Sound::Message => &[A, B, C, D, E, F, G],
        Sound::Ring => &[A, B, C, D, E, F, G, H, I, J],
        Sound::Ringback => &[A, B, C, D],
        _ => &[A, B, C],
    }
}

/// One note of a sound.
struct Note {
    /// Seconds from the start of the sound.
    at: f64,
    hertz: f64,
    /// Level against the sound's other notes; the sound as a whole is brought to its
    /// loudness afterwards.
    gain: f64,
    /// Seconds for the note to fall to about a third.
    decay: f64,
    /// Level of the partials over the note itself (1 is the voice's own).
    bright: f64,
    /// How far sharp the note starts, as a fraction of its pitch; it settles within
    /// [`SETTLE`].
    drop: f64,
}

const fn note(at: f64, hertz: f64, gain: f64, decay: f64) -> Note {
    Note {
        at,
        hertz,
        gain,
        decay,
        bright: 1.0,
        drop: 0.012,
    }
}

impl Note {
    const fn bright(self, bright: f64) -> Note {
        Note { bright, ..self }
    }

    const fn drop(self, drop: f64) -> Note {
        Note { drop, ..self }
    }

    /// Seconds for the note to fall to about a third in `voice`.
    fn decay_in(&self, voice: Voice) -> f64 {
        match voice {
            Voice::Glass => self.decay,
            Voice::Warm => self.decay * WARM_LONGER,
        }
    }

    /// How long the note is rendered: until it is about 60 dB down.
    fn seconds(&self, voice: Voice) -> f64 {
        (self.decay_in(voice) * 7.0).min(3.0)
    }
}

/// What a sound is played on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Voice {
    /// A soft mallet on a glassy bar, [`PARTIALS`].
    Glass,
    /// A rounded FM mallet: a sine bent by another at its own pitch, hard on the strike
    /// (the note's harmonics, like a felt mallet on wood) and soon nearly pure, ringing
    /// [`WARM_LONGER`] times as long as the glass.
    Warm,
}

/// How a variant is made.
struct Style {
    voice: Voice,
    /// Seconds of the strike's rise.
    attack: f64,
    /// Through a chorus, a room and an EQ, or dry.
    space: Option<&'static Space>,
}

fn style(sound: Sound, variant: Variant) -> Style {
    match (sound, variant) {
        (_, Variant::A) => Style {
            voice: Voice::Glass,
            attack: ATTACK,
            space: None,
        },
        (_, Variant::B) | (Sound::Message, Variant::D | Variant::E | Variant::F | Variant::G) => {
            Style {
                voice: Voice::Glass,
                attack: SOFT_ATTACK,
                space: Some(&ROOM),
            }
        }
        _ => Style {
            voice: Voice::Warm,
            attack: WARM_ATTACK,
            space: Some(&ROOM),
        },
    }
}

/// The voice's partials: multiple of the note's pitch, level, and how much sooner than
/// the note it fades. The last one, off the harmonic series, is the shine of the strike.
const PARTIALS: [(f64, f64, f64); 4] = [
    (1.0, 1.0, 1.0),
    (2.0, 0.16, 0.45),
    (4.0, 0.07, 0.18),
    (6.27, 0.035, 0.05),
];

/// How much the warm voice's FM bends the note: on the strike and once it rings, at
/// [`Note::bright`] 1. Over about 1.5 the strike turns nasal; under 0.1 the ring is a bare
/// sine.
const WARM_STRIKE: f64 = 1.0;
const WARM_RING: f64 = 0.2;
/// How long the warm voice's strike lasts against the note's decay.
const WARM_STRIKE_DECAY: f64 = 0.25;
const WARM_LONGER: f64 = 1.3;
/// Notes above this are bent less, in proportion to how much higher they are.
const WARM_FULL_HERTZ: f64 = 880.0;

/// The strike: a raised-cosine rise this long, short enough to sound struck and long
/// enough not to click.
const ATTACK: f64 = 0.004;
/// The same, softer: still heard as a strike, without its edge.
const SOFT_ATTACK: f64 = 0.006;
const WARM_ATTACK: f64 = 0.008;
/// How fast a note's pitch settles from its [`Note::drop`].
const SETTLE: f64 = 0.018;
/// Each note fades out over its last 20 ms, and the sound over its last 25 ms once it is
/// 42 dB under its peak, so nothing stops on a step. With the room, whose tail is quieter
/// and longer, the sound ends 48 dB under its peak, over 60 ms.
const NOTE_FADE: f64 = 0.020;
const SOUND_FADE: f64 = 0.025;
const TAIL_DB: f64 = -42.0;
const ROOM_FADE: f64 = 0.060;
const ROOM_TAIL_DB: f64 = -48.0;
/// The highest peak a sound may reach, -1 dBFS.
const MAX_PEAK: f64 = 0.891;

// A major pentatonic, from A4 to F#6.
const A4: f64 = 440.0;
const CS5: f64 = 554.37;
const E5: f64 = 659.26;
const A5: f64 = 880.0;
const B5: f64 = 987.77;
const CS6: f64 = 1108.73;
const E6: f64 = 1318.51;
const FS6: f64 = 1479.98;

/// The time between the notes of the ringback's slow rise.
const RISE_STEP: f64 = 0.1625;

fn notes(sound: Sound, variant: Variant) -> Vec<Note> {
    match (sound, variant) {
        // The message's drop an octave lower, a little longer, as a lower note rings.
        (Sound::Message, Variant::D) => vec![note(0.0, CS5, 1.0, 0.05).bright(0.7).drop(0.02)],
        // The drop a sixth lower.
        (Sound::Message, Variant::E) => vec![note(0.0, E5, 1.0, 0.045).bright(0.6).drop(0.02)],
        // A droplet: a nearly pure note that starts a fifth-and-more flat and rises to E5
        // within a few tens of milliseconds.
        (Sound::Message, Variant::F) => vec![note(0.0, E5, 1.0, 0.04).bright(0.25).drop(-0.3)],
        // Two soft low notes a third apart, the higher a moment later.
        (Sound::Message, Variant::G) => vec![
            note(0.0, A4, 0.75, 0.05).bright(0.5),
            note(0.035, CS5, 1.0, 0.055).bright(0.5),
        ],
        // The ring's rise, slow, an octave and more lower, soft.
        (Sound::Ringback, Variant::D) => vec![
            note(0.0, A4, 0.6, 0.15).bright(0.4),
            note(RISE_STEP, CS5, 0.65, 0.15).bright(0.4),
            note(2.0 * RISE_STEP, E5, 0.7, 0.18).bright(0.4),
        ],
        _ => first_notes(sound),
    }
}

/// The notes each sound was first made with.
fn first_notes(sound: Sound) -> Vec<Note> {
    match sound {
        Sound::Join => vec![note(0.0, E5, 0.8, 0.05), note(0.08, A5, 1.0, 0.06)],
        Sound::Leave => vec![
            note(0.0, A5, 1.0, 0.05).bright(0.8),
            note(0.08, E5, 0.85, 0.06).bright(0.6),
        ],
        Sound::Message => vec![note(0.0, CS6, 1.0, 0.035).drop(0.02)],
        Sound::Mute => vec![
            note(0.0, B5, 0.7, 0.022).bright(0.6),
            note(0.03, E5, 1.0, 0.03).bright(0.5).drop(0.0),
        ],
        Sound::Unmute => vec![
            note(0.0, E5, 0.7, 0.022).bright(0.6),
            note(0.03, B5, 1.0, 0.03).bright(0.8),
        ],
        Sound::ShareStart => vec![
            note(0.0, A5, 0.75, 0.05),
            note(0.06, CS6, 0.85, 0.05),
            note(0.12, E6, 1.0, 0.052),
        ],
        Sound::ShareStop => vec![
            note(0.0, E6, 1.0, 0.05),
            note(0.06, CS6, 0.85, 0.05),
            note(0.12, A5, 0.8, 0.052).bright(0.7),
        ],
        Sound::WatchStart => vec![note(0.0, CS6, 0.8, 0.03), note(0.07, FS6, 1.0, 0.04)],
        Sound::WatchStop => vec![note(0.0, FS6, 1.0, 0.03), note(0.07, CS6, 0.85, 0.04)],
        Sound::Ring => vec![
            note(0.0, A5, 0.8, 0.08),
            note(0.12, CS6, 0.85, 0.08),
            note(0.24, E6, 1.0, 0.1),
            note(0.5, CS6, 0.8, 0.08),
            note(0.62, FS6, 0.95, 0.13),
        ],
        Sound::Ringback => vec![
            note(0.0, A4, 0.7, 0.16).bright(0.5),
            note(0.0, E5, 0.7, 0.16).bright(0.5),
            note(0.45, A4, 0.6, 0.16).bright(0.5),
            note(0.45, E5, 0.6, 0.16).bright(0.5),
        ],
        Sound::Notice => vec![note(0.0, B5, 0.8, 0.065), note(0.03, E6, 1.0, 0.07)],
        Sound::CallEnded => vec![
            note(0.0, E5, 0.9, 0.05).bright(0.8),
            note(0.08, CS5, 0.9, 0.05).bright(0.7),
            note(0.21, A4, 1.0, 0.06).bright(0.6),
        ],
    }
}

/// How loud each sound is, as the most of [`loudness`] over its length (LUFS). About the
/// same for all; the frequent or less important ones a little softer, the ring of a call
/// this device makes the softest, since it repeats while one waits, and the lower, softer
/// messages as soft.
fn loudness_of(sound: Sound, variant: Variant) -> f64 {
    match (sound, variant) {
        (Sound::Message, Variant::D | Variant::E | Variant::F | Variant::G) => -24.0,
        (sound, _) => match sound {
            Sound::Message => -23.0,
            Sound::WatchStart | Sound::WatchStop => -23.5,
            Sound::Ringback => -24.0,
            _ => -22.0,
        },
    }
}

/// Two channels, left and right.
pub(crate) type Frame = [f32; 2];

/// The sound as it plays, at `rate` frames per second.
pub(crate) fn render(sound: Sound, rate: u32) -> Vec<Frame> {
    render_as(sound, chosen(sound), rate)
}

/// The sound made as `variant`, at `rate` frames per second.
fn render_as(sound: Sound, variant: Variant, rate: u32) -> Vec<Frame> {
    let rate = f64::from(rate);
    let (dry, space) = match songs::song(sound, variant) {
        Some(song) => (song.play(rate), Some(&SONG_ROOM)),
        None => {
            let style = style(sound, variant);
            let notes = notes(sound, variant);
            let seconds = notes
                .iter()
                .map(|n| n.at + n.seconds(style.voice))
                .fold(0.0, f64::max);
            let mut dry = vec![0.0f64; (seconds * rate) as usize + 1];
            for n in &notes {
                add_note(&mut dry, n, rate, &style);
            }
            (dry, style.space)
        }
    };
    let mut out = match space {
        Some(space) => balanced(through(&dry, rate, space)),
        None => dry.iter().map(|&s| [s, s]).collect(),
    };
    // Ends where it is 42 dB (48 dB with the room) under its peak, fading on the way.
    let (tail_db, fade) = if space.is_some() {
        (ROOM_TAIL_DB, ROOM_FADE)
    } else {
        (TAIL_DB, SOUND_FADE)
    };
    let level = |f: &[f64; 2]| f[0].abs().max(f[1].abs());
    let peak = out.iter().map(level).fold(0.0f64, f64::max);
    let floor = peak * 10f64.powf(tail_db / 20.0);
    let fade = (fade * rate) as usize;
    let last = out.iter().rposition(|f| level(f) > floor).unwrap_or(0);
    out.truncate((last + fade).min(out.len()));
    let start = out.len().saturating_sub(fade);
    for (k, f) in out[start..].iter_mut().enumerate() {
        let fall = 0.5 + 0.5 * (PI * k as f64 / fade as f64).cos();
        f[0] *= fall;
        f[1] *= fall;
    }
    // Then to its loudness, never over the highest peak.
    let mut gain = 10f64.powf((loudness_of(sound, variant) - loudness(&out, rate)) / 20.0);
    let peak = out.iter().map(level).fold(0.0f64, f64::max) * gain;
    if peak > MAX_PEAK {
        gain *= MAX_PEAK / peak;
    }
    out.iter()
        .map(|f| [(f[0] * gain) as f32, (f[1] * gain) as f32])
        .collect()
}

fn add_note(out: &mut [f64], n: &Note, rate: f64, style: &Style) {
    let first = (n.at * rate) as usize;
    let len = (n.seconds(style.voice) * rate) as usize;
    let fade = (NOTE_FADE * rate) as usize;
    let decay = n.decay_in(style.voice);
    let mut phase = 0.0f64;
    for i in 0..len {
        let t = i as f64 / rate;
        phase += TAU * n.hertz * (1.0 + n.drop * (-t / SETTLE).exp()) / rate;
        let tone = match style.voice {
            Voice::Glass => {
                let mut tone = 0.0;
                for (k, &(multiple, level, sooner)) in PARTIALS.iter().enumerate() {
                    // Partials too high for the rate would fold back as other notes.
                    if n.hertz * multiple >= 0.45 * rate {
                        continue;
                    }
                    let level = if k == 0 { level } else { level * n.bright };
                    tone += level * (-t / (decay * sooner)).exp() * (phase * multiple).sin();
                }
                tone
            }
            Voice::Warm => {
                // Higher notes are bent less, so their harmonics stay out of where the ear
                // is most sensitive.
                let track = (WARM_FULL_HERTZ / n.hertz).min(1.0);
                let strike = (-t / (decay * WARM_STRIKE_DECAY)).exp();
                let bend = n.bright * track * (WARM_RING + WARM_STRIKE * strike);
                // A third of the level falls away quickly with the strike, the rest rings.
                let level = 0.35 * (-t / (decay * 0.35)).exp() + 0.65 * (-t / decay).exp();
                level * (phase + bend * phase.sin()).sin()
            }
        };
        let rise = if t < style.attack {
            0.5 - 0.5 * (PI * t / style.attack).cos()
        } else {
            1.0
        };
        let left = len - i;
        let fall = if left < fade {
            0.5 - 0.5 * (PI * left as f64 / fade as f64).cos()
        } else {
            1.0
        };
        if let Some(slot) = out.get_mut(first + i) {
            *slot += n.gain * rise * fall * tone;
        }
    }
}

/// The chorus's voices: delay and how far it sways (seconds), how fast it sways (Hz),
/// where its sway starts (radians), and where it sits, from left (-1) to right (1). The
/// sway bends each copy by up to about 12 cents, each its own way, around the note.
const CHORUS: [(f64, f64, f64, f64, f64); 3] = [
    (0.008, 0.0008, 0.83, 0.0, -0.85),
    (0.012, 0.0010, 1.07, 2.1, 0.85),
    (0.016, 0.0007, 1.31, 4.2, 0.0),
];
/// Where a sound is played: its chorus's copies' level against it, how long its room rings
/// (seconds to fall 60 dB) and its level against the sound, and under what it is cut.
struct Space {
    chorus: f64,
    room: f64,
    room_level: f64,
    low_cut: f64,
}

/// The sounds' room: a clear chorus and a short room.
const ROOM: Space = Space {
    chorus: 0.45,
    room: 0.5,
    room_level: 0.2,
    low_cut: 140.0,
};

/// The rings played on instruments: a light chorus, which on a real instrument would
/// sound like a machine, a longer, warmer room, and their low notes kept.
const SONG_ROOM: Space = Space {
    chorus: 0.25,
    room: 0.9,
    room_level: 0.28,
    low_cut: 70.0,
};

/// How long after the sound the room starts.
const ROOM_WAIT: f64 = 0.008;
/// Above this the room's echoes fade sooner, as a soft room's do.
const ROOM_DAMP_HERTZ: f64 = 4_000.0;
/// The room's eight echoing paths, in seconds, and the three that smear the sound before
/// them.
const ROOM_LINES: [f64; 8] = [
    0.0191, 0.0227, 0.0259, 0.0293, 0.0317, 0.0353, 0.0389, 0.0431,
];
const ROOM_SMEAR: [f64; 3] = [0.0051, 0.0037, 0.0019];
const ROOM_SMEAR_GAIN: f64 = 0.55;

/// The sound in stereo, through the chorus, the room and the EQ of `space`, with time for
/// the room's tail.
fn through(dry: &[f64], rate: f64, space: &Space) -> Vec<[f64; 2]> {
    let len = dry.len() + (space.room * rate) as usize;
    let mut out = chorus(dry, len, rate, space.chorus);
    let mut room = Room::new(rate, space.room);
    for (i, frame) in out.iter_mut().enumerate() {
        let [left, right] = room.next(dry.get(i).copied().unwrap_or(0.0));
        frame[0] += space.room_level * left;
        frame[1] += space.room_level * right;
    }
    let mut eq = [
        equalizer(rate, space.low_cut),
        equalizer(rate, space.low_cut),
    ];
    for frame in &mut out {
        for (side, filters) in eq.iter_mut().enumerate() {
            frame[side] = filters.iter_mut().fold(frame[side], |s, f| f.next(s));
        }
    }
    out
}

/// The sound with both sides as loud: the chorus's copies meet the note differently on
/// each side, and the sound would otherwise lean to one.
fn balanced(mut frames: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    let energy = |k: usize| frames.iter().map(|f| f[k] * f[k]).sum::<f64>();
    let (left, right) = (energy(0), energy(1));
    if left > 0.0 && right > 0.0 {
        let middle = 0.5 * (left + right);
        let gains = [(middle / left).sqrt(), (middle / right).sqrt()];
        for f in &mut frames {
            f[0] *= gains[0];
            f[1] *= gains[1];
        }
    }
    frames
}

/// The note in the middle and three copies of it, each a little late by a swaying delay,
/// spread from left to right.
fn chorus(dry: &[f64], len: usize, rate: f64, level: f64) -> Vec<[f64; 2]> {
    // Between samples, read along the line between them; before and after, silence.
    let at = |position: f64| {
        if position < 0.0 {
            return 0.0;
        }
        let i = position as usize;
        let between = position - i as f64;
        let a = dry.get(i).copied().unwrap_or(0.0);
        let b = dry.get(i + 1).copied().unwrap_or(0.0);
        a + (b - a) * between
    };
    (0..len)
        .map(|n| {
            let x = dry.get(n).copied().unwrap_or(0.0);
            let t = n as f64 / rate;
            let mut frame = [x, x];
            for &(delay, sway, hertz, start, pan) in &CHORUS {
                let late = (delay + sway * (TAU * hertz * t + start).sin()) * rate;
                let copy = level * at(n as f64 - late);
                let angle = (pan + 1.0) * PI / 4.0;
                frame[0] += copy * angle.cos();
                frame[1] += copy * angle.sin();
            }
            frame
        })
        .collect()
}

/// A low cut under `low_cut` Hz, 3 dB less around 3.2 kHz where a high note's partials
/// sound sharp, and 4 dB less above about 6.5 kHz.
fn equalizer(rate: f64, low_cut: f64) -> [Biquad; 3] {
    [
        Biquad::high_pass(low_cut, 0.707, rate),
        Biquad::peak(3_200.0, 0.8, -3.0, rate),
        Biquad::high_shelf(6_500.0, -4.0, rate),
    ]
}

/// A line that gives back what went in a fixed number of samples before.
struct Delay {
    line: Vec<f64>,
    at: usize,
}

impl Delay {
    fn new(seconds: f64, rate: f64) -> Self {
        Delay {
            line: vec![0.0; ((seconds * rate) as usize).max(1)],
            at: 0,
        }
    }

    fn out(&self) -> f64 {
        self.line[self.at]
    }

    fn push(&mut self, x: f64) {
        self.line[self.at] = x;
        self.at = (self.at + 1) % self.line.len();
    }
}

/// A small feedback delay network: the sound, a little late and smeared by three
/// all-pass filters, goes round eight lines of different lengths that feed each other
/// through a Householder matrix, losing a little and its highs on each pass. Left takes
/// half the lines and right the other half, so the two sides differ.
struct Room {
    wait: Delay,
    smear: Vec<Delay>,
    lines: Vec<Delay>,
    /// Each line's gain per pass, for the room's time to fall 60 dB.
    gains: Vec<f64>,
    /// Each line's low-pass, and how much of its last output it keeps.
    lows: Vec<f64>,
    damp: f64,
}

impl Room {
    fn new(rate: f64, seconds: f64) -> Self {
        let lines: Vec<Delay> = ROOM_LINES.iter().map(|&s| Delay::new(s, rate)).collect();
        let gains = lines
            .iter()
            .map(|l| 10f64.powf(-3.0 * l.line.len() as f64 / (rate * seconds)))
            .collect();
        Room {
            wait: Delay::new(ROOM_WAIT, rate),
            smear: ROOM_SMEAR.iter().map(|&s| Delay::new(s, rate)).collect(),
            lows: vec![0.0; lines.len()],
            lines,
            gains,
            damp: (-TAU * ROOM_DAMP_HERTZ / rate).exp(),
        }
    }

    fn next(&mut self, x: f64) -> [f64; 2] {
        let mut input = self.wait.out();
        self.wait.push(x);
        for smear in &mut self.smear {
            let late = smear.out();
            let v = input + ROOM_SMEAR_GAIN * late;
            smear.push(v);
            input = late - ROOM_SMEAR_GAIN * v;
        }
        let mut outs = [0.0f64; ROOM_LINES.len()];
        for (k, line) in self.lines.iter().enumerate() {
            self.lows[k] = (1.0 - self.damp) * line.out() + self.damp * self.lows[k];
            outs[k] = self.lows[k] * self.gains[k];
        }
        let mix = outs.iter().sum::<f64>() * 2.0 / outs.len() as f64;
        for (line, out) in self.lines.iter_mut().zip(outs) {
            line.push(input + out - mix);
        }
        let mut sides = [0.0; 2];
        for (k, out) in outs.iter().enumerate() {
            let sign = if k / 2 % 2 == 0 { 1.0 } else { -1.0 };
            sides[k % 2] += sign * out * 0.5;
        }
        sides
    }
}

/// The most loudness of `frames` over any 400 ms, in LUFS: the K-weighted mean square of
/// ITU-R BS.1770 (a shelf up above about 1.7 kHz, the lowest bass taken out), as loudness
/// meters measure it moment by moment, of the mean of the two sides, so a sound the same
/// on both reads as it would alone. A sound shorter than 400 ms counts as that long.
pub(crate) fn loudness(frames: &[[f64; 2]], rate: f64) -> f64 {
    let side = |k: usize| -> Vec<f64> { frames.iter().map(|f| f[k]).collect() };
    let squares: Vec<f64> = weighted_squares(&side(0), rate)
        .iter()
        .zip(weighted_squares(&side(1), rate))
        .map(|(l, r)| 0.5 * (l + r))
        .collect();
    most_loudness(&squares, rate)
}

fn weighted_squares(samples: &[f64], rate: f64) -> Vec<f64> {
    let mut shelf = {
        let (gain_db, q, hertz) = (
            3.999_843_853_973_347,
            0.707_175_236_955_419_3,
            1_681.974_450_955_532,
        );
        let k = (PI * hertz / rate).tan();
        let vh = 10f64.powf(gain_db / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        let a0 = 1.0 + k / q + k * k;
        Biquad::new(
            [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        )
    };
    let mut high_pass = {
        let (q, hertz) = (0.500_327_037_325_395_3, 38.135_470_876_139_82);
        let k = (PI * hertz / rate).tan();
        let a0 = 1.0 + k / q + k * k;
        Biquad::new(
            [1.0, -2.0, 1.0],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        )
    };
    samples
        .iter()
        .map(|&s| high_pass.next(shelf.next(s)).powi(2))
        .collect()
}

fn most_loudness(squares: &[f64], rate: f64) -> f64 {
    let window = (0.4 * rate) as usize;
    let hop = ((0.01 * rate) as usize).max(1);
    let mut sums = Vec::with_capacity(squares.len() + 1);
    sums.push(0.0);
    for s in squares {
        sums.push(sums.last().copied().unwrap_or(0.0) + s);
    }
    let mut most = 0.0f64;
    for start in (0..squares.len().max(1)).step_by(hop) {
        let end = (start + window).min(squares.len());
        most = most.max((sums[end] - sums[start.min(end)]) / window as f64);
    }
    -0.691 + 10.0 * most.max(1e-20).log10()
}

/// A second-order filter, in direct form I.
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Biquad {
            b,
            a,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    /// From the coefficients of the Audio EQ Cookbook (R. Bristow-Johnson), `a0` first.
    fn cookbook(b: [f64; 3], a: [f64; 3]) -> Self {
        Biquad::new(
            [b[0] / a[0], b[1] / a[0], b[2] / a[0]],
            [a[1] / a[0], a[2] / a[0]],
        )
    }

    fn high_pass(hertz: f64, q: f64, rate: f64) -> Self {
        let w = TAU * hertz / rate;
        let alpha = w.sin() / (2.0 * q);
        let c = w.cos();
        Biquad::cookbook(
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        )
    }

    fn peak(hertz: f64, q: f64, db: f64, rate: f64) -> Self {
        let w = TAU * hertz / rate;
        let alpha = w.sin() / (2.0 * q);
        let c = w.cos();
        let a = 10f64.powf(db / 40.0);
        Biquad::cookbook(
            [1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a],
        )
    }

    /// A band-pass whose peak passes at its level.
    fn band_pass(hertz: f64, q: f64, rate: f64) -> Self {
        let w = TAU * hertz / rate;
        let alpha = w.sin() / (2.0 * q);
        Biquad::cookbook(
            [alpha, 0.0, -alpha],
            [1.0 + alpha, -2.0 * w.cos(), 1.0 - alpha],
        )
    }

    /// A shelf of slope 1.
    fn high_shelf(hertz: f64, db: f64, rate: f64) -> Self {
        let w = TAU * hertz / rate;
        let c = w.cos();
        let a = 10f64.powf(db / 40.0);
        let root = a.sqrt() * w.sin() * 2f64.sqrt();
        Biquad::cookbook(
            [
                a * ((a + 1.0) + (a - 1.0) * c + root),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
                a * ((a + 1.0) + (a - 1.0) * c - root),
            ],
            [
                (a + 1.0) - (a - 1.0) * c + root,
                2.0 * ((a - 1.0) - (a + 1.0) * c),
                (a + 1.0) - (a - 1.0) * c - root,
            ],
        )
    }

    fn next(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// How long the output stays open after the last sound, so sounds that come close together
/// (someone writes twice, two people come in) do not each open the device anew.
const KEEP_OPEN: Duration = Duration::from_secs(15);

/// How often the ring and the ringback start again while a call rings: a bar of the
/// ring's song.
const RING_EVERY: f64 = songs::SONG_EVERY;

/// How long a ring takes to fall silent once stopped: at once to the ear, without a click.
const RING_FADE: f64 = 0.030;

/// How often the player looks whether the output still takes samples while a ring plays.
const RING_CHECK: Duration = Duration::from_secs(1);

/// How long a ring plays at most, in seconds, if nothing stops it: twice as long as a
/// call rings ([`fogata_core::RING_MS`]), so a stop that never comes does not ring forever.
const RING_LIMIT: f64 = 2.0 * fogata_core::RING_MS as f64 / 1000.0;

/// What the sound thread is asked to do.
enum Order {
    Play(Sound),
    /// Ring a sound every bar until the next order of this kind; `None` stops ringing.
    Ring(Option<Sound>),
}

static PLAYER: OnceLock<Mutex<mpsc::Sender<Order>>> = OnceLock::new();

/// Each sound made once for each rate, by the player or ahead by [`prepare`]: the ring
/// takes about a second to make. One making a sound that is being made waits for it.
#[allow(clippy::type_complexity, reason = "a list of cells by sound and rate")]
static MADE: Mutex<Vec<((Sound, u32), Arc<OnceLock<Arc<[Frame]>>>)>> = Mutex::new(Vec::new());

/// The rate most outputs play at, which the rings are made at ahead.
const USUAL_RATE: u32 = 48_000;

/// The failures to play logged already: each kind is a warning the first time, so a
/// computer where no sound comes out says why, also when a later failure is not the first
/// one's (the output opened at start, and is busy once a call holds the sound card), and
/// repeats do not fill the log.
static WARNED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// How many different failures are warned about; any more are debug lines.
const WARN_KINDS: usize = 16;

fn warn_once(what: &str) {
    let mut warned = WARNED.lock().unwrap_or_else(PoisonError::into_inner);
    if warned.len() >= WARN_KINDS || warned.iter().any(|w| w == what) {
        tracing::debug!("sound: {what}");
    } else {
        warned.push(what.to_owned());
        tracing::warn!("sound: {what}");
    }
}

/// On Linux the sounds reach the sound server (PipeWire or PulseAudio) through ALSA's
/// plugin for it. Without the plugin ALSA's default output is the sound card itself: the
/// sounds play while nothing else uses the card, and fail while a call or any other
/// program holds it.
const NO_ROUTE: &str = "ALSA does not reach the sound server; install pipewire-alsa \
     (with PipeWire) or PulseAudio's ALSA plugin (libasound2-plugins on Debian and Ubuntu, \
     alsa-plugins-pulseaudio on Fedora, pulseaudio-alsa on Arch)";

/// Whether ALSA's devices, by their names (`pipewire`, `pulse`, `hw:CARD=0,DEV=0`...),
/// include one that goes to a sound server.
fn reaches_sound_server<'a>(pcms: impl IntoIterator<Item = &'a str>) -> bool {
    pcms.into_iter().any(|pcm| {
        let plugin = pcm.split(':').next().unwrap_or(pcm);
        plugin == "pipewire" || plugin == "pulse"
    })
}

/// Says once, as a warning, when ALSA has no way to the sound server, before the first
/// sound: the failures it causes come later, while a call holds the sound card.
fn check_route(host: &cpal::Host) {
    static CHECKED: AtomicBool = AtomicBool::new(false);
    if !cfg!(target_os = "linux") || CHECKED.swap(true, Ordering::Relaxed) {
        return;
    }
    let Ok(devices) = host.devices() else {
        return;
    };
    let pcms: Vec<String> = devices
        .filter_map(|d| d.id().ok())
        .map(|id| id.id().to_owned())
        .collect();
    if !reaches_sound_server(pcms.iter().map(String::as_str)) {
        warn_once(NO_ROUTE);
    }
}

/// The output chosen in the application, as the voice engine names it (its device id), and
/// how many times it was chosen, so the player opens the new one. `None` is the system's
/// default.
static OUTPUT: Mutex<(Option<String>, u64)> = Mutex::new((None, 0));

/// Plays the sounds that follow on the output `id` names, or on the system's default.
/// On Linux they stay on the default here and the voice engine's side moves them, as it
/// names outputs by what the sound server shows rather than by anything ALSA knows.
pub(crate) fn use_output(id: Option<String>) {
    let mut output = OUTPUT.lock().unwrap_or_else(PoisonError::into_inner);
    if output.0 != id {
        output.0 = id;
        output.1 += 1;
    }
}

pub(crate) fn chosen_output() -> (Option<String>, u64) {
    OUTPUT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Plays a sound on the chosen output without waiting for it. Without an output device
/// nothing happens. Sounds that overlap are mixed.
pub(crate) fn play(sound: Sound) {
    send(Order::Play(sound));
}

/// Rings `sound` every bar ([`RING_EVERY`]), each bar over the tail of the one before,
/// until called again: another sound ends the one ringing and rings instead, `None` ends
/// it, faded out in [`RING_FADE`]. Other sounds play over the ring as they come.
pub(crate) fn ring(sound: Option<Sound>) {
    send(Order::Ring(sound));
}

/// Makes the ring and the ringback ahead, on a thread of their own, at the rate most
/// outputs play at: made only once a call rings, the first bar would start late.
pub(crate) fn prepare() {
    prepare_at(USUAL_RATE);
}

fn prepare_at(rate: u32) {
    if is_made(Sound::Ring, rate) && is_made(Sound::Ringback, rate) {
        return;
    }
    let spawned = thread::Builder::new()
        .name("fogata-sound-prep".into())
        .spawn(move || {
            for sound in [Sound::Ring, Sound::Ringback] {
                made(sound, rate);
            }
        });
    if let Err(e) = spawned {
        tracing::debug!("sound: rings not made ahead: {e}");
    }
}

fn made_cell(sound: Sound, rate: u32) -> Arc<OnceLock<Arc<[Frame]>>> {
    let mut made = MADE.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, cell)) = made.iter().find(|(key, _)| *key == (sound, rate)) {
        return cell.clone();
    }
    let cell = Arc::new(OnceLock::new());
    made.push(((sound, rate), cell.clone()));
    cell
}

fn is_made(sound: Sound, rate: u32) -> bool {
    made_cell(sound, rate).get().is_some()
}

/// The sound as it plays at `rate`, made the first time.
fn made(sound: Sound, rate: u32) -> Arc<[Frame]> {
    made_cell(sound, rate)
        .get_or_init(|| render(sound, rate).into())
        .clone()
}

fn send(order: Order) {
    let player = PLAYER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("fogata-sound".into())
            .spawn(move || player(rx, open, RING_CHECK));
        if let Err(e) = spawned {
            warn_once(&format!("no thread to play on: {e}"));
        }
        Mutex::new(tx)
    });
    if let Ok(tx) = player.lock()
        && tx.send(order).is_err()
    {
        warn_once("not played: the player stopped");
    }
}

/// What the output plays, shared with the output stream's callback.
type Queue = Arc<Mutex<Mix>>;

/// The sounds waiting to be played, mixed into one queue, and the rings over them.
#[derive(Default)]
struct Mix {
    queue: VecDeque<Frame>,
    rings: Vec<Ring>,
    /// Frames the output took: a stream that no longer takes any is opened again.
    taken: u64,
}

impl Mix {
    /// The next frame to play.
    fn next(&mut self) -> Frame {
        self.taken += 1;
        let mut frame = self.queue.pop_front().unwrap_or([0.0; 2]);
        self.rings.retain_mut(|ring| ring.add_to(&mut frame));
        frame
    }

    /// Fades out the rings playing and starts `ring`, if any.
    fn ring(&mut self, ring: Option<Ring>) {
        for playing in &mut self.rings {
            playing.stop();
        }
        self.rings.extend(ring);
    }

    fn clear(&mut self) {
        self.queue.clear();
        self.rings.clear();
    }
}

/// A sound that starts again every bar until stopped.
struct Ring {
    bar: Arc<[Frame]>,
    /// Frames from one start to the next.
    every: usize,
    /// Frames played since the first start (less whole bars once those have ended).
    at: usize,
    /// Frames it plays before it stops by itself, and those played.
    limit: usize,
    played: usize,
    /// Frames the fade lasts, and once stopped, those left of it.
    fade: usize,
    left: Option<usize>,
}

impl Ring {
    fn new(bar: Arc<[Frame]>, rate: u32) -> Self {
        let rate = f64::from(rate);
        Ring {
            bar,
            every: ((RING_EVERY * rate) as usize).max(1),
            at: 0,
            limit: (RING_LIMIT * rate) as usize,
            played: 0,
            fade: ((RING_FADE * rate) as usize).max(1),
            left: None,
        }
    }

    fn stop(&mut self) {
        self.left.get_or_insert(self.fade);
    }

    /// Adds what the ring plays now to `frame`: the bar that started last and the tails
    /// of those before it. False once it has faded out.
    fn add_to(&mut self, frame: &mut Frame) -> bool {
        if self.played >= self.limit {
            self.stop();
        }
        self.played += 1;
        let gain = match self.left {
            Some(0) => return false,
            Some(left) => {
                let left = left - 1;
                self.left = Some(left);
                (0.5 - 0.5 * (PI * left as f64 / self.fade as f64).cos()) as f32
            }
            None => 1.0,
        };
        let mut i = self.at % self.every;
        while i <= self.at && i < self.bar.len() {
            frame[0] += gain * self.bar[i][0];
            frame[1] += gain * self.bar[i][1];
            i += self.every;
        }
        self.at += 1;
        // Bars that ended play no more: counting from a later start changes nothing.
        if self.at >= self.bar.len() + self.every {
            self.at -= self.every;
        }
        true
    }
}

/// Adds `frames` to the queue from its start, on top of what is already there.
pub(crate) fn mix_into(queue: &mut VecDeque<Frame>, frames: &[Frame]) {
    for (i, frame) in frames.iter().enumerate() {
        match queue.get_mut(i) {
            Some(slot) => {
                slot[0] += frame[0];
                slot[1] += frame[1];
            }
            None => queue.push_back(*frame),
        }
    }
}

/// Where [`limit`] starts to bend.
const KNEE: f32 = 0.7;

/// Keeps a mix of sounds that overlap under full scale: samples up to 0.7 pass as they
/// are, louder ones bend smoothly towards 1 instead of being cut flat.
pub(crate) fn limit(sample: f32) -> f32 {
    let level = sample.abs();
    if level <= KNEE {
        return sample;
    }
    let bent = KNEE + (1.0 - KNEE) * ((level - KNEE) / (1.0 - KNEE)).tanh();
    bent.copysign(sample)
}

/// An open output.
struct Output {
    /// Plays while it is kept (none in tests).
    _stream: Option<cpal::Stream>,
    rate: u32,
    /// Set by the stream when it stopped for good (its device went away, the system
    /// changed the default output): the next sound opens the output again.
    broken: Arc<AtomicBool>,
    /// When what is queued should have played; still queued well after it, the stream is
    /// not taking samples any more.
    played_by: Instant,
    /// The choice of output it was opened for ([`OUTPUT`]'s count).
    chosen: u64,
}

/// The sound thread: opens the output when a sound comes, mixes every sound that follows
/// into the queue, and closes the output once nothing has played for [`KEEP_OPEN`]. An
/// output that broke or stopped taking samples is opened again for the next sound, and
/// while a ring plays, as soon as that is seen. While a ring is wanted and the output does
/// not open (busy, its server restarting), opening is tried again every `check`.
fn player(
    orders: mpsc::Receiver<Order>,
    mut open: impl FnMut(Queue) -> Result<Output, Box<dyn Error>>,
    check: Duration,
) {
    let queue: Queue = Arc::new(Mutex::new(Mix::default()));
    let mut output: Option<Output> = None;
    // The sound ringing, rung again on an output opened anew, and since when: its
    // [`RING_LIMIT`] counts from then.
    let mut ringing: Option<Sound> = None;
    let mut ringing_since = Instant::now();
    // How many frames the output had taken, and since when.
    let mut taken = (0, Instant::now());
    loop {
        let next = match (&output, ringing) {
            (_, Some(_)) => orders.recv_timeout(check),
            (Some(_), None) => orders.recv_timeout(KEEP_OPEN),
            (None, None) => orders
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        let order = match next {
            Ok(order) => Some(order),
            // Only a look at the output while it rings, or another try to open it.
            Err(mpsc::RecvTimeoutError::Timeout) if ringing.is_some() => None,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                output = None;
                clear(&queue);
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        // A ring that stopped by itself ([`RING_LIMIT`]) rings no more.
        if ringing.is_some()
            && output.is_some()
            && queue
                .lock()
                .is_ok_and(|q| q.rings.iter().all(|r| r.left.is_some()))
        {
            ringing = None;
        }
        // Nor does one whose output never opened in all that time.
        if ringing.is_some()
            && output.is_none()
            && ringing_since.elapsed().as_secs_f64() >= RING_LIMIT
        {
            ringing = None;
        }
        if let Some(Order::Ring(sound)) = order {
            if sound == ringing && output.is_some() {
                continue;
            }
            ringing = sound;
            ringing_since = Instant::now();
            if sound.is_none() {
                if let Ok(mut q) = queue.lock() {
                    q.ring(None);
                }
                continue;
            }
        }
        let chosen = chosen_output().1;
        let now = Instant::now();
        let took = queue.lock().map(|q| q.taken).unwrap_or(0);
        if took != taken.0 {
            taken = (took, now);
        }
        let stuck = output.as_ref().is_some_and(|o| {
            o.broken.load(Ordering::Relaxed)
                || o.chosen != chosen
                || (now > o.played_by + Duration::from_secs(1) && !is_empty(&queue))
                || (ringing.is_some() && now > taken.1 + check)
        });
        if stuck {
            tracing::debug!("sound: opening the output again");
            output = None;
            clear(&queue);
        }
        let mut opened = false;
        if output.is_none() {
            match open(queue.clone()) {
                Ok(new) => {
                    prepare_at(new.rate);
                    output = Some(new);
                    opened = true;
                    taken = (queue.lock().map(|q| q.taken).unwrap_or(0), Instant::now());
                }
                Err(e) if cfg!(target_os = "linux") => {
                    warn_once(&format!(
                        "cannot open the output: {e} (if ALSA does not reach the sound \
                         server: {NO_ROUTE})"
                    ));
                    continue;
                }
                Err(e) => {
                    warn_once(&format!("cannot open the output: {e}"));
                    continue;
                }
            }
        }
        let Some(out) = output.as_mut() else {
            continue;
        };
        // Made before taking the queue: a stream callback kept waiting falls behind, and
        // the output then drops what it was given (the start of the sound).
        let ring = match ringing {
            Some(sound) if opened || matches!(order, Some(Order::Ring(_))) => {
                let mut ring = Ring::new(made(sound, out.rate), out.rate);
                let late = ringing_since.elapsed().as_secs_f64() * f64::from(out.rate);
                ring.limit = ring.limit.saturating_sub(late as usize);
                Some(ring)
            }
            _ => None,
        };
        let samples = match order {
            Some(Order::Play(sound)) => Some(made(sound, out.rate)),
            _ => None,
        };
        if let Ok(mut q) = queue.lock() {
            if ring.is_some() {
                q.ring(ring);
            }
            if let Some(samples) = samples {
                mix_into(&mut q.queue, &samples);
                let seconds = q.queue.len() as f32 / out.rate as f32;
                out.played_by = Instant::now() + Duration::from_secs_f32(seconds);
            }
        }
    }
}

fn clear(queue: &Queue) {
    if let Ok(mut q) = queue.lock() {
        q.clear();
    }
}

fn is_empty(queue: &Queue) -> bool {
    queue.lock().map(|q| q.queue.is_empty()).unwrap_or(true)
}

/// Opens the chosen output, or the default one while it is not connected (or on Linux),
/// playing from `queue`.
fn open(queue: Queue) -> Result<Output, Box<dyn Error>> {
    let (id, chosen) = chosen_output();
    let host = cpal::default_host();
    check_route(&host);
    let device = id
        .filter(|_| !cfg!(target_os = "linux"))
        .and_then(|id| named_output(&host, &id))
        .or_else(|| host.default_output_device())
        .ok_or("no output device")?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate;
    let broken = Arc::new(AtomicBool::new(false));
    let stream =
        crate::voice::by_sample_type!(format, stream(&device, &config, queue, broken.clone()))
            .ok_or_else(|| format!("unsupported sample format {format}"))??;
    stream.play()?;
    tracing::debug!(
        "sound: output open at {rate} Hz, {} channels, {format}",
        config.channels
    );
    Ok(Output {
        _stream: Some(stream),
        rate,
        broken,
        played_by: Instant::now(),
        chosen,
    })
}

/// The output the voice engine calls `id`: Windows and macOS name a device by the id the
/// system gives it (its endpoint id, its Core Audio uid), as cpal does; failing that, by
/// its name.
pub(crate) fn named_output(host: &cpal::Host, id: &str) -> Option<cpal::Device> {
    let devices: Vec<cpal::Device> = host.output_devices().ok()?.collect();
    let by_id = devices
        .iter()
        .position(|d| d.id().is_ok_and(|d| d.id() == id));
    let by_name = || {
        devices
            .iter()
            .position(|d| d.description().is_ok_and(|d| d.name() == id))
    };
    let at = by_id.or_else(by_name)?;
    devices.into_iter().nth(at)
}

/// What `channel` of `channels` plays of a frame: one channel alone gets both sides mixed;
/// with more, the two front channels get left and right, and the others (centre, surround,
/// the further outputs of a sound card) stay silent.
fn on_channel([left, right]: Frame, channel: usize, channels: usize) -> f32 {
    match (channels, channel) {
        (1, _) => 0.5 * (left + right),
        (_, 0) => left,
        (_, 1) => right,
        _ => 0.0,
    }
}

fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Queue,
    broken: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::Error> {
    let channels = config.channels as usize;
    device.build_output_stream(
        *config,
        move |data: &mut [T], _| {
            // The sound thread holds the queue only to add samples already made.
            let mut q = queue.lock().ok();
            for frame in data.chunks_mut(channels) {
                let [left, right] = q.as_mut().map_or([0.0; 2], |q| q.next());
                let played = [limit(left), limit(right)];
                for (channel, slot) in frame.iter_mut().enumerate() {
                    *slot = T::from_sample(on_channel(played, channel, channels));
                }
            }
        },
        move |e| match e.kind() {
            // A late buffer, a route the system followed by itself, or no real-time
            // priority: the stream plays on.
            cpal::ErrorKind::Xrun
            | cpal::ErrorKind::DeviceChanged
            | cpal::ErrorKind::RealtimeDenied => tracing::debug!("sound: {e}"),
            _ => {
                broken.store(true, Ordering::Relaxed);
                warn_once(&format!("the output stopped: {e}"));
            }
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(frames: &[Frame]) -> f32 {
        frames
            .iter()
            .fold(0.0f32, |m, f| m.max(f[0].abs()).max(f[1].abs()))
    }

    fn widened(frames: &[Frame]) -> Vec<[f64; 2]> {
        frames
            .iter()
            .map(|f| [f64::from(f[0]), f64::from(f[1])])
            .collect()
    }

    /// Seconds until the sound falls for good to `db` under its peak.
    fn heard_for(frames: &[Frame], rate: u32, db: f32) -> f32 {
        let floor = peak(frames) * 10f32.powf(db / 20.0);
        let last = frames
            .iter()
            .rposition(|f| f[0].abs().max(f[1].abs()) > floor)
            .unwrap_or(0);
        last as f32 / rate as f32
    }

    /// Every variant of every sound.
    fn every_variant() -> impl Iterator<Item = (Sound, Variant)> {
        Sound::ALL
            .into_iter()
            .flat_map(|s| variants(s).iter().map(move |&v| (s, v)))
    }

    /// The longest a sound is heard, down to 30 dB under its peak. A call's rings repeat
    /// while it rings, and a phrase says more than a note; the notice and the end of a
    /// call ring a little longer than the rest; a ring played on an instrument fills the
    /// bar between rings, and its last notes ring a little into the next.
    fn longest(sound: Sound, variant: Variant) -> f32 {
        if songs::song(sound, variant).is_some() {
            return songs::SONG_EVERY as f32 + 1.2;
        }
        match (sound, variant) {
            (Sound::Ring | Sound::Ringback, _) => 1.4,
            (Sound::Notice | Sound::CallEnded, _) => 0.6,
            _ => 0.42,
        }
    }

    /// Whether the variant goes through a chorus, a room and an EQ.
    fn spaced(sound: Sound, variant: Variant) -> bool {
        songs::song(sound, variant).is_some() || style(sound, variant).space.is_some()
    }

    #[test]
    fn every_variant_is_short_clean_and_under_full_scale() {
        for rate in [44_100, 48_000, 96_000] {
            for (sound, variant) in every_variant() {
                let frames = render_as(sound, variant, rate);
                let what = format!("{sound:?} {variant:?} at {rate} Hz");
                // Heard for no longer than before; the room's quiet tail adds up to 0.7 s
                // more to the whole, a song's longer room and last notes up to 2 s.
                let seconds = frames.len() as f32 / rate as f32;
                let tail = if songs::song(sound, variant).is_some() {
                    2.0
                } else if spaced(sound, variant) {
                    0.7
                } else {
                    0.0
                };
                assert!(
                    (0.1..longest(sound, variant) + tail).contains(&seconds),
                    "{what} lasts {seconds} s"
                );
                let heard = heard_for(&frames, rate, -30.0);
                assert!(
                    heard < longest(sound, variant),
                    "{what} is heard for {heard} s"
                );
                assert!(frames.iter().flatten().all(|s| s.is_finite()), "{what}");
                // Alone, a sound never reaches the limiter's bend, chorus and room included.
                let peak = peak(&frames);
                assert!((0.1..KNEE).contains(&peak), "{what} peaks at {peak}");
                assert!(peak <= MAX_PEAK as f32, "{what} peaks at {peak}");
                for side in 0..2 {
                    let side: Vec<f32> = frames.iter().map(|f| f[side]).collect();
                    // Starts and ends at rest, and never jumps: a jump is a click.
                    assert!(side[0].abs() < 1e-3, "{what}");
                    assert!(side[side.len() - 1].abs() < 1e-3, "{what}");
                    let jump = side
                        .windows(3)
                        .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
                        .fold(0.0f32, f32::max)
                        * (rate as f32 / 48_000.0).powi(2);
                    assert!(jump < 0.03, "{what} jumps by {jump}");
                    // Swings evenly about zero.
                    let mean = side.iter().sum::<f32>() / side.len() as f32;
                    assert!(mean.abs() < 1e-3, "{what} sits at {mean}");
                }
            }
        }
    }

    #[test]
    fn the_two_sides_are_about_as_loud() {
        for (sound, variant) in every_variant() {
            let frames = render_as(sound, variant, 48_000);
            let energy = |k: usize| frames.iter().map(|f| f64::from(f[k]).powi(2)).sum::<f64>();
            let db = 10.0 * (energy(0) / energy(1)).log10();
            assert!(db.abs() < 1.0, "{sound:?} {variant:?}: {db} dB");
            if !spaced(sound, variant) {
                assert!(frames.iter().all(|f| f[0] == f[1]), "{sound:?} {variant:?}");
            }
        }
    }

    #[test]
    fn most_sounds_are_heard_for_under_400_ms() {
        for set in [Variant::A, Variant::B, Variant::C] {
            let short = Sound::ALL
                .iter()
                .filter(|&&sound| heard_for(&render_as(sound, set, 48_000), 48_000, -30.0) < 0.4)
                .count();
            assert!(short >= 8, "only {short} sounds of {set:?} under 400 ms");
        }
        let short = Sound::ALL
            .iter()
            .filter(|&&sound| render(sound, 48_000).len() < 19_200 + 28_800)
            .count();
        assert!(short >= 8, "only {short} sounds end within 1 s");
    }

    #[test]
    fn sounds_are_about_as_loud_as_each_other() {
        for rate in [44_100, 48_000] {
            for (sound, variant) in every_variant() {
                let frames = widened(&render_as(sound, variant, rate));
                let level = loudness(&frames, f64::from(rate));
                assert!(
                    (level - loudness_of(sound, variant)).abs() < 0.5,
                    "{sound:?} {variant:?} at {rate} Hz: {level} LUFS"
                );
                assert!((-24.0..=-22.0).contains(&loudness_of(sound, variant)));
            }
        }
    }

    #[test]
    fn each_sound_is_its_own() {
        let all: Vec<Vec<Frame>> = Sound::ALL.iter().map(|&s| render(s, 48_000)).collect();
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
        for sound in Sound::ALL {
            assert!(variants(sound).contains(&chosen(sound)), "{sound:?}");
            let all: Vec<Vec<Frame>> = variants(sound)
                .iter()
                .map(|&v| render_as(sound, v, 48_000))
                .collect();
            for (i, a) in all.iter().enumerate() {
                for b in &all[i + 1..] {
                    assert_ne!(a, b, "{sound:?}");
                }
            }
        }
    }

    #[test]
    fn the_room_rings_for_about_half_a_second() {
        // An impulse through the room alone: 60 dB down after about `ROOM.room`.
        let rate = 48_000.0;
        let mut room = Room::new(rate, ROOM.room);
        let response: Vec<f64> = (0..(2.0 * rate) as usize)
            .map(|i| {
                let [l, r] = room.next(if i == 0 { 1.0 } else { 0.0 });
                l * l + r * r
            })
            .collect();
        let energy_from = |s: f64| response[(s * rate) as usize..].iter().sum::<f64>();
        let early = energy_from(0.05);
        let fall = 10.0 * (early / energy_from(0.05 + ROOM.room / 2.0)).log10();
        // Half the time to fall 60 dB: about 30 dB, the highs falling faster.
        assert!((24.0..40.0).contains(&fall), "{fall} dB");
        assert!(response.iter().all(|e| e.is_finite()));
    }

    #[test]
    fn one_channel_gets_both_sides() {
        assert_eq!(on_channel([0.2, 0.4], 0, 1), 0.3);
        assert_eq!(on_channel([0.2, 0.4], 0, 2), 0.2);
        assert_eq!(on_channel([0.2, 0.4], 1, 2), 0.4);
        assert_eq!(on_channel([0.2, 0.4], 2, 6), 0.0);
    }

    #[test]
    fn loudness_reads_as_a_meter_does() {
        // A 997 Hz sine at full scale reads -3.01 LUFS; 20 dB lower, -23.01.
        let rate = 48_000.0;
        let sine: Vec<[f64; 2]> = (0..48_000)
            .map(|i| [0.1 * (TAU * 997.0 * i as f64 / rate).sin(); 2])
            .collect();
        let level = loudness(&sine, rate);
        assert!((level + 23.01).abs() < 0.1, "{level}");
        // On one side only, half the power: 3 dB less.
        let one: Vec<[f64; 2]> = sine.iter().map(|f| [f[0], 0.0]).collect();
        assert!((loudness(&one, rate) - level + 3.01).abs() < 0.05);
    }

    /// How often the rings would start again while a call rings: the player does it every
    /// bar ([`RING_EVERY`]), which a ring played on an instrument needs.
    fn every(sound: Sound, variant: Variant) -> Option<f64> {
        match sound {
            _ if songs::song(sound, variant).is_some() => Some(songs::SONG_EVERY),
            Sound::Ring => Some(2.6),
            Sound::Ringback => Some(3.2),
            _ => None,
        }
    }

    #[test]
    fn songs_repeat_under_the_limiter() {
        // Each ring over the tail of the one before, as the app plays them.
        for (sound, variant) in every_variant() {
            let Some(every) = every(sound, variant) else {
                continue;
            };
            let frames = render_as(sound, variant, 48_000);
            let mut queue = VecDeque::new();
            mix_into(&mut queue, &frames);
            let mut second = vec![[0.0; 2]; (every * 48_000.0) as usize];
            second.extend_from_slice(&frames);
            mix_into(&mut queue, &second);
            let both: Vec<Frame> = queue.into_iter().collect();
            assert!(peak(&both) < KNEE, "{sound:?} {variant:?}: {}", peak(&both));
        }
    }

    /// The frequency of the strongest peak of `samples` between `low` and `high` Hz.
    fn strongest(samples: &[f64], rate: f64, low: f64, high: f64) -> f64 {
        const N: usize = 1 << 17;
        let mut re: Vec<f64> = (0..N)
            .map(|k| {
                let x = samples.get(k).copied().unwrap_or(0.0);
                x * (0.5 - 0.5 * (TAU * k as f64 / samples.len().min(N) as f64).cos())
                    * f64::from(u8::from(k < samples.len()))
            })
            .collect();
        let mut im = vec![0.0; N];
        fft(&mut re, &mut im);
        let power = |k: usize| (re[k] * re[k] + im[k] * im[k]).sqrt().max(1e-30).ln();
        let bin = rate / N as f64;
        let k = ((low / bin) as usize..(high / bin) as usize)
            .max_by(|&a, &b| power(a).total_cmp(&power(b)))
            .unwrap();
        // Between bins, by the parabola through the peak and its neighbours.
        let (a, b, c) = (power(k - 1), power(k), power(k + 1));
        (k as f64 + 0.5 * (a - c) / (a - 2.0 * b + c)) * bin
    }

    #[test]
    fn instruments_play_in_tune() {
        let rate = 48_000.0;
        for instrument in [
            songs::Instrument::Marimba,
            songs::Instrument::Vibraphone,
            songs::Instrument::Pad,
            songs::Instrument::Guitar,
            songs::Instrument::WarmGuitar,
            songs::Instrument::Harp,
            songs::Instrument::FeltPiano,
            songs::Instrument::Keys,
        ] {
            for hertz in [110.0, 220.0, 440.0, 659.26, 880.0] {
                let mut noise = songs::Noise::new(7);
                let note = songs::play_note(instrument, hertz, 0.7, 1.0, 0.0, rate, &mut noise);
                let found = strongest(&note, rate, hertz * 0.94, hertz * 1.06);
                let cents = 1200.0 * (found / hertz).log2();
                // The pad's two copies sit 6 cents either side of the note.
                let off = if instrument == songs::Instrument::Pad {
                    7.0
                } else {
                    3.0
                };
                assert!(
                    cents.abs() < off,
                    "{instrument:?} at {hertz} Hz: {cents} cents"
                );
                assert!(note[0].abs() < 1e-3, "{instrument:?} at {hertz} Hz");
                assert!(note.last().is_some_and(|s| s.abs() < 1e-3));
            }
        }
    }

    /// In place, the discrete Fourier transform of a power-of-two `re` + i `im`.
    fn fft(re: &mut [f64], im: &mut [f64]) {
        let n = re.len();
        let mut j = 0;
        for i in 1..n {
            let mut bit = n >> 1;
            while j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j |= bit;
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let angle = -TAU / len as f64;
            for start in (0..n).step_by(len) {
                for k in 0..len / 2 {
                    let (s, c) = (angle * k as f64).sin_cos();
                    let (a, b) = (start + k, start + k + len / 2);
                    let (xr, xi) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                    re[b] = re[a] - xr;
                    im[b] = im[a] - xi;
                    re[a] += xr;
                    im[a] += xi;
                }
            }
            len <<= 1;
        }
    }

    /// The sound's spectral centroid (Hz, power-weighted) and the share of its power
    /// between 2 and 5 kHz, over both sides, in Hann windows of 4096 half overlapping.
    fn spectrum(frames: &[Frame], rate: f64) -> (f64, f64) {
        const N: usize = 4096;
        let mut power = vec![0.0f64; N / 2];
        for side in 0..2 {
            let mut start = 0;
            while start < frames.len() {
                let mut re: Vec<f64> = (0..N)
                    .map(|k| {
                        let x = frames.get(start + k).map_or(0.0, |f| f64::from(f[side]));
                        x * (0.5 - 0.5 * (TAU * k as f64 / N as f64).cos())
                    })
                    .collect();
                let mut im = vec![0.0; N];
                fft(&mut re, &mut im);
                for (k, p) in power.iter_mut().enumerate() {
                    *p += re[k] * re[k] + im[k] * im[k];
                }
                start += N / 2;
            }
        }
        let hertz = |k: usize| k as f64 * rate / N as f64;
        let total: f64 = power.iter().skip(1).sum();
        let centroid = power
            .iter()
            .enumerate()
            .map(|(k, p)| hertz(k) * p)
            .sum::<f64>()
            / total;
        let band = power
            .iter()
            .enumerate()
            .filter(|&(k, _)| (2_000.0..5_000.0).contains(&hertz(k)))
            .map(|(_, p)| p)
            .sum::<f64>();
        (centroid, band / total)
    }

    #[test]
    fn the_spectrum_finds_a_sine() {
        let frames: Vec<Frame> = (0..48_000)
            .map(|i| {
                let s = (0.3 * (TAU * 3_000.0 * i as f64 / 48_000.0).sin()) as f32;
                [s, s]
            })
            .collect();
        let (centroid, band) = spectrum(&frames, 48_000.0);
        assert!((centroid - 3_000.0).abs() < 20.0, "{centroid}");
        assert!(band > 0.99, "{band}");
    }

    fn wav(frames: &[Frame]) -> Vec<u8> {
        let mut wav = Vec::with_capacity(44 + frames.len() * 4);
        let data = (frames.len() * 4) as u32;
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        for field in [16u32, 1 | (2 << 16), 48_000, 192_000, 4 | (16 << 16)] {
            wav.extend_from_slice(&field.to_le_bytes());
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data.to_le_bytes());
        for s in frames.iter().flatten() {
            wav.extend_from_slice(&((s * 32_767.0).round() as i16).to_le_bytes());
        }
        wav
    }

    /// Writes every variant of every sound as a 48 kHz stereo WAV file into the folder
    /// named by `FOGATA_SOUNDS_OUT`, to listen to them side by side (the rings twice, as
    /// they repeat), with `measures.tsv`: each one's length, how long it is heard, its
    /// loudness, peak, spectral centroid and share of power between 2 and 5 kHz.
    #[test]
    #[ignore = "writes files; run by hand with FOGATA_SOUNDS_OUT set and --ignored"]
    fn writes_every_variant() {
        let dir = std::path::PathBuf::from(
            std::env::var("FOGATA_SOUNDS_OUT").expect("FOGATA_SOUNDS_OUT names a folder"),
        );
        std::fs::create_dir_all(&dir).unwrap();
        let mut measures = String::from(
            "sound\tvariant\tseconds\theard_s\tlufs\tpeak_dbfs\tcentroid_hz\tshare_2_5k\n",
        );
        for (sound, variant) in every_variant() {
            let frames = render_as(sound, variant, 48_000);
            let (centroid, band) = spectrum(&frames, 48_000.0);
            measures.push_str(&format!(
                "{sound:?}\t{variant:?}\t{:.3}\t{:.3}\t{:.2}\t{:.2}\t{centroid:.0}\t{band:.4}\n",
                frames.len() as f32 / 48_000.0,
                heard_for(&frames, 48_000, -30.0),
                loudness(&widened(&frames), 48_000.0),
                20.0 * peak(&frames).log10(),
            ));
            let played: Vec<Frame> = match every(sound, variant) {
                Some(every) => {
                    let mut queue = VecDeque::new();
                    mix_into(&mut queue, &frames);
                    let later = (every * 48_000.0) as usize;
                    let mut second = vec![[0.0; 2]; later];
                    second.extend_from_slice(&frames);
                    mix_into(&mut queue, &second);
                    queue.into_iter().collect()
                }
                None => frames,
            };
            std::fs::write(dir.join(format!("{sound:?}-{variant:?}.wav")), wav(&played)).unwrap();
        }
        std::fs::write(dir.join("measures.tsv"), measures).unwrap();
    }

    /// Plays every sound on this computer's default output the way the application does,
    /// one every 1.5 s, then a ring with a message and a notice over it.
    #[test]
    #[ignore = "makes noise; run by hand with --ignored"]
    fn plays_on_the_default_output() {
        for sound in Sound::ALL {
            play(sound);
            thread::sleep(Duration::from_millis(1500));
        }
        play(Sound::Ring);
        thread::sleep(Duration::from_millis(300));
        play(Sound::Message);
        thread::sleep(Duration::from_millis(300));
        play(Sound::Notice);
        thread::sleep(Duration::from_millis(1500));
    }

    #[test]
    fn a_route_to_the_sound_server_is_an_alsa_plugin_for_it() {
        assert!(reaches_sound_server(["default", "null", "pipewire"]));
        assert!(reaches_sound_server(["pulse", "hw:CARD=0,DEV=0"]));
        assert!(reaches_sound_server(["pipewire:NODE=fogata"]));
        assert!(!reaches_sound_server([
            "default",
            "sysdefault:CARD=PCH",
            "hw:CARD=PCH,DEV=0",
            "plughw:CARD=PCH,DEV=0",
            "pipewire_like",
        ]));
        assert!(!reaches_sound_server([]));
    }

    #[test]
    fn each_kind_of_failure_is_warned_about_once() {
        warn_once("test failure a");
        warn_once("test failure a");
        warn_once("test failure b");
        let warned = WARNED.lock().unwrap();
        assert_eq!(warned.iter().filter(|w| *w == "test failure a").count(), 1);
        assert!(warned.iter().any(|w| w == "test failure b"));
        assert!(warned.len() <= WARN_KINDS);
    }

    #[test]
    fn loud_mixes_bend_under_full_scale() {
        assert_eq!(limit(0.5), 0.5);
        assert_eq!(limit(-0.7), -0.7);
        let mut last = 0.0;
        for i in 1..=400 {
            let level = i as f32 / 100.0;
            let bent = limit(level);
            assert!(bent <= 1.0 && bent >= last, "{level} -> {bent}");
            assert_eq!(limit(-level), -bent);
            last = bent;
        }
    }

    #[test]
    fn overlapping_sounds_are_mixed_not_queued() {
        let mut queue = VecDeque::new();
        mix_into(&mut queue, &[[0.1, 0.0], [0.2, 0.1], [0.3, 0.2]]);
        mix_into(&mut queue, &[[0.1, 0.1], [0.1, 0.1]]);
        assert_eq!(
            queue.iter().copied().collect::<Vec<_>>(),
            vec![[0.2, 0.1], [0.3, 0.2], [0.3, 0.2]]
        );
        queue.pop_front();
        mix_into(&mut queue, &[[1.0, 0.5], [1.0, 0.5], [1.0, 0.5]]);
        assert_eq!(queue.len(), 3);
        assert!((queue[0][0] - 1.3).abs() < 1e-6);
        assert!((queue[0][1] - 0.7).abs() < 1e-6);
    }
    fn ring_of(bar: Vec<Frame>, every: usize, fade: usize) -> Ring {
        Ring {
            bar: bar.into(),
            every,
            at: 0,
            limit: usize::MAX,
            played: 0,
            fade,
            left: None,
        }
    }

    #[test]
    fn a_ring_starts_again_every_bar_over_the_tail_of_the_one_before() {
        // A bar of five frames every three: each over the last two of the one before.
        let bar = (1..=5).map(|k| [k as f32, -(k as f32)]).collect();
        let mut mix = Mix::default();
        mix.ring(Some(ring_of(bar, 3, 1)));
        let played: Vec<Frame> = (0..2000).map(|_| mix.next()).collect();
        let left: Vec<f32> = played[..12].iter().map(|f| f[0]).collect();
        assert_eq!(left, [1., 2., 3., 5., 7., 3., 5., 7., 3., 5., 7., 3.]);
        // Long after, still in step.
        for (k, frame) in played.iter().enumerate().skip(3) {
            let want = [5., 7., 3.][k % 3];
            assert_eq!(*frame, [want, -want], "frame {k}");
        }
    }

    #[test]
    fn the_ring_s_bars_line_up_however_long_it_took_to_make() {
        let bar = render(Sound::Ring, 48_000);
        let mut mix = Mix::default();
        mix.ring(Some(Ring::new(bar.clone().into(), 48_000)));
        let every = (RING_EVERY * 48_000.0) as usize;
        assert!(bar.len() > every && bar.len() < 2 * every);
        let played: Vec<Frame> = (0..3 * every).map(|_| mix.next()).collect();
        for k in (0..3 * every).step_by(97) {
            let mut want = [0.0f32; 2];
            for start in [0, every, 2 * every] {
                if let Some(f) = k.checked_sub(start).and_then(|i| bar.get(i)) {
                    want[0] += f[0];
                    want[1] += f[1];
                }
            }
            assert!(
                (played[k][0] - want[0]).abs() < 1e-6 && (played[k][1] - want[1]).abs() < 1e-6,
                "frame {k}"
            );
        }
    }

    #[test]
    fn a_stopped_ring_fades_out_at_once_and_the_other_sounds_play_on() {
        let mut mix = Mix::default();
        mix.ring(Some(ring_of(vec![[0.5, 0.5]; 1000], 1000, 10)));
        for _ in 0..100 {
            assert_eq!(mix.next(), [0.5, 0.5]);
        }
        mix_into(&mut mix.queue, &[[0.25, 0.25]; 40]);
        mix.ring(None);
        let mut last = 0.75;
        for k in 0..10 {
            let [left, right] = mix.next();
            assert_eq!(left, right);
            assert!(left < last && left >= 0.25, "frame {k}: {left}");
            last = left;
        }
        assert_eq!(mix.next(), [0.25, 0.25]);
        assert!(mix.rings.is_empty());
        for _ in 0..20 {
            assert_eq!(mix.next(), [0.25, 0.25]);
        }
    }

    #[test]
    fn a_new_ring_ends_the_one_before() {
        let mut mix = Mix::default();
        mix.ring(Some(ring_of(vec![[1.0, 0.0]; 1000], 1000, 4)));
        mix.next();
        mix.ring(Some(ring_of(vec![[0.0, 1.0]; 1000], 1000, 4)));
        let played: Vec<Frame> = (0..8).map(|_| mix.next()).collect();
        assert!(played.iter().all(|f| f[1] == 1.0));
        assert!(played[0][0] > 0.5 && played[3][0] < 0.5 && played[4][0] == 0.0);
        assert_eq!(mix.rings.len(), 1);
    }

    #[test]
    fn a_sound_is_made_once_for_each_rate() {
        let first = made(Sound::Join, 22_050);
        let again = made(Sound::Join, 22_050);
        assert!(Arc::ptr_eq(&first, &again));
        assert!(is_made(Sound::Join, 22_050));
        assert!(!Arc::ptr_eq(&first, &made(Sound::Join, 24_000)));
    }

    #[test]
    fn a_ring_nobody_stops_stops_by_itself() {
        assert_eq!(RING_LIMIT, 60.0);
        let mut ring = Ring::new(vec![[0.5, 0.5]; 200].into(), 48_000);
        assert_eq!(ring.limit, 60 * 48_000);
        // The same, shortened: 100 frames, then the fade.
        ring.limit = 100;
        ring.fade = 10;
        let mut mix = Mix::default();
        mix.ring(Some(ring));
        let played: Vec<f32> = (0..130).map(|_| mix.next()[0]).collect();
        assert!(played[..100].iter().all(|&x| x == 0.5));
        assert!(played[100..110].windows(2).all(|w| w[1] < w[0]));
        assert!(played[110..].iter().all(|&x| x == 0.0));
        assert!(mix.rings.is_empty());
    }

    #[test]
    fn a_ring_whose_output_does_not_open_starts_once_it_does() {
        let (tx, rx) = mpsc::channel();
        let opened: Arc<Mutex<Vec<Queue>>> = Arc::default();
        let tries = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let player = {
            let (opened, tries) = (opened.clone(), tries.clone());
            thread::spawn(move || {
                player(
                    rx,
                    move |queue| {
                        // Busy three times, then open.
                        if tries.fetch_add(1, Ordering::SeqCst) < 3 {
                            return Err("busy".into());
                        }
                        opened.lock().unwrap().push(queue);
                        Ok(Output {
                            _stream: None,
                            rate: 8_000,
                            broken: Arc::default(),
                            played_by: Instant::now(),
                            chosen: chosen_output().1,
                        })
                    },
                    Duration::from_millis(20),
                )
            })
        };
        let asked = Instant::now();
        tx.send(Order::Ring(Some(Sound::Ringback))).unwrap();
        let queue = loop {
            if let Some(queue) = opened.lock().unwrap().first() {
                break queue.clone();
            }
            assert!(asked.elapsed() < Duration::from_secs(10), "never opened");
            thread::sleep(Duration::from_millis(5));
        };
        // Played as an output would, the ring starts at once, from its first frame.
        let mut ring = None;
        while ring.is_none() {
            assert!(asked.elapsed() < Duration::from_secs(20), "no ring");
            thread::sleep(Duration::from_millis(5));
            let mut q = queue.lock().unwrap();
            ring = q.rings.first().map(|r| (r.at, r.limit));
            q.next();
        }
        let (at, limit) = ring.unwrap();
        assert_eq!(at, 0);
        assert!(tries.load(Ordering::SeqCst) >= 4);
        // Its minute counts from when it was asked for.
        let full = (RING_LIMIT * 8_000.0) as usize;
        assert!(limit < full - 3 * 160, "{limit} of {full}");
        assert!(limit > full - asked.elapsed().as_millis() as usize * 8 - 8);
        drop(tx);
        player.join().unwrap();
    }
}
