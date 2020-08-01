/// Implements `register::cpu::RegisterReadWrite::get`.
macro_rules! sys_coproc_read_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
        fn get(&self) -> u32 {
            let reg;
            unsafe {
                llvm_asm!(
                    concat!(
                        "mrc ", stringify!($cp), ", ", stringify!($opc1), ", $0, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    )
                :   "=r"(reg)
                :
                :
                :   "volatile"
                );
            }
            reg
        }
    };
}
/// Implements `register::cpu::RegisterReadWrite::set`.
macro_rules! sys_coproc_write_raw {
    ($width:ty, [$cp:ident, $crn:ident, $opc1:literal, $crm:ident, $opc2:literal]) => {
    fn set(&self, value: u32) {
            unsafe {
                llvm_asm!(
                    concat!(
                        "mcr ", stringify!($cp), ", ", stringify!($opc1), ", $0, ",
                        stringify!($crn), ", ", stringify!($crm), ", ", stringify!($opc2)
                    )
                :
                :   "r"(value)
                :
                :   "volatile"
                );
            }
        }
    };
}

register::register_bitfields! {u32,
    pub SCTLR [
        /// Enables or disables MMU.
        M OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Enables or disables alignment fault checking.
        A OFFSET(1) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Enables or disables data and unified caches.
        C OFFSET(2) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Enables or disables branch prediction.
        Z OFFSET(11) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Enables or disables instruction caches.
        I OFFSET(12) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Chooses one of two predefined vector table base addresses.
        V OFFSET(13) NUMBITS(1) [
            Low = 0,
            High = 1
        ]
    ]
}

pub const SCTLR: SCTLRAccessor = SCTLRAccessor;
pub struct SCTLRAccessor;

impl register::cpu::RegisterReadWrite<u32, SCTLR::Register> for SCTLRAccessor {
    sys_coproc_read_raw!(u32, [p15, c1, 0, c0, 0]);
    sys_coproc_write_raw!(u32, [p15, c1, 0, c0, 0]);
}

/// Instruction cache invalidate all
pub const ICIALLU: ICIALLUAccessor = ICIALLUAccessor;
pub struct ICIALLUAccessor;

impl register::cpu::RegisterWriteOnly<u32, ()> for ICIALLUAccessor {
    sys_coproc_write_raw!(u32, [p15, c7, 0, c5, 0]);
}
