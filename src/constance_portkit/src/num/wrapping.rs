//! Wrapping counter types
use core::ops;

use crate::utils::Init;

/// Get a type implementing [`WrappingCounterTrait`] that wraps around when
/// incremented past `MAX`.
///
/// This type alias tries to choose the most efficient data type to do the job.
pub type WrappingCounter<const MAX: u64> = If! {
    if (MAX == 0) {
        ()
    } else if (MAX < u8::MAX as u64) {
        FractionalWrappingCounter<u8, MAX>
    } else if (MAX == u8::MAX as u64) {
        u8
    } else if (MAX < u16::MAX as u64) {
        FractionalWrappingCounter<u16, MAX>
    } else if (MAX == u16::MAX as u64) {
        u16
    } else if (MAX < u32::MAX as u64) {
        FractionalWrappingCounter<u32, MAX>
    } else if (MAX == u32::MAX as u64) {
        u32
    } else if (MAX < u64::MAX) {
        FractionalWrappingCounter<u64, MAX>
    } else {
        u64
    }
};

/// Represents a counter type that wraps around when incremented past a
/// predetermined upper bound `MAX` (this bound is not exposed but measurable).
pub trait WrappingCounterTrait: Init + Copy + core::fmt::Debug {
    /// Add a value to `self`. Returns `true` iff wrap-around has occurred.
    ///
    /// `rhs` must be less than or equal to `MAX`.
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool;

    /// Add a value to `self`. Returns the number of times for which wrap-around
    /// has occurred.
    ///
    /// The result must not overflow.
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32;

    fn to_u128(&self) -> u128;
}

impl WrappingCounterTrait for () {
    #[inline]
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
        rhs != 0
    }

    #[inline]
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
        rhs as u32
    }

    #[inline]
    fn to_u128(&self) -> u128 {
        0
    }
}

impl WrappingCounterTrait for u8 {
    #[inline]
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
        let (out, overflow) = self.overflowing_add(rhs as u8);
        *self = out;
        overflow
    }

    #[inline]
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
        debug_assert!(rhs < (1 << 40));
        let new_value = *self as u64 + rhs as u64;
        *self = new_value as u8;
        (new_value >> 8) as u32
    }

    #[inline]
    fn to_u128(&self) -> u128 {
        *self as u128
    }
}

impl WrappingCounterTrait for u16 {
    #[inline]
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
        let (out, overflow) = self.overflowing_add(rhs as u16);
        *self = out;
        overflow
    }

    #[inline]
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
        debug_assert!(rhs < (1 << 48));
        let new_value = *self as u64 + rhs as u64;
        *self = new_value as u16;
        (new_value >> 16) as u32
    }

    #[inline]
    fn to_u128(&self) -> u128 {
        *self as u128
    }
}

impl WrappingCounterTrait for u32 {
    #[inline]
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
        let (out, overflow) = self.overflowing_add(rhs as u32);
        *self = out;
        overflow
    }

    #[inline]
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
        debug_assert!(rhs < (1 << 64));
        let new_value = *self as u64 + rhs as u64;
        *self = new_value as u32;
        (new_value >> 32) as u32
    }

    #[inline]
    fn to_u128(&self) -> u128 {
        *self as u128
    }
}

impl WrappingCounterTrait for u64 {
    #[inline]
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
        let (out, overflow) = self.overflowing_add(rhs as u64);
        *self = out;
        overflow
    }

    #[inline]
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
        debug_assert!(rhs < (1 << 96));
        let new_value = *self as u128 + rhs;
        *self = new_value as u64;
        (new_value >> 64) as u32
    }

    #[inline]
    fn to_u128(&self) -> u128 {
        *self as u128
    }
}

/// Implementation of `WrappingCounterTrait` that wraps around at some boundary
/// that does not naturally occur from the binary representation of the integer
/// type.
///
/// `MAX` must be less than `T::MAX`.
#[derive(Debug, Copy, Clone)]
pub struct FractionalWrappingCounter<T, const MAX: u64> {
    inner: T,
}

impl<T: Init, const MAX: u64> Init for FractionalWrappingCounter<T, MAX> {
    const INIT: Self = Self { inner: Init::INIT };
}

impl<T, const MAX: u64> WrappingCounterTrait for FractionalWrappingCounter<T, MAX>
where
    T: From<u8>
        + core::convert::TryFrom<u64>
        + core::convert::TryFrom<u128>
        + core::convert::Into<u128>
        + ops::Add<Output = T>
        + ops::Sub<Output = T>
        + ops::Rem<Output = T>
        + PartialOrd
        + Copy
        + Init
        + core::fmt::Debug,
{
    #[inline]
    fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
        let t_max = if let Ok(x) = T::try_from(MAX) {
            x
        } else {
            unreachable!()
        };
        let t_rhs = if let Ok(x) = T::try_from(rhs) {
            x
        } else {
            unreachable!()
        };
        if MAX < u64::MAX && (MAX + 1).is_power_of_two() {
            // In this case, `x % (MAX + 1)` can be optimized to a fast bit-wise
            // operation.
            //
            // The conjunction of `MAX < T::MAX` and `(MAX + 1).is_power_of_two()`
            // entails `MAX < T::MAX / 2`. Therefore `self.inner + rhs` does
            // not overflow `T`.
            let new_value = self.inner + t_rhs;
            self.inner = new_value % (t_max + T::from(1));
            new_value >= t_max + T::from(1)
        } else if t_max - self.inner >= t_rhs {
            self.inner = self.inner + t_rhs;
            false
        } else {
            self.inner = t_rhs - (t_max - self.inner) - T::from(1);
            true
        }
    }

    #[inline]
    fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
        let new_value = self.inner.into() as u128 + rhs;
        self.inner = T::try_from(new_value % (MAX as u128 + 1)).ok().unwrap();

        let wrap_count = new_value / (MAX as u128 + 1);
        debug_assert!(wrap_count <= u32::MAX as u128);
        wrap_count as u32
    }

    #[inline]
    fn to_u128(&self) -> u128 {
        self.inner.into()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use core::convert::TryInto;
    use quickcheck_macros::quickcheck;
    use std::{prelude::v1::*, vec};

    /// The naïve implementation of `WrappingCounterTrait`.
    #[derive(Debug, Copy, Clone)]
    struct NaiveWrappingCounter<const MAX: u64> {
        inner: u128,
    }

    impl<const MAX: u64> Init for NaiveWrappingCounter<MAX> {
        const INIT: Self = Self { inner: 0 };
    }

    impl<const MAX: u64> WrappingCounterTrait for NaiveWrappingCounter<MAX> {
        fn wrapping_add_assign64(&mut self, rhs: u64) -> bool {
            assert!(rhs <= MAX);
            let new_value = self.inner + rhs as u128;
            self.inner = new_value % (MAX as u128 + 1);
            new_value > MAX as u128
        }

        fn wrapping_add_assign128_multi32(&mut self, rhs: u128) -> u32 {
            let new_value = self.inner + rhs;
            self.inner = new_value % (MAX as u128 + 1);
            let wrap_count = new_value / (MAX as u128 + 1);
            wrap_count.try_into().unwrap()
        }

        fn to_u128(&self) -> u128 {
            self.inner
        }
    }

    macro_rules! gen_counter_tests {
        ($($name:ident => $max:expr ,)*) => {$(
            mod $name {
                use super::*;

                const MAX: u128 = $max;

                fn do_test_add_assign64(values: impl IntoIterator<Item = u64>) {
                    let mut counter_got: WrappingCounter<{MAX as u64}> = Init::INIT;
                    let mut counter_expected: NaiveWrappingCounter<{MAX as u64}> = Init::INIT;
                    log::trace!("do_test_add_assign64 (MAX = {})", MAX);
                    for value in values {
                        log::trace!(
                            " - ({} + {}) % (MAX + 1) = {} % (MAX + 1) = {}",
                            counter_expected.inner,
                            value,
                            (counter_expected.inner + value as u128),
                            (counter_expected.inner + value as u128) % (MAX + 1),
                        );
                        let got = counter_got.wrapping_add_assign64(value);
                        let expected = counter_expected.wrapping_add_assign64(value);
                        assert_eq!(got, expected);
                    }
                }

                #[test]
                fn add_assign64_zero() {
                    do_test_add_assign64(vec![0, 0, 0,0, 0]);
                }

                #[test]
                fn add_assign64_mixed() {
                    let max = MAX as u64;
                    do_test_add_assign64(vec![0, 1u64.min(max), max, max / 2, max / 10, 0, 4u64.min(max)]);
                }

                #[test]
                fn add_assign64_max() {
                    do_test_add_assign64(vec![MAX as u64; 5]);
                }

                #[test]
                fn add_assign64_half() {
                    do_test_add_assign64(vec![MAX as u64 / 2; 5]);
                }

                #[quickcheck]
                fn add_assign64_quickcheck(cmds: Vec<u32>) {
                    do_test_add_assign64(cmds.iter().map(|&cmd| {
                        let max = MAX as u64;
                        match cmd % 4 {
                            0 => max / 2,
                            1 => (max / 2).saturating_add(1).min(max),
                            2 => max.saturating_sub(1),
                            3 => max,
                            _ => unreachable!(),
                        }.saturating_sub(cmd as u64 >> 2).min(max)
                    }));
                }

                fn do_test_add_assign128_multi32(values: impl IntoIterator<Item = u128>) {
                    let mut counter_got: WrappingCounter<{MAX as u64}> = Init::INIT;
                    let mut counter_expected: NaiveWrappingCounter<{MAX as u64}> = Init::INIT;
                    log::trace!("do_test_add_assign128_multi32 (MAX = {})", MAX);
                    for value in values {
                        log::trace!(
                            " - ({} + {}) % (MAX + 1) = {} % (MAX + 1) = {}",
                            counter_expected.inner,
                            value,
                            (counter_expected.inner + value),
                            (counter_expected.inner + value) % (MAX + 1),
                        );
                        let got = counter_got.wrapping_add_assign128_multi32(value);
                        let expected = counter_expected.wrapping_add_assign128_multi32(value);
                        assert_eq!(got, expected);
                    }
                }

                #[test]
                fn add_assign128_multi32_zero() {
                    do_test_add_assign128_multi32(vec![0; 5]);
                }

                #[test]
                fn add_assign128_multi32_mixed() {
                    do_test_add_assign128_multi32(vec![0, 1u128.min(MAX), MAX, MAX / 2, MAX / 10, 0, 4u128.min(MAX)]);
                }

                #[test]
                fn add_assign128_multi32_max() {
                    do_test_add_assign128_multi32(vec![MAX; 5]);
                }

                #[test]
                fn add_assign128_multi32_max_p1() {
                    do_test_add_assign128_multi32(vec![MAX + 1; 5]);
                }

                #[test]
                fn add_assign128_multi32_half() {
                    do_test_add_assign128_multi32(vec![MAX / 2; 5]);
                }

                #[test]
                fn add_assign128_multi32_extreme() {
                    do_test_add_assign128_multi32(vec![MAX, (MAX + 1) * 0xffff_ffff]);
                }

                #[test]
                #[should_panic]
                fn add_assign128_multi32_result_overflow() {
                    // `NaiveWrappingCounter` is guaranteed to panic on overflow
                    do_test_add_assign128_multi32(vec![MAX, (MAX + 1) * 0xffff_ffff + 1]);
                }

                #[quickcheck]
                fn add_assign128_multi32_quickcheck(cmds: Vec<u32>) {
                    do_test_add_assign128_multi32(cmds.iter().map(|&cmd| {
                        match cmd % 8 {
                            0 => MAX / 2,
                            1 => MAX / 2 + 1,
                            2 => MAX.saturating_sub(1),
                            3 => MAX,
                            4 => (MAX * 2).saturating_sub(1),
                            5 => MAX * 2 + 1,
                            6 => MAX * 0x1_0000_0000,
                            7 => (MAX + 1) * 0x1_0000_0000 - 1,
                            _ => unreachable!(),
                        }.saturating_sub(cmd as u128 >> 2).min(MAX)
                    }));
                }
            }
        )*};
    }

    gen_counter_tests!(
        c0 => 0,
        c1 => 1,
        c_u8_max_m1 => u8::MAX as u128 - 1,
        c_u8_max => u8::MAX as u128,
        c_u8_max2 => u8::MAX as u128 * 2,
        c_u8_max_p1 => u8::MAX as u128 + 1,
        c_u16_max_m1 => u16::MAX as u128 - 1,
        c_u16_max => u16::MAX as u128,
        c_u16_max2 => u16::MAX as u128 * 2,
        c_u16_max_p1 => u16::MAX as u128 + 1,
        c_u32_max_m1 => u32::MAX as u128 - 1,
        c_u32_max => u32::MAX as u128,
        c_u32_max2 => u32::MAX as u128 * 2,
        c_u32_max_p1 => u32::MAX as u128 + 1,
        c_u64_max_m1 => u64::MAX as u128 - 1,
        c_u64_max => u64::MAX as u128,
    );
}
