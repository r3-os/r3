//! Count trailing zeros
#![allow(clippy::if_same_then_else)]
use super::int::BinInteger;

const USIZE_BITS: u32 = usize::BITS;

const HAS_CTZ: bool = if cfg!(target_arch = "riscv32") || cfg!(target_arch = "riscv64") {
    cfg!(target_feature = "b") || cfg!(target_feature = "experimental-b")
} else if cfg!(target_arch = "arm") {
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
const HAS_SHIFTER: bool = if cfg!(target_arch = "msp430") {
    false
} else if cfg!(target_arch = "avr") {
    false
} else {
    true
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
    } else if BITS <= 2 {
        ctz2(x)
    } else if BITS <= 3 && HAS_SHIFTER {
        ctz3_lut(x)
    } else if BITS <= 4 && HAS_SHIFTER {
        ctz4_lut(x)
    } else if BITS <= 8 && HAS_MUL && HAS_SHIFTER {
        ctz8_debruijn(x)
    } else if (cfg!(target_arch = "riscv32") || cfg!(target_arch = "riscv64")) && !HAS_MUL {
        ctz_bsearch11::<BITS>(x)
    } else if BITS <= 8 || !(HAS_MUL && HAS_SHIFTER) {
        ctz_linear::<BITS>(x)
    } else {
        // Fall back to LLVM expansion if we don't have an applicable
        // specialized routine. At the point of writing, this uses either
        // library code or a generic algorithm based on the following one:
        // <http://graphics.stanford.edu/~seander/bithacks.html#CountBitsSetParallel>
        //
        // This algorithm requires loading multiple 32-bit-wide constants and
        // shifts by more than one bit and may be very inefficient on some
        // targets.
        x.trailing_zeros()
    }
}

#[inline]
fn first_set_bit_mask(x: usize) -> usize {
    x & 0usize.wrapping_sub(x)
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
        //  2  3  4  5  6  7  8  9 10 11 12 13 14 15
        (0b01_00_10_00_01_00_11_00_01_00_10_00_01_00 << (x as u32 * 2)) >> 30
    }
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
fn ctz3_lut_nonzero(x: usize) -> u32 {
    debug_assert!(x < 8);
    debug_assert!(x != 0);

    //  2  3  4  5  6  7
    (0b01_00_10_00_01_00_0000000000000000 << (x as u32 * 2)) >> 30

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

/// Implements [`trailing_zeros`] using hierarchical (binary, etc.) search, only
/// using 11-bit-wide constants (this is beneficial to RISC-V). The last level
/// is handled by [`ctz3_lut_nonzero`].
///
///`BITS` must be less than or equal to 33.
#[inline]
fn ctz_bsearch11<const BITS: usize>(mut x: usize) -> u32 {
    const I11: usize = 0b11111111111;
    const I6: usize = 0b111111;
    const I3: usize = 0b111;

    debug_assert!(BITS <= 33);

    if x == 0 {
        return USIZE_BITS;
    }

    let mut i = 0;

    if BITS > 22 && (x & I11) == 0 {
        x >>= 11;
        i += 11;
    }
    if BITS > 11 && (x & I11) == 0 {
        x >>= 11;
        i += 11;
    }

    if BITS > 6 && (x & I6) == 0 {
        x >>= 6;
        i += 6;
    }

    if BITS > 3 && (x & I3) == 0 {
        x >>= 3;
        i += 3;
        if BITS > 6 {
            x &= I3;
        }
    } else if BITS > 3 {
        x &= I3;
    }

    i += ctz3_lut_nonzero(x);

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
    gen_test!(ctz_linear_0, super::ctz_linear::<0>, 0);
    gen_test!(ctz_linear_1, super::ctz_linear::<1>, 1);
    gen_test!(ctz_linear_2, super::ctz_linear::<2>, 2);
    gen_test!(ctz_linear_3, super::ctz_linear::<3>, 3);
    gen_test!(
        ctz_linear_max,
        super::ctz_linear::<{ super::USIZE_BITS as usize }>,
        super::USIZE_BITS
    );
    gen_test!(ctz_bsearch11_0, super::ctz_bsearch11::<0>, 0);
    gen_test!(ctz_bsearch11_1, super::ctz_bsearch11::<1>, 1);
    gen_test!(ctz_bsearch11_2, super::ctz_bsearch11::<2>, 2);
    gen_test!(ctz_bsearch11_3, super::ctz_bsearch11::<3>, 3);
    gen_test!(ctz_bsearch11_5, super::ctz_bsearch11::<5>, 5);
    gen_test!(ctz_bsearch11_10, super::ctz_bsearch11::<10>, 10);
    gen_test!(ctz_bsearch11_14, super::ctz_bsearch11::<14>, 14);
    gen_test!(ctz_bsearch11_21, super::ctz_bsearch11::<21>, 21);
    gen_test!(ctz_bsearch11_32, super::ctz_bsearch11::<32>, 32);
}
