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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i8)]
pub enum ResultCode {
    /// The operation was successful. No additional information is available.
    Success = 0,
    /// A parameter is invalid in a way that is no covered by any other error
    /// codes.
    BadParam = -17,
    /// A specified object identifier ([`Id`]) is invalid.
    ///
    /// [`Id`]: super::Id
    BadId = -18,
    /// The current context disallows the operation.
    BadCtx = -25,
    /// A target object is in a state that disallows the operation.
    BadObjectState = -41,
    /// An operation or an object couldn't be enqueued because there are too
    /// many of such things that already have been enqueued.
    QueueOverflow = -43,
    /// The wait operation was interrupted by [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: crate::kernel::Task::interrupt
    Interrupted = -49,
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
    pub(super) enum BadCtxError {
        BadCtx,
    }
}

define_error! {
    pub(super) enum BadIdError {
        BadId,
    }
}

define_error! {
    /// Error type for [`Task::activate`].
    ///
    /// [`Task::activate`]: super::Task::activate
    pub enum ActivateTaskError: BadCtxError, BadIdError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadCtx,
        /// The task is already active (not in a Dormant state).
        ///
        /// This error code originates from `E_QOVR` defined in the μITRON 4.0
        /// specification. In this specification, the `act_tsk` (activate task)
        /// system service works by enqueueing an activation request. `E_QOVR`
        /// is used to report a condition in which an enqueue count limit has
        /// been reached. Our kernel doesn't support enqueueing actvation
        /// request (at the moment), so any attempts to activate an
        /// already-active task will fail.
        QueueOverflow,
    }
}

define_error! {
    /// Error type for [`Task::current`].
    ///
    /// [`Task::current`]: super::Task::current
    pub enum GetCurrentTaskError: BadCtxError {
        /// CPU Lock is active.
        BadCtx,
    }
}

define_error! {
    /// Error type for [`Task::interrupt`].
    ///
    /// [`Task::interrupt`]: super::Task::interrupt
    pub enum InterruptTaskError: BadCtxError, BadIdError {
        /// The task ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadCtx,
        /// The task is not in a Waiting state.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`Kernel::exit_task`].
    ///
    /// [`Kernel::exit_task`]: super::Kernel::exit_task
    pub enum ExitTaskError: BadCtxError {
        BadCtx,
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
    /// Error type for [`Kernel::park`].
    ///
    /// [`Kernel::park`]: super::Kernel::park
    pub enum ParkError: BadCtxError, WaitError {
        BadCtx,
        Interrupted,
    }
}

define_error! {
    /// Error type for [`Task::unpark`].
    ///
    /// [`Task::unpark`]: super::Task::unpark
    pub enum UnparkError: BadCtxError, BadIdError {
        /// CPU Lock is active.
        BadCtx,
        /// The task ID is out of range.
        BadId,
        /// The task is in a Dormant state.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`Task::unpark_exact`].
    ///
    /// [`Task::unpark_exact`]: super::Task::unpark_exact
    pub enum UnparkExactError: BadCtxError, BadIdError {
        /// CPU Lock is active.
        BadCtx,
        /// The task ID is out of range.
        BadId,
        /// The task already has a token.
        QueueOverflow,
        /// The task is in a Dormant state.
        BadObjectState,
    }
}

define_error! {
    /// Error type for [`EventGroup::set`] and [`EventGroup::clear`].
    ///
    /// [`EventGroup::set`]: super::EventGroup::set
    /// [`EventGroup::clear`]: super::EventGroup::clear
    pub enum UpdateEventGroupError: BadCtxError, BadIdError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadCtx,
    }
}

define_error! {
    /// Error type for [`EventGroup::get`].
    ///
    /// [`EventGroup::get`]: super::EventGroup::get
    pub enum GetEventGroupError: BadCtxError, BadIdError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadCtx,
    }
}

define_error! {
    /// Error type for [`EventGroup::wait`].
    ///
    /// [`EventGroup::wait`]: super::EventGroup::wait
    pub enum WaitEventGroupError: BadCtxError, BadIdError, WaitError {
        /// The event group ID is out of range.
        BadId,
        /// CPU Lock is active.
        BadCtx,
        Interrupted,
    }
}
