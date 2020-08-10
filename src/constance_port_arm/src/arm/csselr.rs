register::register_bitfields! {u32,
    pub CSSELR [
        /// Instruction not Data bit.
        InD OFFSET(0) NUMBITS(1) [
            DataOrUnified = 0,
            Instruction = 1
        ],
        /// Cache level of required cache.
        Level OFFSET(1) NUMBITS(3) []
    ]
}

/// Cache Size Selection Register
pub const CSSELR: CSSELRAccessor = CSSELRAccessor;
pub struct CSSELRAccessor;

impl register::cpu::RegisterReadWrite<u32, CSSELR::Register> for CSSELRAccessor {
    sys_coproc_read_raw!(u32, [p15, c0, 2, c0, 0]);
    sys_coproc_write_raw!(u32, [p15, c0, 2, c0, 0]);
}
