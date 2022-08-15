/// Call the specified closure, aborting if it unwinds.
#[inline]
pub fn abort_on_unwind<R>(f: impl FnOnce() -> R) -> R {
    /// The RAII guard type to cause a double panic on unwind.
    struct Guard;

    impl Drop for Guard {
        #[inline]
        fn drop(&mut self) {
            // Note: Panic strategies may be added in the future, so don't
            // change this to `cfg(panic = "unwind")`
            #[cfg(not(panic = "abort"))]
            panic!("unsafe to unwind in the current state");
        }
    }

    let guard = Guard;

    // Call the specified closure
    let ret = f();

    // Nullify the RAII guard if the call didn't cause unwinding
    core::mem::forget(guard);

    ret
}
