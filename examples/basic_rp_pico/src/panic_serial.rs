use core::{arch::asm, panic::PanicInfo};

// Install a global panic handler that uses the serial port
#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable IRQ
    unsafe { asm!("cpsid i") };

    r3_support_rp2040::sprintln!("{}", info);

    loop {}
}
