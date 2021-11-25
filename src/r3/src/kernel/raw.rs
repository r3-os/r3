//! The low-level kernel interface to be implemented by a kernel implementor.
//!
//! # Safety
//!
//! Most traits in this method are `unsafe trait` because they have to be
//! trustworthy to be able to build sound memory-safety-critical abstractions on
//! top of them.
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
    type RawDebugPrinter: fmt::Debug + Send + Sync;

    /// The type to identify tasks.
    type RawTaskId: Id;

    /// Implements [`Kernel::debug`][1].
    ///
    /// [1]: crate::kernel::Kernel::debug
    fn raw_debug() -> Self::RawDebugPrinter;

    /// Implements [`Kernel::acquire_cpu_lock`][1].
    ///
    /// [1]: crate::kernel::Kernel::acquire_cpu_lock
    fn raw_acquire_cpu_lock() -> Result<(), CpuLockError>;

    /// Implements [`Kernel::release_cpu_lock`][1].
    ///
    /// [1]: crate::kernel::Kernel::release_cpu_lock
    unsafe fn raw_release_cpu_lock() -> Result<(), CpuLockError>;

    /// Return a flag indicating whether CPU Lock is currently active.
    fn raw_has_cpu_lock() -> bool;

    /// Implements [`Kernel::unboost_priority`][1].
    ///
    /// [1]: crate::kernel::Kernel::unboost_priority
    unsafe fn raw_unboost_priority() -> Result<(), BoostPriorityError>;

    /// Implements [`Kernel::is_priority_boost_active`][1].
    ///
    /// [1]: crate::kernel::Kernel::is_priority_boost_active
    fn raw_is_priority_boost_active() -> bool;

    /// Implements [`Kernel::set_time`][1].
    ///
    /// [1]: crate::kernel::Kernel::set_time
    fn raw_set_time(time: Time) -> Result<(), TimeError>;

    // TODO: get time resolution?

    /// Implements [`Kernel::exit_task`][1].
    ///
    /// [1]: crate::kernel::Kernel::exit_task
    unsafe fn raw_exit_task() -> Result<!, ExitTaskError>;

    /// Implements [`Kernel::park`][1].
    ///
    /// [1]: crate::kernel::Kernel::park
    fn raw_park() -> Result<(), ParkError>;

    /// Implements [`Kernel::park_timeout`][1].
    ///
    /// [1]: crate::kernel::Kernel::park_timeout
    fn raw_park_timeout(timeout: Duration) -> Result<(), ParkTimeoutError>;

    /// Implements [`Kernel::sleep`][1].
    ///
    /// [1]: crate::kernel::Kernel::sleep
    fn raw_sleep(duration: Duration) -> Result<(), SleepError>;

    /// Get the current task (i.e., the task in the Running state).
    fn raw_task_current() -> Result<Option<Self::RawTaskId>, GetCurrentTaskError>;

    /// Implements [`Task::activate`][1].
    ///
    /// [1]: crate::kernel::Task::activate
    unsafe fn raw_task_activate(this: Self::RawTaskId) -> Result<(), ActivateTaskError>;

    /// Implements [`Task::interrupt`][1].
    ///
    /// [1]: crate::kernel::Task::interrupt
    unsafe fn raw_task_interrupt(this: Self::RawTaskId) -> Result<(), InterruptTaskError>;

    /// Implements [`Task::unpark_exact`][1].
    ///
    /// [1]: crate::kernel::Task::unpark_exact
    unsafe fn raw_task_unpark_exact(this: Self::RawTaskId) -> Result<(), UnparkExactError>;

    /// Implements [`Task::priority`][1].
    ///
    /// [1]: crate::kernel::Task::priority
    unsafe fn raw_task_priority(this: Self::RawTaskId) -> Result<usize, GetTaskPriorityError>;

    /// Implements [`Task::effective_priority`][1].
    ///
    /// [1]: crate::kernel::Task::effective_priority
    unsafe fn raw_task_effective_priority(
        this: Self::RawTaskId,
    ) -> Result<usize, GetTaskPriorityError>;
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
    fn raw_time() -> Result<Time, TimeError>;
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
    fn raw_boost_priority() -> Result<(), BoostPriorityError>;
}

/// Provides the `task_set_priority` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelTaskSetPriority: KernelBase {
    /// Implements [`Task::set_priority`][1].
    ///
    /// [1]: crate::kernel::Task::set_priority
    unsafe fn raw_task_set_priority(
        this: Self::RawTaskId,
        priority: usize,
    ) -> Result<(), SetTaskPriorityError>;
}

/// Provides the `adjust_time` method.
///
/// # Safety
///
/// See [the module documentation](self).
pub unsafe trait KernelAdjustTime: KernelBase {
    /// Implements [`Kernel::time_user_headroom`][1].
    ///
    /// [1]: crate::kernel::Kernel::time_user_headroom
    const RAW_TIME_USER_HEADROOM: Duration = Duration::from_secs(1);

    /// Implements [`Kernel::adjust_time`][1].
    ///
    /// [1]: crate::kernel::Kernel::adjust_time
    fn raw_adjust_time(delta: Duration) -> Result<(), AdjustTimeError>;
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
    type RawEventGroupId: Id;

    /// Implements [`EventGroup::set`][1].
    ///
    /// [1]: crate::kernel::EventGroup::set
    unsafe fn raw_event_group_set(
        this: Self::RawEventGroupId,
        bits: EventGroupBits,
    ) -> Result<(), UpdateEventGroupError>;

    /// Implements [`EventGroup::clear`][1].
    ///
    /// [1]: crate::kernel::EventGroup::clear
    unsafe fn raw_event_group_clear(
        this: Self::RawEventGroupId,
        bits: EventGroupBits,
    ) -> Result<(), UpdateEventGroupError>;

    /// Implements [`EventGroup::get`][1].
    ///
    /// [1]: crate::kernel::EventGroup::get
    unsafe fn raw_event_group_get(
        this: Self::RawEventGroupId,
    ) -> Result<EventGroupBits, GetEventGroupError>;

    /// Implements [`EventGroup::wait`][1].
    ///
    /// [1]: crate::kernel::EventGroup::wait
    unsafe fn raw_event_group_wait(
        this: Self::RawEventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
    ) -> Result<EventGroupBits, WaitEventGroupError>;

    /// Implements [`EventGroup::wait_timeout`][1].
    ///
    /// [1]: crate::kernel::EventGroup::wait_timeout
    unsafe fn raw_event_group_wait_timeout(
        this: Self::RawEventGroupId,
        bits: EventGroupBits,
        flags: EventGroupWaitFlags,
        timeout: Duration,
    ) -> Result<EventGroupBits, WaitEventGroupTimeoutError>;

    /// Implements [`EventGroup::poll`][1].
    ///
    /// [1]: crate::kernel::EventGroup::poll
    unsafe fn raw_event_group_poll(
        this: Self::RawEventGroupId,
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
    type RawMutexId: Id;

    /// Implements [`Mutex::is_locked`][1].
    ///
    /// [1]: crate::kernel::Mutex::is_locked
    unsafe fn raw_mutex_is_locked(this: Self::RawMutexId) -> Result<bool, QueryMutexError>;

    /// Implements [`Mutex::unlock`][1].
    ///
    /// [1]: crate::kernel::Mutex::unlock
    unsafe fn raw_mutex_unlock(this: Self::RawMutexId) -> Result<(), UnlockMutexError>;

    /// Implements [`Mutex::lock`][1].
    ///
    /// [1]: crate::kernel::Mutex::lock
    unsafe fn raw_mutex_lock(this: Self::RawMutexId) -> Result<(), LockMutexError>;

    /// Implements [`Mutex::lock_timeout`][1].
    ///
    /// [1]: crate::kernel::Mutex::lock_timeout
    unsafe fn raw_mutex_lock_timeout(
        this: Self::RawMutexId,
        timeout: Duration,
    ) -> Result<(), LockMutexTimeoutError>;

    /// Implements [`Mutex::try_lock`][1].
    ///
    /// [1]: crate::kernel::Mutex::try_lock
    unsafe fn raw_mutex_try_lock(this: Self::RawMutexId) -> Result<(), TryLockMutexError>;

    /// Implements [`Mutex::mark_consistent`][1].
    ///
    /// [1]: crate::kernel::Mutex::mark_consistent
    unsafe fn raw_mutex_mark_consistent(
        this: Self::RawMutexId,
    ) -> Result<(), MarkConsistentMutexError>;
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
    type RawSemaphoreId: Id;

    /// Implements [`Semaphore::drain`][1].
    ///
    /// [1]: crate::kernel::Semaphore::drain
    unsafe fn raw_semaphore_drain(this: Self::RawSemaphoreId) -> Result<(), DrainSemaphoreError>;

    /// Implements [`Semaphore::get`][1].
    ///
    /// [1]: crate::kernel::Semaphore::get
    unsafe fn raw_semaphore_get(
        this: Self::RawSemaphoreId,
    ) -> Result<SemaphoreValue, GetSemaphoreError>;

    /// Implements [`Semaphore::signal`][1].
    ///
    /// [1]: crate::kernel::Semaphore::signal
    unsafe fn raw_semaphore_signal(
        this: Self::RawSemaphoreId,
        count: SemaphoreValue,
    ) -> Result<(), SignalSemaphoreError>;

    /// Implements [`Semaphore::signal_one`][1].
    ///
    /// [1]: crate::kernel::Semaphore::signal_one
    unsafe fn raw_semaphore_signal_one(
        this: Self::RawSemaphoreId,
    ) -> Result<(), SignalSemaphoreError>;

    /// Implements [`Semaphore::wait_one`][1].
    ///
    /// [1]: crate::kernel::Semaphore::wait_one
    unsafe fn raw_semaphore_wait_one(this: Self::RawSemaphoreId) -> Result<(), WaitSemaphoreError>;

    /// Implements [`Semaphore::wait_one_timeout`][1].
    ///
    /// [1]: crate::kernel::Semaphore::wait_one_timeout
    unsafe fn raw_semaphore_wait_one_timeout(
        this: Self::RawSemaphoreId,
        timeout: Duration,
    ) -> Result<(), WaitSemaphoreTimeoutError>;

    /// Implements [`Semaphore::poll_one`][1].
    ///
    /// [1]: crate::kernel::Semaphore::poll_one
    unsafe fn raw_semaphore_poll_one(this: Self::RawSemaphoreId) -> Result<(), PollSemaphoreError>;
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
    type RawTimerId: Id;

    /// Implements [`Timer::start`][1].
    ///
    /// [1]: crate::kernel::Timer::start
    unsafe fn raw_timer_start(this: Self::RawTimerId) -> Result<(), StartTimerError>;

    /// Implements [`Timer::stop`][1].
    ///
    /// [1]: crate::kernel::Timer::stop
    unsafe fn raw_timer_stop(this: Self::RawTimerId) -> Result<(), StopTimerError>;

    /// Implements [`Timer::set_delay`][1].
    ///
    /// [1]: crate::kernel::Timer::set_delay
    unsafe fn raw_timer_set_delay(
        this: Self::RawTimerId,
        delay: Option<Duration>,
    ) -> Result<(), SetTimerDelayError>;

    /// Implements [`Timer::set_period`][1].
    ///
    /// [1]: crate::kernel::Timer::set_period
    unsafe fn raw_timer_set_period(
        this: Self::RawTimerId,
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
    const RAW_MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..0;

    /// The list of interrupt lines which are considered [managed].
    ///
    /// Defaults to `&[]` (empty) when unspecified.
    ///
    /// This is useful when the driver employs a fixed priority scheme and
    /// doesn't support changing interrupt line priorities.
    ///
    /// [managed]: crate#interrupt-handling-framework
    const RAW_MANAGED_INTERRUPT_LINES: &'static [InterruptNum] = &[];

    /// Implements [`InterruptLine::set_priority`][1].
    ///
    /// [1]: crate::kernel::InterruptLine::set_priority
    unsafe fn raw_interrupt_line_set_priority(
        this: InterruptNum,
        value: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError>;

    /// Implements [`InterruptLine::enable`][1].
    ///
    /// [1]: crate::kernel::InterruptLine::enable
    unsafe fn raw_interrupt_line_enable(this: InterruptNum)
        -> Result<(), EnableInterruptLineError>;

    /// Implements [`InterruptLine::disable`][1].
    ///
    /// [1]: crate::kernel::InterruptLine::disable
    unsafe fn raw_interrupt_line_disable(
        this: InterruptNum,
    ) -> Result<(), EnableInterruptLineError>;

    /// Implements [`InterruptLine::pend`][1].
    ///
    /// [1]: crate::kernel::InterruptLine::pend
    unsafe fn raw_interrupt_line_pend(this: InterruptNum) -> Result<(), PendInterruptLineError>;

    /// Implements [`InterruptLine::clear`][1].
    ///
    /// [1]: crate::kernel::InterruptLine::clear
    unsafe fn raw_interrupt_line_clear(this: InterruptNum) -> Result<(), ClearInterruptLineError>;

    /// Implements [`InterruptLine::is_pending`][1].
    ///
    /// [1]: crate::kernel::InterruptLine::is_pending
    unsafe fn raw_interrupt_line_is_pending(
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
