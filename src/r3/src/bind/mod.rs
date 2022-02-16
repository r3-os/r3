#![doc = include_str!("../common.md")]
#![doc = r3_core::bind::__internal_module_doc!("r3_core", r#"
<div class="admonition-follows"></div>

> This module re-exports stable items from [`r3_core::bind`][] as well as
> providing some additional items.
"#)]
//!
//! # Examples
//!
//! TODO

pub use r3_core::bind::{
    Bind, BindBorrow, BindBorrowMut, BindDefiner, BindRef, BindTable, BindTake, BindTakeMut,
    BindTakeRef, Binder, ExecutableDefiner, ExecutableDefinerExt, FnBind, UnzipBind,
    INIT_HOOK_PRIORITY,
};
