#![feature(const_fn_trait_bound)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_mut_refs)]
#![feature(const_trait_impl)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(unsupported_naked_functions)]
use r3::{
    kernel::{prelude::*, traits, Cfg, Task},
    prelude::*,
    sync::Mutex,
};

type System = r3_kernel::System<SystemTraits>;
r3_port_std::use_port!(unsafe struct SystemTraits);

#[derive(Debug)]
struct Objects {
    task1: Task<System>,
    task2: Task<System>,
    mutex1: Mutex<System, u32>,
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app<C>(b: &mut Cfg<C>) -> Objects
where
    C: ~const traits::CfgBase<System = System> + ~const traits::CfgTask + ~const traits::CfgMutex,
{
    b.num_task_priority_levels(4);

    let task1 = Task::build()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = Task::build().start(task2_body).priority(3).finish(b);

    let mutex1 = Mutex::build().finish(b);

    Objects {
        task1,
        task2,
        mutex1,
    }
}

fn task1_body(_: usize) {
    log::trace!("COTTAGE = {:#?}", COTTAGE);
    log::trace!("KENREL = {:#?}", System::debug());

    COTTAGE.task2.activate().unwrap();
}

fn task2_body(_: usize) {
    loop {
        dbg!(System::time().unwrap());
        System::sleep(r3::time::Duration::from_secs(1)).unwrap();
    }
}
