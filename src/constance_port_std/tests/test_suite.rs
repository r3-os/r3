//! Runs test cases defined in `constance_test_suite`.
#![feature(never_type)]
#![feature(const_mut_refs)]
#![feature(const_fn)]
#![feature(slice_ptr_len)]

use constance_port_std::PortInstance;
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

    fn success<System: PortInstance>(&self) {
        self.is_successful.store(true, Ordering::Relaxed);
        constance_port_std::shutdown::<System>();
    }

    fn fail(&self) {
        panic!("test failed");
    }

    fn run(&self, func: impl FnOnce()) {
        let _ = env_logger::try_init();

        if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(func)) {
            std::panic::resume_unwind(panic_info);
        }

        if self.is_successful.load(Ordering::Relaxed) {
            return;
        }

        panic!("The program deadlocked without calling `success`");
    }
}

mod kernel_tests {
    pub mod external_interrupt;
    pub mod interrupt_table_sparsity;
    pub mod stack_align;
}

macro_rules! instantiate_kernel_tests {
    ( $( { $($tt:tt)* }, )* ) => {
        instantiate_kernel_tests!(
            @inner

            $( { $($tt)* }, )*

            // Port-specific tests
            { path: crate::kernel_tests::external_interrupt, name_ident: external_interrupt, },
            { path: crate::kernel_tests::interrupt_table_sparsity, name_ident: interrupt_table_sparsity, },
            { path: crate::kernel_tests::stack_align, name_ident: stack_align, },
        );
    };
    ( @inner $(
        { path: $path:path, name_ident: $name_ident:ident, $($rest:tt)* },
    )*) => {$(
        mod $name_ident {
            use constance::kernel::{InterruptNum, InterruptPriority};
            use constance_test_suite::kernel_tests;
            use $path as test_case;

            constance_port_std::use_port!(unsafe struct System);

            struct Driver;
            static TEST_UTIL: super::KernelTestUtil = super::KernelTestUtil::new();

            impl kernel_tests::Driver<test_case::App<System>> for Driver {
                fn app() -> &'static test_case::App<System> {
                    &COTTAGE
                }

                fn success() {
                    TEST_UTIL.success::<System>();
                }

                fn fail() {
                    TEST_UTIL.fail();
                }

                const INTERRUPT_LINES: &'static [InterruptNum] = &[0, 1, 2, 3];
                const INTERRUPT_PRIORITY_LOW: InterruptPriority = 4;
                const INTERRUPT_PRIORITY_HIGH: InterruptPriority = 0;
            }

            static COTTAGE: test_case::App<System> =
                constance::build!(System, test_case::App::new::<Driver> => test_case::App<System>);

            #[test]
            fn run() {
                TEST_UTIL.run(|| {
                    port_std_impl::PORT_STATE.port_boot::<System>();
                });
            }
        }
    )*};
}

constance_test_suite::get_kernel_tests!(instantiate_kernel_tests!());

// TODO: This would be a good place to add semi-whitebox tests for e.g.,
//       2nd-level interrupt handler generation
