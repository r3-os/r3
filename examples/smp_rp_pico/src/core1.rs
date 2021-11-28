use core::fmt;
use r3::kernel::{prelude::*, Task};
use r3_port_arm_m as port;

// --------------------------------------------------------------------------
// Target-specific configuration

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe struct SystemTraits);
port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {}

impl port::SysTickOptions for SystemTraits {
    // "The timer uses a one microsecond reference that is generated in the
    // Watchdog (see Section 4.7.2) which comes from clk_ref."
    const FREQUENCY: u64 = 1_000_000;
}

// --------------------------------------------------------------------------
// Startup

/// Reset core1 and boot the core1 kernel.
///
/// # Safety
///
///  - Must be called from core0.
///  - Must not be called more than once.
///
pub unsafe fn core1_launch(sio: &rp2040::sio::RegisterBlock, psm: &rp2040::psm::RegisterBlock) {
    use core::ptr::addr_of;
    extern "C" {
        static _core1_stack_start: u32;
    }

    // Reset core1
    psm.frce_off.modify(|_, w| w.proc1().set_bit());
    while psm.frce_off.read().proc1().bit_is_clear() {}
    psm.frce_off.modify(|_, w| w.proc1().clear_bit());

    // Based on the SDK's `multicore_launch_core1_raw` function
    let cmd_seq = [
        0,
        0,
        1,
        addr_of!(CORE1_VECTOR_TABLE) as usize,
        addr_of!(_core1_stack_start) as usize,
        core1_entry as usize,
    ];

    let mut it = cmd_seq.iter();
    while let Some(&cmd) = it.next() {
        let cmd = cmd as u32;

        // Drain FIFO before sending a zero
        if cmd == 0 {
            while sio.fifo_st.read().vld().bit_is_set() {
                sio.fifo_rd.read();
            }
            // core 1 may be waiting for fifo space
            cortex_m::asm::sev();
        }

        // Send the command
        while sio.fifo_st.read().rdy().bit_is_clear() {}
        sio.fifo_wr.write(|b| unsafe { b.bits(cmd) });
        cortex_m::asm::sev();

        // Get the response
        while sio.fifo_st.read().vld().bit_is_clear() {
            cortex_m::asm::wfe();
        }
        let response = sio.fifo_rd.read().bits();

        if response != cmd {
            // If the response is incorrect, start over
            it = cmd_seq.iter();
        }
    }
}

unsafe extern "C" fn core1_entry() -> ! {
    unsafe { <SystemTraits as port::EntryPoint>::start() }
}

#[repr(C, align(128))]
struct VectorTable<T>(T);

static CORE1_VECTOR_TABLE: VectorTable<[unsafe extern "C" fn(); 48]> = {
    extern "C" fn unhandled() {
        panic!("unhandled exception");
    }

    extern "C" {
        fn _core1_stack_start();
    }

    let mut table = [unhandled as _; 48];

    let mut i = 0;
    let kernel_handler_table = <SystemTraits as r3_kernel::KernelCfg2>::INTERRUPT_HANDLERS;
    while i < 48 {
        if let Some(handler) = kernel_handler_table.get(i) {
            table[i] = handler;
        }
        i += 1;
    }

    // Make sure to fill the first entry with the main stack pointer. It's not
    // used while launching (because the bootrom will use the one sent by FIFO),
    // but the default implementation of `ThreadingOptions::interrupt_stack_top`
    // will pick up this value from VTOR.
    table[0] = _core1_stack_start;
    table[14] = <SystemTraits as port::EntryPoint>::HANDLE_PEND_SV;

    VectorTable(table)
};

// --------------------------------------------------------------------------
// Sending messages to core0

#[derive(Clone, Copy)]
pub struct Core1(());

impl Core1 {
    #[inline]
    pub fn new(sio: &rp2040::sio::RegisterBlock) -> Option<Self> {
        if sio.cpuid.read().bits() == 1 {
            Some(Core1(()))
        } else {
            None
        }
    }
}

pub fn write_bytes(_core1: Core1, s: &[u8]) {
    let p = unsafe { rp2040::Peripherals::steal() };
    let sio = p.SIO;

    for chunk in s.chunks(4) {
        let word = [
            chunk[0],
            chunk.get(1).cloned().unwrap_or(0),
            chunk.get(2).cloned().unwrap_or(0),
            chunk.get(3).cloned().unwrap_or(0),
        ];

        // Send the command
        while sio.fifo_st.read().rdy().bit_is_clear() {}
        sio.fifo_wr
            .write(|b| unsafe { b.bits(u32::from_le_bytes(word)) });
    }
}

pub fn write_fmt(core1: Core1, args: fmt::Arguments<'_>) {
    struct WrapCore0Write(Core1);

    impl fmt::Write for WrapCore0Write {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            write_bytes(self.0, s.as_bytes());
            Ok(())
        }
    }

    let _ = fmt::Write::write_fmt(&mut WrapCore0Write(core1), args);
}

// --------------------------------------------------------------------------

#[derive(Debug)]
struct Objects {
    #[allow(dead_code)]
    task1: Task<System>,
}

const _COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    SystemTraits::configure_systick(b);

    let task1 = Task::build()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);

    Objects { task1 }
}

fn task1_body(_: usize) {
    let c1 = Core1(());
    write_bytes(c1, b"core1: task1 is running\n");

    let p = unsafe { rp2040::Peripherals::steal() };

    // Configure GP25 (connected to LED on Pico) for output
    // <https://github.com/jannic/rp-microcontroller-rs/blob/master/boards/rp-pico/examples/blink/main.rs>
    let pin = 25;

    p.SIO.gpio_oe_clr.write(|w| unsafe { w.bits(1 << pin) });
    p.SIO.gpio_out_clr.write(|w| unsafe { w.bits(1 << pin) });

    p.PADS_BANK0
        .gpio25
        .write(|w| w.ie().bit(true).od().bit(false));

    p.IO_BANK0.gpio25_ctrl.write(|w| w.funcsel().sio_25());

    p.SIO.gpio_oe_set.write(|w| unsafe { w.bits(1 << pin) });
    p.SIO.gpio_out_set.write(|w| unsafe { w.bits(1 << pin) });

    loop {
        write_fmt(
            c1,
            format_args!(
                "   1 |                  core1: {:?}\n",
                System::time().unwrap()
            ),
        );

        // Blink the LED
        p.SIO.gpio_out_set.write(|w| unsafe { w.bits(1 << pin) });
        System::sleep(r3::time::Duration::from_millis(100)).unwrap();
        p.SIO.gpio_out_clr.write(|w| unsafe { w.bits(1 << pin) });
        System::sleep(r3::time::Duration::from_millis(400)).unwrap();
    }
}
