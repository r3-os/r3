use core::{cell::UnsafeCell, fmt, marker::PhantomData};

use crate::{
    kernel::{cfg::CfgBuilder, Hunk, PollSemaphoreError, Semaphore, WaitSemaphoreError},
    prelude::*,
};

/// A mutual exclusion primitive useful for protecting shared data from
/// concurrent access.
///
/// This type is currently implemented using [`Semaphore`]. It will be
/// upgraded to a real mutex (with priority inversion prevention) in a
/// future version of Constance.
///
/// [`Semaphore`]: crate::kernel::Semaphore
pub struct Mutex<System, T> {
    hunk: Hunk<System, UnsafeCell<T>>,
    sem: Semaphore<System>,
    _phantom: PhantomData<(System, T)>,
}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure
/// is dropped, the lock will be released.
///
/// This structure is created by the [`lock`] and [`try_lock`] methods of
/// [`Mutex`].
///
/// [`lock`]: Mutex::lock
/// [`try_lock`]: Mutex::try_lock
pub struct MutexGuard<'a, System: Kernel, T: 'static> {
    mutex: &'a Mutex<System, T>,
    _no_send_sync: PhantomData<*mut ()>,
}

unsafe impl<System: Kernel, T: 'static + Sync> Sync for MutexGuard<'_, System, T> {}

/// Error type of [`Mutex::lock`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i8)]
pub enum LockError {
    /// CPU Lock is active, the current context is not [waitable], or the
    /// current context is not [a task context].
    ///
    /// [waitable]: crate#contexts
    /// [a task context]: crate#contexts
    BadContext = WaitSemaphoreError::BadContext as i8,
    /// The wait operation was interrupted by [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: crate::kernel::Task::interrupt
    Interrupted = WaitSemaphoreError::Interrupted as i8,
}

/// Error type of [`Mutex::try_lock`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i8)]
pub enum TryLockError {
    /// CPU Lock is active.
    BadContext = PollSemaphoreError::BadContext as i8,
    /// The lock could not be acquire at this time because the operation would
    /// otherwise block.
    WouldBlock = PollSemaphoreError::Timeout as i8,
}

impl<System: Kernel, T: 'static + Init> Mutex<System, T> {
    /// Construct a `Mutex`. The content is initialized with [`Init`].
    ///
    /// This is a configuration function. Call this method from your app's
    /// configuration function.
    pub const fn new(b: &mut CfgBuilder<System>) -> Self {
        Self {
            hunk: Hunk::<_, UnsafeCell<T>>::build().finish(b),
            sem: Semaphore::build().initial(1).maximum(1).finish(b),
            _phantom: PhantomData,
        }
    }
}

impl<System: Kernel, T: 'static> Mutex<System, T> {
    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    pub fn lock(&self) -> Result<MutexGuard<'_, System, T>, LockError> {
        self.sem.wait_one().map_err(|e| match e {
            WaitSemaphoreError::BadId => unreachable!(),
            WaitSemaphoreError::BadContext => LockError::BadContext,
            WaitSemaphoreError::Interrupted => LockError::Interrupted,
        })?;
        Ok(MutexGuard {
            mutex: self,
            _no_send_sync: PhantomData,
        })
    }

    /// Attempt to acquire the mutex.
    pub fn try_lock(&self) -> Result<MutexGuard<'_, System, T>, TryLockError> {
        // A real mutex can't be locked by an interrupt handler. We emulate it
        // by a semaphore at this time, so we need to check whether this
        // condition is violated
        if !System::is_task_context() {
            return Err(TryLockError::BadContext);
        }

        self.sem.poll_one().map_err(|e| match e {
            PollSemaphoreError::BadId => unreachable!(),
            PollSemaphoreError::BadContext => TryLockError::BadContext,
            PollSemaphoreError::Timeout => TryLockError::WouldBlock,
        })?;
        Ok(MutexGuard {
            mutex: self,
            _no_send_sync: PhantomData,
        })
    }

    /// Get a raw pointer to the contained data.
    #[inline]
    pub fn get_ptr(&self) -> *mut T {
        self.hunk.get()
    }
}

impl<System: Kernel, T: fmt::Debug + 'static> fmt::Debug for Mutex<System, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            Err(TryLockError::BadContext) => {
                struct BadContextPlaceholder;
                impl fmt::Debug for BadContextPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<CPU context active>")
                    }
                }

                f.debug_struct("Mutex")
                    .field("data", &BadContextPlaceholder)
                    .finish()
            }
            Err(TryLockError::WouldBlock) => {
                struct LockedPlaceholder;
                impl fmt::Debug for LockedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<locked>")
                    }
                }

                f.debug_struct("Mutex")
                    .field("data", &LockedPlaceholder)
                    .finish()
            }
        }
    }
}

impl<System: Kernel, T: fmt::Debug + 'static> fmt::Debug for MutexGuard<'_, System, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<System: Kernel, T: fmt::Display + 'static> fmt::Display for MutexGuard<'_, System, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// The destructor of `MutexGuard` that releases the lock. It will panic if
/// CPU Lock is active.
impl<System: Kernel, T: 'static> Drop for MutexGuard<'_, System, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.sem.signal_one().unwrap();
    }
}

impl<System: Kernel, T: 'static> core::ops::Deref for MutexGuard<'_, System, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: `MutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &*self.mutex.hunk.get() }
    }
}

impl<System: Kernel, T: 'static> core::ops::DerefMut for MutexGuard<'_, System, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: `MutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &mut *self.mutex.hunk.get() }
    }
}
