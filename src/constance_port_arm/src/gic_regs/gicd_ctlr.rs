register::register_bitfields! {u32,
    pub GICD_CTLR [
        /// Global enable for forwarding pending interrupts from the Distributor
        /// to the CPU interfaces
        Enable OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ]
    ]
}
