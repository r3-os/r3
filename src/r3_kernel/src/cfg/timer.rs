use core::{mem::ManuallyDrop, num::NonZeroUsize};
use r3_core::{
    kernel::raw_cfg::{CfgTimer, TimerDescriptor},
    utils::Init,
};

use crate::{cfg::CfgBuilder, klock::CpuLockCell, timeout, timer, KernelTraits, Port};

unsafe impl<Traits: KernelTraits> const CfgTimer for CfgBuilder<Traits> {
    fn timer_define<Properties: ~const r3_core::bag::Bag>(
        &mut self,
        TimerDescriptor {
            phantom: _,
            param,
            period,
            delay,
            active,
            start,
        }: TimerDescriptor<Self::System>,
        _properties: Properties,
    ) -> timer::TimerId {
        let inner = &mut self.inner;

        let period = if let Some(period) = period {
            // FIXME: Work-around for `Result::expect` being not `const fn`
            if let Ok(x) = timeout::time32_from_duration(period) {
                x
            } else {
                panic!("`period` must not be negative");
            }
        } else {
            // Defaults to `None`
            timeout::BAD_DURATION32
        };

        let delay = if let Some(delay) = delay {
            // FIXME: Work-around for `Result::expect` being not `const fn`
            if let Ok(x) = timeout::time32_from_duration(delay) {
                x
            } else {
                panic!("`delay` must not be negative");
            }
        } else {
            // Defaults to `None`
            timeout::BAD_DURATION32
        };

        inner.timers.push(CfgBuilderTimer {
            start,
            param,
            delay,
            period,
            active,
        });

        unsafe { NonZeroUsize::new_unchecked(inner.timers.len()) }
    }
}

#[doc(hidden)]
pub struct CfgBuilderTimer {
    start: fn(usize),
    param: usize,
    delay: timeout::Time32,
    period: timeout::Time32,
    active: bool,
}

impl Clone for CfgBuilderTimer {
    fn clone(&self) -> Self {
        Self {
            start: self.start,
            param: self.param,
            delay: self.delay,
            period: self.period,
            active: self.active,
        }
    }
}

impl Copy for CfgBuilderTimer {}

impl CfgBuilderTimer {
    /// `i` is an index into [`super::super::KernelCfg2::timer_cb_pool`].
    pub const fn to_state<Traits: KernelTraits>(
        &self,
        attr: &'static timer::TimerAttr<Traits>,
        i: usize,
    ) -> timer::TimerCb<Traits> {
        let timeout = timeout::Timeout::new(timer::timer_timeout_handler::<Traits>, i);

        let timeout = if self.delay == timeout::BAD_DURATION32 {
            timeout.with_at_raw(self.delay)
        } else {
            timeout.with_expiration_at(self.delay)
        };

        timer::TimerCb {
            attr,
            timeout: ManuallyDrop::new(timeout),
            period: CpuLockCell::new(self.period),
            active: CpuLockCell::new(false),
        }
    }

    pub const fn to_attr<Traits: Port>(&self) -> timer::TimerAttr<Traits> {
        timer::TimerAttr {
            entry_point: self.start,
            entry_param: self.param,
            init_active: self.active,
            _phantom: Init::INIT,
        }
    }
}
