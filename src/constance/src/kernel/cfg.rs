//! Static configuration mechanism for the kernel
use core::marker::PhantomData;

use super::{task, Port};
use crate::utils::Init;

mod vec;
#[doc(hidden)]
pub use self::vec::ComptimeVec;

/// Define a configure function.
#[macro_export]
macro_rules! configure {
    (
        $( #[$meta:meta] )*
        $vis:vis fn $ident:ident($ctx:ident: CfgBuilder<$sys:ty>) -> $id_map:ty {
            $($tt:tt)*
        }
    ) => {
        $( #[$meta] )*
        $vis const fn $ident(
            cfg: $crate::kernel::CfgBuilder<$sys>
        ) -> ($crate::kernel::CfgBuilder<$sys>, $id_map) {
            #[allow(unused_mut)]
            let mut $ctx = cfg;

            // `$ctx` will be updated by static configuration API macros
            // (such as `create_task!`) in this way: `$ctx = $ctx.op(...);`
            //
            // FIXME: `&mut` in `const fn` <https://github.com/rust-lang/rust/issues/57349>
            //        is not implemented yet

            let id_map = {
                $($tt)*
            };

            ($ctx, id_map)
        }
    };
}

/// Create a task. Should be used inside [`configure!`].
#[macro_export]
macro_rules! create_task {
    ($ctx:expr) => {{
        $ctx.tasks = $ctx.tasks.push($crate::kernel::CfgBuilderTask {});
        unsafe {
            $crate::kernel::Task::new(::core::num::NonZeroUsize::new_unchecked($ctx.tasks.len()))
        }
    }};
}

/// Attach a configuration function (defined by [`configure!`]) to a "system"
/// type.
#[macro_export]
macro_rules! build {
    ($sys:ty, $configure:expr) => {{
        // `$configure` produces two values: a `CfgBuilder` and an ID map
        // (custom type). We need the first one to be `const` so that we can
        // calculate the values of generic parameters based on its contents.
        const CFG: $crate::kernel::CfgBuilder<$sys> = {
            let mut cfg = $crate::kernel::CfgBuilder::new();
            $configure(cfg).0
        };

        // The second value can be just `let`
        let id_map = {
            let mut cfg = $crate::kernel::CfgBuilder::new();
            $configure(cfg).1
        };

        $crate::array_item_from_fn! {
            const TASK_STATE:
                [$crate::kernel::TaskState<<$sys as $crate::kernel::Port>::PortTaskState>; _] =
                    (0..CFG.tasks.len()).map(|i| CFG.tasks.get(i).to_state());
            const TASK_ATTR: [$crate::kernel::TaskAttr; _] =
                (0..CFG.tasks.len()).map(|i| CFG.tasks.get(i).to_attr());
        }

        // Safety: We are `build!`, so it's okay to `impl` this
        unsafe impl $crate::kernel::KernelCfg for $sys {
            const TASK_STATE: &'static [$crate::kernel::TaskState<
                <$sys as $crate::kernel::Port>::PortTaskState,
            >] = &TASK_STATE;
            const TASK_ATTR: &'static [$crate::kernel::TaskAttr] = &TASK_ATTR;
        }

        id_map
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! array_item_from_fn {
    ($(
        $static_or_const:tt $out:ident: [$ty:ty; _] = (0..$len:expr).map(|$var:ident| $map:expr);
    )*) => {$(
        $static_or_const $out: [$ty; { $len }] = {
            let mut values = [$crate::prelude::Init::INIT; { $len }];
            let mut i = 0;
            while i < values.len() {
                values[i] = {
                    let $var = i;
                    $map
                };
                i += 1;
            }
            values
        };
    )*};
}

// The "real" public interface ends here
// ---------------------------------------------------------------------------

#[doc(hidden)]
pub struct CfgBuilder<System> {
    _phantom: PhantomData<System>,
    pub tasks: ComptimeVec<CfgBuilderTask>,
}

impl<System> CfgBuilder<System> {
    pub const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            tasks: ComptimeVec::new(),
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CfgBuilderTask {}

impl CfgBuilderTask {
    pub const fn to_state<PortTaskState: Init>(&self) -> task::TaskState<PortTaskState> {
        task::TaskState {
            port_task_state: PortTaskState::INIT,
        }
    }

    pub const fn to_attr(&self) -> task::TaskAttr {
        task::TaskAttr {}
    }
}

/// Associates "system" types with kernel-private data. Use [`build!`] to
/// implement.
///
/// # Safety
///
/// This is only intended to be implemented by `build!`.
pub unsafe trait KernelCfg: Port {
    #[doc(hidden)]
    const TASK_STATE: &'static [task::TaskState<Self::PortTaskState>];

    #[doc(hidden)]
    const TASK_ATTR: &'static [task::TaskAttr];
}
