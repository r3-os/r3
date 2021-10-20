tock_registers::register_bitfields! {u32,
    pub CCSIDR [
        /// (Log2(Number of words in cache line)) -2.
        LineSize OFFSET(0) NUMBITS(3) [],
        /// (Associativity of cache) - 1
        Associativity OFFSET(3) NUMBITS(10) [],
        /// (Number of sets in cache) - 1
        NumSets OFFSET(13) NUMBITS(15) []
    ]
}

/// Cache Level ID Register
pub const CCSIDR: CCSIDRAccessor = CCSIDRAccessor;
pub struct CCSIDRAccessor;

impl tock_registers::interfaces::Readable for CCSIDRAccessor {
    type T = u32;
    type R = CCSIDR::Register;
    sys_coproc_read_raw!(u32, [p15, c0, 1, c0, 0]);
}
