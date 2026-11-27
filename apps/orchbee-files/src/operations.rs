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
    CallClose, CallId, ConnectionId, ErrorCode, FilesError, FilesRecord, FilesReply, FilesRequest,
    GrantClass, LaneId, LinkRequest, RecordPlace, RequestError, ScopeUpdate, WorkspaceId,
    decode_files_record, parse_request,
};
use orchbee_local_runtime::DataRoot;

use crate::{
    blocking::Helpers,
    calls::{Call, Calls, DuplicateCall},
    fs::Engine,
    git, grants, roots,
    scope::{Scopes, View},
    subscriptions::{Leaving, Subscriptions},
};

/// Every service of the process and the state they share.
pub(crate) struct Services {
    calls: Arc<Calls>,
    scopes: Scopes,
    helpers: Helpers,
    /// The file engine's listings and read transfers.
    files: Engine,
    /// The `fs.subscribe` streams.
    roots: Subscriptions,
    /// Source control: discovery, status, Git operations and the prompt relay.
    git: git::Service,
}

impl Services {
    /// The services of one link; `root` keeps what they persist.
    pub(crate) fn new(calls: Arc<Calls>, root: Option<DataRoot>) -> Arc<Self> {
        let helpers = Helpers::new();
        Arc::new(Self {
            calls,
            scopes: Scopes::new(),
            files: Engine::new(helpers.clone()),
            helpers,
            roots: Subscriptions::new(),
            git: git::Service::start(root),
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
    }

    /// The Client closed `lane`, with every call on it.
    pub(crate) fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        self.calls.cancel_lane(connection, lane);
        self.roots.leave(Leaving::Lane(connection, lane));
        self.files.lane_closed(connection, lane);
    }

    /// The Client connection ended, with everything it held.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        self.calls.cancel_connection(connection);
        self.roots.leave(Leaving::Connection(connection));
        self.scopes.connection_lost(connection);
        self.files.connection_lost(connection);
        self.git.connection_lost(connection);
    }

    /// An Agent in `workspace` settled.
    pub(crate) fn agent_settled(&self, workspace: WorkspaceId) {
        self.git.agent_settled(workspace);
    }

    /// The Server stated a Workspace's scope, which applies to every call that starts after it.
    pub(crate) fn scope(self: &Arc<Self>, update: ScopeUpdate) {
        let workspace = update.workspace;
        self.git.scope(&update);
        if self.scopes.update(update) && self.roots.any_in(workspace) {
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
            // No operation of this build takes a Put.
            Ok(_) => return call.close(CallClose::Unsupported),
            Err(_) => return call.close(CallClose::InvalidRequest),
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
                    );
                });
            }
            FilesRequest::FsGrant(request) => match view.grantable(&request.path, request.source) {
                Ok(path) => {
                    // A file granted beyond the root brings its repository to source control.
                    let beyond = view.read(&request.path).is_err();
                    self.detach(call, move |services, call| {
                        if grants::grant(&services.scopes, &services.helpers, &view, call, path)
                            && beyond
                        {
                            services.git.opened(request.workspace_id, &request.path);
                        }
                    });
                }
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::FsUngrant(request) => {
                grants::ungrant(&self.scopes, &view, call, &request)
            }
            FilesRequest::FsList(request) => match view.read(&request.path) {
                Ok(path) => self.files.list(call, path, &request),
                Err(refused) => call.reply(&FilesReply::Error(refused)),
            },
            FilesRequest::FsStat(request) => {
                let paths = request.paths.as_slice().iter();
                let paths = paths.map(|path| (path.clone(), view.read(path))).collect();
                self.files.stat(call, paths);
            }
            FilesRequest::FsRead(request) => match view.read(&request.path) {
                Ok(path) => self.files.read(call, path, &request),
                Err(refused) => call.reply(&FilesReply::Error(refused)),
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
            FilesRequest::GitStage(request) => self.git.stage(call, &request),
            FilesRequest::GitUnstage(request) => self.git.unstage(call, &request),
            FilesRequest::GitDiscard(request) => self.git.discard(call, &request),
            FilesRequest::GitClean(request) => self.git.clean(call, &request),
            FilesRequest::GitCommit(request) => self.git.commit(call, &request),
            FilesRequest::GitFetch(request) => self.git.fetch(call, &request),
            FilesRequest::GitPull(request) => self.git.pull(call, &request),
            FilesRequest::GitSync(request) => self.git.sync(call, &request),
            FilesRequest::GitPush(request) => self.git.push(call, &request),
            FilesRequest::GitInit(request) => self.git.init(call, &request),
            FilesRequest::GitMarkSafe(request) => self.git.mark_safe(call, &request),
            FilesRequest::RepositoryConsent(request) => self.git.consent(call, &request),
            FilesRequest::RepositoryClose(request) => self.git.close(call, &request),
            FilesRequest::RelaySubscribe(request) => {
                let lifecycle = grant == GrantClass::Lifecycle;
                self.git
                    .subscribe_relay(call, request.workspace_id, lifecycle);
            }
            FilesRequest::RelayAnswer(request) => self.git.answer(call, &request),
            FilesRequest::ClientFocus(request) => self.git.focus(call, &request),
            FilesRequest::RequestOutcome(request) => self.git.outcome(call, &request),
            // git.show answers through read transfers, which read only files, and content
            // staging takes Puts, which no operation of this build reads.
            FilesRequest::FsRelease(_)
            | FilesRequest::GitShow(_)
            | FilesRequest::GitStageContent(_) => call.close(CallClose::Unsupported),
        }
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
