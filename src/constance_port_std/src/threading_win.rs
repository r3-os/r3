pub unsafe fn exit_thread() -> ! {
    unsafe {
        winapi::um::processthreadsapi::ExitThread(0);
    }
}
