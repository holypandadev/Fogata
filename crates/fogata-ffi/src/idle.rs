//! Whether anyone uses this computer. The system says how long ago its keyboard or mouse
//! was last used, anywhere: on Linux the compositor's idle notifications under Wayland
//! (`wayland.rs`), else D-Bus (the desktop's screen saver, or GNOME's idle monitor);
//! `GetLastInputInfo` on Windows, CoreGraphics on macOS. Input in Fogata's own window
//! ([`note_input`]) counts too, and is all there is where the system does not say. After
//! [`AWAY_AFTER`] without any, friends and members see this device as away
//! ([`fogata_core::Core::set_away`]); the first input brings it back. What they see changes
//! only at the turn of a minute of the clock ([`Coarse`]): they learn the minute someone
//! came back or left, not the second.

#[cfg(target_os = "linux")]
mod wayland;
// Calls Xlib and libXss through dlopen; every unsafe block in it says why it holds.
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
mod x11;

use std::{
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Without input for this long, the device is away.
pub(crate) const AWAY_AFTER: Duration = Duration::from_secs(10 * 60);
/// How often the system is asked while someone is here: away comes at most this late.
const ASK_WHILE_HERE: Duration = Duration::from_secs(15);
/// How often while away: input anywhere brings the device back at most this late.
const ASK_WHILE_AWAY: Duration = Duration::from_secs(2);

/// What the system says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Reading {
    /// Since the last keyboard or mouse input anywhere, when the system says.
    pub idle: Option<Duration>,
    /// The screen is locked, when the system says.
    pub locked: bool,
}

/// Decides whether this device is away, from the system's readings and input in the window.
#[derive(Debug)]
pub(crate) struct Clock {
    after: Duration,
    started: Instant,
    last_input: Instant,
    away: bool,
}

impl Clock {
    /// Away after `after` without input; starting counts as input.
    pub(crate) fn new(after: Duration, now: Instant) -> Self {
        Self {
            after,
            started: now,
            last_input: now,
            away: false,
        }
    }

    /// Takes the system's reading at `now`, and the last input in the window if there was
    /// any: whether the device is away now.
    pub(crate) fn step(&mut self, now: Instant, reading: Reading, window: Option<Instant>) -> bool {
        let mut known = self.started;
        if let Some(at) = window {
            known = known.max(at);
        }
        match reading.idle.and_then(|idle| now.checked_sub(idle)) {
            // The system's word replaces earlier ones: a compositor that said nothing
            // until now (someone here) may say that the last input was long ago.
            Some(at) if !reading.locked => self.last_input = known.max(at),
            // Typing the password at a locked screen is not using the computer: while it
            // is locked, only the input before counts.
            _ => self.last_input = self.last_input.max(known),
        }
        self.away = now.saturating_duration_since(self.last_input) >= self.after;
        self.away
    }

    /// How long until the system is asked again.
    pub(crate) fn next_in(&self) -> Duration {
        if self.away {
            ASK_WHILE_AWAY
        } else {
            ASK_WHILE_HERE
                .min(self.after / 4)
                .max(Duration::from_millis(250))
        }
    }
}

/// What friends and members are shown: away or here as [`Clock`] says, changed only at the
/// turn of a step of the clock (a minute; a quarter of the wait when that is shorter, for
/// tests), so the moment of the change says no more than which minute it came in.
#[derive(Debug)]
pub(crate) struct Coarse {
    step: Duration,
    shown: bool,
    /// When the change waiting is shown.
    due: Option<SystemTime>,
}

impl Coarse {
    pub(crate) fn new(after: Duration) -> Self {
        Self {
            step: Duration::from_secs(60)
                .min(after / 4)
                .max(Duration::from_millis(250)),
            shown: false,
            due: None,
        }
    }

    /// What is shown at `now`, the clock saying `away`.
    pub(crate) fn step(&mut self, away: bool, now: SystemTime) -> bool {
        if away == self.shown {
            self.due = None;
            return self.shown;
        }
        let due = *self.due.get_or_insert_with(|| {
            let since = now.duration_since(UNIX_EPOCH).unwrap_or_default();
            let step = self.step.as_millis().max(1);
            let next = (since.as_millis() / step + 1) * step;
            UNIX_EPOCH + Duration::from_millis(next as u64)
        });
        if now >= due {
            self.shown = away;
            self.due = None;
        }
        self.shown
    }

    /// How long until a change waiting is shown, if one is.
    pub(crate) fn wait(&self, now: SystemTime) -> Option<Duration> {
        self.due
            .map(|due| due.duration_since(now).unwrap_or(Duration::ZERO))
    }
}

/// The last input in Fogata's window.
static WINDOW_INPUT: Mutex<Option<Instant>> = Mutex::new(None);
/// The device is away now: input in the window wakes the clock at once.
static AWAY: AtomicBool = AtomicBool::new(false);
static WAKE: LazyLock<tokio::sync::Notify> = LazyLock::new(tokio::sync::Notify::new);

/// Someone used the keyboard or the mouse in Fogata's window.
pub(crate) fn note_input() {
    *WINDOW_INPUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    if AWAY.load(Ordering::Relaxed) {
        WAKE.notify_one();
    }
}

/// How long without input makes the device away. Debug builds take
/// `FOGATA_TEST_AWAY_AFTER_MS`, for tests that cannot wait ten minutes.
fn away_after() -> Duration {
    if cfg!(debug_assertions)
        && let Some(ms) = std::env::var("FOGATA_TEST_AWAY_AFTER_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
    {
        return Duration::from_millis(ms);
    }
    AWAY_AFTER
}

/// Starts the clock, once for the whole run of the application: it tells whichever core
/// runs, a core started again after a move included.
pub(crate) fn watch(runtime: &tokio::runtime::Runtime) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        runtime.spawn(tick());
    });
}

async fn tick() {
    let after = away_after();
    let mut system = System::new(after);
    let mut clock = Clock::new(after, Instant::now());
    let mut coarse = Coarse::new(after);
    loop {
        let reading = system.read().await;
        let window = *WINDOW_INPUT.lock().unwrap_or_else(|e| e.into_inner());
        let away = clock.step(Instant::now(), reading, window);
        AWAY.store(away, Ordering::Relaxed);
        let now = SystemTime::now();
        let shown = coarse.step(away, now);
        // The node tells friends and members only when what they see changes.
        let _ = crate::api::app::run(move |core| async move { core.set_away(shown).await }).await;
        let next = match coarse.wait(now) {
            Some(due) => clock.next_in().min(due),
            None => clock.next_in(),
        };
        let _ = tokio::time::timeout(next, WAKE.notified()).await;
    }
}

/// How long a question to the system may take.
#[cfg(target_os = "linux")]
const ASK_FOR: Duration = Duration::from_secs(3);

/// Asks the desktop over the session bus: `org.freedesktop.ScreenSaver` (KDE on X11 and
/// other desktops; KDE on Wayland says it does not support it) and
/// `org.gnome.Mutter.IdleMonitor` for the idle time, the screen saver for the lock. A
/// question the desktop answers with an error is not asked again.
#[cfg(target_os = "linux")]
struct System {
    /// The compositor's notifications, while they come; D-Bus is asked for the lock in
    /// any case, and for the idle time when they do not come.
    wayland: Option<std::sync::Arc<wayland::Watch>>,
    bus: Option<zbus::Connection>,
    /// When the session bus was last asked for, while there is none.
    connected: Option<Instant>,
    screen_saver: bool,
    mutter: bool,
    lock: bool,
    /// The X server's own idle time (MIT-SCREEN-SAVER), in an X11 session where no
    /// service on the bus tells it; opened when first needed.
    x11: Option<x11::Idle>,
    x11_tried: bool,
}

/// How often a session bus that could not be reached is asked for again.
#[cfg(target_os = "linux")]
const BUS_AGAIN: Duration = Duration::from_secs(60);

#[cfg(target_os = "linux")]
impl System {
    fn new(after: Duration) -> Self {
        Self {
            wayland: wayland::start(after, || WAKE.notify_one()),
            bus: None,
            connected: None,
            screen_saver: true,
            mutter: true,
            lock: true,
            x11: None,
            x11_tried: false,
        }
    }

    async fn read(&mut self) -> Reading {
        let mut reading = self.read_bus().await;
        if reading.idle.is_none() && self.wayland.is_none() {
            reading.idle = self.read_x11().await;
        }
        reading
    }

    /// The X server's idle time: only in an X11 session, where it counts all input (under
    /// Wayland, Xwayland sees only the input of X11 windows).
    async fn read_x11(&mut self) -> Option<Duration> {
        if !self.x11_tried {
            self.x11_tried = true;
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some()
                || std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland");
            if !wayland {
                self.x11 = x11::Idle::open();
            }
        }
        self.x11.as_ref()?.idle(ASK_FOR).await
    }

    async fn read_bus(&mut self) -> Reading {
        if self.bus.is_none() && self.connected.is_none_or(|at| at.elapsed() >= BUS_AGAIN) {
            self.connected = Some(Instant::now());
            self.bus = tokio::time::timeout(ASK_FOR, zbus::Connection::session())
                .await
                .ok()
                .and_then(Result::ok);
        }
        let Some(bus) = self.bus.clone() else {
            let said = self.wayland.as_ref().and_then(|w| w.said());
            return Reading {
                idle: said.map(|said| said.idle_at(Instant::now())),
                locked: false,
            };
        };
        const SCREEN_SAVER: (&str, &str, &str) = (
            "org.freedesktop.ScreenSaver",
            "/org/freedesktop/ScreenSaver",
            "org.freedesktop.ScreenSaver",
        );
        let said = self.wayland.as_ref().and_then(|w| w.said());
        let mut idle = said.map(|said| said.idle_at(Instant::now()));
        if idle.is_none() && self.screen_saver {
            idle = ask::<u32>(
                &bus,
                SCREEN_SAVER,
                "GetSessionIdleTime",
                &mut self.screen_saver,
            )
            .await
            .map(|seconds| Duration::from_secs(seconds.into()));
        }
        if idle.is_none() && self.mutter {
            idle = ask::<u64>(
                &bus,
                (
                    "org.gnome.Mutter.IdleMonitor",
                    "/org/gnome/Mutter/IdleMonitor/Core",
                    "org.gnome.Mutter.IdleMonitor",
                ),
                "GetIdletime",
                &mut self.mutter,
            )
            .await
            .map(Duration::from_millis);
        }
        let locked = self.lock
            && ask::<bool>(&bus, SCREEN_SAVER, "GetActive", &mut self.lock)
                .await
                .unwrap_or(false);
        Reading { idle, locked }
    }
}

/// Calls `method` of `(name, path, interface)` without arguments; an error answer turns
/// `offered` off, while a question that took too long is asked again next time.
#[cfg(target_os = "linux")]
async fn ask<T>(
    bus: &zbus::Connection,
    (name, path, interface): (&str, &str, &str),
    method: &str,
    offered: &mut bool,
) -> Option<T>
where
    T: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let call = bus.call_method(Some(name), path, Some(interface), method, &());
    match tokio::time::timeout(ASK_FOR, call).await {
        Ok(Ok(reply)) => match reply.body().deserialize::<T>() {
            Ok(value) => Some(value),
            Err(_) => {
                *offered = false;
                None
            }
        },
        Ok(Err(e)) => {
            tracing::debug!("{interface}.{method} not offered here: {e}");
            *offered = false;
            None
        }
        Err(_) => None,
    }
}

/// `GetLastInputInfo`: the tick of the last input of this session, against the tick now.
#[cfg(windows)]
struct System;

#[cfg(windows)]
impl System {
    fn new(_: Duration) -> Self {
        Self
    }

    #[allow(unsafe_code)]
    async fn read(&mut self) -> Reading {
        use windows_sys::Win32::{
            System::SystemInformation::GetTickCount64,
            UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        };
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: `info` is a LASTINPUTINFO with its size set, living across the call, which
        // only writes into it; GetTickCount64 takes nothing.
        let (ok, now) = unsafe { (GetLastInputInfo(&mut info), GetTickCount64()) };
        if ok == 0 {
            return Reading::default();
        }
        // The input's tick is the low 32 bits of the tick count, which wraps every 49 days.
        let since = (now as u32).wrapping_sub(info.dwTime);
        Reading {
            idle: Some(Duration::from_millis(since.into())),
            locked: false,
        }
    }
}

/// CoreGraphics: the seconds since the last input event of any kind in this login session.
#[cfg(target_os = "macos")]
struct System;

#[cfg(target_os = "macos")]
impl System {
    fn new(_: Duration) -> Self {
        Self
    }

    #[allow(unsafe_code)]
    async fn read(&mut self) -> Reading {
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
        }
        /// `kCGEventSourceStateCombinedSessionState`.
        const COMBINED_SESSION_STATE: i32 = 0;
        /// `kCGAnyInputEventType`.
        const ANY_INPUT_EVENT: u32 = u32::MAX;
        // SAFETY: a plain function of two integers, which reads the window server's counters.
        let seconds = unsafe {
            CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT)
        };
        Reading {
            idle: (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds)),
            locked: false,
        }
    }
}

/// Elsewhere only input in the window counts.
#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
struct System;

#[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
impl System {
    fn new(_: Duration) -> Self {
        Self
    }

    async fn read(&mut self) -> Reading {
        Reading::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    fn idle(for_: Duration) -> Reading {
        Reading {
            idle: Some(for_),
            locked: false,
        }
    }

    #[test]
    fn away_after_ten_minutes_without_input() {
        let start = Instant::now();
        let mut clock = Clock::new(AWAY_AFTER, start);
        assert!(!clock.step(start + 9 * MIN, idle(9 * MIN), None));
        assert_eq!(clock.next_in(), ASK_WHILE_HERE);
        assert!(clock.step(start + 10 * MIN, idle(10 * MIN), None));
        assert_eq!(clock.next_in(), ASK_WHILE_AWAY);
        assert!(clock.step(start + 30 * MIN, idle(30 * MIN), None));
    }

    #[test]
    fn input_anywhere_brings_it_back_at_once() {
        let start = Instant::now();
        let mut clock = Clock::new(AWAY_AFTER, start);
        assert!(clock.step(start + 12 * MIN, idle(12 * MIN), None));
        let now = start + 12 * MIN + Duration::from_secs(2);
        assert!(!clock.step(now, idle(Duration::from_millis(300)), None));
        // And ten minutes from that input on, away again.
        assert!(!clock.step(now + 9 * MIN, idle(9 * MIN), None));
        assert!(clock.step(now + 10 * MIN, idle(10 * MIN), None));
    }

    #[test]
    fn input_in_the_window_counts_where_the_system_says_nothing() {
        let start = Instant::now();
        let mut clock = Clock::new(AWAY_AFTER, start);
        let silent = Reading::default();
        // Starting counts as input.
        assert!(!clock.step(start + 9 * MIN, silent, None));
        assert!(clock.step(start + 10 * MIN, silent, None));
        let typed = start + 10 * MIN + Duration::from_secs(1);
        assert!(!clock.step(typed, silent, Some(typed)));
        assert!(!clock.step(typed + 9 * MIN, silent, Some(typed)));
        assert!(clock.step(typed + 10 * MIN, silent, Some(typed)));
        // The system's word counts too: whichever input is the latest.
        assert!(!clock.step(typed + 11 * MIN, idle(MIN), Some(typed)));
    }

    #[test]
    fn a_locked_screen_counts_as_no_input() {
        let start = Instant::now();
        let mut clock = Clock::new(AWAY_AFTER, start);
        assert!(!clock.step(start + 2 * MIN, idle(Duration::ZERO), None));
        // Locked two minutes in; the password typed at it later is no use of the computer.
        let locked = |for_| Reading {
            idle: Some(for_),
            locked: true,
        };
        assert!(!clock.step(start + 11 * MIN, locked(Duration::ZERO), None));
        assert!(clock.step(start + 12 * MIN, locked(Duration::ZERO), None));
        // Unlocked: back.
        assert!(!clock.step(start + 13 * MIN, idle(Duration::ZERO), None));
    }

    #[test]
    fn a_compositor_that_says_idle_late_is_believed() {
        // The compositor says nothing while someone is here (read as no idle time), then
        // that nobody touched anything for the whole wait.
        let start = Instant::now();
        let mut clock = Clock::new(AWAY_AFTER, start);
        for minute in 1..10 {
            assert!(!clock.step(start + minute * MIN, idle(Duration::ZERO), None));
        }
        let idled = start + 19 * MIN;
        assert!(clock.step(idled, idle(AWAY_AFTER), None));
        assert!(clock.step(idled + MIN, idle(AWAY_AFTER + MIN), None));
        // Input in the window since then still counts.
        let typed = idled + 2 * MIN;
        assert!(!clock.step(typed, idle(AWAY_AFTER + 2 * MIN), Some(typed)));
    }

    #[test]
    fn input_the_compositor_sees_at_a_locked_screen_does_not_count() {
        let start = Instant::now();
        let mut clock = Clock::new(AWAY_AFTER, start);
        assert!(!clock.step(start + MIN, idle(Duration::ZERO), None));
        let locked = |for_| Reading {
            idle: Some(for_),
            locked: true,
        };
        // Locked a minute in, then the compositor says idle, then input (the password
        // typed): away 10 minutes after the last input before the lock, all the same.
        assert!(!clock.step(start + 5 * MIN, locked(4 * MIN), None));
        assert!(!clock.step(start + 9 * MIN, locked(Duration::ZERO), None));
        assert!(clock.step(start + 11 * MIN, locked(Duration::ZERO), None));
        assert!(clock.step(start + 20 * MIN, locked(Duration::ZERO), None));
        // Unlocked: someone is here.
        assert!(!clock.step(start + 21 * MIN, idle(Duration::ZERO), None));
    }

    #[test]
    fn what_others_see_changes_only_at_the_turn_of_a_minute() {
        let minute = |m: u64, s: u64| UNIX_EPOCH + Duration::from_secs(1_800_000_000 + m * 60 + s);
        let mut coarse = Coarse::new(AWAY_AFTER);
        assert!(!coarse.step(false, minute(0, 10)));
        assert_eq!(coarse.wait(minute(0, 10)), None);
        // Away at 3:17: shown from 4:00, whatever second the clock said it.
        assert!(!coarse.step(true, minute(3, 17)));
        assert_eq!(coarse.wait(minute(3, 17)), Some(Duration::from_secs(43)));
        assert!(!coarse.step(true, minute(3, 59)));
        assert!(coarse.step(true, minute(4, 0)));
        assert_eq!(coarse.wait(minute(4, 0)), None);
        // Back at 9:02 and away again before 10:00: nothing shown changes.
        assert!(coarse.step(false, minute(9, 2)));
        assert!(coarse.step(true, minute(9, 40)));
        assert!(coarse.step(true, minute(10, 5)));
        // Back at 12:45: shown at 13:00.
        assert!(coarse.step(false, minute(12, 45)));
        assert!(!coarse.step(false, minute(13, 0)));
    }

    #[test]
    fn a_short_wait_for_tests_is_asked_about_often_enough() {
        let start = Instant::now();
        let mut clock = Clock::new(Duration::from_secs(4), start);
        assert_eq!(clock.next_in(), Duration::from_secs(1));
        assert!(clock.step(start + Duration::from_secs(4), Reading::default(), None));
    }
}
