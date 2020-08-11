//! Implements the core algorithm for tickful timing.
use core::fmt;
use num_rational::Ratio;

use crate::{
    num::{
        ceil_ratio128, floor_ratio128, reduce_ratio128,
        wrapping::{Wrapping, WrappingTrait},
    },
    utils::Init,
};

/// The parameters of the tickful timing algorithm.
///
/// It can be passed to [`TickfulCfg::new`] to construct [`TickfulCfg`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TickfulOptions {
    /// The numerator of the hardware timer frequency.
    pub hw_freq_num: u64,
    /// The denominator of the hardware timer frequency.
    pub hw_freq_denom: u64,
    /// The tick period measured in hardware timer cycles.
    /// [`TickfulStateTrait::tick`] should be called in this period.
    pub hw_tick_period: u64,
}

/// Error type for [`TicklessCfg::new`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CfgError {
    /// The numerator of the clock frequency is zero.
    FreqNumZero,
    /// The denominator of the clock frequency is zero.
    FreqDenomZero,
    /// The tick period is zero.
    PeriodZero,
    /// The tick period does not fit in 32 bits when measured in microseconds.
    PeriodOverflowsU32,
    /// The tick period is longer than [`TIME_HARD_HEADROOM`].
    ///
    /// [`TIME_HARD_HEADROOM`]: constance::kernel::TIME_HARD_HEADROOM
    PeriodExceedsKernelHeadroom,
    /// The tick period does not fit in 24 bits.
    PeriodOverflowsSysTick, // TODO: remove this restriction
}

impl CfgError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FreqNumZero => "the numerator of the clock frequency must not be zero",
            Self::FreqDenomZero => "the denominator of the clock frequency must not be zero",
            Self::PeriodZero => "the tick period must not be zero",
            Self::PeriodOverflowsU32 => {
                "the tick period is too long and \
                does not fit in 32 bits when measured in microseconds"
            }
            Self::PeriodExceedsKernelHeadroom => {
                "the tick period must not be longer than `TIME_HARD_HEADROOM`"
            }
            Self::PeriodOverflowsSysTick => {
                "the tick period measured in cycles must be in range `0..=0x1000000`"
            }
        }
    }

    pub const fn panic(self) -> ! {
        core::panicking::panic(self.as_str());
    }
}

impl fmt::Display for CfgError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The precomputed parameters for the tickful implementation of
/// [`constance::kernel::PortTimer`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TickfulCfg {
    /// The integral part of the tick period.
    tick_period_micros: u32,
    /// The fractional part of the tick period. Divided by [`Self::division`].
    /// Must be in range `0..self.division`.
    tick_period_submicros: u64,
    /// The denominator of [`Self::tick_period_submicros`].
    division: u64,
}

impl TickfulCfg {
    /// Construct a `TickfulCfg`.
    pub const fn new(
        TickfulOptions {
            hw_freq_num: freq_num,
            hw_freq_denom: freq_denom,
            hw_tick_period: tick_period_cycles,
        }: TickfulOptions,
    ) -> Result<TickfulCfg, CfgError> {
        if freq_denom == 0 {
            return Err(CfgError::FreqDenomZero);
        } else if freq_num == 0 {
            return Err(CfgError::FreqNumZero);
        } else if tick_period_cycles == 0 {
            return Err(CfgError::PeriodZero);
        } else if tick_period_cycles > 0x1000000 {
            // `tick_period_cycles` must be <= `0x1000000` because SysTick is
            // a 24-bit timer
            return Err(CfgError::PeriodOverflowsSysTick);
        }

        // `tick_period = tick_period_cycles / (freq_num / freq_denom)`
        // `0 < tick_period_secs.numer() <= 0xff_ffff_ffff_ffff_ff00_0000`
        // `0 < tick_period_secs.denom() <=         0xffff_ffff_ffff_ffff`
        let tick_period_secs = Ratio::new_raw(
            freq_denom as u128 * tick_period_cycles as u128,
            freq_num as u128,
        );

        // `0 < tick_period_micros.numer() <= 0xf42_3fff_ffff_ffff_f0bd_c000_0000`
        // `0 < tick_period_micros.denom() <=               0xffff_ffff_ffff_ffff`
        let tick_period_micros = Ratio::new_raw(
            *tick_period_secs.numer() * 1_000_000,
            *tick_period_secs.denom(),
        );
        let tick_period_micros = reduce_ratio128(tick_period_micros);

        // Divide `tick_period_micros` into integral and fractional parts.
        // `0 <= tick_period_micros_floor <= 0xf42_3fff_ffff_ffff_f0bd_c000_0000`
        // `0 < tick_period_micros_ceil <= 0xf42_3fff_ffff_ffff_f0bd_c000_0000`
        // `0 <= tick_period_submicros <= 0xffff_ffff_ffff_fffe`
        let tick_period_micros_floor = floor_ratio128(tick_period_micros);
        let tick_period_micros_ceil = ceil_ratio128(tick_period_micros);
        let tick_period_submicros = *tick_period_micros.numer() % *tick_period_micros.denom();

        // On every tick, the tick count (`PortTimer::tick_count`) is incre-
        // mented by `tick_period_micros_floor` or `tick_period_micros_ceil`.
        // The tick count is only 32 bits wide, so the increment must fit in the
        // 32-bit range for the kernel to be able to figure out the correct
        // elapsed time.
        if tick_period_micros_ceil >= 0x1_0000_0000 {
            return Err(CfgError::PeriodOverflowsU32);
        }

        // Furthermore, there is some limitation on the timer interrupt latency
        // that the kernel can tolerate. In this tickful timing scheme, the tick
        // period equates to the maximum timer interrupt latency observed (i.e.,
        // as seen through `PortTimer::tick_count`) by the kernel¹. This means
        // the upper bound of the tick period is even narrower.
        //
        //  ¹ This assumes `tick_count` advances only when a SysTick handler is
        //    called. If we were to continuously update `tick_count`, we would
        //    have to take the *real* interrupt latency into account.
        //
        if tick_period_micros_ceil > constance::kernel::TIME_HARD_HEADROOM.as_micros() as u128 {
            return Err(CfgError::PeriodExceedsKernelHeadroom);
        }

        Ok(TickfulCfg {
            tick_period_micros: tick_period_micros_floor as u32,
            tick_period_submicros: tick_period_submicros as u64,
            division: *tick_period_micros.denom() as u64,
        })
    }

    pub const fn is_exact(&self) -> bool {
        self.division == 1
    }

    pub const fn division(&self) -> u64 {
        self.division
    }
}

/// Instantiates the optimal version of [`TickfulStateCore`] using a
/// given [`TickfulCfg`]. All instances implement [`TickfulStateTrait`].
pub type TickfulState<const CFG: TickfulCfg> = TickfulStateCore<Wrapping<{ CFG.division() - 1 }>>;

/// The internal state of the tickful implementation of
/// [`constance::kernel::PortTimer`].
#[derive(Debug, Copy, Clone)]
pub struct TickfulStateCore<Submicros> {
    tick_count_micros: u32,
    tick_count_submicros: Submicros,
}

pub trait TickfulStateTrait: Init {
    /// Advance the time by one tick period ([`TickfulOptions::hw_tick_period`]).
    fn tick(&mut self, cfg: &TickfulCfg);

    /// Get the OS tick count.
    fn tick_count(&self) -> u32;
}

impl<Submicros: Init> Init for TickfulStateCore<Submicros> {
    const INIT: Self = Self {
        tick_count_micros: Init::INIT,
        tick_count_submicros: Init::INIT,
    };
}

impl<Submicros: WrappingTrait> TickfulStateTrait for TickfulStateCore<Submicros> {
    #[inline]
    fn tick(&mut self, cfg: &TickfulCfg) {
        self.tick_count_micros = self.tick_count_micros.wrapping_add(cfg.tick_period_micros);
        if self
            .tick_count_submicros
            .wrapping_add_assign64(cfg.tick_period_submicros)
        {
            self.tick_count_micros = self.tick_count_micros.wrapping_add(1);
        }
    }

    #[inline]
    fn tick_count(&self) -> u32 {
        self.tick_count_micros
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    /// Compare the output of `TickfulCfg` to known values.
    #[test]
    fn tickful_known_values() {
        // 1Hz clock, 1-cycle period = 1s
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 1,
                hw_freq_denom: 1,
                hw_tick_period: 1
            })
            .unwrap(),
            TickfulCfg {
                tick_period_micros: 1_000_000,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // 1Hz clock, 1073-cycle period = 1073s
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 1,
                hw_freq_denom: 1,
                hw_tick_period: 1073
            })
            .unwrap(),
            TickfulCfg {
                tick_period_micros: 1073_000_000,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // 10MHz clock, 1-cycle period = (1/10)μs
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 10_000_000,
                hw_freq_denom: 1,
                hw_tick_period: 1
            })
            .unwrap(),
            TickfulCfg {
                tick_period_micros: 0,
                tick_period_submicros: 1,
                division: 10,
            },
        );

        // 125MHz clock, 125-cycle period = 1μs
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 125_000_000,
                hw_freq_denom: 1,
                hw_tick_period: 125
            })
            .unwrap(),
            TickfulCfg {
                tick_period_micros: 1,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // (125/3)MHz clock, 1250-cycle period = 30μs
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 125_000_000,
                hw_freq_denom: 3,
                hw_tick_period: 1250
            })
            .unwrap(),
            TickfulCfg {
                tick_period_micros: 30,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // 375MHz clock, 1250-cycle period = (10/3)μs
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 375_000_000,
                hw_freq_denom: 1,
                hw_tick_period: 1250
            })
            .unwrap(),
            TickfulCfg {
                tick_period_micros: 3,
                tick_period_submicros: 1,
                division: 3,
            },
        );
    }

    /// The clock frequency given to `TickfulCfg` must not be zero.
    #[test]
    fn tickful_zero_freq() {
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 0,
                hw_freq_denom: 1,
                hw_tick_period: 1
            }),
            Err(CfgError::FreqNumZero)
        );
    }

    /// The denominator of the clock frequency given to `TickfulCfg` must not be
    /// zero.
    #[test]
    fn tickful_zero_denom() {
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 1,
                hw_freq_denom: 0,
                hw_tick_period: 1
            }),
            Err(CfgError::FreqDenomZero)
        );
    }

    /// `TickfulCfg` should reject a tick period that does not fit in the
    /// 32-bit tick count.
    #[test]
    fn tickful_tick_too_long1() {
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 1,
                hw_freq_denom: 1,
                hw_tick_period: 5000
            }),
            Err(CfgError::PeriodOverflowsU32)
        ); // 5000 [s] > 2³² [μs]
    }

    /// `TickfulCfg` should reject a tick period that is larger than
    /// [`constance::kernel::TIME_HARD_HEADROOM`].
    #[test]
    fn tickful_tick_too_long2() {
        assert_eq!(
            TickfulCfg::new(TickfulOptions {
                hw_freq_num: 1,
                hw_freq_denom: 1,
                hw_tick_period: 1074
            }),
            Err(CfgError::PeriodExceedsKernelHeadroom)
        ); // 1074 [s] > 2³⁰ [μs]
    }

    macro_rules! tickful_simulate {
        ($freq_num:expr, $freq_denom:expr, $tick_period_cycles:expr) => {{
            const CFG: TickfulCfg = match TickfulCfg::new(TickfulOptions {
                hw_freq_num: $freq_num,
                hw_freq_denom: $freq_denom,
                hw_tick_period: $tick_period_cycles,
            }) {
                Ok(x) => x,
                Err(e) => e.panic(),
            };
            let period =
                Ratio::new($tick_period_cycles, 1u128) / Ratio::new($freq_num, $freq_denom);

            // Actual time, measured in seconds
            let mut time = Ratio::new_raw(0, 1u128);

            // The port
            let mut state: TickfulState<CFG> = Init::INIT;

            // Kernel state
            let mut kernel_time = 0;
            let mut last_tick_count = state.tick_count();

            // Run the simulation for 100 hardware ticks
            for _ in 0..10000 {
                // The kernel system time and the actual time must agree
                assert_eq!((time * 1_000_000).to_integer(), kernel_time);

                // Advance the time
                time += period;
                state.tick(&CFG);

                // Update the kernel system time
                let new_tick_count = state.tick_count();
                kernel_time += new_tick_count.wrapping_sub(last_tick_count) as u128;
                last_tick_count = new_tick_count;
            }
        }};
    }

    #[test]
    fn tickful_simulate1() {
        tickful_simulate!(1, 1, 1);
    }

    #[test]
    fn tickful_simulate2() {
        tickful_simulate!(125_000_000, 1, 125);
    }

    #[test]
    fn tickful_simulate3() {
        tickful_simulate!(375_000_000, 1, 1250);
    }

    #[test]
    fn tickful_simulate4() {
        tickful_simulate!(125_000_000, 3, 125);
    }

    #[test]
    fn tickful_simulate5() {
        tickful_simulate!(10_000_000, 1, 1);
    }

    #[test]
    fn tickful_simulate6() {
        tickful_simulate!(375, 1, 250_000);
    }

    #[test]
    fn tickful_simulate7() {
        tickful_simulate!(0x501e_e2c2_9a0f_7b77, 0xb79a_14f3_6985, 0x64ad);
    }

    #[test]
    fn tickful_simulate8() {
        tickful_simulate!(0xffff_ffff_ffff_ffff, 0xffff_ffff_fffe, 0x41c4);
    }
}
