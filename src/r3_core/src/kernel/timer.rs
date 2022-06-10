//! Timers
use core::{fmt, hash};

use super::{
    raw, raw_cfg, Cfg, SetTimerDelayError, SetTimerPeriodError, StartTimerError, StopTimerError,
};
use crate::{
    closure::{Closure, IntoClosureConst},
    time::Duration,
    utils::{Init, PhantomInvariant},
};

// ----------------------------------------------------------------------------

define_object! {
/// Represents a single timer in a system.
///
#[doc = common_doc_owned_handle!()]
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** A similar concept exists in almost
/// > every operating system.
///
/// [`RawTimerId`]: raw::KernelTimer::RawTimerId
///
/// <div class="toc-header"></div>
///
///  - [Timer States](#timer-states)
///  - [Timer Scheduling](#timer-scheduling)
///      - [Overdue Timers](#overdue-timers)
///      - [Start/Stop](#startstop)
///      - [Dynamic Period](#dynamic-period)
///      - [Infinite Delay and/or Period](#infinite-delay-andor-period)
///  - [Examples](#examples)
///      - [Periodic Timer](#periodic-timer)
///      - [One-Shot Timer](#one-shot-timer)
///  - [Methods](#implementations)  <!-- this section is generated by rustdoc -->
///
/// # Timer States
///
/// A timer may be in one of the following states:
///
///  - **Dormant** — The timer is not running and can be [started].
///
///  - **Active** — The timer is running and can be [stopped].
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
/// .---------------.             start              .--------------.
/// |               | -----------------------------> |              |
/// |    Dormant    |                                |    Active    |
/// |               | <----------------------------- |              |
/// '---------------'              stop              '--------------'
/// ```
)]
///
/// </center>
///
/// [started]: TimerMethods::start
/// [stopped]: TimerMethods::stop
///
/// # Timer Scheduling
///
/// The scheduling of a timer is determined by two state variables:
///
///  - The [delay] is an optional non-negative [duration] value
///    (`Option<Duration>`) that specifies the minimum period of time before the
///    callback function gets called.
///
///    If the delay is `None`, it's treated as infinity and the function will
///    never execute.
///
///    While a timer is active, this value decreases at a steady rate. If the
///    system can't process a timer for an extended period of time, this value
///    might temporarily fall negative.
///
///  - The [period] is an optional non-negative duration value. On expiration,
///    the system adds this value to the timer's delay.
///
/// [delay]: TimerMethods::set_delay
/// [period]: TimerMethods::set_period
/// [duration]: crate::time::Duration
///
/// ## Overdue Timers
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
/// ​
/// Higher-priority interrupt               __________
/// or CPU Lock                            |__________|
///
///                               _____                _____ _____    _____
/// Timer callback               |_____|              |_____|_____|  |_____|
///                              1                    2     3        4
///
/// Delay     7  6  5  4  3  2  1  4  3  2  1  0 -1 -2  1  0  3  2  1  4  3  2  1
///         ├──┬──┬──┬──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┤
///         ↑    initial delay   1   period  2   period  3   period  4   period
///     activated
/// ​
/// ```
)]
///
/// </center>
///
/// When scheduling a next tick, the system takes the observed timer handling
/// latency into account and makes the new delay shorter than the period as
/// needed to ensure that the callback function is called in a steady rate. This
/// behavior is illustrated by the above figure. This is accomplished by adding
/// the specified period to the timer's absolute arrival time instead of
/// recalculating the arrival time based on the current system time. The delay
/// is a difference between the current system time and the arrival time.
///
/// Note that the system does not impose any limit on the extent of this
/// behavior. To put this simply, *if one second elapses, the system makes one
/// second worth of calls no matter what.*
/// If a periodic timer's callback function couldn't complete within the
/// timer's period, the timer latency would steadily increase until it reaches
/// the point where various internal assumptions get broken. While the system is
/// processing overdue calls, the timer interrupt handler might not return. Some
/// kernel timer drivers (most notably the Arm-M tickful SysTick driver) have
/// much lower tolerance for this.
/// To avoid this catastrophic situation, an application should take the
/// precautions shown below:
///
///  - Don't perform an operation that might take an unbounded time in a timer
///    callback function.
///
///  - Off-load time-consuming operations to a task, which is [activated] or
///    [unparked] by a timer callback function.
///
///  - Don't specify zero as period unless you know what you are doing.
///
///  - Keep your target platform's performance characteristics in your mind.
///
/// [activated]: crate::kernel::task::TaskMethods::activate
/// [unparked]: crate::kernel::task::TaskMethods::unpark
///
/// ## Start/Stop
///
/// When a timer is [stopped], the timer will not fire anymore and the delay
/// remains stationary at the captured value. If the captured value is negative,
/// it's rounded to zero. This means that if there are more than one outstanding
/// call at the moment of stopping, they will be dropped.
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                   _____       _____                   _____       _____
/// Timer callback   |_____|     |_____|                 |_____|     |_____|
///                  1           2                       3           4
///
///                  ├──┬──┬──┬──┼──┤╴╴╴╴╴╴╴╴╴╴╴├──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┤
///                  1           2  ↑           ↑        3           4
///                               stop        start
///
///                   _____ _____ _____ _____         _____ _____ _____
/// Timer callback   |_____|_____|_____|_____|       |_____|_____|_____|
///                  1     2     3     4             5     6     7
///
///                  ├──┼──┼──┼──┼──┼──┼─┤╴╴╴╴╴╴╴╴╴╴╴├──┼──┼──┼──┼──┼──┤
///                  1  2  3  4  x  x  x ↑           ↑5 6  7  8  9  10
///                                     stop       start
/// ​
/// ```
)]
///
/// </center>
///
/// Another way to stop a timer is to [set the delay or the period to `None`
/// (infinity)](#infinite-delay-andor-period).
///
/// [stopped]: TimerMethods::stop
///
/// ## Dynamic Period
///
/// The period can be changed anytime. The system reads it before calling a
/// timer callback function and adds it to the timer's current delay value.
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                   _____       _____       _____    _____    _____
/// Timer callback   |_____|     |_____|     |_____|  |_____|  |_____|
///                  1           2           3        4        5
///
/// Delay             4  3  2  1  4  3  2  1  3  2  1  3  2  1  3  2  1
///                  ├──┬──┬──┬──┼──┬──┬──┬──┤
///                  1           2  ↑
///              period = 4     period ← 3   ├──┬──┬──┼──┬──┬──┼──┬──┬──┤
///                                          3        4        5
///
///                   _____ _____ _____ _____ _____ _____ _____       _____
/// Timer callback   |_____|_____|_____|_____|_____|_____|_____|     |_____|
///                  1     2     3     4     5     6     7           8
///
/// Delay             1  0  0  -1 -1 -2 -2 -3 0  -1 2  1  4  3  2  1  4
///                  ├──┼──┼──┼──┼──┼──┼┤
///                  1  2  3  4  x  x  x↑
///              period = 1      ├──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┼──┤
///                              5      ↑    6           7           8
///                                period ← 4
/// ​
/// ```
)]
///
/// </center>
///
/// It might be tricky to understand the outcome of changing the period when
/// there are overdue calls. It could be explained in this way: *If there are
/// one second worth of calls pending, there will still be one second worth of
/// calls pending after changing the period.*
///
/// ## Infinite Delay and/or Period
///
/// If [`delay` is set] to `None` (infinity), the timer will stop firing. Note
/// that the timer is still in the Active state, and the correct way to restart
/// this timer is to reset the delay to a finite value.
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                   _____                               _____       _____
/// Timer callback   |_____|                             |_____|     |_____|
///                  1                                   2           3
///
///                  ├──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┼──┬──┬──┬──┤
///                  1  ↑                       ↑        2           3
///                delay ← None              delay ← 3
/// ​
/// ```
)]
///
/// </center>
///
/// If [`period` is set] to `None` instead, the timer will stop firing after the
/// next tick.
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                   _____       _____                   _____       _____
/// Timer callback   |_____|     |_____|                 |_____|     |_____|
///                  1           2                       3           4
///
///                  ├──┬──┬──┬──┤              ├──┬──┬──┼──┬──┬──┬──┤
///                  1  ↑                       ↑        3           4
///               period ← None  ├──┬──┬──┬──┬──┤
///                              2              ↑
///                                         period ← 4
///                                          delay ← 3
/// ​
/// ```
)]
///
/// </center>
///
/// [`delay` is set]: TimerMethods::set_delay
/// [`period` is set]: TimerMethods::set_period
///
/// # Examples
///
/// ## Periodic Timer
///
/// ```rust
/// # #![feature(const_trait_impl)]
/// # #![feature(const_mut_refs)]
/// use r3_core::{kernel::{Cfg, StaticTimer, traits}, time::Duration};
///
/// const fn configure<C>(b: &mut Cfg<C>) -> StaticTimer<C::System>
/// where
///     C: ~const traits::CfgTimer,
/// {
///     StaticTimer::define()
///         .delay(Duration::from_millis(70))
///         .period(Duration::from_millis(40))
///         .active(true)
///         .start(|| dbg!())
///         .finish(b)
/// }
/// ```
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                            _____       _____       _____       _____
/// Timer callback            |_____|     |_____|     |_____|     |_____|
///                           1           2           3           4
///
///      ├──┬──┬──┬──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┼──┬──┬──┬──┤
///      ↑        70ms        1   40ms    2   40ms    3   40ms    4   40ms
/// system boot
/// ​
/// ```
)]
///
/// </center>
///
/// ## One-Shot Timer
///
/// ```rust
/// # #![feature(const_trait_impl)]
/// # #![feature(const_mut_refs)]
/// use r3_core::{kernel::{Cfg, StaticTimer, traits, prelude::*}, time::Duration};
///
/// const fn configure<C>(b: &mut Cfg<C>) -> StaticTimer<C::System>
/// where
///     C: ~const traits::CfgTimer,
/// {
///     StaticTimer::define()
///         .active(true)
///         .start(|| dbg!())
///         .finish(b)
/// }
/// ```
///
/// [Reset the delay] to schedule a call.
///
/// ```rust
/// use r3_core::{kernel::{TimerRef, traits, prelude::*}, time::Duration};
///
/// fn sched<System: traits::KernelTimer>(timer: TimerRef<'_, System>) {
///     timer.set_delay(Some(Duration::from_millis(40))).unwrap();
/// }
/// ```
///
/// <center>
///
#[doc = svgbobdoc::transform!(
/// ```svgbob
///                         _____                            _____
/// Timer callback         |_____|                          |_____|
///                        1                                2
///
///      ├──┬──┬──┬──┬──┬──┼──┬──┬──┬──┬──┬──┬──┬──┬──┬──┬──┼──┬──┬──┬──┤
///            ↑   40ms    1           ↑        ↑   40ms    2
///          sched                   sched    sched
/// ​
/// ```
)]
///
/// </center>
///
/// [Reset the delay]: TimerMethods::set_delay
///
#[doc = include_str!("../common.md")]
pub struct Timer<System: _>(System::RawTimerId);

/// Represents a single borrowed timer in a system.
#[doc = include_str!("../common.md")]
pub struct TimerRef<System: raw::KernelTimer>(_);

pub type StaticTimer<System>;

pub trait TimerHandle {}
pub trait TimerMethods {}
}

impl<System: raw::KernelTimer> StaticTimer<System> {
    /// Construct a `TimerDefiner` to define a timer in [a
    /// configuration function](crate#static-configuration).
    pub const fn define() -> TimerDefiner<System> {
        TimerDefiner::new()
    }
}

/// The supported operations on [`TimerHandle`].
#[doc = include_str!("../common.md")]
pub trait TimerMethods: TimerHandle {
    /// Start the timer (transition it into the Active state).
    ///
    /// This method has no effect if the timer is already in the Active state.
    #[inline]
    fn start(&self) -> Result<(), StartTimerError> {
        // Safety: `Timer` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelTimer>::raw_timer_start(self.id()) }
    }

    /// Stop the timer (transition it into the Dormant state).
    ///
    /// This method has no effect if the timer is already in the Dormant state.
    #[inline]
    fn stop(&self) -> Result<(), StopTimerError> {
        // Safety: `Timer` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelTimer>::raw_timer_stop(self.id()) }
    }

    /// Set the duration before the next tick.
    ///
    /// If the timer is currently in the Dormant state, this method specifies
    /// the duration between the next activation and the first tick
    /// following the activation.
    ///
    /// `None` means infinity (the timer will never fire).
    #[inline]
    fn set_delay(&self, delay: Option<Duration>) -> Result<(), SetTimerDelayError> {
        // Safety: `Timer` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelTimer>::raw_timer_set_delay(self.id(), delay) }
    }

    /// Set the timer period, which is a quantity to be added to the timer's
    /// absolute arrival time on every tick.
    ///
    /// `None` means infinity.
    #[inline]
    fn set_period(&self, period: Option<Duration>) -> Result<(), SetTimerPeriodError> {
        // Safety: `Timer` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelTimer>::raw_timer_set_period(self.id(), period) }
    }
}

impl<T: TimerHandle> TimerMethods for T {}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`TimerRef`].
#[must_use = "must call `finish()` to complete registration"]
pub struct TimerDefiner<System> {
    _phantom: PhantomInvariant<System>,
    start: Option<Closure>,
    delay: Option<Duration>,
    period: Option<Duration>,
    active: bool,
}

impl<System: raw::KernelTimer> TimerDefiner<System> {
    const fn new() -> Self {
        Self {
            _phantom: Init::INIT,
            start: None,
            delay: None,
            period: None,
            active: false,
        }
    }

    /// \[**Required**\] Specify the timer's entry point. It will be called
    /// in an interrupt context.
    pub const fn start<C: ~const IntoClosureConst>(self, start: C) -> Self {
        Self {
            start: Some(start.into_closure_const()),
            ..self
        }
    }

    /// Specify whether the timer should be started at system startup.
    /// Defaults to `false` (don't activate).
    pub const fn active(self, active: bool) -> Self {
        Self { active, ..self }
    }

    /// Specify the initial [delay].
    /// Defaults to `None` (infinity; the timer will never fire).
    ///
    /// [delay]: TimerMethods::set_delay
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
    /// [period]: TimerMethods::set_period
    pub const fn period(self, period: Duration) -> Self {
        Self {
            period: Some(period),
            ..self
        }
    }

    /// Complete the definition of a mutex, returning a reference to the
    /// mutex.
    pub const fn finish<C: ~const raw_cfg::CfgTimer<System = System>>(
        self,
        c: &mut Cfg<C>,
    ) -> StaticTimer<System> {
        let id = c.raw().timer_define(
            raw_cfg::TimerDescriptor {
                phantom: Init::INIT,
                start: self
                    .start
                    .expect("`start` (timer callback function) is not specified"),
                delay: self.delay,
                period: self.period,
                active: self.active,
            },
            (),
        );
        unsafe { TimerRef::from_id(id) }
    }
}
