use core::{mem::MaybeUninit, ops};

/// `Vec` that can be used in a constant context.
#[doc(hidden)]
pub struct ComptimeVec<T: Copy> {
    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/20>
    storage: [MaybeUninit<T>; MAX_LEN],
    len: usize,
}

const MAX_LEN: usize = 256;

impl<T: Copy> Copy for ComptimeVec<T> {}

impl<T: Copy> Clone for ComptimeVec<T> {
    fn clone(&self) -> Self {
        self.map(Clone::clone)
    }
}

impl<T: Copy> ComptimeVec<T> {
    pub const fn new() -> Self {
        Self {
            storage: [MaybeUninit::uninit(); MAX_LEN],
            len: 0,
        }
    }

    pub const fn push(&mut self, x: T) {
        self.storage[self.len] = MaybeUninit::new(x);
        self.len += 1;
    }

    /// Return a `ComptimeVec` of the same `len` as `self` with function `f`
    /// applied to each element in order.
    pub const fn map<F: ~const FnMut(&T) -> U + Copy, U: Copy>(&self, mut f: F) -> ComptimeVec<U> {
        let mut out = ComptimeVec::new();
        let mut i = 0;
        while i < self.len() {
            out.push(f(&self[i]));
            i += 1;
        }
        out
    }

    /// Borrow the storage as a slice.
    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        // FIXME: Slicing is not `const fn` yet
        let slice = core::ptr::slice_from_raw_parts(
            &self.storage as *const _ as *const MaybeUninit<T>,
            self.len,
        );

        // Safety: `self.storage[0..self.len]` is initialized
        unsafe { MaybeUninit::slice_assume_init_ref(&*slice) }
    }

    /// Borrow the storage as a slice.
    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        // FIXME: Slicing is not `const fn` yet
        let slice = core::ptr::slice_from_raw_parts_mut(
            &mut self.storage as *mut _ as *mut MaybeUninit<T>,
            self.len,
        );

        // Safety: `self.storage[0..self.len]` is initialized
        unsafe { MaybeUninit::slice_assume_init_mut(&mut *slice) }
    }

    pub const fn to_array<const LEN: usize>(&self) -> [T; LEN] {
        // FIXME: Work-around for `assert_eq!` being unsupported in `const fn`
        assert!(self.len() == LEN);

        // FIXME: use <https://github.com/rust-lang/rust/issues/80908> when
        //        it becomes `const fn`
        // Safety: This is equivalent to `transmute_copy(&self.storage)`. The
        // memory layout of `[MaybeUninit<T>; LEN]` is identical to `[T; LEN]`.
        // We initialized all elements in `storage[0..LEN]`, so it's safe to
        // reinterpret that range as `[T; LEN]`.
        unsafe { *(&self.storage as *const _ as *const [T; LEN]) }
    }
}

impl<T: Copy> const ops::Deref for ComptimeVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: Copy> const ops::DerefMut for ComptimeVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

// FIXME: Waiting for <https://github.com/rust-lang/rust/issues/67792>
// FIXME: Waiting for `Iterator` to be usable in `const fn`
// FIXME: Waiting for `FnMut` to be usable in `const fn`
/// An implementation of `$vec.iter().position(|$item| $predicate)` that is
/// compatible with a const context.
#[allow(unused_macros)]
macro_rules! vec_position {
    ($vec:expr, |$item:ident| $predicate:expr) => {{
        let mut i = 0;
        loop {
            if i >= $vec.len() {
                break None;
            }
            let $item = &$vec[i];
            if $predicate {
                break Some(i);
            }
            i += 1;
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;

    #[test]
    fn new() {
        const _VEC: ComptimeVec<u32> = ComptimeVec::new();
    }

    #[test]
    fn push() {
        const fn vec() -> ComptimeVec<u32> {
            // FIXME: Unable to do this inside a `const` item because of
            //        <https://github.com/rust-lang/rust/pull/72934>
            let mut v = ComptimeVec::new();
            v.push(42);
            v
        }
        const VEC: ComptimeVec<u32> = vec();

        const VEC_LEN: usize = VEC.len();
        assert_eq!(VEC_LEN, 1);

        const VEC_VAL: u32 = VEC[0];
        assert_eq!(VEC_VAL, 42);
    }

    #[test]
    fn as_slice() {
        const fn array() {
            let mut x = ComptimeVec::new();
            x.push(1);
            x.push(2);
            x.push(3);
            let slice = x.as_slice();
            // FIXME: `assert_matches!` is not usable in `const fn` yet
            assert!(matches!(slice, [1, 2, 3]));
        }
        array();
    }

    #[test]
    fn map() {
        const fn array() -> [i32; 3] {
            let mut x = ComptimeVec::new();
            x.push(1);
            x.push(2);
            x.push(3);
            // FIXME: Closures don't implement `~const Fn`?
            const fn transform(x: &i32) -> i32 {
                *x + 1
            }
            let y = x.map(transform);
            y.to_array()
        }
        assert_eq!(array(), [2, 3, 4]);
    }

    #[test]
    fn to_array() {
        const fn array() -> [u32; 3] {
            let mut v = ComptimeVec::new();
            v.push(1);
            v.push(2);
            v.push(3);
            v.to_array()
        }
        assert_eq!(array(), [1, 2, 3]);
    }

    #[test]
    fn get_mut() {
        const fn val() -> u32 {
            let mut v = ComptimeVec::new();
            v.push(1);
            v.push(2);
            v.push(3);
            v[1] += 2;
            v[1]
        }
        assert_eq!(val(), 4);
    }

    #[test]
    fn const_vec_position() {
        const fn pos() -> [Option<usize>; 2] {
            let mut v = ComptimeVec::new();
            v.push(42);
            v.push(43);
            v.push(44);
            [
                vec_position!(v, |i| *i == 43),
                vec_position!(v, |i| *i == 50),
            ]
        }
        assert_eq!(pos(), [Some(1), None]);
    }

    #[quickcheck]
    fn vec_position(values: Vec<u8>, expected_index: usize) -> TestResult {
        if values.len() > MAX_LEN {
            return TestResult::discard();
        }

        let needle = if values.is_empty() {
            42
        } else {
            values[expected_index % values.len()]
        };

        // Convert `values` into `ComptimeVec`
        let mut vec = ComptimeVec::new();
        for &e in values.iter() {
            vec.push(e);
        }

        let got = vec_position!(vec, |i| *i == needle);
        let expected = values.iter().position(|i| *i == needle);

        assert_eq!(got, expected);

        TestResult::passed()
    }
}