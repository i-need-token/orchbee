//! Routes each relayed record to the service that serves it, and each link event to the services
//! whose state it ends. Routing runs on the link's reader: a record is decoded, its operation's
//! grant class and Workspace are checked, and its call starts under the Workspace's current scope.
//! An operation that touches the filesystem moves to a thread of its own, so the reader never
//! waits on a disk. An operation no service serves closes as unsupported, which a Client reads as
//! a machine without it.
//!
//! A service joins in three places, all in this module: its state as a field of [`Services`], its
//! operations as arms of `route`, which hand it the call and the [`View`] it started under, and
//! the link events whose state it holds: a cancel, a lane's close, a connection's loss, a scope
//! change and the drain. A service's own modules take the shared parts they use, such as
//! [`Scopes`], [`Helpers`] or one [`Subscriptions`], and never this module, which depends on them.
//! A subscribing operation keeps its own [`Subscriptions`], whose members every link event that
//! ends calls also ends.

use std::{
    sync::{Arc, mpsc},
    thread,
};

use orchbee_files_protocol::{
    CallClose, CallId, ConnectionId, ErrorCode, FilePath, FilesError, FilesRecord, FilesReply,
    FilesRequest, GrantClass, LaneId, LinkRequest, ReadSource, RecordPlace, RequestError,
    ScopeUpdate, WorkspaceId, decode_files_record, parse_request,
};
use orchbee_local_runtime::DataRoot;

use crate::{
    blocking::Helpers,
    calls::{Call, Calls, DuplicateCall},
    edit,
    fs::{Engine, MachineTrash},
    git::{self, ports::Trash},
    grants, roots,
    scope::{Scopes, View, path::NativePath},
    subscriptions::{Leaving, Subscriptions},
};

/// Every service of the process and the state they share.
pub(crate) struct Services {
    calls: Arc<Calls>,
    scopes: Arc<Scopes>,
    helpers: Helpers,
    /// The file engine's listings, read transfers and watches.
    files: Engine,
    /// The `fs.subscribe` streams.
    roots: Arc<Subscriptions>,
    /// Source control: discovery, status, Git operations and the prompt relay.
    git: git::Service,
    /// File edits: writes and the other `files-edit` operations.
    edit: edit::Service,
}

impl Services {
    /// The services of one link; `root` keeps what they persist.
    pub(crate) fn new(calls: Arc<Calls>, root: Option<DataRoot>) -> Arc<Self> {
        let helpers = Helpers::new();
        let scopes = Arc::new(Scopes::new());
        let roots = Arc::new(Subscriptions::new());
        let files = Engine::new(helpers.clone(), Arc::clone(&scopes), Arc::clone(&roots));
        let watches = files.repository_watches();
        let trash: Arc<dyn Trash> = Arc::new(MachineTrash);
        let git = git::Service::start(root, Arc::clone(&scopes), watches, Arc::clone(&trash));
        let edit = edit::Service::new(helpers.clone(), trash, Arc::new(edit::Unobserved));
        Arc::new(Self {
            calls,
            files,
            scopes,
            helpers,
            roots,
            git,
            edit,
        })
    }

    /// Serves one relayed record.
    ///
    /// # Errors
    ///
    /// [`DuplicateCall`] when the Server named a call that is still open.
    pub(crate) fn request(self: &Arc<Self>, request: LinkRequest) -> Result<(), DuplicateCall> {
        if let Some(call) = self
            .calls
            .open(request.call, request.connection, request.lane)?
        {
            self.start(call, request.workspace, request.grant, &request.record);
        }
        Ok(())
    }

    /// The Client cancelled call `id`.
    pub(crate) fn cancel(&self, id: CallId) {
        self.calls.cancel(id);
        self.roots.leave(Leaving::Call(id));
        self.files.interest_changed(None);
    }

    /// The Client closed `lane`, with every call on it.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.calls.cancel_lane(connection, lane);
        self.roots.leave(Leaving::Lane(connection, lane));
        self.files.lane_closed(connection, lane);
        self.files.interest_changed(None);
    }

    /// The Client connection ended, with everything it held.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.calls.cancel_connection(connection);
        self.roots.leave(Leaving::Connection(connection));
        self.scopes.connection_lost(connection);
        self.files.connection_lost(connection);
        self.git.connection_lost(connection);
        self.edit.connection_lost(connection);
    }

    /// An Agent in `workspace` settled.
    pub(crate) fn agent_settled(&self, workspace: WorkspaceId) {
        self.git.agent_settled(workspace);
    }

    /// The Server stated a Workspace's scope, which applies to every call that starts after it.
    pub(crate) fn scope(self: &Arc<Self>, update: ScopeUpdate) {
        let workspace = update.workspace;
        let statement = update.clone();
        let changed = self.scopes.update(update);
        // The engine learns of the statement after the scopes, so the members it states for the
        // Workspace find it stated.
        self.git.scope(&statement);
        if changed {
            self.files.interest_changed(Some(workspace));
        }
        if changed && self.roots.any_in(workspace) {
            let services = Arc::clone(self);
            let refresh = move || {
                roots::refresh(
                    &services.scopes,
                    &services.helpers,
                    &services.roots,
                    workspace,
                );
            };
            // Without a thread the helper cannot start either, so the refresh reports the root as
            // stated at once.
            if let Err(refresh) = on_thread(refresh, |refresh| refresh()) {
                refresh();
            }
        }
    }

    /// Refuses every new call and cancels every open one, ending every subscription.
    pub(crate) fn drain(&self) {
        self.calls.drain();
        self.roots.leave(Leaving::All);
        self.files.drain();
    }

    fn start(
        self: &Arc<Self>,
        call: Call,
        workspace: Option<WorkspaceId>,
        grant: GrantClass,
        record: &[u8],
    ) {
        let json = match decode_files_record(record, RecordPlace::RequestOpen) {
            Ok(FilesRecord::Request(json)) => json,
            // A Put names no Workspace: it brings data to an upload its connection opened.
            Ok(FilesRecord::Put(put)) => return self.files.put(call, put),
            Ok(_) | Err(_) => return call.close(CallClose::InvalidRequest),
        };
        let request = match parse_request(&json) {
            Ok(request) => request,
            Err(RequestError::Unsupported) => return call.close(CallClose::Unsupported),
            Err(RequestError::Invalid) => return call.close(CallClose::InvalidRequest),
        };
        // The Server read the envelope's Workspace from this record, so they agree.
        if workspace != Some(request.workspace()) {
            return call.close(CallClose::InvalidRequest);
        }
        let operation = request.operation();
        if operation.grant() == GrantClass::Lifecycle && grant != GrantClass::Lifecycle {
            return call.reply(&FilesReply::Error(FilesError::new(ErrorCode::Denied)));
        }
        match self
            .scopes
            .view(request.workspace(), call.connection(), operation)
        {
            Ok(view) => self.route(call, view, grant, request),
            Err(refused) => call.reply(&FilesReply::Error(refused)),
        }
    }

    fn route(self: &Arc<Self>, call: Call, view: View, grant: GrantClass, request: FilesRequest) {
        match request {
            FilesRequest::FsSubscribe(request) => {
                let workspace = request.workspace_id;
                self.detach(call, move |services, call| {
                    roots::subscribe(
                        &services.scopes,
                        &services.helpers,
                        &services.roots,
                        workspace,
                        call,
                        || services.files.watch_state(workspace),
                    );
                    services.files.subscribed(workspace);
                });
            }
            FilesRequest::FsGrant(request) => match view.grantable(&request.path, request.source) {
                Ok(path) => {
                    // A file granted beyond the root brings its repository to source control.
                    let beyond = view.read(&request.path).is_err();
                    self.detach(call, move |services, call| {
                        let (scopes, helpers) = (&services.scopes, &services.helpers);
                        if grants::grant(scopes, helpers, &view, call, path, request.source)
                            && beyond
                        {
                            services.git.opened(request.workspace_id, &request.path);
                        }
                        services.files.interest_changed(Some(request.workspace_id));
                    });
                }
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::FsUngrant(request) => {
                grants::ungrant(&self.scopes, &view, call, &request);
                self.files.interest_changed(Some(request.workspace_id));
            }
            FilesRequest::FsRelease(request) => self.files.release(call, &request),
            FilesRequest::FsList(request) => match view.read(&request.path) {
                Ok(path) => self.files.list(call, path, &request),
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::FsStat(request) => {
                let paths = request.paths.as_slice().iter();
                let paths = paths
                    .map(|path| (path.clone(), self.readable(&call, &view, path)))
                    .collect();
                self.files.stat(call, paths);
            }
            FilesRequest::FsRead(request) => match &request.source {
                ReadSource::Path(path) => match self.readable(&call, &view, path) {
                    Ok(path) => self.files.read(call, path, &request),
                    Err(refused) => call.reply(&FilesReply::Error(refused)),
                },
                ReadSource::Export { export, index } => {
                    let workspace = request.workspace_id;
                    match self.edit.exported(&call, workspace, *export, index.get()) {
                        Ok(path) => self.files.read(call, path, &request),
                        Err(refused) => call.reply(&FilesReply::Error(refused)),
                    }
                }
            },
            FilesRequest::FsReadChunk(request) => self.files.read_chunk(call, &request),
            FilesRequest::FsGuessEncoding(request) => match view.read(&request.path) {
                Ok(path) => self.files.guess_encoding(call, path),
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::TransferClose(request) => self.files.close_transfer(call, &request),
            FilesRequest::ScmSubscribe(request) => self.git.subscribe(call, request.workspace_id),
            FilesRequest::GitStatusPage(request) => self.git.status_page(call, &request),
            FilesRequest::GitCheckIgnore(request) => self.git.check_ignore(call, &request),
            FilesRequest::GitRefs(request) => self.git.refs(call, &request),
            FilesRequest::GitUserConfig(request) => self.git.user_config(call, &request),
            FilesRequest::GitOutput(request) => self.git.output(call, &request),
            FilesRequest::GitStage(request) => self.git.stage(call, view, &request),
            FilesRequest::GitUnstage(request) => self.git.unstage(call, view, &request),
            FilesRequest::GitDiscard(request) => self.git.discard(call, view, &request),
            FilesRequest::GitClean(request) => self.git.clean(call, view, &request),
            FilesRequest::GitCommit(request) => self.git.commit(call, view, &request),
            FilesRequest::GitFetch(request) => self.git.fetch(call, view, &request),
            FilesRequest::GitPull(request) => self.git.pull(call, view, &request),
            FilesRequest::GitSync(request) => self.git.sync(call, view, &request),
            FilesRequest::GitPush(request) => self.git.push(call, view, &request),
            FilesRequest::GitInit(request) => self.git.init(call, view, &request),
            FilesRequest::GitMarkSafe(request) => self.git.mark_safe(call, view, &request),
            FilesRequest::RepositoryConsent(request) => self.git.consent(call, &request),
            FilesRequest::RepositoryClose(request) => self.git.close(call, &request),
            FilesRequest::RelaySubscribe(request) => {
                let lifecycle = grant == GrantClass::Lifecycle;
                self.git
                    .subscribe_relay(call, request.workspace_id, lifecycle);
            }
            FilesRequest::RelayAnswer(request) => self.git.answer(call, &request),
            FilesRequest::ClientFocus(request) => self.git.focus(call, &request),
            FilesRequest::RequestOutcome(request) => {
                if let Some(call) = self.edit.outcome(call, &request) {
                    self.git.outcome(call, &request);
                }
            }
            FilesRequest::GitShow(request) => self.git.show(call, &request, |call| {
                let (mode, limit) = (request.mode, request.limit.get());
                let encoding = request.encoding.as_ref();
                self.files
                    .show_read(call, request.transfer, mode, encoding, limit)
            }),
            FilesRequest::FsWrite(request) => {
                // An open upload holds the services weakly, so it never keeps them alive.
                let services = Arc::downgrade(self);
                let (transfer, size) = (request.transfer, request.size.get());
                self.files
                    .upload(call, transfer, size, move |call, content| {
                        if let Some(services) = services.upgrade() {
                            services.detach(call, move |services, call| {
                                services.edit.write(call, view, &request, content);
                            });
                        }
                    });
            }
            FilesRequest::FsCreate(request) => self.detach(call, move |services, call| {
                services.edit.create(call, view, &request);
            }),
            FilesRequest::FsRename(request) => self.detach(call, move |services, call| {
                services.edit.rename(call, view, &request);
            }),
            FilesRequest::FsMove(request) => self.detach(call, move |services, call| {
                services
                    .edit
                    .transfer(call, view, &request, edit::Transfer::Move);
            }),
            FilesRequest::FsCopy(request) => self.detach(call, move |services, call| {
                services
                    .edit
                    .transfer(call, view, &request, edit::Transfer::Copy);
            }),
            FilesRequest::FsExists(request) => self.detach(call, move |services, call| {
                services.edit.exists(call, &view, &request);
            }),
            FilesRequest::FsDelete(request) => self.detach(call, move |services, call| {
                services.edit.delete(call, view, &request);
            }),
            FilesRequest::FsRestore(request) => self.detach(call, move |services, call| {
                services.edit.restore(call, view, &request);
            }),
            FilesRequest::FsClipboard(request) => self.detach(call, move |services, call| {
                services.edit.clipboard(call, view, &request);
            }),
            FilesRequest::PickerOpen(request) => match self.edit.open_picker(&call, &request) {
                Ok(()) => self.detach(call, move |services, call| {
                    services.edit.picker_places(call, &request);
                }),
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::PickerList(request) => match self.edit.picker_folder(&call, &request) {
                Ok(folder) => self.files.picker_list(call, folder, request.cursor),
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::PickerClose(request) => self.edit.close_picker(call, &request),
            FilesRequest::FsImportBegin(_)
            | FilesRequest::FsImportItems(_)
            | FilesRequest::FsImportFile(_)
            | FilesRequest::FsImportEnd(_) => call.close(CallClose::Unsupported),
            FilesRequest::FsExportBegin(request) => self.edit.export_begin(call, view, &request),
            FilesRequest::FsExportEntries(request) => self.detach(call, move |services, call| {
                services.edit.export_entries(call, &request);
            }),
            FilesRequest::GitStageContent(request) => {
                // An open upload holds the services weakly, so it never keeps them alive.
                let services = Arc::downgrade(self);
                let (transfer, size) = (request.transfer, request.size.get());
                self.files
                    .upload(call, transfer, size, move |call, content| {
                        if let Some(services) = services.upgrade() {
                            services.git.stage_content(call, view, &request, content);
                        }
                    });
            }
        }
    }

    /// `path` for a read under `view`, or as one of the call's connection's clipboard sources,
    /// which a paste reads after the root moves.
    fn readable(
        &self,
        call: &Call,
        view: &View,
        path: &FilePath,
    ) -> Result<NativePath, FilesError> {
        view.read(path)
            .or_else(|refused| self.edit.clipboard_source(call, path).ok_or(refused))
    }

    /// Runs `work` with `call` on a thread of its own, or closes the call as over capacity when no
    /// thread can start.
    fn detach(self: &Arc<Self>, call: Call, work: impl FnOnce(&Self, Call) + Send + 'static) {
        let services = Arc::clone(self);
        if let Err(call) = on_thread(call, move |call| work(&services, call)) {
            call.close(CallClose::Capacity);
        }
    }
}

/// Runs `work` with `value` on a new thread, or hands `value` back when no thread can start.
fn on_thread<T: Send + 'static>(value: T, work: impl FnOnce(T) + Send + 'static) -> Result<(), T> {
    let (handover, received) = mpsc::sync_channel(1);
    let started = thread::Builder::new()
        .name("orchbee-files-call".into())
        .spawn(move || {
            if let Ok(value) = received.recv() {
                work(value);
            }
        });
    match started {
        Ok(_) => handover.send(value).map_err(|unsent| unsent.0),
        Err(_) => Err(value),
    }
}

#[cfg(test)]
mod tests;
