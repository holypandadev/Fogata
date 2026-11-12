//! Uploads that a slow reader would hold: a device that takes one chunk now and then keeps
//! its place among the few uploads served at once for as long as it likes, while others
//! wait. Both servers that send files to other devices (files.rs, updates.rs) end such an
//! upload, but only while another device waits for a place ([`Waiting`]) and only once its
//! reader took less than a least rate over a whole window ([`Reader`]): a slow line well
//! above that rate never loses its place, and one ended goes on from what it got when it
//! asks again.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::locked;

/// How fast the reader of one upload takes what it is sent: the bytes it took over the
/// time the upload waited for it to (writes that did not complete at once), in windows of
/// a set length. Time the upload spends on anything else (reading the disk, keeping to a
/// pace of its own) is not the reader's, and does not count.
#[derive(Debug)]
pub(crate) struct Reader {
    window: Duration,
    waited: Duration,
    bytes: u64,
    slow: bool,
}

impl Reader {
    pub(crate) fn new(window: Duration) -> Self {
        Self {
            window,
            waited: Duration::ZERO,
            bytes: 0,
            slow: false,
        }
    }

    /// Counts `bytes` the reader took after the upload waited `waited` for it to. Once the
    /// window is full, says whether the reader took less than `least` bytes a second over
    /// it, and starts the next: a reader that is fast at first and then trickles is caught
    /// in a later window, and a short stall (a dish losing the sky for a few seconds) does
    /// not make a whole window slow.
    pub(crate) fn took(&mut self, bytes: u64, waited: Duration, least: u64) {
        self.waited += waited;
        self.bytes = self.bytes.saturating_add(bytes);
        if self.waited >= self.window {
            self.slow = (self.bytes as f64) < least as f64 * self.waited.as_secs_f64();
            self.waited = Duration::ZERO;
            self.bytes = 0;
        }
    }

    /// Whether the reader took less than the least rate over the last whole window.
    pub(crate) fn too_slow(&self) -> bool {
        self.slow
    }
}

/// Devices kept as waiting at most; one more is not noted, and those noted still count.
const WAITING_KEPT: usize = 256;

/// The devices that waited for a place among the uploads lately (for a while after), by
/// device: a slow upload is ended only for another device (see the module's notes).
#[derive(Debug)]
pub(crate) struct Waiting<K> {
    since: Mutex<HashMap<K, Instant>>,
    lasts: Duration,
}

impl<K: Copy + Eq + Hash> Waiting<K> {
    /// Noted devices count as waiting for `lasts`.
    pub(crate) fn new(lasts: Duration) -> Self {
        Self {
            since: Mutex::new(HashMap::new()),
            lasts,
        }
    }

    /// `who` waits for a place now.
    pub(crate) fn note(&self, who: K) {
        let now = Instant::now();
        let mut since = locked(&self.since);
        if since.len() >= WAITING_KEPT && !since.contains_key(&who) {
            since.retain(|_, at| now.duration_since(*at) < self.lasts);
            if since.len() >= WAITING_KEPT {
                return;
            }
        }
        since.insert(who, now);
    }

    /// Whether a device other than `who` waited for a place lately.
    pub(crate) fn besides(&self, who: &K) -> bool {
        locked(&self.since)
            .iter()
            .any(|(other, at)| other != who && at.elapsed() < self.lasts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    /// A reader is too slow only once a whole window went at less than the least rate: one
    /// at that rate or above never is, one that takes a chunk every half minute is, and one
    /// fast at first that then trickles is in the next window; a fast window clears it.
    #[test]
    fn a_reader_is_too_slow_only_over_a_whole_window_under_the_least_rate() {
        let least = 4 * 1024;
        let window = Duration::from_secs(120);
        // At the least rate, second by second, for an hour.
        let mut steady = Reader::new(window);
        for _ in 0..3600 {
            steady.took(least, SECOND, least);
            assert!(!steady.too_slow());
        }
        // A chunk every 29 seconds: slow once the window is full, not before.
        let mut trickle = Reader::new(window);
        for i in 1..=5u32 {
            trickle.took(64 * 1024, 29 * SECOND, least);
            assert_eq!(trickle.too_slow(), i * 29 >= 120, "after {i} chunks");
        }
        // Fast for a while, then a trickle: caught in the window after.
        let mut stalls = Reader::new(window);
        stalls.took(512 << 20, 119 * SECOND, least);
        stalls.took(64 * 1024, SECOND, least);
        assert!(!stalls.too_slow());
        for _ in 0..5 {
            stalls.took(64 * 1024, 29 * SECOND, least);
        }
        assert!(stalls.too_slow());
        // Fast again: no longer.
        stalls.took(16 << 20, 120 * SECOND, least);
        assert!(!stalls.too_slow());
        // A dish losing the sky for 45 seconds in a window at 0.1 Mbit/s: not slow.
        let mut dish = Reader::new(window);
        dish.took(12_500 * 75, 75 * SECOND, least);
        dish.took(0, 45 * SECOND, least);
        assert!(!dish.too_slow());
    }

    /// Another device waiting counts for a while; the device itself does not.
    #[test]
    fn only_another_device_waiting_counts() {
        let waiting = Waiting::new(Duration::from_millis(200));
        assert!(!waiting.besides(&1));
        waiting.note(1);
        assert!(!waiting.besides(&1));
        assert!(waiting.besides(&2));
        std::thread::sleep(Duration::from_millis(250));
        assert!(!waiting.besides(&2));
        // A crowd of devices noted at once is bounded, and still counts.
        for who in 0..2 * WAITING_KEPT {
            waiting.note(who);
        }
        assert!(locked(&waiting.since).len() <= WAITING_KEPT);
        assert!(waiting.besides(&usize::MAX));
    }
}
