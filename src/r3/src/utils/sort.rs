// FIXME: Work-around for `[T]::sort` being unsupported in `const fn`
/// Sort the provided abstract random-accessible sequence using the specified
/// comparator pseudo-function.
///
/// This sort is not stable.
macro_rules! sort_unstable_by {
    (
        $len:expr,
        |$i:ident| $accessor:expr,
        |$x:ident, $y: ident| $less_than:expr
    ) => {{
        // FIXME: Work-around for closures being uncallable in `const fn`
        macro_rules! index_mut {
            ($i2:expr) => {{
                let $i = $i2;
                $accessor
            }};
        }

        macro_rules! less_than {
            ($x2:expr, $y2:expr) => {{
                let $x = $x2;
                let $y = $y2;
                $less_than
            }};
        }

        let len: usize = $len;

        // Convert the input array into a binary max-heap
        if len > 1 {
            let mut i = (len - 2) / 2;
            while {
                sift_down!(len, i);
                if i == 0 {
                    false
                } else {
                    i -= 1;
                    true
                }
            } {}
        }

        // Fill the output in the reverse order
        // FIXME: Work-around for `for` being unsupported in `const fn`
        if len > 0 {
            let mut i = len - 1;
            while i > 0 {
                // `index_mut!(0)` is the root and contains the largest value.
                // Swap it with `index_mut!(i)`.
                let x = *index_mut!(0);
                let y = *index_mut!(i);
                *index_mut!(0) = y;
                *index_mut!(i) = x;

                // Shrink the heap by one
                i -= 1;

                // Restore the heap invariant
                sift_down!(i + 1, 0);
            }
        }
    }};
}

/// Used internally by `sort_unstable_by`. Based on `sift_down` from the Rust standard
/// library.
macro_rules! sift_down {
    ($len:expr, $start:expr) => {{
        let len: usize = $len;

        let mut hole: usize = $start;
        let mut child = hole * 2 + 1;

        while child < len {
            let right = child + 1;

            // compare with the greater of the two children
            let mut child_value = *index_mut!(child);
            if right < len {
                let right_value = *index_mut!(right);
                if !less_than!(right_value, child_value) {
                    child = right;
                    child_value = right_value;
                }
            }

            // if we are already in order, stop.
            let hole_value = *index_mut!(hole);
            if !less_than!(hole_value, child_value) {
                break;
            }

            // swap
            *index_mut!(hole) = child_value;
            *index_mut!(child) = hole_value;

            hole = child;
            child = hole * 2 + 1;
        }
    }};
}

#[cfg(test)]
mod tests {
    use quickcheck_macros::quickcheck;

    #[test]
    fn const_sort() {
        const fn result() -> [u32; 14] {
            let mut array = [2, 6, 1, 9, 13, 3, 8, 12, 5, 11, 14, 7, 4, 10];
            sort_unstable_by!(14, |i| &mut array[i], |x, y| x < y);
            array
        }

        assert_eq!(result(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    }

    #[quickcheck]
    fn sort(values: Vec<u8>) {
        let mut got: Vec<_> = values.into_iter().collect();
        let mut expected = got.clone();

        sort_unstable_by!(got.len(), |i| &mut got[i], |x, y| x < y);
        expected.sort();

        assert_eq!(got, expected);
    }
}
