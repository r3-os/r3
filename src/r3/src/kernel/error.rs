use core::{fmt, mem::transmute};

/// The macro to define [`ResultCode`].
macro_rules! define_result_code {
    (
        $( #[$meta:meta] )*
        pub enum ResultCode {
            $(
                $( #[$vmeta:meta] )*
                $vname:ident = $vd:expr
            ),* $(,)*
        }
    ) => {
        $( #[$meta] )*
        pub enum ResultCode {
            $(
                $( #[$vmeta] )*
                $vname = $vd
            ),*
        }

        impl ResultCode {
            /// Get the short name of the result code.
            ///
            /// # Examples
            ///
            /// ```
            /// use r3::kernel::ResultCode;
            /// assert_eq!(ResultCode::BadObjectState.as_str(), "BadObjectState");
            /// ```
            pub fn as_str(self) -> &'static str {
                match self {
                    $(
                        Self::$vname => stringify!($vname),
                    )*
                }
            }

            fn fmt(self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl fmt::Debug for ResultCode {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                (*self).fmt(f)
            }
        }
    };
}

define_result_code! {
    /// All result codes (including success) that the C API can return.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Relation to Other Specifications:** All error codes are intentionally
    /// > matched to their equivalents in μITRON4.0 for no particular reasons.
    ///
    /// <div class="admonition-follows"></div>
    ///
    /// > **Rationale:** Using the C API result codes internally reduces the
    /// > interop overhead at an API surface.
    ///
    #[doc = include_str!("../common.md")]
    #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[repr(i8)]
    pub enum ResultCode {
        /// The operation was successful. No additional information is available.
        Success = 0,
        /// The operation is not supported.
        NotSupported = -9,
        /// A parameter is invalid in a way that is no covered by any other error
        /// codes.
        BadParam = -17,
        /// A specified object identifier ([`Id`]) is invalid.
        ///
        /// [`Id`]: super::Id
        BadId = -18,
        /// The current context disallows the operation.
        BadContext = -25,
        /// The caller does not own the resource.
        NotOwner = -29,
        /// Resource deadlock would occur.
        WouldDeadlock = -30,
        /// A target object is in a state that disallows the operation.
        BadObjectState = -41,
        /// An operation or an object couldn't be enqueued because there are too
        /// many of such things that already have been enqueued.
        QueueOverflow = -43,
        /// The owner of a mutex exited while holding the mutex lock.
        Abandoned = -44,
        /// The wait operation was interrupted by [`Task::interrupt`].
        ///
        /// [`Task::interrupt`]: crate::kernel::task::TaskMethods::interrupt
        Interrupted = -49,
        /// The operation timed out.
        Timeout = -50,
    }
}

impl ResultCode {
    /// Get a flag indicating whether the code represents a failure.
    ///
    /// Failure codes have negative values.
    #[inline]
    pub fn is_err(self) -> bool {
        (self as i8) < 0
    }

    /// Get a flag indicating whether the code represents a success.
    ///
    /// Success codes have non-negative values.
    #[inline]
    pub fn is_ok(self) -> bool {
        !self.is_err()
    }
}

macro_rules! define_error {
    (
        mod $mod_name:ident {}
        $( #[$meta:meta] )*
        $vis:vis enum $name:ident $(: $($subty:ident),* $(,)*)? {
            $(
                $( #[$vmeta:meta] )*
                $vname:ident
            ),* $(,)*
        }
    ) => {
        $( #[$meta] )*
        ///
        /// See [`ResultCode`] for all result codes and generic descriptions.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(i8)]
        $vis enum $name {
            $(
                $( #[$vmeta] )*
                // Use the same discriminants as `ResultCode` for cost-free
                // conversion
                $vname = ResultCode::$vname as i8
            ),*
        }

        impl fmt::Debug for $name {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                ResultCode::from(*self).fmt(f)
            }
        }

        impl From<Result<(), $name>> for ResultCode {
            #[inline]
            fn from(x: Result<(), $name>) -> Self {
                match x {
                    Ok(()) => Self::Success,
                    Err(e) => Self::from(e),
                }
            }
        }

        impl From<$name> for ResultCode {
            #[inline]
            fn from(x: $name) -> Self {
                // Safety: `ResultCode` and `$name` has the same representation
                //         type, and the representation of `ResultCode` is a
                //         superset of `x`.
                unsafe { transmute(x) }
            }
        }

        #[cfg(test)]
        mod $mod_name {
            use super::*;

            #[test]
            fn to_result_code() {
                $(
                    assert_eq!(
                        ResultCode::$vname,
                        ResultCode::from($name::$vname),
                    );
                )*
            }

            #[test]
            fn result_to_result_code() {
                $(
                    assert_eq!(
                        ResultCode::$vname,
                        ResultCode::from(Err($name::$vname)),
                    );
                )*
                assert_eq!(
                    ResultCode::Success,
                    ResultCode::from(Result::<(), $name>::Ok(())),
                );
            }
        }

        $($(
            $subty!(impl From<_> for $name);
        )*)?

        #[allow(unused_macros)]
        macro_rules! $name {
            (impl From<_> for $dest_ty:ty) => {
                impl From<$name> for $dest_ty {
                    #[inline]
                    fn from(x: $name) -> Self {
                        match x {
                            $(
                                $name::$vname => Self::$vname,
                            )*
                        }
                    }
                }
            };
        }
    };
}

define_error! {
    mod activate_task_error {}
    /// Error type for [`Task::activate`].
    ///
    /// [`Task::activate`]: super::task::TaskMethods::activate
    pub enum ActivateTaskError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The task is already active (not in the Dormant state).
        ///
        /// This error code originates from `E_QOVR` defined in the μITRON 4.0
        /// specification. In this specification, the `act_tsk` (activate task)
        /// system service works by enqueueing an activation request. `E_QOVR`
        /// is used to report a condition in which an enqueue count limit has
        /// been reached. Our kernel doesn't support enqueueing activation
        /// request (at the moment), so any attempts to activate an
        /// already-active task will fail.
        QueueOverflow,
    }
}

define_error! {
    mod get_current_task_error {}
    /// Error type for [`LocalTask::current`].
    ///
    /// [`LocalTask::current`]: super::task::LocalTask::current
    pub enum GetCurrentTaskError {
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod interrupt_task_error {}
    /// Error type for [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: super::task::TaskMethods::interrupt
    pub enum InterruptTaskError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The task is not in the Waiting state.
        BadObjectState,
    }
}

define_error! {
    mod set_task_priority_error {}
    /// Error type for [`Task::set_priority`].
    ///
    /// [`Task::set_priority`]: super::task::TaskMethods::set_priority
    pub enum SetTaskPriorityError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The priority is out of range, or the task owns a mutex created with
        /// with the protocol attribute having the value [`Ceiling`] and the
        /// task's new priority is higher than the mutex's priority ceiling.
        ///
        /// [`Ceiling`]: crate::kernel::MutexProtocol::Ceiling
        BadParam,
        /// The task is in the Dormant state.
        BadObjectState,
    }
}

define_error! {
    mod get_task_priority_error {}
    /// Error type for [`Task::priority`].
    ///
    /// [`Task::priority`]: super::task::TaskMethods::priority
    pub enum GetTaskPriorityError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The task is in the Dormant state.
        BadObjectState,
    }
}

define_error! {
    mod exit_task_error {}
    /// Error type for [`Kernel::exit_task`].
    ///
    /// [`Kernel::exit_task`]: super::Kernel::exit_task
    pub enum ExitTaskError {
        /// CPU Lock is active, or the current context is not a task context.
        BadContext,
    }
}

define_error! {
    mod cpu_lock_error {}
    /// Error type for [`Kernel::acquire_cpu_lock`] and
    /// [`Kernel::release_cpu_lock`].
    ///
    /// [`Kernel::acquire_cpu_lock`]: super::Kernel::acquire_cpu_lock
    /// [`Kernel::release_cpu_lock`]: super::Kernel::release_cpu_lock
    pub enum CpuLockError {
        /// CPU Lock is already active or inactive.
        BadContext,
    }
}

define_error! {
    mod boost_priority_error {}
    /// Error type for [`Kernel::boost_priority`] and
    /// [`Kernel::unboost_priority`].
    ///
    /// [`Kernel::boost_priority`]: super::Kernel::boost_priority
    /// [`Kernel::unboost_priority`]: super::Kernel::unboost_priority
    pub enum BoostPriorityError {
        /// Priority Boost is already active or inactive, the current
        /// context is not a task context, or CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod time_error {}
    /// Error type for [`Kernel::time`] and
    /// [`Kernel::set_time`].
    ///
    /// [`Kernel::time`]: super::Kernel::time
    /// [`Kernel::set_time`]: super::Kernel::set_time
    pub enum TimeError {
        /// The current context is not a task context, or CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod adjust_time_error {}
    /// Error type for [`Kernel::adjust_time`].
    ///
    /// [`Kernel::adjust_time`]: super::Kernel::adjust_time
    pub enum AdjustTimeError {
        /// CPU Lock is active.
        BadContext,
        /// The requested adjustment is not possible under the current system
        /// state.
        BadObjectState,
    }
}

define_error! {
    mod wait_error {}
    /// Error type for wait operations such as [`EventGroup::wait`].
    ///
    /// [`EventGroup::wait`]: super::event_group::EventGroupMethods::wait
    pub enum WaitError {
        Interrupted,
    }
}

define_error! {
    mod wait_timeout_error {}
    /// Error type for wait operations with timeout such as
    /// [`EventGroup::wait_timeout`].
    ///
    /// [`EventGroup::wait_timeout`]: super::event_group::EventGroupMethods::wait_timeout
    pub enum WaitTimeoutError: WaitError {
        Interrupted,
        Timeout,
    }
}

define_error! {
    mod park_error {}
    /// Error type for [`Kernel::park`].
    ///
    /// [`Kernel::park`]: super::Kernel::park
    pub enum ParkError: WaitError {
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
    }
}

define_error! {
    mod park_timeout_error {}
    /// Error type for [`Kernel::park_timeout`].
    ///
    /// [`Kernel::park_timeout`]: super::Kernel::park_timeout
    pub enum ParkTimeoutError: WaitTimeoutError {
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
        Timeout,
        /// The timeout duration is negative.
        BadParam,
    }
}

define_error! {
    mod unpark_error {}
    /// Error type for [`Task::unpark`].
    ///
    /// [`Task::unpark`]: super::task::TaskMethods::unpark
    pub enum UnparkError {
        /// CPU Lock is active.
        BadContext,
        /// The task ID is out of range.
        BadId,
        /// The task is in the Dormant state.
        BadObjectState,
    }
}

define_error! {
    mod unpark_exact_error {}
    /// Error type for [`Task::unpark_exact`].
    ///
    /// [`Task::unpark_exact`]: super::task::TaskMethods::unpark_exact
    pub enum UnparkExactError {
        /// CPU Lock is active.
        BadContext,
        /// The task ID is out of range.
        BadId,
        /// The task already has a token.
        QueueOverflow,
        /// The task is in the Dormant state.
        BadObjectState,
    }
}

define_error! {
    mod sleep_error {}
    /// Error type for [`Kernel::sleep`].
    ///
    /// [`Kernel::sleep`]: super::Kernel::sleep
    pub enum SleepError {
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
        /// The duration is negative.
        BadParam,
    }
}
define_error! {
    mod update_event_group_error {}
    /// Error type for [`EventGroup::set`] and [`EventGroup::clear`].
    ///
    /// [`EventGroup::set`]: super::event_group::EventGroupMethods::set
    /// [`EventGroup::clear`]: super::event_group::EventGroupMethods::clear
    pub enum UpdateEventGroupError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod get_event_group_error {}
    /// Error type for [`EventGroup::get`].
    ///
    /// [`EventGroup::get`]: super::event_group::EventGroupMethods::get
    pub enum GetEventGroupError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod poll_event_group_error {}
    /// Error type for [`EventGroup::poll`].
    ///
    /// [`EventGroup::poll`]: super::event_group::EventGroupMethods::poll
    pub enum PollEventGroupError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        Timeout,
    }
}

define_error! {
    mod wait_event_group_error {}
    /// Error type for [`EventGroup::wait`].
    ///
    /// [`EventGroup::wait`]: super::event_group::EventGroupMethods::wait
    pub enum WaitEventGroupError: WaitError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
    }
}

define_error! {
    mod wait_event_group_timeout_error {}
    /// Error type for [`EventGroup::wait_timeout`].
    ///
    /// [`EventGroup::wait_timeout`]: super::event_group::EventGroupMethods::wait_timeout
    pub enum WaitEventGroupTimeoutError: WaitTimeoutError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
        Timeout,
        /// The timeout duration is negative.
        BadParam,
    }
}

define_error! {
    mod get_semaphore_error {}
    /// Error type for [`Semaphore::get`].
    ///
    /// [`Semaphore::get`]: super::semaphore::SemaphoreMethods::get
    pub enum GetSemaphoreError {
        /// The semaphore ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod drain_semaphore_error {}
    /// Error type for [`Semaphore::drain`].
    ///
    /// [`Semaphore::drain`]: super::semaphore::SemaphoreMethods::drain
    pub enum DrainSemaphoreError {
        /// The semaphore ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod signal_semaphore_error {}
    /// Error type for [`Semaphore::signal`].
    ///
    /// [`Semaphore::signal`]: super::semaphore::SemaphoreMethods::signal
    pub enum SignalSemaphoreError {
        /// The semaphore ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The semaphore value is already at the maximum value.
        QueueOverflow,
    }
}

define_error! {
    mod poll_semaphore_error {}
    /// Error type for [`Semaphore::poll_one`].
    ///
    /// [`Semaphore::poll_one`]: super::semaphore::SemaphoreMethods::poll_one
    pub enum PollSemaphoreError {
        /// The semaphore ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        Timeout,
    }
}

define_error! {
    mod wait_semaphore_error {}
    /// Error type for [`Semaphore::wait_one`].
    ///
    /// [`Semaphore::wait_one`]: super::semaphore::SemaphoreMethods::wait_one
    pub enum WaitSemaphoreError: WaitError {
        /// The semaphore ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
    }
}

define_error! {
    mod wait_semaphore_timeout_error {}
    /// Error type for [`Semaphore::wait_one_timeout`].
    ///
    /// [`Semaphore::wait_one_timeout`]: super::semaphore::SemaphoreMethods::wait_one_timeout
    pub enum WaitSemaphoreTimeoutError: WaitTimeoutError {
        /// The semaphore ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
        Timeout,
        /// The timeout duration is negative.
        BadParam,
    }
}

define_error! {
    mod query_mutex_error {}
    /// Error type for [`Mutex::is_locked`].
    ///
    /// [`Mutex::is_locked`]: super::mutex::MutexMethods::is_locked
    pub enum QueryMutexError {
        /// The mutex ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod unlock_mutex_error {}
    /// Error type for [`Mutex::unlock`].
    ///
    /// [`Mutex::unlock`]: super::mutex::MutexMethods::unlock
    pub enum UnlockMutexError {
        /// The mutex ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        /// The current task does not currently own the mutex.
        NotOwner,
        /// The correct mutex unlocking order is violated.
        BadObjectState,
    }
}

define_error! {
    mod try_lock_mutex_error {}
    /// Error type for [`Mutex::try_lock`].
    ///
    /// [`Mutex::try_lock`]: super::mutex::MutexMethods::try_lock
    pub enum TryLockMutexError {
        /// The mutex ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not a [task context].
        ///
        /// [task context]: crate#contexts
        BadContext,
        Timeout,
        /// The current task already owns the mutex.
        WouldDeadlock,
        /// The mutex was created with the protocol attribute having the value
        /// [`Ceiling`] and the current task's priority is higher than the
        /// mutex's priority ceiling.
        ///
        /// [`Ceiling`]: crate::kernel::MutexProtocol::Ceiling
        BadParam,
        /// The previous owning task exited while holding the mutex lock. *The
        /// current task shall hold the mutex lock*, but is up to make the
        /// state consistent.
        Abandoned,
    }
}

define_error! {
    mod lock_mutex_error {}
    /// Error type for [`Mutex::lock`].
    ///
    /// [`Mutex::lock`]: super::mutex::MutexMethods::lock
    pub enum LockMutexError: WaitError
    {
        /// The mutex ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
        /// The current task already owns the mutex.
        WouldDeadlock,
        /// The mutex was created with the protocol attribute having the value
        /// [`Ceiling`] and the current task's priority is higher than the
        /// mutex's priority ceiling.
        ///
        /// [`Ceiling`]: crate::kernel::MutexProtocol::Ceiling
        BadParam,
        /// The previous owning task exited while holding the mutex lock. *The
        /// current task shall hold the mutex lock*, but is up to make the
        /// state consistent.
        Abandoned,
    }
}

define_error! {
    mod lock_mutex_timeout_error {}
    /// Error type for [`Mutex::lock_timeout`].
    ///
    /// [`Mutex::lock_timeout`]: super::mutex::MutexMethods::lock_timeout
    pub enum LockMutexTimeoutError: WaitTimeoutError {
        /// The mutex ID is out of range.
        BadId,
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
        Timeout,
        /// The current task already owns the mutex.
        WouldDeadlock,
        /// The timeout duration is negative, or the mutex was created with the
        /// protocol attribute having the value [`Ceiling`] and the current
        /// task's priority is higher than the mutex's priority ceiling.
        ///
        /// [`Ceiling`]: crate::kernel::MutexProtocol::Ceiling
        BadParam,
        /// The previous owning task exited while holding the mutex lock. *The
        /// current task shall hold the mutex lock*, but is up to make the
        /// state consistent.
        Abandoned,
    }
}

define_error! {
    mod mark_consistent_mutex_error {}
    /// Error type for [`Mutex::mark_consistent`].
    ///
    /// [`Mutex::mark_consistent`]: super::mutex::MutexMethods::mark_consistent
    pub enum MarkConsistentMutexError {
        /// The mutex ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The mutex does not protect an inconsistent state.
        BadObjectState,
    }
}

define_error! {
    mod set_interrupt_line_priority_error {}
    /// Error type for [`InterruptLine::set_priority`] and
    /// [`InterruptLine::set_priority_unchecked`].
    ///
    /// [`InterruptLine::set_priority`]: super::InterruptLine::set_priority
    /// [`InterruptLine::set_priority_unchecked`]: super::InterruptLine::set_priority_unchecked
    pub enum SetInterruptLinePriorityError {
        /// The operation is not supported by the kernel.
        NotSupported,
        /// CPU Lock is active, or the current context is not [a task context].
        ///
        /// [a task context]: crate#contexts
        BadContext,
        /// The specified interrupt number or the specified priority value is
        /// out of range.
        BadParam,
    }
}

define_error! {
    mod enable_interrupt_line_error {}
    /// Error type for [`InterruptLine::enable`] and [`InterruptLine::disable`].
    ///
    /// [`InterruptLine::enable`]: super::InterruptLine::enable
    /// [`InterruptLine::disable`]: super::InterruptLine::disable
    pub enum EnableInterruptLineError {
        /// The operation is not supported by the kernel.
        NotSupported,
        /// Enabling or disabling the specified interrupt line is not supported.
        BadParam,
    }
}

define_error! {
    mod pend_interrupt_line_error {}
    /// Error type for [`InterruptLine::pend`].
    ///
    /// [`InterruptLine::pend`]: super::InterruptLine::pend
    pub enum PendInterruptLineError {
        /// Setting a pending flag is not supported by the kernel.
        NotSupported,
        /// Setting the pending flag of the specified interrupt line is not
        /// supported.
        BadParam,
        /// The interrupt line is not configured to allow this operation. For
        /// example, this operation is invalid for an level-triggered interrupt
        /// line.
        ///
        /// A kernel is not required to detect this condition.
        BadObjectState,
    }
}

define_error! {
    mod clear_interrupt_line_error {}
    /// Error type for [`InterruptLine::clear`].
    ///
    /// [`InterruptLine::clear`]: super::InterruptLine::clear
    pub enum ClearInterruptLineError {
        /// Clearing a pending flag is not supported by the kernel.
        NotSupported,
        /// Clearing the pending flag of the specified interrupt line is not
        /// supported.
        BadParam,
        /// The interrupt line is not configured to allow this operation. For
        /// example, this operation is invalid for an level-triggered interrupt
        /// line.
        ///
        /// A kernel is not required to detect this condition.
        BadObjectState,
    }
}

define_error! {
    mod query_interrupt_line_error {}
    /// Error type for [`InterruptLine::is_pending`].
    ///
    /// [`InterruptLine::is_pending`]: super::InterruptLine::is_pending
    pub enum QueryInterruptLineError {
        /// Reading a pending flag is not supported by the kernel.
        NotSupported,
        /// Reading the pending flag of the specified interrupt line is not
        /// supported.
        BadParam,
    }
}

define_error! {
    mod start_timer_error {}
    /// Error type for [`Timer::start`].
    ///
    /// [`Timer::start`]: super::timer::TimerMethods::start
    pub enum StartTimerError {
        /// The timer ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod stop_timer_error {}
    /// Error type for [`Timer::stop`].
    ///
    /// [`Timer::stop`]: super::timer::TimerMethods::stop
    pub enum StopTimerError {
        /// The timer ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    mod set_timer_delay_error {}
    /// Error type for [`Timer::set_delay`].
    ///
    /// [`Timer::set_delay`]: super::timer::TimerMethods::set_delay
    pub enum SetTimerDelayError {
        /// The timer ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The duration is negative.
        BadParam,
    }
}

define_error! {
    mod set_timer_period_error {}
    /// Error type for [`Timer::set_period`].
    ///
    /// [`Timer::set_period`]: super::timer::TimerMethods::set_period
    pub enum SetTimerPeriodError {
        /// The timer ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The duration is negative.
        BadParam,
    }
}
