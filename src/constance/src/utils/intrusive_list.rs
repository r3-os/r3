//! Intrusive doubly linked list backed by a container implementing
//! `std::ops::Index`.
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
pub struct ListAccessorCell<'a, HeadCell, Pool, MapLink> {
    head: HeadCell,
    pool: &'a Pool,
    map_link: MapLink,
}

impl<'a, HeadCell, Index, Pool, MapLink, Element, LinkCell>
    ListAccessorCell<'a, HeadCell, Pool, MapLink>
where
    HeadCell: CellLike<Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &LinkCell,
    LinkCell: CellLike<Target = Option<Link<Index>>>,
    Index: PartialEq + Clone,
{
    pub fn new(head: HeadCell, pool: &'a Pool, map_link: MapLink) -> Self {
        ListAccessorCell {
            head,
            pool,
            map_link,
        }
    }

    pub fn head_cell(&self) -> &HeadCell {
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

impl<'a, HeadCell, Pool, MapLink> ops::Deref for ListAccessorCell<'a, HeadCell, Pool, MapLink> {
    type Target = Pool;

    fn deref(&self) -> &Self::Target {
        self.pool
    }
}

impl<'a, HeadCell, Index, Pool, MapLink, Element, LinkCell> Extend<Index>
    for ListAccessorCell<'a, HeadCell, Pool, MapLink>
where
    HeadCell: CellLike<Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: Fn(&Element) -> &LinkCell,
    LinkCell: CellLike<Target = Option<Link<Index>>>,
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

impl<'a, 'b, HeadCell, Index, Pool, MapLink, Element, LinkCell> Iterator
    for Iter<&'b ListAccessorCell<'a, HeadCell, Pool, MapLink>, Index>
where
    HeadCell: CellLike<Target = ListHead<Index>>,
    Pool: ops::Index<Index, Output = Element>,
    MapLink: 'a + Fn(&Element) -> &LinkCell,
    Element: 'a + 'b,
    LinkCell: CellLike<Target = Option<Link<Index>>>,
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
