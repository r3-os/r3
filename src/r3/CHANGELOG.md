# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-08-11`
- Removed trait bounds in items and item documentations that have been made redundant by the supertraits of `r3_core::kernel::traits::Cfg*`, which now include `~const`.

## [0.2.2] - 2022-03-30

### Changed

- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-30`

## [0.2.1] - 2022-03-19

### Fixed

- Upgrade `svgbobdoc` to `^0.3.0` to fix build failures in documentation build
- Improve rustdoc theme detection on docs.rs

## [0.2.0] - 2022-03-15

**The overall design has been revamped!** The original kernel implementation has been moved to a separate crate `r3_kernel`. `r3_core` harbors the interface definition for a kernel implementation, and `r3` provides additional useful items, such as a mutex-backed cell type (`StaticMutex`). Different kernel implementations that use more exotic architectures (such as interrupt-driven multi-threading) or are built on top on existing RTOSes may be added in the future.

### Changed

While much of the application-level API has retained its general shape, there are some significant changes that may require attention:

 - Introduces *object safety*. All kernel object handle types now have the following variations: `Mutex<_>` (owned), `MutexRef<'_, _>` (borrowed), `StaticMutex` (static). Owned handles aren't usable yet.
 - The way of instantiating a kernel and defining a system type has changed. The port-provided macro now defines a *kernel trait type* (`SystemTraits: r3_kernel::KernelTraits`), which is then used as a generic parameter to create a concrete system type (`type System = r3_kernel::System<SystemTraits>`). Please see the package documentation (`README.md`) for an example.
 - `r3::kernel::Task::current` was moved to `r3::kernel::LocalTask::current` and now requires a task context. It returns `LocalTask`, which cannot be sent to another thread but whose reference (`&LocalTask` or `TaskRef`) can be.
 - `r3::kernel::ResultCode::BadId` was renamed to `NoAccess` and covers general protection failures detected by a now-optional protection mechanism. This means that application and library code can't rely on `NoAccess` being returned reliably anymore (it can't anyway once owned handles are implemented), and that a kernel implementation may use this error code to indicate that a given kernel object ID might be valid, but the caller lacks the necessary privileges to access that object.
 - The `chrono` Cargo feature was renamed to `chrono_0p4`.
 - `r3::sync` is now gated by `cfg(feature = "sync")`.
 - `r3::sync::{Mutex, RecursiveMutex}` is now backed by bindings (`Bind`). The default value is now `<T as Default>::default()` instead of `<T as Init>::INIT`. Additional options are available, which means you no longer have to implement `Init` just to put a custom type in `Mutex`, and that each `Mutex` can have a distinct initial value.
- **Breaking (semver-exempt):** Change the target compiler version to `nightly-2022-03-10`

### Added

- Bindings (`Bind`), a static storage with runtime initialization and configuration-time borrow checking. They can be used, for example, to provide a task or timer callback with a state data storage that can be mutably borrowed and persists across invocations.
- `Kernel::{is_task_context, is_interrupt_context, is_boot_complete}`

### Fixed

- The hard-coded kernel object count limitation has been removed. This was made possible by the use of growable arrays in the compile-time kernel configurator.

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

[Unreleased]: https://github.com/r3-os/r3/compare/r3@0.2.2...HEAD
[0.2.2]: https://github.com/r3-os/r3/compare/r3@0.2.1...r3@0.2.2
[0.2.1]: https://github.com/r3-os/r3/compare/r3@0.2.0...r3@0.2.1
[0.2.0]: https://github.com/r3-os/r3/compare/r3@0.1.3...r3@0.2.0
[0.1.3]: https://github.com/r3-os/r3/compare/r3@0.1.2...r3@0.1.3
[0.1.2]: https://github.com/r3-os/r3/compare/r3@0.1.1...r3@0.1.2
[0.1.1]: https://github.com/r3-os/r3/compare/r3@0.1.0...r3@0.1.1
