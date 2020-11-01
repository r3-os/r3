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
                    $crate::rt::imp::setup_interrupt_handler::<$sys>();
                    <$sys as $crate::EntryPoint>::start();
                }
            }
        };
    };
}
