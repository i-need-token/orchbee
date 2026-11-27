//! Upload transfers: an operation whose data follows as Puts opens one under the transfer the
//! Client chose, and each Put appends its bytes where the previous one ended and answers once they
//! are staged. The Put flagged `commit` must complete the declared size; the operation then takes
//! the data and answers its own call. A Put that skips or overlaps data, passes the declared size,
//! ends the data without committing, or commits before the data is complete is `invalid-request`,
//! and so is the operation's call, whose upload it ends. An upload that receives nothing for 60 s
//! ends with `timeout`. Uploads stage in memory, at most 64 MiB in all, reserved by their declared
//! size when they open: a larger size is `too-large`, and one that does not fit beside the open
//! uploads closes as over capacity.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use orchbee_files_protocol::{
    CallClose, ErrorCode, FilesError, FilesReply, Put, PutEnd, TransferId,
};

use super::{Key, Kind, Transfer, Transfers, Unopened, lock};
use crate::calls::Call;

/// The most data the open uploads stage, in bytes.
const STAGED_BYTES: usize = 64 * 1024 * 1024;
/// How long an upload waits for its next Put.
pub(super) const IDLE: Duration = Duration::from_secs(60);

/// What the operation does with its complete data; it answers the call.
type Commit = Box<dyn FnOnce(Call, Vec<u8>) + Send>;

/// The data an upload staged so far, with the operation's call.
pub(super) struct Upload {
    /// The declared size, which the open uploads reserve.
    pub(super) size: usize,
    data: Vec<u8>,
    touched: Instant,
    call: Call,
    commit: Commit,
}

impl Upload {
    /// Ends the upload as `transfer.close` asked: the operation's call answers `cancelled`.
    pub(super) fn cancel(self) {
        refuse(self.call, ErrorCode::Cancelled);
    }
}

impl Transfers {
    /// Opens upload `transfer` of `call`'s connection on its lane for `size` bytes. `commit`
    /// receives the call and the data once the Put flagged `commit` completes it; a refusal
    /// answers the call at once.
    pub(crate) fn upload(
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
        let mut table = lock(&self.shared);
        if let Err(refusal) = table.admit(key) {
            drop(table);
            return match refusal {
                Unopened::Capacity => call.close(CallClose::Capacity),
                Unopened::Failed(error) => answer(call, &FilesReply::Error(error)),
            };
        }
        let mut data = Vec::new();
        if table.staged + size > STAGED_BYTES || data.try_reserve_exact(size).is_err() {
            drop(table);
            return call.close(CallClose::Capacity);
        }
        table.staged += size;
        let touched = Instant::now();
        let lane = call.lane();
        let upload = Upload {
            size,
            data,
            touched,
            call,
            commit: Box::new(commit),
        };
        let instance = table.insert(key, lane, Kind::Upload(upload));
        drop(table);
        self.expire_after(key, instance, touched);
    }

    /// Stages `put` in its upload and answers `call`, the Put's own call.
    pub(crate) fn put(&self, call: Call, put: Put) {
        let key = (call.connection(), put.transfer);
        let mut table = lock(&self.shared);
        let Some(Transfer {
            instance,
            kind: Kind::Upload(upload),
            ..
        }) = table.open.get_mut(&key)
        else {
            drop(table);
            return refuse(call, ErrorCode::InvalidRequest);
        };
        let instance = *instance;
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
            let ended = table.remove(&key);
            drop(table);
            refuse(call, ErrorCode::InvalidRequest);
            if let Some(Transfer {
                kind: Kind::Upload(upload),
                ..
            }) = ended
            {
                refuse(upload.call, ErrorCode::InvalidRequest);
            }
            return;
        }
        upload.data.extend_from_slice(&put.bytes);
        if put.end == PutEnd::Commit {
            let committed = table.remove(&key);
            drop(table);
            if let Some(Transfer {
                kind: Kind::Upload(upload),
                ..
            }) = committed
            {
                (upload.commit)(upload.call, upload.data);
            }
        } else {
            let touched = Instant::now();
            upload.touched = touched;
            drop(table);
            self.expire_after(key, instance, touched);
        }
        answer(call, &FilesReply::Empty);
    }

    /// Ends upload `key` with `timeout` unless a Put reached it after `touched`.
    fn expire_after(&self, key: Key, instance: u64, touched: Instant) {
        let shared = Arc::downgrade(&self.shared);
        // Without a timer thread an upload lives until it commits or its call's lane ends.
        let _ = self.timer.schedule(touched + self.idle, move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let mut table = lock(&shared);
            let idle = table.open.get(&key).is_some_and(|open| {
                open.instance == instance
                    && matches!(&open.kind, Kind::Upload(upload) if upload.touched == touched)
            });
            let expired = if idle { table.remove(&key) } else { None };
            drop(table);
            if let Some(Transfer {
                kind: Kind::Upload(upload),
                ..
            }) = expired
            {
                refuse(upload.call, ErrorCode::Timeout);
            }
        });
    }
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

#[cfg(test)]
mod tests;
