//! Provides an insertion sort implementation.
use core::{cmp::Ordering, mem::swap};

/// Sort the slice using the insertion sort method.
///
/// # Performance
///
/// It was never faster than `[T]::sort_unstable` for `a.len() > 16`.
///
/// # Examples
///
/// ```
/// let mut v = [-5, 4, 1, -3, 2];
///
/// minisort::insertion_sort(&mut v);
/// assert!(v == [-5, -3, 1, 2, 4]);
/// ```
pub fn insertion_sort<T: Ord>(a: &mut [T]) {
    insertion_sort_inner(a, |x, y| x < y);
}

/// Sort the slice with a key extraction function.
///
/// # Examples
///
/// ```
/// let mut v = [-5i32, 4, 1, -3, 2];
///
/// minisort::insertion_sort_by_key(&mut v, |k| k.abs());
/// assert!(v == [1, 2, -3, 4, -5]);
/// ```
pub fn insertion_sort_by_key<T, K: Ord>(a: &mut [T], mut f: impl FnMut(&T) -> K) {
    insertion_sort_inner(a, |x, y| f(x) < f(y));
}

/// Sort the slice with a comparator function.
///
/// # Examples
///
/// ```
/// let mut v = [5, 4, 1, 3, 2];
/// minisort::insertion_sort_by(&mut v, |a, b| a.cmp(b));
/// assert!(v == [1, 2, 3, 4, 5]);
/// ```
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
    use super::*;
    use quickcheck_macros::quickcheck;

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
