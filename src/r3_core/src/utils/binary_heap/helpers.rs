use core::{mem::ManuallyDrop, ptr};

/// Hole represents a hole in a slice i.e., an index without valid value
/// (because it was moved from or duplicated).
/// In drop, `Hole` will restore the slice by filling the hole
/// position with the value that was originally removed.
pub(super) struct Hole<'a, T: 'a> {
    data: &'a mut [T],
    elt: ManuallyDrop<T>,
    pos: usize,
}

impl<'a, T> Hole<'a, T> {
    /// Create a new `Hole` at index `pos`.
    ///
    /// # Safety
    ///
    /// Unsafe because pos must be within the data slice.
    #[inline]
    pub(super) const unsafe fn new(data: &'a mut [T], pos: usize) -> Self {
        debug_assert!(pos < data.len());
        // SAFE: pos should be inside the slice
        let elt = unsafe { ptr::read(data.get_unchecked2(pos)) };
        Hole {
            data,
            elt: ManuallyDrop::new(elt),
            pos,
        }
    }

    #[inline]
    pub(super) const fn pos(&self) -> usize {
        self.pos
    }

    /// Returns a reference to the element removed.
    #[inline]
    pub(super) const fn element(&self) -> &T {
        &self.elt
    }

    /// Returns a mutable reference to the element removed.
    #[inline]
    pub(super) const fn element_mut(&mut self) -> &mut T {
        &mut self.elt
    }

    /// Returns a reference to the element at `index`.
    ///
    /// Unsafe because index must be within the data slice and not equal to pos.
    #[inline]
    pub(super) const unsafe fn get(&self, index: usize) -> &T {
        debug_assert!(index != self.pos);
        debug_assert!(index < self.data.len());
        unsafe { self.data.get_unchecked2(index) }
    }

    /// Returns a mutable reference to the element at `index`.
    ///
    /// Unsafe because index must be within the data slice and not equal to pos.
    #[inline]
    pub(super) const unsafe fn get_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index != self.pos);
        debug_assert!(index < self.data.len());
        unsafe { self.data.get_unchecked_mut2(index) }
    }

    /// Move hole to new location
    ///
    /// Unsafe because index must be within the data slice and not equal to pos.
    #[inline]
    pub(super) const unsafe fn move_to(&mut self, index: usize) {
        debug_assert!(index != self.pos);
        debug_assert!(index < self.data.len());
        unsafe {
            let index_ptr: *const _ = self.data.get_unchecked2(index);
            let hole_ptr = self.data.get_unchecked_mut2(self.pos);
            ptr::copy_nonoverlapping(index_ptr, hole_ptr, 1);
        }
        self.pos = index;
    }
}

impl<T> const Drop for Hole<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // fill the hole again
        unsafe {
            let pos = self.pos;
            ptr::copy_nonoverlapping(&*self.elt, self.data.get_unchecked_mut2(pos), 1);
        }
    }
}

// FIXME: Work-around for `[T]::get_unchecked<usize>` not being `const fn` yet
trait GetUnchecked {
    type Element;
    unsafe fn get_unchecked2(&self, i: usize) -> &Self::Element;
}

impl<Element> const GetUnchecked for [Element] {
    type Element = Element;

    #[inline]
    unsafe fn get_unchecked2(&self, i: usize) -> &Self::Element {
        unsafe { &*self.as_ptr().add(i) }
    }
}

// FIXME: Work-around for `[T]::get_unchecked_mut<usize>` not being `const fn` yet
trait GetUncheckedMut {
    type Element;
    unsafe fn get_unchecked_mut2(&mut self, i: usize) -> &mut Self::Element;
}

impl<Element> const GetUncheckedMut for [Element] {
    type Element = Element;

    #[inline]
    unsafe fn get_unchecked_mut2(&mut self, i: usize) -> &mut Self::Element {
        unsafe { &mut *self.as_mut_ptr().add(i) }
    }
}
