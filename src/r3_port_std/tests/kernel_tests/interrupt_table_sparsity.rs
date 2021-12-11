//! Checks that [`KernelCfg2::INTERRUPT_HANDLERS`] contains `None` for the
//! elements corresponding to interrupt lines that have no registered interrupt
//! handlers.
use core::marker::PhantomData;
use r3::kernel::{traits, Cfg, InterruptLine, StartupHook, StaticInterruptHandler};
use r3_kernel::{KernelCfg2, System};
use r3_test_suite::kernel_tests::Driver;

use r3_port_std::PortInstance;

pub trait SupportedSystemTraits: PortInstance {}
impl<T: PortInstance> SupportedSystemTraits for T {}

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<Traits: SupportedSystemTraits> App<System<Traits>> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgBase<System = System<Traits>> + ~const traits::CfgInterruptLine,
    {
        StartupHook::define()
            .start(hook_body::<Traits, D>)
            .finish(b);

        InterruptLine::define().line(2).priority(64).finish(b);
        InterruptLine::define().line(3).priority(64).finish(b);

        unsafe {
            StaticInterruptHandler::define()
                .line(2)
                .start(|_| {})
                .finish(b);
            StaticInterruptHandler::define()
                .line(3)
                .start(|_| {})
                .finish(b);
            StaticInterruptHandler::define()
                .line(5)
                .unmanaged()
                .start(|_| {})
                .finish(b);
            StaticInterruptHandler::define()
                .line(7)
                .unmanaged()
                .start(|_| {})
                .finish(b);
        }

        App {
            _phantom: PhantomData,
        }
    }
}

fn hook_body<Traits: SupportedSystemTraits, D: Driver<App<System<Traits>>>>(_: usize) {
    let handlers = <Traits as KernelCfg2>::INTERRUPT_HANDLERS;
    log::debug!("INTERRUPT_HANDLERS = {:#?}", handlers);
    assert_eq!(handlers.storage.len(), 8);
    assert_eq!(handlers.get(0), None);
    assert_eq!(handlers.get(1), None);
    assert!(handlers.get(2).is_some());
    assert!(handlers.get(3).is_some());
    assert_eq!(handlers.get(4), None);
    assert!(handlers.get(5).is_some());
    assert_eq!(handlers.get(6), None);
    assert!(handlers.get(7).is_some());
    D::success();
}
