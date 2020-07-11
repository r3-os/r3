/*
Copyright ⓒ 2016 rust-custom-derive contributors.

Licensed under the MIT license (see LICENSE or <http://opensource.org
/licenses/MIT>) or the Apache License, Version 2.0 (see LICENSE of
<http://www.apache.org/licenses/LICENSE-2.0>), at your option. All
files in the project carrying such notice may not be copied, modified,
or distributed except according to those terms.
*/
extern crate constance_parse_generics;
extern crate rustc_version;

macro_rules! aeqiws {
    ($lhs:expr, $rhs:expr) => {
        {
            let lhs = $lhs;
            let rhs = $rhs;
            let lhs_words = lhs.split_whitespace();
            let rhs_words = rhs.split_whitespace();
            let lhs = lhs_words.collect::<Vec<_>>().join("");
            let rhs = rhs_words.collect::<Vec<_>>().join("");
            assert_eq!(lhs, rhs);
        }
    };
}

macro_rules! pgts {
    ($($body:tt)*) => {
        constance_parse_generics::parse_constr! {
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
