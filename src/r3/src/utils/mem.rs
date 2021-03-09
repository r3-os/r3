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
pub const fn uninit_array<T, const LEN: usize>() -> [MaybeUninit<T>; LEN] {
    unsafe { transmute(()) }
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
            // FIXME: use <https://github.com/rust-lang/rust/issues/80908> when
            //        it becomes `const fn`
            unsafe { transmute(array) }
        };
        assert_eq!(ARRAY1, [1, 2, 3]);
    }
}
