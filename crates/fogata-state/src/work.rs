//! Counts of the work the group's views do, for tests to check how much of it a view costs
//! (a walk of the log, a copy of the group) rather than how long it takes, which a busy
//! computer changes. Per thread, as each test runs on its own. Only in tests, and with the
//! `counts` feature for the tests of the crates that use this one.

use std::cell::Cell;

thread_local! {
    static APPLIED: Cell<usize> = const { Cell::new(0) };
    static COPIED: Cell<usize> = const { Cell::new(0) };
    static SETTLED: Cell<usize> = const { Cell::new(0) };
    static VIEWED: Cell<usize> = const { Cell::new(0) };
    static VERIFIED: Cell<usize> = const { Cell::new(0) };
    static NAMED: Cell<usize> = const { Cell::new(0) };
}

/// What was counted on this thread since [`Work::start`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Work {
    /// Entries tried on a group, one at a time.
    pub applied: usize,
    /// Copies of a group made to try an entry on.
    pub copied: usize,
    /// Groups worked out from the whole log.
    pub settled: usize,
    /// Groups worked out from an entry's parents, to judge it.
    pub viewed: usize,
    /// Signatures checked.
    pub verified: usize,
    /// Named invites read from their bytes, keys checked (see `NamedInvite::of`).
    pub named: usize,
}

impl Work {
    /// Counts from nothing.
    pub fn start() {
        for count in [&APPLIED, &COPIED, &SETTLED, &VIEWED, &VERIFIED, &NAMED] {
            count.set(0);
        }
    }

    /// What was counted since [`Work::start`].
    pub fn done() -> Work {
        Work {
            applied: APPLIED.get(),
            copied: COPIED.get(),
            settled: SETTLED.get(),
            viewed: VIEWED.get(),
            verified: VERIFIED.get(),
            named: NAMED.get(),
        }
    }
}

pub(crate) fn applied() {
    APPLIED.set(APPLIED.get() + 1);
}

pub(crate) fn copied() {
    COPIED.set(COPIED.get() + 1);
}

pub(crate) fn settled() {
    SETTLED.set(SETTLED.get() + 1);
}

pub(crate) fn viewed() {
    VIEWED.set(VIEWED.get() + 1);
}

pub(crate) fn verified() {
    VERIFIED.set(VERIFIED.get() + 1);
}

pub(crate) fn named() {
    NAMED.set(NAMED.get() + 1);
}
