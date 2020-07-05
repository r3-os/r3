pub unsafe fn exit_thread() -> ! {
    unsafe {
        winapi::um::processthreadsapi::ExitThread(0);
    }
}

pub use std::thread::{park, spawn, JoinHandle, Thread, ThreadId};
