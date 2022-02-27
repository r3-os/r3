use core::mem::{ManuallyDrop, MaybeUninit};

use super::Init;

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
/// This function is a `const fn` version of the [unstable]
/// `MaybeUninit::uninit_array` method.
///
/// [unstable]: https://github.com/rust-lang/rust/pull/65580
// FIXME: Remove this function when `MaybeUninit::uninit_array` becomes `const fn`
// FIXME: It is now unstably, but let's keep this function not to require the
//        applications to add a `#![feature(...)]`
pub const fn uninit_array<T, const LEN: usize>() -> [MaybeUninit<T>; LEN] {
    [MaybeUninit::<T>::INIT; LEN]
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
            // `MaybeUninit::array_assume_init` is not `const fn`
            // [ref:const_array_assume_init]
            unsafe { transmute(array) }
        };
        assert_eq!(ARRAY1, [1, 2, 3]);
    }
}
