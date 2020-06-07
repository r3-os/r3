#[doc(hidden)]
/// Used by `use_port!`
pub use std::sync::atomic::{AtomicBool, Ordering};

#[doc(hidden)]
pub use constance::kernel::{Port, PortToKernel, TaskCb};

#[doc(hidden)]
pub struct State {
    pub cpu_lock: AtomicBool,
}

impl State {
    pub const fn new() -> Self {
        Self {
            cpu_lock: AtomicBool::new(true),
        }
    }

    pub unsafe fn dispatch_first_task(&self) -> ! {
        todo!()
    }

    pub unsafe fn yield_cpu(&self) {
        todo!()
    }

    pub unsafe fn enter_cpu_lock(&self) {
        assert!(!self.is_cpu_lock_active());
        self.cpu_lock.store(true, Ordering::Relaxed);
    }

    pub unsafe fn leave_cpu_lock(&self) {
        assert!(self.is_cpu_lock_active());
        self.cpu_lock.store(false, Ordering::Relaxed);
    }

    pub unsafe fn initialize_task_state<System>(&self, _task: &TaskCb<System, ()>) {}

    pub fn is_cpu_lock_active(&self) -> bool {
        self.cpu_lock.load(Ordering::Relaxed)
    }
}

#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        static PORT_STATE: $crate::State = $crate::State::new();

        // Assume `$sys: Kernel`
        unsafe impl $crate::Port for $sys {
            type PortTaskState = ();
            const PORT_TASK_STATE_INIT: () = ();

            unsafe fn dispatch_first_task() -> ! {
                PORT_STATE.dispatch_first_task()
            }

            unsafe fn yield_cpu() {
                PORT_STATE.yield_cpu()
            }

            unsafe fn enter_cpu_lock() {
                PORT_STATE.enter_cpu_lock()
            }

            unsafe fn leave_cpu_lock() {
                PORT_STATE.leave_cpu_lock()
            }

            unsafe fn initialize_task_state(task: &$crate::TaskCb<Self, Self::PortTaskState>) {
                PORT_STATE.initialize_task_state(task)
            }

            fn is_cpu_lock_active() -> bool {
                PORT_STATE.is_cpu_lock_active()
            }
        }

        fn main() {
            // Safety: We are a port, so it's okay to call these
            unsafe {
                <$sys as $crate::PortToKernel>::boot();
            }
        }
    };
}
