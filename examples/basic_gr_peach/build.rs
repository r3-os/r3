fn main() {
    // Use the linker script `memory.x` at the crate root
    println!(
        "cargo:rustc-link-search={}",
        std::env::current_dir().unwrap().display()
    );
}
