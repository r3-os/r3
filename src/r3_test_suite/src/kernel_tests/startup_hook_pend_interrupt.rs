//! Pends an interrupt line in a startup hook. The interrupt handler activates
//! a task.
//!
//! # Bug Case with `::r3_port_arm_m`
//!
//! This test case was written to detect a particular bug in the Arm-M port. At
//! one point, the port was implemented in this way:
//!
//! ```rust,ignore
//! // pseudocode
//!
//! impl PortThreading for System {
//!     fn dispatch_first_task() {
//!         // `state().running_task` is already set at this point.
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
//!     save_the_context_of_running_task(); // (2)
//!
//!     // Update `state().running_task`
//!     System::choose_running_task()
//!
//!     // Restore the task's stack pointer from TCB
//!     // Pop additional registers from the task's stack
//!     restore_the_context_of_running_task();
//! }
//! ```
//!
//! One invariant that the Arm-M port maintains is that the current *Thread
//! mode* context is equal to the one associated with `state().running_task`.
//! This explains why `pend_sv_handler` works.
//! However, this invariant is temporarily broken during the boot process, when
//! the kernel assigns `running_task` before calling `dispatch_first_task`. This
//! special case is handled by the use of SVC (1).
//!
//! However, if an interupt is taken before reaching (1) and the interrupt
//! handler calls `yield_cpu`, `pend_sv_handler` will be called right after
//! `leave_cpu_lock`. `pend_sv_handler` is not prepared to handle this special
//! case and will cause BusFault trying to access the task's stack, whose
//! pointer is not assigned to `PSP` (Process Stack Pointer) at this point yet
//! (2).
use r3::{
    hunk::Hunk,
    kernel::{
        prelude::*, traits, Cfg, InterruptLine, StartupHook, StaticInterruptHandler, StaticTask,
    },
};

use super::Driver;
use crate::utils::SeqTracker;

pub trait SupportedSystem:
    traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic
{
}
impl<T: traits::KernelBase + traits::KernelInterruptLine + traits::KernelStatic> SupportedSystem
    for T
{
}

pub struct App<System: SupportedSystem> {
    task: StaticTask<System>,
    int: Option<InterruptLine<System>>,
    seq: Hunk<System, SeqTracker>,
}

impl<System: SupportedSystem> App<System> {
    pub const fn new<C, D: Driver<Self>>(b: &mut Cfg<C>) -> Self
    where
        C: ~const traits::CfgTask<System = System> + ~const traits::CfgInterruptLine,
    {
        StartupHook::define().start(hook::<System, D>).finish(b);

        let task = StaticTask::define()
            .start(task_body::<System, D>)
            .priority(0)
            .finish(b);

        let int = if let (&[int_line, ..], &[int_pri, ..]) =
            (D::INTERRUPT_LINES, D::INTERRUPT_PRIORITIES)
        {
            StaticInterruptHandler::define()
                .line(int_line)
                .start(isr::<System, D>)
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

        let seq = Hunk::<_, SeqTracker>::define().finish(b);

        App { task, int, seq }
    }
}

fn hook<System: SupportedSystem, D: Driver<App<System>>>() {
    let Some(int) = D::app().int 
    else {
        log::warn!("No interrupt lines defined, skipping the test");
        D::success();
        return;
    };

    D::app().seq.expect_and_replace(0, 1);
    int.pend().unwrap();
    D::app().seq.expect_and_replace(1, 2);
}

fn isr<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(2, 3);
    D::app().task.activate().unwrap();
    D::app().seq.expect_and_replace(3, 4);
}

fn task_body<System: SupportedSystem, D: Driver<App<System>>>() {
    D::app().seq.expect_and_replace(4, 5);
    D::success();
}
