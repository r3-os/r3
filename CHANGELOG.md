# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking:** Change the target compiler version to `nightly-2021-10-18`
- Replace `register 1` with `tock-registers 0.7` because `tock-registers 0.6`, which is used by `register`, isn't compatible with the current target compiler.
- Upgrade `r0` to `^1.0.0`
- Upgrade `tokenlock` to `0.3.4`
- Support `cortex-m` `^0.6` *and* `^0.7`
- Support `cortex-m-rt` `^0.6` *and* `^0.7`
- Support `riscv` `^0.5`, `^0.6`, *and* `^0.7`
- Using the new version of `tokenlock`, some atomics-based hacks were removed. This might marginally improve the runtime performance as the compiler is given more leeway to optimize memory accesses.
- **Breaking:** The `cortex-m-rt` binding has been separated to `r3_port_arm_m::use_rt!`.
- `r3_port_arm_m` now steals `cortex_m::Peripherals` on boot. This is useful in multi-core systems.

<!-- TODO: It doesn't seem like a good idea to congregate the changes of all packages, each of which has its own pace, into a single changelog file -->

### Fixed

- The debug printing of `Mutex` and `RecursiveMutex` in an invalid context now produces a message that makes sense.
- Rewrite invalid `#[naked]` functions in valid forms

## [0.1.1] - 2020-12-20

### Added

- `r3_port_std`'s POSIX backend now supports AArch64.

### Changed

- Change the target compiler version to `nightly-2020-11-25`

### Fixed

- Wrap const generic arguments in braces, fixing builds on the latest compiler version
- Remove `#[naked]` when inlining is prerequisite for correctness; functions with `#[naked]` are no longer eligible for inlining as of [rust-lang/rust#79192](https://github.com/rust-lang/rust/pull/79192).

## 0.1.0 - 2020-11-03

- Initial release.

[Unreleased]: https://github.com/yvt/r3/compare/0.1.1...HEAD
[0.1.1]: https://github.com/yvt/r3/compare/0.1.0...0.1.1
