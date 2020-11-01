register::register_bitfields! {u32,
    pub DACR [
        /// Domain 0 access permission.
        D0 OFFSET(0) NUMBITS(2) [
            /// Any access to the domain generates a Domain fault.
            NoAccess = 0b00,
            /// Accesses are checked against the permission bits in the
            /// translation tables.
            Client = 0b01,
            /// Accesses are not checked against the permission bits in the
            /// translation tables.
            Manager = 0b11
        ]
    ]
}

/// Domain Access Control Register
pub const DACR: DACRAccessor = DACRAccessor;
pub struct DACRAccessor;

impl register::cpu::RegisterReadWrite<u32, DACR::Register> for DACRAccessor {
    sys_coproc_read_raw!(u32, [p15, c3, 0, c0, 0]);
    sys_coproc_write_raw!(u32, [p15, c3, 0, c0, 0]);
}
