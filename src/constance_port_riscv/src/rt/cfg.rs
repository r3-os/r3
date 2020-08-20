#[doc(hidden)]
pub extern crate riscv_rt;

/// Generate entry points using [`::riscv_rt`]. **Requires [`EntryPoint`] to
/// be implemented.**
///
/// [`EntryPoint`]: crate::EntryPoint
#[macro_export]
macro_rules! use_rt {
    (unsafe $sys:ty) => {
        const _: () = {
            #[$crate::riscv_rt::entry]
            fn start() -> ! {
                unsafe {
                    <$sys as $crate::EntryPoint>::start();
                }
            }

            // Called by `riscv-rt`'s startup code
            #[no_mangle]
            fn _setup_interrupts() {
                unsafe {
                    $crate::rt::imp::setup_interrupt_handler::<$sys>();
                }
            }
        };
    };
}
