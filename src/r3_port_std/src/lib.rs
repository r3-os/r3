#![feature(cfg_target_has_atomic)] // `#[cfg(target_has_atomic_load_store)]`
#![feature(thread_local)]
#![feature(deadline_api)]
#![feature(once_cell)]
#![cfg_attr(
    feature = "doc",
    doc(html_logo_url = "https://r3-os.github.io/r3/logo-small.svg")
)]
#![doc = include_str!("./lib.md")]
#![deny(unsafe_op_in_unsafe_fn)]
use atomic_ref::AtomicRef;
use r3_core::{
    kernel::{
        ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        Kernel as _, PendInterruptLineError, QueryInterruptLineError,
        SetInterruptLinePriorityError,
    },
    prelude::*,
};
use r3_kernel::{KernelTraits, Port, PortToKernel, System, TaskCb, UTicks};
use spin::Mutex as SpinMutex;
use std::{
    cell::Cell,
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};

#[cfg(doc)]
#[doc = include_str!("../CHANGELOG.md")]
pub mod _changelog_ {}

#[cfg(unix)]
#[path = "threading_unix.rs"]
mod threading;
#[cfg(windows)]
#[path = "threading_windows.rs"]
mod threading;
#[cfg(not(any(unix, windows)))]
#[path = "unsupported.rs"]
mod threading;

#[cfg(test)]
mod threading_test;

mod sched;
mod ums;
mod utils;

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate r3_core_ks as r3_core;
/// Used by `use_port!`
#[doc(hidden)]
pub extern crate r3_kernel;
/// Used by `use_port!`
#[doc(hidden)]
pub use std::sync::atomic::{AtomicBool, Ordering};
/// Used by `use_port!`
#[doc(hidden)]
pub extern crate env_logger;

/// The number of interrupt lines. The valid range of interrupt numbers is
/// defined as `0..NUM_INTERRUPT_LINES`
pub const NUM_INTERRUPT_LINES: usize = 1024;

/// The (software) interrupt line used for dispatching.
pub const INTERRUPT_LINE_DISPATCH: InterruptNum = 1023;

/// The default interrupt priority for [`INTERRUPT_LINE_DISPATCH`].
pub const INTERRUPT_PRIORITY_DISPATCH: InterruptPriority = 16384;

/// The (software) interrupt line used for timer interrupts.
pub const INTERRUPT_LINE_TIMER: InterruptNum = 1022;

/// The default interrupt priority for [`INTERRUPT_LINE_TIMER`].
pub const INTERRUPT_PRIORITY_TIMER: InterruptPriority = 16383;

/// Implemented on a kernel trait type by [`use_port!`].
///
/// # Safety
///
/// Only meant to be implemented by [`use_port!`].
#[doc(hidden)]
pub unsafe trait PortInstance:
    KernelTraits + Port<PortTaskState = TaskState> + PortToKernel
{
    fn port_state() -> &'static State;
}

/// The internal state of the port.
///
/// # Safety
///
/// For the safety information of this type's methods, see the documentation of
/// the corresponding trait methods of `Port*`.
#[doc(hidden)]
pub struct State {
    thread_group: OnceLock<ums::ThreadGroup<sched::SchedState>>,
    timer_cmd_send: SpinMutex<Option<mpsc::Sender<TimerCmd>>>,
    origin: AtomicRef<'static, Instant>,
}

#[derive(Debug)]
pub struct TaskState {
    /// The task's state in the task state machine.
    ///
    /// This field is expected to be accessed with CPU Lock or a scheduler lock,
    /// so `SpinMutex` is sufficient (no real mutexes are necessary). It could be
    /// even `UnsafeCell`, but we'd like to avoid unsafe code whenever possible.
    /// The runtime performance is not a concern in `r3_port_std`.
    tsm: SpinMutex<Tsm>,
}

impl Init for TaskState {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self::new();
}

/// Task state machine
///
/// These don't exactly align with the task states defined in the kernel.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Tsm {
    /// The task's context state is not initialized. The kernel has to call
    /// `initialize_task_state` first before choosing this task as `running_task`.
    Uninit,
    /// The task's context state is initialized but hasn't started running.
    Dormant,
    /// The task is currently running.
    Running(ums::ThreadId),
}

enum TimerCmd {
    SetTimeout { at: Instant },
}

/// The role of a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThreadRole {
    Unknown,
    Boot,
    /// The backing thread for an interrupt context.
    Interrupt,
    /// The backing thread for a task.
    Task,
}

thread_local! {
    /// The current thread's role. It's automatically assigned after the
    /// creation of a thread managed by the port.
    static THREAD_ROLE: Cell<ThreadRole> = Cell::new(ThreadRole::Unknown);
}

impl TaskState {
    pub const fn new() -> Self {
        Self {
            tsm: SpinMutex::new(Tsm::Uninit),
        }
    }

    fn assert_current_thread(&self) {
        // `self` must represent the current thread
        let expected_thread_id = match &*self.tsm.lock() {
            Tsm::Running(thread_id) => *thread_id,
            _ => unreachable!(),
        };
        assert_eq!(ums::current_thread(), Some(expected_thread_id));
    }

    unsafe fn exit_and_dispatch<Traits: PortInstance>(&self, state: &'static State) -> ! {
        log::trace!("exit_and_dispatch({self:p}) enter");
        self.assert_current_thread();

        let mut lock = state.thread_group.get().unwrap().lock();

        // Dissociate this thread from the task.
        let thread_id = match std::mem::replace(&mut *self.tsm.lock(), Tsm::Uninit) {
            Tsm::Running(thread_id) => thread_id,
            _ => unreachable!(),
        };

        // Make sure this thread will run to completion.
        //
        // Running all threads to completion is a prerequisite for a clean
        // shutdown. Shutdown will not complete if there are running threads.
        //
        // At this point, the thread is already dissociated from the task, so
        // the kernel will never choose this task again. However, the underlying
        // UMS thread is still alive. Thus, we need to temporarily override the
        // normal scheduling to ensure this thread will run to completion.
        lock.scheduler().recycle_thread(thread_id);
        lock.scheduler().cpu_lock = false;
        drop(lock);

        // Invoke the dispatcher
        unsafe { state.yield_cpu::<Traits>() };

        log::trace!("exit_and_dispatch({self:p}) calling exit_thread");
        unsafe { ums::exit_thread() };
    }
}

#[allow(clippy::missing_safety_doc)]
impl State {
    pub const fn new() -> Self {
        Self {
            thread_group: OnceLock::new(),
            timer_cmd_send: SpinMutex::new(None),
            origin: AtomicRef::new(None),
        }
    }

    /// Initialize the user-mode scheduling system and boot the kernel.
    ///
    /// Returns when the shutdown initiated by [`shutdown`] completes.
    pub fn port_boot<Traits: PortInstance>(&self) {
        // Create a UMS thread group.
        let (thread_group, join_handle) = ums::ThreadGroup::new(sched::SchedState::new::<Traits>());

        self.thread_group.set(thread_group).ok().unwrap();

        // Start a timer thread
        let (timer_cmd_send, timer_cmd_recv) = mpsc::channel();
        log::trace!("starting the timer thread");
        let timer_join_handle = std::thread::spawn(move || {
            let mut next_deadline = None;
            loop {
                let recv_result = if let Some(next_deadline) = next_deadline {
                    timer_cmd_recv.recv_deadline(next_deadline)
                } else {
                    timer_cmd_recv
                        .recv()
                        .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                };
                match recv_result {
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        pend_interrupt_line::<Traits>(INTERRUPT_LINE_TIMER).unwrap();
                        next_deadline = None;
                    }
                    Ok(TimerCmd::SetTimeout { at }) => {
                        next_deadline = Some(at);
                    }
                }
            }
        });
        *self.timer_cmd_send.lock() = Some(timer_cmd_send);

        // Create the initial UMS worker thread, where the boot phase of the
        // kernel runs
        let mut lock = self.thread_group.get().unwrap().lock();
        let thread_id = lock.spawn(|_| {
            THREAD_ROLE.with(|role| role.set(ThreadRole::Boot));

            // Safety: We are a port, so it's okay to call this
            unsafe {
                <Traits as PortToKernel>::boot();
            }
        });
        log::trace!("startup thread = {thread_id:?}");
        lock.scheduler().task_thread = Some(thread_id);
        lock.scheduler().recycle_thread(thread_id);
        lock.preempt();

        // Configure timer interrupt
        lock.scheduler()
            .update_line(INTERRUPT_LINE_TIMER, |line| {
                line.priority = INTERRUPT_PRIORITY_TIMER;
                line.enable = true;
                line.start = Some(Self::timer_handler::<Traits>);
            })
            .ok()
            .unwrap();

        drop(lock);

        // Wait until the thread group shuts down
        let result = join_handle.join();

        // Stop the timer thread.
        // `timer_cmd_recv.recv` will return `Err(_)` when we drop the
        // corresponding sender (`timer_cmd_send`).
        log::trace!("stopping the timer thread");
        *self.timer_cmd_send.lock() = None;
        timer_join_handle.join().unwrap();
        log::trace!("stopped the timer thread");

        // Propagate any panic that occured in a worker thread
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    pub unsafe fn dispatch_first_task<Traits: PortInstance>(&'static self) -> ! {
        log::trace!("dispatch_first_task");
        assert_eq!(expect_worker_thread(), ThreadRole::Boot);
        assert!(self.is_cpu_lock_active::<Traits>());

        // Create a UMS worker thread for the dispatcher
        let mut lock = self.thread_group.get().unwrap().lock();

        // Configure PendSV
        // TODO: move this (except for `pended = true`) to `port_boot`
        lock.scheduler()
            .update_line(INTERRUPT_LINE_DISPATCH, |line| {
                line.priority = INTERRUPT_PRIORITY_DISPATCH;
                line.enable = true;
                line.pended = true;
                line.start = Some(Self::dispatch_handler::<Traits>);
            })
            .ok()
            .unwrap();

        lock.scheduler().cpu_lock = false;

        // Start scheduling
        assert!(sched::check_preemption_by_interrupt(
            self.thread_group.get().unwrap(),
            &mut lock
        ));
        drop(lock);

        // Safety: The requirement of `dispatch_first_task` explicitly allows
        // discarding the context.
        unsafe { ums::exit_thread() };
    }

    extern "C" fn dispatch_handler<Traits: PortInstance>() {
        Traits::port_state().dispatch::<Traits>();
    }

    fn dispatch<Traits: PortInstance>(&'static self) {
        assert_eq!(expect_worker_thread(), ThreadRole::Interrupt);

        unsafe { self.enter_cpu_lock::<Traits>() };
        unsafe { Traits::choose_running_task() };
        unsafe { self.leave_cpu_lock::<Traits>() };

        let mut lock = self.thread_group.get().unwrap().lock();

        // Tell the scheduler which task to run next
        // Safety: `running_task` is only modified by `choose_running_task`, so
        //         there's no data race
        let running_task = unsafe { *Traits::state().running_task_ptr() };
        lock.scheduler().task_thread = if let Some(task) = running_task {
            log::trace!("dispatching task {task:p}");

            let mut tsm = task.port_task_state.tsm.lock();

            match &*tsm {
                Tsm::Dormant => {
                    // Spawn a UMS worker thread for this task
                    let thread = lock.spawn(move |_| {
                        THREAD_ROLE.with(|role| role.set(ThreadRole::Task));
                        assert!(!self.is_cpu_lock_active::<Traits>());

                        log::debug!("task {task:p} is now running");

                        // Safety: The port can call this
                        unsafe {
                            (task.attr.entry_point)(task.attr.entry_param);
                        }

                        // Safety: To my knowledge, we have nothing on the
                        // current thread' stack which are unsafe to
                        // `forget`. (`libstd`'s thread entry point might
                        // not be prepared to this, though...)
                        unsafe {
                            System::<Traits>::exit_task().unwrap();
                        }
                    });

                    log::trace!("spawned thread {thread:?} for the task {task:p}");

                    *tsm = Tsm::Running(thread);
                    Some(thread)
                }
                Tsm::Running(thread_id) => Some(*thread_id),
                Tsm::Uninit => unreachable!(),
            }
        } else {
            None
        };
    }

    pub unsafe fn yield_cpu<Traits: PortInstance>(&'static self) {
        log::trace!("yield_cpu");
        expect_worker_thread();
        assert!(!self.is_cpu_lock_active::<Traits>());

        self.pend_interrupt_line::<Traits>(INTERRUPT_LINE_DISPATCH)
            .unwrap();
    }

    pub unsafe fn exit_and_dispatch<Traits: PortInstance>(
        &'static self,
        task: &'static TaskCb<Traits>,
    ) -> ! {
        log::trace!("exit_and_dispatch");
        assert_eq!(expect_worker_thread(), ThreadRole::Task);
        assert!(self.is_cpu_lock_active::<Traits>());

        unsafe {
            task.port_task_state.exit_and_dispatch::<Traits>(self);
        }
    }

    pub unsafe fn enter_cpu_lock<Traits: PortInstance>(&self) {
        log::trace!("enter_cpu_lock");
        expect_worker_thread();

        let mut lock = self.thread_group.get().unwrap().lock();
        assert!(!lock.scheduler().cpu_lock);
        lock.scheduler().cpu_lock = true;
    }

    pub unsafe fn leave_cpu_lock<Traits: PortInstance>(&'static self) {
        log::trace!("leave_cpu_lock");
        expect_worker_thread();

        let mut lock = self.thread_group.get().unwrap().lock();
        assert!(lock.scheduler().cpu_lock);
        lock.scheduler().cpu_lock = false;

        if sched::check_preemption_by_interrupt(self.thread_group.get().unwrap(), &mut lock) {
            drop(lock);
            ums::yield_now();
        }
    }

    pub unsafe fn initialize_task_state<Traits: PortInstance>(
        &self,
        task: &'static TaskCb<Traits>,
    ) {
        log::trace!("initialize_task_state {task:p}");
        expect_worker_thread();
        assert!(self.is_cpu_lock_active::<Traits>());

        let pts = &task.port_task_state;
        let mut tsm = pts.tsm.lock();
        match &*tsm {
            Tsm::Dormant => {}
            Tsm::Running(_) => {
                todo!("terminating a thread is not implemented yet");
            }
            Tsm::Uninit => {
                *tsm = Tsm::Dormant;
            }
        }
    }

    pub fn is_cpu_lock_active<Traits: PortInstance>(&self) -> bool {
        expect_worker_thread();

        (self.thread_group.get().unwrap().lock())
            .scheduler()
            .cpu_lock
    }

    pub fn is_task_context<Traits: PortInstance>(&self) -> bool {
        expect_worker_thread();

        THREAD_ROLE.with(|role| match role.get() {
            ThreadRole::Interrupt | ThreadRole::Boot => false,
            ThreadRole::Task => true,
            _ => panic!("`is_task_context` was called from an unknown thread"),
        })
    }

    pub fn is_interrupt_context<Traits: PortInstance>(&self) -> bool {
        expect_worker_thread();

        THREAD_ROLE.with(|role| match role.get() {
            ThreadRole::Task | ThreadRole::Boot => false,
            ThreadRole::Interrupt => true,
            _ => panic!("`is_task_context` was called from an unknown thread"),
        })
    }

    pub fn is_scheduler_active<Traits: PortInstance>(&self) -> bool {
        expect_worker_thread();

        THREAD_ROLE.with(|role| match role.get() {
            ThreadRole::Interrupt | ThreadRole::Task => true,
            ThreadRole::Boot => false,
            _ => panic!("`is_task_context` was called from an unknown thread"),
        })
    }

    pub fn set_interrupt_line_priority<Traits: PortInstance>(
        &'static self,
        num: InterruptNum,
        priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError> {
        log::trace!("set_interrupt_line_priority({num}, {priority})");
        assert!(matches!(
            expect_worker_thread(),
            ThreadRole::Boot | ThreadRole::Task
        ));

        let mut lock = self.thread_group.get().unwrap().lock();
        lock.scheduler()
            .update_line(num, |line| line.priority = priority)
            .map_err(|sched::BadIntLineError| SetInterruptLinePriorityError::BadParam)?;

        if sched::check_preemption_by_interrupt(self.thread_group.get().unwrap(), &mut lock) {
            drop(lock);
            ums::yield_now();
        }

        Ok(())
    }

    pub fn enable_interrupt_line<Traits: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        log::trace!("enable_interrupt_line({num})");
        expect_worker_thread();

        let mut lock = self.thread_group.get().unwrap().lock();
        lock.scheduler()
            .update_line(num, |line| line.enable = true)
            .map_err(|sched::BadIntLineError| EnableInterruptLineError::BadParam)?;

        if sched::check_preemption_by_interrupt(self.thread_group.get().unwrap(), &mut lock) {
            drop(lock);
            ums::yield_now();
        }

        Ok(())
    }

    pub fn disable_interrupt_line<Traits: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        log::trace!("disable_interrupt_line({num})");
        expect_worker_thread();

        (self.thread_group.get().unwrap().lock())
            .scheduler()
            .update_line(num, |line| line.enable = false)
            .map_err(|sched::BadIntLineError| EnableInterruptLineError::BadParam)
    }

    pub fn pend_interrupt_line<Traits: PortInstance>(
        &'static self,
        num: InterruptNum,
    ) -> Result<(), PendInterruptLineError> {
        log::trace!("pend_interrupt_line({num})");
        expect_worker_thread();

        let mut lock = self.thread_group.get().unwrap().lock();
        lock.scheduler()
            .update_line(num, |line| line.pended = true)
            .map_err(|sched::BadIntLineError| PendInterruptLineError::BadParam)?;

        if sched::check_preemption_by_interrupt(self.thread_group.get().unwrap(), &mut lock) {
            drop(lock);
            ums::yield_now();
        }

        Ok(())
    }

    pub fn clear_interrupt_line<Traits: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<(), ClearInterruptLineError> {
        log::trace!("clear_interrupt_line({num})");
        expect_worker_thread();

        (self.thread_group.get().unwrap().lock())
            .scheduler()
            .update_line(num, |line| line.pended = false)
            .map_err(|sched::BadIntLineError| ClearInterruptLineError::BadParam)
    }

    pub fn is_interrupt_line_pending<Traits: PortInstance>(
        &self,
        num: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        expect_worker_thread();

        (self.thread_group.get().unwrap().lock())
            .scheduler()
            .is_line_pended(num)
            .map_err(|sched::BadIntLineError| QueryInterruptLineError::BadParam)
    }

    // TODO: Make these customizable to test the kernel under multiple conditions
    pub const MAX_TICK_COUNT: UTicks = UTicks::MAX;
    pub const MAX_TIMEOUT: UTicks = UTicks::MAX / 2;

    pub fn tick_count<Traits: PortInstance>(&self) -> UTicks {
        expect_worker_thread();

        let origin = if let Some(x) = self.origin.load(Ordering::Acquire) {
            x
        } else {
            // Establish an origin point.
            let origin = Box::leak(Box::new(Instant::now()));

            // Store `origin` to `self.origin`.
            //
            // 1. If `self.origin` is already initialized at this point, discard
            //    `origin`. Use `Acquire` to synchronize with the canonical
            //    initializing thread.
            //
            // 2. Otherwise, `origin` is now the canonical origin. Use `Release`
            //    to synchronize with other threads, ensuring the initialized
            //    contents of `origin` is visible to them.
            //
            //    `compare_exchange` requires that the success ordering is
            //    stronger than the failure ordering, so we actually have to use
            //    `AcqRel` here.
            //
            // (Actually, this really doesn't matter because it's a kernel for
            // a uniprocessor system, anyway.)
            match self.origin.compare_exchange(
                None,
                Some(origin),
                Ordering::AcqRel,  // case 2
                Ordering::Acquire, // case 1
            ) {
                Ok(_) => origin,      // case 2
                Err(x) => x.unwrap(), // case 1
            }
        };

        let micros = Instant::now().duration_since(*origin).as_micros();

        /// Implementation of <https://xkcd.com/221/> with a different magic
        /// number
        fn get_random_number() -> UTicks {
            0x00c0ffee
        }

        // Calculate `micros % MAX_TICK_COUNT + 1` by truncating upper bits. Add
        // some random number so that the kernel doesn't depend on zero-start.
        (micros as UTicks).wrapping_add(get_random_number())
    }

    pub fn pend_tick_after<Traits: PortInstance>(&self, tick_count_delta: UTicks) {
        expect_worker_thread();
        log::trace!("pend_tick_after({tick_count_delta:?})");

        // Calculate when `timer_tick` should be called
        let now = Instant::now() + Duration::from_micros(tick_count_delta.into());

        // Lock the scheduler because we aren't sure what would happen if
        // `Sender::send` was interrupted
        let _sched_lock = lock_scheduler::<Traits>();

        let timer_cmd_send = self.timer_cmd_send.lock();
        let timer_cmd_send = timer_cmd_send.as_ref().unwrap();
        timer_cmd_send
            .send(TimerCmd::SetTimeout { at: now })
            .unwrap();
    }

    pub fn pend_tick<Traits: PortInstance>(&'static self) {
        expect_worker_thread();
        log::trace!("pend_tick");

        self.pend_interrupt_line::<Traits>(INTERRUPT_LINE_TIMER)
            .unwrap();
    }

    extern "C" fn timer_handler<Traits: PortInstance>() {
        assert_eq!(expect_worker_thread(), ThreadRole::Interrupt);
        log::trace!("timer_handler");

        // Safety: CPU Lock inactive, an interrupt context
        unsafe { <Traits as PortToKernel>::timer_tick() };
    }
}

/// Assert that the current thread is a worker thread of `Traits`.
fn expect_worker_thread() -> ThreadRole {
    // TODO: Check that the current worker thread belongs to
    //       `Traits::port_state().thread_group`
    let role = THREAD_ROLE.with(|r| r.get());
    assert_ne!(role, ThreadRole::Unknown);
    role
}

/// Initiate graceful shutdown.
///
/// The shutdown completes when all threads complete execution. Usually, the
/// process will exit after this.
///
/// Note: There is no safe way to restart the simulated system without
/// restarting an entire process.
pub fn shutdown<Traits: PortInstance>() {
    Traits::port_state()
        .thread_group
        .get()
        .unwrap()
        .lock()
        .shutdown();
}

/// Pend an interrupt line from an external thread.
///
/// It's illegal to call this method from a thread managed by the port (i.e.,
/// you can't call it from a task or an interrupt handler). Use
/// [`r3_core::kernel::InterruptLine::pend`] instead in such cases.
pub fn pend_interrupt_line<Traits: PortInstance>(
    num: InterruptNum,
) -> Result<(), PendInterruptLineError> {
    log::trace!("external-pend_interrupt_line({num})");

    assert_eq!(
        THREAD_ROLE.with(|r| r.get()),
        ThreadRole::Unknown,
        "this method cannot be called from a port-managed thread"
    );

    let state = Traits::port_state();
    let mut lock = state.thread_group.get().unwrap().lock();
    lock.scheduler()
        .update_line(num, |line| line.pended = true)
        .map_err(|sched::BadIntLineError| PendInterruptLineError::BadParam)?;

    if sched::check_preemption_by_interrupt(state.thread_group.get().unwrap(), &mut lock) {
        lock.preempt();
        drop(lock);
    }

    Ok(())
}

/// Temporarily lock the scheduler, disabling preemption.
///
/// *All* operating system and port functions will be unavailable until the lock
/// is relinquished.
pub fn lock_scheduler<Traits: PortInstance>() -> impl Sized {
    let state = Traits::port_state();
    state.thread_group.get().unwrap().lock()
}

#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $SystemTraits:ident) => {
        $vis struct $SystemTraits;

        mod port_std_impl {
            use super::$SystemTraits;
            use $crate::r3_core::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, QueryInterruptLineError, SetInterruptLinePriorityError,
            };
            use $crate::r3_kernel::{
                Port, TaskCb, PortToKernel, PortInterrupts, PortThreading, UTicks, PortTimer,
            };
            use $crate::{State, TaskState, PortInstance};

            pub(super) static PORT_STATE: State = State::new();

            unsafe impl PortInstance for $SystemTraits {
                #[inline]
                fn port_state() -> &'static State {
                    &PORT_STATE
                }
            }

            // Assume `$SystemTraits: KernelTraits`
            unsafe impl PortThreading for $SystemTraits {
                type PortTaskState = TaskState;
                #[allow(clippy::declare_interior_mutable_const)]
                const PORT_TASK_STATE_INIT: Self::PortTaskState = TaskState::new();

                unsafe fn dispatch_first_task() -> ! {
                    PORT_STATE.dispatch_first_task::<Self>()
                }

                unsafe fn yield_cpu() {
                    PORT_STATE.yield_cpu::<Self>()
                }

                unsafe fn exit_and_dispatch(task: &'static TaskCb<Self>) -> ! {
                    PORT_STATE.exit_and_dispatch::<Self>(task);
                }

                unsafe fn enter_cpu_lock() {
                    PORT_STATE.enter_cpu_lock::<Self>()
                }

                unsafe fn leave_cpu_lock() {
                    PORT_STATE.leave_cpu_lock::<Self>()
                }

                unsafe fn initialize_task_state(task: &'static TaskCb<Self>) {
                    PORT_STATE.initialize_task_state::<Self>(task)
                }

                fn is_cpu_lock_active() -> bool {
                    PORT_STATE.is_cpu_lock_active::<Self>()
                }

                fn is_task_context() -> bool {
                    PORT_STATE.is_task_context::<Self>()
                }

                fn is_interrupt_context() -> bool {
                    PORT_STATE.is_interrupt_context::<Self>()
                }

                fn is_scheduler_active() -> bool {
                    PORT_STATE.is_scheduler_active::<Self>()
                }
            }

            unsafe impl PortInterrupts for $SystemTraits {
                const MANAGED_INTERRUPT_PRIORITY_RANGE:
                    ::std::ops::Range<InterruptPriority> = 0..InterruptPriority::MAX;

                unsafe fn set_interrupt_line_priority(
                    line: InterruptNum,
                    priority: InterruptPriority,
                ) -> Result<(), SetInterruptLinePriorityError> {
                    PORT_STATE.set_interrupt_line_priority::<Self>(line, priority)
                }

                unsafe fn enable_interrupt_line(line: InterruptNum) -> Result<(), EnableInterruptLineError> {
                    PORT_STATE.enable_interrupt_line::<Self>(line)
                }

                unsafe fn disable_interrupt_line(line: InterruptNum) -> Result<(), EnableInterruptLineError> {
                    PORT_STATE.disable_interrupt_line::<Self>(line)
                }

                unsafe fn pend_interrupt_line(line: InterruptNum) -> Result<(), PendInterruptLineError> {
                    PORT_STATE.pend_interrupt_line::<Self>(line)
                }

                unsafe fn clear_interrupt_line(line: InterruptNum) -> Result<(), ClearInterruptLineError> {
                    PORT_STATE.clear_interrupt_line::<Self>(line)
                }

                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    PORT_STATE.is_interrupt_line_pending::<Self>(line)
                }
            }

            impl PortTimer for $SystemTraits {
                const MAX_TICK_COUNT: UTicks = State::MAX_TICK_COUNT;
                const MAX_TIMEOUT: UTicks = State::MAX_TIMEOUT;

                unsafe fn tick_count() -> UTicks {
                    PORT_STATE.tick_count::<Self>()
                }

                unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                    PORT_STATE.pend_tick_after::<Self>(tick_count_delta)
                }

                unsafe fn pend_tick() {
                    PORT_STATE.pend_tick::<Self>()
                }
            }
        }

        fn main() {
            $crate::env_logger::init();

            port_std_impl::PORT_STATE.port_boot::<$SystemTraits>();
        }
    };
}
