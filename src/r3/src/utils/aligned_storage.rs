use core::mem::MaybeUninit;

use super::ConstDefault;

/// Untyped storage of the specified size and alignment.
/// This is analogous to C++'s [`std::aligned_storage_t`].
///
/// [`std::aligned_storage_t`]: https://en.cppreference.com/w/cpp/types/aligned_storage
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AlignedStorage<const LEN: usize, const ALIGN: usize>(
    elain::Align<ALIGN>,
    [MaybeUninit<u8>; LEN],
)
where
    elain::Align<ALIGN>: elain::Alignment;

impl<const LEN: usize, const ALIGN: usize> ConstDefault for AlignedStorage<LEN, ALIGN>
where
    elain::Align<ALIGN>: elain::Alignment,
{
    const DEFAULT: Self = Self(elain::Align::NEW, [MaybeUninit::uninit(); LEN]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn size_align() {
        use core::alloc::Layout;

        macro test($len:expr, $align:expr) {{
            let layout = Layout::new::<AlignedStorage<$len, $align>>();
            dbg!(layout);
            assert_eq!(layout.align(), $align);
            assert_eq!(layout.size(), ($len + $align - 1) / $align * $align);
        }}

        macro test_outer($len:expr) {
            test!($len, 1);
            test!($len, 2);
            test!($len, 4);
            test!($len, 8);
            test!($len, 16);
            test!($len, 32);
            test!($len, 1024);
        }

        test_outer!(0);
        test_outer!(1);
        test_outer!(10);
        test_outer!(100);
        test_outer!(1000);
        test_outer!(1234);
        test_outer!(4321);
        test_outer!(10000);
        test_outer!(30000);
    }
}
