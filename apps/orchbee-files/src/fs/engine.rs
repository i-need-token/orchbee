//! The file engine's operations in File Bar terms, the seam the operation dispatch calls. Each
//! takes the decoded request with the native paths scope admitted for it, runs its file calls on
//! a helper with a deadline, and answers once through `done`: from a helper, from the timer at the
//! deadline, or at once when it is refused. The dispatch owns scope, the call's stream and its
//! cancellation flag, and tells the engine when a lane or a connection ends.

use std::{
    io,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use orchbee_files_protocol::{
    Chunk, ConnectionId, EncodingName, ErrorCode, FilePath, FilesError, FsList, FsListResult,
    FsRead, FsReadChunk, FsStatResult, GuessEncodingResult, LaneId, List, ReadResult, StatItem,
    TransferId,
};

use super::{
    blocking::Blocking,
    guessing::guess_file,
    listing::{CursorId, Listings},
    metadata::stat,
    os_error::FsError,
    reading::{ReadFacts, ReadRequest, Transfers},
    timer::Timer,
    wire,
};

pub(crate) struct Engine {
    blocking: Arc<Blocking>,
    listings: Arc<Listings>,
    transfers: Transfers,
}

impl Engine {
    /// Starts the engine's timer.
    ///
    /// # Errors
    ///
    /// The timer thread could not start.
    pub(crate) fn start() -> io::Result<Self> {
        let timer = Arc::new(Timer::start()?);
        let blocking = Arc::new(Blocking::new(Arc::clone(&timer)));
        Ok(Self {
            listings: Arc::new(Listings::new(timer)),
            transfers: Transfers::new(Arc::clone(&blocking)),
            blocking,
        })
    }

    /// `fs.list` of `path`, the native path of `request.path`.
    pub(crate) fn list(
        &self,
        connection: ConnectionId,
        request: &FsList,
        path: PathBuf,
        cancelled: Arc<AtomicBool>,
        done: impl FnOnce(Result<FsListResult, FilesError>) + Send + 'static,
    ) {
        let listings = Arc::clone(&self.listings);
        let cursor = request
            .cursor
            .map(|cursor| CursorId::from_bytes(*cursor.as_bytes()));
        let single_child = request.single_child;
        self.blocking.run(
            move || {
                let budget = wire::page_budget();
                listings.page(connection, &path, cursor, single_child, &budget, &cancelled)
            },
            move |outcome| {
                let page = outcome.map_err(FsError::from).and_then(|page| page);
                done(page.map_err(FsError::to_wire).and_then(wire::list_result));
            },
        );
    }

    /// `fs.stat` of each requested path with its native path.
    pub(crate) fn stat(
        &self,
        paths: Vec<(FilePath, PathBuf)>,
        done: impl FnOnce(Result<FsStatResult, FilesError>) + Send + 'static,
    ) {
        self.blocking.run(
            move || {
                paths
                    .into_iter()
                    .map(|(path, native)| {
                        let entry = stat(&native).map_err(|error| FsError::io(&error).to_wire());
                        let outcome = entry.and_then(|entry| {
                            entry.to_wire().ok_or(FilesError::new(ErrorCode::Failed))
                        });
                        StatItem { path, outcome }
                    })
                    .collect::<Vec<_>>()
            },
            move |outcome| {
                let items = outcome.map_err(|interrupted| FsError::from(interrupted).to_wire());
                done(items.and_then(|items| {
                    List::new(items)
                        .map(|items| FsStatResult { items })
                        .map_err(|_| FilesError::new(ErrorCode::InvalidRequest))
                }));
            },
        );
    }

    /// `fs.read` of `path`, the native path of `request.path`, on `lane`.
    pub(crate) fn read(
        &self,
        connection: ConnectionId,
        lane: LaneId,
        request: &FsRead,
        path: PathBuf,
        done: impl FnOnce(Result<ReadResult, FilesError>) + Send + 'static,
    ) {
        let read = ReadRequest {
            mode: request.mode,
            encoding: request
                .encoding
                .as_ref()
                .map(|encoding| encoding.as_str().to_owned()),
            force_text: request.force_text,
            limit: request.limit.get(),
        };
        self.transfers.open(
            connection,
            lane,
            request.transfer,
            path,
            read,
            move |facts| {
                done(facts.map_err(FsError::to_wire).and_then(ReadFacts::to_wire));
            },
        );
    }

    /// `fs.read-chunk`.
    pub(crate) fn read_chunk(
        &self,
        connection: ConnectionId,
        request: &FsReadChunk,
        cancelled: Arc<AtomicBool>,
        done: impl FnOnce(Result<Chunk, FilesError>) + Send + 'static,
    ) {
        let offset = request.offset.get();
        // A chunk length is at most 128 KiB, which every target's `usize` holds.
        let length = usize::try_from(request.length.get()).unwrap_or(usize::MAX);
        self.transfers.chunk(
            connection,
            request.transfer,
            offset,
            length,
            cancelled,
            move |chunk| done(chunk.map_err(FsError::to_wire)),
        );
    }

    /// `fs.guess-encoding` of `path`, the native path of the request's path.
    pub(crate) fn guess_encoding(
        &self,
        path: PathBuf,
        done: impl FnOnce(Result<GuessEncodingResult, FilesError>) + Send + 'static,
    ) {
        self.blocking.run(
            move || guess_file(&path),
            move |outcome| {
                let guessed = outcome.map_err(FsError::from).and_then(|guessed| guessed);
                done(guessed.map_err(FsError::to_wire).and_then(|name| {
                    let encoding = name.map(EncodingName::new).transpose();
                    let encoding = encoding.map_err(|_| FilesError::new(ErrorCode::Failed))?;
                    Ok(GuessEncodingResult { encoding })
                }));
            },
        );
    }

    /// `transfer.close`.
    ///
    /// # Errors
    ///
    /// `invalid-request` for a transfer that is not open.
    pub(crate) fn close_transfer(
        &self,
        connection: ConnectionId,
        transfer: TransferId,
    ) -> Result<(), FilesError> {
        self.transfers
            .close(connection, transfer)
            .map_err(FsError::to_wire)
    }

    /// Cancels what `lane` of `connection` holds.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.transfers.lane_closed(connection, lane);
    }

    /// Drops what `connection` holds.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.listings.connection_lost(connection);
        self.transfers.connection_lost(connection);
    }
}

#[cfg(test)]
mod tests;
