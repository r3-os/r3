use core::{marker::PhantomData, num::NonZeroUsize};

use crate::{
    kernel::{cfg::CfgBuilder, timeout, timer, Port},
    time::Duration,
};

impl<System: Port> timer::Timer<System> {
    /// Construct a `CfgTimerBuilder` to define a timer in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> CfgTimerBuilder<System> {
        CfgTimerBuilder::new()
    }
}

/// Configuration builder type for [`Timer`].
///
/// [`Timer`]: crate::kernel::Timer
#[must_use = "must call `finish()` to complete registration"]
pub struct CfgTimerBuilder<System> {
    _phantom: PhantomData<System>,
    start: Option<fn(usize)>,
    param: usize,
    delay: Option<Duration>,
    period: Option<Duration>,
    active: bool,
}

impl<System: Port> CfgTimerBuilder<System> {
    const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            start: None,
            param: 0,
            delay: None,
            period: None,
            active: false,
        }
    }

    /// [**Required**] Specify the timer's entry point. It will be called
    /// in an interrupt context.
    pub const fn start(self, start: fn(usize)) -> Self {
        Self {
            start: Some(start),
            ..self
        }
    }

    /// Specify the parameter to `start`. Defaults to `0`.
    pub const fn param(self, param: usize) -> Self {
        Self { param, ..self }
    }

    /// Specify whether the timer should be started at system startup.
    /// Defaults to `false` (don't activate).
    pub const fn active(self, active: bool) -> Self {
        Self { active, ..self }
    }

    /// Specify the initial [delay].
    /// Defaults to `None` (infinity; the timer will never fire).
    ///
    /// [delay]: crate::kernel::Timer::set_delay
    pub const fn delay(self, delay: Duration) -> Self {
        Self {
            delay: Some(delay),
            ..self
        }
    }

    /// Specify the initial [period].
    /// Defaults to `None` (infinity; the timer will stop firing after the next
    /// tick).
    ///
    /// [period]: crate::kernel::Timer::set_period
    pub const fn period(self, period: Duration) -> Self {
        Self {
            period: Some(period),
            ..self
        }
    }

    /// Complete the definition of a timer, returning a reference to the timer.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> timer::Timer<System> {
        let inner = &mut cfg.inner;

        let period = if let Some(period) = self.period {
            if let Ok(x) = timeout::time32_from_duration(period) {
                x
            } else {
                panic!("`period` must not be negative");
            }
        } else {
            // Defaults to `None`
            timeout::BAD_DURATION32
        };

        let delay = if let Some(delay) = self.delay {
            if let Ok(x) = timeout::time32_from_duration(delay) {
                x
            } else {
                panic!("`delay` must not be negative");
            }
        } else {
            // Defaults to `None`
            timeout::BAD_DURATION32
        };

        inner.timers.push(CfgBuilderTimer {
            start: if let Some(x) = self.start {
                x
            } else {
                panic!("`start` (timer callback function) is not specified")
            },
            param: self.param,
            delay,
            period,
            active: self.active,
        });

        unsafe { timer::Timer::from_id(NonZeroUsize::new_unchecked(inner.timers.len())) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderTimer {
    start: fn(usize),
    param: usize,
    delay: timeout::Time32,
    period: timeout::Time32,
    active: bool,
}

impl Clone for CfgBuilderTimer {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            param: self.param,
            delay: self.delay,
            period: self.period,
            active: self.active,
        }
    }
}

impl Copy for CfgBuilderTimer {}

impl CfgBuilderTimer {
    pub const fn to_state<System: Port>(
        &self,
        attr: &'static timer::TimerAttr<System>,
    ) -> timer::TimerCb<System> {
        timer::TimerCb { attr }
    }

    pub const fn to_attr<System: Port>(&self) -> timer::TimerAttr<System> {
        timer::TimerAttr {
            entry_point: self.start,
            entry_param: self.param,
            _phantom: PhantomData,
        }
    }
}
