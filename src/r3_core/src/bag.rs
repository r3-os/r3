//! A heterogeneous collection to store property values.
use core::{any::TypeId, mem::transmute};

/// A heterogeneous collection to store property values.
#[const_trait]
pub trait Bag: private::Sealed + Copy {
    /// Insert an item and return a new `impl Bag`.
    ///
    /// For `const fn`-ness, this method can't have a provided implementation.
    #[inline]
    fn insert<T: 'static>(self, head: T) -> List<T, Self> {
        assert!(self.get::<T>().is_none(), "duplicate entry");
        (head, self)
    }

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

impl const Bag for Empty {
    fn get<T: 'static>(&self) -> Option<&T> {
        None
    }

    fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        None
    }
}

impl<Head: 'static + Copy, Tail: ~const Bag> const Bag for List<Head, Tail> {
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

    #[const_trait]
    pub trait Sealed {}

    impl const Sealed for () {}
    impl<Head: 'static, Tail: ~const Bag> const Sealed for super::List<Head, Tail> {}
    impl<Left: ~const Bag, Right: ~const Bag> const Sealed for super::Either<Left, Right> {}
}
