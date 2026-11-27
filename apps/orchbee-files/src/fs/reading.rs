//! Read transfers: `fs.read` opens one under the transfer the Client chose, `fs.read-chunk` pulls
//! its chunks, and `transfer.close`, the end of its lane or the end of its connection drops it and
//! closes its file. Chunk requests may arrive while the read still opens; they wait in arrival
//! order, as do a text read's requests while another of its chunks is decoded, so text is served
//! in offset order and bytes in any order. Every file call runs on one of the process's helpers; a
//! text read whose helper was abandoned fails its later chunks, since its decoder stayed with it.
//! A connection holds at most 64 transfers, since a Client may keep a transfer open after the
//! streams that carried it closed.

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use orchbee_files_protocol::{
    Chunk, ConnectionId, ErrorCode, FilesError, LaneId, Operation, TransferId,
};

mod reader;

use reader::{ByteReader, Piece, Reader, TextReader};
pub(crate) use reader::{ReadFacts, ReadRequest};

use super::background;
use crate::blocking::Helpers;

/// Open transfers per connection.
const MAXIMUM_TRANSFERS: usize = 64;

type Answer = Box<dyn FnOnce(Result<Chunk, FilesError>) + Send>;
type Key = (ConnectionId, TransferId);

/// The open transfers of every connection.
pub(crate) struct Transfers {
    helpers: Helpers,
    shared: Arc<Mutex<State>>,
}

/// Why a read did not open.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Unopened {
    /// The connection holds as many transfers as it may.
    Capacity,
    /// The transfer's name is taken, or the file could not be read.
    Failed(FilesError),
}

/// The lock guards the table; it is never held while a file call or an answer runs. A panic ends
/// the process, so a poisoned lock is never observed.
#[derive(Default)]
struct State {
    open: HashMap<Key, Transfer>,
    next: u64,
}

struct Transfer {
    lane: LaneId,
    /// Tells this transfer from a later one under the same name.
    instance: u64,
    stage: Stage,
}

enum Stage {
    Opening(VecDeque<Waiting>),
    Text {
        /// Absent while a helper decodes a chunk, and for good once one was abandoned.
        reader: Option<Box<TextReader>>,
        waiting: VecDeque<Waiting>,
        running: bool,
        broken: Option<FilesError>,
    },
    Bytes(Arc<ByteReader>),
    Binary,
}

struct Waiting {
    offset: u64,
    length: usize,
    cancelled: Arc<AtomicBool>,
    answer: Answer,
}

impl Stage {
    /// The requests that wait, which a dropped transfer answers as cancelled.
    fn waiting(&mut self) -> VecDeque<Waiting> {
        match self {
            Self::Opening(waiting) | Self::Text { waiting, .. } => std::mem::take(waiting),
            Self::Bytes(_) | Self::Binary => VecDeque::new(),
        }
    }
}

impl Transfers {
    pub(crate) fn new(helpers: Helpers) -> Self {
        Self {
            helpers,
            shared: Arc::new(Mutex::new(State::default())),
        }
    }

    /// Opens transfer `transfer` of `connection` on `lane` to read `path`, which scope admitted.
    /// `done` receives what the read found, or why it did not open.
    pub(crate) fn open(
        &self,
        connection: ConnectionId,
        lane: LaneId,
        transfer: TransferId,
        path: PathBuf,
        request: ReadRequest,
        done: impl FnOnce(Result<ReadFacts, Unopened>) + Send + 'static,
    ) {
        let key = (connection, transfer);
        let instance = {
            let mut state = lock(&self.shared);
            let refusal = if state.open.contains_key(&key) {
                Some(Unopened::Failed(FilesError::new(ErrorCode::InvalidRequest)))
            } else if state
                .open
                .keys()
                .filter(|open| open.0 == connection)
                .count()
                >= MAXIMUM_TRANSFERS
            {
                Some(Unopened::Capacity)
            } else {
                None
            };
            if let Some(refusal) = refusal {
                drop(state);
                done(Err(refusal));
                return;
            }
            state.next += 1;
            let instance = state.next;
            state.open.insert(
                key,
                Transfer {
                    lane,
                    instance,
                    stage: Stage::Opening(VecDeque::new()),
                },
            );
            instance
        };
        let shared = Arc::clone(&self.shared);
        let helpers = self.helpers.clone();
        background::run(
            &self.helpers,
            Operation::FsRead,
            move || reader::open(&path, &request),
            move |outcome| {
                let outcome = outcome.and_then(|opened| opened);
                opened(&shared, &helpers, key, instance, outcome, done);
            },
        );
    }

    /// Serves `length` bytes of transfer `transfer` at `offset` to `answer`. `cancelled` is the
    /// call's cancellation, which a waiting request checks when its turn comes.
    pub(crate) fn chunk(
        &self,
        connection: ConnectionId,
        transfer: TransferId,
        offset: u64,
        length: usize,
        cancelled: Arc<AtomicBool>,
        answer: impl FnOnce(Result<Chunk, FilesError>) + Send + 'static,
    ) {
        let key = (connection, transfer);
        let request = Waiting {
            offset,
            length,
            cancelled,
            answer: Box::new(answer),
        };
        let mut state = lock(&self.shared);
        let Some(open) = state.open.get_mut(&key) else {
            drop(state);
            (request.answer)(Err(FilesError::new(ErrorCode::InvalidRequest)));
            return;
        };
        match &mut open.stage {
            Stage::Opening(waiting) => waiting.push_back(request),
            Stage::Text { waiting, .. } => {
                waiting.push_back(request);
                drop(state);
                pump(&self.shared, &self.helpers, key);
            }
            Stage::Bytes(reader) => {
                let reader = Arc::clone(reader);
                drop(state);
                serve_bytes(&self.helpers, transfer, reader, request);
            }
            Stage::Binary => {
                drop(state);
                (request.answer)(Err(FilesError::new(ErrorCode::InvalidRequest)));
            }
        }
    }

    /// Ends transfer `transfer` of `connection`.
    ///
    /// # Errors
    ///
    /// `invalid-request` for a transfer that is not open.
    pub(crate) fn close(
        &self,
        connection: ConnectionId,
        transfer: TransferId,
    ) -> Result<(), FilesError> {
        let removed = lock(&self.shared).open.remove(&(connection, transfer));
        let removed = removed.ok_or_else(|| FilesError::new(ErrorCode::InvalidRequest))?;
        self.discard(vec![removed]);
        Ok(())
    }

    /// Drops the transfers that `lane` of `connection` carried.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.remove(|key, open| key.0 == connection && open.lane == lane);
    }

    /// Drops every transfer of `connection`.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.remove(|key, _| key.0 == connection);
    }

    /// Drops every transfer.
    pub(crate) fn drain(&self) {
        self.remove(|_, _| true);
    }

    fn remove(&self, matches: impl Fn(&Key, &Transfer) -> bool) {
        let removed: Vec<Transfer> = {
            let mut state = lock(&self.shared);
            let keys: Vec<Key> = state
                .open
                .iter()
                .filter(|(key, open)| matches(key, open))
                .map(|(key, _)| *key)
                .collect();
            keys.iter()
                .filter_map(|key| state.open.remove(key))
                .collect()
        };
        self.discard(removed);
    }

    /// Answers what waits on `removed` as cancelled and closes their files on a helper, since
    /// closing a file on a network volume can block.
    fn discard(&self, mut removed: Vec<Transfer>) {
        for open in &mut removed {
            for request in open.stage.waiting() {
                (request.answer)(Err(FilesError::new(ErrorCode::Cancelled)));
            }
        }
        if !removed.is_empty() {
            background::run(
                &self.helpers,
                Operation::TransferClose,
                move || drop(removed),
                |_| {},
            );
        }
    }
}

/// The read of `key` opened, or failed to.
fn opened(
    shared: &Arc<Mutex<State>>,
    helpers: &Helpers,
    key: Key,
    instance: u64,
    outcome: Result<(ReadFacts, Reader), FilesError>,
    done: impl FnOnce(Result<ReadFacts, Unopened>),
) {
    let mut state = lock(shared);
    let current = state
        .open
        .get(&key)
        .is_some_and(|open| open.instance == instance);
    if !current {
        // Closed while it opened: its file closes with `outcome`.
        drop(state);
        done(Err(Unopened::Failed(FilesError::new(ErrorCode::Cancelled))));
        return;
    }
    let (facts, reader) = match outcome {
        Ok(opened) => opened,
        Err(error) => {
            let waiting = state
                .open
                .remove(&key)
                .map(|mut open| open.stage.waiting())
                .unwrap_or_default();
            drop(state);
            for request in waiting {
                (request.answer)(Err(error.clone()));
            }
            done(Err(Unopened::Failed(error)));
            return;
        }
    };
    let Some(open) = state.open.get_mut(&key) else {
        return;
    };
    let waiting = open.stage.waiting();
    match reader {
        Reader::Text(reader) => {
            open.stage = Stage::Text {
                reader: Some(reader),
                waiting,
                running: false,
                broken: None,
            };
            drop(state);
            done(Ok(facts));
            pump(shared, helpers, key);
        }
        Reader::Bytes(reader) => {
            let reader = Arc::new(reader);
            open.stage = Stage::Bytes(Arc::clone(&reader));
            drop(state);
            done(Ok(facts));
            for request in waiting {
                serve_bytes(helpers, key.1, Arc::clone(&reader), request);
            }
        }
        Reader::Binary => {
            open.stage = Stage::Binary;
            drop(state);
            done(Ok(facts));
            for request in waiting {
                (request.answer)(Err(FilesError::new(ErrorCode::InvalidRequest)));
            }
        }
    }
}

/// Starts the next waiting chunk of the text read `key` unless one runs.
fn pump(shared: &Arc<Mutex<State>>, helpers: &Helpers, key: Key) {
    loop {
        let mut state = lock(shared);
        let Some(open) = state.open.get_mut(&key) else {
            return;
        };
        let instance = open.instance;
        let Stage::Text {
            reader,
            waiting,
            running,
            broken,
        } = &mut open.stage
        else {
            return;
        };
        if *running {
            return;
        }
        let Some(request) = waiting.pop_front() else {
            return;
        };
        let refusal = if request.cancelled.load(Ordering::Acquire) {
            Some(FilesError::new(ErrorCode::Cancelled))
        } else {
            broken.clone()
        };
        let taken = match refusal {
            Some(_) => None,
            None => reader.take(),
        };
        let Some(mut taken) = taken else {
            drop(state);
            let error = refusal.unwrap_or_else(|| FilesError::new(ErrorCode::Failed));
            (request.answer)(Err(error));
            continue;
        };
        *running = true;
        drop(state);
        let shared = Arc::clone(shared);
        let next = helpers.clone();
        let Waiting {
            offset,
            length,
            answer,
            ..
        } = request;
        background::run(
            helpers,
            Operation::FsReadChunk,
            move || {
                let result = taken.chunk(offset, length);
                (taken, result)
            },
            move |outcome| {
                let result = settle(&shared, key, instance, outcome);
                answer(result.map(|piece| chunk(key.1, piece)));
                pump(&shared, &next, key);
            },
        );
        return;
    }
}

/// Returns a text read's decoder after one chunk, or records that its helper was abandoned.
fn settle(
    shared: &Mutex<State>,
    key: Key,
    instance: u64,
    outcome: Result<(Box<TextReader>, Result<Piece, FilesError>), FilesError>,
) -> Result<Piece, FilesError> {
    let mut state = lock(shared);
    let open = state
        .open
        .get_mut(&key)
        .filter(|open| open.instance == instance);
    let (returned, result) = match outcome {
        Ok((reader, result)) => (Some(reader), result),
        Err(interrupted) => (None, Err(interrupted)),
    };
    if let Some(Transfer {
        stage:
            Stage::Text {
                reader,
                running,
                broken,
                ..
            },
        ..
    }) = open
    {
        *running = false;
        match returned {
            Some(returned) => *reader = Some(returned),
            None => *broken = result.as_ref().err().cloned(),
        }
        return result;
    }
    drop(state);
    // The transfer closed meanwhile; its file closes here.
    drop(returned);
    result
}

fn serve_bytes(helpers: &Helpers, transfer: TransferId, reader: Arc<ByteReader>, request: Waiting) {
    let Waiting {
        offset,
        length,
        cancelled,
        answer,
    } = request;
    background::run(
        helpers,
        Operation::FsReadChunk,
        move || {
            if cancelled.load(Ordering::Acquire) {
                return Err(FilesError::new(ErrorCode::Cancelled));
            }
            reader.chunk(offset, length)
        },
        move |outcome| {
            let result = outcome.and_then(|result| result);
            answer(result.map(|piece| chunk(transfer, piece)));
        },
    );
}

fn chunk(transfer: TransferId, piece: Piece) -> Chunk {
    Chunk {
        transfer,
        offset: piece.offset,
        end: piece.end,
        bytes: piece.bytes,
    }
}

fn lock(shared: &Mutex<State>) -> MutexGuard<'_, State> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
