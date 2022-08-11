//! Pends an interrupt from an external thread.
use r3_core::{
    hunk::Hunk,
    kernel::{traits, Cfg, InterruptLine, StaticInterruptHandler, StaticTask},
};
use r3_kernel::System;
use r3_test_suite::kernel_tests::Driver;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::{sleep, spawn},
    time::Duration,
};

use r3_port_std::PortInstance;

pub trait SupportedSystemTraits: PortInstance {}
impl<T: PortInstance> SupportedSystemTraits for T {}

pub struct App<System: traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic> {
    int: Option<InterruptLine<System>>,
    done: Hunk<System, AtomicBool>,
}

impl<Traits: SupportedSystemTraits> App<System<Traits>> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System<Traits>> + ~const traits::CfgInterruptLine,
    {
        StaticTask::define()
            .start(task_body1::<Traits, D>)
            .priority(1)
            .active(true)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            StaticInterruptHandler::define()
                .line(int_line)
                .start(isr::<Traits, D>)
                .finish(b);

            Some(
                InterruptLine::define()
                    .line(int_line)
                    .priority(int_pri)
                    .enabled(true)
                    .finish(b),
            )
        } else {
            None
        };

        let done = Hunk::<_, AtomicBool>::define().finish(b);

        App { int, done }
    }
}

fn task_body1<Traits: SupportedSystemTraits, D: Driver<App<System<Traits>>>>() {
    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    // Spawn a host thread
    log::debug!("spawning an external thread");
    spawn(move || {
        sleep(Duration::from_millis(100));
        log::debug!("pending {:?}", int);
        r3_port_std::pend_interrupt_line::<Traits>(int.num()).unwrap();
    });

    log::debug!("waiting for `done` to be set...");
    while !D::app().done.load(Ordering::Relaxed) {}
    log::debug!("success!");

    D::success();
}

fn isr<Traits: SupportedSystemTraits, D: Driver<App<System<Traits>>>>() {
    D::app().done.store(true, Ordering::Relaxed);
}
