//! A heterogeneous collection to store property values.
use core::mem::transmute;

/// A heterogeneous collection to store property values.
pub trait Bag: private::Sealed + Copy {
    /// Insert an item and return a new `impl Bag`.
    ///
    /// For `const fn`-ness, this method can't have a provided implementation.
    fn insert<T: 'static>(self, x: T) -> List<T, Self>;

    /// Borrow a `T` if it's included in `self`.
    fn get<T: 'static>(&self) -> Option<&T>;

    /// Mutably borrow a `T` if it's included in `self`.
    fn get_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

/// The empty [`Bag`].
pub const EMPTY: Empty = ();

/// The type of the empty [`Bag`].
pub type Empty = ();

/// A [`Bag`] containing `Head` and the elements from `Tail: Bag`.
pub type List<Head, Tail> = (Head, Tail);

#[doc(no_inline)]
pub use either::Either;

const fn insert_inner<Head: 'static, Tail: ~const Bag>(head: Head, tail: Tail) -> List<Head, Tail> {
    assert!(tail.get::<Head>().is_none(), "duplicate entry");
    (head, tail)
}

impl const Bag for Empty {
    #[inline]
    fn insert<T: 'static>(self, x: T) -> List<T, Self> {
        insert_inner(x, self)
    }

    fn get<T: 'static>(&self) -> Option<&T> {
        None
    }

    fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        None
    }
}

impl<Head: 'static + Copy, Tail: ~const Bag> const Bag for List<Head, Tail> {
    #[inline]
    fn insert<T: 'static>(self, x: T) -> List<T, Self> {
        insert_inner(x, self)
    }

    fn get<T: 'static>(&self) -> Option<&T> {
        // Simulate specialization
        if TypeId::of::<T>().eq(&TypeId::of::<Head>()) {
            Some(unsafe { transmute::<&Head, &T>(&self.0) })
        } else {
            self.1.get()
        }
    }

    fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        // Simulate specialization
        if TypeId::of::<T>().eq(&TypeId::of::<Head>()) {
            Some(unsafe { transmute::<&mut Head, &mut T>(&mut self.0) })
        } else {
            self.1.get_mut()
        }
    }
}

impl<Left: ~const Bag, Right: ~const Bag> const Bag for Either<Left, Right> {
    #[inline]
    fn insert<T: 'static>(self, x: T) -> List<T, Self> {
        insert_inner(x, self)
    }

    fn get<T: 'static>(&self) -> Option<&T> {
        match self {
            Either::Left(x) => x.get(),
            Either::Right(x) => x.get(),
        }
    }

    fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        match self {
            Either::Left(x) => x.get_mut(),
            Either::Right(x) => x.get_mut(),
        }
    }
}

mod private {
    use super::Bag;

    pub trait Sealed {}

    impl const Sealed for () {}
    impl<Head: 'static, Tail: ~const Bag> const Sealed for super::List<Head, Tail> {}
    impl<Left: ~const Bag, Right: ~const Bag> const Sealed for super::Either<Left, Right> {}
}

/// A wrapper of [`core::any::TypeId`] that is usable in a constant context.
struct TypeId {
    inner: core::any::TypeId,
}

impl TypeId {
    #[inline]
    const fn of<T: 'static>() -> Self {
        Self {
            inner: core::any::TypeId::of::<T>(),
        }
    }

    #[inline]
    const fn eq(&self, other: &Self) -> bool {
        // This relies on the implementation details of `TypeId`, but I think
        // we're are okay judging by the fact that WebRender is doing the same
        // <https://github.com/rust-lang/rust/pull/75923#issuecomment-683090745>
        unsafe {
            type TypeIdBytes = [u8; core::mem::size_of::<core::any::TypeId>()];
            let x: TypeIdBytes = transmute(self.inner);
            let y: TypeIdBytes = transmute(other.inner);
            // FIXME: Work-around for `[u8; _]: PartialEq` not being `const fn`
            // FIXME: Work-around for `Range: Iterator` not being `const`
            let mut i = 0;
            while i < x.len() {
                if x[i] != y[i] {
                    return false;
                }
                i += 1;
            }
            true
        }
    }
}
