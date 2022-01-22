//! The TLSF allocator core
use core::{
    alloc::Layout,
    debug_assert,
    hint::unreachable_unchecked,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    num::NonZeroUsize,
    ptr::{addr_of, NonNull},
};

use super::{
    debug_assert_eq, debug_assert_ne,
    int::BinInteger,
    utils::{min_usize, nonnull_slice_len, option_nonnull_as_ptr},
};

#[doc = svgbobdoc::transform!(
/// The TLSF header (top-level) data structure.
///
/// # Data Structure Overview
///
/// <center>
/// ```svgbob
///   First level
///                                                                       FLLEN = 8
///                               ,-----+-----+-----+-----+-----+-----+-----+-----,
///         fl_bitmap: FLBitmap = |  0  |  0  |  0  |  1  |  0  |  0  |  0  |  0  |
///                               +-----+-----+-----+-----+-----+-----+-----+-----+
///                      min size | 2¹¹ | 2¹⁰ |  2⁹ |  2⁸ |  2⁷ |  2⁶ |  2⁵ |  2⁴ |
///                               '-----+-----+-----+--+--+-----+-----+-----+-----'
///                                                    |
/// ╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶|╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶
///   Second Level                                     |
///                                                    v                      SLLEN = 8
///                                  ,-----+-----+-----+-----+-----+-----+-----+-----,
///        "sl_bitmap[4]: SLBitmap"= |  0  |  0  |  1  |  0  |  0  |  0  |  0  |  0  |
///                                  +-----+-----+-----+-----+-----+-----+-----+-----+
///               min size 2⁸(1+n/8) |  7  |  6  |  5  |  4  |  3  |  2  |  1  |  0  |
///                                  +-----+-----+-----+-----+-----+-----+-----+-----+
///                       first_free |     |     |  O  |     |     |     |     |     |
///                                  '-----+-----+--|--+-----+-----+-----+-----+-----'
///                                                 |
///                                                 |  size = 416..448
///                                                 |
/// ╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶|╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶╶
///   Free blocks                                   |
///                                                 |
///             ,-----------------------------------'
///             | ,---+---+-------,    ,---+---+-------,    ,---+---+-------,
///             '-+>O | O-+-------+----+>O | O-+-------+----+>O |   |       |
///               +---+---'       |    +---+---'       |    +---+---'       |
///               |               |    |               |    |               |
///               |               |    |               |    |               |
///               '---------------'    '---------------'    '---------------'
///                   416 bytes            432 bytes            416 bytes
/// ```
/// </center>
///
/// # Properties
///
/// The allocation granularity ([`GRANULARITY`]) is `size_of::<usize>() * 4`
/// bytes, which is the minimum size of a free block.
///
/// The maximum block size is `(GRANULARITY << FLLEN) - GRANULARITY`.
///
)]
#[derive(Debug)]
pub struct Tlsf<'pool, FLBitmap, SLBitmap, const FLLEN: usize, const SLLEN: usize> {
    fl_bitmap: FLBitmap,
    /// `sl_bitmap[fl].get_bit(sl)` is set iff `first_free[fl][sl].is_some()`
    sl_bitmap: [SLBitmap; FLLEN],
    first_free: [[Option<NonNull<FreeBlockHdr>>; SLLEN]; FLLEN],
    _phantom: PhantomData<&'pool mut ()>,
}

// Safety: All memory block headers directly or indirectly referenced by a
//         particular instance of `Tlsf` are logically owned by that `Tlsf` and
//         have no interior mutability, so these are safe.
unsafe impl<FLBitmap, SLBitmap, const FLLEN: usize, const SLLEN: usize> Send
    for Tlsf<'_, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
}

unsafe impl<FLBitmap, SLBitmap, const FLLEN: usize, const SLLEN: usize> Sync
    for Tlsf<'_, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
}

/// The allocation granularity.
///
/// It is `size_of::<usize>() * 4` bytes, which is the minimum size of a TLSF
/// free block.
pub const GRANULARITY: usize = core::mem::size_of::<usize>() * 4;

const GRANULARITY_LOG2: u32 = GRANULARITY.trailing_zeros();

// FIXME: Use `usize::BITS` when it's stable
pub(crate) const USIZE_BITS: u32 = core::mem::size_of::<usize>() as u32 * 8;

/// The header of a memory block.
// The header is actually aligned at `size_of::<usize>() * 4`-byte boundaries
// but the alignment is set to a half value here not to introduce a padding at
// the end of this struct.
#[repr(C)]
#[cfg_attr(target_pointer_width = "16", repr(align(4)))]
#[cfg_attr(target_pointer_width = "32", repr(align(8)))]
#[cfg_attr(target_pointer_width = "64", repr(align(16)))]
#[derive(Debug)]
struct BlockHdr {
    /// The size of the whole memory block, including the header.
    ///
    ///  - `bit[0]` ([`SIZE_USED`]) indicates whether the block is a used memory
    ///    block or not.
    ///
    ///  - `bit[1]` ([`SIZE_LAST_IN_POOL`]) indicates whether the block is the
    ///    last one of the pool or not.
    ///
    ///  - `bit[GRANULARITY_LOG2..]` ([`SIZE_SIZE_MASK`]) represents the size.
    ///
    size: usize,
    prev_phys_block: Option<NonNull<BlockHdr>>,
}

/// The bit of [`BlockHdr::size`] indicating whether the block is a used memory
/// block or not.
const SIZE_USED: usize = 1;
/// The bit of [`BlockHdr::size`] indicating whether the block is a sentinel
/// (the last block in a memory pool) or not. If this bit is set, [`SIZE_USED`]
/// must be set, too (`SIZE_SENTINEL ⟹ SIZE_USED`).
const SIZE_SENTINEL: usize = 2;
/// The bits of [`BlockHdr::size`] indicating the block's size.
const SIZE_SIZE_MASK: usize = !((1 << GRANULARITY_LOG2) - 1);

impl BlockHdr {
    /// Get the next block.
    ///
    /// # Safety
    ///
    /// `self` must have a next block (it must not be the sentinel block in a
    /// pool).
    #[inline]
    const unsafe fn next_phys_block(&self) -> NonNull<BlockHdr> {
        debug_assert!(
            (self.size & SIZE_SENTINEL) == 0,
            "`self` must not be a sentinel"
        );

        // Safety: Since `self.size & SIZE_LAST_IN_POOL` is not lying, the
        //         next block should exist at a non-null location.
        NonNull::new_unchecked((self as *const _ as *mut u8).add(self.size & SIZE_SIZE_MASK)).cast()
    }
}

/// The header of a free memory block.
#[repr(C)]
#[cfg_attr(target_pointer_width = "16", repr(align(8)))]
#[cfg_attr(target_pointer_width = "32", repr(align(16)))]
#[cfg_attr(target_pointer_width = "64", repr(align(32)))]
#[derive(Debug)]
struct FreeBlockHdr {
    common: BlockHdr,
    next_free: Option<NonNull<FreeBlockHdr>>,
    prev_free: Option<NonNull<FreeBlockHdr>>,
}

/// The header of a used memory block. It's `GRANULARITY / 2` bytes long.
///
/// The payload immediately follows this header. However, if the alignment
/// requirement is greater than or equal to [`GRANULARITY`], an up to
/// `align - GRANULARITY / 2` bytes long padding will be inserted between them,
/// and the last part of the padding ([`UsedBlockPad`]) will encode where the
/// header is located.
#[repr(C)]
#[derive(Debug)]
struct UsedBlockHdr {
    common: BlockHdr,
}

/// In a used memory block with an alignment requirement larger than or equal to
/// `GRANULARITY`, the payload is preceded by this structure.
#[derive(Debug)]
#[repr(C)]
struct UsedBlockPad {
    block_hdr: NonNull<UsedBlockHdr>,
}

impl UsedBlockPad {
    #[inline]
    const fn get_for_allocation(ptr: NonNull<u8>) -> *mut Self {
        ptr.cast::<Self>().as_ptr().wrapping_sub(1)
    }
}

impl<FLBitmap: BinInteger, SLBitmap: BinInteger, const FLLEN: usize, const SLLEN: usize> const
    Default for Tlsf<'_, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
    fn default() -> Self {
        Self::INIT
    }
}

// FIXME: `~const` bounds can't appear on any `impl`s but `impl const Trait for
//        Ty` (This is why the `~const` bounds are applied on each method.)
impl<'pool, FLBitmap: BinInteger, SLBitmap: BinInteger, const FLLEN: usize, const SLLEN: usize>
    Tlsf<'pool, FLBitmap, SLBitmap, FLLEN, SLLEN>
{
    // FIXME: Add `const fn new()` when `const fn`s with type bounds are stabilized
    /// An empty pool.
    pub const INIT: Self = Self {
        fl_bitmap: FLBitmap::ZERO,
        sl_bitmap: [SLBitmap::ZERO; FLLEN],
        first_free: [[None; SLLEN]; FLLEN],
        _phantom: {
            let () = Self::VALID;
            PhantomData
        },
    };

    // For testing
    #[allow(dead_code)]
    const FLLEN: usize = FLLEN;
    #[allow(dead_code)]
    const SLLEN: usize = SLLEN;

    /// Evaluates successfully if the parameters are valid.
    const VALID: () = {
        if FLLEN == 0 {
            const_panic!("`FLLEN` must not be zero");
        }
        if SLLEN == 0 {
            const_panic!("`SLLEN` must not be zero");
        }
        if (FLBitmap::BITS as u128) < FLLEN as u128 {
            const_panic!("`FLBitmap` should contain at least `FLLEN` bits");
        }
        if (SLBitmap::BITS as u128) < SLLEN as u128 {
            const_panic!("`SLBitmap` should contain at least `SLLEN` bits");
        }
    };

    /// The maximum size of each memory pool region. This is constrained by
    /// the maximum block size of the segregated list to contain the initial
    /// free memory block.
    const MAX_POOL_SIZE: Option<usize> = {
        let shift = GRANULARITY_LOG2 + FLLEN as u32;
        if shift < USIZE_BITS {
            Some(1 << shift)
        } else {
            None
        }
    };

    /// `SLLEN.log2()`
    const SLI: u32 = if SLLEN.is_power_of_two() {
        SLLEN.trailing_zeros()
    } else {
        const_panic!("`SLLEN` is not power of two")
    };

    /// Find the free block list to store a free block of the specified size.
    #[inline]
    const fn map_floor(size: usize) -> Option<(usize, usize)> {
        debug_assert!(size >= GRANULARITY);
        debug_assert!(size % GRANULARITY == 0);
        let fl = USIZE_BITS - GRANULARITY_LOG2 - 1 - size.leading_zeros();

        // The shift amount can be negative, and rotation lets us handle both
        // cases without branching. Underflowed digits can be simply masked out
        // in `map_floor`.
        let sl = size.rotate_right((fl + GRANULARITY_LOG2).wrapping_sub(Self::SLI));

        // The most significant one of `size` should be now at `sl[SLI]`
        debug_assert!(((sl >> Self::SLI) & 1) == 1);

        // `fl` must be in a valid range
        if fl as usize >= FLLEN {
            return None;
        }

        Some((fl as usize, sl & (SLLEN - 1)))
    }

    /// Find the first free block list whose every item is at least as large
    /// as the specified size.
    #[inline]
    const fn map_ceil(size: usize) -> Option<(usize, usize)> {
        debug_assert!(size >= GRANULARITY);
        debug_assert!(size % GRANULARITY == 0);
        let mut fl = USIZE_BITS - GRANULARITY_LOG2 - 1 - size.leading_zeros();

        // The shift amount can be negative, and rotation lets us handle both
        // cases without branching.
        let mut sl = size.rotate_right((fl + GRANULARITY_LOG2).wrapping_sub(Self::SLI));

        // The most significant one of `size` should be now at `sl[SLI]`
        debug_assert!(((sl >> Self::SLI) & 1) == 1);

        // Underflowed digits appear in `sl[SLI + 1..USIZE-BITS]`. They should
        // be rounded up
        sl = (sl & (SLLEN - 1)) + (sl >= (1 << (Self::SLI + 1))) as usize;

        // if sl[SLI] { fl += 1; sl = 0; }
        fl += (sl >> Self::SLI) as u32;

        // `fl` must be in a valid range
        if fl as usize >= FLLEN {
            return None;
        }

        Some((fl as usize, sl & (SLLEN - 1)))
    }

    const MAX_MAP_CEIL_AND_UNMAP_INPUT: usize = {
        // The maximum value for which `map_ceil(x)` returns `(USIZE_BITS -
        // GRANULARITY_LOG2 - 1, _)`, assuming `FLLEN == ∞`
        let max1 = !(usize::MAX >> (Self::SLI + 1));

        // Now take into account the fact that `FLLEN` is not actually infinity
        if FLLEN as u32 - 1 < USIZE_BITS - GRANULARITY_LOG2 - 1 {
            max1 >> ((USIZE_BITS - GRANULARITY_LOG2 - 1) - (FLLEN as u32 - 1))
        } else {
            max1
        }
    };

    /// Find the first free block list whose every item is at least as large
    /// as the specified size and get the list's minimum size. Returns `None`
    /// if there isn't such a list, or the list's minimum size is not
    /// representable in `usize`.
    #[inline]
    const fn map_ceil_and_unmap(size: usize) -> Option<usize> {
        debug_assert!(size >= GRANULARITY);
        debug_assert!(size % GRANULARITY == 0);

        if size > Self::MAX_MAP_CEIL_AND_UNMAP_INPUT {
            return None;
        }

        let fl = USIZE_BITS - GRANULARITY_LOG2 - 1 - size.leading_zeros();

        let list_min_size = if GRANULARITY_LOG2 < Self::SLI && fl < Self::SLI - GRANULARITY_LOG2 {
            size
        } else {
            let shift = fl + GRANULARITY_LOG2 - Self::SLI;

            // round up
            (size + ((1 << shift) - 1)) & !((1 << shift) - 1)
        };

        Some(list_min_size)
    }

    /// Insert the specified free block to the corresponding free block list.
    ///
    /// Updates `FreeBlockHdr::{prev_free, next_free}`.
    ///
    /// # Safety
    ///
    ///  - `*block.as_ptr()` must be owned by `self`. (It does not have to be
    ///    initialized, however.)
    ///  - `size` must have a corresponding free list, which does not currently
    ///    contain `block`.
    ///
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    const unsafe fn link_free_block(&mut self, mut block: NonNull<FreeBlockHdr>, size: usize)
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let (fl, sl) = Self::map_floor(size).expect("could not map the size");
        let first_free = &mut self.first_free[fl][sl];
        let next_free = mem::replace(first_free, Some(block));
        block.as_mut().next_free = next_free;
        block.as_mut().prev_free = None;
        if let Some(mut next_free) = next_free {
            next_free.as_mut().prev_free = Some(block);
        }

        self.fl_bitmap.set_bit(fl as u32);
        self.sl_bitmap[fl].set_bit(sl as u32);
    }

    /// Remove the specified free block from the corresponding free block list.
    ///
    /// # Safety
    ///
    ///  - `size` must represent the specified free block's size.
    ///  - The free block must be currently included in a free block list.
    ///
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    const unsafe fn unlink_free_block(&mut self, mut block: NonNull<FreeBlockHdr>, size: usize)
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let next_free = block.as_mut().next_free;
        let prev_free = block.as_mut().prev_free;

        if let Some(mut next_free) = next_free {
            next_free.as_mut().prev_free = prev_free;
        }

        if let Some(mut prev_free) = prev_free {
            prev_free.as_mut().next_free = next_free;
        } else {
            let (fl, sl) = Self::map_floor(size).expect("could not map the size");
            let first_free = &mut self.first_free[fl][sl];

            // FIXME: `Option<NonNull<T>>: !const PartialEq`
            debug_assert_eq!(option_nonnull_as_ptr(*first_free), block.as_ptr());
            *first_free = next_free;

            if next_free.is_none() {
                // The free list is now empty - update the bitmap
                self.sl_bitmap[fl].clear_bit(sl as u32);
                if self.sl_bitmap[fl].is_zero() {
                    self.fl_bitmap.clear_bit(fl as u32);
                }
            }
        }
    }

    /// Create a new memory pool at the location specified by a slice pointer.
    ///
    /// Returns the actual number of bytes (counted from the beginning of
    /// `block`) used to create the memory pool. This value is necessary to
    /// calculate the start address to pass to [`Self::append_free_block_ptr`].
    ///
    /// This method does nothing and returns `None` if the given memory block is
    /// too small.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in linear time (`O(block.len())`) because
    /// it might need to divide the memory block to meet the maximum block size
    /// requirement (`(GRANULARITY << FLLEN) - GRANULARITY`).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rlsf::Tlsf;
    /// use std::{mem::MaybeUninit, ptr::NonNull};
    /// static mut POOL: MaybeUninit<[u8; 1024]> = MaybeUninit::uninit();
    /// let mut tlsf: Tlsf<u8, u8, 8, 8> = Tlsf::INIT;
    /// unsafe {
    ///     tlsf.insert_free_block_ptr(NonNull::new(POOL.as_mut_ptr()).unwrap());
    /// }
    /// ```
    ///
    /// # Safety
    ///
    /// The memory block will be considered owned by `self`. The memory block
    /// must outlive `self`.
    ///
    /// # Panics
    ///
    /// This method never panics.
    pub const unsafe fn insert_free_block_ptr(
        &mut self,
        block: NonNull<[u8]>,
    ) -> Option<NonZeroUsize>
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let len = nonnull_slice_len(block);

        // Round up the starting address
        let unaligned_start = block.as_ptr() as *mut u8 as usize;
        let start = unaligned_start.wrapping_add(GRANULARITY - 1) & !(GRANULARITY - 1);

        let len = if let Some(x) = len.checked_sub(start.wrapping_sub(unaligned_start)) {
            if x >= GRANULARITY * 2 {
                // Round down
                x & !(GRANULARITY - 1)
            } else {
                // The block is too small
                return None;
            }
        } else {
            // The block is too small
            return None;
        };

        // Safety: The slice being created here
        let pool_len = const_try!(self.insert_free_block_ptr_aligned(NonNull::new_unchecked(
            core::ptr::slice_from_raw_parts_mut(start as *mut u8, len),
        )));

        // Safety: The sum should not wrap around because it represents the size
        //         of a memory pool on memory
        Some(NonZeroUsize::new_unchecked(
            pool_len.get() + start.wrapping_sub(unaligned_start),
        ))
    }

    /// [`insert_free_block_ptr`] with a well-aligned slice passed by `block`.
    pub(crate) const unsafe fn insert_free_block_ptr_aligned(
        &mut self,
        block: NonNull<[u8]>,
    ) -> Option<NonZeroUsize>
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let start = block.as_ptr() as *mut u8 as usize;
        let mut size = nonnull_slice_len(block);

        let mut cursor = start;

        while size >= GRANULARITY * 2 {
            let chunk_size = if let Some(max_pool_size) = Self::MAX_POOL_SIZE {
                min_usize(size, max_pool_size)
            } else {
                size
            };

            debug_assert_eq!(chunk_size % GRANULARITY, 0);

            // The new free block
            // Safety: `cursor` is not zero.
            let mut block = NonNull::new_unchecked(cursor as *mut FreeBlockHdr);

            // Initialize the new free block
            block.as_mut().common = BlockHdr {
                size: chunk_size - GRANULARITY,
                prev_phys_block: None,
            };

            // Cap the end with a sentinel block (a permanently-used block)
            let mut sentinel_block = block
                .as_ref()
                .common
                .next_phys_block()
                .cast::<UsedBlockHdr>();

            sentinel_block.as_mut().common = BlockHdr {
                size: GRANULARITY | SIZE_USED | SIZE_SENTINEL,
                prev_phys_block: Some(block.cast()),
            };

            // Link the free block to the corresponding free list
            self.link_free_block(block, chunk_size - GRANULARITY);

            // `cursor` can reach `usize::MAX + 1`, but in such a case, this
            // iteration must be the last one
            debug_assert!(cursor.checked_add(chunk_size).is_some() || size == chunk_size);
            size -= chunk_size;
            cursor = cursor.wrapping_add(chunk_size);
        }

        NonZeroUsize::new(cursor.wrapping_sub(start))
    }

    /// Create a new memory pool at the location specified by a slice.
    ///
    /// This method does nothing if the given memory block is too small.
    ///
    /// (The return type is yet to be determined.)
    ///
    /// # Time Complexity
    ///
    /// See [`Self::insert_free_block_ptr`].
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use rlsf::Tlsf;
    /// use std::mem::MaybeUninit;
    /// let mut pool = [MaybeUninit::uninit(); 1024];
    /// let mut tlsf: Tlsf<u8, u8, 8, 8> = Tlsf::INIT;
    /// tlsf.insert_free_block(&mut pool);
    /// ```
    ///
    /// The insertred memory block must outlive `self`:
    ///
    /// ```rust,ignore
    /// use rlsf::Tlsf;
    /// use std::mem::MaybeUninit;
    /// let mut tlsf: Tlsf<u8, u8, 8, 8> = Tlsf::INIT;
    /// let mut pool = [MaybeUninit::uninit(); 1024];
    /// tlsf.insert_free_block(&mut pool);
    /// drop(pool); // dropping the memory block first is not allowed
    /// drop(tlsf);
    /// ```
    ///
    /// # Panics
    ///
    /// This method never panics.
    #[inline]
    pub const fn insert_free_block(&mut self, block: &'pool mut [MaybeUninit<u8>])
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        // Safety: `block` is a mutable reference, which guarantees the absence
        // of aliasing references. Being `'pool` means it will outlive `self`.
        unsafe { self.insert_free_block_ptr(NonNull::new(block as *mut [_] as _).unwrap()) };
    }

    /// Calculate the minimum size of a `GRANULARITY`-byte aligned memory pool
    /// (a well-aligned free memory block to be passed to
    /// [`Self::insert_free_block`]) that is guaranteed to be able to contain
    /// the specified allocation.
    ///
    /// Returns `None` if no amount of additional memory space can make the
    /// allocation containable.
    #[inline]
    pub(crate) const fn pool_size_to_contain_allocation(layout: Layout) -> Option<usize> {
        // The extra bytes consumed by the header and padding. See
        // `Tlsf::allocate` for details.
        let max_overhead =
            layout.align().saturating_sub(GRANULARITY / 2) + mem::size_of::<UsedBlockHdr>();

        // Which segregated list we would look if we were allocating this?
        // And what's the minimum size of a free block required for inclusion
        // in this list?
        let search_size = const_try!(layout.size().checked_add(max_overhead));
        let search_size = const_try!(search_size.checked_add(GRANULARITY - 1)) & !(GRANULARITY - 1);
        let list_min_size = const_try!(Self::map_ceil_and_unmap(search_size));

        // Add the sentinel block size
        list_min_size.checked_add(GRANULARITY)
    }

    /// Attempt to allocate a block of memory.
    ///
    /// Returns the starting address of the allocated memory block on success;
    /// `None` otherwise.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in constant time.
    pub const fn allocate(&mut self, layout: Layout) -> Option<NonNull<u8>>
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        unsafe {
            // The extra bytes consumed by the header and padding.
            //
            // After choosing a free block, we need to adjust the payload's location
            // to meet the alignment requirement. Every block is aligned to
            // `GRANULARITY` bytes. `size_of::<UsedBlockHdr>` is `GRANULARITY / 2`
            // bytes, so the address immediately following `UsedBlockHdr` is only
            // aligned to `GRANULARITY / 2` bytes. Consequently, we need to insert
            // a padding containing at most `max(align - GRANULARITY / 2, 0)` bytes.
            let max_overhead =
                layout.align().saturating_sub(GRANULARITY / 2) + mem::size_of::<UsedBlockHdr>();

            // Search for a suitable free block
            let search_size = const_try!(layout.size().checked_add(max_overhead));
            let search_size =
                const_try!(search_size.checked_add(GRANULARITY - 1)) & !(GRANULARITY - 1);
            let (fl, sl) =
                const_try!(self.search_suitable_free_block_list_for_allocation(search_size));

            // Get a free block: `block`
            let first_free = &mut self.first_free[fl][sl];
            let block = first_free.expect("bitmap outdated");
            let mut next_phys_block = block.as_ref().common.next_phys_block();
            let size_and_flags = block.as_ref().common.size;
            let size = size_and_flags /* size_and_flags & SIZE_SIZE_MASK */;
            debug_assert_eq!(size, size_and_flags & SIZE_SIZE_MASK);

            debug_assert!(size >= search_size);

            // Unlink the free block. We are not using `unlink_free_block` because
            // we already know `(fl, sl)` and that `block.prev_free` is `None`.
            *first_free = block.as_ref().next_free;
            if let Some(mut next_free) = *first_free {
                next_free.as_mut().prev_free = None;
            } else {
                // The free list is now empty - update the bitmap
                let sl_bitmap = &mut self.sl_bitmap[fl];
                sl_bitmap.clear_bit(sl as u32);
                if sl_bitmap.is_zero() {
                    self.fl_bitmap.clear_bit(fl as u32);
                }
            }

            // Decide the starting address of the payload
            let unaligned_ptr = block.as_ptr() as *mut u8 as usize + mem::size_of::<UsedBlockHdr>();
            let ptr = NonNull::new_unchecked(
                (unaligned_ptr.wrapping_add(layout.align() - 1) & !(layout.align() - 1)) as *mut u8,
            );

            if layout.align() < GRANULARITY {
                debug_assert_eq!(unaligned_ptr, ptr.as_ptr() as usize);
            } else {
                debug_assert_ne!(unaligned_ptr, ptr.as_ptr() as usize);
            }

            // Calculate the actual overhead and the final block size of the
            // used block being created here
            let overhead = ptr.as_ptr() as usize - block.as_ptr() as usize;
            debug_assert!(overhead <= max_overhead);

            let new_size = overhead + layout.size();
            let new_size = (new_size + GRANULARITY - 1) & !(GRANULARITY - 1);
            debug_assert!(new_size <= search_size);

            if new_size == size {
                // The allocation completely fills this free block.
                // Updating `next_phys_block.prev_phys_block` is unnecessary in this
                // case because it's still supposed to point to `block`.
            } else {
                // The allocation partially fills this free block. Create a new
                // free block header at `block + new_size..block + size`
                // of length (`new_free_block_size`).
                let mut new_free_block: NonNull<FreeBlockHdr> =
                    NonNull::new_unchecked(block.cast::<u8>().as_ptr().add(new_size)).cast();
                let new_free_block_size = size - new_size;

                // Update `next_phys_block.prev_phys_block` to point to this new
                // free block
                // Invariant: No two adjacent free blocks
                debug_assert!((next_phys_block.as_ref().size & SIZE_USED) != 0);
                next_phys_block.as_mut().prev_phys_block = Some(new_free_block.cast());

                // Create the new free block header
                new_free_block.as_mut().common = BlockHdr {
                    size: new_free_block_size,
                    prev_phys_block: Some(block.cast()),
                };
                self.link_free_block(new_free_block, new_free_block_size);
            }

            // Turn `block` into a used memory block and initialize the used block
            // header. `prev_phys_block` is already set.
            let mut block = block.cast::<UsedBlockHdr>();
            block.as_mut().common.size = new_size | SIZE_USED;

            // Place a `UsedBlockPad` (used by `used_block_hdr_for_allocation`)
            if layout.align() >= GRANULARITY {
                (*UsedBlockPad::get_for_allocation(ptr)).block_hdr = block;
            }

            Some(ptr)
        }
    }

    /// Search for a non-empty free block list for allocation.
    #[inline]
    const fn search_suitable_free_block_list_for_allocation(
        &self,
        min_size: usize,
    ) -> Option<(usize, usize)>
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let (mut fl, mut sl) = const_try!(Self::map_ceil(min_size));

        // Search in range `(fl, sl..SLLEN)`
        sl = self.sl_bitmap[fl].bit_scan_forward(sl as u32) as usize;
        if sl < SLLEN {
            debug_assert!(self.sl_bitmap[fl].get_bit(sl as u32));

            return Some((fl, sl));
        }

        // Search in range `(fl + 1.., ..)`
        fl = self.fl_bitmap.bit_scan_forward(fl as u32 + 1) as usize;
        if fl < FLLEN {
            debug_assert!(self.fl_bitmap.get_bit(fl as u32));

            sl = self.sl_bitmap[fl].trailing_zeros() as usize;
            if sl >= SLLEN {
                debug_assert!(false, "bitmap contradiction");
                unsafe { unreachable_unchecked() };
            }

            debug_assert!(self.sl_bitmap[fl].get_bit(sl as u32));
            Some((fl, sl))
        } else {
            None
        }
    }

    /// Find the `UsedBlockHdr` for an allocation (any `NonNull<u8>` returned by
    /// our allocation functions).
    ///
    /// # Safety
    ///
    ///  - `ptr` must point to an allocated memory block returned by
    ///      `Self::{allocate, reallocate}`.
    ///
    ///  - The memory block must have been allocated with the same alignment
    ///    ([`Layout::align`]) as `align`.
    ///
    #[inline]
    const unsafe fn used_block_hdr_for_allocation(
        ptr: NonNull<u8>,
        align: usize,
    ) -> NonNull<UsedBlockHdr> {
        if align >= GRANULARITY {
            // Read the header pointer
            (*UsedBlockPad::get_for_allocation(ptr)).block_hdr
        } else {
            NonNull::new_unchecked(ptr.as_ptr().sub(GRANULARITY / 2)).cast()
        }
    }

    /// Find the `UsedBlockHdr` for an allocation (any `NonNull<u8>` returned by
    /// our allocation functions) with an unknown alignment.
    ///
    /// Unlike `used_block_hdr_for_allocation`, this function does not require
    /// knowing the allocation's alignment but might be less efficient.
    ///
    /// # Safety
    ///
    ///  - `ptr` must point to an allocated memory block returned by
    ///      `Self::{allocate, reallocate}`.
    ///
    #[inline]
    const unsafe fn used_block_hdr_for_allocation_unknown_align(
        ptr: NonNull<u8>,
    ) -> NonNull<UsedBlockHdr> {
        // Case 1: `align >= GRANULARITY`
        let c1_block_hdr_ptr: *const NonNull<UsedBlockHdr> =
            addr_of!((*UsedBlockPad::get_for_allocation(ptr)).block_hdr);
        // Case 2: `align < GRANULARITY`
        let c2_block_hdr = ptr.cast::<UsedBlockHdr>().as_ptr().wrapping_sub(1);
        let c2_prev_phys_block_ptr: *const Option<NonNull<BlockHdr>> =
            addr_of!((*c2_block_hdr).common.prev_phys_block);

        // They are both present at the same location, so we can be assured that
        // their contents are initialized and we can read them safely without
        // knowing which case applies first.
        debug_assert_eq!(
            c1_block_hdr_ptr as *const usize,
            c2_prev_phys_block_ptr as *const usize
        );

        // Read it as `Option<NonNull<BlockHdr>>`.
        if let Some(block_ptr) = *c2_prev_phys_block_ptr {
            // Where does the block represented by `block_ptr` end?
            // (Note: `block_ptr.size` might include `SIZE_USED`.)
            let block_end = block_ptr.as_ptr() as usize + block_ptr.as_ref().size;

            if ptr.as_ptr() as usize > block_end {
                // The block represented by `block_ptr` does not include `ptr`.
                // It's Case 2.
                NonNull::new_unchecked(c2_block_hdr)
            } else {
                // `ptr` is inside the block - it's Case 1.
                // (Note: `ptr == block_end` should count as being inside
                // because the payload might be zero-sized.)
                *c1_block_hdr_ptr
            }
        } else {
            // It's non-nullable in Case 1, so we can rule out Case 1.
            NonNull::new_unchecked(c2_block_hdr)
        }
    }

    /// Deallocate a previously allocated memory block.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in constant time.
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `self`.
    ///  - The memory block must have been allocated with the same alignment
    ///    ([`Layout::align`]) as `align`.
    ///
    pub const unsafe fn deallocate(&mut self, ptr: NonNull<u8>, align: usize)
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        // Safety: `ptr` is a previously allocated memory block with the same
        //         alignment as `align`. This is upheld by the caller.
        let block = Self::used_block_hdr_for_allocation(ptr, align).cast::<BlockHdr>();
        self.deallocate_block(block);
    }

    /// Deallocate a previously allocated memory block with an unknown alignment.
    ///
    /// Unlike `deallocate`, this function does not require knowing the
    /// allocation's alignment but might be less efficient.
    ///
    /// # Time Complexity
    ///
    /// This method will complete in constant time.
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `self`.
    ///
    pub(crate) const unsafe fn deallocate_unknown_align(&mut self, ptr: NonNull<u8>)
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        // Safety: `ptr` is a previously allocated memory block. This is upheld
        //         by the caller.
        let block = Self::used_block_hdr_for_allocation_unknown_align(ptr).cast::<BlockHdr>();
        self.deallocate_block(block);
    }

    /// Deallocate a previously allocated memory block. Takes a pointer to
    /// `BlockHdr` instead of a payload pointer.
    #[inline]
    const unsafe fn deallocate_block(&mut self, mut block: NonNull<BlockHdr>)
    where
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let mut size = block.as_ref().size & !SIZE_USED;
        debug_assert!((block.as_ref().size & SIZE_USED) != 0);

        // This variable tracks whose `prev_phys_block` we should update.
        let mut new_next_phys_block;

        // Merge the created hole with the next block if the next block is a
        // free block
        // Safety: `block.common` should be fully up-to-date and valid
        let next_phys_block = block.as_ref().next_phys_block();
        let next_phys_block_size_and_flags = next_phys_block.as_ref().size;
        if (next_phys_block_size_and_flags & SIZE_USED) == 0 {
            let next_phys_block_size = next_phys_block_size_and_flags;
            debug_assert_eq!(
                next_phys_block_size_and_flags & SIZE_SIZE_MASK,
                next_phys_block_size
            );

            // It's coalescable. Add its size to `size`. This will transfer
            // any `SIZE_LAST_IN_POOL` flag `next_phys_block` may have at
            // the same time.
            size += next_phys_block_size;

            // Safety: `next_phys_block` is a free block and therefore is not a
            // sentinel block
            new_next_phys_block = next_phys_block.as_ref().next_phys_block();

            // Unlink `next_phys_block`.
            self.unlink_free_block(next_phys_block.cast(), next_phys_block_size);
        } else {
            new_next_phys_block = next_phys_block;
        }

        // Merge with the previous block if it's a free block.
        if let Some(prev_phys_block) = block.as_ref().prev_phys_block {
            let prev_phys_block_size_and_flags = prev_phys_block.as_ref().size;

            if (prev_phys_block_size_and_flags & SIZE_USED) == 0 {
                let prev_phys_block_size = prev_phys_block_size_and_flags;
                debug_assert_eq!(
                    prev_phys_block_size_and_flags & SIZE_SIZE_MASK,
                    prev_phys_block_size
                );

                // It's coalescable. Add its size to `size`.
                size += prev_phys_block_size;

                // Unlink `prev_phys_block`.
                self.unlink_free_block(prev_phys_block.cast(), prev_phys_block_size);

                // Move `block` to where `prev_phys_block` is located. By doing
                // this, `block` will implicitly inherit `prev_phys_block.
                // as_ref().prev_phys_block`.
                block = prev_phys_block;
            }
        }

        // Write the new free block's size and flags.
        debug_assert!((size & SIZE_USED) == 0);
        block.as_mut().size = size;

        // Link this free block to the corresponding free list
        let block = block.cast::<FreeBlockHdr>();
        self.link_free_block(block, size);

        // Link `new_next_phys_block.prev_phys_block` to `block`
        // FIXME: `NonNull<T>: const PartialEq` isn't provided yet
        debug_assert_eq!(
            new_next_phys_block.as_ptr(),
            block.as_ref().common.next_phys_block().as_ptr()
        );
        new_next_phys_block.as_mut().prev_phys_block = Some(block.cast());
    }

    /// Get the payload size of the allocation. The returned size might be
    /// larger than the size specified at the allocation time.
    ///
    /// # Safety
    ///
    ///  - `ptr` must denote a memory block previously allocated via `Self`.
    ///  - The memory block must have been allocated with the same alignment
    ///    ([`Layout::align`]) as `align`.
    ///
    #[inline]
    pub(crate) const unsafe fn size_of_allocation(ptr: NonNull<u8>, align: usize) -> usize {
        // Safety: `ptr` is a previously allocated memory block with the same
        //         alignment as `align`. This is upheld by the caller.
        let block = Self::used_block_hdr_for_allocation(ptr, align);

        let size = block.as_ref().common.size - SIZE_USED;
        debug_assert_eq!(size, block.as_ref().common.size & SIZE_SIZE_MASK);

        let block_end = block.as_ptr() as usize + size;
        let payload_start = ptr.as_ptr() as usize;
        block_end - payload_start
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
        // Safety: `ptr` is a previously allocated memory block.
        //         This is upheld by the caller.
        let block = Self::used_block_hdr_for_allocation_unknown_align(ptr);

        let size = block.as_ref().common.size - SIZE_USED;
        debug_assert_eq!(size, block.as_ref().common.size & SIZE_SIZE_MASK);

        let block_end = block.as_ptr() as usize + size;
        let payload_start = ptr.as_ptr() as usize;
        block_end - payload_start
    }

    // TODO: `reallocate_no_move` (constant-time reallocation)

    /// Shrink or grow a previously allocated memory block.
    ///
    /// Returns the new starting address of the memory block on success;
    /// `None` otherwise.
    ///
    /// # Time Complexity
    ///
    /// Unlike other methods, this method will complete in linear time
    /// (`O(old_size)`).
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
        FLBitmap: ~const BinInteger,
        SLBitmap: ~const BinInteger,
    {
        let old_size = Self::size_of_allocation(ptr, new_layout.align());

        // Allocate a whole new memory block
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
}

#[cfg(test)]
mod tests;
