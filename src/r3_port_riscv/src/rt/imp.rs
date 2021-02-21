use crate::EntryPoint;

pub unsafe fn setup_interrupt_handler<System: EntryPoint>() {
    unsafe {
        asm!("csrw mtvec, {}", in(reg) System::TRAP_HANDLER);
    }
}
