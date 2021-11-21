//! The low-level kernel interface to be implemented by a kernel implementor.
//!
//! # Safety
//!
//! Most traits in this method are `unsafe trait` because they have to be
//! trustworthy to be able to build sound memory-safety abstractions on top
//! of them.
//!
//! The trait methods that operate on a given [`Id`] are all defined as `unsafe
//! fn` to maintain object safety.
//!
use core::{fmt, hash::Hash, ops::Range};

use crate::{
    kernel::error::*,
    time::{Duration, Time},
};

/// A group of traits that must be implemented by kernel object ID types,
/// including [`KernelBase::TaskId`].
pub trait Id: fmt::Debug + Copy + Eq + Ord + Hash {}
impl<T: ?Sized + fmt::Debug + Copy + Eq + Ord + Hash> Id for T {}

/// Provides access to the minimal API exposed by a kernel.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelBase: fmt::Debug + Copy + Sized + 'static {
    type DebugPrinter: fmt::Debug + Send + Sync;

    /// The type to identify tasks.
    type TaskId: Id;

    /// Get an object that implements [`Debug`](fmt::Debug) for dumping the
    /// current kernel state.
    ///
    /// Note that printing this object might consume a large amount of stack
    /// space.
    fn debug() -> Self::DebugPrinter;

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

    /// Get the current task (i.e., the task in the Running state).
    fn task_current() -> Result<Option<Self::TaskId>, GetCurrentTaskError>;

    unsafe fn task_activate(this: Self::TaskId) -> Result<(), ActivateTaskError>;
    unsafe fn task_interrupt(this: Self::TaskId) -> Result<(), InterruptTaskError>;
    unsafe fn task_unpark_exact(this: Self::TaskId) -> Result<(), UnparkExactError>;
    unsafe fn task_priority(this: Self::TaskId) -> Result<usize, GetTaskPriorityError>;
    unsafe fn task_effective_priority(this: Self::TaskId) -> Result<usize, GetTaskPriorityError>;
}

/// Provides the `time` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelTime: KernelBase {
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
    fn time() -> Result<Time, TimeError>;
}

/// Provides the `boost_priority` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelBoostPriority: KernelBase {
    /// Activate [Priority Boost].
    ///
    /// Returns [`BadContext`] if Priority Boost is already active, the
    /// calling context is not a task context, or CPU Lock is active.
    ///
    /// [Priority Boost]: crate#system-states
    /// [`BadContext`]: CpuLockError::BadContext
    fn boost_priority() -> Result<(), BoostPriorityError>;
}

/// Provides the `task_set_priority` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelTaskSetPriority: KernelBase {
    unsafe fn task_set_priority(
        this: Self::TaskId,
        priority: usize,
    ) -> Result<(), SetTaskPriorityError>;
}

/// Provides the `adjust_time` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelAdjustTime: KernelBase {
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
    fn adjust_time(delta: Duration) -> Result<(), AdjustTimeError>;
}

// FIXME: Maybe this should be `non_exhaustive`?
/// Specifies the sorting order of a wait queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueOrder {
    /// The wait queue is processed in a FIFO order.
    Fifo,
    /// The wait queue is processed in a task priority order. Tasks with the
    /// same priorities follow a FIFO order.
    TaskPriority,
}

/// Provides access to the event group API exposed by a kernel.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelEventGroup: KernelBase {
    /// The type to identify event groups.
    type EventGroupId: Id;

    unsafe fn event_group_set(
        this: Self::EventGroupId,
        bits: EventGroupBits,
    ) -> Result<(), UpdateEventGroupError>;
    unsafe fn event_group_clear(
        this: Self::EventGroupId,
        bits: EventGroupBits,
    ) -> Result<(), UpdateEventGroupError>;
    unsafe fn event_group_get(
        this: Self::EventGroupId,
    ) -> Result<EventGroupBits, GetEventGroupError>;
    unsafe fn event_group_wait(
        this: Self::EventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError>;
    unsafe fn event_group_wait_timeout(
        this: Self::EventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
        timeout: Duration,
    ) -> Result<EventGroupBits, WaitEventGroupTimeoutError>;
    unsafe fn event_group_poll(
        this: Self::EventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, PollEventGroupError>;
}

bitflags::bitflags! {
    /// Options for [`EventGroup::wait`].
    pub struct EventGroupWaitFlags: u8 {
        /// Wait for all of the specified bits to be set.
        const ALL = 1 << 0;

        /// Clear the specified bits after waiting for them.
        const CLEAR = 1 << 1;
    }
}

// TODO: Support changing `EventGroupBits`?
/// Unsigned integer type backing event groups.
pub type EventGroupBits = u32;

/// Provides access to the mutex API exposed by a kernel.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelMutex: KernelBase {
    /// The type to identify mutexes.
    type MutexId: Id;

    unsafe fn mutex_is_locked(this: Self::MutexId) -> Result<bool, QueryMutexError>;
    unsafe fn mutex_unlock(this: Self::MutexId) -> Result<(), UnlockMutexError>;
    unsafe fn mutex_lock(this: Self::MutexId) -> Result<(), LockMutexError>;
    unsafe fn mutex_lock_timeout(
        this: Self::MutexId,
        timeout: Duration,
    ) -> Result<(), LockMutexTimeoutError>;
    unsafe fn mutex_try_lock(this: Self::MutexId) -> Result<(), TryLockMutexError>;
    unsafe fn mutex_mark_consistent(this: Self::MutexId) -> Result<(), MarkConsistentMutexError>;
}

/// Specifies the locking protocol to be followed by a [mutex].
///
/// [mutex]: crate::kernel::Mutex
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** The operating systems and operating
/// > system specifications providing an interface for specifying a mutex
/// > protocol include (but are not limited to) the following: POSIX
/// > (`pthread_mutexattr_setprotocol` and `PTHREAD_PRIO_PROTECT`, etc.), RTEMS
/// > Classic API (`RTEMS_PRIORITY_CEILING`, etc.), and μITRON4.0 (`TA_CEILING`,
/// > etc.).
///
/// <div class="admonition-follows"></div>
///
/// > **Rationale:**
/// > When this enumerate type was added, the plan was to only support the
/// > priority ceiling protocol, so having a method
/// > `CfgMutexBuilder::ceiling_priority` taking a priority ceiling value would
/// > have been simpler. Nevertheless, it was decided to use this enumerate
/// > type to accomodate other protocols in the future and to allow specifying
/// > protocol-specific parameters.
#[doc = include_str!("../common.md")]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MutexProtocol {
    /// Locking the mutex does not affect the priority of the owning task.
    None,
    /// Locking the mutex raises the effective priority of the owning task
    /// to the mutex's priority ceiling according to
    /// [the immediate priority ceiling protocol]. The inner value specifies the
    /// priority ceiling.
    ///
    /// The value must be in range `0..`[`num_task_priority_levels`].
    ///
    /// [`num_task_priority_levels`]: crate::kernel::cfg::CfgBuilder::num_task_priority_levels
    /// [the immediate priority ceiling protocol]: https://en.wikipedia.org/wiki/Priority_ceiling_protocol
    Ceiling(usize),
}

/// Provides access to the semaphore API exposed by a kernel.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelSemaphore: KernelBase {
    /// The type to identify semaphores.
    type SemaphoreId: Id;

    unsafe fn semaphore_drain(this: Self::SemaphoreId) -> Result<(), DrainSemaphoreError>;
    unsafe fn semaphore_get(this: Self::SemaphoreId) -> Result<SemaphoreValue, GetSemaphoreError>;
    unsafe fn semaphore_signal(
        this: Self::SemaphoreId,
        count: SemaphoreValue,
    ) -> Result<(), SignalSemaphoreError>;
    unsafe fn semaphore_signal_one(this: Self::SemaphoreId) -> Result<(), SignalSemaphoreError>;
    unsafe fn semaphore_wait_one(this: Self::SemaphoreId) -> Result<(), WaitSemaphoreError>;
    unsafe fn semaphore_wait_one_timeout(
        this: Self::SemaphoreId,
        timeout: Duration,
    ) -> Result<(), WaitSemaphoreTimeoutError>;
    unsafe fn semaphore_poll_one(this: Self::SemaphoreId) -> Result<(), PollSemaphoreError>;
}

/// Unsigned integer type representing the number of permits held by a
/// [semaphore].
///
/// [semaphore]: Semaphore
///
/// <div class="admonition-follows"></div>
///
/// > **Rationale:** On the one hand, using a data type with a target-dependent
/// > size can hurt portability. On the other hand, a fixed-size data type such
/// > as `u32` can significantly increase the runtime overhead on extremely
/// > constrained targets such as AVR and MSP430. In addition, many RISC targets
/// > handle small data types less efficiently. The portability issue shouldn't
/// > pose a problem in practice.
#[doc = include_str!("../common.md")]
pub type SemaphoreValue = usize;

/// Provides access to the timer API exposed by a kernel.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelTimer: KernelBase {
    /// The type to identify timers.
    type TimerId: Id;

    unsafe fn timer_start(this: Self::TimerId) -> Result<(), StartTimerError>;
    unsafe fn timer_stop(this: Self::TimerId) -> Result<(), StopTimerError>;
    unsafe fn timer_set_delay(
        this: Self::TimerId,
        delay: Option<Duration>,
    ) -> Result<(), SetTimerDelayError>;
    unsafe fn timer_set_period(
        this: Self::TimerId,
        period: Option<Duration>,
    ) -> Result<(), SetTimerPeriodError>;
}

/// Provides access to the interrupt line API exposed by a kernel.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelInterruptLine: KernelBase {
    /// The range of interrupt priority values considered [managed].
    ///
    /// Defaults to `0..0` (empty) when unspecified.
    ///
    /// [managed]: crate#interrupt-handling-framework
    #[allow(clippy::reversed_empty_ranges)] // on purpose
    const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..0;

    /// The list of interrupt lines which are considered [managed].
    ///
    /// Defaults to `&[]` (empty) when unspecified.
    ///
    /// This is useful when the driver employs a fixed priority scheme and
    /// doesn't support changing interrupt line priorities.
    ///
    /// [managed]: crate#interrupt-handling-framework
    const MANAGED_INTERRUPT_LINES: &'static [InterruptNum] = &[];

    unsafe fn interrupt_line_set_priority(
        this: InterruptNum,
        value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError>;
    unsafe fn interrupt_line_enable(this: InterruptNum) -> Result<(), EnableInterruptLineError>;
    unsafe fn interrupt_line_disable(this: InterruptNum) -> Result<(), EnableInterruptLineError>;
    unsafe fn interrupt_line_pend(this: InterruptNum) -> Result<(), PendInterruptLineError>;
    unsafe fn interrupt_line_clear(this: InterruptNum) -> Result<(), ClearInterruptLineError>;
    unsafe fn interrupt_line_is_pending(
        this: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError>;
}

/// Numeric value used to identify interrupt lines.
///
/// The meaning of this value is defined by a port and target hardware. They
/// are not necessarily tightly packed from zero.
pub type InterruptNum = usize;

/// Priority value for an interrupt line.
pub type InterruptPriority = i16;

/// A combined second-level interrupt handler.
///
/// # Safety
///
/// Only meant to be called from a first-level interrupt handler. CPU Lock must
/// be inactive.
pub type InterruptHandlerFn = unsafe extern "C" fn();
