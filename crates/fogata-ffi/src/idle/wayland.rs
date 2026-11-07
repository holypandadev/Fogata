//! The compositor's word on Wayland, where the desktop may not say over D-Bus how long the
//! computer has gone unused (KDE does not): an `ext_idle_notifier_v1` notification on the
//! first seat, on a connection of its own, with the time without input that makes this
//! device away. Version 2's input notification is asked for where the compositor has it: it
//! tracks the keyboard and the mouse alone, so a video playing (which inhibits the screen
//! saver) does not keep someone who is not touching the computer from being away. The
//! protocol's pure-Rust implementation is used: no system library is linked or loaded.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

/// What the compositor said last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Said {
    /// Input came since the notification was made, or since it last said idle.
    Here,
    /// No input since then.
    IdleSince(Instant),
}

impl Said {
    /// How long the computer has gone unused at `now`, as the system's reading. While the
    /// compositor says nothing, someone is here: it would have said idle.
    pub(super) fn idle_at(self, now: Instant) -> Duration {
        match self {
            Said::Here => Duration::ZERO,
            Said::IdleSince(at) => now.saturating_duration_since(at),
        }
    }
}

/// The notification's state, shared with the thread that follows it.
#[derive(Debug, Default)]
pub(super) struct Watch {
    /// None until the notification is made, and again once the connection is gone.
    said: Mutex<Option<Said>>,
}

impl Watch {
    pub(super) fn said(&self) -> Option<Said> {
        *self.said.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, said: Option<Said>) {
        *self.said.lock().unwrap_or_else(|e| e.into_inner()) = said;
    }
}

/// Follows the compositor's idle notifications on a thread of its own, when this session is
/// a Wayland one. `wake` is called on every change. A compositor without the protocol, or a
/// connection that fails or closes, leaves the watch saying nothing.
pub(super) fn start(timeout: Duration, wake: fn()) -> Option<Arc<Watch>> {
    std::env::var_os("WAYLAND_DISPLAY")?;
    let watch = Arc::new(Watch::default());
    let shared = watch.clone();
    let started = std::thread::Builder::new()
        .name("fogata-idle".into())
        .spawn(move || {
            if let Err(e) = follow(timeout, &shared, wake) {
                tracing::debug!("no idle notifications from the compositor: {e}");
            }
            shared.set(None);
            wake();
        });
    match started {
        Ok(_) => Some(watch),
        Err(e) => {
            tracing::debug!("cannot follow the compositor's idle notifications: {e}");
            None
        }
    }
}

struct State {
    watch: Arc<Watch>,
    timeout: Duration,
    wake: fn(),
}

fn follow(
    timeout: Duration,
    watch: &Arc<Watch>,
    wake: fn(),
) -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
    let handle = queue.handle();
    let seat: wl_seat::WlSeat = globals.bind(&handle, 1..=1, ())?;
    let notifier: ExtIdleNotifierV1 = globals.bind(&handle, 1..=2, ())?;
    let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
    let _notification = if notifier.version() >= 2 {
        notifier.get_input_idle_notification(ms, &seat, &handle, ())
    } else {
        notifier.get_idle_notification(ms, &seat, &handle, ())
    };
    let mut state = State {
        watch: watch.clone(),
        timeout,
        wake,
    };
    queue.roundtrip(&mut state)?;
    tracing::debug!(
        version = notifier.version(),
        "following the compositor's idle notifications"
    );
    if watch.said().is_none() {
        watch.set(Some(Said::Here));
    }
    loop {
        queue.blocking_dispatch(&mut state)?;
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let now = Instant::now();
        let said = match event {
            // Idle for the timeout by now.
            ext_idle_notification_v1::Event::Idled => {
                tracing::debug!("the compositor says the computer is idle");
                Said::IdleSince(now.checked_sub(state.timeout).unwrap_or(now))
            }
            ext_idle_notification_v1::Event::Resumed => {
                tracing::debug!("the compositor says the computer is used again");
                Said::Here
            }
            _ => return,
        };
        state.watch.set(Some(said));
        (state.wake)();
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtIdleNotifierV1,
        _: <ExtIdleNotifierV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_compositor_said_reads_as_time_without_input() {
        let now = Instant::now();
        assert_eq!(Said::Here.idle_at(now), Duration::ZERO);
        let since = now - Duration::from_secs(600);
        assert_eq!(
            Said::IdleSince(since).idle_at(now + Duration::from_secs(5)),
            Duration::from_secs(605)
        );
    }

    #[test]
    fn without_a_wayland_session_there_is_no_watch() {
        // Only checked where no compositor runs: a test does not connect to one.
        if std::env::var_os("WAYLAND_DISPLAY").is_none() {
            assert!(start(Duration::from_secs(1), || {}).is_none());
        }
    }
}
