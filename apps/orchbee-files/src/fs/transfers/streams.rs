//! Streamed uploads: an operation whose data may pass what uploads stage in memory, such as a file
//! an import brings, opens one under the transfer the Client chose, and each Put hands its bytes
//! on, in order, to the operation's consumer, which writes them where it stages them and answers
//! the Put once they are written. The Puts follow an upload's rules: one that skips or overlaps
//! data, passes the declared size, ends the data without committing, or commits before the data
//! is complete is `invalid-request` and ends the stream, as is one that would leave more than
//! 4 MiB handed on and not yet written, twice a Client's window. A stream ends with `timeout` when
//! no Put arrives for 60 s and with `cancelled` on `transfer.close`, which its consumer receives as
//! its last delivery; the end of its lane, its connection or the drain ends its deliveries
//! without one.

use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Instant,
};

use orchbee_files_protocol::{
    ConnectionId, ErrorCode, FilesError, FilesReply, LaneId, Put, PutEnd, TransferId,
};

use super::{Key, Kind, Table, Transfer, Transfers, Unopened, lock};
use crate::calls::Call;

/// The most bytes a stream hands on that its consumer has not written yet.
const MAXIMUM_WAITING: usize = 4 * 1024 * 1024;

/// What a stream hands its consumer, in order.
pub(crate) enum Delivery {
    /// The bytes of one Put, whose call the consumer answers once they are written.
    Data(Vec<u8>, Call),
    /// The bytes of the Put flagged `commit`, which complete the data, with that Put's call.
    Commit(Vec<u8>, Call),
    /// The stream ended unfinished as this code says: `cancelled`, `timeout` or
    /// `invalid-request`.
    Ended(ErrorCode),
}

/// A stream as the transfers table holds it.
pub(super) struct Stream {
    size: u64,
    received: u64,
    touched: Instant,
    waiting: Arc<AtomicUsize>,
    deliveries: Sender<Delivery>,
}

impl Stream {
    /// Ends the stream as `code` says, telling its consumer.
    pub(super) fn end(self, code: ErrorCode) {
        let _ = self.deliveries.send(Delivery::Ended(code));
    }
}

/// The consumer's side of a stream.
pub(crate) struct Streaming {
    /// The deliveries, in order, which end with the stream.
    pub(crate) deliveries: Receiver<Delivery>,
    waiting: Arc<AtomicUsize>,
    shared: Weak<Mutex<Table>>,
    key: Key,
    instance: u64,
}

impl Streaming {
    /// Records that `bytes` the stream handed on are written.
    pub(crate) fn written(&self, bytes: usize) {
        self.waiting.fetch_sub(bytes, Ordering::AcqRel);
    }

    /// Ends the stream from the consumer's side, as dropping it also does, so its later Puts are
    /// `invalid-request`. The deliveries it holds then are all it will receive.
    pub(crate) fn end(&self) {
        let Some(shared) = self.shared.upgrade() else {
            return;
        };
        let mut table = lock(&shared);
        let ours = table
            .open
            .get(&self.key)
            .is_some_and(|open| open.instance == self.instance);
        let removed = if ours { table.remove(&self.key) } else { None };
        drop(table);
        drop(removed);
    }
}

impl Drop for Streaming {
    /// Ends the stream and refuses the Puts its consumer left unanswered.
    fn drop(&mut self) {
        self.end();
        for waiting in self.deliveries.try_iter() {
            if let Delivery::Data(_, call) | Delivery::Commit(_, call) = waiting {
                refuse(call);
            }
        }
    }
}

impl Transfers {
    /// Opens stream `transfer` of `connection` on `lane` for `size` bytes.
    ///
    /// # Errors
    ///
    /// [`Unopened::Capacity`] when the connection holds as many transfers as it may, or
    /// `invalid-request` when the transfer's name is taken.
    pub(crate) fn stream(
        &self,
        connection: ConnectionId,
        lane: LaneId,
        transfer: TransferId,
        size: u64,
    ) -> Result<Streaming, Unopened> {
        let key = (connection, transfer);
        let (sender, deliveries) = mpsc::channel();
        let waiting = Arc::new(AtomicUsize::new(0));
        let touched = Instant::now();
        let stream = Stream {
            size,
            received: 0,
            touched,
            waiting: Arc::clone(&waiting),
            deliveries: sender,
        };
        let mut table = lock(&self.shared);
        table.admit(key)?;
        let instance = table.insert(key, lane, Kind::Stream(stream));
        drop(table);
        self.stream_expires_after(key, instance, touched);
        Ok(Streaming {
            deliveries,
            waiting,
            shared: Arc::downgrade(&self.shared),
            key,
            instance,
        })
    }

    /// Hands `put` on to its stream; `call`, the Put's own call, is answered here only when the
    /// Put is refused.
    pub(super) fn stream_put(&self, call: Call, put: Put) {
        let key = (call.connection(), put.transfer);
        let mut table = lock(&self.shared);
        let Some(Transfer {
            instance,
            kind: Kind::Stream(stream),
            ..
        }) = table.open.get_mut(&key)
        else {
            drop(table);
            return refuse(call);
        };
        let instance = *instance;
        let length = put.bytes.len();
        let reached = stream.received.checked_add(length as u64);
        let fits = put.offset == stream.received && reached.is_some_and(|end| end <= stream.size);
        let waiting = stream
            .waiting
            .load(Ordering::Acquire)
            .saturating_add(length);
        let admitted = fits
            && waiting <= MAXIMUM_WAITING
            && match put.end {
                PutEnd::Continues => true,
                PutEnd::Last => false,
                PutEnd::Commit => reached == Some(stream.size),
            };
        let Some(reached) = reached.filter(|_| admitted) else {
            let ended = table.remove(&key);
            drop(table);
            refuse(call);
            if let Some(Transfer {
                kind: Kind::Stream(stream),
                ..
            }) = ended
            {
                stream.end(ErrorCode::InvalidRequest);
            }
            return;
        };
        stream.received = reached;
        stream.waiting.fetch_add(length, Ordering::AcqRel);
        let commit = put.end == PutEnd::Commit;
        let touched = (!commit).then(Instant::now);
        let delivery = match touched {
            Some(now) => {
                stream.touched = now;
                Delivery::Data(put.bytes, call)
            }
            None => Delivery::Commit(put.bytes, call),
        };
        // Sent under the lock, so a consumer that ended the stream holds every delivery it will
        // ever receive.
        let unsent = stream.deliveries.send(delivery).err();
        if commit {
            // The data is complete: what remains is the consumer's.
            drop(table.remove(&key));
        }
        drop(table);
        if let Some(touched) = touched {
            self.stream_expires_after(key, instance, touched);
        }
        if let Some(unsent) = unsent
            && let Delivery::Data(_, call) | Delivery::Commit(_, call) = unsent.0
        {
            refuse(call);
        }
    }

    /// Ends stream `key` with `timeout` unless a Put reached it after `touched`.
    fn stream_expires_after(&self, key: Key, instance: u64, touched: Instant) {
        let shared = Arc::downgrade(&self.shared);
        // Without a timer thread a stream lives until it commits or its lane ends.
        let _ = self.timer.schedule(touched + self.idle, move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let mut table = lock(&shared);
            let idle = table.open.get(&key).is_some_and(|open| {
                open.instance == instance
                    && matches!(&open.kind, Kind::Stream(stream) if stream.touched == touched)
            });
            let expired = if idle { table.remove(&key) } else { None };
            drop(table);
            if let Some(Transfer {
                kind: Kind::Stream(stream),
                ..
            }) = expired
            {
                stream.end(ErrorCode::Timeout);
            }
        });
    }
}

fn refuse(call: Call) {
    let refusal = FilesReply::Error(FilesError::new(ErrorCode::InvalidRequest));
    if call.cancelled() {
        // Dropping a cancelled call closes it as cancelled.
        drop(call);
    } else {
        call.reply(&refusal);
    }
}
