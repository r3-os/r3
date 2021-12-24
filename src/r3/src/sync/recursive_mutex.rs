use core::{cell::Cell, fmt, marker::PhantomData, ops::Deref};

use crate::{
    hunk::{DefaultInitTag, Hunk, HunkDefiner, HunkIniter},
    kernel::{
        mutex, prelude::*, traits, Cfg, LockMutexError, MarkConsistentMutexError, MutexProtocol,
        TryLockMutexError,
    },
    utils::Init,
};

/// The definer (static builder) for [`StaticRecursiveMutex`][].
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
///    calling [`GenericRecursiveMutex::mark_consistent`].
///
///  - Mutexes must be unlocked in a lock-reverse order.
///    [`GenericMutexGuard`]`::drop` might panic if this is violated.
///
/// # Example
///
/// See [`StaticRecursiveMutex`].
///
/// [`r3::kernel::Mutex`]: crate::kernel::Mutex
pub struct GenericRecursiveMutex<Cell, Mutex> {
    cell: Cell,
    mutex: Mutex,
}

/// A defined (statically created) [`GenericRecursiveMutex`].
///
/// # Example
///
#[doc = crate::tests::doc_test!(
/// ```rust
/// use core::cell::Cell;
/// use r3::{kernel::StaticTask, sync::StaticRecursiveMutex};
///
/// struct Objects {
///     mutex: StaticRecursiveMutex<System, Cell<i32>>,
/// }
///
/// const fn configure_app<C>(cfg: &mut Cfg<C>) -> Objects
/// where
///     C: ~const traits::CfgBase<System = System> +
///        ~const traits::CfgTask +
///        ~const traits::CfgMutex,
/// {
///     StaticTask::define()
///         .start(task1_body)
///         .priority(2)
///         .active(true)
///         .finish(cfg);
///
///     let mutex = StaticRecursiveMutex::define().finish(cfg);
///
///     Objects { mutex }
/// }
///
/// fn task1_body(_: usize) {
///     let guard = COTTAGE.mutex.lock().unwrap();
///     assert_eq!(guard.get(), 0);
///     guard.set(1);
///
///     {
///         // Recursive lock is allowed
///         let guard2 = COTTAGE.mutex.lock().unwrap();
///         assert_eq!(guard2.get(), 1);
///         guard2.set(2);
///     }
///
///     assert_eq!(guard.get(), 2);
/// #   exit(0);
/// }
/// ```
)]
pub type StaticRecursiveMutex<System, T> =
    GenericRecursiveMutex<Hunk<System, MutexInner<T>>, mutex::StaticMutex<System>>;

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior
// TODO: Owned version

unsafe impl<Cell, Mutex, T: Send> Send for GenericRecursiveMutex<Cell, Mutex> where
    Cell: Deref<Target = MutexInner<T>>
{
}
unsafe impl<Cell, Mutex, T: Send> Sync for GenericRecursiveMutex<Cell, Mutex> where
    Cell: Deref<Target = MutexInner<T>>
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
/// [`GenericRecursiveMutex`].
///
/// [`lock`]: GenericRecursiveMutex::lock
/// [`try_lock`]: GenericRecursiveMutex::try_lock
#[must_use = "if unused the GenericRecursiveMutex will immediately unlock"]
pub struct GenericMutexGuard<'a, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    mutex: &'a GenericRecursiveMutex<Cell, Mutex>,
    _no_send_sync: PhantomData<*mut ()>,
}

unsafe impl<Cell, Mutex, T: Sync> Sync for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
}

/// Type alias for the result of [`GenericRecursiveMutex::lock`].
pub type LockResult<Guard> = Result<Guard, LockError<Guard>>;

/// Type alias for the result of [`GenericRecursiveMutex::try_lock`].
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

/// Error type of [`GenericRecursiveMutex::lock`].
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

/// Error type of [`GenericRecursiveMutex::try_lock`].
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

/// Error type of [`GenericRecursiveMutex::mark_consistent`].
#[derive(Debug)]
#[repr(i8)]
pub enum MarkConsistentError {
    /// CPU Lock is active.
    BadContext = MarkConsistentMutexError::BadContext as i8,
    /// The mutex does not protect an inconsistent state.
    Consistent = MarkConsistentMutexError::BadObjectState as i8,
}

impl<System, T: 'static> StaticRecursiveMutex<System, T>
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

impl<System, T: 'static, InitTag: HunkIniter<MutexInner<T>>> Definer<System, T, InitTag>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Complete the definition of a mutex, returning a reference to the mutex.
    // FIXME: `~const CfgBase` is not implied - compiler bug?
    pub const fn finish<C: ~const traits::CfgMutex<System = System> + ~const traits::CfgBase>(
        self,
        cfg: &mut Cfg<C>,
    ) -> StaticRecursiveMutex<System, T> {
        GenericRecursiveMutex {
            cell: self.hunk.finish(cfg),
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<Cell, Mutex, T> GenericRecursiveMutex<Cell, Mutex>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    ///
    /// # Panics
    ///
    /// This method will panic if the nesting count would overflow.
    pub fn lock(&self) -> LockResult<GenericMutexGuard<'_, Cell, Mutex, T>> {
        let level = &self.cell.level;

        match self.mutex.lock() {
            Ok(()) => {}
            Err(LockMutexError::WouldDeadlock) => {
                level.update(|x| {
                    x.checked_add(1 << LEVEL_COUNT_SHIFT)
                        .expect("nesting count overflow")
                });
            }
            Err(LockMutexError::NoAccess) => unreachable!(),
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
            Err(LockError::Abandoned(GenericMutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }))
        } else {
            Ok(GenericMutexGuard {
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
    pub fn try_lock(&self) -> TryLockResult<GenericMutexGuard<'_, Cell, Mutex, T>> {
        let level = &self.cell.level;

        match self.mutex.try_lock() {
            Ok(()) => {}
            Err(TryLockMutexError::WouldDeadlock) => {
                level.update(|x| {
                    x.checked_add(1 << LEVEL_COUNT_SHIFT)
                        .expect("nesting count overflow")
                });
            }
            Err(TryLockMutexError::NoAccess) => unreachable!(),
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
            Err(TryLockError::Abandoned(GenericMutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            }))
        } else {
            Ok(GenericMutexGuard {
                mutex: self,
                _no_send_sync: PhantomData,
            })
        }
    }

    /// Mark the state protected by the mutex as consistent.
    pub fn mark_consistent(&self) -> Result<(), MarkConsistentError> {
        let level = &self.cell.level;

        match self.mutex.mark_consistent() {
            Ok(()) => {
                // Make the nesting count consistent and mark the content as
                // consistent at the same time
                level.set(0);
                Ok(())
            }
            Err(MarkConsistentMutexError::NoAccess) => unreachable!(),
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
        core::ptr::addr_of!(self.cell.data) as _
    }
}

impl<Cell, Mutex, T: fmt::Debug> fmt::Debug for GenericRecursiveMutex<Cell, Mutex>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Ok(guard) => f
                .debug_struct("GenericRecursiveMutex")
                .field("data", &&*guard)
                .finish(),
            Err(TryLockError::BadContext) => {
                struct BadContextPlaceholder;
                impl fmt::Debug for BadContextPlaceholder {
                    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("<bad context>")
                    }
                }

                f.debug_struct("GenericRecursiveMutex")
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

                f.debug_struct("GenericRecursiveMutex")
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

                f.debug_struct("GenericRecursiveMutex")
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

                f.debug_struct("GenericRecursiveMutex")
                    .field("data", &BadParamPlaceholder)
                    .finish()
            }
        }
    }
}

impl<Cell, Mutex, T: fmt::Debug> fmt::Debug for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<Cell, Mutex, T: fmt::Display> fmt::Display for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

/// The destructor of `GenericMutexGuard` that releases the lock. It will panic if
/// CPU Lock is active.
impl<Cell, Mutex, T> Drop for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    #[inline]
    fn drop(&mut self) {
        let level = &self.mutex.cell.level;
        if level.get() == 0 || level.get() == LEVEL_ABANDONED {
            self.mutex.mutex.unlock().unwrap();
        } else {
            level.update(|x| x - (1 << LEVEL_COUNT_SHIFT));
        }
    }
}

impl<Cell, Mutex, T> Deref for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.mutex.cell.data
    }
}

// Safety: `GenericMutexGuard::deref` provides a stable address
unsafe impl<Cell, Mutex, T> stable_deref_trait::StableDeref
    for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MutexInner<T>>,
    Mutex: mutex::MutexHandle,
{
}
