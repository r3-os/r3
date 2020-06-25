use core::mem::MaybeUninit;

/// `Vec` that can be used in a constant context.
#[doc(hidden)]
pub struct ComptimeVec<T: Copy> {
    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/20>
    storage: [MaybeUninit<T>; MAX_LEN],
    len: usize,
}

const MAX_LEN: usize = 256;

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

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    // FIXME: Waiting for <https://github.com/rust-lang/rust/issues/67792>
    pub const fn get(&self, i: usize) -> &T {
        if i >= self.len() {
            panic!("out of bounds");
        }

        // Safety: `self.storage[0..self.len]` is initialized, and `i < self.len`
        // FIXME: Waiting for `MaybeUninit::as_ptr` to be stabilized
        unsafe { &*(&self.storage[i] as *const _ as *const T) }
    }

    // FIXME: Waiting for <https://github.com/rust-lang/rust/issues/67792>
    pub const fn get_mut(&mut self, i: usize) -> &mut T {
        if i >= self.len() {
            panic!("out of bounds");
        }

        // Safety: `self.storage[0..self.len]` is initialized, and `i < self.len`
        // FIXME: Waiting for `MaybeUninit::as_ptr` to be stabilized
        unsafe { &mut *(&mut self.storage[i] as *mut _ as *mut T) }
    }
}

impl<T: Copy> ComptimeVec<T> {
    pub const fn to_array<const LEN: usize>(&self) -> [T; LEN] {
        // FIXME: Work-around for `assert_eq` being unsupported in `const fn`
        if self.len() != LEN {
            panic!("`self.len() != LEN`");
        }

        // Safety: This is equivalent to `transmute_copy(&self.storage)`. The
        // memory layout of `[MaybeUninit<T>; LEN]` is identical to `[T; LEN]`.
        // We initialized all elements in `storage[0..LEN]`, so it's safe to
        // reinterpret that range as `[T; LEN]`.
        unsafe { *(&self.storage as *const _ as *const [T; LEN]) }
    }
}

// FIXME: Waiting for <https://github.com/rust-lang/rust/issues/67792>
// FIXME: Waiting for `Iterator` to be usable in `const fn`
// FIXME: Waiting for `FnMut` to be usable in `const fn`
/// An implementation of `$vec.iter().position(|$item| $predicate)` that is
/// compatible with a const context.
macro_rules! vec_position {
    ($vec:expr, |$item:ident| $predicate:expr) => {{
        let mut i = 0;
        loop {
            if i >= $vec.len() {
                break None;
            }
            let $item = $vec.get(i);
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

        const VEC_VAL: u32 = *VEC.get(0);
        assert_eq!(VEC_VAL, 42);
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
            *v.get_mut(1) += 2;
            *v.get(1)
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
