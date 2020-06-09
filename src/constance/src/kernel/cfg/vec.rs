use crate::utils::Init;

/// `Vec` that can be used in a constant context.
pub struct ComptimeVec<T: Copy> {
    // FIXME: Waiting for <https://github.com/rust-lang/const-eval/issues/20>
    storage: [Option<T>; MAX_LEN],
    len: usize,
}

const MAX_LEN: usize = 256;

impl<T: Copy> ComptimeVec<T> {
    pub const fn new() -> Self {
        Self {
            storage: [None; MAX_LEN],
            len: 0,
        }
    }

    // FIXME: Waiting for <https://github.com/rust-lang/rust/issues/57349>
    pub const fn push(mut self, x: T) -> Self {
        self.storage[self.len] = Some(x);
        self.len += 1;
        self
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    // FIXME: Waiting for <https://github.com/rust-lang/rust/issues/67792>
    pub const fn get(&self, i: usize) -> T {
        if i >= self.len() {
            panic!("out of bounds");
        }

        // FIXME: Work-around for `[T]::get` not being `const fn`
        if let Some(x) = self.storage[i] {
            x
        } else {
            panic!("out of bounds")
        }
    }
}

impl<T: Copy + Init> ComptimeVec<T> {
    pub const fn to_array<const LEN: usize>(&self) -> [T; LEN] {
        let mut out = [T::INIT; LEN];

        // FIXME: Work-around for `assert_eq` being unsupported in `const fn`
        if self.len() != LEN {
            panic!("`self.len() != LEN`");
        }

        // Copy `self` to `out`
        // FIXME: Work-around for `[T]::copy_from_slice` not being `const fn`
        // FIXME: Work-around for `for` being unsupported in `const fn`
        // FIXME: Waiting for <https://github.com/rust-lang/rust/issues/67792>
        let mut i = 0;
        while i < LEN {
            out[i] = self.get(i);
            i += 1;
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        const _VEC: ComptimeVec<u32> = ComptimeVec::new();
    }

    #[test]
    fn push() {
        const VEC: ComptimeVec<u32> = {
            let mut v = ComptimeVec::new();
            v = v.push(42);
            v
        };

        const VEC_LEN: usize = VEC.len();
        assert_eq!(VEC_LEN, 1);

        const VEC_VAL: u32 = VEC.get(0);
        assert_eq!(VEC_VAL, 42);
    }

    #[test]
    fn to_array() {
        const ARRAY: [u32; 3] = {
            let mut v = ComptimeVec::new();
            v = v.push(1);
            v = v.push(2);
            v = v.push(3);
            v.to_array()
        };
        assert_eq!(ARRAY, [1, 2, 3]);
    }
}
