use crate::EntryPoint;

pub unsafe fn setup_interrupt_handler<System: EntryPoint>() {
    unsafe {
        let int_handler: unsafe fn() -> ! = System::exception_handler;
        asm!("csrw mtvec, {}", in(reg) int_handler);
    }
}
