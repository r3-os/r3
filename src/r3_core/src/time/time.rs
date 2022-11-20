use core::{fmt, ops};

use crate::{
    time::Duration,
    utils::{Init, Zeroable},
};

/// Represents a timestamp used by the API surface of R3-OS.
///
/// The origin is application-defined. If an application desires to represent a
/// calender time using `Time`, it's recommended to use the midnight UTC on
/// January 1, 1970 (a.k.a. “UNIX timestamp”) as the origin.
///
/// `Time` is backed by `u64` and can represent up to 213,503,982 days with
/// microsecond precision.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Zeroable)]
#[repr(transparent)]
pub struct Time {
    micros: u64,
}

impl Init for Time {
    const INIT: Self = Self::ZERO;
}

impl Default for Time {
    fn default() -> Self {
        Self::INIT
    }
}

impl Time {
    /// Zero (the origin).
    pub const ZERO: Self = Time { micros: 0 };

    /// The large representable timestamp.
    pub const MAX: Self = Time { micros: u64::MAX };

    /// Construct a new `Time` from the specified number of microseconds.
    #[inline]
    pub const fn from_micros(micros: u64) -> Self {
        Self { micros }
    }

    /// Construct a new `Time` from the specified number of milliseconds.
    ///
    /// Pancis if `millis` overflows the representable range of `Time`.
    #[inline]
    pub const fn from_millis(millis: u64) -> Self {
        Self::from_micros(millis.checked_mul(1_000).expect("duration overflow"))
    }

    /// Construct a new `Time` from the specified number of seconds.
    ///
    /// Pancis if `secs` overflows the representable range of `Time`.
    #[inline]
    pub const fn from_secs(secs: u64) -> Self {
        Self::from_micros(secs.checked_mul(1_000_000).expect("duration overflow"))
    }

    /// Get the total number of whole microseconds contained in the time span
    /// between this `Time` and [`Self::ZERO`].
    #[inline]
    pub const fn as_micros(self) -> u64 {
        self.micros
    }

    /// Get the total number of whole milliseconds contained in the time span
    /// between this `Time` and [`Self::ZERO`].
    #[inline]
    pub const fn as_millis(self) -> u64 {
        self.micros / 1_000
    }

    /// Get the total number of whole seconds contained in the time span
    /// between this `Time` and [`Self::ZERO`].
    #[inline]
    pub const fn as_secs(self) -> u64 {
        self.micros / 1_000_000
    }

    /// Get the total number of seconds contained in the time span between this
    /// `Time` and [`Self::ZERO`] as `f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use r3_core::time::Time;
    ///
    /// let dur = Time::from_micros(1_201_250_000);
    /// assert_eq!(dur.as_secs_f64(), 1201.25);
    /// ```
    #[inline]
    pub const fn as_secs_f64(self) -> f64 {
        self.micros as f64 / 1_000_000.0
    }

    /// Get the total number of seconds contained in the time span between this
    /// `Time` and [`Self::ZERO`] as `f32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use r3_core::time::Time;
    ///
    /// let dur = Time::from_micros(1_201_250_000);
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

    /// Get the duration since the origin as [`::core::time::Duration`].
    #[inline]
    pub const fn core_duration_since_origin(self) -> core::time::Duration {
        core::time::Duration::from_micros(self.micros)
    }

    /// Get the duration since the specified timestamp as
    /// [`::core::time::Duration`]. Returns `None` if `self` < `reference`.
    #[inline]
    pub const fn core_duration_since(self, reference: Self) -> Option<core::time::Duration> {
        if self.micros >= reference.micros {
            Some(core::time::Duration::from_micros(self.micros))
        } else {
            None
        }
    }

    /// Get the duration since the specified timestamp as [`Duration`]. Returns
    /// `None` if the result overflows the representable range of `Duration`.
    #[inline]
    pub const fn duration_since(self, reference: Self) -> Option<Duration> {
        Some(Duration::from_micros(
            (self.micros as i128 - reference.micros as i128)
                .try_into()
                .ok()?,
        ))
    }

    /// Advance the time by `duration` and return the result.
    #[inline]
    pub const fn wrapping_add(&self, duration: Duration) -> Self {
        Self::from_micros(self.micros.wrapping_add(duration.as_micros() as i64 as u64))
    }

    /// Put back the time by `duration` and return the result.
    #[inline]
    pub const fn wrapping_sub(&self, duration: Duration) -> Self {
        Self::from_micros(self.micros.wrapping_sub(duration.as_micros() as i64 as u64))
    }
}

impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.core_duration_since_origin().fmt(f)
    }
}

impl ops::Add<Duration> for Time {
    type Output = Self;

    /// Advance the time by `duration` and return the result.
    #[inline]
    fn add(self, rhs: Duration) -> Self::Output {
        self.wrapping_add(rhs)
    }
}

impl ops::AddAssign<Duration> for Time {
    /// Advance the time by `duration` in place.
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl ops::Sub<Duration> for Time {
    type Output = Self;

    /// Put back the time by `duration` and return the result.
    #[inline]
    fn sub(self, rhs: Duration) -> Self::Output {
        self.wrapping_sub(rhs)
    }
}

impl ops::SubAssign<Duration> for Time {
    /// Put back the time by `duration` in place.
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

/// Error type returned when a checked timestamp type conversion fails.
#[cfg(feature = "chrono_0p4")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TryFromDateTimeError(());

#[cfg(feature = "chrono_0p4")]
impl TryFrom<chrono_0p4::DateTime<chrono_0p4::Utc>> for Time {
    type Error = TryFromDateTimeError;

    /// Try to construct a `Time` from the specified `chrono_0p4::DateTime<Utc>`.
    /// Returns an error if the specified `DateTime` overflows the representable
    /// range of the destination type.
    ///
    /// The sub-microsecond part is rounded by truncating.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono_0p4::{DateTime, Utc, TimeZone};
    /// use r3_core::time::Time;
    /// assert_eq!(
    ///     Time::try_from(Utc.timestamp(4, 123_456)),
    ///     Ok(Time::from_micros(4_000_123)),
    /// );
    /// assert!(Time::try_from(Utc.timestamp(-1, 999_999_999)).is_err());
    /// ```
    fn try_from(value: chrono_0p4::DateTime<chrono_0p4::Utc>) -> Result<Self, Self::Error> {
        let secs: u64 = value
            .timestamp()
            .try_into()
            .map_err(|_| TryFromDateTimeError(()))?;

        let micros: u64 = value.timestamp_subsec_micros().into();

        Ok(Self::from_micros(
            secs.checked_mul(1_000_000)
                .and_then(|x| x.checked_add(micros))
                .ok_or(TryFromDateTimeError(()))?,
        ))
    }
}

#[cfg(feature = "chrono_0p4")]
impl TryFrom<Time> for chrono_0p4::DateTime<chrono_0p4::Utc> {
    type Error = TryFromDateTimeError;

    /// Try to construct a `chrono_0p4::DateTime<chrono_0p4::Utc>` from the specified
    /// `Time`.
    /// Returns an error if the specified `Time` overflows the representable
    /// range of the destination type.
    ///
    /// # Examples
    ///
    /// ```
    /// use chrono_0p4::{DateTime, Utc, TimeZone};
    /// use r3_core::time::Time;
    /// assert_eq!(
    ///     DateTime::try_from(Time::from_micros(123_456_789)),
    ///     Ok(Utc.timestamp(123, 456_789_000)),
    /// );
    /// assert!(
    ///     DateTime::try_from(Time::from_micros(0xffff_ffff_ffff_ffff))
    ///         .is_err()
    /// );
    /// ```
    fn try_from(value: Time) -> Result<Self, Self::Error> {
        use chrono_0p4::TimeZone;
        chrono_0p4::Utc
            .timestamp_opt(
                (value.micros / 1_000_000) as i64,
                (value.micros % 1_000_000) as u32 * 1_000,
            )
            .single()
            .ok_or(TryFromDateTimeError(()))
    }
}

// TODO: Add more tests
