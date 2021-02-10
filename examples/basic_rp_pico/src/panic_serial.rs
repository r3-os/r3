use core::panic::PanicInfo;

// Install a global panic handler that uses the serial port
#[inline(never)]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Disable IRQ
    unsafe { asm!("cpsid i") };

    // TODO: output panic info

    loop {}
}
