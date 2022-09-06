//! Count trailing zeros
#![allow(clippy::if_same_then_else)]

const USIZE_BITS: u32 = usize::BITS;

#[allow(clippy::needless_bool)]
const HAS_CTZ: bool = if cfg!(target_arch = "riscv32") || cfg!(target_arch = "riscv64") {
    cfg!(target_feature = "b") || cfg!(target_feature = "experimental-b")
} else if cfg!(target_arch = "arm") {
    // (It's actually CLZ + RBIT)
    // Thumb-2
    cfg!(target_feature = "v6t2")
        // Armv5T and later, only in Arm mode
        || (cfg!(target_feature = "v5te") && !cfg!(target_feature = "thumb-mode"))
} else if cfg!(target_arch = "msp430") {
    false
} else {
    // AArch64: All
    // x86: 80386 and later
    true
};

/// Indicates whether the target includes a 32-bit hardware multiplier.
#[allow(clippy::needless_bool)]
const HAS_MUL: bool = if cfg!(target_arch = "riscv32") || cfg!(target_arch = "riscv64") {
    cfg!(target_feature = "m")
} else if cfg!(target_arch = "msp430") {
    cfg!(target_feature = "hwmult32")
} else {
    // Classic Arm: Armv2 and later
    // Arm-A/R/M: All
    // x86: 8086 and later
    true
};

/// Indicates whether the target includes a 32-bit barrel shifter.
#[allow(clippy::needless_bool)]
const HAS_SHIFTER: bool = if cfg!(target_arch = "msp430") {
    false
} else if cfg!(target_arch = "avr") {
    false
} else {
    true
};

/// Indicates whether an array-based look-up table would be faster than other
/// techniques.
///
/// Some targets would use constant pools anyway. On such targets, bit
/// manipulation tricks relying on an instruction-embedded LUT would actually
/// read from a data bus anyway and therefore would never be faster than an
/// array-based LUT.
///
/// Small microcontrollers usually have a low-latency memory system and a
/// single-issue in-order pipeline. Bit manipulation tricks often require many
/// bit manipulation instructions to move bits into a correct place, which
/// sometimes over-weighs the cost of loading an LUT address and then loading
/// one of its entries. Examples: <https://rust.godbolt.org/z/961Pej> (Armv6-M
/// and Armv7-M), <https://cpp.godbolt.org/z/WPnxon> (MSP430 and AVR)
///
/// There are extreme cases that should be taken into consideration as well.
/// For example, SiFive E31 (used in SiFive Freedom E310) does not have a data
/// cache for XiP from an external SPI flash. Therefore, using an array-based
/// LUT on such systems would lead to a catastrophic performance degradation and
/// must be avoided at any cost.
#[allow(clippy::needless_bool)]
const HAS_FAST_LOAD: bool =
    if cfg!(target_arch = "arm") || cfg!(target_arch = "msp430") || cfg!(target_arch = "avr") {
        true
    } else {
        false
    };

/// Return the number of trailing zeros in `x` (`< 1 << BITS`). Returns
/// `usize::BITS` if `x` is zero.
#[inline]
pub fn trailing_zeros<const BITS: usize>(x: usize) -> u32 {
    if BITS == 0 {
        USIZE_BITS
    } else if BITS == 1 {
        if x == 0 {
            USIZE_BITS
        } else {
            0
        }
    } else if HAS_CTZ {
        x.trailing_zeros()
    } else if BITS == 2 && HAS_FAST_LOAD {
        ctz_array_lut::<4>(x)
    } else if BITS == 3 && HAS_FAST_LOAD {
        ctz_array_lut::<8>(x)
    } else if BITS == 4 && HAS_FAST_LOAD {
        ctz_array_lut::<16>(x)
    } else if BITS <= 2 {
        ctz2(x)
    } else if BITS <= 3 && HAS_SHIFTER {
        ctz3_lut(x)
    } else if BITS <= 4 && HAS_SHIFTER {
        ctz4_lut(x)
    } else if BITS <= 8 && HAS_MUL && HAS_SHIFTER {
        ctz8_debruijn(x)
    } else if BITS > 16 && HAS_MUL && HAS_SHIFTER {
        // Use LLVM's emulation code. At the point of writing, it uses a generic
        // algorithm based on the following one:
        // <http://graphics.stanford.edu/~seander/bithacks.html#CountBitsSetParallel>
        //
        // This algorithm requires loading multiple 32-bit-wide constants and
        // shifts by more than one bit and may be very inefficient on some
        // targets. On the other hand, it does not require branching.
        x.trailing_zeros()
    } else if HAS_SHIFTER {
        ctz_bsearch32::<BITS>(x)
    } else {
        ctz_linear::<BITS>(x)
    }
}

#[inline]
fn first_set_bit_mask(x: usize) -> usize {
    x & x.wrapping_neg()
}

/// Implements [`trailing_zeros`] using a [de Bruijn sequence].
/// `x` must be in range `0..0x100`.
///
/// [de Bruijn sequence]: https://en.wikipedia.org/wiki/De_Bruijn_sequence
#[inline]
fn ctz8_debruijn(x: usize) -> u32 {
    debug_assert!(x < 0x100);
    if x == 0 {
        USIZE_BITS
    } else {
        let pat = ((first_set_bit_mask(x) * 0b11101) >> 3) & 0b11100;
        (0b0011_0100_0101_0111_0010_0110_0001_0000 >> pat) & 0b111
    }
}

/// Implements [`trailing_zeros`] using a look-up table.
/// `x` must be in range `0..16`.
#[inline]
fn ctz4_lut(x: usize) -> u32 {
    debug_assert!(x < 16);
    if x == 0 {
        USIZE_BITS
    } else {
        ctz4_lut_nonzero(x)
    }
}

/// Implements [`trailing_zeros`] using a look-up table.
/// `x` must be in range `1..16`.
#[inline]
fn ctz4_lut_nonzero(x: usize) -> u32 {
    debug_assert!(x < 16 && x != 0);
    //  2  3  4  5  6  7  8  9 10 11 12 13 14 15
    (0b01_00_10_00_01_00_11_00_01_00_10_00_01_00 << (x as u32 * 2)) >> 30
}

/// Implements [`trailing_zeros`] using a look-up table.
/// `x` must be in range `0..8`.
#[inline]
fn ctz3_lut(x: usize) -> u32 {
    debug_assert!(x < 8);
    if x == 0 {
        USIZE_BITS
    } else {
        ctz3_lut_nonzero(x)
    }
}

/// Implements [`trailing_zeros`] using a look-up table.
/// `x` must be in range `1..8`.
#[inline]
// This code groups digits irregularly to express a specific meaning
#[allow(clippy::inconsistent_digit_grouping)]
fn ctz3_lut_nonzero(x: usize) -> u32 {
    debug_assert!(x < 8);
    debug_assert!(x != 0);

    //  2  3  4  5  6  7
    (0b01_00_10_00_01_00_0000_0000_0000_0000 << (x as u32 * 2)) >> 30

    // On RISC-V, the above code generates one fewer instruction compared to
    // the following one because a constant value whose bit[11:0] is zero
    // can be loaded with a single `lu` instruction
    //
    //  (0b00_01_00_10_00_01_00_00 >> (x * 2)) & 0b11
}

/// Implements [`trailing_zeros`].
/// `x` must be in range `0..4`.
#[inline]
fn ctz2(x: usize) -> u32 {
    debug_assert!(x < 4);
    if x == 0 {
        USIZE_BITS
    } else {
        (x & 1 ^ 1) as u32
    }
}

/// Implements [`trailing_zeros`] using an array-based look-up table.
#[inline]
fn ctz_array_lut<const LEN: usize>(x: usize) -> u32 {
    struct Lut<const LEN: usize>;
    trait LutTrait {
        const LUT: &'static [u8];
    }
    impl<const LEN: usize> LutTrait for Lut<LEN> {
        const LUT: &'static [u8] = &{
            let mut array = [0u8; LEN];
            // `for` is unusable in `const fn` [ref:const_for]
            let mut i = 0;
            while i < array.len() {
                array[i] = i.trailing_zeros() as u8;
                i += 1;
            }
            array
        };
    }

    let lut = Lut::<LEN>::LUT;
    lut[x & (lut.len() - 1)] as u32
}

/// Implements [`trailing_zeros`] using linear search.
#[inline]
fn ctz_linear<const BITS: usize>(mut x: usize) -> u32 {
    for i in 0..BITS as u32 {
        if x & 1 != 0 {
            return i;
        }
        x >>= 1;
    }
    USIZE_BITS
}

/// Implements [`trailing_zeros`] using binary search. The last level
/// is handled by [`ctz4_lut_nonzero`].
///
///`BITS` must be less than or equal to 32.
#[inline]
fn ctz_bsearch32<const BITS: usize>(x: usize) -> u32 {
    debug_assert!(BITS <= 32);
    let mut x = x as u32;

    if x == 0 {
        return USIZE_BITS;
    }

    let mut i = 0;

    if BITS > 16 && (x & 0xffff) == 0 {
        x >>= 16;
        i += 16;
    }

    if BITS > 8 && (x & 0xff) == 0 {
        x >>= 8;
        i += 8;
    }

    if BITS > 4 && (x & 0xf) == 0 {
        x >>= 4;
        i += 4;
        if BITS > 8 {
            x &= 0xf;
        }
    } else if BITS > 4 {
        x &= 0xf;
    }

    if HAS_FAST_LOAD {
        i += ctz_array_lut::<16>(x as usize);
    } else {
        i += ctz4_lut_nonzero(x as usize);
    }

    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    macro_rules! gen_test {
        ($mod_name:ident, $func:path, $bits:expr) => {
            mod $mod_name {
                use super::*;

                #[quickcheck]
                fn quickcheck(in_value: u128) {
                    let bits = $bits;
                    let in_value = (in_value % (1u128 << bits)) as usize;
                    let got = $func(in_value);
                    let expected = in_value.trailing_zeros();

                    assert_eq!(
                        expected, got,
                        "func({}) = {}, expected = {}",
                        in_value, got, expected,
                    );
                }

                #[test]
                fn continuous() {
                    let bits = $bits;
                    for i in 0..1024u128 {
                        let in_value = if bits < 10 {
                            if (i >> bits) != 0 {
                                break;
                            }
                            i
                        } else {
                            let low = i & 31;
                            let high = i >> 5;
                            low | (high << (bits - 5))
                        } as usize;

                        let got = $func(in_value);
                        let expected = in_value.trailing_zeros();

                        assert_eq!(
                            expected, got,
                            "func({}) = {}, expected = {}",
                            in_value, got, expected,
                        );
                    }
                }
            }
        };
    }

    gen_test!(trailing_zeros_0, super::trailing_zeros::<0>, 0);
    gen_test!(trailing_zeros_1, super::trailing_zeros::<1>, 1);
    gen_test!(trailing_zeros_2, super::trailing_zeros::<2>, 2);
    gen_test!(trailing_zeros_3, super::trailing_zeros::<3>, 3);
    gen_test!(
        trailing_zeros_max,
        super::trailing_zeros::<{ super::USIZE_BITS as usize }>,
        super::USIZE_BITS
    );
    gen_test!(ctz8_debruijn, super::ctz8_debruijn, 8);
    gen_test!(ctz4_lut, super::ctz4_lut, 4);
    gen_test!(ctz3_lut, super::ctz3_lut, 3);
    gen_test!(ctz2, super::ctz2, 2);
    gen_test!(ctz_array_lut_1, super::ctz_array_lut::<2>, 1);
    gen_test!(ctz_array_lut_2, super::ctz_array_lut::<4>, 2);
    gen_test!(ctz_array_lut_3, super::ctz_array_lut::<8>, 3);
    gen_test!(ctz_array_lut_4, super::ctz_array_lut::<16>, 4);
    gen_test!(ctz_array_lut_8, super::ctz_array_lut::<256>, 8);
    gen_test!(ctz_linear_0, super::ctz_linear::<0>, 0);
    gen_test!(ctz_linear_1, super::ctz_linear::<1>, 1);
    gen_test!(ctz_linear_2, super::ctz_linear::<2>, 2);
    gen_test!(ctz_linear_3, super::ctz_linear::<3>, 3);
    gen_test!(
        ctz_linear_max,
        super::ctz_linear::<{ super::USIZE_BITS as usize }>,
        super::USIZE_BITS
    );
    gen_test!(ctz_bsearch32_0, super::ctz_bsearch32::<0>, 0);
    gen_test!(ctz_bsearch32_1, super::ctz_bsearch32::<1>, 1);
    gen_test!(ctz_bsearch32_2, super::ctz_bsearch32::<2>, 2);
    gen_test!(ctz_bsearch32_3, super::ctz_bsearch32::<3>, 3);
    gen_test!(ctz_bsearch32_5, super::ctz_bsearch32::<5>, 5);
    gen_test!(ctz_bsearch32_10, super::ctz_bsearch32::<10>, 10);
    gen_test!(ctz_bsearch32_14, super::ctz_bsearch32::<14>, 14);
    gen_test!(ctz_bsearch32_21, super::ctz_bsearch32::<21>, 21);
    gen_test!(ctz_bsearch32_32, super::ctz_bsearch32::<32>, 32);
}
