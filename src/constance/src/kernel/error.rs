macro_rules! define_error {
    (
        $( #[$meta:meta] )*
        pub enum $name:ident {
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
        pub enum $name {
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
    /// Error type for [`Task::activate`].
    ///
    /// [`Task::activate`]: super::Task::activate
    pub enum ActivateTaskError {
        BadId,
    }
}
