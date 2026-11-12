//! How the bridge runs the calls of the screens ([`crate::api::FLUTTER_RUST_BRIDGE_HANDLER`],
//! in place of the bridge's default handler). The default starts a runtime and a pool of
//! threads of its own, each with as many threads as the computer has cores, that only wait
//! for the core's runtime nearly all the time: here an async call runs on the core's runtime
//! ([`RUNTIME`]) and a blocking one on that runtime's pool for blocking work, whose threads
//! come when needed and go after a while idle. Nothing a call does waits for a thread of
//! the bridge's: a call that blocks takes a worker of the core for that time, as any of the
//! core's own tasks that writes a file does.
//!
//! A panic in a call reaches the screens with what it said scrubbed, as the log has it
//! ([`crate::logs::scrubbed_panic`]): the bridge hands the screens the message as it is,
//! and they may write it where no scrubbing reaches. That holds from the reading of the
//! call's arguments on, which the bridge does before the call runs ([`Scrubbed`]), to the
//! writing of its result. So does a panic in a task a call waits for ([`joined`],
//! [`scrub_panicked`]): tokio's error for it carries what it said, and so does any text of
//! the core's that tells of one, wherever it was put into words ([`told`]).

use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    pin::Pin,
    task::{Context, Poll},
};

use flutter_rust_bridge::{
    BaseAsyncRuntime, BaseCodec, BaseThreadPool, Dart2RustMessageSse, DartAbi, DartFnFuture,
    DartOpaque, JoinHandle, Rust2DartMessageTrait,
    for_generated::{
        ErrorListener, Executor, NoOpErrorListener, SimpleExecutor, SimpleHandler, TaskContext,
        TaskInfo, TaskRetFutTrait,
    },
};

use crate::api::app::RUNTIME;

/// The bridge's handler.
pub type Handler = Scrubbed<NoOpErrorListener>;

/// Makes the handler, once, on the first call of the screens.
pub fn handler() -> Handler {
    // As the bridge's default handler checks.
    assert_eq!(
        crate::frb_generated::FLUTTER_RUST_BRIDGE_CODEGEN_VERSION,
        flutter_rust_bridge::for_generated::FLUTTER_RUST_BRIDGE_RUNTIME_VERSION,
        "the bridge's generated code and its library are of different versions"
    );
    scrubbed_handler(NoOpErrorListener)
}

fn scrubbed_handler<L: ErrorListener + Sync>(listener: L) -> Scrubbed<L> {
    Scrubbed(SimpleHandler::new(
        ScrubbedPanics(SimpleExecutor::new(listener, Blocking, OnCore)),
        listener,
    ))
}

/// The bridge's own handler, its calls run by [`ScrubbedPanics`], and the reading of each
/// call's arguments [`scrubbed`] too: the bridge reads them (`prepare`) before the executor
/// has the call, and a panic there (a value from the screens that does not read as what it
/// should, whose message may then hold it) reached the screens with what it said. A call run
/// on the window's thread reads its arguments within the call, which the executor runs.
pub struct Scrubbed<L: ErrorListener + Sync>(
    SimpleHandler<ScrubbedPanics<SimpleExecutor<L, Blocking, OnCore>>, L>,
);

impl<L: ErrorListener + Sync> std::fmt::Debug for Scrubbed<L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Scrubbed")
    }
}

impl<L: ErrorListener + Sync> flutter_rust_bridge::Handler for Scrubbed<L> {
    fn wrap_normal<Rust2DartCodec, PrepareFn, TaskFn>(
        &self,
        task_info: TaskInfo,
        prepare: PrepareFn,
    ) where
        PrepareFn: FnOnce() -> TaskFn,
        TaskFn: FnOnce(TaskContext) -> Result<Rust2DartCodec::Message, Rust2DartCodec::Message>
            + Send
            + 'static,
        Rust2DartCodec: BaseCodec,
    {
        self.0
            .wrap_normal::<Rust2DartCodec, _, _>(task_info, move || scrubbed(prepare));
    }

    fn wrap_sync<Rust2DartCodec, SyncTaskFn>(
        &self,
        task_info: TaskInfo,
        sync_task: SyncTaskFn,
    ) -> <Rust2DartCodec::Message as Rust2DartMessageTrait>::WireSyncRust2DartType
    where
        SyncTaskFn: FnOnce() -> Result<Rust2DartCodec::Message, Rust2DartCodec::Message>,
        Rust2DartCodec: BaseCodec,
    {
        self.0.wrap_sync::<Rust2DartCodec, _>(task_info, sync_task)
    }

    fn wrap_async<Rust2DartCodec, PrepareFn, TaskFn, TaskRetFut>(
        &self,
        task_info: TaskInfo,
        prepare: PrepareFn,
    ) where
        PrepareFn: FnOnce() -> TaskFn,
        TaskFn: FnOnce(TaskContext) -> TaskRetFut + Send + 'static,
        TaskRetFut: Future<Output = Result<Rust2DartCodec::Message, Rust2DartCodec::Message>>
            + TaskRetFutTrait,
        Rust2DartCodec: BaseCodec,
    {
        self.0
            .wrap_async::<Rust2DartCodec, _, _, _>(task_info, move || scrubbed(prepare));
    }

    fn dart_fn_invoke(
        &self,
        dart_fn: DartOpaque,
        args: Vec<DartAbi>,
    ) -> DartFnFuture<Dart2RustMessageSse> {
        self.0.dart_fn_invoke(dart_fn, args)
    }

    fn dart_fn_handle_output(&self, call_id: i32, message: Dart2RustMessageSse) {
        self.0.dart_fn_handle_output(call_id, message);
    }
}

/// Async calls run on the core's runtime.
#[derive(Debug, Clone, Copy)]
pub struct OnCore;

impl BaseAsyncRuntime for OnCore {
    fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        RUNTIME.spawn(future)
    }
}

/// Blocking calls run on the core's runtime's pool for blocking work.
#[derive(Debug, Clone, Copy)]
pub struct Blocking;

impl BaseThreadPool for Blocking {
    fn execute<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        // The job sends its own result, or its panic, to the screens.
        drop(RUNTIME.spawn_blocking(job));
    }
}

/// Runs each call as `E` does, a panic in it carrying what it said scrubbed.
pub struct ScrubbedPanics<E>(E);

impl<E> std::fmt::Debug for ScrubbedPanics<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScrubbedPanics")
    }
}

/// Runs `f`, a panic in it going on with what it said scrubbed. Going on with
/// [`resume_unwind`] calls no panic hook again: the panic was reported once, and the
/// backtrace the bridge's hook kept for it stays the one sent.
fn scrubbed<R>(f: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => {
            // A scrubbing that fails tells nothing of what was said.
            let said = catch_unwind(AssertUnwindSafe(|| {
                crate::logs::scrubbed_panic(payload.as_ref())
            }))
            .unwrap_or_else(|_| "(could not be scrubbed)".to_string());
            drop(payload);
            resume_unwind(Box::new(said))
        }
    }
}

/// The result of a task waited for, a panic in it told with what it said scrubbed.
pub(crate) fn joined<T>(result: Result<T, tokio::task::JoinError>) -> anyhow::Result<T> {
    result.map_err(|e| match e.try_into_panic() {
        Ok(payload) => anyhow::anyhow!(
            "a task panicked: {}",
            crate::logs::scrubbed_panic(payload.as_ref())
        ),
        Err(e) => e.into(),
    })
}

/// `error` scrubbed whole when a panic of a task waited for is among its causes (tokio's
/// error for it carries what the panic said), or its words tell of one (put into words on
/// the way, in an error of another kind: see [`told`]); else as it is, since the screens
/// read the core's words in it.
pub(crate) fn scrub_panicked(error: anyhow::Error) -> anyhow::Error {
    let panicked = error.chain().any(|cause| {
        cause
            .downcast_ref::<tokio::task::JoinError>()
            .is_some_and(tokio::task::JoinError::is_panic)
    });
    let text = format!("{error:#}");
    if panicked || tells_of_panic(&text) {
        anyhow::anyhow!(crate::logs::scrubbed(&text))
    } else {
        error
    }
}

/// A text of the core's for the screens (why a download failed, a note), scrubbed whole when
/// it tells of a task's panic: an error that held tokio's for it (which carries what the
/// panic said) may have been put into words anywhere on its way here. Any other text stays
/// as the core wrote it, since the screens read its words.
pub(crate) fn told(text: String) -> String {
    if tells_of_panic(&text) {
        crate::logs::scrubbed(&text)
    } else {
        text
    }
}

/// Whether `text` holds tokio's words for a task that panicked (`task 12 panicked with
/// message "..."`), or a panic's as Rust says it.
fn tells_of_panic(text: &str) -> bool {
    text.contains(" panicked")
}

/// A call's future, each poll [`scrubbed`].
struct ScrubbedFuture<F>(Pin<Box<F>>);

impl<F: Future> Future for ScrubbedFuture<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let future = self.0.as_mut();
        scrubbed(|| future.poll(cx))
    }
}

impl<E: Executor> Executor for ScrubbedPanics<E> {
    fn execute_normal<Rust2DartCodec, TaskFn>(&self, task_info: TaskInfo, task: TaskFn)
    where
        TaskFn: FnOnce(TaskContext) -> Result<Rust2DartCodec::Message, Rust2DartCodec::Message>
            + Send
            + 'static,
        Rust2DartCodec: BaseCodec,
    {
        self.0
            .execute_normal::<Rust2DartCodec, _>(task_info, move |context| {
                scrubbed(|| task(context))
            });
    }

    fn execute_sync<Rust2DartCodec, SyncTaskFn>(
        &self,
        task_info: TaskInfo,
        sync_task: SyncTaskFn,
    ) -> Rust2DartCodec::Message
    where
        SyncTaskFn: FnOnce() -> Result<Rust2DartCodec::Message, Rust2DartCodec::Message>,
        Rust2DartCodec: BaseCodec,
    {
        self.0
            .execute_sync::<Rust2DartCodec, _>(task_info, || scrubbed(sync_task))
    }

    fn execute_async<Rust2DartCodec, TaskFn, TaskRetFut>(&self, task_info: TaskInfo, task: TaskFn)
    where
        TaskFn: FnOnce(TaskContext) -> TaskRetFut + Send + 'static,
        TaskRetFut: Future<Output = Result<Rust2DartCodec::Message, Rust2DartCodec::Message>>
            + TaskRetFutTrait,
        Rust2DartCodec: BaseCodec,
    {
        self.0
            .execute_async::<Rust2DartCodec, _, _>(task_info, move |context| {
                ScrubbedFuture(Box::pin(scrubbed(|| task(context))))
            });
    }
}

#[cfg(test)]
mod tests {
    use flutter_rust_bridge::for_generated::{FfiCallMode, SseCodec};

    use super::*;
    use crate::api::app::MAX_WORKERS;

    /// A device's network id, which the log never shows.
    const PEER: &str = "12D3KooWGRUVh7MLpAFtJ6tXc1oFmDy2X1WbSmN4QZbWq9VtXk3A";

    /// What a panic that went through `f` said.
    fn said(f: impl FnOnce()) -> String {
        let payload = catch_unwind(AssertUnwindSafe(f)).expect_err("it panics");
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .expect("it says something")
    }

    /// A panic in a call, blocking, async or on the window's thread, goes on to the
    /// bridge (which tells the screens) saying what the log says of it, nothing personal.
    #[test]
    fn a_panic_in_a_call_goes_on_scrubbed() {
        let raw = said(|| panic!("no answer from {PEER}"));
        assert!(raw.contains(PEER));
        let blocking = said(|| scrubbed(|| panic!("no answer from {PEER}")));
        assert!(
            blocking.contains("no answer") && !blocking.contains(PEER),
            "{blocking}"
        );
        let polled = said(|| {
            RUNTIME.block_on(ScrubbedFuture(Box::pin(async {
                tokio::task::yield_now().await;
                panic!("no answer from {PEER}");
            })))
        });
        assert!(
            polled.contains("no answer") && !polled.contains(PEER),
            "{polled}"
        );
        let executor = ScrubbedPanics(SimpleExecutor::new(NoOpErrorListener, Blocking, OnCore));
        let info = TaskInfo {
            debug_name: "test",
            port: None,
            mode: FfiCallMode::Sync,
        };
        let on_the_window = said(|| {
            executor.execute_sync::<SseCodec, _>(info, || panic!("no answer from {PEER}"));
        });
        assert!(
            on_the_window.contains("no answer") && !on_the_window.contains(PEER),
            "{on_the_window}"
        );
        // A panic that says nothing as text says so.
        assert_eq!(said(|| scrubbed(|| std::panic::panic_any(7))), "(not text)");
        // What does not panic is untouched.
        assert_eq!(scrubbed(|| 5), 5);
    }

    /// A panic in a task a call waits for, which tokio's error tells with what it said,
    /// reaches the screens scrubbed too, also below the errors of the core around it; any
    /// other error stays as the core wrote it, since the screens read its words.
    #[test]
    fn a_panic_in_a_task_waited_for_goes_on_scrubbed() {
        let panicked = || {
            RUNTIME.block_on(async {
                RUNTIME
                    .spawn(async { panic!("no answer from {PEER}") })
                    .await
            })
        };
        let raw = panicked().unwrap_err().to_string();
        assert!(raw.contains(PEER), "{raw}");
        let told = joined(panicked()).unwrap_err().to_string();
        assert!(told.contains("no answer") && !told.contains(PEER), "{told}");
        let below = scrub_panicked(
            anyhow::Error::from(panicked().unwrap_err())
                .context("cannot reach the friend's device"),
        );
        let below = format!("{below:?}");
        assert!(
            below.contains("cannot reach") && below.contains("panicked") && !below.contains(PEER),
            "{below}"
        );
        let words = format!("a group called \"Gaming\" already exists; {PEER}");
        let kept = scrub_panicked(anyhow::anyhow!(words.clone()));
        assert_eq!(kept.to_string(), words);
        // Cancelled, not panicked: said as tokio says it.
        let cancelled = RUNTIME.block_on(async {
            let task = RUNTIME.spawn(std::future::pending::<()>());
            task.abort();
            task.await
        });
        assert!(
            joined(cancelled)
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        // `blocking` and `run` go through these.
        let blocking = RUNTIME
            .block_on(crate::api::app::blocking(|| -> () {
                panic!("no answer from {PEER}")
            }))
            .unwrap_err()
            .to_string();
        assert!(!blocking.contains(PEER), "{blocking}");
    }

    /// What the bridge's handler tells of a panic (its error listener hears what the screens
    /// are sent).
    static HEARD: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    #[derive(Clone, Copy)]
    struct Hears;

    impl ErrorListener for Hears {
        fn on_error(&self, error: flutter_rust_bridge::for_generated::HandlerError) {
            HEARD
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(error.message());
        }
    }

    /// A panic while the bridge reads a call's arguments, before the call runs (a value
    /// from the screens that does not read as what it should, whose message may hold it),
    /// reaches the screens scrubbed as one in the call does.
    #[test]
    fn a_panic_reading_a_calls_arguments_goes_on_scrubbed() {
        use flutter_rust_bridge::Handler as _;
        type Message = <SseCodec as BaseCodec>::Message;
        let handler = scrubbed_handler(Hears);
        let info = |name| TaskInfo {
            debug_name: name,
            port: Some(Default::default()),
            mode: FfiCallMode::Normal,
        };
        let heard = |name: &str| {
            HEARD
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .find(|h| h.contains(name))
                .cloned()
        };
        handler.wrap_async::<SseCodec, _, _, _>(
            info("async"),
            || -> fn(TaskContext) -> std::future::Ready<Result<Message, Message>> {
                panic!("async: not text: {PEER}")
            },
        );
        handler.wrap_normal::<SseCodec, _, _>(
            info("blocking"),
            || -> fn(TaskContext) -> Result<Message, Message> {
                panic!("blocking: not text: {PEER}")
            },
        );
        for name in ["async", "blocking"] {
            let told = heard(name).unwrap_or_else(|| panic!("{name}: nothing heard"));
            assert!(!told.contains(PEER), "{told}");
        }
    }

    /// Text of the core's that tells of a task's panic is scrubbed whole on its way to the
    /// screens, however it was put into words: tokio's error inside an I/O error (whose
    /// causes skip it) or already in a string (why a download failed, a note). Other text
    /// stays as the core wrote it.
    #[test]
    fn text_that_tells_of_a_panic_is_scrubbed_on_its_way_to_the_screens() {
        let panicked = || {
            RUNTIME
                .block_on(async {
                    RUNTIME
                        .spawn(async { panic!("cannot open /home/marta/notes.txt for {PEER}") })
                        .await
                })
                .unwrap_err()
        };
        let hidden = scrub_panicked(
            anyhow::Error::from(std::io::Error::other(panicked())).context("the file did not come"),
        );
        let hidden = format!("{hidden:#}");
        assert!(hidden.contains("did not come"), "{hidden}");
        assert!(
            !hidden.contains(PEER) && !hidden.contains("marta"),
            "{hidden}"
        );
        let worded = told(format!("the file did not come: {}", panicked()));
        assert!(
            !worded.contains(PEER) && !worded.contains("marta"),
            "{worded}"
        );
        let words = "you left the room".to_string();
        assert_eq!(told(words.clone()), words);
    }

    /// The bridge starts no runtime nor threads of its own: async calls run on the core's
    /// workers, blocking ones on its pool for blocking work.
    #[test]
    fn calls_run_on_the_cores_runtime() {
        let core = RUNTIME.handle().id();
        let async_on = RUNTIME
            .block_on(OnCore.spawn(async { tokio::runtime::Handle::current().id() }))
            .unwrap();
        assert_eq!(async_on, core);
        let (tx, rx) = std::sync::mpsc::channel();
        Blocking.execute(move || {
            let _ = tx.send(tokio::runtime::Handle::try_current().map(|h| h.id()).ok());
        });
        let blocking_on = rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(blocking_on, Some(core));
        assert!(RUNTIME.metrics().num_workers() <= MAX_WORKERS);
    }
}
