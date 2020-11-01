The test driver for `r3_port_arm_m`. The test runner (`r3_test_runner`) compiles this crate for each test case.

This crate should compile without an error even when built directly so that workspace-global operations such as `cargo check --workspace` don't break.
