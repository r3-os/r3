//! The `const fn` implementation of checked integer-to-integer conversion.

// FIXME: This is a work-around for `TryFrom` being unavailable in `const fn`

#[inline]
pub const fn try_i32_into_u32(x: i32) -> Option<u32> {
    if x >= 0 {
        Some(x as u32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn quickcheck_try_i32_into_u32(x: i32) {
        assert_eq!(try_i32_into_u32(x), u32::try_from(x).ok());
    }

    #[test]
    fn test_try_i32_into_u32() {
        for &x in &[i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX] {
            assert_eq!(try_i32_into_u32(x), u32::try_from(x).ok());
        }
    }
}
