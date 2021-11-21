#[cfg(feature = "priority_boost")]
use core::sync::atomic::Ordering;
use r3::kernel::{traits::KernelBase as _, BoostPriorityError};

use crate::{error::BadContextError, KernelTraits, System};
#[cfg(feature = "priority_boost")]
use crate::{klock, task};

/// If the current context is not a task context, return `Err(BadContext)`.
pub(super) fn expect_task_context<Traits: KernelTraits>() -> Result<(), BadContextError> {
    if !Traits::is_task_context() {
        Err(BadContextError::BadContext)
    } else {
        Ok(())
    }
}

/// If the current context is not waitable, return `Err(BadContext)`.
pub(super) fn expect_waitable_context<Traits: KernelTraits>() -> Result<(), BadContextError> {
    if !Traits::is_task_context() || System::<Traits>::is_priority_boost_active() {
        Err(BadContextError::BadContext)
    } else {
        Ok(())
    }
}

/// Implements `Kernel::boost_priority`.
#[cfg(feature = "priority_boost")]
pub(super) fn boost_priority<Traits: KernelTraits>() -> Result<(), BoostPriorityError> {
    if Traits::is_cpu_lock_active()
        || !Traits::is_task_context()
        || System::<Traits>::is_priority_boost_active()
    {
        Err(BoostPriorityError::BadContext)
    } else {
        Traits::state()
            .priority_boost
            .store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Implements `Kernel::unboost_priority`.
#[cfg(feature = "priority_boost")]
pub(super) fn unboost_priority<Traits: KernelTraits>() -> Result<(), BoostPriorityError> {
    if !Traits::is_task_context() || !System::<Traits>::is_priority_boost_active() {
        Err(BoostPriorityError::BadContext)
    } else {
        // Acquire CPU Lock after checking other states so that
        // `drop_in_place(&mut lock)` doesn't get emitted twice
        let lock = klock::lock_cpu()?;
        Ttraits::state()
            .priority_boost
            .store(false, Ordering::Relaxed);

        // Check pending preemption
        task::unlock_cpu_and_check_preemption::<System>(lock);
        Ok(())
    }
}

/// Implements `Kernel::unboost_priority`.
#[cfg(not(feature = "priority_boost"))]
pub(super) fn unboost_priority<Traits: KernelTraits>() -> Result<(), BoostPriorityError> {
    // Priority Boost is disabled statically, so this function will always
    // return `BadContext`
    Err(BoostPriorityError::BadContext)
}
