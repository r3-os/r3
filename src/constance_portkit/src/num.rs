//! Operations for integer and rational numbers, supporting `const fn`
use num_rational::Ratio;

/// Find the greatest common divisorf of two given numbers.
pub const fn gcd128(x: u128, y: u128) -> u128 {
    if y == 0 {
        x
    } else {
        gcd128(y, x % y)
    }
}

/// Reduce the given fraction.
pub const fn reduce_ratio128(r: Ratio<u128>) -> Ratio<u128> {
    let gcd = gcd128(*r.numer(), *r.denom());
    Ratio::new_raw(*r.numer() / gcd, *r.denom() / gcd)
}

/// Apply the floor function on the given fractional number.
pub const fn floor_ratio128(r: Ratio<u128>) -> u128 {
    *r.numer() / *r.denom()
}

/// Apply the ceiling function on the given fractional number.
pub const fn ceil_ratio128(r: Ratio<u128>) -> u128 {
    if *r.numer() % *r.denom() == 0 {
        *r.numer() / *r.denom()
    } else {
        *r.numer() / *r.denom() + 1
    }
}

/// Divide and round up the result.
#[inline]
pub const fn ceil_div128(x: u128, y: u128) -> u128 {
    (x + y - 1) / y
}

/// Get the minimum of two numbers.
pub const fn min128(x: u128, y: u128) -> u128 {
    if x < y {
        x
    } else {
        y
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use quickcheck_macros::quickcheck;

    #[test]
    fn test_gcd128() {
        for &(x, y) in &[(0, 0), (0, 1), (1, 0), (1, 1)] {
            assert_eq!(gcd128(x, y), num_integer::gcd(x, y));
        }
    }

    #[quickcheck]
    fn quickcheck_gcd128(x: u128, y: u128) {
        assert_eq!(gcd128(x, y), num_integer::gcd(x, y));
    }

    #[quickcheck]
    fn quickcheck_gcd128_large(x: u128, y: u128) {
        let (x, y) = (!x, !y);
        assert_eq!(gcd128(x, y), num_integer::gcd(x, y));
    }
}
