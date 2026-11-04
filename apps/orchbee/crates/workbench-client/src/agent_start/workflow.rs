//! Agent startup workflow. One absolute budget covers admission, reservation,
//! the single send, observation and best-effort name release.

use super::failure::ExchangeFailure;
use super::{DeliveryDisposition, Report, Request, ReservationState, Verdict, elapsed_ms};
use super::{Failure, StartFailure, Submission};
use crate::agent_runtime::startup as agent_start_runtime;
use crate::agent_runtime::startup::binding::PrimaryTerminal;
use crate::local_client::{
    LocalClient,
    agent::{AgentExchangeError, AgentExchangeFailure},
};
use orchbee_agent_observation_protocol::{
    AgentHostEpoch, AgentHostTerminalId, AgentLaunchOutcome, AgentLaunchReceipt,
    AgentLaunchRequest, AgentLaunchReservation, AgentLifecycle, AgentObservationFreshness,
    AgentPresentationId, AgentSafeInteger, AgentSemanticStatus, AgentSessionId, ObapRequestId,
    ShellRoot, classify_shell_root, is_console_host,
};
use orchbee_local_ipc::LocalIo;
use orchbee_terminal_protocol::{TerminalProcess, TerminalProcessConsole, TerminalProcessInfo};
use orchbee_workbench_protocol::{Snapshot, WorkbenchPanelId};
use std::sync::Arc;
use std::time::{Duration, Instant};

const READ_BUDGET: Duration = Duration::from_secs(5);
const SETTLE: Duration = Duration::from_millis(3000);

#[cfg(test)]
mod tests;

struct Workflow<'a> {
    progress: Option<&'a dyn Fn(super::Progress)>,
    request: &'a Request,
    started: Instant,
    deadline: Instant,
    reservation: Option<ObapRequestId>,
    last_observed: Option<AgentLaunchReservation>,
    state: ReservationState,
    submission: Option<Submission>,
    delivery: DeliveryDisposition,
    last_read_failure: Option<Box<Failure>>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_on_origin(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
    origin: &crate::local_client::LocalClientOrigin,
    clock: crate::local_client::LocalOperationClock,
    panel: orchbee_workbench_protocol::WorkbenchPanelId,
    progress: &dyn Fn(super::Progress),
) -> Report {
    let admitted = step_deadline(deadline).and_then(|step| {
        agent_start_runtime::admit().map_err(|_| Failure::Unsupported)?;
        agent_start_runtime::snapshot_from_origin(origin, clock, step, wake)
            .map_err(|error| Failure::Admission(Box::new(error)))
    });
    match settle_cancellation(admitted, wake) {
        Ok((client, snapshot)) => run_admitted(
            &request,
            started,
            deadline,
            wake,
            client,
            snapshot,
            panel,
            Some(progress),
        ),
        Err(failure) => Report::preflight(request, started, failure),
    }
}

/// Cap one startup exchange at five seconds without extending the caller's absolute budget.
///
/// # Errors
/// Returns a deadline failure when no part of the original budget remains.
pub fn step_deadline(deadline: Instant) -> Result<Instant, Failure> {
    let now = Instant::now();
    if now >= deadline {
        return Err(Failure::Start(StartFailure::Deadline));
    }
    Ok(deadline.min(now + READ_BUDGET))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_admitted(
    request: &Request,
    started: Instant,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
    mut client: LocalClient,
    snapshot: Snapshot,
    panel: WorkbenchPanelId,
    progress: Option<&dyn Fn(super::Progress)>,
) -> Report {
    let mut workflow = Workflow {
        progress,
        request,
        started,
        deadline,
        reservation: None,
        last_observed: None,
        state: ReservationState::None,
        submission: None,
        delivery: DeliveryDisposition::NotApplied,
        last_read_failure: None,
    };
    let result = match super::command::launch(request.provider.as_str(), &request.args) {
        Some(text) => workflow.span(&mut client, &snapshot, &panel, &text, wake),
        None => Err(Failure::Start(StartFailure::CommandTooLong)),
    };
    let result = settle_cancellation(result, wake);
    drop(client);
    workflow.finish(result)
}

fn settle_cancellation<T>(
    result: Result<T, Failure>,
    wake: Option<LocalIo<'_>>,
) -> Result<T, Failure> {
    match result {
        Err(failure)
            if !failure.unknown()
                && agent_start_runtime::wait_backoff(wake, Duration::from_millis(1)) =>
        {
            Err(Failure::Start(StartFailure::Cancelled))
        }
        result => result,
    }
}

impl Workflow<'_> {
    fn step(&self) -> Result<Instant, Failure> {
        let now = Instant::now();
        if now >= self.deadline {
            return Err(Failure::Start(StartFailure::Deadline));
        }
        Ok(self.deadline.min(now + READ_BUDGET))
    }

    fn span(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        panel: &WorkbenchPanelId,
        text: &str,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentSessionId, Failure> {
        let terminal =
            agent_start_runtime::binding::primary(snapshot, panel).map_err(Failure::Primary)?;
        let answer = self.launch_on_session(client, snapshot, &terminal, text, wake);
        if answer.is_err() && self.state == ReservationState::Pending {
            self.release(client);
        }
        answer
    }

    fn launch_on_session(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        terminal: &PrimaryTerminal,
        text: &str,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentSessionId, Failure> {
        let sample = self.sample(client, terminal, wake)?;
        let shell = idle_shell(&sample.info)?;
        let before = shell.clone();
        let epoch = AgentHostEpoch::parse(&sample.epoch.to_text()).map_err(|_| invalid())?;
        let physical = AgentHostTerminalId::parse(&sample.info.terminal_id.to_text())
            .map_err(|_| invalid())?;
        let presentation =
            AgentPresentationId::parse(terminal.presentation.as_str()).map_err(|_| invalid())?;
        let ttl = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if ttl <= 3000 {
            return Err(Failure::Start(StartFailure::Deadline));
        }
        interrupted(wake)?;
        let reservation_id = agent_start_runtime::obap_identity().map_err(|_| Failure::Identity)?;
        self.reservation = Some(reservation_id.clone());
        // Once a mutating exchange starts, only its correlated reply can settle
        // this knowledge. A lost reply is never followed by another reserve.
        self.state = ReservationState::Unknown;
        let outcome = exchange(
            client,
            AgentLaunchRequest::Reserve {
                request_id: reservation_id.clone(),
                name: self.request.name.clone(),
                provider_id: self.request.provider.clone(),
                presentation_id: presentation.clone(),
                host_epoch: epoch.clone(),
                host_terminal_id: physical.clone(),
                ttl_ms: AgentSafeInteger::new(u64::try_from(ttl).map_err(|_| invalid())?),
            },
            self.step()?,
            wake,
        )?;
        let (reservation, run_token) = match outcome {
            AgentLaunchOutcome::Reserved {
                reservation,
                run_token,
            } if reservation.reservation_id == reservation_id
                && reservation.name == self.request.name
                && reservation.provider_id == self.request.provider
                && reservation.binding.presentation_id == presentation
                && reservation.binding.host_epoch == epoch
                && reservation.binding.host_terminal_id == physical =>
            {
                (reservation, run_token)
            }
            outcome @ (AgentLaunchOutcome::Rejected { .. }
            | AgentLaunchOutcome::Unavailable { .. }) => {
                self.state = ReservationState::None;
                return Err(refusal(outcome));
            }
            // This Agent launch phase consumes its corresponding receipt; other outcomes cannot establish that phase.
            _ => return Err(Failure::Start(StartFailure::ReplyNotConfirmed)),
        };
        self.state = ReservationState::Pending;
        self.last_observed = Some(reservation.clone());
        let latest = self.sample(client, terminal, wake)?;
        let latest_shell = idle_shell(&latest.info)?;
        // Both observations use the same frozen presentation binding. Recheck
        // the live Host incarnation and shell process before the one input send.
        if latest.epoch != sample.epoch
            || latest.info.terminal_id != sample.info.terminal_id
            || latest_shell.pid != before.pid
            || latest_shell.creation_time != before.creation_time
            || latest_shell.image != before.image
            || latest_shell.command_line != before.command_line
        {
            return Err(Failure::Start(StartFailure::BindingChanged));
        }
        interrupted(wake)?;
        let powershell = before.image.as_deref().is_some_and(|image| {
            matches!(
                crate::agent_provider::basename(image).as_str(),
                "powershell" | "pwsh"
            )
        });
        // Without arguments no literal argv wrapper is needed; the short form keeps the
        // shell's echo to one line for the board and the CLI alike.
        let concise = self
            .request
            .args
            .is_empty()
            .then(|| super::command::interactive(self.request.provider.as_str(), powershell))
            .flatten();
        let text = concise.as_deref().unwrap_or(text);
        // Managed PowerShell strips this exact comment prefix, echoes the run token in its
        // start and finish markers, and exports it to this command only. Any other PowerShell
        // reads the prefix as a block comment and runs the command unchanged.
        let receipted;
        let text = if managed_powershell(&sample.info) {
            receipted = format!("<#orchbee-run:{run_token}#> {text}");
            receipted.as_str()
        } else {
            text
        };
        let receipt = match agent_start_runtime::send::submit_prompt(
            client,
            &terminal.binding,
            snapshot.revision,
            text,
            powershell,
            self.step()?,
            wake,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                let failure = Arc::new(error);
                let failed = Failure::Send(Arc::clone(&failure));
                self.delivery = if failed.unknown() {
                    DeliveryDisposition::Unknown
                } else {
                    DeliveryDisposition::NotApplied
                };
                self.submission = Some(Submission::Failed(failure));
                return Err(failed);
            }
        };
        let submitted = Instant::now();
        self.delivery = delivery_disposition(receipt.verdict());
        // Preserve the actual input receipt independently of subsequent readiness.
        let refusal = receipt.refusal();
        self.submission = Some(Submission::Sent(Box::new(receipt)));
        if let Some(reason) = refusal {
            return Err(Failure::Start(StartFailure::InputNotAccepted(reason)));
        }
        if let Some(progress) = self.progress {
            progress(super::Progress::Observing);
        }
        self.observe(client, snapshot, terminal, &reservation, submitted, wake)
    }

    fn observe(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        terminal: &PrimaryTerminal,
        reservation: &AgentLaunchReservation,
        submitted: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentSessionId, Failure> {
        loop {
            interrupted(wake)?;
            let outcome = exchange(
                client,
                AgentLaunchRequest::Inspect {
                    request_id: agent_start_runtime::obap_identity()
                        .map_err(|_| Failure::Identity)?,
                    reservation_id: reservation.reservation_id.clone(),
                },
                self.step()?,
                wake,
            )
            .map_err(|failure| self.read_failure(failure))?;
            let (pending, candidate, receipt) = match outcome {
                AgentLaunchOutcome::Pending {
                    reservation: pending,
                    candidate_session_id,
                    receipt,
                } if same_reservation(reservation, &pending) => {
                    (pending, candidate_session_id, receipt)
                }
                other => return Err(refusal(other)),
            };
            self.last_observed = Some(pending.clone());
            // The launched command already returned to the managed PowerShell prompt.
            if let AgentLaunchReceipt::Finished { succeeded } = receipt {
                return Err(Failure::Start(if succeeded {
                    StartFailure::AgentExited
                } else {
                    StartFailure::LaunchFailed
                }));
            }
            let process = self.sample(client, terminal, wake)?;
            if process.epoch.to_text() != reservation.binding.host_epoch.as_str()
                || process.info.terminal_id.to_text()
                    != reservation.binding.host_terminal_id.as_str()
                || terminal.presentation.as_str() != reservation.binding.presentation_id.as_str()
            {
                return Err(Failure::Start(StartFailure::BindingChanged));
            }
            let identified = agent_start_runtime::identity::read(
                client,
                snapshot,
                &terminal.presentation,
                reservation.binding.host_terminal_id.as_str(),
                self.step()?,
                wake,
            );
            // The Server's run is the only foreground; without its answer this step selects none.
            let foreground =
                identified.map(|server| agent_start_runtime::foreground(&process.info, server));
            match foreground.map(|foreground| (foreground.selection, foreground.provider)) {
                Some((agent_start_runtime::Selection::Ambiguous, _)) => {
                    return Err(Failure::Start(StartFailure::RuntimeAmbiguous));
                }
                Some((agent_start_runtime::Selection::AgentChain, provider))
                    if provider != Some(self.request.provider.as_str()) =>
                {
                    // The Server settled another provider in this Terminal generation.
                    return Err(Failure::Start(StartFailure::ProviderMismatch));
                }
                Some((agent_start_runtime::Selection::Unavailable, _)) => {
                    return Err(Failure::Start(StartFailure::AgentExited));
                }
                _ => {}
            }
            if submitted.elapsed() >= SETTLE && idle_shell(&process.info).is_ok() {
                return Err(Failure::Start(StartFailure::AgentExited));
            }
            if let Some(session_id) = candidate {
                let observed =
                    agent_start_runtime::observation::read(client, &session_id, self.step()?, wake)
                        .map_err(|error| {
                            self.read_failure(Failure::Observation(Box::new(error)))
                        })?;
                let detail = &observed.session;
                let pinned = detail.snapshot.provider_id == self.request.provider
                    && detail.snapshot.lifecycle == AgentLifecycle::Live
                    && detail
                        .snapshot
                        .presentation_attachments
                        .iter()
                        .any(|attachment| {
                            attachment.navigable
                                && attachment.presentation_id == reservation.binding.presentation_id
                                && attachment.generation == reservation.binding.generation
                        });
                if !pinned {
                    return Err(Failure::Start(StartFailure::BindingChanged));
                }
                let waiting = !detail.snapshot.pending_requests.is_empty()
                    || matches!(detail.status, AgentSemanticStatus::NeedsAttention { .. });
                if let Some(progress) = self.progress {
                    progress(if waiting {
                        super::Progress::WaitingInput
                    } else {
                        super::Progress::Observing
                    });
                } else if waiting {
                    return Err(Failure::Start(StartFailure::AgentBlocked));
                }
                let ready = !waiting
                    && detail.snapshot.observation.freshness == AgentObservationFreshness::Fresh
                    && matches!(
                        detail.status,
                        AgentSemanticStatus::Idle { .. }
                            | AgentSemanticStatus::Succeeded { .. }
                            | AgentSemanticStatus::Failed { .. }
                            | AgentSemanticStatus::Canceled { .. }
                            | AgentSemanticStatus::Interrupted { .. }
                    );
                if ready
                    && foreground.is_some_and(|foreground| {
                        foreground.selection == agent_start_runtime::Selection::AgentChain
                            && foreground.provider == Some(self.request.provider.as_str())
                    })
                    && submitted.elapsed() >= SETTLE
                    && pending.settle_remaining_ms.value() == 0
                {
                    interrupted(wake)?;
                    self.state = ReservationState::Unknown;
                    let outcome = exchange(
                        client,
                        AgentLaunchRequest::Complete {
                            request_id: agent_start_runtime::obap_identity()
                                .map_err(|_| Failure::Identity)?,
                            reservation_id: reservation.reservation_id.clone(),
                            session_id: session_id.clone(),
                        },
                        self.step()?,
                        wake,
                    )?;
                    match outcome {
                        AgentLaunchOutcome::Completed {
                            reservation_id,
                            name,
                            session_id: confirmed,
                        } if reservation_id == reservation.reservation_id
                            && name == self.request.name
                            && confirmed == session_id =>
                        {
                            self.state = ReservationState::Completed;
                            return Ok(session_id);
                        }
                        AgentLaunchOutcome::Rejected {
                            reason:
                                orchbee_agent_observation_protocol::AgentLaunchRejectionReason::NotReady,
                        } => self.state = ReservationState::Pending,
                        other @ (AgentLaunchOutcome::Rejected { .. }
                        | AgentLaunchOutcome::Unavailable { .. }) => {
                            self.state = ReservationState::Pending;
                            return Err(refusal(other));
                        }
                        // This Agent launch phase consumes its corresponding receipt; other outcomes cannot establish that phase.
                        _ => return Err(Failure::Start(StartFailure::ReplyNotConfirmed)),
                    }
                }
            }
            let delay = Duration::from_millis(100)
                .min(self.deadline.saturating_duration_since(Instant::now()));
            if agent_start_runtime::wait_backoff(wake, delay) {
                return Err(Failure::Start(StartFailure::Cancelled));
            }
        }
    }

    fn release(&mut self, client: &mut LocalClient) {
        let Some(reservation_id) = self.reservation.clone() else {
            return;
        };
        let Ok(deadline) = self.step() else {
            return;
        };
        let Ok(request_id) = agent_start_runtime::obap_identity() else {
            return;
        };
        self.state = ReservationState::Unknown;
        // A signal has already stopped startup. Give the name-release exchange
        // at most one second of the ORIGINAL budget, without its latched wake.
        let outcome = exchange(
            client,
            AgentLaunchRequest::Cancel {
                request_id,
                reservation_id: reservation_id.clone(),
            },
            deadline.min(Instant::now() + Duration::from_secs(1)),
            None,
        );
        match outcome {
            Ok(AgentLaunchOutcome::Released {
                reservation_id: confirmed,
            }) if confirmed == reservation_id => self.state = ReservationState::Released,
            Ok(AgentLaunchOutcome::Rejected {
                reason:
                    orchbee_agent_observation_protocol::AgentLaunchRejectionReason::ReservationNotFound,
            }) => self.state = ReservationState::Released,
            // This Agent launch phase consumes its corresponding receipt; other outcomes cannot establish that phase.
            _ => {}
        }
    }

    fn sample(
        &mut self,
        client: &mut LocalClient,
        terminal: &PrimaryTerminal,
        wake: Option<LocalIo<'_>>,
    ) -> Result<agent_start_runtime::processes::Observation, Failure> {
        loop {
            interrupted(wake)?;
            let result =
                agent_start_runtime::processes::read(client, self.step()?, &terminal.binding, wake)
                    .map_err(|error| Failure::Process(Box::new(error)));
            match result {
                Err(failure) if failure.transient_sample() => {
                    self.last_read_failure = Some(Box::new(failure));
                    let delay = Duration::from_millis(50)
                        .min(self.deadline.saturating_duration_since(Instant::now()));
                    if agent_start_runtime::wait_backoff(wake, delay) {
                        return Err(Failure::Start(StartFailure::Cancelled));
                    }
                }
                other => {
                    if other.is_ok() {
                        self.last_read_failure = None;
                    }
                    return other.map_err(|failure| self.read_failure(failure));
                }
            }
        }
    }

    fn read_failure(&mut self, failure: Failure) -> Failure {
        // Read transports may report a closed pipe when their deadline fires.
        // The startup deadline governs these observations, but never rewrites
        // the uncertainty of a mutation or an input receipt.
        if Instant::now() >= self.deadline && (failure.transport_lost() || failure.timed_out()) {
            self.last_read_failure = Some(Box::new(failure));
            Failure::Start(StartFailure::Deadline)
        } else {
            failure
        }
    }

    fn finish(self, result: Result<AgentSessionId, Failure>) -> Report {
        let verdict = match result {
            Ok(session) => Verdict::Started { session },
            Err(failure) => Verdict::Failed {
                failure,
                read_cause: self.last_read_failure,
            },
        };
        Report {
            name: self.request.name.clone(),
            provider: self.request.provider.clone(),
            reservation: self.reservation,
            last_observed: self.last_observed,
            reservation_state: self.state,
            submission: self.submission,
            delivery: self.delivery,
            elapsed_ms: elapsed_ms(self.started),
            verdict,
        }
    }
}

const fn delivery_disposition(verdict: agent_start_runtime::SendVerdict) -> DeliveryDisposition {
    match verdict {
        agent_start_runtime::SendVerdict::Quiet
        | agent_start_runtime::SendVerdict::AcceptedCleanupUnconfirmed => {
            DeliveryDisposition::Applied
        }
        agent_start_runtime::SendVerdict::PartialAccepted => DeliveryDisposition::PartiallyApplied,
        agent_start_runtime::SendVerdict::OutcomeUnknown => DeliveryDisposition::Unknown,
        agent_start_runtime::SendVerdict::Rejected
        | agent_start_runtime::SendVerdict::Unavailable => DeliveryDisposition::NotApplied,
    }
}

fn same_reservation(a: &AgentLaunchReservation, b: &AgentLaunchReservation) -> bool {
    a.reservation_id == b.reservation_id
        && a.name == b.name
        && a.provider_id == b.provider_id
        && a.binding == b.binding
}

/// What one process sample proves about the foreground of a terminal a close would end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CloseSample {
    /// A recognized interactive shell, and nothing else holds or may hold its console.
    ShellOnly,
    /// Another program holds the console, or the root is not an interactive shell.
    Program,
    /// The sample cannot tell: it reached its row limit, its root is unreadable, or a row's console
    /// is not observable.
    Unknown,
}

/// Reads a sample the way the Host's close gate reads its process observation: only a member that
/// holds the Terminal console is a foreground program. A detached or console-less member does not
/// hold it, unlike the stricter rule that admits an Agent start.
pub(crate) fn close_sample(sample: &TerminalProcessInfo) -> CloseSample {
    if sample.processes.len() >= orchbee_terminal_protocol::MAXIMUM_PROCESS_SAMPLE_ROWS {
        return CloseSample::Unknown;
    }
    match root_shell(sample, &agent_start_runtime::interpret(sample)) {
        ShellRoot::Interactive => {}
        ShellRoot::Other => return CloseSample::Program,
        ShellRoot::Unreadable => return CloseSample::Unknown,
    }
    let mut verdict = CloseSample::ShellOnly;
    for row in &sample.processes {
        if row.pid == sample.root_pid || row.image.as_deref().is_some_and(is_console_host) {
            continue;
        }
        match row.console {
            TerminalProcessConsole::Attached => return CloseSample::Program,
            TerminalProcessConsole::Unknown => verdict = CloseSample::Unknown,
            TerminalProcessConsole::Detached | TerminalProcessConsole::None => {}
        }
    }
    verdict
}

/// How the sample's root reads as an interactive shell, by the rule every close and start owner shares.
fn root_shell(
    sample: &TerminalProcessInfo,
    interpreted: &[crate::agent_runtime::processes::Process<'_>],
) -> ShellRoot {
    interpreted
        .iter()
        .find(|p| p.facts.pid == sample.root_pid)
        .map_or(ShellRoot::Unreadable, |root| {
            classify_shell_root(
                root.facts.image.as_deref(),
                root.argv.as_deref(),
                &orchbee_local_runtime::windows_interactive_powershell_initialization(),
            )
        })
}

fn idle_shell(sample: &TerminalProcessInfo) -> Result<&TerminalProcess, Failure> {
    let refuse = || Failure::Start(StartFailure::TerminalNotIdle);
    if sample.processes.len() >= orchbee_terminal_protocol::MAXIMUM_PROCESS_SAMPLE_ROWS {
        return Err(refuse());
    }
    let interpreted = agent_start_runtime::interpret(sample);
    if !matches!(root_shell(sample, &interpreted), ShellRoot::Interactive) {
        return Err(refuse());
    }
    if interpreted.iter().any(|p| {
        p.facts.pid != sample.root_pid
            && p.facts
                .image
                .as_deref()
                .is_none_or(|image| !is_console_host(image))
    }) {
        return Err(refuse());
    }
    sample
        .processes
        .iter()
        .find(|p| p.pid == sample.root_pid)
        .ok_or_else(refuse)
}

/// The shell runs the Worker's fixed PowerShell initialization, which carries the run markers.
fn managed_powershell(sample: &TerminalProcessInfo) -> bool {
    agent_start_runtime::interpret(sample)
        .iter()
        .find(|p| p.facts.pid == sample.root_pid)
        .is_some_and(|root| {
            root.facts.image.as_deref().is_some_and(|image| {
                matches!(
                    crate::agent_provider::basename(image).as_str(),
                    "powershell" | "pwsh"
                )
            }) && root
                .argv
                .as_deref()
                .and_then(<[String]>::last)
                .is_some_and(|script| {
                    script
                        == &orchbee_local_runtime::windows_interactive_powershell_initialization()
                })
        })
}

fn interrupted(wake: Option<LocalIo<'_>>) -> Result<(), Failure> {
    if agent_start_runtime::wait_backoff(wake, Duration::from_millis(1)) {
        Err(Failure::Start(StartFailure::Cancelled))
    } else {
        Ok(())
    }
}
fn invalid() -> Failure {
    Failure::Start(StartFailure::InvalidProtocol)
}

fn refusal(outcome: AgentLaunchOutcome) -> Failure {
    match outcome {
        AgentLaunchOutcome::Rejected { reason } => Failure::Start(StartFailure::Rejected(reason)),
        AgentLaunchOutcome::Unavailable { .. } => Failure::Start(StartFailure::RuntimeUnavailable),
        AgentLaunchOutcome::Status { .. }
        | AgentLaunchOutcome::Reserved { .. }
        | AgentLaunchOutcome::Pending { .. }
        | AgentLaunchOutcome::Completed { .. }
        | AgentLaunchOutcome::Released { .. } => invalid(),
    }
}
fn exchange(
    client: &mut LocalClient,
    request: AgentLaunchRequest,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
) -> Result<AgentLaunchOutcome, Failure> {
    let mutating = !matches!(request, AgentLaunchRequest::Inspect { .. });
    client
        .submit_agent_launch_before_cancellable(request, deadline, wake)
        .map(|result| result.outcome().clone())
        .map_err(|failure| match failure {
            AgentExchangeError::Failed { .. } if mutating => {
                Failure::Start(StartFailure::ReplyNotConfirmed)
            }
            AgentExchangeError::Failed {
                reason:
                    AgentExchangeFailure::Write(io)
                    | AgentExchangeFailure::ResponseDeadline(io)
                    | AgentExchangeFailure::Response(
                        orchbee_agent_observation_protocol::AgentWireError::Io(io),
                    ),
                ..
            }
            | AgentExchangeError::RequestNotSubmitted(io) => Failure::Start(
                StartFailure::Exchange(if io.kind() == std::io::ErrorKind::TimedOut {
                    ExchangeFailure::Timeout
                } else {
                    ExchangeFailure::Transport
                }),
            ),
            AgentExchangeError::Failed { .. } => invalid(),
            // Only the explicitly recognized protocol failures have a more specific transport classification here.
            _ => Failure::Start(StartFailure::Exchange(ExchangeFailure::Internal)),
        })
}
