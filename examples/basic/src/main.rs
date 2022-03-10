#![feature(const_fn_fn_ptr_basics)]
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![deny(unsafe_op_in_unsafe_fn)]
use r3::{
    kernel::{prelude::*, StaticTask},
    sync::StaticMutex,
};

type System = r3_kernel::System<SystemTraits>;
r3_port_std::use_port!(unsafe struct SystemTraits);

#[derive(Debug)]
struct Objects {
    _task1: StaticTask<System>,
    task2: StaticTask<System>,
    _mutex1: StaticMutex<System, u32>,
}

const COTTAGE: Objects = r3_kernel::build!(SystemTraits, configure_app => Objects);

const fn configure_app(b: &mut r3_kernel::Cfg<'_, SystemTraits>) -> Objects {
    b.num_task_priority_levels(4);

    let task1 = StaticTask::define()
        .start(task1_body)
        .priority(2)
        .active(true)
        .finish(b);
    let task2 = StaticTask::define().start(task2_body).priority(3).finish(b);

    let mutex1 = StaticMutex::define().finish(b);

    Objects {
        _task1: task1,
        task2,
        _mutex1: mutex1,
    }
}

fn task1_body() {
    log::trace!("COTTAGE = {:#?}", COTTAGE);
    log::trace!("KENREL = {:#?}", System::debug());

    COTTAGE.task2.activate().unwrap();
}

fn task2_body() {
    loop {
        dbg!(System::time().unwrap());
        System::sleep(r3::time::Duration::from_secs(1)).unwrap();
    }
}
