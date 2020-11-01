#![allow(dead_code)] // suppress warning when doing selective testing
use r3::utils::Init;
use core::sync::atomic::{AtomicUsize, Ordering};

pub(crate) mod benchmark;
pub(crate) mod compute;
mod sort;
pub(crate) mod time;
mod trig;

/// An atomic counter for checking an execution sequence.
pub(crate) struct SeqTracker {
    counter: AtomicUsize,
}

impl Init for SeqTracker {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();
}

impl SeqTracker {
    /// Construct `SeqTracker`.
    pub(crate) const fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }

    pub(crate) fn get(&self) -> usize {
        self.counter.load(Ordering::Relaxed)
    }

    /// Assert that the counter is equal to `old` and then replace it with
    /// `new`.
    #[track_caller]
    pub(crate) fn expect_and_replace(&self, old: usize, new: usize) {
        // Note: Some targets don't support CAS atomics
        let got = self.counter.load(Ordering::Relaxed);
        log::debug!("{} (expected: {}) → {}", got, old, new);
        assert_eq!(got, old, "expected {}, got {}", old, got);
        self.counter.store(new, Ordering::Relaxed);
    }
}
