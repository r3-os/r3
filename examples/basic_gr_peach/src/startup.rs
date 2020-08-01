use core::{fmt::Write, panic::PanicInfo};
use register::cpu::{RegisterReadWrite, RegisterWriteOnly};
use rtt_target::{ChannelMode, UpChannel};

use crate::arm;

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

#[link_section = ".vector_table"]
#[naked]
#[no_mangle]
fn vector_table() {
    unsafe {
        llvm_asm!("
            b UnhandledException
            b UndefinedInstruction
            b SupervisorCall
            b PrefetchAbort
            b DataAbort
            b UnhandledException
            b IRQHandler
            b FIQHandler

            # The vector table may be located in a non-canonical location (an
            # alias of the location in which our code is supposed to execute).
            # Perform an absolute jump to bring us back to a canonical location.
        UnhandledException:
            ldr pc, =$0
        UndefinedInstruction:
            ldr pc, =$1
        SupervisorCall:
            ldr pc, =$2
        PrefetchAbort:
            ldr pc, =$3
        DataAbort:
            ldr pc, =$4
        IRQHandler:
            ldr pc, =$5
        FIQHandler:
            ldr pc, =$6
        "
        :
        :   "X"(unhandled_exception_handler as extern "C" fn())
        ,   "X"(undefined_instruction_handler as extern "C" fn())
        ,   "X"(supervisor_call_handler as extern "C" fn())
        ,   "X"(prefetch_abort_handler as extern "C" fn())
        ,   "X"(data_abort_handler as extern "C" fn())
        ,   "X"(irq_handler as extern "C" fn())
        ,   "X"(fiq_handler as extern "C" fn())
        :
        :   "volatile");
    }
}

#[naked]
#[no_mangle]
fn reset_handler() {
    unsafe {
        // Set the stack pointer before calling Rust code
        llvm_asm!("
            ldr r0, =_stack_start

            # Set the stack for User/System mode
            mov sp, r0

            # Set the stack for IRQ mode
            msr cpsr_c, #0xd2
            mov sp, r0

            # Set the stack for FIQ mode
            msr cpsr_c, #0xd1
            mov sp, r0

            # Set the stack for Abort mode
            msr cpsr_c, #0xd7
            mov sp, r0

            # Set the stack for Undefined Instruction mode
            msr cpsr_c, #0xdb
            mov sp, r0

            # Set the stack for Supervisor mode
            msr cpsr_c, #0xd3
            mov sp, r0

            # Back to System mode (IRQ and FIQ both masked, Arm instruction set)
            msr cpsr_c, #0xdf

            b $0
        "
        :
        :   "X"(reset_handler1 as extern "C" fn())
        :
        :   "volatile");
    }
}

extern "C" fn reset_handler1() {
    extern "C" {
        // These symbols come from `link.x`
        static mut __sbss: u32;
        static mut __ebss: u32;

        static mut __sdata: u32;
        static mut __edata: u32;
        static __sidata: u32;
    }

    arm::SCTLR.modify(
        // Disable data and unified caches
        arm::SCTLR::C::Disable,
    );

    // Invalidate instruction cache
    arm::ICIALLU.set(0);

    // TODO: invalidate data and unified cache

    arm::SCTLR.modify(
        // Enable data and unified caches
        // TODO: arm::SCTLR::C::Enable +
        // Enable instruction caches
        arm::SCTLR::I::Enable +
        // Disable MMU
        arm::SCTLR::M::Disable +
        // Use the low vector table base address
        arm::SCTLR::V::Low +
        // Enable alignment fault checking
        arm::SCTLR::A::Enable +
        // Enable branch prediction
        arm::SCTLR::Z::Enable,
    );

    // Initialize RAM
    unsafe {
        r0::zero_bss(&mut __sbss, &mut __ebss);
        r0::init_data(&mut __sdata, &mut __edata, &__sidata);
    }

    // TODO: Remap `.vector_table` to `0x00000000`
    //       (this feature is RZ/A1H-specific)

    super::main();
}

extern "C" fn unhandled_exception_handler() {
    panic!("reserved exception");
}

extern "C" fn undefined_instruction_handler() {
    panic!("undefined instruction");
}

extern "C" fn supervisor_call_handler() {
    panic!("unexpected supervisor call");
}

extern "C" fn prefetch_abort_handler() {
    panic!("prefetch abort");
}

extern "C" fn data_abort_handler() {
    panic!("data abort");
}

extern "C" fn irq_handler() {
    panic!("unexpecte irq");
}

extern "C" fn fiq_handler() {
    panic!("unexpecte fiq");
}
