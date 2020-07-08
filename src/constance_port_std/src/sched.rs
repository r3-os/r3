//! Simulates a hardware scheduler.
use constance::{
    kernel::{cfg::InterruptHandlerFn, InterruptNum, InterruptPriority, Kernel},
    utils::Init,
};
use std::collections::{BTreeSet, HashMap};

use crate::{ums, ThreadRole, NUM_INTERRUPT_LINES, THREAD_ROLE};

/// The state of the simulated hardware scheduler.
pub struct SchedState {
    /// Interrupt lines.
    int_lines: HashMap<InterruptNum, IntLine>,
    /// `int_lines.iter().filter(|_,a| a.pended && a.enable)
    /// .map(|i,a| (a.priority, i)).collect()`.
    pended_lines: BTreeSet<(InterruptPriority, InterruptNum)>,
    active_int_handlers: Vec<(InterruptPriority, ums::ThreadId)>,
    pub cpu_lock: bool,

    /// The currently-selected task thread.
    pub task_thread: Option<ums::ThreadId>,

    /// Garbage can
    zombies: Vec<ums::ThreadId>,
}

/// The configuration of an interrupt line.
#[derive(Debug)]
pub struct IntLine {
    pub priority: InterruptPriority,
    pub start: Option<InterruptHandlerFn>,
    pub enable: bool,
    pub pended: bool,
}

impl Init for IntLine {
    const INIT: Self = IntLine {
        priority: 0,
        start: None,
        enable: false,
        pended: false,
    };
}

pub struct BadIntLineError;

impl SchedState {
    pub fn new<System: Kernel>() -> Self {
        let mut this = Self {
            int_lines: HashMap::new(),
            pended_lines: BTreeSet::new(),
            active_int_handlers: Vec::new(),
            cpu_lock: true,
            task_thread: None,
            zombies: Vec::new(),
        };

        for i in 0..NUM_INTERRUPT_LINES {
            if let Some(handler) = System::INTERRUPT_HANDLERS.get(i) {
                this.int_lines.insert(
                    i as InterruptNum,
                    IntLine {
                        start: Some(handler),
                        ..IntLine::INIT
                    },
                );
            }
        }

        this
    }

    pub fn update_line(
        &mut self,
        i: InterruptNum,
        f: impl FnOnce(&mut IntLine),
    ) -> Result<(), BadIntLineError> {
        if i >= NUM_INTERRUPT_LINES {
            return Err(BadIntLineError);
        }
        let line = self.int_lines.entry(i).or_insert_with(|| IntLine::INIT);
        self.pended_lines.remove(&(line.priority, i));
        f(line);
        if line.enable && line.pended {
            self.pended_lines.insert((line.priority, i));
        }
        Ok(())
    }

    pub fn is_line_pended(&self, i: InterruptNum) -> Result<bool, BadIntLineError> {
        if i >= NUM_INTERRUPT_LINES {
            return Err(BadIntLineError);
        }

        if let Some(line) = self.int_lines.get(&i) {
            Ok(line.pended)
        } else {
            Ok(false)
        }
    }

    /// Schedule the specified thread until it naturally exits.
    pub fn recycle_thread(&mut self, thread_id: ums::ThreadId) {
        self.zombies.push(thread_id);
    }
}

impl ums::Scheduler for SchedState {
    fn choose_next_thread(&mut self) -> Option<ums::ThreadId> {
        if let Some(&thread_id) = self.zombies.first() {
            // Clean up zombie threads as soon as possible
            Some(thread_id)
        } else if let Some(&(_, thread_id)) = self.active_int_handlers.last() {
            Some(thread_id)
        } else if self.cpu_lock {
            // CPU Lock owned by a task thread
            Some(self.task_thread.unwrap())
        } else {
            self.task_thread
        }
    }

    fn thread_exited(&mut self, thread_id: ums::ThreadId) {
        if let Some(i) = self.zombies.iter().position(|id| *id == thread_id) {
            log::trace!("removing the zombie thread {:?}", thread_id);
            self.zombies.swap_remove(i);
            return;
        }

        log::warn!("thread_exited: unexpected thread {:?}", thread_id);
    }
}

/// Check for any pending interrupts that can be activated under the current
/// condition. If there are one or more of them, activate them and return
/// `true`, in which case the caller should call
/// [`ums::ThreadGroupLockGuard::preempt`], [`ums::yield_now`],
/// [`ums::exit_thread`].
///
/// This should be called after changing some properties of `SchedState` in a
/// way that might cause interrupt handlers to activate, such as disabling
/// `cpu_lock`.
#[must_use]
pub fn check_preemption_by_interrupt(
    thread_group: &'static ums::ThreadGroup<SchedState>,
    lock: &mut ums::ThreadGroupLockGuard<SchedState>,
) -> bool {
    let mut activated_any = false;

    // Check pending interrupts
    loop {
        let sched_state = lock.scheduler();

        // Find the highest pended priority
        let (pri, num) = if let Some(&x) = sched_state.pended_lines.iter().next() {
            x
        } else {
            // No interrupt is pended
            break;
        };

        // Masking by CPU Lock
        if sched_state.cpu_lock && is_interrupt_priority_managed(pri) {
            log::trace!(
                "not handling an interrupt with priority {} because of CPU Lock",
                pri
            );
            break;
        }

        // Masking by an already active interrupt
        if let Some(&(existing_pri, _)) = sched_state.active_int_handlers.last() {
            if existing_pri < pri {
                log::trace!(
                    "not handling an interrupt with priority {} because of \
                        an active interrupt handler with priority {}",
                    pri,
                    existing_pri,
                );
                break;
            }
        }

        // Take the interrupt
        sched_state.pended_lines.remove(&(pri, num));

        // Find the interrupt handler for `num`. Return
        // `default_interrupt_handler` if there's none.
        let start = sched_state
            .int_lines
            .get(&num)
            .and_then(|line| line.start)
            .unwrap_or(default_interrupt_handler);

        let thread_id = lock.spawn(move |thread_id| {
            THREAD_ROLE.with(|role| role.set(ThreadRole::Interrupt));

            // Safety: The port can call an interrupt handler
            unsafe { start() }

            let mut lock = thread_group.lock();

            // Make this interrupt handler inactive
            let (_, popped_thread_id) = lock.scheduler().active_int_handlers.pop().unwrap();
            assert_eq!(thread_id, popped_thread_id);
            log::trace!(
                "an interrupt handler for an interrupt {} (priority = {}) exited",
                num,
                pri
            );

            // Make sure this thread will run to completion
            lock.scheduler().zombies.push(thread_id);

            let _ = check_preemption_by_interrupt(thread_group, &mut lock);
        });

        log::trace!(
            "handling an interrupt {} (priority = {}) with thread {:?}",
            num,
            pri,
            thread_id
        );

        lock.scheduler().active_int_handlers.push((pri, thread_id));

        activated_any = true;
    }

    activated_any
}

fn is_interrupt_priority_managed(p: InterruptPriority) -> bool {
    p >= 0
}

extern "C" fn default_interrupt_handler() {
    panic!("Unhandled interrupt");
}
