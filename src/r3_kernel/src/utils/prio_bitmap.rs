//! Provides `FixedPrioBitmap`, a bit array structure supporting
//! logarithmic-time bit scan operations.
use core::fmt;

use super::{ctz::trailing_zeros, BinInteger, Init};

/// The maximum bit count supported by [`FixedPrioBitmap`].
pub const FIXED_PRIO_BITMAP_MAX_LEN: usize = WORD_LEN * WORD_LEN * WORD_LEN;

/// A bit array structure supporting logarithmic-time bit scan operations.
///
/// All valid instantiations implement [`PrioBitmap`].
pub type FixedPrioBitmap<const LEN: usize> = If! {
    if (LEN <= WORD_LEN) {
        OneLevelPrioBitmap<LEN>
    } else if (LEN <= WORD_LEN * WORD_LEN) {
        TwoLevelPrioBitmapImpl<
            OneLevelPrioBitmap<{(LEN + WORD_LEN - 1) / WORD_LEN}>,
            {(LEN + WORD_LEN - 1) / WORD_LEN}
        >
    } else if (LEN <= WORD_LEN * WORD_LEN * WORD_LEN) {
        TwoLevelPrioBitmapImpl<
            TwoLevelPrioBitmapImpl<
                OneLevelPrioBitmap<{(LEN + WORD_LEN * WORD_LEN - 1) / (WORD_LEN * WORD_LEN)}>,
                {(LEN + WORD_LEN * WORD_LEN - 1) / (WORD_LEN * WORD_LEN)}
            >,
            {(LEN + WORD_LEN - 1) / WORD_LEN}
        >
    } else {
        TooManyLevels
    }
};

/// Get an instantiation of `OneLevelPrioBitmapImpl` capable of storing `LEN`
/// entries.
#[doc(hidden)]
pub type OneLevelPrioBitmap<const LEN: usize> = If! {
    |LEN: usize|
    if (LEN == 0) {
        ()
    } else if (LEN <= 8 && LEN <= WORD_LEN) {
        OneLevelPrioBitmapImpl<u8, LEN>
    } else if (LEN <= 16 && LEN <= WORD_LEN) {
        OneLevelPrioBitmapImpl<u16, LEN>
    } else if (LEN <= 32 && LEN <= WORD_LEN) {
        OneLevelPrioBitmapImpl<u32, LEN>
    } else if (LEN <= 64 && LEN <= WORD_LEN) {
        OneLevelPrioBitmapImpl<u64, LEN>
    } else if (LEN <= 128 && LEN <= WORD_LEN) {
        OneLevelPrioBitmapImpl<u128, LEN>
    } else {
        TooManyLevels
    }
};

/// Trait for [`FixedPrioBitmap`].
///
/// All methods panic when the given bit position is out of range.
pub trait PrioBitmap: Init + Send + Sync + Clone + Copy + fmt::Debug + 'static {
    /// Get the bit at the specified position.
    fn get(&self, i: usize) -> bool;

    /// Clear the bit at the specified position.
    fn clear(&mut self, i: usize);

    /// Set the bit at the specified position.
    fn set(&mut self, i: usize);

    /// Get the position of the first set bit.
    fn find_set(&self) -> Option<usize>;
}

impl PrioBitmap for () {
    fn get(&self, _: usize) -> bool {
        unreachable!()
    }

    fn clear(&mut self, _: usize) {
        unreachable!()
    }

    fn set(&mut self, _: usize) {
        unreachable!()
    }

    fn find_set(&self) -> Option<usize> {
        None
    }
}

/// Stores `LEN` (≤ `T::BITS`) entries.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct OneLevelPrioBitmapImpl<T, const LEN: usize> {
    bits: T,
}

impl<T: BinInteger, const LEN: usize> Init for OneLevelPrioBitmapImpl<T, LEN> {
    const INIT: Self = Self { bits: T::INIT };
}

impl<T: BinInteger, const LEN: usize> fmt::Debug for OneLevelPrioBitmapImpl<T, LEN> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.bits.one_digits()).finish()
    }
}

impl<T: BinInteger, const LEN: usize> PrioBitmap for OneLevelPrioBitmapImpl<T, LEN> {
    fn get(&self, i: usize) -> bool {
        assert!(i < LEN && i < usize::try_from(T::BITS).unwrap());
        self.bits.get_bit(i as u32)
    }

    fn clear(&mut self, i: usize) {
        assert!(i < LEN && i < usize::try_from(T::BITS).unwrap());
        self.bits.clear_bit(i as u32);
    }

    fn set(&mut self, i: usize) {
        assert!(i < LEN && i < usize::try_from(T::BITS).unwrap());
        self.bits.set_bit(i as u32);
    }

    fn find_set(&self) -> Option<usize> {
        if LEN <= usize::BITS as usize {
            // Use an optimized version of `trailing_zeros`
            let bits = self.bits.to_usize().unwrap();
            let i = trailing_zeros::<LEN>(bits);
            if i == usize::BITS {
                None
            } else {
                Some(i as usize)
            }
        } else {
            let i = self.bits.trailing_zeros();
            if i == T::BITS {
                None
            } else {
                Some(i as usize)
            }
        }
    }
}

/// Stores `WORD_LEN * LEN` entries. `T` must implement `PrioBitmap` and
/// be able to store `LEN` entries.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct TwoLevelPrioBitmapImpl<T, const LEN: usize> {
    // Invariant: `first.get(i) == (second[i] != 0)`
    first: T,
    second: [Word; LEN],
}

type Word = usize;
const WORD_LEN: usize = core::mem::size_of::<Word>() * 8;

impl<T: PrioBitmap, const LEN: usize> Init for TwoLevelPrioBitmapImpl<T, LEN> {
    const INIT: Self = Self {
        first: T::INIT,
        second: [0; LEN],
    };
}

impl<T: PrioBitmap, const LEN: usize> fmt::Debug for TwoLevelPrioBitmapImpl<T, LEN> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list()
            .entries(self.second.iter().enumerate().flat_map(|(group_i, group)| {
                group
                    .one_digits()
                    .map(move |subgroup_i| subgroup_i as usize + group_i * WORD_LEN)
            }))
            .finish()
    }
}

impl<T: PrioBitmap, const LEN: usize> PrioBitmap for TwoLevelPrioBitmapImpl<T, LEN> {
    fn get(&self, i: usize) -> bool {
        self.second[i / WORD_LEN].get_bit(u32::try_from(i % WORD_LEN).unwrap())
    }

    fn clear(&mut self, i: usize) {
        let group = &mut self.second[i / WORD_LEN];
        group.clear_bit(u32::try_from(i % WORD_LEN).unwrap());
        if *group == 0 {
            self.first.clear(i / WORD_LEN);
        }
    }

    fn set(&mut self, i: usize) {
        let group = &mut self.second[i / WORD_LEN];
        group.set_bit(u32::try_from(i % WORD_LEN).unwrap());
        self.first.set(i / WORD_LEN);
    }

    fn find_set(&self) -> Option<usize> {
        self.first.find_set().map(|group_i| {
            let group = self.second[group_i];
            let subgroup_i = group.trailing_zeros() as usize;
            debug_assert_ne!(subgroup_i, WORD_LEN);
            subgroup_i + group_i * WORD_LEN
        })
    }
}

/// Indicates the requested size is not supported.
#[doc(hidden)]
#[non_exhaustive]
pub struct TooManyLevels {}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;
    use std::collections::BTreeSet;

    struct BTreePrioBitmap(BTreeSet<usize>);

    impl BTreePrioBitmap {
        fn new() -> Self {
            Self(BTreeSet::new())
        }

        fn enum_set_bits(&self) -> Vec<usize> {
            self.0.iter().cloned().collect()
        }

        fn clear(&mut self, i: usize) {
            self.0.remove(&i);
        }

        fn set(&mut self, i: usize) {
            self.0.insert(i);
        }

        fn find_set(&self) -> Option<usize> {
            self.0.iter().next().cloned()
        }
    }

    /// A modifying operation on `PrioBitmap`.
    #[derive(Debug)]
    enum Cmd {
        Insert(usize),
        Remove(usize),
    }

    /// Map random bytes to operations on `PrioBitmap`.
    fn interpret(bytecode: &[u8], bitmap_len: usize) -> impl Iterator<Item = Cmd> + '_ {
        let mut i = 0;
        let mut known_set_bits = Vec::new();
        std::iter::from_fn(move || {
            if bitmap_len == 0 {
                None
            } else if let Some(instr) = bytecode.get(i..i + 5) {
                i += 5;

                let value = u32::from_le_bytes([instr[1], instr[2], instr[3], instr[4]]) as usize;

                if instr[0] % 2 == 0 || known_set_bits.is_empty() {
                    let bit = value % bitmap_len;
                    known_set_bits.push(bit);
                    Some(Cmd::Insert(bit))
                } else {
                    let i = value % known_set_bits.len();
                    let bit = known_set_bits.swap_remove(i);
                    Some(Cmd::Remove(bit))
                }
            } else {
                None
            }
        })
    }

    fn enum_set_bits(bitmap: &impl PrioBitmap, bitmap_len: usize) -> Vec<usize> {
        (0..bitmap_len).filter(|&i| bitmap.get(i)).collect()
    }

    fn test_inner<T: PrioBitmap>(bytecode: Vec<u8>, size: usize) {
        let mut subject = T::INIT;
        let mut reference = BTreePrioBitmap::new();

        log::info!("size = {size}");

        for cmd in interpret(&bytecode, size) {
            log::trace!("    {cmd:?}");
            match cmd {
                Cmd::Insert(bit) => {
                    subject.set(bit);
                    reference.set(bit);
                }
                Cmd::Remove(bit) => {
                    subject.clear(bit);
                    reference.clear(bit);
                }
            }

            assert_eq!(subject.find_set(), reference.find_set());
        }

        assert_eq!(subject.find_set(), reference.find_set());
        assert_eq!(enum_set_bits(&subject, size), reference.enum_set_bits());
    }

    macro_rules! gen_test {
        ($(#[$m:meta])* mod $name:ident, $size:literal) => {
            $(#[$m])*
            mod $name {
                use super::*;

                #[quickcheck]
                fn test(bytecode: Vec<u8>) {
                    test_inner::<FixedPrioBitmap<$size>>(bytecode, $size);
                }
            }
        };
    }

    gen_test!(mod size_0, 0);
    gen_test!(mod size_1, 1);
    gen_test!(mod size_10, 10);
    gen_test!(mod size_100, 100);
    gen_test!(mod size_1000, 1000);
    gen_test!(
        #[cfg(any(target_pointer_width = "32", target_pointer_width = "64", target_pointer_width = "128"))]
        mod size_10000, 10000
    );
    gen_test!(
        #[cfg(any(target_pointer_width = "64", target_pointer_width = "128"))]
        mod size_100000, 100000
    );
}
