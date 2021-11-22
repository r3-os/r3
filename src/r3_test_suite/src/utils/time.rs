use core::ops::Range;
use r3::{
    kernel::traits,
    time::{Duration, Time},
};

/// Extension methods for system types.
pub trait KernelTimeExt: traits::KernelBase {
    /// Get the current time in milliseconds. Panics if the result does not fit
    /// in `u32`.
    #[track_caller]
    #[cfg(feature = "system_time")]
    fn time_ms() -> u32;

    #[inline]
    #[track_caller]
    fn set_time_ms(x: u32) {
        Self::set_time(Time::from_millis(x as _)).unwrap();
    }

    #[inline]
    fn assert_time_ms_range(_range: Range<u32>) {}

    #[inline]
    #[track_caller]
    fn sleep_ms(x: u32) {
        Self::sleep(Duration::from_millis(x as _)).unwrap();
    }
}

#[cfg(not(feature = "system_time"))]
impl<T: traits::KernelBase> KernelTimeExt for T {}

#[cfg(feature = "system_time")]
impl<T: traits::KernelBase + traits::KernelTime> KernelTimeExt for T {
    #[inline]
    #[track_caller]
    fn time_ms() -> u32 {
        Self::time().unwrap().as_millis().try_into().unwrap()
    }

    #[inline]
    #[track_caller]
    #[cfg(feature = "system_time")]
    fn assert_time_ms_range(range: Range<u32>) {
        let t = Self::time_ms();
        log::trace!("time = {:?}ms (expected = {:?}ms)", t, range);
        assert!(
            range.contains(&t),
            "time = {:?}ms (expected = {:?}ms)",
            t,
            range
        );
    }
}
