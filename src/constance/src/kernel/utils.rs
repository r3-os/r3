use core::{fmt, marker::PhantomData, ops};
use tokenlock::TokenLock;

use super::{error::BadCtxError, Kernel};
use crate::utils::{intrusive_list::CellLike, Init};

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

#[derive(Clone, Copy)]
pub(super) struct CpuLockKeyhole<System> {
    _phantom: PhantomData<System>,
}

impl<System> fmt::Debug for CpuLockKeyhole<System> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CpuLockKeyhole").finish()
    }
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
pub(super) struct CpuLockCell<System, T: ?Sized>(tokenlock::TokenLock<T, CpuLockKeyhole<System>>);

impl<System, T> CpuLockCell<System, T> {
    #[allow(dead_code)]
    pub(super) const fn new(x: T) -> Self {
        Self(tokenlock::TokenLock::new(CpuLockKeyhole::INIT, x))
    }
}

impl<System: Kernel, T: fmt::Debug> fmt::Debug for CpuLockCell<System, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Ok(lock) = lock_cpu() {
            f.debug_tuple("CpuLockCell")
                .field(self.0.read(&*lock))
                .finish()
        } else {
            write!(f, "CpuLockCell(< locked >)")
        }
    }
}

impl<System, T: Init> Init for CpuLockCell<System, T> {
    const INIT: Self = Self(Init::INIT);
}

impl<System, T> ops::Deref for CpuLockCell<System, T> {
    type Target = tokenlock::TokenLock<T, CpuLockKeyhole<System>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<System, T> ops::DerefMut for CpuLockCell<System, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, Element: Clone, System: Kernel> CellLike<&'a mut CpuLockGuard<System>>
    for CpuLockCell<System, Element>
{
    type Target = Element;

    fn get(&self, key: &&'a mut CpuLockGuard<System>) -> Self::Target {
        self.read(&***key).clone()
    }
    fn set(&self, key: &mut &'a mut CpuLockGuard<System>, value: Self::Target) {
        self.replace(&mut ***key, value);
    }
}

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
