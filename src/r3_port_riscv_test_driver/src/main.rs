#![feature(const_fn_fn_ptr_basics)]
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(const_ptr_offset)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(decl_macro)]
#![feature(asm_const)]
#![feature(asm_sym)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "run", no_std)]
#![cfg_attr(feature = "run", no_main)]

#[cfg(feature = "output-rtt")]
mod logger_rtt;
#[cfg(feature = "output-uart")]
mod logger_uart;

#[cfg(feature = "output-rtt")]
mod panic_rtt;
#[cfg(feature = "output-uart")]
mod panic_uart;

#[cfg(feature = "output-e310x-uart")]
#[path = "uart_e310x.rs"]
mod uart;
#[cfg(feature = "output-u540-uart")]
#[path = "uart_u540.rs"]
mod uart;
#[cfg(feature = "output-k210-uart")]
#[path = "uart_k210.rs"]
mod uart;

#[cfg(feature = "interrupt-e310x")]
mod interrupt_e310x;

#[cfg(any(feature = "board-e310x-red-v", feature = "board-e310x-qemu"))]
mod e310x;
#[cfg(feature = "board-maix")]
mod k210;
#[cfg(feature = "board-u540-qemu")]
mod u540;

#[allow(unused_macros)]
macro_rules! instantiate_test {
    // If a test case is specified, instantiate the test case
    ({ path: $path:path, $($tt:tt)* }, $($excess:tt)*) => {
        // Only one test case can be specified
        reject_excess!($($excess)*);

        use r3::kernel::{StartupHook, InterruptPriority, InterruptNum};
        #[cfg(feature = "kernel_tests")]
        use r3_test_suite::kernel_tests;
        #[cfg(feature = "kernel_benchmarks")]
        use r3_test_suite::kernel_benchmarks;
        use r3_port_riscv as port;
        use $path as test_case;

        fn report_success() {
            // The test runner will catch this
            #[cfg(feature = "output-rtt")]
            rtt_target::rprintln!("!- TEST WAS SUCCESSFUL -!");

            #[cfg(feature = "output-uart")]
            uart::stdout_write_str("!- TEST WAS SUCCESSFUL -!");

            loop {
                // prevent the loop from being optimized out
                // <https://github.com/rust-lang/rust/issues/28728>
                unsafe { core::arch::asm!("") };
            }
        }

        fn report_fail() {
            panic!("test failed");
        }

        type System = r3_kernel::System<SystemTraits>;
        port::use_port!(unsafe struct SystemTraits);

        #[cfg(feature = "timer-clint")]
        port::use_mtime!(unsafe impl PortTimer for SystemTraits);
        #[cfg(feature = "timer-sbi")]
        port::use_sbi_timer!(unsafe impl PortTimer for SystemTraits);

        impl port::ThreadingOptions for SystemTraits {
            #[cfg(feature = "boot-minimal-s")]
            const PRIVILEGE_LEVEL: u8 = port::PRIVILEGE_LEVEL_SUPERVISOR;
        }

        #[cfg(feature = "boot-rt")]
        port::use_rt!(unsafe SystemTraits);

        #[cfg(feature = "boot-minimal-s")]
        #[no_mangle]
        #[naked]
        extern "C" fn start() {
            unsafe {
                core::arch::asm!(
                    "
                    # Configure the initial stack
                    la a0, {MAIN_STACK}
                    li a1, 8192
                    andi a0, a0, -16
                    add sp, a0, a1

                    call {start_kernel}
                    ",
                    start_kernel = sym start_kernel,
                    MAIN_STACK = sym MAIN_STACK,
                    options(noreturn)
                );
            }

            static MAIN_STACK: core::mem::MaybeUninit<[u8; 8192]> = core::mem::MaybeUninit::uninit();

            extern "C" fn start_kernel() {
                unsafe {
                    core::arch::asm!(
                        "csrw stvec, {}",
                        in(reg) <SystemTraits as port::EntryPoint>::TRAP_HANDLER,
                    );
                    <SystemTraits as port::EntryPoint>::start();
                }
            }
        }

        #[cfg(feature = "interrupt-e310x")]
        use_interrupt_e310x!(unsafe impl InterruptController for SystemTraits);

        #[cfg(feature = "interrupt-u540-qemu")]
        port::use_plic!(unsafe impl InterruptController for SystemTraits);
        #[cfg(feature = "interrupt-u540-qemu")]
        impl port::PlicOptions for SystemTraits {
            const MAX_PRIORITY: InterruptPriority = 7;
            const MAX_NUM: InterruptNum = 53;
            const PLIC_BASE: usize = 0x0c00_0000;
            const CONTEXT: usize = 1;
        }

        #[cfg(feature = "interrupt-k210")]
        port::use_plic!(unsafe impl InterruptController for SystemTraits);
        #[cfg(feature = "interrupt-k210")]
        impl port::PlicOptions for SystemTraits {
            const MAX_PRIORITY: InterruptPriority = 7;
            const MAX_NUM: InterruptNum = 65;
            const PLIC_BASE: usize = 0x0c00_0000;
            const CONTEXT: usize = 0;
        }

        #[cfg(feature = "timer-clint")]
        impl port::MtimeOptions for SystemTraits {
            const MTIME_PTR: usize = 0x0200_bff8;

            #[cfg(any(
                feature = "board-e310x-red-v",
                feature = "board-e310x-qemu",
                feature = "board-maix"
            ))]
            const MTIMECMP_PTR: usize = 0x0200_4000;
            #[cfg(feature = "board-u540-qemu")]
            const MTIMECMP_PTR: usize = 0x0200_4008 /* kernel runs on hart 1 */;

            #[cfg(any(feature = "board-e310x-red-v", feature = "board-e310x-qemu"))]
            const FREQUENCY: u64 = e310x::MTIME_FREQUENCY;
            #[cfg(feature = "board-u540-qemu")]
            const FREQUENCY: u64 = u540::MTIME_FREQUENCY;
            #[cfg(feature = "board-maix")]
            const FREQUENCY: u64 = k210::MTIME_FREQUENCY;

            // Updating `mtime` is not supported by QEMU.
            const RESET_MTIME: bool = false;
        }

        #[cfg(feature = "timer-sbi")]
        impl port::SbiTimerOptions for SystemTraits {
            #[cfg(feature = "board-u540-qemu")]
            const FREQUENCY: u64 = u540::MTIME_FREQUENCY;
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
                unsafe {
                    let mcycle;
                    core::arch::asm!("csrr {}, mcycle", out(reg)mcycle);
                    mcycle
                }
            }

            const PERFORMANCE_TIME_UNIT: &'static str = "cycle(s)";

            #[cfg(feature = "interrupt-e310x")]
            const INTERRUPT_LINES: &'static [InterruptNum] = &[
                crate::interrupt_e310x::INTERRUPT_GPIO0,
                // `USE_NESTING` is only enabled on QEMU
                #[cfg(feature = "board-e310x-qemu")]
                crate::interrupt_e310x::INTERRUPT_GPIO1,
            ];
            const INTERRUPT_PRIORITIES: &'static [InterruptPriority] = &[6, 2];
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
            #[cfg(feature = "interrupt-e310x")]
            const INTERRUPT_LINES: &'static [InterruptNum] = &[
                crate::interrupt_e310x::INTERRUPT_GPIO0,
                // `USE_NESTING` is only enabled on QEMU
                #[cfg(feature = "board-e310x-qemu")]
                crate::interrupt_e310x::INTERRUPT_GPIO1,
            ];
            const INTERRUPT_PRIORITIES: &'static [InterruptPriority] = &[6, 2];
        }

        static COTTAGE: test_case::App<System> =
            r3_kernel::build!(SystemTraits, configure_app => test_case::App<System>);

        const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> test_case::App<System> {
            // Initialize the clock
            #[cfg(any(feature = "board-e310x-red-v", feature = "board-e310x-qemu"))]
            StartupHook::define().start(|| {
                e310x::clocks();
            }).finish(b);

            #[cfg(feature = "interrupt-e310x")]
            SystemTraits::configure_interrupt(b);

            #[cfg(feature = "interrupt-u540-qemu")]
            SystemTraits::configure_plic(b);

            #[cfg(feature = "interrupt-k210")]
            SystemTraits::configure_plic(b);

            SystemTraits::configure_timer(b);

            // Initialize RTT (Real-Time Transfer) with two up channels and set
            // the first one as the print channel for the printing macros, and
            // the second one as log output
            #[cfg(feature = "output-rtt")]
            StartupHook::define().start(|| {
                let channels = rtt_target::rtt_init! {
                    up: {
                        0: {
                            size: 512
                            mode: BlockIfFull
                            name: "Terminal"
                        }
                        1: {
                            size: 1024
                            mode: NoBlockSkip
                            name: "Log"
                        }
                    }
                };

                unsafe {
                    rtt_target::set_print_channel_cs(
                        channels.up.0,
                        &((|arg, f| f(arg)) as rtt_target::CriticalSectionFunc),
                    )
                };
                logger_rtt::init(channels.up.1);
            }).finish(b);

            // Redirect the log output to stderr
            #[cfg(feature = "output-uart")]
            StartupHook::define().start(|| {
                logger_uart::init();
            }).finish(b);

            test_case::App::new::<_, Driver>(b)
        }

        /// Like `riscv::interrupt::free` but uses the correct CSR for the
        /// application's privilege level
        #[inline]
        fn with_cpu_lock(f: impl FnOnce()) {
            use r3::{prelude::*, kernel::CpuLockError};
            struct Guard(Result<(), CpuLockError>);
            let _guard = Guard(System::acquire_cpu_lock());
            f();
            impl Drop for Guard {
                fn drop(&mut self) {
                    if self.0.is_ok() {
                        let _ = unsafe { System::release_cpu_lock() };
                    }
                }
            }
        }
    };

    () => {}
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
// Generated by `build.rs`. Invokes `instantiate_test!` when a driver-defined
// test is requeseted.
include!(concat!(env!("OUT_DIR"), "/gen.rs"));

#[cfg(feature = "kernel_tests")]
mod driver_kernel_tests {
    pub mod execute_lr_sc;
}

#[cfg(not(feature = "run"))]
fn main() {
    panic!("This executable should not be invoked directly");
}

// Wildcard imports take less precedence
#[allow(unused_imports)]
use default_impl::*;
#[allow(dead_code)]
mod default_impl {
    #[track_caller]
    fn with_cpu_lock(_: impl FnOnce()) {
        unreachable!();
    }
}
