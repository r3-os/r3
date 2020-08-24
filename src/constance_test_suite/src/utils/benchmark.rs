//! The benchmark framework that runs on Constance.
use constance::{
    kernel::{cfg::CfgBuilder, Hunk, Kernel, Task},
    utils::Init,
};
use core::fmt;
use staticvec::StaticVec;
use try_lock::TryLock;

use crate::utils::sort::insertion_sort;

/// Identifies a measured interval.
pub type Interval = &'static str;

pub trait BencherOptions<System> {
    fn performance_time() -> u32;

    const PERFORMANCE_TIME_UNIT: &'static str;

    /// Get a reference to the associated [`BencherCottage`].
    fn cottage() -> &'static BencherCottage<System>;

    /// Execute a single benchmark iteration.
    ///
    /// The bencher calls this method for one or more times from its main taks.
    fn iter();

    /// Signal the completion of a benchmark run.
    fn finish();
}

/// The API to be used by a measured program. Automatically implemented on every
/// `T: `[`BencherOptions`]`<System>`.
pub trait Bencher<System> {
    fn mark_start();
    fn mark_end(int: Interval);
}

/// The cottage object of the bencher. Created by [`configure`].
pub struct BencherCottage<System> {
    task: Task<System>,
    state: Hunk<System, BencherState>,
}

struct BencherState(TryLock<BencherStateInner>);
struct BencherStateInner {
    mark: u32,
    intervals: StaticVec<IntervalRecord, 8>,
}

struct IntervalRecord {
    name: Interval,
    samples: StaticVec<u32, 45>,
}

impl Init for BencherState {
    const INIT: Self = Self(TryLock::new(BencherStateInner {
        mark: 0,
        intervals: StaticVec::new(),
    }));
}

pub const fn configure<System: Kernel, Options: BencherOptions<System>>(
    b: &mut CfgBuilder<System>,
) -> BencherCottage<System> {
    let task = Task::build()
        .start(main_task::<System, Options>)
        .active(true)
        .priority(7)
        .finish(b);

    let state = Hunk::<System, BencherState>::build().finish(b);

    BencherCottage { task, state }
}

impl<System: Kernel, Options: BencherOptions<System>> Bencher<System> for Options {
    #[inline(never)]
    fn mark_start() {
        let mut state = Self::cottage().state.0.try_lock().unwrap();
        state.mark = Options::performance_time();
    }

    #[inline(never)]
    fn mark_end(name: Interval) {
        let mut state = Self::cottage().state.0.try_lock().unwrap();
        let state = &mut *state;
        let delta = Options::performance_time() - state.mark;

        // Find the `IntervalRecord` for `int`. If there's none, create one
        let interval = if let Some(x) = state
            .intervals
            .iter_mut()
            .find(|interval| interval.name == name)
        {
            x
        } else if state
            .intervals
            .try_push(IntervalRecord {
                name,
                samples: StaticVec::new(),
            })
            .is_ok()
        {
            state.intervals.last_mut().unwrap()
        } else {
            panic!("too many unique measurement intervals");
        };

        // Record the measured duration. Drop any excessive samples.
        let _ = interval.samples.try_push(delta);
    }
}

fn main_task<System: Kernel, Options: BencherOptions<System>>(_: usize) {
    while {
        Options::mark_start();
        Options::mark_end("(empty)");

        Options::iter();

        let state = Options::cottage().state.0.try_lock().unwrap();

        // If there's no custom intervals defined at this point, it's a usage
        // error.
        if state.intervals.len() <= 1 {
            panic!("`mark_end` has never been called during the iteration");
        }

        // Repeat until all instances of `IntervalRecord::samples` are full.
        state.intervals.iter().any(|i| i.samples.is_not_full())
    } {}

    // Report the result
    {
        let mut state = Options::cottage().state.0.try_lock().unwrap();
        for interval in state.intervals.iter_mut() {
            assert!(interval.samples.is_full());

            // Discard first few samples
            let samples = &mut interval.samples[4..45];
            assert_eq!(samples.len(), 41);

            // Sort the samples. Use insertion sort to save code size.
            insertion_sort(samples);

            // Extract percentiles
            let percentiles = [
                samples[0],  // 0%
                samples[4],  // 10%
                samples[20], // 50%
                samples[36], // 90%
                samples[40], // 100%
            ];

            // Calculate the mean
            let sum: u32 = samples.iter().sum();
            let mean = FixedPoint2(sum * 100 / samples.len() as u32);

            log::warn!(
                "{}... mean = {}, med = {} [{}]",
                interval.name,
                mean,
                percentiles[2],
                Options::PERFORMANCE_TIME_UNIT,
            );

            log::info!(
                "  (0/10/50/90/100th percentiles: {} ─ {} ═ {} ═ {} ─ {})",
                percentiles[0],
                percentiles[1],
                percentiles[2],
                percentiles[3],
                percentiles[4],
            );
        }
    }

    Options::finish();
}

/// A fixed-point number with two fractional digits.
struct FixedPoint2(u32);

impl fmt::Display for FixedPoint2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}
