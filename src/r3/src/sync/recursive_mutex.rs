use core::{cell::Cell, fmt, marker::PhantomData};

use crate::{
    hunk::{DefaultInitTag, Hunk, HunkDefiner, HunkIniter},
    kernel::{
        mutex, traits, Cfg, LockMutexError, MarkConsistentMutexError, MutexProtocol,
        TryLockMutexError,
    },
    utils::Init,
};

/// The definer (static builder) for [`RecursiveMutex`][].
pub struct Definer<System, T, InitTag> {
    mutex: mutex::MutexDefiner<System>,
    hunk: HunkDefiner<System, MutexInner<T>, InitTag>,
}

/// A recursive mutex, which can be locked by a task for multiple times
/// without causing a deadlock.
///
/// This type is implemented using [`r3::kernel::Mutex`], the low-level
/// synchronization primitive and therefore inherits its properties.
/// The important inherited properties are listed below:
///
///  - When trying to lock an abandoned mutex, the lock function will return
///    `Err(LockError::Abandoned(lock_guard))`. This state can be exited by
///    calling [`RecursiveMutex::mark_consistent`].
///
///  - Mutexes must be unlocked in a lock-reverse order. [`MutexGuard`]`::drop`
///    might panic if this is violated.
///
/// [`r3::kernel::Mutex`]: crate::kernel::Mutex
pub struct RecursiveMutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    hunk: Hunk<System, MutexInner<T>>,
    mutex: mutex::Mutex<System>,
}

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior

unsafe impl<System, T: 'static + Send> Send for RecursiveMutex<System, T> where
    System: traits::KernelMutex + traits::KernelStatic
{
}
unsafe impl<System, T: 'static + Send> Sync for RecursiveMutex<System, T> where
    System: traits::KernelMutex + traits::KernelStatic
{
}

#[doc(hidden)]
pub struct MutexInner<T> {
    /// A bit field containing *the nesting count* (`bits[1..BITS]`) and
    /// *an abandonment flag* (`bits[0]`, [`LEVEL_ABANDONED`]).
    ///
    /// A nesting count `i` indicates the mutex has been locked for `i + 1`
    /// times. It must be `0` if the mutex is currently unlocked.
    ///
    /// The abandonment flag indicates that the nesting count is consistent but
    /// the inner data is still inconsistent. A recursive mutex can be in one
    /// of the following states:
    ///
    ///  - Fully consistent
    ///  - Nesting count consistent, data inconsistent
    ///  - Fully inconsistent
    ///
    level: Cell<usize>,
    /// The inner data.
    data: T,
}

impl<T: Init> Init for MutexInner<T> {
    const INIT: Self = Self {
        level: Cell::new(0),
        data: Init::INIT,
    };
}

/// The bit in [`MutexInner::level`] indicating that the nesting count is
/// consistent but the inner data is still inconsistent.
const LEVEL_ABANDONED: usize = 1;

/// The bit position of the nesting count in [`MutexInner::level`].
const LEVEL_COUNT_SHIFT: u32 = 1;

/// An RAII implementation of a "scoped lock" of a mutex. When this structure
/// is dropped, the lock will be released.
///
/// This structure is created by the [`lock`] and [`try_lock`] methods of
/// [`RecursiveMutex`].
///
/// [`lock`]: RecursiveMutex::lock
/// [`try_lock`]: RecursiveMutex::try_lock
#[must_use = "if unused the RecursiveMutex will immediately unlock"]
pub struct MutexGuard<'a, System, T: 'static>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    mutex: &'a RecursiveMutex<System, T>,
    _no_send_sync: PhantomData<*mut ()>,
}

unsafe impl<System, T: 'static + Sync> Sync for MutexGuard<'_, System, T> where
    System: traits::KernelMutex + traits::KernelStatic
{
}

/// Type alias for the result of [`RecursiveMutex::lock`].
pub type LockResult<Guard> = Result<Guard, LockError<Guard>>;

/// Type alias for the result of [`RecursiveMutex::try_lock`].
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

/// Error type of [`RecursiveMutex::lock`].
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
            Self::BadParam => "BadParam",
            Self::Abandoned(_) => "Abandoned",
        })
    }
}

/// Error type of [`RecursiveMutex::try_lock`].
#[repr(i8)]
pub enum TryLockError<Guard> {
    /// CPU Lock is active, or the current context is not [a task context].
    ///
    /// [a task context]: crate#contexts
    BadContext = TryLockMutexError::BadContext as i8,
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
            Self::BadParam => "BadParam",
            Self::Abandoned(_) => "Abandoned",
        })
    }
}

/// Error type of [`RecursiveMutex::mark_consistent`].
#[derive(Debug)]
#[repr(i8)]
pub enum MarkConsistentError {
    /// CPU Lock is active.
    BadContext = MarkConsistentMutexError::BadContext as i8,
    /// The mutex does not protect an inconsistent state.
    Consistent = MarkConsistentMutexError::BadObjectState as i8,
}

impl<System, T: 'static> RecursiveMutex<System, T>
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

impl<System, T: 'static, InitTag: HunkIniter<MutexInner<T>>> Definer<System, T, InitTag>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Complete the definition of a mutex, returning a reference to the mutex.
    // FIXME: `~const CfgBase` is not implied - compiler bug?
    pub const fn finish<C: ~const traits::CfgMutex<System = System> + ~const traits::CfgBase>(
        self,
        cfg: &mut Cfg<C>,
    ) -> RecursiveMutex<System, T> {
        RecursiveMutex {
            hunk: self.hunk.finish(cfg),
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<System, T: 'static> RecursiveMutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    ///
    /// # Panics
    ///
    /// This method will panic if the nesting count would overflow.
    pub fn lock(&self) -> LockResult<MutexGuard<'_, System, T>> {
        let level = &self.hunk.level;

        match self.mutex.lock() {
            Ok(()) => {}
            Err(LockMutexError::WouldDeadlock) => {
                level.update(|x| {
                    x.checked_add(1 << LEVEL_COUNT_SHIFT)
                        .expect("nesting count overflow")
                });
            }
            Err(LockMutexError::BadId) => unreachable!(),
            Err(LockMutexError::BadContext) => return Err(LockError::BadContext),
            Err(LockMutexError::Interrupted) => return Err(LockError::Interrupted),
            Err(LockMutexError::BadParam) => return Err(LockError::BadParam),
            Err(LockMutexError::Abandoned) => {
                // Make the nesting count consistent
                level.set(LEVEL_ABANDONED);
                self.mutex.mark_consistent().unwrap();
            }
        }

        if (level.get() & LEVEL_ABANDONED) != 0 {
            Err(LockError::Abandoned(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }))
        } else {
            Ok(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            })
        }
    }

    /// Attempt to acquire the mutex.
    ///
    /// # Panics
    ///
    /// This method will panic if the nesting count would overflow.
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, System, T>> {
        let level = &self.hunk.level;

        match self.mutex.try_lock() {
            Ok(()) => {}
            Err(TryLockMutexError::WouldDeadlock) => {
                level.update(|x| {
                    x.checked_add(1 << LEVEL_COUNT_SHIFT)
                        .expect("nesting count overflow")
                });
            }
            Err(TryLockMutexError::BadId) => unreachable!(),
            Err(TryLockMutexError::BadContext) => return Err(TryLockError::BadContext),
            Err(TryLockMutexError::Timeout) => return Err(TryLockError::WouldBlock),
            Err(TryLockMutexError::BadParam) => return Err(TryLockError::BadParam),
            Err(TryLockMutexError::Abandoned) => {
                // Make the nesting count consistent
                level.set(LEVEL_ABANDONED);
                self.mutex.mark_consistent().unwrap();
            }
        }

        if (level.get() & LEVEL_ABANDONED) != 0 {
            Err(TryLockError::Abandoned(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }))
        } else {
            Ok(MutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            })
        }
    }

    /// Mark the state protected by the mutex as consistent.
    pub fn mark_consistent(&self) -> Result<(), MarkConsistentError> {
        let level = &self.hunk.level;

        match self.mutex.mark_consistent() {
            Ok(()) => {
                // Make the nesting count consistent and mark the content as
                // consistent at the same time
                level.set(0);
                Ok(())
            }
            Err(MarkConsistentMutexError::BadId) => unreachable!(),
            Err(MarkConsistentMutexError::BadContext) => Err(MarkConsistentError::BadContext),
            Err(MarkConsistentMutexError::BadObjectState) => {
                // The nesting count is consistent.
                if (level.get() & LEVEL_ABANDONED) != 0 {
                    // Mark the content as consistent
                    level.update(|x| x & !LEVEL_ABANDONED);
                    Ok(())
                } else {
                    // The mutex is fully consistent.
                    Err(MarkConsistentError::Consistent)
                }
            }
        }
    }

    /// Get a raw pointer to the contained data.
    #[inline]
    pub fn get_ptr(&self) -> *mut T {
        core::ptr::addr_of!(self.hunk.data) as _
    }
}

impl<System, T: fmt::Debug + 'static> fmt::Debug for RecursiveMutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f
                .debug_struct("RecursiveMutex")
                .field("data", &&*guard)
                .finish(),
            Err(TryLockError::BadContext) => {
                struct BadContextPlaceholder;
                impl fmt::Debug for BadContextPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<bad context>")
                    }
                }

                f.debug_struct("RecursiveMutex")
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

                f.debug_struct("RecursiveMutex")
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

                f.debug_struct("RecursiveMutex")
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

                f.debug_struct("RecursiveMutex")
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
        let level = &self.mutex.hunk.level;
        if level.get() == 0 || level.get() == LEVEL_ABANDONED {
            self.mutex.mutex.unlock().unwrap();
        } else {
            level.update(|x| x - (1 << LEVEL_COUNT_SHIFT));
        }
    }
}

impl<System, T: 'static> core::ops::Deref for MutexGuard<'_, System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.mutex.hunk.data
    }
}
