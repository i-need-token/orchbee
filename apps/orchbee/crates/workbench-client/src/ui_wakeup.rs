//! Coalesce worker notifications for the single UI waiter without owning input,
//! payloads, or acknowledgements. Mailbox state remains the delivery authority.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, Thread};

/// A notification for the thread that created it. A signal survives the race
/// between checking for work and parking; repeated signals may coalesce.
#[derive(Clone)]
pub struct UiWakeup {
    pending: Arc<AtomicBool>,
    waiter: Thread,
}

impl UiWakeup {
    /// Bind this wakeup to the current UI thread, never to a connection worker.
    pub fn current() -> Self {
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            waiter: thread::current(),
        }
    }

    /// Notify the owning UI loop that an already-derived local redraw is pending.
    pub fn signal(&self) {
        self.pending.store(true, Ordering::Release);
        self.waiter.unpark();
    }

    /// Consume only the notification, before inspecting the mailbox. Work
    /// published after this exchange leaves another notification pending.
    pub fn take(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }
}

#[cfg(test)]
mod tests;
