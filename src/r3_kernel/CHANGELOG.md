# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-11-10`

## [0.1.3] - 2022-08-16

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-08-11`

### Fixed

- Fixed an unexposed soundness bug in `Timeout`'s destructor in which the destructor started unwinding instead of aborting on precondition violation. This could only be triggered by a bug in internal code, and we are not aware of any instances of such bugs. Triggering the bug also requires the `unwind` panic strategy, which is not supported by bare-metal targets.

## [0.1.2] - 2022-03-30

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-30`

## [0.1.1] - 2022-03-19

### Fixed

- Upgrade `svgbobdoc` to `^0.3.0` to fix build failures in documentation build
- Improve rustdoc theme detection on docs.rs

## 0.1.0 - 2022-03-15

Initial release. Supports `r3_core ^0.1`.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_kernel@0.1.3...HEAD
[0.1.3]: https://github.com/r3-os/r3/compare/r3_kernel@0.1.2...r3@0.1.3
[0.1.2]: https://github.com/r3-os/r3/compare/r3_kernel@0.1.1...r3@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3_kernel@0.1.0...r3@0.1.1
