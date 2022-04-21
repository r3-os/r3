use core::{fmt, ops};

use crate::utils::{Init, ZeroInit};

/// Represents a signed time span used by the API surface of R3-OS.
///
/// `Duration` is backed by `i32` and can represent the range
/// [-35′47.483648″, +35′47.483647″] with microsecond precision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Duration {
    micros: i32,
}

impl Init for Duration {
    const INIT: Self = Self::ZERO;
}

// Safety: `Duration` is `repr(transparent)` and the only inner field is `i32`,
//         which is `ZeroInit`
unsafe impl ZeroInit for Duration {}

impl Default for Duration {
    fn default() -> Self {
        Self::INIT
    }
}

impl Duration {
    /// An empty interval.
    pub const ZERO: Self = Duration { micros: 0 };

    /// The large representable positive time span (+35′47.483647″).
    pub const MAX: Self = Duration { micros: i32::MAX };

    /// The large representable negative time span (-35′47.483648″).
    pub const MIN: Self = Duration { micros: i32::MIN };

    /// Construct a new `Duration` from the specified number of microseconds.
    #[inline]
    pub const fn from_micros(micros: i32) -> Self {
        Self { micros }
    }

    /// Construct a new `Duration` from the specified number of milliseconds.
    ///
    /// Pancis if `millis` overflows the representable range of `Duration`.
    #[inline]
    pub const fn from_millis(millis: i32) -> Self {
        Self::from_micros(millis.checked_mul(1_000).expect("duration overflow"))
    }

    /// Construct a new `Duration` from the specified number of seconds.
    ///
    /// Pancis if `secs` overflows the representable range of `Duration`.
    #[inline]
    pub const fn from_secs(secs: i32) -> Self {
        Self::from_micros(secs.checked_mul(1_000_000).expect("duration overflow"))
    }

    /// Get the total number of whole microseconds contained by this `Duration`.
    #[inline]
    pub const fn as_micros(self) -> i32 {
        self.micros
    }

    /// Get the total number of whole milliseconds contained by this `Duration`.
    #[inline]
    pub const fn as_millis(self) -> i32 {
        self.micros / 1_000
    }

    /// Get the total number of whole seconds contained by this `Duration`.
    #[inline]
    pub const fn as_secs(self) -> i32 {
        self.micros / 1_000_000
    }

    /// Get the total number of seconds contained by this `Duration` as `f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use r3_core::time::Duration;
    ///
    /// let dur = Duration::from_micros(1_201_250_000);
    /// assert_eq!(dur.as_secs_f64(), 1201.25);
    /// ```
    #[inline]
    pub const fn as_secs_f64(self) -> f64 {
        self.micros as f64 / 1_000_000.0
    }

    /// Get the total number of seconds contained by this `Duration` as `f32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use r3_core::time::Duration;
    ///
    /// let dur = Duration::from_micros(1_201_250_000);
    /// assert_eq!(dur.as_secs_f32(), 1201.25);
    /// ```
    #[inline]
    pub const fn as_secs_f32(self) -> f32 {
        // An integer larger than 16777216 can't be converted to `f32`
        // accurately. Split `self` into an integer part and fractional part and
        // convert them separately so that integral values are preserved
        // during the conversion.
        (self.micros / 1_000_000) as f32 + (self.micros % 1_000_000) as f32 / 1_000_000.0
    }

    /// Return `true` if and only if `self` is positive.
    #[inline]
    pub const fn is_positive(self) -> bool {
        self.micros.is_positive()
    }

    /// Return `true` if and only if `self` is negative.
    #[inline]
    pub const fn is_negative(self) -> bool {
        self.micros.is_negative()
    }

    /// Multiply `self` by the specified value, returning `None` if the result
    /// overflows.
    #[inline]
    pub const fn checked_mul(self, other: i32) -> Option<Self> {
        // `Option::map` is inconvenient to use in `const fn` [ref:const_option_map]
        if let Some(x) = self.micros.checked_mul(other) {
            Some(Self::from_micros(x))
        } else {
            None
        }
    }

    /// Divide `self` by the specified value, returning `None` if the result
    /// overflows or `other` is zero.
    #[inline]
    pub const fn checked_div(self, other: i32) -> Option<Self> {
        // `Option::map` is inconvenient to use in `const fn` [ref:const_option_map]
        if let Some(x) = self.micros.checked_div(other) {
            Some(Self::from_micros(x))
        } else {
            None
        }
    }

    /// Calculate the absolute value of `self`, returning `None` if
    /// `self == MIN`.
    #[inline]
    pub const fn checked_abs(self) -> Option<Self> {
        // `Option::map` is inconvenient to use in `const fn` [ref:const_option_map]
        if let Some(x) = self.micros.checked_abs() {
            Some(Self::from_micros(x))
        } else {
            None
        }
    }

    /// Add the specified value to `self`, returning `None` if the result
    /// overflows.
    #[inline]
    pub const fn checked_add(self, other: Self) -> Option<Self> {
        // `Option::map` is inconvenient to use in `const fn` [ref:const_option_map]
        if let Some(x) = self.micros.checked_add(other.micros) {
            Some(Self::from_micros(x))
        } else {
            None
        }
    }

    /// Subtract the specified value from `self`, returning `None` if the result
    /// overflows.
    #[inline]
    pub const fn checked_sub(self, other: Self) -> Option<Self> {
        // `Option::map` is inconvenient to use in `const fn` [ref:const_option_map]
        if let Some(x) = self.micros.checked_sub(other.micros) {
            Some(Self::from_micros(x))
        } else {
            None
        }
    }
}

/// Error type returned when a checked duration type conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromDurationError(());

impl TryFrom<core::time::Duration> for Duration {
    type Error = TryFromDurationError;

    /// Try to construct a `Duration` from the specified `core::time::Duration`.
    /// Returns an error if the specified `Duration` overflows the representable
    /// range of the destination type.
    ///
    /// The sub-microsecond part is rounded by truncating.
    fn try_from(value: core::time::Duration) -> Result<Self, Self::Error> {
        Ok(Self::from_micros(
            value
                .as_micros()
                .try_into()
                .map_err(|_| TryFromDurationError(()))?,
        ))
    }
}

impl TryFrom<Duration> for core::time::Duration {
    type Error = TryFromDurationError;

    /// Try to construct a `core::time::Duration` from the specified `Duration`.
    /// Returns an error if the specified `Duration` represents a negative time
    /// span.
    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        if value.micros < 0 {
            Err(TryFromDurationError(()))
        } else {
            Ok(Self::from_micros(value.micros as u64))
        }
    }
}

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let abs_dur = core::time::Duration::from_micros(self.micros.unsigned_abs().into());
        if self.micros < 0 {
            write!(f, "-")?;
        }
        abs_dur.fmt(f)
    }
}

impl ops::Add for Duration {
    type Output = Self;

    /// Perform a checked addition, panicking on overflow.
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs)
            .expect("overflow when adding durations")
    }
}

impl ops::AddAssign for Duration {
    /// Perform a checked addition, panicking on overflow.
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl ops::Sub for Duration {
    type Output = Self;

    /// Perform a checked subtraction, panicking on overflow.
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_sub(rhs)
            .expect("overflow when subtracting durations")
    }
}

impl ops::SubAssign for Duration {
    /// Perform a checked subtraction, panicking on overflow.
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl ops::Mul<i32> for Duration {
    type Output = Duration;

    /// Perform a checked multiplication, panicking on overflow.
    #[inline]
    fn mul(self, rhs: i32) -> Self::Output {
        self.checked_mul(rhs)
            .expect("overflow when multiplying duration by scalar")
    }
}

impl ops::Mul<Duration> for i32 {
    type Output = Duration;

    /// Perform a checked multiplication, panicking on overflow.
    #[inline]
    fn mul(self, rhs: Duration) -> Self::Output {
        rhs.checked_mul(self)
            .expect("overflow when multiplying duration by scalar")
    }
}

impl ops::MulAssign<i32> for Duration {
    /// Perform a checked multiplication, panicking on overflow.
    #[inline]
    fn mul_assign(&mut self, rhs: i32) {
        *self = *self * rhs;
    }
}

impl ops::Div<i32> for Duration {
    type Output = Duration;

    /// Perform a checked division, panicking on overflow or when `rhs` is zero.
    #[inline]
    fn div(self, rhs: i32) -> Self::Output {
        self.checked_div(rhs)
            .expect("divide by zero or overflow when dividing duration by scalar")
    }
}

impl ops::DivAssign<i32> for Duration {
    /// Perform a checked division, panicking on overflow or when `rhs` is zero.
    #[inline]
    fn div_assign(&mut self, rhs: i32) {
        *self = *self / rhs;
    }
}

impl core::iter::Sum for Duration {
    /// Perform a checked summation, panicking on overflow.
    fn sum<I: Iterator<Item = Duration>>(iter: I) -> Self {
        iter.fold(Duration::ZERO, |x, y| {
            x.checked_add(y)
                .expect("overflow in iter::sum over durations")
        })
    }
}

impl<'a> core::iter::Sum<&'a Duration> for Duration {
    /// Perform a checked summation, panicking on overflow.
    fn sum<I: Iterator<Item = &'a Duration>>(iter: I) -> Self {
        iter.cloned().sum()
    }
}

#[cfg(feature = "chrono_0p4")]
impl TryFrom<chrono_0p4::Duration> for Duration {
    type Error = TryFromDurationError;

    /// Try to construct a `Duration` from the specified `chrono_0p4::Duration`.
    /// Returns an error if the specified `Duration` overflows the representable
    /// range of the destination type.
    ///
    /// The sub-microsecond part is rounded by truncating.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono_0p4::Duration as ChronoDuration;
    /// use r3_core::time::Duration as OsDuration;
    /// assert_eq!(
    ///     OsDuration::try_from(ChronoDuration::nanoseconds(123_456)),
    ///     Ok(OsDuration::from_micros(123)),
    /// );
    /// assert_eq!(
    ///     OsDuration::try_from(ChronoDuration::nanoseconds(-123_456)),
    ///     Ok(OsDuration::from_micros(-123)),
    /// );
    /// assert!(
    ///     OsDuration::try_from(ChronoDuration::microseconds(0x100000000))
    ///         .is_err()
    /// );
    /// ```
    fn try_from(value: chrono_0p4::Duration) -> Result<Self, Self::Error> {
        Ok(Self::from_micros(
            value
                .num_microseconds()
                .and_then(|x| x.try_into().ok())
                .ok_or(TryFromDurationError(()))?,
        ))
    }
}

#[cfg(feature = "chrono_0p4")]
impl From<Duration> for chrono_0p4::Duration {
    /// Construct a `chrono_0p4::Duration` from the specified `Duration`.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono_0p4::Duration as ChronoDuration;
    /// use r3_core::time::Duration as OsDuration;
    /// assert_eq!(
    ///     ChronoDuration::from(OsDuration::from_micros(123_456)),
    ///     ChronoDuration::microseconds(123_456),
    /// );
    /// ```
    fn from(value: Duration) -> Self {
        Self::microseconds(value.micros as i64)
    }
}

// TODO: Add more tests
// TODO: Maybe add macros to construct a `Duration` with compile-time overflow
//       check
