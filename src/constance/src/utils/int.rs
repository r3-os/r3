use core::{convert, fmt, ops};
use num_integer::Integer;

use super::{Init, ZeroInit};

/// Get the smallest unsigned integer type capable of representing the specified
/// value.
pub type UIntegerWithBound<const MAX: u128> = If! {
    if (MAX <= u8::MAX as u128) {
        u8
    } else if (MAX <= u16::MAX as u128) {
        u16
    } else if (MAX <= u32::MAX as u128) {
        u32
    } else if (MAX <= u64::MAX as u128) {
        u64
    } else {
        u128
    }
};

/// Integral types with efficient binary operations.
pub trait BinInteger:
    Integer
    + Clone
    + Copy
    + Sized
    + ops::AddAssign
    + ops::SubAssign
    + ops::MulAssign
    + ops::DivAssign
    + fmt::Debug
    + Init
    + ZeroInit
    + Send
    + Sync
    + num_traits::cast::ToPrimitive
    + convert::TryFrom<usize>
    + 'static
{
    type OneDigits: Iterator<Item = u32>;

    const BITS: u32;

    fn ones(range: ops::Range<u32>) -> Self;

    fn ones_truncated(range: ops::Range<u32>) -> Self;

    /// Return the number of trailing zeros in its binary representation.
    fn trailing_zeros(&self) -> u32;

    /// Return the number of leading zeros in its binary representation.
    fn leading_zeros(&self) -> u32;

    /// Return the number of ones in its binary representation.
    fn count_ones(&self) -> u32;

    /// Return the position of the least significant set bit since the position
    /// `start`.
    ///
    /// Retruns `Self::BITS` if none was found.
    fn bit_scan_forward(&self, start: u32) -> u32;

    /// Slice a part of its binary representation as `u32`.
    fn extract_u32(&self, range: ops::Range<u32>) -> u32;

    /// Retrieve whether the specified bit is set or not.
    fn get_bit(&self, i: u32) -> bool;

    /// Set a single bit.
    fn set_bit(&mut self, i: u32);

    /// Clear a single bit.
    fn clear_bit(&mut self, i: u32);

    /// Perform `ceil` treating the value as a fixed point number with `fp`
    /// fractional part digits.
    fn checked_ceil_fix(self, fp: u32) -> Option<Self>;

    /// Get an iterator over set bits, from the least significant bit to
    /// the most significant one.
    fn one_digits(&self) -> Self::OneDigits;
}

/// Unsigned integral types with efficient binary operations.
pub trait BinUInteger: BinInteger {
    /// Return `ture` if and only if `self == 2^k` for some `k`.
    fn is_power_of_two(&self) -> bool;
}

#[doc(hidden)]
pub struct OneDigits<T>(T);

macro_rules! impl_binary_integer {
    ($type:ty) => {
        impl BinInteger for $type {
            type OneDigits = OneDigits<Self>;

            const BITS: u32 = core::mem::size_of::<$type>() as u32 * 8;

            #[inline]
            fn ones(range: ops::Range<u32>) -> Self {
                assert!(range.end <= Self::BITS);
                Self::ones_truncated(range)
            }
            #[inline]
            fn ones_truncated(range: ops::Range<u32>) -> Self {
                assert!(range.start <= range.end);
                if range.end >= Self::BITS {
                    (0 as Self).wrapping_sub(1 << range.start)
                } else {
                    ((1 as Self) << range.end).wrapping_sub(1 << range.start)
                }
            }
            #[inline]
            fn trailing_zeros(&self) -> u32 {
                (*self).trailing_zeros()
            }
            #[inline]
            fn leading_zeros(&self) -> u32 {
                (*self).leading_zeros()
            }
            #[inline]
            fn count_ones(&self) -> u32 {
                (*self).count_ones()
            }
            #[inline]
            fn bit_scan_forward(&self, start: u32) -> u32 {
                if start >= Self::BITS {
                    Self::BITS
                } else {
                    (*self & !Self::ones(0..start)).trailing_zeros()
                }
            }
            #[inline]
            fn extract_u32(&self, range: ops::Range<u32>) -> u32 {
                let start = range.start;
                ((self & Self::ones_truncated(range)) >> start) as u32
            }
            #[inline]
            fn get_bit(&self, i: u32) -> bool {
                if i < Self::BITS {
                    self & ((1 as Self) << i) != 0
                } else {
                    false
                }
            }
            #[inline]
            fn set_bit(&mut self, i: u32) {
                if i < Self::BITS {
                    *self |= (1 as Self) << i;
                }
            }
            #[inline]
            fn clear_bit(&mut self, i: u32) {
                if i < Self::BITS {
                    *self &= !((1 as Self) << i);
                }
            }
            #[inline]
            fn checked_ceil_fix(self, fp: u32) -> Option<Self> {
                if fp >= Self::BITS {
                    if self == 0 {
                        Some(0)
                    } else {
                        None
                    }
                } else {
                    let mask = Self::ones(0..fp);
                    self.checked_add(mask).map(|x| x & !mask)
                }
            }
            #[inline]
            fn one_digits(&self) -> Self::OneDigits {
                OneDigits(*self)
            }
        }
        impl Iterator for OneDigits<$type> {
            type Item = u32;
            fn next(&mut self) -> Option<u32> {
                if self.0 == 0 {
                    None
                } else {
                    let index = self.0.trailing_zeros();
                    self.0 &= !((1 as $type) << index);
                    Some(index)
                }
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                let ones = self.len();
                (ones, Some(ones))
            }
            fn count(self) -> usize {
                self.len()
            }
        }
        impl ExactSizeIterator for OneDigits<$type> {
            fn len(&self) -> usize {
                self.0.count_ones() as usize
            }
        }
        impl DoubleEndedIterator for OneDigits<$type> {
            fn next_back(&mut self) -> Option<u32> {
                if self.0 == 0 {
                    None
                } else {
                    let index = <$type>::BITS - 1 - self.0.leading_zeros();
                    self.0 &= !((1 as $type) << index);
                    Some(index)
                }
            }
        }
    };
}

macro_rules! impl_binary_uinteger {
    ($type:ty) => {
        impl BinUInteger for $type {
            #[inline]
            fn is_power_of_two(&self) -> bool {
                Self::is_power_of_two(*self)
            }
        }
    };
}

impl_binary_integer!(i8);
impl_binary_integer!(i16);
impl_binary_integer!(i32);
impl_binary_integer!(i64);
impl_binary_integer!(i128);
impl_binary_integer!(isize);

impl_binary_integer!(u8);
impl_binary_integer!(u16);
impl_binary_integer!(u32);
impl_binary_integer!(u64);
impl_binary_integer!(u128);
impl_binary_integer!(usize);

impl_binary_uinteger!(u8);
impl_binary_uinteger!(u16);
impl_binary_uinteger!(u32);
impl_binary_uinteger!(u64);
impl_binary_uinteger!(u128);
impl_binary_uinteger!(usize);

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    macro_rules! gen_test {
        ($t:ident) => {
            mod $t {
                use super::*;

                #[test]
                fn is_power_of_two() {
                    assert!(!(&(0 as $t)).is_power_of_two());
                    assert!((&(1 as $t)).is_power_of_two());
                    assert!((&(2 as $t)).is_power_of_two());
                    assert!(!(&(3 as $t)).is_power_of_two());
                }

                #[quickcheck]
                fn one_digits(mut set_bits: Vec<u32>) -> bool {
                    // Wrap around the bit positions by the target type's size
                    for bit in set_bits.iter_mut() {
                        *bit = *bit % $t::BITS;
                    }

                    // Sort and remove duplicates, which gives us the expected
                    // sequence to be returned by `one_digits`
                    set_bits.sort();
                    set_bits.dedup();

                    // Create an integer
                    let i: $t = set_bits.iter().fold(0, |i, &bit| i | (1 << bit));

                    let got_set_bits: Vec<u32> = i.one_digits().collect();

                    log::trace!("i = 0x{:x}", i);
                    log::trace!("    got = {:?}", got_set_bits);
                    log::trace!("    expected = {:?}", set_bits);

                    got_set_bits == set_bits
                }
            }
        };
    }

    gen_test!(u8);
    gen_test!(u16);
    gen_test!(u32);
    gen_test!(u64);
    gen_test!(u128);
    gen_test!(usize);
}
