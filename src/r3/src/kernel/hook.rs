//! Hooks
use super::{raw, raw_cfg, Cfg};
use crate::utils::{ComptimeVec, Init, PhantomInvariant};

// TODO: Other types of hooks

/// Represents a registered startup hook in a system.
///
/// There are no operations defined for startup hooks, so this type
/// is only used for static configuration.
///
/// Startup hooks execute during the boot process with [CPU Lock] active, after
/// initializing kernel structures and before scheduling the first task.
///
/// [CPU Lock]: crate#system-states
///
/// <div class="admonition-follows"></div>
///
/// > **Relation to Other Specifications:** `StartupHook` (AUTOSAR OS,
/// > OSEK/VDX), last function (TI-RTOS), initialization routine (μITRON4.0).
///
#[doc = include_str!("../common.md")]
pub struct StartupHook<System: raw::KernelBase>(PhantomInvariant<System>);

impl<System: raw::KernelBase> StartupHook<System> {
    /// Construct a `StartupHookDefiner` to register a startup hook in
    /// [a configuration function](crate#static-configuration).
    pub const fn define() -> StartupHookDefiner<System> {
        StartupHookDefiner::new()
    }

    const fn new() -> Self {
        Self(Init::INIT)
    }
}

/// The definer (static builder) for [`StartupHook`].
#[must_use = "must call `finish()` to complete registration"]
pub struct StartupHookDefiner<System> {
    _phantom: PhantomInvariant<System>,
    start: Option<fn(usize)>,
    param: usize,
    priority: i32,
    unchecked: bool,
}

impl<System: raw::KernelBase> StartupHookDefiner<System> {
    const fn new() -> Self {
        Self {
            _phantom: Init::INIT,
            start: None,
            param: 0,
            priority: 0,
            unchecked: false,
        }
    }

    /// \[**Required**\] Specify the entry point.
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
    pub const fn finish<C: ~const raw_cfg::CfgBase<System = System>>(
        self,
        cfg: &mut Cfg<C>,
    ) -> StartupHook<System> {
        if self.priority < 0 && !self.unchecked {
            panic!("negative priority is unsafe and should be unlocked by `unchecked`");
        }

        let startup_hooks = &mut cfg.startup_hooks;
        let order = startup_hooks.len();
        startup_hooks.push(CfgStartupHook {
            start: self.start.expect("`start` is not specified"),
            param: self.param,
            priority: self.priority,
            order,
        });

        StartupHook::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CfgStartupHook {
    start: fn(usize),
    param: usize,
    priority: i32,
    /// The registration order.
    order: usize,
}

/// Sort startup hooks by (priority, order).
pub(crate) const fn sort_hooks(startup_hooks: &mut ComptimeVec<CfgStartupHook>) {
    sort_unstable_by!(
        startup_hooks.len(),
        |i| startup_hooks.get_mut(i),
        |x, y| if x.priority != y.priority {
            x.priority < y.priority
        } else {
            x.order < y.order
        }
    );
}

/// A startup hook.
///
/// This type isn't technically public but needs to be `pub` so that it can be
/// referred to by [`KernelStatic`].
///
/// [`KernelStatic`]: crate::kernel::cfg::KernelStatic
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct StartupHookAttr {
    pub(super) start: fn(usize),
    pub(super) param: usize,
}

impl Init for StartupHookAttr {
    const INIT: Self = Self {
        start: |_| {},
        param: 0,
    };
}

impl CfgStartupHook {
    pub const fn to_attr(&self) -> StartupHookAttr {
        StartupHookAttr {
            start: self.start,
            param: self.param,
        }
    }
}
