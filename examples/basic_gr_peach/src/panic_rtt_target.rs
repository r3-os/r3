use core::{fmt::Write, panic::PanicInfo};
use rtt_target::{ChannelMode, UpChannel};

// Install a global panic handler that uses RTT
#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable IRQ
    unsafe { llvm_asm!("cpsid i"::::"volatile") };

    if let Some(mut channel) = unsafe { UpChannel::conjure(0) } {
        channel.set_mode(ChannelMode::BlockIfFull);

        writeln!(channel, "{}", info).ok();
    }

    loop {}
}
