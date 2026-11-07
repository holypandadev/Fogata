//! Files written off the event pump. One thread writes them, in the order they were asked
//! for, each the same way as before (a temporary file, synced, renamed over the old one), so
//! a slow disk delays the files, not the events. A write of a file that waits for its turn
//! is replaced by a newer write of the same file, unless something asked for in between must
//! come after it: a burst of changes to one file is written once, with the newest content.

use std::{
    collections::VecDeque,
    panic::AssertUnwindSafe,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::JoinHandle,
};

use fogata_crypto::GroupId;

/// A file written whole, of which only the newest content matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum File {
    Meta,
    Group(GroupId),
    Friends,
    Profiles,
    Animations,
}

type Job = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct Queue {
    /// Writes in the order they go, each with the file it rewrites whole (`None` for one that
    /// does something else, which nothing asked for after it may pass).
    jobs: VecDeque<(Option<File>, Job)>,
    /// A write is being made.
    busy: bool,
    /// No more writes come: the thread ends once the queue is empty.
    closing: bool,
}

#[derive(Default)]
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, guard: MutexGuard<'a, Queue>) -> MutexGuard<'a, Queue> {
        self.changed
            .wait(guard)
            .unwrap_or_else(PoisonError::into_inner)
    }
}

pub(crate) struct Writer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    pub(crate) fn start() -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let thread = std::thread::Builder::new()
            .name("fogata-writer".into())
            .spawn({
                let shared = shared.clone();
                move || write_all(&shared)
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// Rewrites `file` in turn with `write`, in place of a write of the same file that waits
    /// (see the module).
    pub(crate) fn soon(&self, file: File, write: impl FnOnce() + Send + 'static) {
        let mut queue = self.shared.lock();
        for (waiting, job) in queue.jobs.iter_mut().rev() {
            match waiting {
                Some(other) if *other == file => {
                    *job = Box::new(write);
                    return;
                }
                Some(_) => {}
                None => break,
            }
        }
        queue.jobs.push_back((Some(file), Box::new(write)));
        self.shared.changed.notify_all();
    }

    /// Runs `job` once everything asked for before it is written; what is returned waits
    /// for it. The job is asked for at once, not when that is first waited for.
    pub(crate) fn then<T, F>(&self, job: F) -> impl Future<Output = anyhow::Result<T>> + use<T, F>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let (done, result) = tokio::sync::oneshot::channel();
        {
            let mut queue = self.shared.lock();
            queue.jobs.push_back((
                None,
                Box::new(move || {
                    let _ = done.send(job());
                }),
            ));
            self.shared.changed.notify_all();
        }
        async move {
            result
                .await
                .map_err(|_| anyhow::anyhow!("the files could not be written"))
        }
    }

    /// Runs `job` once everything asked for before it is written, without waiting.
    pub(crate) fn after(&self, job: impl FnOnce() + Send + 'static) {
        let mut queue = self.shared.lock();
        queue.jobs.push_back((None, Box::new(job)));
        self.shared.changed.notify_all();
    }
}

/// What is asked for is written before the writer goes.
impl Drop for Writer {
    fn drop(&mut self) {
        self.shared.lock().closing = true;
        self.shared.changed.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn write_all(shared: &Shared) {
    let mut queue = shared.lock();
    loop {
        let Some((_, job)) = queue.jobs.pop_front() else {
            if queue.closing {
                return;
            }
            queue = shared.wait(queue);
            continue;
        };
        queue.busy = true;
        drop(queue);
        // A write that panics loses itself only: the ones after it are still made.
        if std::panic::catch_unwind(AssertUnwindSafe(job)).is_err() {
            tracing::error!("a file could not be written");
        }
        queue = shared.lock();
        queue.busy = false;
        shared.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_go_in_order_and_a_waiting_one_takes_the_newest_content() {
        let writer = Writer::start().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (hold, held) = std::sync::mpsc::channel::<()>();
        // The first write holds the thread while the others are asked for.
        writer.after(move || {
            let _ = held.recv();
        });
        let note = |what: &'static str| {
            let seen = seen.clone();
            move || seen.lock().unwrap().push(what)
        };
        writer.soon(File::Meta, note("meta 1"));
        writer.soon(File::Friends, note("friends 1"));
        writer.soon(File::Meta, note("meta 2"));
        writer.after(note("after"));
        // Past something that must come first, a write of the same file goes again.
        writer.soon(File::Meta, note("meta 3"));
        writer.soon(File::Friends, note("friends 2"));
        hold.send(()).unwrap();
        let last = writer.then(note("last")).await;
        assert!(last.is_ok());
        assert_eq!(
            *seen.lock().unwrap(),
            [
                "meta 2",
                "friends 1",
                "after",
                "meta 3",
                "friends 2",
                "last"
            ]
        );
    }

    #[test]
    fn what_was_asked_for_is_written_before_the_writer_goes() {
        let seen = Arc::new(Mutex::new(0));
        {
            let writer = Writer::start().unwrap();
            for _ in 0..50 {
                let seen = seen.clone();
                writer.after(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    *seen.lock().unwrap() += 1;
                });
            }
        }
        assert_eq!(*seen.lock().unwrap(), 50);
    }
}
