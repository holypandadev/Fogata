//! The calls the screens make. The core runs on its own runtime, started once by [`start`];
//! the other calls need it running.

use std::{
    collections::VecDeque,
    future::Future,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use anyhow::{Context, Result, anyhow};
use flutter_rust_bridge::frb;
use fogata_core::{
    Core, NetOptions, Store,
    diagnose::{self, diagnose},
};
use fogata_crypto::{DevicePublicKey, GroupId};
use fogata_friends::Avatar;
use qrcode::{Color, EcLevel, QrCode};
use tokio::runtime::Runtime;

use super::types::{
    UiActivityPage, UiAnimated, UiAnimation, UiAtRest, UiBanner, UiCall, UiEvent, UiFinding,
    UiFriend, UiGroup, UiGroupInvite, UiInvitation, UiJoinAsk, UiMediaBridge, UiMember, UiProfile,
    UiQr, UiReach, UiRequests, UiRoom, UiSound, UiVideo, activity, call, event, finding, friend,
    group, group_invite, invitation, join_ask, member, profile, reach, request, room as room_view,
};
use crate::{frb_generated::StreamSink, sounds::Sound};

pub(crate) static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("fogata")
        .build()
        .expect("cannot start the runtime")
});
/// The core, while it runs; it stops for a move of the account and starts again after.
static CORE: std::sync::RwLock<Option<Core>> = std::sync::RwLock::new(None);
static STARTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Cores started in this process; the events of an earlier one are not passed on.
static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SINK: Mutex<Sink> = Mutex::new(Sink {
    stream: None,
    early: VecDeque::new(),
    given: 0,
});
/// Most events kept for the screens before they listen.
const EARLY_EVENTS: usize = 256;
/// Events given to the screens and not taken yet ([`events_taken`]), past which no more is
/// given: they wait in the core's queue meanwhile, which bounds them and tells the node to
/// hold back. As many as are kept before the screens listen, so none of those is lost.
const GIVEN_AT_MOST: usize = EARLY_EVENTS;
/// Told when the screens took events, or a new stream or core starts.
static TAKEN: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Where the core's events go: the screens' stream, or, until they listen, a queue, so a
/// notice the core gives as it starts (a damaged list restored, the identity open on
/// another computer) is not lost.
struct Sink {
    stream: Option<StreamSink<UiEvent>>,
    early: VecDeque<UiEvent>,
    /// Events given (or kept in `early`) and not taken yet.
    given: usize,
}

/// Waits until the screens took enough of the events given to them for one more.
async fn room_for_more() {
    loop {
        let taken = TAKEN.notified();
        tokio::pin!(taken);
        taken.as_mut().enable();
        if SINK.lock().unwrap_or_else(|e| e.into_inner()).given < GIVEN_AT_MOST {
            return;
        }
        taken.await;
    }
}

/// The screens took `count` more of the events given to them (see [`events`]). Those of a
/// stream replaced meanwhile may be told too: room for a few more at most, once.
#[frb(sync)]
pub fn events_taken(count: u32) {
    let mut sink = SINK.lock().unwrap_or_else(|e| e.into_inner());
    // Those kept until the screens listen are taken by nobody yet: a late word of a stream
    // that went makes no room past them, or the next would push one out unseen.
    let kept = if sink.stream.is_none() {
        sink.early.len()
    } else {
        0
    };
    sink.given = sink.given.saturating_sub(count as usize).max(kept);
    drop(sink);
    TAKEN.notify_waiters();
}

/// Runs `work` with the core on the core's runtime.
pub(crate) async fn run<T, F>(work: impl FnOnce(Core) -> F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let core = CORE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .context("Fogata is not running yet")?;
    RUNTIME.spawn(work(core)).await?
}

fn group_id(id: &str) -> Result<GroupId> {
    GroupId::from_bytes(&hex::decode(id)?).map_err(|_| anyhow!("not a group id"))
}

pub(crate) fn device(id: &str) -> Result<DevicePublicKey> {
    DevicePublicKey::from_bytes(&hex::decode(id)?).map_err(|_| anyhow!("not a device id"))
}

fn voice_id(id: &str) -> Result<[u8; 8]> {
    hex::decode(id)?
        .try_into()
        .map_err(|_| anyhow!("not a voice room id"))
}

pub(crate) fn file_id(id: &str) -> Result<[u8; 32]> {
    hex::decode(id)?
        .try_into()
        .map_err(|_| anyhow!("not a file id"))
}

fn to_avatar(preset: u8, photo: Option<Vec<u8>>) -> Avatar {
    match photo {
        Some(bytes) => Avatar::Photo(bytes),
        None => Avatar::Preset(preset),
    }
}

#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
    raise_open_files_limit();
}

/// Lets the app keep more files and sockets open than a desktop app gets by default (256 on
/// macOS), as room for calls, files and bursts of TCP connections to the DHT.
#[cfg(unix)]
#[allow(unsafe_code)]
fn raise_open_files_limit() {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit and setrlimit only read and write the struct they are given.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            return;
        }
        // macOS refuses more than OPEN_MAX (10240), even when the hard limit is unlimited.
        let wanted = limit.rlim_max.min(10240);
        if limit.rlim_cur < wanted {
            limit.rlim_cur = wanted;
            libc::setrlimit(libc::RLIMIT_NOFILE, &limit);
        }
    }
}

#[cfg(not(unix))]
fn raise_open_files_limit() {}

// ----- Before the core runs -----

/// Where this platform keeps the data folder; an error when the system names none.
#[frb(sync)]
pub fn default_data_dir() -> Result<String> {
    Ok(fogata_core::default_data_dir()?
        .to_string_lossy()
        .into_owned())
}

/// Off the UI thread: a damaged friends file is read through the keychain, which may wait
/// for the person to unlock it.
pub fn has_profile(data_dir: String) -> Result<bool> {
    use_keychain();
    Core::has_profile(Path::new(&data_dir))
}

/// While the data folder has no profile: the folder where the profile and friends made in it
/// were kept when they could not be read or were refused, if they were. The welcome screen
/// says so, rather than look like a new device's.
pub fn kept_profile(data_dir: String) -> Result<Option<String>> {
    Ok(Core::kept_profile(Path::new(&data_dir))?.map(|p| p.to_string_lossy().into_owned()))
}

/// The data folder's secrets are sealed with a key the system keychain keeps. Copies run
/// for tests set `FOGATA_SEAL=off`: their folders stay plain, so they put nothing in the
/// keychain, while a sealed folder still opens. Debug builds only: in a release build
/// nothing around it (a script, a launcher) can leave a new folder plain.
pub(crate) fn use_keychain() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let keychain = std::sync::Arc::new(fogata_core::vault::SystemKeychain);
        fogata_core::vault::use_keychain(
            if seal_off(cfg!(debug_assertions), std::env::var("FOGATA_SEAL").ok()) {
                fogata_core::vault::Keychain::Open(keychain)
            } else {
                fogata_core::vault::Keychain::Seal(keychain)
            },
        );
    });
}

/// Whether a new data folder stays plain: `FOGATA_SEAL=off`, in a debug build only.
fn seal_off(debug_build: bool, seal: Option<String>) -> bool {
    debug_build && seal.is_some_and(|v| v == "off")
}

/// Whether this device carries other members' traffic when they cannot connect directly:
/// the `bridge` setting of `app.json`, on unless turned off. Read at start, so a change
/// applies the next time the application starts.
pub(crate) fn bridge_enabled(data_dir: &Path) -> bool {
    pref(data_dir, "bridge").unwrap_or(true)
}

/// Whether this device appears offline: the `offline` setting of `app.json`, off unless
/// turned on. Read at start so that friends never see it online; changed with
/// [`set_appear_offline`].
fn appears_offline(data_dir: &Path) -> bool {
    pref(data_dir, "offline").unwrap_or(false)
}

fn pref(data_dir: &Path, name: &str) -> Option<bool> {
    std::fs::read(data_dir.join("app.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|prefs| prefs.get(name)?.as_bool())
}

/// Saves the application's settings file (`app.json`) in the data folder, readable by the
/// owner only, like the files the core keeps there. Off the UI thread: the write waits for
/// the disk.
pub fn write_prefs(data_dir: String, json: String) -> Result<()> {
    fogata_core::store::write_private(&Path::new(&data_dir).join("app.json"), json.as_bytes())
}

/// Makes the data folder and its `window` folder, readable by this user only, before the
/// window writes anything there: another user who could open the file that says the
/// application runs could hold a lock on it and keep the application from starting.
#[frb(sync)]
pub fn prepare_data_dir(data_dir: String) -> Result<()> {
    let dir = Path::new(&data_dir);
    for folder in [dir.to_path_buf(), dir.join("window")] {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            builder.mode(0o700);
            builder
                .create(&folder)
                .with_context(|| format!("cannot create {}", folder.display()))?;
            // One made by an earlier version, or by hand, too.
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))
                .with_context(|| format!("cannot keep {} private", folder.display()))?;
        }
        #[cfg(not(unix))]
        builder
            .create(&folder)
            .with_context(|| format!("cannot create {}", folder.display()))?;
    }
    Ok(())
}

/// Writes a file of the window's (a picture of the profile, a problem report, a note for
/// the next start) as the core writes its own: readable by the owner only (on Unix),
/// through a temporary file, so a crash leaves the old contents or the new ones, and a
/// link at `path` is replaced rather than written through. The folder must be there.
pub fn write_private(path: String, bytes: Vec<u8>) -> Result<()> {
    fogata_core::store::write_private(Path::new(&path), &bytes)
}

/// Before anything secret of a new profile is written (or an account moves here): `None`
/// when the system keychain keeps the key that encrypts this computer's data, or the data
/// is kept unencrypted as it was or as the person chose; why not when it cannot keep one,
/// and then nothing was written. Off the UI thread: the keychain may wait for the person
/// to unlock it.
pub fn keychain_problem(data_dir: String) -> Result<Option<String>> {
    use_keychain();
    Ok(match Store::open(data_dir.into())?.sealing()? {
        fogata_core::store::Sealing::NoKeychain(why) => Some(why),
        _ => None,
    })
}

/// The person chose to keep this computer's data unencrypted while no keychain keeps a
/// key for it; it is encrypted at the first start one does.
pub fn keep_unencrypted(data_dir: String) -> Result<()> {
    fogata_core::vault::allow_plain(Path::new(&data_dir))
}

/// This device's fingerprint, making its key on first use.
pub fn device_fingerprint(data_dir: String) -> Result<String> {
    use_keychain();
    let (identity, _) = Store::open(data_dir.into())?.identity()?;
    Ok(identity.public_key().fingerprint_grouped())
}

/// Gives this device a new key while it has no profile yet; returns its fingerprint.
pub fn new_device_key(data_dir: String) -> Result<String> {
    use_keychain();
    let identity = Store::open(data_dir.into())?.new_identity()?;
    Ok(identity.public_key().fingerprint_grouped())
}

pub fn create_profile(
    data_dir: String,
    name: String,
    preset: u8,
    photo: Option<Vec<u8>>,
) -> Result<UiProfile> {
    use_keychain();
    let made = Core::create_profile(Path::new(&data_dir), &name, Some(to_avatar(preset, photo)))?;
    Ok(profile(&made))
}

/// Starts the core: the network node with every group and friend. Calling it again does
/// nothing more. Returns the profile.
pub async fn start(data_dir: String) -> Result<UiProfile> {
    use_keychain();
    {
        let _starting = STARTING.lock().await;
        if CORE.read().unwrap_or_else(|e| e.into_inner()).is_none() {
            use tracing_subscriber::{EnvFilter, Layer, fmt, prelude::*};
            crate::logs::know_computer(Path::new(&data_dir));
            let stderr = crate::logs::stderr_layer();
            let _ = tracing_subscriber::registry()
                .with(stderr)
                // The days' log in the data folder, for problem reports, with nothing
                // personal in it.
                .with(
                    fmt::layer()
                        .with_ansi(false)
                        .fmt_fields(crate::logs::ScrubFields)
                        .with_writer(crate::logs::DayFiles::new(Path::new(&data_dir)))
                        .with_filter(EnvFilter::new(crate::logs::LEVELS)),
                )
                .try_init();
            let options = NetOptions {
                bridge: bridge_enabled(Path::new(&data_dir)),
                hidden: appears_offline(Path::new(&data_dir)),
                ..NetOptions::default()
            };
            let dir = PathBuf::from(data_dir);
            let free = dir.clone();
            // A folder still held fails to start below, saying so.
            let _ = RUNTIME.spawn_blocking(move || wait_until_free(&free)).await;
            let (core, events) = {
                let _runtime = RUNTIME.enter();
                Core::start(dir, options)?
            };
            // Room downloads, and videos fetched to play, start only when they fit.
            core.use_free_space(crate::disk::free_space);
            let me = core.me();
            {
                let _runtime = RUNTIME.enter();
                let mut updates = fogata_core::updates::UpdateOptions::compiled_in();
                // Tests and labs: friends and members only, never the releases page.
                if std::env::var("FOGATA_UPDATE_WEB").is_ok_and(|v| v.trim() == "off") {
                    updates.web = None;
                }
                if let Err(e) = core.start_updates(updates) {
                    tracing::warn!("updates do not run: {e:#}");
                }
            }
            *CORE.write().unwrap_or_else(|e| e.into_inner()) = Some(core);
            let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            crate::logs::watch_names(&RUNTIME);
            // Ready before a call first rings.
            crate::sounds::prepare();
            crate::idle::watch(&RUNTIME);
            RUNTIME.spawn(pass_on(
                events,
                move |e| event(e, &me),
                move |e| deliver(e, run),
            ));
        }
    }
    my_profile().await
}

/// Passes the core's events on to the screens ([`deliver`]) until the core's stream ends,
/// and then says the core stopped, so the window does not stay online with nothing coming
/// in. An event that cannot be passed on (a panic while reading it) is left out, not the
/// ones after it. Each waits until the screens have room for it ([`room_for_more`]): until
/// then it waits in the core's queue, within its bounds.
async fn pass_on(
    mut events: fogata_core::CoreEvents,
    view: impl Fn(fogata_core::CoreEvent) -> UiEvent,
    deliver: impl Fn(UiEvent) -> bool,
) {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    loop {
        room_for_more().await;
        let Some(e) = events.recv().await else {
            break;
        };
        let passed = catch_unwind(AssertUnwindSafe(|| {
            crate::logs::after_event(&e);
            deliver(view(e))
        }));
        match passed {
            Ok(true) => {}
            Ok(false) => return,
            Err(_) => tracing::error!("an event of the core could not be passed on"),
        }
    }
    let stopped = catch_unwind(AssertUnwindSafe(|| {
        deliver(view(fogata_core::CoreEvent::Stopped))
    }));
    if stopped.is_err() {
        tracing::error!("the screens could not be told that the core stopped");
    }
}

/// Gives an event of the core of run `run` to the screens, or keeps it until they listen;
/// false once another core runs.
fn deliver(e: UiEvent, run: u64) -> bool {
    let mut sink = SINK.lock().unwrap_or_else(|e| e.into_inner());
    // What a core stopped for a move still says goes nowhere: the screens follow the core
    // started after it.
    if RUNS.load(std::sync::atomic::Ordering::SeqCst) != run {
        return false;
    }
    match &sink.stream {
        Some(stream) => {
            if stream.add(e).is_err() {
                sink.stream = None;
            } else {
                sink.given += 1;
            }
        }
        None => {
            if sink.early.len() == EARLY_EVENTS {
                sink.early.pop_front();
            } else {
                sink.given += 1;
            }
            sink.early.push_back(e);
        }
    }
    true
}

/// Stops the core, so that a move of the account to another computer (or from one) has
/// the data folder to itself: friends and members see this device go, and nothing of the
/// account changes until it starts again ([`start`]). Fails if the data folder is still
/// held once it waited a moment for it.
pub async fn stop_core(data_dir: String) -> Result<()> {
    // A start under way ends first: else it would put a core in place once this one
    // returned, saying the core had stopped.
    let _starting = STARTING.lock().await;
    // The microphone closes with the core: a voice message recorded now could not be sent.
    // A message playing stops: it is the account's, which the core no longer holds.
    RUNTIME.spawn_blocking(crate::voice::record::cancel).await?;
    crate::voice::play::stop();
    let core = CORE.write().unwrap_or_else(|e| e.into_inner()).take();
    // Its calls go with it, and so does their ring.
    crate::sounds::ring(None);
    // The screens' stream of events ends here: a stream that waits for an event that never
    // comes cannot be let go of on their side (cancelling it waits for the next one).
    // Starting again gives them a new one.
    RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    {
        let mut sink = SINK.lock().unwrap_or_else(|e| e.into_inner());
        sink.stream = None;
        sink.early.clear();
        sink.given = 0;
    }
    // A core of an earlier run waiting for room hears it is over.
    TAKEN.notify_waiters();
    if let Some(core) = core {
        // Never more than a moment: a core whose node stopped by itself says so at once.
        RUNTIME
            .spawn(async move {
                match tokio::time::timeout(STOP_WAIT, core.shutdown()).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::debug!("the core had stopped already: {e:#}"),
                    Err(_) => tracing::warn!("the core did not stop in time"),
                }
                drop(core);
            })
            .await?;
    }
    RUNTIME
        .spawn_blocking(move || wait_until_free(Path::new(&data_dir)))
        .await?
}

/// Longest a core takes to stop for a move.
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// Waits a moment for a core that just stopped, or a move, to let go of the data folder:
/// its lock goes with the last of its tasks. Fails if it is still held after
/// [`FREE_WAIT`].
fn wait_until_free(dir: &Path) -> Result<()> {
    let tries = FREE_WAIT.as_millis() / 100;
    let mut held = anyhow!("not tried");
    for _ in 0..tries {
        match Store::open(dir.to_path_buf()).and_then(|store| store.hold_running().map(drop)) {
            Ok(()) => return Ok(()),
            Err(e) => held = e,
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(held.context(format!(
        "the data folder {} is still in use {}s after Fogata stopped: try again in a moment",
        dir.display(),
        FREE_WAIT.as_secs()
    )))
}

/// Longest a core that stopped, or a move, is waited for to let go of the data folder.
const FREE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// What changes from now on; a new call replaces the previous stream. The screens say
/// when they took each event ([`events_taken`]): no more than [`GIVEN_AT_MOST`] are given
/// before they do.
pub fn events(sink: StreamSink<UiEvent>) -> Result<()> {
    let mut held = SINK.lock().unwrap_or_else(|e| e.into_inner());
    // What the previous stream was given and did not take is never taken now.
    held.given = held.early.len();
    for e in std::mem::take(&mut held.early) {
        if sink.add(e).is_err() {
            held.given = 0;
            drop(held);
            TAKEN.notify_waiters();
            return Ok(());
        }
    }
    held.stream = Some(sink);
    drop(held);
    TAKEN.notify_waiters();
    Ok(())
}

/// A QR code for a link.
#[frb(sync)]
pub fn qr(text: String) -> Result<UiQr> {
    let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::Q)
        .map_err(|e| anyhow!("{e}"))?;
    Ok(UiQr {
        size: code.width() as u32,
        dark: code
            .to_colors()
            .into_iter()
            .map(|c| c == Color::Dark)
            .collect(),
    })
}

/// Plays the short sounds on the output the voice engine calls `id` (as it lists devices),
/// or on the system's default when `None` or not connected.
#[frb(sync)]
pub fn set_sound_output(id: Option<String>) {
    crate::sounds::use_output(id);
}

/// Plays a short sound without waiting for it.
#[frb(sync)]
pub fn play_sound(sound: UiSound) {
    crate::sounds::play(sound_of(sound));
}

/// Plays `sound` (the ring or the ringback) again every bar until called again; `None`
/// stops it at once, fading out.
#[frb(sync)]
pub fn play_ring(sound: Option<UiSound>) {
    crate::sounds::ring(sound.map(sound_of));
}

fn sound_of(sound: UiSound) -> Sound {
    match sound {
        UiSound::Join => Sound::Join,
        UiSound::Leave => Sound::Leave,
        UiSound::Message => Sound::Message,
        UiSound::Mute => Sound::Mute,
        UiSound::Unmute => Sound::Unmute,
        UiSound::ShareStart => Sound::ShareStart,
        UiSound::ShareStop => Sound::ShareStop,
        UiSound::WatchStart => Sound::WatchStart,
        UiSound::WatchStop => Sound::WatchStop,
        UiSound::Ring => Sound::Ring,
        UiSound::Ringback => Sound::Ringback,
        UiSound::Notice => Sound::Notice,
        UiSound::CallEnded => Sound::CallEnded,
    }
}

/// Where flutter_webrtc's native code finds the microphone's noise suppression: the
/// address of a table of C functions in this library, for its `fogataNoiseSuppression`.
#[frb(sync)]
pub fn noise_suppressor() -> i64 {
    std::ptr::addr_of!(crate::denoise::API) as usize as i64
}

/// Saves a copy of the file at `path`, which someone else sent, into `dir` under the name
/// `name` made safe (see [`fogata_core::saved_name`]), or the next free one: never over a
/// file nor through a link. Marked as downloaded from the internet. Returns where it went.
pub fn save_received_file(path: String, name: String, dir: String) -> Result<String> {
    let mut from = std::fs::File::open(&path).with_context(|| format!("cannot read {path}"))?;
    save_received(&mut from, &name, &dir)
}

/// [`save_received_file`] for a file someone else sent that is held in memory (a picture's
/// preview).
pub fn save_received_bytes(bytes: Vec<u8>, name: String, dir: String) -> Result<String> {
    save_received(&mut bytes.as_slice(), &name, &dir)
}

fn save_received(from: &mut impl std::io::Read, name: &str, dir: &str) -> Result<String> {
    std::fs::create_dir_all(dir).with_context(|| format!("cannot make {dir}"))?;
    let path = fogata_core::save_received(from, Path::new(dir), name)
        .with_context(|| format!("cannot save {name}"))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Opens Windows' file manager with `path` selected; nothing happens elsewhere. Explorer
/// reads its command line itself and splits it at commas, so the path goes in quotes after
/// `/select,` (a Windows path cannot hold quotes), as the command line is written. It is
/// started from the Windows folder: by its name alone, Windows would look for it in the
/// application's folder and the current one first.
#[frb(sync)]
pub fn reveal_in_explorer(path: String) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        anyhow::ensure!(!path.contains('"'), "not a path: {path}");
        let windows = std::env::var_os("SystemRoot")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
        let mut explorer = std::process::Command::new(windows.join("explorer.exe"))
            .raw_arg(format!("/select,\"{path}\""))
            .spawn()?;
        // It hands the window to the running Explorer and ends at once.
        std::thread::spawn(move || {
            let _ = explorer.wait();
        });
    }
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

/// Appears offline or not: friends and members show this device as offline (except in a
/// room they share), and it stays connected. The application also saves it as `offline`
/// in `app.json` for the next start.
pub async fn set_appear_offline(offline: bool) -> Result<()> {
    run(|core| async move { core.set_hidden(offline).await }).await
}

/// Someone used the keyboard or the mouse in the window: where the system does not say how
/// long the computer has gone unused, this is all that keeps this device from showing as
/// away after ten minutes. Cheap; the screens call it at most about once a second.
#[frb(sync)]
pub fn note_input() {
    crate::idle::note_input();
}

/// The name of the secret that keeps this device's data from being all encrypted, when one
/// is damaged (it does not open with the folder's key), if that is what keeps it.
pub async fn damaged_secret() -> Result<Option<String>> {
    run(|core| async move {
        // Opens every secret of the folder to find it: away from the async threads.
        let damaged = tokio::task::spawn_blocking(move || core.damaged_secret()).await?;
        Ok(damaged.map(|path| {
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        }))
    })
    .await
}

/// Sets the damaged secret aside at the next start, which takes back its copy from before
/// encrypting and finishes; the screens start the application again after this.
pub async fn set_damaged_secret_aside() -> Result<()> {
    run(|core| async move {
        tokio::task::spawn_blocking(move || core.set_damaged_secret_aside()).await?
    })
    .await
}

/// How this device's secrets (its identity, friends and direct chats) are kept on this
/// computer.
pub async fn at_rest() -> Result<UiAtRest> {
    run(|core| async move {
        Ok(match core.at_rest() {
            fogata_core::AtRest::Sealed => UiAtRest::Sealed,
            fogata_core::AtRest::Unfinished => UiAtRest::Unfinished,
            fogata_core::AtRest::NoKeychain => UiAtRest::NoKeychain,
            fogata_core::AtRest::Plain => UiAtRest::Plain,
        })
    })
    .await
}

/// Before the application quits: closes every connection, so that friends and members see
/// this device go at once. The core takes no network calls after it.
pub async fn shutdown() -> Result<()> {
    run(|core| async move { core.shutdown().await }).await
}

// ----- Profile -----

pub async fn my_profile() -> Result<UiProfile> {
    run(|core| async move {
        let made = core
            .profile()
            .await?
            .context("this device has no profile")?;
        Ok(profile(&made))
    })
    .await
}

pub async fn set_profile(name: String, preset: u8, photo: Option<Vec<u8>>) -> Result<()> {
    run(|core| async move {
        core.set_profile(Some(name), Some(to_avatar(preset, photo)))
            .await
    })
    .await
}

// ----- Friends -----

pub async fn friends() -> Result<Vec<UiFriend>> {
    run(|core| async move { Ok(core.friends().await?.into_iter().map(friend).collect()) }).await
}

pub async fn requests() -> Result<UiRequests> {
    run(|core| async move {
        let (incoming, outgoing) = core.requests().await?;
        Ok(UiRequests {
            incoming: incoming.into_iter().map(request).collect(),
            outgoing: outgoing.into_iter().map(request).collect(),
        })
    })
    .await
}

/// A friend link, also shown as a QR code, for one person and ten minutes.
pub async fn friend_link() -> Result<UiInvitation> {
    run(|core| async move { Ok(invitation(core.friend_link().await?)) }).await
}

/// A short friend code for one person and ten minutes.
pub async fn friend_code() -> Result<UiInvitation> {
    run(|core| async move { Ok(invitation(core.friend_code().await?)) }).await
}

/// Sends a friend request with a link or a code from the other device.
pub async fn add_friend(text: String) -> Result<()> {
    run(|core| async move { core.add_friend(&text).await }).await
}

/// Sends a friend request to a member of a group this device is in, from its card in the
/// group: no link or code needed. `name` is the name the group shows for it.
pub async fn ask_member(device_id: String, name: String) -> Result<()> {
    let device = device(&device_id)?;
    run(|core| async move { core.ask_member(device, &name).await }).await
}

pub async fn answer_request(device_id: String, accept: bool) -> Result<()> {
    let device = device(&device_id)?;
    run(|core| async move { core.answer_request(device, accept).await }).await
}

/// Forgets a friend, or cancels a request this device sent.
pub async fn remove_friend(device_id: String) -> Result<()> {
    let device = device(&device_id)?;
    run(|core| async move { core.remove_friend(device).await }).await
}

/// Changes the banner friends see on this device's card. The picture is a PNG or JPEG of at
/// most 64 KiB.
pub async fn set_banner(color: u32, picture: Option<Vec<u8>>) -> Result<()> {
    run(|core| async move { core.set_banner(color, picture).await }).await
}

fn animated(of: UiAnimated) -> fogata_core::Animated {
    match of {
        UiAnimated::Photo => fogata_core::Animated::Photo,
        UiAnimated::Banner => fogata_core::Animated::Banner,
    }
}

/// Makes `picture` (a GIF or WebP of at most 550 KiB; none to stop) move in place of this
/// device's picture or banner. The still
/// picture, set first with [`set_profile`] or [`set_banner`], must be its first frame.
pub async fn set_animation(of: UiAnimated, picture: Option<Vec<u8>>) -> Result<()> {
    run(|core| async move { core.set_animation(animated(of), picture).await }).await
}

/// The animation of a friend's or member's picture or banner, while the still picture it
/// moves in place of is the device's picture or banner.
pub async fn animation(device_id: String, of: UiAnimated) -> Result<Option<UiAnimation>> {
    let device = device(&device_id)?;
    run(|core| async move {
        Ok(core
            .animation(device, animated(of))
            .await?
            .map(|(still, picture)| UiAnimation { still, picture }))
    })
    .await
}

/// The bio friends see on this device's card; none until the person writes one.
pub async fn my_bio() -> Result<Option<String>> {
    run(|core| async move { core.bio().await }).await
}

/// Changes the bio friends see on this device's card: at most 190 characters on 4 lines,
/// tidied as names are. An empty one shows no bio.
pub async fn set_bio(text: String) -> Result<()> {
    run(|core| async move { core.set_bio(text).await }).await
}

/// A friend's banner, once it sent one.
pub async fn friend_banner(device_id: String) -> Result<Option<UiBanner>> {
    let device = device(&device_id)?;
    run(|core| async move {
        Ok(core.friend_banner(device).await?.map(|b| UiBanner {
            color: b.color,
            picture: b.picture,
        }))
    })
    .await
}

/// The ids of the friends this device has in common with a friend online now.
pub async fn mutual_friends(device_id: String) -> Result<Vec<String>> {
    let device = device(&device_id)?;
    run(|core| async move {
        Ok(core
            .mutual_friends(device)
            .await?
            .iter()
            .map(|d| hex::encode(d.as_bytes()))
            .collect())
    })
    .await
}

// ----- Groups -----

/// Every group, live rooms first.
pub async fn groups() -> Result<Vec<UiGroup>> {
    run(|core| async move { Ok(core.groups().await?.into_iter().map(group).collect()) }).await
}

/// What the members of a group did, the newest first: `take` lines after the first `skip`.
pub async fn group_activity(group_id: String, skip: u32, take: u32) -> Result<UiActivityPage> {
    let id = self::group_id(&group_id)?;
    run(|core| async move {
        let (lines, more) = core
            .group_activity(id, skip as usize, take as usize)
            .await?;
        Ok(UiActivityPage {
            lines: lines.into_iter().map(activity).collect(),
            more,
        })
    })
    .await
}

pub async fn members(group_id: String) -> Result<Vec<UiMember>> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { Ok(core.members(id).await?.into_iter().map(member).collect()) }).await
}

/// Makes a group with these friends in it; returns its id.
pub async fn create_group(name: String, icon: u8, friends: Vec<String>) -> Result<String> {
    let friends = friends
        .iter()
        .map(|f| device(f))
        .collect::<Result<Vec<_>>>()?;
    run(|core| async move {
        let id = core.create_group(&name, icon, &friends).await?;
        Ok(hex::encode(id.as_bytes()))
    })
    .await
}

/// Groups friends added this device to, waiting for the person to join or decline.
pub async fn group_invitations() -> Result<Vec<UiGroupInvite>> {
    run(|core| async move {
        Ok(core
            .group_invitations()
            .await?
            .into_iter()
            .map(group_invite)
            .collect())
    })
    .await
}

/// Joins a group a friend added this device to.
pub async fn accept_group_invitation(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.accept_group_invitation(id).await }).await
}

/// Declines a group a friend added this device to.
pub async fn decline_group_invitation(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.decline_group_invitation(id).await }).await
}

/// A single-use invite to a group, for people who are not friends yet (managers).
pub async fn invite(group_id: String, days: u32) -> Result<String> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.invite(id, u64::from(days)).await }).await
}

/// Starts joining a group with an invite; returns the group's name.
pub async fn join_group(text: String) -> Result<String> {
    run(|core| async move { Ok(core.join_group(&text, None, None).await?.1) }).await
}

pub async fn add_to_group(group_id: String, friend_id: String) -> Result<()> {
    let (id, friend) = (self::group_id(&group_id)?, device(&friend_id)?);
    run(|core| async move { core.add_to_group(id, friend).await }).await
}

pub async fn remove_from_group(group_id: String, member_id: String) -> Result<()> {
    let (id, member) = (self::group_id(&group_id)?, device(&member_id)?);
    run(|core| async move { core.remove_from_group(id, member).await }).await
}

/// Moves a member below this device in the group up to just below it.
pub async fn promote(group_id: String, member_id: String) -> Result<()> {
    let (id, member) = (self::group_id(&group_id)?, device(&member_id)?);
    run(|core| async move { core.promote(id, member).await }).await
}

/// Moves a member below this device in the group down to the end.
pub async fn demote(group_id: String, member_id: String) -> Result<()> {
    let (id, member) = (self::group_id(&group_id)?, device(&member_id)?);
    run(|core| async move { core.demote(id, member).await }).await
}

/// Who manages a group: the first `first` on its list, or everyone (`None`).
pub async fn set_group_managers(group_id: String, first: Option<u8>) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.set_managers(id, first).await }).await
}

pub async fn rename_group(group_id: String, name: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.rename_group(id, &name).await }).await
}

pub async fn set_group_icon(group_id: String, icon: u8) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.set_group_icon(id, icon).await }).await
}

/// Chooses the group's banner for everyone (managers): a PNG, JPEG, GIF or WebP of at most
/// 550 KiB, or none to take it away.
pub async fn set_group_banner(group_id: String, picture: Option<Vec<u8>>) -> Result<()> {
    let group = self::group_id(&group_id)?;
    run(|core| async move { core.set_group_banner(group, picture).await }).await
}

/// The group's banner, when it has one.
pub async fn group_banner(group_id: String) -> Result<Option<Vec<u8>>> {
    let group = self::group_id(&group_id)?;
    run(|core| async move { core.group_banner(group).await }).await
}

/// Sets the group's picture for everyone, a small PNG or JPEG, or none (managers).
pub async fn set_group_picture(group_id: String, picture: Option<Vec<u8>>) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.set_group_picture(id, picture).await }).await
}

/// The most members and the most people in each voice room (managers).
pub async fn set_group_limits(group_id: String, members: u8, room: u8) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.set_group_limits(id, members, room).await }).await
}

/// Opens a voice room called `name`, or renames `voice` (managers); returns its id.
pub async fn set_voice_room(
    group_id: String,
    voice: Option<String>,
    name: String,
) -> Result<String> {
    let id = self::group_id(&group_id)?;
    let voice = voice.as_deref().map(voice_id).transpose()?;
    run(|core| async move {
        let voice = core.set_voice_room(id, voice, &name).await?;
        Ok(hex::encode(voice))
    })
    .await
}

/// Closes a voice room nobody is in; the first one stays (managers).
pub async fn remove_voice_room(group_id: String, voice: String) -> Result<()> {
    let (id, voice) = (self::group_id(&group_id)?, voice_id(&voice)?);
    run(|core| async move { core.remove_voice_room(id, voice).await }).await
}

/// Who may enter a voice room: everyone in the group (`members` none), or only those
/// members (device ids) and the managers (managers).
pub async fn set_room_access(
    group_id: String,
    voice: String,
    members: Option<Vec<String>>,
) -> Result<()> {
    let (id, voice) = (self::group_id(&group_id)?, voice_id(&voice)?);
    let members = members
        .map(|m| m.iter().map(|d| device(d)).collect::<Result<Vec<_>>>())
        .transpose()?;
    run(|core| async move { core.set_room_access(id, voice, members).await }).await
}

/// Takes a member below this device out of a voice room now; it may come back at once.
pub async fn remove_from_room(group_id: String, voice: String, member_id: String) -> Result<()> {
    let (id, voice, member) = (
        self::group_id(&group_id)?,
        voice_id(&voice)?,
        device(&member_id)?,
    );
    run(|core| async move { core.remove_from_room(id, voice, member).await }).await
}

/// Keeps a member below this device out of a voice room for `minutes`, or until someone
/// above it lets it back (none); it leaves the room now.
pub async fn keep_out_of_room(
    group_id: String,
    voice: String,
    member_id: String,
    minutes: Option<u32>,
) -> Result<()> {
    let (id, voice, member) = (
        self::group_id(&group_id)?,
        voice_id(&voice)?,
        device(&member_id)?,
    );
    run(|core| async move { core.keep_out_of_room(id, voice, member, minutes).await }).await
}

/// Lets a member below this device back into a voice room before its time out runs out.
pub async fn let_back_into_room(group_id: String, voice: String, member_id: String) -> Result<()> {
    let (id, voice, member) = (
        self::group_id(&group_id)?,
        voice_id(&voice)?,
        device(&member_id)?,
    );
    run(|core| async move { core.let_back_into_room(id, voice, member).await }).await
}

/// Cancels the group's invites not used yet (managers).
pub async fn cancel_invites(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.cancel_invites(id).await }).await
}

/// People waiting for this manager to let them in with an invite, oldest first.
pub async fn join_asks() -> Result<Vec<UiJoinAsk>> {
    run(|core| async move { Ok(core.join_asks().await?.into_iter().map(join_ask).collect()) }).await
}

/// Lets someone waiting with an invite in, or says no, which also cancels the invite
/// (managers).
pub async fn answer_join(group_id: String, device_id: String, accept: bool) -> Result<()> {
    let id = self::group_id(&group_id)?;
    let device = device(&device_id)?;
    run(|core| async move { core.answer_join(id, device, accept).await }).await
}

pub async fn leave_group(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.leave_group(id).await }).await
}

// ----- Rooms -----

/// Enters one of a group's voice rooms, starting a room there if nobody is in it; leaves
/// any other room first. Fails when the voice room is full.
pub async fn join_room(group_id: String, voice: String) -> Result<()> {
    let (id, voice) = (self::group_id(&group_id)?, voice_id(&voice)?);
    run(|core| async move { core.join_voice_room(id, voice, None).await }).await
}

pub async fn leave_room(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.leave_room(id).await }).await
}

pub async fn say(group_id: String, text: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.say(id, &text).await }).await
}

/// Tells the others in the voice room of the group this device is in that it is typing in
/// the room's chat (`typing`), or stopped.
pub async fn room_typing(group_id: String, typing: bool) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.typing(id, typing).await }).await
}

/// One of a group's voice rooms, if a room is open in it.
pub async fn room(group_id: String, voice: String) -> Result<Option<UiRoom>> {
    let (id, voice) = (self::group_id(&group_id)?, voice_id(&voice)?);
    run(|core| async move {
        let me = core.me();
        Ok(core.room(id, voice).await?.map(|r| room_view(r, &me)))
    })
    .await
}

/// Shares a file in the room while this device is in it; returns the file's id. A video
/// goes with its first picture, size and length, as this device read them.
pub async fn share_file(group_id: String, path: String, video: Option<UiVideo>) -> Result<String> {
    let id = self::group_id(&group_id)?;
    let video = video.map(|v| fogata_core::VideoPreview {
        thumb: v.thumb,
        width: v.width,
        height: v.height,
        duration_ms: u64::try_from(v.duration_ms).unwrap_or(0),
    });
    run(|core| async move {
        let info = core.share_file(id, PathBuf::from(path), video).await?;
        Ok(hex::encode(info.id))
    })
    .await
}

/// Fetches a video of the room into `dir` to play it, unless it is there already, without
/// sharing it on; returns where it is. Progress comes as download events.
pub async fn fetch_to_play(group_id: String, file: String, dir: String) -> Result<String> {
    let (id, file) = (self::group_id(&group_id)?, file_id(&file)?);
    run(|core| async move {
        let path = core.fetch_to_play(id, file, Path::new(&dir)).await?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

pub async fn stop_sharing(group_id: String, file: String) -> Result<()> {
    let (id, file) = (self::group_id(&group_id)?, file_id(&file)?);
    run(|core| async move { core.stop_sharing(id, file).await }).await
}

/// Starts downloading a room's file into `dir`; returns where it will be saved.
pub async fn download(group_id: String, file: String, dir: String) -> Result<String> {
    let (id, file) = (self::group_id(&group_id)?, file_id(&file)?);
    run(|core| async move {
        let dest = core.download(id, file, Path::new(&dir)).await?;
        Ok(dest.to_string_lossy().into_owned())
    })
    .await
}

/// A picture of the room fetched into memory for its preview, up to 10 MB: nothing is
/// written to disk and this device does not share it on.
pub async fn fetch_preview(group_id: String, file: String) -> Result<Vec<u8>> {
    let (id, file) = (self::group_id(&group_id)?, file_id(&file)?);
    run(|core| async move { core.fetch_preview(id, file, 10 * 1024 * 1024).await }).await
}

// ----- Direct calls -----

/// Rings a friend who is online, in a call of their own (leaving any room). Returns the
/// call's group, whose room the call takes place in.
pub async fn start_call(device_id: String) -> Result<String> {
    let friend = device(&device_id)?;
    run(|core| async move { Ok(hex::encode(core.start_call(friend).await?.as_bytes())) }).await
}

/// Answers a call ringing here (leaving any room).
pub async fn accept_call(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.accept_call(id).await }).await
}

pub async fn decline_call(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.decline_call(id).await }).await
}

/// Stops ringing, or hangs up.
pub async fn end_call(group_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    run(|core| async move { core.end_call(id).await }).await
}

/// Rings a friend into the call `group_id` this device is in.
pub async fn add_to_call(group_id: String, device_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    let friend = device(&device_id)?;
    run(|core| async move { core.add_to_call(id, friend).await }).await
}

/// Stops ringing a friend into the call `group_id`.
pub async fn cancel_add(group_id: String, device_id: String) -> Result<()> {
    let id = self::group_id(&group_id)?;
    let friend = device(&device_id)?;
    run(|core| async move { core.cancel_add(id, friend).await }).await
}

/// The direct calls ringing or going on.
pub async fn calls() -> Result<Vec<UiCall>> {
    run(|core| async move { Ok(core.calls().await?.iter().map(call).collect()) }).await
}

/// The friends still in a call this device left, by device id: calling one of them goes
/// back into that call.
pub async fn call_returns() -> Result<Vec<String>> {
    run(|core| async move {
        Ok(core
            .call_returns()
            .iter()
            .map(|d| hex::encode(d.as_bytes()))
            .collect())
    })
    .await
}

// ----- Calls -----

/// The local TURN endpoint for the WebRTC engine: media to other members goes through it
/// and over this device's member connections.
pub async fn media_bridge() -> Result<UiMediaBridge> {
    run(|core| async move {
        let bridge = core.media_bridge().await?;
        Ok(UiMediaBridge {
            host: bridge.host.to_string(),
            port: bridge.port,
            username: bridge.username,
            password: bridge.password,
        })
    })
    .await
}

/// Sends a call signal (offer, answer, candidate or state) to a member in a group's room.
pub async fn send_signal(group_id: String, device_id: String, payload: String) -> Result<()> {
    let (id, to) = (self::group_id(&group_id)?, device(&device_id)?);
    run(|core| async move { core.signal(id, to, payload.into_bytes()).await }).await
}

// ----- Network -----

/// What the network looks like from here, in plain words.
pub async fn network_check() -> Result<Vec<UiFinding>> {
    run(|core| async move {
        let report = core.network_report().await?;
        let mut findings = diagnose(&report);
        if let Some(command) = core.firewall_command().await {
            findings.insert(0, firewall_finding(&command));
        }
        Ok(findings.iter().map(finding).collect())
    })
    .await
}

/// What keeps friends and members from reaching this computer, from what the running node
/// has seen so far: at once, without the check's wait, for the screens where a connection
/// failed. Nothing is guessed: what the node has not seen is left out.
pub async fn reachability() -> Result<UiReach> {
    run(|core| async move { Ok(reach(&diagnose::reach(&core.network_report().await?))) }).await
}

/// The command that lets Fogata through this computer's own firewall, when that lets
/// nothing in for it (ufw on Linux, Windows' firewall, macOS's application firewall).
pub async fn firewall_command() -> Result<Option<String>> {
    run(|core| async move { Ok(core.firewall_command().await) }).await
}

fn firewall_finding(command: &str) -> fogata_core::diagnose::Finding {
    fogata_core::diagnose::Finding {
        level: fogata_core::diagnose::Level::Bad,
        topic: fogata_core::diagnose::Topic::Firewall,
        text: format!(
            "This computer's firewall lets nothing in for Fogata, so a friend in the same place behind another router cannot reach it. To let Fogata in, run: {command}"
        ),
    }
}

#[cfg(test)]
mod voice_tests {
    use super::*;

    /// Stopping the core closes the microphone and throws away what it was recording, and
    /// stops a voice message playing.
    #[test]
    fn stopping_the_core_ends_a_recording() {
        let _one = crate::voice::record::tests::ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stopped = crate::voice::record::fake_recording();
        assert!(crate::voice::record::state().is_some());
        crate::voice::play::load_quietly("friend", "file");
        assert!(crate::voice::play::state().is_some());
        let dir = std::env::temp_dir().join(format!("fogata-stop-voice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        RUNTIME
            .block_on(stop_core(dir.to_string_lossy().into_owned()))
            .unwrap();
        assert!(stopped.load(std::sync::atomic::Ordering::Relaxed));
        assert!(crate::voice::record::state().is_none());
        assert!(crate::voice::play::state().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod stop_tests {
    use super::*;

    /// Stopping the core says so when the data folder is still held after the wait,
    /// rather than that it is free for a move.
    #[test]
    fn stopping_says_when_the_folder_is_still_held() {
        let _one = crate::voice::record::tests::ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = std::env::temp_dir().join(format!("fogata-stop-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::open_with(dir.clone(), fogata_core::vault::Keychain::Off).unwrap();
        let held = store.hold_running().unwrap();
        let stopped = RUNTIME.block_on(stop_core(dir.to_string_lossy().into_owned()));
        let said = format!("{:#}", stopped.unwrap_err());
        assert!(said.contains("is still in use"), "{said}");
        drop(held);
        RUNTIME
            .block_on(stop_core(dir.to_string_lossy().into_owned()))
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Stopping the core waits for a start under way, which would otherwise put a core in
    /// place once the stop said it had stopped.
    #[test]
    fn stopping_waits_for_a_start_under_way() {
        let _one = crate::voice::record::tests::ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = std::env::temp_dir().join(format!("fogata-stop-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let starting = RUNTIME.block_on(STARTING.lock());
        let stopping = RUNTIME.spawn(stop_core(dir.to_string_lossy().into_owned()));
        std::thread::sleep(std::time::Duration::from_millis(500));
        assert!(
            !stopping.is_finished(),
            "the stop did not wait for the start"
        );
        drop(starting);
        RUNTIME.block_on(stopping).unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use crate::api::types::UiEventKind;
    use fogata_core::CoreEvent;

    /// One event that cannot be read loses that event only, and the end of the core's
    /// events tells the screens it stopped.
    #[test]
    fn the_screens_hear_when_the_core_stops() {
        let me = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let (send, events) = fogata_core::events::queue(None);
        send.send(CoreEvent::GroupsChanged).unwrap();
        send.send(CoreEvent::FriendsChanged).unwrap();
        send.send(CoreEvent::JoinAsksChanged).unwrap();
        drop(send);
        let got = Mutex::new(Vec::new());
        RUNTIME.block_on(pass_on(
            events,
            |e| {
                assert!(!matches!(e, CoreEvent::FriendsChanged), "unreadable");
                event(e, &me)
            },
            |e| {
                got.lock().unwrap().push(e.kind);
                true
            },
        ));
        let got = got.into_inner().unwrap();
        assert!(
            matches!(
                got[..],
                [
                    UiEventKind::GroupsChanged,
                    UiEventKind::JoinAsksChanged,
                    UiEventKind::Stopped
                ]
            ),
            "{got:?}"
        );
    }

    /// Screens that take nothing are given no more than [`GIVEN_AT_MOST`] events: the rest
    /// wait in the core's queue, and come as the screens take theirs, none lost.
    #[test]
    fn events_wait_until_the_screens_take_them() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _one = crate::voice::record::tests::ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reset = || {
            let mut sink = SINK.lock().unwrap();
            sink.stream = None;
            sink.early.clear();
            sink.given = 0;
        };
        reset();
        let me = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let run = RUNS.load(Ordering::SeqCst);
        let (send, events) = fogata_core::events::queue(None);
        for n in 0..1000 {
            send.send(CoreEvent::Notice(n.to_string())).unwrap();
        }
        drop(send);
        let given = std::sync::Arc::new(AtomicUsize::new(0));
        let counted = given.clone();
        let passing = RUNTIME.spawn(pass_on(
            events,
            move |e| event(e, &me),
            move |e| {
                counted.fetch_add(1, Ordering::SeqCst);
                deliver(e, run)
            },
        ));
        let settled = |n| {
            for _ in 0..200 {
                if given.load(Ordering::SeqCst) == n {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    return given.load(Ordering::SeqCst);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            given.load(Ordering::SeqCst)
        };
        // The screens take `n` of the events kept for them, and say so.
        let mut seen = Vec::new();
        let mut take = |n: usize| {
            {
                let mut sink = SINK.lock().unwrap();
                let n = n.min(sink.early.len());
                seen.extend(sink.early.drain(..n));
            }
            events_taken(n as u32);
        };
        assert_eq!(settled(GIVEN_AT_MOST), GIVEN_AT_MOST);
        assert_eq!(SINK.lock().unwrap().early.len(), GIVEN_AT_MOST);
        // A late word of taken events, with nothing taken: no more come, none is pushed out.
        events_taken(100);
        assert_eq!(settled(GIVEN_AT_MOST), GIVEN_AT_MOST);
        assert_eq!(SINK.lock().unwrap().early.len(), GIVEN_AT_MOST);
        take(100);
        assert_eq!(settled(GIVEN_AT_MOST + 100), GIVEN_AT_MOST + 100);
        // Taken as they come: all of them, and the core's end, none lost on the way.
        while !passing.is_finished() {
            take(1000);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        take(1000);
        assert_eq!(given.load(Ordering::SeqCst), 1001);
        assert_eq!(seen.len(), 1001, "events were lost");
        reset();
    }

    /// Once another core runs, the earlier one's events go nowhere, its end included.
    #[test]
    fn a_core_stopped_on_purpose_says_nothing() {
        let me = fogata_crypto::DeviceIdentity::generate()
            .unwrap()
            .public_key();
        let (send, events) = fogata_core::events::queue(None);
        send.send(CoreEvent::GroupsChanged).unwrap();
        drop(send);
        let got = Mutex::new(Vec::new());
        RUNTIME.block_on(pass_on(
            events,
            |e| event(e, &me),
            |e| {
                got.lock().unwrap().push(e.kind);
                false
            },
        ));
        assert_eq!(got.into_inner().unwrap().len(), 1);
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fogata-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::symlink_metadata(path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    }

    /// The data folder is made private before the window's lock file is opened in it, a
    /// folder already there included.
    #[test]
    fn the_data_folder_is_the_owners_only() {
        let dir = temp("prepare");
        let data = dir.join("data");
        prepare_data_dir(data.to_string_lossy().into_owned()).unwrap();
        assert!(data.join("window").is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(mode(&data), 0o700);
            assert_eq!(mode(&data.join("window")), 0o700);
            std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::set_permissions(data.join("window"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
            prepare_data_dir(data.to_string_lossy().into_owned()).unwrap();
            assert_eq!(mode(&data), 0o700);
            assert_eq!(mode(&data.join("window")), 0o700);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file of the window's is the owner's only, written whole, and a link where it goes
    /// is replaced, not followed.
    #[test]
    fn the_windows_files_are_written_privately() {
        let dir = temp("private");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("banner.png");
        write_private(file.to_string_lossy().into_owned(), b"one".to_vec()).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"one");
        #[cfg(unix)]
        {
            assert_eq!(mode(&file), 0o600);
            let target = dir.join("elsewhere");
            std::fs::write(&target, b"theirs").unwrap();
            let link = dir.join("report.zip");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            write_private(link.to_string_lossy().into_owned(), b"two".to_vec()).unwrap();
            assert_eq!(std::fs::read(&target).unwrap(), b"theirs");
            assert!(std::fs::symlink_metadata(&link).unwrap().is_file());
            assert_eq!(std::fs::read(&link).unwrap(), b"two");
        }
        // Nothing of the temporary file is left.
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            if cfg!(unix) { 3 } else { 1 }
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod seal_tests {
    use super::seal_off;

    /// A release build seals a new data folder whatever `FOGATA_SEAL` says: nothing that
    /// starts it (a script, a launcher) can leave the secrets plain. A debug build, as the
    /// tests run, leaves it plain when asked.
    #[test]
    fn only_a_debug_build_leaves_a_folder_plain() {
        let off = || Some("off".to_string());
        assert!(!seal_off(false, off()));
        assert!(seal_off(true, off()));
        assert!(!seal_off(true, None));
        assert!(!seal_off(true, Some("on".to_string())));
    }
}
