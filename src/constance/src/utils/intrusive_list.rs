//! Intrusive doubly linked list backed by a container implementing
//! `std::ops::Index`.
use core::mem::transmute;
use core::ops;

/// Circualr linked list header.
#[derive(Debug, Copy, Clone)]
pub struct ListHead<Index> {
    pub first: Option<Index>,
}

impl<Index> Default for ListHead<Index> {
    fn default() -> Self {
        Self { first: None }
    }
}

/// Links to neighbor items.
#[derive(Debug, Copy, Clone)]
pub struct Link<Index> {
    pub prev: Index,
    pub next: Index,
}

impl<Index> ListHead<Index> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.first.is_none()
    }

    pub fn accessor<'a, Pool, MapLink, Element>(
        &'a self,
        pool: &'a Pool,
        map_link: MapLink,
    ) -> ListAccessor<'a, Index, Pool, MapLink>
    where
        Pool: 'a + ops::Index<Index, Output = Element>,
        MapLink: Fn(&Element) -> &Option<Link<Index>>,
    {
        ListAccessor {
            head: self,
            pool,
            map_link,
        }
    }

    pub fn accessor_mut<'a, Pool, MapLink, Element>(
        &'a mut self,
        pool: &'a mut Pool,
        map_link: MapLink,
    ) -> ListAccessorMut<'a, Index, Pool, MapLink>
    where
        Pool: 'a + ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
        MapLink: FnMut(&mut Element) -> &mut Option<Link<Index>>,
    {
        ListAccessorMut {
            head: self,
            pool,
            map_link,
        }
    }
}

/// Accessor to a linked list.
#[derive(Debug)]
pub struct ListAccessor<'a, Index, Pool, MapLink> {
    head: &'a ListHead<Index>,
    pool: &'a Pool,
    map_link: MapLink,
}

impl<'a, Index, Pool, MapLink, Element> ListAccessor<'a, Index, Pool, MapLink>
where
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &Option<Link<Index>>,
    Index: PartialEq + Clone,
{
    pub fn head(&self) -> &ListHead<Index> {
        self.head
    }

    pub fn pool(&self) -> &Pool {
        self.pool
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_empty()
    }

    pub fn front(&self) -> Option<Index> {
        self.head.first.clone()
    }

    pub fn back(&self) -> Option<Index> {
        self.head.first.clone().map(|p| {
            (self.map_link)(&self.pool[p])
                .as_ref()
                .unwrap()
                .prev
                .clone()
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

    pub fn iter(&self) -> Iter<&Self, Index> {
        Iter {
            next: self.head.first.clone(),
            accessor: self,
        }
    }
}

impl<'a, Index, Pool: 'a, MapLink> ops::Deref for ListAccessor<'a, Index, Pool, MapLink> {
    type Target = Pool;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

/// Mutable accessor to a linked list.
#[derive(Debug)]
pub struct ListAccessorMut<'a, Index, Pool, MapLink> {
    head: &'a mut ListHead<Index>,
    pool: &'a mut Pool,
    map_link: MapLink,
}

impl<'a, Index, Pool, MapLink, Element> ListAccessorMut<'a, Index, Pool, MapLink>
where
    Pool: ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
    MapLink: FnMut(&mut Element) -> &mut Option<Link<Index>>,
    Index: PartialEq + Clone,
{
    pub fn head(&self) -> &ListHead<Index> {
        self.head
    }

    pub fn head_mut(&mut self) -> &mut ListHead<Index> {
        self.head
    }

    pub fn pool(&self) -> &Pool {
        self.pool
    }

    pub fn pool_mut(&mut self) -> &mut Pool {
        self.pool
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_empty()
    }

    pub fn front(&mut self) -> Option<Index> {
        self.head.first.clone()
    }

    pub fn back(&mut self) -> Option<Index> {
        self.head.first.clone().map(|p| {
            (self.map_link)(&mut self.pool[p])
                .as_ref()
                .unwrap()
                .prev
                .clone()
        })
    }

    pub fn front_data(&mut self) -> Option<&mut Element> {
        if let Some(p) = self.front() {
            Some(&mut self.pool[p])
        } else {
            None
        }
    }

    pub fn back_data(&mut self) -> Option<&mut Element> {
        if let Some(p) = self.back() {
            Some(&mut self.pool[p])
        } else {
            None
        }
    }

    /// Insert `item` before the position `p` (if `at` is `Some(p)`) or to the
    /// the list's back (if `at` is `None`).
    pub fn insert(&mut self, item: Index, at: Option<Index>) {
        #[allow(clippy::debug_assert_with_mut_call)]
        {
            debug_assert!(
                (self.map_link)(&mut self.pool[item.clone()]).is_none(),
                "item is already linked"
            );
        }

        if let Some(first) = self.head.first.clone() {
            let (next, update_first) = if let Some(at) = at {
                let update_first = at == first;
                (at, update_first)
            } else {
                (first, false)
            };

            let prev = (self.map_link)(&mut self.pool[next.clone()])
                .as_mut()
                .unwrap()
                .prev
                .clone();
            (self.map_link)(&mut self.pool[prev.clone()])
                .as_mut()
                .unwrap()
                .next = item.clone();
            (self.map_link)(&mut self.pool[next.clone()])
                .as_mut()
                .unwrap()
                .prev = item.clone();
            *(self.map_link)(&mut self.pool[item.clone()]) = Some(Link { prev, next });

            if update_first {
                self.head.first = Some(item);
            }
        } else {
            debug_assert!(at.is_none());

            let link = (self.map_link)(&mut self.pool[item.clone()]);
            self.head.first = Some(item.clone());
            *link = Some(Link {
                prev: item.clone(),
                next: item,
            });
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
        #[allow(clippy::debug_assert_with_mut_call)]
        {
            debug_assert!(
                (self.map_link)(&mut self.pool[item.clone()]).is_some(),
                "item is not linked"
            );
        }

        let link: Link<Index> = {
            let link_ref = (self.map_link)(&mut self.pool[item.clone()]);
            if self.head.first.as_ref() == Some(&item) {
                let next = link_ref.as_ref().unwrap().next.clone();
                if next == item {
                    // The list just became empty
                    self.head.first = None;
                    *link_ref = None;
                    return item;
                }

                // Move the head pointer
                self.head.first = Some(next);
            }

            link_ref.clone().unwrap()
        };

        (self.map_link)(&mut self.pool[link.prev.clone()])
            .as_mut()
            .unwrap()
            .next = link.next.clone();
        (self.map_link)(&mut self.pool[link.next])
            .as_mut()
            .unwrap()
            .prev = link.prev;
        *(self.map_link)(&mut self.pool[item.clone()]) = None;

        item
    }

    pub fn pop_back(&mut self) -> Option<Index> {
        self.back().map(|item| self.remove(item))
    }

    pub fn pop_front(&mut self) -> Option<Index> {
        self.front().map(|item| self.remove(item))
    }

    /// Create an iterator.
    ///
    /// # Safety
    ///
    /// If the link structure is corrupt, it may return a mutable reference to
    /// the same element more than once, which is an undefined behavior.
    pub unsafe fn iter_mut(&mut self) -> Iter<&mut Self, Index> {
        Iter {
            next: self.head.first.clone(),
            accessor: self,
        }
    }

    /// Create a draining iterator.
    ///
    /// # Safety
    ///
    /// If the link structure is corrupt, it may return a mutable reference to
    /// the same element more than once, which is an undefined behavior.
    pub unsafe fn drain<'b>(&'b mut self) -> Drain<'a, 'b, Index, Pool, MapLink, Element> {
        Drain { accessor: self }
    }
}

impl<'a, Index, Pool: 'a, MapLink> ops::Deref for ListAccessorMut<'a, Index, Pool, MapLink> {
    type Target = Pool;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

impl<'a, Index, Pool: 'a, MapLink> ops::DerefMut for ListAccessorMut<'a, Index, Pool, MapLink> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pool
    }
}

impl<'a, Index, Pool, MapLink, Element> Extend<Index> for ListAccessorMut<'a, Index, Pool, MapLink>
where
    Pool: 'a + ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
    MapLink: FnMut(&mut Element) -> &mut Option<Link<Index>>,
    Index: PartialEq + Clone,
{
    fn extend<I: IntoIterator<Item = Index>>(&mut self, iter: I) {
        for item in iter {
            self.push_back(item);
        }
    }
}

pub trait CellLike {
    type Target;

    fn get(&self) -> Self::Target;
    fn set(&self, value: Self::Target);

    fn modify(&self, f: impl FnOnce(&mut Self::Target))
    where
        Self: Sized,
    {
        let mut x = self.get();
        f(&mut x);
        self.set(x);
    }
}

impl<Element: Copy> CellLike for core::cell::Cell<Element> {
    type Target = Element;

    fn get(&self) -> Self::Target {
        self.get()
    }
    fn set(&self, value: Self::Target) {
        self.set(value);
    }
}

impl<Element: CellLike> CellLike for &Element {
    type Target = Element::Target;

    fn get(&self) -> Self::Target {
        (*self).get()
    }
    fn set(&self, value: Self::Target) {
        (*self).set(value);
    }
}

/// `Cell`-based accessor to a linked list.
#[derive(Debug)]
pub struct ListAccessorCell<'a, H, Pool, MapLink> {
    head: H,
    pool: &'a Pool,
    map_link: MapLink,
}

impl<'a, H, Index, Pool, MapLink, Element, L> ListAccessorCell<'a, H, Pool, MapLink>
where
    H: CellLike<Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &L,
    L: CellLike<Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    pub fn new(head: H, pool: &'a Pool, map_link: MapLink) -> Self {
        ListAccessorCell {
            head,
            pool,
            map_link,
        }
    }

    pub fn head_cell(&self) -> &H {
        &self.head
    }

    pub fn head(&self) -> ListHead<Index> {
        self.head.get()
    }

    pub fn set_head(&self, head: ListHead<Index>) {
        self.head.set(head);
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
        self.head()
            .first
            .map(|p| (self.map_link)(&self.pool[p]).get().unwrap().prev)
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
    pub fn insert(&self, item: Index, at: Option<Index>) {
        debug_assert!(
            (self.map_link)(&self.pool[item.clone()]).get().is_none(),
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
                .get()
                .unwrap()
                .prev;
            (self.map_link)(&self.pool[prev.clone()])
                .modify(|l| l.as_mut().unwrap().next = item.clone());
            (self.map_link)(&self.pool[next.clone()])
                .modify(|l| l.as_mut().unwrap().prev = item.clone());
            (self.map_link)(&self.pool[item.clone()]).set(Some(Link { prev, next }));

            if update_first {
                head.first = Some(item);
                self.set_head(head);
            }
        } else {
            debug_assert!(at.is_none());

            let link = (self.map_link)(&self.pool[item.clone()]);
            link.set(Some(Link {
                prev: item.clone(),
                next: item.clone(),
            }));

            head.first = Some(item);
            self.set_head(head);
        }
    }

    pub fn push_back(&self, item: Index) {
        self.insert(item, None);
    }

    pub fn push_front(&self, item: Index) {
        let at = self.front();
        self.insert(item, at);
    }

    /// Remove `item` from the list. Returns `item`.
    pub fn remove(&self, item: Index) -> Index {
        debug_assert!(
            (self.map_link)(&self.pool[item.clone()]).get().is_some(),
            "item is not linked"
        );

        let link: Link<Index> = {
            let link_ref = (self.map_link)(&self.pool[item.clone()]);
            let mut head = self.head();
            if head.first.as_ref() == Some(&item) {
                let next = link_ref.get().unwrap().next;
                if next == item {
                    // The list just became empty
                    head.first = None;
                    self.set_head(head);

                    link_ref.set(None);
                    return item;
                }

                // Move the head pointer
                head.first = Some(next);
                self.set_head(head);
            }

            link_ref.get().unwrap()
        };

        (self.map_link)(&self.pool[link.prev.clone()])
            .modify(|l| l.as_mut().unwrap().next = link.next.clone());
        (self.map_link)(&self.pool[link.next.clone()])
            .modify(|l| l.as_mut().unwrap().prev = link.prev.clone());
        (self.map_link)(&self.pool[item.clone()]).set(None);

        item
    }

    pub fn pop_back(&self) -> Option<Index> {
        self.back().map(|item| self.remove(item))
    }

    pub fn pop_front(&self) -> Option<Index> {
        self.front().map(|item| self.remove(item))
    }

    pub fn iter(&self) -> Iter<&Self, Index> {
        Iter {
            next: self.head().first,
            accessor: self,
        }
    }

    pub fn clear(&self) {
        for (_, el) in self.iter() {
            (self.map_link)(el).set(None);
        }
        self.set_head(ListHead::new());
    }
}

impl<'a, H, Pool, MapLink> ops::Deref for ListAccessorCell<'a, H, Pool, MapLink> {
    type Target = Pool;

    fn deref(&self) -> &Self::Target {
        self.pool
    }
}

impl<'a, H, Index, Pool, MapLink, Element, L> Extend<Index>
    for ListAccessorCell<'a, H, Pool, MapLink>
where
    H: CellLike<Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &L,
    L: CellLike<Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    fn extend<I: IntoIterator<Item = Index>>(&mut self, iter: I) {
        for item in iter {
            self.push_back(item);
        }
    }
}

/// An iterator over the elements of `ListAccessor`, `ListAccessorMut`, or
/// `ListAccessorCell`.
#[derive(Debug)]
pub struct Iter<Element, Index> {
    accessor: Element,
    next: Option<Index>,
}

impl<'a, 'b, Index, Pool, MapLink, Element> Iterator
    for Iter<&'b ListAccessor<'a, Index, Pool, MapLink>, Index>
where
    Pool: ops::Index<Index, Output = Element>,
    MapLink: 'a + Fn(&Element) -> &Option<Link<Index>>,
    Element: 'a,
    Index: PartialEq + Clone,
{
    type Item = (Index, &'a Element);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next.take() {
            let new_next = (self.accessor.map_link)(&self.accessor.pool[next.clone()])
                .as_ref()
                .unwrap()
                .next
                .clone();
            if Some(&new_next) == self.accessor.head.first.as_ref() {
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

impl<'a, 'b, Index, Pool, MapLink, Element> Iterator
    for Iter<&'b mut ListAccessorMut<'a, Index, Pool, MapLink>, Index>
where
    Pool: ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
    MapLink: 'a + FnMut(&mut Element) -> &mut Option<Link<Index>>,
    Element: 'a + 'b,
    Index: PartialEq + Clone,
{
    type Item = (Index, &'a mut Element);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next.take() {
            let new_next = (self.accessor.map_link)(&mut self.accessor.pool[next.clone()])
                .as_ref()
                .unwrap()
                .next
                .clone();
            if Some(&new_next) == self.accessor.head.first.as_ref() {
                self.next = None;
            } else {
                self.next = Some(new_next);
            }
            Some((next.clone(), unsafe {
                transmute(&mut self.accessor.pool[next])
            }))
        } else {
            None
        }
    }
}

impl<'a, 'b, H, Index, Pool, MapLink, Element, L> Iterator
    for Iter<&'b ListAccessorCell<'a, H, Pool, MapLink>, Index>
where
    H: CellLike<Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: 'a + Fn(&Element) -> &L,
    Element: 'a + 'b,
    L: CellLike<Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    type Item = (Index, &'a Element);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.next.take() {
            let new_next = (self.accessor.map_link)(&self.accessor.pool[next.clone()])
                .get()
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

#[derive(Debug)]
pub struct Drain<'a, 'b, Index, Pool, MapLink, Element>
where
    Pool: ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
    MapLink: 'a + FnMut(&mut Element) -> &mut Option<Link<Index>>,
    Element: 'a + 'b,
    Index: PartialEq + Clone,
{
    accessor: &'b mut ListAccessorMut<'a, Index, Pool, MapLink>,
}

impl<'a, 'b, Index, Pool, MapLink, Element> Iterator
    for Drain<'a, 'b, Index, Pool, MapLink, Element>
where
    Pool: ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
    MapLink: 'a + FnMut(&mut Element) -> &mut Option<Link<Index>>,
    Element: 'a + 'b,
    Index: PartialEq + Clone,
{
    type Item = (Index, &'a mut Element);

    fn next(&mut self) -> Option<Self::Item> {
        let ptr = self.accessor.pop_front();
        if let Some(p) = ptr {
            Some((p.clone(), unsafe { transmute(&mut self.accessor.pool[p]) }))
        } else {
            None
        }
    }
}

impl<'a, 'b, Index, Pool, MapLink, Element> Drop for Drain<'a, 'b, Index, Pool, MapLink, Element>
where
    Pool: ops::Index<Index, Output = Element> + ops::IndexMut<Index>,
    MapLink: 'a + FnMut(&mut Element) -> &mut Option<Link<Index>>,
    Element: 'a + 'b,
    Index: PartialEq + Clone,
{
    fn drop(&mut self) {
        while self.accessor.pop_back().is_some() {}
    }
}

#[cfg(test)]
fn push<Element>(this: &mut Vec<Element>, x: Element) -> usize {
    let i = this.len();
    this.push(x);
    i
}

#[test]
fn basic_mut() {
    let mut pool = Vec::new();
    let mut head = ListHead::new();
    let mut accessor = head.accessor_mut(&mut pool, |&mut (_, ref mut link)| link);

    let ptr1 = push(&mut accessor, (1, None));
    accessor.push_back(ptr1);

    let ptr2 = push(&mut accessor, (2, None));
    accessor.push_back(ptr2);

    let ptr3 = push(&mut accessor, (3, None));
    accessor.push_front(ptr3);

    println!("{:?}", (accessor.pool(), accessor.head()));

    assert!(!accessor.is_empty());
    assert_eq!(accessor.front(), Some(ptr3));
    assert_eq!(accessor.back(), Some(ptr2));
    assert_eq!(accessor.front_data().unwrap().0, 3);
    assert_eq!(accessor.back_data().unwrap().0, 2);

    let items: Vec<_> = unsafe { accessor.iter_mut() }
        .map(|(_, &mut (x, _))| x)
        .collect();
    assert_eq!(items, vec![3, 1, 2]);

    accessor.remove(ptr1);
    accessor.remove(ptr2);
    accessor.remove(ptr3);

    assert!(accessor.is_empty());
}

#[test]
fn basic_cell() {
    use std::cell::Cell;
    let mut pool = Vec::new();
    let head = Cell::new(ListHead::new());

    macro_rules! get_accessor {
        () => {
            ListAccessorCell::new(&head, &pool, |(_, link)| link)
        };
    }

    let ptr1 = push(&mut pool, (1, Cell::new(None)));
    get_accessor!().push_back(ptr1);

    let ptr2 = push(&mut pool, (2, Cell::new(None)));
    get_accessor!().push_back(ptr2);

    let ptr3 = push(&mut pool, (3, Cell::new(None)));
    get_accessor!().push_front(ptr3);

    println!("{:?}", (&pool, &head));

    let accessor = get_accessor!();
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
            ListAccessorCell::new(&head, &pool, |(_, link)| link)
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

    get_accessor!().clear();

    assert_eq!(head.get().first, None);
    for &ptr in &ptrs {
        let e = &pool[ptr];
        assert!(e.1.get().is_none());
    }
}

#[test]
fn drain() {
    let mut pool = Vec::new();
    let mut head = ListHead::new();
    let mut accessor = head.accessor_mut(&mut pool, |&mut (_, ref mut link)| link);

    let ptr1 = push(&mut accessor, (1, None));
    accessor.push_back(ptr1);

    let ptr2 = push(&mut accessor, (2, None));
    accessor.push_back(ptr2);

    let ptr3 = push(&mut accessor, (3, None));
    accessor.push_front(ptr3);

    let items: Vec<_> = unsafe { accessor.drain() }
        .map(|(_, &mut (x, _))| x)
        .collect();
    assert_eq!(items, vec![3, 1, 2]);

    assert!(accessor.is_empty());
}

#[test]
fn basic() {
    let mut pool = Vec::new();
    let mut head = ListHead::new();
    let (_, ptr2, ptr3) = {
        let mut accessor = head.accessor_mut(&mut pool, |&mut (_, ref mut link)| link);

        let ptr1 = push(&mut accessor, (1, None));
        accessor.push_back(ptr1);

        let ptr2 = push(&mut accessor, (2, None));
        accessor.push_back(ptr2);

        let ptr3 = push(&mut accessor, (3, None));
        accessor.push_front(ptr3);

        println!("{:?}", (accessor.pool(), accessor.head()));

        (ptr1, ptr2, ptr3)
    };

    let accessor = head.accessor(&pool, |&(_, ref link)| link);
    assert!(!accessor.is_empty());
    assert_eq!(accessor.front(), Some(ptr3));
    assert_eq!(accessor.back(), Some(ptr2));
    assert_eq!(accessor.front_data().unwrap().0, 3);
    assert_eq!(accessor.back_data().unwrap().0, 2);

    let items: Vec<_> = accessor.iter().map(|(_, &(x, _))| x).collect();
    assert_eq!(items, vec![3, 1, 2]);
}
