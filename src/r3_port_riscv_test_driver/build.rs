use std::{env, fmt::Write, fs, path::Path};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rerun-if-env-changed=R3_TEST_DRIVER_LINK_SEARCH");
    if let Ok(link_search) = env::var("R3_TEST_DRIVER_LINK_SEARCH") {
        println!("cargo:rustc-link-search={link_search}");
    }

    let mut generated_code = String::new();

    // Driver-defined test
    println!("cargo:rerun-if-env-changed=R3_DRIVER_TEST");
    let selected_test = match env::var("R3_DRIVER_TEST") {
        Ok(x) => x,
        Err(env::VarError::NotPresent) => String::new(),
        Err(env::VarError::NotUnicode(_)) => {
            panic!("R3_DRIVER_TEST is not a valid UTF-8 string");
        }
    };

    if let Some(name) = selected_test.strip_prefix("kernel_tests::") {
        writeln!(
            generated_code,
            r#"
            instantiate_test!({{
                path: crate::driver_kernel_tests::{0},
            }},);
            "#,
            name,
        )
        .unwrap();
    } else if !selected_test.is_empty() {
        panic!("unknown test type: {:?}", selected_test);
    }

    let out_generated_code_path = Path::new(&out_dir).join("gen.rs");
    fs::write(&out_generated_code_path, &generated_code).unwrap();
}
