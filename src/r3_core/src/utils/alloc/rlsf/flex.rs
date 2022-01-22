//! An allocator with flexible backing stores
use core::{alloc::Layout, debug_assert, ptr::NonNull, unimplemented};

use super::{
    int::BinInteger,
    utils::{
        min_usize, nonnull_slice_end, nonnull_slice_from_raw_parts, nonnull_slice_len,
        nonnull_slice_start,
    },
    Init, Tlsf, ALIGN, GRANULARITY,
};

/// The trait for dynamic storage allocators that can back [`FlexTlsf`].
pub unsafe trait FlexSource {
    /// Allocate a memory block of the requested minimum size.
    ///
    /// Returns the address range of the allocated memory block.
    ///
    /// # Safety
    ///
    /// `min_size` must be a multiple of [`GRANULARITY`]. `min_size` must not
    /// be zero.
    #[inline]
    #[default_method_body_is_const]
    unsafe fn alloc(&mut self, min_size: usize) -> Option<NonNull<[u8]>> {
        let _ = min_size;
        None
    }

    /// Attempt to grow the specified allocation without moving it. Returns
    /// the final allocation size (which must be greater than or equal to
    /// `min_new_len`) on success.
    ///
    /// # Safety
    ///
    /// `ptr` must be an existing allocation made by this
    /// allocator. `min_new_len` must be greater than or equal to `ptr.len()`.
    #[inline]
    #[default_method_body_is_const]
    unsafe fn realloc_inplace_grow(
        &mut self,
        ptr: NonNull<[u8]>,
        min_new_len: usize,
    ) -> Option<usize> {
        let _ = (ptr, min_new_len);
        None
    }

    /// Deallocate a previously allocated memory block.
    ///
    /// # Safety
    ///
    /// `ptr` must denote an existing allocation made by this allocator.
    #[inline]
    unsafe fn dealloc(&mut self, ptr: NonNull<[u8]>) {
        let _ = ptr;
        unimplemented!("`supports_dealloc` returned `true`, but `dealloc` is not implemented");
    }

    /// Check if this allocator implements [`Self::dealloc`].
    ///
    /// If this method returns `false`, [`FlexTlsf`] will not call `dealloc` to
    /// release memory blocks. It also applies some optimizations.
    ///
    /// The returned value must be constant for a particular instance of `Self`.
    #[inline]
    #[default_method_body_is_const]
    fn supports_dealloc(&self) -> bool {
        false
    }

    /// Check if this allocator implements [`Self::realloc_inplace_grow`].
    ///
    /// If this method returns `false`, [`FlexTlsf`] will not call
    /// `realloc_inplace_grow` to attempt to grow memory blocks. It also applies
    /// some optimizations.
    ///
    /// The returned value must be constant for a particular instance of `Self`.
    #[inline]
    #[default_method_body_is_const]
    fn supports_realloc_inplace_grow(&self) -> bool {
        false
    }

    /// Returns `true` if this allocator is implemented by managing one
    /// contiguous region, which is grown every time `alloc` or
    /// `realloc_inplace_grow` is called.
    ///
    /// For example, in WebAssembly, there is usually only one continuous
    /// memory region available for data processing, and the only way to acquire
    /// more memory is to grow this region by executing `memory.grow`
    /// instructions. An implementation of `FlexSource` based on this system
    /// would use this instruction to implement both `alloc` and
    /// `realloc_inplace_grow` methods. Therefore, it's pointless for
    /// [`FlexTlsf`] to call `alloc` when `realloc_inplace_grow` fails. This
    /// method can be used to remove such redundant calls to `alloc`.
    ///
    /// The returned value must be constant for a particular instance of `Self`.
    #[inline]
    #[default_method_body_is_const]
    fn is_contiguous_growable(&self) -> bool {
        false
    }

    /// Get the minimum alignment of allocations made by this allocator.
    /// [`FlexTlsf`] may be less efficient if this method returns a value
    /// less than [`GRANULARITY`].
    ///
    /// The returned value must be constant for a particular instance of `Self`.
    #[inline]
    #[default_method_body_is_const]
    fn min_align(&self) -> usize {
        1
    }
}

trait FlexSourceExt: FlexSource {
    fn use_growable_pool(&self) -> bool;
}

impl<T: ~const FlexSource> const FlexSourceExt for T {
    #[inline]
    fn use_growable_pool(&self) -> bool {
        // `growable_pool` is used for deallocation and pool growth.
        // Let's not think about the wasted space caused when this method
        // returns `false`.
        self.supports_dealloc() || self.supports_realloc_inplace_grow()
    }
}

/// Wraps [`core::alloc::GlobalAlloc`] to implement the [`FlexSource`] trait.
///
/// Since this type does not implement [`FlexSource::realloc_inplace_grow`],
/// it is likely to end up with terribly fragmented memory pools.
#[derive(Default, Debug, Copy, Clone)]
pub struct GlobalAllocAsFlexSource<T, const ALIGN: usize>(pub T);

impl<T: core::alloc::GlobalAlloc, const ALIGN: usize> GlobalAllocAsFlexSource<T, ALIGN> {
    const ALIGN: usize = if ALIGN.is_power_of_two() {
        if ALIGN < GRANULARITY {
            GRANULARITY
        } else {
            ALIGN
        }
    } else {
        const_panic!("`ALIGN` is not power of two")
    };
}

impl<T: Init, const ALIGN: usize> Init for GlobalAllocAsFlexSource<T, ALIGN> {
    const INIT: Self = Self(Init::INIT);
}

unsafe impl<T: core::alloc::GlobalAlloc, const ALIGN: usize> FlexSource
    for GlobalAllocAsFlexSource<T, ALIGN>
{
    #[inline]
    unsafe fn alloc(&mut self, min_size: usize) -> Option<NonNull<[u8]>> {
        let layout = Layout::from_size_align(min_size, Self::ALIGN)
            .ok()?
            .pad_to_align();
        // Safety: The caller upholds that `min_size` is not zero
        let start = self.0.alloc(layout);
        let start = NonNull::new(start)?;
        Some(nonnull_slice_from_raw_parts(start, layout.size()))
    }

    #[inline]
    unsafe fn dealloc(&mut self, ptr: NonNull<[u8]>) {
        // Safety: This layout was previously used for allocation, during which
        //         the layout was checked for validity
        let layout = Layout::from_size_align_unchecked(nonnull_slice_len(ptr), Self::ALIGN);

        // Safety: `start` denotes an existing allocation with layout `layout`
        self.0.dealloc(ptr.as_ptr() as _, layout);
    }

    fn supports_dealloc(&self) -> bool {
        true
    }

    #[inline]
    fn min_align(&self) -> usize {
        Self::ALIGN
    }
}

/// A wrapper of [`Tlsf`] that automatically acquires fresh memory pools from
/// [`FlexSource`].
#[derive(Debug)]
#[must_use = "call `destroy` to drop it cleanly"]
pub struct FlexTlsf<Source, FLBitmap, SLBitmap, const FLLEN: usize, const SLLEN: usize> {
    /// The lastly created memory pool.
    growable_pool: Option<Pool>,
    source: Source,
    tlsf: Tlsf<'static, FLBitmap, SLBitmap, FLLEN, SLLEN>,
}

#[derive(Debug, Copy, Clone)]
struct Pool {
    /// The starting address of the memory allocation.
    alloc_start: NonNull<u8>,
    /// The length of the memory allocation.
    alloc_len: usize,
    /// The length of the memory pool created within the allocation.
    /// This might be slightly less than `alloc_len`.
    pool_len: usize,
}

// Safety: `Pool` is totally thread-safe
unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

/// Pool footer stored at the end of each pool. It's only used when
/// supports_dealloc() == true`.
///
/// The footer is stored in the sentinel block's unused space or any padding
/// present at the end of each pool. This is why `PoolFtr` can't be larger than
/// two `usize`s.
#[repr(C)]
#[derive(Copy, Clone)]
struct PoolFtr {
    /// The previous allocation. Forms a singly-linked list.
    prev_alloc: Option<NonNull<[u8]>>,
}

const _: () = if core::mem::size_of::<PoolFtr>() != GRANULARITY / 2 {
    const_panic!("bad `PoolFtr` size");
};

const _: () = assert!(core::mem::align_of::<PoolFtr>() <= ALIGN);

impl PoolFtr {
    /// Get a pointer to `PoolFtr` for a given allocation.
    #[inline]
    const fn get_for_alloc(alloc: NonNull<[u8]>, alloc_align: usize) -> *mut Self {
        let alloc_end = nonnull_slice_end(alloc);
        let ptr = alloc_end.wrapping_sub(core::mem::size_of::<Self>());

        // If `alloc_end` is not well-aligned, we need to adjust the location
        // of `PoolFtr`, but that's impossible in CTFE
        assert!(alloc_align >= ALIGN);

        ptr as _
    }
}

/// Initialization with a [`FlexSource`] provided by [`Default::default`]
impl<
        Source: FlexSource + ~const Default,
        FLBitmap: BinInteger,
        SLBitmap: BinInteger,
        const FLLEN: usize,
        const SLLEN: usize,
    > const Default for FlexTlsf<Source, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
    #[inline]
    fn default() -> Self {
        Self {
            source: Source::default(),
            tlsf: Tlsf::INIT,
            growable_pool: None,
        }
    }
}

/// Initialization with a [`FlexSource`] provided by [`Init::INIT`]
impl<
        Source: FlexSource + Init,
        FLBitmap: BinInteger,
        SLBitmap: BinInteger,
        const FLLEN: usize,
        const SLLEN: usize,
    > Init for FlexTlsf<Source, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
    // FIXME: Add `const fn new()` when `const fn`s with type bounds are stabilized
    /// An empty pool.
    const INIT: Self = Self {
        source: Source::INIT,
        tlsf: Tlsf::INIT,
        growable_pool: None,
    };
}

// FIXME: `~const` bounds can't appear on any `impl`s but `impl const Trait for
//        Ty` (This is why the `~const` bounds are applied on each method.)
impl<
        Source: FlexSource,
        FLBitmap: BinInteger,
        SLBitmap: BinInteger,
        const FLLEN: usize,
        const SLLEN: usize,
    > FlexTlsf<Source, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
    /// Construct a new `FlexTlsf` object.
    #[inline]
    pub const fn new(source: Source) -> Self {
        Self {
            source,
            tlsf: Tlsf::INIT,
            growable_pool: None,
        }
    }

    /// Borrow the contained `Source`.
    #[inline]
    pub const fn source_ref(&self) -> &Source {
        &self.source
    }

    /// Mutably borrow the contained `Source`.
    ///
    /// # Safety
    ///
    /// The caller must not replace the `Source` with another one or modify
    /// any existing allocations in the `Source`.
    #[inline]
    pub const unsafe fn source_mut_unchecked(&mut self) -> &mut Source {
        &mut self.source
    }

    /// Attempt to allocate a block of memory.
    ///
    /// Returns the starting address of the allocated memory block on success;
    /// `None` otherwise.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in constant time (assuming `Source`'s methods
    /// do so as well).
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub const fn allocate(&mut self, layout: Layout) -> Option<NonNull<u8>>
    where
        Source: ~const FlexSource,
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        if let Some(x) = self.tlsf.allocate(layout) {
            return Some(x);
        }

        const_try!(self.increase_pool_to_contain_allocation(layout));

        let result = self.tlsf.allocate(layout);

        if result.is_none() {
            // Not a hard error, but it's still unexpected because
            // `increase_pool_to_contain_allocation` was supposed to make this
            // allocation possible
            debug_assert!(
                false,
                "the allocation failed despite the effort by \
                `increase_pool_to_contain_allocation`"
            );
        }

        result
    }

    /// Increase the amount of memory pool to guarantee the success of the
    /// given allocation. Returns `Some(())` on success.
    #[inline]
    const fn increase_pool_to_contain_allocation(&mut self, layout: Layout) -> Option<()>
    where
        Source: ~const FlexSource,
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let use_growable_pool = self.source.use_growable_pool();

        // How many extra bytes we need to get from the source for the
        // allocation to success?
        let extra_bytes_well_aligned = const_try!(
            Tlsf::<'static, FLBitmap, SLBitmap, FLLEN, SLLEN>::pool_size_to_contain_allocation(
                layout,
            )
        );

        // The sentinel block + the block to store the allocation
        debug_assert!(extra_bytes_well_aligned >= GRANULARITY * 2);

        if let (Some(_), true) = (self.growable_pool, use_growable_pool) {
            // Growable pool is not supported in this version
        } // if let Some(growable_pool) = self.growable_pool

        // Create a brand new allocation. `source.min_align` indicates the
        // minimum alignment that the created allocation will satisfy.
        // `extra_bytes_well_aligned` is the pool size that can contain the
        // allocation *if* the pool was well-aligned. If `source.min_align` is
        // not well-aligned enough, we need to allocate extra bytes.
        let extra_bytes = if self.source.min_align() < GRANULARITY {
            //
            //                    wasted                             wasted
            //                     ╭┴╮                               ╭──┴──╮
            //                     ┌─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┬─┐
            //         Allocation: │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │ │
            //                     └─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┴─┘
            //                       ┌───────┬───────┬───────┬───────┐
            // Pool created on it:   │       │       │       │       │
            //                       └───────┴───────┴───────┴───────┘
            //                       ╰───┬───╯
            //                      GRANULARITY
            //
            const_try!(extra_bytes_well_aligned.checked_add(GRANULARITY))
        } else {
            extra_bytes_well_aligned
        };

        // Safety: `extra_bytes` is non-zero and aligned to `GRANULARITY` bytes
        let alloc = const_try!(unsafe { self.source.alloc(extra_bytes) });

        let is_well_aligned = self.source.min_align() >= super::GRANULARITY;

        // Safety: The passed memory block is what we acquired from
        //         `self.source`, so we have the ownership
        let pool_len = unsafe {
            if is_well_aligned {
                self.tlsf.insert_free_block_ptr_aligned(alloc)
            } else {
                panic!("this version of `rlsf` requires `min_align() >= GRANULARITY`");
            }
        };
        let pool_len = if let Some(pool_len) = pool_len {
            pool_len.get()
        } else {
            unsafe {
                debug_assert!(false, "`pool_size_to_contain_allocation` is an impostor");
                // Safety: It's unreachable
                core::hint::unreachable_unchecked()
            }
        };

        if self.source.supports_dealloc() {
            // Link the new memory pool's `PoolFtr::prev_alloc_end` to the
            // previous pool (`self.growable_pool`).
            let pool_ftr = PoolFtr::get_for_alloc(alloc, self.source.min_align());
            let prev_alloc = if let Some(p) = self.growable_pool {
                Some(nonnull_slice_from_raw_parts(p.alloc_start, p.alloc_len))
            } else {
                None
            };
            // Safety: `(*pool_ftr).prev_alloc` is within a pool footer
            //         we control
            unsafe { (*pool_ftr).prev_alloc = prev_alloc };
        }

        if use_growable_pool {
            self.growable_pool = Some(Pool {
                alloc_start: nonnull_slice_start(alloc),
                alloc_len: nonnull_slice_len(alloc),
                pool_len,
            });
        }

        Some(())
    }

    /// Deallocate a previously allocated memory block.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in constant time (assuming `Source`'s methods
    /// do so as well).
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `self`.
    ///  - The memory block must have been allocated with the same alignment
    ///    ([`Layout::align`]) as `align`.
    ///
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub const unsafe fn deallocate(&mut self, ptr: NonNull<u8>, align: usize)
    where
        Source: ~const FlexSource,
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        // Safety: Upheld by the caller
        self.tlsf.deallocate(ptr, align)
    }

    /// Deallocate a previously allocated memory block with an unknown alignment.
    ///
    /// Unlike `deallocate`, this function does not require knowing the
    /// allocation's alignment but might be less efficient.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in constant time (assuming `Source`'s methods
    /// do so as well).
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `self`.
    ///
    pub(crate) const unsafe fn deallocate_unknown_align(&mut self, ptr: NonNull<u8>)
    where
        Source: ~const FlexSource,
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        // Safety: Upheld by the caller
        self.tlsf.deallocate_unknown_align(ptr)
    }

    /// Shrink or grow a previously allocated memory block.
    ///
    /// Returns the new starting address of the memory block on success;
    /// `None` otherwise.
    ///
    /// # Time Complexity
    ///
    /// Unlike other methods, this method will complete in linear time
    /// (`O(old_size)`), assuming `Source`'s methods do so as well.
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `self`.
    ///  - The memory block must have been allocated with the same alignment
    ///    ([`Layout::align`]) as `new_layout`.
    ///
    pub const unsafe fn reallocate(
        &mut self,
        ptr: NonNull<u8>,
        new_layout: Layout,
    ) -> Option<NonNull<u8>>
    where
        Source: ~const FlexSource,
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        // Do this early so that the compiler can de-duplicate the evaluation of
        // `size_of_allocation`, which is done here as well as in
        // `Tlsf::reallocate`.
        let old_size = Tlsf::<'static, FLBitmap, SLBitmap, FLLEN, SLLEN>::size_of_allocation(
            ptr,
            new_layout.align(),
        );

        // Safety: Upheld by the caller
        if let Some(x) = self.tlsf.reallocate(ptr, new_layout) {
            return Some(x);
        }

        // Allocate a whole new memory block. The following code section looks
        // the same as the one in `Tlsf::reallocate`, but `self.allocation`
        // here refers to `FlexTlsf::allocate`, which inserts new meory pools
        // as necessary.
        let new_ptr = const_try!(self.allocate(new_layout));

        // Move the existing data into the new location
        core::ptr::copy_nonoverlapping(
            ptr.as_ptr(),
            new_ptr.as_ptr(),
            min_usize(old_size, new_layout.size()),
        );

        // Deallocate the old memory block.
        self.deallocate(ptr, new_layout.align());

        Some(new_ptr)
    }

    /// Get the payload size of the allocation with an unknown alignment. The
    /// returned size might be larger than the size specified at the allocation
    /// time.
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `Self`.
    ///
    #[inline]
    pub(crate) const unsafe fn size_of_allocation_unknown_align(ptr: NonNull<u8>) -> usize {
        // Safety: Upheld by the caller
        Tlsf::<'static, FLBitmap, SLBitmap, FLLEN, SLLEN>::size_of_allocation_unknown_align(ptr)
    }
}

// FIXME: There isn't a way to add `~const` to a type definition, so this
//        `destroy` cannot be `Drop::drop`
// FIXME: `~const` bounds can't appear on any `impl`s but
//        `impl const Trait for Ty`
impl<Source: FlexSource, FLBitmap, SLBitmap, const FLLEN: usize, const SLLEN: usize>
    FlexTlsf<Source, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
    /// Deallocate all memory blocks and destroy `self`.
    pub const fn destroy(mut self)
    where
        Source: ~const FlexSource + ~const Drop,
        FLBitmap: ~const Drop,
        SLBitmap: ~const Drop,
    {
        if self.source.supports_dealloc() {
            debug_assert!(self.source.use_growable_pool());

            // Deallocate all memory pools
            let align = self.source.min_align();
            let mut cur_alloc_or_none = if let Some(p) = self.growable_pool {
                Some(nonnull_slice_from_raw_parts(p.alloc_start, p.alloc_len))
            } else {
                None
            };

            while let Some(cur_alloc) = cur_alloc_or_none {
                // Safety: We control the referenced pool footer
                let cur_ftr = unsafe { *PoolFtr::get_for_alloc(cur_alloc, align) };

                // Safety: It's an allocation we allocated from `self.source`
                unsafe { self.source.dealloc(cur_alloc) };

                cur_alloc_or_none = cur_ftr.prev_alloc;
            }
        }
    }
}

#[cfg(test)]
mod tests;
