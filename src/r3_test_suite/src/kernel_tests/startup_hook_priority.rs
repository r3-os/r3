//! Make sure startup hooks are called in the ascending order of priority.
use r3::{
    hunk::Hunk,
    kernel::{traits, Cfg, StartupHook},
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem: traits::KernelBase + traits::KernelStatic {}
impl<T: traits::KernelBase + traits::KernelStatic> SupportedSystem for T {}

pub struct App<System: SupportedSystem> {
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask,
    {
        StartupHook::define()
            .start((0, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((11, hook::<System, D>))
            .priority(30)
            .finish(b);
        StartupHook::define()
            .start((9, hook::<System, D>))
            .priority(10)
            .finish(b);
        StartupHook::define()
            .start((1, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((15, hook::<System, D>))
            .priority(70)
            .finish(b);
        StartupHook::define()
            .start((13, hook::<System, D>))
            .priority(50)
            .finish(b);
        StartupHook::define()
            .start((2, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((12, hook::<System, D>))
            .priority(40)
            .finish(b);
        StartupHook::define()
            .start((3, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((4, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((5, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((18, hook::<System, D>))
            .priority(100)
            .finish(b);
        StartupHook::define()
            .start((6, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((17, hook::<System, D>))
            .priority(90)
            .finish(b);
        StartupHook::define()
            .start((7, hook::<System, D>))
            .priority(5)
            .finish(b);
        StartupHook::define()
            .start((16, hook::<System, D>))
            .priority(80)
            .finish(b);
        StartupHook::define()
            .start((10, hook::<System, D>))
            .priority(20)
            .finish(b);
        StartupHook::define()
            .start((14, hook::<System, D>))
            .priority(60)
            .finish(b);
        StartupHook::define()
            .start((8, hook::<System, D>))
            .priority(5)
            .finish(b);

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { seq }
    }
}

fn hook<System: SupportedSystem, D: Driver<App<System>>>(i: usize) {
    log::trace!("hook({})", i);
    D::app().seq.expect_and_replace(i, i + 1);

    if i == 18 {
        D::success();
    }
}
