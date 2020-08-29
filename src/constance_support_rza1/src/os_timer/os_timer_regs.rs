#![allow(non_snake_case)]
use register::mmio::{ReadOnly, ReadWrite, WriteOnly};

#[repr(C)]
pub struct OsTimer {
    /// OSTM compare register
    pub CMP: ReadWrite<u32>,
    /// OSTM counter register
    pub CNT: ReadOnly<u32>,
    _reserved1: u32,
    _reserved2: u32,
    /// OSTM count enable status register
    pub TE: ReadOnly<u8>,
    _reserved3: [u8; 3],
    /// OSTM count start trigger register
    pub TS: WriteOnly<u8>,
    _reserved4: [u8; 3],
    /// OSTM count stop trigger register
    pub TT: WriteOnly<u8>,
    _reserved5: [u8; 3],
    _reserved6: u32,
    /// OSTM control register
    pub CTL: ReadWrite<u8, CTL::Register>,
}

register::register_bitfields! {u8,
    pub CTL [
        /// Controls enabling/disabling of OSTMnTINT interrupt requests when
        /// counting starts.
        MD0 OFFSET(0) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Specifies the operating mode for the counter.
        MD1 OFFSET(1) NUMBITS(1) [
            IntervalTimer = 0,
            FreeRunningComparison = 1
        ]
    ]
}
