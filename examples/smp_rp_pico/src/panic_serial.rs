use core::panic::PanicInfo;

// Install a global panic handler that uses the serial port
#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable IRQ
    unsafe { asm!("cpsid i") };

    // Check which core we are running on
    let p = unsafe { rp2040::Peripherals::steal() };
    let cpuid = p.SIO.cpuid.read().bits();

    match cpuid {
        0 => {
            r3_support_rp2040::sprintln!("{}", info);

            loop {
                r3_support_rp2040::usbstdio::poll::<crate::core0::System>();
            }
        }
        1 => {
            use crate::core1;
            core1::write_fmt(core1::Core1::new(&p.SIO).unwrap(), format_args!("{}", info));

            // Halt the system
            loop {
                unsafe { asm!("") };
            }
        }
        _ => loop {},
    }
}
