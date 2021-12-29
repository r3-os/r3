#include "parker_windows.pml"

// -------------------------------------------------------------------------
// Test bench

bool exit = false;
int counter = 0;

active proctype main_thread() {
    do
    ::  (exit || counter > 8) -> break
    ::  else ->
        counter = counter + 1
        suspension_point()
    od

    thread_exited()
}

active proctype parker() {
    remote_park()

    // The main thread is suspended, so the counter shouldn't be updated
    // anymore
    // <https://devblogs.microsoft.com/oldnewthing/20150205-00/?p=44743>
    int counter1 = counter;
    int counter2 = counter;
    assert(counter1 == counter2);

    unpark()
    exit = true
}
