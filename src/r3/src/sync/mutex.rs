use core::{
    cell::UnsafeCell,
    fmt,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use crate::{
    hunk::Hunk,
    kernel::{
        mutex, prelude::*, traits, Cfg, LockMutexError, MarkConsistentMutexError, MutexProtocol,
        TryLockMutexError,
    },
    sync::source::{DefaultSource, Source},
    utils::Init,
};

/// The definer (static builder) for [`StaticMutex`][].
pub struct Definer<System, Source> {
    mutex: mutex::MutexDefiner<System>,
    source: Source,
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
/// # Example
///
/// See [`StaticMutex`].
///
/// [`r3::kernel::Mutex`]: crate::kernel::Mutex
pub struct GenericMutex<Cell, Mutex> {
    cell: Cell,
    mutex: Mutex,
}

/// A defined (statically created) [`GenericMutex`].
///
/// # Example
///
#[doc = crate::tests::doc_test!(
/// ```rust
/// use r3::{kernel::StaticTask, sync::StaticMutex};
///
/// struct Objects {
///     task2: StaticTask<System>,
///     mutex: StaticMutex<System, i32>,
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
///     let task2 = StaticTask::define()
///         .start(task2_body)
///         .priority(1)
///         .finish(cfg);
///
///     let mutex = StaticMutex::define().init(|| 1).finish(cfg);
///
///     Objects { task2, mutex }
/// }
///
/// fn task1_body() {
///     let mut guard = COTTAGE.mutex.lock().unwrap();
///
///     // Although `task2` has a higher priority, it's unable to
///     // access `*guard` until `task1` releases the lock
///     COTTAGE.task2.activate().unwrap();
///
///     assert_eq!(*guard, 1);
///     *guard = 2;
/// }
///
/// fn task2_body() {
///     let mut guard = COTTAGE.mutex.lock().unwrap();
///     assert_eq!(*guard, 2);
///     *guard = 3;
/// #   exit(0);
/// }
/// ```
)]
pub type StaticMutex<System, T> =
    GenericMutex<Hunk<System, UnsafeCell<MaybeUninit<T>>>, mutex::StaticMutex<System>>;

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior
// TODO: Owned version

unsafe impl<Cell, Mutex, T: Send> Send for GenericMutex<Cell, Mutex> where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>
{
}
unsafe impl<Cell, Mutex, T: Send> Sync for GenericMutex<Cell, Mutex> where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>
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
    GenericMutexGuard<'a, Hunk<System, UnsafeCell<MaybeUninit<T>>>, mutex::MutexRef<'a, System>>;

unsafe impl<Cell, Mutex, T: Sync> Sync for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
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
    pub const fn define() -> Definer<System, DefaultSource<T>> {
        Definer {
            mutex: mutex::StaticMutex::define(),
            source: DefaultSource::INIT, // [ref:default_source_is_default]
        }
    }
}

impl<System, Source> Definer<System, Source>
where
    System: traits::KernelMutex,
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
impl_source_setter!(impl Definer<System, #Source>);

/// # Finalization
///
/// The following method completes the definition of a mutex.
impl<System, Source> Definer<System, Source>
where
    System: traits::KernelMutex + traits::KernelStatic,
{
    /// Complete the definition of a mutex, returning a reference to the mutex.
    // FIXME: `~const CfgBase` is not implied - compiler bug?
    pub const fn finish<C: ~const traits::CfgMutex<System = System> + ~const traits::CfgBase>(
        self,
        cfg: &mut Cfg<C>,
    ) -> StaticMutex<System, Source::Target>
    where
        Source: ~const self::Source<System>,
    {
        GenericMutex {
            cell: self.source.into_unsafe_cell_hunk(cfg),
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<Cell, Mutex, T> GenericMutex<Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
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
            Err(LockMutexError::NoAccess) => unreachable!(),
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
            Err(TryLockMutexError::NoAccess) => unreachable!(),
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
            MarkConsistentMutexError::NoAccess => unreachable!(),
            MarkConsistentMutexError::BadContext => MarkConsistentError::BadContext,
            MarkConsistentMutexError::BadObjectState => MarkConsistentError::Consistent,
        })
    }

    /// Get a raw pointer to the contained data.
    #[inline]
    pub fn get_ptr(&self) -> *mut T {
        self.cell.get().cast()
    }
}

impl<Cell, Mutex, T: fmt::Debug> fmt::Debug for GenericMutex<Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
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
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
    Mutex: mutex::MutexHandle,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<Cell, Mutex, T: fmt::Display> fmt::Display for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
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
    // TODO: Currently `Cell` is always a `Hunk` given by `Source`, but in the
    //       future, we might have other ways to provide `Cell`. For now we just
    //       reference [ref:source_cell] when we're using its precondidtions
    //       concerns memory safety. To support other variations of `Cell`, we
    //       might need a better mechanism to express these preconditions than
    //       to reference [ref:source_cell] whenever they are relevant.
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
    Mutex: mutex::MutexHandle,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: `GenericMutexGuard` represents a permit acquired from the
        // semaphore, which grants the bearer an exclusive access to the
        // underlying data. Since this `Hunk` was given by `Source`
        // ([ref:source_cell]), we are authorized to enforce the runtime borrow
        // rules on its contents.
        //
        // [ref:source_cell] says that the contents may be unavailable outside
        // the context of an executable object. We are in the clear because
        // `kernel::Mutex` can only be locked in a task context.
        unsafe { (*self.mutex.cell.get()).assume_init_ref() }
    }
}

impl<Cell, Mutex, T> DerefMut for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
    Mutex: mutex::MutexHandle,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: See the `deref` above.
        unsafe { (*self.mutex.cell.get()).assume_init_mut() }
    }
}

// Safety: `MutexGuard::deref` provides a stable address
unsafe impl<Cell, Mutex, T> stable_deref_trait::StableDeref for GenericMutexGuard<'_, Cell, Mutex>
where
    Cell: Deref<Target = UnsafeCell<MaybeUninit<T>>>,
    Mutex: mutex::MutexHandle,
{
}
