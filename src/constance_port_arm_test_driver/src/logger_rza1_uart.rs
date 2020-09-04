use core::fmt::Write;
use rza1::scif0 as scif;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let peripherals = unsafe { rza1::Peripherals::steal() };

        interrupt_free(|| {
            writeln!(
                SCWriter(&peripherals.SCIF2),
                "[{:5} {}] {}",
                record.level(),
                record.target(),
                record.args()
            )
            .unwrap();
        });
    }

    fn flush(&self) {}
}

#[derive(Clone, Copy)]
struct SCWriter<'a>(&'a scif::RegisterBlock);

impl Write for SCWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            self.write_u8(b);
        }
        Ok(())
    }
}

impl SCWriter<'_> {
    fn write_u8(self, x: u8) {
        let sc = self.0;
        if x == b'\n' {
            self.write_u8(b'\r');
        }
        while sc.fsr.read().tdfe().bit_is_clear() {}
        sc.ftdr.write(|w| w.d().bits(x));
        sc.fsr
            .modify(|_, w| w.tdfe().clear_bit().tend().clear_bit());
    }
}

pub fn init() {
    let peripherals = unsafe { rza1::Peripherals::steal() };
    let rza1::Peripherals {
        CPG, GPIO, SCIF2, ..
    } = peripherals;

    // Supply clock to SC2
    CPG.stbcr4.modify(|_, w| w.mstp45().clear_bit());

    // On GR-PEACH, the nets `TGT_[TR]XD` are connected to `P6_[23]`. Configure
    // `P6_[23]` to use its 7th alternative function - `SC2`.
    let (mask, shift) = (0b11, 2);

    GPIO.pmc6
        .modify(|_, w| w.pmc62().set_bit().pmc63().set_bit());
    GPIO.pfcae6
        .modify(|_, w| w.pfcae62().set_bit().pfcae63().set_bit());
    GPIO.pfce6
        .modify(|_, w| w.pfce62().set_bit().pfce63().set_bit());
    GPIO.pfc6
        .modify(|_, w| w.pfc62().clear_bit().pfc63().clear_bit());
    GPIO.pm6
        .modify(|_, w| w.pm62().set_bit().pm63().clear_bit());

    SCIF2.scr.write(|w| {
        w.tie()
            .clear_bit()
            .rie()
            .clear_bit()
            .te()
            .set_bit()
            .re()
            .clear_bit()
            .reie()
            .clear_bit()
            .cke()
            .internal_sck_in()
    });
    SCIF2.smr.write(|w| {
        w
            // Asynchronous
            .ca()
            .clear_bit()
            // 8-bit data
            .chr()
            .clear_bit()
            // No parity bits
            .pe()
            .clear_bit()
            // One stop bit
            .stop()
            .clear_bit()
            .cks()
            .divide_by_1()
    });
    // 66.666e6/115200/(64*2**(2*0-1))-1 = 17.0843...
    SCIF2.brr.write(|w| w.d().bits(17));

    log::set_logger(&Logger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
}

#[inline]
fn interrupt_free<T>(x: impl FnOnce() -> T) -> T {
    let cpsr: u32;
    unsafe { asm!("mrs {}, cpsr", out(reg)cpsr) };
    let unmask = (cpsr & (1 << 7)) == 0;

    unsafe { asm!("cpsid i") };

    let ret = x();

    if unmask {
        unsafe { asm!("cpsie i") };
    }

    ret
}
