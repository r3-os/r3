use r3_support_rza1::sprintln;
use core::panic::PanicInfo;

// Install a global panic handler that uses the serial port
#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable IRQ
    unsafe { llvm_asm!("cpsid i"::::"volatile") };

    sprintln!("{}", info);

    loop {}
}
