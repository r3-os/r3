//! Mutexes
use core::{fmt, hash};

use super::{
    raw, raw_cfg, Cfg, LockMutexError, LockMutexTimeoutError, MarkConsistentMutexError,
    QueryMutexError, TryLockMutexError, UnlockMutexError,
};
use crate::time::Duration;

pub use raw::MutexProtocol;

// ----------------------------------------------------------------------------

define_object! {
/// Represents a single mutex in a system.
///
#[doc = common_doc_owned_handle!()]
///
/// Mutexes are similar to binary semaphores (semaphores restricted to one
/// permit at maximum) but differ in some ways, such as the inclusion of a
/// mechanism for preventing unbounded priority inversion.
///
/// When a mutex is locked, it is considered to be owned by the task while the
/// lock is held and can only be unlocked by the same task. This also means that
/// a mutex cannot be locked (even with a non-blocking operation) in a
/// [non-task context], where there is no task to hold the mutex.
///
/// See [`r3::sync::mutex`] for a thread-safe container that uses this
/// `Mutex` internally to protect shared data from concurrent access.
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** Present in many general-purpose and
/// > real-time operating systems.
///
/// [`RawMutexId`]: raw::KernelMutex::RawMutexId
/// [non-task context]: crate#contexts
// Intra-doc links can't refer to downstream crates [ref:downstream_intra_doc_link]
/// [`r3::sync::mutex`]: ../../r3/sync/mutex/index.html
///
/// # Examples
///
/// ```rust
/// #![feature(const_trait_impl)]
/// #![feature(const_mut_refs)]
/// use r3_core::kernel::{
///     LockMutexError, StaticMutex, MutexProtocol, Cfg, traits, prelude::*,
/// };
///
/// struct Objects<System: traits::KernelMutex> {
///     mutex: StaticMutex<System>,
/// }
///
/// const fn configure<C>(cfg: &mut Cfg<C>) -> Objects<C::System>
/// where
///     C: ~const traits::CfgMutex,
/// {
///     let mutex = StaticMutex::define()
///         .protocol(MutexProtocol::Ceiling(1))
///         .finish(cfg);
///     Objects { mutex }
/// }
///
/// fn hoge<System: traits::KernelMutex>(app: &Objects<System>) {
///     match app.mutex.lock() {
///         Ok(()) => {},
///         Err(LockMutexError::Abandoned) => {
///             app.mutex.mark_consistent().unwrap();
///         }
///         Err(e) => panic!("failed to lock the mutex: {e:?}"),
///     }
///     app.mutex.unlock().unwrap();
/// }
/// ```
///
/// # Robustness
///
/// If a task exits while holding a mutex, the mutex is considered to be
/// *abandoned*. An abandoned mutex can still be locked, but the lock function
/// will return `Err(Abandoned)`. **Note that the calling task will receive the
/// ownership of the mutex in this case.** The abandonment state will last until
/// [`Mutex::mark_consistent`] is called on the mutex.
///
/// When a task exits while holding more than one mutex, the order in which the
/// mutexes are abandoned is not specified.
///
/// <div class="admonition-follows"></div>
///
/// > <details>
/// > <summary>Relation to Other Specifications</summary>
/// >
/// > This behavior is based on robust mutexes from POSIX.1-2008
/// > (`PTHREAD_MUTEX_ROBUST`) with one difference:
/// > A mutex never falls into an irrecoverable state — [`Mutex::lock`] would
/// > repeatedly return `Err(Abandoned)` until [`Mutex::mark_consistent`] is
/// > called. This change reduces the internal state bits and the complexity of
/// > the internal logic not to punish normal usage too much. It also loosely
/// > imitates the poisoning semantics of `std::sync::Mutex`.
/// >
/// > A [Win32 mutex] incorporates a flag indicating if the mutex has been
/// > abandoned. An abandoned mutex can be locked as usual, but the wait
/// > function will return `WAIT_ABANDONED`. The flag is cleared automatically,
/// > i.e., unlike POSIX, an abandoned mutex doesn't have to be explicitly
/// > marked consistent.
/// >
/// > In μITRON4.0 and μT-Kernel, abandoned mutexes are implicitly unlocked.
/// >
/// > All of the other operating systems' behavior described above can be
/// > emulated by having a per-mutex flag and performing additional tasks in the
/// > API translation layer.
/// >
/// > </details>
///
/// [Win32 mutex]: https://docs.microsoft.com/en-us/windows/win32/sync/mutex-objects
/// [`Mutex::lock`]: MutexMethods::lock
/// [`Mutex::mark_consistent`]: MutexMethods::mark_consistent
///
/// <div class="admonition-follows"></div>
///
/// > <details>
/// > <summary>Rationale</summary>
/// >
/// > Every customization option brings an additional overhead.
/// > The overhead introduced by the robustness is likely to outweigh the
/// > overhead to provide choices. Therefore, we decided not to add an attribute
/// > to control the robustness.
/// >
/// > We desired a predictable behavior in as many cases a possible, which
/// > excludes the option of leaving the behavior undefined. Failing to unlock
/// > a mutex usually indicates a serious programming error. A future version of
/// > R3 might include functionality to terminate an arbitrary task,
/// > e.g., to respond to a fatal condition such as panicking and a bus error by
/// > containing the fault to the faulting task. In these cases, the data
/// > protected by an abandoned mutex may be left in an inconsistent state
/// > and should be restored to a consistent state before it can be safely
/// > accessed again. To ensure this recommendation is followed correctly
/// > (unless explicitly opted out), we decided to make the robustness the
/// > default behavior.
/// >
/// > </details>
///
/// # Locking Protocols
///
/// `Mutex` supports [the immediate priority ceiling protocol] to avoid
/// unbounded [priority inversion].
///
/// A locking protocol can be chosen by [`MutexDefiner::protocol`][].
/// Additional information can be found at [`MutexProtocol`][].
///
/// [the immediate priority ceiling protocol]: https://en.wikipedia.org/wiki/Priority_ceiling_protocol
/// [priority inversion]: https://en.wikipedia.org/wiki/Priority_inversion
///
/// <div class="admonition-follows"></div>
///
/// > <details>
/// > <summary>Relation to Other Specifications</summary>
/// >
/// > POSIX supports specifying a locking protocol by
/// > `pthread_mutexattr_setprotocol`. The following protocols are supported:
/// > `PTHREAD_PRIO_NONE` (none), `PTHREAD_PRIO_INHERIT`
/// > ([the priority inheritance protocol]), and `PTHREAD_PRIO_PROTECT` (the
/// > immediate priority ceiling protocol).
/// >
/// > μITRON4.0 supports both the priority inheritance protocol and the
/// > immediate priority ceiling protocol. It permits an implementation to
/// > adhere to the simplified priority control rule, which lowers a task's
/// > effective priority only when the task unlocks the last mutex lock held by
/// > the task.
/// >
/// > [Mutexes in ChibiOS/RT] implements the priority inheritance protocol.
/// > Unlock operations must always be performed in lock-reverse order. This
/// > restriction is required for an efficient implementation of the priority
/// > inheritance protocol.
/// >
/// > [Mutexes in ChibiOS/RT]: http://chibios.sourceforge.net/docs3/rt/group__mutexes.html
/// >
/// > Mutexes in the TOPPERS next generation and third generation kernels only
/// > support the immediate priority ceiling protocol. The third generation
/// > kernels further restrict the unlock order to be a lock-reverse order.
/// >
/// > The following table summaries the properties of mutexes in each operating
/// > system or operating system specification.
/// >
/// > |  Specification   | PI  | PC  | Unlock Order | Lower Priority |
/// > | ---------------- | --- | --- | ------------ | -------------- |
/// > | ChibiOS/RT       | yes | no  | lock-reverse | immediate      |
/// > | FreeRTOS         | yes | no  | arbitrary    | last mutex     |
/// > | POSIX            | yes | yes | arbitrary    | immediate      |
/// > | RTEMS            | yes | yes | arbitrary    | last mutex     |
/// > | TOPPERS 3rd Gen  | no  | yes | lock-reverse | immediate      |
/// > | TOPPERS Next Gen | no  | yes | arbitrary    | immediate      |
/// > | VxWorks          | yes | yes | arbitrary    | ?              |
/// > | μITRON4.0        | yes | yes | arbitrary    |                |
/// > | **R3**           | no  | yes | lock-reverse | immediate      |
/// >
/// >  - The **PI** column indicates the availability of
/// >    [the priority inheritance protocol].
/// >
/// >  - The **PC** column indicates the availability of the priority ceiling
/// >    protocol.
/// >
/// >  - The **Unlock Order** column indicates any restrictions imposed on
/// >    the unlocking order.
/// >
/// >  - The **Lower Priority** column indicates whether an owning task's
/// >    priority may be lowered whenever it unlocks a mutex or only when it
/// >    unlocks the last mutex held.
/// >
/// > </details>
///
/// [the priority inheritance protocol]: https://en.wikipedia.org/wiki/Priority_inheritance
///
/// <div class="admonition-follows"></div>
///
/// > <details>
/// > <summary>Rationale</summary>
/// >
/// > There are numerous reasons that led to the decision not to implement the
/// > priority inheritance protocol.
/// >
/// >  - We couldn't afford time to implement and test both protocols at this
/// >    time.
/// >    The entire project is at a prototyping stage, so we would better
/// >    implement the other one when there is an actual need for it.
/// >
/// >  - There are many arguments against using the priority inheritance
/// >    protocol in real-time systems, although they are somewhat out-dated.
/// >
/// >    Victor Yodaiken. “Against priority inheritance.” (2004):
/// >
/// >    > The RTLinux core does not support priority inheritance for a simple
/// >    > reason: priority inheritance is incompatible with reliable real-time
/// >    > system design. Priority inheritance is neither efficient nor
/// >    > reliable. Implementations are either incomplete (and unreliable) or
/// >    > surprisingly complex and intrusive. In fact, the original academic
/// >    > paper presenting priority inheritance \[3\] specifies (and “proves
/// >    > correct”) an inheritance algorithm that is wrong. Worse, the basic
/// >    > intent of the mechanism is to compensate for writing real-time
/// >    > software without taking care of the interaction between priority and
/// >    > mutual exclusion. All too often the result will be incorrect software
/// >    > with errors that are hard to find during test.
/// >
/// >    > Inheritance algorithms are complicated and easy to get wrong. In
/// >    > practice putting priority inheritance into an operating system
/// >    > increases the inversion delays produced by the operating system.
/// >
/// >    > The VxWorks designers originally tried to evade the issue by having a
/// >    > thread retain its highest inherited priority until it released all
/// >    > locks — but this can cause unbounded inversion.
/// >
/// >    Uresh Vahalia. *Unix Internals: The New Frontiers*. Prentice-Hall,
/// >    1996:
/// >
/// >    > Priority inheritance reduces the amount of time a high-priority
/// >    > process must block on resources held by lower-priority processes. The
/// >    > worst-case delay, however, is still much greater than what is
/// >    > acceptable for many real-time applications. One reason is that the
/// >    > blocking chain can grow arbitrarily long.
/// >
/// > We decided to restrict the unlocking order to a lock-reverse order to
/// > minimize the cost of maintaining the list of mutexes held by a task.
/// >
/// > </details>
///
#[doc = include_str!("../common.md")]
pub struct Mutex<System: _>(System::RawMutexId);

/// Represents a single borrowed mutex in a system.
#[doc = include_str!("../common.md")]
pub struct MutexRef<System: raw::KernelMutex>(_);

pub type StaticMutex<System>;

pub trait MutexHandle {}
pub trait MutexMethods {}
}

impl<System: raw::KernelMutex> StaticMutex<System> {
    /// Construct a `MutexDefiner` to define a mutex in [a
    /// configuration function](crate#static-configuration).
    pub const fn define() -> MutexDefiner<System> {
        MutexDefiner::new()
    }
}

/// The supported operations on [`MutexHandle`].
#[doc = include_str!("../common.md")]
pub trait MutexMethods: MutexHandle {
    /// Get a flag indicating whether the mutex is currently locked.
    #[inline]
    fn is_locked(&self) -> Result<bool, QueryMutexError> {
        // Safety: `Mutex` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelMutex>::raw_mutex_is_locked(self.id()) }
    }

    /// Unlock the mutex.
    ///
    /// Mutexes must be unlocked in a lock-reverse order, or this method may
    /// return [`UnlockMutexError::BadObjectState`].
    #[inline]
    fn unlock(&self) -> Result<(), UnlockMutexError> {
        // Safety: `Mutex` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelMutex>::raw_mutex_unlock(self.id()) }
    }

    /// Acquire the mutex, blocking the current thread until it is able to do
    /// so.
    ///
    /// An [abandoned mutex] can still be locked, but this method will return
    /// `Err(Abandoned)`. **Note that the current task will receive the
    /// ownership of the mutex even in this case.**
    ///
    /// [abandoned mutex]: #robustness
    ///
    /// This system service may block. Therefore, calling this method is not
    /// allowed in [a non-waitable context] and will return `Err(BadContext)`.
    ///
    /// [a non-waitable context]: crate#contexts
    #[inline]
    fn lock(&self) -> Result<(), LockMutexError> {
        // Safety: `Mutex` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelMutex>::raw_mutex_lock(self.id()) }
    }

    /// [`lock`](Self::lock) with timeout.
    #[inline]
    fn lock_timeout(&self, timeout: Duration) -> Result<(), LockMutexTimeoutError> {
        // Safety: `Mutex` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelMutex>::raw_mutex_lock_timeout(self.id(), timeout) }
    }

    /// Non-blocking version of [`lock`](Self::lock). Returns
    /// immediately with [`TryLockMutexError::Timeout`] if the unblocking
    /// condition is not satisfied.
    ///
    /// Note that unlike [`Semaphore::poll_one`], this operation is disallowed
    /// in a non-task context because a mutex lock needs an owning task.
    ///
    /// [`Semaphore::poll_one`]: crate::kernel::semaphore::SemaphoreMethods::poll_one
    #[inline]
    fn try_lock(&self) -> Result<(), TryLockMutexError> {
        // Safety: `Mutex` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelMutex>::raw_mutex_try_lock(self.id()) }
    }

    /// Mark the state protected by the mutex as consistent.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Relation to Other Specifications:** Equivalent to
    /// > `pthread_mutex_consistent` from POSIX.1-2008.
    ///
    #[inline]
    fn mark_consistent(&self) -> Result<(), MarkConsistentMutexError> {
        // Safety: `Mutex` represents a permission to access the
        //         referenced object.
        unsafe { <Self::System as raw::KernelMutex>::raw_mutex_mark_consistent(self.id()) }
    }
}

impl<T: MutexHandle> MutexMethods for T {}

// ----------------------------------------------------------------------------

/// The definer (static builder) for [`MutexRef`][].
#[must_use = "must call `finish()` to complete registration"]
pub struct MutexDefiner<System> {
    inner: raw_cfg::MutexDescriptor<System>,
}

impl<System: raw::KernelMutex> MutexDefiner<System> {
    const fn new() -> Self {
        Self {
            inner: raw_cfg::MutexDescriptor {
                phantom: core::marker::PhantomData,
                protocol: MutexProtocol::None,
            },
        }
    }

    /// Specify the mutex's protocol. Defaults to `None` when unspecified.
    pub const fn protocol(self, protocol: MutexProtocol) -> Self {
        Self {
            inner: raw_cfg::MutexDescriptor {
                protocol,
                ..self.inner
            },
        }
    }

    /// Complete the definition of a mutex, returning a reference to the
    /// mutex.
    pub const fn finish<C: ~const raw_cfg::CfgMutex<System = System>>(
        self,
        c: &mut Cfg<C>,
    ) -> StaticMutex<System> {
        let id = c.raw().mutex_define(self.inner, ());
        unsafe { MutexRef::from_id(id) }
    }
}
