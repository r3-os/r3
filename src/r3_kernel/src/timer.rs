//! Timers
use core::{fmt, marker::PhantomData, mem::ManuallyDrop};
use r3_core::{
    kernel::{traits, SetTimerDelayError, SetTimerPeriodError, StartTimerError, StopTimerError},
    time::Duration,
    utils::Init,
};

use crate::{
    error::NoAccessError,
    klock::{assume_cpu_lock, lock_cpu, CpuLockCell, CpuLockGuard, CpuLockTokenRefMut},
    timeout,
    utils::pin::static_pin,
    Id, KernelCfg2, KernelTraits, System,
};

pub(super) type TimerId = Id;

impl<Traits: KernelTraits> System<Traits> {
    /// Get the [`TimerCb`] for the specified raw ID.
    ///
    /// # Safety
    ///
    /// See [`crate::bad_id`].
    #[inline]
    unsafe fn timer_cb(this: TimerId) -> Result<&'static TimerCb<Traits>, NoAccessError> {
        Traits::get_timer_cb(this.get() - 1).ok_or_else(|| unsafe { crate::bad_id::<Traits>() })
    }
}

unsafe impl<Traits: KernelTraits> traits::KernelTimer for System<Traits> {
    type RawTimerId = TimerId;

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_timer_start(this: TimerId) -> Result<(), StartTimerError> {
        let mut lock = lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let timer_cb = unsafe { Self::timer_cb(this)? };
        start_timer(lock.borrow_mut(), timer_cb);
        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_timer_stop(this: TimerId) -> Result<(), StopTimerError> {
        let mut lock = lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let timer_cb = unsafe { Self::timer_cb(this)? };
        stop_timer(lock.borrow_mut(), timer_cb);
        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_timer_set_delay(
        this: TimerId,
        delay: Option<Duration>,
    ) -> Result<(), SetTimerDelayError> {
        let time32 = if let Some(x) = delay {
            timeout::time32_from_duration(x)?
        } else {
            timeout::BAD_DURATION32
        };
        let mut lock = lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let timer_cb = unsafe { Self::timer_cb(this)? };
        set_timer_delay(lock.borrow_mut(), timer_cb, time32);
        Ok(())
    }

    #[cfg_attr(not(feature = "inline_syscall"), inline(never))]
    unsafe fn raw_timer_set_period(
        this: TimerId,
        period: Option<Duration>,
    ) -> Result<(), SetTimerPeriodError> {
        let time32 = if let Some(x) = period {
            timeout::time32_from_duration(x)?
        } else {
            timeout::BAD_DURATION32
        };
        let mut lock = lock_cpu::<Traits>()?;
        // Safety: The caller is responsible for providing a valid object ID
        let timer_cb = unsafe { Self::timer_cb(this)? };
        set_timer_period(lock.borrow_mut(), timer_cb, time32);
        Ok(())
    }
}

/// *Timer control block* - the state data of a timer.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by a macro.
#[doc(hidden)]
pub struct TimerCb<Traits: KernelCfg2> {
    /// The static properties of the timer.
    pub(super) attr: &'static TimerAttr<Traits>,

    /// The timeout object for the timer.
    ///
    ///  - If the delay is `Some(_)` and the timer is in the Active state, the
    ///    timeout object is linked. The delay is implicitly defined in this
    ///    case.
    ///
    ///  - If the delay is `None` or the timer is in the Dormant state, the
    ///    timeout object is unlinked. The delay can be retrieved by
    ///    [`timeout::Timeout::at_raw`].
    ///
    // FIXME: `!Drop` is a requirement of `array_item_from_fn!` that ideally
    //        should be removed
    pub(super) timeout: ManuallyDrop<timeout::Timeout<Traits>>,

    /// `true` iff the timer is in the Active state.
    pub(super) active: CpuLockCell<Traits, bool>,

    pub(super) period: CpuLockCell<Traits, timeout::Time32>,
}

impl<Traits: KernelTraits> Init for TimerCb<Traits> {
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self = Self {
        attr: &Init::INIT,
        timeout: Init::INIT,
        active: Init::INIT,
        period: Init::INIT,
    };
}

impl<Traits: KernelTraits> fmt::Debug for TimerCb<Traits> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TimerCb")
            .field("self", &(self as *const _))
            .field("attr", &self.attr)
            .field("timeout", &self.timeout)
            .field("active", &self.active)
            .field("period", &self.period)
            .finish()
    }
}

/// The static properties of a timer.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by a macro.
#[doc(hidden)]
pub struct TimerAttr<Traits> {
    /// The entry point of the timer.
    ///
    /// # Safety
    ///
    /// This is only meant to be used by a kernel port, as a timer callback,
    /// not by user code. Using this in other ways may cause an undefined
    /// behavior.
    pub(super) entry_point: fn(usize),

    /// The parameter supplied for `entry_point`.
    pub(super) entry_param: usize,

    /// The initial state of the timer.
    pub(super) init_active: bool,

    pub(super) _phantom: PhantomData<Traits>,
}

impl<Traits> Init for TimerAttr<Traits> {
    const INIT: Self = Self {
        entry_point: |_| {},
        entry_param: 0,
        init_active: false,
        _phantom: PhantomData,
    };
}

impl<Traits: KernelTraits> fmt::Debug for TimerAttr<Traits> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TimerAttr")
            .field("entry_point", &self.entry_point)
            .field("entry_param", &self.entry_param)
            .finish()
    }
}

/// Initialize a timer at boot time.
pub(super) fn init_timer<Traits: KernelTraits>(
    mut lock: CpuLockTokenRefMut<'_, Traits>,
    timer_cb: &'static TimerCb<Traits>,
) {
    if timer_cb.attr.init_active {
        // Get the initial delay value
        let delay = timer_cb.timeout.at_raw(lock.borrow_mut());

        if delay != timeout::BAD_DURATION32 {
            // Schedule the first tick
            timeout::insert_timeout(lock.borrow_mut(), static_pin(&timer_cb.timeout));
        }

        timer_cb.active.replace(&mut *lock, true);
    }
}

/// The core portion of [`Timer::start`].
fn start_timer<Traits: KernelTraits>(
    mut lock: CpuLockTokenRefMut<'_, Traits>,
    timer_cb: &'static TimerCb<Traits>,
) {
    if timer_cb.active.get(&*lock) {
        return;
    }

    // Get the current delay value
    let delay = timer_cb.timeout.at_raw(lock.borrow_mut());

    if delay != timeout::BAD_DURATION32 {
        // Schedule the next tick
        timer_cb
            .timeout
            .set_expiration_after(lock.borrow_mut(), delay);
        timeout::insert_timeout(lock.borrow_mut(), static_pin(&timer_cb.timeout));
    }

    timer_cb.active.replace(&mut *lock, true);
}

/// The core portion of [`Timer::stop`].
fn stop_timer<Traits: KernelTraits>(
    mut lock: CpuLockTokenRefMut<'_, Traits>,
    timer_cb: &TimerCb<Traits>,
) {
    if timer_cb.timeout.is_linked(lock.borrow_mut()) {
        debug_assert!(timer_cb.active.get(&*lock));

        // Capture the current delay value
        let delay = timer_cb
            .timeout
            .saturating_duration_until_timeout(lock.borrow_mut());

        // Unlink the timeout
        timeout::remove_timeout(lock.borrow_mut(), &timer_cb.timeout);

        // Store the captured delay value
        timer_cb.timeout.set_at_raw(lock.borrow_mut(), delay);
    }

    timer_cb.active.replace(&mut *lock, false);
}

/// The core portion of [`Timer::set_delay`].
fn set_timer_delay<Traits: KernelTraits>(
    mut lock: CpuLockTokenRefMut<'_, Traits>,
    timer_cb: &'static TimerCb<Traits>,
    delay: timeout::Time32,
) {
    let is_active = timer_cb.active.get(&*lock);

    if timer_cb.timeout.is_linked(lock.borrow_mut()) {
        timeout::remove_timeout(lock.borrow_mut(), &timer_cb.timeout);
    }

    if is_active && delay != timeout::BAD_DURATION32 {
        timer_cb
            .timeout
            .set_expiration_after(lock.borrow_mut(), delay);
        timeout::insert_timeout(lock.borrow_mut(), static_pin(&timer_cb.timeout));
    } else {
        timer_cb.timeout.set_at_raw(lock.borrow_mut(), delay);
    }
}

/// The core portion of [`Timer::set_period`].
fn set_timer_period<Traits: KernelTraits>(
    mut lock: CpuLockTokenRefMut<'_, Traits>,
    timer: &TimerCb<Traits>,
    period: timeout::Time32,
) {
    timer.period.replace(&mut *lock, period);
}

/// The timeout callback function for a timer. This function should be
/// registered as a callback function when initializing [`TimerCb::timeout`].
///
/// `i` is an index into [`super::KernelCfg2::timer_cb_pool`].
pub(super) fn timer_timeout_handler<Traits: KernelTraits>(
    i: usize,
    mut lock: CpuLockGuard<Traits>,
) -> CpuLockGuard<Traits> {
    let timer_cb = Traits::get_timer_cb(i).unwrap();

    // Schedule the next tick
    debug_assert!(!timer_cb.timeout.is_linked(lock.borrow_mut()));
    debug_assert!(timer_cb.active.get(&*lock));

    let period = timer_cb.period.get(&*lock);
    if period == timeout::BAD_DURATION32 {
        timer_cb
            .timeout
            .set_at_raw(lock.borrow_mut(), timeout::BAD_DURATION32);
    } else {
        timer_cb
            .timeout
            .adjust_expiration(lock.borrow_mut(), period);
        timeout::insert_timeout(lock.borrow_mut(), static_pin(&timer_cb.timeout));
    }

    // Release CPU Lock before calling the application-provided callback
    // function
    drop(lock);

    let TimerAttr {
        entry_point,
        entry_param,
        ..
    } = timer_cb.attr;
    entry_point(*entry_param);

    // Re-acquire CPU Lock
    lock_cpu().unwrap_or_else(|_| unsafe { assume_cpu_lock() })
}
