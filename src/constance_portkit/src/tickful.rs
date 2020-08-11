//! Implements the core algorithm for `systick_tickful`.
use num_rational::Ratio;

use crate::{
    num::{
        ceil_ratio128, floor_ratio128, reduce_ratio128,
        wrapping::{Wrapping, WrappingTrait},
    },
    utils::Init,
};

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
    pub const fn new(freq_num: u64, freq_denom: u64, tick_period_cycles: u64) -> TickfulCfg {
        if freq_denom == 0 {
            panic!("the denominator of the clock frequency must not be zero");
        } else if freq_num == 0 {
            panic!("the numerator of the clock frequency must not be zero");
        } else if tick_period_cycles == 0 || tick_period_cycles > 0x1000000 {
            // `tick_period_cycles` must be <= `0x1000000` because SysTick is
            // a 24-bit timer
            panic!("the tick period measured in cycles must be in range `0..=0x1000000`");
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
            panic!(
                "cannot satisfy the timing requirements; \
               the period of SysTick is too long and does not fit in 32 bits \
               when measured in microseconds"
            );
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
            panic!(
                "cannot satisfy the timing requirements; \
               the period of SysTick must not be longer than `TIME_HARD_HEADROOM`"
            );
        }

        TickfulCfg {
            tick_period_micros: tick_period_micros_floor as u32,
            tick_period_submicros: tick_period_submicros as u64,
            division: *tick_period_micros.denom() as u64,
        }
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
    fn tick(&mut self, cfg: &TickfulCfg);
    fn tick_count(&self) -> u32;
}

impl<Submicros: Init> Init for TickfulStateCore<Submicros> {
    const INIT: Self = Self {
        tick_count_micros: Init::INIT,
        tick_count_submicros: Init::INIT,
    };
}

impl<Submicros: WrappingTrait> TickfulStateTrait for TickfulStateCore<Submicros> {
    /// Advance the counter by one hardware tick.
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

    /// Get the tick count.
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
            TickfulCfg::new(1, 1, 1),
            TickfulCfg {
                tick_period_micros: 1_000_000,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // 1Hz clock, 1073-cycle period = 1073s
        assert_eq!(
            TickfulCfg::new(1, 1, 1073),
            TickfulCfg {
                tick_period_micros: 1073_000_000,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // 10MHz clock, 1-cycle period = (1/10)μs
        assert_eq!(
            TickfulCfg::new(10_000_000, 1, 1),
            TickfulCfg {
                tick_period_micros: 0,
                tick_period_submicros: 1,
                division: 10,
            },
        );

        // 125MHz clock, 125-cycle period = 1μs
        assert_eq!(
            TickfulCfg::new(125_000_000, 1, 125),
            TickfulCfg {
                tick_period_micros: 1,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // (125/3)MHz clock, 1250-cycle period = 30μs
        assert_eq!(
            TickfulCfg::new(125_000_000, 3, 1250),
            TickfulCfg {
                tick_period_micros: 30,
                tick_period_submicros: 0,
                division: 1,
            },
        );

        // 375MHz clock, 1250-cycle period = (10/3)μs
        assert_eq!(
            TickfulCfg::new(375_000_000, 1, 1250),
            TickfulCfg {
                tick_period_micros: 3,
                tick_period_submicros: 1,
                division: 3,
            },
        );
    }

    /// The clock frequency given to `TickfulCfg` must not be zero.
    #[should_panic]
    #[test]
    fn tickful_zero_freq() {
        TickfulCfg::new(0, 1, 1);
    }

    /// The denominator of the clock frequency given to `TickfulCfg` must not be
    /// zero.
    #[should_panic]
    #[test]
    fn tickful_zero_denom() {
        TickfulCfg::new(1, 0, 1);
    }

    /// `TickfulCfg` should reject a tick period that does not fit in the
    /// 32-bit tick count.
    #[should_panic]
    #[test]
    fn tickful_tick_too_long1() {
        TickfulCfg::new(1, 1, 5000); // 5000 [s] > 2³² [μs]
    }

    /// `TickfulCfg` should reject a tick period that is larger than
    /// [`constance::kernel::TIME_HARD_HEADROOM`].
    #[should_panic]
    #[test]
    fn tickful_tick_too_long2() {
        TickfulCfg::new(1, 1, 1074); // 1074 [s] > 2³⁰ [μs]
    }

    macro_rules! tickful_simulate {
        ($freq_num:expr, $freq_denom:expr, $tick_period_cycles:expr) => {{
            const CFG: TickfulCfg = TickfulCfg::new($freq_num, $freq_denom, $tick_period_cycles);
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
