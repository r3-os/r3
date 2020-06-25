// FIXME: Work-around for `[T]::sort` being unsupported in `const fn`
/// Sort the provided abstract random-accessible sequence using the specified
/// comparator pseudo-function.
///
/// This sort is stable.
macro_rules! sort_by {
    ($len:expr, |$i:ident| $accessor:expr, |$x:ident, $y: ident| $less_than:expr) => {{
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

        // Insertion sort
        // FIXME: Work-around for `for` being unsupported in `const fn`
        let mut i = 1;
        while i < len {
            // FIXME: Work-around for `for` being unsupported in `const fn`
            let mut j = i;
            while j > 0 {
                let x = *index_mut!(j - 1);
                let y = *index_mut!(j);
                if !less_than!(y, x) {
                    break;
                }

                // swap()
                *index_mut!(j - 1) = y;
                *index_mut!(j) = x;

                j -= 1;
            }
            i += 1;
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
            sort_by!(14, |i| &mut array[i], |x, y| x < y);
            array
        }

        assert_eq!(result(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
    }

    #[quickcheck]
    fn sort(values: Vec<u8>) {
        let mut got: Vec<_> = values.into_iter().enumerate().collect();
        let mut expected = got.clone();

        sort_by!(got.len(), |i| &mut got[i], |x, y| x.1 < y.1);
        expected.sort_by_key(|x| x.1);

        assert_eq!(got, expected);
    }
}
