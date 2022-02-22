//! `const fn`-compatible [`core::cell::RefCell`].
// FIXME: `RefCell::borrow`, etc. are not `const fn` yet, hence this stuff
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

/// A mutable memory location with dynamically checked borrow rules
pub struct RefCell<T: ?Sized> {
    // FIXME: `Cell` isn't `const fn`-compatible either
    borrow: UnsafeCell<BorrowFlag>,
    value: UnsafeCell<T>,
}

/// An error returned by [`RefCell::try_borrow`].
pub struct BorrowError;

/// An error returned by [`RefCell::try_borrow_mut`].
pub struct BorrowMutError;

// Positive values represent the number of `Ref` active. Negative values
// represent the number of `RefMut` active. Multiple `RefMut`s can only be
// active at a time if they refer to distinct, nonoverlapping components of a
// `RefCell` (e.g., different ranges of a slice).
type BorrowFlag = isize;
const UNUSED: BorrowFlag = 0;

impl<T> RefCell<T> {
    /// Creates a new `RefCell` containing `value`.
    pub const fn new(value: T) -> RefCell<T> {
        RefCell {
            value: UnsafeCell::new(value),
            borrow: UnsafeCell::new(UNUSED),
        }
    }

    /// Consumes the `RefCell`, returning the wrapped value.
    pub const fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> RefCell<T> {
    /// Immutably borrows the wrapped value.
    #[track_caller]
    pub const fn borrow(&self) -> Ref<'_, T> {
        // FIXME: `Result::expect` is not `const fn` yet
        if let Ok(x) = self.try_borrow() {
            x
        } else {
            panic!("already mutably borrowed")
        }
    }

    /// Immutably borrows the wrapped value, returning an error if the value is currently mutably
    /// borrowed.
    pub const fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        if unsafe { *self.borrow.get() } >= 0 {
            unsafe { *self.borrow.get() += 1 };
            Ok(Ref {
                // Safety: The borrow counting guarantees the absence of unique access.
                value: unsafe { &*self.value.get() },
                borrow: &self.borrow,
            })
        } else {
            Err(BorrowError)
        }
    }

    /// Mutably borrows the wrapped value.
    #[track_caller]
    pub const fn borrow_mut(&self) -> RefMut<'_, T> {
        // FIXME: `Result::expect` is not `const fn` yet
        if let Ok(x) = self.try_borrow_mut() {
            x
        } else {
            panic!("already borrowed")
        }
    }

    /// Mutably borrows the wrapped value, returning an error if the value is currently borrowed.
    pub const fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, BorrowMutError> {
        if unsafe { *self.borrow.get() } == 0 {
            unsafe { *self.borrow.get() = -1 };
            Ok(RefMut {
                // Safety: The borrow counting guarantees unique access.
                value: unsafe { &mut *self.value.get() },
                borrow: &self.borrow,
            })
        } else {
            Err(BorrowMutError)
        }
    }
}

pub struct Ref<'b, T: ?Sized> {
    value: &'b T,
    borrow: &'b UnsafeCell<BorrowFlag>,
}

pub struct RefMut<'b, T: ?Sized> {
    value: &'b mut T,
    borrow: &'b UnsafeCell<BorrowFlag>,
}

impl<T: ?Sized> const Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> const Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> const DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

impl<T: ?Sized> const Drop for Ref<'_, T> {
    fn drop(&mut self) {
        unsafe { *self.borrow.get() -= 1 };
    }
}

impl<T: ?Sized> const Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        unsafe { *self.borrow.get() += 1 };
    }
}
