# Coding Guidelines

## General

- [The Rust API guidelines] should be followed unless specified otherwise.

[The Rust API guidelines]: https://github.com/rust-lang/api-guidelines/tree/91939a78784e97ec3e2d84abed905738a7fd4224

## Naming

### Casing (CC-CASE)

Crate names should be in `snake_case`. This is left unclear in [C-CASE] and many crates in public (e.g., `tokio-core`) use `kebab-case`, which is automatically converted to `snake_case` when referred to in code. We use `snake_case` in all cases not to surprise newcomers and to facilitate grepping.

Cargo feature names should be in `snake_case`.

Other names should follow [C-CASE] and [RFC 430].

[C-CASE]: https://github.com/rust-lang/api-guidelines/blob/91939a78784e97ec3e2d84abed905738a7fd4224/src/naming.md#casing-conforms-to-rfc-430-c-case
[RFC 430]: https://github.com/rust-lang/rfcs/blob/master/text/0430-finalizing-naming-conventions.md

### Type parameters should be self-descriptive (CC-TYPE-PARAM)

The type parameters of public items (and private items, albeit more leniently) should be self-descriptive and should not be abbreviated to any extent more than other names.

```rust
// good
pub struct ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey, InconsistencyHandler> {}

impl<'a, HeadCell, Index, Pool, MapLink, Element, LinkCell, CellKey>
    ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey, HandleInconsistencyByReturningError>
where
    HeadCell: CellLike<CellKey, Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &LinkCell,
    LinkCell: CellLike<CellKey, Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{}

// bad - indecipherable, but could be good r/rustjerk material
pub struct ListAccessorCell<'a, H, P, M, C, I> {}

impl<'a, H, I, P, M, E, L, C>
    ListAccessorCell<'a, H, P, M, C, I>
where
    H: CellLike<C, Target = ListHead<I>>,
    P: ops::Index<I, Output = E>,
    M: Fn(&E) -> &L,
    L: CellLike<C, Target = Option<Link<I>>>,
    I: PartialEq + Clone,
{}

```

Exceptions:

- The `T` parameter of a container type or anything in which the semantics of the parameter is clear (e.g., `Mutex<System, T>`.
- `F: FnOnce`

## Documentation

### Optional features should be documented properly (CC-DOC-OPT-FEATURES)

All optional features of `constance` must be listed and explained in the crate-level documentation.

In addition, every public item gated by such features must have [`#[doc(cfg(feature = ...))]` attribute](https://github.com/rust-lang/rust/issues/43781), which displays the required feature on the generated documentation.

```rust
/// Get the current [system time].
///
/// [system time]: crate#kernel-timing
#[cfg(feature = "system_time")]
#[doc(cfg(feature = "system_time"))]
fn time() -> Result<Time, TimeError>;
```

## Testing

### The CI configuration should cover a variety of combinations of optional features (CC-CI-OPT-FEATURES)

We want to ensure the kernel can be built successfully under any combination of optional features (e.g., `system_time`). To this end, the CI configuration must run the test suite should run the test suite with various combinations of such features. At least, the test suite should run with each feature singled out (i.e., with other features disabled).

## Performance

### Unused features should not incur a runtime overhead (CC-PERF-UNUSED)

The runtime overhead caused by unused features should be minimized or eliminated in one of the following ways:

- In many cases, the compiler can simply optimize out unused code.

  Example: If no startup hooks are defined, the compiler will simply remove the startup hook initialization loop because it can figure out that `STARTUP_HOOKS` has no elements.

- If the usage of such features can be detected statically in a configuration macro (e.g., `constance::build!`), the macro should control the type choices based on that.

  Examples:

  - `constance_portkit::tickful::TickfulState` (used by timer drivers) chooses the optimal algorithm based on parameters.

  - Kernel objects are defined statically and their control blocks are stored in static arrays.

- If the above options are infeasible, expose either a `CfgBuilder` method or a Cargo feature to let downstream crates and application developers specify whether a feature should be compiled in.

  Examples:

  - The system clock is controlled by `system_time` feature. The system time is tracked by an internal variable that is updated on timer interrupts, and there's no hope of the compiler optimizing this out. It's impossible for `build!` to detect the usage of `System::time()`. The system clock is not tied to any particular kernel objects, so the software components dependent on the system clock might not have a configuration function. On the other hand, Cargo features are designed exactly for this use case.

  - Application code can change task priorities at runtime. The maximum (lowest) priority affects the size of internal kernel structures, but it's impossible for `build!` to figure that out. Therefore, `CfgBuilder` exposes `num_task_priority_levels` method.
