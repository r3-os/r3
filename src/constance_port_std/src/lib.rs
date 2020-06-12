#![feature(const_fn)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
use atomic_ref::AtomicRef;
use constance::{prelude::*, utils::intrusive_list::StaticListHead};
use parking_lot::{lock_api::RawMutex, Mutex};
use std::{
    any::Any,
    mem::ManuallyDrop,
    sync::atomic::AtomicU8,
    thread::{self, JoinHandle},
};

mod threading;

#[doc(hidden)]
pub use constance::kernel::{Port, PortToKernel, TaskCb};
/// Used by `use_port!`
#[doc(hidden)]
pub use std::sync::atomic::{AtomicBool, Ordering};
#[doc(hidden)]
pub extern crate env_logger;

#[doc(hidden)]
pub struct State {
    cpu_lock: AtomicBool,
    dispatcher: AtomicRef<'static, thread::Thread>,
    dispatcher_pending: AtomicBool,
    panic_payload: Mutex<Option<Box<dyn Any + Send>>>,
}

#[derive(Debug)]
pub struct TaskState {
    thread: ManuallyDrop<Mutex<Option<JoinHandle<()>>>>,
    tsm: AtomicU8,
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
const TSM_RUNNABLE: u8 = 3;

impl Init for TaskState {
    const INIT: Self = Self::new();
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

        self.tsm.store(TSM_RUNNABLE, Ordering::Release);

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

impl State {
    pub const fn new() -> Self {
        Self {
            cpu_lock: AtomicBool::new(true),
            dispatcher: AtomicRef::new(None),
            dispatcher_pending: AtomicBool::new(true),
            panic_payload: Mutex::const_new(RawMutex::INIT, None),
        }
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

        // This thread becomes the dispatcher
        self.dispatcher.store(
            Some(Box::leak(Box::new(thread::current()))),
            Ordering::Relaxed,
        );

        loop {
            if !self.dispatcher_pending.swap(false, Ordering::Acquire) {
                thread::park();
                continue;
            }

            // Enable CPU Lock
            self.cpu_lock.store(true, Ordering::Relaxed);

            // If one of the tasks panics, resume rewinding in the dispatcher,
            // thus ensuring the whole program is terminated or a test failure
            // is reported.
            if let Some(panic_payload) = self.panic_payload.lock().take() {
                std::panic::resume_unwind(panic_payload);
            }

            // Let the kernel decide the next task to run
            // Safety: CPU Lock enabled (implied by us being the dispatcher)
            unsafe {
                System::choose_running_task();
            }

            // Run that task
            if let Some(task) = System::state().running_task() {
                log::trace!("dispatching task {:p}", task);

                let pts = &task.port_task_state;

                // The task must be in `DORMANT` or `RUNNABLE`.
                assert_ne!(pts.tsm.load(Ordering::Relaxed), TSM_UNINIT);
                assert_ne!(pts.tsm.load(Ordering::Relaxed), TSM_RUNNING);

                let mut thread_cell = pts.thread.lock();
                if thread_cell.is_none() {
                    // Start the task's thread
                    let jh = thread::Builder::new()
                        .spawn(move || {
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

                // Disable CPU Lock
                self.cpu_lock.store(false, Ordering::Relaxed);

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

        unsafe {
            task.port_task_state.exit_and_dispatch(self);
        }
    }

    pub unsafe fn enter_cpu_lock(&self) {
        log::trace!("enter_cpu_lock");
        assert!(!self.is_cpu_lock_active());
        self.cpu_lock.store(true, Ordering::Relaxed);
    }

    pub unsafe fn leave_cpu_lock(&self) {
        log::trace!("leave_cpu_lock");
        assert!(self.is_cpu_lock_active());
        self.cpu_lock.store(false, Ordering::Relaxed);
    }

    pub unsafe fn initialize_task_state<System: Kernel>(&self, task: &'static TaskCb<System>)
    where
        System: Port<PortTaskState = TaskState>,
    {
        log::trace!("initialize_task_state {:p}", task);

        let pts = &task.port_task_state;
        match pts.tsm.load(Ordering::Relaxed) {
            TSM_DORMANT => {}
            TSM_RUNNING | TSM_RUNNABLE => {
                todo!("terminating a thread is not implemented yet");
            }
            TSM_UNINIT => {
                pts.tsm.store(TSM_DORMANT, Ordering::Relaxed);
            }
            _ => unreachable!(),
        }
    }

    pub fn is_cpu_lock_active(&self) -> bool {
        let b = self.cpu_lock.load(Ordering::Relaxed);
        log::trace!("is_cpu_lock_active -> {:?}", b);
        b
    }
}

#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        static PORT_STATE: $crate::State = $crate::State::new();

        // Assume `$sys: Kernel`
        unsafe impl $crate::Port for $sys {
            type PortTaskState = $crate::TaskState;
            const PORT_TASK_STATE_INIT: Self::PortTaskState = $crate::TaskState::new();

            unsafe fn dispatch_first_task() -> ! {
                PORT_STATE.dispatch_first_task::<Self>()
            }

            unsafe fn yield_cpu() {
                PORT_STATE.yield_cpu::<Self>()
            }

            unsafe fn exit_and_dispatch(task: &'static $crate::TaskCb<Self>) -> ! {
                PORT_STATE.exit_and_dispatch::<Self>(task);
            }

            unsafe fn enter_cpu_lock() {
                PORT_STATE.enter_cpu_lock()
            }

            unsafe fn leave_cpu_lock() {
                PORT_STATE.leave_cpu_lock()
            }

            unsafe fn initialize_task_state(task: &'static $crate::TaskCb<Self>) {
                PORT_STATE.initialize_task_state(task)
            }

            fn is_cpu_lock_active() -> bool {
                PORT_STATE.is_cpu_lock_active()
            }
        }

        fn main() {
            $crate::env_logger::init();

            // Safety: We are a port, so it's okay to call these
            unsafe {
                <$sys as $crate::PortToKernel>::boot();
            }
        }
    };
}
