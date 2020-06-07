//! Utility
use core::{cell::UnsafeCell, sync::atomic};

mod zeroinit;
pub use self::zeroinit::*;

/// Trait for types having a constant default value. This is essentially a
/// constant version of `Default`.
pub trait Init {
    /// The default value.
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;
}

impl<T: 'static> Init for &'_ [T] {
    const INIT: Self = &[];
}

impl Init for &'_ str {
    const INIT: Self = "";
}

impl<T> Init for Option<T> {
    const INIT: Self = None;
}

impl<T> Init for atomic::AtomicPtr<T> {
    const INIT: Self = atomic::AtomicPtr::new(core::ptr::null_mut());
}

impl<T: Init> Init for UnsafeCell<T> {
    const INIT: Self = UnsafeCell::new(T::INIT);
}

impl<T: Init> Init for RawCell<T> {
    const INIT: Self = RawCell::new(T::INIT);
}

impl<T: Init, I: Init> Init for tokenlock::TokenLock<T, I> {
    const INIT: Self = Self::new(I::INIT, T::INIT);
}

macro_rules! impl_init {
    (
        $($ty:ty => $value:expr,)*
    ) => {
        $(
            impl Init for $ty {
                const INIT: Self = $value;
            }
        )*
    };
}

impl_init! {
    bool => false,
    char => '\0',
    u8 => 0,
    u16 => 0,
    u32 => 0,
    u64 => 0,
    u128 => 0,
    i8 => 0,
    i16 => 0,
    i32 => 0,
    i64 => 0,
    i128 => 0,
    usize => 0,
    isize => 0,
    f32 => 0.0,
    f64 => 0.0,
    atomic::AtomicU8 => atomic::AtomicU8::new(0),
    atomic::AtomicU16=> atomic::AtomicU16::new(0),
    atomic::AtomicU32 => atomic::AtomicU32::new(0),
    atomic::AtomicU64 => atomic::AtomicU64::new(0),
    atomic::AtomicUsize => atomic::AtomicUsize::new(0),
    atomic::AtomicI8 => atomic::AtomicI8::new(0),
    atomic::AtomicI16 => atomic::AtomicI16::new(0),
    atomic::AtomicI32 => atomic::AtomicI32::new(0),
    atomic::AtomicI64 => atomic::AtomicI64::new(0),
    atomic::AtomicIsize => atomic::AtomicIsize::new(0),
    () => (),
}

macro_rules! tuple_impl_init {
    ( $h:ident, $($t:ident,)* ) => {
        impl<$h: Init, $($t: Init,)*> Init for ($h, $($t,)*) {
            const INIT: Self = (
                $h::INIT,
                $($t::INIT,)*
            );
        }

        tuple_impl_init! { $($t,)* }
    };
    () => {};
}

tuple_impl_init! {
    A, B, C, D, E, F, G, H, I, J, K, L,
}

macro_rules! array_impl_init {
    {$n:expr, $t:ident $($ts:ident)*} => {
        impl<T> Init for [T; $n] where T: Init {
            const INIT: Self = [$t::INIT, $($ts::INIT),*];
        }
        array_impl_init!{($n - 1), $($ts)*}
    };
    {$n:expr,} => {
        impl<T> Init for [T; $n] {
            const INIT: Self = [];
        }
    };
}

array_impl_init! {32, T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T T}

/// Like `UnsafeCell`, but implements `Sync`.
#[derive(Debug)]
#[repr(transparent)]
pub struct RawCell<T: ?Sized>(UnsafeCell<T>);

unsafe impl<T: Sync + ?Sized> Sync for RawCell<T> {}

impl<T> RawCell<T> {
    pub const fn new(x: T) -> Self {
        Self(UnsafeCell::new(x))
    }

    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: ?Sized> RawCell<T> {
    pub const fn get(&self) -> *mut T {
        self.0.get()
    }
}

/// A "type function" producing a type.
#[doc(hidden)]
pub trait TypeFn {
    type Output;
}

/// Untyped storage of the specified size and alignment.
/// This is analogous to C++'s [`std::aligned_storage_t`].
///
/// [`std::aligned_storage_t`]: https://en.cppreference.com/w/cpp/types/aligned_storage
///
/// This type alias expands to something like the following:
///
/// ```rust,ignore
/// #[repr(align(8))]
/// #[derive(Clone, Copy)]
/// struct AlignedStorage_256_8([u8; 256]);
/// impl Init for AlignedStorage_256_8 { /* ... */ }
/// ```
pub type AlignedStorage<const LEN: usize, const ALIGN: usize> =
    <AlignedStorageFn<LEN, ALIGN> as TypeFn>::Output;

#[doc(hidden)]
pub struct AlignedStorageFn<const LEN: usize, const ALIGN: usize>;

#[doc(hidden)]
pub mod aligned_storage_0b1 {
    /// Implements `TypeFn` on `AlignedStorageFn` for each possible alignemtn
    /// value.
    macro_rules! impl_aligned_storage_fn {
        ($align:tt, $($rest:tt)*) => {
            use super::{TypeFn, Init, AlignedStorageFn};

            impl<const LEN: usize> TypeFn for AlignedStorageFn<LEN, $align> {
                type Output = Bytes<LEN>;
            }

            #[repr(align($align))]
            #[derive(Clone, Copy)]
            pub struct Bytes<const LEN: usize>(pub [u8; LEN]);

            impl<const LEN: usize> Init for Bytes<LEN> {
                const INIT: Self = Self([0; LEN]);
            }

            // It's not allowed to define multiple items with identical names
            // in the same scope. Macros such as `concat!` don't work in an
            // identifier position.
            // The solution? Define them in child modules! As a bonus, these
            // types receive paths remotely resembling the binary representation
            // of alignments, for example:
            // `aligned_storage_0b1::_0::_0::_0::_0::Bytes` (`0b10000`-byte
            // alignment).
            pub mod _0 {
                impl_aligned_storage_fn! { $($rest)* }
            }
        };
        () => {};
    }

    impl_aligned_storage_fn! {
        1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384,
    }
}
