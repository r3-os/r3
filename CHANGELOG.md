# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Change the target compiler version to `nightly-2021-02-19`
- Upgrade `register` to `>= 0.5.1, < 2.0.0`
- Upgrade `r0` to `^1.0.0`
- Upgrade `tokenlock` to `0.3.4`
- Using the new version of `tokenlock`, some atomics-based hacks were removed. This might marginally improve the runtime performance as the compiler is given more leeway to optimize memory accesses.

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
