//! The Git engine: one thread owns every Workspace's repositories, write queues, request
//! identities, prompts and subscriptions, and each Git run gets a thread of its own that reports
//! its end back. Per repository one write runs at a time, in arrival order, while reads run beside
//! it. A fetch joins one already queued or running when it would add nothing: a background fetch
//! joins any, a foreground fetch only an identical foreground one, and the shared run stops only
//! once every caller cancelled. A prompt goes to the Client that started its operation, else to
//! the most recently focused Client that holds `lifecycle` and subscribes to the Workspace's
//! relay; a background operation's prompt, or one no Client can answer, gets an empty answer at
//! once.
//!
//! The engine reads no settings: autofetch is the Client's timer, and every request carries the
//! options VS Code would read from its configuration.

use super::{
    command::{self, Access, RelayVariables},
    consents::Consents,
    discovery::{self, Discovered, Found, Probe},
    errors::GitError,
    mutations::{self, Cleaned, Commit, Context},
    native_paths,
    output::{GitOutput, Page},
    ports::{Change, Members, Report, Trash, Watches},
    reads::{self, RefsPage, UserConfig},
    relay::{self, Answer, Ask, Relay, Token},
    remote::{self, Fetch, Push},
    runner::{self, Limits},
    status::{self, Snapshot},
    workspace::{Effect, StatusPage, Summary, Workspace},
};
use crate::requests::{Begin, Outcome, Requests};
use orchbee_files_protocol::{
    BusyOperation, CleanMode, ConnectionId, ErrorCode, GitProgram, GitState, PromptId, PromptKind,
    RepositoryKind, RequestId, Text, WorkspaceId, WorkspaceScope,
};
use orchbee_local_runtime::DataRoot;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsString,
    io,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// The longest the engine sleeps while it watches cancellations and ended subscriptions.
const POLL: Duration = Duration::from_millis(250);
/// The deadline of references, remotes, user configuration and `check-ignore`.
const READ_DEADLINE: Duration = Duration::from_secs(30);
/// How long the version check may take.
const VERSION_DEADLINE: Duration = Duration::from_secs(10);
/// `status --porcelain=v2` needs Git 2.11.
const OLDEST_VERSION: (u32, u32, u32) = (2, 11, 0);
/// The longest version text the summary carries.
const VERSION_TEXT_LIMIT: usize = 64;
/// A credential prompt closes unanswered after 10 minutes, an editor prompt after 8 hours.
const CREDENTIAL_PROMPT: Duration = Duration::from_secs(10 * 60);
const EDITOR_PROMPT: Duration = Duration::from_secs(8 * 60 * 60);
/// The prompts one connection holds at once, as many as its relay snapshot carries.
const PROMPTS_PER_CONNECTION: usize = 16;
/// The longest path a prompt names.
const PATH_LIMIT: usize = 4096;

/// Where a request's answer goes; dropping it unanswered ends the call without one.
pub(super) type Reply<T> = Box<dyn FnOnce(Result<T, GitError>) + Send>;
/// How a mutation ended, which a repeated request identity answers again.
pub(super) type Ended = Result<Done, GitError>;

/// A subscription's stream.
pub(super) trait Sink<T>: Send {
    /// Sends one frame; the first is the subscription's snapshot.
    fn send(&mut self, frame: &T);
    /// Whether the subscription ended, as its Client's cancel or its connection's loss ends it.
    fn ended(&self) -> bool;
}

/// One Workspace's source control summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScmSummary {
    pub(super) git: GitProgram,
    pub(super) summary: Summary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Prompt {
    pub(super) id: PromptId,
    pub(super) kind: PromptKind,
    pub(super) text: String,
    pub(super) repository: String,
    pub(super) path: Option<String>,
}

/// What a relay subscription streams: the prompts routed to its connection, then each change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RelayEvent {
    Snapshot(Vec<Prompt>),
    Opened(Prompt),
    /// Answered, timed out, moved to another Client, or ended with its operation.
    Closed(PromptId),
}

/// A write to one repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Mutation {
    Stage(Vec<String>),
    Unstage(Vec<String>),
    Discard(Vec<String>),
    #[expect(
        dead_code,
        reason = "git.stage-content is routed once the dispatch reads a transfer's Puts"
    )]
    StageContent {
        path: String,
        content: Vec<u8>,
    },
    Clean {
        paths: Vec<String>,
        mode: CleanMode,
    },
    Commit(Commit),
    Fetch {
        fetch: Fetch,
        background: bool,
    },
    Pull {
        rebase: bool,
    },
    Sync {
        rebase: bool,
    },
    Push(Push),
}

impl Mutation {
    const fn busy(&self) -> BusyOperation {
        match self {
            Self::Stage(_) => BusyOperation::Stage,
            Self::Unstage(_) => BusyOperation::Unstage,
            Self::Discard(_) => BusyOperation::Discard,
            Self::StageContent { .. } => BusyOperation::StageContent,
            Self::Clean { .. } => BusyOperation::Clean,
            Self::Commit(_) => BusyOperation::Commit,
            Self::Fetch { .. } => BusyOperation::Fetch,
            Self::Pull { .. } => BusyOperation::Pull,
            Self::Sync { .. } => BusyOperation::Sync,
            Self::Push(_) => BusyOperation::Push,
        }
    }

    /// Whether its commands may ask for a credential or a message.
    const fn prompts(&self) -> bool {
        matches!(
            self,
            Self::Commit(_)
                | Self::Fetch { .. }
                | Self::Pull { .. }
                | Self::Sync { .. }
                | Self::Push(_)
        )
    }

    /// The paths it names, which must be well formed.
    fn paths(&self) -> &[String] {
        match self {
            Self::Stage(paths)
            | Self::Unstage(paths)
            | Self::Discard(paths)
            | Self::Clean { paths, .. } => paths,
            Self::StageContent { path, .. } => std::slice::from_ref(path),
            _ => &[],
        }
    }

    fn limits(&self, cancel: Arc<AtomicBool>, prompts: Option<Arc<AtomicUsize>>) -> Limits {
        let (kind, deadline, idle) = match self {
            Self::Stage(_) => ("stage", mutations::BATCH_DEADLINE, None),
            Self::Unstage(_) => ("unstage", mutations::BATCH_DEADLINE, None),
            Self::Discard(_) => ("discard", mutations::BATCH_DEADLINE, None),
            Self::StageContent { .. } => ("stage-content", mutations::BATCH_DEADLINE, None),
            Self::Clean { .. } => ("clean", mutations::BATCH_DEADLINE, None),
            Self::Commit(commit) => ("commit", mutations::commit_deadline(commit), None),
            Self::Fetch { .. } => ("fetch", remote::DEADLINE, Some(remote::IDLE)),
            Self::Pull { .. } => ("pull", remote::DEADLINE, Some(remote::IDLE)),
            Self::Sync { .. } => ("sync", remote::DEADLINE, Some(remote::IDLE)),
            Self::Push(_) => ("push", remote::DEADLINE, Some(remote::IDLE)),
        };
        Limits {
            kind,
            deadline,
            idle,
            cancel,
            prompts,
        }
    }
}

/// What a mutation answers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Done {
    Empty,
    Cleaned(Vec<Cleaned>),
    /// The commit HEAD names afterwards.
    Committed(String),
    /// The new repository's canonical toplevel.
    Initialized(String),
}

/// The call a mutation arrived on.
pub(super) struct Call {
    pub(super) workspace: WorkspaceId,
    pub(super) connection: ConnectionId,
    pub(super) request: RequestId,
    pub(super) cancel: Arc<AtomicBool>,
}

/// What a read runs with once the engine found its repository.
struct Reading {
    found: Found,
    version: (u32, u32, u32),
    output: Arc<Mutex<GitOutput>>,
}

type ReadRun = Box<dyn FnOnce(Result<Reading, GitError>) + Send>;

/// The engine's handle. Requests reach its thread in order; each answers through its reply.
pub(super) struct Git {
    sender: Sender<Message>,
    actor: Option<JoinHandle<()>>,
}

impl Git {
    /// Starts the engine: checks the machine's Git and serves requests. `root`
    /// keeps the consents to parent repositories; without it they last as long as the process.
    pub(super) fn start(
        root: Option<DataRoot>,
        trash: Arc<dyn Trash>,
        watches: Box<dyn Watches>,
        members: Box<dyn Members>,
    ) -> io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let own = sender.clone();
        let actor = thread::Builder::new()
            .name("orchbee-files-git".into())
            .spawn(move || Actor::new(own, root, trash, watches, members).run(&receiver))?;
        Ok(Self {
            sender,
            actor: Some(actor),
        })
    }

    pub(super) fn scope(&self, workspace: WorkspaceId, scope: WorkspaceScope) {
        self.send(Message::Scope(workspace, scope));
    }

    /// An Agent in the Workspace settled.
    pub(super) fn agent_settled(&self, workspace: WorkspaceId) {
        self.send(Message::AgentSettled(workspace));
    }

    /// A Client connection ended with everything it held.
    pub(super) fn connection_lost(&self, connection: ConnectionId) {
        self.send(Message::ConnectionLost(connection));
    }

    /// A Client opened `path`, a file outside the root, whose repository becomes a candidate.
    pub(super) fn opened(&self, workspace: WorkspaceId, path: String) {
        self.send(Message::Opened(workspace, path));
    }

    /// `scm.subscribe`.
    pub(super) fn subscribe(
        &self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        sink: Box<dyn Sink<ScmSummary>>,
    ) {
        self.send(Message::Subscribe {
            workspace,
            connection,
            sink,
        });
    }

    /// `relay.subscribe`; only a connection holding `lifecycle` is routed prompts.
    pub(super) fn subscribe_relay(
        &self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        lifecycle: bool,
        sink: Box<dyn Sink<RelayEvent>>,
    ) {
        self.send(Message::SubscribeRelay {
            workspace,
            connection,
            lifecycle,
            sink,
        });
    }

    /// `client.focus`.
    pub(super) fn focus(
        &self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        focused: bool,
        reply: Reply<()>,
    ) {
        self.send(Message::Focus {
            workspace,
            connection,
            focused,
            reply,
        });
    }

    /// `git.status-page`.
    pub(super) fn status_page(
        &self,
        workspace: WorkspaceId,
        repository: String,
        revision: u64,
        cursor: u64,
        reply: Reply<StatusPage>,
    ) {
        self.send(Message::StatusPage {
            workspace,
            repository,
            revision,
            cursor,
            reply,
        });
    }

    /// `git.check-ignore`.
    pub(super) fn check_ignore(
        &self,
        workspace: WorkspaceId,
        repository: String,
        paths: Vec<String>,
        cancel: Arc<AtomicBool>,
        reply: Reply<Vec<String>>,
    ) {
        if !paths.iter().all(|path| native_paths::well_formed(path)) {
            reply(Err(GitError::new(ErrorCode::InvalidRequest)));
            return;
        }
        self.read(workspace, repository, move |reading| {
            reply(reading.and_then(|reading| {
                let limits = Limits::new("check-ignore", READ_DEADLINE, cancel);
                reads::check_ignore(&reading.found, &paths, &limits, &reading.output)
            }));
        });
    }

    /// `git.refs`.
    pub(super) fn refs(
        &self,
        workspace: WorkspaceId,
        repository: String,
        cursor: Option<u64>,
        cancel: Arc<AtomicBool>,
        reply: Reply<RefsPage>,
    ) {
        self.read(workspace, repository, move |reading| {
            reply(reading.and_then(|reading| {
                let limits = Limits::new("refs", READ_DEADLINE, cancel);
                reads::refs(
                    &reading.found,
                    reading.version,
                    cursor,
                    &limits,
                    &reading.output,
                )
            }));
        });
    }

    /// `git.user-config`.
    pub(super) fn user_config(
        &self,
        workspace: WorkspaceId,
        repository: String,
        cancel: Arc<AtomicBool>,
        reply: Reply<UserConfig>,
    ) {
        self.read(workspace, repository, move |reading| {
            reply(reading.and_then(|reading| {
                let limits = Limits::new("config", mutations::CONFIG_DEADLINE, cancel);
                reads::user_config(&reading.found, &limits, &reading.output)
            }));
        });
    }

    /// `git.output`.
    pub(super) fn output(&self, workspace: WorkspaceId, cursor: Option<u64>, reply: Reply<Page>) {
        self.send(Message::Output {
            workspace,
            cursor,
            reply,
        });
    }

    /// `repository.consent`.
    pub(super) fn consent(&self, workspace: WorkspaceId, repository: String, reply: Reply<()>) {
        self.send(Message::Consent {
            workspace,
            repository,
            consented: true,
            reply,
        });
    }

    /// `repository.close`.
    pub(super) fn close(&self, workspace: WorkspaceId, repository: String, reply: Reply<()>) {
        self.send(Message::Consent {
            workspace,
            repository,
            consented: false,
            reply,
        });
    }

    /// A write to `repository`: stage, unstage, discard, content staging, clean, commit, fetch,
    /// pull, sync or push.
    pub(super) fn mutate(
        &self,
        call: Call,
        repository: String,
        mutation: Mutation,
        reply: Reply<Done>,
    ) {
        let paths = mutation.paths();
        if !paths.iter().all(|path| native_paths::well_formed(path)) {
            reply(Err(GitError::new(ErrorCode::InvalidRequest)));
            return;
        }
        self.send(Message::Mutate {
            call,
            repository,
            work: Work::Mutation(mutation),
            reply,
        });
    }

    /// `git.init` in a folder of the root.
    pub(super) fn init(&self, call: Call, path: String, reply: Reply<Done>) {
        self.path_work(call, path, Work::Init, reply);
    }

    /// `git.mark-safe` of a repository the summary lists as unsafe.
    pub(super) fn mark_safe(&self, call: Call, path: String, reply: Reply<Done>) {
        self.path_work(call, path, Work::MarkSafe, reply);
    }

    /// `request.outcome`.
    pub(super) fn outcome(
        &self,
        workspace: WorkspaceId,
        request: RequestId,
        reply: Reply<Outcome<Ended>>,
    ) {
        self.send(Message::Outcome {
            workspace,
            request,
            reply,
        });
    }

    /// `relay.answer` from `connection`, the one the prompt was routed to.
    pub(super) fn answer(
        &self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        prompt: PromptId,
        answer: Answer,
        reply: Reply<()>,
    ) {
        self.send(Message::Answer {
            workspace,
            connection,
            prompt,
            answer,
            reply,
        });
    }

    fn path_work(
        &self,
        call: Call,
        path: String,
        work: impl FnOnce(String) -> Work,
        reply: Reply<Done>,
    ) {
        if !native_paths::well_formed(&path) {
            reply(Err(GitError::new(ErrorCode::InvalidRequest)));
            return;
        }
        self.send(Message::Mutate {
            call,
            repository: path.clone(),
            work: work(path),
            reply,
        });
    }

    fn read(
        &self,
        workspace: WorkspaceId,
        repository: String,
        run: impl FnOnce(Result<Reading, GitError>) + Send + 'static,
    ) {
        self.send(Message::Read {
            workspace,
            repository,
            run: Box::new(run),
        });
    }

    /// Hands `message` to the engine; one sent after it stopped is dropped with its reply.
    fn send(&self, message: Message) {
        let _ = self.sender.send(message);
    }
}

/// Dropping the engine stops it: running commands are stopped, subscriptions end and the relay
/// endpoint goes. Replies it still held drop with it, which closes their calls without one.
impl Drop for Git {
    fn drop(&mut self) {
        if let Some(actor) = self.actor.take() {
            let _ = self.sender.send(Message::Stop);
            let _ = actor.join();
        }
    }
}

enum Message {
    Scope(WorkspaceId, WorkspaceScope),
    AgentSettled(WorkspaceId),
    ConnectionLost(ConnectionId),
    Changed(WorkspaceId, String, Change),
    Opened(WorkspaceId, String),
    Subscribe {
        workspace: WorkspaceId,
        connection: ConnectionId,
        sink: Box<dyn Sink<ScmSummary>>,
    },
    SubscribeRelay {
        workspace: WorkspaceId,
        connection: ConnectionId,
        lifecycle: bool,
        sink: Box<dyn Sink<RelayEvent>>,
    },
    Focus {
        workspace: WorkspaceId,
        connection: ConnectionId,
        focused: bool,
        reply: Reply<()>,
    },
    StatusPage {
        workspace: WorkspaceId,
        repository: String,
        revision: u64,
        cursor: u64,
        reply: Reply<StatusPage>,
    },
    Read {
        workspace: WorkspaceId,
        repository: String,
        run: ReadRun,
    },
    Output {
        workspace: WorkspaceId,
        cursor: Option<u64>,
        reply: Reply<Page>,
    },
    Consent {
        workspace: WorkspaceId,
        repository: String,
        consented: bool,
        reply: Reply<()>,
    },
    Mutate {
        call: Call,
        repository: String,
        work: Work,
        reply: Reply<Done>,
    },
    Outcome {
        workspace: WorkspaceId,
        request: RequestId,
        reply: Reply<Outcome<Ended>>,
    },
    Answer {
        workspace: WorkspaceId,
        connection: ConnectionId,
        prompt: PromptId,
        answer: Answer,
        reply: Reply<()>,
    },
    Discovered {
        workspace: WorkspaceId,
        generation: u64,
        discovered: Discovered,
    },
    Probed {
        workspace: WorkspaceId,
        found: Found,
    },
    StatusEnded {
        workspace: WorkspaceId,
        repository: String,
        generation: u64,
        result: Result<Snapshot, GitError>,
    },
    JobEnded {
        workspace: WorkspaceId,
        job: u64,
        ended: Ended,
    },
    Ask(Ask),
    Stop,
}

/// What a job runs.
enum Work {
    Mutation(Mutation),
    Init(String),
    MarkSafe(String),
}

/// One caller of a job, which a joined fetch shares with others.
struct Caller {
    request: RequestId,
    cancel: Arc<AtomicBool>,
    reply: Reply<Done>,
}

/// A queued or running write, or a running initialization or trust.
struct Job {
    id: u64,
    /// The repository a mutation writes to; none for a folder's job.
    found: Option<Found>,
    connection: ConnectionId,
    work: Option<Work>,
    /// The fetch it runs, which a later fetch may join.
    fetch: Option<(Fetch, bool)>,
    callers: Vec<Caller>,
    /// Stops the run: the caller's own cancel, or for a fetch, set once every caller cancelled.
    cancel: Arc<AtomicBool>,
    started: bool,
    token: Option<Token>,
}

impl Job {
    /// Whether the fetch `(fetch, background)` would add nothing to this job.
    fn joins(&self, fetch: &Fetch, background: bool) -> bool {
        self.fetch
            .as_ref()
            .is_some_and(|(running, ran_in_background)| {
                background || (!ran_in_background && running == fetch)
            })
    }
}

struct Subscriber {
    connection: ConnectionId,
    sink: Box<dyn Sink<ScmSummary>>,
}

struct RelaySubscriber {
    connection: ConnectionId,
    lifecycle: bool,
    sink: Box<dyn Sink<RelayEvent>>,
}

/// One operation whose helpers may prompt, by its token.
struct Operation {
    workspace: WorkspaceId,
    repository: String,
    connection: ConnectionId,
    background: bool,
    /// Its open prompts, during which the idle watchdog waits.
    prompts: Arc<AtomicUsize>,
    cancel: Arc<AtomicBool>,
}

/// An open prompt and where its answer goes.
struct Open {
    token: Token,
    workspace: WorkspaceId,
    connection: ConnectionId,
    prompt: Prompt,
    expires: Instant,
    answer: mpsc::Sender<Answer>,
}

/// One Workspace's state.
struct Space {
    workspace: Workspace,
    output: Arc<Mutex<GitOutput>>,
    requests: Requests<Ended>,
    /// Repeats of a running request identity, answered when it ends.
    waiters: HashMap<RequestId, Vec<Reply<Done>>>,
    subscribers: HashMap<u64, Subscriber>,
    relays: Vec<RelaySubscriber>,
    /// Each connection's last focus, by the engine's focus count.
    focused: HashMap<ConnectionId, u64>,
    discovery: Option<Arc<AtomicBool>>,
    statuses: HashMap<(String, u64), Arc<AtomicBool>>,
    /// Writes per repository toplevel, the running one first.
    queues: HashMap<String, VecDeque<Job>>,
    /// Running initializations and trusts.
    loose: HashMap<u64, Job>,
    dirty: bool,
    closed: bool,
}

impl Space {
    fn new() -> Self {
        Self {
            workspace: Workspace::new(),
            output: Arc::new(Mutex::new(GitOutput::new())),
            requests: Requests::new(),
            waiters: HashMap::new(),
            subscribers: HashMap::new(),
            relays: Vec::new(),
            focused: HashMap::new(),
            discovery: None,
            statuses: HashMap::new(),
            queues: HashMap::new(),
            loose: HashMap::new(),
            dirty: false,
            closed: false,
        }
    }

    /// The relay subscriber a prompt of an operation started by `initiator` goes to; none while
    /// the Workspace is closed, since no Client may answer there.
    fn route(&self, initiator: ConnectionId) -> Option<ConnectionId> {
        if self.closed {
            return None;
        }
        let eligible = || {
            self.relays
                .iter()
                .filter(|relay| relay.lifecycle && !relay.sink.ended())
        };
        if eligible().any(|relay| relay.connection == initiator) {
            return Some(initiator);
        }
        eligible()
            .max_by_key(|relay| self.focused.get(&relay.connection).copied().unwrap_or(0))
            .map(|relay| relay.connection)
    }

    fn relay_send(&mut self, connection: ConnectionId, event: &RelayEvent) {
        for relay in self
            .relays
            .iter_mut()
            .filter(|relay| relay.connection == connection)
        {
            relay.sink.send(event);
        }
    }
}

struct Actor {
    sender: Sender<Message>,
    program: GitProgram,
    version: (u32, u32, u32),
    /// Bound while an operation that may prompt runs.
    relay: Option<Relay>,
    /// The relay could not be offered once, which the log says once.
    relay_failed: bool,
    executable: Option<OsString>,
    consents: Consents,
    trash: Arc<dyn Trash>,
    watches: Box<dyn Watches>,
    members: Box<dyn Members>,
    /// The members last stated per Workspace; a Workspace without any is absent.
    stated_members: HashMap<WorkspaceId, Vec<String>>,
    spaces: HashMap<WorkspaceId, Space>,
    tokens: HashMap<Token, Operation>,
    prompts: Vec<Open>,
    next_id: u64,
    focus_count: u64,
}

impl Actor {
    fn new(
        sender: Sender<Message>,
        root: Option<DataRoot>,
        trash: Arc<dyn Trash>,
        watches: Box<dyn Watches>,
        members: Box<dyn Members>,
    ) -> Self {
        let (program, version) = program();
        Self {
            sender,
            program,
            version,
            relay: None,
            relay_failed: false,
            executable: std::env::current_exe().ok().map(OsString::from),
            consents: Consents::new(root),
            trash,
            watches,
            members,
            stated_members: HashMap::new(),
            spaces: HashMap::new(),
            tokens: HashMap::new(),
            prompts: Vec::new(),
            next_id: 0,
            focus_count: 0,
        }
    }

    fn run(mut self, receiver: &Receiver<Message>) {
        loop {
            let now = Instant::now();
            let mut wake = self.next_wake();
            if self.polling() {
                wake = Some(wake.map_or(now + POLL, |wake| wake.min(now + POLL)));
            }
            let message = match wake {
                Some(wake) => match receiver.recv_timeout(wake.saturating_duration_since(now)) {
                    Ok(message) => Some(message),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match receiver.recv() {
                    Ok(message) => Some(message),
                    Err(_) => break,
                },
            };
            let now = Instant::now();
            match message {
                Some(Message::Stop) => break,
                Some(message) => self.handle(now, message),
                None => {}
            }
            self.poll(now);
            self.state_members();
        }
        self.shut_down();
    }

    fn handle(&mut self, now: Instant, message: Message) {
        match message {
            Message::Scope(workspace, scope) => self.scope(now, workspace, scope),
            Message::AgentSettled(workspace) => {
                self.with_space(now, workspace, |space, now| {
                    space.workspace.agent_settled(now)
                });
            }
            Message::ConnectionLost(connection) => self.connection_lost(now, connection),
            Message::Changed(workspace, repository, change) => {
                self.with_space(now, workspace, |space, now| {
                    space.workspace.changed(now, &repository, change)
                });
            }
            Message::Opened(workspace, path) => self.probe(workspace, &path),
            Message::Probed { workspace, found } => {
                self.with_space(now, workspace, |space, now| {
                    space.workspace.file_opened(now, found)
                });
            }
            Message::Subscribe {
                workspace,
                connection,
                sink,
            } => self.subscribe(now, workspace, connection, sink),
            Message::SubscribeRelay {
                workspace,
                connection,
                lifecycle,
                mut sink,
            } => {
                let Some(space) = live(&mut self.spaces, workspace) else {
                    return;
                };
                let routed = self
                    .prompts
                    .iter()
                    .filter(|open| open.workspace == workspace && open.connection == connection)
                    .map(|open| open.prompt.clone())
                    .collect();
                sink.send(&RelayEvent::Snapshot(routed));
                space.relays.push(RelaySubscriber {
                    connection,
                    lifecycle,
                    sink,
                });
            }
            Message::Focus {
                workspace,
                connection,
                focused,
                reply,
            } => {
                if focused {
                    self.focus_count += 1;
                }
                let count = self.focus_count;
                let found = self.with_space(now, workspace, |space, now| {
                    if focused {
                        space.focused.insert(connection, count);
                    }
                    space.workspace.focus(now, connection, focused)
                });
                reply(if found {
                    Ok(())
                } else {
                    Err(GitError::new(ErrorCode::NotFound))
                });
            }
            Message::StatusPage {
                workspace,
                repository,
                revision,
                cursor,
                reply,
            } => reply(
                live(&mut self.spaces, workspace)
                    .ok_or(GitError::new(ErrorCode::NotFound))
                    .and_then(|space| space.workspace.page(&repository, revision, cursor)),
            ),
            Message::Read {
                workspace,
                repository,
                run,
            } => self.read(workspace, &repository, run),
            Message::Output {
                workspace,
                cursor,
                reply,
            } => reply(
                live(&mut self.spaces, workspace)
                    .ok_or(GitError::new(ErrorCode::NotFound))
                    .map(|space| lock(&space.output).read(cursor)),
            ),
            Message::Consent {
                workspace,
                repository,
                consented,
                reply,
            } => {
                let answered = self.consent(now, workspace, &repository, consented);
                // A repository's working tree enters the scope before its Client learns of the
                // consent and leaves it before the Client learns of the close.
                self.state_members();
                reply(answered);
            }
            Message::Mutate {
                call,
                repository,
                work,
                reply,
            } => self.mutate(now, call, &repository, work, reply),
            Message::Outcome {
                workspace,
                request,
                reply,
            } => reply(
                self.spaces
                    .get_mut(&workspace)
                    .ok_or(GitError::new(ErrorCode::NotFound))
                    .map(|space| space.requests.outcome(now, request)),
            ),
            Message::Answer {
                workspace,
                connection,
                prompt,
                answer,
                reply,
            } => reply(self.answer(workspace, connection, prompt, answer)),
            Message::Discovered {
                workspace,
                generation,
                discovered,
            } => {
                let consents = self.consents.read();
                self.with_space(now, workspace, |space, now| {
                    space.discovery = None;
                    let mut effects = consents
                        .map(|consents| {
                            space
                                .workspace
                                .set_consents(now, consents.into_iter().collect())
                        })
                        .unwrap_or_default();
                    effects.extend(space.workspace.discovered(now, generation, discovered));
                    effects
                });
            }
            Message::StatusEnded {
                workspace,
                repository,
                generation,
                result,
            } => {
                self.with_space(now, workspace, |space, now| {
                    space.statuses.remove(&(repository.clone(), generation));
                    space.dirty = true;
                    space
                        .workspace
                        .status_ended(now, &repository, generation, result)
                });
            }
            Message::JobEnded {
                workspace,
                job,
                ended,
            } => self.job_ended(now, workspace, job, ended),
            Message::Ask(ask) => self.ask(now, ask),
            Message::Stop => {}
        }
    }

    /// Runs `change` on the Workspace's state and carries out the effects it returns; false when
    /// the engine has no such open Workspace.
    fn with_space(
        &mut self,
        now: Instant,
        workspace: WorkspaceId,
        change: impl FnOnce(&mut Space, Instant) -> Vec<Effect>,
    ) -> bool {
        let Some(space) = live(&mut self.spaces, workspace) else {
            return false;
        };
        let effects = change(space, now);
        self.apply(workspace, effects);
        true
    }

    fn scope(&mut self, now: Instant, workspace: WorkspaceId, scope: WorkspaceScope) {
        let root = match scope {
            WorkspaceScope::Root(root) => Some(root),
            WorkspaceScope::NoRoot => None,
            WorkspaceScope::Closed => {
                self.close_space(now, workspace);
                return;
            }
        };
        // A Workspace opened again keeps the identities, writes and subscriptions its closing left.
        let opened = live(&mut self.spaces, workspace).is_none();
        let consents = if opened { self.consents.read() } else { None };
        self.spaces
            .entry(workspace)
            .or_insert_with(Space::new)
            .closed = false;
        self.with_space(now, workspace, |space, now| {
            let mut effects = consents
                .map(|consents| {
                    space
                        .workspace
                        .set_consents(now, consents.into_iter().collect())
                })
                .unwrap_or_default();
            if opened {
                for (subscription, subscriber) in &space.subscribers {
                    effects.extend(space.workspace.subscribe(
                        now,
                        *subscription,
                        subscriber.connection,
                    ));
                }
            }
            effects.extend(space.workspace.set_root(now, root));
            effects
        });
    }

    /// The Workspace closed: its discovery, statuses and watches end, and its subscriptions stay
    /// open showing no repository until their Clients end them or it opens again, as a closed
    /// Workspace's root subscriptions show no root. Its request identities and the writes already
    /// queued remain.
    fn close_space(&mut self, now: Instant, workspace: WorkspaceId) {
        let Some(space) = live(&mut self.spaces, workspace) else {
            return;
        };
        space.closed = true;
        if let Some(discovery) = space.discovery.take() {
            discovery.store(true, Ordering::Release);
        }
        for (_, status) in space.statuses.drain() {
            status.store(true, Ordering::Release);
        }
        let mut effects = Vec::new();
        for subscription in space.subscribers.keys().copied().collect::<Vec<_>>() {
            effects.extend(space.workspace.unsubscribe(now, subscription));
        }
        space.workspace.reset();
        space.dirty = true;
        for effect in effects {
            if let Effect::Unwatch { repository } = effect {
                self.watches.unwatch(workspace, &repository);
            }
        }
        // No Client may answer in a closed Workspace, so its prompts end as unanswered ones do.
        let (ended, open) = std::mem::take(&mut self.prompts)
            .into_iter()
            .partition::<Vec<_>, _>(|open| open.workspace == workspace);
        self.prompts = open;
        for open in ended {
            let answer = unanswered(open.prompt.kind);
            self.close_prompt(open, Some(answer));
        }
    }

    fn connection_lost(&mut self, now: Instant, connection: ConnectionId) {
        let workspaces = self.spaces.keys().copied().collect::<Vec<_>>();
        for workspace in workspaces {
            // A closed Workspace keeps its subscriptions, which end with their connection too.
            let Some(space) = self.spaces.get_mut(&workspace) else {
                continue;
            };
            space
                .subscribers
                .retain(|_, subscriber| subscriber.connection != connection);
            space.relays.retain(|relay| relay.connection != connection);
            space.focused.remove(&connection);
            let effects = space.workspace.connection_lost(now, connection);
            self.apply(workspace, effects);
        }
        self.reroute();
    }

    fn subscribe(
        &mut self,
        now: Instant,
        workspace: WorkspaceId,
        connection: ConnectionId,
        mut sink: Box<dyn Sink<ScmSummary>>,
    ) {
        self.next_id += 1;
        let subscription = self.next_id;
        let git = self.program.clone();
        self.with_space(now, workspace, |space, now| {
            let effects = space.workspace.subscribe(now, subscription, connection);
            sink.send(&ScmSummary {
                git,
                summary: space.workspace.summary(),
            });
            space
                .subscribers
                .insert(subscription, Subscriber { connection, sink });
            effects
        });
    }

    fn consent(
        &mut self,
        now: Instant,
        workspace: WorkspaceId,
        repository: &str,
        consented: bool,
    ) -> Result<(), GitError> {
        let space = live(&mut self.spaces, workspace).ok_or(GitError::new(ErrorCode::NotFound))?;
        let outside = space.workspace.repository(repository).is_some_and(|shown| {
            matches!(shown.kind, RepositoryKind::Parent | RepositoryKind::File)
        });
        let known = if consented {
            space.workspace.consentable(repository)
        } else {
            space.workspace.displays(repository)
        };
        if !known {
            return Err(GitError::new(ErrorCode::NotFound));
        }
        // A consent outlives the process and reaches every Workspace; one the record cannot keep
        // holds in this Workspace for as long as the process runs.
        let recorded =
            (consented || outside) && self.consents.answer(repository, consented).is_ok();
        let consents = recorded
            .then(|| self.consents.read())
            .flatten()
            .map(|consents| consents.into_iter().collect::<HashSet<_>>());
        let workspaces = self.spaces.keys().copied().collect::<Vec<_>>();
        for other in workspaces {
            self.with_space(now, other, |space, now| {
                if other == workspace {
                    if consented {
                        space.workspace.consent(now, repository)
                    } else {
                        space.workspace.close(now, repository)
                    }
                } else if let Some(consents) = &consents {
                    space.workspace.set_consents(now, consents.clone())
                } else {
                    Vec::new()
                }
            });
        }
        Ok(())
    }

    fn probe(&self, workspace: WorkspaceId, path: &str) {
        let Some(space) = self.spaces.get(&workspace).filter(|space| !space.closed) else {
            return;
        };
        if self.program.state != GitState::Ok || !native_paths::well_formed(path) {
            return;
        }
        let Some(folder) = Path::new(path).parent().map(Path::to_path_buf) else {
            return;
        };
        let output = Arc::clone(&space.output);
        let sender = self.sender.clone();
        let _ = thread::Builder::new()
            .name("orchbee-files-git-run".into())
            .spawn(move || {
                let limits = Limits::new(
                    "rev-parse",
                    discovery::DEADLINE,
                    Arc::new(AtomicBool::new(false)),
                );
                if let Probe::Repository(found) = discovery::probe(&folder, None, &limits, &output)
                {
                    let _ = sender.send(Message::Probed { workspace, found });
                }
            });
    }

    fn read(&mut self, workspace: WorkspaceId, repository: &str, run: ReadRun) {
        let unavailable = self.unavailable();
        let Some(space) = live(&mut self.spaces, workspace) else {
            run(Err(GitError::new(ErrorCode::NotFound)));
            return;
        };
        if let Some(error) = unavailable {
            run(Err(error));
            return;
        }
        let Some(found) = space
            .workspace
            .repository(repository)
            .filter(|_| space.workspace.displays(repository))
            .map(|shown| shown.found.clone())
        else {
            run(Err(GitError::new(ErrorCode::NotFound)));
            return;
        };
        let reading = Reading {
            found,
            version: self.version,
            output: Arc::clone(&space.output),
        };
        let _ = thread::Builder::new()
            .name("orchbee-files-git-run".into())
            .spawn(move || run(Ok(reading)));
    }

    /// Why no Git command can run, if none can.
    fn unavailable(&self) -> Option<GitError> {
        match self.program.state {
            GitState::Ok => None,
            GitState::Missing => Some(GitError::unavailable("git-missing")),
            GitState::TooOld => Some(GitError::unavailable("git-too-old")),
        }
    }

    fn mutate(&mut self, now: Instant, call: Call, target: &str, work: Work, reply: Reply<Done>) {
        let unavailable = self.unavailable();
        self.next_id += 1;
        let id = self.next_id;
        let Some(space) = live(&mut self.spaces, call.workspace) else {
            reply(Err(GitError::new(ErrorCode::NotFound)));
            return;
        };
        if let Some(error) = unavailable {
            reply(Err(error));
            return;
        }
        match space.requests.begin(now, call.request) {
            Begin::Completed(ended) => {
                reply(ended);
                return;
            }
            Begin::Running => {
                space.waiters.entry(call.request).or_default().push(reply);
                return;
            }
            Begin::Run => {}
        }
        let caller = Caller {
            request: call.request,
            cancel: Arc::clone(&call.cancel),
            reply,
        };
        let refused = match &work {
            Work::Mutation(_) => (!space.workspace.displays(target)).then_some(ErrorCode::NotFound),
            Work::Init(path) => (!space
                .workspace
                .root()
                .is_some_and(|root| native_paths::relative(root, path).is_some()))
            .then_some(ErrorCode::OutsideScope),
            Work::MarkSafe(path) => {
                (!space.workspace.lists_unsafe(path)).then_some(ErrorCode::NotFound)
            }
        };
        if let Some(code) = refused {
            finish(space, caller, Err(GitError::new(code)), now);
            return;
        }
        let mutation = match work {
            Work::Mutation(mutation) => mutation,
            folder => {
                let job = Job {
                    id,
                    found: None,
                    connection: call.connection,
                    work: Some(folder),
                    fetch: None,
                    callers: vec![caller],
                    cancel: call.cancel,
                    started: false,
                    token: None,
                };
                space.loose.insert(id, job);
                self.start(call.workspace, None, id);
                return;
            }
        };
        let Some(found) = space
            .workspace
            .repository(target)
            .map(|shown| shown.found.clone())
        else {
            finish(space, caller, Err(GitError::new(ErrorCode::NotFound)), now);
            return;
        };
        let repository = found.toplevel.clone();
        let queue = space.queues.entry(repository.clone()).or_default();
        if let Mutation::Fetch { fetch, background } = &mutation
            && let Some(job) = queue.iter_mut().find(|job| job.joins(fetch, *background))
        {
            job.callers.push(caller);
            return;
        }
        let fetch = match &mutation {
            Mutation::Fetch { fetch, background } => Some((fetch.clone(), *background)),
            _ => None,
        };
        let cancel = if fetch.is_some() {
            Arc::new(AtomicBool::new(false))
        } else {
            call.cancel
        };
        queue.push_back(Job {
            id,
            found: Some(found),
            connection: call.connection,
            work: Some(Work::Mutation(mutation)),
            fetch,
            callers: vec![caller],
            cancel,
            started: false,
            token: None,
        });
        space.workspace.write_queued(&repository);
        self.start(call.workspace, Some(&repository), id);
    }

    /// Starts job `id`: a folder's job, or the first write of `repository` unless one runs.
    fn start(&mut self, workspace: WorkspaceId, repository: Option<&str>, id: u64) {
        let prompts = self
            .spaces
            .get_mut(&workspace)
            .and_then(|space| waiting(&mut space.queues, &mut space.loose, repository, id))
            .is_some_and(
                |job| matches!(&job.work, Some(Work::Mutation(mutation)) if mutation.prompts()),
            );
        let relay = if prompts { self.relay() } else { None };
        let Some(space) = self.spaces.get_mut(&workspace) else {
            return;
        };
        let Some(job) = waiting(&mut space.queues, &mut space.loose, repository, id) else {
            return;
        };
        let Some(work) = job.work.take() else {
            return;
        };
        job.started = true;
        let id = job.id;
        let cancel = Arc::clone(&job.cancel);
        let output = Arc::clone(&space.output);
        let trash = Arc::clone(&self.trash);
        let version = self.version;
        let failed = Message::JobEnded {
            workspace,
            job: id,
            ended: Err(GitError::new(ErrorCode::Failed)),
        };
        let run: Box<dyn FnOnce() -> Ended + Send> = match work {
            Work::Init(path) => Box::new(move || {
                let limits = Limits::new("init", mutations::BATCH_DEADLINE, cancel);
                let context = Context {
                    limits: &limits,
                    output: &output,
                    relay: None,
                };
                mutations::init(&path, version, &context).map(Done::Initialized)
            }),
            Work::MarkSafe(path) => Box::new(move || {
                let limits = Limits::new("mark-safe", mutations::CONFIG_DEADLINE, cancel);
                let context = Context {
                    limits: &limits,
                    output: &output,
                    relay: None,
                };
                mutations::mark_safe(&path, &context).map(|()| Done::Empty)
            }),
            Work::Mutation(mutation) => {
                let Some(found) = job.found.clone() else {
                    // A write's job always names its repository.
                    let _ = self.sender.send(failed);
                    return;
                };
                // A discard reads which of its paths the last status lists as submodules.
                let snapshot = matches!(mutation, Mutation::Discard(_))
                    .then(|| {
                        space
                            .workspace
                            .repository(&found.toplevel)
                            .and_then(|shown| shown.snapshot.clone())
                    })
                    .flatten();
                space
                    .workspace
                    .write_running(&found.toplevel, mutation.busy());
                space.dirty = true;
                let prompts = Arc::new(AtomicUsize::new(0));
                let relayed = match relay {
                    Some((endpoint, executable)) => relay::random::<32>().ok().map(|token| {
                        let background = job
                            .fetch
                            .as_ref()
                            .is_some_and(|(_, background)| *background);
                        self.tokens.insert(
                            token,
                            Operation {
                                workspace,
                                repository: found.toplevel.clone(),
                                connection: job.connection,
                                background,
                                prompts: Arc::clone(&prompts),
                                cancel: Arc::clone(&cancel),
                            },
                        );
                        job.token = Some(token);
                        RelayVariables {
                            executable,
                            endpoint,
                            token: relay::hex(&token),
                        }
                    }),
                    None => None,
                };
                let limits = mutation.limits(cancel, relayed.as_ref().map(|_| prompts));
                Box::new(move || {
                    let context = Context {
                        limits: &limits,
                        output: &output,
                        relay: relayed.as_ref(),
                    };
                    mutation.execute(&found, snapshot.as_deref(), version, &*trash, &context)
                })
            }
        };
        spawn(&self.sender, failed, move || Message::JobEnded {
            workspace,
            job: id,
            ended: run(),
        });
    }

    /// The relay's endpoint and the helper executable, binding the endpoint unless it is bound;
    /// `None` when Git's prompts cannot reach this process, so they fail as in a terminal without
    /// one.
    fn relay(&mut self) -> Option<(OsString, OsString)> {
        if self.relay.is_none() && self.executable.is_some() {
            let asks = self.sender.clone();
            self.relay = Relay::bind(move |ask| {
                let _ = asks.send(Message::Ask(ask));
            })
            .ok();
        }
        let endpoint = self
            .relay
            .as_ref()
            .map(|relay| relay.endpoint().as_os_str().to_owned());
        match (endpoint, self.executable.clone()) {
            (Some(endpoint), Some(executable)) => Some((endpoint, executable)),
            _ => {
                if !std::mem::replace(&mut self.relay_failed, true) {
                    orchbee_local_runtime::log_event(
                        orchbee_local_runtime::LogLevel::Warning,
                        "git-relay-unavailable",
                        &[],
                    );
                }
                None
            }
        }
    }

    fn job_ended(&mut self, now: Instant, workspace: WorkspaceId, id: u64, ended: Ended) {
        let Some(space) = self.spaces.get_mut(&workspace) else {
            return;
        };
        let (job, repository) = if let Some(job) = space.loose.remove(&id) {
            (job, None)
        } else {
            let Some(repository) = space
                .queues
                .iter()
                .find(|(_, queue)| queue.front().is_some_and(|job| job.id == id))
                .map(|(repository, _)| repository.clone())
            else {
                return;
            };
            let Some(job) = space
                .queues
                .get_mut(&repository)
                .and_then(VecDeque::pop_front)
            else {
                return;
            };
            (job, Some(repository))
        };
        // A new repository, or one now trusted, appears at the next discovery.
        if repository.is_none() && ended.is_ok() {
            space.workspace.rediscover(now);
        }
        for caller in job.callers {
            finish(space, caller, ended.clone(), now);
        }
        let mut effects = Vec::new();
        if let Some(repository) = &repository {
            effects = space.workspace.write_ended(now, repository);
            if space.queues.get(repository).is_some_and(VecDeque::is_empty) {
                space.queues.remove(repository);
            }
        }
        if let Some(token) = job.token {
            self.revoke(&token);
        }
        self.apply(workspace, effects);
        if let Some(repository) = repository {
            let next = self
                .spaces
                .get(&workspace)
                .and_then(|space| space.queues.get(&repository))
                .and_then(VecDeque::front)
                .map(|job| job.id);
            if let Some(next) = next {
                self.start(workspace, Some(&repository), next);
            }
        }
    }

    /// Carries out the Workspace's effects.
    fn apply(&mut self, workspace: WorkspaceId, effects: Vec<Effect>) {
        for effect in effects {
            let Some(space) = self.spaces.get_mut(&workspace) else {
                return;
            };
            match effect {
                Effect::Discover { generation, root } => {
                    let cancel = Arc::new(AtomicBool::new(false));
                    if let Some(previous) = space.discovery.replace(Arc::clone(&cancel)) {
                        previous.store(true, Ordering::Release);
                    }
                    let empty = Message::Discovered {
                        workspace,
                        generation,
                        discovered: Discovered::default(),
                    };
                    if self.program.state != GitState::Ok {
                        let _ = self.sender.send(empty);
                        continue;
                    }
                    let output = Arc::clone(&space.output);
                    spawn(&self.sender, empty, move || Message::Discovered {
                        workspace,
                        generation,
                        discovered: discovery::discover(&root, &cancel, &output),
                    });
                }
                Effect::Status {
                    repository,
                    generation,
                } => {
                    space.dirty = true;
                    let Some(found) = space
                        .workspace
                        .repository(&repository)
                        .map(|shown| shown.found.clone())
                    else {
                        continue;
                    };
                    let cancel = Arc::new(AtomicBool::new(false));
                    space
                        .statuses
                        .insert((repository.clone(), generation), Arc::clone(&cancel));
                    let output = Arc::clone(&space.output);
                    let failed = Message::StatusEnded {
                        workspace,
                        repository: repository.clone(),
                        generation,
                        result: Err(GitError::new(ErrorCode::Failed)),
                    };
                    spawn(&self.sender, failed, move || {
                        let limits = Limits::new("status", status::DEADLINE, cancel);
                        Message::StatusEnded {
                            workspace,
                            repository,
                            generation,
                            result: status::status(&found, status::ENTRY_LIMIT, &limits, &output),
                        }
                    });
                }
                Effect::CancelStatus {
                    repository,
                    generation,
                } => {
                    if let Some(cancel) = space.statuses.remove(&(repository, generation)) {
                        cancel.store(true, Ordering::Release);
                    }
                }
                Effect::Watch { repository } => {
                    if let Some(shown) = space.workspace.repository(&repository) {
                        let sender = self.sender.clone();
                        let watched = repository.clone();
                        let report = Report::new(move |change| {
                            let _ =
                                sender.send(Message::Changed(workspace, watched.clone(), change));
                        });
                        self.watches
                            .watch(workspace, &repository, &shown.found.git_dir, report);
                    }
                }
                Effect::Unwatch { repository } => self.watches.unwatch(workspace, &repository),
                Effect::Publish => space.dirty = true,
            }
        }
    }

    /// Ends what its Clients left: subscriptions whose calls ended, queued callers that cancelled,
    /// fetches every caller cancelled, prompts past their time. Then fires the timers that are due
    /// and sends each changed summary.
    fn poll(&mut self, now: Instant) {
        let workspaces = self.spaces.keys().copied().collect::<Vec<_>>();
        let mut relays_ended = false;
        for workspace in workspaces {
            let Some(space) = self.spaces.get_mut(&workspace) else {
                continue;
            };
            let mut effects = Vec::new();
            let ended = space
                .subscribers
                .iter()
                .filter(|(_, subscriber)| subscriber.sink.ended())
                .map(|(subscription, _)| *subscription)
                .collect::<Vec<_>>();
            for subscription in ended {
                space.subscribers.remove(&subscription);
                effects.extend(space.workspace.unsubscribe(now, subscription));
            }
            let relays = space.relays.len();
            space.relays.retain(|relay| !relay.sink.ended());
            relays_ended |= space.relays.len() != relays;
            let mut abandoned = Vec::new();
            for (repository, queue) in &mut space.queues {
                for job in queue.iter_mut() {
                    let (cancelled, waiting) = job
                        .callers
                        .drain(..)
                        .partition::<Vec<_>, _>(|caller| caller.cancel.load(Ordering::Acquire));
                    if job.started {
                        // A started job answers every caller when it ends.
                        job.callers = cancelled.into_iter().chain(waiting).collect();
                        if job
                            .callers
                            .iter()
                            .all(|caller| caller.cancel.load(Ordering::Acquire))
                        {
                            job.cancel.store(true, Ordering::Release);
                        }
                    } else {
                        job.callers = waiting;
                        abandoned.extend(cancelled);
                    }
                }
                let queued = queue.len();
                queue.retain(|job| job.started || !job.callers.is_empty());
                for _ in queue.len()..queued {
                    effects.extend(space.workspace.write_ended(now, repository));
                }
            }
            space.queues.retain(|_, queue| !queue.is_empty());
            for caller in abandoned {
                finish(space, caller, Err(GitError::new(ErrorCode::Cancelled)), now);
            }
            // A closed Workspace stays only while a write runs, an outcome may still be asked or a
            // subscription is open.
            if space.closed
                && space.queues.is_empty()
                && space.loose.is_empty()
                && space.subscribers.is_empty()
                && space.relays.is_empty()
                && space.requests.idle(now)
            {
                self.spaces.remove(&workspace);
                continue;
            }
            effects.extend(space.workspace.wake(now));
            self.apply(workspace, effects);
        }
        if relays_ended {
            self.reroute();
        }
        let (expired, open) = std::mem::take(&mut self.prompts)
            .into_iter()
            .partition::<Vec<_>, _>(|open| open.expires <= now);
        self.prompts = open;
        for open in expired {
            let answer = unanswered(open.prompt.kind);
            self.close_prompt(open, Some(answer));
        }
        self.publish();
    }

    fn publish(&mut self) {
        for space in self.spaces.values_mut() {
            if !std::mem::take(&mut space.dirty) || space.subscribers.is_empty() {
                continue;
            }
            let summary = ScmSummary {
                git: self.program.clone(),
                summary: space.workspace.summary(),
            };
            for subscriber in space.subscribers.values_mut() {
                subscriber.sink.send(&summary);
            }
        }
    }

    fn next_wake(&self) -> Option<Instant> {
        self.spaces
            .values()
            .filter_map(|space| space.workspace.next_wake())
            .chain(self.prompts.iter().map(|open| open.expires))
            .min()
    }

    /// Whether something waits on a Client's cancel or a subscription's end, which the engine
    /// watches every 250 ms.
    fn polling(&self) -> bool {
        !self.prompts.is_empty()
            || self.spaces.values().any(|space| {
                !space.subscribers.is_empty()
                    || !space.relays.is_empty()
                    || !space.queues.is_empty()
            })
    }

    /// A helper's prompt: answered at once for a background operation or when no Client can take
    /// it, else routed.
    fn ask(&mut self, now: Instant, ask: Ask) {
        let Some(operation) = self.tokens.get(&ask.token) else {
            let _ = ask.answer.send(Answer::Cancelled);
            return;
        };
        if ask
            .path
            .as_ref()
            .is_some_and(|path| path.len() > PATH_LIMIT)
        {
            let _ = ask.answer.send(Answer::Cancelled);
            return;
        }
        let workspace = operation.workspace;
        let route = (!operation.background)
            .then(|| {
                self.spaces
                    .get(&workspace)
                    .and_then(|space| space.route(operation.connection))
            })
            .flatten()
            .filter(|connection| self.routed(workspace, *connection) < PROMPTS_PER_CONNECTION);
        let id = relay::random::<16>().ok().and_then(PromptId::from_bytes);
        let (Some(connection), Some(id)) = (route, id) else {
            let _ = ask.answer.send(Answer::Empty);
            return;
        };
        operation.prompts.fetch_add(1, Ordering::AcqRel);
        let prompt = Prompt {
            id,
            kind: ask.kind,
            text: ask.text,
            repository: operation.repository.clone(),
            path: ask.path,
        };
        let expires = now
            + if prompt.kind == PromptKind::Editor {
                EDITOR_PROMPT
            } else {
                CREDENTIAL_PROMPT
            };
        self.prompts.push(Open {
            token: ask.token,
            workspace,
            connection,
            prompt: prompt.clone(),
            expires,
            answer: ask.answer,
        });
        // The file an editor prompt names is in scope before its Client learns of the prompt.
        self.state_members();
        if let Some(space) = self.spaces.get_mut(&workspace) {
            space.relay_send(connection, &RelayEvent::Opened(prompt));
        }
    }

    /// How many open prompts are routed to `connection` in `workspace`.
    fn routed(&self, workspace: WorkspaceId, connection: ConnectionId) -> usize {
        self.prompts
            .iter()
            .filter(|open| open.workspace == workspace && open.connection == connection)
            .count()
    }

    fn answer(
        &mut self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        prompt: PromptId,
        answer: Answer,
    ) -> Result<(), GitError> {
        let index = self
            .prompts
            .iter()
            .position(|open| {
                open.workspace == workspace
                    && open.connection == connection
                    && open.prompt.id == prompt
            })
            .ok_or(GitError::new(ErrorCode::NotFound))?;
        let open = self.prompts.remove(index);
        if answer == Answer::Cancelled
            && let Some(operation) = self.tokens.get(&open.token)
        {
            operation.cancel.store(true, Ordering::Release);
        }
        self.close_prompt(open, Some(answer));
        Ok(())
    }

    /// Ends an open prompt: its helper gets `answer`, or fails without one, and the Client it was
    /// routed to learns it closed.
    fn close_prompt(&mut self, open: Open, answer: Option<Answer>) {
        if let Some(operation) = self.tokens.get(&open.token) {
            operation.prompts.fetch_sub(1, Ordering::AcqRel);
        }
        if let Some(answer) = answer {
            let _ = open.answer.send(answer);
        }
        if let Some(space) = self.spaces.get_mut(&open.workspace) {
            space.relay_send(open.connection, &RelayEvent::Closed(open.prompt.id));
        }
    }

    /// Moves each prompt whose Client stopped subscribing to the next Client that can take it;
    /// with none, its helper gets an empty answer.
    fn reroute(&mut self) {
        let mut kept = Vec::new();
        for mut open in std::mem::take(&mut self.prompts) {
            let Some(space) = self.spaces.get_mut(&open.workspace) else {
                let _ = open.answer.send(Answer::Empty);
                continue;
            };
            let subscribed = space
                .relays
                .iter()
                .any(|relay| relay.connection == open.connection && relay.lifecycle);
            if subscribed {
                kept.push(open);
                continue;
            }
            let initiator = self
                .tokens
                .get(&open.token)
                .map_or(open.connection, |operation| operation.connection);
            let held = |connection: ConnectionId| {
                kept.iter()
                    .filter(|other: &&Open| {
                        other.workspace == open.workspace && other.connection == connection
                    })
                    .count()
            };
            match space
                .route(initiator)
                .filter(|connection| held(*connection) < PROMPTS_PER_CONNECTION)
            {
                Some(connection) => {
                    open.connection = connection;
                    space.relay_send(connection, &RelayEvent::Opened(open.prompt.clone()));
                    kept.push(open);
                }
                None => {
                    if let Some(operation) = self.tokens.get(&open.token) {
                        operation.prompts.fetch_sub(1, Ordering::AcqRel);
                    }
                    let _ = open.answer.send(Answer::Empty);
                }
            }
        }
        self.prompts = kept;
    }

    /// The operation of `token` ended: its prompts close without an answer, and the relay closes
    /// once no operation may prompt.
    fn revoke(&mut self, token: &Token) {
        self.tokens.remove(token);
        if self.tokens.is_empty() {
            self.relay = None;
        }
        let (ended, open) = std::mem::take(&mut self.prompts)
            .into_iter()
            .partition::<Vec<_>, _>(|open| open.token == *token);
        self.prompts = open;
        for open in ended {
            self.close_prompt(open, None);
        }
    }

    /// States each Workspace's members that changed since they were last stated: the working trees
    /// of its displayed consented repositories and the files its open editor prompts name. An
    /// editor's file is taken as Git names it, without checking that it lies in the Git directory:
    /// only the commands of the prompting operation hold the token that opens a prompt, and each of
    /// them already runs as the user.
    fn state_members(&mut self) {
        let mut current: HashMap<WorkspaceId, Vec<String>> = HashMap::new();
        for (workspace, space) in &self.spaces {
            let consented = space.workspace.consented();
            if !consented.is_empty() {
                current.insert(*workspace, consented);
            }
        }
        for open in &self.prompts {
            if let (PromptKind::Editor, Some(path)) = (open.prompt.kind, &open.prompt.path) {
                current
                    .entry(open.workspace)
                    .or_default()
                    .push(path.clone());
            }
        }
        for (workspace, members) in &current {
            if self.stated_members.get(workspace) != Some(members) {
                self.members.state(*workspace, members);
            }
        }
        for workspace in self.stated_members.keys() {
            if !current.contains_key(workspace) {
                self.members.state(*workspace, &[]);
            }
        }
        self.stated_members = current;
    }

    /// Stops every run the engine started; their replies end with their calls.
    fn shut_down(&mut self) {
        for space in self.spaces.values_mut() {
            if let Some(discovery) = space.discovery.take() {
                discovery.store(true, Ordering::Release);
            }
            for (_, status) in space.statuses.drain() {
                status.store(true, Ordering::Release);
            }
            for job in space.queues.values().flatten().chain(space.loose.values()) {
                job.cancel.store(true, Ordering::Release);
            }
        }
        for operation in self.tokens.values() {
            operation.cancel.store(true, Ordering::Release);
        }
        self.prompts.clear();
        self.relay = None;
    }
}

impl Mutation {
    /// Runs the mutation. A discard updates the paths `snapshot` lists as submodules instead of
    /// checking them out.
    fn execute(
        self,
        found: &Found,
        snapshot: Option<&Snapshot>,
        version: (u32, u32, u32),
        trash: &dyn Trash,
        context: &Context<'_>,
    ) -> Ended {
        match self {
            Self::Stage(paths) => mutations::stage(found, &paths, context).map(|()| Done::Empty),
            Self::Unstage(paths) => {
                mutations::unstage(found, &paths, context).map(|()| Done::Empty)
            }
            Self::Discard(paths) => {
                let (submodules, paths) = paths
                    .into_iter()
                    .partition::<Vec<_>, _>(|path| submodule(found, snapshot, path));
                mutations::discard(found, &paths, &submodules, context).map(|()| Done::Empty)
            }
            Self::StageContent { path, content } => {
                mutations::stage_content(found, &path, content, context).map(|()| Done::Empty)
            }
            Self::Clean { paths, mode } => Ok(Done::Cleaned(mutations::clean(
                found, &paths, mode, trash, context,
            ))),
            Self::Commit(commit) => mutations::commit(found, &commit, context).map(Done::Committed),
            Self::Fetch { fetch, .. } => {
                remote::fetch(found, &fetch, context).map(|()| Done::Empty)
            }
            Self::Pull { rebase } => {
                remote::pull(found, rebase, version, context).map(|()| Done::Empty)
            }
            Self::Sync { rebase } => {
                remote::sync(found, rebase, version, context).map(|()| Done::Empty)
            }
            Self::Push(push) => remote::push(found, &push, version, context).map(|()| Done::Empty),
        }
    }
}

/// Whether `snapshot` lists `path` as a submodule.
fn submodule(found: &Found, snapshot: Option<&Snapshot>, path: &str) -> bool {
    let Some(relative) = native_paths::relative(&found.toplevel, path) else {
        return false;
    };
    let relative = native_paths::git_relative(relative);
    snapshot.is_some_and(|snapshot| {
        snapshot
            .entries
            .iter()
            .any(|entry| entry.submodule && entry.path == relative.as_bytes())
    })
}

/// Job `id` of `queues` and `loose` when it waits to start: a folder's job, or the first write of
/// `repository`.
fn waiting<'a>(
    queues: &'a mut HashMap<String, VecDeque<Job>>,
    loose: &'a mut HashMap<u64, Job>,
    repository: Option<&str>,
    id: u64,
) -> Option<&'a mut Job> {
    let job = match repository {
        Some(repository) => queues.get_mut(repository)?.front_mut()?,
        None => loose.get_mut(&id)?,
    };
    (!job.started).then_some(job)
}

/// The open Workspace `workspace`.
fn live(spaces: &mut HashMap<WorkspaceId, Space>, workspace: WorkspaceId) -> Option<&mut Space> {
    spaces.get_mut(&workspace).filter(|space| !space.closed)
}

/// What a prompt nobody answered gets: an editor fails, so Git abandons the message, and a
/// credential prompt answers empty, so Git reports the failed authentication.
fn unanswered(kind: PromptKind) -> Answer {
    if kind == PromptKind::Editor {
        Answer::Cancelled
    } else {
        Answer::Empty
    }
}

/// Records how `caller`'s request ended and answers it, with every repeat that waited on it.
fn finish(space: &mut Space, caller: Caller, ended: Ended, now: Instant) {
    space.requests.complete(now, caller.request, ended.clone());
    for waiter in space.waiters.remove(&caller.request).unwrap_or_default() {
        waiter(ended.clone());
    }
    (caller.reply)(ended);
}

/// Runs `work` on a thread of its own and sends the message it ends with; when no thread starts,
/// `failed` is sent instead.
fn spawn(
    sender: &Sender<Message>,
    failed: Message,
    work: impl FnOnce() -> Message + Send + 'static,
) {
    let reporter = sender.clone();
    let spawned = thread::Builder::new()
        .name("orchbee-files-git-run".into())
        .spawn(move || {
            let _ = reporter.send(work());
        });
    if spawned.is_err() {
        let _ = sender.send(failed);
    }
}

fn lock(output: &Mutex<GitOutput>) -> std::sync::MutexGuard<'_, GitOutput> {
    output
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The machine's Git, as `git --version` reports it.
fn program() -> (GitProgram, (u32, u32, u32)) {
    let mut command = command::git(&std::env::temp_dir(), Access::Read, None);
    command.arg("--version");
    let limits = Limits::new(
        "version",
        VERSION_DEADLINE,
        Arc::new(AtomicBool::new(false)),
    );
    let ran = runner::run(command, Vec::new(), &limits, &Mutex::new(GitOutput::new()));
    let missing = GitProgram {
        state: GitState::Missing,
        version: None,
    };
    let Ok(ran) = ran else {
        return (missing, (0, 0, 0));
    };
    // A Git whose version cannot be read cannot be shown to be new enough.
    let Some(version) = command::version(&ran.stdout) else {
        let program = GitProgram {
            state: GitState::TooOld,
            version: None,
        };
        return (program, (0, 0, 0));
    };
    let text = String::from_utf8_lossy(&ran.stdout);
    let text = text
        .trim()
        .trim_start_matches("git version ")
        .chars()
        .scan(0, |length, character| {
            *length += character.len_utf8();
            (*length <= VERSION_TEXT_LIMIT).then_some(character)
        })
        .collect::<String>();
    let state = if version >= OLDEST_VERSION {
        GitState::Ok
    } else {
        GitState::TooOld
    };
    (
        GitProgram {
            state,
            version: Text::new(text).ok(),
        },
        version,
    )
}
