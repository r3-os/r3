use core::{fmt::Write, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts
    unsafe { riscv::register::mstatus::clear_mie() };

    let _ = writeln!(crate::uart::stdout(), "{}", info).ok();

    loop {}
}
