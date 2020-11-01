//! Measures the execution times of port operations.
use core::marker::PhantomData;
use r3::kernel::{cfg::CfgBuilder, Kernel};

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: AppInner<System>,
    }
}

struct AppInner<System> {
    _phantom: PhantomData<System>,
}

const I_ENTER_CPU_LOCK: Interval = "`enter_cpu_lock`";
const I_TRY_ENTER_CPU_LOCK: Interval = "`try_enter_cpu_lock`";
const I_LEAVE_CPU_LOCK: Interval = "`leave_cpu_lock`";
const I_YIELD_CPU: Interval = "`yield_cpu`";

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<System, Self>>(_: &mut CfgBuilder<System>) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<System, Self>>() {
        B::mark_start();
        unsafe { System::enter_cpu_lock() };
        B::mark_end(I_ENTER_CPU_LOCK);

        B::mark_start();
        unsafe { System::leave_cpu_lock() };
        B::mark_end(I_LEAVE_CPU_LOCK);

        B::mark_start();
        unsafe { System::try_enter_cpu_lock() };
        B::mark_end(I_TRY_ENTER_CPU_LOCK);
        unsafe { System::leave_cpu_lock() };

        B::mark_start();
        unsafe { System::yield_cpu() };
        B::mark_end(I_YIELD_CPU);
    }
}
