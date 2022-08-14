#![expect(clippy::enum_variant_names)] // The common prefix for `RGN::Outer*` is intentional

tock_registers::register_bitfields! {u32,
    pub TTBR0 [
        /// Cacheable bit. Indicates whether the translation table walk is to
        /// Inner Cacheable memory.
        ///
        /// ARMv7-A without Multiprocessing Extensions
        C OFFSET(0) NUMBITS(1) [],

        /// Shareable bit. Indicates the Shareable attribute for the memory
        /// associated with the translation table walks.
        S OFFSET(1) NUMBITS(1) [],

        /// Region bits. Indicates the Outer cacheability attributes for the
        /// memory associated with the translation table walks.
        RGN OFFSET(3) NUMBITS(2) [
            OuterNonCacheable = 0b00,
            OuterWriteBackWriteAllocate = 0b01,
            OuterWriteThrough = 0b10,
            OuterWriteBackNoWriteAllocate = 0b11
        ],

        /// Not Outer Shareable bit. Indicates the Outer Shareable attribute for
        /// the memory associated with a translation table walk that has the
        /// Shareable attribute, indicated by TTBR0.S == 1.
        NOS OFFSET(5) NUMBITS(1) [
            OuterShareable = 0,
            InnerShareable = 1
        ],

        /// Translation table base 0 address.
        BASE OFFSET(14) NUMBITS(18) []
    ]
}

/// Translation Table Base Register 0
pub const TTBR0: TTBR0Accessor = TTBR0Accessor;
pub struct TTBR0Accessor;

impl tock_registers::interfaces::Readable for TTBR0Accessor {
    type T = u32;
    type R = TTBR0::Register;
    sys_coproc_read_raw!(u32, [p15, c2, 0, c0, 0]);
}

impl tock_registers::interfaces::Writeable for TTBR0Accessor {
    type T = u32;
    type R = TTBR0::Register;
    sys_coproc_write_raw!(u32, [p15, c2, 0, c0, 0]);
}
