use r3::{
    bind::bind,
    kernel::{InterruptLine, InterruptNum, StaticInterruptHandler, StaticTask},
    prelude::*,
    sync::StaticMutex,
};
use r3_port_arm_m as port;
use r3_support_rp2040 as support_rp2040;

// --------------------------------------------------------------------------
// Target-specific configuration

type System = r3_kernel::System<SystemTraits>;
port::use_port!(unsafe pub struct SystemTraits);
port::use_rt!(unsafe SystemTraits);
port::use_systick_tickful!(unsafe impl PortTimer for SystemTraits);

impl port::ThreadingOptions for SystemTraits {}

impl port::SysTickOptions for SystemTraits {
    // "The timer uses a one microsecond reference that is generated in the
    // Watchdog (see Section 4.7.2) which comes from clk_ref."
    const FREQUENCY: u64 = 1_000_000;
}

impl support_rp2040::usbstdio::Options for SystemTraits {
    fn handle_input(s: &[u8]) {
        if s == b"\r" || s == b"\n" {
            support_rp2040::sprint!("\n");
            return;
        }

        // echo the input with brackets
        if let Ok(s) = core::str::from_utf8(s) {
            support_rp2040::sprint!("[{}]", s);
        } else {
            support_rp2040::sprint!("[<not UTF-8>]");
        }
    }
}

// --------------------------------------------------------------------------

#[derive(Debug)]
struct Objects {
    #[allow(dead_code)]
    task1: StaticTask<System>,
    task2: StaticTask<System>,
    #[allow(dead_code)]
    mutex1: StaticMutex<System, u32>,
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app(b: &mut r3_kernel::Cfg<SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    let (rp2040_resets, rp2040_usbctrl_regs) = bind((), || {
        // Configure peripherals
        let p = unsafe { rp2040_pac::Peripherals::steal() };
        support_rp2040::clock::init_clock(
            &p.CLOCKS,
            &p.XOSC,
            &p.PLL_SYS,
            &p.PLL_USB,
            &p.RESETS,
            &p.WATCHDOG,
        );

        // Reset and enable IO bank 0
        p.RESETS
            .reset
            .modify(|_, w| w.pads_bank0().set_bit().io_bank0().set_bit());
        p.RESETS
            .reset
            .modify(|_, w| w.pads_bank0().clear_bit().io_bank0().clear_bit());
        while p.RESETS.reset_done.read().pads_bank0().bit_is_clear() {}
        while p.RESETS.reset_done.read().io_bank0().bit_is_clear() {}

        // Boot the core1 kernel
        // Safety: We are core0 and calling this function only once
        unsafe { crate::core1::core1_launch(&p.SIO, &p.PSM) };

        (p.RESETS, p.USBCTRL_REGS)
    })
    .unpure()
    .finish(b)
    .unzip();

    support_rp2040::usbstdio::configure::<_, SystemTraits>(b, rp2040_resets, rp2040_usbctrl_regs);

    SystemTraits::configure_systick(b);

    let task1 = StaticTask::define()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = StaticTask::define().start(task2_body).priority(3).finish(b);

    let mutex1 = StaticMutex::define().finish(b);

    // Listen for messages from core1
    let int_fifo = rp2040_pac::Interrupt::SIO_IRQ_PROC0 as InterruptNum + port::INTERRUPT_EXTERNAL0;
    InterruptLine::define()
        .line(int_fifo)
        // The priority should be lower than USB interrupts so that USB packets
        // can handled by the USB interrupt handler while we are doing
        // `write_bytes`
        .priority(0x40)
        .enabled(true)
        .finish(b);
    StaticInterruptHandler::define()
        .line(int_fifo)
        .start(|| {
            let p = unsafe { rp2040_pac::Peripherals::steal() };
            let sio = p.SIO;
            while sio.fifo_st.read().vld().bit_is_set() {
                let bytes = sio.fifo_rd.read().bits().to_le_bytes();
                let mut bytes = &bytes[..];

                // `bytes` may contain less than four valid bytes
                while let [head @ .., 0] = bytes {
                    bytes = head;
                }

                support_rp2040::stdout::write_bytes(bytes);
            }
        })
        .finish(b);

    Objects {
        task1,
        task2,
        mutex1,
    }
}

fn task1_body() {
    support_rp2040::sprintln!("COTTAGE = {:?}", COTTAGE);

    COTTAGE.task2.activate().unwrap();
}

fn task2_body() {
    loop {
        support_rp2040::sprintln!(" 0   | core0: {:?}", System::time().unwrap());
        System::sleep(r3::time::Duration::from_millis(700)).unwrap();
    }
}
