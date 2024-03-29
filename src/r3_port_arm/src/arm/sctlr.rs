tock_registers::register_bitfields! {u32,
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
        ],

        /// Access flag enable
        AFE OFFSET(29) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Thumb Exception Enable
        TE OFFSET(30) NUMBITS(1) [
            Arm = 0,
            Thumb = 1
        ]
    ]
}

pub const SCTLR: SCTLRAccessor = SCTLRAccessor;
pub struct SCTLRAccessor;

impl tock_registers::interfaces::Readable for SCTLRAccessor {
    type T = u32;
    type R = SCTLR::Register;
    sys_coproc_read_raw!(u32, [p15, c1, 0, c0, 0]);
}

impl tock_registers::interfaces::Writeable for SCTLRAccessor {
    type T = u32;
    type R = SCTLR::Register;
    sys_coproc_write_raw!(u32, [p15, c1, 0, c0, 0]);
}
