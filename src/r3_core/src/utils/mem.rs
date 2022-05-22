use core::mem::{ManuallyDrop, MaybeUninit};

union Xmute<T, U> {
    t: ManuallyDrop<T>,
    u: ManuallyDrop<U>,
}

/// Similar to `core::mem::transmute` except that `T` and `U` are not required
/// to be the same size.
///
/// # Safety
///
/// See `core::mem::transmute`.
pub const unsafe fn transmute<T, U>(x: T) -> U {
    unsafe {
        ManuallyDrop::into_inner(
            Xmute {
                t: ManuallyDrop::new(x),
            }
            .u,
        )
    }
}

/// Construct a `[MaybeUninit<T>; LEN]` whose elements are uninitialized.
///
/// This function exposes the unstable `MaybeUninit::uninit_array` method.
/// `[ref:const_uninit_array]`
#[inline]
pub const fn uninit_array<T, const LEN: usize>() -> [MaybeUninit<T>; LEN] {
    MaybeUninit::uninit_array()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::MaybeUninit;

    #[test]
    fn uninit_array() {
        const ARRAY1: [u32; 3] = {
            let array = [
                MaybeUninit::new(1u32),
                MaybeUninit::new(2),
                MaybeUninit::new(3),
            ];
            unsafe { MaybeUninit::array_assume_init(array) }
        };
        assert_eq!(ARRAY1, [1, 2, 3]);
    }
}
