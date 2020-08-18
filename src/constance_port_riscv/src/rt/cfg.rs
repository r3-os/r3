#[doc(hidden)]
pub extern crate riscv_rt;

/// Generate entry points using [`::riscv_rt`]. **Requires [`EntryPoint`] to
/// be implemented.**
///
/// [`EntryPoint`]: crate::EntryPoint
#[macro_export]
macro_rules! use_rt {
    (unsafe $sys:ty) => {
        #[$crate::riscv_rt::entry]
        fn start() -> ! {
            unsafe {
                <$sys as $crate::EntryPoint>::start();
            }
        }

        #[export_name = "MachineExternal"]
        #[export_name = "MachineTimer"]
        #[export_name = "MachineSoft"]
        #[naked]
        fn MachineExternal() {
            unsafe {
                <$sys as $crate::EntryPoint>::external_interrupt_handler();
            }
        }
    };
}
