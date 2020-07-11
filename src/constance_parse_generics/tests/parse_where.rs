/*
Copyright ⓒ 2016 rust-custom-derive contributors.

Licensed under the MIT license (see LICENSE or <http://opensource.org
/licenses/MIT>) or the Apache License, Version 2.0 (see LICENSE of
<http://www.apache.org/licenses/LICENSE-2.0>), at your option. All
files in the project carrying such notice may not be copied, modified,
or distributed except according to those terms.
*/
#![cfg_attr(feature="use-parse-generics-poc", feature(plugin))]
#![cfg_attr(feature="use-parse-generics-poc", plugin(parse_generics_poc))]
extern crate parse_generics_shim;

macro_rules! as_item { ($i:item) => { $i } }

macro_rules! aeqiws {
    ($lhs:expr, $rhs:expr) => {
        {
            let lhs = $lhs;
            let rhs = $rhs;
            let lhs_words = $lhs.split_whitespace();
            let rhs_words = $rhs.split_whitespace();
            for (i, (l, r)) in lhs_words.zip(rhs_words).enumerate() {
                if l != r {
                    panic!("assertion failed: `(left == right)` (left: `{:?}`, right: `{:?}`, at word {}, `{:?}` != `{:?}`)", lhs, rhs, i, l, r);
                }
            }
        }
    };
}

macro_rules! pwts {
    ($fields:tt, $($body:tt)*) => {
        parse_generics_shim::parse_where_shim! {
            $fields,
            then stringify!(),
            $($body)*
        }
    };
}

#[test]
fn test_no_where() {
    aeqiws!(
        pwts!({..}, X),
        r#"
            { clause : [ ] , preds : [ ] , .. } ,
            X
        "#
    );

    aeqiws!(
        pwts!({ clause, preds }, X),
        r#"
            { clause : [ ] , preds : [ ] , } ,
            X
        "#
    );

    aeqiws!(
        pwts!({ preds }, X),
        r#"
            { preds : [ ] , } ,
            X
        "#
    );
}

#[test]
fn test_where() {
    aeqiws!(
        pwts!({..}, where 'a: 'b; X),
        r#"
            {
                clause : [ where 'a : 'b , ] ,
                preds : [ 'a : 'b , ] ,
                ..
            } ,
            ; X
        "#
    );

    aeqiws!(
        pwts!({..}, where T: 'a + U; X),
        r#"
            {
                clause : [ where T : 'a + U , ] ,
                preds : [ T : 'a + U , ] ,
                ..
            } ,
            ; X
        "#
    );

    aeqiws!(
        pwts!({..}, where 'a: 'b, T: 'a + U; X),
        r#"
            {
                clause : [ where 'a : 'b , T : 'a + U , ] ,
                preds : [ 'a : 'b , T : 'a + U , ] ,
                ..
            } ,
            ; X
        "#
    );

    aeqiws!(
        pwts!({..}, where 'a: 'b, T: 'a + U, {} X),
        r#"
            {
                clause : [ where 'a : 'b , T : 'a + U , ] ,
                preds : [ 'a : 'b , T : 'a + U , ] ,
                ..
            } ,
            { } X
        "#
    );

    aeqiws!(
        pwts!({..}, where for<> T: 'a; X),
        r#"
            {
                clause : [ where T : 'a , ] ,
                preds : [ T : 'a , ] ,
                ..
            } ,
            ; X
        "#
    );

    aeqiws!(
        pwts!({..}, where for<'a> T: 'a; X),
        r#"
            {
                clause : [ where for < 'a , > T : 'a , ] ,
                preds : [ for < 'a , > T : 'a , ] ,
                ..
            } ,
            ; X
        "#
    );

    aeqiws!(
        pwts!({..}, where for<'a: 'b> T: 'a; X),
        r#"
            {
                clause : [ where for < 'a : 'b , > T : 'a , ] ,
                preds : [ for < 'a : 'b , > T : 'a , ] ,
                ..
            } ,
            ; X
        "#
    );

    aeqiws!(
        pwts!({..}, where 'a: 'b, for<'a: 'b> T: 'a, 'c: 'a + 'b; X),
        r#"
            {
                clause : [ where 'a : 'b , for < 'a : 'b , > T : 'a , 'c : 'a + 'b , ] ,
                preds : [ 'a : 'b , for < 'a : 'b , > T : 'a , 'c : 'a + 'b , ] ,
                ..
            } ,
            ; X
        "#
    );
}
