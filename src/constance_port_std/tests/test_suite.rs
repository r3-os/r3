//! Runs test cases defined in `constance_test_suite`.
#![feature(const_loop)]
#![feature(const_if_match)]
#![feature(never_type)]
#![feature(const_mut_refs)]

use std::sync::atomic::{AtomicBool, Ordering};

struct KernelTestUtil {
    is_successful: AtomicBool,
}

impl KernelTestUtil {
    const fn new() -> Self {
        Self {
            is_successful: AtomicBool::new(false),
        }
    }

    fn success(&self) {
        self.is_successful.store(true, Ordering::Relaxed);
    }

    fn fail(&self) {
        panic!("test failed");
    }

    fn run(&self, func: impl FnOnce() -> !) {
        let _ = env_logger::try_init();

        let panic_info = std::panic::catch_unwind(std::panic::AssertUnwindSafe(func))
            .err()
            .unwrap();

        // "No task to schedule" is not a failure - it's the only way to stop
        // the dispatcher loop
        if let Some(msg) = panic_info.downcast_ref::<&'static str>() {
            if msg.contains("No task to schedule") {
                if self.is_successful.load(Ordering::Relaxed) {
                    return;
                }

                panic!("The program deadlocked without calling `success`");
            }
        }

        std::panic::resume_unwind(panic_info);
    }
}

macro_rules! instantiate_kernel_tests {
    ($(
        { name_ident: $name_ident:ident, $($rest:tt)* },
    )*) => {$(
        mod $name_ident {
            use constance_test_suite::kernel_tests::{self, $name_ident as test_case};

            constance_port_std::use_port!(unsafe struct System);

            struct Driver;
            static TEST_UTIL: super::KernelTestUtil = super::KernelTestUtil::new();

            impl kernel_tests::Driver<test_case::App<System>> for Driver {
                fn app() -> &'static test_case::App<System> {
                    &COTTAGE
                }
                fn success() {
                    TEST_UTIL.success();
                }
                fn fail() {
                    TEST_UTIL.fail();
                }
            }

            static COTTAGE: test_case::App<System> =
                constance::build!(System, test_case::App::new::<Driver>);

            #[test]
            fn run() {
                TEST_UTIL.run(|| {
                    // Safety: We are a port, so it's okay to call this
                    unsafe {
                        <System as constance::kernel::PortToKernel>::boot();
                    }
                });
            }
        }
    )*};
}

constance_test_suite::get_kernel_tests!(instantiate_kernel_tests!());

// TODO: This would be a good place to add semi-whitebox tests for e.g.,
//       2nd-level interrupt handler generation
