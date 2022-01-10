# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The new option `ThreadingOptions::PRIVILEGE_LEVEL` allows for running the kernel in other privilege levels than M-mode.
- `use_sbi_timer!` can be used to install a timer driver based on [the RISC-V Supervisor Binary Interface](https://github.com/riscv-non-isa/riscv-sbi-doc).

### Changed

- Rename `use_timer!` → `use_mtime!`, `TimerOptions` → `MtimeOptions`

### Fixed

- The default stack alignment (`PortThreading::STACK_ALIGN`) now conforms to the standard ABI requirement (128-bit alignment).
- The port startup code now calls `<Traits as Timer>::init`.

## [0.1.3] - 2021-10-29

This release only includes changes to the documentation.

## [0.1.2] - 2021-10-23

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2021-10-18`
- Support `riscv` `^0.5`, `^0.6`, *and* `^0.7`
- Replace `register 1` with `tock-registers 0.7` because `tock-registers 0.6`, which is used by `register`, isn't compatible with the current target compiler.

### Fixed

- Rewrite invalid `#[naked]` functions in valid forms

## [0.1.1] - 2020-12-20

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2020-11-25`

### Fixed

- Remove `#[naked]` when inlining is prerequisite for correctness; functions with `#[naked]` are no longer eligible for inlining as of [rust-lang/rust#79192](https://github.com/rust-lang/rust/pull/79192).

## 0.1.0 - 2020-11-03

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3_port_riscv@0.1.3...HEAD
[0.1.3]: https://github.com/r3-os/r3/compare/r3_port_riscv@0.1.2...r3_port_riscv@0.1.3
[0.1.2]: https://github.com/r3-os/r3/compare/r3_port_riscv@0.1.1...r3_port_riscv@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3_port_riscv@0.1.0...r3_port_riscv@0.1.1

