use core::ops::Range;
use r3::{
    kernel::{prelude::*, traits, TimeError},
    time::{Duration, Time},
};

/// Indicates a Boost Priority capability.
///
/// This token is returned by
/// [`KernelBoostPriorityExt::BOOST_PRIORITY_CAPABILITY`]. You can also create
/// this directly, but [`KernelBoostPriorityExt::boost_priority`] will panic if
/// Boost Priority isn't actually supported.
#[derive(Debug, Clone, Copy)]
pub struct BoostPriorityCapability;

/// Extends system types to add `boost_priority` unconditionally. Whether
/// `boost_priority` is actually supported is controlled by the `priority_boost`
/// feature.
pub trait KernelBoostPriorityExt: traits::KernelBase {
    /// Indicates whether Priority Boost is supported.
    const BOOST_PRIORITY_CAPABILITY: Option<BoostPriorityCapability>;

    /// Enable Priority Boost if it's supported. Panic otherwise.
    #[track_caller]
    fn boost_priority(cap: BoostPriorityCapability) -> Result<(), r3::kernel::BoostPriorityError>;
}

#[cfg(not(feature = "priority_boost"))]
impl<T: traits::KernelBase> KernelBoostPriorityExt for T {
    const BOOST_PRIORITY_CAPABILITY: Option<BoostPriorityCapability> = None;

    #[inline]
    #[track_caller]
    fn boost_priority(_: BoostPriorityCapability) -> Result<(), r3::kernel::BoostPriorityError> {
        unreachable!("Priority Boost is not supported")
    }
}

#[cfg(feature = "priority_boost")]
impl<T: traits::KernelBase + traits::KernelBoostPriority> KernelBoostPriorityExt for T {
    const BOOST_PRIORITY_CAPABILITY: Option<BoostPriorityCapability> =
        Some(BoostPriorityCapability);

    #[inline]
    #[track_caller]
    fn boost_priority(_: BoostPriorityCapability) -> Result<(), r3::kernel::BoostPriorityError> {
        <Self as Kernel>::boost_priority()
    }
}

/// Indicates a system time tracking capability.
///
/// This token is returned by
/// [`KernelTimeExt::TIME_CAPABILITY`]. You can also create
/// this directly, but [`KernelTimeExt::time`] will panic if
/// Boost Priority isn't actually supported.
#[derive(Debug, Clone, Copy)]
pub struct TimeCapability;

/// Extension methods for system types.
pub trait KernelTimeExt: traits::KernelBase {
    /// Indicates whether Priority Boost is supported.
    const TIME_CAPABILITY: Option<TimeCapability>;

    /// Get the current time. Panics if it's not supported.
    fn time(cap: TimeCapability) -> Result<Time, TimeError>;

    /// Get the current time in milliseconds. Panics if the result does not fit
    /// in `u32`.
    #[track_caller]
    fn time_ms(cap: TimeCapability) -> u32 {
        Self::time(cap).unwrap().as_millis().try_into().unwrap()
    }

    #[inline]
    #[track_caller]
    fn set_time_ms(x: u32) {
        Self::set_time(Time::from_millis(x as _)).unwrap();
    }

    #[inline]
    #[track_caller]
    fn assert_time_ms_range(range: Range<u32>) {
        if let Some(cap) = Self::TIME_CAPABILITY {
            let t = Self::time_ms(cap);
            log::trace!("time = {:?}ms (expected = {:?}ms)", t, range);
            assert!(
                range.contains(&t),
                "time = {:?}ms (expected = {:?}ms)",
                t,
                range
            );
        }
    }

    #[inline]
    #[track_caller]
    fn sleep_ms(x: u32) {
        <Self as Kernel>::sleep(Duration::from_millis(x as _)).unwrap();
    }
}

#[cfg(not(feature = "system_time"))]
impl<T: traits::KernelBase> KernelTimeExt for T {
    const TIME_CAPABILITY: Option<TimeCapability> = None;

    fn time(_: TimeCapability) -> Result<Time, TimeError> {
        unreachable!("`time` is not supported")
    }
}

#[cfg(feature = "system_time")]
impl<T: traits::KernelBase + traits::KernelTime> KernelTimeExt for T {
    const TIME_CAPABILITY: Option<TimeCapability> = Some(TimeCapability);

    fn time(_: TimeCapability) -> Result<Time, TimeError> {
        <Self as traits::Kernel>::time()
    }
}
