fn main() {
    // Use the linker script `link.x` at the crate root
    println!(
        "cargo:rustc-link-search={}",
        std::env::current_dir().unwrap().display()
    );
}
