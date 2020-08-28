//! <div class="distractor"><a style="background-image:
//! url(https://derpicdn.net/img/2019/6/30/2079083/medium.png);
//! padding-bottom: 100%" href="http://derpibooru.org/2079083"
//! title="Screwdriver"></a></div>
#![doc(include = "../../constance/src/common.md")]
#![feature(external_doc)] // `#[doc(include = ...)]`
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(naked_functions)]
#![feature(llvm_asm)]
#![feature(unsafe_block_in_unsafe_fn)] // `unsafe fn` doesn't imply `unsafe {}`
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "run", no_std)]
#![cfg_attr(feature = "run", no_main)]

#[cfg(feature = "output-semihosting")]
mod logger_semihosting;

#[cfg(feature = "output-semihosting")]
mod panic_semihosting;

#[allow(unused_macros)]
macro_rules! instantiate_test {
    // If a test case is specified, instantiate the test case
    ({ path: $path:path, name_ident: $ident:ident, $($tt:tt)* }, $($excess:tt)*) => {
        // Only one test case can be specified
        reject_excess!($($excess)*);

        use constance::kernel::{StartupHook, InterruptPriority, InterruptNum,
            cfg::CfgBuilder};
        #[cfg(feature = "kernel_benchmarks")]
        use constance_test_suite::kernel_benchmarks;
        #[cfg(feature = "kernel_tests")]
        use constance_test_suite::kernel_tests;
        use constance_port_arm as port;
        use $path as test_case;

        fn report_success() {
            // The test runner will catch this
            #[cfg(feature = "output-semihosting")]
            arm_semihosting::hprintln!("!- TEST WAS SUCCESSFUL -!").unwrap();

            loop {}
        }

        fn report_fail() {
            panic!("test failed");
        }

        port::use_port!(unsafe struct System);
        port::use_startup!(unsafe System);
        port::use_gic!(unsafe impl PortInterrupts for System);
        port::use_sp804!(unsafe impl PortTimer for System);

        impl port::ThreadingOptions for System {}

        impl port::StartupOptions for System {
            // ARM RealView Platform Baseboard Explore for Cortex-A9
            const MEMORY_MAP: &'static [port::MemoryMapSection] = &[
                port::MemoryMapSection::new(0x0100_0000..0x0140_0000, 0x0100_0000)
                    .with_executable(true)
                    .with_writable(false),
                port::MemoryMapSection::new(0x0140_0000..0x0180_0000, 0x0100_0000),
                port::MemoryMapSection::new(0x1000_0000..0x1010_0000, 0x1000_0000)
                    .as_device_memory(),
                port::MemoryMapSection::new(0x1f00_0000..0x1f10_0000, 0x1f00_0000)
                    .as_device_memory(),
            ];
        }

        impl port::GicOptions for System {
            const GIC_DISTRIBUTOR_BASE: usize = 0x1f001000;
            const GIC_CPU_BASE: usize = 0x1f000100;
        }

        impl port::Sp804Options for System {
            const SP804_BASE: usize = 0x10011000;
            const FREQUENCY: u64 = 1_000_000;
            const INTERRUPT_NUM: InterruptNum = 36;
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

            fn performance_time() -> u32 {
                // SP804 Timer1
                !unsafe { ((<System as port::Sp804Options>::SP804_BASE + 4)
                    as *const u32).read_volatile() }
            }

            const PERFORMANCE_TIME_UNIT: &'static str = "μs";

            // Chose PPIs.
            // SGIs (software-generated interrupts) don't support disabling.
            const INTERRUPT_LINES: &'static [InterruptNum] = &[16, 17, 18, 19];
            const INTERRUPT_PRIORITY_LOW: InterruptPriority = 0x60;
            const INTERRUPT_PRIORITY_HIGH: InterruptPriority = 0x20;
        }

        #[cfg(feature = "kernel_tests")]
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
            System::configure_sp804(b);

            // Redirect the log output to stderr
            #[cfg(feature = "output-semihosting")]
            StartupHook::build().start(|_| {
                logger_semihosting::init();
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
#[cfg(feature = "kernel_benchmarks")]
constance_test_suite::get_selected_kernel_benchmarks!(instantiate_test!());
#[cfg(feature = "kernel_tests")]
constance_test_suite::get_selected_kernel_tests!(instantiate_test!());

#[cfg(not(feature = "run"))]
fn main() {
    panic!("This executable should be invoked directly");
}
