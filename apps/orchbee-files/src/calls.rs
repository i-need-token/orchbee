//! The calls the Server relays to this process and the handles their operations end them
//! through. A call ends exactly once: by a close, or when its handle is dropped, which closes it
//! as cancelled after its Client's cancel and as outcome-unknown otherwise, so a Client stream
//! never waits on a handle nothing holds. A request call ends with its one Result record; an
//! Events call streams a Snapshot record, then Update records, until it ends.

use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use orchbee_files_protocol::{
    CallClose, CallId, Chunk, ConnectionId, FilesNotice, FilesRecord, FilesReply, FrameKind,
    LaneId, LinkFrame, LinkMessage, encode_files_record, encode_link_message, encode_notice,
    encode_reply,
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
            connection,
            lane,
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

    /// Queues one reply record of call `id`.
    ///
    /// # Panics
    ///
    /// For a record over its bound, which the operation that produced it keeps within.
    fn queue_frame(&self, id: CallId, kind: FrameKind, record: &FilesRecord) {
        let record = encode_files_record(record).expect("an operation bounds its records");
        let frame = encode_link_message(&LinkMessage::Frame(LinkFrame {
            call: id,
            kind,
            record,
        }))
        .expect("a bounded record fits one frame");
        self.outbound.push(frame);
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
    connection: ConnectionId,
    lane: LaneId,
    calls: Arc<Calls>,
    cancelled: Arc<AtomicBool>,
    ended: bool,
}

impl Call {
    /// The connection that made the call.
    pub(crate) const fn connection(&self) -> ConnectionId {
        self.connection
    }

    /// The lane the call arrived on.
    pub(crate) const fn lane(&self) -> LaneId {
        self.lane
    }

    /// Whether the call's Client cancelled it, its lane or connection closed, or the process is
    /// draining.
    pub(crate) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// The flag [`Self::cancelled`] reads, for work that checks it without the call.
    pub(crate) fn cancellation(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    /// Ends the call without a reply record.
    pub(crate) fn close(mut self, reason: CallClose) {
        self.finish(reason);
    }

    /// Ends a request call with its Result record.
    ///
    /// # Panics
    ///
    /// For a reply over the Result record's bound, which the operation keeps within.
    pub(crate) fn reply(mut self, reply: &FilesReply) {
        let record = FilesRecord::Result(encode_reply(reply));
        self.calls.queue_frame(self.id, FrameKind::Result, &record);
        self.finish(CallClose::Complete);
    }

    /// Ends a read's chunk request with its Chunk record.
    ///
    /// # Panics
    ///
    /// For data over the Chunk record's bound, which the read keeps within.
    pub(crate) fn chunk(mut self, chunk: Chunk) {
        let record = FilesRecord::Chunk(chunk);
        self.calls.queue_frame(self.id, FrameKind::Result, &record);
        self.finish(CallClose::Complete);
    }

    /// Starts an Events call's stream with its Snapshot record.
    ///
    /// # Panics
    ///
    /// As [`Events::update`].
    pub(crate) fn snapshot(self, notice: &FilesNotice) -> Events {
        let events = Events { call: self };
        events.queue(FrameKind::Snapshot, notice);
        events
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
        let reason = if self.cancelled() {
            CallClose::Cancelled
        } else {
            CallClose::OutcomeUnknown
        };
        self.finish(reason);
    }
}

/// An Events call after its Snapshot. It ends when dropped: as cancelled once its Client cancelled
/// it, its lane or connection closed, or the process drained.
pub(crate) struct Events {
    call: Call,
}

impl Events {
    /// The call's identity, which its subscription is known by.
    pub(crate) const fn id(&self) -> CallId {
        self.call.id
    }

    /// The connection that subscribed.
    pub(crate) const fn connection(&self) -> ConnectionId {
        self.call.connection
    }

    /// The lane the subscription arrived on.
    pub(crate) const fn lane(&self) -> LaneId {
        self.call.lane
    }

    /// Queues one Update record.
    ///
    /// # Panics
    ///
    /// For a notice over the Notice record's bound, which its producer keeps within.
    pub(crate) fn update(&self, notice: &FilesNotice) {
        self.queue(FrameKind::Update, notice);
    }

    fn queue(&self, kind: FrameKind, notice: &FilesNotice) {
        let record = FilesRecord::Notice(encode_notice(notice));
        self.call.calls.queue_frame(self.call.id, kind, &record);
    }
}

#[cfg(test)]
mod tests;
