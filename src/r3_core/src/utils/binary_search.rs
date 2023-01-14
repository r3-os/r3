//! Given an abstract random-accessible sequence, returns an index pointing to
//! the first element that is not less than (greater than or equal to) some
//! value (which is implicit to this macro). The provided predicate is used to
//! compare the element at a given index to that value.
//!
//! Returns a `usize` in range `0..=$len`.
macro_rules! lower_bound {
    (
        $len:expr,
        |$i:ident| $less_than:expr
    ) => {{
        let mut i = 0usize;
        let mut end: usize = $len;
        while end > i {
            let mid = i + (end - i) / 2;
            let lt = {
                let $i = mid;
                $less_than
            };
            if lt {
                i = mid + 1;
            } else {
                end = mid;
            }
        }
        i
    }};
}

#[cfg(test)]
mod tests {
    use quickcheck_macros::quickcheck;

    #[test]
    fn const_lower_bound() {
        const fn result() -> usize {
            let array = [5, 5, 5, 5, 6, 6, 7, 8, 9, 10, 11, 12, 12, 12, 13, 13];
            lower_bound!(array.len(), |i| array[i] < 12)
        }

        assert_eq!(result(), 11);
    }

    #[quickcheck]
    fn lower_bound(mut values: Vec<u32>, arbitrary_value: u32) {
        values.sort();

        log::debug!("values = {values:?}");

        for (i, &e) in values.iter().enumerate() {
            let mut expected = i;
            while expected > 0 && values[expected - 1] == values[expected] {
                expected -= 1;
            }

            let got = lower_bound!(values.len(), |i| values[i] < e);
            log::debug!("  lower_bound(values[{i}]) = {got} (expected {expected})");

            assert_eq!(got, expected);
        }

        for (i, win) in values.windows(2).enumerate() {
            if win[1] - win[0] < 2 {
                continue;
            }
            let mid = win[0] + (win[1] - win[0]) / 2;
            let got = lower_bound!(values.len(), |i| values[i] < mid);
            log::debug!("  lower_bound(mean(values[{i}] + values[{i} + 1])) = {got}");
            assert_eq!(got, i + 1);
        }

        if values.is_empty() {
            let got = lower_bound!(values.len(), |i| values[i] < arbitrary_value);
            log::debug!("  lower_bound({arbitrary_value}) = {got}");
            assert_eq!(got, 0);
        } else {
            if *values.first().unwrap() > 0 {
                #[allow(unused_comparisons)]
                let got = lower_bound!(values.len(), |i| values[i] < 0);
                log::debug!("  lower_bound(0) = {got}");
                assert_eq!(got, 0);
            }
            if *values.last().unwrap() < u32::MAX {
                let got = lower_bound!(values.len(), |i| values[i] < u32::MAX);
                log::debug!("  lower_bound({}) = {got}", u32::MAX);
                assert_eq!(got, values.len());
            }
        }
    }
}
