//! Validates error codes returned by interrupt line manipulation methods. Also,
//! checks miscellaneous properties of interrupt lines.
use constance::{
    kernel::{self, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;

pub struct App<System> {
    int: Option<InterruptLine<System>>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! { Task<_>, start = task_body::<System, D>, priority = 0, active = true };

            let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
                unsafe {
                    new! { InterruptHandler<_>,
                        line = int_line, start = isr::<System, D>, unmanaged };
                }

                Some(new! { InterruptLine<_>, line = int_line })
            } else {
                None
            };

            App { int }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let int = if let Some(int) = D::app().int {
        int
    } else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    let managed_range = System::MANAGED_INTERRUPT_PRIORITY_RANGE;

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
            value => panic!("{:?}", value),
        }
        int.clear().unwrap();
        unsafe { System::release_cpu_lock() }.unwrap();

        // Pending the interrupt should succeed. The interrupt line is disabled,
        // so the interrupt handler will not actually get called.
        int.disable().unwrap();
        int.pend().unwrap();
        match int.is_pending() {
            Ok(true) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
            value => panic!("{:?}", value),
        }
        int.clear().unwrap();
        int.enable().unwrap();
    }

    match int.is_pending() {
        Ok(false) | Err(kernel::QueryInterruptLineError::NotSupported) => {}
        value => panic!("{:?}", value),
    }

    D::success();
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    unreachable!();
}
