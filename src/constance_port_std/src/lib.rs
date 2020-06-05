/// Used by `use_port!`
#[doc(hidden)]
pub use constance::kernel::Port;

#[macro_export]
macro_rules! use_port {
    (unsafe $sys:ty) => {
        // Assume `$sys: Kernel`
        unsafe impl $crate::Port for $sys {
            type PortTaskState = ();
            const PORT_TASK_STATE_INIT: () = ();

            fn dispatch() {
                todo!()
            }
        }

        fn main() {
            todo!()
        }
    };
}
