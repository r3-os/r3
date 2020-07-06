use std::sync::atomic;

pub trait HasAtomicEquivalent {
    type AtomicEquivalent;
}

impl HasAtomicEquivalent for u8 {
    type AtomicEquivalent = atomic::AtomicU8;
}
impl HasAtomicEquivalent for u16 {
    type AtomicEquivalent = atomic::AtomicU16;
}
impl HasAtomicEquivalent for u32 {
    type AtomicEquivalent = atomic::AtomicU32;
}
impl HasAtomicEquivalent for u64 {
    type AtomicEquivalent = atomic::AtomicU64;
}
impl HasAtomicEquivalent for usize {
    type AtomicEquivalent = atomic::AtomicUsize;
}
impl HasAtomicEquivalent for i8 {
    type AtomicEquivalent = atomic::AtomicI8;
}
impl HasAtomicEquivalent for i16 {
    type AtomicEquivalent = atomic::AtomicI16;
}
impl HasAtomicEquivalent for i32 {
    type AtomicEquivalent = atomic::AtomicI32;
}
impl HasAtomicEquivalent for i64 {
    type AtomicEquivalent = atomic::AtomicI64;
}
impl HasAtomicEquivalent for isize {
    type AtomicEquivalent = atomic::AtomicIsize;
}

pub type Atomic<T> = <T as HasAtomicEquivalent>::AtomicEquivalent;
