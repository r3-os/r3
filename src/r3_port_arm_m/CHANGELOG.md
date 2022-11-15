# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.3] - 2022-11-16

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-11-10`

## [0.3.2] - 2022-08-16

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-08-11`

### Fixed

- Fixed a typo in an error message.

## [0.3.1] - 2022-03-19

### Fixed

- Improve rustdoc theme detection on docs.rs

## [0.3.0] - 2022-03-15

### Changed

- **Breaking:** Adjusted for the new design of R3-OS (separation between interface and implementation). Supports `r3_kernel ^0.1`.
- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-10`

### Fixed

- The default stack alignment (`PortThreading::STACK_ALIGN`) now conforms to the architectural requirement (double-word alignment).

## [0.2.1] - 2021-10-29

This release only includes changes to the documentation.

## [0.2.0] - 2021-10-23

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2021-10-18`
- **Breaking:** The `cortex-m-rt` binding has been separated to `r3_port_arm_m::use_rt!`.
- Support `cortex-m` `^0.6` *and* `^0.7`
- Support `cortex-m-rt` `^0.6` *and* `^0.7`
- `r3_port_arm_m` now steals `cortex_m::Peripherals` on boot. This is useful in multi-core systems.

## [0.1.1] - 2020-12-20

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2020-11-25`

## 0.1.0 - 2020-11-03

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.3.2...HEAD
[0.3.2]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.3.1...r3_port_arm_m@0.3.2
[0.3.1]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.3.0...r3_port_arm_m@0.3.1
[0.3.0]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.2.1...r3_port_arm_m@0.3.0
[0.2.1]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.2.0...r3_port_arm_m@0.2.1
[0.2.0]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.1.1...r3_port_arm_m@0.2.0
[0.1.1]: https://github.com/r3-os/r3/compare/r3_port_arm_m@0.1.0...r3_port_arm_m@0.1.1

