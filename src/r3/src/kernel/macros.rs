//! Macros for internal use

/// Define a kernel object wrapper.
#[macropol::macropol] // Replace `$metavariables` in literals and doc comments
macro_rules! define_object {
    (
        $(#[$meta:meta])*
        pub struct $Name:ident<System: $Trait:path>(System::$RawId:ident);
    ) => {
        $(#[$meta])*
        ///
        /// This type is ABI-compatible with `System::`[`$&RawId`][].
        ///
        /// [`$&RawId`]: $&Trait::$&RawId
        #[repr(transparent)]
        pub struct $Name<System: $Trait>(<System as $Trait>::$RawId);

        impl<System: $Trait> Clone for $Name<System> {
            #[inline]
            fn clone(&self) -> Self {
                Self(self.0)
            }
        }

        impl<System: $Trait> Copy for $Name<System> {}

        impl<System: $Trait> PartialEq for $Name<System> {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl<System: $Trait> Eq for $Name<System> {}

        impl<System: $Trait> hash::Hash for $Name<System> {
            #[inline]
            fn hash<H>(&self, state: &mut H)
            where
                H: hash::Hasher,
            {
                hash::Hash::hash(&self.0, state);
            }
        }

        impl<System: $Trait> fmt::Debug for $Name<System> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_tuple("$&Name").field(&self.0).finish()
            }
        }

        impl<System: $Trait> $Name<System> {
            /// Construct a `$&Name` from `$&RawId`.
            ///
            /// # Safety
            ///
            /// The kernel can handle invalid IDs without a problem. However, the
            /// constructed `$&Name` may point to an object that is not intended to be
            /// manipulated except by its creator. This is usually prevented by making
            /// `$&Name` an opaque handle, but this safeguard can be circumvented by
            /// this method.
            #[inline]
            pub const unsafe fn from_id(id: <System as $Trait>::$RawId) -> Self {
                Self(id)
            }


            /// Get the raw `$&RawId` value representing this object.
            #[inline]
            pub const fn id(self) -> <System as $Trait>::$RawId {
                self.0
            }
        }
    };
}
