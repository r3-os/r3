//! Make sure startup hooks are called in the ascending order of priority.
use constance::{
    hunk::Hunk,
    kernel::{cfg::CfgBuilder, StartupHook},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    seq: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    pub const fn new<D: Driver<Self>>(b: &mut CfgBuilder<System>) -> Self {
        StartupHook::build()
            .start(hook::<System, D>)
            .param(0)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(11)
            .priority(30)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(9)
            .priority(10)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(1)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(15)
            .priority(70)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(13)
            .priority(50)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(2)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(12)
            .priority(40)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(3)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(4)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(5)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(18)
            .priority(100)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(6)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(17)
            .priority(90)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(7)
            .priority(5)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(16)
            .priority(80)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(10)
            .priority(20)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(14)
            .priority(60)
            .finish(b);
        StartupHook::build()
            .start(hook::<System, D>)
            .param(8)
            .priority(5)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::build().finish(b);

        App { seq }
    }
}

fn hook<System: Kernel, D: Driver<App<System>>>(i: usize) {
    log::trace!("hook({})", i);
    D::app().seq.expect_and_replace(i, i + 1);

    if i == 18 {
        D::success();
    }
}
