//! Runs test cases defined in `r3_test_suite`.
#![feature(const_refs_to_cell)]
#![feature(const_mut_refs)]
#![feature(slice_ptr_len)]
#![feature(never_type)]

use r3_port_std::PortInstance;
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

    fn success<Traits: PortInstance>(&self) {
        self.is_successful.store(true, Ordering::Relaxed);
        r3_port_std::shutdown::<Traits>();
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
            use r3_core::kernel::{InterruptNum, InterruptPriority};
            use r3_test_suite::kernel_benchmarks;
            use $path as test_case;

            type System = r3_kernel::System<SystemTraits>;
            r3_port_std::use_port!(unsafe struct SystemTraits);

            struct Driver;
            static TEST_UTIL: super::KernelTestUtil = super::KernelTestUtil::new();

            impl kernel_benchmarks::Driver<test_case::App<System>> for Driver {
                fn app() -> &'static test_case::App<System> {
                    &COTTAGE
                }

                fn success() {
                    TEST_UTIL.success::<SystemTraits>();
                }

                fn performance_time() -> u32 {
                    port_std_impl::PORT_STATE.tick_count::<SystemTraits>()
                }

                const PERFORMANCE_TIME_UNIT: &'static str = "μs";

                const INTERRUPT_LINES: &'static [InterruptNum] = &[0, 1, 2, 3];
                const INTERRUPT_PRIORITIES: &'static [InterruptPriority] = &[0, 4];
            }

            static COTTAGE: test_case::App<System> =
                r3_kernel::build!(SystemTraits, test_case::App::new::<_, Driver> => test_case::App<System>);

            pub fn run() {
                TEST_UTIL.run(|| {
                    port_std_impl::PORT_STATE.port_boot::<SystemTraits>();
                });
            }
        })*

        static KERNEL_BENCHMARKS: &[(&str, fn())] = &[
            $( ($name_str, $name_ident::run) ),*
        ];
    };
}

r3_test_suite::get_kernel_benchmarks!(instantiate_kernel_tests!());

fn main() {
    env_logger::from_env(
        env_logger::Env::default().default_filter_or("r3_test_suite=info,test_suite=info"),
    )
    .init();

    for (name, entry) in KERNEL_BENCHMARKS {
        log::info!("--- kernel benchmark '{}' ---", name);
        entry();
    }
}
