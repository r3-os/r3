# Toolchain Limitations

This document lists some of the known limitations or incomplete features present in the current compiler toolchain, the compiler itself, or the dependent packages, which, when resolved, will improve the quality of our codebase.

All items in here are given [Tagref][1] tags for cross-referencing. All code examples in here are [doc-tested][2] to maintain validity.


## What should be listed here?

The items listed here should meet the following criteria:

 1. There's a concrete example in our codebase where they limit the code quality.
 2. They appear temporary on the basis that they are obvious or recognized compiler bugs (e.g., they are listed under [the Rust bug tracker][3] with a C-bug label), or that they represent unimplemented features, and there's a conceivable way (preferably shown by a submitted `(pre-)*`RFC) in which they might be implemented in a foreseeable feature.


## Generics

### `[tag:generic_fn_ptr_wrapper]` A generic wrapper of a function pointer type can't be generic over higher-ranked-ness

You can have these two:

```rust
struct St1(fn(u32));
struct St2(for<'a> fn(&'a u32));
```

But you can't make a single generic type subsuming both whilst hiding `fn`. Even if you expose `fn`, you still can't make a single generic method covering both case whilst utilizing `fn`.

```rust,compile_fail,E0592
struct StGeneric<T>(T);
impl<T> StGeneric<fn(T)> {
    fn call(&self, x: T) { (self.0)(x); }
}
impl<T> StGeneric<for<'a> fn(&'a T)> {
    // error[E0592]: duplicate definitions with name `call`
    fn call(&self, x: &T) { (self.0)(x); }
}
```


### `[tag:trait_constraints_on_associated_types_do_not_propagate]` Trait constraints on associated types do not propagate to the trait's use sites

*Upstream issue:* [rust-lang/rust#32722](https://github.com/rust-lang/rust/issues/32722)

According to [this comment](https://github.com/rust-lang/rust/issues/32722#issuecomment-618044689), this is a symptom of [rust-lang/rust#20671](https://github.com/rust-lang/rust/issues/20671).

```rust,compile_fail,E0277
trait KernelMutex {}

trait CfgBase {
    type System;
}

trait CfgMutex: CfgBase
where
    Self::System: KernelMutex,
{}

// error[E0277]: the trait bound `<C as CfgBase>::System: KernelMutex` is not satisfied
fn foo<C: CfgMutex>() {}
```


### `[tag:impl_trait_false_type_alias_bounds]` `type_alias_bounds` misfires when `impl Trait` is used in a portion of a type alias

*Upstream issue:* [rust-lang/rust#94395](https://github.com/rust-lang/rust/issues/94395)

```rust,compile_fail
#![feature(type_alias_impl_trait)]

// error: bounds on generic parameters are not enforced in type aliases
#[deny(type_alias_bounds)]
type Alias<T: Send> = (impl Send,);

pub fn foo<T: Send>(x: T) -> Alias<T> {
    (x,)
}
```

Removing `: Send` from the type alias as per the compiler's suggestion results in a hard error.

```rust,compile_fail,E0277
#![feature(type_alias_impl_trait)]

// error[E0277]: `T` cannot be sent between threads safely
type Alias<T> = (impl Send,);

pub fn foo<T: Send>(x: T) -> Alias<T> {
    (x,)
}
```


### `[tag:const_generic_parameter_false_type_alias_bounds]` `type_alias_bounds` misfires when the trait bound is used by a const generic parameter

*Upstream issue:* [rust-lang/rust#94398](https://github.com/rust-lang/rust/issues/94398)

```rust,compile_fail
#![feature(generic_const_exprs)]

trait Trait {
    const N: usize;
}

struct Struct<const N: usize>;

// error: bounds on generic parameters are not enforced in type aliases
#[deny(type_alias_bounds)]
type Alias<T: Trait> = Struct<{<T as Trait>::N}>;
```

Removing `: Trait` from the type alias as per the compiler's suggestion results in a hard error.

```rust,compile_fail,E0277
#![feature(generic_const_exprs)]

trait Trait {
    const N: usize;
}

struct Struct<const N: usize>;

// error[E0277]: the trait bound `T: Trait` is not satisfied
type Alias<T> = Struct<{<T as Trait>::N}>;
```


## Language features and `const fn`s

### `[tag:const_for]` `for` loops are unusable in `const fn`

Technically it's available under the compiler feature `const_for`, but the lack of necessary trait implementations (e.g., `[ref:range_const_iterator]`, `[ref:const_slice_iter]`) and the difficulty of implementing `const Iterator` (`[ref:iterator_const_default]`) make it mostly unusable.


### `[tag:const_static_item_ref]` `const`s and `const fn`s can't refer to `static`s

*Upstream issue:* [rust-lang/const-eval#11](https://github.com/rust-lang/const-eval/issues/11)


### `[tag:const_untyped_pointer]` “untyped pointers are not allowed in constant”

*Upstream issue:* [rust-lang/rust#90474](https://github.com/rust-lang/rust/issues/90474)

```rust,compile_fail
#![feature(core_intrinsics)]
#![feature(const_heap)]
use core::mem::MaybeUninit;
struct ClosureEnv(MaybeUninit<*mut ()>);
// error: untyped pointers are not allowed in constant
const A: MaybeUninit<*mut ()> = unsafe {
    MaybeUninit::new(core::intrinsics::const_allocate(4, 4) as _)
};
```


### `[tag:const_trait_not_implied]` `~const` in associated type bounds are not implied in the trait's use sites

Associated type bounds are usually implied in the trait's use sites (e.g., if an associated type `Trait::Proj` is bounded by `Clone`, `T: Trait` implies `T::Proj: Clone`). However, this doesn't seem to apply to `~const`.

```rust
trait Trait {
    type Proj: Clone;
}

fn clone_projection<T: Trait>(p: &T::Proj) -> T::Proj {
    p.clone()
}
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]

trait Trait {
    type Proj: ~const Clone;
}

const fn clone_projection<T: ~const Trait>(p: &T::Proj) -> T::Proj {
    // error[E0277]: the trait bound `<T as Trait>::Proj: ~const Clone` is not satisfied
    p.clone()
}
```


### `[tag:const_supertraits]` Supertraits can't have `~const`

*Upstream PR:* [rust-lang/rust#93429](https://github.com/rust-lang/rust/pull/93429) might resolve this

```rust
#![feature(const_trait_impl)]
trait Trait: Clone {}
```

```rust,compile_fail
#![feature(const_trait_impl)]
// error: `~const` is not allowed here
trait Trait: ~const Clone {}
```


### `[tag:impl_block_const_bounds]` The trait bounds of an `impl` block can't include `~const`

```rust,compile_fail
#![feature(const_trait_impl)]
struct Cfg<C>(C);
trait CfgBase {}
// error: `~const` is not allowed here
impl<C: ~const CfgBase> Cfg<C> {
    const fn num_task_priority_levels(&self, _: usize) {}
}
```

A work-around is to move the trait bounds to the `const fn`s inside.

```rust
#![feature(const_trait_impl)]
struct Cfg<C>(C);
trait CfgBase {}
impl<C> Cfg<C> {
    const fn num_task_priority_levels(&self, _: usize)
    where
        C: ~const CfgBase
    {}
}
```


### `[tag:const_closures]` Closures can't be `impl const Fn`

```rust
#![feature(const_trait_impl)]
const fn identity<C: ~const Fn()>(x: C) -> C { x }
const fn foo() {}
identity(foo);
identity(|| {});
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
const fn identity<C: ~const Fn()>(x: C) -> C { x }
// error[E0277]: the trait bound `[closure@lib.rs:6:26: 6:31]: ~const Fn<()>` is not satisfied
const _: () = { identity(|| {}); };
```

### `[tag:false_unconstrained_generic_const_on_type_alias]` An unrelated generic parameter causes "unconstrained generic constant" when using a type alias including a generic constant

*Upstream issue:* [rust-lang/rust#89421](https://github.com/rust-lang/rust/issues/89421) (possibly related)

```rust
#![feature(generic_const_exprs)]
type Alias<const N: usize> = [(); {N + 1}];
const N: usize = 1;
fn foo() {
    let _: Alias<N> = [(); 2];
}
```

```rust,compile_fail
#![feature(generic_const_exprs)]
type Alias<const N: usize> = [(); {N + 1}];
const N: usize = 1;
fn foo<T>() {
    // error: unconstrained generic constant
    let _: Alias<N> = [(); 2];
}
```


## `const fn`s and `const` trait implementations

### `[tag:const_array_from_fn]` `core::array::from_fn` is not `const fn`

```rust
#![feature(const_trait_impl)]
#![feature(array_from_fn)]
let _: [(); 1] = core::array::from_fn(unit);
fn unit(_: usize) {}
```

```rust,compile_fail,E0015
#![feature(const_trait_impl)]
#![feature(array_from_fn)]
// error[E0015]: cannot call non-const fn `std::array::from_fn::<fn(usize)
// {unit}, (), 1_usize>` in constants
const _: [(); 1] = core::array::from_fn(unit);
const fn unit(_: usize) {}
```


### `[tag:const_array_map]` `<[T; _]>::map` is not `const fn`

```rust
let _: [(); 3] = [0usize, 1, 2].map(f);
fn f(_: usize) {}
```

```rust,compile_fail,E0015
#![feature(const_trait_impl)]
// error[E0015]: cannot call non-const fn `array::<impl [usize; 3]>::map::<
// fn(usize) {f}, ()>` in constants
const _: [(); 3] = [0usize, 1, 2].map(f);
const fn f(_: usize) {}
```


### `[tag:const_slice_sort_unstable]` `<[T]>::sort_unstable*` is not `const fn`

```rust
use core::cmp::Ordering;
const fn comparer(_: &i32, _: &i32) -> Ordering { Ordering::Equal }
[1].sort_unstable_by(comparer);
```

```rust,compile_fail,E0015
#![feature(const_mut_refs)]
use core::cmp::Ordering;
const fn comparer(_: &i32, _: &i32) -> Ordering { Ordering::Equal }
// error[E0015]: cannot call non-const fn `core::slice::<impl [i32]>::
// sort_unstable_by::<for<'r, 's> fn(&'r i32, &'s i32) -> std::cmp::Ordering
// {comparer}>` in constants
const _: () = [1].sort_unstable_by(comparer);
```


### `[tag:const_option_map]` `Option::map` is inconvenient to use in `const fn`

Although it's unstably `const fn` now ([rust-lang/rust#91930](https://github.com/rust-lang/rust/issues/91930)), `[ref:const_closures]` significantly limits its usability.


### `[tag:const_result_expect]` `Result::expect` is not `const fn`

```rust
Ok::<(), ()>(()).expect("");
```

```rust,compile_fail,E0015
// error[E0015]: cannot call non-const fn `Result::<(), ()>::expect` in constants
const _: () = Ok::<(), ()>(()).expect("");
```


### `[tag:const_result_map]` `Result::map[_err]` is not `const fn`

```rust
const fn identity<T>(x: T) -> T { x }
Ok::<(), ()>(()).map(identity);
```

```rust,compile_fail,E0015
const fn identity<T>(x: T) -> T { x }
// error[E0015]: cannot call non-const fn `Result::<(), ()>::map::<(), fn(())
// {_doctest_main_lib_rs_452_0::identity::<()>}>` in constants
const _: () = { Ok::<(), ()>(()).map(identity); };
```


### `[tag:const_slice_iter]` `<[T]>::iter` is not `const fn`

```rust
b"".iter();
```

```rust,compile_fail,E0015
// error[E0015]: cannot call non-const fn `core::slice::<impl [u8]>::iter` in
// constants
const _: () = { b"".iter(); };
```


### `[tag:const_uninit_array]` `MaybeUninit::uninit_array` is unstable

```rust,compile_fail,E0658
use core::mem::MaybeUninit;
// error[E0658]: use of unstable library feature 'maybe_uninit_uninit_array'
const _: [MaybeUninit<u32>; 4] = MaybeUninit::uninit_array();
```


### `[tag:derive_const_partial_eq]` `derive(PartialEq)` doesn't derive `~const PartialEq`

```rust
#[derive(PartialEq)]
struct A;
assert!(A == A);
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
#[derive(PartialEq)]
struct A;
// error[E0277]: can't compare `A` with `A` in const contexts
const _: () = assert!(A == A);
```


### `[tag:array_const_partial_eq]` `[T; _]: !~const PartialEq`

The standard library doesn't provide a `const` trait implementation of `PartialEq` for `[T; _]`.

```rust
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
assert!(PartialEq::eq(&[A, A], &[A, A]));
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
// error[E0277]: can't compare `[A; 2]` with `[A; 2]` in const contexts
const _: () = assert!(PartialEq::eq(&[A, A], &[A, A]));
```



### `[tag:slice_const_partial_eq]` `[T]: !~const PartialEq`

The standard library doesn't provide a `const` trait implementation of `PartialEq` for `[T]`.

```rust
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
const SLICE: &[A] = &[];
assert!(PartialEq::eq(SLICE, SLICE));
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
const SLICE: &[A] = &[];
// error[E0277]: can't compare `[A]` with `[A]` in const contexts
const _: () = assert!(PartialEq::eq(SLICE, SLICE));
```


### `[tag:option_const_partial_eq]` `Option<T>: !~const PartialEq`

The standard library doesn't provide a `const` trait implementation of `PartialEq` for `Option<T>`.

```rust
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
assert!(PartialEq::eq(&Some(A), &Some(A)));
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
// error[E0277]: can't compare `Option<A>` with `Option<A>` in const contexts
const _: () = assert!(PartialEq::eq(&Some(A), &Some(A)));
```


### `[tag:range_const_partial_eq]` `Range<T>: !~const PartialEq`

The standard library doesn't provide a `const` trait implementation of `PartialEq` for `Range<T>`.

```rust
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
assert!(PartialEq::eq(&(A..A), &(A..A)));
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
// error[E0277]: can't compare `std::ops::Range<A>` with `std::ops::Range<A>` in
// const contexts
const _: () = assert!(PartialEq::eq(&(A..A), &(A..A)));
```


### `[tag:type_id_partial_eq]` `TypeId: !const PartialEq`

The standard library doesn't provide a `const` trait implementation of `PartialEq` for `core::any::TypeId`.

```rust
use core::any::TypeId;
assert!(TypeId::of::<()>() == TypeId::of::<()>());
```

```rust,compile_fail,E0015
#![feature(const_type_id)]
use core::any::TypeId;
// error[E0015]: cannot call non-const operator in constants
const _: () = assert!(TypeId::of::<()>() == TypeId::of::<()>());
```


### `[tag:range_const_iterator]` `Range<T>: !~const Iterator`

The standard library doesn't provide a `const` trait implementation of `Range<T>: Iterator`.

```rust
assert!(matches!((2..4).next(), Some(2)));
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
// `assert!` is used here due to [ref:const_assert_eq]
// `matches!` is used here due to [ref:option_const_partial_eq]
// error[E0277]: the trait bound `std::ops::Range<i32>: ~const Iterator` is not
// satisfied
const _: () = assert!(matches!((2..4).next(), Some(2)));
```


### `[tag:iterator_const_default]` `Iterator` lack `#[const_trait]`

Implementing `const Iterator` requires you to implement all of its methods, which is impossible to do correctly.

```rust,compile_fail
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]

struct MyIterator;

// error: const trait implementations may not use non-const default functions
// note: `size_hint`, `count`, `last`, `advance_by`, `nth`, `step_by`, `chain`,
// `zip`, `intersperse`, `intersperse_with`, `map`, `for_each`, `filter`,
// `filter_map`, `enumerate`, `peekable`, `skip_while`, `take_while`,
// `map_while`, `skip`, `take`, `scan`, `flat_map`, `flatten`, `fuse`,
// `inspect`, `by_ref`, `collect`, `try_collect`, `partition`,
// `partition_in_place`, `is_partitioned`, `try_fold`, `try_for_each`, `fold`,
// `reduce`, `try_reduce`, `all`, `any`, `find`, `find_map`, `try_find`,
// `position`, `rposition`, `max`, `min`, `max_by_key`, `max_by`, `min_by_key`,
// `min_by`, `rev`, `unzip`, `copied`, `cloned`, `cycle`, `sum`, `product`,
// `cmp`, `cmp_by`, `partial_cmp`, `partial_cmp_by`, `eq`, `eq_by`, `ne`, `lt`,
// `le`, `gt`, `ge`, `is_sorted`, `is_sorted_by`, `is_sorted_by_key`,
// `__iterator_get_unchecked` not implemented
impl const Iterator for MyIterator {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        Some(42)
    }
}
```


### `[tag:int_const_ord]` `<integer>: !const Ord`

The standard library doesn't provide `const` trait implementations of `Ord` for the built-in integer types.

```rust
assert!(2i32.max(3) == 3);
```

```rust,compile_fail,E0277
#![feature(const_trait_impl)]
// error[E0277]: the trait bound `i32: ~const Ord` is not satisfied
const _: () = assert!(2i32.max(3) == 3);
```


### `[tag:const_assert_eq]` `assert_eq!` and similar macros are unusable in `const fn`

```rust,compile_fail,E0015
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
struct A;
impl const PartialEq for A {
    fn eq(&self, _: &Self) -> bool { true }
    fn ne(&self, _: &Self) -> bool { false }
}
impl const core::fmt::Debug for A {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unimplemented!()
    }
}

// error[E0015]: cannot call non-const fn `assert_failed::<A, A>` in constants
const _: () = assert_eq!(A, A);
```


### `[tag:cell_const]` `Cell` is unusable in `const fn`

```rust
core::cell::Cell::new(0).set(42);
```

```rust,compile_fail,E0015
// error[E0015]: cannot call non-const fn `Cell::<i32>::set` in constants
const _: () = core::cell::Cell::new(0).set(42);
```


### `[tag:ref_cell_const]` `RefCell` is unusable in `const fn`

```rust
core::cell::RefCell::new(0).borrow();
```

```rust,compile_fail,E0015
// error[E0015]: cannot call non-const fn `RefCell::<i32>::borrow` in constants
// error[E0493]: destructors cannot be evaluated at compile-time
const _: () = { core::cell::RefCell::new(0).borrow(); };
```


### `[tag:tokenlock_const]` [`tokenlock`][4] doesn't support locking in `const fn`

```rust
tokenlock::with_branded_token(|token| {
    let tl = tokenlock::BrandedTokenLock::wrap(42);
    assert!(*tl.read(&token) == 42);
});
```

```rust,compile_fail,E0015
#![feature(const_trait_impl)]
// error[E0015]: cannot call non-const fn `with_branded_token::<(),
// [closure@lib.rs:3:45: 6:2]>` in constants
const _: () = tokenlock::with_branded_token(|token| {
    let tl = tokenlock::BrandedTokenLock::wrap(42);
    assert!(*tl.read(&token) == 42);
});
```


## Unsized types

### `[tag:unsized_maybe_uninit]` `MaybeUninit<T>` requires `T: Sized`

*Upstream issue:* [rust-lang/rust#80158](https://github.com/rust-lang/rust/issues/80158)

```rust,compile_fail,E0277
// error[E0277]: the size for values of type `[u8]` cannot be known at
// compilation time
fn foo(_: &core::mem::MaybeUninit<[u8]>) {}
```


## Interior mutability

### `[tag:missing_interior_mutability_trait]` Missing trait for representing the lack of interior mutability

*Upstream RFC:* [rust-lang/rfcs#2944](https://github.com/rust-lang/rfcs/pull/2944) (closed)
\

## Existential types

### `[tag:opaque_type_extraneous_capture]` An opaque type captures unused generic type parameters

It may be possible that it's an intended behavior.

```rust
#![feature(type_alias_impl_trait)]
trait Trait {
    type Projection: 'static + Send;
    fn get(self) -> Self::Projection;
}
type Projection<U: 'static + Send> = impl 'static + Send;
impl<T, U: 'static + Send> Trait for (T, U) {
    type Projection = Projection<U>;
    fn get(self) -> Self::Projection { self.1 }
}
```

```rust,compile_fail,E0310
#![feature(type_alias_impl_trait)]
trait Trait {
    type Projection: 'static + Send;
    fn get(self) -> Self::Projection;
}
impl<T, U: 'static + Send> Trait for (T, U) {
    // error[E0310]: the parameter type `T` may not live long enough
    type Projection = impl 'static + Send;
    fn get(self) -> Self::Projection { self.1 }
}
```


## Macros

### `[tag:decl_macro_unused]` The `unused_macros` lint misfires when a private macro 2.0 is used in a public macro 2.0

*Upstream issue:* [rust-lang/rust#80940](https://github.com/rust-lang/rust/issues/80940)

```rust,compile_fail
#![feature(decl_macro)]

#[deny(unused_macros)] // error: unused macro definition: `inner`
macro inner() {}

pub macro public() {
    $crate::inner!()
}
```


## rustdoc

### `[tag:downstream_intra_doc_link]` Intra-doc links can't refer to downstream crates

*Upstream issue:* [rust-lang/rust#74481](https://github.com/rust-lang/rust/issues/74481)


### `[tag:rustdoc_images]` There's no supported way to include images from relative paths in a rustdoc output

*Upstream issue:* [rust-lang/rust#32104](https://github.com/rust-lang/rust/issues/32104)

A doc comment can include an image tag with a relative path, but this won't render correctly because rustdoc doesn't copy the referenced image file to the appropriate directory.


## Miscellaneous

### `[tag:unnamed_const_dead_code]` `dead_code` misfires when a `fn` is only used for compile-time assertions

*Upstream issue:* [rust-lang/rust#89717](https://github.com/rust-lang/rust/issues/89717)

```rust,compile_fail
#![deny(dead_code)]
const fn foo() {}  // error: function is never used: `foo`
const _: () = foo();
```


### `[tag:method_repr_align]` `#[repr(align(_))]` is not supported on associated functions or methods

*Upstream issue:* [rust-lang/rust#82232 (comment)](https://github.com/rust-lang/rust/issues/82232#issuecomment-905929314)

```rust
#![feature(fn_align)]
#[repr(align(4))]
fn foo() {}
```

```rust,compile_fail,E0517
#![feature(fn_align)]
struct A;
impl A {
    // error[E0517]: attribute should be applied to a struct, enum, function, or union
    #[repr(align(4))]
    fn foo() {}
}
```


[1]: https://github.com/stepchowfun/tagref
[2]: https://doc.rust-lang.org/1.58.1/rustdoc/documentation-tests.html#documentation-tests
[3]: https://github.com/rust-lang/rust/issues
[4]: https://crates.io/crates/tokenlock
