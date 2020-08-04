use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CONSTANCE_PORT_ARM_M_TEST_DRIVER_LINK_SEARCH");
    if let Ok(link_search) = env::var("CONSTANCE_PORT_ARM_M_TEST_DRIVER_LINK_SEARCH") {
        println!("cargo:rustc-link-search={}", link_search);
    }
}
