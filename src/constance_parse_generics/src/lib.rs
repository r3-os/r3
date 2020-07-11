/*
Copyright ⓒ 2016 rust-custom-derive contributors.

Licensed under the MIT license (see LICENSE or <http://opensource.org
/licenses/MIT>) or the Apache License, Version 2.0 (see LICENSE of
<http://www.apache.org/licenses/LICENSE-2.0>), at your option. All
files in the project carrying such notice may not be copied, modified,
or distributed except according to those terms.
*/
/*!
This crate provides stable, partial implementations of the `parse_generics!` and `parse_where!` macros proposed in [RFC #1583].  These macros serve two purposes:

1. They allow crate authors to use the macros in a limited capacity whether or not the RFC is accepted.
2. They demonstrate to the Rust core team that there is demand for this functionality.
3. They provide a migration path from the partial implementation to the full one, assuming the RFC does get accepted.

Because these macros are implemented using `macro_rules!`, they have the following limitations:

- In general, only lifetimes `'a` through `'z` are accepted.
- Only a subset of the full output formats are supported.
- They are significantly less efficient, and consume a non-trivial amount of the recursion limit.

<style type="text/css">
.link-block { font-family: "Fira Sans"; }
.link-block > p { display: inline-block; }
.link-block > p > strong { font-weight: 500; margin-right: 1em; }
.link-block > ul { display: inline-block; padding: 0; list-style: none; }
.link-block > ul > li {
  font-size: 0.8em;
  background-color: #eee;
  border: 1px solid #ccc;
  padding: 0.3em;
  display: inline-block;
}
</style>
<span></span><div class="link-block">

**Links**

* [Latest Release](https://crates.io/crates/parse-generics-shim)
* [Latest Docs](https://danielkeep.github.io/rust-parse-generics/doc/parse_generics_shim/index.html)
* [Repository](https://github.com/DanielKeep/rust-parse-generics)

<span></span></div>

# Table of Contents

- [`parse_generics_shim!`](#parse_generics_shim)
- [`parse_where_shim!`](#parse_where_shim)
- [Using `parse-generics-poc`](#using-parse-generics-poc)

[RFC #1583]: https://github.com/rust-lang/rfcs/pull/1583

## `parse_generics_shim!`

```ignore
macro_rules! parse_generics_shim {
    (
        { $($fields:ident),+ },
        then $callback_name:ident ! ( $($callback_args:tt)* ),
        $($code:tt)*
    ) => { ... };
}
```

Parses a generic parameter list (if present) from the start of `$($code:tt)*`, expanding to the parsed information plus the unconsumed tokens *after* the parameter list.  The general form of the expansion is:

```ignore
$callback_name! {
    $($callback_args)*
    {
        $(
            $fields: [ .. ],
        )+
    },
    $($tail)*
}
```

### Callback

`$callback_name` and `$callback_args` specify the macro to invoke with the result of parsing.  Note that `$callback_args` may be contained in *any* of `( .. )`, `[ .. ]`, or `{ .. }`.

### Fields

`$fields` indicates which pieces of information you want in the expansion.  The available fields are:

- `constr` - comma-terminated list of generic parameters plus their constraints.
- `params` - comma-terminated list of generic parameter names (both lifetimes and types).
- `ltimes` - comma-terminated list of generic lifetime names.
- `tnames` - comma-terminated list of generic type names.

The shim *only* supports the following combinations:

- `{ constr, params, ltimes, tnames }`
- `{ constr }`
- `{ .. }`

The fields will appear in the output in the same order they appear in the input.  One special case is `{ .. }` which causes *all* fields to be emitted, followed by a literal `..` token.

**Warning**: there is explicitly *no* guarantee that the list of fields will stay the same over time.  As such, it is **strongly** recommended that you never directly match the `..` token after the fields.  Instead, you should use the following construct:

```ignore
macro_rules! match_output {
    (
        {
            // Match the fields you care about.
            constr: $constr:tt,
            params: [ $($params:tt,)* ],

            // Ignore the rest; *never* explicitly match `..`!
            $($_fields:tt)*
        },

        $($tail:tt)*
    ) => { ... };
}
```

### Code

`$code` is the actual source code to be parsed.  If it starts with `<`, the macro will parse a generic parameter list.  If it *does not* start with `<`, the macro will proceed as though the input started with an empty generic parameter list (*i.e.* `<>`).

### Examples

The following show how the various invocation forms affect the output:

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_generics_shim! {
    { constr, params, ltimes, tnames },
    then stringify!(output:),
    <'a, T, U: 'a + Copy> X
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        constr: [ 'a, T, U: 'a + Copy, ],
        params: [ 'a, T, U, ],
        ltimes: [ 'a, ],
        tnames: [ T, U, ],
    },
    X
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_generics_shim! {
    { constr },
    then stringify!(output:),
    <'a, T, U: 'a + Copy> X
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        constr: [ 'a, T, U: 'a + Copy, ],
    },
    X
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_generics_shim! {
    { .. },
    then stringify!(output:),
    <'a, T, U: 'a + Copy> X
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        constr: [ 'a, T, U: 'a + Copy, ],
        params: [ 'a, T, U, ],
        ltimes: [ 'a, ],
        tnames: [ T, U, ],
        ..
    },
    X
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

The input does not *have* to start with a generic parameter list.  Note that both of the invocations below expand to the same result:

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_generics_shim! {
    { constr, params, ltimes, tnames },
    then stringify!(output:),
    <> X
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        constr: [],
        params: [],
        ltimes: [],
        tnames: [],
    },
    X
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_generics_shim! {
    { constr, params, ltimes, tnames },
    then stringify!(output:),
    X
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        constr: [],
        params: [],
        ltimes: [],
        tnames: [],
    },
    X
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

## `parse_where_shim!`

```ignore
macro_rules! parse_where_shim {
    (
        { $($fields:ident),+ },
        then $callback_name:ident ! ( $($callback_args:tt)* ),
        $($code:tt)*
    ) => { ... };
}
```

Parses a `where` clause (if present) from the start of `$($code:tt)*`, expanding to the parsed information plus the unconsumed tokens *after* the clause.  The general form of the expansion is:

```ignore
$callback_name! {
    $($callback_args)*
    {
        $(
            $fields: [ .. ],
        )+
    },
    $($tail)*
}
```

### Callback

`$callback_name` and `$callback_args` specify the macro to invoke with the result of parsing.  Note that `$callback_args` may be contained in *any* of `( .. )`, `[ .. ]`, or `{ .. }`.

### Fields

`$fields` indicates which pieces of information you want in the expansion.  The available fields are:

- `clause` - comma-terminated clause *including* the `where` keyword.  If there is no clause, the `where` keyword is omitted.  Use this if you simply wish to pass a `where` clause through unmodified.
- `preds` - comma-terminated list of predicates.  Use this if you wish to modify or append to the predicates.

The shim *only* supports the following combinations:

- `{ clause, preds }`
- `{ preds }`
- `{ .. }`

The fields will appear in the output in the same order they appear in the input.  One special case is `{ .. }` which causes *all* fields to be emitted, followed by a literal `..` token.

**Warning**: there is explicitly *no* guarantee that the list of fields will stay the same over time.  As such, it is **strongly** recommended that you never directly match the `..` token after the fields.  Instead, you should use the following construct:

```ignore
macro_rules! match_output {
    (
        {
            // Match the fields you care about.
            clause: [ $($clause:tt)* ],

            // Ignore the rest; *never* explicitly match `..`!
            $($_fields:tt)*
        },

        $($tail:tt)*
    ) => { ... };
}
```

### Code

`$code` is the actual source code to be parsed.  If it starts with `where`, the macro will parse a `where` clause, stopping when it encounters any of the following: `;`, `{`, or `=`.  If it *does not* start with `where`, the macro will expand with an empty predicate list.

### Examples

The following show how the various invocation forms affect the output:

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_where_shim! {
    { preds },
    then stringify!(output:),
    where
        'a: 'b,
        T: 'a + Copy,
        for<'c> U: Foo<'c>,
    { struct fields... }
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        preds: [ 'a: 'b, T: 'a + Copy, for<'c,> U: Foo<'c>, ],
    },
    { struct fields... }
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_where_shim! {
    { .. },
    then stringify!(output:),
    where
        'a: 'b,
        T: 'a + Copy,
        for<'c> U: Foo<'c>,
    { struct fields... }
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        clause: [ where 'a: 'b, T: 'a + Copy, for<'c,> U: Foo<'c>, ],
        preds: [ 'a: 'b, T: 'a + Copy, for<'c,> U: Foo<'c>, ],
        ..
    },
    { struct fields... }
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

The input does not *have* to start with a `where` clause:

```rust
# #![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
# #![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
# #[macro_use] extern crate parse_generics_shim;
# fn main() {
# assert_eq!( (
parse_where_shim! {
    { preds },
    then stringify!(output:),
    ; X
}

// Expands to:
# /*
stringify!(
# */
# ).replace(char::is_whitespace, "") , "
    output: {
        preds: [],
    },
    ; X
# ".replace(char::is_whitespace, "")); /*
)
# */ }
```

## Using `parse-generics-poc`

### For Crate Authors

Add the following to your `Cargo.toml` manifest:

```toml
[features]
use-parse-generics-poc = [
    "parse-generics-poc",
    "parse-generics-shim/use-parse-generics-poc",
]

[dependencies]
parse-generics-poc = { version = "0.1.0", optional = true }
parse-generics-shim = "0.1.0"
```

This allows your users to enable the proof-of-concept compiler plugin *through* your crate.  You should also copy and modify the following section (replacing `whizzo` with your crate's name).

### For Crate Users

Add the following to your `Cargo.toml` manifest:

```toml
[features]
use-parse-generics-poc = [
    "whizzo/use-parse-generics-poc",
    "parse-generics-poc",
    "parse-generics-shim/use-parse-generics-poc",
]

[dependencies]
whizzo = "0.1.0"
parse-generics-poc = { version = "0.1.0", optional = true }
parse-generics-shim = "0.1.0"
```

Then, add the following to your crate's root module:

```ignore
#![cfg_attr(feature="parse-generics-poc", feature(plugin))]
#![cfg_attr(feature="parse-generics-poc", plugin(parse_generics_poc))]
#[macro_use] extern crate parse_generics_shim;
#[macro_use] extern crate whizzo;
```

By default, this will use stable-but-inferior implementations of the generics-parsing macros.  In particular, you cannot use lifetimes other than `'a` through `'z`, and macros may fail to expand for sufficiently complex inputs.

If a macro fails to expand due to the "recursion limit", place the following attribute at the top of your crate's root module, and raise the number until the macro works:

```rust
#![recursion_limit="32"]
```

If you are using a compatible nightly compiler, you can enable the fully-featured versions of the generics-parsing macros (see the proposed [RFC #1583](https://github.com/rust-lang/rfcs/pull/1583) for context).  If you have followed the instructions above, this can be done by adding `--features=use-parse-generic-poc` to your `cargo build` command.

The [documentation for `parse-generics-poc`](https://danielkeep.github.io/rust-parse-generics/doc/parse_generics_poc/index.html) will specify *which* nightly it is known to be compatible with.  If you are using `rustup`, you can configure your crate to use the appropriate compiler using the following (replacing the date shown with the one listed in the `parse-generics-poc` documentation):

```sh
rustup override add nightly-2016-04-06
```
*/
#![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
#![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]

#[cfg(not(feature="use-parse-generics-poc"))]
#[doc(hidden)]
#[macro_export]
macro_rules! parse_generics_shim_util {
    (
        @callback
        ($cb_name:ident $(::$cb_sub:ident)* ! ($($cb_arg:tt)*)),
        $($tail:tt)*
    ) => {
        $cb_name $(::$cb_sub)* ! { $($cb_arg)* $($tail)* }
    };

    (
        @callback
        ($cb_name:ident $(::$cb_sub:ident)* ! [$($cb_arg:tt)*]),
        $($tail:tt)*
    ) => {
        $cb_name $(::$cb_sub)* ! { $($cb_arg)* $($tail)* }
    };

    (
        @callback
        ($cb_name:ident $(::$cb_sub:ident)* ! {$($cb_arg:tt)*}),
        $($tail:tt)*
    ) => {
        $cb_name $(::$cb_sub)* ! { $($cb_arg)* $($tail)* }
    };
}

mod parse_constr;
mod parse_generics_shim;
mod parse_where_shim;
