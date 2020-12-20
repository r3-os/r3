# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2020-12-20

### Added

- `r3_port_std`'s POSIX backend now supports AArch64.

### Fixed

- Wrap const generic arguments in braces, fixing builds on the latest compiler version
- Remove `#[naked]` when inlining is prerequisite for correctness; functions with `#[naked]` are no longer eligible for inlining as of [rust-lang/rust#79192](https://github.com/rust-lang/rust/pull/79192).

## 0.1.0 - 2020-11-03

- Initial release.

[Unreleased]: https://github.com/yvt/r3/compare/0.1.1...HEAD
[0.1.1]: https://github.com/yvt/r3/compare/0.1.0...0.1.1
