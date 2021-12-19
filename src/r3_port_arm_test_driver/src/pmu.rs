//! Arm PMU
macro_rules! sys_coproc_read_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        #[inline]
        fn get(&self) -> u32 {
            let reg;
            unsafe {
                core::arch::asm!(
                    concat!(
                        "mrc ", stringify!($cp), ", ", stringify!($opc1), ", {}, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    ),
                    out(reg)reg,
                );
            }
            reg
        }
    };
}
/// Implements `register::cpu::RegisterReadWrite::set`.
macro_rules! sys_coproc_write_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        #[inline]
        fn set(&self, value: u32) {
            unsafe {
                core::arch::asm!(
                    concat!(
                        "mcr ", stringify!($cp), ", ", stringify!($opc1), ", {}, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    ),
                    in(reg)value,
                );
            }
        }
    };
}

register::register_bitfields! {u32,
    pub PMCNTENSET [
        /// PMCCNTR enable bit.
        C OFFSET(31) NUMBITS(1) []
    ]
}

/// Performance Monitors Count Enable Set register
pub const PMCNTENSET: PMCNTENSETAccessor = PMCNTENSETAccessor;
pub struct PMCNTENSETAccessor;

impl register::cpu::RegisterReadWrite<u32, PMCNTENSET::Register> for PMCNTENSETAccessor {
    sys_coproc_read_raw!(u32, [p15, c9, 0, c12, 1]);
    sys_coproc_write_raw!(u32, [p15, c9, 0, c12, 1]);
}

register::register_bitfields! {u32,
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

impl register::cpu::RegisterReadWrite<u32, PMCR::Register> for PMCRAccessor {
    sys_coproc_read_raw!(u32, [p15, c9, 0, c12, 0]);
    sys_coproc_write_raw!(u32, [p15, c9, 0, c12, 0]);
}

/// Performance Monitors Cycle Count Register
pub const PMCCNTR: PMCCNTRAccessor = PMCCNTRAccessor;
pub struct PMCCNTRAccessor;

impl register::cpu::RegisterReadWrite<u32, ()> for PMCCNTRAccessor {
    sys_coproc_read_raw!(u32, [p15, c9, 0, c13, 0]);
    sys_coproc_write_raw!(u32, [p15, c9, 0, c13, 0]);
}
