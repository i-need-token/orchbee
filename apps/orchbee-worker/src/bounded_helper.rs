//! Runs a blocking system call that has no bound of its own, such as a filesystem probe or an
//! account lookup, on a helper thread, and waits for it only up to a deadline, so the serving
//! thread keeps reading its peers. A call past its deadline keeps running until the system
//! answers, and its late answer is dropped; a pool caps how many such calls run at once.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

/// The helper threads one kind of call may keep running, stalled ones included.
pub(crate) struct HelperPool {
    running: AtomicUsize,
    limit: usize,
}

impl HelperPool {
    pub(crate) const fn new(limit: usize) -> Self {
        Self {
            running: AtomicUsize::new(0),
            limit,
        }
    }

    /// Runs `call` on a helper thread named `name` and returns its answer if it arrives within
    /// `deadline`. `None` means no answer: the pool was full, the thread could not start, or the
    /// deadline passed.
    pub(crate) fn run<T: Send + 'static>(
        &'static self,
        name: &str,
        deadline: Duration,
        call: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        if self.running.fetch_add(1, Ordering::AcqRel) >= self.limit {
            self.running.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let spawned = thread::Builder::new().name(name.to_owned()).spawn(move || {
            let answer = call();
            self.running.fetch_sub(1, Ordering::AcqRel);
            // A call past its deadline finds the receiver gone; its answer is dropped.
            let _late = sender.send(answer);
        });
        if spawned.is_err() {
            self.running.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        receiver.recv_timeout(deadline).ok()
    }
}
