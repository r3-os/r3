use arrayvec::ArrayVec;
use core::{marker::Destruct, ops};

#[const_trait]
pub trait VecLike:
    ~const ops::Deref<Target = [<Self as VecLike>::Element]> + ~const ops::DerefMut
{
    type Element;
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn pop(&mut self) -> Option<Self::Element>;
    fn push(&mut self, x: Self::Element);
}

impl<T: ~const Destruct, const N: usize> VecLike for ArrayVec<T, N> {
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

impl<T: ~const Destruct> const VecLike for crate::utils::ComptimeVec<T> {
    type Element = T;
    fn is_empty(&self) -> bool {
        (**self).is_empty()
    }
    fn len(&self) -> usize {
        (**self).len()
    }
    fn pop(&mut self) -> Option<Self::Element> {
        (*self).pop()
    }
    fn push(&mut self, x: Self::Element) {
        (*self).push(x)
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
