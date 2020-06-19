use core::marker::PhantomData;

// FIXME: An elaborate work-around for limitations such as
//        <https://github.com/rust-lang/rust/issues/72821>.
pub struct U0;
pub struct UInt<U, B>(PhantomData<(U, B)>);

pub struct B0;
pub struct B1;

/// Natural number. Use [`U`] to get a type implementing this.
pub trait Nat {
    type Succ: Nat;
    const N: usize;
}

impl Nat for U0 {
    type Succ = UInt<U0, B1>;
    const N: usize = 0;
}

impl<U: Nat, B> Nat for UInt<U, B> {
    default type Succ = U0;
    default const N: usize = 0;
}
impl<U: Nat> Nat for UInt<U, B0> {
    type Succ = UInt<U, B1>;
    const N: usize = U::N * 2;
}
impl<U: Nat> Nat for UInt<U, B1> {
    type Succ = UInt<<U as Nat>::Succ, B0>;
    const N: usize = U::N * 2 + 1;
}

/// Convert a value to a bit type.
type Bn<const I: usize> = If! { if (I == 0) { B0 } else { B1 } };

/// Convert a value to a binary integer type.
///
/// `I` must be less than or equal to [`U_MAX`].
pub type U<const I: usize> = UInt<
    UInt<
        UInt<
            UInt<
                UInt<
                    UInt<
                        UInt<
                            UInt<
                                UInt<
                                    UInt<
                                        UInt<
                                            UInt<
                                                UInt<
                                                    UInt<
                                                        UInt<
                                                            UInt<U0, Bn<{ I & 32768 }>>,
                                                            Bn<{ I & 16384 }>,
                                                        >,
                                                        Bn<{ I & 8192 }>,
                                                    >,
                                                    Bn<{ I & 4096 }>,
                                                >,
                                                Bn<{ I & 2048 }>,
                                            >,
                                            Bn<{ I & 1024 }>,
                                        >,
                                        Bn<{ I & 512 }>,
                                    >,
                                    Bn<{ I & 256 }>,
                                >,
                                Bn<{ I & 128 }>,
                            >,
                            Bn<{ I & 64 }>,
                        >,
                        Bn<{ I & 32 }>,
                    >,
                    Bn<{ I & 16 }>,
                >,
                Bn<{ I & 8 }>,
            >,
            Bn<{ I & 4 }>,
        >,
        Bn<{ I & 2 }>,
    >,
    Bn<{ I & 1 }>,
>;

/// Maximum input value for [`U`].
pub const U_MAX: usize = 65535;

/// Type-level function producing a `Nat`.
pub trait NatFn {
    type Output: Nat;
}

/// Saturating increment operation.
///
///  - `Self::Output::N == T::N` if `Self::Output::N == Limit::N`.
///  - `Self::Output::N == T::N + 1` otherwise.
///
pub type IncrSat<T, Limit> = <IncrSatOp<T, Limit> as NatFn>::Output;

pub struct IncrSatOp<T, Limit>(T, Limit);

impl<T: Nat, Limit> NatFn for IncrSatOp<T, Limit> {
    default type Output = T::Succ;
}

impl<T: Nat> NatFn for IncrSatOp<T, T> {
    type Output = T;
}

/// Evaluate a piece of code for the specified number of times. The iteration
/// counter is available as a constant expression.
macro_rules! const_for_times {
    (
        // The iterated code cannot reference outer generic parameters.
        // Instead, all outer generic parameter should be repeated in
        // `$iter_gparam`. `$iter_ctx_ty` should use generic parameters from
        // `$iter_gparam`.
        //
        // A value parameter can be passed through `$ctx_param` of type
        // `$iter_ctx_ty`.
        ///
        // THe iteration position can be read by `$i::N`.
        fn iter<
            $(  [  $iter_gparam:ident $($iter_gparam_bounds:tt)*  ],  )*
            $i:ident: Nat
        >($ctx_param:ident: $iter_ctx_ty:ty) {
            $($iter:tt)*
        }

        // `$len` must be `U<ITERATION_COUNT>`.
        (0..$len:ty).for_each(|i| iter::<[$($ctx_t:ty),*], i>($ctx:expr))
    ) => {{
        use $crate::utils::for_times::{Nat, U, IncrSat};

        #[inline(always)]
        const fn iter_inner<
            $(  $iter_gparam $($iter_gparam_bounds)*  ,)*
            $i: Nat
        >($ctx_param: $iter_ctx_ty) {
            $($iter)*
        }

        #[inline(always)]
        const fn iter_outer<
            $(  $iter_gparam $($iter_gparam_bounds)*  ,)*
            Counter: Nat,
            Limit: Nat
        >($ctx_param: $iter_ctx_ty) {
            if Counter::N < Limit::N {
                iter_inner::<
                    $( $iter_gparam ,)*
                    Counter
                >($ctx_param);

                iter_outer::<
                    $( $iter_gparam ,)*
                    IncrSat<Counter, Limit>,
                    Limit
                >($ctx_param);
            }
        }

        iter_outer::<$($ctx_t,)* U<0>, $len>($ctx);
    }};
}

/// Construct an array by evaluating a piece of code for each element. The
/// iteration counter is available as a constant expression.
macro_rules! const_array_from_fn {
    (
        // The iterated code cannot reference outer generic parameters.
        // Instead, all outer generic parameter should be repeated in
        // `$iter_gparam`. `$iter_ctx_ty` should use generic parameters from
        // `$iter_gparam`.
        //
        // A value parameter can be passed through `$ctx_param` of type
        // `$iter_ctx_ty`.
        ///
        // THe iteration position can be read by `$i::N`.
        fn iter<
            $(  [  $iter_gparam:ident $($iter_gparam_bounds:tt)*  ],  )*
            $i:ident: Nat
        >(ref mut $ctx_param:ident: $iter_ctx_ty:ty) -> $ty:ty {
            $($iter:tt)*
        }

        // `$len` must be `U<$len_value>`
        (0..$len_value:expr).map(|i| iter::<[$($ctx_t:ty),*], i>($ctx:expr)).collect::<[_; $len:ty]>()
    ) => {{
        use core::mem::MaybeUninit;
        use $crate::utils::for_times::Nat;
        let array = [MaybeUninit::uninit(); $len_value];

        if array.len() != <$len as Nat>::N {
            unreachable!();
        }

        const_for_times! {
            fn iter<
                $(  [  $iter_gparam $($iter_gparam_bounds)*  ],  )*
                $i: Nat
            >(ctx_param: &mut ($iter_ctx_ty, *mut MaybeUninit<$ty>)) {
                #[allow(unused_variables)]
                let $ctx_param = &mut ctx_param.0;
                let value = {
                    $($iter)*
                };

                // Safety: `$i::N` is in range `0..$len`, so
                // `ctx_param.1 + $i::N` points to a location inside `array`.
                unsafe {
                    *ctx_param.1.add($i::N) = MaybeUninit::new(value);
                }
            }

            (0..$len).for_each(|i| iter::<[$($ctx_t),*], i>(
                // FIXME: `[T]::as_mut_ptr` is not `const fn` yet
                &mut ($ctx, array.as_ptr() as *mut _)
            ))
        }

        const unsafe fn __assume_init<
            $($iter_gparam $($iter_gparam_bounds)*,)*
            const LEN: usize
        >(array: [MaybeUninit<$ty>; LEN]) -> [$ty; LEN] {
            // Safety: This is equivalent to `transmute_copy(&array)`. The
            // memory layout of `[MaybeUninit<T>; $len]` is identical to `[T; $len]`.
            // We initialized all elements in `array[0..$len]`, so it's safe to
            // reinterpret that range as `[T; $len]`.
            unsafe { *(array.as_ptr() as *const _ as *const [$ty; LEN]) }
        }

        // Safety: See the body of `__assume_init`.
        unsafe { __assume_init::<$($ctx_t,)* {$len_value}>(array) }
    }};
}

#[cfg(test)]
mod tests {
    use super::U;

    #[test]
    fn test() {
        struct Cell<T>(T, u128);

        const GOT: u128 = {
            let mut cell = Cell("unused", 0);
            const_for_times! {
                fn iter<[T], I: Nat>(cell: &mut Cell<T>) {
                    cell.1 = cell.1 * 10 + I::N as u128;
                }

                (0..U<20>).for_each(|i| iter::<[_], i>(&mut cell))
            }
            cell.1
        };

        let expected = {
            let mut cell = 0;
            for i in 0..20 {
                cell = cell * 10 + i;
            }
            cell
        };

        assert_eq!(expected, GOT);
    }

    #[test]
    fn const_array_from_fn() {
        struct Cell<T>(T, u128);
        const GOT: [u128; 20] = {
            let mut cell = Cell("unused", 0);
            const_array_from_fn! {
                fn iter<[T], I: Nat>(ref mut cell: &mut Cell<T>) -> u128 {
                    cell.1 = cell.1 * 10 + I::N as u128;
                    cell.1
                }

                (0..20).map(|i| iter::<[&'static str], i>(&mut cell)).collect::<[_; U<20>]>()
            }
        };

        let expected = {
            let mut cell = Cell("unused", 0);

            fn iter<T>(cell: &mut Cell<T>, i: usize) -> u128 {
                cell.1 = cell.1 * 10 + i as u128;
                cell.1
            }

            (0..20)
                .map(|i| iter::<&'static str>(&mut cell, i))
                .collect::<Vec<_>>()
        };

        assert_eq!(GOT[..], *expected);
    }
}
