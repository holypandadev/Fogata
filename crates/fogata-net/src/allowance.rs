//! How much this device uploads to each friend or member in a day: what it serves (files
//! shared in a room, release packages) can be asked for again and again, and each time uses
//! the upload of the home's internet line.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::locked;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);
/// Askers whose day is remembered; past it, those whose day is over are forgotten.
const KEPT: usize = 4096;

/// Bytes each asker may be sent in a day, counted from its first byte that day.
#[derive(Debug)]
pub(crate) struct Allowance<K> {
    per_day: u64,
    used: Mutex<HashMap<K, (Instant, u64)>>,
}

impl<K: Eq + Hash> Allowance<K> {
    pub(crate) fn new(per_day: u64) -> Self {
        Self {
            per_day,
            used: Mutex::new(HashMap::new()),
        }
    }

    /// Whether `bytes` more may be sent to `asker` now; if so, they are counted.
    pub(crate) fn take(&self, asker: K, bytes: u64) -> bool {
        self.take_at(asker, bytes, Instant::now())
    }

    /// Whether `bytes` more may be sent to `asker` now, counting nothing: a transfer that
    /// would be cut off on its way is not started.
    pub(crate) fn fits(&self, asker: &K, bytes: u64) -> bool {
        self.fits_at(asker, bytes, Instant::now())
    }

    fn fits_at(&self, asker: &K, bytes: u64, now: Instant) -> bool {
        let used = locked(&self.used);
        let sent = used
            .get(asker)
            .filter(|(since, _)| now.saturating_duration_since(*since) < DAY)
            .map_or(0, |(_, sent)| *sent);
        sent.saturating_add(bytes) <= self.per_day
    }

    fn take_at(&self, asker: K, bytes: u64, now: Instant) -> bool {
        let mut used = locked(&self.used);
        if used.len() >= KEPT {
            used.retain(|_, (since, _)| now.saturating_duration_since(*since) < DAY);
        }
        let (since, sent) = used.entry(asker).or_insert((now, 0));
        if now.saturating_duration_since(*since) >= DAY {
            (*since, *sent) = (now, 0);
        }
        if *sent + bytes > self.per_day {
            return false;
        }
        *sent += bytes;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_asker_gets_its_day_and_then_waits_for_the_next() {
        let allowance = Allowance::new(1000);
        let start = Instant::now();
        assert!(allowance.take_at(1, 600, start));
        assert!(!allowance.take_at(1, 600, start));
        assert!(allowance.take_at(1, 400, start));
        // Another asker has its own.
        assert!(allowance.take_at(2, 1000, start));
        assert!(allowance.take_at(1, 1000, start + DAY));
    }

    /// A transfer is started only when all of it fits in the day: then each of its parts
    /// is taken, and none is refused on its way.
    #[test]
    fn a_transfer_is_started_only_when_it_fits_whole() {
        let allowance = Allowance::new(1000);
        let start = Instant::now();
        assert!(allowance.take_at(1, 300, start));
        assert!(!allowance.fits_at(&1, 800, start));
        assert!(allowance.fits_at(&1, 700, start));
        assert!((0..7).all(|_| allowance.take_at(1, 100, start)));
        assert!(allowance.fits_at(&1, 800, start + DAY));
    }
}
