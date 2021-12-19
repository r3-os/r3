use arrayvec::ArrayVec;
use core::ops;

pub trait VecLike: ops::Deref<Target = [<Self as VecLike>::Element]> + ops::DerefMut {
    // FIXME: Work-around for the lack of compiler support for adding a
    //        `const Default` bound to a non-`const` `impl` and the lack of
    //        a `ConstDefault` impl for `ArrayVec`
    const DEFAULT: Self;
    type Element;
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn pop(&mut self) -> Option<Self::Element>;
    fn push(&mut self, x: Self::Element);
}

impl<T, const N: usize> VecLike for ArrayVec<T, N> {
    const DEFAULT: Self = Self::new_const();
    type Element = T;
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn pop(&mut self) -> Option<Self::Element> {
        self.pop()
    }
    fn push(&mut self, x: Self::Element) {
        self.push(x)
    }
}

#[cfg(test)]
impl<T> VecLike for Vec<T> {
    const DEFAULT: Self = Vec::new();
    type Element = T;
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
    fn len(&self) -> usize {
        self.len()
    }
    fn pop(&mut self) -> Option<Self::Element> {
        self.pop()
    }
    fn push(&mut self, x: Self::Element) {
        self.push(x)
    }
}
