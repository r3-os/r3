# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

**The design has been wholly revamped!** `r3` now only defines the interface between an application and a kernel implementation. The current kernel implementation has been moved to `r3_kernel`. Different kernel implementations that use more exotic architectures (such as interrupt-driven multi-threading) or are built on top on existing RTOSes may be added in the future.

### Added

 - `r3::time::{Duration, Time}: const Default`

### Changed

While much of the application-level API has retained its general shape, there are some significant changes that may require attention:

 - Introduces *object safety*. All kernel object handle types now have the following variations: `Mutex<_>` (owned), `MutexRef<'_, _>` (borrowed), `StaticMutex` (static). Owned handles aren't usable yet.
 - `r3::kernel::Task::current` was moved to `r3::kernel::LocalTask::current` and now requires a task context. It returns `LocalTask`, which cannot be sent to another thread but whose reference (`&LocalTask` or `TaskRef`) can be.
 - `r3::kernel::ResultCode::BadId` was renamed to `NoAccess` and covers general protection failures detected by a now-optional protection mechanism. This means that application and library code can't rely on `NoAccess` being returned reliably anymore (it can't anyway once owned handles are implemented), and that a kernel implementation may use this error code to indicate that a given kernel object ID might be valid, but the caller lacks the necessary privileges to access that object.
 - The `chrono` Cargo feature was renamed to `chrono_0p4`.

TODO

## [0.1.3] - 2021-10-29

This release only includes changes to the documentation.

## [0.1.2] - 2021-10-23

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2021-10-18`
- Upgrade `tokenlock` to `0.3.4`
- Using the new version of `tokenlock`, some atomics-based hacks were removed. This might marginally improve the runtime performance as the compiler is given more leeway to optimize memory accesses.

### Fixed

- The debug printing of `Mutex` and `RecursiveMutex` in an invalid context now produces a message that makes sense.

## [0.1.1] - 2020-12-20

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2020-11-25`

### Fixed

- Wrap const generic arguments in braces, fixing builds on the latest compiler version

## 0.1.0 - 2020-11-03

Initial release.

[Unreleased]: https://github.com/r3-os/r3/compare/r3@0.1.3...HEAD
[0.1.3]: https://github.com/r3-os/r3/compare/r3@0.1.2...r3@0.1.3
[0.1.2]: https://github.com/r3-os/r3/compare/r3@0.1.1...r3@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3@0.1.0...r3@0.1.1
