#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(asm_const)]
#![feature(asm_sym)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "run", no_std)]
#![cfg_attr(feature = "run", no_main)]

#[cfg(feature = "board-rp_pico")]
mod board_rp2040;
#[cfg(feature = "output-rtt")]
mod logger_rtt;
#[cfg(feature = "output-semihosting")]
mod logger_semihosting;

#[allow(unused_macros)]
macro_rules! instantiate_test {
    // If a test case is specified, instantiate the test case
    ({ path: $path:path, name_ident: $ident:ident, $($tt:tt)* }, $($excess:tt)*) => {
        // Only one test case can be specified
        reject_excess!($($excess)*);

        use r3::kernel::{InterruptNum, InterruptPriority, StartupHook};
        #[cfg(feature = "kernel_benchmarks")]
        use r3_test_suite::kernel_benchmarks;
        #[cfg(feature = "kernel_tests")]
        use r3_test_suite::kernel_tests;
        use r3_port_arm_m as port;
        use $path as test_case;

        // Install a global panic handler
        #[cfg(feature = "output-rtt")]
        use panic_rtt_target as _;
        #[cfg(feature = "output-semihosting")]
        use panic_semihosting as _;
        // `board-rp_pico`: provided by `crate::board_rp2040`

        fn report_success() {
            // The test runner will catch this
            #[cfg(feature = "output-rtt")]
            rtt_target::rprintln!("!- TEST WAS SUCCESSFUL -!");

            #[cfg(feature = "output-semihosting")]
            cortex_m_semihosting::hprintln!("!- TEST WAS SUCCESSFUL -!");

            #[cfg(feature = "board-rp_pico")]
            r3_support_rp2040::sprintln!(
                "{BEGIN_MAIN}!- TEST WAS SUCCESSFUL -!",
                BEGIN_MAIN = crate::board_rp2040::mux::BEGIN_MAIN,
            );

            #[cfg(feature = "board-rp_pico")]
            board_rp2040::enter_poll_loop();

            loop {}
        }

        fn report_fail() {
            panic!("test failed");
        }

        type System = r3_kernel::System<SystemTraits>;
        port::use_port!(unsafe struct SystemTraits);
        port::use_rt!(unsafe SystemTraits);
        port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

        impl port::ThreadingOptions for SystemTraits {
            // On some chips, RTT stops working when the processor is suspended
            // by the WFI instruction, which interferes with test result
            // collection.
            const USE_WFI: bool = false;

            #[cfg(feature = "cpu-lock-by-basepri")]
            const CPU_LOCK_PRIORITY_MASK: u8 = 0x20;
        }

        impl port::SysTickOptions for SystemTraits {
            #[cfg(feature = "board-rp_pico")]
            const FREQUENCY: u64 = board_rp2040::SYSTICK_FREQUENCY;

            // STM32F401
            // SysTick = AHB/8, AHB = HSI (internal 16-MHz RC oscillator)
            #[cfg(not(feature = "board-rp_pico"))]
            const FREQUENCY: u64 = 2_000_000;
        }

        struct Driver;

        #[cfg(feature = "kernel_benchmarks")]
        impl kernel_benchmarks::Driver<test_case::App<System>> for Driver {
            fn app() -> &'static test_case::App<System> {
                &COTTAGE
            }
            fn success() {
                report_success();
            }

            #[cfg(not(feature = "board-rp_pico"))]
            fn performance_time() -> u32 {
                cortex_m::peripheral::DWT::get_cycle_count()
            }

            #[cfg(feature = "board-rp_pico")]
            fn performance_time() -> u32 {
                board_rp2040::performance_time()
            }

            const PERFORMANCE_TIME_UNIT: &'static str = "cycle(s)";

            // Most targets should have at least four interrupt lines
            const INTERRUPT_LINES: &'static [InterruptNum] = &[16, 17, 18, 19];
            const INTERRUPT_PRIORITIES: &'static [InterruptPriority] = &[0x20, 0x60];
        }

        #[cfg(feature = "kernel_tests")]
        impl kernel_tests::Driver<test_case::App<System>> for Driver {
            type System = System;

            fn app() -> &'static test_case::App<System> {
                &COTTAGE
            }
            fn success() {
                report_success();
            }
            fn fail() {
                report_fail();
            }

            // Most targets should have at least four interrupt lines
            const INTERRUPT_LINES: &'static [InterruptNum] = &[16, 17, 18, 19];
            const INTERRUPT_PRIORITIES: &'static [InterruptPriority] = &[0x20, 0x60];

            #[cfg(feature = "cpu-lock-by-basepri")]
            const UNMANAGED_INTERRUPT_PRIORITIES: &'static [InterruptPriority] = &[0x00];
        }

        static COTTAGE: test_case::App<System> =
            r3_kernel::build!(SystemTraits, configure_app => test_case::App<System>);

        const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> test_case::App<System> {
            // Configure DWT for performance measurement
            #[cfg(feature = "kernel_benchmarks")]
            #[cfg(not(feature = "board-rp_pico"))]
            StartupHook::<System>::define().start(|| {
                unsafe {
                    let mut peripherals = cortex_m::peripheral::Peripherals::steal();
                    peripherals.DCB.enable_trace();
                    cortex_m::peripheral::DWT::unlock();
                    peripherals.DWT.enable_cycle_counter();
                }
            }).finish(b);

            // Initialize RTT (Real-Time Transfer) with two up channels and set
            // the first one as the print channel for the printing macros, and
            // the second one as log output
            #[cfg(feature = "output-rtt")]
            StartupHook::define().start(|| {
                let channels = rtt_target::rtt_init! {
                    up: {
                        0: {
                            size: 1024
                            mode: NoBlockSkip
                            name: "Terminal"
                        }
                        1: {
                            size: 1024
                            mode: NoBlockSkip
                            name: "Log"
                        }
                    }
                };

                rtt_target::set_print_channel(channels.up.0);
                logger_rtt::init(channels.up.1);
            }).finish(b);

            // Redirect the log output to stderr
            #[cfg(feature = "output-semihosting")]
            StartupHook::define().start(|| {
                logger_semihosting::init();
            }).finish(b);

            // Create a USB serial device and redirect the log output and the
            // main message to it
            #[cfg(feature = "board-rp_pico")]
            board_rp2040::configure(b);

            SystemTraits::configure_systick(b);

            test_case::App::new::<_, Driver>(b)
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
#[cfg(feature = "kernel_benchmarks")]
r3_test_suite::get_selected_kernel_benchmarks!(instantiate_test!());
#[cfg(feature = "kernel_tests")]
r3_test_suite::get_selected_kernel_tests!(instantiate_test!());

#[cfg(not(feature = "run"))]
fn main() {
    panic!("This executable should not be invoked directly");
}
