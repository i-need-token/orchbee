//! The calls the Server relays to this process and the handles their operations end them
//! through. A call ends exactly once: by a close, or when its handle is dropped, which closes it
//! as cancelled after its Client's cancel and as outcome-unknown otherwise, so a Client stream
//! never waits on a handle nothing holds.

use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use orchbee_files_protocol::{
    CallClose, CallId, ConnectionId, LaneId, LinkMessage, encode_link_message,
};

use crate::outbound::Outbound;

/// The open calls and the queue their frames leave through. The lock guards the open calls and
/// whether new ones are refused; it is taken before the queue's own lock, never after, so each
/// call's close is queued in the order the registry changed. A panic ends the process, so a
/// poisoned lock is never observed.
pub(crate) struct Calls {
    outbound: Arc<Outbound>,
    state: Mutex<State>,
    settled: Condvar,
}

#[derive(Default)]
struct State {
    open: HashMap<CallId, Entry>,
    draining: bool,
}

struct Entry {
    connection: ConnectionId,
    lane: LaneId,
    cancelled: Arc<AtomicBool>,
}

/// The Server named a call that is already open, which no Server does.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct DuplicateCall;

impl Calls {
    pub(crate) fn new(outbound: Arc<Outbound>) -> Arc<Self> {
        Arc::new(Self {
            outbound,
            state: Mutex::new(State::default()),
            settled: Condvar::new(),
        })
    }

    /// Opens call `id`, which arrived on `lane` of `connection`. A draining process closes it as
    /// stopping at once and returns `None`.
    pub(crate) fn open(
        self: &Arc<Self>,
        id: CallId,
        connection: ConnectionId,
        lane: LaneId,
    ) -> Result<Option<Call>, DuplicateCall> {
        let mut state = self.lock();
        if state.draining {
            self.queue_close(id, CallClose::Stopping);
            return Ok(None);
        }
        if state.open.contains_key(&id) {
            return Err(DuplicateCall);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        state.open.insert(
            id,
            Entry {
                connection,
                lane,
                cancelled: Arc::clone(&cancelled),
            },
        );
        Ok(Some(Call {
            id,
            calls: Arc::clone(self),
            cancelled,
            ended: false,
        }))
    }

    /// The Client cancelled call `id`; a call that already ended is not affected.
    pub(crate) fn cancel(&self, id: CallId) {
        if let Some(entry) = self.lock().open.get(&id) {
            entry.cancelled.store(true, Ordering::Release);
        }
    }

    /// The Client closed `lane`, cancelling every call on it.
    pub(crate) fn cancel_lane(&self, connection: ConnectionId, lane: LaneId) {
        self.cancel_where(|entry| entry.connection == connection && entry.lane == lane);
    }

    /// The Client connection ended, cancelling every call it made.
    pub(crate) fn cancel_connection(&self, connection: ConnectionId) {
        self.cancel_where(|entry| entry.connection == connection);
    }

    /// Refuses every new call and cancels every open one.
    pub(crate) fn drain(&self) {
        self.lock().draining = true;
        self.cancel_where(|_| true);
    }

    /// Waits until every call ended, or until `deadline`; true when all did.
    pub(crate) fn settled_before(&self, deadline: Instant) -> bool {
        let mut state = self.lock();
        while !state.open.is_empty() {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            state = self
                .settled
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }

    fn cancel_where(&self, selected: impl Fn(&Entry) -> bool) {
        for entry in self.lock().open.values().filter(|entry| selected(entry)) {
            entry.cancelled.store(true, Ordering::Release);
        }
    }

    fn end(&self, id: CallId, reason: CallClose) {
        let mut state = self.lock();
        if state.open.remove(&id).is_some() {
            self.queue_close(id, reason);
            if state.open.is_empty() {
                self.settled.notify_all();
            }
        }
    }

    fn queue_close(&self, call: CallId, reason: CallClose) {
        let frame = encode_link_message(&LinkMessage::Close { call, reason })
            .expect("a close always fits one frame");
        self.outbound.push(frame);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One open call, which its operation ends once.
pub(crate) struct Call {
    id: CallId,
    calls: Arc<Calls>,
    cancelled: Arc<AtomicBool>,
    ended: bool,
}

impl Call {
    /// Ends the call without a reply record.
    pub(crate) fn close(mut self, reason: CallClose) {
        self.finish(reason);
    }

    fn finish(&mut self, reason: CallClose) {
        if !self.ended {
            self.ended = true;
            self.calls.end(self.id, reason);
        }
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        let reason = if self.cancelled.load(Ordering::Acquire) {
            CallClose::Cancelled
        } else {
            CallClose::OutcomeUnknown
        };
        self.finish(reason);
    }
}

#[cfg(test)]
mod tests;
