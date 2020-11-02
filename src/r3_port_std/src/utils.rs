use std::sync::atomic;

pub mod iterpool;

pub trait HasAtomicEquivalent {
    type AtomicEquivalent;
}

#[cfg(target_has_atomic_load_store = "8")]
impl HasAtomicEquivalent for u8 {
    type AtomicEquivalent = atomic::AtomicU8;
}
#[cfg(target_has_atomic_load_store = "16")]
impl HasAtomicEquivalent for u16 {
    type AtomicEquivalent = atomic::AtomicU16;
}
#[cfg(target_has_atomic_load_store = "32")]
impl HasAtomicEquivalent for u32 {
    type AtomicEquivalent = atomic::AtomicU32;
}
#[cfg(target_has_atomic_load_store = "64")]
impl HasAtomicEquivalent for u64 {
    type AtomicEquivalent = atomic::AtomicU64;
}
#[cfg(target_has_atomic_load_store = "ptr")]
impl HasAtomicEquivalent for usize {
    type AtomicEquivalent = atomic::AtomicUsize;
}
#[cfg(target_has_atomic_load_store = "8")]
impl HasAtomicEquivalent for i8 {
    type AtomicEquivalent = atomic::AtomicI8;
}
#[cfg(target_has_atomic_load_store = "16")]
impl HasAtomicEquivalent for i16 {
    type AtomicEquivalent = atomic::AtomicI16;
}
#[cfg(target_has_atomic_load_store = "32")]
impl HasAtomicEquivalent for i32 {
    type AtomicEquivalent = atomic::AtomicI32;
}
#[cfg(target_has_atomic_load_store = "64")]
impl HasAtomicEquivalent for i64 {
    type AtomicEquivalent = atomic::AtomicI64;
}
#[cfg(target_has_atomic_load_store = "ptr")]
impl HasAtomicEquivalent for isize {
    type AtomicEquivalent = atomic::AtomicIsize;
}

pub type Atomic<T> = <T as HasAtomicEquivalent>::AtomicEquivalent;

pub trait LockConsuming {
    type LockGuard;

    fn lock(self) -> Self::LockGuard;
}

impl<'a, T> LockConsuming for &'a try_mutex::TryMutex<T> {
    type LockGuard = try_mutex::TryMutexGuard<'a, T>;

    #[inline]
    fn lock(self) -> Self::LockGuard {
        self.try_lock().unwrap()
    }
}
