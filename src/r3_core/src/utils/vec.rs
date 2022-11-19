use core::{alloc::Layout, marker::Destruct, ops, ptr::NonNull};

use super::{AllocError, Allocator, ConstAllocator};

/// `Vec` that can only be used in a constant context.
#[doc(hidden)]
pub struct ComptimeVec<T: ~const Destruct> {
    ptr: NonNull<T>,
    len: usize,
    capacity: usize,
    allocator: ConstAllocator,
}

impl<T: ~const Clone + ~const Destruct> const Clone for ComptimeVec<T> {
    fn clone(&self) -> Self {
        // FIXME: Work-around for a mysterious error saying "the trait bound
        // `for<'r> fn(&'r T) -> T {<T as Clone>::clone}: ~const FnMut<(&T,)>`
        // is not satisfied" when it's simply written as `self.map(T::clone)`
        #[inline]
        const fn clone_shim<T: ~const Clone>(x: &T) -> T {
            x.clone()
        }
        self.map(clone_shim)
    }
}

impl<T: ~const Destruct> const Drop for ComptimeVec<T> {
    fn drop(&mut self) {
        if core::mem::needs_drop::<T>() {
            while self.pop().is_some() {}
        }

        // Safety: The referent is a valid heap allocation from `self.allocator`,
        // and `self` logically owns it
        unsafe {
            self.allocator
                .deallocate(self.ptr.cast(), layout_array::<T>(self.capacity));
        }
    }
}

impl<T: ~const Destruct> ComptimeVec<T> {
    pub const fn new_in(allocator: ConstAllocator) -> Self {
        Self::with_capacity_in(0, allocator)
    }

    pub const fn with_capacity_in(capacity: usize, allocator: ConstAllocator) -> Self {
        Self {
            ptr: unwrap_alloc_error(allocator.allocate(layout_array::<T>(capacity))).cast(),
            len: 0,
            capacity,
            allocator,
        }
    }

    pub const fn repeat_in(allocator: ConstAllocator, x: T, n: usize) -> Self
    where
        T: Copy,
    {
        let mut this = Self::with_capacity_in(n, allocator);
        while this.len() < n {
            this.push(x);
        }
        this
    }

    pub const fn allocator(&self) -> &ConstAllocator {
        &self.allocator
    }

    pub const fn push(&mut self, x: T) {
        unsafe {
            self.reserve(1);
            // Safety: `self.len` is in-bounds
            self.ptr.as_ptr().wrapping_add(self.len).write(x)
        }
        self.len += 1;
    }

    pub const fn pop(&mut self) -> Option<T> {
        unsafe {
            if let Some(i) = self.len.checked_sub(1) {
                self.len = i;
                // Safety: The `i`-th element was present, but since `len <= i`
                // now, we can remove it
                Some(self.ptr.as_ptr().wrapping_add(i).read())
            } else {
                None
            }
        }
    }

    const fn reserve(&mut self, additional: usize) {
        // There's already an enough room?
        if self.capacity - self.len >= additional {
            return;
        }

        let mut new_cap = self.capacity.checked_add(2).expect("capacity overflow");
        while new_cap - self.len < additional {
            new_cap = new_cap.checked_mul(2).expect("capacity overflow");
        }

        unsafe {
            self.ptr = unwrap_alloc_error(self.allocator.grow(
                self.ptr.cast(),
                layout_array::<T>(self.capacity),
                layout_array::<T>(new_cap),
            ))
            .cast();
            self.capacity = new_cap;
        }
    }

    /// Return a `ComptimeVec` of the same `len` as `self` with function `f`
    /// applied to each element in order.
    pub const fn map<F: ~const FnMut(&T) -> U + ~const Destruct, U: ~const Destruct>(
        &self,
        mut f: F,
    ) -> ComptimeVec<U> {
        let mut out = ComptimeVec::with_capacity_in(self.len, self.allocator.clone());
        let mut i = 0;
        while i < self.len() {
            out.push(f(&self[i]));
            i += 1;
        }
        out
    }

    /// Remove all elements.
    pub const fn clear(&mut self)
    where
        T: ~const Destruct,
    {
        if core::mem::needs_drop::<T>() {
            while self.pop().is_some() {}
        } else {
            self.len = 0;
        }
    }

    /// Borrow the storage as a slice.
    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Borrow the storage as a slice.
    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    pub const fn to_array<const LEN: usize>(&self) -> [T; LEN]
    where
        T: Copy,
    {
        // `assert!` is used here due to [ref:const_assert_eq]
        assert!(self.len() == LEN);

        // Safety: The memory layout of `[MaybeUninit<T>; LEN]` is identical to
        // `[T; LEN]`. We initialized all elements in `storage[0..LEN]`, so it's
        // safe to reinterpret that range as `[T; LEN]`.
        unsafe { *self.ptr.as_ptr().cast() }
    }
}

impl<T: ~const Destruct> const ops::Deref for ComptimeVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: ~const Destruct> const ops::DerefMut for ComptimeVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

// Slices aren't iterable in `const fn` [ref:const_slice_iter]
// Closures are unusable in `const fn` [ref:const_closures]
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

/// Unwrap `Result<T, AllocError>`.
const fn unwrap_alloc_error<T: ~const Destruct>(x: Result<T, AllocError>) -> T {
    match x {
        Ok(x) => x,
        Err(AllocError) => panic!("compile-time heap allocation failed"),
    }
}

/// Calculate the `Layout` for `[T; len]`.
const fn layout_array<T>(len: usize) -> Layout {
    let singular = Layout::new::<T>();
    let Some(size) = singular.size().checked_mul(len) else { panic!("size overflow") };
    let Ok(layout) = Layout::from_size_align(size, singular.align()) else { unreachable!() };
    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;

    #[test]
    fn as_slice() {
        #[allow(dead_code)] // [ref:unnamed_const_dead_code]
        const fn array(allocator: &ConstAllocator) {
            let mut x = ComptimeVec::new_in(allocator.clone());
            x.push(1);
            x.push(2);
            x.push(3);
            x.push(4);
            // `assert!` is used here due to [ref:const_assert_eq]
            // `matches!` is used here due to [ref:option_const_partial_eq]
            assert!(matches!(x.pop(), Some(4)));
            let slice = x.as_slice();
            // `assert!` is used here due to [ref:const_assert_eq]
            // `matches!` is used here due to [ref:slice_const_partial_eq]
            assert!(matches!(slice, [1, 2, 3]));
        }
        const _: () = ConstAllocator::with(array);
    }

    #[test]
    fn map() {
        const fn array(allocator: &ConstAllocator) -> [i32; 3] {
            let mut x = ComptimeVec::new_in(allocator.clone());
            x.push(1);
            x.push(2);
            x.push(3);
            // Closures don't implement `const Fn` [ref:const_closures]
            const fn transform(x: &i32) -> i32 {
                *x + 1
            }
            let y = x.map(transform);
            y.to_array()
        }
        const OUT: [i32; 3] = ConstAllocator::with(array);
        assert_eq!(OUT, [2, 3, 4]);
    }

    #[test]
    fn to_array() {
        const fn array(allocator: &ConstAllocator) -> [u32; 3] {
            let mut v = ComptimeVec::new_in(allocator.clone());
            v.push(1);
            v.push(2);
            v.push(3);
            v.to_array()
        }
        const OUT: [u32; 3] = ConstAllocator::with(array);
        assert_eq!(OUT, [1, 2, 3]);
    }

    #[test]
    fn get_mut() {
        const fn val(allocator: &ConstAllocator) -> u32 {
            let mut v = ComptimeVec::new_in(allocator.clone());
            v.push(1);
            v.push(2);
            v.push(3);
            v[1] += 2;
            v[1]
        }
        const OUT: u32 = ConstAllocator::with(val);
        assert_eq!(OUT, 4);
    }

    #[test]
    fn const_vec_position() {
        const fn pos(allocator: &ConstAllocator) -> [Option<usize>; 2] {
            let mut v = ComptimeVec::new_in(allocator.clone());
            v.push(42);
            v.push(43);
            v.push(44);
            [
                vec_position!(v, |i| *i == 43),
                vec_position!(v, |i| *i == 50),
            ]
        }
        const OUT: [Option<usize>; 2] = ConstAllocator::with(pos);
        assert_eq!(OUT, [Some(1), None]);
    }

    #[test]
    fn drop_on_clear() {
        #[allow(dead_code)] // [ref:unnamed_const_dead_code]
        const fn array(allocator: &ConstAllocator) {
            let mut x = ComptimeVec::new_in(allocator.clone());

            // If the destructor is not called for these `ConstAllocator`s,
            // `ConstAllocator::with(array)` will panic
            x.push(allocator.clone());
            x.push(allocator.clone());
            x.push(allocator.clone());
        }
        const _: () = ConstAllocator::with(array);
    }

    #[quickcheck]
    fn vec_position(values: Vec<u8>, expected_index: usize) -> TestResult {
        let needle = if values.is_empty() {
            42
        } else {
            values[expected_index % values.len()]
        };

        let got = vec_position!(values, |i| *i == needle);
        let expected = values.iter().position(|i| *i == needle);

        assert_eq!(got, expected);

        TestResult::passed()
    }
}
