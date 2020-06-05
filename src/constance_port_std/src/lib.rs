/// Used by `use_port!`
#[doc(hidden)]
pub use constance::kernel::Port;

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
            todo!()
        }
    };
}
