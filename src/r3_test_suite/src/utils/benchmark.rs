//! The benchmark framework that runs on R3.
use arrayvec::ArrayVec;
use core::{cell::UnsafeCell, fmt};
use r3::{
    hunk::Hunk,
    kernel::{raw_cfg, traits, Cfg, StaticTask},
    utils::ConstDefault,
};

use crate::utils::sort::insertion_sort;

/// Identifies a measured interval.
pub type Interval = &'static str;

/// Implemented for all system types implementing the necessary traits for
/// the bencher.
pub trait SupportedSystem: traits::KernelBase + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelStatic> SupportedSystem for T {}

/// The options for the bencher.
///
/// # Safety
///
///  - Implementing this trait causes [`Bencher`] to be implemented on the same
///    type. The application code should not call any of `Bencher`'s methods
///    outside the duration of a call to [`Self::iter`].
///
///  - `Bencher`'s methods access a global object without synchronization. The
///    application code should ensure no data race occurs.
///
pub unsafe trait BencherOptions<System: SupportedSystem> {
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
pub trait Bencher<System: SupportedSystem> {
    fn mark_start();
    fn mark_end(int: Interval);
    fn main_task() -> StaticTask<System>;
}

/// The cottage object of the bencher. Created by [`configure`].
pub struct BencherCottage<System: SupportedSystem> {
    task: StaticTask<System>,
    state: Hunk<System, BencherState>,
}

struct BencherState(UnsafeCell<BencherStateInner>);
struct BencherStateInner {
    mark: u32,
    intervals: ArrayVec<IntervalRecord, 8>,
}

unsafe impl Sync for BencherState {}

struct IntervalRecord {
    name: Interval,
    samples: ArrayVec<u32, 45>,
}

impl ConstDefault for BencherState {
    #[allow(clippy::declare_interior_mutable_const)]
    const DEFAULT: Self = Self(UnsafeCell::new(BencherStateInner {
        mark: 0,
        intervals: ArrayVec::new_const(),
    }));
}

pub const fn configure<C, System: SupportedSystem, Options: BencherOptions<System>>(
    b: &mut Cfg<C>,
) -> BencherCottage<System>
where
    C: ~const raw_cfg::CfgBase<System = System> + ~const raw_cfg::CfgTask,
{
    let task = StaticTask::define()
        .start(main_task::<System, Options>)
        .active(true)
        .priority(3)
        .finish(b);

    let state = Hunk::<System, BencherState>::define().finish(b);

    BencherCottage { task, state }
}

impl<System: SupportedSystem, Options: BencherOptions<System>> Bencher<System> for Options {
    #[inline(never)]
    fn mark_start() {
        let state = unsafe { &mut *Self::cottage().state.0.get() };
        state.mark = Options::performance_time();
    }

    #[inline(never)]
    fn mark_end(name: Interval) {
        let state = unsafe { &mut *Self::cottage().state.0.get() };
        let delta = Options::performance_time().wrapping_sub(state.mark);

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
                samples: ArrayVec::new(),
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

    #[inline]
    fn main_task() -> StaticTask<System> {
        Options::cottage().task
    }
}

fn main_task<System: SupportedSystem, Options: BencherOptions<System>>(_: usize) {
    while {
        Options::mark_start();
        Options::mark_end("(empty)");

        Options::iter();

        let state = unsafe { &mut *Options::cottage().state.0.get() };

        // If there's no custom intervals defined at this point, it's a usage
        // error.
        if state.intervals.len() <= 1 {
            panic!("`mark_end` has never been called during the iteration");
        }

        // Repeat until all instances of `IntervalRecord::samples` are full.
        !state.intervals.iter().all(|i| i.samples.is_full())
    } {}

    // Report the result
    {
        let state = unsafe { &mut *Options::cottage().state.0.get() };
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
