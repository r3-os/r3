#[cfg(feature = "priority_boost")]
use core::sync::atomic::Ordering;

#[cfg(feature = "priority_boost")]
use super::{task, utils};
use super::{BadContextError, BoostPriorityError, Kernel};

/// If the current context is not a task context, return `Err(BadContext)`.
pub(super) fn expect_task_context<System: Kernel>() -> Result<(), BadContextError> {
    if !System::is_task_context() {
        Err(BadContextError::BadContext)
    } else {
        Ok(())
    }
}

/// If the current context is not waitable, return `Err(BadContext)`.
pub(super) fn expect_waitable_context<System: Kernel>() -> Result<(), BadContextError> {
    if !System::is_task_context() || System::is_priority_boost_active() {
        Err(BadContextError::BadContext)
    } else {
        Ok(())
    }
}

/// Implements `Kernel::boost_priority`.
#[cfg(feature = "priority_boost")]
pub(super) fn boost_priority<System: Kernel>() -> Result<(), BoostPriorityError> {
    if System::is_cpu_lock_active()
        || !System::is_task_context()
        || System::is_priority_boost_active()
    {
        Err(BoostPriorityError::BadContext)
    } else {
        System::state()
            .priority_boost
            .store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Implements `Kernel::unboost_priority`.
#[cfg(feature = "priority_boost")]
pub(super) fn unboost_priority<System: Kernel>() -> Result<(), BoostPriorityError> {
    if !System::is_task_context() || !System::is_priority_boost_active() {
        Err(BoostPriorityError::BadContext)
    } else {
        // Acquire CPU Lock after checking other states so that
        // `drop_in_place(&mut lock)` doesn't get emitted twice
        let lock = utils::lock_cpu()?;
        System::state()
            .priority_boost
            .store(false, Ordering::Relaxed);

        // Check pending preemption
        task::unlock_cpu_and_check_preemption::<System>(lock);
        Ok(())
    }
}

/// Implements `Kernel::unboost_priority`.
#[cfg(not(feature = "priority_boost"))]
pub(super) fn unboost_priority<System: Kernel>() -> Result<(), BoostPriorityError> {
    // Priority Boost is disabled statically, so this function will always
    // return `BadContext`
    Err(BoostPriorityError::BadContext)
}
