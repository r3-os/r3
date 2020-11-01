//! Implements the core algorithm for tickless timing.
use core::fmt;
use num_rational::Ratio;

use crate::{
    num::{
        ceil_div128, floor_ratio128, gcd128, min128, reduce_ratio128,
        wrapping::{Wrapping, WrappingTrait},
    },
    utils::Init,
};

/// The parameters of the tickless timing algorithm.
///
/// It can be passed to [`TicklessCfg::new`] to construct [`TicklessCfg`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TicklessOptions {
    /// The numerator of the hardware timer frequency.
    pub hw_freq_num: u64,
    /// The denominator of the hardware timer frequency.
    pub hw_freq_denom: u64,
    /// The headroom for interrupt latency, measured in hardware timer cycles.
    pub hw_headroom_ticks: u32,
    /// Forces [`hw_max_tick_count`] to be `u32::MAX`. This might require the
    /// use of a less-efficient algorithm.
    ///
    /// [`hw_max_tick_count`]: TicklessCfg::hw_max_tick_count
    pub force_full_hw_period: bool,
    /// Allow the use of [`TicklessStateTrait::reset`].
    pub resettable: bool,
}

/// Error type for [`TicklessCfg::new`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CfgError {
    /// The numerator of the clock frequency is zero.
    FreqNumZero,
    /// The denominator of the clock frequency is zero.
    FreqDenomZero,
    /// The clock frequency is too high.
    FreqTooHigh,
    /// Intermediate calculation overflowed. the clock frequency might be too
    /// complex or too low.
    InternalOverflow,
    /// The calculated value of [`TicklessCfg::max_timeout`] is too low.
    OSMaxTimeoutTooLow,
}

impl CfgError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FreqNumZero => "the numerator of the clock frequency must not be zero",
            Self::FreqDenomZero => "the denominator of the clock frequency must not be zero",
            Self::FreqTooHigh => "the timer frequency is too fast",
            Self::InternalOverflow => {
                "intermediate calculation overflowed. the clock frequency might \
                 be too complex or too low"
            }
            Self::OSMaxTimeoutTooLow => {
                "the calculated maximum OS timeout is too low. lowering the \
                 interrupt latency headroom might help"
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
    pub const fn new(
        TicklessOptions {
            hw_freq_num,
            hw_freq_denom,
            hw_headroom_ticks,
            force_full_hw_period,
            resettable,
        }: TicklessOptions,
    ) -> Result<Self, CfgError> {
        if hw_freq_denom == 0 {
            return Err(CfgError::FreqDenomZero);
        } else if hw_freq_num == 0 {
            return Err(CfgError::FreqNumZero);
        }

        // `hw_ticks_per_micro = freq_num / freq_denom / 1_000_000`
        let hw_ticks_per_micro =
            Ratio::new_raw(hw_freq_num as u128, hw_freq_denom as u128 * 1_000_000);
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
            return Err(CfgError::FreqTooHigh);
        }

        if *hw_ticks_per_micro.denom() > u64::MAX as u128 {
            return Err(CfgError::InternalOverflow);
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
            && (!force_full_hw_period ||
                (0x1_0000_0000 % hw_global_period == 0
                 && hw_global_period >= global_period))
            && !resettable
        {
            // If the period is measurable without wrap-around in both ticks,
            // the stateless algorithm is applicable.
            let repeat = min128(
                0x1_0000_0000 / hw_global_period,
                0x1_0000_0000 / global_period,
            );
            let hw_max_tick_count = hw_global_period * repeat - 1;
            let max_tick_count = global_period * repeat - 1;

            if force_full_hw_period {
                assert!(hw_max_tick_count == u32::MAX as u128);
            }

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
                return Err(CfgError::OSMaxTimeoutTooLow);
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
                return Err(CfgError::OSMaxTimeoutTooLow);
            }
            assert!(max_timeout <= u32::MAX as u128);

            (TicklessAlgorithm::Stateful, max_timeout as u32)
        };

        Ok(Self {
            hw_ticks_per_micro: hw_ticks_per_micro_floor as u32,
            hw_subticks_per_micro: hw_subticks_per_micro as u64,
            algorithm,
            division: *hw_ticks_per_micro.denom() as u64,
            max_timeout,
        })
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
    #[inline]
    pub const fn max_tick_count(&self) -> u32 {
        match self.algorithm {
            TicklessAlgorithm::Stateless { max_tick_count, .. } => max_tick_count,
            TicklessAlgorithm::Stateful => u32::MAX,
        }
    }

    /// Get the maximum time interval that can be reliably measured, taking an
    /// interrupt latency into account.
    #[inline]
    pub const fn max_timeout(&self) -> u32 {
        self.max_timeout
    }

    /// Get the subtick division.
    #[inline]
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

#[cfg_attr(doc, svgbobdoc::transform)]
/// The stateless and tickless implementation of
/// [`constance::kernel::PortTimer`].
///
/// The stateless algorithm is chosen if the hardware ticks and OS ticks “line
/// up” periodically with a period shorter than the representable ranges of both
/// tick counts.
///
/// <center>
/// ```svgbob
///  HW ticks    ┌──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┐
///  ³/₇μs/tick  0                    7                    14                   21
///              ,                    ,                    ,           (hw_max_tick_count + 1)
///              |                    |                    |                    ,
///              '                    '                    '                    '
///  OS ticks    ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
///  1μs/tick    0                    3                    6                    9
///                                                                     (max_tick_count + 1)
/// ```
/// </center>
///
#[doc(include = "./common.md")]
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

/// Operations implemented by all valid instantiations of [`TicklessState`].
#[doc(include = "./common.md")]
pub trait TicklessStateTrait: Init + Copy + core::fmt::Debug {
    /// Mark the given hardware tick count as the origin (where
    /// OS tick count is exactly zero).
    ///
    /// To use this method, [`TicklessOptions::resettable`] must be set to
    /// `true` when constructing [`TicklessCfg`].
    ///
    /// `self` must be in the initial state (`Init::INIT`) when this method is
    /// called.
    fn reset(&mut self, cfg: &TicklessCfg, hw_tick_count: u32);

    /// Mark a reference point. Returns the reference point's OS tick count
    /// (in range `0..=cfg.`[`max_tick_count`]`()`).
    ///
    /// `hw_tick_count` should be in range `0..=cfg.`[`hw_max_tick_count`]`()`
    /// and satisfy the requirements of [`TicklessStateTrait::tick_count`].
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
    ///
    /// [`max_tick_count`]: TicklessCfg::max_tick_count
    /// [`hw_max_tick_count`]: TicklessCfg::hw_max_tick_count
    fn mark_reference(&mut self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32;

    /// [Mark a reference point] and start measuring the specified time interval
    /// `ticks` (measured in OS ticks = microseconds).
    ///
    /// The caller can use the information contained in the returned
    /// [`Measurement`] to configure timer hardware and receive an interrupt
    /// at the end of measurement.
    ///
    /// `hw_tick_count` should be in range `0..=cfg.`[`hw_max_tick_count`]`()`
    /// and satisfy the requirements of [`TicklessStateTrait::tick_count`].
    ///
    /// `ticks` should be in range `1..=cfg.`[`max_timeout`]`()`.
    ///
    /// [Mark a reference point]: Self::mark_reference
    /// [`hw_max_tick_count`]: TicklessCfg::hw_max_tick_count
    /// [`max_timeout`]: TicklessCfg::max_timeout
    #[inline]
    fn mark_reference_and_measure(
        &mut self,
        cfg: &TicklessCfg,
        hw_tick_count: u32,
        ticks: u32,
    ) -> Measurement {
        debug_assert_ne!(ticks, 0);

        let cur_tick_count = self.mark_reference(cfg, hw_tick_count);
        let end_tick_count = add_mod_u32(cur_tick_count, ticks, cfg.max_tick_count());
        let end_hw_tick_count = self.tick_count_to_hw_tick_count(cfg, end_tick_count);
        let hw_ticks = sub_mod_u32(end_hw_tick_count, hw_tick_count, cfg.hw_max_tick_count());

        #[track_caller]
        #[inline]
        fn add_mod_u32(x: u32, y: u32, max: u32) -> u32 {
            debug_assert!(x <= max);
            debug_assert!(y <= max);
            if max == u32::MAX || (max - x) >= y {
                x.wrapping_add(y)
            } else {
                x.wrapping_add(y).wrapping_add(u32::MAX - max)
            }
        }

        #[track_caller]
        #[inline]
        fn sub_mod_u32(x: u32, y: u32, max: u32) -> u32 {
            debug_assert!(x <= max);
            debug_assert!(y <= max);
            if max == u32::MAX || y < x {
                x.wrapping_sub(y)
            } else {
                x.wrapping_sub(y).wrapping_sub(u32::MAX - max)
            }
        }

        Measurement {
            end_hw_tick_count,
            hw_ticks,
        }
    }

    #[cfg_attr(doc, svgbobdoc::transform)]
    /// Calculate the earliest hardware tick count representing a point of time
    /// that coincides or follows the one represented by the specified OS tick
    /// count.
    ///
    /// Returns a value in range `0..=cfg.`[`hw_max_tick_count`]`()`.
    ///
    /// `tick_count` must satisfy the following condition: Given a last
    /// reference point `ref_tick_count` (a value returned by
    /// [`mark_reference`]), there must exist `i` such that
    /// `i ∈ 1..=cfg.`[`max_timeout`]`()` and `tick_count == (ref_tick_count +
    /// i) % (cfg.`[`max_tick_count`]`() + 1)`.
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
    /// <center>
    /// ```svgbob
    ///                          timer interrupt,
    ///                        calls mark_reference
    ///                                |
    ///                                v
    ///  HW ticks    ┌──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┐
    ///  ³/₇μs/tick  0                 ,  7                    14
    ///                            ┌───┘
    ///                            '
    ///  OS ticks    ┌──────┬──────┬──────┬──────┬──────┬──────┐
    ///  1μs/tick    0             ^                           6
    ///                            |
    ///                      ref_tick_count
    /// ```
    /// </center>
    ///
    /// `cfg` must be the instance of [`TicklessCfg`] that was passed to
    /// [`TicklessState`] to derive `Self`.
    ///
    /// [`mark_reference`]: Self::mark_reference
    /// [`max_timeout`]: TicklessCfg::max_timeout
    /// [`max_tick_count`]: TicklessCfg::max_tick_count
    /// [`hw_max_tick_count`]: TicklessCfg::hw_max_tick_count
    fn tick_count_to_hw_tick_count(&self, cfg: &TicklessCfg, tick_count: u32) -> u32;

    #[cfg_attr(doc, svgbobdoc::transform)]
    /// Get the OS tick count
    /// (in range `0..=cfg.`[`max_tick_count`]`()`).
    ///
    /// `cfg` must be the instance of [`TicklessCfg`] that was passed to
    /// [`TicklessState`] to derive `Self`.
    ///
    /// `hw_tick_count` should be in range `0..=cfg.`[`hw_max_tick_count`]`()`.
    /// In addition, `hw_tick_count` must satisfy the following condition:
    ///
    ///  - Let `ref_hw_tick_count` and `ref_tick_count` be the last reference
    ///    point (the last values passed to and returned by [`mark_reference`],
    ///    respectively).
    ///  - Let `period = cfg.`[`max_tick_count`]`() + 1`.
    ///  - Let `hw_period = cfg.`[`hw_max_tick_count`]`() + 1`.
    ///  - Let `hw_max_timeout = (tick_count_to_hw_tick_count((ref_tick_count +
    ///    cfg.max_timeout) % period) + hw_period - ref_hw_tick_count) %
    ///    hw_period`.
    ///  - There must exist `hw_timeout` and `latency` such that
    ///    `hw_timeout ∈ 0..=hw_max_timeout`, `latency ∈ 0..=hw_headroom_ticks`,
    ///    and `hw_tick_count == (ref_hw_tick_count + hw_timeout + latency) %
    ///    hw_period`.
    ///
    /// **Note:** `ref_hw_tick_count` should not be confused with the
    /// identically-named private field of [`TicklessStateCore`].
    ///
    /// <center>
    /// ```svgbob
    ///                       ref_hw_tick_count
    ///                                │
    ///          hw_headroom_ticks     │            hw_max_timeout
    ///                  │             v                    │
    ///              ────┴───────,     ,────────────────────┴─────────────────, ,────
    ///                          '     '                                      ' '
    ///              ░░░░░░░░░░░░░     ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
    ///  HW ticks    ┌──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┐
    ///  ³/₇μs/tick  0                 ,  7                    14             ,     21
    ///                            ┌───┘                                     ┌┘  hw_period
    ///                            '                                         '
    ///  OS ticks    ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
    ///  1μs/tick    0             ,      3                    6             ,      12
    ///                            '───────────────────────────────────┬─────'    period
    ///                            ^                                   │
    ///                            |                              max_timeout
    ///                   ref_tick_count
    /// ```
    /// </center>
    ///
    /// In the above diagram, `hw_tick_count` should fall within the filled
    /// zone.
    ///
    /// [`max_tick_count`]: TicklessCfg::max_tick_count
    /// [`max_timeout`]: TicklessCfg::max_timeout
    /// [`hw_max_tick_count`]: TicklessCfg::hw_max_tick_count
    /// [`mark_reference`]: Self::mark_reference
    fn tick_count(&self, cfg: &TicklessCfg, hw_tick_count: u32) -> u32;
}

/// Result type of [`TicklessStateTrait::mark_reference_and_measure`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Measurement {
    /// The hardware tick count at which the measurement ends.
    ///
    /// This value is equal to `(hw_tick_count + self.hw_ticks) %
    /// (cfg.`[`hw_max_tick_count`]`() + 1)`.
    ///
    /// [`hw_max_tick_count`]: TicklessCfg::hw_max_tick_count
    pub end_hw_tick_count: u32,
    /// The number of hardware ticks in the measured interval.
    pub hw_ticks: u32,
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
    fn reset(&mut self, _cfg: &TicklessCfg, _hw_tick_count: u32) {
        // `TicklessStatelessCore` can be chosen only if
        // `TicklessOptions::resettable` was set to `false`
        unreachable!()
    }

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
    fn reset(&mut self, _cfg: &TicklessCfg, hw_tick_count: u32) {
        debug_assert_eq!(self.ref_tick_count, 0);
        self.ref_hw_tick_count = hw_tick_count;
    }

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
            TicklessCfg::new(TicklessOptions {
                hw_freq_num: 1,
                hw_freq_denom: 1,
                hw_headroom_ticks: 1,
                force_full_hw_period: false,
                resettable: false,
            })
            .unwrap(),
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

        // 1Hz clock, 1-cycle period = 1s, 1-cycle latency tolerance
        // `hw_max_tick_count` is fixed at `u32::MAX`
        assert_eq!(
            TicklessCfg::new(TicklessOptions {
                hw_freq_num: 1,
                hw_freq_denom: 1,
                hw_headroom_ticks: 1,
                force_full_hw_period: true,
                resettable: false,
            })
            .unwrap(),
            TicklessCfg {
                hw_ticks_per_micro: 0,
                hw_subticks_per_micro: 1,
                algorithm: TicklessAlgorithm::Stateful,
                division: 1_000_000,
                max_timeout: 4_292_967_296,
            },
        );
    }

    /// The clock frequency given to `TicklessCfg` must not be zero.
    #[test]
    fn tickless_zero_freq() {
        assert_eq!(
            TicklessCfg::new(TicklessOptions {
                hw_freq_num: 0,
                hw_freq_denom: 1,
                hw_headroom_ticks: 1,
                force_full_hw_period: false,
                resettable: false,
            }),
            Err(CfgError::FreqNumZero)
        );
    }

    /// The denominator of the clock frequency given to `TicklessCfg` must not be
    /// zero.
    #[test]
    fn tickless_zero_denom() {
        assert_eq!(
            TicklessCfg::new(TicklessOptions {
                hw_freq_num: 1,
                hw_freq_denom: 0,
                hw_headroom_ticks: 1,
                force_full_hw_period: false,
                resettable: false,
            }),
            Err(CfgError::FreqDenomZero)
        );
    }

    /// `TicklessCfg` should reject a timer frequency that is too fast.
    #[test]
    fn tickless_tick_too_fast() {
        // 2³²MHz → 2³² HW ticks/μs
        assert_eq!(
            TicklessCfg::new(TicklessOptions {
                hw_freq_num: 1_000_000 * 0x1_0000_0000,
                hw_freq_denom: 1,
                hw_headroom_ticks: 0,
                force_full_hw_period: false,
                resettable: false,
            }),
            Err(CfgError::FreqTooHigh)
        );
    }

    /// `TicklessCfg` should reject if an intermediate value overflows.
    #[test]
    fn tickless_tick_too_complex() {
        // 1.00000000000000000043368086899420177... Hz
        // (0x1fffffffffffffff is a Mersenne prime number.)
        assert_eq!(
            TicklessCfg::new(TicklessOptions {
                hw_freq_num: 0x1fffffffffffffff,
                hw_freq_denom: 0x1ffffffffffffffe,
                hw_headroom_ticks: 0,
                force_full_hw_period: false,
                resettable: false,
            }),
            Err(CfgError::InternalOverflow)
        );
    }

    #[quickcheck_macros::quickcheck]
    fn quickcheck_cfg(
        hw_freq_num: u64,
        hw_freq_denom: u64,
        hw_headroom_ticks: u32,
        force_full_hw_period: bool,
        resettable: bool,
    ) {
        // `TicklessCfg::new` includes various integrity checks
        let _ = TicklessCfg::new(TicklessOptions {
            hw_freq_num,
            hw_freq_denom,
            hw_headroom_ticks,
            force_full_hw_period,
            resettable,
        });
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
        mod $ident:ident {},
        $freq_num:expr,
        $freq_denom:expr,
        $hw_headroom_ticks:expr,
        $force_full_hw_period:expr,
        $resettable:expr $(,)*
    ) {
        mod $ident {
            use super::*;

            const CFG: TicklessCfg = match TicklessCfg::new(TicklessOptions {
                hw_freq_num: $freq_num,
                hw_freq_denom: $freq_denom,
                hw_headroom_ticks: $hw_headroom_ticks,
                force_full_hw_period: $force_full_hw_period,
                resettable: $resettable,
            }) {
                Ok(x) => x,
                Err(e) => e.panic(),
            };
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

                if $resettable {
                    hw_tick_count = 0x1234567;

                    // The current implement chooses the stateful algorithm
                    // (`hw_max_tick_count == u32::MAX`) when `resettable` is
                    // set
                    assert!(hw_tick_count <= CFG.hw_max_tick_count());

                    state.reset(&CFG, hw_tick_count);
                }

                let tick_count = state.tick_count(&CFG, hw_tick_count);
                log::trace!("    HW = {}, OS = {}", hw_tick_count, tick_count);
                assert_eq!(tick_count, 0);

                for op in ops {
                    log::debug!("  {:?}", op);

                    let mut state2 = state;
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

                    // Do the same calculatioon with `mark_reference_and_measure`.
                    // The two results must be congruent. Skip this if `op.timeout
                    // == 0`, in which case `mark_reference_and_measure` should
                    // not be used.
                    if op.timeout != 0 {
                        let measurement =
                            state2.mark_reference_and_measure(&CFG, hw_tick_count, op.timeout);

                        assert_eq!(measurement.end_hw_tick_count, end_hw_tick_count);
                        assert_eq!(measurement.hw_ticks, len_hw_tick_count);
                    }

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

    tickless_simulate!(mod sim1 {}, 1, 1, 1, false, false);
    tickless_simulate!(mod sim2 {}, 125_000_000, 1, 125, false, false);
    tickless_simulate!(mod sim3 {}, 375_000_000, 1, 1250, false, false);
    tickless_simulate!(mod sim4 {}, 125_000_000, 3, 0, false, false);
    tickless_simulate!(mod sim5 {}, 125_000_000, 3, 125, false, false);
    tickless_simulate!(mod sim6 {}, 125_000_000, 3, 125_000_000, false, false);
    tickless_simulate!(mod sim7 {}, 125_000_000, 3, 0xffff_ffa7, false, false);
    tickless_simulate!(mod sim8 {}, 10_000_000, 1, 1, false, false);
    tickless_simulate!(mod sim9 {}, 375, 1, 250_000, false, false);
    tickless_simulate!(mod sim10 {}, 1, 260, 0, false, false);
    tickless_simulate!(mod sim11 {}, 1, 260, 1, false, false);
    tickless_simulate!(mod sim12 {}, 1, 260, 10, false, false);
    tickless_simulate!(mod sim13 {}, 0x501e_e2c2_9a0f, 0xb79a, 0, false, false);
    tickless_simulate!(mod sim14 {}, 0x501e_e2c2_9a0f, 0xb79a, 0x64, false, false);
    tickless_simulate!(
        mod sim15 {},
        0x501e_e2c2_9a0f,
        0xb79a,
        0x1_0000,
        false,
        false
    );
    tickless_simulate!(mod sim16 {}, 0x501e_e2c2_9a0f, 0xb79a_14f3, 0, false, false);
    tickless_simulate!(
        mod sim17 {},
        0x501e_e2c2_9a0f,
        0xb79a_14f3,
        0x64,
        false,
        false
    );
    tickless_simulate!(mod sim18 {}, 0xb79a_14f3, 0x1e_e2c2_9a0f, 1, false, false);
    tickless_simulate!(
        mod sim19 {},
        0xff_ffff_ffff_ffff,
        0xff_ffff_fffe,
        0x41,
        false,
        false,
    );

    tickless_simulate!(mod sim1_full {}, 1, 1, 1, true, false);
    tickless_simulate!(mod sim2_full {}, 125_000_000, 1, 125, true, false);
    tickless_simulate!(mod sim3_full {}, 375_000_000, 1, 1250, true, false);
    tickless_simulate!(mod sim4_full {}, 125_000_000, 3, 0, true, false);
    tickless_simulate!(mod sim5_full {}, 125_000_000, 3, 125, true, false);
    tickless_simulate!(mod sim6_full {}, 125_000_000, 3, 125_000_000, true, false);
    tickless_simulate!(mod sim7_full {}, 125_000_000, 3, 0xffff_ffa7, true, false);
    tickless_simulate!(mod sim8_full {}, 10_000_000, 1, 1, true, false);
    tickless_simulate!(mod sim9_full {}, 375, 1, 250_000, true, false);
    tickless_simulate!(mod sim10_full {}, 1, 260, 0, true, false);
    tickless_simulate!(mod sim11_full {}, 1, 260, 1, true, false);
    tickless_simulate!(mod sim12_full {}, 1, 260, 10, true, false);
    tickless_simulate!(mod sim13_full {}, 0x501e_e2c2_9a0f, 0xb79a, 0, true, false);
    tickless_simulate!(
        mod sim14_full {},
        0x501e_e2c2_9a0f,
        0xb79a,
        0x64,
        true,
        false
    );
    tickless_simulate!(
        mod sim15_full {},
        0x501e_e2c2_9a0f,
        0xb79a,
        0x1_0000,
        true,
        false
    );
    tickless_simulate!(
        mod sim16_full {},
        0x501e_e2c2_9a0f,
        0xb79a_14f3,
        0,
        true,
        false
    );
    tickless_simulate!(
        mod sim17_full {},
        0x501e_e2c2_9a0f,
        0xb79a_14f3,
        0x64,
        true,
        false
    );
    tickless_simulate!(
        mod sim18_full {},
        0xb79a_14f3,
        0x1e_e2c2_9a0f,
        1,
        true,
        false
    );
    tickless_simulate!(
        mod sim19_full {},
        0xff_ffff_ffff_ffff,
        0xff_ffff_fffe,
        0x41,
        true,
        false,
    );

    tickless_simulate!(mod sim1_reset {}, 1, 1, 1, false, true);
    tickless_simulate!(mod sim2_reset {}, 125_000_000, 1, 125, false, true);
    tickless_simulate!(mod sim3_reset {}, 375_000_000, 1, 1250, false, true);
    tickless_simulate!(mod sim4_reset {}, 125_000_000, 3, 0, false, true);
    tickless_simulate!(mod sim5_reset {}, 125_000_000, 3, 125, false, true);
    tickless_simulate!(mod sim6_reset {}, 125_000_000, 3, 125_000_000, false, true);
    tickless_simulate!(mod sim7_reset {}, 125_000_000, 3, 0xffff_ffa7, false, true);
    tickless_simulate!(mod sim8_reset {}, 10_000_000, 1, 1, false, true);
    tickless_simulate!(mod sim9_reset {}, 375, 1, 250_000, false, true);
    tickless_simulate!(mod sim10_reset {}, 1, 260, 0, false, true);
    tickless_simulate!(mod sim11_reset {}, 1, 260, 1, false, true);
    tickless_simulate!(mod sim12_reset {}, 1, 260, 10, false, true);
    tickless_simulate!(mod sim13_reset {}, 0x501e_e2c2_9a0f, 0xb79a, 0, false, true);
    tickless_simulate!(
        mod sim14_reset {},
        0x501e_e2c2_9a0f,
        0xb79a,
        0x64,
        false,
        true
    );
    tickless_simulate!(
        mod sim15_reset {},
        0x501e_e2c2_9a0f,
        0xb79a,
        0x1_0000,
        false,
        true
    );
    tickless_simulate!(
        mod sim16_reset {},
        0x501e_e2c2_9a0f,
        0xb79a_14f3,
        0,
        false,
        true
    );
    tickless_simulate!(
        mod sim17_reset {},
        0x501e_e2c2_9a0f,
        0xb79a_14f3,
        0x64,
        false,
        true
    );
    tickless_simulate!(
        mod sim18_reset {},
        0xb79a_14f3,
        0x1e_e2c2_9a0f,
        1,
        false,
        true
    );
    tickless_simulate!(
        mod sim19_reset {},
        0xff_ffff_ffff_ffff,
        0xff_ffff_fffe,
        0x41,
        false,
        true,
    );
}
