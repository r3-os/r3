#![allow(dead_code)] // suppress warning when doing selective testing
use constance::utils::Init;
use core::sync::atomic::{AtomicUsize, Ordering};

pub(crate) mod compute;
mod trig;

/// An atomic counter for checking an execution sequence.
pub(crate) struct SeqTracker {
    counter: AtomicUsize,
}

impl Init for SeqTracker {
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
        log::debug!(
            "{} (expected: {}) → {}",
            self.counter.load(Ordering::Relaxed),
            old,
            new
        );
        let got = self.counter.compare_and_swap(old, new, Ordering::Relaxed);
        assert_eq!(got, old, "expected {}, got {}", old, got);
    }
}
