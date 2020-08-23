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

    fn run(&self, func: impl FnOnce()) {
        if let Err(panic_info) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(func)) {
            std::panic::resume_unwind(panic_info);
        }

        if self.is_successful.load(Ordering::Relaxed) {
            return;
        }

        panic!("The program deadlocked without calling `success`");
    }
}

macro_rules! instantiate_kernel_tests {
    ( $( { $($tt:tt)* }, )* ) => {
        instantiate_kernel_tests!(
            @inner

            $( { $($tt)* }, )*

            // Port-specific tests (none)
        );
    };
    ( @inner $(
        { path: $path:path, name_ident: $name_ident:ident, name_str: $name_str:literal $($rest:tt)* },
    )*) => {
        $(mod $name_ident {
            use constance::kernel::{InterruptNum, InterruptPriority};
            use constance_test_suite::kernel_benchmarks;
            use $path as test_case;

            constance_port_std::use_port!(unsafe struct System);

            struct Driver;
            static TEST_UTIL: super::KernelTestUtil = super::KernelTestUtil::new();

            impl kernel_benchmarks::Driver<test_case::App<System>> for Driver {
                fn app() -> &'static test_case::App<System> {
                    &COTTAGE
                }

                fn success() {
                    TEST_UTIL.success::<System>();
                }

                fn performance_time() -> u32 {
                    port_std_impl::PORT_STATE.tick_count::<System>()
                }

                const PERFORMANCE_TIME_UNIT: &'static str = "μs";

                const INTERRUPT_LINES: &'static [InterruptNum] = &[0, 1, 2, 3];
                const INTERRUPT_PRIORITY_LOW: InterruptPriority = 4;
                const INTERRUPT_PRIORITY_HIGH: InterruptPriority = 0;
            }

            static COTTAGE: test_case::App<System> =
                constance::build!(System, test_case::App::new::<Driver> => test_case::App<System>);

            pub fn run() {
                TEST_UTIL.run(|| {
                    port_std_impl::PORT_STATE.port_boot::<System>();
                });
            }
        })*

        static KERNEL_BENCHMARKS: &[(&str, fn())] = &[
            $( ($name_str, $name_ident::run) ),*
        ];
    };
}

constance_test_suite::get_kernel_benchmarks!(instantiate_kernel_tests!());

fn main() {
    env_logger::from_env(
        env_logger::Env::default().default_filter_or("constance_test_suite=info,test_suite=info"),
    )
    .init();

    for (name, entry) in KERNEL_BENCHMARKS {
        log::info!("--- kernel benchmark '{}' ---", name);
        entry();
    }
}
