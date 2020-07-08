//! High-performance non-thread safe object pool (with an optional iteration
//! functionality).
//!
//! It also provides a type akin to pointers so you can realize linked list
//! data structures on it within the "safe" Rust. Memory safety is guaranteed by
//! runtime checks.
//!
//! Initial allocations are guaranteed to return consecutive pointers:
//!
//! ```rust,ignore
//! use crate::utils::iterpool::{Pool, PoolPtr};
//! let mut pool = Pool::new();
//! assert_eq!(pool.allocate(42), PoolPtr::new(0));
//! assert_eq!(pool.allocate(42), PoolPtr::new(1));
//! assert_eq!(pool.allocate(42), PoolPtr::new(2));
//! ```
//!
//! Allocation Performance
//! ----------------------
//!
//! `Pool` outperformed Rust's default allocator (jemalloc) by at least twice
//! if each thread was given an exclusive access to an individual `Pool`.
//! It is expected that it will exhibit slightly better performance characteristics
//! on the real world use due to an improved spatial locality.
//!
//! It also comes with a sacrifice. It is impossible to return a free space to
//! the global heap without destroying entire the pool.
#![allow(dead_code)]
#![allow(clippy::trivially_copy_pass_by_ref)]
use std::{mem, num::NonZeroUsize, ops};

/// High-performance non-thread safe object pool.
#[derive(Debug, Clone)]
pub struct Pool<T> {
    storage: Vec<Entry<T>>,
    first_free: Option<PoolPtr>,
}

/// High-performance non-thread safe object pool with an ability to iterate
/// through allocated objects.
#[derive(Debug, Clone)]
pub struct IterablePool<T> {
    storage: Vec<ItEntry<T>>,
    first_free: Option<PoolPtr>,
    first_used: Option<PoolPtr>,
}

/// A (potentially invalid) pointer to an object in `Pool`, but without
/// information about which specific `Pool` this is associated with.
///
/// `Pool` uses zero-based indices, but when stored in `PoolPtr`, they are
/// one-based to meet the requirement of `NonZeroUsize`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct PoolPtr(pub NonZeroUsize);

#[derive(Debug, Clone)]
enum Entry<T> {
    Used(T),

    /// This entry is free. Points the next free entry.
    Free(Option<PoolPtr>),
}

#[derive(Debug, Clone)]
enum ItEntry<T> {
    /// This entry if occupied. Points the next and previous occupied entry
    /// (this forms a circular doubly-linked list).
    Used(T, (PoolPtr, PoolPtr)),

    /// This entry is free. Points the next free entry (forms a
    /// singly-linked list).
    Free(Option<PoolPtr>),
}

impl PoolPtr {
    /// Return an uninitialized pointer that has no guarantee regarding its
    /// usage with any `Pool`.
    ///
    /// This value can be used as a memory-efficient replacement for
    /// `Option<PoolPtr>` without a tag indicating whether it has a
    /// valid value or not.
    ///
    /// The returned pointer actually has a well-defined initialized value so
    /// using it will never result in an undefined behavior, hence this function
    /// is not marked with `unsafe`. It is just that it has no specific object
    /// or pool associated with it in a meaningful way.
    #[inline]
    pub fn uninitialized() -> Self {
        PoolPtr(NonZeroUsize::new(1).unwrap())
    }

    /// Construct a `PoolPtr` from a given `x: usize`.
    ///
    /// `x` must be less than `usize::MAX`.
    pub fn new(x: usize) -> Self {
        PoolPtr(NonZeroUsize::new(x.wrapping_add(1)).expect("count overflow"))
    }

    fn get(&self) -> usize {
        self.0.get() - 1
    }
}

impl<T> Entry<T> {
    fn as_ref(&self) -> Option<&T> {
        match self {
            Entry::Used(value) => Some(value),
            Entry::Free(_) => None,
        }
    }

    fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Entry::Used(value) => Some(value),
            Entry::Free(_) => None,
        }
    }

    fn next_free_index(&self) -> Option<PoolPtr> {
        match self {
            Entry::Used(_) => unreachable!(),
            Entry::Free(i) => *i,
        }
    }
}

impl<T> ItEntry<T> {
    fn as_ref(&self) -> Option<&T> {
        match self {
            ItEntry::Used(value, _) => Some(value),
            ItEntry::Free(_) => None,
        }
    }
    fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            ItEntry::Used(value, _) => Some(value),
            ItEntry::Free(_) => None,
        }
    }
    fn next_previous_used_index(&self) -> (PoolPtr, PoolPtr) {
        match self {
            ItEntry::Used(_, pn) => *pn,
            ItEntry::Free(_) => unreachable!(),
        }
    }
    fn next_previous_used_index_mut(&mut self) -> &mut (PoolPtr, PoolPtr) {
        match self {
            ItEntry::Used(_, pn) => pn,
            ItEntry::Free(_) => unreachable!(),
        }
    }
    fn next_free_index(&self) -> Option<PoolPtr> {
        match self {
            ItEntry::Used(_, _) => unreachable!(),
            ItEntry::Free(i) => *i,
        }
    }
}

impl<T> Pool<T> {
    pub const fn new() -> Self {
        Self {
            storage: Vec::new(),
            first_free: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut pool = Self {
            storage: Vec::with_capacity(capacity),
            first_free: None,
        };
        if capacity > 0 {
            for i in 0..capacity - 1 {
                pool.storage.push(Entry::Free(Some(PoolPtr::new(i + 1))));
            }
            pool.storage.push(Entry::Free(None));
            pool.first_free = Some(PoolPtr::new(0));
        }
        pool
    }

    pub fn clear(&mut self) {
        self.storage.clear();
        self.first_free = None;
    }

    pub fn reserve(&mut self, additional: usize) {
        if additional == 0 {
            return;
        }
        let existing_surplus = if self.first_free.is_some() {
            1 // at least one
        } else {
            0
        } + self.storage.capacity()
            - self.storage.len();
        if additional > existing_surplus {
            let needed_surplus =
                self.storage.capacity() - self.storage.len() + (additional - existing_surplus);
            self.storage.reserve(needed_surplus);
        }
    }

    pub fn allocate(&mut self, x: T) -> PoolPtr {
        match self.first_free {
            None => {
                self.storage.push(Entry::Used(x));
                PoolPtr::new(self.storage.len() - 1)
            }
            Some(i) => {
                let i = i.get();
                let next_free = self.storage[i].next_free_index();
                self.first_free = next_free;
                self.storage[i] = Entry::Used(x);
                PoolPtr::new(i)
            }
        }
    }

    pub fn deallocate<S: Into<PoolPtr>>(&mut self, i: S) -> Option<T> {
        let i = i.into();
        let e = &mut self.storage[i.get()];
        match e {
            Entry::Used(_) => {}
            Entry::Free(_) => {
                return None;
            }
        }
        let x = match mem::replace(e, Entry::Free(self.first_free)) {
            Entry::Used(x) => x,
            Entry::Free(_) => unreachable!(),
        };
        self.first_free = Some(i);
        Some(x)
    }

    pub fn get(&self, fp: PoolPtr) -> Option<&T> {
        self.storage.get(fp.get()).and_then(Entry::as_ref)
    }

    pub fn get_mut(&mut self, fp: PoolPtr) -> Option<&mut T> {
        self.storage.get_mut(fp.get()).and_then(Entry::as_mut)
    }

    /// Iterate over objects. Unlike `IterablePool`, `Pool` can't skip free
    /// space, so this might be less efficient.
    pub fn iter(&self) -> impl Iterator<Item = &'_ T> + '_ {
        self.storage.iter().filter_map(|e| match e {
            Entry::Free(_) => None,
            Entry::Used(x) => Some(x),
        })
    }

    /// Iterate over objects, allowing mutation. Unlike `IterablePool`,
    /// `Pool` can't skip free space, so this might be less efficient.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &'_ mut T> + '_ {
        self.storage.iter_mut().filter_map(|e| match e {
            Entry::Free(_) => None,
            Entry::Used(x) => Some(x),
        })
    }

    /// Iterate over objects and their pointers. Unlike `IterablePool`, `Pool`
    /// can't skip free space, so this might be less efficient.
    pub fn ptr_iter(&self) -> impl Iterator<Item = (PoolPtr, &'_ T)> + '_ {
        self.storage
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Entry::Free(_) => None,
                Entry::Used(x) => Some((PoolPtr::new(i), x)),
            })
    }

    /// Iterate over objects, allowing mutation. Unlike `IterablePool`,
    /// `Pool` can't skip free space, so this might be less efficient.
    pub fn ptr_iter_mut(&mut self) -> impl Iterator<Item = (PoolPtr, &'_ mut T)> + '_ {
        self.storage
            .iter_mut()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Entry::Free(_) => None,
                Entry::Used(x) => Some((PoolPtr::new(i), x)),
            })
    }
}

impl<T> IterablePool<T> {
    pub const fn new() -> Self {
        Self {
            storage: Vec::new(),
            first_free: None,
            first_used: None,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut pool = Self {
            storage: Vec::with_capacity(capacity),
            first_free: None,
            first_used: None,
        };
        if capacity > 0 {
            for i in 0..capacity - 1 {
                pool.storage.push(ItEntry::Free(Some(PoolPtr::new(i + 1))));
            }
            pool.storage.push(ItEntry::Free(None));
            pool.first_free = Some(PoolPtr::new(0));
        }
        pool
    }

    pub fn clear(&mut self) {
        self.storage.clear();
        self.first_free = None;
        self.first_used = None;
    }

    pub fn reserve(&mut self, additional: usize) {
        if additional == 0 {
            return;
        }
        let existing_surplus = if self.first_free.is_some() {
            1 // at least one
        } else {
            0
        } + self.storage.capacity()
            - self.storage.len();
        if additional > existing_surplus {
            let needed_surplus =
                self.storage.capacity() - self.storage.len() + (additional - existing_surplus);
            self.storage.reserve(needed_surplus);
        }
    }

    pub fn allocate(&mut self, x: T) -> PoolPtr {
        use std::mem::replace;

        if self.first_free.is_none() {
            self.storage.push(ItEntry::Free(None));
            self.first_free = Some(PoolPtr::new(self.storage.len() - 1));
        }

        let i = self.first_free.unwrap();

        let next_prev = if let Some(first_used) = self.first_used {
            // Insert after the `self.first_used`
            let next = {
                let next_prev = self.storage[first_used.get()].next_previous_used_index_mut();
                replace(&mut next_prev.0, i)
            };
            self.storage[next.get()].next_previous_used_index_mut().1 = i;

            (next, first_used)
        } else {
            (i, i)
        };

        self.first_free = self.storage[i.get()].next_free_index();
        self.storage[i.get()] = ItEntry::Used(x, next_prev);
        self.first_used = Some(i);

        i
    }

    pub fn deallocate<S: Into<PoolPtr>>(&mut self, i: S) -> Option<T> {
        let i = i.into();
        let x = match mem::replace(&mut self.storage[i.get()], ItEntry::Free(self.first_free)) {
            ItEntry::Used(x, (next, prev)) => {
                if next == i {
                    assert_eq!(self.first_used, Some(i));
                    assert_eq!(next, prev);
                    self.first_used = None;
                } else {
                    if self.first_used == Some(i) {
                        self.first_used = Some(next);
                    }
                    self.storage[next.get()].next_previous_used_index_mut().1 = prev;
                    self.storage[prev.get()].next_previous_used_index_mut().0 = next;
                }
                x
            }
            ItEntry::Free(_) => unreachable!(),
        };
        self.first_free = Some(i);
        Some(x)
    }

    pub fn get(&self, fp: PoolPtr) -> Option<&T> {
        self.storage.get(fp.get()).and_then(ItEntry::as_ref)
    }

    pub fn get_mut(&mut self, fp: PoolPtr) -> Option<&mut T> {
        self.storage.get_mut(fp.get()).and_then(ItEntry::as_mut)
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter(self.ptr_iter())
    }

    pub fn ptr_iter(&self) -> PtrIter<'_, T> {
        PtrIter {
            remaining_len: self.storage.len(),
            pool: self,
            cur: self.first_used,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut(self.ptr_iter_mut())
    }

    pub fn ptr_iter_mut(&mut self) -> PtrIterMut<'_, T> {
        PtrIterMut {
            remaining_len: self.storage.len(),
            cur: self.first_used,
            pool: self,
        }
    }
}

impl<T> ops::Index<PoolPtr> for Pool<T> {
    type Output = T;

    fn index(&self, index: PoolPtr) -> &Self::Output {
        self.get(index).expect("dangling ptr")
    }
}

impl<T> ops::IndexMut<PoolPtr> for Pool<T> {
    fn index_mut(&mut self, index: PoolPtr) -> &mut Self::Output {
        self.get_mut(index).expect("dangling ptr")
    }
}

impl<T> ops::Index<PoolPtr> for IterablePool<T> {
    type Output = T;

    fn index(&self, index: PoolPtr) -> &Self::Output {
        self.get(index).expect("dangling ptr")
    }
}

impl<T> ops::IndexMut<PoolPtr> for IterablePool<T> {
    fn index_mut(&mut self, index: PoolPtr) -> &mut Self::Output {
        self.get_mut(index).expect("dangling ptr")
    }
}

/// An iterator over the elements of a `IterablePool`.
#[derive(Debug, Clone)]
pub struct Iter<'a, T>(PtrIter<'a, T>);

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|x| x.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// An iterator over the elements of a `IterablePool`.
#[derive(Debug, Clone)]
pub struct PtrIter<'a, T> {
    pool: &'a IterablePool<T>,
    cur: Option<PoolPtr>,
    remaining_len: usize,
}

impl<'a, T> Iterator for PtrIter<'a, T> {
    type Item = (PoolPtr, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.cur {
            let entry = &self.pool.storage[cur.get()];
            self.cur = Some(entry.next_previous_used_index().0);
            if self.cur == self.pool.first_used {
                // Reached the end
                self.cur = None;
            }
            self.remaining_len -= 1;
            Some((cur, entry.as_ref().unwrap()))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining_len))
    }
}

/// A mutable iterator over the elements of a `IterablePool`.
#[derive(Debug)]
pub struct IterMut<'a, T>(PtrIterMut<'a, T>);

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|x| x.1)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// A mutable iterator over the elements of a `IterablePool`.
#[derive(Debug)]
pub struct PtrIterMut<'a, T> {
    pool: &'a mut IterablePool<T>,
    cur: Option<PoolPtr>,
    remaining_len: usize,
}

impl<'a, T> Iterator for PtrIterMut<'a, T> {
    type Item = (PoolPtr, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        use std::mem::transmute;
        if let Some(cur) = self.cur {
            // extend the lifetime of the mutable reference
            let entry: &mut ItEntry<_> = unsafe { transmute(&mut self.pool.storage[cur.get()]) };
            self.cur = Some(entry.next_previous_used_index().0);
            if self.cur == self.pool.first_used {
                // Reached the end
                self.cur = None;
            }
            self.remaining_len -= 1;
            Some((cur, entry.as_mut().unwrap()))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining_len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut pool = Pool::new();
        let ptr1 = pool.allocate(1);
        let ptr2 = pool.allocate(2);
        assert_eq!(pool[ptr1], 1);
        assert_eq!(pool[ptr2], 2);

        assert_eq!(pool.iter().cloned().collect::<Vec<_>>(), vec![1, 2]);
        pool.deallocate(ptr1);
        assert_eq!(pool.iter().cloned().collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    #[should_panic]
    fn dangling_ptr() {
        let mut pool = Pool::new();
        let ptr = pool.allocate(1);
        pool.deallocate(ptr);
        pool[ptr];
    }

    #[test]
    fn get_fail() {
        let pool = Pool::<u32>::new();
        let ptr = PoolPtr::uninitialized();
        assert!(pool.get(ptr).is_none());
    }

    #[test]
    fn it_get_fail() {
        let pool = IterablePool::<u32>::new();
        let ptr = PoolPtr::uninitialized();
        assert!(pool.get(ptr).is_none());
    }

    #[test]
    fn pool_iter_size_hint() {
        let mut pool = Pool::new();
        let ptr1 = pool.allocate(1);
        let _ptr2 = pool.allocate(2);
        pool.deallocate(ptr1);

        let it = pool.iter();
        let (lower, upper) = dbg!(it.size_hint());
        assert!(lower <= 1);
        assert!(upper.unwrap() >= 1);
        drop(it);

        assert_eq!(pool.iter_mut().size_hint(), (lower, upper));
        assert_eq!(pool.ptr_iter().size_hint(), (lower, upper));
        assert_eq!(pool.ptr_iter_mut().size_hint(), (lower, upper));
    }

    #[test]
    fn iterable_pool_iter_size_hint() {
        let mut pool = IterablePool::new();
        let ptr1 = pool.allocate(1);
        let _ptr2 = pool.allocate(2);
        pool.deallocate(ptr1);

        let it = pool.iter();
        let (lower, upper) = dbg!(it.size_hint());
        assert!(lower <= 1);
        assert!(upper.unwrap() >= 1);
        drop(it);

        assert_eq!(pool.iter_mut().size_hint(), (lower, upper));
    }
}
