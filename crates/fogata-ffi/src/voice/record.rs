//! Recording a voice message from the microphone chosen in the application.
//!
//! The application names microphones as the voice engine lists them. On Linux that is the
//! sound server's name of the input (PipeWire's PulseAudio service, or PulseAudio), which
//! ALSA's plugins for it open by name (`pulse:DEVICE=…`, `pipewire:NODE=…`). On Windows and
//! macOS it is the id the system gives the device (its endpoint id, its Core Audio uid),
//! as cpal names devices, or failing that its name. An input not found is the system's
//! default.
//!
//! What comes in is made mono at 48 kHz, cleaned of noise as calls are when that is on,
//! and encoded as it comes ([`fogata_voice::Recorder`]); the levels make the bars that
//! move while recording. Recording stops by itself at 15 minutes.

use std::{
    sync::{
        Arc, Mutex, PoisonError,
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
use fogata_denoise::Denoiser;
use fogata_voice::{Linear, RATE, Recorder, Recording};

/// Levels (of 20 ms each) that make one bar while recording: 200 ms.
const LEVELS_A_BAR: usize = 10;

/// Bars handed to the screen while recording: more than the bar shows.
const LIVE_BARS: usize = 160;

/// How long the microphone may stay without sound before it counts as not working: none
/// at all comes, none comes any more, or (on macOS, where a microphone the system has not
/// allowed records silence) only digital silence.
const NOTHING_FOR: Duration = Duration::from_secs(2);

/// Longest a capture thread waits for the microphone before it looks whether to stop.
#[cfg(target_os = "linux")]
const WAIT_MS: u32 = 100;

/// Longest sending waits for the capture thread to end once told to stop, so all it took is
/// in; one that takes longer (a device that hangs) is left to end by itself, its microphone
/// closing then. The window's thread never waits for it.
const STOP_WAIT: Duration = Duration::from_millis(300);

/// Samples in 10 ms at 48 kHz: what noise suppression takes at a time.
const TEN_MS: usize = 480;

/// What is recorded, shared with the thread that records.
struct Take {
    recorder: Recorder,
    /// Noise suppression, when on.
    denoiser: Option<Denoiser>,
    /// From the microphone's rate to 48 kHz.
    resampler: Option<Linear>,
    /// Sound at 48 kHz waiting for a whole 10 ms.
    pending: Vec<f32>,
    /// When the microphone opened.
    opened: Instant,
    /// Samples that came, when the last came, and whether any was not silence.
    came: u64,
    last_came: Instant,
    sound: bool,
    /// Why the microphone does not work, once that is clear.
    failed: Option<String>,
}

impl Take {
    /// Takes sound from the microphone at `rate`: whether to go on recording.
    fn push(&mut self, mono: &[f32], rate: u32) -> bool {
        if self.failed.is_some() || self.recorder.full() {
            return false;
        }
        self.came += mono.len() as u64;
        if !mono.is_empty() {
            self.last_came = Instant::now();
        }
        self.sound |= mono.iter().any(|&s| s != 0.0);
        let resampler = self
            .resampler
            .get_or_insert_with(|| Linear::new(rate, RATE));
        resampler.push(mono, &mut self.pending);
        let whole = self.pending.len() / TEN_MS * TEN_MS;
        if let Some(denoiser) = self.denoiser.as_mut() {
            for frame in self.pending[..whole].chunks_mut(TEN_MS) {
                // The suppressor takes samples as 16-bit values in floats.
                frame.iter_mut().for_each(|s| *s *= 32768.0);
                denoiser.process(true, RATE as usize, frame);
                frame.iter_mut().for_each(|s| *s /= 32768.0);
            }
        }
        if let Err(e) = self.recorder.push(&self.pending[..whole]) {
            self.failed = Some(e.to_string());
        }
        self.pending.drain(..whole);
        self.failed.is_none() && !self.recorder.full()
    }

    /// Notes a microphone that gives nothing, stopped giving anything, or gives only
    /// silence on macOS. Asked while it records.
    fn check(&mut self) {
        if self.failed.is_some() || self.recorder.full() {
            return;
        }
        if self.opened.elapsed() > NOTHING_FOR
            && (self.came == 0 || (cfg!(target_os = "macos") && !self.sound))
        {
            self.failed = Some(if self.came == 0 {
                "no sound comes from the microphone".into()
            } else {
                "the microphone records only silence".into()
            });
        } else if self.came > 0 && self.last_came.elapsed() > NOTHING_FOR {
            self.failed = Some("no more sound comes from the microphone".into());
        }
    }
}

/// A recording under way, or stopped and waiting to be sent.
struct Active {
    /// Which recording it is: one of [`RECORDINGS`].
    id: u64,
    take: Arc<Mutex<Take>>,
    /// Set to stop the thread that records.
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    /// Set by the thread that records when it ended.
    ended: Arc<AtomicBool>,
    /// The finished message and how it showed, once made: kept until it is sent or thrown
    /// away, so a send that fails can be tried again.
    done: Option<(Option<Recording>, Progress)>,
}

impl Active {
    /// Stops the microphone, keeping what was recorded, and waits for the recording's
    /// thread to end, so all it took is in: at most [`STOP_WAIT`].
    fn stop_capture(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        wait_for_end(&self.ended, self.thread.take());
    }

    /// Tells the recording's thread to stop, without waiting: the window's thread asks
    /// this. The thread closes the microphone itself as soon as it sees it, within a tenth
    /// of a second (longer only for a device that hangs).
    fn let_go(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread = None;
    }
}

/// Waits for a recording's thread, told to stop, to end, so all it took is in: at most
/// [`STOP_WAIT`]. One that takes longer is left to end by itself.
fn wait_for_end(ended: &AtomicBool, thread: Option<thread::JoinHandle<()>>) {
    let until = Instant::now() + STOP_WAIT;
    while !ended.load(Ordering::Relaxed) && Instant::now() < until {
        thread::sleep(Duration::from_millis(5));
    }
    if ended.load(Ordering::Relaxed) {
        if let Some(t) = thread {
            let _ = t.join();
        }
    } else {
        tracing::warn!("voice: the microphone does not stop; it closes when it answers");
    }
}

/// The recording. Whatever holds it does no slow work meanwhile: the window's thread asks
/// for it every 80 ms ([`state`]), and to stop or throw the recording away.
static ACTIVE: Mutex<Option<Active>> = Mutex::new(None);

/// Held while a recording is finished, so two sends of one recording make it once.
static FINISHING: Mutex<()> = Mutex::new(());

/// Recordings thrown away so far, counted under [`ACTIVE`]'s lock: a start whose
/// microphone opens after one was (the recording deleted while it opened) closes it again
/// instead of recording unseen.
static CANCELS: AtomicU64 = AtomicU64::new(0);

/// Recordings started so far: each has its number, from 1.
static RECORDINGS: AtomicU64 = AtomicU64::new(0);

/// What a recording's thread tells once the microphone opened (whether the input named
/// could not be used and the default records instead), or why it did not.
type Opened = mpsc::Sender<Result<bool, String>>;

fn lock_take(take: &Mutex<Take>) -> std::sync::MutexGuard<'_, Take> {
    take.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the screen shows of the recording.
#[derive(Clone, Debug)]
pub(crate) struct Progress {
    pub elapsed_ms: u64,
    /// The last bars, oldest first, a bar for each 200 ms.
    pub bars: Vec<u8>,
    /// The microphone is open now.
    pub capturing: bool,
    /// 15 minutes recorded: it stopped by itself.
    pub full: bool,
    /// The microphone could not be used.
    pub failed: bool,
}

/// Starts recording from `input` (as the voice engine names it; `None` for the system's
/// default), cleaning noise when `denoise`. A recording under way is thrown away. Fails
/// when no microphone opens; otherwise says whether the input named could not be used
/// and the system's default records instead.
pub(crate) fn start(input: Option<String>, denoise: bool) -> Result<bool, String> {
    start_with(denoise, move |take, stop, opened| {
        capture(input, take, stop, opened);
    })
}

/// [`start`], with `capture` opening the microphone and recording on the recording's own
/// thread: it says whether the microphone opened, then records into the take until stopped.
fn start_with(
    denoise: bool,
    capture: impl FnOnce(Arc<Mutex<Take>>, Arc<AtomicBool>, Opened) + Send + 'static,
) -> Result<bool, String> {
    cancel();
    let cancels = CANCELS.load(Ordering::SeqCst);
    let take = Arc::new(Mutex::new(Take {
        recorder: Recorder::new().map_err(|e| e.to_string())?,
        denoiser: denoise.then(Denoiser::new),
        resampler: None,
        pending: Vec::new(),
        opened: Instant::now(),
        came: 0,
        last_came: Instant::now(),
        sound: false,
        failed: None,
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let ended = Arc::new(AtomicBool::new(false));
    let (opened_tx, opened_rx) = mpsc::channel();
    let thread = {
        let (take, stop, ended) = (take.clone(), stop.clone(), ended.clone());
        thread::Builder::new()
            .name("fogata-voice-record".into())
            .spawn(move || {
                capture(take, stop, opened_tx);
                ended.store(true, Ordering::Relaxed);
            })
            .map_err(|e| e.to_string())?
    };
    let fell_back = match opened_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(fell_back)) => fell_back,
        Ok(Err(e)) => {
            stop.store(true, Ordering::Relaxed);
            let _ = thread.join();
            return Err(e);
        }
        Err(_) => {
            stop.store(true, Ordering::Relaxed);
            return Err("the microphone did not open".into());
        }
    };
    lock_take(&take).opened = Instant::now();
    let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
    let recording = Active {
        id: RECORDINGS.fetch_add(1, Ordering::SeqCst) + 1,
        take,
        stop,
        thread: Some(thread),
        ended,
        done: None,
    };
    if CANCELS.load(Ordering::SeqCst) != cancels {
        drop(active);
        let mut recording = recording;
        recording.stop_capture();
        return Err("the recording was deleted while the microphone opened".into());
    }
    *active = Some(recording);
    Ok(fell_back)
}

/// The recording as it stands, if there is one.
pub(crate) fn state() -> Option<Progress> {
    let active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
    let active = active.as_ref()?;
    if let Some((_, shown)) = &active.done {
        return Some(shown.clone());
    }
    let mut take = lock_take(&active.take);
    if active.thread.is_some() && !active.stop.load(Ordering::Relaxed) {
        take.check();
    }
    let levels = take.recorder.levels();
    // The mean of each 200 ms's levels, so syllables and pauses show apart.
    let bars: Vec<u8> = levels
        .chunks(LEVELS_A_BAR)
        .map(|c| (c.iter().map(|&l| u32::from(l)).sum::<u32>() / c.len() as u32) as u8)
        .collect();
    let from = bars.len().saturating_sub(LIVE_BARS);
    Some(Progress {
        elapsed_ms: take.recorder.duration_ms(),
        bars: bars[from..].to_vec(),
        capturing: active.thread.is_some()
            && !active.stop.load(Ordering::Relaxed)
            && take.failed.is_none()
            && !take.recorder.full(),
        full: take.recorder.full(),
        failed: take.failed.is_some(),
    })
}

/// Stops the microphone and keeps what was recorded, to send or throw away. Does not wait
/// for the microphone to close.
pub(crate) fn stop() {
    if let Some(active) = ACTIVE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        active.let_go();
    }
}

/// Throws the recording away. Does not wait for the microphone to close.
pub(crate) fn cancel() {
    let active = {
        let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
        CANCELS.fetch_add(1, Ordering::SeqCst);
        active.take()
    };
    if let Some(mut active) = active {
        active.let_go();
    }
}

/// Ends the recording: the finished message, or `None` when there was none, or it was
/// shorter than half a second; and which recording it was. It stays (see [`forget`]) until
/// it is sent.
pub(crate) fn finish(serial: u32) -> Result<(Option<Recording>, u64), String> {
    let _finishing = FINISHING.lock().unwrap_or_else(PoisonError::into_inner);
    let shown = state();
    // The microphone closes without the recording held: that can take up to STOP_WAIT.
    let (id, ended, thread) = {
        let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(active) = active.as_mut() else {
            return Ok((None, 0));
        };
        if let Some((done, _)) = &active.done {
            return Ok((done.clone(), active.id));
        }
        active.stop.store(true, Ordering::Relaxed);
        (active.id, active.ended.clone(), active.thread.take())
    };
    wait_for_end(&ended, thread);
    let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
    // Thrown away meanwhile, maybe with another one started: nothing to send.
    let Some(active) = active.as_mut().filter(|a| a.id == id) else {
        return Ok((None, 0));
    };
    let recorder = {
        let fresh = Recorder::new().map_err(|e| e.to_string())?;
        std::mem::replace(&mut lock_take(&active.take).recorder, fresh)
    };
    let done = if recorder.duration_ms() < 500 {
        None
    } else {
        recorder.finish(serial).map_err(|e| e.to_string())?
    };
    let shown = shown.map(|s| Progress {
        capturing: false,
        ..s
    });
    if let Some(shown) = shown {
        active.done = Some((done.clone(), shown));
    }
    Ok((done, active.id))
}

/// Forgets recording `id` (as [`finish`] numbered it) once it was sent; one started while
/// it went stays.
pub(crate) fn forget(id: u64) {
    let active = {
        let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
        if active.as_ref().is_some_and(|a| a.id == id) {
            active.take()
        } else {
            None
        }
    };
    if let Some(mut active) = active {
        active.let_go();
    }
}

/// The recording thread: opens the input, says whether it opened, and records until
/// stopped or the message is full.
fn capture(input: Option<String>, take: Arc<Mutex<Take>>, stop: Arc<AtomicBool>, opened: Opened) {
    let chosen = input.is_some();
    #[cfg(target_os = "linux")]
    match alsa_in::open(input.as_deref()) {
        Ok((pcm, rate, by_name)) => {
            let _ = opened.send(Ok(chosen && !by_name));
            alsa_in::record(&pcm, rate, &take, &stop);
            return;
        }
        Err(e) => tracing::debug!("voice: no ALSA input ({e}); trying cpal's default"),
    }
    let started = cpal_in::open(
        if cfg!(target_os = "linux") {
            None
        } else {
            input
        },
        take.clone(),
    );
    let (stream, by_name) = match started {
        Ok(started) => started,
        Err(e) => {
            let _ = opened.send(Err(e));
            return;
        }
    };
    // On Linux cpal's is the default input, never the one chosen.
    let _ = opened.send(Ok(chosen && (cfg!(target_os = "linux") || !by_name)));
    // The stream records on its own thread while it is kept here.
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(20));
        let take = lock_take(&take);
        if take.failed.is_some() || take.recorder.full() {
            break;
        }
    }
    drop(stream);
}

/// Inputs through cpal: Windows, macOS, and Linux's default when ALSA's plugins are not
/// there.
mod cpal_in {
    use super::*;

    /// The input the voice engine calls `id`: by the system's id for it, as cpal names
    /// it; failing that, by its name.
    fn named_input(host: &cpal::Host, id: &str) -> Option<cpal::Device> {
        let devices: Vec<cpal::Device> = host.input_devices().ok()?.collect();
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

    /// The stream, and whether it is the input `input` names.
    pub(super) fn open(
        input: Option<String>,
        take: Arc<Mutex<Take>>,
    ) -> Result<(cpal::Stream, bool), String> {
        let host = cpal::default_host();
        let named = input.and_then(|id| named_input(&host, &id));
        let by_name = named.is_some();
        let device = named
            .or_else(|| host.default_input_device())
            .ok_or("no microphone")?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let stream = crate::voice::by_sample_type!(format, stream(&device, &config, take))
            .ok_or_else(|| format!("unsupported sample format {format}"))?
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((stream, by_name))
    }

    fn stream<T: SizedSample>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        take: Arc<Mutex<Take>>,
    ) -> Result<cpal::Stream, cpal::Error>
    where
        f32: FromSample<T>,
    {
        let channels = usize::from(config.channels.max(1));
        let rate = config.sample_rate;
        let failed = take.clone();
        let mut mono = Vec::new();
        device.build_input_stream(
            *config,
            move |data: &[T], _| {
                mono.clear();
                mono.extend(data.chunks(channels).map(|frame| {
                    frame.iter().map(|&s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                }));
                lock_take(&take).push(&mono, rate);
            },
            move |e| match e.kind() {
                cpal::ErrorKind::Xrun
                | cpal::ErrorKind::DeviceChanged
                | cpal::ErrorKind::RealtimeDenied => tracing::debug!("voice: {e}"),
                _ => {
                    tracing::warn!("voice: the microphone stopped: {e}");
                    lock_take(&failed).failed = Some(e.to_string());
                }
            },
            None,
        )
    }
}

/// Inputs on Linux through ALSA's plugins for the sound server, which open an input by the
/// name the sound server (and so the voice engine) gives it.
#[cfg(target_os = "linux")]
mod alsa_in {
    use alsa::{
        Direction, ValueOr,
        pcm::{Access, Format, HwParams, PCM},
    };

    use super::*;

    /// Whether `id` can go into an ALSA device name as it is.
    fn plain(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 256
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-:@+".contains(c))
    }

    /// The ALSA devices to try, in order, for the input `id`.
    pub(super) fn names(id: Option<&str>) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(id) = id.filter(|id| plain(id)) {
            names.push(format!("pulse:DEVICE=\"{id}\""));
            names.push(format!("pipewire:NODE=\"{id}\""));
        }
        names.push("default".into());
        names
    }

    /// The input open, its rate, and whether it is the one `id` names (not ALSA's default).
    pub(super) fn open(id: Option<&str>) -> Result<(PCM, u32, bool), String> {
        let mut last = String::from("no input");
        for name in names(id) {
            match open_one(&name) {
                Ok((pcm, rate)) => {
                    tracing::debug!("voice: recording from ALSA's {name}");
                    return Ok((pcm, rate, name != "default"));
                }
                Err(e) => last = format!("{name}: {e}"),
            }
        }
        Err(last)
    }

    fn open_one(name: &str) -> alsa::Result<(PCM, u32)> {
        // Non-blocking: reading waits at most [`WAIT_MS`] for sound, so a stop is seen
        // that soon even from a device that sends nothing.
        let pcm = PCM::new(name, Direction::Capture, true)?;
        let rate = {
            let hw = HwParams::any(&pcm)?;
            hw.set_access(Access::RWInterleaved)?;
            hw.set_format(Format::float())?;
            hw.set_channels(1)?;
            let rate = hw.set_rate_near(RATE, ValueOr::Nearest)?;
            hw.set_period_size_near(480, ValueOr::Nearest)?;
            hw.set_buffer_size_near(4800)?;
            pcm.hw_params(&hw)?;
            rate
        };
        pcm.prepare()?;
        pcm.start()?;
        Ok((pcm, rate))
    }

    pub(super) fn record(pcm: &PCM, rate: u32, take: &Mutex<Take>, stop: &AtomicBool) {
        let io = match pcm.io_f32() {
            Ok(io) => io,
            Err(e) => {
                lock_take(take).failed = Some(e.to_string());
                return;
            }
        };
        let mut buf = vec![0.0f32; 480];
        while !stop.load(Ordering::Relaxed) {
            match pcm.wait(Some(WAIT_MS)) {
                // Nothing yet: look at the stop again, and whether the microphone was
                // found to give nothing.
                Ok(false) => {
                    if lock_take(take).failed.is_some() {
                        break;
                    }
                    continue;
                }
                Ok(true) => {}
                Err(e) => {
                    if pcm.try_recover(e, true).is_err() {
                        tracing::warn!("voice: the microphone stopped: {e}");
                        lock_take(take).failed = Some(e.to_string());
                        break;
                    }
                    continue;
                }
            }
            match io.readi(&mut buf) {
                Ok(n) => {
                    if !lock_take(take).push(&buf[..n], rate) {
                        break;
                    }
                }
                // Ready, yet nothing to read: a moment before waiting again, so the loop
                // never spins.
                Err(e) if e.errno() == libc::EAGAIN => thread::sleep(Duration::from_millis(2)),
                Err(e) => {
                    if pcm.try_recover(e, true).is_err() {
                        tracing::warn!("voice: the microphone stopped: {e}");
                        lock_take(take).failed = Some(e.to_string());
                        break;
                    }
                }
            }
        }
        let _ = pcm.drop();
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn inputs_are_tried_by_name_then_the_default() {
            assert_eq!(
                names(Some("alsa_input.usb-Mic_00.mono-fallback")),
                [
                    "pulse:DEVICE=\"alsa_input.usb-Mic_00.mono-fallback\"",
                    "pipewire:NODE=\"alsa_input.usb-Mic_00.mono-fallback\"",
                    "default"
                ]
            );
            // A name that could change what ALSA reads is never put in one.
            for odd in ["a\"b", "a b", "x}y", "", "a\nb"] {
                assert_eq!(names(Some(odd)), ["default"]);
            }
            assert_eq!(names(None), ["default"]);
        }
    }
}

/// A recording that records nothing until stopped, as if a microphone were open: for
/// tests of what ends a recording. Returns what says it stopped.
#[cfg(test)]
pub(crate) fn fake_recording() -> Arc<AtomicBool> {
    fake_recording_closing_in(Duration::ZERO).0
}

/// [`fake_recording`] whose microphone takes `closing` to close once stopped. Returns what
/// says it stopped, and what says it closed.
#[cfg(test)]
fn fake_recording_closing_in(closing: Duration) -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    fake_recording_closing_when(move || thread::sleep(closing))
}

/// [`fake_recording`] whose microphone, once stopped, closes when `closing` returns.
/// Returns what says it stopped, and what says it closed.
#[cfg(test)]
fn fake_recording_closing_when(
    closing: impl FnOnce() + Send + 'static,
) -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    let stop = Arc::new(AtomicBool::new(false));
    let ended = Arc::new(AtomicBool::new(false));
    let thread = {
        let (stop, ended) = (stop.clone(), ended.clone());
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(5));
            }
            closing();
            ended.store(true, Ordering::Relaxed);
        })
    };
    *ACTIVE.lock().unwrap_or_else(PoisonError::into_inner) = Some(Active {
        id: RECORDINGS.fetch_add(1, Ordering::SeqCst) + 1,
        take: tests::take(),
        stop: stop.clone(),
        thread: Some(thread),
        ended: ended.clone(),
        done: None,
    });
    (stop, ended)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(super) fn take() -> Arc<Mutex<Take>> {
        Arc::new(Mutex::new(Take {
            recorder: Recorder::new().unwrap(),
            denoiser: None,
            resampler: None,
            pending: Vec::new(),
            opened: Instant::now(),
            came: 0,
            last_came: Instant::now(),
            sound: false,
            failed: None,
        }))
    }

    /// A microphone that gave sound and then nothing more, with no error, counts as not
    /// working once that lasts; a pause of the person speaking does not, being sound too.
    #[test]
    fn a_microphone_that_stops_giving_sound_is_noticed() {
        let take = take();
        let mut t = lock_take(&take);
        t.opened = Instant::now() - 2 * NOTHING_FOR;
        assert!(t.push(&[0.0; 480], RATE));
        assert!(t.push(&[0.2; 480], RATE));
        t.check();
        assert_eq!(t.failed, None);
        t.last_came = Instant::now() - NOTHING_FOR - Duration::from_millis(100);
        t.check();
        assert!(t.failed.is_some());
        // Nothing more is taken once it failed.
        assert!(!t.push(&[0.2; 480], RATE));
    }

    /// Tests that use the one recording there is, one at a time.
    pub(crate) static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// A microphone that takes `opens_in` to open, then records nothing until stopped;
    /// `closed` is set once it closed.
    fn slow_microphone(
        opens_in: Duration,
        closed: Arc<AtomicBool>,
    ) -> impl FnOnce(Arc<Mutex<Take>>, Arc<AtomicBool>, Opened) + Send + 'static {
        move |_take, stop, opened| {
            thread::sleep(opens_in);
            let _ = opened.send(Ok(false));
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(5));
            }
            closed.store(true, Ordering::Relaxed);
        }
    }

    /// Deleted while the microphone opens: once it opens it closes again, and no recording
    /// is left under way unseen.
    #[test]
    fn a_recording_deleted_while_the_microphone_opens_does_not_go_on() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let closed = Arc::new(AtomicBool::new(false));
        let starting = {
            let closed = closed.clone();
            thread::spawn(move || {
                start_with(false, slow_microphone(Duration::from_millis(300), closed))
            })
        };
        thread::sleep(Duration::from_millis(100));
        cancel();
        assert!(starting.join().unwrap().is_err());
        assert!(state().is_none(), "{:?}", state());
        let until = Instant::now() + Duration::from_secs(2);
        while !closed.load(Ordering::Relaxed) && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            closed.load(Ordering::Relaxed),
            "the microphone is still open"
        );
        // One not deleted records.
        let closed = Arc::new(AtomicBool::new(false));
        assert_eq!(
            start_with(
                false,
                slow_microphone(Duration::from_millis(20), closed.clone())
            ),
            Ok(false)
        );
        assert!(state().is_some_and(|s| s.capturing));
        cancel();
        // It closes by itself, soon.
        let until = Instant::now() + Duration::from_secs(2);
        while !closed.load(Ordering::Relaxed) && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(closed.load(Ordering::Relaxed));
    }

    /// The recording sent is forgotten once it went, not one started meanwhile.
    #[test]
    fn forgetting_a_sent_recording_keeps_one_started_since() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        fake_recording();
        let (_, sent) = finish(1).unwrap();
        // Thrown away while it went, and another one started.
        cancel();
        let second = fake_recording();
        forget(sent);
        assert!(state().is_some_and(|s| s.capturing));
        assert!(!second.load(Ordering::Relaxed));
        // Sent in turn, it is forgotten.
        let (_, sent) = finish(2).unwrap();
        forget(sent);
        assert!(state().is_none());
        assert!(second.load(Ordering::Relaxed));
    }

    /// Stopping and throwing away, which the window's thread asks for, do not wait for a
    /// microphone that is slow to close; sending, on a thread of its own, waits for it so
    /// everything recorded is in.
    #[test]
    fn the_window_does_not_wait_for_the_microphone_to_close() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let closing = Duration::from_millis(250);
        for throw_away in [false, true] {
            let (stopped, closed) = fake_recording_closing_in(closing);
            let asked = Instant::now();
            if throw_away {
                cancel();
            } else {
                stop();
                assert!(state().is_some_and(|s| !s.capturing));
            }
            let took = asked.elapsed();
            assert!(took < Duration::from_millis(50), "{took:?}");
            assert!(stopped.load(Ordering::Relaxed));
            if !throw_away {
                let _ = finish(1).unwrap();
                assert!(closed.load(Ordering::Relaxed), "sent before all was in");
                cancel();
            }
            let until = Instant::now() + Duration::from_secs(2);
            while !closed.load(Ordering::Relaxed) && Instant::now() < until {
                thread::sleep(Duration::from_millis(5));
            }
            assert!(closed.load(Ordering::Relaxed));
        }
    }

    /// While sending waits for the microphone to close, what the window's thread asks for
    /// every 80 ms (where the recording stands) and stopping it come at once: the
    /// microphone here closes only once they came, and sending still waits for it.
    #[test]
    fn the_window_does_not_wait_while_a_recording_is_sent() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let (let_close, may_close) = mpsc::channel::<()>();
        let (stopped, closed) = fake_recording_closing_when(move || {
            let _ = may_close.recv_timeout(Duration::from_secs(5));
        });
        let sending = {
            let closed = closed.clone();
            thread::spawn(move || {
                let sent = finish(1);
                (sent, closed.load(Ordering::Relaxed))
            })
        };
        let until = Instant::now() + Duration::from_secs(2);
        while !stopped.load(Ordering::Relaxed) && Instant::now() < until {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(stopped.load(Ordering::Relaxed), "sending did not stop it");
        // Were the recording held meanwhile, these would wait until sending gave up on
        // the microphone, which would then close only after it went.
        assert!(state().is_some_and(|s| !s.capturing));
        stop();
        let _ = let_close.send(());
        let (sent, closed_before) = sending.join().unwrap();
        let (_, id) = sent.unwrap();
        assert!(closed_before, "sent before all was in");
        assert!(id > 0);
        forget(id);
        assert!(state().is_none());
    }

    /// Thrown away while it is sent, after the microphone closed: nothing goes.
    #[test]
    fn a_recording_thrown_away_while_it_is_sent_does_not_go() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        let (let_close, may_close) = mpsc::channel::<()>();
        let (stopped, _) = fake_recording_closing_when(move || {
            let _ = may_close.recv_timeout(Duration::from_secs(5));
        });
        let sending = thread::spawn(|| finish(1));
        let until = Instant::now() + Duration::from_secs(2);
        while !stopped.load(Ordering::Relaxed) && Instant::now() < until {
            thread::sleep(Duration::from_millis(1));
        }
        cancel();
        let second = fake_recording();
        let _ = let_close.send(());
        assert_eq!(sending.join().unwrap().unwrap().1, 0);
        // The one started meanwhile records on.
        assert!(state().is_some_and(|s| s.capturing));
        assert!(!second.load(Ordering::Relaxed));
        cancel();
    }

    /// A thread that ignores the stop for a while (a device that hangs): stopping waits
    /// for it no longer than [`STOP_WAIT`]; one that stops is waited for.
    #[test]
    fn stopping_never_waits_long_for_the_microphone() {
        for hangs in [true, false] {
            let stop = Arc::new(AtomicBool::new(false));
            let ended = Arc::new(AtomicBool::new(false));
            let thread = {
                let (stop, ended) = (stop.clone(), ended.clone());
                thread::spawn(move || {
                    let until = Instant::now() + Duration::from_secs(3);
                    while Instant::now() < until && (hangs || !stop.load(Ordering::Relaxed)) {
                        thread::sleep(Duration::from_millis(10));
                    }
                    ended.store(true, Ordering::Relaxed);
                })
            };
            let mut active = Active {
                id: 0,
                take: take(),
                stop,
                thread: Some(thread),
                ended: ended.clone(),
                done: None,
            };
            let started = Instant::now();
            active.stop_capture();
            let took = started.elapsed();
            assert!(took < STOP_WAIT + Duration::from_millis(100), "{took:?}");
            assert_eq!(ended.load(Ordering::Relaxed), !hangs);
        }
    }
}
