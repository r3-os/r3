use super::{Init, TypeFn};

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
