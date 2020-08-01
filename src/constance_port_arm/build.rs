fn main() {
    // Bring `link_*.x` into the list of search paths
    println!(
        "cargo:rustc-link-search={}",
        std::env::current_dir().unwrap().join("ldscripts").display()
    );
}
