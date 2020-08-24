//! TODO
use constance::kernel::{cfg::CfgBuilder, Kernel};
use core::marker::PhantomData;

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub struct App<System> {
        inner: AppInner<System>,
    }
}

struct AppInner<System> {
    _phantom: PhantomData<System>,
}

const I_TRACE: Interval = "greeting";

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<Self>>(b: &mut CfgBuilder<System>) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<Self>>() {
        B::mark_start();
        log::trace!("Good morning, Angel!");
        B::mark_end(I_TRACE);
    }
}
