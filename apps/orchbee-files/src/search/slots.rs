//! The rg processes this process runs at once. Text searches and file listings share 4 slots; a
//! run waits for one before it starts rg and gives it back when rg has ended, and a run whose
//! search is cancelled stops waiting.

use std::{
    sync::{Condvar, Mutex, PoisonError},
    time::Duration,
};

/// The rg processes that may run at once.
const SLOTS: usize = 4;
/// The longest a waiter sleeps before it looks at its cancellation again.
const WAIT_SLICE: Duration = Duration::from_millis(10);

/// The slots of one process. A panic ends the process, so a poisoned lock is never observed.
pub(super) struct Slots {
    taken: Mutex<usize>,
    freed: Condvar,
}

/// One taken slot, given back when dropped.
pub(super) struct Slot<'a> {
    slots: &'a Slots,
}

impl Slots {
    pub(super) const fn new() -> Self {
        Self {
            taken: Mutex::new(0),
            freed: Condvar::new(),
        }
    }

    /// A slot once one is free; `None` once `cancelled` holds.
    pub(super) fn acquire(&self, cancelled: impl Fn() -> bool) -> Option<Slot<'_>> {
        let mut taken = self.taken.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if cancelled() {
                return None;
            }
            if *taken < SLOTS {
                *taken += 1;
                return Some(Slot { slots: self });
            }
            taken = self
                .freed
                .wait_timeout(taken, WAIT_SLICE)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        let mut taken = self
            .slots
            .taken
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *taken -= 1;
        drop(taken);
        self.slots.freed.notify_one();
    }
}
