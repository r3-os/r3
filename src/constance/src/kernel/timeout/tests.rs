use super::*;

#[test]
fn test_time32_from_duration() {
    assert_eq!(Ok(0), time32_from_duration(Duration::from_micros(0)));
    assert_eq!(
        Ok(0x7fff_ffff),
        time32_from_duration(Duration::from_micros(0x7fff_ffff))
    );
    assert_eq!(
        Err(BadParamError::BadParam),
        time32_from_duration(Duration::from_micros(-1))
    );
    assert_eq!(
        Err(BadParamError::BadParam),
        time32_from_duration(Duration::from_micros(-0x80000000))
    );
}

#[test]
fn test_time32_from_neg_duration() {
    assert_eq!(Ok(0), time32_from_neg_duration(Duration::from_micros(0)));
    assert_eq!(
        Ok(0x8000_0000),
        time32_from_neg_duration(Duration::from_micros(-0x8000_0000))
    );
    assert_eq!(
        Err(BadParamError::BadParam),
        time32_from_neg_duration(Duration::from_micros(1))
    );
    assert_eq!(
        Err(BadParamError::BadParam),
        time32_from_neg_duration(Duration::from_micros(0x7fff_ffff))
    );
}

#[test]
fn test_wrapping_time32_from_duration() {
    assert_eq!(0, wrapping_time32_from_duration(Duration::from_micros(0)));
    assert_eq!(
        0xffff_ffff,
        wrapping_time32_from_duration(Duration::from_micros(-1))
    );
    assert_eq!(
        0x8000_0000,
        wrapping_time32_from_duration(Duration::from_micros(-0x8000_0000))
    );
    assert_eq!(1, wrapping_time32_from_duration(Duration::from_micros(1)));
    assert_eq!(
        0x7fff_ffff,
        wrapping_time32_from_duration(Duration::from_micros(0x7fff_ffff))
    );
}

#[test]
fn test_wrapping_time64_from_duration() {
    assert_eq!(0, wrapping_time64_from_duration(Duration::from_micros(0)));
    assert_eq!(
        0xffff_ffff_ffff_ffff,
        wrapping_time64_from_duration(Duration::from_micros(-1))
    );
    assert_eq!(
        0xffff_ffff_8000_0000,
        wrapping_time64_from_duration(Duration::from_micros(-0x8000_0000))
    );
    assert_eq!(1, wrapping_time64_from_duration(Duration::from_micros(1)));
    assert_eq!(
        0x7fff_ffff,
        wrapping_time64_from_duration(Duration::from_micros(0x7fff_ffff))
    );
}
