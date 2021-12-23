//! Macros for internal use

/// Define a kernel object wrapper.
#[macropol::macropol] // Replace `$metavariables` in literals and doc comments
macro_rules! define_object {
    (
        $(#[$meta:meta])*
        pub struct $Name:ident<System: _>(System::$RawId:ident);
        $(#[$meta_ref:meta])*
        pub struct $NameRef:ident<System: $Trait:path>(_);
        pub type $StaticName:ident<System>;
        pub trait $NameHandle:ident {}
        pub trait $NameMethods:ident {}
    ) => {
        #[deny(unused_macros)]
        macro common_doc_owned_handle() { crate::utils::undoc!(
            /// This type is ABI-compatible with `System::`[`$&RawId`][].
            ///
            /// See [`$&NameRef`][] for the [borrowed][] counterpart.
            /// **See [`$&NameMethods`][] for the operations provided by this handle
            /// type.**
            ///
            /// [`$&RawId`]: $&Trait::$&RawId
            /// [`$&NameMethods`]: #impl-$&NameMethods
            /// [borrowed]: crate#object-handles
        ) }

        $(#[$meta])*
        #[repr(transparent)]
        pub struct $Name<System: NotSupportedYet>(<System as $Trait>::$RawId);

        $(#[$meta_ref])*
        ///
        /// This type is ABI-compatible with `System::`[`$&RawId`][]. It's
        /// logically equivalent to `&'a $&Name` but instead stores `$&RawId`
        /// directly.
        ///
        /// See [`$&Name`][] for the [owned][] counterpart and the description
        /// of this kernel object.
        /// **See [`$&NameMethods`][] for the operations provided by this handle
        /// type.**
        ///
        /// [`$&RawId`]: $&Trait::$&RawId
        /// [`$&NameMethods`]: #impl-$&NameMethods
        /// [owned]: crate#object-handles
        #[repr(transparent)]
        pub struct $NameRef<'a, System: $Trait>(
            <System as $Trait>::$RawId,
            core::marker::PhantomData<&'a ()>,
        );

        /// A [static handle][] type: [`$&NameRef`][]`<'static, System>`
        ///
        /// This type is ABI-compatible with `System::`[`$&RawId`][]. It's
        /// logically equivalent to `&'static $&Name` but instead stores
        /// `$&RawId` directly.
        ///
        /// See [`$&Name`][] for the [owned][] counterpart and the description
        /// of this kernel object.
        /// **See [`$&NameMethods`][] for the operations provided by this handle
        /// type.**
        ///
        /// [`$&RawId`]: $&Trait::$&RawId
        /// [`$&NameMethods`]: $&NameRef#impl-$&NameMethods
        /// [Static handle]: crate#object-handles
        /// [owned]: crate#object-handles
        pub type $StaticName<System> = $NameRef<'static, System>;

        use private::NotSupportedYet;
        mod private {
            use super::*;

            /// Owned handles aren't supported yet.
            pub trait NotSupportedYet: $Trait {}
        }

        /// The trait for safe wrappers of `System::`[`$&RawId`][], i.e.,
        /// [`$&Name`][] and [`$&NameRef`][].
        ///
        /// [`$&RawId`]: $&Trait::$&RawId
        pub unsafe trait $NameHandle {
            /// The system type this object pertains to.
            type System: $Trait;

            /// Construct a `$&Name` from `$&RawId`.
            ///
            /// # Safety
            ///
            /// This function is marked as `unsafe` to prevent safe code from
            /// compromising [object safety][1].
            ///
            /// [1]: crate#object-safety
            unsafe fn from_id(id: <Self::System as $Trait>::$RawId) -> Self;

            /// Get the raw `$&RawId` value representing this object.
            fn id(&self) -> <Self::System as $Trait>::$RawId;

            /// [Borrow][] `self` as [`$&NameRef`][].
            ///
            /// [Borrow]: crate#object-handles
            fn borrow(&self) -> $NameRef<'_, Self::System>;
        }

        unsafe impl<System: NotSupportedYet> const $NameHandle for $Name<System> {
            type System = System;

            #[inline]
            unsafe fn from_id(id: <System as $Trait>::$RawId) -> Self {
                Self(id)
            }

            #[inline]
            fn id(&self) -> System::$RawId {
                self.0
            }

            #[inline]
            fn borrow(&self) -> $NameRef<'_, Self::System> {
                $NameRef(self.0, core::marker::PhantomData)
            }
        }

        unsafe impl<System: $Trait> const $NameHandle for $NameRef<'_, System> {
            type System = System;

            #[inline]
            unsafe fn from_id(id: <System as $Trait>::$RawId) -> Self {
                Self(id, core::marker::PhantomData)
            }

            #[inline]
            fn id(&self) -> System::$RawId {
                self.0
            }

            #[inline]
            fn borrow(&self) -> $NameRef<'_, Self::System> {
                *self
            }
        }

        // `$Name` intentionally lacks support for cloning.

        impl<System: NotSupportedYet> PartialEq for $Name<System> {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl<System: NotSupportedYet> PartialEq<$NameRef<'_, System>> for $Name<System> {
            #[inline]
            fn eq(&self, other: &$NameRef<'_, System>) -> bool {
                self.0 == other.0
            }
        }

        impl<System: NotSupportedYet> Eq for $Name<System> {}

        impl<System: NotSupportedYet> hash::Hash for $Name<System> {
            #[inline]
            fn hash<H>(&self, state: &mut H)
            where
                H: hash::Hasher,
            {
                self.borrow().hash(state)
            }
        }

        impl<System: NotSupportedYet> fmt::Debug for $Name<System> {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.borrow().fmt(f)
            }
        }

        impl<System: $Trait> Clone for $NameRef<'_, System> {
            #[inline]
            fn clone(&self) -> Self {
                Self(self.0, self.1)
            }
        }

        impl<System: $Trait> Copy for $NameRef<'_, System> {}

        impl<System: $Trait> PartialEq for $NameRef<'_, System> {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl<System: NotSupportedYet> PartialEq<$Name<System>> for $NameRef<'_, System> {
            #[inline]
            fn eq(&self, other: &$Name<System>) -> bool {
                self.0 == other.0
            }
        }

        impl<System: $Trait> Eq for $NameRef<'_, System> {}

        impl<System: $Trait> hash::Hash for $NameRef<'_, System> {
            #[inline]
            fn hash<H>(&self, state: &mut H)
            where
                H: hash::Hasher,
            {
                hash::Hash::hash(&self.0, state);
            }
        }

        impl<System: $Trait> fmt::Debug for $NameRef<'_, System> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_tuple("$&Name").field(&self.0).finish()
            }
        }
    };
}
