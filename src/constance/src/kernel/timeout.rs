//! Manages timeouts (timed events) and the system clock.
//!
//! # Absolute Time Values
//!
//! There are two kinds of absolute time values used by this system.
//!
//! **A system time** corresponds to the value of [`Kernel::time`]. This is
//! affected by both of [`Kernel::set_time`] and [`Kernel::adjust_time`].
//!
//! On the other hand, **an event time** is only affected by
//! [`Kernel::adjust_time`]. *Time* usually refers to this kind of time unless
//! specified otherwise.
//!
//! # Ticks
//!
//! **A tick** is a point of time that can be used as a reference to represent
//! points of time in proximity. The first tick is [created] at boot time. A new
//! tick is created whenever [`PortToKernel::timer_tick`] is called. The system
//! tracks the latest tick that was created, which the system will use to
//! [derive] the latest system or event time by comparing
//! [the `tick_count` associated with the tick] to [the current `tick_count`].
//!
//! [created]: TimeoutGlobals::init
//! [`PortToKernel::timer_tick`]: super::PortToKernel::timer_tick
//! [derive]: system_time
//! [the `tick_count` associated with the tick]: TimeoutGlobals::last_tick_count
//! [the current `tick_count`]: super::PortTimer::tick_count
//!
//! It's important to create ticks at a steady rate. This is because tick counts
//! only have a limited range (`0..=`[`MAX_TICK_COUNT`]), and we can't calculate
//! the correct duration between the current time and the last tick if they are
//! too far away.
//!
//! [`MAX_TICK_COUNT`]: super::PortTimer::MAX_TICK_COUNT
use core::fmt;

use super::{
    utils::{lock_cpu, CpuLockCell, CpuLockGuardBorrowMut},
    Kernel, TimeError, UTicks,
};
use crate::{time::Time, utils::Init};

/// A kernel-global state for timed event management.
pub(super) struct TimeoutGlobals<System> {
    /// The value of [`PortTimer::tick_count`] on the previous “tick”.
    ///
    /// [`PortTimer::tick_count`]: super::PortTimer::tick_count
    last_tick_count: CpuLockCell<System, UTicks>,

    /// The event time on the previous “tick”.
    last_tick_time: CpuLockCell<System, Time32>,

    /// The system time on the previous “tick”.
    ///
    /// The current system time is always greater than or equal to
    /// `last_tick_sys_time`.
    last_tick_sys_time: CpuLockCell<System, Time64>,
}

impl<System> Init for TimeoutGlobals<System> {
    const INIT: Self = Self {
        last_tick_count: Init::INIT,
        last_tick_time: Init::INIT,
        last_tick_sys_time: Init::INIT,
    };
}

impl<System: Kernel> fmt::Debug for TimeoutGlobals<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TimeoutGlobals")
            .field("last_tick_count", &self.last_tick_count)
            .field("last_tick_time", &self.last_tick_time)
            .field("last_tick_sys_time", &self.last_tick_sys_time)
            .finish()
    }
}

// ---------------------------------------------------------------------------

/// An internal utility to access `TimeoutGlobals`.
trait KernelTimeoutGlobalsExt: Sized {
    fn g_timeout() -> &'static TimeoutGlobals<Self>;
}

impl<T: Kernel> KernelTimeoutGlobalsExt for T {
    /// Shortcut for `&Self::state().timeout`.
    #[inline(always)]
    fn g_timeout() -> &'static TimeoutGlobals<Self> {
        &Self::state().timeout
    }
}

// ---------------------------------------------------------------------------

/// Represents an absolute time.
type Time64 = u64;

/// Represents an absolute time with a reduced range.
type Time32 = u32;

#[inline]
fn time64_from_sys_time(sys_time: Time) -> Time64 {
    sys_time.as_micros()
}

#[inline]
fn sys_time_from_time64(sys_time: Time64) -> Time {
    Time::from_micros(sys_time)
}

// ---------------------------------------------------------------------------

/// A timeout.
struct Timeout<System> {
    /// The arrival time of the timeout. This is *an event time*.
    at: CpuLockCell<System, Time32>,
    // TODO
}

// ---------------------------------------------------------------------------

impl<System: Kernel> TimeoutGlobals<System> {
    /// Initialize the timekeeping system.
    pub(super) fn init(&self, mut lock: CpuLockGuardBorrowMut<'_, System>) {
        // Mark the first “tick”
        // Safety: CPU Lock active
        self.last_tick_count
            .replace(&mut *lock.borrow_mut(), unsafe { System::tick_count() });

        // Schedule the next tick
        // Safety: CPU Lock active
        unsafe { System::pend_tick_after(System::MAX_TIMEOUT) };
    }
}

// ---------------------------------------------------------------------------

/// Implements [`Kernel::time`].
pub(super) fn system_time<System: Kernel>() -> Result<Time, TimeError> {
    let mut lock = lock_cpu::<System>()?;

    let (duration_since_last_tick, _) = duration_since_last_tick(lock.borrow_mut());
    let last_tick_sys_time = System::g_timeout()
        .last_tick_sys_time
        .get(&*lock.borrow_mut());
    let cur_sys_time = last_tick_sys_time.wrapping_add(duration_since_last_tick as Time64);

    // Convert `Time64` to a public type
    Ok(sys_time_from_time64(cur_sys_time))
}

/// Implements [`Kernel::set_time`].
pub(super) fn set_system_time<System: Kernel>(new_sys_time: Time) -> Result<(), TimeError> {
    let mut lock = lock_cpu::<System>()?;

    let (duration_since_last_tick, _) = duration_since_last_tick(lock.borrow_mut());

    // Adjust `last_tick_sys_time` so that `system_time` will return the value
    // equal to `new_sys_time`
    let new_last_tick_sys_time =
        time64_from_sys_time(new_sys_time).wrapping_sub(duration_since_last_tick as Time64);

    System::g_timeout()
        .last_tick_sys_time
        .replace(&mut *lock.borrow_mut(), new_last_tick_sys_time);

    Ok(())
}

/// Calculate the elapsed time since the last tick.
///
/// Returns two values:
///
///  1. The duration in range `0..=System::MAX_TICK_COUNT`.
///  2. The value of `System::tick_count()` used for calculation.
///
#[inline]
fn duration_since_last_tick<System: Kernel>(
    mut lock: CpuLockGuardBorrowMut<'_, System>,
) -> (Time32, Time32) {
    // Safety: CPU Lock active
    let tick_count = unsafe { System::tick_count() };

    let last_tick_count = System::g_timeout().last_tick_count.get(&*lock.borrow_mut());

    // Guess the current time, taking the wrap-around behavior into account.
    // Basically, we want to find the smallest value of `time`
    // (≥ `last_tick_time`) that satisfies the following equation:
    //
    //     (last_tick_count + (time - last_tick_time)) % (MAX_TICK_COUNT + 1)
    //       == tick_count
    //
    let elapsed = if System::MAX_TICK_COUNT == UTicks::MAX || tick_count >= last_tick_count {
        // last_tick_count    tick_count
        // ┌──────┴────────────────┴────────┬───────────┐
        // 0      ╚════════════════╝  MAX_TICK_COUNT    MAX
        //              elapsed
        tick_count.wrapping_sub(last_tick_count)
    } else {
        //   tick_count     last_tick_count
        // ┌──────┴────────────────┴────────┬───────────┐
        // 0 ═════╝                ╚════════           MAX
        //                          elapsed
        // Note: If `System::MAX_TICK_COUNT == UTicks::MAX`, this reduces to
        // the first case because we are using wrapping arithmetics.
        tick_count.wrapping_sub(last_tick_count) - (UTicks::MAX - System::MAX_TICK_COUNT)
    };

    (elapsed, tick_count)
}

/// Implements [`PortToKernel::timer_tick`].
///
/// Precondition: CPU Lock inactive, an interrupt context
///
/// [`PortToKernel::timer_tick`]: super::PortToKernel::timer_tick
pub(super) fn handle_tick<System: Kernel>() {
    // The precondition includes CPU Lock being inactive, so this `unwrap`
    // should succeed
    let mut lock = lock_cpu::<System>().unwrap();

    // Mark the current “tick”
    let (duration_since_last_tick, tick_count) =
        duration_since_last_tick::<System>(lock.borrow_mut());

    let g_timeout = System::g_timeout();
    g_timeout.last_tick_count.replace(&mut *lock, tick_count);
    g_timeout
        .last_tick_time
        .replace_with(&mut *lock, |old_value| {
            old_value.wrapping_add(duration_since_last_tick)
        });
    g_timeout
        .last_tick_sys_time
        .replace_with(&mut *lock, |old_value| {
            old_value.wrapping_add(duration_since_last_tick as Time64)
        });

    // Schedule the next tick
    // Safety: CPU Lock active
    unsafe { System::pend_tick_after(System::MAX_TIMEOUT) };
}
