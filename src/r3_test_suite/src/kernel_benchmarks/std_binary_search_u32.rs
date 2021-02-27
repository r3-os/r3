//! <https://github.com/rust-lang/rust/pull/74024>
use core::marker::PhantomData;
use r3::kernel::{cfg::CfgBuilder, Kernel, Semaphore, Task};

use super::Bencher;
use crate::utils::benchmark::Interval;

use_benchmark_in_kernel_benchmark! {
    pub unsafe struct App<System> {
        inner: AppInner<System>,
    }
}

struct AppInner<System> {
    _phantom: PhantomData<System>,
}

const I_U32_16C_BEST: Interval = "u32, 16 elems (const), best";
const I_U32_16C_WORST1: Interval = "u32, 16 elems (const), worst 1";
const I_U32_16C_WORST2: Interval = "u32, 16 elems (const), worst 2";
const I_U32_2_BEST: Interval = "u32, 2 elems, best";
const I_U32_2_WORST1: Interval = "u32, 2 elems, worst 1";
const I_U32_2_WORST2: Interval = "u32, 2 elems, worst 2";
const I_U32_16_BEST: Interval = "u32, 16 elems, best";
const I_U32_16_WORST1: Interval = "u32, 16 elems, worst 1";
const I_U32_16_WORST2: Interval = "u32, 16 elems, worst 2";
const I_U32_256_BEST: Interval = "u32, 256 elems, best";
const I_U32_256_WORST1: Interval = "u32, 256 elems, worst 1";
const I_U32_256_WORST2: Interval = "u32, 256 elems, worst 2";

const I_NEW_U32_16C_BEST: Interval = "#74024, u32, 16 elems (const), best";
const I_NEW_U32_16C_WORST1: Interval = "#74024, u32, 16 elems (const), worst 1";
const I_NEW_U32_16C_WORST2: Interval = "#74024, u32, 16 elems (const), worst 2";
const I_NEW_U32_2_BEST: Interval = "#74024, u32, 2 elems, best";
const I_NEW_U32_2_WORST1: Interval = "#74024, u32, 2 elems, worst 1";
const I_NEW_U32_2_WORST2: Interval = "#74024, u32, 2 elems, worst 2";
const I_NEW_U32_16_BEST: Interval = "#74024, u32, 16 elems, best";
const I_NEW_U32_16_WORST1: Interval = "#74024, u32, 16 elems, worst 1";
const I_NEW_U32_16_WORST2: Interval = "#74024, u32, 16 elems, worst 2";
const I_NEW_U32_256_BEST: Interval = "#74024, u32, 256 elems, best";
const I_NEW_U32_256_WORST1: Interval = "#74024, u32, 256 elems, worst 1";
const I_NEW_U32_256_WORST2: Interval = "#74024, u32, 256 elems, worst 2";

static TEST_ARRAY: [u32; 256] = {
    let mut a = [0; 256];
    let mut i = 0;
    while i < 256 {
        a[i as usize] = i * 0x01010101;
        i += 1;
    }
    a
};

#[inline(never)]
fn black_box<T>(dummy: T) {
    unsafe { asm!("# unused: {}", in(reg) &dummy) };
}

impl<System: Kernel> AppInner<System> {
    /// Used by `use_benchmark_in_kernel_benchmark!`
    const fn new<B: Bencher<System, Self>>(b: &mut CfgBuilder<System>) -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Used by `use_benchmark_in_kernel_benchmark!`
    fn iter<B: Bencher<System, Self>>() {
        for &(n, best_value, i_best, i_worst1, i_worst2) in &[
            (2, 0x01010101, I_U32_2_BEST, I_U32_2_WORST1, I_U32_2_WORST2),
            (
                16,
                0x08080808,
                I_U32_16_BEST,
                I_U32_16_WORST1,
                I_U32_16_WORST2,
            ),
            (
                256,
                0x80808080,
                I_U32_256_BEST,
                I_U32_256_WORST1,
                I_U32_256_WORST2,
            ),
        ] {
            let array = &TEST_ARRAY[0..n];
            B::mark_start();
            black_box(array.binary_search(&best_value));
            B::mark_end(i_best);

            B::mark_start();
            black_box(array.binary_search(&1));
            B::mark_end(i_worst1);

            B::mark_start();
            black_box(array.binary_search(&0xfffffffe));
            B::mark_end(i_worst2);
        }

        {
            let array = &TEST_ARRAY[0..16];
            B::mark_start();
            black_box(array.binary_search(&0x08080808));
            B::mark_end(I_U32_16C_BEST);

            B::mark_start();
            black_box(array.binary_search(&1));
            B::mark_end(I_U32_16C_WORST1);

            B::mark_start();
            black_box(array.binary_search(&0xfffffffe));
            B::mark_end(I_U32_16C_WORST2);
        }

        // ------------------------------------------------------------

        for &(n, best_value, i_best, i_worst1, i_worst2) in &[
            (
                2,
                0x01010101,
                I_NEW_U32_2_BEST,
                I_NEW_U32_2_WORST1,
                I_NEW_U32_2_WORST2,
            ),
            (
                16,
                0x08080808,
                I_NEW_U32_16_BEST,
                I_NEW_U32_16_WORST1,
                I_NEW_U32_16_WORST2,
            ),
            (
                256,
                0x80808080,
                I_NEW_U32_256_BEST,
                I_NEW_U32_256_WORST1,
                I_NEW_U32_256_WORST2,
            ),
        ] {
            let array = &TEST_ARRAY[0..n];
            B::mark_start();
            black_box(new_bsearch::binary_search(array, &best_value));
            B::mark_end(i_best);

            B::mark_start();
            black_box(new_bsearch::binary_search(array, &1));
            B::mark_end(i_worst1);

            B::mark_start();
            black_box(new_bsearch::binary_search(array, &0xfffffffe));
            B::mark_end(i_worst2);
        }

        {
            let array = &TEST_ARRAY[0..16];
            B::mark_start();
            black_box(new_bsearch::binary_search(array, &0x08080808));
            B::mark_end(I_NEW_U32_16C_BEST);

            B::mark_start();
            black_box(new_bsearch::binary_search(array, &1));
            B::mark_end(I_NEW_U32_16C_WORST1);

            B::mark_start();
            black_box(new_bsearch::binary_search(array, &0xfffffffe));
            B::mark_end(I_NEW_U32_16C_WORST2);
        }
    }
}

mod new_bsearch {
    use core::cmp::Ordering::{self, Greater, Less};

    pub fn binary_search<T>(this: &[T], x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        binary_search_by(this, |p| p.cmp(x))
    }

    #[inline]
    pub fn binary_search_by<'a, T, F>(this: &'a [T], mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        let mut left = 0;
        let mut right = this.len();
        while left < right {
            // Never overflow because `slice::len()` max is `isize::MAX`.
            // For Zero Sized Type (ZST), the max could be `usize::MAX`.
            // e.g `[(); usize::MAX]`. However, we still never overflow,
            // because all elements are the same (equals to unit type),
            // we can get the result in O(1).
            //
            // ```
            // let b = [(); usize::MAX];
            // assert_eq!(b.binary_search(&()), Ok(usize::MAX / 2));
            // ```
            let mid = (left + right) / 2;
            // SAFETY: the call is made safe by the following invariants:
            // - `mid >= 0`
            // - `mid < size`: `mid` is limited by `[left; right)` bound.
            let cmp = f(unsafe { this.get_unchecked(mid) });

            // The reason why we use if/else control flow rather than match
            // is because match reorders comparison operations, which is perf sensitive.
            // This is x86 asm for u8: https://rust.godbolt.org/z/8Y8Pra.
            if cmp == Less {
                left = mid + 1;
            } else if cmp == Greater {
                right = mid;
            } else {
                return Ok(mid);
            }
        }
        Err(left)
    }
}
