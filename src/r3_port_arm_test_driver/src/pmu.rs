//! Arm PMU
/// Implements `register::cpu::RegisterReadWrite::get`.
#[macropol::macropol]
macro_rules! sys_coproc_read_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        /// `mrc $&cp, $&opc1, {out_reg}, $&crn, $&crm, $&opc2`
        #[inline]
        fn get(&self) -> u32 {
            let reg;
            unsafe {
                core::arch::asm!(
                    "mrc $&cp, $&opc1, {}, $&crn, $&crm, $&opc2",
                    lateout(reg) reg,
                );
            }
            reg
        }
    };
}
/// Implements `register::cpu::RegisterReadWrite::set`.
#[macropol::macropol]
macro_rules! sys_coproc_write_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        /// `mcr $&cp, $&opc1, {in_reg}, $&crn, $&crm, $&opc2`
        #[inline]
        fn set(&self, value: u32) {
            unsafe {
                core::arch::asm!(
                    "mcr $&cp, $&opc1, {}, $&crn, $&crm, $&opc2",
                    in(reg) value,
                );
            }
        }
    };
}

tock_registers::register_bitfields! {u32,
    pub PMCNTENSET [
        /// PMCCNTR enable bit.
        C OFFSET(31) NUMBITS(1) []
    ]
}

/// Performance Monitors Count Enable Set register
pub const PMCNTENSET: PMCNTENSETAccessor = PMCNTENSETAccessor;
pub struct PMCNTENSETAccessor;

impl tock_registers::interfaces::Readable for PMCNTENSETAccessor {
    type T = u32;
    type R = PMCNTENSET::Register;
    sys_coproc_read_raw!(u32, [p15, c9, 0, c12, 1]);
}

impl tock_registers::interfaces::Writeable for PMCNTENSETAccessor {
    type T = u32;
    type R = PMCNTENSET::Register;
    sys_coproc_write_raw!(u32, [p15, c9, 0, c12, 1]);
}

tock_registers::register_bitfields! {u32,
    pub PMCR [
        /// Clock divider.
        D OFFSET(3) NUMBITS(1) [
            DivideBy1 = 0,
            DivideBy64 = 1
        ],
        /// Enable.
        E OFFSET(0) NUMBITS(1) []
    ]
}

///  Performance Monitors Control Register
pub const PMCR: PMCRAccessor = PMCRAccessor;
pub struct PMCRAccessor;

impl tock_registers::interfaces::Readable for PMCRAccessor {
    type T = u32;
    type R = PMCR::Register;
    sys_coproc_read_raw!(u32, [p15, c9, 0, c12, 0]);
}

impl tock_registers::interfaces::Writeable for PMCRAccessor {
    type T = u32;
    type R = PMCR::Register;
    sys_coproc_write_raw!(u32, [p15, c9, 0, c12, 0]);
}

/// Performance Monitors Cycle Count Register
pub const PMCCNTR: PMCCNTRAccessor = PMCCNTRAccessor;
pub struct PMCCNTRAccessor;

impl tock_registers::interfaces::Readable for PMCCNTRAccessor {
    type T = u32;
    type R = ();
    sys_coproc_read_raw!(u32, [p15, c9, 0, c13, 0]);
}

impl tock_registers::interfaces::Writeable for PMCCNTRAccessor {
    type T = u32;
    type R = ();
    sys_coproc_write_raw!(u32, [p15, c9, 0, c13, 0]);
}
