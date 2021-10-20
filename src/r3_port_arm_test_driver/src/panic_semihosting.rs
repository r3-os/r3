use arm_semihosting::{debug, debug::EXIT_FAILURE, hio};
use arrayvec::ArrayString;
use core::{fmt::Write, panic::PanicInfo};

static mut BUFFER: ArrayString<512> = ArrayString::new_const();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts
    unsafe { asm!("cpsid i") };

    if let Ok(mut hstdout) = hio::hstdout() {
        // The test runner stops reading the output when it encounters a stop
        // word (`panicked at`). Actually it continues reading for some time,
        // but semihosting output incurs a huge delay on each call and the
        // `Display` implementation of `PanicInfo` produces a message in small
        // chunks, so the test runner would stop reading after the first chunk
        // (`panicked at '`).
        //
        // To avoid this problem, put the whole message in a buffer and send it
        // with a single semihosting call.
        let buffer = unsafe { &mut BUFFER };
        buffer.clear();
        let _ = writeln!(buffer, "{}", info);

        let _ = write!(hstdout, "{}", buffer);
    }
    debug::exit(EXIT_FAILURE);

    loop {}
}
