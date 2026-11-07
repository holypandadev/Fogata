//! Files rewritten whole whenever friends or members change something: the friend book (with
//! every friend's picture and banner) and the members' profiles. A friend or member can send
//! changes as often as it likes, so a burst of them is written once: the first change at
//! once, and the rest together a moment later, and whatever waits when the application quits.

use std::time::{Duration, Instant};

/// A file of this kind is written at most this often.
pub(crate) const SAVE_GAP: Duration = Duration::from_secs(3);

/// When a file was last written, and whether a write waits.
#[derive(Debug, Default)]
pub(crate) struct Saves {
    last: Option<Instant>,
    waiting: bool,
}

/// When to write after a change.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum When {
    Now,
    /// In this long, with whatever has changed by then.
    In(Duration),
    /// A write already waits, and will take this change too.
    Waiting,
}

impl Saves {
    /// Something changed at `now`: when to write it.
    pub(crate) fn changed(&mut self, now: Instant) -> When {
        if self.waiting {
            return When::Waiting;
        }
        match self.last.map(|last| now.saturating_duration_since(last)) {
            Some(since) if since < SAVE_GAP => {
                self.waiting = true;
                When::In(SAVE_GAP - since)
            }
            _ => {
                self.last = Some(now);
                When::Now
            }
        }
    }

    /// The write that waited is made at `now`; returns whether one waited.
    pub(crate) fn take_waiting(&mut self, now: Instant) -> bool {
        let waited = std::mem::take(&mut self.waiting);
        if waited {
            self.last = Some(now);
        }
        waited
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_of_changes_is_written_twice() {
        let mut saves = Saves::default();
        let start = Instant::now();
        assert_eq!(saves.changed(start), When::Now);
        let soon = start + Duration::from_secs(1);
        assert_eq!(saves.changed(soon), When::In(Duration::from_secs(2)));
        for n in 0..100 {
            assert_eq!(
                saves.changed(soon + Duration::from_millis(n)),
                When::Waiting
            );
        }
        let later = start + SAVE_GAP;
        assert!(saves.take_waiting(later));
        assert!(!saves.take_waiting(later));
        // Changes keep coming: the next waits for the gap again.
        assert_eq!(
            saves.changed(later + Duration::from_secs(1)),
            When::In(Duration::from_secs(2))
        );
        // A change long after the last write goes at once.
        let mut saves = Saves::default();
        assert_eq!(saves.changed(start), When::Now);
        assert_eq!(saves.changed(start + SAVE_GAP), When::Now);
    }
}
