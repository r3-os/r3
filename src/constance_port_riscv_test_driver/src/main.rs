#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(llvm_asm)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "test", no_std)]
#![cfg_attr(feature = "test", no_main)]

#[cfg(feature = "output-htif")]
mod logger_htif;

#[cfg(feature = "output-htif")]
mod panic_htif;

#[allow(unused_macros)]
macro_rules! instantiate_test {
    // If a test case is specified, instantiate the test case
    ({ path: $path:path, name_ident: $ident:ident, $($tt:tt)* }, $($excess:tt)*) => {
        // Only one test case can be specified
        reject_excess!($($excess)*);

        use constance::kernel::{StartupHook, InterruptPriority, InterruptNum,
            cfg::CfgBuilder};
        use constance_test_suite::kernel_tests;
        use constance_port_riscv as port;
        use $path as test_case;

        fn report_success() {
            // The test runner will catch this
            // TODO
            // #[cfg(feature = "output-htif")]
            // riscv_htif::hprintln!("!- TEST WAS SUCCESSFUL -!").unwrap();

            loop {}
        }

        fn report_fail() {
            panic!("test failed");
        }

        port::use_port!(unsafe struct System);
        port::use_rt!(unsafe System);
        port::use_plic!(unsafe impl PortInterrupts for System);

        impl port::ThreadingOptions for System {}

        impl port::PlicOptions for System {
            const MAX_PRIORITY: InterruptPriority = 255;
            const PLIC_BASE: usize = 0x2000_0000; // TODO
        }

        use constance::kernel::UTicks;
        impl constance::kernel::PortTimer for System {
            // TODO
            const MAX_TICK_COUNT: UTicks = 0xffffffff;
            const MAX_TIMEOUT: UTicks = 0x80000000;
            unsafe fn tick_count() -> UTicks {
                0
            }
            unsafe fn pend_tick_after(tick_count_delta: UTicks) {
                if tick_count_delta < Self::MAX_TIMEOUT {
                    todo!("pend_tick_after")
                }
            }
        }

        struct Driver;

        impl kernel_tests::Driver<test_case::App<System>> for Driver {
            fn app() -> &'static test_case::App<System> {
                &COTTAGE
            }
            fn success() {
                report_success();
            }
            fn fail() {
                report_fail();
            }
            // Chose PPIs.
            // SGIs (software-generated interrupts) don't support disabling.
            const INTERRUPT_LINES: &'static [InterruptNum] = &[16, 17, 18, 19];
            const INTERRUPT_PRIORITY_LOW: InterruptPriority = 0x60;
            const INTERRUPT_PRIORITY_HIGH: InterruptPriority = 0x20;
        }

        static COTTAGE: test_case::App<System> =
            constance::build!(System, configure_app => test_case::App<System>);

        const fn configure_app(b: &mut CfgBuilder<System>) -> test_case::App<System> {
            // Redirect the log output to stderr
            #[cfg(feature = "output-htif")]
            StartupHook::build().start(|_| {
                logger_htif::init();
            }).finish(b);

            test_case::App::new::<Driver>(b)
        }
    };

    () => {
        compile_error!("no test is specified");
    }
}

#[allow(unused_macros)]
macro_rules! reject_excess {
    () => {};
    ($($tt:tt)*) => {
        compile_error!("can't specify more than one test");
    };
}

// Get the selected test case and instantiate
#[cfg(feature = "test")]
constance_test_suite::get_selected_kernel_tests!(instantiate_test!());

#[cfg(not(feature = "test"))]
fn main() {
    panic!("This executable should be invoked directly");
}
