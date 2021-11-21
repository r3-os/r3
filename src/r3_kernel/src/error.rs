use core::fmt;
use r3::kernel as errors;

macro_rules! define_suberror {
    (
        $( #[doc $( $doc:tt )*] )*
        $( #[into( $Supererror:path )] )*
        $vis:vis enum $Name:ident {
            $( $Variant:ident, )*
        }
    ) => {
        $( #[doc $( $doc )*] )*
        #[repr(i8)]
        #[derive(PartialEq, Eq, Copy, Clone)]
        $vis enum $Name {
            $( $Variant = errors::ResultCode::$Variant as _ ),*
        }

        impl fmt::Debug for $Name {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                errors::ResultCode::from(*self).fmt(f)
            }
        }

        define_suberror! {
            @into
            #[into(errors::ResultCode)]
            $( #[into( $Supererror )] )*
            enum $Name {
                $( $Variant, )*
            }
        }
    };

    (
        @into
        #[into( $Supererror0:path )]
        $( #[into( $Supererror:path )] )*
        enum $Name:ident {
            $( $Variant:ident, )*
        }
    ) => {
        impl From<$Name> for $Supererror0 {
            #[inline]
            fn from(x: $Name) -> Self {
                match x {
                    $( $Name::$Variant => Self::$Variant ),*
                }
            }
        }

        define_suberror! {
            @into
            $( #[into( $Supererror )] )*
            enum $Name {
                $( $Variant, )*
            }
        }
    };

    ( @into enum $($_:tt)* ) => {};
}

define_suberror! {
    /// `BadContext`
    #[into(errors::ActivateTaskError)]
    #[into(errors::AdjustTimeError)]
    #[into(errors::BoostPriorityError)]
    #[into(errors::CpuLockError)]
    #[into(errors::DrainSemaphoreError)]
    #[into(errors::ExitTaskError)]
    #[into(errors::GetCurrentTaskError)]
    #[into(errors::GetEventGroupError)]
    #[into(errors::GetSemaphoreError)]
    #[into(errors::GetTaskPriorityError)]
    #[into(errors::InterruptTaskError)]
    #[into(errors::LockMutexError)]
    #[into(errors::LockMutexTimeoutError)]
    #[into(errors::MarkConsistentMutexError)]
    #[into(errors::ParkError)]
    #[into(errors::ParkTimeoutError)]
    #[into(errors::PollEventGroupError)]
    #[into(errors::PollSemaphoreError)]
    #[into(errors::QueryMutexError)]
    #[into(errors::SetInterruptLinePriorityError)]
    #[into(errors::SetTaskPriorityError)]
    #[into(errors::SetTimerDelayError)]
    #[into(errors::SetTimerPeriodError)]
    #[into(errors::SignalSemaphoreError)]
    #[into(errors::SleepError)]
    #[into(errors::StartTimerError)]
    #[into(errors::StopTimerError)]
    #[into(errors::TimeError)]
    #[into(errors::TryLockMutexError)]
    #[into(errors::UnlockMutexError)]
    #[into(errors::UnparkError)]
    #[into(errors::UnparkExactError)]
    #[into(errors::UpdateEventGroupError)]
    #[into(errors::WaitEventGroupError)]
    #[into(errors::WaitEventGroupTimeoutError)]
    #[into(errors::WaitSemaphoreError)]
    #[into(errors::WaitSemaphoreTimeoutError)]
    pub(super) enum BadContextError {
        BadContext,
    }
}

define_suberror! {
    /// `BadId`
    #[into(errors::ActivateTaskError)]
    #[into(errors::DrainSemaphoreError)]
    #[into(errors::GetEventGroupError)]
    #[into(errors::GetSemaphoreError)]
    #[into(errors::GetTaskPriorityError)]
    #[into(errors::InterruptTaskError)]
    #[into(errors::LockMutexError)]
    #[into(errors::LockMutexTimeoutError)]
    #[into(errors::MarkConsistentMutexError)]
    #[into(errors::PollEventGroupError)]
    #[into(errors::PollSemaphoreError)]
    #[into(errors::QueryMutexError)]
    #[into(errors::SetTaskPriorityError)]
    #[into(errors::SetTimerDelayError)]
    #[into(errors::SetTimerPeriodError)]
    #[into(errors::SignalSemaphoreError)]
    #[into(errors::StartTimerError)]
    #[into(errors::StopTimerError)]
    #[into(errors::TryLockMutexError)]
    #[into(errors::UnlockMutexError)]
    #[into(errors::UnparkError)]
    #[into(errors::UnparkExactError)]
    #[into(errors::UpdateEventGroupError)]
    #[into(errors::WaitEventGroupError)]
    #[into(errors::WaitEventGroupTimeoutError)]
    #[into(errors::WaitSemaphoreError)]
    #[into(errors::WaitSemaphoreTimeoutError)]
    pub(super) enum BadIdError {
        BadId,
    }
}

define_suberror! {
    /// `BadParam`
    #[into(errors::LockMutexTimeoutError)]
    #[into(errors::ParkTimeoutError)]
    #[into(errors::SetTimerDelayError)]
    #[into(errors::SetTimerPeriodError)]
    #[into(errors::SleepError)]
    #[into(errors::WaitEventGroupTimeoutError)]
    #[into(errors::WaitSemaphoreTimeoutError)]
    pub(super) enum BadParamError {
        BadParam,
    }
}

define_suberror! {
    /// `BadObjectState`
    #[into(errors::InterruptTaskError)]
    pub(super) enum BadObjectStateError {
        BadObjectState,
    }
}

define_suberror! {
    /// Some of the error codes shared by [`TryLockMutexError`],
    /// [`LockMutexError`], and [`LockMutexTimeoutError`]. Used internally
    /// by the mutex implementation.
    #[into(errors::LockMutexError)]
    #[into(errors::LockMutexTimeoutError)]
    #[into(errors::TryLockMutexError)]
    pub(super) enum LockMutexPrecheckError {
        WouldDeadlock,
        BadParam,
    }
}

/// Convert `self` to `WaitError`, panicking if `self == Self::Timeout`.
#[inline]
pub(super) fn expect_not_timeout(e: errors::WaitTimeoutError) -> errors::WaitError {
    match e {
        errors::WaitTimeoutError::Interrupted => errors::WaitError::Interrupted,
        errors::WaitTimeoutError::Timeout => {
            unreachable!("got timeout result for a non-timeout wait")
        }
    }
}
