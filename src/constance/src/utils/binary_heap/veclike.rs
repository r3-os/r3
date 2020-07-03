use core::ops;
use staticvec::StaticVec;

pub trait VecLike:
    ops::Deref<Target = [<Self as VecLike>::Element]>
    + ops::DerefMut
    + ops::Index<usize, Output = <Self as VecLike>::Element>
    + ops::IndexMut<usize, Output = <Self as VecLike>::Element>
{
    type Element;
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn pop(&mut self) -> Option<Self::Element>;
    fn push(&mut self, x: Self::Element);
}

impl<T, const N: usize> VecLike for StaticVec<T, N> {
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
