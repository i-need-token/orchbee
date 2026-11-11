//! The Session's Agent catalog: one row per startable provider saying whether this Session's
//! machine has its command, kept current by the registered Worker's Agent probe job. It owns no
//! Client answer and no Workbench state; the `workbench.agent-catalog` read and refresh call
//! [`AgentCatalog::read`] and [`AgentCatalog::refresh`], and the probe pump owns the Worker
//! exchange.
//!
//! Rows follow the startable catalog's order. A provider whose start command the registry classes
//! as generic ([`StartableAgent::checkable`]) is `CantCheck`, and its command is never sent; every
//! other row shows the last completed lookup's answer for its command.
//!
//! # Lookup schedule
//!
//! States:
//! - `NoWorker`: no Worker generation is registered, at first and after the registered one retires.
//! - `Due`: the registered generation needs a lookup, which the pump starts next.
//! - `Running`: one lookup is on the Worker; its exchange is in the pump's hands or a poll is due.
//! - `Settled`: no lookup is running, and the last one ended at a known instant.
//! - `Stopped`: the Session is stopping; the pump exits and nothing starts again.
//!
//! Transitions:
//! - `begin(generation)`, from any state but `Stopped`: `Due`. A lookup of an older generation is
//!   abandoned unanswered, because its Worker is gone.
//! - The pump takes `Due`: `Running`, with a Start naming the checkable commands.
//! - Start `Accepted` or Poll `Pending`: `Running`, with a Poll after [`POLL_INTERVAL`].
//! - Poll `Completed` with one answer per command: `Settled`, status `Complete`, checked now.
//! - Start `Unsupported`: `Settled`, status `Unsupported`.
//! - Any other answer, a lost exchange, or [`LOOKUP_DEADLINE`] passing: `Settled`, status `Failed`.
//!   A lookup the Worker had accepted is then cancelled once, so its job slot frees.
//! - `refresh` in `Settled` at least [`REFRESH_FLOOR`] after the last end: `Due`. A refresh in
//!   `Due` or `Running` is absorbed, one sooner in `Settled` is ignored, and one in `NoWorker` or
//!   `Stopped` does nothing.
//! - `retire(the registered generation)`: `NoWorker`; a `Due` or `Running` lookup ends `Failed`.
//! - `stop`: `Stopped`.
//!
//! Status `Running` covers `Due` and `Running`. A failed later lookup keeps the last completed
//! answers. While no lookup has completed, checkable rows are `Checking` during a lookup and
//! `Unknown` after a failed one; `Unsupported` leaves them `Unknown`. The revision starts at 1 and
//! advances whenever what [`AgentCatalog::read`] shows changes, Recent included.
//!
//! # Recent
//!
//! The catalog also holds the Session's Recent list, which the Session coordinator feeds through
//! [`AgentCatalog::started`] when it accepts a creation carrying an Agent start and when a run of a
//! startable provider is first published. The checkpoint saves it and a restored Session hands it
//! back through [`AgentCatalog::restore_recent`]; a Recent change alone advances no Workbench
//! revision, so it reaches disk with the next periodic or final save.
use orchbee_agent_observation_protocol::{
    AgentReportProvider, STARTABLE_AGENT_PROVIDER_IDS, StartableAgent,
};
use orchbee_capability_node_protocol::{
    AgentProbeJobAction, AgentProbeJobOutcome as Outcome, AgentProbeJobPollOutcome as Poll,
    AgentProbeJobResult, AgentProbeJobStartOutcome as Start, AgentProbeResult, OperationId,
    WorkerGeneration, WorkerId,
};
use orchbee_local_runtime::{LogLevel, LogValue, log_event};
use orchbee_server::{SessionChangeSignal, SessionChangeSource};
use orchbee_workbench_protocol::AgentStartSelector;
use recent::Recent;
use std::{
    num::NonZeroU64,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
    time::{Duration, Instant},
};

mod probes;
mod recent;
#[cfg(test)]
mod tests;

/// The pause between polls of a running lookup.
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How long the Server waits for one lookup: the Worker's own six seconds and the exchanges
/// around them.
const LOOKUP_DEADLINE: Duration = Duration::from_secs(10);
/// A refresh this soon after the last lookup ended starts nothing.
const REFRESH_FLOOR: Duration = Duration::from_secs(2);
/// Whether this Session's machine can start Agents at all: only Windows Sessions start them
/// today. A policy constant rather than a `cfg`, so every branch compiles everywhere.
const STARTABLE: bool = cfg!(windows);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LookupStatus {
    Running,
    Complete,
    Failed,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentRowState {
    Found,
    NotFound,
    CantCheck,
    Checking,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentCatalogRow {
    pub(crate) provider_id: &'static str,
    pub(crate) product_name: &'static str,
    /// The start command and its fixed arguments, joined by a space.
    pub(crate) command: String,
    pub(crate) state: AgentRowState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentCatalogSnapshot {
    pub(crate) revision: NonZeroU64,
    pub(crate) startable: bool,
    pub(crate) status: LookupStatus,
    /// Unix milliseconds when the last lookup completed; `None` before one has.
    pub(crate) checked_at: Option<u64>,
    /// Recently started provider ids, newest first, one each.
    pub(crate) recent: Vec<&'static str>,
    /// Exactly one row per startable provider, in catalog order.
    pub(crate) agents: Vec<AgentCatalogRow>,
}

/// What the probe pump does next.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Step {
    /// Exchange `action` with Worker `worker` of the ticket's generation, then hand the answer to
    /// [`AgentCatalog::apply`] with the same ticket.
    Exchange {
        ticket: Ticket,
        worker: WorkerId,
        action: AgentProbeJobAction,
    },
    /// Nothing is due before the deadline, or before a wake when there is none.
    Wait(Option<Instant>),
    Stopped,
}

/// Fences an answer to the lookup and Worker generation its exchange was handed out for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Ticket {
    pub(crate) generation: WorkerGeneration,
    lookup: u64,
}

/// Registry and pump wake. Lock order: this is a leaf; no other lock is taken while it is held,
/// and watchers are woken after it is released.
#[derive(Debug)]
pub(crate) struct AgentCatalog {
    state: Mutex<State>,
    pump: Condvar,
    watchers: SessionChangeSource,
    prober: Mutex<Option<JoinHandle<()>>>,
}

impl Default for AgentCatalog {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::new()),
            pump: Condvar::new(),
            watchers: SessionChangeSource::default(),
            prober: Mutex::new(None),
        }
    }
}

impl AgentCatalog {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Runs `change` and publishes its effect: the revision advances when the shown catalog
    /// changed, and watchers wake when what [`read`](Self::read) answers changed.
    fn change<T>(&self, change: impl FnOnce(&mut State) -> T) -> T {
        let mut state = self.lock();
        let before = state.published();
        let value = change(&mut state);
        state.publish();
        let woken = state.woken;
        let published = state.published() != before;
        drop(state);
        if woken {
            self.pump.notify_all();
        }
        if published {
            self.watchers.notify_agent_catalog();
        }
        value
    }

    /// Wakes `signal` whenever what [`read`](Self::read) answers changes.
    pub(crate) fn watch(&self, signal: &Arc<SessionChangeSignal>) {
        self.watchers.watch(signal);
    }

    /// Records a lookup that ended. Only the pump records, so a caller holding the Worker
    /// registry, such as a retiring carrier, never waits on the diagnostic log.
    fn record(&self) {
        let ended = self.lock().ended.take();
        if let Some(ended) = ended {
            ended.record();
        }
    }

    /// A newly registered Worker generation replaces the previous one and is looked up at once.
    pub(crate) fn begin(&self, worker: WorkerId, generation: NonZeroU64) {
        self.change(|state| state.begin(worker, WorkerGeneration::new(generation)));
    }

    /// The generation's carrier retired; a lookup it was running ends failed.
    pub(crate) fn retire(&self, generation: NonZeroU64) {
        self.change(|state| state.retire(WorkerGeneration::new(generation), Instant::now()));
    }

    /// The catalog as it stands; `None` before the first Worker registered and once stopping.
    pub(crate) fn read(&self) -> Option<AgentCatalogSnapshot> {
        self.lock().snapshot()
    }

    /// Asks for a new lookup under the schedule's rules and answers with the catalog as it
    /// stands.
    pub(crate) fn refresh(&self, now: Instant) -> Option<AgentCatalogSnapshot> {
        self.change(|state| state.refresh(now));
        self.read()
    }

    pub(crate) fn next(&self, now: Instant) -> Step {
        let step = self.change(|state| state.next(now));
        self.record();
        step
    }

    /// Applies the answer to the exchange handed out with `ticket`; `None` when it could not be
    /// admitted or answered. `unix_ms` stamps a completed lookup.
    pub(crate) fn apply(
        &self,
        ticket: Ticket,
        result: Option<AgentProbeJobResult>,
        now: Instant,
        unix_ms: u64,
    ) {
        self.change(|state| state.apply(ticket, result, now, unix_ms));
        self.record();
    }

    /// Moves `agent` to the front of Recent.
    pub(crate) fn started(&self, agent: AgentStartSelector) {
        self.change(|state| state.recent.start(agent));
    }

    /// Replaces Recent with the list a checkpoint saved, newest first. Only Windows Sessions keep a
    /// checkpoint.
    #[cfg(windows)]
    pub(crate) fn restore_recent(&self, saved: &[AgentStartSelector]) {
        self.change(|state| state.recent = Recent::restored(saved));
    }

    /// Recent as the checkpoint saves it, newest first.
    #[cfg(windows)]
    pub(crate) fn recent(&self) -> Vec<AgentStartSelector> {
        self.lock().recent.agents().to_vec()
    }

    /// Whether the pump should keep waiting for the answer to `ticket`'s exchange.
    pub(crate) fn awaits(&self, ticket: Ticket, now: Instant) -> bool {
        self.lock().awaits(ticket, now)
    }

    /// Waits for a wake, the deadline or a stop, whichever comes first.
    pub(crate) fn wait(&self, deadline: Option<Instant>) {
        let state = self.lock();
        if state.stopped || state.woken {
            return;
        }
        let state = match deadline {
            Some(deadline) => {
                self.pump
                    .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .0
            }
            None => self
                .pump
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        };
        drop(state);
    }

    /// Only a Windows Session stop joins the pump; elsewhere it lives as long as the process.
    #[cfg(windows)]
    pub(crate) fn stop(&self) {
        let mut state = self.lock();
        let before = state.published();
        state.stopped = true;
        let published = state.published() != before;
        drop(state);
        self.pump.notify_all();
        if published {
            self.watchers.notify_agent_catalog();
        }
    }

    /// A start of `agent` found its command missing in a new shell: its row reads `NotFound` until
    /// the next completed lookup. A row the catalog cannot check, or one no completed lookup has
    /// answered, stays as it is.
    #[cfg(windows)]
    pub(crate) fn mark_not_found(&self, agent: StartableAgent) {
        self.change(|state| {
            let index = state
                .entries
                .iter()
                .find(|entry| entry.agent.id() == agent.id())
                .and_then(|entry| entry.probe);
            if let Some(answer) = index
                .zip(state.answers.as_mut())
                .and_then(|(index, answers)| answers.get_mut(index))
            {
                *answer = AgentProbeResult::Missing;
            }
        });
    }
}

/// One startable provider's fixed catalog facts.
#[derive(Debug)]
struct Entry {
    agent: StartableAgent,
    product_name: &'static str,
    command: String,
    /// This row's position among a lookup's commands, when the provider is checkable.
    probe: Option<usize>,
}

#[derive(Debug)]
enum Lookup {
    NoWorker,
    Due,
    Running(Running),
    Settled { ended: Instant },
}

#[derive(Debug)]
struct Running {
    ticket: Ticket,
    deadline: Instant,
    phase: Phase,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    /// The pump holds this lookup's exchange; `job` once the Worker accepted it.
    Exchanging { job: Option<OperationId> },
    /// The Worker runs job `job`; the next Poll is due at `at`.
    Polling { job: OperationId, at: Instant },
}

/// A Cancel the pump owes the Worker job of a lookup that ended failed.
#[derive(Debug)]
struct Owed {
    ticket: Ticket,
    job: OperationId,
}

/// What [`AgentCatalog::read`] shows, apart from the revision.
#[derive(Debug, Eq, PartialEq)]
struct Shown {
    status: LookupStatus,
    checked_at: Option<u64>,
    states: Vec<AgentRowState>,
    recent: Recent,
}

/// A lookup that ended, as its diagnostic record carries it.
#[derive(Debug)]
struct Ended {
    status: LookupStatus,
    found: u64,
    missing: u64,
    unavailable: u64,
}

impl Ended {
    fn record(&self) {
        let (level, outcome) = match self.status {
            LookupStatus::Complete => (LogLevel::Info, "complete"),
            LookupStatus::Unsupported => (LogLevel::Info, "unsupported"),
            LookupStatus::Failed | LookupStatus::Running => (LogLevel::Warning, "failed"),
        };
        log_event(
            level,
            "agent-catalog-lookup",
            &[
                ("outcome", LogValue::Token(outcome)),
                ("found", LogValue::Count(self.found)),
                ("missing", LogValue::Count(self.missing)),
                ("unavailable", LogValue::Count(self.unavailable)),
            ],
        );
    }
}

#[derive(Debug)]
struct State {
    entries: Vec<Entry>,
    /// The Start every lookup sends; `None` when the checkable commands exceed the job's bounds,
    /// so every lookup fails.
    start: Option<AgentProbeJobAction>,
    commands: usize,
    worker: Option<(WorkerId, WorkerGeneration)>,
    /// A Worker generation has registered at least once.
    begun: bool,
    lookup: Lookup,
    lookups: u64,
    status: LookupStatus,
    /// The last completed lookup's answers, one per command.
    answers: Option<Vec<AgentProbeResult>>,
    checked_at: Option<u64>,
    owed: Option<Owed>,
    recent: Recent,
    revision: NonZeroU64,
    shown: Shown,
    /// A lookup that ended and the pump has not recorded yet.
    ended: Option<Ended>,
    stopped: bool,
    /// A wake that arrived while the pump was busy, consumed by its next `next`.
    woken: bool,
}

impl State {
    fn new() -> Self {
        let mut commands = Vec::new();
        let entries = STARTABLE_AGENT_PROVIDER_IDS
            .iter()
            .filter_map(|id| {
                let agent = StartableAgent::parse(id)?;
                let product_name = AgentReportProvider::from_native_provider_id(id)?.product_name();
                let probe = agent.checkable().then(|| {
                    commands.push(agent.executable().to_owned());
                    commands.len() - 1
                });
                let command = std::iter::once(agent.executable())
                    .chain(agent.launch_arguments().iter().copied())
                    .collect::<Vec<_>>()
                    .join(" ");
                Some(Entry {
                    agent,
                    product_name,
                    command,
                    probe,
                })
            })
            .collect();
        let count = commands.len();
        let mut state = Self {
            entries,
            start: AgentProbeJobAction::start(commands).ok(),
            commands: count,
            worker: None,
            begun: false,
            lookup: Lookup::NoWorker,
            lookups: 0,
            status: LookupStatus::Running,
            answers: None,
            checked_at: None,
            owed: None,
            recent: Recent::default(),
            revision: NonZeroU64::MIN,
            shown: Shown {
                status: LookupStatus::Running,
                checked_at: None,
                states: Vec::new(),
                recent: Recent::default(),
            },
            ended: None,
            stopped: false,
            woken: false,
        };
        state.shown = state.shown();
        state
    }

    fn begin(&mut self, worker: WorkerId, generation: WorkerGeneration) {
        if self.stopped {
            return;
        }
        self.worker = Some((worker, generation));
        self.begun = true;
        self.owed = None;
        self.lookup = Lookup::Due;
        self.status = LookupStatus::Running;
        self.woken = true;
    }

    fn retire(&mut self, generation: WorkerGeneration, now: Instant) {
        if self.worker.is_none_or(|(_, current)| current != generation) {
            return;
        }
        if matches!(self.lookup, Lookup::Due | Lookup::Running(_)) {
            self.end(LookupStatus::Failed, now);
        }
        self.worker = None;
        self.owed = None;
        self.lookup = Lookup::NoWorker;
    }

    fn refresh(&mut self, now: Instant) {
        if let Lookup::Settled { ended } = self.lookup
            && !self.stopped
            && self.worker.is_some()
            && now.saturating_duration_since(ended) >= REFRESH_FLOOR
        {
            self.lookup = Lookup::Due;
            self.status = LookupStatus::Running;
            self.woken = true;
        }
    }

    fn next(&mut self, now: Instant) -> Step {
        self.woken = false;
        loop {
            if self.stopped {
                return Step::Stopped;
            }
            let Some((worker, generation)) = self.worker else {
                return Step::Wait(None);
            };
            if let Some(owed) = self.owed.take() {
                if owed.ticket.generation == generation {
                    return Step::Exchange {
                        ticket: owed.ticket,
                        worker,
                        action: AgentProbeJobAction::Cancel { job_id: owed.job },
                    };
                }
                continue;
            }
            match &mut self.lookup {
                Lookup::NoWorker | Lookup::Settled { .. } => return Step::Wait(None),
                Lookup::Due => {
                    self.lookups += 1;
                    let ticket = Ticket {
                        generation,
                        lookup: self.lookups,
                    };
                    let Some(action) = self.start.clone() else {
                        self.end(LookupStatus::Failed, now);
                        continue;
                    };
                    self.lookup = Lookup::Running(Running {
                        ticket,
                        deadline: now + LOOKUP_DEADLINE,
                        phase: Phase::Exchanging { job: None },
                    });
                    return Step::Exchange {
                        ticket,
                        worker,
                        action,
                    };
                }
                Lookup::Running(running) if now >= running.deadline => {
                    self.end(LookupStatus::Failed, now);
                }
                Lookup::Running(running) => match running.phase {
                    Phase::Polling { job, at } if now >= at => {
                        running.phase = Phase::Exchanging { job: Some(job) };
                        return Step::Exchange {
                            ticket: running.ticket,
                            worker,
                            action: AgentProbeJobAction::Poll { job_id: job },
                        };
                    }
                    Phase::Polling { at, .. } => {
                        return Step::Wait(Some(at.min(running.deadline)));
                    }
                    Phase::Exchanging { .. } => return Step::Wait(Some(running.deadline)),
                },
            }
        }
    }

    fn apply(
        &mut self,
        ticket: Ticket,
        result: Option<AgentProbeJobResult>,
        now: Instant,
        unix_ms: u64,
    ) {
        let Lookup::Running(running) = &mut self.lookup else {
            return;
        };
        let Phase::Exchanging { job } = running.phase else {
            return;
        };
        if running.ticket != ticket {
            return;
        }
        let Some(result) = result else {
            self.end(LookupStatus::Failed, now);
            return;
        };
        match (job, result.outcome) {
            (None, Outcome::Start(Start::Accepted)) => {
                running.phase = Phase::Polling {
                    job: result.job_id,
                    at: now + POLL_INTERVAL,
                };
            }
            (Some(job), Outcome::Poll(Poll::Pending)) => {
                running.phase = Phase::Polling {
                    job,
                    at: now + POLL_INTERVAL,
                };
            }
            (Some(_), Outcome::Poll(Poll::Completed { results }))
                if results.len() == self.commands =>
            {
                self.answers = Some(results);
                self.checked_at = Some(unix_ms);
                self.end(LookupStatus::Complete, now);
            }
            (None, Outcome::Start(Start::Unsupported)) => {
                self.end(LookupStatus::Unsupported, now);
            }
            _ => self.end(LookupStatus::Failed, now),
        }
    }

    fn awaits(&self, ticket: Ticket, now: Instant) -> bool {
        !self.stopped
            && matches!(&self.lookup, Lookup::Running(running)
                if running.ticket == ticket && now < running.deadline)
    }

    /// Settles the running lookup with `status`; a failed one the Worker had accepted owes it a
    /// Cancel.
    fn end(&mut self, status: LookupStatus, now: Instant) {
        if status == LookupStatus::Failed
            && let Lookup::Running(Running {
                ticket,
                phase: Phase::Exchanging { job: Some(job) } | Phase::Polling { job, .. },
                ..
            }) = self.lookup
        {
            self.owed = Some(Owed { ticket, job });
        }
        self.lookup = Lookup::Settled { ended: now };
        // The pump wakes to record the ending.
        self.woken = true;
        self.status = status;
        let count = |wanted| {
            self.answers.as_ref().map_or(0, |answers| {
                answers.iter().filter(|answer| **answer == wanted).count() as u64
            })
        };
        self.ended = Some(if status == LookupStatus::Complete {
            Ended {
                status,
                found: count(AgentProbeResult::Found),
                missing: count(AgentProbeResult::Missing),
                unavailable: count(AgentProbeResult::Unavailable),
            }
        } else {
            Ended {
                status,
                found: 0,
                missing: 0,
                unavailable: 0,
            }
        });
    }

    fn state(&self, entry: &Entry) -> AgentRowState {
        let Some(index) = entry.probe else {
            return AgentRowState::CantCheck;
        };
        if self.status == LookupStatus::Unsupported {
            return AgentRowState::Unknown;
        }
        match self.answers.as_ref().and_then(|answers| answers.get(index)) {
            Some(AgentProbeResult::Found) => AgentRowState::Found,
            Some(AgentProbeResult::Missing) => AgentRowState::NotFound,
            Some(AgentProbeResult::Unavailable) => AgentRowState::Unknown,
            None if self.status == LookupStatus::Running => AgentRowState::Checking,
            None => AgentRowState::Unknown,
        }
    }

    fn shown(&self) -> Shown {
        Shown {
            status: self.status,
            checked_at: self.checked_at,
            states: self.entries.iter().map(|entry| self.state(entry)).collect(),
            recent: self.recent.clone(),
        }
    }

    fn publish(&mut self) {
        let shown = self.shown();
        if shown != self.shown {
            self.shown = shown;
            self.revision = self.revision.saturating_add(1);
        }
    }

    /// What decides [`AgentCatalog::read`]'s answer: whether there is one, and its revision.
    const fn published(&self) -> (bool, NonZeroU64) {
        (self.begun && !self.stopped, self.revision)
    }

    fn snapshot(&self) -> Option<AgentCatalogSnapshot> {
        if !self.begun || self.stopped {
            return None;
        }
        Some(AgentCatalogSnapshot {
            revision: self.revision,
            startable: STARTABLE,
            status: self.shown.status,
            checked_at: self.shown.checked_at,
            recent: self
                .shown
                .recent
                .agents()
                .iter()
                .map(|agent| agent.agent().id())
                .collect(),
            agents: self
                .entries
                .iter()
                .zip(&self.shown.states)
                .map(|(entry, state)| AgentCatalogRow {
                    provider_id: entry.agent.id(),
                    product_name: entry.product_name,
                    command: entry.command.clone(),
                    state: *state,
                })
                .collect(),
        })
    }
}
