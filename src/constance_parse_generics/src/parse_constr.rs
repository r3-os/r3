/*
Copyright ⓒ 2016 rust-custom-derive contributors.

Licensed under the MIT license (see LICENSE or <http://opensource.org
/licenses/MIT>) or the Apache License, Version 2.0 (see LICENSE of
<http://www.apache.org/licenses/LICENSE-2.0>), at your option. All
files in the project carrying such notice may not be copied, modified,
or distributed except according to those terms.
*/
#[cfg(not(feature="use-parse-generics-poc"))]
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! parse_constr {
    (
        @parse
        { $callback:tt }, $allow:tt, $constr:tt,
        , $($body:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            $constr,
            , $($body)*
        }
    };

    (
        @parse
        { $callback:tt }, $allow:tt, $constr:tt,
        > $($body:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            $constr,
            > $($body)*
        }
    };

    (
        @parse
        { $callback:tt }, $allow:tt, $constr:tt,
        ; $($body:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            $constr,
            ; $($body)*
        }
    };

    (
        @parse
        { $callback:tt }, $allow:tt, $constr:tt,
        = $($body:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            $constr,
            = $($body)*
        }
    };

    (
        @parse
        { $callback:tt }, $allow:tt, $constr:tt,
        {$($delim:tt)*} $($body:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            $constr,
            {$($delim)*} $($body)*
        }
    };

    (
        @parse
        $prefix:tt, $allow:tt, {$($constr:tt)*},
        + $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, $allow,
            {$($constr)* +},
            $($body)*
        }
    };

    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'static $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'static}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'a $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'a}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'b $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'b}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'c $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'c}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'd $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'd}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'e $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'e}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'f $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'f}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'g $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'g}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'h $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'h}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'i $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'i}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'j $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'j}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'k $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'k}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'l $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'l}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'm $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'm}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'n $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'n}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'o $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'o}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'p $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'p}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'q $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'q}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'r $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'r}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 's $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 's}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 't $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 't}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'u $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'u}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'v $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'v}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'w $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'w}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'x $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'x}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'y $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'y}, $($body)* } };
    (@parse $prefix:tt, (true, $atr:tt), {$($constr:tt)*}, 'z $($body:tt)*) => { parse_constr! { @parse $prefix, (true, $atr), {$($constr)* 'z}, $($body)* } };

    (
        @parse
        $prefix:tt, $allow:tt, {$($constr:tt)*},
        ?Sized $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, $allow,
            {$($constr)* ?Sized},
            $($body)*
        }
    };

    (
        @parse
        $prefix:tt, ($_alt:tt, true), {$($constr:tt)*},
        :: $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, (false, true),
            {$($constr)* ::},
            $($body)*
        }
    };

    (
        @parse
        $prefix:tt, ($_alt:tt, true), {$($constr:tt)*},
        < $($body:tt)*
    ) => {
        parse_constr! {
            @parse_delim
            { $prefix, (false, true) },
            [ # ],
            {$($constr)* <},
            $($body)*
        }
    };

    (
        @parse
        $prefix:tt, ($_alt:tt, true), {$($constr:tt)*},
        $trname:ident $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, (false, true),
            {$($constr)* $trname},
            $($body)*
        }
    };

    (
        @parse_delim
        { $prefix:tt, $allow:tt },
        [ # ], {$($constr:tt)*},
        > $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, $allow, {$($constr)* >},
            $($body)*
        }
    };

    (
        @parse_delim
        { $prefix:tt, $allow:tt },
        [ # ], {$($constr:tt)*},
        >> $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, $allow, {$($constr)* >},
            > $($body)*
        }
    };

    (
        @parse_delim
        { $prefix:tt, $allow:tt },
        [ # # ], {$($constr:tt)*},
        >> $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            $prefix, $allow, {$($constr)* >>},
            $($body)*
        }
    };

    (
        @parse_delim
        $prefix:tt,
        [ $($stack:tt)* ], {$($constr:tt)*},
        < $($body:tt)*
    ) => {
        parse_constr! {
            @parse_delim
            $prefix, [ # $($stack)* ], {$($constr)* <},
            $($body)*
        }
    };

    (
        @parse_delim
        $prefix:tt,
        [ # $($stack:tt)* ], {$($constr:tt)*},
        > $($body:tt)*
    ) => {
        parse_constr! {
            @parse_delim
            $prefix, [ $($stack)* ], {$($constr)* >},
            $($body)*
        }
    };

    (
        @parse_delim
        $prefix:tt,
        [ # # $($stack:tt)* ], {$($constr:tt)*},
        >> $($body:tt)*
    ) => {
        parse_constr! {
            @parse_delim
            $prefix, [ $($stack)* ], {$($constr)* >>},
            $($body)*
        }
    };

    (
        @parse_delim
        $prefix:tt,
        $stack:tt, {$($constr:tt)*},
        $other:tt $($body:tt)*
    ) => {
        parse_constr! {
            @parse_delim
            $prefix, $stack, {$($constr)* $other},
            $($body)*
        }
    };

    (
        ($allow_lt:tt, $allow_tr:tt),
        then $callback:ident$(::$callback_sub:ident)*!$callback_arg:tt,
        $($body:tt)*
    ) => {
        parse_constr! {
            @parse
            {
                ($callback$(::$callback_sub)*!$callback_arg)
            },
            ($allow_lt, $allow_tr),
            {},
            $($body)*
        }
    };
}
