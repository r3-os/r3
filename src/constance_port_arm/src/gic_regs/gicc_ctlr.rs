register::register_bitfields! {u32,
    pub GICC_CTLR [
        /// Enable for the signaling of Group 1 interrupts by the CPU interface
        /// to the connected processor.
        Enable OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ]
    ]
}
