//! Provides a type-safe wrapper of `AtomicPtr`.
#![allow(dead_code)]
use std::marker::PhantomData;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{
    fmt,
    hint::unreachable_unchecked,
    mem,
    ptr::{self, NonNull},
};

/// Types whose value can be converted into a non-zero pointer-sized value
/// and forth.
///
/// This trait is marked as `unsafe` because `from_raw` processes an
/// unvalidated pointer (which is supposed to be one returned by `into_raw`)
/// and the implementations must not panic.
pub unsafe trait PtrSized: Sized {
    /// Convert `Self` into a pointer.
    ///
    /// The returned pointer may be an invalid pointer (i.e. undereferenceable).
    fn into_raw(this: Self) -> NonNull<()>;

    /// Convert a pointer created by `into_raw` back to `Self`.
    unsafe fn from_raw(ptr: NonNull<()>) -> Self;
}

/// Types implementing `PtrSized` and having converted pointer values that can
/// be interpreted as safely-dereferenceable `*const Self::Target` .
///
/// This trait is marked as `unsafe` because it puts a restriction on the
/// implementation of `PtrSized`.
///
/// It's possible that some type can implement either of `TypedPtrSized` and
/// `TrivialPtrSized`, but not both of them. In such cases, prefer
/// `TypedPtrSized` because `TrivialPtrSized` can be implemented without a
/// knowledge about a specific type while `TypedPtrSized` can't.
pub unsafe trait TypedPtrSized: PtrSized {
    type Target;
}

/// Types implementing `PtrSized` with a trivial implementation (i.e.,
/// conversion is done by mere transmutation).
///
/// This trait is marked as `unsafe` because it puts a restriction on the
/// implementation of `PtrSized`.
pub unsafe trait TrivialPtrSized: PtrSized {}

/// The pointed value is safe to mutate.
///
/// Types with `TypedPtrSized` usually implement this. However, there are
/// various reasons not to implement this; for example, they should not if the
/// deferenced value represents an internal state and must not be mutated.
/// `Arc` does not implement this because there may be other references to the
/// dereferenced value.
pub unsafe trait MutPtrSized: TypedPtrSized {}

trait PtrSizedExt: PtrSized {
    fn option_into_raw(this: Option<Self>) -> *mut ();
    unsafe fn option_from_raw(ptr: *mut ()) -> Option<Self>;
}

impl<T: PtrSized> PtrSizedExt for T {
    fn option_into_raw(this: Option<Self>) -> *mut () {
        if let Some(x) = this {
            Self::into_raw(x).as_ptr()
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn option_from_raw(ptr: *mut ()) -> Option<Self> {
        NonNull::new(ptr).map(|x| unsafe { Self::from_raw(x) })
    }
}

unsafe impl<T> PtrSized for Box<T> {
    fn into_raw(this: Self) -> NonNull<()> {
        NonNull::from(Box::leak(this)).cast()
    }
    unsafe fn from_raw(ptr: NonNull<()>) -> Self {
        unsafe { Box::from_raw(ptr.as_ptr() as _) }
    }
}
unsafe impl<T> TypedPtrSized for Box<T> {
    type Target = T;
}
unsafe impl<T> MutPtrSized for Box<T> {}
unsafe impl<T> TrivialPtrSized for Box<T> {}

/// An atomic reference cell that allows assignment only once throughout its
/// lifetime.
#[derive(Default)]
pub struct SetOnceAtom<T: PtrSized> {
    ptr: AtomicPtr<()>,
    phantom: PhantomData<T>,
}

impl<T: PtrSized> SetOnceAtom<T> {
    /// Construct an empty `SetOnceAtom`.
    pub const fn empty() -> Self {
        Self {
            ptr: AtomicPtr::new(ptr::null_mut()),
            phantom: PhantomData,
        }
    }

    /// Construct a `SetOnceAtom`.
    pub fn new(x: Option<T>) -> Self {
        Self {
            ptr: AtomicPtr::new(T::option_into_raw(x) as *mut ()),
            phantom: PhantomData,
        }
    }

    /// Store a value if nothing is stored yet.
    ///
    /// Returns `Ok(())` if the operation was successful. Returns `Err(x)`
    /// if the cell was already occupied.
    pub fn store(&self, x: Option<T>) -> Result<(), Option<T>> {
        let new_ptr = T::option_into_raw(x);
        match self.ptr.compare_exchange(
            ptr::null_mut(),
            new_ptr as *mut _,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(unsafe { T::option_from_raw(new_ptr) }),
        }
    }

    /// Return the inner object, consuming `self`.
    pub fn into_inner(mut self) -> Option<T> {
        let ret = unsafe { T::option_from_raw(*self.ptr.get_mut()) };

        // Skip drop
        mem::forget(self);

        ret
    }

    /// Remove and return the inner object.
    pub fn take(&mut self) -> Option<T> {
        let ret = mem::replace(self.ptr.get_mut(), ptr::null_mut());
        unsafe { T::option_from_raw(ret) }
    }
}

impl<T: TrivialPtrSized> SetOnceAtom<T> {
    /// Get a reference to the inner object.
    pub fn get(&self) -> Option<&T> {
        if self.ptr.load(Ordering::Acquire).is_null() {
            None
        } else {
            Some(unsafe { &*((&self.ptr) as *const _ as *const T) })
        }
    }

    /// Get a mutable reference to the inner object.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.ptr.get_mut().is_null() {
            None
        } else {
            Some(unsafe { &mut *((&mut self.ptr) as *mut _ as *mut T) })
        }
    }

    /// Insert a value computed from `f` if the contained value is `None`.
    /// After that, return a reference to the inner object.
    ///
    /// Because multiple threads can compete to do this, by the time the call to
    /// `f` is complete, `self` may be already occupied. In this case, the
    /// computed value that couldn't be stored to `self` will be returned as the
    /// second value.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use atom2::SetOnceAtom;
    /// let cell = SetOnceAtom::empty();
    /// // `f` is evaluated because `cell` is empty at this point
    /// assert_eq!(
    ///     cell.get_or_racy_insert_with(|| Box::new(42u32)),
    ///     (&Box::new(42), None)
    /// );
    ///
    /// // `f` isn't evaluated because `cell` is occupied
    /// assert_eq!(
    ///     cell.get_or_racy_insert_with(|| unreachable!()),
    ///     (&Box::new(42), None)
    /// );
    ///
    /// // (It's hard to cause the second return value to be `Some` in a
    /// // reliable way.)
    /// ```
    pub fn get_or_racy_insert_with(&self, f: impl FnOnce() -> T) -> (&T, Option<T>) {
        if let Some(inner) = self.get() {
            (inner, None)
        } else {
            let value = f();
            // Let `Err(x) = self.store(y)`. `x` is a moved value of `y`.
            // Ergo, this is safe.
            let extra = self
                .store(Some(value))
                .err()
                .map(|x| x.unwrap_or_else(|| unsafe { unreachable_unchecked() }));
            (
                self.get()
                    .unwrap_or_else(|| unsafe { unreachable_unchecked() }),
                extra,
            )
        }
    }

    /// Insert a value computed from `f` if the contained value is `None`.
    /// After that, return a mutable reference to the inner object.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use atom2::SetOnceAtom;
    /// let mut cell = SetOnceAtom::empty();
    /// // `f` is evaluated because `cell` is empty at this point
    /// assert_eq!(cell.get_mut_or_insert_with(|| Box::new(42u32)), &mut Box::new(42));
    ///
    /// // `f` isn't evaluated because `cell` is occupied
    /// assert_eq!(cell.get_mut_or_insert_with(|| unreachable!()), &mut Box::new(42));
    /// ```
    pub fn get_mut_or_insert_with(&mut self, f: impl FnOnce() -> T) -> &mut T {
        if self.ptr.get_mut().is_null() {
            *self.ptr.get_mut() = T::into_raw(f()).as_ptr();
            // We've just filled the inner value, so this is safe.
            self.get_mut()
                .unwrap_or_else(|| unsafe { unreachable_unchecked() })
        } else {
            // This is safe by the safety requirement of `TrivialPtrSized`
            unsafe { &mut *((&mut self.ptr) as *mut _ as *mut T) }
        }
    }
}

impl<T: TypedPtrSized> SetOnceAtom<T> {
    /// Dereference the inner object.
    pub fn as_inner_ref(&self) -> Option<&T::Target> {
        let p = self.ptr.load(Ordering::Acquire) as *mut T::Target;
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p })
        }
    }
}

impl<T: TypedPtrSized + MutPtrSized> SetOnceAtom<T> {
    /// Mutably dereference the inner object.
    pub fn as_inner_mut(&mut self) -> Option<&mut T::Target> {
        let p = self.ptr.load(Ordering::Acquire) as *mut T::Target;
        if p.is_null() {
            None
        } else {
            Some(unsafe { &mut *p })
        }
    }
}

impl<T: TypedPtrSized> fmt::Debug for SetOnceAtom<T>
where
    T::Target: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("SetOnceAtom")
            .field(&self.as_inner_ref())
            .finish()
    }
}

impl<T: PtrSized> Drop for SetOnceAtom<T> {
    fn drop(&mut self) {
        unsafe {
            T::option_from_raw(*self.ptr.get_mut());
        }
    }
}
