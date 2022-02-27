macro_rules! doctest {
    (#[path = $path:literal] mod $name:ident) => {
        #[doc = include_str!(concat!("../../", $path))]
        pub mod $name {}
    };
}

doctest!(#[path = "doc/toolchain_limitations.md"] mod toolchain_limitations);
