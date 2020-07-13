#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "test", no_std)]
#![cfg_attr(feature = "test", no_main)]

#[allow(unused_macros)]
macro_rules! instantiate_test {
    // If a test case is specified, instantiate the test case
    ({ path: $path:path, name_ident: $ident:ident, $($tt:tt)* }, $($excess:tt)*) => {
        // Only one test case can be specified
        reject_excess!($($excess)*);

        use constance::kernel::{InterruptNum, StartupHook};
        use constance_test_suite::kernel_tests;
        use $path as test_case;

        // Install a global panic handler that uses RTT
        use panic_rtt_target as _;

        fn report_success() {
            // The test runner will catch this
            rtt_target::rprintln!("!- TEST WAS SUCCESSFUL -!");
            loop {}
        }

        fn report_fail() {
            panic!("test failed");
        }

        constance_port_arm_m::use_port!(unsafe struct System);

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
            const INTERRUPT_LINES: &'static [InterruptNum] = &[]; // TODO
        }

        static COTTAGE: test_case::App<System> =
            constance::build!(System, configure_app => test_case::App<System>);

        unsafe impl constance_port_arm_m::PortCfg for System {}

        constance::configure! {
            const fn configure_app(_: &mut CfgBuilder<System>) -> test_case::App<System> {
                // Initialize RTT (Real-Time Transfer) with a single up channel and set
                // it as the print channel for the printing macros
                new! { StartupHook<_>, start = |_| {
                    rtt_target::rtt_init_print!();
                } };

                call!(test_case::App::new::<Driver>)
            }
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
