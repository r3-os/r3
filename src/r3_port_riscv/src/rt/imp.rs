use crate::EntryPoint;

pub unsafe fn setup_interrupt_handler<System: EntryPoint>() {
    unsafe {
        core::arch::asm!("csrw mtvec, {}", in(reg) System::TRAP_HANDLER);
    }
}
