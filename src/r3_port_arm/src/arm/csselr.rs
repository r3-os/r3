tock_registers::register_bitfields! {u32,
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

impl tock_registers::interfaces::Readable for CSSELRAccessor {
    type T = u32;
    type R = CSSELR::Register;
    sys_coproc_read_raw!(u32, [p15, c0, 2, c0, 0]);
}

impl tock_registers::interfaces::Writeable for CSSELRAccessor {
    type T = u32;
    type R = CSSELR::Register;
    sys_coproc_write_raw!(u32, [p15, c0, 2, c0, 0]);
}
