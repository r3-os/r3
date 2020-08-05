register::register_bitfields! {u32,
    pub GICD_TYPER [
        /// Indicates whether the GIC implements the Security Extensions.
        SecurityExtn OFFSET(10) NUMBITS(1) [
            Unimplemented = 0,
            Implemented = 1
        ],

        /// Indicates the number of implemented CPU interfaces. The number of
        /// implemented CPU interfaces is one more than the value of this field,
        /// for example if this field is 0b011, there are four CPU interfaces.
        /// If the GIC implements the Virtualization Extensions, this is also
        /// the number of virtual CPU interfaces.
        CPUNumber OFFSET(5) NUMBITS(3) [],

        /// Indicates the maximum number of interrupts that the GIC supports.
        /// If ITLinesNumber=N, the maximum number of interrupts is 32(N+1). The
        /// interrupt ID range is from 0 to (number of IDs – 1).
        ITLinesNumber OFFSET(0) NUMBITS(5) []
    ]
}
