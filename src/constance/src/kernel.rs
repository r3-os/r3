//! The RTOS kernel
use atomic_ref::AtomicRef;
use core::{
    borrow::BorrowMut,
    fmt,
    mem::forget,
    num::NonZeroUsize,
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    time::{Duration, Time},
    utils::{binary_heap::VecLike, intrusive_list::StaticListHead, BinUInteger, Init, PrioBitmap},
};

#[macro_use]
pub mod cfg;
mod error;
mod event_group;
mod hunk;
mod interrupt;
mod startup;
mod state;
mod task;
mod timeout;
mod utils;
mod wait;
pub use self::{
    error::*, event_group::*, hunk::*, interrupt::*, startup::*, task::*, timeout::*, wait::*,
};

/// Numeric value used to identify various kinds of kernel objects.
pub type Id = NonZeroUsize;

/// Provides access to the global API functions exposed by the kernel.
///
/// This trait is automatically implemented on "system" types that have
/// sufficient trait `impl`s to instantiate the kernel.
#[doc(include = "./common.md")]
pub trait Kernel: Port + KernelCfg2 + Sized + 'static {
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
    fn boost_priority() -> Result<(), BoostPriorityError>;

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

    #[cfg_attr(doc, svgbobdoc::transform)]
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

impl<T: Port + KernelCfg2 + 'static> Kernel for T {
    fn acquire_cpu_lock() -> Result<(), CpuLockError> {
        if Self::is_cpu_lock_active() {
            Err(CpuLockError::BadContext)
        } else {
            // Safety: CPU Lock inactive
            unsafe { Self::enter_cpu_lock() };
            Ok(())
        }
    }

    unsafe fn release_cpu_lock() -> Result<(), CpuLockError> {
        if !Self::is_cpu_lock_active() {
            Err(CpuLockError::BadContext)
        } else {
            // Safety: CPU Lock active
            unsafe { Self::leave_cpu_lock() };
            Ok(())
        }
    }

    fn has_cpu_lock() -> bool {
        Self::is_cpu_lock_active()
    }

    fn boost_priority() -> Result<(), BoostPriorityError> {
        state::boost_priority::<Self>()
    }

    unsafe fn unboost_priority() -> Result<(), BoostPriorityError> {
        state::unboost_priority::<Self>()
    }

    fn is_priority_boost_active() -> bool {
        Self::state().priority_boost.load(Ordering::Relaxed)
    }

    fn time() -> Result<Time, TimeError> {
        timeout::system_time::<Self>()
    }
    fn set_time(time: Time) -> Result<(), TimeError> {
        timeout::set_system_time::<Self>(time)
    }
    fn adjust_time(delta: Duration) -> Result<(), AdjustTimeError> {
        timeout::adjust_system_and_event_time::<Self>(delta)
    }

    unsafe fn exit_task() -> Result<!, ExitTaskError> {
        // Safety: Just forwarding the function call
        unsafe { exit_current_task::<Self>() }
    }

    fn park() -> Result<(), ParkError> {
        task::park_current_task::<Self>()
    }

    fn park_timeout(timeout: Duration) -> Result<(), ParkTimeoutError> {
        task::park_current_task_timeout::<Self>(timeout)
    }
    fn sleep(timeout: Duration) -> Result<(), SleepError> {
        task::put_current_task_on_sleep_timeout::<Self>(timeout)
    }
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// Customizable things needed by both of `Port` and `KernelCfg2` should live
/// here because `Port` cannot refer to an associated item defined by
/// `KernelCfg2`.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg1: Sized + Send + Sync + 'static {
    /// The number of task priority levels.
    const NUM_TASK_PRIORITY_LEVELS: usize;

    /// Unsigned integer type capable of representing the range
    /// `0..NUM_TASK_PRIORITY_LEVELS`.
    type TaskPriority: BinUInteger;

    // FIXME: This is a work-around for trait methods being uncallable in `const fn`
    //        <https://github.com/rust-lang/rfcs/pull/2632>
    //        <https://github.com/rust-lang/const-eval/pull/8>
    /// All possible values of `TaskPriority`.
    ///
    /// `TASK_PRIORITY_LEVELS[i]` is equivalent to
    /// `TaskPriority::try_from(i).unwrap()` except that the latter doesn't work
    /// in `const fn`.
    const TASK_PRIORITY_LEVELS: &'static [Self::TaskPriority];
}

/// Implemented by a port. This trait contains items related to low-level
/// operations for controlling CPU states and context switching.
///
/// # Safety
///
/// Implementing a port is inherently unsafe because it's responsible for
/// initializing the execution environment and providing a dispatcher
/// implementation.
///
/// These methods are only meant to be called by the kernel.
#[doc(include = "./common.md")]
#[allow(clippy::missing_safety_doc)]
pub unsafe trait PortThreading: KernelCfg1 {
    type PortTaskState: Send + Sync + Init + 'static;

    /// The initial value of [`TaskCb::port_task_state`] for all tasks.
    #[allow(clippy::declare_interior_mutable_const)] // it's intentional
    const PORT_TASK_STATE_INIT: Self::PortTaskState;

    /// The default stack size for tasks.
    const STACK_DEFAULT_SIZE: usize = 1024;

    /// The alignment requirement for task stack regions.
    const STACK_ALIGN: usize = core::mem::size_of::<usize>();

    /// Transfer the control to [`State::running_task`], discarding the current
    /// (startup) context.
    ///
    /// Precondition: CPU Lock active, Startup phase
    unsafe fn dispatch_first_task() -> !;

    /// Yield the processor.
    ///
    /// In a task context, this method immediately transfers the control to
    /// a dispatcher. The dispatcher should call
    /// [`PortToKernel::choose_running_task`] to find the next task to run and
    /// transfer the control to that task.
    ///
    /// In an interrupt context, the effect of this method will be deferred
    /// until the processor completes the execution of all active interrupt
    /// handler threads.
    ///
    /// Precondition: CPU Lock inactive
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Port Implementation Note:** One way to handle the interrupt context
    /// > case is to set a flag variable and check it in the epilogue of a
    /// > first-level interrupt handler. Another way is to raise a low-priority
    /// > interrupt (such as PendSV in Arm-M) and implement dispatching in the
    /// > handler.
    unsafe fn yield_cpu();

    /// Destroy the state of the previously running task (`task`, which might
    /// already have been removed from [`State::running_task`]) and proceed to
    /// the dispatcher.
    ///
    /// Precondition: CPU Lock active
    unsafe fn exit_and_dispatch(task: &'static task::TaskCb<Self>) -> !;

    /// Disable all kernel-managed interrupts (this state is called *CPU Lock*).
    ///
    /// Precondition: CPU Lock inactive
    unsafe fn enter_cpu_lock();

    /// Re-enable kernel-managed interrupts previously disabled by
    /// `enter_cpu_lock`, thus deactivating the CPU Lock state.
    ///
    /// Precondition: CPU Lock active
    unsafe fn leave_cpu_lock();

    /// Prepare the task for activation. More specifically, set the current
    /// program counter to [`TaskAttr::entry_point`] and the current stack
    /// pointer to either end of [`TaskAttr::stack`], ensuring the task will
    /// start execution from `entry_point` next time the task receives the
    /// control.
    ///
    /// Do not call this for a running task. Calling this for a dormant task is
    /// always safe. For tasks in other states, whether this method is safe is
    /// dependent on how the programming language the task code is written in
    /// is implemented. In particular, this is unsafe for Rust task code because
    /// it might violate the requirement of [`Pin`] if there's a `Pin` pointing
    /// to something on the task's stack.
    ///
    /// [`Pin`]: core::pin::Pin
    ///
    /// Precondition: CPU Lock active
    unsafe fn initialize_task_state(task: &'static task::TaskCb<Self>);

    /// Return a flag indicating whether a CPU Lock state is active.
    fn is_cpu_lock_active() -> bool;

    /// Return a flag indicating whether the current context is
    /// [an task context].
    ///
    /// [an task context]: crate#contexts
    fn is_task_context() -> bool;
}

/// Implemented by a port. This trait contains items related to controlling
/// interrupt lines.
///
/// # Safety
///
/// Implementing a port is inherently unsafe because it's responsible for
/// initializing the execution environment and providing a dispatcher
/// implementation.
///
/// These methods are only meant to be called by the kernel.
#[doc(include = "./common.md")]
#[allow(clippy::missing_safety_doc)]
pub unsafe trait PortInterrupts: KernelCfg1 {
    /// The range of interrupt priority values considered [managed].
    ///
    /// Defaults to `0..0` (empty) when unspecified.
    ///
    /// [managed]: crate#interrupt-handling-framework
    #[allow(clippy::reversed_empty_ranges)] // on purpose
    const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<InterruptPriority> = 0..0;

    /// Set the priority of the specified interrupt line.
    ///
    /// Precondition: CPU Lock active. Task context or boot phase.
    unsafe fn set_interrupt_line_priority(
        _line: InterruptNum,
        _priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        Err(SetInterruptLinePriorityError::NotSupported)
    }

    /// Enable the specified interrupt line.
    unsafe fn enable_interrupt_line(_line: InterruptNum) -> Result<(), EnableInterruptLineError> {
        Err(EnableInterruptLineError::NotSupported)
    }

    /// Disable the specified interrupt line.
    unsafe fn disable_interrupt_line(_line: InterruptNum) -> Result<(), EnableInterruptLineError> {
        Err(EnableInterruptLineError::NotSupported)
    }

    /// Set the pending flag of the specified interrupt line.
    unsafe fn pend_interrupt_line(_line: InterruptNum) -> Result<(), PendInterruptLineError> {
        Err(PendInterruptLineError::NotSupported)
    }

    /// Clear the pending flag of the specified interrupt line.
    unsafe fn clear_interrupt_line(_line: InterruptNum) -> Result<(), ClearInterruptLineError> {
        Err(ClearInterruptLineError::NotSupported)
    }

    /// Read the pending flag of the specified interrupt line.
    unsafe fn is_interrupt_line_pending(
        _line: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        Err(QueryInterruptLineError::NotSupported)
    }
}

/// Implemented by a port. This trait contains items related to controlling
/// a system timer.
///
/// # Safety
///
/// These methods are only meant to be called by the kernel.
#[doc(include = "./common.md")]
#[allow(clippy::missing_safety_doc)]
pub trait PortTimer {
    /// The maximum value that [`tick_count`] can return. Must be greater
    /// than zero.
    ///
    /// [`tick_count`]: Self::tick_count
    const MAX_TICK_COUNT: UTicks;

    /// The maximum value that can be passed to [`pend_tick_after`]. Must be
    /// greater than zero.
    ///
    /// This value should be somewhat smaller than `MAX_TICK_COUNT`. The
    /// difference determines the kernel's resilience against overdue
    /// timer interrupts.
    ///
    /// [`pend_tick_after`]: Self::pend_tick_after
    const MAX_TIMEOUT: UTicks;

    /// Read the current tick count (timer value).
    ///
    /// This value steadily increases over time. When it goes past
    /// `MAX_TICK_COUNT`, it “wraps around” to `0`.
    ///
    /// The returned value must be in range `0..=`[`MAX_TICK_COUNT`].
    ///
    /// Precondition: CPU Lock active
    ///
    /// [`MAX_TICK_COUNT`]: Self::MAX_TICK_COUNT
    unsafe fn tick_count() -> UTicks;

    /// Indicate that `tick_count_delta` ticks may elapse before the kernel
    /// should receive a call to [`PortToKernel::timer_tick`].
    ///
    /// “`tick_count_delta` ticks” include the current (ongoing) tick. For
    /// example, `tick_count_delta == 1` means `timer_tick` should be
    /// preferably called right after the next tick boundary.
    ///
    /// The driver might track time in a coarser granularity than microseconds.
    /// In this case, the driver should wait until the earliest moment when
    /// `tick_count() >= current_tick_count + tick_count_delta` (where
    /// `current_tick_count` is the current value of `tick_count()`; not taking
    /// the wrap-around behavior into account) is fulfilled and call
    /// `timer_tick`.
    ///
    /// It's legal to ignore the calls to this method entirely and call
    /// `timer_tick` at a steady rate, resulting in something similar to a
    /// “tickful” kernel. The default implementation does nothing assuming that
    /// the port driver is implemented in this way.
    ///
    /// `tick_count_delta` must be in range `1..=`[`MAX_TIMEOUT`].
    ///
    /// Precondition: CPU Lock active
    ///
    /// [`MAX_TIMEOUT`]: Self::MAX_TIMEOUT
    unsafe fn pend_tick_after(tick_count_delta: UTicks) {
        let _ = tick_count_delta;
    }

    /// Pend a call to [`PortToKernel::timer_tick`] as soon as possible.
    ///
    /// The default implementation calls `pend_tick_after(1)`.
    ///
    /// Precondition: CPU Lock active
    unsafe fn pend_tick() {
        unsafe { Self::pend_tick_after(1) };
    }
}

/// Unsigned integer type representing a tick count used by
/// [a port timer driver]. The period of each tick is fixed at one microsecond.
///
/// [a port timer driver]: PortTimer
pub type UTicks = u32;

/// Represents a particular group of traits that a port should implement.
pub trait Port: PortThreading + PortInterrupts + PortTimer {}

impl<T: PortThreading + PortInterrupts + PortTimer> Port for T {}

/// Methods intended to be called by a port.
///
/// # Safety
///
/// These are only meant to be called by the port.
#[allow(clippy::missing_safety_doc)]
pub trait PortToKernel {
    /// Initialize runtime structures.
    ///
    /// Should be called for exactly once by the port before calling into any
    /// user (application) or kernel code.
    ///
    /// Precondition: CPU Lock active, Preboot phase
    // TODO: Explain phases
    unsafe fn boot() -> !;

    /// Determine the next task to run and store it in [`State::running_task_ref`].
    ///
    /// Precondition: CPU Lock active / Postcondition: CPU Lock active
    unsafe fn choose_running_task();

    /// Called by [a port timer driver] to “announce” new ticks.
    ///
    /// This method can be called anytime, but the driver is expected to attempt
    /// to ensure the calls occur near tick boundaries. For an optimal
    /// operation, the driver should implement [`pend_tick_after`] and handle
    /// the calls made by the kernel to figure out the optimal moment to call
    /// `timer_tick`.
    ///
    /// [a port timer driver]: PortTimer
    /// [`pend_tick_after`]: PortTimer::pend_tick_after
    ///
    /// Precondition: CPU Lock inactive, an interrupt context
    unsafe fn timer_tick();
}

impl<System: Kernel> PortToKernel for System {
    unsafe fn boot() -> ! {
        let mut lock = unsafe { utils::assume_cpu_lock::<Self>() };

        // Safety: (1) User code hasn't executed yet at this point. (2) The
        // creator of this `HunkAttr` is responsible for creating a valid
        // instance of `HunkAttr`.
        unsafe {
            System::HUNK_ATTR.init_hunks();
        }

        // Initialize all tasks
        for cb in Self::task_cb_pool() {
            task::init_task(lock.borrow_mut(), cb);
        }

        // Choose the first `runnnig_task`
        task::choose_next_running_task(lock.borrow_mut());

        // Initialize the timekeeping system
        System::state().timeout.init(lock.borrow_mut());

        // Initialize all interrupt lines
        // Safety: The contents of `INTERRUPT_ATTR` has been generated and
        // verified by `panic_if_unmanaged_safety_is_violated` for *unsafe
        // safety*. Thus the use of unmanaged priority values has been already
        // authorized.
        unsafe {
            System::INTERRUPT_ATTR.init(lock.borrow_mut());
        }

        // Release CPU Lock
        drop(lock);

        // Call startup hooks
        for hook in Self::STARTUP_HOOKS {
            // Safety: This is the intended place to call startup hooks.
            unsafe { (hook.start)(hook.param) };
        }

        // Reacquire CPU Lock
        let mut _lock = utils::lock_cpu::<System>().unwrap();

        // Safety: CPU Lock is active, Startup phase
        unsafe {
            Self::dispatch_first_task();
        }
    }

    unsafe fn choose_running_task() {
        // Safety: The precondition of this method includes CPU Lock being
        // active
        let mut lock = unsafe { utils::assume_cpu_lock::<Self>() };

        task::choose_next_running_task(lock.borrow_mut());

        // Post-condition: CPU Lock active
        forget(lock);
    }

    unsafe fn timer_tick() {
        timeout::handle_tick::<Self>();
    }
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg2: Port + Sized {
    // Most associated items are hidden because they have no use outside the
    // kernel. The rest is not hidden because it's meant to be accessed by port
    // code.
    #[doc(hidden)]
    const HUNK_ATTR: HunkAttr;

    #[doc(hidden)]
    type TaskReadyBitmap: PrioBitmap;

    #[doc(hidden)]
    type TaskReadyQueue: BorrowMut<[StaticListHead<TaskCb<Self>>]> + Init + 'static;

    #[doc(hiddden)]
    type TimeoutHeap: VecLike<Element = timeout::TimeoutRef<Self>> + Init + 'static;

    /// The table of combined second-level interrupt handlers.
    ///
    /// A port should generate first-level interrupt handlers that call them.
    const INTERRUPT_HANDLERS: &'static cfg::InterruptHandlerTable;

    #[doc(hidden)]
    const INTERRUPT_ATTR: InterruptAttr<Self>;

    #[doc(hidden)]
    const STARTUP_HOOKS: &'static [StartupHookAttr];

    /// Access the kernel's global state.
    fn state() -> &'static State<Self>;

    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/11>
    //        to be resolved because `TaskCb` includes interior mutability
    //        and can't be referred to by `const`
    #[doc(hidden)]
    fn task_cb_pool() -> &'static [TaskCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_task_cb(i: usize) -> Option<&'static TaskCb<Self>> {
        Self::task_cb_pool().get(i)
    }

    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/11>
    //        to be resolved because `EventGroupCb` includes interior mutability
    //        and can't be referred to by `const`
    #[doc(hidden)]
    fn event_group_cb_pool() -> &'static [EventGroupCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_event_group_cb(i: usize) -> Option<&'static EventGroupCb<Self>> {
        Self::event_group_cb_pool().get(i)
    }
}

/// Global kernel state.
pub struct State<
    System: KernelCfg2,
    PortTaskState: 'static = <System as PortThreading>::PortTaskState,
    TaskReadyBitmap: 'static = <System as KernelCfg2>::TaskReadyBitmap,
    TaskReadyQueue: 'static = <System as KernelCfg2>::TaskReadyQueue,
    TaskPriority: 'static = <System as KernelCfg1>::TaskPriority,
    TimeoutHeap: 'static = <System as KernelCfg2>::TimeoutHeap,
> {
    // TODO: Make `running_task` non-null to simplify runtime code
    /// The currently or recently running task. Can be in a Running or Waiting
    /// state.
    running_task: AtomicRef<'static, TaskCb<System, PortTaskState, TaskPriority>>,

    /// The task ready bitmap, in which each bit indicates whether the
    /// task ready queue corresponding to that bit contains a task or not.
    task_ready_bitmap: utils::CpuLockCell<System, TaskReadyBitmap>,

    /// The task ready queues, in which each queue represents the list of
    /// Ready tasks at the corresponding priority level.
    ///
    /// Invariant: `task_ready_bitmap[i].first.is_some() ==
    ///  task_ready_queue.get(i)`
    task_ready_queue: utils::CpuLockCell<System, TaskReadyQueue>,

    /// `true` if Priority Boost is active.
    priority_boost: AtomicBool,

    /// The global state of the timekeeping system.
    timeout: timeout::TimeoutGlobals<System, TimeoutHeap>,
}

impl<
        System: KernelCfg2,
        PortTaskState: 'static,
        TaskReadyBitmap: 'static + Init,
        TaskReadyQueue: 'static + Init,
        TaskPriority: 'static,
        TimeoutHeap: 'static + Init,
    > Init
    for State<System, PortTaskState, TaskReadyBitmap, TaskReadyQueue, TaskPriority, TimeoutHeap>
{
    const INIT: Self = Self {
        running_task: AtomicRef::new(None),
        task_ready_bitmap: Init::INIT,
        task_ready_queue: Init::INIT,
        priority_boost: AtomicBool::new(false),
        timeout: Init::INIT,
    };
}

impl<
        System: Kernel,
        PortTaskState: 'static + fmt::Debug,
        TaskReadyBitmap: 'static + fmt::Debug,
        TaskReadyQueue: 'static + fmt::Debug,
        TaskPriority: 'static + fmt::Debug,
        TimeoutHeap: 'static + fmt::Debug,
    > fmt::Debug
    for State<System, PortTaskState, TaskReadyBitmap, TaskReadyQueue, TaskPriority, TimeoutHeap>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("running_task", &self.running_task)
            .field("task_ready_bitmap", &self.task_ready_bitmap)
            .field("task_ready_queue", &self.task_ready_queue)
            .field("priority_boost", &self.priority_boost)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl<System: KernelCfg2> State<System> {
    /// Get the currently running task.
    pub fn running_task(&self) -> Option<&'static TaskCb<System>> {
        self.running_task.load(Ordering::Relaxed)
    }

    /// Get a reference to the variable storing the currently running task.
    ///
    /// # Safety
    ///
    /// Modifying the stored value is not allowed.
    pub unsafe fn running_task_ref(&self) -> &AtomicRef<'static, TaskCb<System>> {
        &self.running_task
    }
}
