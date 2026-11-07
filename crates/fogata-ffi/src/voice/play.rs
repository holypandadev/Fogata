//! Playing a voice message on the output chosen in the application, as the short sounds
//! are (`sounds.rs`): one message at a time, at 1×, 1.5× or 2×. Paused, the message stays
//! ready where it was; the output closes a few seconds after nothing plays.
//!
//! The output's callback decodes the message while it holds the player alone; what the
//! screen asks for every few frames (what is loaded, where it is, whether it plays) is kept
//! apart and read without the player, so it never waits for the decoding.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use fogata_voice::{Linear, Player, RATE};

use crate::sounds;

/// How long the output stays open with nothing playing.
const KEEP_OPEN: Duration = Duration::from_secs(4);

/// Where the message loaded is and whether it plays: set as it plays, read by the screen.
#[derive(Debug, Default)]
struct Progress {
    playing: AtomicBool,
    position_ms: AtomicU64,
    duration_ms: AtomicU64,
}

impl Progress {
    fn of(player: &Player) -> Arc<Self> {
        let progress = Arc::new(Progress::default());
        progress.follow(player);
        progress
    }

    /// Takes where `player` is now.
    fn follow(&self, player: &Player) {
        self.position_ms
            .store(player.position_ms(), Ordering::Relaxed);
        self.duration_ms
            .store(player.duration_ms(), Ordering::Relaxed);
    }
}

/// The message loaded, as the output's callback plays it.
struct Loaded {
    player: Player,
    progress: Arc<Progress>,
}

/// The message loaded, as the screen sees it.
struct Shown {
    /// The attachment's id, and its chat's friend.
    file: String,
    friend: String,
    speed: f64,
    progress: Arc<Progress>,
}

/// What plays: the player, held while it decodes, and what is shown of it. Whatever holds
/// both takes the player first.
#[derive(Default)]
struct Voice {
    player: Mutex<Option<Loaded>>,
    shown: Mutex<Option<Shown>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn voice() -> &'static Arc<Voice> {
    static VOICE: OnceLock<Arc<Voice>> = OnceLock::new();
    VOICE.get_or_init(Arc::default)
}

impl Voice {
    fn is_loaded(&self, friend: &str, file: &str) -> bool {
        lock(&self.shown)
            .as_ref()
            .is_some_and(|s| s.file == file && s.friend == friend)
    }

    /// Loads `player` as `file` of `friend`, at `speed`, in place of whatever was loaded.
    fn load(&self, friend: String, file: String, mut player: Player, speed: f64, playing: bool) {
        player.set_speed(speed);
        let progress = Progress::of(&player);
        progress.playing.store(playing, Ordering::Relaxed);
        let mut loaded = lock(&self.player);
        let before = loaded.replace(Loaded {
            player,
            progress: progress.clone(),
        });
        *lock(&self.shown) = Some(Shown {
            file,
            friend,
            speed,
            progress,
        });
        drop(loaded);
        drop(before);
    }

    /// Plays what is loaded from where it is, at `speed`, from its start once it ended.
    fn resume(&self, speed: f64) {
        let mut loaded = lock(&self.player);
        let Some(l) = loaded.as_mut() else {
            return;
        };
        if l.player.ended() {
            l.player.rewind();
        }
        l.player.set_speed(speed);
        l.progress.follow(&l.player);
        l.progress.playing.store(true, Ordering::Relaxed);
        if let Some(s) = lock(&self.shown).as_mut() {
            s.speed = speed;
        }
    }

    fn pause(&self) {
        if let Some(s) = lock(&self.shown).as_ref() {
            s.progress.playing.store(false, Ordering::Relaxed);
        }
    }

    fn set_speed(&self, speed: f64) {
        let mut loaded = lock(&self.player);
        if let Some(l) = loaded.as_mut() {
            l.player.set_speed(speed);
            l.progress.follow(&l.player);
        }
        if let Some(s) = lock(&self.shown).as_mut() {
            s.speed = speed;
        }
    }

    fn stop(&self) {
        let before = lock(&self.player).take();
        *lock(&self.shown) = None;
        drop(before);
    }

    fn state(&self) -> Option<Playing> {
        lock(&self.shown).as_ref().map(|s| Playing {
            file: s.file.clone(),
            friend: s.friend.clone(),
            position_ms: s.progress.position_ms.load(Ordering::Relaxed),
            duration_ms: s.progress.duration_ms.load(Ordering::Relaxed),
            playing: s.progress.playing.load(Ordering::Relaxed),
            speed: s.speed,
        })
    }

    fn playing_now(&self) -> bool {
        lock(&self.shown)
            .as_ref()
            .is_some_and(|s| s.progress.playing.load(Ordering::Relaxed))
    }
}

/// What the screen shows of the message playing or paused.
#[derive(Clone, Debug)]
pub(crate) struct Playing {
    pub file: String,
    pub friend: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub playing: bool,
    pub speed: f64,
}

/// Tells the output thread that something plays.
fn wake() {
    static THREAD: OnceLock<Mutex<mpsc::Sender<()>>> = OnceLock::new();
    let tx = THREAD.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        if let Err(e) = thread::Builder::new()
            .name("fogata-voice-play".into())
            .spawn(move || output(rx))
        {
            tracing::warn!("voice: no thread to play on: {e}");
        }
        Mutex::new(tx)
    });
    let _ = lock(tx).send(());
}

/// Whether `file` of `friend` is loaded (playing or paused).
pub(crate) fn is_loaded(friend: &str, file: &str) -> bool {
    voice().is_loaded(friend, file)
}

/// Plays the message loaded from where it is, at `speed`, from its start once it ended.
pub(crate) fn resume(speed: f64) {
    voice().resume(speed);
    wake();
}

/// Loads `bytes` as `file` of `friend` and plays it from its start at `speed`, in place of
/// whatever was loaded.
pub(crate) fn play(friend: String, file: String, bytes: Vec<u8>, speed: f64) -> Result<(), String> {
    let player = Player::new(&bytes).map_err(|e| e.to_string())?;
    voice().load(friend, file, player, speed, true);
    wake();
    Ok(())
}

/// A short message, as recorded here.
#[cfg(test)]
fn message(seconds: usize) -> Vec<u8> {
    let mut recorder = fogata_voice::Recorder::new().expect("an encoder");
    let pcm: Vec<f32> = (0..RATE as usize * seconds)
        .map(|i| 0.3 * (i as f32 * 0.05).sin())
        .collect();
    recorder.push(&pcm).expect("encoded");
    recorder
        .finish(1)
        .expect("finished")
        .expect("a message")
        .file
}

/// Loads a short message as playing without waking the output, so nothing is heard: for
/// tests of what stops it.
#[cfg(test)]
pub(crate) fn load_quietly(friend: &str, file: &str) {
    let player = Player::new(&message(1)).expect("a message");
    voice().load(friend.into(), file.into(), player, 1.0, true);
}

pub(crate) fn pause() {
    voice().pause();
}

pub(crate) fn set_speed(speed: f64) {
    voice().set_speed(speed);
}

/// Unloads the message: nothing is playing or paused.
pub(crate) fn stop() {
    voice().stop();
}

pub(crate) fn state() -> Option<Playing> {
    voice().state()
}

/// The output thread: opens the output when something plays and closes it once nothing
/// has for [`KEEP_OPEN`], or when it broke or another output was chosen.
fn output(wakes: mpsc::Receiver<()>) {
    let mut open: Option<(cpal::Stream, Arc<AtomicBool>, u64)> = None;
    let mut idle_since = Instant::now();
    loop {
        match wakes.recv_timeout(Duration::from_millis(250)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        let chosen = sounds::chosen_output().1;
        if open
            .as_ref()
            .is_some_and(|(_, broken, at)| broken.load(Ordering::Relaxed) || *at != chosen)
        {
            open = None;
        }
        if voice().playing_now() {
            idle_since = Instant::now();
            if open.is_none() {
                match open_output() {
                    Ok((stream, broken)) => open = Some((stream, broken, chosen)),
                    Err(e) => {
                        tracing::warn!("voice: cannot open the output: {e}");
                        pause();
                    }
                }
            }
        } else if open.is_some() && idle_since.elapsed() > KEEP_OPEN {
            open = None;
        }
    }
}

/// Opens the chosen output (on Linux the default, which the voice engine's side moves to
/// the chosen one), playing from what is loaded.
fn open_output() -> Result<(cpal::Stream, Arc<AtomicBool>), String> {
    let (id, _) = sounds::chosen_output();
    let host = cpal::default_host();
    let device = id
        .filter(|_| !cfg!(target_os = "linux"))
        .and_then(|id| sounds::named_output(&host, &id))
        .or_else(|| host.default_output_device())
        .ok_or("no output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    // A short buffer, so pausing and the time shown follow at once: what a device takes
    // by default can be most of a second.
    let short = match supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => {
            Some(short_buffer(supported.sample_rate(), *min, *max))
        }
        cpal::SupportedBufferSize::Unknown => None,
    };
    let mut config: cpal::StreamConfig = supported.into();
    let broken = Arc::new(AtomicBool::new(false));
    let build = |config: &cpal::StreamConfig| {
        super::by_sample_type!(format, stream(&device, config, broken.clone())).unwrap_or_else(
            || {
                tracing::warn!("voice: unsupported sample format {format}");
                Err(cpal::Error::from(cpal::ErrorKind::UnsupportedConfig))
            },
        )
    };
    let stream = match short {
        Some(frames) => {
            config.buffer_size = cpal::BufferSize::Fixed(frames);
            build(&config).or_else(|_| {
                config.buffer_size = cpal::BufferSize::Default;
                build(&config)
            })
        }
        None => build(&config),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    tracing::debug!(
        "voice: output open at {} Hz, {} channels",
        config.sample_rate,
        config.channels
    );
    Ok((stream, broken))
}

/// Frames of a buffer of about 40 ms at `rate`, within what the device says it takes.
fn short_buffer(rate: u32, min: u32, max: u32) -> u32 {
    // Not `clamp`, which panics when a device says its smallest is bigger than its biggest.
    (rate / 25).max(min).min(max)
}

/// Sound made for the output, at its rate: what the last call left over. Made with room for
/// all it holds, so the output's callback never waits for memory.
struct Feed {
    resampler: Linear,
    queue: VecDeque<f32>,
    chunk: Vec<f32>,
    changed: Vec<f32>,
}

/// Samples of the message made at once.
const CHUNK: usize = 480;

impl Feed {
    fn new(rate: u32) -> Self {
        // The most a chunk becomes: at the fastest rate a device may have, 8× as many.
        let most = CHUNK * 8 + 2;
        Feed {
            resampler: Linear::new(RATE, rate),
            queue: VecDeque::with_capacity(most),
            chunk: vec![0.0; CHUNK],
            changed: Vec::with_capacity(most),
        }
    }

    /// The next sample at the output's rate; silence while nothing plays.
    fn next(&mut self, loaded: &mut Option<Loaded>) -> f32 {
        if let Some(s) = self.queue.pop_front() {
            return s;
        }
        let Some(l) = loaded
            .as_mut()
            .filter(|l| l.progress.playing.load(Ordering::Relaxed))
        else {
            return 0.0;
        };
        let n = l.player.fill(&mut self.chunk);
        if n < self.chunk.len() {
            // The end: it shows as played to the end, paused.
            l.progress.playing.store(false, Ordering::Relaxed);
        }
        l.progress.follow(&l.player);
        self.changed.clear();
        self.resampler.push(&self.chunk[..n], &mut self.changed);
        self.queue.extend(self.changed.iter().copied());
        self.queue.pop_front().unwrap_or(0.0)
    }
}

/// Fills `data`, of `channels` interleaved, with what plays: the output's callback. It holds
/// the player alone while it decodes.
fn render<T: SizedSample + FromSample<f32>>(
    voice: &Voice,
    feed: &mut Feed,
    data: &mut [T],
    channels: usize,
) {
    let mut loaded = lock(&voice.player);
    for frame in data.chunks_mut(channels) {
        let s = sounds::limit(feed.next(&mut loaded));
        // Mono on the two front channels; the others stay silent.
        for (channel, slot) in frame.iter_mut().enumerate() {
            *slot = T::from_sample(if channel < 2 { s } else { 0.0 });
        }
    }
}

fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    broken: Arc<AtomicBool>,
) -> Result<cpal::Stream, cpal::Error> {
    let channels = usize::from(config.channels.max(1));
    let mut feed = Feed::new(config.sample_rate);
    let voice = voice().clone();
    device.build_output_stream(
        *config,
        move |data: &mut [T], _| render(&voice, &mut feed, data, channels),
        move |e| match e.kind() {
            cpal::ErrorKind::Xrun
            | cpal::ErrorKind::DeviceChanged
            | cpal::ErrorKind::RealtimeDenied => tracing::debug!("voice: {e}"),
            _ => {
                broken.store(true, Ordering::Relaxed);
                tracing::warn!("voice: the output stopped: {e}");
            }
        },
        None,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;

    use super::*;

    /// A device whose smallest buffer is said to be bigger than its biggest (cpal passes on
    /// what Core Audio and WASAPI's drivers report, unchecked) opens with a buffer within
    /// what it said, never stopping the thread that plays.
    #[test]
    fn a_device_that_misreports_its_buffers_still_plays() {
        assert_eq!(short_buffer(48_000, 64, 4096), 1920);
        assert_eq!(short_buffer(48_000, 4096, 8192), 4096);
        assert_eq!(short_buffer(48_000, 64, 512), 512);
        let odd = short_buffer(48_000, 4096, 512);
        assert!((512..=4096).contains(&odd), "{odd}");
    }

    /// What the screen asks for every few frames never waits for the decoding: while the
    /// output's callback holds the player, as it does all the time it decodes, what is
    /// loaded, where it is and whether it plays still come. Here the player is held until
    /// they came: were one of them to wait for it, they would never come.
    #[test]
    fn the_screen_never_waits_for_the_decoding() {
        let voice = Voice::default();
        let player = Player::new(&message(20)).unwrap();
        voice.load("friend".into(), "file".into(), player, 2.0, true);
        let (held, read) = (Barrier::new(2), mpsc::channel());
        thread::scope(|s| {
            s.spawn(|| {
                held.wait();
                let state = voice.state();
                let loaded = voice.is_loaded("friend", "file");
                let playing = voice.playing_now();
                let _ = read.0.send((state, loaded, playing));
            });
            let player = lock(&voice.player);
            held.wait();
            let came = read.1.recv_timeout(Duration::from_secs(10));
            drop(player);
            let (state, loaded, playing) = came.expect("a read waited for the player");
            assert!(state.is_some_and(|s| s.playing && s.file == "file"));
            assert!(loaded && playing);
        });
        // Decoded in one go, it played to the end meanwhile, and shows so.
        let mut feed = Feed::new(44_100);
        let mut data = vec![0.0f32; 44_100 * 2 * 11];
        render(&voice, &mut feed, &mut data, 2);
        let state = voice.state().unwrap();
        assert!(
            !state.playing && state.position_ms == state.duration_ms,
            "{state:?}"
        );
        assert!(state.duration_ms >= 19_900, "{state:?}");
    }
}
