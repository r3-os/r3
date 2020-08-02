register::register_bitfields! {u32,
    pub TTBCR [
        /// Indicate the width of the base address held in TTBR0.
        N OFFSET(0) NUMBITS(3) [],

        /// Translation table walk disable for translations using TTBR0. This
        /// bit controls whether a translation table walk is performed on a TLB
        /// miss for an address that is translated using TTBR0. The meanings of
        /// the possible values of this bit are equivalent to those for the PD1
        /// bit.
        ///
        /// Requires the Security Extensions.
        PD0 OFFSET(4) NUMBITS(1) [
            Default = 0,
            Fault = 1
        ],

        /// Translation table walk disable for translations using TTBR1. This
        /// bit controls whether a translation table walk is performed on a TLB
        /// miss, for an address that is translated using TTBR1.
        ///
        /// Requires the Security Extensions.
        PD1 OFFSET(5) NUMBITS(1) [
            Default = 0,
            Fault = 1
        ],

        /// Extended Address Enable.
        ///
        /// Requires the Large Physical Address Extension.
        EAE OFFSET(31) NUMBITS(1) []
    ]
}

/// Translation Table Base Control Register
pub const TTBCR: TTBCRAccessor = TTBCRAccessor;
pub struct TTBCRAccessor;

impl register::cpu::RegisterReadWrite<u32, TTBCR::Register> for TTBCRAccessor {
    sys_coproc_read_raw!(u32, [p15, c2, 0, c0, 2]);
    sys_coproc_write_raw!(u32, [p15, c2, 0, c0, 2]);
}
