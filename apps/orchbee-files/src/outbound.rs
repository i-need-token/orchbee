//! The frames waiting for the Server, in the order their owners queued them, and the one writer
//! that sends them. An owner never writes to the link itself, so a slow Server delays frames
//! instead of the threads that produce them.

use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

/// The queue the writer drains. Its lock guards the frames, their byte count and whether the
/// writer still sends; a panic ends the process, so a poisoned lock is never observed.
pub(crate) struct Outbound {
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    frames: VecDeque<Vec<u8>>,
    /// The writer holds a frame it took and has not finished writing.
    writing: bool,
    closed: bool,
}

impl Outbound {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    /// Queues `frame` after every frame queued before it; a closed queue drops it.
    pub(crate) fn push(&self, frame: Vec<u8>) {
        let mut state = self.lock();
        if !state.closed {
            state.frames.push_back(frame);
            self.changed.notify_all();
        }
    }

    /// The next frame for the writer, waiting for one; `None` once the queue is closed. The
    /// writer calls [`Self::written`] after sending it.
    pub(crate) fn next(&self) -> Option<Vec<u8>> {
        let mut state = self.lock();
        loop {
            if state.closed {
                return None;
            }
            if let Some(frame) = state.frames.pop_front() {
                state.writing = true;
                return Some(frame);
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The writer finished sending the frame it took.
    pub(crate) fn written(&self) {
        self.lock().writing = false;
        self.changed.notify_all();
    }

    /// Waits until the writer sent every queued frame, or until `deadline`; true when it did.
    pub(crate) fn sent_before(&self, deadline: Instant) -> bool {
        let mut state = self.lock();
        while !state.closed && (state.writing || !state.frames.is_empty()) {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        !state.writing && state.frames.is_empty()
    }

    /// Stops the writer and drops whatever is still queued.
    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.frames.clear();
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
