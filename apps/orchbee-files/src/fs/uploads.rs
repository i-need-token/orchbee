//! Upload transfers: an operation whose data follows as Puts opens one under the transfer the
//! Client chose, and each Put appends its bytes where the previous one ended and answers once they
//! are staged. The Put flagged `commit` must complete the declared size; the operation then takes
//! the data and answers its own call. A Put that skips or overlaps data, passes the declared size,
//! ends the data without committing, or commits before the data is complete is `invalid-request`,
//! and so is the operation's call, whose upload it ends. The end of the call's lane or connection,
//! or the drain, drops the upload with its call, and an upload that receives nothing for 60 s ends
//! with `timeout`. Uploads stage in memory, at most 64 MiB in all, reserved by their declared size
//! when they open: a larger size is `too-large`, and one that does not fit beside the open uploads
//! closes as over capacity.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    CallClose, ConnectionId, ErrorCode, FilesError, FilesReply, LaneId, Put, PutEnd, TransferId,
};

use super::timer::Timer;
use crate::calls::Call;

/// The most data the open uploads stage, in bytes.
const STAGED_BYTES: usize = 64 * 1024 * 1024;
/// How long an upload waits for its next Put.
const IDLE: Duration = Duration::from_secs(60);

/// What the operation does with its complete data; it answers the call.
type Commit = Box<dyn FnOnce(Call, Vec<u8>) + Send>;

type Key = (ConnectionId, TransferId);

/// The open uploads of every connection.
pub(crate) struct Uploads {
    shared: Arc<Mutex<State>>,
    timer: Arc<Timer>,
    idle: Duration,
}

/// The lock is never held while a call is answered or dropped. A panic ends the process, so a
/// poisoned lock is never observed.
#[derive(Default)]
struct State {
    open: HashMap<Key, Upload>,
    /// The declared sizes of the open uploads.
    reserved: usize,
    next: u64,
}

struct Upload {
    lane: LaneId,
    /// Tells this upload from a later one under the same transfer.
    instance: u64,
    size: usize,
    data: Vec<u8>,
    touched: Instant,
    call: Call,
    commit: Commit,
}

impl Uploads {
    /// No uploads yet; `timer` ends idle ones.
    pub(crate) fn new(timer: Arc<Timer>) -> Self {
        Self::with_idle(timer, IDLE)
    }

    fn with_idle(timer: Arc<Timer>, idle: Duration) -> Self {
        Self {
            shared: Arc::default(),
            timer,
            idle,
        }
    }

    /// Opens upload `transfer` of `call`'s connection on its lane for `size` bytes. `commit`
    /// receives the call and the data once the Put flagged `commit` completes it; a refusal
    /// answers the call at once.
    pub(crate) fn open(
        &self,
        call: Call,
        transfer: TransferId,
        size: u64,
        commit: impl FnOnce(Call, Vec<u8>) + Send + 'static,
    ) {
        let size = match usize::try_from(size) {
            Ok(size) if size <= STAGED_BYTES => size,
            _ => return refuse(call, ErrorCode::TooLarge),
        };
        let key = (call.connection(), transfer);
        let mut state = lock(&self.shared);
        if state.open.contains_key(&key) {
            drop(state);
            return refuse(call, ErrorCode::InvalidRequest);
        }
        let mut data = Vec::new();
        if state.reserved + size > STAGED_BYTES || data.try_reserve_exact(size).is_err() {
            drop(state);
            return call.close(CallClose::Capacity);
        }
        state.reserved += size;
        state.next += 1;
        let instance = state.next;
        let touched = Instant::now();
        state.open.insert(
            key,
            Upload {
                lane: call.lane(),
                instance,
                size,
                data,
                touched,
                call,
                commit: Box::new(commit),
            },
        );
        drop(state);
        self.expire_after(key, instance, touched);
    }

    /// Stages `put` in its upload and answers `call`, the Put's own call.
    pub(crate) fn put(&self, call: Call, put: Put) {
        let key = (call.connection(), put.transfer);
        let mut state = lock(&self.shared);
        let Some(upload) = state.open.get_mut(&key) else {
            drop(state);
            return refuse(call, ErrorCode::InvalidRequest);
        };
        let staged = upload.data.len();
        let reached = staged.checked_add(put.bytes.len());
        let fits = put.offset == staged as u64 && reached.is_some_and(|end| end <= upload.size);
        let admitted = fits
            && match put.end {
                PutEnd::Continues => true,
                // No operation of this build takes data that ends without committing.
                PutEnd::Last => false,
                PutEnd::Commit => reached == Some(upload.size),
            };
        if !admitted {
            let ended = remove(&mut state, &key);
            drop(state);
            refuse(call, ErrorCode::InvalidRequest);
            if let Some(upload) = ended {
                refuse(upload.call, ErrorCode::InvalidRequest);
            }
            return;
        }
        upload.data.extend_from_slice(&put.bytes);
        if put.end == PutEnd::Commit {
            let committed = remove(&mut state, &key);
            drop(state);
            if let Some(upload) = committed {
                (upload.commit)(upload.call, upload.data);
            }
        } else {
            let touched = Instant::now();
            upload.touched = touched;
            let instance = upload.instance;
            drop(state);
            self.expire_after(key, instance, touched);
        }
        answer(call, &FilesReply::Empty);
    }

    /// Drops the uploads whose calls `lane` of `connection` carried.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.remove_where(|key, upload| key.0 == connection && upload.lane == lane);
    }

    /// Drops the uploads of `connection`.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.remove_where(|key, _| key.0 == connection);
    }

    /// Drops every upload.
    pub(crate) fn drain(&self) {
        self.remove_where(|_, _| true);
    }

    /// Drops the uploads `matches` picks; their calls close as the link event left them.
    fn remove_where(&self, matches: impl Fn(&Key, &Upload) -> bool) {
        let mut state = lock(&self.shared);
        let keys = state
            .open
            .iter()
            .filter(|(key, upload)| matches(key, upload))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        let removed = keys
            .iter()
            .filter_map(|key| remove(&mut state, key))
            .collect::<Vec<_>>();
        drop(state);
        drop(removed);
    }

    /// Ends upload `key` with `timeout` unless a Put reached it after `touched`.
    fn expire_after(&self, key: Key, instance: u64, touched: Instant) {
        let shared = Arc::downgrade(&self.shared);
        // Without a timer thread an upload lives until it commits or its call's lane ends.
        let _ = self.timer.schedule(touched + self.idle, move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let mut state = lock(&shared);
            let idle = state
                .open
                .get(&key)
                .is_some_and(|upload| upload.instance == instance && upload.touched == touched);
            let expired = if idle { remove(&mut state, &key) } else { None };
            drop(state);
            if let Some(upload) = expired {
                refuse(upload.call, ErrorCode::Timeout);
            }
        });
    }
}

/// Removes upload `key` and releases its reservation.
fn remove(state: &mut State, key: &Key) -> Option<Upload> {
    let upload = state.open.remove(key)?;
    state.reserved -= upload.size;
    Some(upload)
}

fn refuse(call: Call, code: ErrorCode) {
    answer(call, &FilesReply::Error(FilesError::new(code)));
}

fn answer(call: Call, reply: &FilesReply) {
    if call.cancelled() {
        // Dropping a cancelled call closes it as cancelled.
        drop(call);
    } else {
        call.reply(reply);
    }
}

fn lock(shared: &Mutex<State>) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
