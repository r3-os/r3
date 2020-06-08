#[cfg(windows)]
pub unsafe fn exit_thread() -> ! {
    unsafe {
        winapi::um::processthreadsapi::ExitThread(0);
    }
}

#[cfg(unix)]
pub unsafe fn exit_thread() -> ! {
    unsafe {
        libc::pthread_exit(std::ptr::null_mut());
    }
}
