#![feature(const_fn)]
#![feature(external_doc)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![doc(include = "./lib.md")]
#![deny(unsafe_op_in_unsafe_fn)]
use atomic_ref::AtomicRef;
use constance::{
    kernel::{
        self, ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
        PendInterruptLineError, Port, PortToKernel, QueryInterruptLineError,
        SetInterruptLinePriorityError, TaskCb,
    },
    prelude::*,
    utils::intrusive_list::StaticListHead,
};
use parking_lot::{lock_api::RawMutex, Mutex};
use std::{
    any::Any,
    cell::Cell,
    collections::{BTreeSet, HashMap},
    mem::{replace, ManuallyDrop},
    sync::atomic::AtomicU8,
    thread::{self, JoinHandle},
};

mod threading;

/// Used by `use_port!`
#[doc(hidden)]
pub extern crate constance;
/// Used by `use_port!`
#[doc(hidden)]
pub use std::sync::atomic::{AtomicBool, Ordering};
/// Used by `use_port!`
#[doc(hidden)]
pub extern crate env_logger;

/// The number of interrupt lines. The valid range of interrupt numbers is
/// defined as `0..NUM_INTERRUPT_LINES`
pub const NUM_INTERRUPT_LINES: usize = 1024;

/// The internal state of the port.
///
/// # Safety
///
/// For the safety information of this type's methods, see the documentation of
/// the corresponding trait methods of `Port*`.
#[doc(hidden)]
pub struct State {
    cpu_lock: AtomicBool,
    dispatcher: AtomicRef<'static, thread::Thread>,
    dispatcher_pending: AtomicBool,
    panic_payload: Mutex<Option<Box<dyn Any + Send>>>,
    int_state: Mutex<Option<IntState>>,
    /// When handling an interrupt, this field tracks the interrupt priority.
    active_int_priority: Mutex<InterruptPriority>,
}

#[derive(Debug)]
pub struct TaskState {
    thread: ManuallyDrop<Mutex<Option<JoinHandle<()>>>>,
    tsm: AtomicU8,
}

/// The state of the simulated interrupt controller.
#[derive(Debug)]
struct IntState {
    int_lines: HashMap<InterruptNum, IntLine>,
    /// `int_lines.iter().filter(|_,a| a.pended && a.enable)
    /// .map(|i,a| (a.priority, i)).collect()`.
    pended_lines: BTreeSet<(InterruptPriority, InterruptNum)>,
}

/// The configuration of an interrupt line.
#[derive(Debug)]
struct IntLine {
    priority: InterruptPriority,
    start: Option<kernel::cfg::InterruptHandlerFn>,
    enable: bool,
    pended: bool,
}

// Task state machine
//
// These don't exactly align with the task states defined in the kernel.
//
/// The task's context state is not initialized. The kernel has to call
/// `initialize_task_state` first before choosing this task as `running_task`.
const TSM_UNINIT: u8 = 0;
/// The task's context state is initialized but hasn't started running.
const TSM_DORMANT: u8 = 1;
/// The task is currently running.
const TSM_RUNNING: u8 = 2;
/// The task is currently suspended.
const TSM_READY: u8 = 3;

impl Init for TaskState {
    const INIT: Self = Self::new();
}

impl Init for IntLine {
    const INIT: Self = IntLine {
        priority: 0,
        start: None,
        enable: false,
        pended: false,
    };
}

/// The role of a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThreadRole {
    Unknown,
    /// The main thread, on which dispatcher and interrupt handlers execute.
    Main,
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
            thread: ManuallyDrop::new(Mutex::const_new(RawMutex::INIT, None)),
            tsm: AtomicU8::new(TSM_UNINIT),
        }
    }

    fn assert_current_thread(&self) {
        // `self` must represent the current thread
        assert_eq!(
            Some(thread::current().id()),
            self.thread.lock().as_ref().map(|jh| jh.thread().id()),
            "`self` is not a current thread"
        );
    }

    /// Yield the current task `self` and invoke the dispatcher.
    fn yield_current(&self, state: &State) {
        log::trace!("yield_current({:p}) enter", self);
        self.assert_current_thread();

        self.tsm.store(TSM_READY, Ordering::Release);

        // Unpark the dispatcher
        state.invoke_dispatcher();

        // Suspend the current thread until woken up
        while self.tsm.load(Ordering::Acquire) != TSM_RUNNING {
            thread::park();
        }
        log::trace!("yield_current({:p}) leave", self);
    }

    unsafe fn exit_and_dispatch(&self, state: &State) -> ! {
        log::trace!("exit_and_dispatch({:p}) enter", self);
        self.assert_current_thread();

        // `self` must represent the current thread
        assert_eq!(
            Some(thread::current().id()),
            self.thread.lock().as_ref().map(|jh| jh.thread().id()),
            "`self` is not a current thread"
        );

        // Remove itself from `self.thread`
        let mut thread_cell = self.thread.lock();
        *thread_cell = None;
        self.tsm.store(TSM_UNINIT, Ordering::Release);
        drop(thread_cell);

        // Unpark the dispatcher
        state.invoke_dispatcher();

        log::trace!("exit_and_dispatch({:p}) calling exit_thread", self);
        unsafe {
            threading::exit_thread();
        }
    }
}

struct BadIntLineError;

impl IntState {
    fn new<System: Kernel>() -> Self {
        let mut this = Self {
            int_lines: HashMap::new(),
            pended_lines: BTreeSet::new(),
        };

        for i in 0..NUM_INTERRUPT_LINES {
            if let Some(handler) = System::INTERRUPT_HANDLERS.get(i) {
                this.int_lines.insert(
                    i as InterruptNum,
                    IntLine {
                        start: Some(handler),
                        ..IntLine::INIT
                    },
                );
            }
        }

        this
    }

    fn update_line(
        &mut self,
        i: InterruptNum,
        f: impl FnOnce(&mut IntLine),
    ) -> Result<(), BadIntLineError> {
        if i >= NUM_INTERRUPT_LINES {
            return Err(BadIntLineError);
        }
        let line = self.int_lines.entry(i).or_insert_with(|| IntLine::INIT);
        self.pended_lines.remove(&(line.priority, i));
        f(line);
        if line.enable && line.pended {
            self.pended_lines.insert((line.priority, i));
        }
        Ok(())
    }

    fn is_line_pended(&self, i: InterruptNum) -> Result<bool, BadIntLineError> {
        if i >= NUM_INTERRUPT_LINES {
            return Err(BadIntLineError);
        }

        if let Some(line) = self.int_lines.get(&i) {
            Ok(line.pended)
        } else {
            Ok(false)
        }
    }

    fn highest_pended_priority(&self) -> Option<InterruptPriority> {
        self.pended_lines.iter().next().map(|&(pri, _)| pri)
    }

    fn take_highest_pended_priority(
        &mut self,
    ) -> Option<(
        InterruptNum,
        InterruptPriority,
        kernel::cfg::InterruptHandlerFn,
    )> {
        let (pri, num) = self.pended_lines.iter().next().cloned()?;
        self.pended_lines.remove(&(pri, num));

        // Find the interrupt handler for `num`. Return
        // `default_interrupt_handler` if there's none.
        let start = self
            .int_lines
            .get(&num)
            .and_then(|line| line.start)
            .unwrap_or(Self::default_interrupt_handler);

        Some((num, pri, start))
    }

    extern "C" fn default_interrupt_handler() {
        panic!("Unhandled interrupt");
    }
}

#[allow(clippy::missing_safety_doc)]
impl State {
    pub const fn new() -> Self {
        Self {
            cpu_lock: AtomicBool::new(true),
            dispatcher: AtomicRef::new(None),
            dispatcher_pending: AtomicBool::new(true),
            panic_payload: Mutex::const_new(RawMutex::INIT, None),
            int_state: Mutex::const_new(RawMutex::INIT, None),
            active_int_priority: Mutex::const_new(RawMutex::INIT, 0),
        }
    }

    pub fn init<System: Kernel>(&self) {
        // Register the current thread as the main thread.
        THREAD_ROLE.with(|role| role.set(ThreadRole::Main));

        // Initialize the interrupt controller
        *self.int_state.lock() = Some(IntState::new::<System>());
    }

    fn invoke_dispatcher(&self) {
        let dispatcher = self.dispatcher.load(Ordering::Relaxed).unwrap();
        self.dispatcher_pending.store(true, Ordering::Release);
        dispatcher.unpark();
    }

    pub unsafe fn dispatch_first_task<System: Kernel>(&'static self) -> !
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("dispatch_first_task");
        assert!(self.is_cpu_lock_active());

        // The current thread must be a main thread
        THREAD_ROLE.with(|role| assert_eq!(role.get(), ThreadRole::Main));

        // This thread becomes the dispatcher
        self.dispatcher.store(
            Some(Box::leak(Box::new(thread::current()))),
            Ordering::Relaxed,
        );

        self.cpu_lock.store(false, Ordering::Relaxed);

        loop {
            if !self.dispatcher_pending.swap(false, Ordering::Acquire) {
                thread::park();
                continue;
            }

            // This thread can unpark under the following circumstances:
            //
            //  1. Voluntary yield by a task thread. CPU Lock is inactive in
            //     this case.
            //  2. An interrupt was pended by a task thread. CPU Lock might be
            //     active in this case.
            //  3. `dispatch_first_task` was called at boot time. This case is
            //     reduced to the first case.
            //

            // Check pending interrupts
            while let Some((num, pri, f)) = {
                let mut int_state = self.int_state.lock();
                int_state.as_mut().unwrap().take_highest_pended_priority()
            } {
                *self.active_int_priority.lock() = pri;

                log::trace!(
                    "handling a top-level interrupt {} (priority = {})",
                    num,
                    pri
                );

                // Safety: The port can call an interrupt handler
                unsafe { f() };
            }

            // If one of the tasks panics, resume rewinding in the dispatcher,
            // thus ensuring the whole program is terminated or a test failure
            // is reported.
            if let Some(panic_payload) = self.panic_payload.lock().take() {
                std::panic::resume_unwind(panic_payload);
            }

            let has_cpu_lock = self.cpu_lock.load(Ordering::Relaxed);

            if has_cpu_lock {
                // The dispatcher was invoked to call an unmanaged interrupt
                // handler. Do not call `choose_running_task` on the way out.
            } else {
                // Enable CPU Lock
                self.cpu_lock.store(true, Ordering::Relaxed);

                // Let the kernel decide the next task to run
                // Safety: CPU Lock enabled (implied by us being the dispatcher)
                unsafe {
                    System::choose_running_task();
                }
            }

            // Run that task
            if let Some(task) = System::state().running_task() {
                log::trace!("dispatching task {:p}", task);

                let pts = &task.port_task_state;

                // The task must be in `DORMANT` or `READY`.
                assert_ne!(pts.tsm.load(Ordering::Relaxed), TSM_UNINIT);
                assert_ne!(pts.tsm.load(Ordering::Relaxed), TSM_RUNNING);

                let mut thread_cell = pts.thread.lock();
                if thread_cell.is_none() {
                    // Start the task's thread
                    let jh = thread::Builder::new()
                        .spawn(move || {
                            THREAD_ROLE.with(|role| role.set(ThreadRole::Task));

                            while pts.tsm.load(Ordering::Acquire) != TSM_RUNNING {
                                thread::park();
                            }

                            assert!(!self.is_cpu_lock_active());

                            log::debug!("task {:p} is now running", task);

                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    // Safety: The port can call this
                                    unsafe {
                                        (task.attr.entry_point)(task.attr.entry_param);
                                    }
                                }));

                            // If the task panics, send the panic info to the
                            // dispatcher
                            if let Err(panic_payload) = result {
                                *self.panic_payload.lock() = Some(panic_payload);
                            }

                            // Safety: To my knowledge, we have nothing on the
                            // current thread' stack which are unsafe to
                            // `forget`. (`libstd`'s thread entry point might
                            // not be prepared to this, though...)
                            unsafe {
                                System::exit_task().unwrap();
                            }
                        })
                        .unwrap();
                    *thread_cell = Some(jh);
                }

                if !has_cpu_lock {
                    // Disable CPU Lock
                    self.cpu_lock.store(false, Ordering::Relaxed);
                }

                // Unpark the task's thread
                pts.tsm.store(TSM_RUNNING, Ordering::Release);
                thread_cell.as_ref().unwrap().thread().unpark();
            } else {
                // Since we don't have timers or interrupts yet, this means we
                // are in deadlock
                //
                // The test code currently relies on this behavior (panic on
                // deadlock) to exit the dispatcher loop. When we have timers
                // and interrupts, we need to devise another way to exit the
                // dispatcher loop.
                panic!("No task to schedule");
            }
        }
    }

    pub unsafe fn yield_cpu<System: Kernel>(&self)
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("yield_cpu");
        assert!(!self.is_cpu_lock_active());

        // The dispatcher will automatically run when the current interrupt
        // handler returns the control to it
        if self.is_interrupt_context() {
            return;
        }

        self.yield_cpu_inner::<System>();
    }

    fn yield_cpu_inner<System: Kernel>(&self)
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        let task = System::state().running_task().expect("no running task");
        task.port_task_state.yield_current(self);
    }

    pub unsafe fn exit_and_dispatch<System: Kernel>(&self, task: &'static TaskCb<System>) -> !
    where
        System: Port<PortTaskState = TaskState>,
        // Work-around <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("exit_and_dispatch");
        assert!(self.is_cpu_lock_active());

        unsafe { self.leave_cpu_lock::<System>() };

        unsafe {
            task.port_task_state.exit_and_dispatch(self);
        }
    }

    pub unsafe fn enter_cpu_lock(&self) {
        log::trace!("enter_cpu_lock");
        assert!(!self.is_cpu_lock_active());
        self.cpu_lock.store(true, Ordering::Relaxed);
    }

    pub unsafe fn leave_cpu_lock<System: Kernel>(&self)
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("leave_cpu_lock");
        assert!(self.is_cpu_lock_active());
        self.cpu_lock.store(false, Ordering::Relaxed);

        self.check_preemption_by_interrupt::<System>();
    }

    pub unsafe fn initialize_task_state<System: Kernel>(&self, task: &'static TaskCb<System>)
    where
        System: Port<PortTaskState = TaskState>,
    {
        log::trace!("initialize_task_state {:p}", task);

        let pts = &task.port_task_state;
        match pts.tsm.load(Ordering::Relaxed) {
            TSM_DORMANT => {}
            TSM_RUNNING | TSM_READY => {
                todo!("terminating a thread is not implemented yet");
            }
            TSM_UNINIT => {
                pts.tsm.store(TSM_DORMANT, Ordering::Relaxed);
            }
            _ => unreachable!(),
        }
    }

    pub fn is_cpu_lock_active(&self) -> bool {
        self.cpu_lock.load(Ordering::Relaxed)
    }

    pub fn is_interrupt_context(&self) -> bool {
        THREAD_ROLE.with(|role| match role.get() {
            ThreadRole::Main => true,
            ThreadRole::Task => false,
            _ => panic!("`is_interrupt_context` was called from an unknown thread"),
        })
    }

    fn is_interrupt_priority_managed(p: InterruptPriority) -> bool {
        p >= 0
    }

    /// Check if there's a pending interrupt that can preempt the current thread.
    /// If there's one, let it preempt immediately.
    fn check_preemption_by_interrupt<System: Kernel>(&self)
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        let mut int_state_lock = self.int_state.lock();
        let int_state = int_state_lock.as_mut().unwrap();

        let mut highest_pri = int_state.highest_pended_priority();

        // Masking by CPU Lock
        if self.is_cpu_lock_active() {
            if let Some(pri) = highest_pri {
                if Self::is_interrupt_priority_managed(pri) {
                    highest_pri = None;
                }
            }
        }

        let highest_pri = if let Some(highest_pri) = highest_pri {
            highest_pri
        } else {
            return;
        };

        if self.is_interrupt_context() {
            if highest_pri < *self.active_int_priority.lock() {
                // Nested activation - call the handler now
                let (num, _, f) = int_state.take_highest_pended_priority().unwrap();
                drop(int_state_lock);

                log::trace!(
                    "being preempted by a nested interrupt {} (priority = {})",
                    num,
                    highest_pri
                );

                let pri = replace(&mut *self.active_int_priority.lock(), highest_pri);

                // Safety: The port can call an interrupt handler
                unsafe { f() };

                *self.active_int_priority.lock() = pri;

                log::trace!(
                    "returning from a nested interrupt {} (priority = {})",
                    num,
                    highest_pri
                );
            } else {
                log::trace!(
                    "a nested interrupt with priority {} will not preempt because \
                    it has a lower priority than the current one ({})",
                    highest_pri,
                    *self.active_int_priority.lock()
                );
            }
        } else {
            // Top-level activation - yield the control to the dispatcher
            drop(int_state_lock);

            log::trace!(
                "being preempted by a top-level interrupt (priority = {})",
                highest_pri
            );

            self.yield_cpu_inner::<System>();
        }
    }

    pub fn set_interrupt_line_priority<System: Kernel>(
        &self,
        num: InterruptNum,
        priority: InterruptPriority,
    ) -> Result<(), SetInterruptLinePriorityError>
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("set_interrupt_line_priority{:?}", (num, priority));

        (self.int_state.lock().as_mut().unwrap())
            .update_line(num, |line| line.priority = priority)
            .map_err(|BadIntLineError| SetInterruptLinePriorityError::BadParam)?;

        self.check_preemption_by_interrupt::<System>();

        Ok(())
    }

    pub fn enable_interrupt_line<System: Kernel>(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError>
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("enable_interrupt_line{:?}", (num,));

        (self.int_state.lock().as_mut().unwrap())
            .update_line(num, |line| line.enable = true)
            .map_err(|BadIntLineError| EnableInterruptLineError::BadParam)?;

        self.check_preemption_by_interrupt::<System>();

        Ok(())
    }

    pub fn disable_interrupt_line(
        &self,
        num: InterruptNum,
    ) -> Result<(), EnableInterruptLineError> {
        log::trace!("disable_interrupt_line{:?}", (num,));

        (self.int_state.lock().as_mut().unwrap())
            .update_line(num, |line| line.enable = false)
            .map_err(|BadIntLineError| EnableInterruptLineError::BadParam)
    }

    pub fn pend_interrupt_line<System: Kernel>(
        &self,
        num: InterruptNum,
    ) -> Result<(), PendInterruptLineError>
    where
        System: Port<PortTaskState = TaskState>,
        // FIXME: Work-around for <https://github.com/rust-lang/rust/issues/43475>
        System::TaskReadyQueue: std::borrow::BorrowMut<[StaticListHead<TaskCb<System>>]>,
    {
        log::trace!("pend_interrupt_line{:?}", (num,));

        (self.int_state.lock().as_mut().unwrap())
            .update_line(num, |line| line.pended = true)
            .map_err(|BadIntLineError| PendInterruptLineError::BadParam)?;

        self.check_preemption_by_interrupt::<System>();

        Ok(())
    }

    pub fn clear_interrupt_line(&self, num: InterruptNum) -> Result<(), ClearInterruptLineError> {
        log::trace!("clear_interrupt_line{:?}", (num,));

        (self.int_state.lock().as_mut().unwrap())
            .update_line(num, |line| line.pended = false)
            .map_err(|BadIntLineError| ClearInterruptLineError::BadParam)
    }

    pub fn is_interrupt_line_pending(
        &self,
        num: InterruptNum,
    ) -> Result<bool, QueryInterruptLineError> {
        (self.int_state.lock().as_ref().unwrap())
            .is_line_pended(num)
            .map_err(|BadIntLineError| QueryInterruptLineError::BadParam)
    }
}

#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        mod port_std_impl {
            use super::$sys;
            use $crate::constance::kernel::{
                ClearInterruptLineError, EnableInterruptLineError, InterruptNum, InterruptPriority,
                PendInterruptLineError, Port, QueryInterruptLineError, SetInterruptLinePriorityError,
                TaskCb, PortToKernel, PortInterrupts, PortThreading,
            };
            use $crate::{State, TaskState};

            pub(super) static PORT_STATE: State = State::new();

            // Assume `$sys: Kernel`
            unsafe impl PortThreading for $sys {
                type PortTaskState = TaskState;
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
                    PORT_STATE.enter_cpu_lock()
                }

                unsafe fn leave_cpu_lock() {
                    PORT_STATE.leave_cpu_lock::<Self>()
                }

                unsafe fn initialize_task_state(task: &'static TaskCb<Self>) {
                    PORT_STATE.initialize_task_state(task)
                }

                fn is_cpu_lock_active() -> bool {
                    PORT_STATE.is_cpu_lock_active()
                }

                fn is_interrupt_context() -> bool {
                    PORT_STATE.is_interrupt_context()
                }
            }

            unsafe impl PortInterrupts for $sys {
                const MANAGED_INTERRUPT_PRIORITY_RANGE:
                    ::std::ops::Range<InterruptPriority> = 0..InterruptPriority::max_value();

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
                    PORT_STATE.disable_interrupt_line(line)
                }

                unsafe fn pend_interrupt_line(line: InterruptNum) -> Result<(), PendInterruptLineError> {
                    PORT_STATE.pend_interrupt_line::<Self>(line)
                }

                unsafe fn clear_interrupt_line(line: InterruptNum) -> Result<(), ClearInterruptLineError> {
                    PORT_STATE.clear_interrupt_line(line)
                }

                unsafe fn is_interrupt_line_pending(
                    line: InterruptNum,
                ) -> Result<bool, QueryInterruptLineError> {
                    PORT_STATE.is_interrupt_line_pending(line)
                }
            }
        }

        fn main() {
            use $crate::constance::kernel::PortToKernel;

            $crate::env_logger::init();

            port_std_impl::PORT_STATE.init::<$sys>();

            // Safety: We are a port, so it's okay to call these
            unsafe {
                <$sys as PortToKernel>::boot();
            }
        }
    };
}
