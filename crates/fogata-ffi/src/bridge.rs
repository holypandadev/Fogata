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
//! and they may write it where no scrubbing reaches. So does a panic in a task a call waits
//! for ([`joined`], [`scrub_panicked`]): tokio's error for it carries what it said.

use std::{
    future::Future,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    pin::Pin,
    task::{Context, Poll},
};

use flutter_rust_bridge::{
    BaseAsyncRuntime, BaseCodec, BaseThreadPool, JoinHandle,
    for_generated::{
        Executor, NoOpErrorListener, SimpleExecutor, SimpleHandler, TaskContext, TaskInfo,
        TaskRetFutTrait,
    },
};

use crate::api::app::RUNTIME;

/// The bridge's handler.
pub type Handler = SimpleHandler<
    ScrubbedPanics<SimpleExecutor<NoOpErrorListener, Blocking, OnCore>>,
    NoOpErrorListener,
>;

/// Makes the handler, once, on the first call of the screens.
pub fn handler() -> Handler {
    // As the bridge's default handler checks.
    assert_eq!(
        crate::frb_generated::FLUTTER_RUST_BRIDGE_CODEGEN_VERSION,
        flutter_rust_bridge::for_generated::FLUTTER_RUST_BRIDGE_RUNTIME_VERSION,
        "the bridge's generated code and its library are of different versions"
    );
    SimpleHandler::new(
        ScrubbedPanics(SimpleExecutor::new(NoOpErrorListener, Blocking, OnCore)),
        NoOpErrorListener,
    )
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
/// error for it carries what the panic said); else as it is, since the screens read the
/// core's words in it.
pub(crate) fn scrub_panicked(error: anyhow::Error) -> anyhow::Error {
    let panicked = error.chain().any(|cause| {
        cause
            .downcast_ref::<tokio::task::JoinError>()
            .is_some_and(tokio::task::JoinError::is_panic)
    });
    if panicked {
        anyhow::anyhow!(crate::logs::scrubbed(&format!("{error:#}")))
    } else {
        error
    }
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
