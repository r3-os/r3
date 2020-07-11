/*
Copyright ⓒ 2016 rust-custom-derive contributors.

Licensed under the MIT license (see LICENSE or <http://opensource.org
/licenses/MIT>) or the Apache License, Version 2.0 (see LICENSE of
<http://www.apache.org/licenses/LICENSE-2.0>), at your option. All
files in the project carrying such notice may not be copied, modified,
or distributed except according to those terms.
*/
#![cfg(not(feature="use-parse-generics-poc"))]
extern crate parse_generics_shim;
extern crate rustc_version;

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

macro_rules! pgts {
    ($($body:tt)*) => {
        parse_generics_shim::parse_constr! {
            (true, true),
            then stringify!(),
            $($body)*
        }
    };
}

macro_rules! rv {
    ($($tts:tt)*) => {
        ::rustc_version::version_matches(concat!($(stringify!($tts)),*))
    };
}

#[test]
fn test_simple() {
    aeqiws!(pgts!('a, X), "{ 'a } , , X");
    aeqiws!(pgts!('a> X), "{ 'a } , > X");
    aeqiws!(pgts!('a {X}), "{ 'a } , { X }");
    aeqiws!(pgts!('a; X), "{ 'a } , ; X");
    aeqiws!(pgts!('a = X), "{ 'a } , = X");

    aeqiws!(pgts!(T; X), "{ T } , ; X");
    aeqiws!(pgts!('a + T; X), "{ 'a + T } , ; X");
    aeqiws!(pgts!('a + 'b; X), "{ 'a + 'b } , ; X");
    aeqiws!(pgts!('a + 'b + T; X), "{ 'a + 'b + T } , ; X");
    aeqiws!(pgts!('a + ::std::clone::Clone; X),
        if rv!(1.10) { "{ 'a + :: std :: clone :: Clone } , ; X" }
        else { "{ 'a + :: std:: clone:: Clone } , ; X" } );
    aeqiws!(pgts!('a + From<u8>; X), "{ 'a + From < u8 > } , ; X");
    aeqiws!(pgts!('a + From<Bar<u8>>; X), "{ 'a + From < Bar < u8 >> } , ; X");
    aeqiws!(pgts!('a + From<Bar>> X), "{ 'a + From < Bar > } , > X");
}
