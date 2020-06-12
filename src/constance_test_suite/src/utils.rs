use constance::utils::Init;
use core::sync::atomic::{AtomicUsize, Ordering};

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

    /// Assert that the counter is equal to `old` and then replace it with
    /// `new`.
    pub(crate) fn expect_and_replace(&self, old: usize, new: usize) {
        assert_eq!(
            self.counter.compare_and_swap(old, new, Ordering::Relaxed),
            old
        );
    }
}
