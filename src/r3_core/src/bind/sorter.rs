//! The mechanism to determine the initialization order of bindings and check
//! the borrowing rules. It provides a weird interface to allow testing by
//! runtime code.
//!
//! Some preconditions:
//!
//! - `[tag:bind_conflicting_take]` `bind_users(bind_i)` may have one `(_, Take
//!   | TakeMut)` or any number of `(_, TakeRef)` but not both.
//!
//! - `[tag:bind_conflicting_borrows]` For any `(bind_i, usage)`,
//!   `bind_users(bind_i)` may have one `(usage, BorrowMut)` or any number of
//!   `(usage, Borrow)` but not both.
//!
//! The initialization order is determined by topological sorting
//! `[tag:bind_topo]`. The vertices are defined as follows:
//!
//! - `[tag:bind_topo_vert_bind]` Define a vertex `v(Bind(bind_i))` for each
//!   binding. This represents the initialization of the binding.
//!
//! - `[tag:bind_topo_vert_exe]` Define a vertex `v(Executable)` encompassing
//!   all exectable objects.
//!
//! - `[tag:bind_topo_vert_disown]` Define another vertex `vd(bind_i)` for each
//!   binding, if it's borrowed indefinitely (i.e., there's `(_, Take | TakeRef
//!   | TakeMut)` in `bind_users(bind_i)`).
//!
//! The edges are defined as follows:
//!
//! - For each element `u = (usage, borrow_type)` in `bind_users(bind_i0)`:
//!
//!     - `[tag:bind_topo_edge_borrow]` If `u` is `(usage, Borrow | BorrowMut)`,
//!       define an edge `(v(Bind(bind_i0)), usage)`. This means the value must
//!       be borrowed after it's created.
//!
//!     - `[tag:bind_topo_edge_take]` If `u` is `(usage, Take | TakeRef |
//!       TakeMut)`, define an edge `(vd(bind_i0), usage)`. This makes
//!       `vd(bind_i)` mark the starting point of indefinite borrow(s).
//!
//!     - `[tag:bind_topo_edge_borrow_any_vs_take_mut]` If an element `(_, Take
//!       | TakeMut)` is present in `bind_users(bind_i0)`, and `u` is
//!       `(_, Borrow | BorrowMut)`, define an edge `(usage, vd(bind_i0))`.
//!
//!     - `[tag:bind_topo_edge_borrow_mut_vs_take_ref]` If an element
//!       `(_, TakeRef)` is present in `bind_users(bind_i0)`, and `u` is
//!       `(_, BorrowMut)`, define an edge `(usage, vd(bind_i0))`.
//!
//! - `[tag:bind_topo_edge_disown]` Define an edge `(v(Bind(bind_i)),
//!   vd(bind_i))` if `vd(bind_i)` exists in the graph.
//!
// FIXME: We might be able to improve the interface when `ComptimeVec` is not
// restricted to "comptime" anymore
use core::ops::{Deref, DerefMut, Index, IndexMut};

use super::{BindBorrowType, BindUsage};
use crate::utils::{
    binary_heap::{BinaryHeap, BinaryHeapCtx, VecLike},
    refcell::RefCell,
    Init,
};

pub(super) trait SorterCallback {
    /// Push a binding to the end of the initialization order.
    fn push_bind_order(&mut self, bind_i: usize);

    /// Report an error.
    ///
    /// If any errors are reported, the binding definition is invalid, and no
    /// code using the defined bindings shall be allowed to execute at runtime.
    fn report_error(&mut self, e: SorterError<'_>);

    fn num_binds(&self) -> usize;

    /// Get the consumers of the specified binding.
    fn bind_users(&self, bind_i: usize) -> &[(BindUsage, BindBorrowType)];
}

pub(super) enum SorterError<'a> {
    /// There exists no valid initialization order because of a dependency
    /// cycle.
    BindCycle { bind_is: &'a [usize] },
    /// The binding is borrowed indefinitely or taken in a way that
    /// violates the borrow rules.
    ConflictingIndefiniteBorrow { bind_i: usize },
}

/// A temporary storage to store information about a binding.
#[derive(Clone, Copy)]
pub(super) struct SorterBindInfo1 {
    /// `None`: Not borrowed indefinitely, `Some(false)`: Has a shared
    /// indefinite borrow, `Some(true)` Has an exclusive indefinite borrow
    borrowed_indefinitely: Option<bool>,
    /// An index into `[SorterUseInfo]`. Forms a singly-linked list containing
    /// all bindings that this binding needs to consume for initialization.
    first_use_i: Option<usize>,
}

impl Init for SorterBindInfo1 {
    const INIT: Self = Self {
        borrowed_indefinitely: None,
        first_use_i: None,
    };
}

// FIXME: This could be merged into one `SorterBindInfo` if `tokenlock` could
// be used in `const fn`
/// A temporary storage to store information about a binding.
#[derive(Clone, Copy)]
pub(super) struct SorterBindInfo2 {
    /// The `TopologicalSortVertexInfo`s for `v(Bind(bind_i))`
    /// ([ref:bind_topo_vert_bind]) and `vd(bind_i)`
    /// ([ref:bind_topo_vert_disown]), respectively.
    temp_sort: [TopologicalSortVertexInfo; 2],
}

impl Init for SorterBindInfo2 {
    const INIT: Self = Self {
        temp_sort: Init::INIT,
    };
}

#[derive(Clone, Copy)]
pub(super) struct SorterUseInfo {
    /// A binding this binding depends on for initialization.
    bind_i: usize,
    borrow_type: BindBorrowType,
    /// The next index of [`SorterUseInfo`].
    next_use_i: Option<usize>,
}

/// Used by [`sort_bindings`][]
#[derive(Clone, Copy)]
pub(super) enum Vertex {
    /// [ref:bind_topo_vert_bind]
    BindInit(usize),
    /// [ref:bind_topo_vert_disown]
    BindDisown(usize),
    /// [ref:bind_topo_vert_exe]
    Executable,
}

pub(super) const fn sort_bindings<Callback, SorterUseInfoList, VertexList>(
    cb: &mut Callback,
    temp_binds1: &mut [SorterBindInfo1],
    temp_binds2: &mut [SorterBindInfo2],
    temp_uses: &mut SorterUseInfoList,
    temp_vertices: &mut VertexList,
) where
    Callback: ~const SorterCallback,
    SorterUseInfoList: ~const VecLike<Element = SorterUseInfo> + ~const Deref + ~const DerefMut,
    VertexList: ~const VecLike<Element = Vertex> + ~const Deref + ~const DerefMut,
{
    // Preconditions
    let num_binds = cb.num_binds();
    assert!(temp_binds1.len() >= num_binds);
    assert!(temp_binds2.len() >= num_binds);
    assert!(temp_uses.is_empty());
    assert!(temp_vertices.is_empty());

    {
        // FIXME: Waiting for `Iterator` to be usable in `const fn`
        let mut bind_i = 0;
        while bind_i < num_binds {
            let bind_users = cb.bind_users(bind_i);

            let mut num_indefinite_shared = 0;
            let mut num_indefinite_exclusive = 0;

            // FIXME: Waiting for `Iterator` to be usable in `const fn`
            let mut i = 0;
            while i < bind_users.len() {
                // Reject impossible combinations that should be caught earlier
                match bind_users[i] {
                    (BindUsage::Bind(_), BindBorrowType::Borrow)
                    | (BindUsage::Bind(_), BindBorrowType::BorrowMut)
                    | (BindUsage::Bind(_), BindBorrowType::Take)
                    | (BindUsage::Bind(_), BindBorrowType::TakeRef)
                    | (BindUsage::Bind(_), BindBorrowType::TakeMut)
                    | (BindUsage::Executable, BindBorrowType::Take)
                    | (BindUsage::Executable, BindBorrowType::TakeRef)
                    | (BindUsage::Executable, BindBorrowType::TakeMut) => {}
                    // [ref:borrow_is_indefinite_for_executable]
                    (BindUsage::Executable, BindBorrowType::Borrow)
                    | (BindUsage::Executable, BindBorrowType::BorrowMut) => {
                        unreachable!()
                    }
                }

                // Count indefinite borrows
                match bind_users[i].1 {
                    BindBorrowType::Borrow | BindBorrowType::BorrowMut => {}
                    BindBorrowType::TakeRef => {
                        num_indefinite_shared += 1;
                    }
                    BindBorrowType::Take | BindBorrowType::TakeMut => {
                        num_indefinite_exclusive += 1;
                    }
                }

                // Collect dependencies in the reverse direction
                if let (BindUsage::Bind(user_bind_i), borrow_type) = bind_users[i] {
                    let use_i = temp_uses.len();
                    let other_bind_first_use_i = &mut temp_binds1[user_bind_i].first_use_i;
                    temp_uses.push(SorterUseInfo {
                        bind_i,
                        borrow_type,
                        next_use_i: *other_bind_first_use_i,
                    });
                    *other_bind_first_use_i = Some(use_i);
                }

                i += 1;
            }

            temp_binds1[bind_i].borrowed_indefinitely =
                match (num_indefinite_shared, num_indefinite_exclusive) {
                    (0, 0) => None,
                    (_, 0) => Some(false),
                    (0, 1) => Some(true),
                    _ => {
                        // [ref:bind_conflicting_take]
                        cb.report_error(SorterError::ConflictingIndefiniteBorrow { bind_i });
                        Some(false)
                    }
                };

            bind_i += 1;
        }
    }

    // Helper types needed for topological sorting. `Vertex` is defined outside
    // so that the caller can provide a storage for `Vertex`.
    impl Vertex {
        const fn discriminant(&self) -> usize {
            match self {
                Vertex::BindInit(_) => 0,
                Vertex::BindDisown(_) => 1,
                Vertex::Executable => 2,
            }
        }

        const fn lt(&self, rhs: &Vertex) -> bool {
            match (*self, *rhs) {
                (Vertex::BindInit(lhs_bind_i), Vertex::BindInit(rhs_bind_i)) => {
                    lhs_bind_i < rhs_bind_i
                }
                (Vertex::BindDisown(lhs_bind_i), Vertex::BindDisown(rhs_bind_i)) => {
                    lhs_bind_i < rhs_bind_i
                }
                (Vertex::Executable, Vertex::Executable) => false,
                _ => self.discriminant() < rhs.discriminant(),
            }
        }
    }

    /// Represents a binding initialization graph.
    struct Graph<'a, Callback> {
        temp_binds1: &'a [SorterBindInfo1],
        cb: &'a RefCell<&'a mut Callback>,
        temp_uses: &'a [SorterUseInfo],
    }

    impl<'a, Callback> const GraphAccess<Vertex> for Graph<'a, Callback>
    where
        Callback: ~const SorterCallback,
    {
        type VertexIter<'b>
        where
            Self: 'b,
        = VertexIter<'a>;

        fn vertices(&self) -> Self::VertexIter<'_> {
            VertexIter {
                temp_binds1: self.temp_binds1,
                st: VertexIterState::BindInit(0),
            }
        }

        type SuccessorIter<'b>
        where
            Self: 'b,
        = SuccessorIter<'a, Callback>;

        fn successors(&self, v: &Vertex) -> Self::SuccessorIter<'_> {
            let st = match *v {
                Vertex::Executable => SuccessorIterState::End, // ∅
                Vertex::BindInit(bind_i) => SuccessorIterState::BindInitToBorrowingUser(bind_i, 0),
                Vertex::BindDisown(bind_i) => {
                    if self.temp_binds1[bind_i].borrowed_indefinitely.is_some() {
                        SuccessorIterState::BindDisownToTakingUser(bind_i, 0)
                    } else {
                        SuccessorIterState::End // ∅
                    }
                }
            };

            SuccessorIter {
                temp_binds1: self.temp_binds1,
                cb: self.cb,
                temp_uses: self.temp_uses,
                st,
            }
        }
    }

    /// An iterator over all vertices in the binding initialization graph
    /// [ref:bind_topo].
    struct VertexIter<'a> {
        temp_binds1: &'a [SorterBindInfo1],
        st: VertexIterState,
    }

    enum VertexIterState {
        BindInit(usize),
        BindDisown(usize),
        Executable,
        End,
    }

    impl const MyIterator for VertexIter<'_> {
        type Item = Vertex;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                match self.st {
                    VertexIterState::BindInit(bind_i) if bind_i >= self.temp_binds1.len() => {
                        self.st = VertexIterState::Executable;
                    }
                    VertexIterState::BindInit(bind_i) => {
                        self.st = VertexIterState::BindDisown(bind_i);
                        return Some(Vertex::BindInit(bind_i));
                    }
                    VertexIterState::BindDisown(bind_i) => {
                        self.st = VertexIterState::BindInit(bind_i + 1);
                        // This vertex is included conditionally
                        // ([ref:bind_topo_vert_disown])
                        if self.temp_binds1[bind_i].borrowed_indefinitely.is_some() {
                            return Some(Vertex::BindDisown(bind_i));
                        }
                    }
                    VertexIterState::Executable => {
                        self.st = VertexIterState::End;
                        return Some(Vertex::Executable);
                    }
                    VertexIterState::End => return None,
                }
            }
        }
    }

    /// An iterator over the successors of a specific vertex in the binding
    /// initialization graph [ref:bind_topo]. (I.e., `{d | (s, d) ∈ E}` for a
    /// given `s`)
    struct SuccessorIter<'a, Callback> {
        temp_binds1: &'a [SorterBindInfo1],
        cb: &'a RefCell<&'a mut Callback>,
        temp_uses: &'a [SorterUseInfo],
        st: SuccessorIterState,
    }

    enum SuccessorIterState {
        BindInitToBorrowingUser(usize, usize),
        BindInitToDisown(usize),
        BindInitToDependencyDisown(usize, Option<usize>),

        BindDisownToTakingUser(usize, usize),

        End,
    }

    impl<Callback> const MyIterator for SuccessorIter<'_, Callback>
    where
        Callback: ~const SorterCallback,
    {
        type Item = Vertex;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                match self.st {
                    // [ref:bind_topo_edge_borrow] `(v(Bind(bind_i)), usage)`
                    SuccessorIterState::BindInitToBorrowingUser(bind_i, bind_user_i) => {
                        let cb = self.cb.borrow();
                        let bind_users = cb.bind_users(bind_i);
                        // FIXME: `[T]::get` is not `const fn` yet
                        if bind_user_i < bind_users.len() {
                            self.st = SuccessorIterState::BindInitToBorrowingUser(
                                bind_i,
                                bind_user_i + 1,
                            );
                            if matches!(
                                bind_users[bind_user_i].1,
                                BindBorrowType::Borrow | BindBorrowType::BorrowMut
                            ) {
                                return Some(match bind_users[bind_user_i].0 {
                                    BindUsage::Executable => Vertex::Executable,
                                    BindUsage::Bind(user_bind_i) => Vertex::BindInit(user_bind_i),
                                });
                            }
                        } else {
                            self.st = SuccessorIterState::BindInitToDisown(bind_i);
                        }
                    }
                    // [ref:bind_topo_edge_disown] `(v(Bind(bind_i)), vd(bind_i))`
                    SuccessorIterState::BindInitToDisown(bind_i) => {
                        self.st = SuccessorIterState::BindInitToDependencyDisown(bind_i, None);
                        if self.temp_binds1[bind_i].borrowed_indefinitely.is_some() {
                            return Some(Vertex::BindDisown(bind_i));
                        }
                    }
                    // [ref:bind_topo_edge_borrow_any_vs_take_mut] `(v(Bind(bind_i)), vd(usage.bind_i))`
                    // [ref:bind_topo_edge_borrow_mut_vs_take_ref] `(v(Bind(bind_i)), vd(usage.bind_i))`
                    SuccessorIterState::BindInitToDependencyDisown(bind_i, Some(use_i)) => {
                        let usage = &self.temp_uses[use_i];
                        self.st = SuccessorIterState::BindInitToDependencyDisown(
                            bind_i,
                            usage.next_use_i,
                        );
                        match (
                            usage.borrow_type,
                            self.temp_binds1[usage.bind_i].borrowed_indefinitely,
                        ) {
                            // [ref:bind_topo_edge_borrow_any_vs_take_mut]
                            | (BindBorrowType::Borrow | BindBorrowType::BorrowMut, Some(true))
                            // [ref:bind_topo_edge_borrow_mut_vs_take_ref]
                            | (BindBorrowType::BorrowMut, Some(false))
                            => {
                                return Some(Vertex::BindDisown(usage.bind_i));
                            }
                            _ => {}
                        }
                    }
                    SuccessorIterState::BindInitToDependencyDisown(_, None) => {
                        self.st = SuccessorIterState::End;
                    }

                    // [ref:bind_topo_edge_take] `(vd(bind_i), usage)`
                    SuccessorIterState::BindDisownToTakingUser(bind_i, bind_user_i) => {
                        let cb = self.cb.borrow();
                        let bind_users = cb.bind_users(bind_i);
                        // FIXME: `[T]::get` is not `const fn` yet
                        if bind_user_i < bind_users.len() {
                            self.st =
                                SuccessorIterState::BindDisownToTakingUser(bind_i, bind_user_i + 1);
                            if matches!(
                                bind_users[bind_user_i].1,
                                BindBorrowType::Take
                                    | BindBorrowType::TakeRef
                                    | BindBorrowType::TakeMut
                            ) {
                                return Some(match bind_users[bind_user_i].0 {
                                    BindUsage::Executable => Vertex::Executable,
                                    BindUsage::Bind(user_bind_i) => Vertex::BindInit(user_bind_i),
                                });
                            }
                        } else {
                            self.st = SuccessorIterState::End;
                        }
                    }

                    SuccessorIterState::End => return None,
                }
            }
        }
    }

    struct MyTopologicalSortOutputSink<'a, Callback> {
        cb: &'a RefCell<&'a mut Callback>,
    }

    impl<Callback> const TopologicalSortOutputSink<Vertex> for MyTopologicalSortOutputSink<'_, Callback>
    where
        Callback: ~const SorterCallback,
    {
        fn push(&mut self, v: Vertex) {
            if let Vertex::BindInit(bind_i) = v {
                self.cb.borrow_mut().push_bind_order(bind_i)
            }
        }
    }

    struct MyVertexInfoMap<'a> {
        executable_info: TopologicalSortVertexInfo,
        temp_binds2: &'a mut [SorterBindInfo2],
    }

    impl<'a> const Index<&'a Vertex> for MyVertexInfoMap<'_> {
        type Output = TopologicalSortVertexInfo;

        fn index(&self, index: &'a Vertex) -> &Self::Output {
            match *index {
                Vertex::Executable => &self.executable_info,
                Vertex::BindInit(bind_i) => &self.temp_binds2[bind_i].temp_sort[0],
                Vertex::BindDisown(bind_i) => &self.temp_binds2[bind_i].temp_sort[1],
            }
        }
    }

    impl<'a> const IndexMut<&'a Vertex> for MyVertexInfoMap<'_> {
        fn index_mut(&mut self, index: &'a Vertex) -> &mut Self::Output {
            match *index {
                Vertex::Executable => &mut self.executable_info,
                Vertex::BindInit(bind_i) => &mut self.temp_binds2[bind_i].temp_sort[0],
                Vertex::BindDisown(bind_i) => &mut self.temp_binds2[bind_i].temp_sort[1],
            }
        }
    }

    // Perform topological sorting
    let cb = RefCell::new(cb);
    if topological_sort(
        &Graph {
            temp_binds1,
            cb: &cb,
            temp_uses,
        },
        &mut Vertex::lt,
        &mut MyTopologicalSortOutputSink { cb: &cb },
        temp_vertices,
        &mut MyVertexInfoMap {
            executable_info: Init::INIT,
            temp_binds2,
        },
    ) {
        // Success! The result has been returned via
        // `MyTopologicalSortOutputSink`.
        return;
    }

    // TODO: Report cycles
    cb.borrow_mut()
        .report_error(SorterError::BindCycle { bind_is: &[] });
} // fn sort_bindings

// Helper traits
// --------------------------------------------------------------------------

// FIXME: `Iterator` is currently very hard to implement `const`-ly
/// An [`Iterator`][] usable in `const fn`.
trait MyIterator {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}

trait GraphAccess<VertexRef> {
    type VertexIter<'a>: ~const MyIterator<Item = VertexRef> + ~const Drop + 'a
    where
        Self: 'a;
    fn vertices(&self) -> Self::VertexIter<'_>;

    type SuccessorIter<'a>: ~const MyIterator<Item = VertexRef> + ~const Drop + 'a
    where
        Self: 'a;
    fn successors(&self, v: &VertexRef) -> Self::SuccessorIter<'_>;
}

// Topological sorting
// --------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct TopologicalSortVertexInfo {
    num_predecessors: usize,
}

impl Init for TopologicalSortVertexInfo {
    const INIT: Self = Self {
        num_predecessors: 0,
    };
}

trait TopologicalSortOutputSink<VertexRef> {
    fn push(&mut self, v: VertexRef);
}

/// Perform topological sorting using Kahn's algorithm¹.
///
/// When faced with ambiguous choices, this function uses `vertex_ord_lt` to
/// give some vertices preference over others. If the answer in which all
/// vertices are sorted in the ascending order according to `vertex_ord_lt` is a
/// valid topological ordering, this function will output that answer.
///
/// Returns `true` if a valid topological ordering exists and has been placed
/// in `out_vertices`.
///
/// ¹ Kahn, Arthur B. (1962), "Topological sorting of large networks",
/// *Communications of the ACM*, **5** (11): 558–562, doi:10.1145/368996.369025
const fn topological_sort<
    'a,
    Graph,
    VertexRef,
    VertexRefLessThan,
    ReadyVertexQueue,
    VertexInfoMap,
    OutputSink,
>(
    graph: &'a Graph,
    vertex_ord_lt: &mut VertexRefLessThan,
    out_vertices: &mut OutputSink,
    temp_ready_vertex_queue: &mut ReadyVertexQueue,
    temp_vertex_info: &mut VertexInfoMap,
) -> bool
where
    Graph: ~const GraphAccess<VertexRef>,
    // FIXME: The compiler ignores `~const` in the associated type definition?
    Graph::VertexIter<'a>: ~const MyIterator + ~const Drop,
    // FIXME: The compiler ignores `~const` in the associated type definition?
    Graph::SuccessorIter<'a>: ~const MyIterator + ~const Drop,
    VertexRef: Copy,
    VertexRefLessThan: ~const FnMut(&VertexRef, &VertexRef) -> bool,
    // FIXME: `~const Deref[Mut]` isn't implied because of
    // [ref:veclike_const_supertrait]
    ReadyVertexQueue: ~const VecLike<Element = VertexRef> + ~const Deref + ~const DerefMut,
    for<'index> VertexInfoMap: ~const Index<&'index VertexRef, Output = TopologicalSortVertexInfo>
        + ~const IndexMut<&'index VertexRef>,
    OutputSink: ~const TopologicalSortOutputSink<VertexRef>,
{
    // Preconditions
    assert!(temp_ready_vertex_queue.is_empty());

    struct ReadyVertexQueueBinaryHeapCtx<'a, VertexRefLessThan> {
        vertex_ord_lt: &'a mut VertexRefLessThan,
    }

    impl<VertexRefLessThan, VertexRef> const BinaryHeapCtx<VertexRef>
        for ReadyVertexQueueBinaryHeapCtx<'_, VertexRefLessThan>
    where
        VertexRefLessThan: ~const FnMut(&VertexRef, &VertexRef) -> bool,
    {
        fn lt(&mut self, x: &VertexRef, y: &VertexRef) -> bool {
            (self.vertex_ord_lt)(x, y)
        }
    }

    // Calculate `TopologicalSortVertexInfo::num_predecessors`
    let mut num_vertices_remaining = 0;
    {
        let mut it_vertices = graph.vertices();
        while let Some(v) = it_vertices.next() {
            temp_vertex_info[&v] = TopologicalSortVertexInfo {
                num_predecessors: 0,
            };
            num_vertices_remaining += 1;
        }
    }
    {
        let mut it_vertices = graph.vertices();
        while let Some(v) = it_vertices.next() {
            let mut it_successors = graph.successors(&v);
            while let Some(successor) = it_successors.next() {
                temp_vertex_info[&successor].num_predecessors += 1;
            }
        }
    }

    // Push predecessor-less vertices to `temp_ready_vertex_queue`
    {
        let mut it_vertices = graph.vertices();
        while let Some(v) = it_vertices.next() {
            if temp_vertex_info[&v].num_predecessors == 0 {
                temp_ready_vertex_queue
                    .heap_push(v, ReadyVertexQueueBinaryHeapCtx { vertex_ord_lt });
            }
        }
    }

    while let Some(v) =
        temp_ready_vertex_queue.heap_pop(ReadyVertexQueueBinaryHeapCtx { vertex_ord_lt })
    {
        // Remove `v` from the graph, and push the now-predecessor-less
        // vertices to `temp_ready_vertex_queue`
        let mut it_successors = graph.successors(&v);
        while let Some(successor) = it_successors.next() {
            temp_vertex_info[&successor].num_predecessors -= 1;
            if temp_vertex_info[&successor].num_predecessors == 0 {
                temp_ready_vertex_queue
                    .heap_push(successor, ReadyVertexQueueBinaryHeapCtx { vertex_ord_lt });
            }
        }

        // Append `v` to the output list
        out_vertices.push(v);
        num_vertices_remaining -= 1;
    }

    // Success?
    num_vertices_remaining == 0
}
