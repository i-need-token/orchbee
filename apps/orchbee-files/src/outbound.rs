//! The frames waiting for the Server, in the order their owners queued them, and the one writer
//! that sends them. An owner never writes to the link itself, so a slow Server delays frames
//! instead of the threads that produce them.
//!
//! The queue keeps 4 MiB. A subscription Update that would pass it collapses its subscription: that
//! Update and every later one fold into one place at the end of the queue until the writer reaches
//! it, as [`collapse`] folds them, so however late the Server reads, a watch sends one rescan and
//! an SCM subscription its latest summary. Replies, closes, Snapshots and relay prompts always join
//! the queue: each answers a Client's request or an operation's prompt, so the Client's window and
//! streams bound them.

mod collapse;

use std::{
    collections::{HashMap, VecDeque},
    sync::{Condvar, Mutex, MutexGuard, PoisonError},
    time::Instant,
};

use orchbee_files_protocol::{
    CallId, FilesNotice, FilesRecord, FrameKind, LinkFrame, LinkMessage, encode_files_record,
    encode_link_message,
};

use self::collapse::Folded;

/// The bytes of queued frames an Update may fill without collapsing its subscription.
const CAPACITY: usize = 4 * 1024 * 1024;

/// The queue the writer drains. Its lock guards the queued frames, their byte count, what each
/// collapsed subscription folded and whether the writer still sends; a panic ends the process, so
/// a poisoned lock is never observed.
pub(crate) struct Outbound {
    capacity: usize,
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Default)]
struct State {
    queued: VecDeque<Queued>,
    /// The length of the frames in `queued`.
    bytes: usize,
    /// What each collapsed subscription folded, until the writer reaches its place.
    collapsed: HashMap<CallId, Folded>,
    /// The writer holds frames it took and has not finished writing.
    writing: bool,
    closed: bool,
}

enum Queued {
    Frame(Vec<u8>),
    /// The place of a collapsed subscription's Updates.
    Collapsed(CallId),
}

impl Outbound {
    pub(crate) fn new() -> Self {
        Self::with_capacity(CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        }
    }

    /// Queues `frame` after every frame queued before it, past the bound if need be; a closed queue
    /// drops it.
    pub(crate) fn push(&self, frame: Vec<u8>) {
        let mut state = self.lock();
        if !state.closed {
            state.bytes += frame.len();
            state.queued.push_back(Queued::Frame(frame));
            self.changed.notify_all();
        }
    }

    /// Queues `update`, encoded as `frame`, of subscription `call`. It joins the queue when it fits
    /// within the bound and the subscription has no place waiting; otherwise it folds into that
    /// place, which the first Update that does not fit opens at the end of the queue. An Update
    /// that never collapses joins as [`Self::push`] queues it.
    pub(crate) fn update(&self, call: CallId, update: &FilesNotice, frame: Vec<u8>) {
        if !collapse::folds(update) {
            self.push(frame);
            return;
        }
        let mut state = self.lock();
        if state.closed {
            return;
        }
        if let Some(folded) = state.collapsed.get_mut(&call) {
            folded.fold(update, frame);
        } else if state.bytes + frame.len() <= self.capacity {
            state.bytes += frame.len();
            state.queued.push_back(Queued::Frame(frame));
            self.changed.notify_all();
        } else {
            let mut folded = Folded::default();
            folded.fold(update, frame);
            state.collapsed.insert(call, folded);
            state.queued.push_back(Queued::Collapsed(call));
            self.changed.notify_all();
        }
    }

    /// The next frames for the writer, waiting for some; `None` once the queue is closed. The
    /// writer calls [`Self::written`] after sending them.
    pub(crate) fn next(&self) -> Option<Vec<u8>> {
        let mut state = self.lock();
        loop {
            if state.closed {
                return None;
            }
            match state.queued.pop_front() {
                Some(Queued::Frame(frame)) => {
                    state.bytes -= frame.len();
                    state.writing = true;
                    return Some(frame);
                }
                Some(Queued::Collapsed(call)) => {
                    // A place keeps what its subscription folded until the writer takes it here.
                    let Some(folded) = state.collapsed.remove(&call) else {
                        continue;
                    };
                    state.writing = true;
                    drop(state);
                    return Some(folded.frames(call));
                }
                None => {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }

    /// The writer finished sending the frames it took.
    pub(crate) fn written(&self) {
        self.lock().writing = false;
        self.changed.notify_all();
    }

    /// Waits until the writer sent every queued frame, or until `deadline`; true when it did.
    pub(crate) fn sent_before(&self, deadline: Instant) -> bool {
        let mut state = self.lock();
        while !state.closed && (state.writing || !state.queued.is_empty()) {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            state = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        !state.writing && state.queued.is_empty()
    }

    /// Stops the writer and drops whatever is still queued.
    pub(crate) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.queued.clear();
        state.collapsed.clear();
        state.bytes = 0;
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The link frame that carries `record` as call `call`'s `kind`.
///
/// # Panics
///
/// For a record over its bound, which every producer keeps within.
pub(crate) fn frame(call: CallId, kind: FrameKind, record: &FilesRecord) -> Vec<u8> {
    let record = encode_files_record(record).expect("an operation bounds its records");
    encode_link_message(&LinkMessage::Frame(LinkFrame { call, kind, record }))
        .expect("a bounded record fits one frame")
}

#[cfg(test)]
mod tests;
