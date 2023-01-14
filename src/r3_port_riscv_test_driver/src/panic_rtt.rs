use core::{arch::asm, fmt::Write, panic::PanicInfo};
use rtt_target::{ChannelMode, UpChannel};

// Install a global panic handler that uses RTT
#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts
    unsafe { asm!("csrci mstatus, 8") };

    if let Some(mut channel) = unsafe { UpChannel::conjure(0) } {
        channel.set_mode(ChannelMode::BlockIfFull);

        writeln!(channel, "{info}").ok();
    }

    loop {}
}
