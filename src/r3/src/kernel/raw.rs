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

    /// Implements [`Kernel::debug`][1].
    ///
    /// [1]: crate::kernel::Kernel::debug
    fn debug() -> Self::DebugPrinter;

    /// Implements [`Kernel::acquire_cpu_lock`][1].
    ///
    /// [1]: crate::kernel::Kernel::acquire_cpu_lock
    fn acquire_cpu_lock() -> Result<(), CpuLockError>;

    /// Implements [`Kernel::release_cpu_lock`][1].
    ///
    /// [1]: crate::kernel::Kernel::release_cpu_lock
    unsafe fn release_cpu_lock() -> Result<(), CpuLockError>;

    /// Return a flag indicating whether CPU Lock is currently active.
    fn has_cpu_lock() -> bool;

    /// Implements [`Kernel::unboost_priority`][1].
    ///
    /// [1]: crate::kernel::Kernel::unboost_priority
    unsafe fn unboost_priority() -> Result<(), BoostPriorityError>;

    /// Implements [`Kernel::is_priority_boost_active`][1].
    ///
    /// [1]: crate::kernel::Kernel::is_priority_boost_active
    fn is_priority_boost_active() -> bool;

    /// Implements [`Kernel::set_time`][1].
    ///
    /// [1]: crate::kernel::Kernel::set_time
    fn set_time(time: Time) -> Result<(), TimeError>;

    // TODO: get time resolution?

    /// Implements [`Kernel::exit_task`][1].
    ///
    /// [1]: crate::kernel::Kernel::exit_task
    unsafe fn exit_task() -> Result<!, ExitTaskError>;

    /// Implements [`Kernel::park`][1].
    ///
    /// [1]: crate::kernel::Kernel::park
    fn park() -> Result<(), ParkError>;

    /// Implements [`Kernel::park_timeout`][1].
    ///
    /// [1]: crate::kernel::Kernel::park_timeout
    fn park_timeout(timeout: Duration) -> Result<(), ParkTimeoutError>;

    /// Implements [`Kernel::sleep`][1].
    ///
    /// [1]: crate::kernel::Kernel::sleep
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
    /// Implements [`Kernel::time`][1].
    ///
    /// [1]: crate::kernel::Kernel::time
    fn time() -> Result<Time, TimeError>;
}

/// Provides the `boost_priority` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelBoostPriority: KernelBase {
    /// Implements [`Kernel::boost_priority`][1].
    ///
    /// [1]: crate::kernel::Kernel::boost_priority
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
    /// Implements [`Kernel::adjust_time`][1].
    ///
    /// [1]: crate::kernel::Kernel::adjust_time
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
