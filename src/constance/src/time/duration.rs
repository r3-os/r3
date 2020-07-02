use core::{convert::TryInto, fmt};

use crate::utils::{Init, ZeroInit};

/// Represents a signed time span used by the API surface of the Constance
/// RTOS.
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
        // FIXME: `Option::expect` is not `const fn` yet
        Self::from_micros(if let Some(x) = millis.checked_mul(1_000) {
            x
        } else {
            panic!("duration overflow");
        })
    }

    /// Construct a new `Duration` from the specified number of seconds.
    ///
    /// Pancis if `secs` overflows the representable range of `Duration`.
    #[inline]
    pub const fn from_secs(secs: i32) -> Self {
        // FIXME: `Option::expect` is not `const fn` yet
        Self::from_micros(if let Some(x) = secs.checked_mul(1_000_000) {
            x
        } else {
            panic!("duration overflow");
        })
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
    /// use constance::time::Duration;
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
    /// use constance::time::Duration;
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
        // FIXME: `Option::map` is not `const fn` yet
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
        // FIXME: `Option::map` is not `const fn` yet
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
        // FIXME: `Option::map` is not `const fn` yet
        if let Some(x) = self.micros.checked_abs() {
            Some(Self::from_micros(x))
        } else {
            None
        }
    }
}

/// Error type returned when a checked duration type conversion fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromDurationError(());

impl core::convert::TryFrom<core::time::Duration> for Duration {
    type Error = TryFromDurationError;

    /// Try to construct a `Duration` from the specified `core::time::Duration`.
    /// Returns an error if the specified `Duration` overflows the representable
    /// range of the destination type.
    ///
    /// The sub-microsecond part is rounded down.
    fn try_from(value: core::time::Duration) -> Result<Self, Self::Error> {
        Ok(Self::from_micros(
            value
                .as_micros()
                .try_into()
                .map_err(|_| TryFromDurationError(()))?,
        ))
    }
}

impl core::convert::TryFrom<Duration> for core::time::Duration {
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
        let abs_dur = core::time::Duration::from_micros((self.micros as i64).abs() as u64);
        if self.micros < 0 {
            write!(f, "-")?;
        }
        abs_dur.fmt(f)
    }
}

// TODO: Add more tests
// TODO: Maybe add macros to construct a `Duration` with compile-time overflow
//       check
// TODO: Interoperation with `::chrono`