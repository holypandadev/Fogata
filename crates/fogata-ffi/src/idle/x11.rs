//! The X server's own word in an X11 session where no service on the session bus says how
//! long the computer has gone unused (i3 and other window managers without a desktop): the
//! MIT-SCREEN-SAVER extension's idle time, which counts the keyboard and the mouse of the
//! whole server. Its library (libXss) and Xlib are opened when first needed, as the video
//! player opens GStreamer: without them, nothing changes and only input in Fogata's window
//! counts.

use std::{
    ffi::{CStr, c_char, c_int, c_ulong, c_void},
    sync::mpsc,
    time::Duration,
};

use tokio::sync::oneshot;

/// `XScreenSaverInfo` (X11/extensions/scrnsaver.h).
#[repr(C)]
struct Info {
    window: c_ulong,
    state: c_int,
    kind: c_int,
    til_or_since: c_ulong,
    idle: c_ulong,
    event_mask: c_ulong,
}

type OpenDisplay = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type CloseDisplay = unsafe extern "C" fn(*mut c_void) -> c_int;
type DefaultRoot = unsafe extern "C" fn(*mut c_void) -> c_ulong;
type QueryExtension = unsafe extern "C" fn(*mut c_void, *mut c_int, *mut c_int) -> c_int;
type QueryInfo = unsafe extern "C" fn(*mut c_void, c_ulong, *mut Info) -> c_int;

/// The functions of Xlib and libXss that are used.
#[derive(Clone, Copy)]
struct Api {
    open_display: OpenDisplay,
    close_display: CloseDisplay,
    default_root: DefaultRoot,
    has_extension: QueryExtension,
    query: QueryInfo,
}

impl Api {
    /// Looks the functions up; None without either library or any of them.
    fn load() -> Option<Self> {
        let xlib = library(c"libX11.so.6")?;
        let xss = library(c"libXss.so.1")?;
        // SAFETY: each symbol is looked up by its name in the library that defines it and
        // given the type its header declares.
        unsafe {
            Some(Self {
                open_display: std::mem::transmute::<*mut c_void, OpenDisplay>(symbol(
                    xlib,
                    c"XOpenDisplay",
                )?),
                close_display: std::mem::transmute::<*mut c_void, CloseDisplay>(symbol(
                    xlib,
                    c"XCloseDisplay",
                )?),
                default_root: std::mem::transmute::<*mut c_void, DefaultRoot>(symbol(
                    xlib,
                    c"XDefaultRootWindow",
                )?),
                has_extension: std::mem::transmute::<*mut c_void, QueryExtension>(symbol(
                    xss,
                    c"XScreenSaverQueryExtension",
                )?),
                query: std::mem::transmute::<*mut c_void, QueryInfo>(symbol(
                    xss,
                    c"XScreenSaverQueryInfo",
                )?),
            })
        }
    }
}

/// A connection of its own to the X server named by `DISPLAY`. It is opened, used and
/// closed by the one thread of [`Idle`]; Xlib calls wait for the server for as long as it
/// takes, so no other thread touches it.
struct Connection {
    api: Api,
    display: *mut c_void,
    root: c_ulong,
}

impl Connection {
    /// Opens the display; None when it does not open or has no MIT-SCREEN-SAVER.
    fn open(api: Api) -> Option<Self> {
        // SAFETY: a null name opens the display `DISPLAY` names.
        let display = unsafe { (api.open_display)(std::ptr::null()) };
        if display.is_null() {
            return None;
        }
        let (mut event, mut error) = (0, 0);
        // SAFETY: an open display and two integers that live across the call.
        if unsafe { (api.has_extension)(display, &mut event, &mut error) } == 0 {
            tracing::debug!("the X server has no MIT-SCREEN-SAVER");
            // SAFETY: the display opened above, not used again.
            unsafe { (api.close_display)(display) };
            return None;
        }
        // SAFETY: an open display.
        let root = unsafe { (api.default_root)(display) };
        Some(Self { api, display, root })
    }

    /// How long since the last keyboard or mouse input anywhere on the server.
    fn idle(&mut self) -> Option<Duration> {
        let mut info = Info {
            window: 0,
            state: 0,
            kind: 0,
            til_or_since: 0,
            idle: 0,
            event_mask: 0,
        };
        // SAFETY: the display opened in `open`, used by this thread alone, its root window,
        // and an `XScreenSaverInfo` that lives across the call, which only writes into it.
        let ok = unsafe { (self.api.query)(self.display, self.root, &mut info) };
        // `unsigned long` is 32 bits on 32-bit systems.
        #[allow(clippy::unnecessary_cast)]
        (ok != 0).then(|| Duration::from_millis(info.idle as u64))
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: the display opened in `open`, not used again.
        unsafe { (self.api.close_display)(self.display) };
    }
}

type Reply = oneshot::Sender<Option<Duration>>;

/// The X server's idle time, asked of a thread of its own: Xlib's calls block for as long as
/// the server takes, and a stuck one must not hold a worker of the runtime. Questions that
/// the thread has not reached yet are not queued beyond one, and one whose asker has given
/// up is not asked of the server.
pub(super) struct Idle {
    asks: mpsc::SyncSender<Reply>,
}

impl Idle {
    /// Loads the libraries and starts the thread, which opens the display; None without the
    /// libraries or a `DISPLAY`. When the display does not open, or has no extension, the
    /// thread ends and every question gets None.
    pub(super) fn open() -> Option<Self> {
        std::env::var_os("DISPLAY")?;
        let api = Api::load()?;
        Self::start(move || Connection::open(api), Connection::idle)
    }

    /// Runs [`connect`](Connection::open) on a new thread, which then answers with `ask`.
    /// `C` is made there and never leaves it.
    fn start<C>(
        connect: impl FnOnce() -> Option<C> + Send + 'static,
        ask: impl Fn(&mut C) -> Option<Duration> + Send + 'static,
    ) -> Option<Self> {
        let (asks, questions) = mpsc::sync_channel::<Reply>(1);
        std::thread::Builder::new()
            .name("fogata-idle-x11".into())
            .spawn(move || {
                let Some(mut connection) = connect() else {
                    return;
                };
                // Ends when [`Idle`] is dropped.
                while let Ok(reply) = questions.recv() {
                    if !reply.is_closed() {
                        let _ = reply.send(ask(&mut connection));
                    }
                }
            })
            .ok()?;
        Some(Self { asks })
    }

    /// How long since the last keyboard or mouse input anywhere on the server; None when it
    /// does not say within [`within`](Duration), or is busy with an earlier question.
    pub(super) async fn idle(&self, within: Duration) -> Option<Duration> {
        let (reply, answer) = oneshot::channel();
        self.asks.try_send(reply).ok()?;
        tokio::time::timeout(within, answer).await.ok()?.ok()?
    }
}

fn library(name: &CStr) -> Option<*mut c_void> {
    // SAFETY: a library name; the handle is kept for the run.
    let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        tracing::debug!("{} not found", name.to_string_lossy());
        return None;
    }
    Some(handle)
}

/// # Safety
/// `library` is a handle from [`library`].
unsafe fn symbol(library: *mut c_void, name: &CStr) -> Option<*mut c_void> {
    // SAFETY: the caller's handle and a symbol's name.
    let found = unsafe { libc::dlsym(library, name.as_ptr()) };
    (!found.is_null()).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Run on an X server, `DISPLAY` set (the test rigs' Xvfb): `cargo test -p fogata-ffi
    /// -- --ignored reads_the_idle_time_of_an_x_server`.
    #[tokio::test]
    #[ignore = "needs an X server"]
    async fn reads_the_idle_time_of_an_x_server() {
        let idle = Idle::open().expect("an X server with MIT-SCREEN-SAVER");
        let within = Duration::from_secs(3);
        let first = idle.idle(within).await.expect("an idle time");
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let later = idle.idle(within).await.expect("an idle time");
        assert!(
            later >= first + Duration::from_millis(1000),
            "{first:?} then {later:?}"
        );
    }

    #[tokio::test]
    async fn answers_from_its_own_thread() {
        let main = std::thread::current().id();
        let idle = Idle::start(
            || Some(()),
            move |()| {
                assert_ne!(std::thread::current().id(), main);
                Some(Duration::from_secs(7))
            },
        )
        .unwrap();
        assert_eq!(
            idle.idle(Duration::from_secs(1)).await,
            Some(Duration::from_secs(7))
        );
    }

    /// A server that does not answer holds the thread of its connection, never the runtime's
    /// worker: the question times out, the one after it is not even waited for, and the
    /// runtime's only worker goes on with other work meanwhile.
    #[test]
    fn a_stuck_server_does_not_hold_a_worker() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let idle = Idle::start(
                || Some(()),
                |()| {
                    std::thread::sleep(Duration::from_secs(3));
                    None
                },
            )
            .unwrap();
            let ticks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let counted = ticks.clone();
            let ticker = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    counted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
            let began = Instant::now();
            assert_eq!(idle.idle(Duration::from_millis(200)).await, None);
            // The thread is inside the first question; one more waits in the queue, and
            // those after it are turned away at once.
            assert_eq!(idle.idle(Duration::from_millis(200)).await, None);
            let third = Instant::now();
            assert_eq!(idle.idle(Duration::from_secs(5)).await, None);
            assert!(third.elapsed() < Duration::from_millis(100));
            assert!(began.elapsed() < Duration::from_secs(1));
            assert!(ticks.load(std::sync::atomic::Ordering::Relaxed) > 20);
            ticker.abort();
        });
        // The stuck thread is the one the runtime's shutdown does not wait for.
        runtime.shutdown_background();
    }

    #[tokio::test]
    async fn a_display_that_does_not_open_answers_none() {
        let idle = Idle::start(|| None::<()>, |()| Some(Duration::ZERO)).unwrap();
        assert_eq!(idle.idle(Duration::from_secs(1)).await, None);
        assert_eq!(idle.idle(Duration::from_secs(1)).await, None);
    }

    #[tokio::test]
    async fn the_connection_is_closed_when_it_is_dropped() {
        struct Closes(std::sync::mpsc::Sender<()>);
        impl Drop for Closes {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let (closed, closes) = std::sync::mpsc::channel();
        let idle = Idle::start(move || Some(Closes(closed)), |_| Some(Duration::ZERO)).unwrap();
        assert!(idle.idle(Duration::from_secs(1)).await.is_some());
        drop(idle);
        closes.recv_timeout(Duration::from_secs(2)).unwrap();
    }
}
