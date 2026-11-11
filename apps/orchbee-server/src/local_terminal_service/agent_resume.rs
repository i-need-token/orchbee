//! Owns the one thread that types each retained Agent session reference's resume command into its
//! restored shell after a managed Windows restart. It waits for the first Client view of this
//! Server lifetime, then attempts each pending reference once, viewed terminals first. An attempt
//! sends only through the Host's prompt gate, into a shell the Host launched with prompt marks, of
//! the restored Terminal generation in the retained directory that no Client input or command has
//! touched. A Host refusal typed nothing, so it puts the reference back and the attempt reads and
//! sends again until its deadline; every wait runs on this thread. A held Panel's reference joins
//! when a Retry starts its shell: behind the running driver, or in a new run once the last one
//! ended. An attempt consumes its reference when it ends, by a refutation, an accepted or otherwise
//! failed send, or its deadline, and leaves it pending only while its terminal is unavailable or
//! the Server stops. An attempt that ends because the prompt could not be confirmed publishes one
//! not-resumed fact on its Presentation, the only thing Clients are shown; no session id is ever
//! logged or published.
use super::{LocalTerminalService, create_reference, placement};
use crate::{
    session_coordinator::{ProcessObservationTarget, ResumeTarget, SessionCoordinator},
    terminal_coordinator::{BindingViewerTarget, WorkerPlacement},
};
use orchbee_client_server_protocol::{
    SendPromptState, SendRejectedReason, ServerMessage, SessionId as ObcsSessionId,
    TerminalId as ClientTerminalId, TerminalSendOutcome, TerminalSendRequest,
    WorkerId as ClientWorkerId,
};
use orchbee_server::RetainedAgentSession;
use orchbee_workbench_protocol::{
    AgentNotResumedReason, PresentationBinding, TerminalPresentationId, WorkbenchDirectoryPath,
};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex, Weak},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod admission;
mod plan;
mod signals;
#[cfg(test)]
mod tests;

pub(super) use admission::ResumeFacts;
use admission::{Readiness, readiness};
use plan::ResumeShell;

/// Clients size what they show before the first delivery, and a retried shell's Client sizes it.
const SETTLE: Duration = Duration::from_secs(1);
/// The least time between two deliveries.
const SPACING: Duration = Duration::from_millis(250);
/// The time between two reads of one candidate.
const POLL: Duration = Duration::from_millis(250);
/// How long one candidate may take to read as ready and pass the Host's prompt gate, from the
/// start of its attempt.
const READINESS: Duration = Duration::from_secs(10);
/// The wait before sending again after the Host found the prompt younger than its quiet period.
const SETTLING: Duration =
    Duration::from_millis(orchbee_terminal_protocol::PROMPT_LINE_QUIET_MILLISECONDS);
/// The largest Unix millisecond time the Workbench protocol carries.
const MAXIMUM_UNIX_MILLISECONDS: u64 = 9_007_199_254_740_991;

/// Why a pending reference was consumed without a delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Reason {
    ClientInput,
    CommandObserved,
    BindingChanged,
    DirectoryMismatch,
    NotIdle,
    UnsupportedShell,
    NotReady,
    /// The Host refused every gated send until the deadline, typing nothing.
    PromptUnconfirmed,
    /// The terminal's Host predates the prompt gate.
    TerminalTooOld,
    TokenUnavailable,
    Gone,
    SendFailed,
}

impl Reason {
    /// The fact a give-up publishes: only where the prompt could not be confirmed, so the person
    /// expected the conversation back and nothing came. Client input, an observed command, a
    /// changed or gone Presentation and a moved directory say the person or the layout already
    /// moved on, and an unavailable token or a failed send stays a diagnostic.
    const fn notice(self) -> Option<AgentNotResumedReason> {
        match self {
            Self::NotIdle | Self::UnsupportedShell | Self::NotReady | Self::PromptUnconfirmed => {
                Some(AgentNotResumedReason::PromptUnconfirmed)
            }
            Self::TerminalTooOld => Some(AgentNotResumedReason::TerminalTooOld),
            Self::ClientInput
            | Self::CommandObserved
            | Self::BindingChanged
            | Self::DirectoryMismatch
            | Self::TokenUnavailable
            | Self::Gone
            | Self::SendFailed => None,
        }
    }
}

/// Why a pending reference stays pending for the next checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kept {
    Unavailable,
    Stopping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Sent,
    Skipped(Reason),
    Kept(Kept),
}

impl Outcome {
    /// The closed diagnostic kind; it carries no id.
    const fn kind(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Skipped(Reason::ClientInput) => "skipped:client-input",
            Self::Skipped(Reason::CommandObserved) => "skipped:command-observed",
            Self::Skipped(Reason::BindingChanged) => "skipped:binding-changed",
            Self::Skipped(Reason::DirectoryMismatch) => "skipped:directory-mismatch",
            Self::Skipped(Reason::NotIdle) => "skipped:not-idle",
            Self::Skipped(Reason::UnsupportedShell) => "skipped:unsupported-shell",
            Self::Skipped(Reason::NotReady) => "skipped:not-ready",
            Self::Skipped(Reason::PromptUnconfirmed) => "skipped:prompt-unconfirmed",
            Self::Skipped(Reason::TerminalTooOld) => "skipped:terminal-too-old",
            Self::Skipped(Reason::TokenUnavailable) => "skipped:token-unavailable",
            Self::Skipped(Reason::Gone) => "skipped:gone",
            Self::Skipped(Reason::SendFailed) => "skipped:send-failed",
            Self::Kept(Kept::Unavailable) => "kept:unavailable",
            Self::Kept(Kept::Stopping) => "kept:stopping",
        }
    }

    /// Whether this attempt reached the Worker, which spaces the next one.
    const fn sent(self) -> bool {
        matches!(self, Self::Sent | Self::Skipped(Reason::SendFailed))
    }
}

/// What one gated send came to.
enum Delivery {
    /// The attempt ended.
    Done(Outcome),
    /// The Host typed nothing and the reference is pending again.
    Refused(SendPromptState),
}

struct Candidate {
    presentation: TerminalPresentationId,
    directory: WorkbenchDirectoryPath,
}

/// A candidate with the binding its Presentation had when the driver took it, which every guard
/// compares against.
pub(super) struct Admitted {
    candidate: Candidate,
    committed: Option<PresentationBinding>,
    /// When a Retry started its shell; its attempt waits `SETTLE` from then.
    retried: Option<Instant>,
}

/// The restored Presentations whose reference survived startup, in restoration order, each with
/// the retained directory its resume must find.
#[derive(Default)]
pub(crate) struct AgentResumeCandidates(Vec<Candidate>);

impl AgentResumeCandidates {
    pub(super) fn push(
        &mut self,
        presentation: TerminalPresentationId,
        directory: WorkbenchDirectoryPath,
    ) {
        self.0.push(Candidate {
            presentation,
            directory,
        });
    }
}

#[derive(Default)]
pub(super) struct AgentResume {
    signals: Arc<signals::Signals>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for AgentResume {
    fn drop(&mut self) {
        // Session ownership is process-lifetime; dropping signals the thread and never joins.
        self.signals.stop();
    }
}

impl LocalTerminalService {
    /// Starts the resume driver for the startup `candidates`; an empty list starts nothing. The
    /// driver keeps Client notes from now until it ends, so it starts before any Client can
    /// connect. The caller holds no registry or coordinator lock.
    pub(crate) fn start_agent_resume(
        self: &Arc<Self>,
        candidates: AgentResumeCandidates,
    ) -> io::Result<()> {
        if candidates.0.is_empty() {
            return Ok(());
        }
        if self
            .agent_resume
            .handle
            .lock()
            .map_err(|_| io::Error::other("Agent resume owner poisoned"))?
            .is_some()
        {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        let initial = candidates
            .0
            .into_iter()
            .map(|candidate| Admitted {
                committed: self.committed_binding(&candidate.presentation),
                candidate,
                retried: None,
            })
            .collect();
        if !self.agent_resume.signals.activate() {
            return Err(io::ErrorKind::NotConnected.into());
        }
        self.spawn_agent_resume(initial)
    }

    /// Hands the pending reference of a Presentation a Retry just started in `directory` to the
    /// driver: behind the running one, or as a new run when none runs. The caller holds the
    /// registry and coordinator locks it committed the new binding under, so notes about the new
    /// terminal start before any Client can reach it. A refusal leaves the reference pending.
    pub(super) fn resume_retried_agent(
        self: &Arc<Self>,
        coordinator: &SessionCoordinator,
        current: Option<WorkerPlacement>,
        presentation: TerminalPresentationId,
        directory: WorkbenchDirectoryPath,
    ) -> io::Result<()> {
        let admitted = Admitted {
            committed: binding(coordinator.agent_resume_target(&presentation, current)),
            candidate: Candidate {
                presentation,
                directory,
            },
            retried: Some(Instant::now()),
        };
        match self.agent_resume.signals.offer(admitted) {
            signals::Offer::Queued => Ok(()),
            signals::Offer::Refused => Err(io::ErrorKind::NotConnected.into()),
            signals::Offer::Start(admitted) => self.spawn_agent_resume(vec![admitted]),
        }
    }

    /// Spawns a run whose note keeping the caller already started. A previous run already ended
    /// its note keeping and takes no lock after that, so joining it returns at once.
    fn spawn_agent_resume(self: &Arc<Self>, initial: Vec<Admitted>) -> io::Result<()> {
        let signals = Arc::clone(&self.agent_resume.signals);
        let Ok(mut slot) = self.agent_resume.handle.lock() else {
            signals.finish();
            return Err(io::Error::other("Agent resume owner poisoned"));
        };
        if let Some(previous) = slot.take()
            && previous.join().is_err()
        {
            eprintln!("orchbee-server diagnostic component=agent-resume outcome=driver-panicked");
        }
        let service = Arc::downgrade(self);
        let thread_signals = Arc::clone(&signals);
        let spawned = thread::Builder::new()
            .name("orchbee-agent-resume".into())
            .spawn(move || run(&service, &thread_signals, initial));
        match spawned {
            Ok(handle) => *slot = Some(handle),
            Err(error) => {
                signals.finish();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Stops the driver and joins it within the Session owner's deadline. A reference not yet
    /// attempted stays pending, so the final checkpoint carries it. Idempotent.
    pub(crate) fn stop_agent_resume_before(&self, deadline: Instant) -> io::Result<()> {
        self.agent_resume.signals.stop();
        loop {
            {
                let mut handle = self
                    .agent_resume
                    .handle
                    .lock()
                    .map_err(|_| io::Error::other("Agent resume handle poisoned"))?;
                if handle.as_ref().is_none_or(JoinHandle::is_finished) {
                    if let Some(handle) = handle.take() {
                        handle
                            .join()
                            .map_err(|_| io::Error::other("Agent resume driver panicked"))?;
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

    /// A Client viewer attached to `target`. The caller holds the coordinator lock.
    pub(super) fn note_client_view(&self, target: BindingViewerTarget) {
        self.agent_resume.signals.note_view(target);
    }

    /// A Client's typed input, paste or send was admitted for `target`. The caller holds the
    /// coordinator lock.
    pub(super) fn note_client_input(&self, target: BindingViewerTarget) {
        self.agent_resume.signals.note_input(target);
    }

    /// Consumes the candidate's reference for `reason`.
    fn consume(&self, candidate: &Candidate, reason: Reason) -> Outcome {
        if let Ok(mut coordinator) = self.coordinator.lock() {
            coordinator.take_pending_agent_session(&candidate.presentation);
        }
        Outcome::Skipped(reason)
    }

    /// Ends the attempt for `reason`: consumes the reference and, when the prompt could not be
    /// confirmed, publishes that nothing was typed on the Presentation still bound as `committed`,
    /// in one Workbench revision.
    fn give_up(
        &self,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
        reason: Reason,
    ) -> Outcome {
        let Some(notice) = reason.notice() else {
            return self.consume(candidate, reason);
        };
        if let Ok(mut coordinator) = self.coordinator.lock() {
            coordinator.give_up_agent_resume(
                &candidate.presentation,
                committed,
                notice,
                unix_milliseconds(),
            );
        }
        Outcome::Skipped(reason)
    }

    /// The Presentation's current binding, if it is still a bound member.
    fn committed_binding(
        &self,
        presentation: &TerminalPresentationId,
    ) -> Option<PresentationBinding> {
        let workers = self.workers.lock().ok()?;
        let coordinator = self.coordinator.lock().ok()?;
        binding(
            coordinator
                .agent_resume_target(presentation, placement(workers.request_handle().as_ref())),
        )
    }

    /// Whether a Client viewed the candidate's current terminal.
    fn viewed(&self, candidate: &Candidate) -> bool {
        let (Ok(workers), Ok(coordinator)) = (self.workers.lock(), self.coordinator.lock()) else {
            return false;
        };
        let current = placement(workers.request_handle().as_ref());
        match coordinator.agent_resume_target(&candidate.presentation, current) {
            ResumeTarget::Started(target) => self.agent_resume.signals.viewed(target.target),
            ResumeTarget::Gone | ResumeTarget::Unavailable(_) => false,
        }
    }

    /// Checks every guard that does not need a fresh read and returns the terminal a delivery may
    /// reach. A failed guard consumes the reference, except an unavailable terminal.
    fn guard(
        &self,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
        restored: Option<BindingViewerTarget>,
    ) -> Result<ProcessObservationTarget, Outcome> {
        let (Ok(workers), Ok(mut coordinator)) = (self.workers.lock(), self.coordinator.lock())
        else {
            return Err(Outcome::Kept(Kept::Unavailable));
        };
        let current = placement(workers.request_handle().as_ref());
        let refuted = self.refutation(&coordinator, candidate, committed, restored, current);
        if let Err(Outcome::Skipped(_)) = refuted {
            coordinator.take_pending_agent_session(&candidate.presentation);
        }
        refuted
    }

    /// The guards themselves, read under the coordinator lock the caller holds: the reference is
    /// still pending, the Presentation keeps its committed binding and restored generation, and no
    /// Client input or shell command has reached that terminal.
    fn refutation(
        &self,
        coordinator: &SessionCoordinator,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
        restored: Option<BindingViewerTarget>,
        current: Option<crate::terminal_coordinator::WorkerPlacement>,
    ) -> Result<ProcessObservationTarget, Outcome> {
        if !coordinator.has_pending_agent_session(&candidate.presentation) {
            return Err(Outcome::Skipped(Reason::Gone));
        }
        let target = match coordinator.agent_resume_target(&candidate.presentation, current) {
            ResumeTarget::Gone => return Err(Outcome::Skipped(Reason::Gone)),
            ResumeTarget::Unavailable(binding) if Some(&binding) == committed => {
                return Err(Outcome::Kept(Kept::Unavailable));
            }
            ResumeTarget::Unavailable(_) => return Err(Outcome::Skipped(Reason::BindingChanged)),
            ResumeTarget::Started(target) => target,
        };
        if Some(&target.binding) != committed
            || restored.is_some_and(|restored| restored != target.target)
        {
            return Err(Outcome::Skipped(Reason::BindingChanged));
        }
        if self.agent_resume.signals.touched(target.target) {
            return Err(Outcome::Skipped(Reason::ClientInput));
        }
        if coordinator.agent_resume_command_observed(&target) {
            return Err(Outcome::Skipped(Reason::CommandObserved));
        }
        Ok(target)
    }

    /// One candidate's whole attempt: guards and a read, then a gated send when the read is worth
    /// it, again and again until the Host accepts, a guard or read refutes, or the deadline.
    fn attempt(
        &self,
        signals: &signals::Signals,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
    ) -> Outcome {
        let deadline = Instant::now() + READINESS;
        let mut restored = None;
        let mut refused = false;
        loop {
            if signals.stopping() {
                return Outcome::Kept(Kept::Stopping);
            }
            let target = match self.guard(candidate, committed, restored) {
                Ok(target) => target,
                Err(outcome) => return outcome,
            };
            restored = Some(target.target);
            let facts = self
                .request_resume_facts(target.clone())
                .unwrap_or_default();
            let mut pause = POLL;
            match readiness(&facts, &candidate.directory) {
                Readiness::Try(shell) => match self.deliver(candidate, committed, &target, shell) {
                    Delivery::Done(outcome) => return outcome,
                    Delivery::Refused(state) => {
                        refused = true;
                        if state == SendPromptState::Settling {
                            pause = SETTLING;
                        }
                    }
                },
                Readiness::Skip(reason) => return self.give_up(candidate, committed, reason),
                Readiness::Wait => {}
            }
            let next = Instant::now() + pause;
            if next >= deadline {
                let reason = if refused {
                    Reason::PromptUnconfirmed
                } else {
                    Reason::NotReady
                };
                return self.give_up(candidate, committed, reason);
            }
            if !signals.sleep_until(next) {
                return Outcome::Kept(Kept::Stopping);
            }
        }
    }

    /// Builds the line, then admits and sends it through the Host's prompt gate in one critical
    /// section: the guards are checked again under the send's own locks, and the reference is
    /// taken before the records leave.
    fn deliver(
        &self,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
        target: &ProcessObservationTarget,
        shell: ResumeShell,
    ) -> Delivery {
        let Some(session) = self
            .coordinator
            .lock()
            .ok()
            .and_then(|coordinator| coordinator.pending_agent_session(&candidate.presentation))
        else {
            return Delivery::Done(Outcome::Skipped(Reason::Gone));
        };
        let token = if shell == ResumeShell::ManagedPowerShell {
            match plan::run_token() {
                Some(token) => Some(token),
                None => return Delivery::Done(self.consume(candidate, Reason::TokenUnavailable)),
            }
        } else {
            None
        };
        let text = plan::text(&session, shell, token.as_deref());
        let request = plan::records(&text, shell.powershell()).and_then(|records| {
            TerminalSendRequest::idle_prompt(
                create_reference()?,
                ObcsSessionId::parse(target.session_id.as_str()).ok()?,
                ClientWorkerId::parse(target.binding.worker_id.as_str()).ok()?,
                ClientTerminalId::parse(target.binding.terminal_id.as_str()).ok()?,
                records,
            )
            .ok()
        });
        let Some(request) = request else {
            return Delivery::Done(self.consume(candidate, Reason::SendFailed));
        };
        let mut admission = None;
        let message = self.deliver_send(&request, |coordinator, sent| {
            let outcome = self.admit(coordinator, candidate, committed, target, *sent, &session);
            let admitted = outcome.is_none();
            admission = Some(outcome);
            admitted
        });
        match admission {
            // The send's own admission refused before the guards: the terminal is unavailable.
            None => Delivery::Done(Outcome::Kept(Kept::Unavailable)),
            Some(Some(outcome)) => Delivery::Done(outcome),
            Some(None) => self.settle(candidate, committed, target, session, &message),
        }
    }

    /// Reads an admitted send's result. Accepted input is sent. A Host refusal that typed nothing
    /// puts the reference back for the next try, and a Host without the gate gives up at once,
    /// since trying again cannot change that. Any other result consumed the reference.
    fn settle(
        &self,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
        target: &ProcessObservationTarget,
        session: RetainedAgentSession,
        message: &ServerMessage,
    ) -> Delivery {
        let ServerMessage::TerminalSendResult(result) = message else {
            return Delivery::Done(Outcome::Skipped(Reason::SendFailed));
        };
        let refusal = match result.outcome() {
            TerminalSendOutcome::Accepted { .. } => return Delivery::Done(Outcome::Sent),
            TerminalSendOutcome::Rejected {
                reason:
                    reason @ (SendRejectedReason::PromptUnconfirmed(_)
                    | SendRejectedReason::HostPromptGateUnsupported),
                progress,
                ..
            } if progress.records_accepted() == 0 && progress.accepted_encoded_bytes() == 0 => {
                *reason
            }
            _ => return Delivery::Done(Outcome::Skipped(Reason::SendFailed)),
        };
        if let Ok(mut coordinator) = self.coordinator.lock() {
            coordinator.return_agent_session(&candidate.presentation, target, session);
        }
        match refusal {
            SendRejectedReason::PromptUnconfirmed(state) => Delivery::Refused(state),
            _ => Delivery::Done(self.give_up(candidate, committed, Reason::TerminalTooOld)),
        }
    }

    /// The final check under the send's locks. Nothing means admitted, with the reference taken
    /// and handed to the coordinator to carry forward.
    fn admit(
        &self,
        coordinator: &mut SessionCoordinator,
        candidate: &Candidate,
        committed: Option<&PresentationBinding>,
        target: &ProcessObservationTarget,
        sent: BindingViewerTarget,
        session: &RetainedAgentSession,
    ) -> Option<Outcome> {
        let refusal = match self.refutation(
            coordinator,
            candidate,
            committed,
            Some(target.target),
            Some(sent.worker),
        ) {
            Ok(current) if current.target == sent => None,
            Ok(_) => Some(Outcome::Skipped(Reason::BindingChanged)),
            Err(outcome) => Some(outcome),
        };
        match refusal {
            Some(Outcome::Kept(kept)) => Some(Outcome::Kept(kept)),
            Some(outcome) => {
                coordinator.take_pending_agent_session(&candidate.presentation);
                Some(outcome)
            }
            None => match coordinator.take_pending_agent_session(&candidate.presentation) {
                Some(taken) if taken == *session => {
                    let typed = crate::session_coordinator::resume_arguments(session);
                    coordinator.carry_agent_session(target, taken, typed);
                    None
                }
                _ => Some(Outcome::Skipped(Reason::Gone)),
            },
        }
    }
}

/// The binding a resume target names, if its Presentation is still a bound member.
fn binding(target: ResumeTarget) -> Option<PresentationBinding> {
    match target {
        ResumeTarget::Gone => None,
        ResumeTarget::Unavailable(binding) => Some(binding),
        ResumeTarget::Started(target) => Some(target.binding),
    }
}

/// Ends note keeping however the driver returns early; the retried candidates it leaves stay
/// pending. Disarmed when the run ended by finding no retried candidate, so a run a later Retry
/// already started keeps its notes.
struct Finish<'a> {
    signals: &'a signals::Signals,
    armed: bool,
}
impl Drop for Finish<'_> {
    fn drop(&mut self) {
        if self.armed {
            for _ in 0..self.signals.finish() {
                report(Outcome::Kept(Kept::Stopping));
            }
        }
    }
}

/// Now in Unix milliseconds, when the clock reads within the range the Workbench protocol carries.
fn unix_milliseconds() -> Option<u64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    u64::try_from(elapsed.as_millis())
        .ok()
        .filter(|milliseconds| (1..=MAXIMUM_UNIX_MILLISECONDS).contains(milliseconds))
}

fn report(outcome: Outcome) {
    eprintln!(
        "orchbee-server diagnostic component=agent-resume outcome={}",
        outcome.kind()
    );
}

fn run(service: &Weak<LocalTerminalService>, signals: &signals::Signals, initial: Vec<Admitted>) {
    let mut finish = Finish {
        signals,
        armed: true,
    };
    let keep = |rest: usize| {
        for _ in 0..rest {
            report(Outcome::Kept(Kept::Stopping));
        }
    };
    if !signals.wait_for_view(SETTLE) {
        return keep(initial.len());
    }
    let Some(viewed) = service.upgrade().map(|service| {
        initial
            .iter()
            .map(|admitted| service.viewed(&admitted.candidate))
            .collect::<Vec<_>>()
    }) else {
        return keep(initial.len());
    };
    let mut ordered: Vec<_> = viewed.into_iter().zip(initial).collect();
    ordered.sort_by_key(|(viewed, _)| !viewed);
    let mut queue: VecDeque<Admitted> = ordered.into_iter().map(|(_, admitted)| admitted).collect();
    let mut last_send: Option<Instant> = None;
    loop {
        let Some(admitted) = queue.pop_front().or_else(|| signals.next_retried()) else {
            finish.armed = false;
            return;
        };
        let settled = admitted
            .retried
            .is_none_or(|retried| signals.sleep_until(retried + SETTLE));
        let spaced = settled && last_send.is_none_or(|last| signals.sleep_until(last + SPACING));
        let Some(service) = spaced.then(|| service.upgrade()).flatten() else {
            return keep(1 + queue.len());
        };
        let outcome = service.attempt(signals, &admitted.candidate, admitted.committed.as_ref());
        drop(service);
        if outcome.sent() {
            last_send = Some(Instant::now());
        }
        report(outcome);
    }
}
