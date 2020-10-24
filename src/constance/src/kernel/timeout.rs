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
use core::{
    fmt,
    marker::PhantomPinned,
    pin::Pin,
    ptr::NonNull,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use super::{
    state::expect_task_context,
    task,
    utils::{lock_cpu, CpuLockCell, CpuLockGuard, CpuLockGuardBorrowMut},
    AdjustTimeError, BadParamError, Kernel, TimeError, UTicks,
};
use crate::{
    time::{Duration, Time},
    utils::{
        binary_heap::{BinaryHeap, BinaryHeapCtx},
        Init,
    },
};

#[cfg(tests)]
mod tests;

/// A kernel-global state for timed event management.
pub(super) struct TimeoutGlobals<System, TimeoutHeap: 'static> {
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
    #[cfg(feature = "system_time")]
    last_tick_sys_time: CpuLockCell<System, Time64>,

    /// The gap between the frontier and the previous tick.
    ///
    /// This value only can be increased by [`adjust_system_and_event_time`].
    /// The upper bound is [`USER_HEADROOM`].
    frontier_gap: CpuLockCell<System, Time32>,

    /// The heap (priority queue) containing outstanding timeouts, sorted by
    /// arrival time.
    heap: CpuLockCell<System, TimeoutHeap>,

    /// Flag indicating whether `handle_tick` is in progress or not.
    handle_tick_in_progress: CpuLockCell<System, bool>,
}

impl<System, TimeoutHeap: Init + 'static> Init for TimeoutGlobals<System, TimeoutHeap> {
    const INIT: Self = Self {
        last_tick_count: Init::INIT,
        last_tick_time: Init::INIT,
        #[cfg(feature = "system_time")]
        last_tick_sys_time: Init::INIT,
        frontier_gap: Init::INIT,
        heap: Init::INIT,
        handle_tick_in_progress: Init::INIT,
    };
}

impl<System: Kernel, TimeoutHeap: fmt::Debug> fmt::Debug for TimeoutGlobals<System, TimeoutHeap> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TimeoutGlobals")
            .field("last_tick_count", &self.last_tick_count)
            .field("last_tick_time", &self.last_tick_time)
            .field(
                "last_tick_sys_time",
                match () {
                    #[cfg(feature = "system_time")]
                    () => &self.last_tick_sys_time,
                    #[cfg(not(feature = "system_time"))]
                    () => &(),
                },
            )
            .field("frontier_gap", &self.frontier_gap)
            .field("heap", &self.heap)
            .field("handle_tick_in_progress", &self.handle_tick_in_progress)
            .finish()
    }
}

// ---------------------------------------------------------------------------

/// An internal utility to access `TimeoutGlobals`.
trait KernelTimeoutGlobalsExt: Kernel {
    fn g_timeout() -> &'static TimeoutGlobals<Self, Self::TimeoutHeap>;
}

impl<T: Kernel> KernelTimeoutGlobalsExt for T {
    /// Shortcut for `&Self::state().timeout`.
    #[inline(always)]
    fn g_timeout() -> &'static TimeoutGlobals<Self, Self::TimeoutHeap> {
        &Self::state().timeout
    }
}

// Types representing times
// ---------------------------------------------------------------------------

/// Represents an absolute time.
#[cfg(feature = "system_time")]
type Time64 = u64;

/// Represents an absolute time with a reduced range. This is also used to
/// represent a relative time span.
pub(super) type Time32 = u32;

/// Atomic cell of [`Time32`].
type AtomicTime32 = AtomicU32;

/// A value of type [`Time32`] that can be used to represent a “null” value.
/// [`time32_from_duration`] and [`time32_from_neg_duration`] never returns this
/// value. Do not pass this value to any of this module's methods.
pub(super) const BAD_DURATION32: Time32 = u32::MAX;

#[inline]
#[cfg(feature = "system_time")]
fn time64_from_sys_time(sys_time: Time) -> Time64 {
    sys_time.as_micros()
}

#[inline]
#[cfg(feature = "system_time")]
fn sys_time_from_time64(sys_time: Time64) -> Time {
    Time::from_micros(sys_time)
}

#[inline]
pub(super) const fn time32_from_duration(duration: Duration) -> Result<Time32, BadParamError> {
    // Ok(duration
    //     .as_micros()
    //     .try_into()
    //     .map_err(|_| BadParamError::BadParam)?)

    // FIXME: This is a work-around for `TryFrom` being unavailable in `const fn`
    //        and `map_err` being unavailable in `const fn`
    if let Some(x) = crate::utils::convert::try_i32_into_u32(duration.as_micros()) {
        Ok(x)
    } else {
        Err(BadParamError::BadParam)
    }
}

/// Convert the negation of `duration` to `Time32`.
#[inline]
pub(super) fn time32_from_neg_duration(duration: Duration) -> Result<Time32, BadParamError> {
    // Unlike `time32_from_duration`, there's no nice way to do this
    let duration = duration.as_micros();
    if duration > 0 {
        Err(BadParamError::BadParam)
    } else {
        Ok(0u32.wrapping_sub(duration as u32))
    }
}

/// Convert `duration` to `Time32`. Negative values are wrapped around.
#[inline]
pub(super) fn wrapping_time32_from_duration(duration: Duration) -> Time32 {
    duration.as_micros() as Time32
}

/// Convert `duration` to `Time64`. Negative values are wrapped around.
#[inline]
#[cfg(feature = "system_time")]
pub(super) fn wrapping_time64_from_duration(duration: Duration) -> Time64 {
    duration.as_micros() as i64 as Time64
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

// Timeouts
// ---------------------------------------------------------------------------

/// A timeout.
///
/// `Timeout` is a `!Unpin` type. Once registered by [`insert_timeout`], the
/// `Timeout` must stay in the same memory location until it's unregistered.
/// Dropping isn't allowed either. `Timeout::drop` can detect the violation of
/// this requirement and cause a panic.
///
/// `Timeout` is unregistered by one of the following ways:
///
///  - On expiration, right before its callback function is called.
///  - [`remove_timeout`] can unregister a `Timeout` at anytime. There is a
///    RAII guard type [`TimeoutGuard`] that does this automatically.
///
pub(super) struct Timeout<System: Kernel> {
    /// The arrival time of the timeout. This is *an event time*.
    ///
    /// This is defined as an atomic variable only because [`TimeoutHeapCtx`]
    /// needs to access this. Otherwise, this would have been
    /// [`CpuLockCell`]`<System, _>`. `Ordering::Relaxed` is overkill but that's
    /// the weakest ordering that `std::sync::atomic` provides.
    at: AtomicTime32,

    /// The position of this timeout in [`TimeoutGlobals::heap`].
    ///
    /// Similarly to [`Self::at`], this is defined as an atomic variable only
    /// because [`TimeoutHeapCtx`] needs to access this.
    ///
    /// [`HEAP_POS_NONE`] indicates this timeout is not included in the heap.
    heap_pos: AtomicUsize,

    /// Callback function.
    callback: TimeoutFn<System>,

    /// Parameter given to the callback function.
    callback_param: usize,

    /// Un-implement `Unpin`.
    _pin: PhantomPinned,

    // TODO: callback
    _phantom: core::marker::PhantomData<System>,
}

/// Tiemout callback function.
///
/// The callback function is called with CPU Lock active and an interrupt
/// context when the associated [`Timeout`] expires.
///
/// The callback function may wake up tasks. When it does that, it doesn't have
/// to call `unlock_cpu_and_check_preemption` or `yield_cpu` - it's
/// automatically taken care of.
pub(super) type TimeoutFn<System> = fn(usize, CpuLockGuard<System>) -> CpuLockGuard<System>;

/// Value of [`Timeout::heap_pos`] indicating the timeout is not included in the
/// heap.
const HEAP_POS_NONE: usize = usize::MAX;

impl<System: Kernel> Init for Timeout<System> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        at: Init::INIT,
        heap_pos: Init::INIT,
        callback: |_, x| x,
        callback_param: Init::INIT,
        _pin: PhantomPinned,
        _phantom: core::marker::PhantomData,
    };
}

impl<System: Kernel> Drop for Timeout<System> {
    #[inline]
    fn drop(&mut self) {
        if *self.heap_pos.get_mut() != HEAP_POS_NONE {
            // The timeout is still in the heap. Dropping `self` now would
            // cause use-after-free. Since we don't have CPU Lock and we aren't
            // sure if we can get a hold of it, aborting is the only course of
            // action we can take. The owner of `Timeout` is responsible for
            // ensuring this does not happen.
            //
            // Actually, `libcore` doesn't have an equivalent of
            // `std::process::abort`. `panic!` in `drop` will escalate to abort,
            // I think?
            panic!("timeout is still linked");
        }
    }
}

impl<System: Kernel> fmt::Debug for Timeout<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Timeout")
            .field("at", &self.at)
            .field("heap_pos", &self.heap_pos)
            .field("callback", &self.callback)
            .field("callback_param", &self.callback_param)
            .finish()
    }
}

impl<System: Kernel> Timeout<System> {
    /// Construct a `Timeout`.
    ///
    /// The expiration time is set to zero (the origin at boot time, an
    /// unspecified time point otherwise).
    pub(super) const fn new(callback: TimeoutFn<System>, callback_param: usize) -> Self {
        Self {
            at: AtomicTime32::new(0),
            heap_pos: AtomicUsize::new(HEAP_POS_NONE),
            callback,
            callback_param,
            _pin: PhantomPinned,
            _phantom: core::marker::PhantomData,
        }
    }

    /// Get a flag indicating whether the `Timeout` is currently in the heap.
    pub(super) fn is_linked(&self, _lock: CpuLockGuardBorrowMut<'_, System>) -> bool {
        self.heap_pos.load(Ordering::Relaxed) != HEAP_POS_NONE
    }

    /// Configure the `Timeout` to expire in the specified duration.
    pub(super) fn set_expiration_after(
        &self,
        lock: CpuLockGuardBorrowMut<'_, System>,
        duration_time32: Time32,
    ) {
        debug_assert_ne!(duration_time32, BAD_DURATION32);

        let current_time = current_time(lock);
        let at = current_time.wrapping_add(duration_time32);
        self.at.store(at, Ordering::Relaxed);
    }

    /// Adjust the `Timeout`'s expiration time.
    ///
    /// Intended to be used by periodic events before re-registering the
    /// `Timeout`.
    pub(super) fn adjust_expiration(
        &self,
        _lock: CpuLockGuardBorrowMut<'_, System>,
        duration_time32: Time32,
    ) {
        debug_assert_ne!(duration_time32, BAD_DURATION32);

        let at = self
            .at
            .load(Ordering::Relaxed)
            .wrapping_add(duration_time32);
        self.at.store(at, Ordering::Relaxed);
    }

    #[inline]
    pub(super) fn saturating_duration_until_timeout(
        &self,
        lock: CpuLockGuardBorrowMut<'_, System>,
    ) -> Time32 {
        saturating_duration_until_timeout(self, current_time(lock))
    }

    /// Get the raw expiration time.
    pub(super) fn at_raw(&self, _lock: CpuLockGuardBorrowMut<'_, System>) -> Time32 {
        self.at.load(Ordering::Relaxed)
    }

    /// Set the raw expiration time.
    ///
    /// This might be useful for storing arbitrary data in an unlinked `Timeout`.
    pub(super) fn set_at_raw(&self, _lock: CpuLockGuardBorrowMut<'_, System>, value: Time32) {
        self.at.store(value, Ordering::Relaxed);
    }

    /// Set the raw expiration time, returning the modified instance of `self`.
    ///
    /// This might be useful for storing arbitrary data in an unlinked `Timeout`.
    pub(super) const fn with_at_raw(mut self, at: Time32) -> Self {
        self.at = AtomicTime32::new(at);
        self
    }

    /// Set the expiration time with a duration since boot, returning the
    /// modified instance of `self`.
    pub(super) const fn with_expiration_at(mut self, at: Time32) -> Self {
        assert!(at != BAD_DURATION32, "`at` must be a valid duration");

        self.at = AtomicTime32::new(at);
        self
    }
}

/// A reference to a [`Timeout`].
#[doc(hidden)]
pub struct TimeoutRef<System: Kernel>(NonNull<Timeout<System>>);

// Safety: `Timeout` is `Send + Sync`
unsafe impl<System: Kernel> Send for TimeoutRef<System> {}
unsafe impl<System: Kernel> Sync for TimeoutRef<System> {}

impl<System: Kernel> Clone for TimeoutRef<System> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<System: Kernel> Copy for TimeoutRef<System> {}

impl<System: Kernel> fmt::Debug for TimeoutRef<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("TimeoutRef").field(&self.0).finish()
    }
}

/// Used when manipulating [`TimeoutGlobals::heap`]. Provides the correct
/// comparator function for [`Timeout`]s. Ensures [`Timeout::heap_pos`] is
/// up-to-date.
struct TimeoutHeapCtx {
    critical_point: Time32,
}

impl<System: Kernel> BinaryHeapCtx<TimeoutRef<System>> for TimeoutHeapCtx {
    #[inline]
    fn lt(&mut self, x: &TimeoutRef<System>, y: &TimeoutRef<System>) -> bool {
        // Safety: `x` and `y` are in the heap, so the pointees must be valid
        let (x, y) = unsafe {
            (
                x.0.as_ref().at.load(Ordering::Relaxed),
                y.0.as_ref().at.load(Ordering::Relaxed),
            )
        };
        let critical_point = self.critical_point;
        x.wrapping_sub(critical_point) < y.wrapping_sub(critical_point)
    }

    #[inline]
    fn on_move(&mut self, e: &mut TimeoutRef<System>, new_index: usize) {
        // Safety: `e` is in the heap, so the pointee must be valid
        unsafe { e.0.as_ref() }
            .heap_pos
            .store(new_index, Ordering::Relaxed);
    }
}

// Initialization
// ---------------------------------------------------------------------------

impl<System: Kernel, TimeoutHeap> TimeoutGlobals<System, TimeoutHeap> {
    /// Initialize the timekeeping system.
    pub(super) fn init(&self, mut lock: CpuLockGuardBorrowMut<'_, System>) {
        // Mark the first “tick”
        // Safety: CPU Lock active
        self.last_tick_count
            .replace(&mut *lock.borrow_mut(), unsafe { System::tick_count() });

        // Schedule the next tick. There are no timeouts registered at the
        // moment, so use `MAX_TIMEOUT`.
        // Safety: CPU Lock active
        unsafe { System::pend_tick_after(System::MAX_TIMEOUT) };
    }
}

// Global Time Management
// ---------------------------------------------------------------------------

/// Implements [`Kernel::time`].
#[cfg(feature = "system_time")]
pub(super) fn system_time<System: Kernel>() -> Result<Time, TimeError> {
    expect_task_context::<System>()?;
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
    expect_task_context::<System>()?;

    match () {
        #[cfg(feature = "system_time")]
        () => {
            let mut lock = lock_cpu::<System>()?;
            let (duration_since_last_tick, _) = duration_since_last_tick(lock.borrow_mut());

            // Adjust `last_tick_sys_time` so that `system_time` will return the value
            // equal to `new_sys_time`
            let new_last_tick_sys_time =
                time64_from_sys_time(new_sys_time).wrapping_sub(duration_since_last_tick as Time64);

            System::g_timeout()
                .last_tick_sys_time
                .replace(&mut *lock.borrow_mut(), new_last_tick_sys_time);
        }

        #[cfg(not(feature = "system_time"))]
        () => {
            // If `system_time` feature is disabled, the system time is not
            // observable, so this function is no-op. It still needs to validate
            // the current context and return an error as needed.
            let _ = new_sys_time; // suppress "unused parameter"
            lock_cpu::<System>()?;
        }
    }

    Ok(())
}

/// Implements [`Kernel::adjust_time`].
pub(super) fn adjust_system_and_event_time<System: Kernel>(
    delta: Duration,
) -> Result<(), AdjustTimeError> {
    let mut lock = lock_cpu::<System>()?;
    let g_timeout = System::g_timeout();

    // For the `delta.is_negative()` case, we'd like to check if the adjustment
    // would throw the frontier out of the valid range. The frontier is a
    // time-dependent quantity, so first we need to get the latest value of the
    // frontier.
    //
    // `mark_tick` will update `frontier_gap` with the latest value without
    // introducing any application-visible side-effects.
    //
    // This is also useful for the `delta.is_positive()` case because it updates
    // `last_tick_time`.
    mark_tick(lock.borrow_mut());

    if delta.is_negative() {
        let delta_abs = time32_from_neg_duration(delta).unwrap();

        let new_frontier_gap = g_timeout.frontier_gap.get(&*lock) + delta_abs;

        if new_frontier_gap > USER_HEADROOM {
            // The frontier would be too far away
            return Err(AdjustTimeError::BadObjectState);
        }

        g_timeout.frontier_gap.replace(&mut *lock, new_frontier_gap);
    } else if delta.is_positive() {
        let delta_abs = time32_from_duration(delta).unwrap();

        // Check the top element (representing the earliest timeout) in the heap
        if let Some(&timeout_ref) = g_timeout.heap.read(&*lock).get(0) {
            // Safety: `timeout_ref` is in the heap, meaning the pointee is valid
            let timeout = unsafe { timeout_ref.0.as_ref() };

            let current_time = g_timeout.last_tick_time.get(&*lock);

            // How much time do we have before `timeout` enters the hard headroom
            // zone?
            let duration =
                saturating_duration_before_timeout_exhausting_user_headroom(timeout, current_time);

            if duration < delta_abs {
                // The timeout would enter the hard headroom zone if we made
                // this adjustment
                return Err(AdjustTimeError::BadObjectState);
            }
        }

        g_timeout
            .frontier_gap
            .replace_with(&mut *lock, |old_value| old_value.saturating_sub(delta_abs));
    } else {
        // Do nothing
        return Ok(());
    }

    // Update the current system time and the current event time
    let delta32 = wrapping_time32_from_duration(delta);
    g_timeout
        .last_tick_time
        .replace_with(&mut *lock, |old_value| old_value.wrapping_add(delta32));

    #[cfg(feature = "system_time")]
    {
        let delta64 = wrapping_time64_from_duration(delta);
        g_timeout
            .last_tick_sys_time
            .replace_with(&mut *lock, |old_value| old_value.wrapping_add(delta64));
    }

    // Schedule the next tick
    let current_time = g_timeout.last_tick_time.get(&*lock);
    pend_next_tick(lock.borrow_mut(), current_time);

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

/// Create a tick now.
fn mark_tick<System: Kernel>(mut lock: CpuLockGuardBorrowMut<'_, System>) {
    let (duration_since_last_tick, tick_count) =
        duration_since_last_tick::<System>(lock.borrow_mut());

    let g_timeout = System::g_timeout();
    g_timeout.last_tick_count.replace(&mut *lock, tick_count);
    g_timeout
        .last_tick_time
        .replace_with(&mut *lock, |old_value| {
            old_value.wrapping_add(duration_since_last_tick)
        });
    #[cfg(feature = "system_time")]
    g_timeout
        .last_tick_sys_time
        .replace_with(&mut *lock, |old_value| {
            old_value.wrapping_add(duration_since_last_tick as Time64)
        });

    g_timeout
        .frontier_gap
        .replace_with(&mut *lock, |old_value| {
            old_value.saturating_sub(duration_since_last_tick)
        });
}

/// Implements [`PortToKernel::timer_tick`].
///
/// Precondition: CPU Lock inactive, an interrupt context
///
/// [`PortToKernel::timer_tick`]: super::PortToKernel::timer_tick
#[inline]
pub(super) fn handle_tick<System: Kernel>() {
    // The precondition includes CPU Lock being inactive, so this `unwrap`
    // should succeed
    let mut lock = lock_cpu::<System>().unwrap();

    mark_tick(lock.borrow_mut());

    let g_timeout = System::g_timeout();
    let current_time = g_timeout.last_tick_time.get(&*lock);
    let critical_point = critical_point(current_time);

    // Set `handle_tick_in_progress`. This will suppress redundant calls to
    // `pend_next_tick` made by timeout handlers.
    g_timeout.handle_tick_in_progress.replace(&mut *lock, true);

    // Process expired timeouts.
    //
    // For each iteration, check the top element (representing the earliest
    // timeout) in the heap. Exit from the loop if the heap is empty.
    while let Some(&timeout_ref) = g_timeout.heap.read(&*lock).get(0) {
        // Safety: `timeout_ref` is in the heap, meaning the pointee is valid
        let timeout = unsafe { &*timeout_ref.0.as_ptr() };

        // How much time do we have before `timeout` becomes overdue?
        let remaining = saturating_duration_until_timeout(timeout, current_time);
        if remaining > 0 {
            break;
        }

        // The timeout has expired. Remove it from the heap.
        let Timeout {
            callback,
            callback_param,
            ..
        } = *timeout;

        debug_assert_eq!(timeout.heap_pos.load(Ordering::Relaxed), 0);
        timeout.heap_pos.store(HEAP_POS_NONE, Ordering::Relaxed);

        g_timeout
            .heap
            .write(&mut *lock)
            .heap_remove(0, TimeoutHeapCtx { critical_point });

        // (Note: `timeout` is considered invalid at this point because it's not
        // in the heap anymore)

        // Call the callback function.
        lock = callback(callback_param, lock);
    }

    g_timeout.handle_tick_in_progress.replace(&mut *lock, false);

    // Schedule the next tick
    pend_next_tick(lock.borrow_mut(), current_time);

    // Callback functions might have woken up some tasks. Check for dispatch and
    // release CPU Lock.
    task::unlock_cpu_and_check_preemption(lock);
}

/// Get the current event time.
fn current_time<System: Kernel>(mut lock: CpuLockGuardBorrowMut<'_, System>) -> Time32 {
    let (duration_since_last_tick, _) = duration_since_last_tick::<System>(lock.borrow_mut());

    let g_timeout = System::g_timeout();
    g_timeout
        .last_tick_time
        .get(&*lock)
        .wrapping_add(duration_since_last_tick)
}

/// Schedule the next tick.
fn pend_next_tick<System: Kernel>(lock: CpuLockGuardBorrowMut<'_, System>, current_time: Time32) {
    let mut delay = System::MAX_TIMEOUT;

    // Check the top element (representing the earliest timeout) in the heap
    let g_timeout = System::g_timeout();
    if let Some(&timeout_ref) = g_timeout.heap.read(&*lock).get(0) {
        // Safety: `timeout_ref` is in the heap, meaning the pointee is valid
        let timeout = unsafe { timeout_ref.0.as_ref() };

        // How much time do we have before `timeout` becomes overdue?
        delay = delay.min(saturating_duration_until_timeout(timeout, current_time));
    }

    // Safety: CPU Lock active
    unsafe {
        if delay == 0 {
            System::pend_tick();
        } else {
            System::pend_tick_after(delay);
        }
    }
}

// Timeout Management
// ---------------------------------------------------------------------------

/// Find the critical point based on the current event time.
#[inline]
fn critical_point(current_time: Time32) -> Time32 {
    current_time.wrapping_sub(HARD_HEADROOM + USER_HEADROOM)
}

/// Calculate the duration until the specified timeout is reached. Returns `0`
/// if the timeout is already overdue.
fn saturating_duration_until_timeout<System: Kernel>(
    timeout: &Timeout<System>,
    current_time: Time32,
) -> Time32 {
    let critical_point = critical_point(current_time);

    let duration_until_violating_critical_point = timeout
        .at
        .load(Ordering::Relaxed)
        .wrapping_sub(critical_point);

    duration_until_violating_critical_point.saturating_sub(HARD_HEADROOM + USER_HEADROOM)
}

/// Calculate the duration before the specified timeout surpasses the user
/// headroom zone (and enters the hard headroom zone).
fn saturating_duration_before_timeout_exhausting_user_headroom<System: Kernel>(
    timeout: &Timeout<System>,
    current_time: Time32,
) -> Time32 {
    let critical_point = critical_point(current_time);

    let duration_until_violating_critical_point = timeout
        .at
        .load(Ordering::Relaxed)
        .wrapping_sub(critical_point);

    duration_until_violating_critical_point.saturating_sub(HARD_HEADROOM)
}

/// Register the specified timeout.
pub(super) fn insert_timeout<System: Kernel>(
    mut lock: CpuLockGuardBorrowMut<'_, System>,
    timeout: Pin<&Timeout<System>>,
) {
    // This check is important for memory safety. For each `Timeout`, there can
    // be only one heap entry pointing to that `Timeout`. `heap_pos` indicates
    // whether there's a corresponding heap entry or not. If we let two entries
    // reside in the heap, when we remove the first one, we would falsely flag
    // the `Timeout` as "not in the heap". If we drop the `Timeout` in this
    // state, The second entry would be still referencing the no-longer existent
    // `Timeout`.
    assert_eq!(
        timeout.heap_pos.load(Ordering::Relaxed),
        HEAP_POS_NONE,
        "timeout is already registered",
    );

    let current_time = current_time(lock.borrow_mut());
    let critical_point = critical_point(current_time);

    // Insert a reference to `timeout` into the heap
    //
    // `Timeout` is `!Unpin` and `Timeout::drop` ensures it's not dropped while
    // it's still in the heap, so `*timeout` will never be leaked¹ while being
    // referenced by the heap. Therefore, it's safe to insert a reference
    // to `*timeout` into the heap.
    //
    //  ¹ Rust jargon meaning destroying an object without running its
    //    destructor.
    let pos = System::g_timeout().heap.write(&mut *lock).heap_push(
        TimeoutRef((&*timeout).into()),
        TimeoutHeapCtx { critical_point },
    );

    // `TimeoutHeapCtx:on_move` should have assigned `heap_pos`
    debug_assert_eq!(timeout.heap_pos.load(Ordering::Relaxed), pos);

    if !System::g_timeout().handle_tick_in_progress.get(&*lock) {
        // (Re-)schedule the next tick
        pend_next_tick(lock, current_time);
    }
}

/// Unregister the specified `Timeout`. Does nothing if it's not registered.
#[inline]
pub(super) fn remove_timeout<System: Kernel>(
    lock: CpuLockGuardBorrowMut<'_, System>,
    timeout: &Timeout<System>,
) {
    remove_timeout_inner(lock, timeout);

    // Reset `heap_pos` here so that the compiler can eliminate the check in
    // `Timeout::drop`. See the following example:
    //
    //     // `remove_timeout` is marked as `#[inline]`, so the compiler can
    //     // figure out that `heap_pos` is set to `HEAP_POS_NONE` by this call
    //     remove_timeout(lock, &timeout);
    //
    //     // `Timeout::drop` checks `heap_pos` and panics if `heap_pos` is
    //     // not `HEAP_POS_NONE`. The compiler will likely eliminate this
    //     // check.
    //     drop(timeout);
    //
    timeout.heap_pos.store(HEAP_POS_NONE, Ordering::Relaxed);
}

fn remove_timeout_inner<System: Kernel>(
    mut lock: CpuLockGuardBorrowMut<'_, System>,
    timeout: &Timeout<System>,
) {
    let current_time = current_time(lock.borrow_mut());
    let critical_point = critical_point(current_time);

    // Remove `timeout` from the heap
    //
    // If `heap_pos == HEAP_POS_NONE`, we are supposed to do nothing.
    // `HEAP_POS_NONE` is a huge value, so `heap_remove` will inevitably reject
    // such a huge value by bounds check. This way, we can check both for bounds
    // and `HEAP_POS_NONE` in one fell swoop.
    let heap_pos = timeout.heap_pos.load(Ordering::Relaxed);

    let timeout_ref = System::g_timeout()
        .heap
        .write(&mut *lock)
        .heap_remove(heap_pos, TimeoutHeapCtx { critical_point });

    if timeout_ref.is_none() {
        // The cause of failure must be `timeout` not being registered in the
        // first place. (Bounds check failure would be clearly because of
        // our programming error.)
        debug_assert_eq!(heap_pos, HEAP_POS_NONE);
        return;
    }

    // The removed element should have pointed to `timeout`
    debug_assert_eq!(
        timeout_ref.unwrap().0.as_ptr() as *const _,
        timeout as *const _
    );

    if !System::g_timeout().handle_tick_in_progress.get(&*lock) {
        // (Re-)schedule the next tick
        pend_next_tick(lock, current_time);
    }
}

/// RAII guard that automatically unregisters `Timeout` when dropped.
pub(super) struct TimeoutGuard<'a, 'b, System: Kernel> {
    pub(super) timeout: Pin<&'a Timeout<System>>,
    pub(super) lock: CpuLockGuardBorrowMut<'b, System>,
}

impl<'a, 'b, System: Kernel> Drop for TimeoutGuard<'a, 'b, System> {
    #[inline]
    fn drop(&mut self) {
        remove_timeout(self.lock.borrow_mut(), &self.timeout);
    }
}
