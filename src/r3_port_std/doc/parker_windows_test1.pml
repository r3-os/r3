#include "parker_windows.pml"

// -------------------------------------------------------------------------
// Test bench

inline assert_parking_invariant() {
    assert(suspension_count != 0 || conservative_token_count >= 0)
}

active proctype main_thread() {
    assert_parking_invariant()
    local_park()
    assert_parking_invariant()
    local_park()
    assert_parking_invariant()

    thread_exited()
}

active proctype parker() {
    remote_park()
    remote_park()
}

active proctype unparker() {
    unpark()
    unpark()
    unpark()
    unpark()
}