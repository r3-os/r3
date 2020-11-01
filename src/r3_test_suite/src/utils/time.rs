use r3::{
    kernel::Kernel,
    time::{Duration, Time},
};
use core::ops::Range;

/// Extension methods for [`Kernel`].
pub trait KernelTimeExt: Kernel {
    /// Get the current time in milliseconds. Panics if the result does not fit
    /// in `u32`.
    #[inline]
    #[track_caller]
    #[cfg(feature = "system_time")]
    fn time_ms() -> u32 {
        use core::convert::TryInto;
        Self::time().unwrap().as_millis().try_into().unwrap()
    }

    #[inline]
    #[track_caller]
    fn set_time_ms(x: u32) {
        Self::set_time(Time::from_millis(x as _)).unwrap();
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

    #[inline]
    #[cfg(not(feature = "system_time"))]
    fn assert_time_ms_range(_range: Range<u32>) {}

    #[inline]
    #[track_caller]
    fn sleep_ms(x: u32) {
        Self::sleep(Duration::from_millis(x as _)).unwrap();
    }
}

impl<T: Kernel> KernelTimeExt for T {}
