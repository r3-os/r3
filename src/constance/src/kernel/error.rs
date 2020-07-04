macro_rules! define_error {
    (
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
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(i8)]
        $vis enum $name {
            $(
                $( #[$vmeta] )*
                // Use the same discriminants as `ResultCode` for cost-free
                // conversion
                $vname = ResultCode::$vname as i8
            ),*
        }

        impl From<Result<(), $name>> for ResultCode {
            #[inline]
            fn from(x: Result<(), $name>) -> Self {
                match x {
                    Ok(()) => Self::Success,
                    $(
                        Err($name::$vname) => Self::$vname,
                    )*
                }
            }
        }

        impl From<$name> for ResultCode {
            #[inline]
            fn from(x: $name) -> Self {
                match x {
                    $(
                        $name::$vname => Self::$vname,
                    )*
                }
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
#[doc(include = "../common.md")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    /// A target object is in a state that disallows the operation.
    BadObjectState = -41,
    /// An operation or an object couldn't be enqueued because there are too
    /// many of such things that already have been enqueued.
    QueueOverflow = -43,
    /// The wait operation was interrupted by [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: crate::kernel::Task::interrupt
    Interrupted = -49,
    /// The operation timed out.
    Timeout = -50,
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

define_error! {
    pub(super) enum BadContextError {
        BadContext,
    }
}

define_error! {
    pub(super) enum BadIdError {
        BadId,
    }
}

define_error! {
    pub(super) enum BadParamError {
        BadParam,
    }
}

define_error! {
    pub(super) enum BadObjectStateError {
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`Task::activate`].
    ///
    /// [`Task::activate`]: super::Task::activate
    pub enum ActivateTaskError: BadContextError, BadIdError {
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
    /// Error type for [`Task::current`].
    ///
    /// [`Task::current`]: super::Task::current
    pub enum GetCurrentTaskError: BadContextError {
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    /// Error type for [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: super::Task::interrupt
    pub enum InterruptTaskError: BadContextError, BadIdError, BadObjectStateError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
        /// The task is not in the Waiting state.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`Kernel::exit_task`].
    ///
    /// [`Kernel::exit_task`]: super::Kernel::exit_task
    pub enum ExitTaskError: BadContextError {
        /// CPU Lock is active, or the current context is not a task context.
        BadContext,
    }
}

define_error! {
    /// Error type for [`Kernel::acquire_cpu_lock`] and
    /// [`Kernel::release_cpu_lock`].
    ///
    /// [`Kernel::acquire_cpu_lock`]: super::Kernel::acquire_cpu_lock
    /// [`Kernel::release_cpu_lock`]: super::Kernel::release_cpu_lock
    pub enum CpuLockError: BadContextError {
        /// CPU Lock is already active or inactive.
        BadContext,
    }
}

define_error! {
    /// Error type for [`Kernel::boost_priority`] and
    /// [`Kernel::unboost_priority`].
    ///
    /// [`Kernel::boost_priority`]: super::Kernel::boost_priority
    /// [`Kernel::unboost_priority`]: super::Kernel::unboost_priority
    pub enum BoostPriorityError: BadContextError {
        /// Priority Boost is already active or inactive, the current
        /// context is not a task context, or CPU Lock is active.
        BadContext,
    }
}

define_error! {
    /// Error type for [`Kernel::time`] and
    /// [`Kernel::set_time`].
    ///
    /// [`Kernel::time`]: super::Kernel::time
    /// [`Kernel::set_time`]: super::Kernel::set_time
    pub enum TimeError: BadContextError {
        /// The current context is not a task context, or CPU Lock is active.
        BadContext,
    }
}

define_error! {
    /// Error type for [`Kernel::adjust_time`].
    ///
    /// [`Kernel::adjust_time`]: super::Kernel::adjust_time
    pub enum AdjustTimeError: BadContextError {
        /// CPU Lock is active.
        BadContext,
        /// The requested adjustment is not possible under the current system
        /// state.
        BadObjectState,
    }
}

define_error! {
    /// Error type for wait operations such as [`EventGroup::wait`].
    ///
    /// [`EventGroup::wait`]: super::EventGroup::wait
    pub enum WaitError {
        Interrupted,
    }
}

define_error! {
    /// Error type for wait operations with timeout such as
    /// [`EventGroup::wait_timeout`].
    ///
    /// [`EventGroup::wait_timeout`]: super::EventGroup::wait_timeout
    pub enum WaitTimeoutError: WaitError {
        Interrupted,
        Timeout,
    }
}

impl WaitTimeoutError {
    /// Convert `self` to `WaitError`, panicking if `self == Self::Timeout`.
    pub(super) fn expect_not_timeout(self) -> WaitError {
        match self {
            Self::Interrupted => WaitError::Interrupted,
            Self::Timeout => panic!("got timeout result for a non-timeout wait"),
        }
    }
}

define_error! {
    /// Error type for [`Kernel::park`].
    ///
    /// [`Kernel::park`]: super::Kernel::park
    pub enum ParkError: BadContextError, WaitError {
        /// CPU Lock is active, or the current context is not [waitable].
        ///
        /// [waitable]: crate#contexts
        BadContext,
        Interrupted,
    }
}

define_error! {
    /// Error type for [`Kernel::park_timeout`].
    ///
    /// [`Kernel::park_timeout`]: super::Kernel::park_timeout
    pub enum ParkTimeoutError: BadContextError, WaitTimeoutError, BadParamError {
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
    /// Error type for [`Task::unpark`].
    ///
    /// [`Task::unpark`]: super::Task::unpark
    pub enum UnparkError: BadContextError, BadIdError {
        /// CPU Lock is active.
        BadContext,
        /// The task ID is out of range.
        BadId,
        /// The task is in the Dormant state.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`Task::unpark_exact`].
    ///
    /// [`Task::unpark_exact`]: super::Task::unpark_exact
    pub enum UnparkExactError: BadContextError, BadIdError {
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
    /// Error type for [`Kernel::sleep`].
    ///
    /// [`Kernel::sleep`]: super::Kernel::sleep
    pub enum SleepError: BadContextError {
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
    /// Error type for [`EventGroup::set`] and [`EventGroup::clear`].
    ///
    /// [`EventGroup::set`]: super::EventGroup::set
    /// [`EventGroup::clear`]: super::EventGroup::clear
    pub enum UpdateEventGroupError: BadContextError, BadIdError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    /// Error type for [`EventGroup::get`].
    ///
    /// [`EventGroup::get`]: super::EventGroup::get
    pub enum GetEventGroupError: BadContextError, BadIdError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadContext,
    }
}

define_error! {
    /// Error type for [`EventGroup::wait`].
    ///
    /// [`EventGroup::wait`]: super::EventGroup::wait
    pub enum WaitEventGroupError: BadContextError, BadIdError, WaitError {
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
    /// Error type for [`EventGroup::wait_timeout`].
    ///
    /// [`EventGroup::wait_timeout`]: super::EventGroup::wait_timeout
    pub enum WaitEventGroupTimeoutError: BadContextError, BadIdError, WaitTimeoutError {
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
    /// Error type for [`InterruptLine::set_priority`] and
    /// [`InterruptLine::set_priority_unchecked`].
    ///
    /// [`InterruptLine::set_priority`]: super::InterruptLine::set_priority
    /// [`InterruptLine::set_priority_unchecked`]: super::InterruptLine::set_priority_unchecked
    pub enum SetInterruptLinePriorityError: BadContextError, BadParamError {
        /// The operation is not supported by the port.
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
    /// Error type for [`InterruptLine::enable`] and [`InterruptLine::disable`].
    ///
    /// [`InterruptLine::enable`]: super::InterruptLine::enable
    /// [`InterruptLine::disable`]: super::InterruptLine::disable
    pub enum EnableInterruptLineError: BadParamError {
        /// The operation is not supported by the port.
        NotSupported,
        /// Enabling or disabling the specified interrupt line is not supported.
        BadParam,
    }
}

define_error! {
    /// Error type for [`InterruptLine::pend`].
    ///
    /// [`InterruptLine::pend`]: super::InterruptLine::pend
    pub enum PendInterruptLineError: BadParamError {
        /// Setting a pending flag is not supported by the port.
        NotSupported,
        /// Setting the pending flag of the specified interrupt line is not
        /// supported.
        BadParam,
        /// The interrupt line is not configured to allow this operation. For
        /// example, this operation is invalid for an level-triggered interrupt
        /// line.
        ///
        /// A port is not required to detect this condition.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`InterruptLine::clear`].
    ///
    /// [`InterruptLine::clear`]: super::InterruptLine::clear
    pub enum ClearInterruptLineError: BadParamError {
        /// Clearing a pending flag is not supported by the port.
        NotSupported,
        /// Clearing the pending flag of the specified interrupt line is not
        /// supported.
        BadParam,
        /// The interrupt line is not configured to allow this operation. For
        /// example, this operation is invalid for an level-triggered interrupt
        /// line.
        ///
        /// A port is not required to detect this condition.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`InterruptLine::is_pending`].
    ///
    /// [`InterruptLine::is_pending`]: super::InterruptLine::is_pending
    pub enum QueryInterruptLineError: BadParamError {
        /// Reading a pending flag is not supported by the port.
        NotSupported,
        /// Reading the pending flag of the specified interrupt line is not
        /// supported.
        BadParam,
    }
}
