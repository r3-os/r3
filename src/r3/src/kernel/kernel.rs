//! Kernel global functions
use core::fmt;

use crate::{
    kernel::{
        raw, AdjustTimeError, BoostPriorityError, CpuLockError, ExitTaskError, ParkError,
        ParkTimeoutError, SleepError, TimeError,
    },
    time::{Duration, Time},
};

/// Provides access to the global functionalities of a kernel.
///
/// This trait is mostly comprised of the same methods as those of the traits
/// from the [`raw`] module. However, this trait is covered under a stronger
/// semver guarantee as it's an application-facing API. (TODO: Link to the
/// relevant portion of the document)
pub trait Kernel: private::Sealed {
    type DebugPrinter: fmt::Debug + Send + Sync;

    /// Get an object that implements [`Debug`](fmt::Debug) for dumping the
    /// current kernel state.
    ///
    /// Note that printing this object might consume a large amount of stack
    /// space.
    fn debug() -> <Self as Kernel>::DebugPrinter;

    /// Activate [CPU Lock].
    ///
    /// Returns [`BadContext`] if CPU Lock is already active.
    ///
    /// [CPU Lock]: crate#system-states
    /// [`BadContext`]: CpuLockError::BadContext
    fn acquire_cpu_lock() -> Result<(), CpuLockError>;

    /// Deactivate [CPU Lock].
    ///
    /// Returns [`BadContext`] if CPU Lock is already inactive.
    ///
    /// [CPU Lock]: crate#system-states
    /// [`BadContext`]: CpuLockError::BadContext
    ///
    /// # Safety
    ///
    /// CPU Lock is useful for creating a critical section. By making this
    /// method `unsafe`, safe code is prevented from interfering with a critical
    /// section.
    ///
    /// Deactivating CPU Lock in a boot context is disallowed.
    unsafe fn release_cpu_lock() -> Result<(), CpuLockError>;

    /// Return a flag indicating whether CPU Lock is currently active.
    fn has_cpu_lock() -> bool;

    /// Activate [Priority Boost].
    ///
    /// Returns [`BadContext`] if Priority Boost is already active, the
    /// calling context is not a task context, or CPU Lock is active.
    ///
    /// [Priority Boost]: crate#system-states
    /// [`BadContext`]: CpuLockError::BadContext
    fn boost_priority() -> Result<(), BoostPriorityError>
    where
        Self: raw::KernelBoostPriority;

    /// Deactivate [Priority Boost].
    ///
    /// Returns [`BadContext`] if Priority Boost is already inactive, the
    /// calling context is not a task context, or CPU Lock is active.
    ///
    /// [Priority Boost]: crate#system-states
    /// [`BadContext`]: CpuLockError::BadContext
    ///
    /// # Safety
    ///
    /// Priority Boost is useful for creating a critical section. By making this
    /// method `unsafe`, safe code is prevented from interfering with a critical
    /// section.
    unsafe fn unboost_priority() -> Result<(), BoostPriorityError>;

    /// Return a flag indicating whether [Priority Boost] is currently active.
    ///
    /// [Priority Boost]: crate#system-states
    fn is_priority_boost_active() -> bool;

    /// Set the current [system time].
    ///
    /// This method *does not change* the relative arrival times of outstanding
    /// timed events nor the relative time of the frontier (a concept used in
    /// the definition of [`adjust_time`]).
    ///
    /// [system time]: crate#kernel-timing
    /// [`adjust_time`]: Self::adjust_time
    ///
    /// This method will return [`TimeError::BadContext`] when called in a
    /// non-task context.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Rationale:** This restriction originates from μITRON4.0. It's
    /// > actually unnecessary in the current implementation, but allows
    /// > headroom for potential changes in the implementation.
    fn set_time(time: Time) -> Result<(), TimeError>;

    /// Get the current [system time].
    ///
    /// [system time]: crate#kernel-timing
    ///
    /// This method will return [`TimeError::BadContext`] when called in a
    /// non-task context.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Rationale:** This restriction originates from μITRON4.0. It's
    /// > actually unnecessary in the current implementation, but allows
    /// > headroom for potential changes in the implementation.
    fn time() -> Result<Time, TimeError>
    where
        Self: raw::KernelTime;

    /// Move the current [system time] forward or backward by the specified
    /// amount.
    ///
    /// This method *changes* the relative arrival times of outstanding
    /// timed events.
    ///
    /// The kernel uses a limited number of bits to represent the arrival times
    /// of outstanding timed events. This means that there's some upper bound
    /// on how far the system time can be moved away without breaking internal
    /// invariants. This method ensures this bound is not violated by the
    /// methods described below. This method will return `BadObjectState` if
    /// this check fails.
    ///
    /// **Moving Forward (`delta > 0`):** If there are no outstanding time
    /// events, adjustment in this direction is unbounded. Otherwise, let
    /// `t` be the relative arrival time (in relation to the current time) of
    /// the earliest outstanding time event.
    /// If `t - delta < -`[`TIME_USER_HEADROOM`] (i.e., if the adjustment would
    /// make the event overdue by more than `TIME_USER_HEADROOM`), the check
    /// will fail.
    ///
    /// The events made overdue by the call will be processed when the port
    /// timer driver announces a new tick. It's unspecified whether this happens
    /// before or after the call returns.
    ///
    /// **Moving Backward (`delta < 0`):** First, we introduce the concept of
    /// **a frontier**. The frontier represents the point of time at which the
    /// system time advanced the most. Usually, the frontier is identical to
    /// the current system time because the system time keeps moving forward
    /// (a). However, adjusting the system time to past makes them temporarily
    /// separate from each other (b). In this case, the frontier stays in place
    /// until the system time eventually catches up with the frontier and they
    /// start moving together again (c).
    ///
    /// <center>
    ///
    #[doc = svgbobdoc::transform_mdstr!(
    /// ```svgbob
    ///                                   system time
    ///                                    ----*------------------------
    ///                                                     ^ frontier
    /// ​
    ///                                                (b)
    /// ​
    ///                                    --------*--------------------
    ///       system time                                   ^
    /// ----------*------------            ------------*----------------
    ///           ^ frontier                                ^
    ///                                    -----------------*-----------
    ///          (a)                                        ^
    ///                                    ----------------------*------
    ///                                                          ^
    ///                                                (c)
    /// ```
    )]
    ///
    /// </center>
    ///
    /// Let `frontier` be the current relative time of the frontier (in relation
    /// to the current time). If `frontier - delta > `[`TIME_USER_HEADROOM`]
    /// (i.e., if the adjustment would move the frontier too far away), the
    /// check will fail.
    ///
    /// [system time]: crate#kernel-timing
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Observation:** Even under ideal circumstances, all timed events are
    /// > bound to be overdue by a very small extent because of various factors
    /// > such as an intrinsic interrupt latency, insufficient timer resolution,
    /// > and uses of CPU Lock. This means the minimum value of `t` in the above
    /// > explanation is not `0` but a somewhat smaller value. The consequence
    /// > is that `delta` can never reliably be `>= TIME_USER_HEADROOM`.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Relation to Other Specifications:** `adj_tim` from
    /// > [the TOPPERS 3rd generation kernels]
    ///
    /// [the TOPPERS 3rd generation kernels]: https://www.toppers.jp/index.html
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Rationale:** When moving the system time forward, capping by a
    /// > frontier instead of an actual latest arrival time has advantages over
    /// > other schemes that involve tracking the latest arrival time:
    /// >
    /// >  - Linear-scanning all outstanding timed events to find the latest
    /// >    arrival time would take a linear time.
    /// >
    /// >  - Using a double-ended data structure for an event queue, such as a
    /// >    balanced search tree and double heaps, would increase the runtime
    /// >    cost of maintaining the structure.
    /// >
    /// > Also, the gap between the current time and the frontier is completely
    /// > in control of the code that calls `adjust_time`, making the behavior
    /// > more predictable.
    fn adjust_time(delta: Duration) -> Result<(), AdjustTimeError>
    where
        Self: raw::KernelAdjustTime;

    // TODO: get time resolution?

    /// Terminate the current task, putting it into the Dormant state.
    ///
    /// The kernel (to be precise, the port) makes an implicit call to this
    /// function when a task entry point function returns.
    ///
    /// # Safety
    ///
    /// On a successful call, this function destroys the current task's stack
    /// without running any destructors on stack-allocated objects and renders
    /// all references pointing to such objects invalid. The caller is
    /// responsible for taking this possibility into account and ensuring this
    /// doesn't lead to an undefined behavior.
    ///
    unsafe fn exit_task() -> Result<!, ExitTaskError>;

    /// Put the current task into the Waiting state until the task's token is
    /// made available by [`Task::unpark`]. The token is initially absent when
    /// the task is activated.
    ///
    /// The token will be consumed when this method returns successfully.
    ///
    /// This system service may block. Therefore, calling this method is not
    /// allowed in [a non-waitable context] and will return `Err(BadContext)`.
    ///
    /// [a non-waitable context]: crate#contexts
    fn park() -> Result<(), ParkError>;

    /// [`park`](Self::park) with timeout.
    ///
    /// This system service may block. Therefore, calling this method is not
    /// allowed in [a non-waitable context] and will return `Err(BadContext)`.
    ///
    /// [a non-waitable context]: crate#contexts
    fn park_timeout(timeout: Duration) -> Result<(), ParkTimeoutError>;

    /// Block the current task for the specified duration.
    fn sleep(duration: Duration) -> Result<(), SleepError>;
}

mod private {
    pub trait Sealed {}
    impl<T: super::raw::KernelBase> Sealed for T {}
}

impl<T: raw::KernelBase> Kernel for T {
    type DebugPrinter = <Self as raw::KernelBase>::RawDebugPrinter;

    #[inline]
    fn debug() -> <Self as Kernel>::DebugPrinter {
        <T as raw::KernelBase>::raw_debug()
    }

    #[inline]
    fn acquire_cpu_lock() -> Result<(), CpuLockError> {
        <T as raw::KernelBase>::raw_acquire_cpu_lock()
    }

    #[inline]
    unsafe fn release_cpu_lock() -> Result<(), CpuLockError> {
        // Safety: Just forwarding the calls
        unsafe { <T as raw::KernelBase>::raw_release_cpu_lock() }
    }

    #[inline]
    fn has_cpu_lock() -> bool {
        <T as raw::KernelBase>::raw_has_cpu_lock()
    }

    #[inline]
    fn boost_priority() -> Result<(), BoostPriorityError>
    where
        Self: raw::KernelBoostPriority,
    {
        <T as raw::KernelBoostPriority>::raw_boost_priority()
    }

    #[inline]
    unsafe fn unboost_priority() -> Result<(), BoostPriorityError> {
        // Safety: Just forwarding the calls
        unsafe { <T as raw::KernelBase>::raw_unboost_priority() }
    }

    #[inline]
    fn is_priority_boost_active() -> bool {
        <T as raw::KernelBase>::raw_is_priority_boost_active()
    }

    #[inline]
    fn set_time(time: Time) -> Result<(), TimeError> {
        <T as raw::KernelBase>::raw_set_time(time)
    }

    #[inline]
    fn time() -> Result<Time, TimeError>
    where
        Self: raw::KernelTime,
    {
        <T as raw::KernelTime>::raw_time()
    }

    #[inline]
    fn adjust_time(delta: Duration) -> Result<(), AdjustTimeError>
    where
        Self: raw::KernelAdjustTime,
    {
        <T as raw::KernelAdjustTime>::raw_adjust_time(delta)
    }

    #[inline]
    unsafe fn exit_task() -> Result<!, ExitTaskError> {
        // Safety: Just forwarding the calls
        unsafe { <T as raw::KernelBase>::raw_exit_task() }
    }

    #[inline]
    fn park() -> Result<(), ParkError> {
        <T as raw::KernelBase>::raw_park()
    }

    #[inline]
    fn park_timeout(timeout: Duration) -> Result<(), ParkTimeoutError> {
        <T as raw::KernelBase>::raw_park_timeout(timeout)
    }

    #[inline]
    fn sleep(duration: Duration) -> Result<(), SleepError> {
        <T as raw::KernelBase>::raw_sleep(duration)
    }
}
