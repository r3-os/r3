use constance::kernel::{InterruptNum, InterruptPriority};

/// The options for [`use_sp804!`].
pub trait Sp804Options {
    /// The base address of SP804's memory-mapped registers.
    const SP804_BASE: usize;

    /// The numerator of the effective timer clock rate of the dual timer.
    const FREQUENCY: u64;

    /// The denominator of the effective timer clock rate of the dual timer.
    /// Defaults to `1`.
    const FREQUENCY_DENOMINATOR: u64 = 1;

    /// The maximum permissible timer interrupt latency, measured in hardware
    /// timer cycles.
    ///
    /// Defaults to `min(FREQUENCY * 60 / FREQUENCY_DENOMINATOR, 0x40000000)`.
    const HEADROOM: u32 = min128(
        Self::FREQUENCY as u128 * 60 / Self::FREQUENCY_DENOMINATOR as u128,
        0x40000000,
    ) as u32;

    /// The interrupt priority of the timer interrupt line.
    /// Defaults to `0xc0`.
    const INTERRUPT_PRIORITY: InterruptPriority = 0xc0;

    /// The timer's interrupt number.
    const INTERRUPT_NUM: InterruptNum;
}

const fn min128(x: u128, y: u128) -> u128 {
    if x < y {
        x
    } else {
        y
    }
}
