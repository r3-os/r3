use core::{cell::UnsafeCell, fmt, marker::PhantomData};

use crate::{
    hunk::{DefaultInitTag, Hunk, HunkDefiner, HunkIniter},
    kernel::{
        mutex, traits, Cfg, LockMutexError, MarkConsistentMutexError, MutexProtocol,
        TryLockMutexError,
    },
};

/// The definer (static builder) for [`Mutex`][].
pub struct Definer<System, T, InitTag> {
    mutex: mutex::MutexDefiner<System>,
    hunk: HunkDefiner<System, UnsafeCell<T>, InitTag>,
}

/// A mutual exclusion primitive useful for protecting shared data from
/// concurrent access.
///
/// This type is implemented using [`r3::kernel::Mutex`], the low-level
/// synchronization primitive and therefore inherits its properties.
/// The important inherited properties are listed below:
///
///  - When trying to lock an abandoned mutex, the lock function will return
///    `Err(LockError::Abandoned(lock_guard))`. This state can be exited by
///    calling [`Mutex::mark_consistent`].
///
///  - Mutexes must be unlocked in a lock-reverse order. [`MutexGuard`]`::drop`
///    might panic if this is violated.
///
/// [`r3::kernel::Mutex`]: crate::kernel::Mutex
pub struct Mutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    hunk: Hunk<System, UnsafeCell<T>>,
    mutex: mutex::Mutex<System>,
}

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior

unsafe impl<System, T: 'static + Send> Send for Mutex<System, T> where
    System: traits::KernelMutex + traits::KernelStatic
{
}
unsafe impl<System, T: 'static + Send> Sync for Mutex<System, T> where
    System: traits::KernelMutex + traits::KernelStatic
{
}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure
/// is dropped, the lock will be released.
///
/// This structure is created by the [`lock`] and [`try_lock`] methods of
/// [`Mutex`].
///
/// [`lock`]: Mutex::lock
/// [`try_lock`]: Mutex::try_lock
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, System, T: 'static>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    mutex: &'a Mutex<System, T>,
    _no_send_sync: PhantomData<*mut ()>,
}

unsafe impl<System, T: 'static + Sync> Sync for MutexGuard<'_, System, T> where
    System: traits::KernelMutex + traits::KernelStatic
{
}

/// Type alias for the result of [`Mutex::lock`].
pub type LockResult<Guard> = Result<Guard, LockError<Guard>>;

/// Type alias for the result of [`Mutex::try_lock`].
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

/// Error type of [`Mutex::lock`].
#[repr(i8)]
pub enum LockError<Guard> {
    /// CPU Lock is active, or the current context is not [waitable].
    ///
    /// [waitable]: crate#contexts
    BadContext = LockMutexError::BadContext as i8,
    /// The wait operation was interrupted by [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: crate::kernel::Task::interrupt
    Interrupted = LockMutexError::Interrupted as i8,
    /// The current task already owns the mutex.
    WouldDeadlock = LockMutexError::WouldDeadlock as i8,
    /// The mutex was created with the protocol attribute having the value
    /// [`Ceiling`] and the current task's priority is higher than the
    /// mutex's priority ceiling.
    ///
    /// [`Ceiling`]: crate::kernel::MutexProtocol::Ceiling
    BadParam = LockMutexError::BadParam as i8,
    /// The previous owning task exited while holding the mutex lock. *The
    /// current task shall hold the mutex lock*, but is up to make the
    /// state consistent.
    Abandoned(Guard) = LockMutexError::Abandoned as i8,
}

impl<Guard> fmt::Debug for LockError<Guard> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadContext => "BadContext",
            Self::Interrupted => "Interrupted",
            Self::WouldDeadlock => "WouldDeadlock",
            Self::BadParam => "BadParam",
            Self::Abandoned(_) => "Abandoned",
        })
    }
}

/// Error type of [`Mutex::try_lock`].
#[repr(i8)]
pub enum TryLockError<Guard> {
    /// CPU Lock is active, or the current context is not [a task context].
    ///
    /// [a task context]: crate#contexts
    BadContext = TryLockMutexError::BadContext as i8,
    /// The current task already owns the mutex.
    WouldDeadlock = LockMutexError::WouldDeadlock as i8,
    /// The lock could not be acquire at this time because the operation would
    /// otherwise block.
    WouldBlock = TryLockMutexError::Timeout as i8,
    /// The mutex was created with the protocol attribute having the value
    /// [`Ceiling`] and the current task's priority is higher than the
    /// mutex's priority ceiling.
    ///
    /// [`Ceiling`]: crate::kernel::MutexProtocol::Ceiling
    BadParam = TryLockMutexError::BadParam as i8,
    /// The previous owning task exited while holding the mutex lock. *The
    /// current task shall hold the mutex lock*, but is up to make the
    /// state consistent.
    Abandoned(Guard) = TryLockMutexError::Abandoned as i8,
}

impl<Guard> fmt::Debug for TryLockError<Guard> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadContext => "BadContext",
            Self::WouldBlock => "WouldBlock",
            Self::WouldDeadlock => "WouldDeadlock",
            Self::BadParam => "BadParam",
            Self::Abandoned(_) => "Abandoned",
        })
    }
}

/// Error type of [`Mutex::mark_consistent`].
#[derive(Debug)]
#[repr(i8)]
pub enum MarkConsistentError {
    /// CPU Lock is active.
    BadContext = MarkConsistentMutexError::BadContext as i8,
    /// The mutex does not protect an inconsistent state.
    Consistent = MarkConsistentMutexError::BadObjectState as i8,
}

impl<System, T: 'static> Mutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Construct a `Definer` to define a mutex in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> Definer<System, T, DefaultInitTag> {
        Definer {
            mutex: mutex::Mutex::build(),
            hunk: Hunk::build(),
        }
    }
}

impl<System, T: 'static, InitTag> Definer<System, T, InitTag>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Specify the mutex's protocol. Defaults to `None` when unspecified.
    pub const fn protocol(self, protocol: MutexProtocol) -> Self {
        Self {
            mutex: self.mutex.protocol(protocol),
            ..self
        }
    }
}

impl<System, T: 'static, InitTag: HunkIniter<UnsafeCell<T>>> Definer<System, T, InitTag>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Complete the definition of a mutex, returning a reference to the mutex.
    // FIXME: `~const CfgBase` is not implied - compiler bug?
    pub const fn finish<C: ~const traits::CfgMutex<System = System> + ~const traits::CfgBase>(
        self,
        cfg: &mut Cfg<C>,
    ) -> Mutex<System, T> {
        Mutex {
            hunk: self.hunk.finish(cfg),
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<System, T: 'static> Mutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    pub fn lock(&self) -> LockResult<MutexGuard<'_, System, T>> {
        match self.mutex.lock() {
            Ok(()) => Ok(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }),
            Err(LockMutexError::BadId) => unreachable!(),
            Err(LockMutexError::BadContext) => Err(LockError::BadContext),
            Err(LockMutexError::Interrupted) => Err(LockError::Interrupted),
            Err(LockMutexError::WouldDeadlock) => Err(LockError::WouldDeadlock),
            Err(LockMutexError::BadParam) => Err(LockError::BadParam),
            Err(LockMutexError::Abandoned) => Err(LockError::Abandoned(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            })),
        }
    }

    /// Attempt to acquire the mutex.
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, System, T>> {
        match self.mutex.try_lock() {
            Ok(()) => Ok(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }),
            Err(TryLockMutexError::BadId) => unreachable!(),
            Err(TryLockMutexError::BadContext) => Err(TryLockError::BadContext),
            Err(TryLockMutexError::WouldDeadlock) => Err(TryLockError::WouldDeadlock),
            Err(TryLockMutexError::Timeout) => Err(TryLockError::WouldBlock),
            Err(TryLockMutexError::BadParam) => Err(TryLockError::BadParam),
            Err(TryLockMutexError::Abandoned) => Err(TryLockError::Abandoned(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            })),
        }
    }

    /// Mark the state protected by the mutex as consistent.
    pub fn mark_consistent(&self) -> Result<(), MarkConsistentError> {
        self.mutex.mark_consistent().map_err(|e| match e {
            MarkConsistentMutexError::BadId => unreachable!(),
            MarkConsistentMutexError::BadContext => MarkConsistentError::BadContext,
            MarkConsistentMutexError::BadObjectState => MarkConsistentError::Consistent,
        })
    }

    /// Get a raw pointer to the contained data.
    #[inline]
    pub fn get_ptr(&self) -> *mut T {
        self.hunk.get()
    }
}

impl<System, T: fmt::Debug + 'static> fmt::Debug for Mutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            Err(TryLockError::BadContext) => {
                struct BadContextPlaceholder;
                impl fmt::Debug for BadContextPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<bad context>")
                    }
                }

                f.debug_struct("Mutex")
                    .field("data", &BadContextPlaceholder)
                    .finish()
            }
            Err(TryLockError::WouldBlock | TryLockError::WouldDeadlock) => {
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
            Err(TryLockError::Abandoned(_)) => {
                struct AbandonedPlaceholder;
                impl fmt::Debug for AbandonedPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<abandoned>")
                    }
                }

                f.debug_struct("Mutex")
                    .field("data", &AbandonedPlaceholder)
                    .finish()
            }
            Err(TryLockError::BadParam) => {
                struct BadParamPlaceholder;
                impl fmt::Debug for BadParamPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<current priority too high>")
                    }
                }

                f.debug_struct("Mutex")
                    .field("data", &BadParamPlaceholder)
                    .finish()
            }
        }
    }
}

impl<System, T: fmt::Debug + 'static> fmt::Debug for MutexGuard<'_, System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<System, T: fmt::Display + 'static> fmt::Display for MutexGuard<'_, System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// The destructor of `MutexGuard` that releases the lock. It will panic if
/// CPU Lock is active.
impl<System, T: 'static> Drop for MutexGuard<'_, System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    #[inline]
    fn drop(&mut self) {
        self.mutex.mutex.unlock().unwrap();
    }
}

impl<System, T: 'static> core::ops::Deref for MutexGuard<'_, System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: `MutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &*self.mutex.hunk.get() }
    }
}

impl<System, T: 'static> core::ops::DerefMut for MutexGuard<'_, System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: `MutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &mut *self.mutex.hunk.get() }
    }
}
