use core::{cell::UnsafeCell, fmt, marker::PhantomData};

use crate::{
    kernel::{
        self,
        cfg::{CfgBuilder, CfgHunkBuilder, CfgMutexBuilder, DefaultInitTag, HunkIniter},
        Hunk, LockMutexError, MarkConsistentMutexError, MutexProtocol, TryLockMutexError,
    },
    prelude::*,
};

/// Configuration builder type for [`Mutex`].
pub struct MutexBuilder<System, T, InitTag> {
    mutex: CfgMutexBuilder<System>,
    hunk: CfgHunkBuilder<System, UnsafeCell<T>, InitTag>,
}

/// A mutual exclusion primitive useful for protecting shared data from
/// concurrent access.
///
/// This type is implemented using [`constance::kernel::Mutex`], the low-level
/// synchronization primitive and therefore inherits its properties.
/// The important inherited properties are listed below:
///
///  - When trying to lock an abandoned mutex, the lock function will return
///    `Err(LockError::Abandoned(lock_guard))`. This state can be exited by
///    calling [`Mutex::mark_consistent`].
///
///  - Mutexes must be unlocked in a lock-reverse order. [`MutexGuard`]`::drop`
///    will panic if this is violated.
///
/// [`constance::kernel::Mutex`]: crate::kernel::Mutex
pub struct Mutex<System, T> {
    hunk: Hunk<System, UnsafeCell<T>>,
    mutex: kernel::Mutex<System>,
}

// TODO: Test the panicking behavior on invalid unlock order
// TODO: Test the abandonment behavior

unsafe impl<System: Kernel, T: 'static + Send> Send for Mutex<System, T> {}
unsafe impl<System: Kernel, T: 'static + Send> Sync for Mutex<System, T> {}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure
/// is dropped, the lock will be released.
///
/// This structure is created by the [`lock`] and [`try_lock`] methods of
/// [`Mutex`].
///
/// [`lock`]: Mutex::lock
/// [`try_lock`]: Mutex::try_lock
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, System: Kernel, T: 'static> {
    mutex: kernel::Mutex<System>,
    data: *mut T,
    _phantom_lifetime: PhantomData<&'a ()>,
}

unsafe impl<System: Kernel, T: 'static + Sync> Sync for MutexGuard<'_, System, T> {}

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

impl<System: Kernel, T: 'static> Mutex<System, T> {
    /// Construct a `MutexBuilder` to define a mutex in [a configuration
    /// function](crate#static-configuration).
    pub const fn build() -> MutexBuilder<System, T, DefaultInitTag> {
        MutexBuilder {
            mutex: kernel::Mutex::build(),
            hunk: kernel::Hunk::build(),
        }
    }
}

impl<System: Kernel, T: 'static, InitTag> MutexBuilder<System, T, InitTag> {
    /// Specify the mutex's protocol. Defaults to `None` when unspecified.
    pub const fn protocol(self, protocol: MutexProtocol) -> Self {
        Self {
            mutex: self.mutex.protocol(protocol),
            ..self
        }
    }
}

impl<System: Kernel, T: 'static, InitTag: HunkIniter<UnsafeCell<T>>>
    MutexBuilder<System, T, InitTag>
{
    /// Complete the definition of a mutex, returning a reference to the mutex.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> Mutex<System, T> {
        Mutex {
            hunk: self.hunk.finish(cfg),
            mutex: self.mutex.finish(cfg),
        }
    }
}

impl<System: Kernel, T: 'static> Mutex<System, T> {
    //  These methods have `#[inline]` because we want to optimize out `Mutex`
    //  objects from a final binary whenever possible, as well as to minimize
    //  the runtime overhead.
    //
    //  Take the following code as example:
    //
    //      const M: sync::Mutex<System, u32> = /* ... */;
    //      fn hoge() -> u32 { *M.lock().unwrap() }
    //
    //  If `sync::Mutex::lock` weren't inlined, the caller would have to pass a
    //  reference to a `sync::Mutex` object, which must be stored in a read-only
    //  data section. Furthermore, `sync::Mutex::lock` method would have to read
    //  `sync::Mutex::mutex` (the inner mutex object). The compiled code would
    //  look like the following:
    //
    //      static M_REALIZED: sync::Mutex<System, u32> = sync::Mutex {
    //          mutex: kernel::Mutex(42),
    //          hunk: kernel::Hunk {
    //              start: 4,
    //              len: 4,
    //          },
    //      };
    //
    //      fn hoge() -> u32 {
    //          // (1) Load the address of `M_REALIZED` into a register and
    //          // (2) perform a subroutine call to `sync_mutex_lock`
    //          let guard = sync_mutex_lock(&M_REALIZED).unwrap();
    //
    //          // (5) Load the hunk's offset, (6) calculate the hunk's address,
    //          // and (7) load the contained value
    //          *((HUNK_POOL + guard.mutex.hunk.start) as *const u32)
    //      }
    //
    //      fn sync_mutex_lock(this: &sync::Mutex<System, u32>) -> LockResult</* ... */> {
    //          // (3) Load `this.mutex` and (4) perform a subroutine call to
    //          // `kernel::Mutex::lock`
    //          match this.mutex.lock() {
    //              /* ... */
    //          }
    //      }
    //
    //  With inlining:
    //
    //      fn hoge() -> u32 {
    //          // (1) Load `42` and (2) the hunk's address into registers and
    //          // (3) perform a subroutine call to `sync_mutex_lock_inner`
    //          let guard = sync_mutex_lock_inner(
    //              kernel::Mutex(42),
    //              *((HUNK_POOL + 4) as *const u32),
    //          ).unwrap();
    //
    //          // (5) Load the contained value
    //          *guard.data
    //      }
    //
    //      fn sync_mutex_lock_inner(mutex: kernel::Mutex<System, u32>, data: *mut u32) -> LockResult</* ... */> {
    //          // (4) Perform a subroutine call to `kernel::Mutex::lock`
    //          match mutex.lock() {
    //              /* ... */
    //          }
    //      }
    //

    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    #[inline]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, System, T>> {
        fn lock_inner<'a, System: Kernel, T: 'static>(
            mutex: kernel::Mutex<System>,
            data: *mut T,
        ) -> LockResult<MutexGuard<'a, System, T>> {
            match mutex.lock() {
                Ok(()) => Ok(MutexGuard {
                    mutex,
                    data,
                    _phantom_lifetime: PhantomData,
                }),
                Err(LockMutexError::BadId) => unreachable!(),
                Err(LockMutexError::BadContext) => Err(LockError::BadContext),
                Err(LockMutexError::Interrupted) => Err(LockError::Interrupted),
                Err(LockMutexError::WouldDeadlock) => Err(LockError::WouldDeadlock),
                Err(LockMutexError::BadParam) => Err(LockError::BadParam),
                Err(LockMutexError::Abandoned) => Err(LockError::Abandoned(MutexGuard {
                    mutex,
                    data,
                    _phantom_lifetime: PhantomData,
                })),
            }
        }

        lock_inner(self.mutex, self.get_ptr())
    }

    /// Attempt to acquire the mutex.
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, System, T>> {
        fn try_lock_inner<'a, System: Kernel, T: 'static>(
            mutex: kernel::Mutex<System>,
            data: *mut T,
        ) -> TryLockResult<MutexGuard<'a, System, T>> {
            match mutex.try_lock() {
                Ok(()) => Ok(MutexGuard {
                    mutex,
                    data,
                    _phantom_lifetime: PhantomData,
                }),
                Err(TryLockMutexError::BadId) => unreachable!(),
                Err(TryLockMutexError::BadContext) => Err(TryLockError::BadContext),
                Err(TryLockMutexError::WouldDeadlock) => Err(TryLockError::WouldDeadlock),
                Err(TryLockMutexError::Timeout) => Err(TryLockError::WouldBlock),
                Err(TryLockMutexError::BadParam) => Err(TryLockError::BadParam),
                Err(TryLockMutexError::Abandoned) => Err(TryLockError::Abandoned(MutexGuard {
                    mutex,
                    data,
                    _phantom_lifetime: PhantomData,
                })),
            }
        }

        try_lock_inner(self.mutex, self.get_ptr())
    }

    /// Mark the state protected by the mutex as consistent.
    #[inline]
    pub fn mark_consistent(&self) -> Result<(), MarkConsistentError> {
        fn mark_consistent_inner<System: Kernel>(
            this: kernel::Mutex<System>,
        ) -> Result<(), MarkConsistentError> {
            this.mark_consistent().map_err(|e| match e {
                MarkConsistentMutexError::BadId => unreachable!(),
                MarkConsistentMutexError::BadContext => MarkConsistentError::BadContext,
                MarkConsistentMutexError::BadObjectState => MarkConsistentError::Consistent,
            })
        }

        mark_consistent_inner(self.mutex)
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
        self.mutex.unlock().unwrap();
    }
}

impl<System: Kernel, T: 'static> core::ops::Deref for MutexGuard<'_, System, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: `MutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &*self.data }
    }
}

impl<System: Kernel, T: 'static> core::ops::DerefMut for MutexGuard<'_, System, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: `MutexGuard` represents a permit acquired from the semaphore,
        //         which grants the bearer an exclusive access to the underlying
        //         data
        unsafe { &mut *self.data }
    }
}
