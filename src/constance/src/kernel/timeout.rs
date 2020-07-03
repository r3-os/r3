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
//! tick is created whenever [`PortToKernel::timer_tick`] is called. It's also
//! created when a new timeout is registered.
//!
//! The system tracks the latest tick that was created, which the system will
//! use to [derive] the latest system or event time by comparing
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
//!
//! # Event Times
//!
//! This line represents the value range of [`Time32`]. A current event time
//! (CET) is a mobile point on the line, constantly moving left to right. When
//! it reaches the end of the line, it goes back to the other end and keeps
//! moving. The arrival times of timeouts are immobile points on the line.
//!
//! ```text
//! ═════╤══════════════════════════════════════════════════════════
//!      │
//!     CET
//! ```
//!
//! There are some *zones* defined around CET (they move along with CET):
//!
//! ```text
//!                                       critical point
//!                                              │     overdue
//! ▃▃▃▃▃▃                                       │▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃
//! ═════╤═══════════════════════════════════════╧══════════════════
//! ▓▓▓▓▓│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓
//!     CET         enqueueable    user headroom   hard headroom
//! ```
//!
//!  - `CET ..= CET + DURATION_MAX`: Newly registered timeouts always belong to
//!    this **enqueueable zone**.
//!
//!  - `CET - USER_HEADROOM ..= CET + DURATION_MAX + USER_HEADROOM`:
//!    The **user headroom zone** surrounds the enqueueable zone. `adjust_time`
//!    may move timeouts to this zone. `adjust_time` does not allow adjustment
//!    that would move timeouts outside of this zone.
//!
//!    Timeouts can also move to this zone because of overdue timer interrupts.
//!
//!  - `CET - USER_HEADROOM - HARD_HEADROOM .. CET - USER_HEADROOM`:
//!    Timeouts can enter the **hard headroom zone** only because of overdue
//!    timer interrupts.
//!
//!  - `CET - USER_HEADROOM - HARD_HEADROOM ..= CET`: Timeouts in this **overdue
//!    zone** are said to be overdue. They will be processed the next time
//!    [`handle_tick`] is called.
//!
//! **Note 1:** `DURATION_MAX` is defined as `Duration::MAX.as_micros()` and is
//! equal to `0x80000000`.
//!
//! **Note 2:** `CET - USER_HEADROOM - HARD_HEADROOM + (Time32::MAX + 1)` is
//! equal to `CET + DURATION_MAX + USER_HEADROOM + 1`. In other words,
//! `HARD_HEADROOM` is defined for the hard headroom zone to fill the remaining
//! area.
//!
//! The earlier endpoint of the hard headroom zone is called **the critical
//! point**. No timeouts shall go past this point. It's an application's
//! responsibility to ensure this does not happen. Event times `x` and `y`
//! can have their chronological order determined by
//! `(x as Time32).wrapping_sub(critical_point).cmp(&(y as Time32).wrapping_sub(critical_point))`.
//!
//! ## Frontier
//!
//! We need to cap the amount of backward time adjustment so that
//! timeouts won't move past the critical point (from left).
//! We use the frontier-based method to enforce this in lieu of checking every
//! outstanding timeout for reasons explained in [`Kernel::adjust_time`].
//! The frontier (a concept used in the definition of [`Kernel::adjust_time`])
//! is a mobile point on the line that moves in the same way as the original
//! definition - it represents the most advanced CET the system has ever
//! observed. Timeouts are always created in relative to CET. This means the
//! arrival times of all registered timeouts are bounded by
//! `frontier + DURATION_MAX`, and thus enforcing `frontier - CET <=
//! USER_HEADROOM` is sufficient to achieve our goal here.
//!
//! ```text
//!                                    (CET + DURATION_MAX
//!                                 == frontier + DURATION_MAX)
//!           frontier                        event
//! ▃▃▃▃▃▃▃▃▃▃▃▃▃v                              v        │▃▃▃▃▃▃▃▃▃▃
//! ═════════════╤═══════════════════════════════════════╧══════════
//! ▒▒▒▒▒▒▓▓▓▓▓▓▓│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒▒▒▒
//!             CET         enqueueable       user headroom
//!
//! After adjust_time(-USER_HEADROOM):
//!
//!                             (CET + DURATION_MAX + USER_HEADROOM
//!                                 == frontier + DURATION_MAX)
//!           frontier                        event
//! ▃▃▃▃▃▃       v                              v│▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃▃
//! ═════╤═══════════════════════════════════════╧══════════════════
//! ▓▓▓▓▓│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░▓▓▓▓▓▓▓▓▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▓▓
//!     CET         enqueueable       user headroom
//! ```
//!
use core::fmt;

use super::{
    utils::{lock_cpu, CpuLockCell, CpuLockGuardBorrowMut},
    Kernel, TimeError, UTicks,
};
use crate::{
    time::{Duration, Time},
    utils::Init,
};

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

const USER_HEADROOM: Time32 = 1 << 29;

const HARD_HEADROOM: Time32 = 1 << 30;

/// The extent of how overdue a timed event can be made or how far a timed event
/// can be delayed past `Duration::MAX` by a call to [`adjust_time`].
///
/// [`adjust_time`]: crate::kernel::Kernel::adjust_time
///
/// The value is `1 << 29` microseconds.
pub const TIME_USER_HEADROOM: Duration = Duration::from_micros(USER_HEADROOM as i32);

/// The extent of how overdue the firing of [`timer_tick`] can be without
/// breaking the kernel timing algorithm.
///
/// [`timer_tick`]: crate::kernel::PortToKernel::timer_tick
///
/// The value is `1 << 30` microseconds.
pub const TIME_HARD_HEADROOM: Duration = Duration::from_micros(HARD_HEADROOM as i32);

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
