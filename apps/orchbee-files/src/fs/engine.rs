//! The file engine's operations as the dispatch routes them. Each takes its call with the native
//! paths scope admitted for it, runs its filesystem work on the process's helpers without making
//! the caller wait, and answers the call once; a call its Client cancelled meanwhile closes as
//! cancelled instead. The dispatch tells the engine when a lane or a connection ends, when a
//! Workspace's subscribers, opened files or scope change, and when the process drains.

use std::{
    collections::HashSet,
    ffi::OsStr,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError, atomic::AtomicBool},
};

use orchbee_files_protocol::{
    CallClose, ConnectionId, EncodingName, Entry, ErrorCode, FilePath, FilesError, FilesReply,
    FsList, FsListResult, FsRead, FsReadChunk, FsStatResult, GuessEncodingResult, LaneId, List,
    Operation, PageCursor, PathsRequest, PickerListResult, Put, ReadMode, StatItem, TransferId,
    TransferRequest, WatchState, WorkspaceId,
};

use super::{
    background,
    guessing::guess_file,
    listing::{CursorId, Listings},
    timer::Timer,
    transfers::{Opening, ReadFacts, ReadRequest, Streaming, Transfers, Unopened},
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

/// The listings, transfers and watches of every connection.
pub(crate) struct Engine {
    helpers: Helpers,
    listings: Arc<Listings>,
    transfers: Transfers,
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
            transfers: Transfers::new(helpers.clone(), timer),
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
    /// read, and so does each folder of a single-child chain, so a change after a read reaches the
    /// Client; a failed listing unloads what it loaded, and a chain folder it entered but left out
    /// unloads.
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
        let chain = request.single_child.then(|| {
            Arc::new(ChainLoads::new(
                Arc::clone(&self.watching),
                connection,
                workspace,
            ))
        });
        let entering = chain.clone();
        background::run(
            &self.helpers,
            Operation::FsList,
            move || {
                let budget = wire::page_budget();
                let path = path.as_path();
                let mut enter = entering
                    .as_deref()
                    .map(|chain| move |below: &Path| chain.enter(below));
                let enter = enter
                    .as_mut()
                    .map(|enter| enter as &mut dyn FnMut(&Path) -> bool);
                listings.page(connection, path, cursor, enter, &budget, &cancelled)
            },
            move |outcome| {
                let page = outcome.and_then(|page| page);
                let result = page.and_then(wire::list_result);
                if let Some(chain) = &chain {
                    chain.finish(result.as_ref().ok());
                }
                if result.is_err() && loaded {
                    watching.unload(connection, workspace, &[folder]);
                }
                answer(call, result.map(FilesReply::FsList));
            },
        );
    }

    /// `picker.list` of `path`, any directory an open picker admits: a page of its entries as
    /// `fs.list` pages them, without loading the directory for change delivery.
    pub(crate) fn picker_list(&self, call: Call, path: NativePath, cursor: Option<PageCursor>) {
        let listings = Arc::clone(&self.listings);
        let connection = call.connection();
        let cancelled = call.cancellation();
        let cursor = cursor.map(|cursor| CursorId::from_bytes(*cursor.as_bytes()));
        background::run(
            &self.helpers,
            Operation::PickerList,
            move || {
                let budget = wire::page_budget();
                listings.page(
                    connection,
                    path.as_path(),
                    cursor,
                    None,
                    &budget,
                    &cancelled,
                )
            },
            move |outcome| {
                let listed = outcome.and_then(|page| page).and_then(wire::list_result);
                let result = listed.map(|listed| {
                    FilesReply::PickerList(PickerListResult {
                        entries: listed.entries,
                        cursor: listed.cursor,
                    })
                });
                answer(call, result);
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
        self.transfers.upload(call, transfer, size, commit);
    }

    /// Opens stream `transfer` of `connection` on `lane` for the `size` bytes an operation
    /// receives as Puts, handed on as they arrive.
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
        self.transfers.stream(connection, lane, transfer, size)
    }

    /// One Put of an upload or a stream, answered on its own call.
    pub(crate) fn put(&self, call: Call, put: Put) {
        self.transfers.put(call, put);
    }

    /// `transfer.close`.
    pub(crate) fn close_transfer(&self, call: Call, request: &TransferRequest) {
        let closed = self.transfers.close(call.connection(), request.transfer);
        answer(call, closed.map(|()| FilesReply::Empty));
    }

    /// Drops what `lane` of `connection` holds.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.transfers.lane_closed(connection, lane);
    }

    /// Drops what `connection` holds.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.listings.connection_lost(connection);
        self.transfers.connection_lost(connection);
        self.watching.connection_lost(connection);
    }

    /// Drops every transfer, answering the chunk requests that wait on them, and every watch.
    pub(crate) fn drain(&self) {
        self.transfers.drain();
        self.watching.drain();
    }
}

/// The folders one listing's single-child chain entered, each loaded before it was read. Once the
/// listing answers, a folder it loaded but left out unloads, and a helper the listing abandoned
/// enters nothing more. A panic ends the process, so a poisoned lock is never observed.
struct ChainLoads {
    watching: Arc<Watching>,
    connection: ConnectionId,
    workspace: WorkspaceId,
    /// Each entered folder and whether it was not loaded before; `None` once the listing answered.
    entered: Mutex<Option<Vec<(NativePath, bool)>>>,
}

impl ChainLoads {
    fn new(watching: Arc<Watching>, connection: ConnectionId, workspace: WorkspaceId) -> Self {
        Self {
            watching,
            connection,
            workspace,
            entered: Mutex::new(Some(Vec::new())),
        }
    }

    /// Loads `folder` as the chain enters it; false for a path this machine cannot name, and once
    /// the listing answered, which ends the chain.
    fn enter(&self, folder: &Path) -> bool {
        let Some(folder) = folder
            .to_str()
            .and_then(|folder| NativePath::parse(folder).ok())
        else {
            return false;
        };
        // The load happens under the lock, so none follows the listing's answer.
        let mut entered = self.lock();
        let Some(entered) = entered.as_mut() else {
            return false;
        };
        let fresh = self.watching.load(self.connection, self.workspace, &folder);
        entered.push((folder, fresh));
        true
    }

    /// Ends the chain of a listing that answered `listed`, or failed: each folder it loaded that
    /// the answer leaves out unloads.
    fn finish(&self, listed: Option<&FsListResult>) {
        let kept: HashSet<String> = listed
            .iter()
            .filter_map(|listed| listed.descendants.as_ref())
            .flat_map(|chain| chain.as_slice())
            .filter_map(|descendant| NativePath::parse(descendant.path.as_str()).ok())
            .map(|folder| folder.key())
            .collect();
        let entered = self.lock().take().unwrap_or_default();
        let left_out: Vec<NativePath> = entered
            .into_iter()
            .filter(|(folder, fresh)| *fresh && !kept.contains(&folder.key()))
            .map(|(folder, _)| folder)
            .collect();
        if !left_out.is_empty() {
            self.watching
                .unload(self.connection, self.workspace, &left_out);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Vec<(NativePath, bool)>>> {
        self.entered.lock().unwrap_or_else(PoisonError::into_inner)
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
