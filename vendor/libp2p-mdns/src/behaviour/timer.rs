// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::time::{Duration, Instant};

/// Simple wrapper for the different type of timers
#[derive(Debug)]
#[cfg(feature = "tokio")]
pub struct Timer<T> {
    inner: T,
}

/// Builder interface to homogenize the different implementations
#[allow(unreachable_pub)] // Users should not depend on this.
pub trait Builder: Send + Unpin + 'static {
    /// Creates a timer that emits an event once at the given time instant.
    fn at(instant: Instant) -> Self;

    /// Creates a timer that emits events periodically.
    fn interval(duration: Duration) -> Self;

    /// Creates a timer that emits events periodically, starting at start.
    fn interval_at(start: Instant, duration: Duration) -> Self;
}

#[cfg(feature = "tokio")]
pub(crate) mod tokio {
    use std::{
        pin::Pin,
        task::{Context, Poll},
    };

    use ::tokio::time::{self, Instant as TokioInstant, Interval, MissedTickBehavior};
    use futures::Stream;

    use super::*;

    /// Tokio wrapper
    pub(crate) type TokioTimer = Timer<Interval>;

    /// The period of a timer meant to fire once: a year, far enough never to come, and small
    /// enough to add to any instant.
    const ONE_SHOT_PERIOD: Duration = Duration::from_secs(365 * 24 * 60 * 60);
    impl Builder for TokioTimer {
        fn at(instant: Instant) -> Self {
            // Fires once at `instant`; the behaviour makes a new timer each time it fires. The
            // period only has to be long, not endless: polled more than 5 ms after `instant`
            // (a busy computer, one back from sleep), tokio adds it to the current time, and
            // the near-endless period of the original overflowed that addition and panicked.
            let mut inner = time::interval_at(TokioInstant::from_std(instant), ONE_SHOT_PERIOD);
            inner.set_missed_tick_behavior(MissedTickBehavior::Skip);
            Self { inner }
        }

        fn interval(duration: Duration) -> Self {
            let mut inner = time::interval_at(TokioInstant::now() + duration, duration);
            inner.set_missed_tick_behavior(MissedTickBehavior::Skip);
            Self { inner }
        }

        fn interval_at(start: Instant, duration: Duration) -> Self {
            let mut inner = time::interval_at(TokioInstant::from_std(start), duration);
            inner.set_missed_tick_behavior(MissedTickBehavior::Skip);
            Self { inner }
        }
    }

    impl Stream for TokioTimer {
        type Item = TokioInstant;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.inner.poll_tick(cx).map(Some)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (usize::MAX, None)
        }
    }
}

#[cfg(all(test, feature = "tokio"))]
mod tests {
    use std::time::{Duration, Instant};

    use futures::StreamExt;

    use super::{Builder, tokio::TokioTimer};

    /// Polled well after the instant it was set for, as on a busy computer, a one-shot timer
    /// fires instead of panicking.
    #[test]
    fn a_timer_polled_late_fires() {
        let runtime = ::tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut timer = TokioTimer::at(Instant::now() + Duration::from_millis(1));
            std::thread::sleep(Duration::from_millis(30));
            let fired = ::tokio::time::timeout(Duration::from_secs(5), timer.next()).await;
            assert!(matches!(fired, Ok(Some(_))));
        });
    }
}
