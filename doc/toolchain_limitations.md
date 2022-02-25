# Toolchain Limitations

This document lists some of the known limitations or incomplete features present in the current compiler toolchain or the compiler itself, which, when resolved, will improve the quality of our codebase.

All items in here are given [Tagref][1] tags for cross-referencing. All code examples in here are [doc-tested][2] to maintain validity.

## What should be listed here?

The items listed here should meet the following criteria:

 1. There's a concrete example in our codebase where they limit the code quality.
 2. They appear temporary on the basis that they are obvious or recognized compiler bugs (e.g., they are listed under [the Rust bug tracker][3] with a C-bug label), or that they represent unimplemented features, and there's a conceivable way (preferably shown by a submitted `(pre-)*`RFC) in which they might be implemented in a foreseeable feature.

[1]: https://github.com/stepchowfun/tagref
[2]: https://doc.rust-lang.org/1.58.1/rustdoc/documentation-tests.html#documentation-tests
[3]: https://github.com/rust-lang/rust/issues
