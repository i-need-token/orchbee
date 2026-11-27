//! Read transfers: `fs.read` opens one under the transfer the Client chose, as `git.show` does for
//! content the Git engine produces, and `fs.read-chunk` pulls its chunks. Chunk requests may arrive
//! while the read still opens or its content is still produced; they wait in arrival order, as do
//! a text read's requests while another of its chunks is decoded, so text is served in offset
//! order and bytes in any order. Every file call runs on one of the process's helpers; a text read
//! whose helper was abandoned fails its later chunks, since its decoder stayed with it. A read that
//! ends answers the requests that wait on it as cancelled and closes its file on a helper.

use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use orchbee_files_protocol::{
    Chunk, ConnectionId, ErrorCode, FilesError, LaneId, Operation, TransferId,
};

mod reader;

use reader::{ByteReader, Content, Piece, Reader, TextReader};
pub(crate) use reader::{ReadFacts, ReadRequest};

use super::{Key, Kind, Table, Transfer, Transfers, Unopened, lock};
use crate::{blocking::Helpers, fs::background};

type Answer = Box<dyn FnOnce(Result<Chunk, FilesError>) + Send>;

/// How far a read got.
pub(super) enum Stage {
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

pub(super) struct Waiting {
    offset: u64,
    length: usize,
    cancelled: Arc<AtomicBool>,
    answer: Answer,
}

impl Stage {
    /// The requests that wait, which an ended read answers as cancelled.
    fn waiting(&mut self) -> VecDeque<Waiting> {
        match self {
            Self::Opening(waiting) | Self::Text { waiting, .. } => std::mem::take(waiting),
            Self::Bytes(_) | Self::Binary => VecDeque::new(),
        }
    }
}

impl Transfers {
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
        match self.begin(connection, lane, transfer) {
            Ok(opening) => opening.read_file(path, request, done),
            Err(refusal) => done(Err(refusal)),
        }
    }

    /// Opens transfer `transfer` of `connection` on `lane` before what it reads is known: chunk
    /// requests wait on it until the returned [`Opening`] reads its content or fails.
    ///
    /// # Errors
    ///
    /// [`Unopened::Capacity`] when the connection holds as many transfers as it may, or
    /// `invalid-request` when the transfer's name is taken.
    pub(crate) fn begin(
        &self,
        connection: ConnectionId,
        lane: LaneId,
        transfer: TransferId,
    ) -> Result<Opening, Unopened> {
        let key = (connection, transfer);
        let mut table = lock(&self.shared);
        table.admit(key)?;
        let instance = table.insert(key, lane, Kind::Read(Stage::Opening(VecDeque::new())));
        Ok(Opening {
            shared: Arc::clone(&self.shared),
            helpers: self.helpers.clone(),
            key,
            instance,
            settled: false,
        })
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
        let mut table = lock(&self.shared);
        let Some(Transfer {
            kind: Kind::Read(stage),
            ..
        }) = table.open.get_mut(&key)
        else {
            drop(table);
            (request.answer)(Err(FilesError::new(ErrorCode::InvalidRequest)));
            return;
        };
        match stage {
            Stage::Opening(waiting) => waiting.push_back(request),
            Stage::Text { waiting, .. } => {
                waiting.push_back(request);
                drop(table);
                pump(&self.shared, &self.helpers, key);
            }
            Stage::Bytes(reader) => {
                let reader = Arc::clone(reader);
                drop(table);
                serve_bytes(&self.helpers, transfer, reader, request);
            }
            Stage::Binary => {
                drop(table);
                (request.answer)(Err(FilesError::new(ErrorCode::InvalidRequest)));
            }
        }
    }
}

/// Answers what waits on the ended reads `stages` as cancelled and closes their files on a helper,
/// since closing a file on a network volume can block.
pub(super) fn discard(helpers: &Helpers, mut stages: Vec<Stage>) {
    for stage in &mut stages {
        for request in stage.waiting() {
            (request.answer)(Err(FilesError::new(ErrorCode::Cancelled)));
        }
    }
    if !stages.is_empty() {
        background::run(
            helpers,
            Operation::TransferClose,
            move || drop(stages),
            |_| {},
        );
    }
}

/// A transfer whose content is not read yet: chunk requests queue on it until it reads its
/// content or fails. Dropped before either, it ends as cancelled.
pub(crate) struct Opening {
    shared: Arc<Mutex<Table>>,
    helpers: Helpers,
    key: Key,
    instance: u64,
    settled: bool,
}

impl Opening {
    /// Reads `path`, which scope admitted, on a helper.
    fn read_file(
        mut self,
        path: PathBuf,
        request: ReadRequest,
        done: impl FnOnce(Result<ReadFacts, Unopened>) + Send + 'static,
    ) {
        self.settled = true;
        let shared = Arc::clone(&self.shared);
        let helpers = self.helpers.clone();
        let (key, instance) = (self.key, self.instance);
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

    /// Reads `content`, held in memory, which touches no file and so needs no helper; `done`
    /// receives what the read found, or why it did not open.
    pub(crate) fn read_content(
        mut self,
        content: Content,
        request: &ReadRequest,
        done: impl FnOnce(Result<ReadFacts, Unopened>),
    ) {
        self.settled = true;
        let outcome = reader::open_content(content, request);
        opened(
            &self.shared,
            &self.helpers,
            self.key,
            self.instance,
            outcome,
            done,
        );
    }

    /// Ends the transfer with `error`, which its waiting chunk requests and `done` receive.
    pub(crate) fn fail(
        mut self,
        error: FilesError,
        done: impl FnOnce(Result<ReadFacts, Unopened>),
    ) {
        self.settled = true;
        opened(
            &self.shared,
            &self.helpers,
            self.key,
            self.instance,
            Err(error),
            done,
        );
    }
}

impl Drop for Opening {
    fn drop(&mut self) {
        if !self.settled {
            let cancelled = Err(FilesError::new(ErrorCode::Cancelled));
            opened(
                &self.shared,
                &self.helpers,
                self.key,
                self.instance,
                cancelled,
                |_| {},
            );
        }
    }
}

/// The read of `key` opened, or failed to.
fn opened(
    shared: &Arc<Mutex<Table>>,
    helpers: &Helpers,
    key: Key,
    instance: u64,
    outcome: Result<(ReadFacts, Reader), FilesError>,
    done: impl FnOnce(Result<ReadFacts, Unopened>),
) {
    let mut table = lock(shared);
    let current = table
        .open
        .get(&key)
        .is_some_and(|open| open.instance == instance);
    if !current {
        // Closed while it opened: its file closes with `outcome`.
        drop(table);
        done(Err(Unopened::Failed(FilesError::new(ErrorCode::Cancelled))));
        return;
    }
    let (facts, reader) = match outcome {
        Ok(opened) => opened,
        Err(error) => {
            let removed = table.remove(&key);
            drop(table);
            if let Some(Transfer {
                kind: Kind::Read(mut stage),
                ..
            }) = removed
            {
                for request in stage.waiting() {
                    (request.answer)(Err(error.clone()));
                }
            }
            done(Err(Unopened::Failed(error)));
            return;
        }
    };
    let Some(Transfer {
        kind: Kind::Read(stage),
        ..
    }) = table.open.get_mut(&key)
    else {
        return;
    };
    let waiting = stage.waiting();
    match reader {
        Reader::Text(reader) => {
            *stage = Stage::Text {
                reader: Some(reader),
                waiting,
                running: false,
                broken: None,
            };
            drop(table);
            done(Ok(facts));
            pump(shared, helpers, key);
        }
        Reader::Bytes(reader) => {
            let reader = Arc::new(reader);
            *stage = Stage::Bytes(Arc::clone(&reader));
            drop(table);
            done(Ok(facts));
            for request in waiting {
                serve_bytes(helpers, key.1, Arc::clone(&reader), request);
            }
        }
        Reader::Binary => {
            *stage = Stage::Binary;
            drop(table);
            done(Ok(facts));
            for request in waiting {
                (request.answer)(Err(FilesError::new(ErrorCode::InvalidRequest)));
            }
        }
    }
}

/// Starts the next waiting chunk of the text read `key` unless one runs.
fn pump(shared: &Arc<Mutex<Table>>, helpers: &Helpers, key: Key) {
    loop {
        let mut table = lock(shared);
        let Some(open) = table.open.get_mut(&key) else {
            return;
        };
        let instance = open.instance;
        let Kind::Read(Stage::Text {
            reader,
            waiting,
            running,
            broken,
        }) = &mut open.kind
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
            drop(table);
            let error = refusal.unwrap_or_else(|| FilesError::new(ErrorCode::Failed));
            (request.answer)(Err(error));
            continue;
        };
        *running = true;
        drop(table);
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
    shared: &Mutex<Table>,
    key: Key,
    instance: u64,
    outcome: Result<(Box<TextReader>, Result<Piece, FilesError>), FilesError>,
) -> Result<Piece, FilesError> {
    let mut table = lock(shared);
    let open = table
        .open
        .get_mut(&key)
        .filter(|open| open.instance == instance);
    let (returned, result) = match outcome {
        Ok((reader, result)) => (Some(reader), result),
        Err(interrupted) => (None, Err(interrupted)),
    };
    if let Some(Transfer {
        kind:
            Kind::Read(Stage::Text {
                reader,
                running,
                broken,
                ..
            }),
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
    drop(table);
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

#[cfg(test)]
mod tests;
