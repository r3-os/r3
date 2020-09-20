use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CONSTANCE_TEST_DRIVER_LINK_SEARCH");
    if let Ok(link_search) = env::var("CONSTANCE_TEST_DRIVER_LINK_SEARCH") {
        println!("cargo:rustc-link-search={}", link_search);
    }

    // Use the linker script `device.x` at the crate root
    println!(
        "cargo:rustc-link-search={}",
        std::env::current_dir().unwrap().display()
    );
}
