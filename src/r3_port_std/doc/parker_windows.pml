// The Windows remote parker reimplemented in Promela for verification by SPIN
// model checker (http://spinroot.com/spin/whatispin.html)
//
// Usage:
//
//     $ spin -a parker_windows_test1.pml && gcc -o pan -O3 pan.c && ./pan
//
//     If the verification fails, analyze the generated trail file by:
//     $ spin -t -r -s -l -g -c parker_windows_test1.pml
//

// -------------------------------------------------------------------------
// Atomic operations

inline fetch_add(var, operand, out_old_value) {
    d_step { out_old_value = var; var = var + operand; }
}

inline fetch_sub(var, operand, out_old_value) {
    d_step { out_old_value = var; var = var - operand; }
}

// -------------------------------------------------------------------------
// Win32

/// <https://docs.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitonaddress>
///
/// Should only be called by the main thread.
inline wait_on_address(var, futex, old_value) {
    int start_futex;
    d_step {
        if
        ::  (old_value == var) ->
            start_futex = futex
        ::  else ->
            start_futex = futex - 1   // don't wait
        fi
    }

    do
    ::  (futex > start_futex) -> break
    ::  suspension_point()
    od
}

/// <https://docs.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-wakebyaddressall>
inline wake_by_address_all(futex) {
    d_step { futex = futex + 1 }
}

int suspension_count = 0;
chan suspension_response = [0] of { bit }
bool has_thread_exited = false;

/// Suspend the main thread synchronously
/// <https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-suspendthread>
/// plus <https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getthreadcontext>
inline suspend_thread_sync() {
    // SuspendThread
    d_step {
        suspension_count = suspension_count + 1
    }

    // GetThreadContext
    do
    ::  suspension_response!0 -> break
    ::  has_thread_exited -> break
    od

    (true) // work around "jump into d_step sequence"
}

/// Resume the main thread synchronously
/// <https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-resumethread>
inline resume_thread() {
    d_step {
        if
        ::  (suspension_count > 0) ->
            suspension_count = suspension_count - 1
        :: else -> skip
        fi
    }
}

/// Give the schedule a chance to act. Should only be called by the main thread.
inline suspension_point() {
    bit _unused;
    do
    ::  (suspension_count == 0) -> break
    ::  suspension_response?_unused
    od

    (true) // work around "jump into d_step sequence"
}

/// Notify the scheduler that the main thread has exited. Obviously, should only
/// be called by the main thread.
inline thread_exited() {
    has_thread_exited = true;
}

// -------------------------------------------------------------------------
// The parker

int token_count = 0;
int token_count_futex = 0;
bool locked = false;

int conservative_token_count = 0;

inline unpark() {
    atomic { (!locked) -> locked = true }

    d_step { conservative_token_count = conservative_token_count + 1 }

    int old_token_count;
    fetch_add(token_count, 1, old_token_count);

    if
    ::  (old_token_count == -1) ->
        wake_by_address_all(token_count_futex)
        resume_thread()
    ::  else -> skip
    fi

    locked = false
}

inline local_park() {
    suspension_point()
    d_step { token_count = token_count - 1 }
    d_step { conservative_token_count = conservative_token_count - 1 }
    suspension_point()
    int remembered_token_count;
    do
    ::  remembered_token_count = token_count
        suspension_point()
        if
        ::  (remembered_token_count >= 0) -> break
        ::  else ->
            suspension_point()
            wait_on_address(token_count, token_count_futex, remembered_token_count)
        fi
    od
    suspension_point()
}

inline remote_park() {
    atomic { (!locked) -> locked = true }

    int old_token_count;
    fetch_sub(token_count, 1, old_token_count);

    if
    ::  (old_token_count == 0) ->
        suspend_thread_sync()
    ::  else -> skip
    fi

    d_step { conservative_token_count = conservative_token_count - 1 }

    locked = false
}
