#![feature(const_maybe_uninit_array_assume_init)]
#![feature(const_maybe_uninit_uninit_array)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(const_slice_from_raw_parts)]
#![feature(maybe_uninit_uninit_array)]
#![feature(const_precise_live_drops)]
#![feature(const_raw_ptr_comparison)]
#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(const_intrinsic_copy)]
#![feature(exhaustive_patterns)] // `let Ok(()) = Ok::<(), !>(())`
#![feature(generic_const_exprs)]
#![feature(const_refs_to_cell)]
#![feature(maybe_uninit_slice)]
#![feature(const_slice_index)]
#![feature(const_option_ext)]
#![feature(const_trait_impl)]
#![feature(const_ptr_write)]
#![feature(core_intrinsics)]
#![feature(specialization)]
#![feature(assert_matches)]
#![feature(const_mut_refs)]
#![feature(const_ptr_read)]
#![feature(const_convert)]
#![feature(const_option)]
#![feature(const_deref)]
#![feature(const_heap)]
#![feature(const_swap)]
#![feature(never_type)] // `!`
#![feature(decl_macro)]
#![feature(let_else)]
#![feature(doc_cfg)] // `#[doc(cfg(...))]`
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(
    feature = "doc",
    doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")
)]
#![doc = include_str!("./lib.md")]
#![doc = include_str!("./common.md")]
#![doc = include!("../doc/traits.rs")] // `![traits]`
#![cfg_attr(
    feature = "_full",
    doc = r#"<style type="text/css">.disabled-feature-warning { display: none; }</style>"#
)]
#![cfg_attr(not(test), no_std)] // Link `std` only when building a test (`cfg(test)`)

// `array_item_from_fn!` requires `MaybeUninit`.
#[doc(hidden)]
pub extern crate core;

// `build!` requires `ArrayVec`
#[doc(hidden)]
pub extern crate arrayvec;

// `build!` requires `r3_core`
#[doc(hidden)]
pub extern crate r3_core;

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

pub mod utils;

#[cfg(feature = "priority_boost")]
use core::sync::atomic::{AtomicBool, Ordering};
use core::{fmt, marker::PhantomData, mem::forget, num::NonZeroUsize, ops::Range};

use r3_core::{
    kernel::{
        cfg::{DelegateKernelStatic, KernelStatic},
        raw,
    },
    time::{Duration, Time},
    utils::Init,
};

use crate::utils::{binary_heap::VecLike, BinUInteger};

#[macro_use]
pub mod cfg;
mod error;
mod event_group;
mod interrupt;
mod klock;
mod mutex;
mod semaphore;
mod state;
mod task;
mod timeout;
mod timer;
mod wait;

// Some of these re-exports are for our macros, the others are really public
pub use {event_group::*, interrupt::*, mutex::*, semaphore::*, task::*, timeout::*, timer::*};

/// Numeric value used to identify various kinds of kernel objects.
pub type Id = NonZeroUsize;

/// Wraps a provided [trait type][1] `Traits` to instantiate a kernel. This type
/// implements the traits from [`r3_core::kernel::raw`], making it usable as a
/// kernel, if `Traits` implements some appropriate traits, which consequently
/// make it implement [`KernelTraits`].
///
/// [1]: crate#kernel-trait-type
pub struct System<Traits>(PhantomData<Traits>);

impl<Traits> Clone for System<Traits> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Traits> Copy for System<Traits> {}

impl<Traits> core::fmt::Debug for System<Traits> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("System")
    }
}

/// The instantiation of [`r3_core::kernel::Cfg`] used by [`build!`] to configure
/// a kernel. `CfgBuilder<...>` in this alias Implements
/// [`~const`]` `[`raw_cfg::CfgBase`]`<`[`System`]`<Traits>>` and many other
/// `raw_cfg` traits.
///
/// [`~const`]: https://github.com/rust-lang/rust/issues/77463
/// [`raw_cfg::CfgBase`]: r3_core::kernel::raw_cfg::CfgBase
pub type Cfg<'c, Traits> = r3_core::kernel::Cfg<'c, cfg::CfgBuilder<Traits>>;

/// Represents a complete [kernel trait type][1].
///
/// [1]: crate#the-kernrel-trait-type
pub trait KernelTraits: Port + KernelCfg2 + KernelStatic<System<Self>> + 'static {}

impl<Traits: Port + KernelCfg2 + KernelStatic<System<Self>> + 'static> KernelTraits for Traits {}

/// Implement `KernelStatic<System<Traits>>` on `System<Traits>` if the same
/// trait is implemented on `Traits`.
impl<Traits: KernelStatic<System<Traits>>> DelegateKernelStatic<System<Traits>> for System<Traits> {
    type Target = Traits;
}

unsafe impl<Traits: KernelTraits> raw::KernelBase for System<Traits> {
    const RAW_SUPPORTED_QUEUE_ORDERS: &'static [Option<raw::QueueOrderKind>] = &[
        Some(raw::QueueOrderKind::Fifo),
        Some(raw::QueueOrderKind::TaskPriority),
    ];

    #[inline]
    fn raw_acquire_cpu_lock() -> Result<(), r3_core::kernel::CpuLockError> {
        // Safety: `try_enter_cpu_lock` is only meant to be called by
        //         the kernel
        if unsafe { Traits::try_enter_cpu_lock() } {
            Ok(())
        } else {
            Err(r3_core::kernel::CpuLockError::BadContext)
        }
    }

    #[inline]
    unsafe fn raw_release_cpu_lock() -> Result<(), r3_core::kernel::CpuLockError> {
        if !Traits::is_cpu_lock_active() {
            Err(r3_core::kernel::CpuLockError::BadContext)
        } else {
            // Safety: CPU Lock active
            unsafe { Traits::leave_cpu_lock() };
            Ok(())
        }
    }

    #[inline]
    fn raw_has_cpu_lock() -> bool {
        Traits::is_cpu_lock_active()
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_unboost_priority() -> Result<(), r3_core::kernel::BoostPriorityError> {
        state::unboost_priority::<Traits>()
    }

    #[inline]
    #[cfg(feature = "priority_boost")]
    fn raw_is_priority_boost_active() -> bool {
        Traits::state().priority_boost.load(Ordering::Relaxed)
    }

    #[inline]
    #[cfg(not(feature = "priority_boost"))]
    fn raw_is_priority_boost_active() -> bool {
        false
    }

    #[inline]
    fn raw_is_task_context() -> bool {
        Traits::is_task_context()
    }

    #[inline]
    fn raw_is_interrupt_context() -> bool {
        Traits::is_interrupt_context()
    }

    #[inline]
    fn raw_is_boot_complete() -> bool {
        Traits::is_scheduler_active()
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_set_time(time: Time) -> Result<(), r3_core::kernel::TimeError> {
        timeout::set_system_time::<Traits>(time)
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_exit_task() -> Result<!, r3_core::kernel::ExitTaskError> {
        // Safety: Just forwarding the function call
        unsafe { task::exit_current_task::<Traits>() }
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_park() -> Result<(), r3_core::kernel::ParkError> {
        task::park_current_task::<Traits>()
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_park_timeout(timeout: Duration) -> Result<(), r3_core::kernel::ParkTimeoutError> {
        task::park_current_task_timeout::<Traits>(timeout)
    }
    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_sleep(timeout: Duration) -> Result<(), r3_core::kernel::SleepError> {
        task::put_current_task_on_sleep_timeout::<Traits>(timeout)
    }

    type RawDebugPrinter = KernelDebugPrinter<Traits>;

    /// Get an object that implements [`Debug`](fmt::Debug) for dumping the
    /// current kernel state.
    ///
    /// Note that printing this object might consume a large amount of stack
    /// space.
    #[inline]
    fn raw_debug() -> Self::RawDebugPrinter {
        KernelDebugPrinter(PhantomData)
    }

    type RawTaskId = task::TaskId;

    #[inline]
    fn raw_task_current() -> Result<Self::RawTaskId, r3_core::kernel::GetCurrentTaskError> {
        Self::task_current()
    }

    #[inline]
    unsafe fn raw_task_activate(
        this: Self::RawTaskId,
    ) -> Result<(), r3_core::kernel::ActivateTaskError> {
        Self::task_activate(this)
    }

    #[inline]
    unsafe fn raw_task_interrupt(
        this: Self::RawTaskId,
    ) -> Result<(), r3_core::kernel::InterruptTaskError> {
        Self::task_interrupt(this)
    }

    #[inline]
    unsafe fn raw_task_unpark_exact(
        this: Self::RawTaskId,
    ) -> Result<(), r3_core::kernel::UnparkExactError> {
        Self::task_unpark_exact(this)
    }

    #[inline]
    unsafe fn raw_task_priority(
        this: Self::RawTaskId,
    ) -> Result<usize, r3_core::kernel::GetTaskPriorityError> {
        Self::task_priority(this)
    }

    #[inline]
    unsafe fn raw_task_effective_priority(
        this: Self::RawTaskId,
    ) -> Result<usize, r3_core::kernel::GetTaskPriorityError> {
        Self::task_effective_priority(this)
    }
}

unsafe impl<Traits: KernelTraits> raw::KernelTaskSetPriority for System<Traits> {
    #[inline]
    unsafe fn raw_task_set_priority(
        this: Self::RawTaskId,
        priority: usize,
    ) -> Result<(), r3_core::kernel::SetTaskPriorityError> {
        Self::task_set_priority(this, priority)
    }
}

#[cfg(feature = "priority_boost")]
#[doc(cfg(feature = "priority_boost"))]
unsafe impl<Traits: KernelTraits> raw::KernelBoostPriority for System<Traits> {
    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_boost_priority() -> Result<(), r3_core::kernel::BoostPriorityError> {
        state::boost_priority::<Traits>()
    }
}

#[cfg(feature = "system_time")]
#[doc(cfg(feature = "system_time"))]
unsafe impl<Traits: KernelTraits> raw::KernelTime for System<Traits> {
    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_time() -> Result<Time, r3_core::kernel::TimeError> {
        timeout::system_time::<Traits>()
    }
}

unsafe impl<Traits: KernelTraits> raw::KernelAdjustTime for System<Traits> {
    const RAW_TIME_USER_HEADROOM: Duration = TIME_USER_HEADROOM;

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    fn raw_adjust_time(delta: Duration) -> Result<(), r3_core::kernel::AdjustTimeError> {
        timeout::adjust_system_and_event_time::<Traits>(delta)
    }
}

/// The object returned by `<`[`System`]` as `[`KernelBase`]`>::debug`.
/// Implements [`fmt::Debug`].
///
/// **This type is exempt from the API stability guarantee.**
///
/// [`KernelBase`]: r3_core::kernel::raw::KernelBase
pub struct KernelDebugPrinter<T>(PhantomData<T>);

impl<T: KernelTraits> fmt::Debug for KernelDebugPrinter<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct PoolPrinter<'a, T>(&'a [T]);

        impl<T: fmt::Debug> fmt::Debug for PoolPrinter<'_, T> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                // dictionary-style printing with key = object ID, value = object
                f.debug_map().entries(self.0.iter().enumerate()).finish()
            }
        }

        f.debug_struct("Kernel")
            .field("state", T::state())
            .field("task_cb_pool", &PoolPrinter(T::task_cb_pool()))
            .field(
                "event_group_cb_pool",
                &PoolPrinter(T::event_group_cb_pool()),
            )
            .field("mutex_cb_pool", &PoolPrinter(T::mutex_cb_pool()))
            .field("semaphore_cb_pool", &PoolPrinter(T::semaphore_cb_pool()))
            .field("timer_cb_pool", &PoolPrinter(T::timer_cb_pool()))
            .finish()
    }
}

/// Associates a kernel trait type with kernel-private data. Use [`build!`] to
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

    /// Task ready queue type.
    #[doc(hidden)]
    type TaskReadyQueue: readyqueue::Queue<Self>;

    /// Convert `usize` to [`Self::TaskPriority`][]. Returns `None` if
    /// `i >= Self::NUM_TASK_PRIORITY_LEVELS`.
    fn to_task_priority(i: usize) -> Option<Self::TaskPriority>;
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
#[doc = include_str!("./common.md")]
#[allow(clippy::missing_safety_doc)]
pub unsafe trait PortThreading: KernelCfg1 + KernelStatic<System<Self>> {
    type PortTaskState: Send + Sync + Init + fmt::Debug + 'static;

    /// The initial value of [`TaskCb::port_task_state`] for all tasks.
    #[allow(clippy::declare_interior_mutable_const)] // it's intentional
    const PORT_TASK_STATE_INIT: Self::PortTaskState;

    /// The default stack size for tasks.
    const STACK_DEFAULT_SIZE: usize = 1024;

    /// The alignment requirement for task stack regions.
    ///
    /// Both ends of stack regions are aligned by `STACK_ALIGN`. It's
    /// automatically enforced by the kernel configurator for automatically
    /// allocated stack regions (this applies to tasks created without
    /// [`StackHunk`]). The kernel configurator does not check the alignemnt
    /// for manually-allocated stack regions.
    ///
    /// [`StackHunk`]: r3_core::kernel::task::StackHunk
    const STACK_ALIGN: usize = core::mem::size_of::<usize>();

    /// Transfer the control to the dispatcher, discarding the current
    /// (startup) context. `*state.`[`running_task_ptr`]`()` is `None` at this
    /// point. The dispatcher should call [`PortToKernel::choose_running_task`]
    /// to find the next task to run and transfer the control to that task.
    ///
    /// Precondition: CPU Lock active, a boot context
    ///
    /// [`running_task_ptr`]: State::running_task_ptr
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

    /// Destroy the state of the previously running task (`task`, which has
    /// already been removed from `*state.`[`running_task_ptr`]`()`) and proceed
    /// to the dispatcher.
    ///
    /// Precondition: CPU Lock active
    ///
    /// [`running_task_ptr`]: State::running_task_ptr
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

    /// Activate CPU Lock. Return `true` iff CPU Lock was inactive before the
    /// call.
    unsafe fn try_enter_cpu_lock() -> bool {
        if Self::is_cpu_lock_active() {
            false
        } else {
            // Safety: CPU Lock inactive
            unsafe { Self::enter_cpu_lock() };
            true
        }
    }

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
    /// [a task context].
    ///
    /// [a task context]: r3_core#contexts
    fn is_task_context() -> bool;

    /// Return a flag indicating whether the current context is
    /// [an interrupt context].
    ///
    /// [an interrupt context]: r3_core#contexts
    fn is_interrupt_context() -> bool;

    /// Return a flag indicating whether [`Self::dispatch_first_task`][] was
    /// called.
    fn is_scheduler_active() -> bool;
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
#[doc = include_str!("./common.md")]
#[allow(clippy::missing_safety_doc)]
pub unsafe trait PortInterrupts: KernelCfg1 {
    /// The range of interrupt priority values considered [managed].
    ///
    /// Defaults to `0..0` (empty) when unspecified.
    ///
    /// [managed]: crate#interrupt-handling-framework
    #[allow(clippy::reversed_empty_ranges)] // on purpose
    const MANAGED_INTERRUPT_PRIORITY_RANGE: Range<raw::InterruptPriority> = 0..0;

    /// The list of interrupt lines which are considered [managed].
    ///
    /// Defaults to `&[]` (empty) when unspecified.
    ///
    /// This is useful when the driver employs a fixed priority scheme and
    /// doesn't support changing interrupt line priorities.
    ///
    /// [managed]: crate#interrupt-handling-framework
    const MANAGED_INTERRUPT_LINES: &'static [raw::InterruptNum] = &[];

    /// Set the priority of the specified interrupt line.
    ///
    /// Precondition: CPU Lock active. Task context or boot phase.
    unsafe fn set_interrupt_line_priority(
        _line: raw::InterruptNum,
        _priority: raw::InterruptPriority,
    ) -> Result<(), r3_core::kernel::SetInterruptLinePriorityError> {
        Err(r3_core::kernel::SetInterruptLinePriorityError::NotSupported)
    }

    /// Enable the specified interrupt line.
    unsafe fn enable_interrupt_line(
        _line: raw::InterruptNum,
    ) -> Result<(), r3_core::kernel::EnableInterruptLineError> {
        Err(r3_core::kernel::EnableInterruptLineError::NotSupported)
    }

    /// Disable the specified interrupt line.
    unsafe fn disable_interrupt_line(
        _line: raw::InterruptNum,
    ) -> Result<(), r3_core::kernel::EnableInterruptLineError> {
        Err(r3_core::kernel::EnableInterruptLineError::NotSupported)
    }

    /// Set the pending flag of the specified interrupt line.
    unsafe fn pend_interrupt_line(
        _line: raw::InterruptNum,
    ) -> Result<(), r3_core::kernel::PendInterruptLineError> {
        Err(r3_core::kernel::PendInterruptLineError::NotSupported)
    }

    /// Clear the pending flag of the specified interrupt line.
    unsafe fn clear_interrupt_line(
        _line: raw::InterruptNum,
    ) -> Result<(), r3_core::kernel::ClearInterruptLineError> {
        Err(r3_core::kernel::ClearInterruptLineError::NotSupported)
    }

    /// Read the pending flag of the specified interrupt line.
    unsafe fn is_interrupt_line_pending(
        _line: raw::InterruptNum,
    ) -> Result<bool, r3_core::kernel::QueryInterruptLineError> {
        Err(r3_core::kernel::QueryInterruptLineError::NotSupported)
    }
}

/// Implemented by a port. This trait contains items related to controlling
/// a system timer.
///
/// # Safety
///
/// These methods are only meant to be called by the kernel.
#[doc = include_str!("./common.md")]
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
    /// This is ignored and can take any value if `pend_tick_after` is
    /// implemented as no-op.
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

    /// Determine the next task to run and store it in [`State::running_task_ptr`].
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
    /// This method will call `pend_tick` or `pend_tick_after`.
    ///
    /// [a port timer driver]: PortTimer
    /// [`pend_tick_after`]: PortTimer::pend_tick_after
    ///
    /// Precondition: CPU Lock inactive, an interrupt context
    unsafe fn timer_tick();
}

impl<Traits: KernelTraits> PortToKernel for Traits {
    #[inline(always)]
    unsafe fn boot() -> ! {
        let mut lock = unsafe { klock::assume_cpu_lock::<Traits>() };

        // Initialize all tasks
        for cb in Traits::task_cb_pool() {
            task::init_task(lock.borrow_mut(), cb);
        }

        // Initialize the timekeeping system
        Traits::state().timeout.init(lock.borrow_mut());

        for cb in Traits::timer_cb_pool() {
            timer::init_timer(lock.borrow_mut(), cb);
        }

        // Initialize all interrupt lines
        // Safety: The contents of `INTERRUPT_ATTR` has been generated and
        // verified by `panic_if_unmanaged_safety_is_violated` for *unsafe
        // safety*. Thus the use of unmanaged priority values has been already
        // authorized.
        unsafe {
            Traits::INTERRUPT_ATTR.init(lock.borrow_mut());
        }

        // Call startup hooks
        // Safety: This is the intended place to call startup hooks.
        unsafe { (Traits::STARTUP_HOOK)() };

        forget(lock);

        // Safety: CPU Lock is active, Startup phase
        unsafe {
            Traits::dispatch_first_task();
        }
    }

    #[inline(always)]
    unsafe fn choose_running_task() {
        // Safety: The precondition of this method includes CPU Lock being
        // active
        let mut lock = unsafe { klock::assume_cpu_lock::<Traits>() };

        task::choose_next_running_task(lock.borrow_mut());

        // Post-condition: CPU Lock active
        forget(lock);
    }

    #[inline(always)]
    unsafe fn timer_tick() {
        timeout::handle_tick::<Traits>();
    }
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg2: Port + Sized + KernelStatic<System<Self>> {
    // Most associated items are hidden because they have no use outside the
    // kernel. The rest is not hidden because it's meant to be accessed by port
    // code.
    #[doc(hidden)]
    type TimeoutHeap: VecLike<Element = timeout::TimeoutRef<Self>> + Init + fmt::Debug + 'static;

    /// The table of combined second-level interrupt handlers.
    ///
    /// A port should generate first-level interrupt handlers that call them.
    const INTERRUPT_HANDLERS: &'static cfg::InterruptHandlerTable;

    #[doc(hidden)]
    const INTERRUPT_ATTR: InterruptAttr<Self>;

    /// The startup hook set through `CfgBase`. The `unsafe`-ness ensures it's
    /// not called by code non grata.
    #[doc(hidden)]
    const STARTUP_HOOK: unsafe fn();

    /// Access the kernel's global state.
    fn state() -> &'static State<Self>;

    #[doc(hidden)]
    fn hunk_pool_ptr() -> *mut u8;

    // This can't be `const` because of [ref:const_static_item_ref]
    #[doc(hidden)]
    fn task_cb_pool() -> &'static [TaskCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_task_cb(i: usize) -> Option<&'static TaskCb<Self>> {
        Self::task_cb_pool().get(i)
    }

    // This can't be `const` because of [ref:const_static_item_ref]
    #[doc(hidden)]
    fn event_group_cb_pool() -> &'static [EventGroupCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_event_group_cb(i: usize) -> Option<&'static EventGroupCb<Self>> {
        Self::event_group_cb_pool().get(i)
    }

    // This can't be `const` because of [ref:const_static_item_ref]
    #[doc(hidden)]
    fn mutex_cb_pool() -> &'static [MutexCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_mutex_cb(i: usize) -> Option<&'static MutexCb<Self>> {
        Self::mutex_cb_pool().get(i)
    }

    // This can't be `const` because of [ref:const_static_item_ref]
    #[doc(hidden)]
    fn semaphore_cb_pool() -> &'static [SemaphoreCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_semaphore_cb(i: usize) -> Option<&'static SemaphoreCb<Self>> {
        Self::semaphore_cb_pool().get(i)
    }

    // This can't be `const` because of [ref:const_static_item_ref]
    #[doc(hidden)]
    fn timer_cb_pool() -> &'static [TimerCb<Self>];

    #[doc(hidden)]
    #[inline(always)]
    fn get_timer_cb(i: usize) -> Option<&'static TimerCb<Self>> {
        Self::timer_cb_pool().get(i)
    }
}

/// Global kernel state.
pub struct State<
    Traits: KernelCfg2,
    PortTaskState: 'static = <Traits as PortThreading>::PortTaskState,
    TaskReadyQueue: 'static = <Traits as KernelCfg1>::TaskReadyQueue,
    TaskPriority: 'static = <Traits as KernelCfg1>::TaskPriority,
    TimeoutHeap: 'static = <Traits as KernelCfg2>::TimeoutHeap,
> {
    /// The currently or recently running task. Can be in a Running, Waiting, or
    /// Ready state. The last two only can be observed momentarily around a
    /// call to `yield_cpu` or in an interrupt handler.
    ///
    /// It must refer to an element of [`KernelCfg2::task_cb_pool`].
    running_task:
        klock::CpuLockCell<Traits, Option<&'static TaskCb<Traits, PortTaskState, TaskPriority>>>,

    /// The task ready queue.
    task_ready_queue: TaskReadyQueue,

    #[cfg(feature = "priority_boost")]
    /// `true` if Priority Boost is active.
    priority_boost: AtomicBool,

    /// The global state of the timekeeping system.
    timeout: timeout::TimeoutGlobals<Traits, TimeoutHeap>,
}

impl<
        Traits: KernelCfg2,
        PortTaskState: 'static,
        TaskReadyQueue: 'static + Init,
        TaskPriority: 'static,
        TimeoutHeap: 'static + Init,
    > Init for State<Traits, PortTaskState, TaskReadyQueue, TaskPriority, TimeoutHeap>
{
    const INIT: Self = Self {
        running_task: klock::CpuLockCell::new(None),
        task_ready_queue: Init::INIT,
        #[cfg(feature = "priority_boost")]
        priority_boost: AtomicBool::new(false),
        timeout: Init::INIT,
    };
}

impl<
        Traits: KernelTraits,
        PortTaskState: 'static + fmt::Debug,
        TaskReadyQueue: 'static + fmt::Debug,
        TaskPriority: 'static + fmt::Debug,
        TimeoutHeap: 'static + fmt::Debug,
    > fmt::Debug for State<Traits, PortTaskState, TaskReadyQueue, TaskPriority, TimeoutHeap>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("State")
            .field("running_task", &self.running_task.get_and_debug_fmt())
            .field("task_ready_queue", &self.task_ready_queue)
            .field(
                "priority_boost",
                match () {
                    #[cfg(feature = "priority_boost")]
                    () => &self.priority_boost,
                    #[cfg(not(feature = "priority_boost"))]
                    () => &(),
                },
            )
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl<Traits: KernelCfg2> State<Traits> {
    /// Get the currently running task.
    #[inline]
    fn running_task(
        &self,
        lock: klock::CpuLockTokenRefMut<Traits>,
    ) -> Option<&'static TaskCb<Traits>> {
        *self.running_task.read(&*lock)
    }

    /// Get a pointer to the variable storing the currently running task.
    ///
    /// Reading the variable is safe as long as the read is free of data race.
    /// Note that only the dispatcher (that calls
    /// [`PortToKernel::choose_running_task`]) can modify the variable
    /// asynchonously. For example, it's safe to read it in a task context. It's
    /// also safe to read it in the dispatcher. On the other hand, reading it in
    /// a non-task context (except for the dispatcher, of course) may lead to
    /// an undefined behavior unless CPU Lock is activated while reading the
    /// variable.
    ///
    /// Writing the variable is not allowed.
    #[inline]
    pub fn running_task_ptr(&self) -> *mut Option<&'static TaskCb<Traits>> {
        self.running_task.as_ptr()
    }
}

/// Report the use of an invalid ID, which is defined to be UB for this is a
/// violation of [object safety][].
///
/// # Safety
///
/// This function should only be called when an invalid ID is provided by a
/// caller. Under the object safety rules, we are allowed to cause an undefined
/// behavior in such cases.
///
/// [object safety]: r3_core#object-safety
#[inline]
unsafe fn bad_id<Traits: KernelCfg2>() -> error::NoAccessError {
    // TODO: Support returning `NoAccess`
    let _ = error::NoAccessError::NoAccess;
    if cfg!(debug_assertion) {
        panic!("invalid kernel object ID");
    } else {
        // Safety: The caller ensures this function is never reached
        unsafe { core::hint::unreachable_unchecked() }
    }
}
