//! Checks that [`KernelStatic::INTERRUPT_HANDLERS`] contains `None` for the
//! elements corresponding to interrupt lines that have no registered interrupt
//! handlers.
use core::marker::PhantomData;
use r3::kernel::{cfg::KernelStatic, traits, Cfg, InterruptHandler, InterruptLine, StartupHook};
use r3_kernel::System;
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
        StartupHook::build().start(hook_body::<Traits, D>).finish(b);

        InterruptLine::build().line(2).priority(64).finish(b);
        InterruptLine::build().line(3).priority(64).finish(b);

        unsafe {
            InterruptHandler::build().line(2).start(|_| {}).finish(b);
            InterruptHandler::build().line(3).start(|_| {}).finish(b);
            InterruptHandler::build()
                .line(5)
                .unmanaged()
                .start(|_| {})
                .finish(b);
            InterruptHandler::build()
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
    let handlers = <System<Traits> as KernelStatic>::INTERRUPT_HANDLERS;
    log::debug!("INTERRUPT_HANDLERS = {:#?}", handlers);
    assert_eq!(handlers.len(), 8);
    assert_eq!(handlers[0], None);
    assert_eq!(handlers[1], None);
    assert!(handlers[2].is_some());
    assert!(handlers[3].is_some());
    assert_eq!(handlers[4], None);
    assert!(handlers[5].is_some());
    assert_eq!(handlers[6], None);
    assert!(handlers[7].is_some());
    D::success();
}
