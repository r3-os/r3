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
