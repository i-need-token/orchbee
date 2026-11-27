//! The file engine's operations as the dispatch routes them. Each takes its call with the native
//! paths scope admitted for it, runs its filesystem work on the process's helpers without making
//! the caller wait, and answers the call once; a call its Client cancelled meanwhile closes as
//! cancelled instead. The dispatch tells the engine when a lane or a connection ends, when a
//! Workspace's subscribers, opened files or scope change, and when the process drains.

use std::{
    ffi::OsStr,
    sync::{Arc, atomic::AtomicBool},
};

use orchbee_files_protocol::{
    CallClose, ConnectionId, EncodingName, Entry, ErrorCode, FilePath, FilesError, FilesReply,
    FsList, FsRead, FsReadChunk, FsStatResult, GuessEncodingResult, LaneId, List, Operation,
    PathsRequest, Put, ReadMode, StatItem, TransferId, TransferRequest, WatchState, WorkspaceId,
};

use super::{
    background,
    guessing::guess_file,
    listing::{CursorId, Listings},
    reading::{Opening, ReadFacts, ReadRequest, Transfers, Unopened},
    timer::Timer,
    uploads::Uploads,
    watch::{Timing, Watching},
    wire,
};
use crate::{
    blocking::Helpers,
    calls::Call,
    entry, error,
    git::ports,
    scope::{Scopes, path::NativePath},
    subscriptions::Subscriptions,
};

/// The listings, read transfers, uploads and watches of every connection.
pub(crate) struct Engine {
    helpers: Helpers,
    listings: Arc<Listings>,
    transfers: Transfers,
    uploads: Uploads,
    watching: Arc<Watching>,
}

impl Engine {
    /// An engine whose filesystem work runs on `helpers` and whose watches deliver to the
    /// `fs.subscribe` streams of `subscriptions` under `scopes`.
    pub(crate) fn new(
        helpers: Helpers,
        scopes: Arc<Scopes>,
        subscriptions: Arc<Subscriptions>,
    ) -> Self {
        let timer = Arc::new(Timer::new());
        Self {
            listings: Arc::new(Listings::new(Arc::clone(&timer))),
            transfers: Transfers::new(helpers.clone()),
            uploads: Uploads::new(timer),
            watching: Arc::new(Watching::new(
                helpers.clone(),
                scopes,
                subscriptions,
                Timing::VS_CODE,
            )),
            helpers,
        }
    }

    /// `fs.list` of `path`, which scope admitted for `request`. The folder loads before it is
    /// read, so a change after the read reaches the Client; a failed listing unloads what it
    /// loaded, and a single-child chain loads with its listing.
    pub(crate) fn list(&self, call: Call, path: NativePath, request: &FsList) {
        let listings = Arc::clone(&self.listings);
        let connection = call.connection();
        let cancelled = call.cancellation();
        let workspace = request.workspace_id;
        let loaded = self.watching.load(connection, workspace, &path);
        let watching = Arc::clone(&self.watching);
        let folder = path.clone();
        let cursor = request
            .cursor
            .map(|cursor| CursorId::from_bytes(*cursor.as_bytes()));
        let single_child = request.single_child;
        background::run(
            &self.helpers,
            Operation::FsList,
            move || {
                let budget = wire::page_budget();
                let path = path.as_path();
                listings.page(connection, path, cursor, single_child, &budget, &cancelled)
            },
            move |outcome| {
                let page = outcome.and_then(|page| page);
                let result = page.and_then(wire::list_result);
                match &result {
                    Ok(listed) => {
                        let chain = listed.descendants.iter().flat_map(|chain| chain.as_slice());
                        for descendant in chain {
                            if let Ok(below) = NativePath::parse(descendant.path.as_str()) {
                                watching.load(connection, workspace, &below);
                            }
                        }
                    }
                    Err(_) if loaded => watching.unload(connection, workspace, &[folder]),
                    Err(_) => {}
                }
                answer(call, result.map(FilesReply::FsList));
            },
        );
    }

    /// `fs.release`: unloads each folder, so its changes no longer reach the Client.
    pub(crate) fn release(&self, call: Call, request: &PathsRequest) {
        let paths: Vec<NativePath> = request
            .paths
            .as_slice()
            .iter()
            .filter_map(|path| NativePath::parse(path.as_str()).ok())
            .collect();
        self.watching
            .unload(call.connection(), request.workspace_id, &paths);
        call.reply(&FilesReply::Empty);
    }

    /// A subscription joined `workspace`.
    pub(crate) fn subscribed(&self, workspace: WorkspaceId) {
        self.watching.subscribed(workspace);
    }

    /// The subscribers, opened files or scope of `workspace`, or of any Workspace, may have
    /// changed.
    pub(crate) fn interest_changed(&self, workspace: Option<WorkspaceId>) {
        self.watching.changed(workspace);
    }

    /// `workspace`'s watch state for a new subscription's Snapshot.
    pub(crate) fn watch_state(&self, workspace: WorkspaceId) -> WatchState {
        self.watching.state(workspace)
    }

    /// The watches of the repositories the Git engine displays.
    pub(crate) fn repository_watches(&self) -> Box<dyn ports::Watches> {
        Box::new(self.watching.repositories())
    }

    /// `fs.stat` of each requested path, with its native path or the reason scope refused it.
    pub(crate) fn stat(&self, call: Call, paths: Vec<(FilePath, Result<NativePath, FilesError>)>) {
        background::run(
            &self.helpers,
            Operation::FsStat,
            move || {
                paths
                    .into_iter()
                    .map(|(path, native)| StatItem {
                        path,
                        outcome: native.and_then(|native| stat(&native)),
                    })
                    .collect::<Vec<_>>()
            },
            move |outcome| {
                let items = outcome.and_then(|items| {
                    List::new(items).map_err(|_| FilesError::new(ErrorCode::Failed))
                });
                answer(
                    call,
                    items.map(|items| FilesReply::FsStat(FsStatResult { items })),
                );
            },
        );
    }

    /// `fs.read` of `path`, which scope admitted for `request`, on the call's lane.
    pub(crate) fn read(&self, call: Call, path: NativePath, request: &FsRead) {
        let read = ReadRequest {
            mode: request.mode,
            encoding: request
                .encoding
                .as_ref()
                .map(|encoding| encoding.as_str().to_owned()),
            force_text: request.force_text,
            limit: request.limit.get(),
        };
        let (connection, lane) = (call.connection(), call.lane());
        let path = path.as_path().to_path_buf();
        self.transfers.open(
            connection,
            lane,
            request.transfer,
            path,
            read,
            answered(call),
        );
    }

    /// The read transfer `git.show` answers through: transfer `transfer` opens on the call's lane
    /// at once, so the chunk requests a Client sends behind the request wait for the content the
    /// Git engine produces. `None` when it cannot open, after the call was answered.
    pub(crate) fn show_read(
        &self,
        call: Call,
        transfer: TransferId,
        mode: ReadMode,
        encoding: Option<&EncodingName>,
        limit: u64,
    ) -> Option<ShowTransfer> {
        let request = ReadRequest {
            mode,
            encoding: encoding.map(|encoding| encoding.as_str().to_owned()),
            force_text: false,
            limit,
        };
        match self
            .transfers
            .begin(call.connection(), call.lane(), transfer)
        {
            Ok(opening) => Some(ShowTransfer {
                opening,
                call,
                request,
            }),
            Err(refusal) => {
                answered(call)(Err(refusal));
                None
            }
        }
    }

    /// `fs.read-chunk`.
    pub(crate) fn read_chunk(&self, call: Call, request: &FsReadChunk) {
        // A chunk length is at most 128 KiB, which every target's `usize` holds.
        let length = usize::try_from(request.length.get()).unwrap_or(usize::MAX);
        let (connection, cancelled) = (call.connection(), call.cancellation());
        let offset = request.offset.get();
        self.transfers.chunk(
            connection,
            request.transfer,
            offset,
            length,
            cancelled,
            move |chunk| match chunk {
                Ok(chunk) if !call.cancelled() => call.chunk(chunk),
                Ok(_) => drop(call),
                Err(failure) => answer(call, Err(failure)),
            },
        );
    }

    /// `fs.guess-encoding` of `path`, which scope admitted.
    pub(crate) fn guess_encoding(&self, call: Call, path: NativePath) {
        background::run(
            &self.helpers,
            Operation::FsGuessEncoding,
            move || guess_file(path.as_path()),
            move |outcome| {
                let guessed = outcome.and_then(|guessed| guessed).and_then(|name| {
                    let encoding = name.map(EncodingName::new).transpose();
                    let encoding = encoding.map_err(|_| FilesError::new(ErrorCode::Failed))?;
                    Ok(GuessEncodingResult { encoding })
                });
                answer(call, guessed.map(FilesReply::GuessEncoding));
            },
        );
    }

    /// Opens upload `transfer` on the call's lane for the `size` bytes an operation receives as
    /// Puts; `commit` receives the call and the data once the Put flagged `commit` completes it.
    pub(crate) fn upload(
        &self,
        call: Call,
        transfer: TransferId,
        size: u64,
        commit: impl FnOnce(Call, Vec<u8>) + Send + 'static,
    ) {
        self.uploads.open(call, transfer, size, commit);
    }

    /// One Put of an upload, answered on its own call.
    pub(crate) fn put(&self, call: Call, put: Put) {
        self.uploads.put(call, put);
    }

    /// `transfer.close`.
    pub(crate) fn close_transfer(&self, call: Call, request: &TransferRequest) {
        let closed = self.transfers.close(call.connection(), request.transfer);
        answer(call, closed.map(|()| FilesReply::Empty));
    }

    /// Drops what `lane` of `connection` holds.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.transfers.lane_closed(connection, lane);
        self.uploads.lane_closed(connection, lane);
    }

    /// Drops what `connection` holds.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.listings.connection_lost(connection);
        self.transfers.connection_lost(connection);
        self.uploads.connection_lost(connection);
        self.watching.connection_lost(connection);
    }

    /// Drops every transfer, answering the chunk requests that wait on them, every upload and
    /// every watch.
    pub(crate) fn drain(&self) {
        self.transfers.drain();
        self.uploads.drain();
        self.watching.drain();
    }
}

/// [`ports::ShowRead`] over the read transfer the engine opened for a `git.show` call.
pub(crate) struct ShowTransfer {
    opening: Opening,
    call: Call,
    request: ReadRequest,
}

impl ports::ShowRead for ShowTransfer {
    fn limit(&self) -> u64 {
        self.request.limit
    }

    fn cancellation(&self) -> Arc<AtomicBool> {
        self.call.cancellation()
    }

    fn supply(self: Box<Self>, content: ports::Content) {
        let Self {
            opening,
            call,
            request,
        } = *self;
        opening.read_content(content, &request, answered(call));
    }

    fn fail(self: Box<Self>, error: FilesError) {
        let Self { opening, call, .. } = *self;
        opening.fail(error, answered(call));
    }
}

/// What answers a read's call once the read opened, or did not.
fn answered(call: Call) -> impl FnOnce(Result<ReadFacts, Unopened>) + Send + 'static {
    move |opened| match opened {
        Ok(facts) => answer(call, facts.to_wire().map(FilesReply::Read)),
        Err(Unopened::Capacity) => call.close(CallClose::Capacity),
        Err(Unopened::Failed(failure)) => answer(call, Err(failure)),
    }
}

/// The entry at `path`.
fn stat(path: &NativePath) -> Result<Entry, FilesError> {
    entry::read(path.as_path(), OsStr::new(path.name())).map_err(|failure| error::from_io(&failure))
}

/// Answers `call` with `reply`, or closes it as cancelled when its Client cancelled it meanwhile.
fn answer(call: Call, reply: Result<FilesReply, FilesError>) {
    if call.cancelled() {
        // Dropping a cancelled call closes it as cancelled.
        drop(call);
    } else {
        call.reply(&reply.unwrap_or_else(FilesReply::Error));
    }
}

#[cfg(test)]
mod tests;
