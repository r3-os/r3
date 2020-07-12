use core::marker::PhantomData;

use crate::{
    kernel::{cfg::CfgBuilder, startup, Port},
    utils::ComptimeVec,
};

impl<System: Port> startup::StartupHook<System> {
    /// Construct a `CfgStartupHookBuilder` to register a startup hook in
    /// [a configuration function](crate#static-configuration).
    pub const fn build() -> CfgStartupHookBuilder<System> {
        CfgStartupHookBuilder::new()
    }
}

/// Configuration builder type for [`StartupHook`].
///
/// [`StartupHook`]: crate::kernel::StartupHook
pub struct CfgStartupHookBuilder<System> {
    _phantom: PhantomData<System>,
    start: Option<fn(usize)>,
    param: usize,
    priority: i32,
    unchecked: bool,
}

impl<System: Port> CfgStartupHookBuilder<System> {
    const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            start: None,
            param: 0,
            priority: 0,
            unchecked: false,
        }
    }

    /// [**Required**] Specify the entry point.
    pub const fn start(self, start: fn(usize)) -> Self {
        Self {
            start: Some(start),
            ..self
        }
    }

    /// Specify the parameter to `start`. Defaults to `0`.
    pub const fn param(self, param: usize) -> Self {
        Self { param, ..self }
    }

    /// Specify the priority. Defaults to `0` when unspecified.
    ///
    /// Startup hooks will execute in the ascending order of priority.
    /// Startup hooks with identical priority values will execute in the
    /// registration order.
    ///
    /// `priority` must not be negative. This limitation can be relaxed by
    /// calling [`Self::unchecked`].
    pub const fn priority(self, priority: i32) -> Self {
        Self { priority, ..self }
    }

    /// Allow the use of a negative [priority value].
    ///
    /// [priority value]: Self::priority
    ///
    /// # Safety
    ///
    /// Startup hooks with negative priority values can rely on their execution
    /// order for memory safety.
    pub const unsafe fn unchecked(self) -> Self {
        Self {
            unchecked: true,
            ..self
        }
    }

    /// Complete the registration of a startup hook, returning an `StartupHook`
    /// object.
    pub const fn finish(self, cfg: &mut CfgBuilder<System>) -> startup::StartupHook<System> {
        let inner = &mut cfg.inner;

        if self.priority < 0 && !self.unchecked {
            panic!("negative priority is unsafe and should be unlocked by `unchecked`");
        }

        inner.startup_hooks.push(CfgBuilderStartupHook {
            start: if let Some(x) = self.start {
                x
            } else {
                panic!("`start` is not specified")
            },
            param: self.param,
            priority: self.priority,
        });

        startup::StartupHook::new()
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CfgBuilderStartupHook {
    start: fn(usize),
    param: usize,
    priority: i32,
}

/// Sort startup hooks by priority.
pub(super) const fn sort_hooks(startup_hooks: &mut ComptimeVec<CfgBuilderStartupHook>) {
    sort_by!(startup_hooks.len(), |i| startup_hooks.get_mut(i), |x, y| x
        .priority
        < y.priority);
}

impl CfgBuilderStartupHook {
    pub const fn to_attr(&self) -> startup::StartupHookAttr {
        startup::StartupHookAttr {
            start: self.start,
            param: self.param,
        }
    }
}
