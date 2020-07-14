//! Checks that when a stack is automatically allocated, both ends of the
//! stack region are aligned to a port-specific alignment.
use constance::{kernel::Task, prelude::*};
use constance_test_suite::kernel_tests::Driver;
use core::marker::PhantomData;

pub struct App<System> {
    _phantom: PhantomData<System>,
}

impl<System: Kernel> App<System> {
    constance::configure! {
        pub const fn new<D: Driver<Self>>(_: &mut CfgBuilder<System>) -> Self {
            new! {
                Task<_>,
                start = task_body::<System, D>, priority = 0, active = true,
                stack_size = 4095,
            };

            App {
                _phantom: PhantomData,
            }
        }
    }
}

fn task_body<System: Kernel, D: Driver<App<System>>>(_: usize) {
    let expected_alignment = System::STACK_ALIGN;
    for task_cb in System::task_cb_pool() {
        let stack = task_cb.attr.stack.as_ptr();
        let start = stack as *mut u8;
        let end = start.wrapping_add(stack.len());
        log::trace!("stack = {:?}..{:?}", start, end);

        assert_eq!(start as usize % expected_alignment, 0);
        assert_eq!(end as usize % expected_alignment, 0);
    }
    D::success();
}
