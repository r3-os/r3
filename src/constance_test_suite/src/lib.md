A collection of tests that can be used to verify the correct operation of [the Constance RTOS] and a port implementation.

<div class="distractor"><a style="background-image: url(https://derpicdn.net/img/2014/4/5/593273/medium.png); padding-bottom: 66.66%" href="http://derpibooru.org/593273" title="She passed!"></a></div>

[the Constance RTOS]: constance

# Terminology

 - *A target system* is a computer (e.g., a microcontroller development board) on which test programs run. It's controlled by *a host system*.

 - *A test runner* is a program that runs on a host system and manages the execution of test programs on a target system.

 - *A test driver* is a runtime component that lives on the target system together with a test program. It stands between a test runner and a test program and is responsible for accepting commands from a test runner and returning test results to it.

 - *A test program* is a program that provides stimuli to software components to be tested. It verifies the correct operation of such software components by comparing their actual responses to expected ones.

 - *A test case* is a particular condition in which the behavior of software components is validated. Test cases are often described in the form of programs that interact with the tested components, in which case they are equivalent to test programs.

# Organization

## Build Options

Cargo features `tests_all` and `tests_selective` and an environment variable `CONSTANCE_TEST` specify which test case to compile.

- `tests_all` (enabled by default) enables all test cases.
- `tests_selective` enables the test cases specified by `CONSTANCE_TEST`. `CONSTANCE_TEST` should contain a value like `kernel_tests::basic`.

Constance exposes some optional features through Cargo features (e.g., `system_time`). They are all disabled by default. This crate provides Cargo features of the same names, which enable the respective features of Constance as well as corresponding test cases. `full` Cargo feature enables all of such features (*enabled by default*).

<div class="admonition-follows"></div>

> **Warning:** When building test cases, you should not directly enable the optional features of `constance` (unless they are needed by your test driver or something else) because they will not affect the set of enabled test cases.
>
> ```shell
> # good: enables the test cases dependent on a system time
> cargo test -p constance_port_std --no-default-features \
>   --features constance_test_suite/system_time
>
> # bad: doesn't enable any additional test cases
> cargo test -p constance_port_std --no-default-features \
>   --features constance/system_time
> ```

## Kernel Tests

[`kernel_tests`] contains a set of test cases, each of which is contained in its own module such as [`basic`]. Each module contains a struct named `App<System>` and its constructor `App::new`, which is [a configuration function]. `App::new` takes exactly one generic parameter `D` taking a type implementing [`Driver`]`<App>`, which must be supplied by a test driver.

```rust,ignore
#[cfg(any(
    feature = "tests_all",
    all(feature = "tests_selective", /* ... */)
))]
pub mod test1 {
    pub struct App<System> { /* ... */ }

    impl<System: Kernel> App<System> {
        pub const fn new<D: Driver<Self>>(_: CfgBuilder<System>) -> Self { /* ... */ }
    }
}
pub mod test2 { /* ... */ }
pub mod test3 { /* ... */ }
```

[`kernel_tests`]: crate::kernel_tests
[`basic`]: crate::kernel_tests::basic
[a configuration function]: constance#static-configuration
[`Driver`]: crate::kernel_tests::Driver

A test runner should choose one of test cases, link it to a test driver, run it, and retrieve and report the result. This should be repeated for all test cases.

Each test case concludes in one of the following ways:

 - Calls [`Driver::success`], indicating the test was successful. The test program ensures all tasks are dormant soon after calling this¹. A test runner, however, is allowed to terminate the test program as soon as receiving this indication.
 - Calls [`Driver::fail`], indicating the test was failure.
 - Enters an infinite loop or a deadlock state. This should be detected by a test runner using a timeout duration at least as long as 30 seconds and handled as failure.
 - Panics. This should be handled as failure.

*TODO:* Tests for panic handling

¹ This is because `constance_port_std` doesn't support task termination.

[`Driver::success`]: crate::kernel_tests::Driver::success
[`Driver::fail`]: crate::kernel_tests::Driver::fail

## Kernel Benchmark Tests

[`kernel_benchmarks`] is similar to [`kernel_tests`] but included test cases are intended to measure the runtime performance of kernel functionalities. The results are printed using [`::log`].

Each test case concludes in one of the following ways:

 - Calls [`Driver::success`], indicating the test was complete. The test program ensures all tasks are dormant soon after calling this¹. A test runner, however, is allowed to terminate the test program as soon as receiving this indication.
 - Enters an infinite loop or a deadlock state. This should be detected by a test runner using a timeout duration at least as long as 30 seconds and handled as failure.
 - Panics. This should be handled as failure.

¹ This is because `constance_port_std` doesn't support task termination.

# Writing a Test Runner

## Kernel Tests

For some targets, you can just instantiate all the test cases and have a test driver choose one of them at runtime. You can use [`get_kernel_tests`] to duplicate a piece of code for every defined test case.

However, for most targets, this is not an option. The reasons include (but aren't limited to) that the target has insufficient memory to contain all test cases, or a port instantiation occupies a global namespace (for something like symbol names) and cannot co-exist with other instantiations in a single executable image. In this case, your test runner needs to be able to build a test executable for every test case. The implementation of this consists of two parts:

 1. The test runner should have access to the list of test cases. A naïve approach would be to include the list in the test runner's code, which isn't preferable from a maintainability point of view. Instead, it should use the list exported by this crate as [`TEST_NAMES`].

 2. The build system should be able to conditionally compile test cases as instructed by the test runner. Cargo features aren't ideal for this because they have to be listed in `Cargo.toml` before downstream crates can use them (Cargo rejects unrecognized features), and we want to minimize data redundancy. The build script of this crate implements a work-around. The test runner, when compiling a test driver, should set the environment variable `CONSTANCE_TEST` to the name of the test to run. The Cargo feature `tests_selective` should also be enabled. The build script of this crate will read the value of `CONSTANCE_TEST` and enable the corresponding test case. In the test driver's code, the enabled test can be discovered by using the macro [`get_selected_kernel_tests!`].

[`TEST_NAMES`]: crate::kernel_tests::TEST_NAMES

## Kernel Benchmark Tests

Writing a test runner for the kernel benchmark tests is very similar to writng one for the kernel tests. [`get_kernel_benchmarks`], [`kernel_benchmarks::TEST_NAMES`], and [`get_selected_kernel_benchmarks!`] should be used instead.
