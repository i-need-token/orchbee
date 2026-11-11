//! The one thread that carries every Agent start candidate of a Windows Session: it reads each new
//! terminal through the process observer's on-demand lane, types a marked cmd's line through the
//! Host's prompt gate, and publishes the failure fact the decision names. Candidates are offered
//! under the registry and coordinator locks the creation settled under, and Client input is noted
//! under the coordinator lock, so input either precedes a send's admission or follows its send.
//! The state lock is a leaf: nothing is acquired while it is held. Every wait runs on this thread,
//! which holds no lock while it waits.
use super::model::{Command, Delivery, Event, Observation};
use crate::local_terminal_service::{
    LocalTerminalService,
    agent_resume::{Readiness, ResumeFacts, readiness},
    controller_connection::ControlTarget,
    placement,
    prompt_line::{self, Answer},
};
use crate::{
    session_coordinator::{
        AgentStartLaunch, ProcessObservationTarget, ResumeTarget, SessionCoordinator,
        names_provider,
    },
    terminal_coordinator::{BindingViewerTarget, WorkerPlacement},
};
use orchbee_agent_observation_protocol::{ShellRoot, idle_shell_only};
use orchbee_client_server_protocol::SendPromptState;
use orchbee_terminal_protocol::MAXIMUM_PROCESS_SAMPLE_ROWS;
use orchbee_workbench_protocol::AgentStartFailureReason;
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// A wait with nothing due still wakes now and then; offers and stops wake it at once.
const IDLE_WAIT: Duration = Duration::from_secs(3600);

/// One launch's start: what it started, the terminal and generation it started in, its decision,
/// and when it is next due.
struct Candidate {
    launch: AgentStartLaunch,
    target: ProcessObservationTarget,
    delivery: Delivery,
    due: Instant,
}

#[derive(Default)]
struct State {
    /// The thread runs and takes offers.
    running: bool,
    stop: bool,
    /// Candidates offered since the thread last took them.
    offered: Vec<Candidate>,
    /// Each live candidate's terminal, and whether Client input reached it since its launch.
    touched: BTreeMap<ControlTarget, bool>,
}

#[derive(Default)]
struct Signals {
    state: Mutex<State>,
    changed: Condvar,
}

impl Signals {
    /// Every update is one assignment, insertion or removal, so a poisoned state is still
    /// consistent.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hands a candidate to the running thread; false when none runs or the Server is stopping.
    fn offer(&self, candidate: Candidate) -> bool {
        let mut state = self.state();
        if !state.running || state.stop {
            return false;
        }
        state
            .touched
            .insert(ControlTarget::from(candidate.target.target), false);
        state.offered.push(candidate);
        self.changed.notify_all();
        true
    }

    fn take_offered(&self) -> Vec<Candidate> {
        std::mem::take(&mut self.state().offered)
    }

    fn note_input(&self, target: BindingViewerTarget) {
        if let Some(touched) = self.state().touched.get_mut(&ControlTarget::from(target)) {
            *touched = true;
        }
    }

    fn touched(&self, target: BindingViewerTarget) -> bool {
        self.state()
            .touched
            .get(&ControlTarget::from(target))
            .copied()
            .unwrap_or(false)
    }

    fn release(&self, target: BindingViewerTarget) {
        self.state().touched.remove(&ControlTarget::from(target));
    }

    fn stop(&self) {
        self.state().stop = true;
        self.changed.notify_all();
    }

    fn stopping(&self) -> bool {
        self.state().stop
    }

    /// Waits until `until`, or without end when `None`, unless an offer or a stop arrives first.
    /// False when stopping.
    fn wait(&self, until: Option<Instant>) -> bool {
        let mut state = self.state();
        loop {
            if state.stop {
                return false;
            }
            if !state.offered.is_empty() {
                return true;
            }
            let wait = match until {
                None => IDLE_WAIT,
                Some(until) => match until.checked_duration_since(Instant::now()) {
                    Some(wait) if !wait.is_zero() => wait,
                    _ => return true,
                },
            };
            state = self
                .changed
                .wait_timeout(state, wait)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// The thread ended: offers stop, and the candidates it carried end without typing.
    fn finish(&self) -> usize {
        let mut state = self.state();
        state.running = false;
        state.touched.clear();
        std::mem::take(&mut state.offered).len()
    }
}

#[derive(Default)]
pub(in crate::local_terminal_service) struct AgentStartDelivery {
    signals: Arc<Signals>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for AgentStartDelivery {
    fn drop(&mut self) {
        // Session ownership is process-lifetime; dropping signals the thread and never joins.
        self.signals.stop();
    }
}

/// How a guard found the candidate's terminal before a read.
enum Guard {
    /// The Presentation keeps the launch's binding and generation.
    Kept {
        target: ProcessObservationTarget,
        touched: bool,
    },
    /// It left, rebound, or its terminal stopped answering.
    Refuted,
}

impl LocalTerminalService {
    /// Starts the start delivery thread. Only Windows Session composition starts it, before any
    /// Client can create a terminal; until then, and once it stops, a creation's start is dropped
    /// with nothing typed.
    pub(crate) fn start_agent_start_delivery(self: &Arc<Self>) -> io::Result<()> {
        let delivery = &self.agent_start_delivery;
        let mut slot = delivery
            .handle
            .lock()
            .map_err(|_| io::Error::other("Agent start delivery owner poisoned"))?;
        if slot.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        {
            let mut state = delivery.signals.state();
            if state.stop {
                return Err(io::ErrorKind::NotConnected.into());
            }
            state.running = true;
        }
        let service = Arc::downgrade(self);
        let signals = Arc::clone(&delivery.signals);
        let spawned = thread::Builder::new()
            .name("orchbee-agent-start".into())
            .spawn(move || run(&service, &signals));
        match spawned {
            Ok(handle) => *slot = Some(handle),
            Err(error) => {
                delivery.signals.finish();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Stops the thread and joins it within the Session owner's deadline. A candidate it carried
    /// ends with nothing typed and no fact. Idempotent.
    pub(crate) fn stop_agent_start_delivery_before(&self, deadline: Instant) -> io::Result<()> {
        self.agent_start_delivery.signals.stop();
        loop {
            {
                let mut handle = self
                    .agent_start_delivery
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Agent start delivery handle poisoned"))?;
                if handle.as_ref().is_none_or(JoinHandle::is_finished) {
                    if let Some(handle) = handle.take() {
                        handle
                            .join()
                            .map_err(|_| io::Error::other("Agent start delivery panicked"))?;
                    }
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Hands a settled creation's Agent start to the thread as one candidate for the terminal it
    /// started. The caller holds the registry and coordinator locks the creation settled under, so
    /// notes about that terminal start before any Client input can reach it.
    pub(in crate::local_terminal_service) fn deliver_agent_start(
        &self,
        coordinator: &mut SessionCoordinator,
        current: Option<WorkerPlacement>,
        launch: &AgentStartLaunch,
    ) {
        let target = match coordinator.agent_resume_target(&launch.presentation, current) {
            ResumeTarget::Started(target) if target.binding == launch.binding => target,
            ResumeTarget::Started(_) | ResumeTarget::Unavailable(_) | ResumeTarget::Gone => {
                return report("dropped:unavailable");
            }
        };
        let now = Instant::now();
        let candidate = Candidate {
            launch: launch.clone(),
            target,
            delivery: Delivery::new(now),
            due: now,
        };
        if !self.agent_start_delivery.signals.offer(candidate) {
            report("dropped:not-running");
        }
    }

    /// A Client's typed input, paste or send was admitted for `target`. The caller holds the
    /// coordinator lock.
    pub(in crate::local_terminal_service) fn note_agent_start_input(
        &self,
        target: BindingViewerTarget,
    ) {
        self.agent_start_delivery.signals.note_input(target);
    }

    /// One due step of `candidate`: its guards and a read, then the send its decision asks for.
    /// True when the candidate ended.
    fn advance_agent_start(&self, signals: &Signals, candidate: &mut Candidate) -> bool {
        let event = match self.guard_agent_start(signals, candidate) {
            Guard::Refuted => Event::Refuted,
            Guard::Kept { target, touched } => {
                let facts = self.request_resume_facts(target).unwrap_or_default();
                Event::Read {
                    observation: observe(&facts, candidate.launch.provider.agent().id()),
                    touched,
                }
            }
        };
        let mut command = candidate.delivery.step(event, Instant::now());
        loop {
            match command {
                Command::ReadAt(due) => {
                    candidate.due = due;
                    return false;
                }
                Command::Send => {
                    let event = self.send_agent_start(signals, candidate);
                    command = candidate.delivery.step(event, Instant::now());
                }
                Command::End(reason) => {
                    self.end_agent_start(candidate, reason);
                    return true;
                }
            }
        }
    }

    /// Checks under the registry and coordinator locks that the Presentation keeps the launch's
    /// binding and generation, and whether Client input or a shell command reached its terminal.
    fn guard_agent_start(&self, signals: &Signals, candidate: &Candidate) -> Guard {
        let (Ok(workers), Ok(coordinator)) = (self.workers.lock(), self.coordinator.lock()) else {
            return Guard::Refuted;
        };
        let current = placement(workers.request_handle().as_ref());
        match coordinator.agent_resume_target(&candidate.launch.presentation, current) {
            ResumeTarget::Started(target)
                if target.binding == candidate.target.binding
                    && target.target == candidate.target.target =>
            {
                let touched = signals.touched(target.target)
                    || coordinator.agent_resume_command_observed(&target);
                Guard::Kept { target, touched }
            }
            ResumeTarget::Started(_) | ResumeTarget::Unavailable(_) | ResumeTarget::Gone => {
                Guard::Refuted
            }
        }
    }

    /// Sends the bare line once through the Host's prompt gate. The guards are checked again
    /// under the send's own locks, so Client input admitted before the send refutes it.
    fn send_agent_start(&self, signals: &Signals, candidate: &Candidate) -> Event {
        let command = &candidate.launch.command;
        let line = std::iter::once(command.program())
            .chain(command.arguments().iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        let Some(request) = prompt_line::request(&candidate.target, &line, false) else {
            return Event::Failed;
        };
        let mut admitted = false;
        let message = self.deliver_send(&request, |coordinator, sent| {
            admitted = matches!(
                coordinator.agent_resume_target(&candidate.launch.presentation, Some(sent.worker)),
                ResumeTarget::Started(target)
                    if target.binding == candidate.target.binding
                        && target.target == candidate.target.target
                        && target.target == *sent
                        && !signals.touched(target.target)
                        && !coordinator.agent_resume_command_observed(&target)
            );
            admitted
        });
        if !admitted {
            return Event::Refuted;
        }
        match prompt_line::answer(&message) {
            Answer::Accepted => Event::Accepted,
            Answer::Refused(state) => Event::Refused {
                settling: state == SendPromptState::Settling,
            },
            Answer::GateUnsupported => Event::GateUnsupported,
            Answer::Failed => Event::Failed,
        }
    }

    /// Ends the candidate: publishes its fact on the Presentation still bound as the launch
    /// committed it, and lets a `not-found` mark its catalog row.
    fn end_agent_start(&self, candidate: &Candidate, reason: Option<AgentStartFailureReason>) {
        report(match (reason, candidate.delivery.typed()) {
            (None, false) => "ended",
            (None, true) => "ended:typed",
            (Some(AgentStartFailureReason::NotFound), _) => "failed:not-found",
            (Some(AgentStartFailureReason::Exited), _) => "failed:exited",
            (Some(AgentStartFailureReason::UnsupportedShell), _) => "failed:unsupported-shell",
            (Some(AgentStartFailureReason::PromptUnconfirmed), _) => "failed:prompt-unconfirmed",
            (Some(AgentStartFailureReason::TerminalTooOld), _) => "failed:terminal-too-old",
        });
        let Some(reason) = reason else {
            return;
        };
        if let Ok(mut coordinator) = self.coordinator.lock() {
            coordinator.publish_agent_start_failure(&candidate.launch, reason);
        }
        if reason == AgentStartFailureReason::NotFound
            && let Ok(workers) = self.workers.lock()
        {
            workers
                .agent_catalog()
                .mark_not_found(candidate.launch.provider.agent());
        }
    }
}

/// What one read showed, or nothing when it held no process sample.
fn observe(facts: &ResumeFacts, provider: &str) -> Option<Observation> {
    let sample = facts.sample.as_ref()?;
    let others = sample
        .processes
        .iter()
        .filter(|process| process.pid != sample.root_pid);
    let idle = sample
        .processes
        .iter()
        .any(|process| process.pid == sample.root_pid)
        && idle_shell_only(
            ShellRoot::Interactive,
            sample.processes.len() < MAXIMUM_PROCESS_SAMPLE_ROWS,
            others.map(|process| process.image.as_deref()),
        );
    Some(Observation {
        kind: sample.launch_kind,
        idle,
        provider: names_provider(sample, provider),
        prompt: matches!(readiness(facts, None), Readiness::Try(_)),
    })
}

/// The closed diagnostic kind; it carries no id.
fn report(outcome: &str) {
    eprintln!("orchbee-server diagnostic component=agent-start outcome={outcome}");
}

/// Ends offers however the thread returns.
struct Finish<'a>(&'a Signals);
impl Drop for Finish<'_> {
    fn drop(&mut self) {
        for _ in 0..self.0.finish() {
            report("dropped:stopping");
        }
    }
}

fn run(service: &Weak<LocalTerminalService>, signals: &Signals) {
    let _finish = Finish(signals);
    let mut candidates: Vec<Candidate> = Vec::new();
    loop {
        candidates.append(&mut signals.take_offered());
        let next = candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, candidate)| candidate.due)
            .map(|(index, candidate)| (index, candidate.due));
        let index = match next {
            Some((index, due)) if due <= Instant::now() => index,
            _ => {
                if signals.wait(next.map(|(_, due)| due)) {
                    continue;
                }
                break;
            }
        };
        let Some(service) = service.upgrade() else {
            break;
        };
        let ended = service.advance_agent_start(signals, &mut candidates[index]);
        drop(service);
        if ended {
            let candidate = candidates.swap_remove(index);
            signals.release(candidate.target.target);
        }
        if signals.stopping() {
            break;
        }
    }
    for _ in &candidates {
        report("dropped:stopping");
    }
}
