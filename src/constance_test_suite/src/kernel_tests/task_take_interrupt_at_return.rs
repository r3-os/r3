//! Pends an interrupt line before exiting a task holding CPU Lock. The
//! interrupt should be handled when the task releases CPU Lock as part of the
//! required behavior of `exit_task`, which is called implicitly when the
//! entry point function returns. The interrupt handler re-activates this task.
//!
//! # Bug Case with `::constance_port_arm_m`
//!
//! This test case was written to detect a particular bug in the Arm-M port. At
//! one point, the port was implemented in this way:
//!
//! ```rust,ignore
//! // pseudocode
//!
//! impl PortThreading for System {
//!     fn exit_and_dispatch() {
//!         // `state().running_task` is `None` at this point.
//!         leave_cpu_lock();
//!
//!         // (1)
//!         // Pend SVC (this is synchronous - if we did this with CPU Lock
//!         // active, the processor would escalate it to HardFault)
//!         asm!("svc 42");
//!     }
//!
//!     fn yield_cpu() {
//!         // Pend PendSV (this is asynchronous)
//!         pend_pend_sv();
//!     }
//! }
//!
//! fn svc_handler() {      // SVC handler
//!     // Restore the task's stack pointer from TCB
//!     // Pop additional registers from the task's stack
//!     restore_the_context_of_running_task();
//! }
//!
//! fn pend_sv_handler() {  // PendSV handler
//!     // Push additional registers to the task's stack
//!     // Copy the task's stack pointer to TCB
//!     assert!(state().running_task.is_some());
//!     save_the_context_of_running_task(); // (2)
//!
//!     // Update `state().running_task`
//!     System::choose_running_task()
//!
//!     // Restore the task's stack pointer from TCB
//!     // Pop additional registers from the task's stack
//!     if state().running_task.is_none() { todo!(); }
//!     restore_the_context_of_running_task();
//! }
//! ```
//!
//! One invariant that the Arm-M port maintains is that the current *Thread
//! mode* context is equal to the one associated with `state().running_task`.
//! This explains why `pend_sv_handler` works.
//! However, this invariant is temporarily broken while cleaning up a exited
//! task, when the kernel clears `running_task` before calling
//! `exit_and_dispatch`. This special case is handled by the use of SVC (1).
//!
//! However, if an interupt is taken before reaching (1) and the interrupt
//! handler calls `yield_cpu`, `pend_sv_handler` will be called right after
//! `leave_cpu_lock`. `pend_sv_handler` is not prepared to handle `running_task`
//! being `None` and trips an assertion (2).
//!
//! Also, when the interrupt handler re-activates the original task, it might
//! corrupt the exception frame corresponding to the current interrupt
//! activation by overwriting a part of it, causing an unpredictable behavior
//! on return.
//!
//! ```text
//!
//!   Top → ┌───────────┐        ┌───────────┐
//!         │           │        │           │
//!         ├───────────┤        │ Exception │
//!         │           │        │   Frame   │
//!         │ Exception │        │           │
//!         │   Frame   │        ├───────────┤
//!         │           │        │ Corrupted │
//!         ├───────────┤        ├───────────┤
//!         │           │ ← PSP  │           │ ← PSP
//!         │           │        │           │
//!         │           │        │           │
//!         │           │        │           │
//!         └───────────┘        └───────────┘
//!
//!         Handler entry            After
//!                          initialize_task_state
//!
//! ```
use constance::{
    kernel::{Hunk, InterruptHandler, InterruptLine, Task},
    prelude::*,
};

use super::Driver;
use crate::utils::SeqTracker;

pub struct App<System> {
    task: Task<System>,
    int: Option<InterruptLine<System>>,
    seq: Hunk<System, SeqTracker>,
    state: Hunk<System, SeqTracker>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            let task = new! { Task<_>, start = task_body::<System, D>, priority = 2, active = true };

            let int = if let [int_line, ..] = *D::INTERRUPT_LINES {
                new! { InterruptHandler<_>, line = int_line, start = isr::<System, D> };

                Some(new! { InterruptLine<_>,
                    line = int_line, priority = D::INTERRUPT_PRIORITY_LOW, enabled = true })
            } else {
                None
            };

            let seq = new! { Hunk<_, SeqTracker> };
            let state = new! { Hunk<_, SeqTracker> };

            App { task, int, seq, state }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    if D::app().state.get() == 0 {
        // The first run of `task`
        D::app().state.expect_and_replace(0, 1);

        // Acquire CPU Lock, which will be released when the task exits
        System::acquire_cpu_lock().unwrap();

        let int = if let Some(int) = D::app().int {
            int
        } else {
            log::warn!("No interrupt lines defined, skipping the test");
            D::success();
            return;
        };

        D::app().seq.expect_and_replace(0, 1);
        int.pend().unwrap();
        // When the task exits, `isr` will execute and re-activate `task`
        D::app().seq.expect_and_replace(1, 2);
    } else if D::app().state.get() == 1 {
        // The second run of `task`
        D::app().seq.expect_and_replace(4, 5);
        D::success();
    }
}

fn isr<System: Kernel, D: Driver<App<System>>>(_: usize) {
    D::app().seq.expect_and_replace(2, 3);
    D::app().task.activate().unwrap();
    D::app().seq.expect_and_replace(3, 4);
}