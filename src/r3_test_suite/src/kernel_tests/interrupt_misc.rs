//! Validates error codes returned by interrupt line manipulation methods. Also,
//! checks miscellaneous properties of interrupt lines.
use core::sync::atomic::{AtomicBool, Ordering};
use r3::{
    hunk::Hunk,
    kernel::{
        self, prelude::*, traits, Cfg, InterruptLine, StartupHook, StaticInterruptHandler,
        StaticTask,
    },
};

use super::Driver;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    int: Option<InterruptLine<System>>,
    interrupt_expected: Hunk<System, AtomicBool>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgInterruptLine,
    {
        StaticTask::define()
            .start(task_body::<System, D>)
            .priority(0)
            .active(true)
            .finish(b);

        StartupHook::define()
            .start(startup_hook::<System, D>)
            .finish(b);

        let interrupt_expected = Hunk::<System, AtomicBool>::define().finish(b);

        let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
            unsafe {
                StaticInterruptHandler::define()
                    .line(int_line)
                    .start(isr::<System, D>)
                    .unmanaged()
                    .finish(b);
            }

            Some(InterruptLine::define().line(int_line).finish(b))
        } else {
            None
        };

        App {
            int,
            interrupt_expected,
        }
    }
}

fn startup_hook<System: SupportedSystem, D: Driver<App<System>>>() {
    let Some(int) = D::app().int else { return };

    let managed_range = System::RAW_MANAGED_INTERRUPT_PRIORITY_RANGE;

    // `set_priority` is disallowed in a boot context
    assert_eq!(
        int.set_priority(managed_range.start),
        Err(kernel::SetInterruptLinePriorityError::BadContext),
    );

    // Other methods are allowed in a boot context
    int.enable().unwrap();
    int.disable().unwrap();
    match int.is_pending() {
        Ok(false) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
        value => panic!("{value:?}"),
    }

    // Before doing the next test, make sure `clear` is supported
    // There's the same test in `task_body`. The difference is that this one
    // here executes in a boot context.
    if int.clear().is_ok() {
        int.pend().unwrap();
        match int.is_pending() {
            Ok(true) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
            value => panic!("{value:?}"),
        }
        int.clear().unwrap();
    }
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    let Some(int) = D::app().int
    else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    let managed_range = System::RAW_MANAGED_INTERRUPT_PRIORITY_RANGE;

    if managed_range.end > managed_range.start {
        for pri in managed_range.clone() {
            int.set_priority(pri).unwrap();
        }

        for pri in managed_range.clone() {
            unsafe { int.set_priority_unchecked(pri) }.unwrap();
        }

        // `set_priority` is disallowed when CPU Lock is active
        System::acquire_cpu_lock().unwrap();
        assert_eq!(
            int.set_priority(managed_range.start),
            Err(kernel::SetInterruptLinePriorityError::BadContext),
        );
        assert_eq!(
            unsafe { int.set_priority_unchecked(managed_range.start) },
            Err(kernel::SetInterruptLinePriorityError::BadContext),
        );
        unsafe { System::release_cpu_lock() }.unwrap();
    }

    // `set_priority` rejects unmanaged priority
    if let Some(pri) = managed_range.start.checked_sub(1) {
        assert_eq!(
            int.set_priority(pri),
            Err(kernel::SetInterruptLinePriorityError::BadParam),
        );
    }
    assert_eq!(
        int.set_priority(managed_range.end),
        Err(kernel::SetInterruptLinePriorityError::BadParam),
    );

    int.enable().unwrap();

    // Before doing the next test, make sure `clear` is supported
    if int.clear().is_ok() {
        // Pending the interrupt should succeed. We instantly clear the pending
        // flag, so the interrupt handler will not actually get called.
        System::acquire_cpu_lock().unwrap();
        int.pend().unwrap();
        match int.is_pending() {
            Ok(true) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
            value => panic!("{value:?}"),
        }
        int.clear().unwrap();
        unsafe { System::release_cpu_lock() }.unwrap();

        // Pending the interrupt should succeed. The interrupt line is disabled,
        // so the interrupt handler will not actually get called.
        int.disable().unwrap();
        int.pend().unwrap();
        match int.is_pending() {
            Ok(true) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
            value => panic!("{value:?}"),
        }
        int.clear().unwrap();
        int.enable().unwrap();
    }

    match int.is_pending() {
        Ok(false) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
        value => panic!("{value:?}"),
    }

    if let &[pri, ..] = D::INTERRUPT_PRIORITIES {
        D::app().interrupt_expected.store(true, Ordering::Relaxed);
        log::debug!("Pending the interrupt line");
        int.set_priority(pri).unwrap();
        int.pend().unwrap();
    } else {
        log::warn!("No interrupt priorities defined, skipping the rest of the test");
        D::success();
    }
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>() {
    log::debug!("The interrupt handler is running");
    assert!(D::app().interrupt_expected.load(Ordering::Relaxed));

    // Context query
    assert!(!System::is_task_context());
    assert!(System::is_interrupt_context());
    assert!(System::is_boot_complete());

    D::success();
}
