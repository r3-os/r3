//! Kernel state locking mechanism
use core::{fmt, ops};
use tokenlock::UnsyncTokenLock;

use crate::{
    error::BadContextError,
    utils::{intrusive_list::CellLike, Init},
    PortThreading,
};

pub(super) struct CpuLockTag<Traits>(Traits);

/// The key that "unlocks" [`CpuLockCell`].
pub(super) type CpuLockToken<Traits> = tokenlock::UnsyncSingletonToken<CpuLockTag<Traits>>;

/// The keyhole type for [`UnsyncTokenLock`] that can be "unlocked" by
/// [`CpuLockToken`].
pub(super) type CpuLockKeyhole<Traits> = tokenlock::SingletonTokenId<CpuLockTag<Traits>>;

/// Cell type that can be accessed by [`CpuLockToken`] (which can be obtained
/// by [`lock_cpu`]).
pub(super) struct CpuLockCell<Traits, T: ?Sized>(UnsyncTokenLock<T, CpuLockKeyhole<Traits>>);

impl<Traits, T> CpuLockCell<Traits, T> {
    pub(super) const fn new(x: T) -> Self {
        Self(UnsyncTokenLock::new(CpuLockKeyhole::INIT, x))
    }
}

impl<Traits: PortThreading, T: ?Sized> CpuLockCell<Traits, T> {
    /// Clone the contents and apply debug formatting.
    ///
    /// `CpuLockCell` needs to acquire CPU Lock when doing debug formatting and
    /// fails to do so if CPU Lock is already active. This means nested
    /// `CpuLockCell` won't be printed.
    ///
    /// The debug formatting proxy returned by this method releases CPU Lock
    /// before printing the contents, thus allowing any contained `CpuLockCell`s
    /// to be printed.
    pub(super) fn get_and_debug_fmt(&self) -> impl fmt::Debug + '_
    where
        T: Clone + fmt::Debug,
    {
        self.debug_fmt_with(|x, f| x.fmt(f))
    }

    /// Return a debug formatting proxy of the cell. The given closure is used
    /// to format the cloned contents.
    pub(super) fn debug_fmt_with<'a, F: 'a + Fn(T, &mut fmt::Formatter) -> fmt::Result>(
        &'a self,
        f: F,
    ) -> impl fmt::Debug + 'a
    where
        T: Clone,
    {
        struct DebugFmtWith<'a, Traits, T: ?Sized, F> {
            cell: &'a CpuLockCell<Traits, T>,
            f: F,
        }

        impl<Traits: PortThreading, T: Clone, F: Fn(T, &mut fmt::Formatter) -> fmt::Result>
            fmt::Debug for DebugFmtWith<'_, Traits, T, F>
        {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                if let Ok(lock) = lock_cpu() {
                    let inner = self.cell.0.read(&*lock).clone();
                    drop(lock);

                    f.write_str("CpuLockCell(")?;
                    (self.f)(inner, f)?;
                    f.write_str(")")
                } else {
                    f.write_str("CpuLockCell(< locked >)")
                }
            }
        }

        DebugFmtWith { cell: self, f }
    }

    /// Return a debug formatting proxy of the cell. The given closure is used
    /// to format the borrowed contents. Note that CPU Lock is active when the
    /// closure is called.
    pub(super) fn debug_fmt_with_ref<'a, F: 'a + Fn(&T, &mut fmt::Formatter) -> fmt::Result>(
        &'a self,
        f: F,
    ) -> impl fmt::Debug + 'a {
        struct DebugFmtWithRef<'a, Traits, T: ?Sized, F> {
            cell: &'a CpuLockCell<Traits, T>,
            f: F,
        }

        impl<Traits: PortThreading, T: ?Sized, F: Fn(&T, &mut fmt::Formatter) -> fmt::Result>
            fmt::Debug for DebugFmtWithRef<'_, Traits, T, F>
        {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                if let Ok(lock) = lock_cpu() {
                    f.write_str("CpuLockCell(")?;
                    (self.f)(self.cell.0.read(&*lock), f)?;
                    f.write_str(")")
                } else {
                    f.write_str("CpuLockCell(< locked >)")
                }
            }
        }

        DebugFmtWithRef { cell: self, f }
    }
}

impl<Traits: PortThreading, T: fmt::Debug> fmt::Debug for CpuLockCell<Traits, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.debug_fmt_with_ref(|x, f| x.fmt(f)).fmt(f)
    }
}

impl<Traits, T: Init> Init for CpuLockCell<Traits, T> {
    const INIT: Self = Self(Init::INIT);
}

impl<Traits, T> ops::Deref for CpuLockCell<Traits, T> {
    type Target = UnsyncTokenLock<T, CpuLockKeyhole<Traits>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Traits, T> ops::DerefMut for CpuLockCell<Traits, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, Element: Clone, Traits: PortThreading> CellLike<&'a mut CpuLockGuard<Traits>>
    for CpuLockCell<Traits, Element>
{
    type Target = Element;

    fn get(&self, key: &&'a mut CpuLockGuard<Traits>) -> Self::Target {
        (**self).get(&***key)
    }
    fn set(&self, key: &mut &'a mut CpuLockGuard<Traits>, value: Self::Target) {
        CellLike::set(&**self, &mut &mut ***key, value);
    }
    fn modify<T>(
        &self,
        key: &mut &'a mut CpuLockGuard<Traits>,
        f: impl FnOnce(&mut Self::Target) -> T,
    ) -> T {
        CellLike::modify(&**self, &mut &mut ***key, f)
    }
}

impl<'a, Element: Clone, Traits: PortThreading> CellLike<CpuLockTokenRefMut<'a, Traits>>
    for CpuLockCell<Traits, Element>
{
    type Target = Element;

    fn get(&self, key: &CpuLockTokenRefMut<'a, Traits>) -> Self::Target {
        (**self).get(&**key)
    }
    fn set(&self, key: &mut CpuLockTokenRefMut<'a, Traits>, value: Self::Target) {
        CellLike::set(&**self, &mut &mut **key, value);
    }
    fn modify<T>(
        &self,
        key: &mut CpuLockTokenRefMut<'a, Traits>,
        f: impl FnOnce(&mut Self::Target) -> T,
    ) -> T {
        CellLike::modify(&**self, &mut &mut **key, f)
    }
}

/// Attempt to enter a CPU Lock state and get an RAII guard.
/// Return `BadContext` if the kernel is already in a CPU Lock state.
pub(super) fn lock_cpu<Traits: PortThreading>() -> Result<CpuLockGuard<Traits>, BadContextError> {
    // Safety: `try_enter_cpu_lock` is only meant to be called by the kernel
    if unsafe { Traits::try_enter_cpu_lock() } {
        // Safety: We just entered a CPU Lock state. This also means there are
        //         no instances of `CpuLockGuard` existing at this point.
        Ok(unsafe { assume_cpu_lock() })
    } else {
        Err(BadContextError::BadContext)
    }
}

/// Assume a CPU Lock state and get `CpuLockGuard`.
///
/// # Safety
///
/// The system must be really in a CPU Lock state. There must be no instances of
/// `CpuLockGuard` existing at the point of the call.
pub(super) unsafe fn assume_cpu_lock<Traits: PortThreading>() -> CpuLockGuard<Traits> {
    debug_assert!(Traits::is_cpu_lock_active());

    CpuLockGuard {
        // Safety: There are no other instances of `CpuLockToken`; this is
        //         upheld by the caller.
        token: unsafe { CpuLockToken::new_unchecked() },
    }
}

/// RAII guard for a CPU Lock state.
///
/// [`CpuLockToken`] can be borrowed from this type.
pub(super) struct CpuLockGuard<Traits: PortThreading> {
    token: CpuLockToken<Traits>,
}

impl<Traits: PortThreading> CpuLockGuard<Traits> {
    /// Construct a [`CpuLockTokenRefMut`] by borrowing `self`.
    pub(super) fn borrow_mut(&mut self) -> CpuLockTokenRefMut<'_, Traits> {
        self.token.borrow_mut()
    }
}

impl<Traits: PortThreading> Drop for CpuLockGuard<Traits> {
    fn drop(&mut self) {
        // Safety: CPU Lock is currently active, and it's us (the kernel) who
        // are currently controlling the CPU Lock state
        unsafe {
            Traits::leave_cpu_lock();
        }
    }
}

impl<Traits: PortThreading> ops::Deref for CpuLockGuard<Traits> {
    type Target = CpuLockToken<Traits>;
    fn deref(&self) -> &Self::Target {
        &self.token
    }
}

impl<Traits: PortThreading> ops::DerefMut for CpuLockGuard<Traits> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.token
    }
}

/// Borrowed version of [`CpuLockGuard`]. This is equivalent to
/// `&'a mut CpuLockGuard` but does not consume memory.
///
///  - Always prefer this over `&mut CpuLockGuard` in function parameters.
///  - When you pass `&'a mut _` to a function, the compiler automatically
///    reborrows it as `&'b mut _` so that the original `&'a mut _` remains
///    accessible after the function call. This does not happen with
///    `CpuLockTokenRefMut`. You have to call [`borrow_mut`] manually.
///
/// [`borrow_mut`]: tokenlock::UnsyncSingletonTokenRefMut::borrow_mut
pub(super) type CpuLockTokenRefMut<'a, Traits> =
    tokenlock::UnsyncSingletonTokenRefMut<'a, CpuLockTag<Traits>>;

/// Borrowed version of [`CpuLockGuard`]. This is equivalent to
/// `&'a CpuLockGuard` but does not consume memory.
///
/// Compared to [`CpuLockTokenRefMut`], this is only used in very limited
/// circumstances, such as allowing a callback function to mutate the contents
/// of `CpuLockCell<Traits, Cell<_>>` cells belonging to its implementor but not
/// to reenter its caller.
pub(super) type CpuLockTokenRef<'a, Traits> =
    tokenlock::UnsyncSingletonTokenRef<'a, CpuLockTag<Traits>>;
