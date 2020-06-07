#[doc(hidden)]
/// Used by `use_port!`
pub use std::sync::atomic::{AtomicBool, Ordering};

#[doc(hidden)]
pub use constance::kernel::{init_hunks, Port};

#[doc(hidden)]
pub struct State {
    pub cpu_lock: AtomicBool,
}

impl State {
    pub const fn new() -> Self {
        Self {
            cpu_lock: AtomicBool::new(false),
        }
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

            unsafe fn yield_cpu() {
                PORT_STATE.yield_cpu()
            }

            unsafe fn enter_cpu_lock() {
                PORT_STATE.enter_cpu_lock()
            }

            unsafe fn leave_cpu_lock() {
                PORT_STATE.leave_cpu_lock()
            }

            fn is_cpu_lock_active() -> bool {
                PORT_STATE.is_cpu_lock_active()
            }
        }

        fn main() {
            // Safety: We are a port, so it's okay to call this
            unsafe {
                <$sys as $crate::PortToKernel>::init();
            }
            todo!()
        }
    };
}
