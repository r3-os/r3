# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2021-10-23

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

[Unreleased]: https://github.com/yvt/r3/compare/r3_port_arm_m@0.1.2...HEAD
[0.1.2]: https://github.com/yvt/r3/compare/r3_port_arm_m@0.1.1...r3_port_arm_m@0.1.2
[0.1.1]: https://github.com/yvt/r3/compare/r3_port_arm_m@0.1.0...r3_port_arm_m@0.1.1

