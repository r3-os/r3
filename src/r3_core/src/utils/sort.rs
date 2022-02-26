// Work-around for `[T]::sort████████` being unsupported in `const fn`
// [ref:const_slice_sort_unstable]
/// Sort the specified slice using the specified comparator.
///
/// This sort is not stable.
pub(crate) const fn slice_sort_unstable_by<T, Comparer>(mut v: &mut [T], is_less: Comparer)
where
    Comparer: ~const FnMut(&T, &T) -> bool + Copy,
{
    // This implementation is based on `heapsort` from the Rust standard
    // library.

    // This binary heap respects the invariant `parent >= child`.
    //
    // Closures aren't `~const FnMut` yet [ref:const_closures], so this
    // `sift_down` was replaced with a bare `const fn`
    const fn sift_down<T, Comparer>(v: &mut [T], mut node: usize, mut is_less: Comparer)
    where
        Comparer: ~const FnMut(&T, &T) -> bool + Copy,
    {
        loop {
            // Children of `node`:
            let left = 2 * node + 1;
            let right = 2 * node + 2;

            // Choose the greater child.
            let greater = if right < v.len() && is_less(&v[left], &v[right]) {
                right
            } else {
                left
            };

            // Stop if the invariant holds at `node`.
            if greater >= v.len() || !is_less(&v[node], &v[greater]) {
                break;
            }

            // Swap `node` with the greater child, move one step down, and continue sifting.
            v.swap(node, greater);
            node = greater;
        }
    }

    // Build the heap in linear time.
    // `for` is unusable in `const fn` [ref:const_for]
    let mut i = v.len() / 2;
    while i > 0 {
        i -= 1;
        sift_down(v, i, is_less);
    }

    // Pop maximal elements from the heap.
    while v.len() >= 2 {
        v.swap(0, v.len() - 1);
        v = v.split_last_mut().unwrap().1;
        sift_down(v, 0, is_less);
    }
}

/// `const fn`-compatible closure.
///
/// This is a work-around for closures not being `const Fn`.
/// <!-- [ref:const_closures] -->
macro_rules! closure {
    (|$($pname:ident: $pty:ty),*| -> $rty:ty $x:block) => {{
        const fn __closure__($($pname: $pty),*) -> $rty $x
        __closure__
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[test]
    fn const_sort() {
        const fn result() -> [u32; 14] {
            let mut array = [2, 6, 1, 9, 13, 3, 8, 12, 5, 11, 14, 7, 4, 10];
            slice_sort_unstable_by(&mut array, closure!(|x: &u32, y: &u32| -> bool { *x < *y }));
            array
        }

        assert_eq!(result(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    }

    #[quickcheck]
    fn sort(values: Vec<u8>) {
        let mut got: Vec<_> = values.into_iter().collect();
        let mut expected = got.clone();

        slice_sort_unstable_by(&mut got, closure!(|x: &u8, y: &u8| -> bool { *x < *y }));
        expected.sort();

        assert_eq!(got, expected);
    }
}
