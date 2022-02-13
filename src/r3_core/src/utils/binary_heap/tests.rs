use super::*;
use arrayvec::ArrayVec;
use quickcheck_macros::quickcheck;

/// A modifying operation on `BinaryHeap`.
#[derive(Debug)]
enum Cmd {
    Insert(usize),
    Remove(usize),
}

/// Map random bytes to operations on `BinaryHeap`.
fn interpret(bytecode: &[u8], max_len: usize) -> impl Iterator<Item = Cmd> + '_ {
    let mut i = 0;
    let mut len = 0;
    std::iter::from_fn(move || {
        if let Some(instr) = bytecode.get(i..i + 5) {
            i += 5;

            let value = u32::from_le_bytes([instr[1], instr[2], instr[3], instr[4]]) as usize;

            if (instr[0] % 2 == 0 && len != max_len) || len == 0 {
                len += 1;
                Some(Cmd::Insert(value))
            } else {
                len -= 1;
                Some(Cmd::Remove(value % (len + 1)))
            }
        } else {
            None
        }
    })
}

struct Ctx;

impl BinaryHeapCtx<usize> for Ctx {
    fn lt(&mut self, x: &usize, y: &usize) -> bool {
        *x < *y
    }
}

fn test_inner<T: BinaryHeap + Default + super::VecLike<Element = usize> + std::fmt::Debug>(
    bytecode: Vec<u8>,
    max_len: usize,
) {
    let mut subject = T::default();
    let mut reference = Vec::new();

    log::debug!("max_len = {}, bytecode len = {}", max_len, bytecode.len());

    for cmd in interpret(&bytecode, max_len) {
        log::trace!("    {:?}", cmd);
        match cmd {
            Cmd::Insert(value) => {
                let i = subject.heap_push(value, Ctx);
                log::trace!("     → {}", i);

                let i = reference.binary_search(&value).unwrap_or_else(|x| x);
                reference.insert(i, value);
            }
            Cmd::Remove(i) => {
                let out_subject = subject.heap_remove(i, Ctx).unwrap();
                log::trace!("     → {}", out_subject);

                let i_ref = reference.binary_search(&out_subject).unwrap();
                reference.remove(i_ref);
            }
        }
        log::trace!("[sorted: {:?}]", reference);
        log::trace!("[subject: {:?}]", subject);
        if subject.len() > 0 {
            assert_eq!(subject[0], reference[0]);
        }
    }
}

#[quickcheck]
fn test_arrayvec_4(bytecode: Vec<u8>) {
    test_inner::<ArrayVec<usize, 4>>(bytecode, 4);
}

#[quickcheck]
fn test_arrayvec_256(bytecode: Vec<u8>) {
    test_inner::<ArrayVec<usize, 256>>(bytecode, 256);
}

#[quickcheck]
fn test_vec(bytecode: Vec<u8>) {
    test_inner::<Vec<usize>>(bytecode, usize::MAX);
}

#[test]
fn test1() {
    let _ = env_logger::builder().is_test(true).try_init();

    test_inner::<Vec<usize>>(
        vec![
            0, 0, 0, 0, 0, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 15, 47, 50, 0, 50, 15, 0, 0, 0, 0, 63,
            13, 48, 32, 72,
        ],
        usize::MAX,
    );
}

#[derive(Debug, Clone, Copy)]
struct El {
    value: usize,
    id: usize,
}

struct TrackingCtx<'a> {
    el_position: &'a mut [Option<usize>],
}

impl BinaryHeapCtx<El> for TrackingCtx<'_> {
    fn lt(&mut self, x: &El, y: &El) -> bool {
        x.value < y.value
    }

    fn on_move(&mut self, e: &mut El, new_index: usize) {
        self.el_position[e.id] = Some(new_index);
        log::trace!("         on_move{:?}", (e, new_index));
    }
}

#[quickcheck]
fn position_tracking(bytecode: Vec<u8>) {
    // Expected Invariant: `subject[el_position[i]].id == i`
    let mut el_position: Vec<Option<usize>> = Vec::new();
    let el_position = &mut el_position;

    let mut subject: Vec<El> = Vec::new(); // : `BinaryHeap`

    log::debug!("bytecode len = {}", bytecode.len());

    for cmd in interpret(&bytecode, usize::MAX) {
        log::trace!("    {:?}", cmd);
        match cmd {
            Cmd::Insert(value) => {
                let id = el_position.len();
                el_position.push(None);
                let i = subject.heap_push(El { value, id }, TrackingCtx { el_position });
                log::trace!("     → {}", i);

                // `on_move` should have reported the position for the
                // newly-inserted element
                assert_eq!(el_position[id], Some(i));
            }
            Cmd::Remove(i) => {
                let out_subject = subject.heap_remove(i, TrackingCtx { el_position }).unwrap();
                log::trace!("     → {:?}", out_subject);

                // For a removed element, we must modify `el_position` manually
                el_position[out_subject.id] = None;
            }
        }

        log::trace!("[subject: {:?}]", subject);
        log::trace!("[el_position: {:?}]", el_position);

        // Check if `el_position` correctly represents
        // the current state of `subject`
        for (id, &pos) in el_position.iter().enumerate() {
            if let Some(pos) = pos {
                assert_eq!(subject[pos].id, id);
            }
        }
    }
}
