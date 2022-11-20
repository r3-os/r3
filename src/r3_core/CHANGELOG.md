# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking:** `{Bind,Hunk}Definer::zeroed` now requires `T: Zeroable` instead of `T: ZeroInit`.

### Added

- `r3_core::utils::{Zeroable,ZeroableInOption}` (re-exported from `bytemuck ^1`)
- Implement `Zeroable` on `r3_core::time::{Duration, Time}`

## [0.1.4] - 2022-11-16

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-11-10`

### Fixed

- Remove a broken link to the rustdoc-generated Macros section in the crate documentation

## [0.1.3] - 2022-08-16

### Added

- The new blanket-implemented `CfgStatic` trait can be used to simplify some trait bounds of configuration functions.
- The `Cfg*` traits now include `~const` in their supertraits ([rust-lang/rust#93429](https://github.com/rust-lang/rust/pull/93429)), making `~const CfgBase` trait bound unnecessary if it's implied by others.

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-08-11`

### Fixed

- The `Cfg*` traits now imply the corresponding `Kernel*` traits (e.g., `C: CfgTimer` implies `C::System: KernelTimer`), making some trait bounds in configuration functions unnecessary.

## [0.1.2] - 2022-03-30

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-30`

## [0.1.1] - 2022-03-19

### Fixed

- Upgrade `svgbobdoc` to `^0.3.0` to fix build failures in documentation build
- Improve rustdoc theme detection on docs.rs

## 0.1.0 - 2022-03-15

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_core@0.1.4...HEAD
[0.1.4]: https://github.com/r3-os/r3/compare/r3_core@0.1.3...r3@0.1.4
[0.1.3]: https://github.com/r3-os/r3/compare/r3_core@0.1.2...r3@0.1.3
[0.1.2]: https://github.com/r3-os/r3/compare/r3_core@0.1.1...r3@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3_core@0.1.0...r3@0.1.1
