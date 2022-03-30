# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-30`

## [0.2.1] - 2022-03-19

### Fixed

- Upgrade `svgbobdoc` to `^0.3.0` to fix build failures in documentation build
- Improve rustdoc theme detection on docs.rs

## [0.2.0] - 2022-03-15

### Changed

- **Breaking:** Adjusted for the new design of R3-OS (separation between interface and implementation). Supports `r3_kernel ^0.1`.
- **Breaking:** `sym_static!` was redesigned to address multiple issues.
- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-10`

## [0.1.3] - 2021-10-29

This release only includes changes to the documentation.

## [0.1.2] - 2021-10-23

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2021-10-18`

## [0.1.1] - 2020-12-20

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2020-11-25`

### Added

- The POSIX backend now supports AArch64.

## 0.1.0 - 2020-11-03

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_portkit@0.2.1...HEAD
[0.2.1]: https://github.com/r3-os/r3/compare/r3_portkit@0.2.0...r3_portkit@0.2.1
[0.2.0]: https://github.com/r3-os/r3/compare/r3_portkit@0.1.3...r3_portkit@0.2.0
[0.1.3]: https://github.com/r3-os/r3/compare/r3_portkit@0.1.2...r3_portkit@0.1.3
[0.1.2]: https://github.com/r3-os/r3/compare/r3_portkit@0.1.1...r3_portkit@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3_portkit@0.1.0...r3_portkit@0.1.1

