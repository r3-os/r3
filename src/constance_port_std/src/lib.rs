/// Used by `use_port!`
#[doc(hidden)]
pub use constance::kernel::{init_hunks, Port};

#[macro_export]
macro_rules! use_port {
    (unsafe $vis:vis struct $sys:ident) => {
        $vis struct $sys;

        // Assume `$sys: Kernel`
        unsafe impl $crate::Port for $sys {
            type PortTaskState = ();
            const PORT_TASK_STATE_INIT: () = ();

            fn dispatch() {
                todo!()
            }
        }

        fn main() {
            unsafe {
                $crate::init_hunks::<$sys>();
            }
            todo!()
        }
    };
}
