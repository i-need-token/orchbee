//! The Git engine's side of the dispatch: each request the dispatch routes here becomes a call to
//! the engine, the engine's answer becomes the call's Result record, and a subscription's frames
//! become its Snapshot and Update records. Each record is measured against its bound before it is
//! queued, since the call queue ends the process for one over it; the engine's pages, summaries and
//! prompts keep within the bound, so the measure only turns a defect into a failed call.

use super::{
    engine::{self, Git, Mutation, Sink},
    errors::GitError,
    mutations::{Commit, Message},
    ports::{NoTrash, NoWatches},
    records,
    relay::Answer,
    remote::{self, Fetch, Push},
};
use crate::calls::{Call, Events};
use orchbee_files_protocol::{
    CallClose, ClientFocus, CommitMessage, ConnectionId, Counter, ErrorCode, FetchRemotes,
    FilePath, FilesError, FilesNotice, FilesReply, GitCheckIgnore, GitClean, GitCommit, GitFetch,
    GitOutput, GitPath, GitPaths, GitPull, GitPush, GitRefs, GitStatusPage, MAXIMUM_REPLY_BYTES,
    PromptAnswer, RelayAnswer, RepositoryRequest, RequestId, RequestOutcome, ScopeUpdate,
    WorkspaceId, encode_notice, encode_reply,
};
use orchbee_local_runtime::DataRoot;
use std::sync::Arc;

/// The Git engine as the dispatch reaches it.
pub(crate) struct Service {
    /// `None` when the engine's thread could not start; every Git call then closes as over
    /// capacity.
    git: Option<Git>,
}

impl Service {
    /// Starts the engine; `root` keeps the consents to repositories beyond a root.
    pub(crate) fn start(root: Option<DataRoot>) -> Self {
        Self {
            // The file engine's adapters replace these stand-ins once it implements the ports.
            git: Git::start(root, Arc::new(NoTrash), Box::new(NoWatches)).ok(),
        }
    }

    /// The Server stated a Workspace's scope.
    pub(crate) fn scope(&self, update: &ScopeUpdate) {
        if let Some(git) = &self.git {
            git.scope(update.workspace, update.scope.clone());
        }
    }

    /// An Agent in `workspace` settled, which refreshes its statuses at once.
    pub(crate) fn agent_settled(&self, workspace: WorkspaceId) {
        if let Some(git) = &self.git {
            git.agent_settled(workspace);
        }
    }

    /// A Client connection ended with its subscriptions, focus and prompts.
    pub(crate) fn connection_lost(&self, connection: ConnectionId) {
        if let Some(git) = &self.git {
            git.connection_lost(connection);
        }
    }

    /// A connection was granted `path`, a file beyond its Workspace's root, whose repository then
    /// becomes a candidate.
    pub(crate) fn opened(&self, workspace: WorkspaceId, path: &FilePath) {
        if let Some(git) = &self.git {
            git.opened(workspace, path.as_str().to_owned());
        }
    }

    /// `scm.subscribe`.
    pub(crate) fn subscribe(&self, call: Call, workspace: WorkspaceId) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let connection = call.connection();
        git.subscribe(
            workspace,
            connection,
            Box::new(Stream::new(call, records::summary)),
        );
    }

    /// `relay.subscribe`; a connection is routed prompts only when it holds `lifecycle`.
    pub(crate) fn subscribe_relay(&self, call: Call, workspace: WorkspaceId, lifecycle: bool) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let connection = call.connection();
        git.subscribe_relay(
            workspace,
            connection,
            lifecycle,
            Box::new(Stream::new(call, records::relayed)),
        );
    }

    /// `client.focus`.
    pub(crate) fn focus(&self, call: Call, request: &ClientFocus) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let connection = call.connection();
        git.focus(
            request.workspace_id,
            connection,
            request.focused,
            replying(call, records::empty),
        );
    }

    /// `git.status-page`.
    pub(crate) fn status_page(&self, call: Call, request: &GitStatusPage) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        git.status_page(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            request.revision.get(),
            request.cursor.map_or(0, records::page_index),
            replying(call, records::status_page),
        );
    }

    /// `git.check-ignore`.
    pub(crate) fn check_ignore(&self, call: Call, request: &GitCheckIgnore) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.check_ignore(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            strings(request.paths.as_slice()),
            cancel,
            replying(call, records::check_ignore),
        );
    }

    /// `git.refs`.
    pub(crate) fn refs(&self, call: Call, request: &GitRefs) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.refs(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            request.cursor.map(records::page_index),
            cancel,
            replying(call, records::refs),
        );
    }

    /// `git.user-config`.
    pub(crate) fn user_config(&self, call: Call, request: &RepositoryRequest) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.user_config(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            cancel,
            replying(call, records::user_config),
        );
    }

    /// `git.output`.
    pub(crate) fn output(&self, call: Call, request: &GitOutput) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        git.output(
            request.workspace_id,
            request.cursor.map(Counter::get),
            replying(call, records::output),
        );
    }

    /// `repository.consent`.
    pub(crate) fn consent(&self, call: Call, request: &RepositoryRequest) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        git.consent(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            replying(call, records::empty),
        );
    }

    /// `repository.close`.
    pub(crate) fn close(&self, call: Call, request: &RepositoryRequest) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        git.close(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            replying(call, records::empty),
        );
    }

    /// `git.stage`.
    pub(crate) fn stage(&self, call: Call, request: &GitPaths) {
        let mutation = Mutation::Stage(strings(request.paths.as_slice()));
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.unstage`.
    pub(crate) fn unstage(&self, call: Call, request: &GitPaths) {
        let mutation = Mutation::Unstage(strings(request.paths.as_slice()));
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.discard`.
    pub(crate) fn discard(&self, call: Call, request: &GitPaths) {
        let mutation = Mutation::Discard(strings(request.paths.as_slice()));
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.clean`.
    pub(crate) fn clean(&self, call: Call, request: &GitClean) {
        let mutation = Mutation::Clean {
            paths: strings(request.paths.as_slice()),
            mode: request.mode,
        };
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.commit`.
    pub(crate) fn commit(&self, call: Call, request: &GitCommit) {
        let mutation = Mutation::Commit(Commit {
            message: match &request.message {
                CommitMessage::Text(text) => Message::Text(text.as_str().to_owned()),
                CommitMessage::Editor => Message::Editor,
            },
            amend: request.amend,
            signoff: request.signoff,
            all: request.all,
            no_verify: request.no_verify,
        });
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.fetch`.
    pub(crate) fn fetch(&self, call: Call, request: &GitFetch) {
        let mutation = Mutation::Fetch {
            fetch: Fetch {
                remotes: match &request.remotes {
                    FetchRemotes::Default => remote::FetchRemotes::Default,
                    FetchRemotes::Remote(name) => {
                        remote::FetchRemotes::Remote(name.as_str().to_owned())
                    }
                    FetchRemotes::All => remote::FetchRemotes::All,
                },
                prune: request.prune,
            },
            background: request.background,
        };
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.pull`.
    pub(crate) fn pull(&self, call: Call, request: &GitPull) {
        let mutation = Mutation::Pull {
            rebase: request.rebase,
        };
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.sync`.
    pub(crate) fn sync(&self, call: Call, request: &GitPull) {
        let mutation = Mutation::Sync {
            rebase: request.rebase,
        };
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.push`.
    pub(crate) fn push(&self, call: Call, request: &GitPush) {
        let mutation = Mutation::Push(Push {
            remote: request.remote.as_ref().map(|name| name.as_str().to_owned()),
            branch: request.branch.as_ref().map(|name| name.as_str().to_owned()),
            set_upstream: request.set_upstream,
            force: request.force,
            tags: request.tags,
        });
        self.mutate(
            call,
            request.workspace_id,
            request.request_id,
            &request.repository,
            mutation,
        );
    }

    /// `git.init`.
    pub(crate) fn init(&self, call: Call, request: &GitPath) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let caller = caller(&call, request.workspace_id, request.request_id);
        git.init(
            caller,
            request.path.as_str().to_owned(),
            replying(call, records::done),
        );
    }

    /// `git.mark-safe`.
    pub(crate) fn mark_safe(&self, call: Call, request: &GitPath) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let caller = caller(&call, request.workspace_id, request.request_id);
        git.mark_safe(
            caller,
            request.path.as_str().to_owned(),
            replying(call, records::done),
        );
    }

    /// `relay.answer`.
    pub(crate) fn answer(&self, call: Call, request: &RelayAnswer) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let answer = match &request.answer {
            PromptAnswer::Value(value) => Answer::Value(value.as_str().to_owned()),
            PromptAnswer::Empty => Answer::Empty,
            PromptAnswer::Cancelled => Answer::Cancelled,
        };
        let connection = call.connection();
        git.answer(
            request.workspace_id,
            connection,
            request.prompt,
            answer,
            replying(call, records::empty),
        );
    }

    /// `request.outcome`.
    pub(crate) fn outcome(&self, call: Call, request: &RequestOutcome) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        git.outcome(
            request.workspace_id,
            request.request_id,
            replying(call, records::outcome),
        );
    }

    /// A write to `repository` under request identity `request`.
    fn mutate(
        &self,
        call: Call,
        workspace: WorkspaceId,
        request: RequestId,
        repository: &FilePath,
        mutation: Mutation,
    ) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let caller = caller(&call, workspace, request);
        git.mutate(
            caller,
            repository.as_str().to_owned(),
            mutation,
            replying(call, records::done),
        );
    }

    /// The engine with `call`, or nothing once `call` closed as over capacity because the engine's
    /// thread never started.
    fn engine(&self, call: Call) -> Option<(&Git, Call)> {
        if let Some(git) = &self.git {
            return Some((git, call));
        }
        call.close(CallClose::Capacity);
        None
    }
}

/// The identity a write carries through the engine, which cancels with `call`.
fn caller(call: &Call, workspace: WorkspaceId, request: RequestId) -> engine::Call {
    engine::Call {
        workspace,
        connection: call.connection(),
        request,
        cancel: call.cancellation(),
    }
}

/// The reply that ends `call` with the Result record `record` makes of the engine's answer. An
/// engine that stops first drops it, which closes the call without one.
fn replying<T: 'static>(
    call: Call,
    record: fn(Result<T, GitError>) -> FilesReply,
) -> engine::Reply<T> {
    Box::new(move |answered| finish(call, &record(answered)))
}

/// Ends `call` with `reply`, or as failed when its record would pass the Result bound.
fn finish(call: Call, reply: &FilesReply) {
    if encode_reply(reply).len() <= MAXIMUM_REPLY_BYTES {
        call.reply(reply);
    } else {
        call.reply(&FilesReply::Error(FilesError::new(ErrorCode::Failed)));
    }
}

fn strings(paths: &[FilePath]) -> Vec<String> {
    paths.iter().map(|path| path.as_str().to_owned()).collect()
}

/// A subscription on its way through the engine: the first frame it carries becomes the Snapshot
/// that opens its stream, and each later one an Update. A frame whose record would pass the
/// Notice bound is left out.
struct Stream<T> {
    phase: Option<Phase>,
    notice: fn(&T) -> Option<FilesNotice>,
}

enum Phase {
    Waiting(Call),
    Open(Events),
}

impl<T> Stream<T> {
    const fn new(call: Call, notice: fn(&T) -> Option<FilesNotice>) -> Self {
        Self {
            phase: Some(Phase::Waiting(call)),
            notice,
        }
    }
}

impl<T> Sink<T> for Stream<T> {
    fn send(&mut self, frame: &T) {
        let Some(notice) = (self.notice)(frame)
            .filter(|notice| encode_notice(notice).len() <= MAXIMUM_REPLY_BYTES)
        else {
            return;
        };
        self.phase = match self.phase.take() {
            Some(Phase::Waiting(call)) => Some(Phase::Open(call.snapshot(&notice))),
            Some(Phase::Open(events)) => {
                events.update(&notice);
                Some(Phase::Open(events))
            }
            None => None,
        };
    }

    fn ended(&self) -> bool {
        match &self.phase {
            Some(Phase::Waiting(call)) => call.cancelled(),
            Some(Phase::Open(events)) => events.cancelled(),
            None => true,
        }
    }
}
