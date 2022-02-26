use core::{cell::Cell, fmt, marker::PhantomData, mem::MaybeUninit, ops::Deref};

use crate::{
    hunk::Hunk,
    kernel::{
        mutex, prelude::*, traits, Cfg, LockMutexError, MarkConsistentMutexError, MutexProtocol,
        TryLockMutexError,
    },
    sync::source::{DefaultSource, Source},
    utils::Init,
};

/// The definer (static builder) for [`StaticRecursiveMutex`][].
#[doc = include_str!("../common.md")]
pub struct Definer<System, Source> {
    mutex: mutex::MutexDefiner<System>,
    source: Source,
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
///     let mutex = StaticRecursiveMutex::define()
///         .init(|| Cell::new(1))
///         .finish(cfg);
///
///     Objects { mutex }
/// }
///
/// fn task1_body() {
///     let guard = COTTAGE.mutex.lock().unwrap();
///     assert_eq!(guard.get(), 1);
///     guard.set(2);
///
///     {
///         // Recursive lock is allowed
///         let guard2 = COTTAGE.mutex.lock().unwrap();
///         assert_eq!(guard2.get(), 2);
///         guard2.set(3);
///     }
///
///     assert_eq!(guard.get(), 3);
/// #   exit(0);
/// }
/// ```
)]
pub type StaticRecursiveMutex<System, T> =
    GenericRecursiveMutex<Hunk<System, MaybeUninit<MutexInner<T>>>, mutex::StaticMutex<System>>;

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior
// TODO: Owned version

unsafe impl<Cell, Mutex, T: Send> Send for GenericRecursiveMutex<Cell, Mutex> where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>
{
}
unsafe impl<Cell, Mutex, T: Send> Sync for GenericRecursiveMutex<Cell, Mutex> where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>
{
}

/// The inner data structure of [`GenericRecursiveMutex`][].
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

impl<T> MutexInner<T> {
    /// Construct `MutexInner`.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            level: Cell::new(0),
            data,
        }
    }
}

impl<T: Init> Init for MutexInner<T> {
    const INIT: Self = Self::new(T::INIT);
}

impl<T: ~const Default> const Default for MutexInner<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// Forwarded to [`Self::new`][].
impl<T> const From<T> for MutexInner<T> {
    #[inline]
    fn from(x: T) -> Self {
        Self::new(x)
    }
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
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
    Mutex: mutex::MutexHandle,
{
    mutex: &'a GenericRecursiveMutex<Cell, Mutex>,
    _no_send_sync: PhantomData<*mut ()>,
}

unsafe impl<Cell, Mutex, T: Sync> Sync for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
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
    pub const fn define() -> Definer<System, DefaultSource<MutexInner<T>>> {
        Definer {
            mutex: mutex::MutexRef::define(),
            source: DefaultSource::INIT, // [ref:default_source_is_default]
        }
    }
}

impl<System, Source> Definer<System, Source>
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

// Define methods to set `Definer::source`
impl_source_setter!(
    #[autowrap(MutexInner::new, MutexInner)]
    impl Definer<System, #Source>
);

impl<System, Source> Definer<System, Source>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Complete the definition of a mutex, returning a reference to the mutex.
    // `CfgMutex` can't have `~const CfgBase` as a supertrait because of
    // [ref:const_supertraits], hence we need to specify `~const CfgBase` here
    pub const fn finish<C: ~const traits::CfgMutex<System = System> + ~const traits::CfgBase, T>(
        self,
        cfg: &mut Cfg<C>,
    ) -> StaticRecursiveMutex<System, T>
    where
        Source: ~const self::Source<System, Target = MutexInner<T>>,
    {
        GenericRecursiveMutex {
            // Safety: It's safe to unwrap `UnsafeCell` because there's already
            // a `Cell` in `MutexInner` where it's needed. `T` is never mutably
            // borrowed there.
            cell: unsafe { self.source.into_unsafe_cell_hunk(cfg).transmute() },
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<Cell, Mutex, T> GenericRecursiveMutex<Cell, Mutex>
where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
    Mutex: mutex::MutexHandle,
{
    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    ///
    /// # Panics
    ///
    /// This method will panic if the nesting count would overflow.
    pub fn lock(&self) -> LockResult<GenericMutexGuard<'_, Cell, Mutex, T>> {
        let level;

        match self.mutex.lock() {
            Ok(()) => {
                // Safety: We are in a task, which means `self.cell` is
                // initialized [ref:source_cell]
                level = unsafe { &self.cell.assume_init_ref().level };
            }
            Err(LockMutexError::WouldDeadlock) => {
                // Safety: We are the owning task, which means `self.cell` is
                // initialized [ref:source_cell]
                level = unsafe { &self.cell.assume_init_ref().level };
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
                // Safety: It being abandoned means there was a task that owned
                // the lock, which means we are past the boot phase, which means
                // `self.cell` is initialized [ref:source_cell]
                level = unsafe { &self.cell.assume_init_ref().level };

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
        let level;

        match self.mutex.try_lock() {
            Ok(()) => {
                // Safety: We are in a task, which means `self.cell` is
                // initialized [ref:source_cell]
                level = unsafe { &self.cell.assume_init_ref().level };
            }
            Err(TryLockMutexError::WouldDeadlock) => {
                // Safety: We are the owning task, which means `self.cell` is
                // initialized [ref:source_cell]
                level = unsafe { &self.cell.assume_init_ref().level };
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
                // Safety: It being abandoned means there was a task that owned
                // the lock, which means we are past the boot phase, which means
                // `self.cell` is initialized [ref:source_cell]
                level = unsafe { &self.cell.assume_init_ref().level };

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
        match self.mutex.mark_consistent() {
            Ok(()) => {
                // Safety: It having been inconsistent means there was a task
                // that owned a lock, which means we are past the boot phase,
                // which means `self.cell` is initialized [ref:source_cell]
                let level = unsafe { &self.cell.assume_init_ref().level };

                // Make the nesting count consistent and mark the content as
                // consistent at the same time
                level.set(0);
                Ok(())
            }
            Err(MarkConsistentMutexError::NoAccess) => unreachable!(),
            Err(MarkConsistentMutexError::BadContext) => Err(MarkConsistentError::BadContext),
            Err(MarkConsistentMutexError::BadObjectState) => {
                // Safety: CPU Lock is inactive, which means we are past the
                // boot phase, which means `self.cell` is initialized
                // [ref:source_cell]
                let level = unsafe { &self.cell.assume_init_ref().level };

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
        // Safety: Not really unsafe because we aren't borrowing anything
        unsafe { core::ptr::addr_of!((*self.cell.as_ptr()).data) as *mut T }
    }
}

impl<Cell, Mutex, T: fmt::Debug> fmt::Debug for GenericRecursiveMutex<Cell, Mutex>
where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
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
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<Cell, Mutex, T: fmt::Display> fmt::Display for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
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
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
    Mutex: mutex::MutexHandle,
{
    #[inline]
    fn drop(&mut self) {
        // Safety: We own the lock, which means we are past the boot phase,
        // which means `self.mutex.cell` is initialized [ref:source_cell]
        let level = unsafe { &self.mutex.cell.assume_init_ref().level };
        if level.get() == 0 || level.get() == LEVEL_ABANDONED {
            self.mutex.mutex.unlock().unwrap();
        } else {
            level.update(|x| x - (1 << LEVEL_COUNT_SHIFT));
        }
    }
}

impl<Cell, Mutex, T> Deref for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
    Mutex: mutex::MutexHandle,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: We own the lock, which means we are past the boot phase,
        // which means `self.mutex.cell` is initialized [ref:source_cell]
        unsafe { &self.mutex.cell.assume_init_ref().data }
    }
}

// Safety: `GenericMutexGuard::deref` provides a stable address
unsafe impl<Cell, Mutex, T> stable_deref_trait::StableDeref
    for GenericMutexGuard<'_, Cell, Mutex, T>
where
    Cell: Deref<Target = MaybeUninit<MutexInner<T>>>,
    Mutex: mutex::MutexHandle,
{
}
