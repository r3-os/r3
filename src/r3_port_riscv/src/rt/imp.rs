use crate::EntryPoint;

pub unsafe fn setup_interrupt_handler<System: EntryPoint>() {
    unsafe {
        let int_handler: unsafe fn() -> ! = exception_handler::<System>;
        asm!("csrw mtvec, {}", in(reg) int_handler);
    }
}

#[naked]
unsafe fn exception_handler<System: EntryPoint>() -> ! {
    // Align `exception_handler` to a 4-byte boundary
    // (Required by `mtvec`)
    unsafe { asm!(".align 2") };

    unsafe { System::exception_handler() };
}
