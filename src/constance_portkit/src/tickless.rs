//! Implements the core algorithm for tickless timing.
use num_rational::Ratio;

use crate::{
    num::{
        ceil_div128, floor_ratio128, gcd128, min128, reduce_ratio128,
        wrapping::{Wrapping, WrappingTrait},
    },
    utils::Init,
};

/// The precomputed parameters for the tickless implementation of
/// [`constance::kernel::PortTimer`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TicklessCfg {
    /// The integral part of the number of hardware ticks per microseconds.
    hw_ticks_per_micro: u32,
    /// The fractional part of the number of hardware ticks per microseconds,
    /// divided by [`Self::division`].
    hw_subticks_per_micro: u64,
    /// The algorithm to use.
    algorithm: TicklessAlgorithm,
    /// The denominator of [`Self::hw_subticks_per_micro`].
    division: u64,
    /// The maximum interval (measured in microseconds) that can be reliably
    /// measured.
    max_timeout: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum TicklessAlgorithm {
    /// See [`TicklessStatelessCore`].
    Stateless {
        /// Get the maximum hardware tick count (period minus one cycle).
        hw_max_tick_count: u32,
        /// Get the maximum OS tick count (period minus one cycle).
        max_tick_count: u32,
    },
    /// See [`TicklessStateCore`].
    Stateful,
}

impl TicklessCfg {
    /// Construct a `TicklessCfg`.
    #[allow(clippy::int_plus_one)] // for consistency
    pub const fn new(freq_num: u64, freq_denom: u64, hw_headroom_ticks: u32) -> Self {
        if freq_denom == 0 {
            panic!("the denominator of the clock frequency must not be zero");
        } else if freq_num == 0 {
            panic!("the numerator of the clock frequency must not be zero");
        }

        // `hw_ticks_per_micro = freq_num / freq_denom / 1_000_000`
        let hw_ticks_per_micro = Ratio::new_raw(freq_num as u128, freq_denom as u128 * 1_000_000);
        let hw_ticks_per_micro = reduce_ratio128(hw_ticks_per_micro);
        assert!(*hw_ticks_per_micro.numer() >= 1);
        assert!(*hw_ticks_per_micro.numer() <= 0xffff_ffff_ffff_ffff);
        assert!(*hw_ticks_per_micro.denom() >= 1);
        assert!(*hw_ticks_per_micro.denom() <= 0xf_423f_ffff_ffff_fff0_bdc0);

        // Split `hw_ticks_per_micro` into integral and fractional parts.
        let hw_ticks_per_micro_floor = floor_ratio128(hw_ticks_per_micro);
        let hw_subticks_per_micro = *hw_ticks_per_micro.numer() % *hw_ticks_per_micro.denom();
        assert!(hw_ticks_per_micro_floor <= 0xffff_ffff_ffff_ffff);
        assert!(hw_subticks_per_micro <= *hw_ticks_per_micro.denom() - 1);

        if hw_ticks_per_micro_floor > u32::MAX as u128 {
            panic!(
                "cannot satisfy the timing requirements; \
               the timer frequency is too fast"
            );
        }

        if *hw_ticks_per_micro.denom() > u64::MAX as u128 {
            panic!(
                "cannot satisfy the timing requirements; \
               intermediate calculation overflowed. the clock frequency might \
               be too complex or too low"
            );
        }

        // Try the stateless algorithm first. Find the period at which HW ticks
        // and OS ticks align. The result is `hw_global_period` HW ticks ==
        // `global_period` microseconds.
        let (hw_global_period, global_period) = if hw_subticks_per_micro == 0 {
            assert!(*hw_ticks_per_micro.denom() == 1);
            assert!(hw_ticks_per_micro_floor != 0);
            (hw_ticks_per_micro_floor, 1)
        } else {
            // (1..).map(|i| (i, hw_subticks_per_micro * i))
            //      .filter(|(i, subticks)| subticks % hw_ticks_per_micro.denom() == 0)
            //      .nth(0)
            //      .0
            //  = lcm(hw_subticks_per_micro, hw_ticks_per_micro.denom())
            //     / hw_subticks_per_micro
            //  = hw_ticks_per_micro.denom()
            //     / gcd(hw_subticks_per_micro, hw_ticks_per_micro.denom())
            let global_period = *hw_ticks_per_micro.denom()
                / gcd128(hw_subticks_per_micro, *hw_ticks_per_micro.denom());

            // global_period * hw_ticks_per_micro
            //  = hw_ticks_per_micro.numer()
            //     / gcd(hw_subticks_per_micro, hw_ticks_per_micro.denom())
            let hw_global_period = *hw_ticks_per_micro.numer()
                / gcd128(hw_subticks_per_micro, *hw_ticks_per_micro.denom());

            (hw_global_period, global_period)
        };
        assert!(hw_global_period >= 1);
        assert!(hw_global_period <= *hw_ticks_per_micro.numer());
        assert!(global_period >= 1);
        assert!(global_period <= *hw_ticks_per_micro.denom());

        let (algorithm, max_timeout) = if hw_global_period <= 0x1_0000_0000
            && global_period <= 0x1_0000_0000
            // Prevent `[hw_]max_tick_count` from being zero
            && (hw_global_period <= 0x8000_0000 || global_period > 1)
            && (global_period <= 0x8000_0000 || hw_global_period > 1)
        {
            // If the period is measurable without wrap-around in both ticks,
            // the stateless algorithm is applicable.
            let repeat = min128(
                0x1_0000_0000 / hw_global_period,
                0x1_0000_0000 / global_period,
            );
            let hw_max_tick_count = hw_global_period * repeat - 1;
            let max_tick_count = global_period * repeat - 1;

            // Find the maximum value of `max_timeout` such that:
            //
            //  // For every possible reference point...
            //  ∀ref_hw_tick_count ∈ 0..=hw_max_tick_count:
            //    let ref_tick_count = floor(ref_hw_tick_count / hw_ticks_per_micro);
            //
            //    // Timeout is set to maximum
            //    let next_tick_count = ref_tick_count + max_timeout;
            //    let next_hw_tick_count = ceil(next_tick_count * hw_ticks_per_micro);
            //
            //    // Take an interrupt latency into account
            //    let late_hw_tick_count = next_hw_tick_count + hw_headroom_ticks;
            //
            //    // Convert it back to OS tick count
            //    let late_tick_count = floor(late_hw_tick_count / hw_ticks_per_micro);
            //
            //    // The tick count of the next tick shouldn't completely
            //    // "revolve" around
            //    late_tick_count <= ref_tick_count + max_tick_count
            //
            let max_timeout = (
                // `late_tick_count <= ref_tick_count + max_tick_count`
                (max_tick_count as u128 * *hw_ticks_per_micro.numer() + *hw_ticks_per_micro.numer()
                    - 1)
                .saturating_sub(
                    *hw_ticks_per_micro.denom() - 1
                        + hw_headroom_ticks as u128 * *hw_ticks_per_micro.denom(),
                )
            ) / *hw_ticks_per_micro.numer();

            if max_timeout == 0 {
                panic!("The calculated `MAX_TIMEOUT` is too low - lower the headroom");
            }
            assert!(max_timeout <= u32::MAX as u128);

            (
                TicklessAlgorithm::Stateless {
                    hw_max_tick_count: hw_max_tick_count as u32,
                    max_tick_count: max_tick_count as u32,
                },
                max_timeout as u32,
            )
        } else {
            let hw_max_tick_count = u32::MAX;
            let max_tick_count = u32::MAX;
            // Find the maximum value of `max_timeout` such that:
            //
            //  // For every possible reference point...
            //  ∀ref_tick_count ∈ 0..=max_tick_count
            //  ∀ref_hw_tick_count ∈ 0..=hw_max_tick_count
            //  ∀ref_hw_subtick_count ∈ 0..hw_ticks_per_micro.denom():
            //    // Timeout is set to maximum
            //    let next_hw_tick_count = ceil(
            //      ref_hw_tick_count + ref_hw_subtick_count / hw_ticks_per_micro.denom() +
            //        max_timeout * hw_ticks_per_micro
            //    );
            //
            //    // Take an interrupt latency into account
            //    let late_hw_tick_count = next_hw_tick_count + hw_headroom_ticks;
            //
            //    // Convert it back to OS tick count
            //    let elapsed_hw_ticks = late_hw_tick_count -
            //       (ref_hw_tick_count + ref_hw_subtick_count / hw_ticks_per_micro.denom());
            //    let elapsed_ticks = elapsed_hw_ticks / hw_ticks_per_micro;
            //    let late_tick_count = ref_tick_count + floor(elapsed_ticks);
            //
            //    (
            //      // The hardware tick count of the next tick shouldn't completely
            //      // "revolve" around
            //      late_hw_tick_count <= ref_hw_tick_count + hw_max_tick_count &&
            //
            //      // The OS tick count of the next tick shouldn't completely
            //      // "revolve" around
            //      late_tick_count <= ref_tick_count + max_tick_count
            //    )
            //
            let max_timeout = min128(
                // `late_hw_tick_count <= ref_hw_tick_count + hw_max_tick_count`
                ((hw_max_tick_count - hw_headroom_ticks) as u128 * *hw_ticks_per_micro.denom())
                    .saturating_sub(*hw_ticks_per_micro.denom() - 1),
                // `late_tick_count <= ref_tick_count + max_tick_count`
                (max_tick_count as u128 * *hw_ticks_per_micro.numer()
                    + *hw_ticks_per_micro.numer()
                    - 1)
                .saturating_sub(
                    *hw_ticks_per_micro.denom() - 1
                        + hw_headroom_ticks as u128 * *hw_ticks_per_micro.denom(),
                ),
            ) / *hw_ticks_per_micro.numer();

            if max_timeout == 0 {
                panic!("The calculated `MAX_TIMEOUT` is too low - lower the headroom");
            }
            assert!(max_timeout <= u32::MAX as u128);

            (TicklessAlgorithm::Stateful, max_timeout as u32)
        };

        Self {
            hw_ticks_per_micro: hw_ticks_per_micro_floor as u32,
            hw_subticks_per_micro: hw_subticks_per_micro as u64,
            algorithm,
            division: *hw_ticks_per_micro.denom() as u64,
            max_timeout,
        }
    }

    /// Get the maximum hardware tick count (period minus one cycle).
    #[inline]
    pub const fn hw_max_tick_count(&self) -> u32 {
        match self.algorithm {
            TicklessAlgorithm::Stateless {
                hw_max_tick_count, ..
            } => hw_max_tick_count,
            TicklessAlgorithm::Stateful => u32::MAX,
        }
    }

    /// Get the maximum OS tick count (period minus one cycle).
    pub const fn max_tick_count(&self) -> u32 {
        match self.algorithm {
            TicklessAlgorithm::Stateless { max_tick_count, .. } => max_tick_count,
            TicklessAlgorithm::Stateful => u32::MAX,
        }
    }

    /// Get the maximum time interval that can be reliably measured, taking an
    /// interrupt latency into account.
    pub const fn max_timeout(&self) -> u32 {
        self.max_timeout
    }

    /// Get the subtick division.
    pub const fn division(&self) -> u64 {
        self.division
    }
}

/// Instantiates the optimal version of [`TicklessStateCore`] using a
/// given [`TicklessCfg`]. All instances implement [`TicklessStateTrait`].
pub type TicklessState<const CFG: TicklessCfg> = If! {
    if (matches!(CFG.algorithm, TicklessAlgorithm::Stateful)) {
        TicklessStateCore<Wrapping<{ CFG.division() - 1 }>>
    } else {
        TicklessStatelessCore
    }
};

/// The stateless and tickless implementation of
/// [`constance::kernel::PortTimer`].
///
/// The stateless algorithm is chosen if the hardware ticks and OS ticks “line
/// up” periodically with a period shorter than the representable ranges of both
/// tick counts.
///
/// ```text
///  HW ticks    ┌──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┐
///  ³/₇μs/tick  0                    7                    14
///              ╎                    ╎           (hw_max_tick_count + 1)
///              ╎                    ╎                    ╎
///  OS ticks    ┌──────┬──────┬──────┬──────┬──────┬──────┐
///  1μs/tick    0                                         6
///                                                (max_tick_count + 1)
/// ```
#[derive(Debug, Copy, Clone)]
pub struct TicklessStatelessCore;

/// The internal state of the tickless implementation of
/// [`constance::kernel::PortTimer`].
#[derive(Debug, Copy, Clone)]
pub struct TicklessStateCore<Subticks> {
    /// The OS tick count at the reference point.
    ref_tick_count: u32,
    /// The hardware tick count at the reference point.
    ref_hw_tick_count: u32,
    /// The fractional part of the hardware tick count at the reference point.
    /// Must be in range `0..cfg.division` for a given `cfg: `[`TicklessCfg`].
    ref_hw_subtick_count: Subticks,
}

pub trait TicklessStateTrait: Init + Copy + core::fmt::Debug {
    /// Mark a reference point. Returns the reference point's OS tick count.
    ///
    /// All reference points are exactly aligned to OS ticks (microseconds).
    ///
    /// The client should call this method periodically for a correct behavior.
    /// The client should use the [`Self::tick_count_to_hw_tick_count`] method
    /// to determine the next hardware tick count to mark the next reference
    /// point on.
    ///
    /// `cfg` must be the instance of [`TicklessCfg`] that was passed to
    /// [`TicklessState`] to derive `Self`.
    fn mark_reference(&mut self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32;

    /// Calculate the earliest hardware tick count representing a point of time
    /// that coincides or follows the one represented by the specified OS tick
    /// count.
    ///
    /// `tick_count` must satisfy the following condition: Given a last
    /// reference point `ref_tick_count` (a value returned by
    /// [`mark_reference`]), there must exist `i` such that
    /// `i ∈ 1..=cfg.max_timeout()` and `tick_count == (ref_tick_count + i) %
    /// (cfg.max_tick_count() + 1)`.
    ///
    /// In particular, `tick_count` must not be identical to `ref_tick_count`.
    /// If this was allowed, the result could refer to the past. Consider the
    /// following diagram. In this case, `mark_reference` is called at the 6th
    /// hardware tick, creating a reference point at time 2μs. Now if you call
    /// `tick_count_to_hw_tick_count` with `tick_count = 2`, the returned value
    /// will refer to the 5th hardware tick, which is in the past. Because of
    /// wrap-around arithmetics, it's impossible to tell if the returned value
    /// refers to the past or not.
    ///
    /// ```text
    ///                         timer interrupt,
    ///                       calls mark_reference
    ///                                ↓
    ///  HW ticks    ┌──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┐
    ///  ³/₇μs/tick  0                    7                    14
    ///                                ╎
    ///                                ╎
    ///  OS ticks    ┌──────┬──────┬──────┬──────┬──────┬──────┐
    ///  1μs/tick    0             ↑                           6
    ///                      ref_tick_count
    /// ```
    ///
    /// `cfg` must be the instance of [`TicklessCfg`] that was passed to
    /// [`TicklessState`] to derive `Self`.
    ///
    /// [`mark_reference`]: Self::mark_reference
    fn tick_count_to_hw_tick_count(&self, cfg: &TicklessCfg, tick_count: u32) -> u32;

    /// Get the OS tick count.
    ///
    /// `cfg` must be the instance of [`TicklessCfg`] that was passed to
    /// [`TicklessState`] to derive `Self`.
    ///
    /// `hw_tick_count` must satisfy the following condition: Given a last
    /// reference point `ref_hw_tick_count` (a value passed to
    /// [`mark_reference`]), there must exist `timeout` and `latency` such that
    /// `timeout ∈ 0..=cfg.max_timeout()`, `latency ∈ 0..= hw_headroom_ticks`,
    /// and `hw_tick_count == (ref_hw_tick_count + i) % (cfg.hw_max_tick_count()
    /// + 1)`.
    ///
    /// [`mark_reference`]: Self::mark_reference
    fn tick_count(&self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32;
}

impl Init for TicklessStatelessCore {
    const INIT: Self = Self;
}

impl<Subticks: Init> Init for TicklessStateCore<Subticks> {
    const INIT: Self = Self {
        ref_tick_count: Init::INIT,
        ref_hw_tick_count: Init::INIT,
        ref_hw_subtick_count: Init::INIT,
    };
}

impl TicklessStateTrait for TicklessStatelessCore {
    #[inline]
    fn mark_reference(&mut self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32 {
        self.tick_count(cfg, hw_tick_count)
    }

    #[inline]
    fn tick_count_to_hw_tick_count(&self, cfg: &TicklessCfg, tick_count: u32) -> u32 {
        // ceil(tick_count * (hw_ticks_per_micro + hw_subticks_per_micro / division))
        //  = tick_count * hw_ticks_per_micro + ceil(tick_count * hw_subticks_per_micro / division)
        let mut hw_tick_count = (tick_count * cfg.hw_ticks_per_micro).wrapping_add(ceil_div128(
            tick_count as u128 * cfg.hw_subticks_per_micro as u128,
            cfg.division as u128,
        )
            as u32);

        // Wrap around
        let hw_max_tick_count = cfg.hw_max_tick_count();
        if hw_max_tick_count != u32::MAX && hw_tick_count == hw_max_tick_count + 1 {
            hw_tick_count = 0;
        }

        debug_assert!(hw_tick_count <= hw_max_tick_count);
        hw_tick_count
    }

    #[inline]
    fn tick_count(&self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32 {
        // floor(hw_tick_count /
        //       (hw_ticks_per_micro + hw_subticks_per_micro / division))
        //  = floor((hw_tick_count * division) /
        //          (hw_ticks_per_micro * division + hw_subticks_per_micro))
        let tick_count: u128 = (hw_tick_count as u128 * cfg.division as u128)
            / (cfg.hw_ticks_per_micro as u128 * cfg.division as u128
                + cfg.hw_subticks_per_micro as u128);

        debug_assert!(tick_count <= u32::MAX as u128);

        tick_count as u32
    }
}

impl<Subticks: WrappingTrait> TicklessStateTrait for TicklessStateCore<Subticks> {
    #[inline]
    fn mark_reference(&mut self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32 {
        // Calculate the tick count
        let new_ref_tick_count = self.tick_count(cfg, hw_tick_count);

        let advance_micros = new_ref_tick_count.wrapping_sub(self.ref_tick_count);
        self.ref_tick_count = new_ref_tick_count;

        self.ref_hw_tick_count = self
            .ref_hw_tick_count
            .wrapping_add(advance_micros.wrapping_mul(cfg.hw_ticks_per_micro));

        let overflow = self.ref_hw_subtick_count.wrapping_add_assign128_multi32(
            cfg.hw_subticks_per_micro as u128 * advance_micros as u128,
        );
        self.ref_hw_tick_count = self.ref_hw_tick_count.wrapping_add(overflow);

        new_ref_tick_count
    }

    #[inline]
    fn tick_count_to_hw_tick_count(&self, cfg: &TicklessCfg, tick_count: u32) -> u32 {
        debug_assert_ne!(tick_count, self.ref_tick_count);

        let micros = tick_count.wrapping_sub(self.ref_tick_count);
        // ceil(ref_hw_tick_count + ref_hw_subtick_count / division +
        //      micros * (hw_ticks_per_micro + hw_subticks_per_micro / division))
        //  = ceil(ref_hw_subtick_count / division +
        //        micros * (hw_ticks_per_micro + hw_subticks_per_micro / division))
        //     + ref_hw_tick_count
        //  = ceil((
        //       ref_hw_subtick_count +
        //       micros * (hw_ticks_per_micro * division + hw_subticks_per_micro)
        //    ) / division) + ref_hw_tick_count
        let division = cfg.division as u128;
        let ref_hw_tick_count = self.ref_hw_tick_count;
        let ref_hw_subtick_count = self.ref_hw_subtick_count.to_u128();
        let hw_ticks_per_micro = cfg.hw_ticks_per_micro as u128;
        let hw_subticks_per_micro = cfg.hw_subticks_per_micro as u128;
        ref_hw_tick_count.wrapping_add(ceil_div128(
            ref_hw_subtick_count
                + micros as u128 * (hw_ticks_per_micro * division + hw_subticks_per_micro),
            division,
        ) as u32)
    }

    #[inline]
    fn tick_count(&self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32 {
        // (hw_tick_count - (ref_hw_tick_count + ref_hw_subtick_count / division))
        //      / (hw_ticks_per_micro + hw_subticks_per_micro / division) + ref_tick_count
        //  = ((hw_tick_count - ref_hw_tick_count) * division - ref_hw_subtick_count)
        //         / (hw_ticks_per_micro * division + hw_subticks_per_micro) + ref_tick_count
        let division = cfg.division as u128;
        let ref_hw_tick_count = self.ref_hw_tick_count;
        let ref_hw_subtick_count = self.ref_hw_subtick_count.to_u128();
        let hw_ticks_per_micro = cfg.hw_ticks_per_micro as u128;
        let hw_subticks_per_micro = cfg.hw_subticks_per_micro as u128;
        self.ref_tick_count.wrapping_add(
            ((hw_tick_count.wrapping_sub(ref_hw_tick_count) as u128 * division
                - ref_hw_subtick_count)
                / (hw_ticks_per_micro * division + hw_subticks_per_micro)) as u32,
        )
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use itertools::merge;
    use std::{prelude::v1::*, vec};

    /// Compare the output of `TicklessCfg` to known values.
    #[test]
    fn tickless_known_values() {
        // 1Hz clock, 1-cycle period = 1s, 1-cycle latency tolerance
        assert_eq!(
            TicklessCfg::new(1, 1, 1),
            TicklessCfg {
                hw_ticks_per_micro: 0,
                hw_subticks_per_micro: 1,
                algorithm: TicklessAlgorithm::Stateless {
                    hw_max_tick_count: 4_293,
                    max_tick_count: 4_293_999_999,
                },
                division: 1_000_000,
                max_timeout: 4_292_000_000,
            },
        );
    }

    /// The clock frequency given to `TicklessCfg` must not be zero.
    #[should_panic(expected = "the numerator of the clock frequency must not be zero")]
    #[test]
    fn tickless_zero_freq() {
        TicklessCfg::new(0, 1, 1);
    }

    /// The denominator of the clock frequency given to `TicklessCfg` must not be
    /// zero.
    #[should_panic(expected = "the denominator of the clock frequency must not be zero")]
    #[test]
    fn tickless_zero_denom() {
        TicklessCfg::new(1, 0, 1);
    }

    /// `TicklessCfg` should reject a timer frequency that is too fast.
    #[should_panic(expected = "the timer frequency is too fast")]
    #[test]
    fn tickless_tick_too_fast() {
        // 2³²MHz → 2³² HW ticks/μs
        TicklessCfg::new(1_000_000 * 0x1_0000_0000, 1, 0);
    }

    /// `TicklessCfg` should reject if an intermediate value overflows.
    #[should_panic(expected = "intermediate calculation overflowed")]
    #[test]
    fn tickless_tick_too_complex() {
        // 1.00000000000000000043368086899420177... Hz
        // (0x1fffffffffffffff is a Mersenne prime number.)
        TicklessCfg::new(0x1fffffffffffffff, 0x1ffffffffffffffe, 0);
    }

    #[derive(Debug, Copy, Clone)]
    struct Op {
        timeout: u32,
        latency: u32,
    }

    /// Choose some values from `x`. The returned values are sorted in an
    /// ascending order and always include the endpoints.
    fn choose_values_from_range(x: std::ops::RangeInclusive<u32>) -> Box<dyn Iterator<Item = u32>> {
        if x.end() - x.start() < 10 {
            // Return all values
            Box::new(x)
        } else {
            Box::new((0..=10).map(move |i| {
                if i < 2 {
                    x.start() + i
                } else if i < 8 {
                    x.start() + 2 + (x.end() - x.start() - 4) / 6 * (i - 2)
                } else {
                    x.end() - (10 - i)
                }
            }))
        }
    }

    #[track_caller]
    fn add_mod(x: u32, y: u32, modulus: u64) -> u32 {
        assert!((x as u64) < modulus);
        assert!((y as u64) < modulus);
        ((x as u64 + y as u64) % modulus) as u32
    }

    #[track_caller]
    fn sub_mod(x: u32, y: u32, modulus: u64) -> u32 {
        assert!((x as u64) < modulus);
        assert!((y as u64) < modulus);
        if x < y {
            (x as u64 + modulus - y as u64) as u32
        } else {
            x - y
        }
    }

    macro tickless_simulate(
        mod $ident:ident {}, $freq_num:expr, $freq_denom:expr, $hw_headroom_ticks:expr
    ) {
        mod $ident {
            use super::*;

            const CFG: TicklessCfg = TicklessCfg::new($freq_num, $freq_denom, $hw_headroom_ticks);
            const MAX_TIMEOUT: u32 = CFG.max_timeout();
            const HW_PERIOD: u64 = CFG.hw_max_tick_count() as u64 + 1;
            const PERIOD: u64 = CFG.max_tick_count() as u64 + 1;

            fn do_test(ops: impl IntoIterator<Item = Op>) {
                let mut state: TicklessState<CFG> = Init::INIT;
                let mut hw_tick_count: u32 = 0;

                let _ = env_logger::builder().is_test(true).try_init();

                log::info!("CFG = {:?}", CFG);
                log::info!("MAX_TIMEOUT = {:?}", MAX_TIMEOUT);
                log::info!("HW_PERIOD = {:?}", HW_PERIOD);
                log::info!("PERIOD = {:?}", PERIOD);

                for op in ops {
                    log::debug!("  {:?}", op);

                    let start_tick_count = state.mark_reference(&CFG, hw_tick_count);

                    log::trace!("    HW = {}, OS = {}", hw_tick_count, start_tick_count);
                    log::trace!("    state = {:?}", state);

                    assert_eq!(state.tick_count(&CFG, hw_tick_count), start_tick_count);

                    // How many HW ticks does it take to wait for `op.timeout`?
                    let end_hw_tick_count = if op.timeout == 0 {
                        hw_tick_count
                    } else {
                        let end_tick_count = add_mod(start_tick_count, op.timeout, PERIOD);
                        log::trace!("    Want to wait until OS = {}", end_tick_count);
                        state.tick_count_to_hw_tick_count(&CFG, end_tick_count)
                    };
                    let len_hw_tick_count = sub_mod(end_hw_tick_count, hw_tick_count, HW_PERIOD);

                    log::trace!(
                        "    Should wait for {} HW ticks (end HW = {})",
                        len_hw_tick_count,
                        end_hw_tick_count
                    );

                    // Extend the timeout by an interrupt latency
                    let late_len_hw_tick_count = len_hw_tick_count + op.latency;
                    assert!(late_len_hw_tick_count <= CFG.hw_max_tick_count());

                    log::trace!("    Will wait for {} HW ticks", late_len_hw_tick_count);

                    // OS tick count should increase monotonically (this
                    // property is assumed, not checked here) while we are
                    // waiting for the next tick
                    let mut last_tick_count = start_tick_count;
                    let mut elapsed = 0;
                    let sample_points = merge(
                        choose_values_from_range(0..=late_len_hw_tick_count),
                        vec![len_hw_tick_count.saturating_sub(1), len_hw_tick_count],
                    );
                    for hw_elapsed in sample_points {
                        log::trace!("    - HW = {} + {}", hw_tick_count, hw_elapsed);

                        let hw_tick_count = add_mod(hw_tick_count, hw_elapsed, HW_PERIOD);
                        let tick_count = state.tick_count(&CFG, hw_tick_count);
                        elapsed += sub_mod(tick_count, last_tick_count, PERIOD);
                        last_tick_count = tick_count;

                        log::trace!(
                            "      OS = {} ({} + {})",
                            tick_count,
                            start_tick_count,
                            elapsed
                        );

                        // The OS tick count shouldn't increase more than
                        // `CFG.max_tick_count()` between timer interrupts or
                        // the kernel would lose track of time
                        assert!(elapsed <= CFG.max_tick_count());

                        if hw_elapsed < len_hw_tick_count {
                            // `len_hw_tick_count` must be the minimum amount
                            // of waiting required to fulfill the request
                            assert!(elapsed < op.timeout);
                        }
                    }

                    // Must wait at least for the specified duration
                    assert!(elapsed >= op.timeout);

                    hw_tick_count = add_mod(hw_tick_count, late_len_hw_tick_count, HW_PERIOD);
                }
            }

            #[test]
            fn ones() {
                do_test(
                    std::iter::repeat(Op {
                        timeout: 1,
                        latency: 0,
                    })
                    .take(10),
                );
            }

            #[test]
            fn ones_max_latency() {
                do_test(
                    std::iter::repeat(Op {
                        timeout: 1,
                        latency: $hw_headroom_ticks,
                    })
                    .take(10),
                );
            }

            #[test]
            fn max_timeout_max_latency() {
                do_test(
                    std::iter::repeat(Op {
                        timeout: MAX_TIMEOUT,
                        latency: $hw_headroom_ticks,
                    })
                    .take(10),
                );
            }

            #[test]
            fn max_timeout() {
                do_test(
                    std::iter::repeat(Op {
                        timeout: MAX_TIMEOUT,
                        latency: 0,
                    })
                    .take(10),
                );
            }

            #[test]
            fn max_timeout_and_zero() {
                do_test(vec![
                    Op {
                        timeout: MAX_TIMEOUT,
                        latency: 0,
                    },
                    Op {
                        timeout: 0,
                        latency: 0,
                    },
                    Op {
                        timeout: MAX_TIMEOUT,
                        latency: 0,
                    },
                    Op {
                        timeout: 0,
                        latency: 0,
                    },
                ]);
            }

            // FIXME: Using `#[quickcheck]` here causes "undefined identifier"
            //        errors for `do_test`, etc.
            #[test]
            fn quickcheck() {
                quickcheck::quickcheck::<fn(Vec<u64>, u32, u32)>(
                    |values: Vec<u64>, s1: u32, s2: u32| {
                        do_test(
                            values
                                .chunks_exact(2)
                                .map(|c| Op {
                                    timeout: (c[0].rotate_left(s1) % (MAX_TIMEOUT as u64 + 1))
                                        as u32,
                                    latency: (c[1].rotate_left(s2) % ($hw_headroom_ticks + 1))
                                        as u32,
                                })
                                .take(10),
                        );
                    },
                );
            }
        }
    }

    tickless_simulate!(mod sim1 {}, 1, 1, 1);
    tickless_simulate!(mod sim2 {}, 125_000_000, 1, 125);
    tickless_simulate!(mod sim3 {}, 375_000_000, 1, 1250);
    tickless_simulate!(mod sim4 {}, 125_000_000, 3, 0);
    tickless_simulate!(mod sim5 {}, 125_000_000, 3, 125);
    tickless_simulate!(mod sim6 {}, 125_000_000, 3, 125_000_000);
    tickless_simulate!(mod sim7 {}, 125_000_000, 3, 0xffff_ffa7);
    tickless_simulate!(mod sim8 {}, 10_000_000, 1, 1);
    tickless_simulate!(mod sim9 {}, 375, 1, 250_000);
    tickless_simulate!(mod sim10 {}, 1, 260, 0);
    tickless_simulate!(mod sim11 {}, 1, 260, 1);
    tickless_simulate!(mod sim12 {}, 1, 260, 10);
    tickless_simulate!(mod sim13 {}, 0x501e_e2c2_9a0f, 0xb79a, 0);
    tickless_simulate!(mod sim14 {}, 0x501e_e2c2_9a0f, 0xb79a, 0x64);
    tickless_simulate!(mod sim15 {}, 0x501e_e2c2_9a0f, 0xb79a, 0x1_0000);
    tickless_simulate!(mod sim16 {}, 0x501e_e2c2_9a0f, 0xb79a_14f3, 0);
    tickless_simulate!(mod sim17 {}, 0x501e_e2c2_9a0f, 0xb79a_14f3, 0x64);
    tickless_simulate!(mod sim18 {}, 0xb79a_14f3, 0x1e_e2c2_9a0f, 1);
    tickless_simulate!(mod sim19 {}, 0xff_ffff_ffff_ffff, 0xff_ffff_fffe, 0x41);
}
