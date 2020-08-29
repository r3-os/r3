use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts
    unsafe { riscv::register::mstatus::clear_mie() };

    crate::uart::stdout_write_fmt(format_args!("{}\n", info));

    loop {}
}
