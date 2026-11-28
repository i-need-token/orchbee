//! The Server's side of the File Bar: it has the Worker start `orchbee-files` on a Client's first
//! File Bar call, admits the process's one link, relays every connection's File Bar streams over
//! it, states each Workspace's scope from the Session's Navigation, passes on each Agent
//! completion, and drains the process after ten idle minutes or when the Session stops. A build
//! for the release environment never starts the process.

mod completion;
mod launch;
mod lifecycle;
mod link;
mod mailbox;
mod relay;
mod scope;

pub(crate) use mailbox::Outbound;
pub(crate) use relay::Relay;

use crate::worker_obcn_carrier::WorkerRequestHandle;
use lifecycle::{Demand, Lifecycle, Refusal, StartFailure, Step};
use link::{Event, Link, Route};
use mailbox::Mailbox;
use orchbee_connection_handshake::CapabilitySet;
use orchbee_files_protocol::{
    CallId, ConnectionId, DrainReason, ErrorCode, FilesError, FilesRecord, FilesReply, GrantClass,
    LaneId, LinkMessage, LinkRequest, Token, WorkspaceId, WorkspaceScope, encode_files_record,
    encode_reply,
};
use orchbee_local_runtime::{LogLevel, LogValue};
use orchbee_server::{SessionChangeSignal, SessionEventJournal};
use orchbee_workbench_protocol::{MultiplexStreamId, StreamCloseReason, StreamFrameKind};
use std::{
    collections::{HashSet, VecDeque},
    io,
    num::NonZeroU64,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// How long a Session stop waits for a drained process to end its link.
const STOP_GRACE: Duration = Duration::from_secs(5);

/// The scopes of these Workspaces, in order; `None` for one the Session does not have.
pub(crate) type Scopes = dyn Fn(&[WorkspaceId]) -> Vec<Option<WorkspaceScope>> + Send + Sync;

/// What the File Bar reads from the rest of the Server.
pub(crate) struct Host {
    /// The Worker that serves now, while it is active.
    pub(crate) worker: Box<dyn Fn() -> Option<WorkerRequestHandle> + Send + Sync>,
    pub(crate) scopes: Box<Scopes>,
}

/// One call a Client connection opened.
struct Call {
    id: CallId,
    connection: ConnectionId,
    lane: LaneId,
    workspace: Option<WorkspaceId>,
    grant: GrantClass,
    record: Vec<u8>,
    events: bool,
    mutation: bool,
    stream: MultiplexStreamId,
    mailbox: Arc<Mailbox>,
}

impl Call {
    /// Ends the call's stream with `error`, as `orchbee-files` would answer it.
    fn answer(&self, error: FilesError) {
        answer(&self.mailbox, self.stream, error);
    }

    fn refuse(&self, refusal: Refusal) {
        let reason = match refusal {
            Refusal::Withheld => {
                return self
                    .mailbox
                    .close(self.stream, StreamCloseReason::Unsupported, false);
            }
            Refusal::Stopping => {
                return self
                    .mailbox
                    .close(self.stream, StreamCloseReason::Stopping, false);
            }
            Refusal::Restarting => "restarting",
            Refusal::CrashedRepeatedly => "crashed-repeatedly",
            Refusal::BuildMismatch => "build-mismatch",
        };
        self.answer(unavailable(reason));
    }
}

struct State {
    lifecycle: Lifecycle,
    /// Calls that wait for a start.
    pending: VecDeque<Call>,
    /// Calls a start took from `pending` that are not on the link yet.
    flushing: HashSet<CallId>,
    /// Calls of `flushing` whose Client closed their stream meanwhile.
    cancelled: HashSet<CallId>,
    link: Option<Arc<Link>>,
    /// The Worker generation of the last start.
    generation: Option<NonZeroU64>,
    stopping: bool,
}

/// The File Bar of this Server's Session.
pub(crate) struct FileBar {
    runtime_directory: PathBuf,
    host: Host,
    state: Mutex<State>,
    signal: Arc<SessionChangeSignal>,
    calls: AtomicU64,
    links: AtomicU64,
    supervisor: Mutex<Option<JoinHandle<()>>>,
    /// The record a call the link loses before it answers ends with.
    restarting: Arc<[u8]>,
    /// The Session's journal, whose Agent completions the process hears of.
    journal: Option<SessionEventJournal>,
}

impl FileBar {
    /// Starts the File Bar of the Session whose runtime directory is `runtime_directory` and
    /// whose Workbench commits `journal` publishes.
    pub(crate) fn start(
        runtime_directory: PathBuf,
        host: Host,
        journal: Option<&SessionEventJournal>,
    ) -> io::Result<Arc<Self>> {
        let restarting = encode_files_record(&FilesRecord::Result(encode_reply(
            &FilesReply::Error(unavailable("restarting")),
        )))
        .map_err(io::Error::other)?;
        let bar = Arc::new(Self {
            runtime_directory,
            host,
            state: Mutex::new(State {
                lifecycle: Lifecycle::new(orchbee_local_runtime::is_release()),
                pending: VecDeque::new(),
                flushing: HashSet::new(),
                cancelled: HashSet::new(),
                link: None,
                generation: None,
                stopping: false,
            }),
            signal: Arc::default(),
            calls: AtomicU64::new(0),
            links: AtomicU64::new(0),
            supervisor: Mutex::new(None),
            restarting: restarting.into(),
            journal: journal.cloned(),
        });
        if let Some(journal) = journal {
            journal.watch(&bar.signal);
        }
        let supervising = Arc::clone(&bar);
        let handle = thread::Builder::new()
            .name("orchbee-file-bar".into())
            .spawn(move || supervising.supervise())?;
        *bar.lock_supervisor() = Some(handle);
        Ok(bar)
    }

    /// The File Bar streams of the Client connection `connection`, which holds `grant` and whose
    /// handshake answer lists `capabilities`.
    pub(crate) fn relay(
        self: &Arc<Self>,
        connection: ConnectionId,
        grant: Option<GrantClass>,
        capabilities: CapabilitySet,
    ) -> Relay {
        Relay::new(Arc::clone(self), connection, grant, capabilities)
    }

    /// Stops for the Session's end: calls are refused from now on, and a running process is
    /// drained and gets a bounded time to end its link.
    pub(crate) fn stop_before(&self, deadline: Instant) {
        let (link, pending) = {
            let mut state = self.lock();
            state.stopping = true;
            let draining = state.lifecycle.stop();
            let link = state
                .link
                .clone()
                .filter(|link| draining == Some(link.id()));
            (link, std::mem::take(&mut state.pending))
        };
        for call in pending {
            call.refuse(Refusal::Stopping);
        }
        self.signal.wake();
        let grace = deadline.min(Instant::now() + STOP_GRACE);
        if let Some(link) = link {
            link.send(&LinkMessage::Drain(DrainReason::Session));
            loop {
                let observed = self.signal.wake_revision();
                let now = Instant::now();
                if self.lock().link.is_none() || now >= grace {
                    break;
                }
                self.signal.wait(observed, Some(grace - now));
            }
            link.close();
        }
        let handle = self.lock_supervisor().take();
        if let Some(handle) = handle {
            // A start in progress ends by its own deadline and drains what it started.
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }

    fn next_call(&self) -> CallId {
        let next = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
        CallId::new(next).expect("the counter starts above zero")
    }

    /// Routes `call` now, keeps it for the start a demand needs, or refuses it.
    fn submit(&self, call: Call) {
        let now = Instant::now();
        let mut state = self.lock();
        let retired = self.retire_replaced(&mut state);
        let demand = state.lifecycle.demand(now);
        match (demand, state.link.clone()) {
            (Demand::Route, Some(link)) => {
                drop(state);
                self.route(&link, call);
            }
            (Demand::Route, None) | (Demand::Queue, _) => {
                state.pending.push_back(call);
                drop(state);
                self.signal.wake();
            }
            (Demand::Refuse(refusal), _) => {
                drop(state);
                call.refuse(refusal);
            }
        }
        if let Some(link) = retired {
            link.close();
        }
    }

    /// Ends `call`, whose Client closed its stream.
    fn cancel(&self, call: CallId) {
        let mut state = self.lock();
        if let Some(position) = state.pending.iter().position(|pending| pending.id == call) {
            state.pending.remove(position);
        } else if state.flushing.contains(&call) {
            state.cancelled.insert(call);
        } else if let Some(link) = state.link.clone()
            && link.unroute(call)
        {
            link.send(&LinkMessage::Cancel { call });
            state.lifecycle.call_ended(Instant::now(), link.id());
        }
    }

    /// Ends what the Client connection `connection` held: its waiting calls, its open `calls`,
    /// and in the process its grants, transfers and subscriptions.
    fn connection_lost(&self, connection: ConnectionId, calls: &[CallId]) {
        let mut state = self.lock();
        state.pending.retain(|call| call.connection != connection);
        let link = state.link.clone();
        for call in calls {
            if state.flushing.contains(call) {
                state.cancelled.insert(*call);
            } else if let Some(link) = &link
                && link.unroute(*call)
            {
                state.lifecycle.call_ended(Instant::now(), link.id());
            }
        }
        if let Some(link) = link {
            link.send(&LinkMessage::ConnectionLost { connection });
        }
    }

    /// The connection `connection` closed `lane`, ending every transfer on it.
    fn lane_closed(&self, connection: ConnectionId, lane: LaneId) {
        let link = self.lock().link.clone();
        if let Some(link) = link {
            link.send(&LinkMessage::LaneClosed { connection, lane });
        }
    }

    /// Sends `call` over `link` after the scope update its Workspace needs; a Workspace that is
    /// not the Session's and that the link never heard of is denied.
    fn route(&self, link: &Arc<Link>, call: Call) {
        let known = call.workspace.is_none_or(|workspace| {
            let mut scopes = link.scopes();
            let current = (self.host.scopes)(&[workspace]).pop().flatten();
            match scopes.before_call(workspace, current) {
                Ok(update) => {
                    if let Some(update) = update {
                        link.send(&LinkMessage::Scope(update));
                    }
                    true
                }
                Err(scope::Unknown) => false,
            }
        });
        // The state lock orders the request before a cancel or connection loss of its Client.
        let mut state = self.lock();
        state.flushing.remove(&call.id);
        let cancelled = state.cancelled.remove(&call.id);
        if cancelled || !known {
            state.lifecycle.call_ended(Instant::now(), link.id());
            drop(state);
            if !cancelled {
                call.answer(FilesError::new(ErrorCode::Denied));
            }
            return;
        }
        let Call {
            id,
            connection,
            lane,
            workspace,
            grant,
            record,
            events,
            mutation,
            stream,
            mailbox,
        } = call;
        let route = Route::new(
            Arc::clone(&mailbox),
            stream,
            events,
            mutation,
            Arc::clone(&self.restarting),
        );
        let request = LinkMessage::Request(LinkRequest {
            call: id,
            connection,
            lane,
            workspace,
            grant,
            record,
        });
        // Whoever removes the route of a link that ended closes the call's stream.
        let lost = if link.route(id, route) {
            !link.send(&request) && link.unroute(id)
        } else {
            true
        };
        if lost {
            state.lifecycle.call_ended(Instant::now(), link.id());
            drop(state);
            answer(&mailbox, stream, unavailable("restarting"));
        }
    }

    /// Takes the link of a replaced Worker's process out of service, so the successor Worker
    /// starts a fresh one.
    fn retire_replaced(&self, state: &mut State) -> Option<Arc<Link>> {
        let current = (self.host.worker)().map(|worker| worker.generation());
        if current.is_none() || state.generation.is_none() || current == state.generation {
            return None;
        }
        state.generation = current;
        let retired = state.lifecycle.retire();
        state.link.take().filter(|link| retired == Some(link.id()))
    }

    fn supervise(self: Arc<Self>) {
        let mut commits = self.signal.commit_revision();
        let mut completions = 0;
        loop {
            let observed = self.signal.wake_revision();
            let now = Instant::now();
            let (step, link) = {
                let mut state = self.lock();
                if state.stopping {
                    return;
                }
                let wanted = !state.pending.is_empty();
                (state.lifecycle.step(now, wanted), state.link.clone())
            };
            let wait = match step {
                Step::Start => {
                    self.start_process();
                    continue;
                }
                Step::Drain(id) => {
                    if let Some(link) = link.as_ref().filter(|link| link.id() == id) {
                        link.send(&LinkMessage::Drain(DrainReason::Idle));
                    }
                    continue;
                }
                Step::Close(id) => {
                    if let Some(link) = link.as_ref().filter(|link| link.id() == id) {
                        link.close();
                    }
                    None
                }
                Step::Wait(at) => at.map(|at| at.saturating_duration_since(now)),
            };
            let current = self.signal.commit_revision();
            if current != commits {
                commits = current;
                if let Some(link) = &link {
                    self.restate(link);
                }
            }
            if let Some(journal) = &self.journal {
                completions = completion::settle(journal, link.as_deref(), completions);
            }
            self.signal.wait(observed, wait);
        }
    }

    /// Brings every open Workspace the link heard of to its current scope.
    fn restate(&self, link: &Link) {
        let mut scopes = link.scopes();
        let workspaces = scopes.workspaces();
        if workspaces.is_empty() {
            return;
        }
        let current = (self.host.scopes)(&workspaces);
        for update in scopes.changes(&workspaces, current) {
            link.send(&LinkMessage::Scope(update));
        }
    }

    fn start_process(self: &Arc<Self>) {
        let worker = (self.host.worker)();
        let launched = match &worker {
            None => Err(StartFailure::NoWorker),
            Some(worker) => {
                self.lock().generation = Some(worker.generation());
                launch::launch(&self.runtime_directory, worker).map_err(|error| {
                    let (stage, failure) = match error {
                        launch::LaunchError::BuildMismatch => {
                            ("build-mismatch", StartFailure::BuildMismatch)
                        }
                        launch::LaunchError::Failed(stage) => (stage, StartFailure::Crashed),
                    };
                    log(LogLevel::Warning, "file-bar-start-failed", "stage", stage);
                    failure
                })
            }
        };
        let id = self.links.fetch_add(1, Ordering::Relaxed) + 1;
        let link = launched.and_then(|launched| {
            let weak = Arc::downgrade(self);
            let events: Arc<dyn Fn(u64, Event) + Send + Sync> = Arc::new(move |link, event| {
                if let Some(bar) = weak.upgrade() {
                    bar.link_event(link, event);
                }
            });
            let process = launched.process;
            Link::serve(id, launched.generation, process, launched.stream, events).map_err(|_| {
                if let Some(worker) = &worker {
                    launch::end(worker, process);
                }
                log(LogLevel::Warning, "file-bar-start-failed", "stage", "link");
                StartFailure::Crashed
            })
        });
        let now = Instant::now();
        let mut state = self.lock();
        let mut left = None;
        let refusal = match link {
            Ok(link) if state.lifecycle.started(now, id) => {
                if link.ended() {
                    // The process left before its link went into service.
                    left = Some(link.process());
                    if state.lifecycle.link_ended(now, id) {
                        Refusal::CrashedRepeatedly
                    } else {
                        Refusal::Restarting
                    }
                } else {
                    state.link = Some(Arc::clone(&link));
                    let pending = std::mem::take(&mut state.pending);
                    for call in &pending {
                        state.lifecycle.demand(now);
                        state.flushing.insert(call.id);
                    }
                    drop(state);
                    for call in pending {
                        self.route(&link, call);
                    }
                    return;
                }
            }
            Ok(link) => {
                // The Session stopped while the process started.
                drop(state);
                link.send(&LinkMessage::Drain(DrainReason::Session));
                return;
            }
            Err(failure) => state.lifecycle.start_failed(now, failure),
        };
        let pending = std::mem::take(&mut state.pending);
        drop(state);
        if refusal == Refusal::CrashedRepeatedly {
            log(
                LogLevel::Error,
                "file-bar-unavailable",
                "reason",
                "crashed-repeatedly",
            );
        }
        for call in pending {
            call.refuse(refusal);
        }
        if let (Some(worker), Some(process)) = (&worker, left) {
            launch::end(worker, process);
        }
    }

    fn link_event(&self, link: u64, event: Event) {
        let now = Instant::now();
        let mut state = self.lock();
        let ended = match event {
            Event::CallEnded => {
                state.lifecycle.call_ended(now, link);
                None
            }
            Event::Activity(holds) => {
                state.lifecycle.holds(now, link, holds);
                None
            }
            Event::Ended => state.link.take_if(|current| current.id() == link),
        };
        let mut ending = None;
        if let Some(ended) = ended {
            let worker = (self.host.worker)();
            if worker.as_ref().map(WorkerRequestHandle::generation) == Some(ended.generation()) {
                if state.lifecycle.link_ended(now, link) {
                    log(
                        LogLevel::Error,
                        "file-bar-unavailable",
                        "reason",
                        "crashed-repeatedly",
                    );
                }
                ending = worker.map(|worker| (worker, ended.process()));
            } else {
                // The Worker that started it was replaced, which ended its process.
                state.lifecycle.retire();
                state.generation = worker.map(|worker| worker.generation());
            }
        }
        drop(state);
        self.signal.wake();
        // A process that lost its link exits; one that stopped reading is ended.
        if let Some((worker, process)) = ending {
            launch::end(&worker, process);
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_supervisor(&self) -> MutexGuard<'_, Option<JoinHandle<()>>> {
        self.supervisor
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Ends `stream` with `error`, as `orchbee-files` would answer its call.
fn answer(mailbox: &Mailbox, stream: MultiplexStreamId, error: FilesError) {
    let record = encode_files_record(&FilesRecord::Result(encode_reply(&FilesReply::Error(
        error,
    ))));
    let delivered = record.is_ok_and(|record| {
        mailbox
            .frame(stream, NonZeroU64::MIN, StreamFrameKind::Result, record)
            .is_ok()
    });
    let reason = if delivered {
        StreamCloseReason::Complete
    } else {
        StreamCloseReason::Capacity
    };
    mailbox.close(stream, reason, false);
}

fn unavailable(reason: &'static str) -> FilesError {
    FilesError::unavailable(Token::new(reason).expect("each reason is a token"))
}

fn log(level: LogLevel, event: &'static str, field: &'static str, value: &'static str) {
    orchbee_local_runtime::log_event(level, event, &[(field, LogValue::Token(value))]);
}
