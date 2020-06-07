use core::{marker::PhantomData, ops};
use tokenlock::TokenLock;

use super::{error::BadCtxError, Kernel};
use crate::utils::Init;

pub(super) fn expect_cpu_lock_inactive<System: Kernel>() -> Result<(), BadCtxError> {
    if System::is_cpu_lock_active() {
        Err(BadCtxError::BadCtx)
    } else {
        Ok(())
    }
}

#[non_exhaustive]
pub(super) struct CpuLockToken<System> {
    _phantom: PhantomData<System>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CpuLockKeyhole<System> {
    _phantom: PhantomData<System>,
}

// This is safe because `CpuLockToken` only can be borrowed from `CpuLockGuard`,
// and there is only one instance of `CpuLockGuard` at any point of time
unsafe impl<System> tokenlock::Token<CpuLockKeyhole<System>> for CpuLockToken<System> {
    fn eq_id(&self, _: &CpuLockKeyhole<System>) -> bool {
        true
    }
}

impl<System> Init for CpuLockKeyhole<System> {
    const INIT: Self = Self {
        _phantom: PhantomData,
    };
}

/// Cell type that can be accessed by [`CpuLockToken`] (which can be obtained
/// by [`lock_cpu`]).
pub(super) type CpuLockCell<System, T> = tokenlock::TokenLock<T, CpuLockKeyhole<System>>;

/// Attempt to enter a CPU Lock state and get an RAII guard.
/// Return `BadCtx` if the kernel is already in a CPU Lock state.
pub(super) fn lock_cpu<System: Kernel>() -> Result<CpuLockGuard<System>, BadCtxError> {
    expect_cpu_lock_inactive::<System>()?;

    // Safety: CPU Lock is currently inactive, and it's us (the kernel) who
    // are currently controlling the CPU Lock state
    unsafe {
        System::enter_cpu_lock();
    }

    // Safety: We just entered a CPU Lock state
    Ok(unsafe { assume_cpu_lock() })
}

/// Assume a CPU Lock state and get `CpuLockGuard`.
///
/// # Safety
///
/// The system must be really in a CPU Lock state.
pub(super) unsafe fn assume_cpu_lock<System: Kernel>() -> CpuLockGuard<System> {
    debug_assert!(System::is_cpu_lock_active());

    CpuLockGuard {
        token: CpuLockToken {
            _phantom: PhantomData,
        },
    }
}

/// RAII guard for a CPU Lock state.
///
/// [`CpuLockToken`] can be borrowed from this type.
pub(super) struct CpuLockGuard<System: Kernel> {
    token: CpuLockToken<System>,
}

impl<System: Kernel> Drop for CpuLockGuard<System> {
    fn drop(&mut self) {
        // Safety: CPU Lock is currently active, and it's us (the kernel) who
        // are currently controlling the CPU Lock state
        unsafe {
            System::leave_cpu_lock();
        }
    }
}

impl<System: Kernel> ops::Deref for CpuLockGuard<System> {
    type Target = CpuLockToken<System>;
    fn deref(&self) -> &Self::Target {
        &self.token
    }
}

impl<System: Kernel> ops::DerefMut for CpuLockGuard<System> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.token
    }
}
