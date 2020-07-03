//! Binary heap with a contextful comparator
//!
//! The implementation is mostly based on the Rust standard library's
//! `BinaryHeap`.
mod helpers;
#[cfg(test)]
mod tests;
mod veclike;
pub use self::veclike::*;

/// Context type for [`BinaryHeap`]'s operations.
pub trait BinaryHeapCtx<Element> {
    /// Return `true` iff `x < y`.
    fn lt(&mut self, x: &Element, y: &Element) -> bool;

    // TODO: Call `on_move`
    /// Called when the existing element `e` is moved to the new position
    /// `new_index`.
    fn on_move(&mut self, e: &mut Element, new_index: usize) {
        let _ = (e, new_index);
    }
}

impl<T: Ord> BinaryHeapCtx<T> for () {
    fn lt(&mut self, x: &T, y: &T) -> bool {
        *x < *y
    }
}

/// Min-heap.
pub trait BinaryHeap: VecLike {
    /// Remove the least item from the heap and return it.
    fn heap_pop(&mut self, ctx: impl BinaryHeapCtx<Self::Element>) -> Option<Self::Element>;

    /// Remove the item at the specified position and return it.
    fn heap_remove(
        &mut self,
        i: usize,
        ctx: impl BinaryHeapCtx<Self::Element>,
    ) -> Option<Self::Element>;

    /// Push an item onto the heap and return its position.
    fn heap_push(&mut self, item: Self::Element, ctx: impl BinaryHeapCtx<Self::Element>) -> usize;
}

impl<T: VecLike> BinaryHeap for T {
    fn heap_pop(&mut self, ctx: impl BinaryHeapCtx<Self::Element>) -> Option<Self::Element> {
        self.heap_remove(0, ctx)
    }

    fn heap_remove(
        &mut self,
        i: usize,
        mut ctx: impl BinaryHeapCtx<Self::Element>,
    ) -> Option<Self::Element> {
        if i >= self.len() {
            return None;
        }

        if let Some(mut item) = self.pop() {
            let slice = &mut **self;
            if i < slice.len() {
                // Swap the last item with the item at `i`
                core::mem::swap(&mut slice[i], &mut item);
                ctx.on_move(&mut slice[i], i);

                let should_sift_up = i > 0 && ctx.lt(&slice[i], &slice[(i - 1) / 2]);

                // Sift down or up the item at `i`, restoring the invariant
                // Safety: `i` points to an element within `slice`.
                unsafe {
                    if should_sift_up {
                        sift_up(slice, 0, i, ctx);
                    } else {
                        sift_down_to_bottom(slice, i, ctx);
                    }
                }
            }
            Some(item)
        } else {
            debug_assert!(false);
            None
        }
    }

    fn heap_push(&mut self, item: Self::Element, ctx: impl BinaryHeapCtx<Self::Element>) -> usize {
        let i = self.len();
        self.push(item);

        let slice = &mut **self;
        assert!(i < slice.len());

        // Safety: `i` points to an element within `slice`.
        unsafe { sift_up(slice, 0, i, ctx) }
    }
}

// The implementations of sift_up and sift_down use unsafe blocks in
// order to move an element out of the vector (leaving behind a
// hole), shift along the others and move the removed element back into the
// vector at the final location of the hole.
// The `Hole` type is used to represent this, and make sure
// the hole is filled back at the end of its scope, even on panic.
// Using a hole reduces the constant factor compared to using swaps,
// which involves twice as many moves.
/// Sift-up operation
///
/// # Safety
///
/// `pos` must point to an element within `this`.
unsafe fn sift_up<Element>(
    this: &mut [Element],
    start: usize,
    pos: usize,
    mut ctx: impl BinaryHeapCtx<Element>,
) -> usize {
    unsafe {
        // Take out the value at `pos` and create a hole.
        let mut hole = helpers::Hole::new(this, pos);

        while hole.pos() > start {
            let parent = (hole.pos() - 1) / 2;
            if !ctx.lt(hole.element(), hole.get(parent)) {
                break;
            }
            hole.move_to(parent);
        }
        hole.pos()
    }
}

/// Take an element at `pos` and move it all the way down the heap,
/// then sift it up to its position.
///
/// Note: This is faster when the element is known to be large / should
/// be closer to the bottom.
///
/// # Safety
///
/// `pos` must point to an element within `this`.
unsafe fn sift_down_to_bottom<Element>(
    this: &mut [Element],
    mut pos: usize,
    mut ctx: impl BinaryHeapCtx<Element>,
) {
    let end = this.len();
    let start = pos;
    unsafe {
        let mut hole = helpers::Hole::new(this, pos);
        let mut child = 2 * pos + 1;
        while child < end {
            let right = child + 1;
            // compare with the greater of the two children
            if right < end && !ctx.lt(hole.get(child), hole.get(right)) {
                child = right;
            }
            hole.move_to(child);
            child = 2 * hole.pos() + 1;
        }
        pos = hole.pos();
    }

    // Safety: `pos` points to an element within `this`.
    unsafe { sift_up(this, start, pos, ctx) };
}
