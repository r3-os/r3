use arm_semihosting::{debug, debug::EXIT_FAILURE, hio};
use core::{fmt::Write, panic::PanicInfo};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts
    unsafe { llvm_asm!("cpsid i"::::"volatile") };

    if let Ok(mut hstdout) = hio::hstdout() {
        writeln!(hstdout, "{}", info).ok();
    }
    debug::exit(EXIT_FAILURE);

    loop {}
}
