# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-08-11`

## [0.2.2] - 2022-03-30

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-30`

## [0.2.1] - 2022-03-19

### Fixed

- Improve rustdoc theme detection on docs.rs

## [0.2.0] - 2022-03-15

### Changed

- **Breaking:** Adjusted for the new design of R3-OS (separation between interface and implementation). Supports `r3_kernel ^0.1`.
- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-10`

### Fixed

- The default stack alignment (`PortThreading::STACK_ALIGN`) now conforms to the architectural requirement (double-word alignment).

## [0.1.2] - 2021-10-29

This release only includes changes to the documentation.

## [0.1.1] - 2021-10-23

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2021-10-18`
- Upgrade `r0` to `^1.0.0`
- Replace `register 1` with `tock-registers 0.7` because `tock-registers 0.6`, which is used by `register`, isn't compatible with the current target compiler.

### Fixed

- Remove `#[naked]` when inlining is prerequisite for correctness; functions with `#[naked]` are no longer eligible for inlining as of [rust-lang/rust#79192](https://github.com/rust-lang/rust/pull/79192).
- Rewrite invalid `#[naked]` functions in valid forms

## 0.1.0 - 2020-11-03

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_port_arm@0.2.2...HEAD
[0.2.2]: https://github.com/r3-os/r3/compare/r3_port_arm@0.2.1...r3_port_arm@0.2.2
[0.2.1]: https://github.com/r3-os/r3/compare/r3_port_arm@0.2.0...r3_port_arm@0.2.1
[0.2.0]: https://github.com/r3-os/r3/compare/r3_port_arm@0.1.2...r3_port_arm@0.2.0
[0.1.2]: https://github.com/r3-os/r3/compare/r3_port_arm@0.1.1...r3_port_arm@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3_port_arm@0.1.0...r3_port_arm@0.1.1
