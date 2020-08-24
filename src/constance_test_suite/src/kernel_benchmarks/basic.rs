//! TODO
use constance::{kernel::cfg::CfgBuilder, prelude::*};
use core::marker::PhantomData;

use super::Driver;
use crate::utils::benchmark::{self, Bencher};

pub struct App<System> {
    benchmark: benchmark::BencherCottage<System>,
}

struct BenchmarkOptions<System, D>(PhantomData<(System, D)>);

const I_TRACE: benchmark::Interval = "greeting";

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        App {
            benchmark: benchmark::configure::<System, BenchmarkOptions<System, D>>(b),
        }
    }
}

impl<System: Kernel, D: Driver<App<System>>> benchmark::BencherOptions<System>
    for BenchmarkOptions<System, D>
{
    type App = App<System>;
    type Driver = D;

    fn cottage() -> &'static benchmark::BencherCottage<System> {
        &D::app().benchmark
    }

    fn iter() {
        Self::mark_start();
        log::trace!("Good morning, Angel!");
        Self::mark_end(I_TRACE);
    }
}
