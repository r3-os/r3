#![allow(non_snake_case)]
use register::mmio::{ReadOnly, ReadWrite, WriteOnly};

#[repr(C)]
pub struct Sp804 {
    pub Timer1Load: ReadWrite<u32>,
    pub Timer1Value: ReadOnly<u32>,
    pub Timer1Control: ReadWrite<u32, Control::Register>,
    pub Timer1IntClr: WriteOnly<u32>,
    pub Timer1RIS: ReadOnly<u32>,
    pub Timer1MIS: ReadOnly<u32>,
    pub Timer1BGLoad: ReadWrite<u32>,
    _reserved1: u32,
    pub Timer2Load: ReadWrite<u32>,
    pub Timer2Value: ReadOnly<u32>,
    pub Timer2Control: ReadWrite<u32, Control::Register>,
    pub Timer2IntClr: WriteOnly<u32>,
    pub Timer2RIS: ReadOnly<u32>,
    pub Timer2MIS: ReadOnly<u32>,
    pub Timer2BGLoad: ReadWrite<u32>,
    _reserved2: u32,
}

register::register_bitfields! {u32,
    pub Control [
        /// Selects one-shot or wrapping counter mode
        OneShot OFFSET(0) NUMBITS(1) [
            Wrapping = 0,
            OneShot = 1
        ],

        /// Selects 16/32 bit counter operation
        TimerSize OFFSET(1) NUMBITS(1) [
            SixteenBits = 0,
            ThirtyTwoBits = 1
        ],

        /// Prescale bits
        TimerPre OFFSET(2) NUMBITS(2) [
            DivideBy1 = 0b00,
            DivideBy16 = 0b01,
            DivideBy256 = 0b10
        ],

        /// Interrupt enable bit
        IntEnable OFFSET(5) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ],

        /// Mode bit
        TimerMode OFFSET(6) NUMBITS(1) [
            FreeRunning = 0,
            Periodic = 1
        ],

        /// Enable bit
        TimerEn OFFSET(7) NUMBITS(1) [
            Disable = 0,
            Enable = 1
        ]
    ]
}
