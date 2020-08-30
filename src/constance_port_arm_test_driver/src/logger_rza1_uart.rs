use core::fmt::Write;
use register::FieldValue;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        interrupt_free(|| {
            writeln!(
                SCWriter(rza1::SC2()),
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
struct SCWriter(&'static rza1::SC);

impl Write for SCWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            self.write_u8(b);
        }
        Ok(())
    }
}

impl SCWriter {
    fn write_u8(self, x: u8) {
        let sc = self.0;
        if x == b'\n' {
            self.write_u8(b'\r');
        }
        while sc.FSR.read(rza1::FSR::TDFE) == 0 {}
        sc.FTDR.set(x);
        sc.FSR
            .modify(rza1::FSR::TDFE::CLEAR + rza1::FSR::TEND::CLEAR);
    }
}

pub fn init() {
    // Supply clock to SC2
    rza1::STBCR4().set(rza1::STBCR4().get() & !0x20);

    // On GR-PEACH, the nets `TGT_[TR]XD` are connected to `P6_[23]`. Configure
    // `P6_[23]` to use its 7th alternative function - `SC2`.
    let (mask, shift) = (0b11, 2);
    rza1::PMC(6).modify(FieldValue::<u16, ()>::new(mask, shift, 0b11));
    rza1::PFCAE(6).modify(FieldValue::<u16, ()>::new(mask, shift, 0b11));
    rza1::PFCE(6).modify(FieldValue::<u16, ()>::new(mask, shift, 0b11));
    rza1::PFC(6).modify(FieldValue::<u16, ()>::new(mask, shift, 0b00));
    rza1::PM(6).modify(FieldValue::<u16, ()>::new(mask, shift, 0b01));

    let sc2 = rza1::SC2();
    sc2.SCR.write(
        rza1::SCR::TIE::CLEAR
            + rza1::SCR::RIE::CLEAR
            + rza1::SCR::TE::SET
            + rza1::SCR::RE::CLEAR
            + rza1::SCR::REIE::CLEAR
            + rza1::SCR::CKE::AsynchronousInternalNoOutput,
    );
    sc2.SMR.write(
        rza1::SMR::CA::Asynchronous
            + rza1::SMR::CHR::EightBitData
            + rza1::SMR::PE::NoParityBit
            + rza1::SMR::STOP::OneStopBit
            + rza1::SMR::CKS::DivideBy1,
    );
    // 66.666e6/115200/(64*2**(2*0-1))-1 = 17.0843...
    sc2.BRR.set(17);

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

#[allow(non_snake_case)]
mod rza1 {
    use register::mmio::{ReadOnly, ReadWrite, WriteOnly};

    #[inline]
    pub fn STBCR4() -> &'static ReadWrite<u8> {
        unsafe { &*(0xfcfe0424 as *const _) }
    }

    /// Port mode register (set = input)
    #[inline]
    pub fn PM(n: usize) -> &'static ReadWrite<u16> {
        assert!(n < 12);
        unsafe { &*((0xfcfe3300 + n * 4) as *const _) }
    }

    /// Port mode control register
    #[inline]
    pub fn PMC(n: usize) -> &'static ReadWrite<u16> {
        assert!(n < 12);
        unsafe { &*((0xfcfe3400 + n * 4) as *const _) }
    }

    /// Port function control register
    #[inline]
    pub fn PFC(n: usize) -> &'static ReadWrite<u16> {
        assert!(n < 12);
        unsafe { &*((0xfcfe3500 + n * 4) as *const _) }
    }

    /// Port function control expansion register
    #[inline]
    pub fn PFCE(n: usize) -> &'static ReadWrite<u16> {
        assert!(n < 12);
        unsafe { &*((0xfcfe3600 + n * 4) as *const _) }
    }

    /// Port function control additional expansion register
    #[inline]
    pub fn PFCAE(n: usize) -> &'static ReadWrite<u16> {
        assert!(n < 12);
        unsafe { &*((0xfcfe3a00 + n * 4) as *const _) }
    }

    /// Serial Communication Interface with FIFO
    #[repr(C)]
    pub struct SC {
        /// Serial mode register
        pub SMR: ReadWrite<u16, SMR::Register>,
        _r0: u16,
        /// Bit rate register
        pub BRR: ReadWrite<u8>,
        _r1: [u8; 3],
        /// Serial control register
        pub SCR: ReadWrite<u16, SCR::Register>,
        _r2: u16,
        /// Transmit FIFO data register
        pub FTDR: WriteOnly<u8>,
        _r3: [u8; 3],
        /// Serial status register
        pub FSR: ReadWrite<u16, FSR::Register>,
        _r4: u16,
        /// Receive FIFO data register
        pub FRDR: ReadOnly<u8>,
        _r5: [u8; 3],
        /// FIFO control register
        pub FCR: ReadWrite<u16>,
        _r6: u16,
        /// FIFO data count set register
        pub FDR: ReadOnly<u16>,
        _r7: u16,
        /// Serial port register
        pub SPTR: ReadWrite<u16>,
        _r8: u16,
        /// Line status register
        pub LSR: ReadWrite<u16>,
        _r9: u16,
        /// Serial extension mode register
        pub EMR: ReadWrite<u16>,
        _r10: u16,
    }

    pub fn SC2() -> &'static SC {
        unsafe { &*(0xE8008000 as *const SC) }
    }

    register::register_bitfields! {u16,
        pub SMR [
            CA OFFSET(7) NUMBITS(1) [
                Asynchronous = 0,
                ClockSynchronous = 1
            ],
            CHR OFFSET(6) NUMBITS(1) [
                EightBitData = 0,
                SevenBitData = 1
            ],
            PE OFFSET(5) NUMBITS(1) [
                NoParityBit = 0,
                HasParityBit = 1
            ],
            OOE OFFSET(4) NUMBITS(1) [
                EvenParity = 0,
                OddParity = 1
            ],
            STOP OFFSET(3) NUMBITS(1) [
                OneStopBit = 0,
                TwoStopBits = 1
            ],
            CKS OFFSET(0) NUMBITS(2) [
                DivideBy1 = 0b00,
                DivideBy4 = 0b01,
                DivideBy16 = 0b10,
                DivideBy64 = 0b11
            ]
        ]
    }

    register::register_bitfields! {u16,
        pub SCR [
            /// Transmit Interrupt Enable
            TIE OFFSET(7) NUMBITS(1) [],
            /// Receive Interrupt Enable
            RIE OFFSET(6) NUMBITS(1) [],
            /// Transmit Enable
            TE OFFSET(5) NUMBITS(1) [],
            /// Receive Enable
            RE OFFSET(4) NUMBITS(1) [],
            /// Receive Error Interrupt Enable
            REIE OFFSET(3) NUMBITS(1) [],
            /// Clock Enable
            CKE OFFSET(0) NUMBITS(2) [
                AsynchronousInternalNoOutput = 0b00
            ]
        ]
    }

    register::register_bitfields! {u16,
        pub FSR [
            /// Transmit FIFO Data Empty
            TDFE OFFSET(5) NUMBITS(1) [],
            /// Transmit End
            TEND OFFSET(6) NUMBITS(1) []
        ]
    }
}
