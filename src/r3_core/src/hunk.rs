//! Type-safe hunks
use core::{
    fmt,
    marker::PhantomData,
    mem,
    ops::Deref,
    ptr::{slice_from_raw_parts, slice_from_raw_parts_mut},
};

use crate::{
    kernel::{self, cfg, hunk, raw, raw_cfg, Cfg, StartupHook},
    utils::{Init, ZeroInit},
};

/// The priority of the [startup hooks] used to initialize [typed hunks]. It has
/// a negative value so that startup hooks with non-negative priorities (which
/// can be created without `unsafe` blocks) will never see an uninitialized
/// value in a typed hunk.
///
/// [startup hooks]: crate::kernel::StartupHook
/// [typed hunks]: Hunk
pub const INIT_HOOK_PRIORITY: i32 = -0x7000_0000;

/// Represents a single typed hunk in a system.
///
/// Hunks are nothing more than static variables defined in a kernel
/// configuration. They come in handy when you are designing a component that
/// can be instantiated by a kernel configuration and wanting each instance to
/// have its own separate state data.
///
/// This type is implemented on top of [`r3::kernel::Hunk`], the untyped
/// hunk type.
///
/// [`r3::kernel::Hunk`]: crate::kernel::Hunk
#[doc = include_str!("./common.md")]
pub struct Hunk<System, T: ?Sized> {
    /// The offset of the hunk. `System::HUNK_ATTR.hunk_pool_ptr()` must be
    /// added before dereferencing.
    offset: *const T,
    _phantom: PhantomData<System>,
}

unsafe impl<System, T: ?Sized + Send> Send for Hunk<System, T> {}
unsafe impl<System, T: ?Sized + Sync> Sync for Hunk<System, T> {}

impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized> Hunk<System, T> {
    /// Construct a `HunkDefiner` to define a hunk in [a configuration
    /// function](crate#static-configuration).
    pub const fn define() -> HunkDefiner<System, T, DefaultInitTag> {
        HunkDefiner {
            _phantom: PhantomData,
            len: 1,
            align: 1,
        }
    }
}

/// As a generic parameter of [`HunkDefiner`], indicates that the [hunk]
/// should be initialized with [`Init`].
///
/// [`Init`]: crate::utils::Init
/// [hunk]: crate::kernel::Hunk
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct DefaultInitTag;

/// As a generic parameter of [`HunkDefiner`], indicates that the [hunk]
/// should be zero-initialized.
///
/// [hunk]: crate::kernel::Hunk
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct ZeroInitTag;

/// Implemented on [`DefaultInitTag`] and [`ZeroInitTag`] when `T` can be
/// initialized in this way.
pub trait HunkIniter<T> {
    /// A flag indicating whether [`Self::init`] should be called for
    /// initialization.
    const NEEDS_INIT: bool;

    /// Initialize the specified memory region.
    fn init(dest: &mut mem::MaybeUninit<T>);
}

impl<T: Init> HunkIniter<T> for DefaultInitTag {
    const NEEDS_INIT: bool = true;
    fn init(dest: &mut mem::MaybeUninit<T>) {
        *dest = mem::MaybeUninit::new(T::INIT);
    }
}

impl<T> HunkIniter<T> for ZeroInitTag {
    const NEEDS_INIT: bool = false;
    fn init(_: &mut mem::MaybeUninit<T>) {
        // Do nothing - a hunk pool is zero-initialized by default
        // [ref:hunk_pool_is_zeroed]
    }
}

/// The definer (static builder) for [`Hunk`].
///
/// `InitTag` is either [`DefaultInitTag`] or [`ZeroInitTag`].
#[must_use = "must call `finish()` to complete registration"]
pub struct HunkDefiner<System, T: ?Sized, InitTag> {
    _phantom: PhantomData<(System, InitTag, T)>,
    len: usize,
    align: usize,
}

impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized, InitTag>
    HunkDefiner<System, T, InitTag>
{
    /// Specify the element count. Defaults to `1`. Must be `1` for a non-array
    /// hunk.
    pub const fn len(self, len: usize) -> Self {
        Self { len, ..self }
    }

    /// Specify the minimum alignment. Defaults to `1`.
    pub const fn align(self, align: usize) -> Self {
        Self { align, ..self }
    }

    /// Zero-initialize the hunk.
    pub const fn zeroed(self) -> HunkDefiner<System, T, ZeroInitTag>
    where
        T: ZeroInit,
    {
        // Safety: `T: ZeroInit`, so it's zero-initializable
        unsafe { self.zeroed_unchecked() }
    }

    /// Zero-initialize the hunk even if it might be unsafe.
    ///
    /// # Safety
    ///
    /// If zero initialization is not a valid bit pattern for `T`, accessing the
    /// hunk's contents may result in an undefined behavior.
    pub const unsafe fn zeroed_unchecked(self) -> HunkDefiner<System, T, ZeroInitTag> {
        HunkDefiner {
            _phantom: PhantomData,
            len: self.len,
            align: self.align,
        }
    }
}

impl<System: raw::KernelBase + cfg::KernelStatic, T, InitTag: HunkIniter<T>>
    HunkDefiner<System, T, InitTag>
{
    /// Complete the definition of a hunk, returning a reference to the hunk.
    pub const fn finish<C: ~const raw_cfg::CfgBase<System = System>>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Hunk<System, T> {
        let untyped_hunk = kernel::Hunk::<System>::define()
            .len(mem::size_of::<T>())
            .align(max(mem::align_of::<T>(), self.align))
            .finish(cfg);

        assert!(self.len == 1, "Non-array hunk must have `len` of `1`");

        let start = untyped_hunk.offset();

        // Insert an initializer
        if InitTag::NEEDS_INIT {
            unsafe {
                StartupHook::define()
                    .priority(INIT_HOOK_PRIORITY)
                    .start((start, |start| {
                        let untyped_hunk = kernel::Hunk::<System>::from_offset(start).as_ptr();
                        // Safety: The destination is large enough to contain `T`
                        InitTag::init(&mut *(untyped_hunk as *mut mem::MaybeUninit<T>));
                    }))
                    .unchecked()
                    .finish(cfg);
            }
        }

        Hunk {
            offset: start as _,
            _phantom: PhantomData,
        }
    }
}

impl<System: raw::KernelBase + cfg::KernelStatic, T, InitTag: HunkIniter<T>>
    HunkDefiner<System, [T], InitTag>
{
    /// Complete the definition of a hunk, returning a reference to the hunk.
    pub const fn finish<C: ~const raw_cfg::CfgBase<System = System>>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Hunk<System, [T]> {
        assert!(self.align.is_power_of_two(), "`align` is not power of two");

        let untyped_hunk = kernel::Hunk::<System>::define()
            .len(mem::size_of::<T>() * self.len)
            .align(max(mem::align_of::<T>(), self.align))
            .finish(cfg);

        let start = untyped_hunk.offset();

        // Insert an initializer
        if InitTag::NEEDS_INIT {
            // TODO: There is no way to pass a length into the initializer
            todo!();
        }

        Hunk {
            offset: slice_from_raw_parts_mut(start as _, self.len),
            _phantom: PhantomData,
        }
    }
}

impl<System: raw::KernelBase + cfg::KernelStatic, T> Init for Hunk<System, [T]> {
    // Safety: This is safe because it points to nothing
    #[allow(clippy::invalid_null_ptr_usage)]
    const INIT: Self = Self {
        offset: slice_from_raw_parts_mut(core::ptr::null_mut(), 0),
        _phantom: PhantomData,
    };
}

impl<System: raw::KernelBase + cfg::KernelStatic, T: fmt::Debug + ?Sized> fmt::Debug
    for Hunk<System, T>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Hunk")
            .field(&Self::as_ptr(*self))
            .field(&&**self)
            .finish()
    }
}

impl<System, T: ?Sized> Clone for Hunk<System, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<System, T: ?Sized> Copy for Hunk<System, T> {}

impl<System, T: ?Sized> Hunk<System, T> {
    /// Reinterpret the hunk as another type.
    ///
    /// # Safety
    ///
    ///  - Similarly to [`core::mem::transmute`], this is **incredibly** unsafe.
    ///  - The byte offset must be valid for the destination type.
    ///
    pub const unsafe fn transmute<U>(self) -> Hunk<System, U> {
        Hunk {
            offset: self.offset.cast(),
            _phantom: PhantomData,
        }
    }

    /// Calculate the offset from the hunk.
    ///
    /// # Safety
    ///
    ///  - The resulting hunk may point to memory that the caller is not
    ///    supposed to access.
    ///
    pub const unsafe fn wrapping_offset(self, count: isize) -> Self
    where
        T: Sized,
    {
        Hunk {
            offset: self.offset.wrapping_offset(count),
            _phantom: PhantomData,
        }
    }
}

impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized> Hunk<System, T> {
    /// Get the untyped hunk.
    #[inline]
    pub fn untyped_hunk(this: Self) -> kernel::Hunk<System> {
        hunk::Hunk::from_offset(this.offset as *const u8 as usize)
    }

    // The following methods are not `const fn` on account of `const`s being
    // unable to refer to `static`s [ref:const_static_item_ref]

    /// Get a raw pointer to the hunk's contents.
    #[inline]
    pub fn as_ptr(this: Self) -> *const T {
        (Self::untyped_hunk(this).as_ptr() as *const u8).with_metadata_of(this.offset)
    }

    /// Get a raw pointer to the raw bytes of the hunk.
    #[inline]
    pub fn as_bytes_ptr(this: Self) -> *const [u8] {
        slice_from_raw_parts(Self::untyped_hunk(this).as_ptr(), mem::size_of_val(&*this))
    }

    /// Get a reference to the hunk's contents.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn as_ref<'a>(this: Self) -> &'a T
    where
        T: 'a,
    {
        unsafe { &*Self::as_ptr(this) }
    }

    /// Get a reference to the raw bytes of the hunk.
    ///
    /// # Safety
    ///
    /// The result might include uninitialized bytes and/or interior mutability,
    /// so it might be unsafe to access.
    #[inline]
    pub unsafe fn as_bytes(this: Self) -> &'static [u8] {
        // Safety: The caller is responsible for making sure interpreting the
        // contents as `[u8]` is safe
        unsafe { &*Self::as_bytes_ptr(this) }
    }
}

impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized> AsRef<T> for Hunk<System, T> {
    #[inline]
    fn as_ref(&self) -> &T {
        unsafe { &*Self::as_ptr(*self) }
    }
}

impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized> Deref for Hunk<System, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

// Safety: `Hunk::deref` provides a stable address
unsafe impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized> stable_deref_trait::StableDeref
    for Hunk<System, T>
{
}

// Safety: `Hunk::clone` preserves the address
unsafe impl<System: raw::KernelBase + cfg::KernelStatic, T: ?Sized>
    stable_deref_trait::CloneStableDeref for Hunk<System, T>
{
}

// `Ord::max` is not available in `const fn` [ref:int_const_ord]
const fn max(x: usize, y: usize) -> usize {
    if x > y {
        x
    } else {
        y
    }
}
