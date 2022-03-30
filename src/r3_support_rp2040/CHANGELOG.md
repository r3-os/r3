# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-30`

## [0.2.1] - 2022-03-19

### Fixed

- Improve rustdoc theme detection on docs.rs

## [0.2.0] - 2022-03-15

### Changed

- **Breaking:** Adjusted for the new design of R3-OS (separation between interface and implementation). Supports `r3_port_arm_m ^0.3`.
- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-10`
- [`rp2040-pac ^0.3`](https://crates.io/crates/rp2040-pac) replaces [`rp2040 ^0.1`](https://crates.io/crates/rp2040) as the RP2040 peripheral access crate used by `r3_support_rp2040`.

## [0.1.1] - 2021-10-29

This release only includes changes to the documentation.

## 0.1.0 - 2021-10-23

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_support_rp2040@0.2.1...HEAD
[0.2.1]: https://github.com/r3-os/r3/compare/r3_support_rp2040@0.2.0...r3_support_rp2040@0.2.1
[0.2.0]: https://github.com/r3-os/r3/compare/r3_support_rp2040@0.1.1...r3_support_rp2040@0.2.0
[0.1.1]: https://github.com/r3-os/r3/compare/r3_support_rp2040@0.1.0...r3_support_rp2040@0.1.1
