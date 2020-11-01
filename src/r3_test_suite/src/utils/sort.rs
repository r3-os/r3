//! Provides an insertion sort implementation.
use core::{cmp::Ordering, mem::swap};

/// Sort the slice using the insertion sort method.
///
/// # Performance
///
/// It was never faster than `[T]::sort_unstable` for `a.len() > 16`.
pub fn insertion_sort<T: Ord>(a: &mut [T]) {
    insertion_sort_inner(a, |x, y| x < y);
}

/// Sort the slice with a key extraction function.
pub fn insertion_sort_by_key<T, K: Ord>(a: &mut [T], mut f: impl FnMut(&T) -> K) {
    insertion_sort_inner(a, |x, y| f(x) < f(y));
}

/// Sort the slice with a comparator function.
pub fn insertion_sort_by<T>(a: &mut [T], mut f: impl FnMut(&T, &T) -> Ordering) {
    insertion_sort_inner(a, |x, y| f(x, y) == Ordering::Less);
}

fn insertion_sort_inner<T>(a: &mut [T], mut f: impl FnMut(&T, &T) -> bool) {
    for i in 1..a.len() {
        let mut ap = &mut a[0..=i];

        while let [.., p1, p2] = ap {
            if f(p1, p2) {
                break;
            }
            swap(p1, p2);
            ap = ap.split_last_mut().unwrap().1;
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use quickcheck_macros::quickcheck;
    use std::vec::Vec;

    #[quickcheck]
    fn result_is_sorted(mut v: Vec<i32>) -> bool {
        insertion_sort(&mut v);
        v.is_sorted()
    }

    #[quickcheck]
    fn result_is_sorted_by_key(mut v: Vec<(i32, i32)>) -> bool {
        insertion_sort_by_key(&mut v, |e| e.1);
        v.is_sorted_by_key(|e| e.1)
    }

    #[quickcheck]
    fn result_is_sorted_by(mut v: Vec<i32>) -> bool {
        insertion_sort_by(&mut v, |x, y| y.cmp(x));
        v.is_sorted_by(|x, y| Some(y.cmp(x)))
    }
}
