//! Transfers: the reads `fs.read` and `git.show` open, whose chunks `fs.read-chunk` pulls, the
//! uploads whose Puts bring an operation its data, and the streams that hand an operation its data
//! as it arrives. The Client names each transfer, and a name is unique in its connection whatever
//! the transfer's kind; a connection holds at most 64, since a Client may keep a transfer open
//! after the streams that carried it closed. `transfer.close`, the end of the transfer's lane or
//! connection, or the drain ends a transfer: a read answers the chunk requests that wait on it as
//! cancelled and closes its file, an upload drops its data with its operation's call, which
//! `transfer.close` answers as cancelled, and a stream tells its consumer.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use orchbee_files_protocol::{ConnectionId, ErrorCode, FilesError, LaneId, TransferId};

mod reading;
mod streams;
mod uploads;

pub(crate) use reading::{Opening, ReadFacts, ReadRequest};
pub(crate) use streams::{Delivery, Streaming};

use super::timer::Timer;
use crate::blocking::Helpers;

/// Open transfers per connection.
const MAXIMUM_TRANSFERS: usize = 64;

type Key = (ConnectionId, TransferId);

/// The open transfers of every connection.
pub(crate) struct Transfers {
    helpers: Helpers,
    timer: Arc<Timer>,
    /// How long an upload waits for its next Put.
    idle: Duration,
    shared: Arc<Mutex<Table>>,
}

/// Why a transfer did not open.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Unopened {
    /// The connection holds as many transfers as it may.
    Capacity,
    /// The transfer's name is taken, or the file could not be read.
    Failed(FilesError),
}

/// The lock guards the table; it is never held while a file call runs or a call is answered or
/// dropped. A panic ends the process, so a poisoned lock is never observed.
#[derive(Default)]
struct Table {
    open: HashMap<Key, Transfer>,
    next: u64,
    /// The declared sizes of the open uploads.
    staged: usize,
}

struct Transfer {
    lane: LaneId,
    /// Tells this transfer from a later one under the same name.
    instance: u64,
    kind: Kind,
}

enum Kind {
    Read(reading::Stage),
    Upload(uploads::Upload),
    Stream(streams::Stream),
}

impl Table {
    /// Whether `key` may name a new transfer: no transfer of any kind holds the name, and the
    /// connection holds fewer than it may.
    ///
    /// # Errors
    ///
    /// `invalid-request` for a name that is taken, or [`Unopened::Capacity`].
    fn admit(&self, key: Key) -> Result<(), Unopened> {
        if self.open.contains_key(&key) {
            return Err(Unopened::Failed(FilesError::new(ErrorCode::InvalidRequest)));
        }
        let held = self.open.keys().filter(|open| open.0 == key.0);
        if held.count() >= MAXIMUM_TRANSFERS {
            return Err(Unopened::Capacity);
        }
        Ok(())
    }

    /// Opens a transfer of `kind` under `key` on `lane` and returns its instance.
    fn insert(&mut self, key: Key, lane: LaneId, kind: Kind) -> u64 {
        self.next += 1;
        let instance = self.next;
        let transfer = Transfer {
            lane,
            instance,
            kind,
        };
        self.open.insert(key, transfer);
        instance
    }

    /// Removes transfer `key`, releasing what an upload reserved.
    fn remove(&mut self, key: &Key) -> Option<Transfer> {
        let removed = self.open.remove(key)?;
        if let Kind::Upload(upload) = &removed.kind {
            self.staged -= upload.size;
        }
        Some(removed)
    }
}

impl Transfers {
    /// No transfers yet; `timer` ends the uploads that wait too long for a Put.
    pub(crate) fn new(helpers: Helpers, timer: Arc<Timer>) -> Self {
        Self::with_idle(helpers, timer, uploads::IDLE)
    }

    fn with_idle(helpers: Helpers, timer: Arc<Timer>, idle: Duration) -> Self {
        Self {
            helpers,
            timer,
            idle,
            shared: Arc::default(),
        }
    }

    /// `transfer.close`: ends transfer `transfer` of `connection`, a read or an upload.
    ///
    /// # Errors
    ///
    /// `invalid-request` for a transfer that is not open.
    pub(crate) fn close(
        &self,
        connection: ConnectionId,
        transfer: TransferId,
    ) -> Result<(), FilesError> {
        let removed = lock(&self.shared).remove(&(connection, transfer));
        let removed = removed.ok_or_else(|| FilesError::new(ErrorCode::InvalidRequest))?;
        match removed.kind {
            Kind::Read(stage) => reading::discard(&self.helpers, vec![stage]),
            Kind::Upload(upload) => upload.cancel(),
            Kind::Stream(stream) => stream.end(ErrorCode::Cancelled),
        }
        Ok(())
    }

    /// Drops the transfers that `lane` of `connection` carried.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.remove_where(|key, open| key.0 == connection && open.lane == lane);
    }

    /// Drops every transfer of `connection`.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.remove_where(|key, _| key.0 == connection);
    }

    /// Drops every transfer.
    pub(crate) fn drain(&self) {
        self.remove_where(|_, _| true);
    }

    /// Drops the transfers `matches` picks; an upload's call closes as the event that ended it
    /// left it, and a stream's deliveries end.
    fn remove_where(&self, matches: impl Fn(&Key, &Transfer) -> bool) {
        let removed: Vec<Transfer> = {
            let mut table = lock(&self.shared);
            let keys: Vec<Key> = table
                .open
                .iter()
                .filter(|(key, open)| matches(key, open))
                .map(|(key, _)| *key)
                .collect();
            keys.iter().filter_map(|key| table.remove(key)).collect()
        };
        let mut reads = Vec::new();
        for transfer in removed {
            match transfer.kind {
                Kind::Read(stage) => reads.push(stage),
                Kind::Upload(upload) => drop(upload),
                Kind::Stream(stream) => drop(stream),
            }
        }
        reading::discard(&self.helpers, reads);
    }
}

fn lock(shared: &Mutex<Table>) -> MutexGuard<'_, Table> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}
