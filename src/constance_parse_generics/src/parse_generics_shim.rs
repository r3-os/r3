/*
Copyright ⓒ 2016 rust-custom-derive contributors.

Licensed under the MIT license (see LICENSE or <http://opensource.org
/licenses/MIT>) or the Apache License, Version 2.0 (see LICENSE of
<http://www.apache.org/licenses/LICENSE-2.0>), at your option. All
files in the project carrying such notice may not be copied, modified,
or distributed except according to those terms.
*/
#[cfg(feature="use-parse-generics-poc")]
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! parse_generics_shim {
    ($($body:tt)*) => {
        parse_generics! { $($body)* }
    };
}

#[cfg(not(feature="use-parse-generics-poc"))]
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! parse_generics_shim {
    (
        @parse_start
        $prefix:tt,
        <> $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @emit_output
            $prefix,
            {
                constr: [],
                ltimes: [],
                tnames: [],
            },
            $($tail)*
        }
    };

    (
        @parse_start
        $prefix:tt,
        < $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            {
                constr: [],
                ltimes: [],
                tnames: [],
            },
            $($tail)*
        }
    };

    (
        @parse_start
        $prefix:tt,
        $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @emit_output
            $prefix,
            {
                constr: [],
                ltimes: [],
                tnames: [],
            },
            $($tail)*
        }
    };

    (
        @parse
        $prefix:tt,
        $fields:tt,
        > $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @emit_output
            $prefix,
            $fields,
            $($tail)*
        }
    };

    (
        @parse
        $prefix:tt,
        $fields:tt,
        $(,)+ $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            $fields,
            $($tail)*
        }
    };

    (@parse $prefix:tt, $fields:tt, 'static: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'static: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'a: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'a: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'b: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'b: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'c: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'c: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'd: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'd: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'e: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'e: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'f: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'f: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'g: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'g: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'h: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'h: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'i: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'i: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'j: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'j: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'k: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'k: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'l: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'l: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'm: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'm: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'n: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'n: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'o: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'o: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'p: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'p: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'q: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'q: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'r: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'r: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 's: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 's: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 't: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 't: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'u: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'u: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'v: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'v: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'w: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'w: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'x: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'x: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'y: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'y: }, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'z: $($tail:tt)*) => { parse_constr! { (true, false), then parse_generics_shim! { @app_lt $prefix, $fields, 'z: }, $($tail)* } };

    (@parse $prefix:tt, $fields:tt, 'static $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'static: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'a $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'a: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'b $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'b: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'c $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'c: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'd $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'd: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'e $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'e: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'f $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'f: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'g $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'g: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'h $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'h: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'i $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'i: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'j $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'j: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'k $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'k: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'l $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'l: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'm $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'm: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'n $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'n: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'o $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'o: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'p $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'p: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'q $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'q: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'r $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'r: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 's $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 's: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 't $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 't: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'u $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'u: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'v $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'v: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'w $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'w: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'x $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'x: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'y $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'y: {}, $($tail)* } };
    (@parse $prefix:tt, $fields:tt, 'z $($tail:tt)* ) => { parse_generics_shim! { @app_lt $prefix, $fields, 'z: {}, $($tail)* } };

    (
        @app_lt
        $prefix:tt,
        {
            constr: [$($constr:tt)*],
            ltimes: [$($ltimes:tt)*],
            tnames: $tnames:tt,
        },
        $lt:tt: {},
        $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            {
                constr: [$($constr)* $lt,],
                ltimes: [$($ltimes)* $lt,],
                tnames: $tnames,
            },
            $($tail)*
        }
    };

    (
        @app_lt
        $prefix:tt,
        {
            constr: [$($constr:tt)*],
            ltimes: [$($ltimes:tt)*],
            tnames: $tnames:tt,
        },
        $lt:tt: {$($ltconstr:tt)*},
        $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            {
                constr: [$($constr)* $lt: $($ltconstr)*,],
                ltimes: [$($ltimes)* $lt,],
                tnames: $tnames,
            },
            $($tail)*
        }
    };

    (
        @parse
        $prefix:tt,
        $fields:tt,
        $tname:ident: $($tail:tt)*
    ) => {
        parse_constr! {
            (true, true),
            then parse_generics_shim! {
                @app_ty
                $prefix,
                $fields,
                $tname:
            },
            $($tail)*
        }
    };

    (
        @parse
        $prefix:tt,
        {
            constr: [$($constr:tt)*],
            ltimes: $ltimes:tt,
            tnames: [$($tnames:tt)*],
        },
        $tname:ident $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            {
                constr: [$($constr)* $tname,],
                ltimes: $ltimes,
                tnames: [$($tnames)* $tname,],
            },
            $($tail)*
        }
    };

    (
        @app_ty
        $prefix:tt,
        {
            constr: [$($constr:tt)*],
            ltimes: $ltimes:tt,
            tnames: [$($tnames:tt)*],
        },
        $ty:ident: {},
        $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            {
                constr: [$($constr)* $ty,],
                ltimes: $ltimes,
                tnames: [$($tnames)* $ty,],
            },
            $($tail)*
        }
    };

    (
        @app_ty
        $prefix:tt,
        {
            constr: [$($constr:tt)*],
            ltimes: $ltimes:tt,
            tnames: [$($tnames:tt)*],
        },
        $ty:ident: {$($tyconstr:tt)*},
        $($tail:tt)*
    ) => {
        parse_generics_shim! {
            @parse
            $prefix,
            {
                constr: [$($constr)* $ty: $($tyconstr)*,],
                ltimes: $ltimes,
                tnames: [$($tnames)* $ty,],
            },
            $($tail)*
        }
    };

    (
        @emit_output
        { { .. }, $callback:tt },
        {
            constr: $constr:tt,
            ltimes: [$($ltimes:tt)*],
            tnames: [$($tnames:tt)*],
        },
        $($tail:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            {
                constr: $constr,
                params: [$($ltimes)* $($tnames)*],
                ltimes: [$($ltimes)*],
                tnames: [$($tnames)*],
                ..
            },
            $($tail)*
        }
    };

    (
        @emit_output
        { { constr, params, ltimes, tnames }, $callback:tt },
        {
            constr: $constr:tt,
            ltimes: [$($ltimes:tt)*],
            tnames: [$($tnames:tt)*],
        },
        $($tail:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            {
                constr: $constr,
                params: [$($ltimes)* $($tnames)*],
                ltimes: [$($ltimes)*],
                tnames: [$($tnames)*],
            },
            $($tail)*
        }
    };

    (
        @emit_output
        { { constr }, $callback:tt },
        {
            constr: $constr:tt,
            ltimes: [$($ltimes:tt)*],
            tnames: [$($tnames:tt)*],
        },
        $($tail:tt)*
    ) => {
        parse_generics_shim_util! {
            @callback
            $callback,
            {
                constr: $constr,
            },
            $($tail)*
        }
    };

    (
        $fields:tt,
        then $callback:ident$(::$callback_sub:ident)*!$callback_arg:tt,
        $($body:tt)*
    ) => {
        parse_generics_shim! {
            @parse_start
            { $fields, ($callback$(::$callback_sub)*!$callback_arg) },
            $($body)*
        }
    };
}
