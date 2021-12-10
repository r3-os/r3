use core::{
    cell::UnsafeCell,
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{
    hunk::{DefaultInitTag, Hunk, HunkDefiner, HunkIniter},
    kernel::{
        mutex, prelude::*, traits, Cfg, LockMutexError, MarkConsistentMutexError, MutexProtocol,
        TryLockMutexError,
    },
};

/// The definer (static builder) for [`StaticMutex`][].
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
///    calling [`GenericMutex::mark_consistent`].
///
///  - Mutexes must be unlocked in a lock-reverse order.
///    [`GenericMutexGuard`]`::drop` might panic if this is violated.
///
/// [`r3::kernel::Mutex`]: crate::kernel::Mutex
pub struct GenericMutex<Cell, Mutex> {
    cell: Cell,
    mutex: Mutex,
}

/// A defined (statically created) [`GenericMutex`].
pub type StaticMutex<System, T> =
    GenericMutex<Hunk<System, UnsafeCell<T>>, mutex::MutexRef<'static, System>>;

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior
// TODO: Owned version

unsafe impl<Cell, Mutex, T: Send> Send for GenericMutex<Cell, Mutex> where
    Cell: Deref<Target = UnsafeCell<T>>
{
}
unsafe impl<Cell, Mutex, T: Send> Sync for GenericMutex<Cell, Mutex> where
    Cell: Deref<Target = UnsafeCell<T>>
{
}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure
/// is dropped, the lock will be released.
///
/// This structure is created by the [`lock`] and [`try_lock`] methods of
/// [`GenericMutex`].
///
/// [`lock`]: GenericMutex::lock
/// [`try_lock`]: GenericMutex::try_lock
#[must_use = "if unused the GenericMutex will immediately unlock"]
pub struct GenericMutexGuard<'a, Cell, Mutex: mutex::MutexHandle> {
    mutex: &'a GenericMutex<Cell, Mutex>,
    _no_send_sync: PhantomData<*mut ()>,
}

/// The specialization of [`GenericMutexGuard`] for [`StaticMutex`].
pub type StaticMutexGuard<'a, System, T> =
    GenericMutexGuard<'a, Hunk<System, UnsafeCell<T>>, mutex::MutexRef<'a, System>>;

unsafe impl<Cell, Mutex, T: Sync> Sync for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
}

/// Type alias for the result of [`GenericMutex::lock`].
pub type LockResult<Guard> = Result<Guard, LockError<Guard>>;

/// Type alias for the result of [`GenericMutex::try_lock`].
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

/// Error type of [`GenericMutex::lock`].
#[repr(i8)]
pub enum LockError<Guard> {
    /// CPU Lock is active, or the current context is not [waitable].
    ///
    /// [waitable]: crate#contexts
    BadContext = LockMutexError::BadContext as i8,
    /// The wait operation was interrupted by [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: crate::kernel::task::TaskMethods::interrupt
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

/// Error type of [`GenericMutex::try_lock`].
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

/// Error type of [`GenericMutex::mark_consistent`].
#[derive(Debug)]
#[repr(i8)]
pub enum MarkConsistentError {
    /// CPU Lock is active.
    BadContext = MarkConsistentMutexError::BadContext as i8,
    /// The mutex does not protect an inconsistent state.
    Consistent = MarkConsistentMutexError::BadObjectState as i8,
}

impl<System, T: 'static> StaticMutex<System, T>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Construct a `Definer` to define a mutex in [a configuration
    /// function](crate#static-configuration).
    pub const fn define() -> Definer<System, T, DefaultInitTag> {
        Definer {
            mutex: mutex::MutexRef::define(),
            hunk: Hunk::define(),
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
    ) -> StaticMutex<System, T> {
        GenericMutex {
            cell: self.hunk.finish(cfg),
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<Cell, Mutex, T> GenericMutex<Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    pub fn lock(&self) -> LockResult<GenericMutexGuard<'_, Cell, Mutex>> {
        match self.mutex.lock() {
            Ok(()) => Ok(GenericMutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }),
            Err(LockMutexError::BadId) => unreachable!(),
            Err(LockMutexError::BadContext) => Err(LockError::BadContext),
            Err(LockMutexError::Interrupted) => Err(LockError::Interrupted),
            Err(LockMutexError::WouldDeadlock) => Err(LockError::WouldDeadlock),
            Err(LockMutexError::BadParam) => Err(LockError::BadParam),
            Err(LockMutexError::Abandoned) => Err(LockError::Abandoned(GenericMutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            })),
        }
    }

    /// Attempt to acquire the mutex.
    pub fn try_lock(&self) -> TryLockResult<GenericMutexGuard<'_, Cell, Mutex>> {
        match self.mutex.try_lock() {
            Ok(()) => Ok(GenericMutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }),
            Err(TryLockMutexError::BadId) => unreachable!(),
            Err(TryLockMutexError::BadContext) => Err(TryLockError::BadContext),
            Err(TryLockMutexError::WouldDeadlock) => Err(TryLockError::WouldDeadlock),
            Err(TryLockMutexError::Timeout) => Err(TryLockError::WouldBlock),
            Err(TryLockMutexError::BadParam) => Err(TryLockError::BadParam),
            Err(TryLockMutexError::Abandoned) => Err(TryLockError::Abandoned(GenericMutexGuard {
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
        self.cell.get()
    }
}

impl<Cell, Mutex, T: fmt::Debug> fmt::Debug for GenericMutex<Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f
                .debug_struct("GenericMutex")
                .field("data", &&*guard)
                .finish(),
            Err(TryLockError::BadContext) => {
                struct BadContextPlaceholder;
                impl fmt::Debug for BadContextPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<bad context>")
                    }
                }

                f.debug_struct("GenericMutex")
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

                f.debug_struct("GenericMutex")
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

                f.debug_struct("GenericMutex")
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

                f.debug_struct("GenericMutex")
                    .field("data", &BadParamPlaceholder)
                    .finish()
            }
        }
    }
}

impl<Cell, Mutex, T: fmt::Debug> fmt::Debug for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<Cell, Mutex, T: fmt::Display> fmt::Display for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// The destructor of `GenericMutexGuard` that releases the lock. It will panic if
/// CPU Lock is active.
impl<Cell, Mutex> Drop for GenericMutexGuard<'_, Cell, Mutex>
where
    Mutex: mutex::MutexHandle,
{
    #[inline]
    fn drop(&mut self) {
        self.mutex.mutex.unlock().unwrap();
    }
}

impl<Cell, Mutex, T> Deref for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: `GenericMutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &*self.mutex.cell.get() }
    }
}

impl<Cell, Mutex, T> DerefMut for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: `GenericMutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &mut *self.mutex.cell.get() }
    }
}

// Safety: `MutexGuard::deref` provides a stable address
unsafe impl<Cell, Mutex, T> stable_deref_trait::StableDeref for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<T>>,
    Mutex: mutex::MutexHandle,
{
}
