//! The allocator
use core::{alloc::Layout, ptr, ptr::NonNull};

macro_rules! const_try_result {
    ($x:expr) => {
        match $x {
            Ok(x) => x,
            Err(x) => return Err(x),
        }
    };
}

/// Compile-time allocator.
///
/// This is implemented on top of [`core::intrinsics::const_allocate`][].
///
/// # Stability
///
/// This type is subject to the kernel-side API stability guarantee.
pub struct ConstAllocator {
    // We very much want to put these in `core::cell::*`, but they aren't very
    // useful in `const fn`, unfortunately.
    /// The number of the following objects pertaining to `self`:
    ///
    ///  - Clones of `ConstAllocator` from which new allocations can be created.
    ///  - Live allocations created through `ConstAllocator as Allocator`.
    ///
    ref_count: *mut usize,
}

impl ConstAllocator {
    /// Call the specified closure, passing a reference to a `Self` constructed
    /// on the stack.
    ///
    /// Does not work at runtime.
    ///
    /// All clones of `Self` and all allocations must be destroyed before the
    /// closure returns. This is because leaking const-allocated
    /// (interior-)mutable references to runtime code is unsound. See
    /// <https://github.com/rust-lang/rust/pull/91884#discussion_r774659436>.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(const_eval_limit)]
    /// #![feature(const_trait_impl)]
    /// #![feature(const_mut_refs)]
    /// #![feature(const_option)]
    /// #![feature(let_else)]
    /// #![const_eval_limit = "500000"]
    /// use core::{alloc::Layout, ptr::NonNull};
    /// use r3_core::utils::{ConstAllocator, Allocator};
    /// const _: () = ConstAllocator::with(doit);
    /// const fn doit(al: &ConstAllocator) {
    ///     // You can clone `*al`, but you must destroy the clone before this
    ///     // function returns
    ///     let al = al.clone();
    ///
    ///     unsafe {
    ///         let mut blocks = [None; 256];
    ///         let mut i = 0;
    ///         while i < blocks.len() {
    ///             // Allocate a memory block
    ///             let Ok(layout) = Layout::from_size_align(i * 64, 8) else { unreachable!() };
    ///             let Ok(alloc) = al.allocate(layout) else { unreachable!() };
    ///
    ///             // Write something
    ///             let alloc = alloc.cast::<u8>();
    ///             if i > 0 { *alloc.as_ptr() = i as u8; }
    ///
    ///             // Remember the allocation
    ///             blocks[i] = Some((alloc, layout));
    ///
    ///             i += 1;
    ///         }
    ///
    ///         i = 1;
    ///         while i < blocks.len() {
    ///             // Check the value inside the allocation
    ///             let (ptr, _) = blocks[i].unwrap();
    ///             assert!(*ptr.as_ptr() == i as u8);
    ///             i += 1;
    ///         }
    ///
    ///         // You must deallocate all allocations before this
    ///         // function returns
    ///         i = 0;
    ///         while i < blocks.len() {
    ///             let (ptr, layout) = blocks[i].unwrap();
    ///             al.deallocate(ptr, layout);
    ///             i += 1;
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// It's an error to leak allocations:
    ///
    /// ```rust,compile_fail,E0080
    /// # #![feature(const_trait_impl)]
    /// # #![feature(let_else)]
    /// # use core::alloc::Layout;
    /// # use r3_core::utils::{ConstAllocator, Allocator};
    /// # const _: () = ConstAllocator::with(doit);
    /// const fn doit(al: &ConstAllocator) {
    ///     let Ok(layout) = Layout::from_size_align(64, 8) else { unreachable!() };
    ///     let _ = al.allocate(layout);
    /// }
    /// ```
    ///
    /// ```rust,compile_fail,E0080
    /// # #![feature(const_trait_impl)]
    /// # use r3_core::utils::{ConstAllocator, Allocator};
    /// # const _: () = ConstAllocator::with(doit);
    /// const fn doit(al: &ConstAllocator) {
    ///     core::mem::forget(al.clone());
    /// }
    /// ```
    #[inline]
    pub const fn with<F, R>(f: F) -> R
    where
        F: ~const FnOnce(&ConstAllocator) -> R,
    {
        Self::with_inner(f)
    }

    /// The variant of [`Self::with`] that lets you pass an additional parameter
    /// to the closure.
    ///
    /// This can be used to work around the lack of compiler support for const
    /// closures.
    #[inline]
    pub const fn with_parametric<P, F, R>(p: P, f: F) -> R
    where
        F: ~const FnOnce(P, &ConstAllocator) -> R,
    {
        Self::with_inner((p, f))
    }

    const fn with_inner<F: ~const FnOnceConstAllocator>(f: F) -> F::Output {
        struct RefCountGuard(usize);
        impl const Drop for RefCountGuard {
            fn drop(&mut self) {
                if self.0 != 0 {
                    panic!(
                        "there are outstanding allocations or \
                        allocator references"
                    );
                }
            }
        }

        let mut ref_count = RefCountGuard(1);
        let ref_count = (&mut ref_count.0) as *mut _;

        let this = Self { ref_count };

        f.call(&this)
    }
}

/// The trait for types accepted by [`ConstAllocator::with_inner`].
trait FnOnceConstAllocator {
    type Output;
    fn call(self, allocator: &ConstAllocator) -> Self::Output;
}

/// This implementation's `call` method simply calls the `FnOnce` receiver.
impl<T: ~const FnOnce(&ConstAllocator) -> Output, Output> const FnOnceConstAllocator for T {
    type Output = Output;
    fn call(self, allocator: &ConstAllocator) -> Self::Output {
        self(allocator)
    }
}

/// This implementation's `call` method calls the `FnOnce` receiver with an
/// associated parameter value.
impl<P, T: ~const FnOnce(P, &ConstAllocator) -> Output, Output> const FnOnceConstAllocator
    for (P, T)
{
    type Output = Output;
    fn call(self, allocator: &ConstAllocator) -> Self::Output {
        let mut this = core::mem::MaybeUninit::new(self);
        // Safety: It's initialized
        let this = unsafe { &mut *this.as_mut_ptr() };
        // Safety: `md.0` and `md.1` are in `MaybeUninit`, so this will not
        // cause double free
        let param = unsafe { core::ptr::read(&this.0) };
        let func = unsafe { core::ptr::read(&this.1) };
        func(param, allocator)

        // FIXME: The following implementation doesn't work because of
        //        <https://github.com/rust-lang/rust/issues/86897>
        // (self.1)(self.0, allocator)
    }
}

impl const Clone for ConstAllocator {
    fn clone(&self) -> Self {
        unsafe { *self.ref_count += 1 };
        Self {
            ref_count: self.ref_count,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        *self = source.clone();
    }
}

impl const Drop for ConstAllocator {
    fn drop(&mut self) {
        unsafe { *self.ref_count -= 1 };
    }
}

/// The `AllocError` error indicates an allocation failure
/// that may be due to resource exhaustion or to
/// something wrong when combining the given input arguments with this
/// allocator.
///
/// # Stability
///
/// This trait is subject to the kernel-side API stability guarantee.
#[derive(Clone, Copy)]
pub struct AllocError;

/// `const fn`-compatible [`core::alloc::Allocator`].
///
/// # Safety
///
/// See [`core::alloc::Allocator`]'s documentation.
///
/// # Stability
///
/// This trait is subject to the kernel-side API stability guarantee.
pub unsafe trait Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;

    #[default_method_body_is_const]
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = const_try_result!(self.allocate(layout));
        // SAFETY: `alloc` returns a valid memory block
        unsafe { ptr.as_ptr().cast::<u8>().write_bytes(0, ptr.len()) }
        Ok(ptr)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);

    #[default_method_body_is_const]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        let new_ptr = const_try_result!(self.allocate(new_layout));

        // SAFETY: because `new_layout.size()` must be greater than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
        // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // safe. The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr().cast(), old_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    #[default_method_body_is_const]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        let new_ptr = const_try_result!(self.allocate_zeroed(new_layout));

        // SAFETY: because `new_layout.size()` must be greater than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // writes for `old_layout.size()` bytes. Also, because the old allocation wasn't yet
        // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // safe. The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr().cast(), old_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    #[default_method_body_is_const]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        let new_ptr = const_try_result!(self.allocate(new_layout));

        // SAFETY: because `new_layout.size()` must be lower than or equal to
        // `old_layout.size()`, both the old and new memory allocation are valid for reads and
        // writes for `new_layout.size()` bytes. Also, because the old allocation wasn't yet
        // deallocated, it cannot overlap `new_ptr`. Thus, the call to `copy_nonoverlapping` is
        // safe. The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr().cast(), new_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    #[default_method_body_is_const]
    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

unsafe impl<A> const Allocator for &A
where
    A: ~const Allocator + ?Sized,
{
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (**self).allocate(layout)
    }

    #[inline]
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (**self).allocate_zeroed(layout)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (**self).deallocate(ptr, layout) }
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (**self).grow(ptr, old_layout, new_layout) }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (**self).grow_zeroed(ptr, old_layout, new_layout) }
    }

    #[inline]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        unsafe { (**self).shrink(ptr, old_layout, new_layout) }
    }
}

unsafe impl const Allocator for ConstAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = unsafe { core::intrinsics::const_allocate(layout.size(), layout.align()) };
        if let Some(ptr) = NonNull::new(ptr) {
            unsafe { *self.ref_count += 1 };
            Ok(NonNull::slice_from_raw_parts(ptr, layout.size()))
        } else {
            Err(AllocError)
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe { core::intrinsics::const_deallocate(ptr.as_ptr(), layout.size(), layout.align()) };
        unsafe { *self.ref_count -= 1 };
    }
}
