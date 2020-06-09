//! Intrusive doubly linked list backed by a container implementing
//! `std::ops::Index`.
#![allow(dead_code)]
use core::{fmt, ops};

use super::Init;

/// Circualr linked list header.
#[derive(Debug, Copy, Clone)]
pub struct ListHead<Index> {
    pub first: Option<Index>,
}

impl<Index> Default for ListHead<Index> {
    fn default() -> Self {
        Self::INIT
    }
}

impl<Index> Init for ListHead<Index> {
    const INIT: Self = Self { first: None };
}

/// Links to neighbor items.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Link<Index> {
    pub prev: Index,
    pub next: Index,
}

impl<Index: Init> Init for Link<Index> {
    const INIT: Self = Self {
        prev: Index::INIT,
        next: Index::INIT,
    };
}

impl<Index> ListHead<Index> {
    pub const fn new() -> Self {
        Self::INIT
    }

    pub fn is_empty(&self) -> bool {
        self.first.is_none()
    }
}

/// A virtual container of `T`s that can be indexed by `Ident<&'static T>`.
#[derive(Debug, Clone, Copy)]
pub struct Static;

impl<T> ops::Index<Ident<&'static T>> for Static {
    type Output = T;

    fn index(&self, index: Ident<&'static T>) -> &Self::Output {
        index.0
    }
}

/// Reference wrapper that implements `PartialEq` and `Eq` by identity
/// comparison.
#[derive(Clone, Copy)]
pub struct Ident<T>(pub T);

impl<T> fmt::Debug for Ident<&'_ T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Do not print the pointee. This is a safe measure against infinite
        // recursion.
        f.debug_tuple("Ident").field(&(self.0 as *const T)).finish()
    }
}

impl<T: ?Sized> PartialEq for Ident<&'_ T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}

impl<T: ?Sized> Eq for Ident<&'_ T> {}

/// Circualr linked list header where elements are linked by
/// [`StaticLink`]`<Element>` (a pair of `&'static Element`).
pub type StaticListHead<Element> = ListHead<Ident<&'static Element>>;

/// Links to neighbor items with a `'static` lifetime.
///
/// See also: [`StaticListHead`]
pub type StaticLink<Element> = Link<Ident<&'static Element>>;

pub trait CellLike<Key> {
    type Target;

    fn get(&self, key: &Key) -> Self::Target;
    fn set(&self, key: &mut Key, value: Self::Target);

    fn modify(&self, key: &mut Key, f: impl FnOnce(&mut Self::Target))
    where
        Self: Sized,
    {
        let mut x = self.get(key);
        f(&mut x);
        self.set(key, x);
    }
}

impl<Element: Copy> CellLike<()> for core::cell::Cell<Element> {
    type Target = Element;

    fn get(&self, _: &()) -> Self::Target {
        self.get()
    }
    fn set(&self, _: &mut (), value: Self::Target) {
        self.set(value);
    }
}

impl<'a, Element: Clone, Token, Key> CellLike<&'a mut Key> for tokenlock::TokenLock<Element, Token>
where
    Key: tokenlock::Token<Token>,
{
    type Target = Element;

    fn get(&self, key: &&'a mut Key) -> Self::Target {
        self.read(*key).clone()
    }
    fn set(&self, key: &mut &'a mut Key, value: Self::Target) {
        self.replace(*key, value);
    }
}

impl<Key, Element: CellLike<Key>> CellLike<Key> for &Element {
    type Target = Element::Target;

    fn get(&self, key: &Key) -> Self::Target {
        (*self).get(key)
    }
    fn set(&self, key: &mut Key, value: Self::Target) {
        (*self).set(key, value);
    }
}

/// `Cell`-based accessor to a linked list.
#[derive(Debug)]
pub struct ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey> {
    head: HeadCell,
    pool: &'a Pool,
    map_link: MapLink,
    /// `Key` used to read or write cells.
    cell_key: CellKey,
}

impl<'a, HeadCell, Index, Pool, MapLink, Element, LinkCell, CellKey>
    ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey>
where
    HeadCell: CellLike<CellKey, Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &LinkCell,
    LinkCell: CellLike<CellKey, Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    pub fn new(head: HeadCell, pool: &'a Pool, map_link: MapLink, cell_key: CellKey) -> Self {
        ListAccessorCell {
            head,
            pool,
            map_link,
            cell_key,
        }
    }

    pub fn head_cell(&self) -> &HeadCell {
        &self.head
    }

    pub fn head(&self) -> ListHead<Index> {
        self.head.get(&self.cell_key)
    }

    pub fn set_head(&mut self, head: ListHead<Index>) {
        self.head.set(&mut self.cell_key, head);
    }

    pub fn pool(&self) -> &Pool {
        self.pool
    }

    pub fn is_empty(&self) -> bool {
        self.head().is_empty()
    }

    pub fn front(&self) -> Option<Index> {
        self.head().first
    }

    pub fn back(&self) -> Option<Index> {
        self.head().first.map(|p| {
            (self.map_link)(&self.pool[p])
                .get(&self.cell_key)
                .unwrap()
                .prev
        })
    }

    pub fn front_data(&self) -> Option<&Element> {
        if let Some(p) = self.front() {
            Some(&self.pool[p])
        } else {
            None
        }
    }

    pub fn back_data(&self) -> Option<&Element> {
        if let Some(p) = self.back() {
            Some(&self.pool[p])
        } else {
            None
        }
    }

    /// Insert `item` before the position `p` (if `at` is `Some(p)`) or to the
    /// the list's back (if `at` is `None`).
    pub fn insert(&mut self, item: Index, at: Option<Index>) {
        debug_assert!(
            (self.map_link)(&self.pool[item.clone()])
                .get(&self.cell_key)
                .is_none(),
            "item is already linked"
        );

        let mut head = self.head();

        if let Some(first) = head.first {
            let (next, update_first) = if let Some(at) = at {
                let update_first = at == first;
                (at, update_first)
            } else {
                (first, false)
            };

            let prev = (self.map_link)(&self.pool[next.clone()])
                .get(&self.cell_key)
                .unwrap()
                .prev;
            (self.map_link)(&self.pool[prev.clone()]).modify(&mut self.cell_key, |l| {
                l.as_mut().unwrap().next = item.clone()
            });
            (self.map_link)(&self.pool[next.clone()]).modify(&mut self.cell_key, |l| {
                l.as_mut().unwrap().prev = item.clone()
            });
            (self.map_link)(&self.pool[item.clone()])
                .set(&mut self.cell_key, Some(Link { prev, next }));

            if update_first {
                head.first = Some(item);
                self.set_head(head);
            }
        } else {
            debug_assert!(at.is_none());

            let link = (self.map_link)(&self.pool[item.clone()]);
            link.set(
                &mut self.cell_key,
                Some(Link {
                    prev: item.clone(),
                    next: item.clone(),
                }),
            );

            head.first = Some(item);
            self.set_head(head);
        }
    }

    pub fn push_back(&mut self, item: Index) {
        self.insert(item, None);
    }

    pub fn push_front(&mut self, item: Index) {
        let at = self.front();
        self.insert(item, at);
    }

    /// Remove `item` from the list. Returns `item`.
    pub fn remove(&mut self, item: Index) -> Index {
        debug_assert!(
            (self.map_link)(&self.pool[item.clone()])
                .get(&self.cell_key)
                .is_some(),
            "item is not linked"
        );

        let link: Link<Index> = {
            let link_ref = (self.map_link)(&self.pool[item.clone()]);
            let mut head = self.head();
            if head.first.as_ref() == Some(&item) {
                let next = link_ref.get(&self.cell_key).unwrap().next;
                if next == item {
                    // The list just became empty
                    head.first = None;
                    self.set_head(head);

                    link_ref.set(&mut self.cell_key, None);
                    return item;
                }

                // Move the head pointer
                head.first = Some(next);
                self.set_head(head);
            }

            link_ref.get(&self.cell_key).unwrap()
        };

        (self.map_link)(&self.pool[link.prev.clone()]).modify(&mut self.cell_key, |l| {
            l.as_mut().unwrap().next = link.next.clone()
        });
        (self.map_link)(&self.pool[link.next.clone()]).modify(&mut self.cell_key, |l| {
            l.as_mut().unwrap().prev = link.prev.clone()
        });
        (self.map_link)(&self.pool[item.clone()]).set(&mut self.cell_key, None);

        item
    }

    pub fn pop_back(&mut self) -> Option<Index> {
        self.back().map(|item| self.remove(item))
    }

    pub fn pop_front(&mut self) -> Option<Index> {
        self.front().map(|item| self.remove(item))
    }

    pub fn iter(&self) -> Iter<&Self, Index> {
        Iter {
            next: self.head().first,
            accessor: self,
        }
    }
}

impl<'a, HeadCell, Pool, MapLink, CellKey> ops::Deref
    for ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey>
{
    type Target = Pool;

    fn deref(&self) -> &Self::Target {
        self.pool
    }
}

impl<'a, HeadCell, Index, Pool, MapLink, Element, LinkCell, CellKey> Extend<Index>
    for ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey>
where
    HeadCell: CellLike<CellKey, Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &LinkCell,
    LinkCell: CellLike<CellKey, Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    fn extend<I: IntoIterator<Item = Index>>(&mut self, iter: I) {
        for item in iter {
            self.push_back(item);
        }
    }
}

/// An iterator over the elements of `ListAccessorCell`.
#[derive(Debug)]
pub struct Iter<Element, Index> {
    accessor: Element,
    next: Option<Index>,
}

impl<'a, 'b, HeadCell, Index, Pool, MapLink, Element, LinkCell, CellKey> Iterator
    for Iter<&'b ListAccessorCell<'a, HeadCell, Pool, MapLink, CellKey>, Index>
where
    HeadCell: CellLike<CellKey, Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: 'a + Fn(&Element) -> &LinkCell,
    Element: 'a + 'b,
    LinkCell: CellLike<CellKey, Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    type Item = (Index, &'a Element);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next.take() {
            let new_next = (self.accessor.map_link)(&self.accessor.pool[next.clone()])
                .get(&self.accessor.cell_key)
                .unwrap()
                .next;
            if Some(&new_next) == self.accessor.head().first.as_ref() {
                self.next = None;
            } else {
                self.next = Some(new_next);
            }
            Some((next.clone(), &self.accessor.pool[next]))
        } else {
            None
        }
    }
}

#[cfg(test)]
fn push<Element>(this: &mut Vec<Element>, x: Element) -> usize {
    let i = this.len();
    this.push(x);
    i
}

#[test]
fn basic_cell() {
    use std::cell::Cell;
    let mut pool = Vec::new();
    let head = Cell::new(ListHead::new());

    macro_rules! get_accessor {
        () => {
            ListAccessorCell::new(&head, &pool, |(_, link)| link, ())
        };
    }

    let ptr1 = push(&mut pool, (1, Cell::new(None)));
    get_accessor!().push_back(ptr1);

    let ptr2 = push(&mut pool, (2, Cell::new(None)));
    get_accessor!().push_back(ptr2);

    let ptr3 = push(&mut pool, (3, Cell::new(None)));
    get_accessor!().push_front(ptr3);

    println!("{:?}", (&pool, &head));

    let mut accessor = get_accessor!();
    assert!(!accessor.is_empty());
    assert_eq!(accessor.front(), Some(ptr3));
    assert_eq!(accessor.back(), Some(ptr2));
    assert_eq!(accessor.front_data().unwrap().0, 3);
    assert_eq!(accessor.back_data().unwrap().0, 2);

    let items: Vec<_> = accessor.iter().map(|(_, (x, _))| *x).collect();
    assert_eq!(items, vec![3, 1, 2]);

    accessor.remove(ptr1);
    println!("{:?}", (&pool, &head));
    accessor.remove(ptr2);
    println!("{:?}", (&pool, &head));
    accessor.remove(ptr3);
    println!("{:?}", (&pool, &head));

    assert!(accessor.is_empty());
}

#[test]
fn clear_cell() {
    use std::cell::Cell;
    let mut pool = Vec::new();
    let head = Cell::new(ListHead::new());

    macro_rules! get_accessor {
        () => {
            ListAccessorCell::new(&head, &pool, |(_, link)| link, ())
        };
    }

    let ptrs = [
        push(&mut pool, (1, Cell::new(None))),
        push(&mut pool, (2, Cell::new(None))),
        push(&mut pool, (3, Cell::new(None))),
    ];

    get_accessor!().push_back(ptrs[0]);
    get_accessor!().push_back(ptrs[1]);
    get_accessor!().push_front(ptrs[2]);

    while get_accessor!().pop_front().is_some() {}

    assert_eq!(head.get().first, None);
    for &ptr in &ptrs {
        let e = &pool[ptr];
        assert!(e.1.get().is_none());
    }
}

#[cfg(test)]
fn push_static<Element>(x: Element) -> Ident<&'static Element> {
    Ident(Box::leak(Box::new(x)))
}

#[test]
fn basic_cell_static() {
    use std::cell::Cell;
    let head = Cell::new(ListHead::<Ident<&'static El>>::new());

    #[derive(Debug)]
    struct El(u32, Cell<Option<Link<Ident<&'static El>>>>);

    macro_rules! get_accessor {
        () => {
            ListAccessorCell::new(&head, &Static, |El(_, link)| link, ())
        };
    }

    let ptr1 = push_static(El(1, Cell::new(None)));
    get_accessor!().push_back(ptr1);

    let ptr2 = push_static(El(2, Cell::new(None)));
    get_accessor!().push_back(ptr2);

    let ptr3 = push_static(El(3, Cell::new(None)));
    get_accessor!().push_front(ptr3);

    println!("{:?}", &head);

    let mut accessor = get_accessor!();
    assert!(!accessor.is_empty());
    assert_eq!(accessor.front(), Some(ptr3));
    assert_eq!(accessor.back(), Some(ptr2));
    assert_eq!(accessor.front_data().unwrap().0, 3);
    assert_eq!(accessor.back_data().unwrap().0, 2);

    let items: Vec<_> = accessor.iter().map(|(_, El(x, _))| *x).collect();
    assert_eq!(items, vec![3, 1, 2]);

    accessor.remove(ptr1);
    println!("{:?}", &head);
    accessor.remove(ptr2);
    println!("{:?}", &head);
    accessor.remove(ptr3);
    println!("{:?}", &head);

    assert!(accessor.is_empty());
}
