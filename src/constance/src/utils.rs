//! Utility

/// Trait for types having a constant default value. This is essentially a
/// constant version of `Default`.
pub trait Init {
    /// The default value.
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;
}

macro_rules! impl_init {
    (
        $($ty:ty => $value:expr,)*
    ) => {
        $(
            impl Init for $ty {
                const INIT: Self = $value;
            }
        )*
    };
}

impl_init! {
    u8 => 0,
    u16 => 0,
    u32 => 0,
    u64 => 0,
    u128 => 0,
    i8 => 0,
    i16 => 0,
    i32 => 0,
    i64 => 0,
    i128 => 0,
    usize => 0,
    isize => 0,
    () => (),
}
