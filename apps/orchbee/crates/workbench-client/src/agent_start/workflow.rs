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
    AgentHostEpoch, AgentHostTerminalId, AgentLaunchOutcome, AgentLaunchRequest,
    AgentLaunchReservation, AgentLifecycle, AgentObservationFreshness, AgentPresentationId,
    AgentSafeInteger, AgentSemanticStatus, AgentSessionId, ObapRequestId,
};
use orchbee_local_ipc::LocalIo;
use orchbee_terminal_protocol::{TerminalProcess, TerminalProcessInfo};
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
        let reservation = match outcome {
            AgentLaunchOutcome::Reserved { reservation }
                if reservation.reservation_id == reservation_id
                    && reservation.name == self.request.name
                    && reservation.provider_id == self.request.provider
                    && reservation.binding.presentation_id == presentation
                    && reservation.binding.host_epoch == epoch
                    && reservation.binding.host_terminal_id == physical =>
            {
                reservation
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
        let concise = (self.progress.is_some() && self.request.args.is_empty())
            .then(|| super::command::interactive(self.request.provider.as_str(), powershell))
            .flatten();
        let text = concise.as_deref().unwrap_or(text);
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
        self.observe(client, terminal, &reservation, submitted, wake)
    }

    fn observe(
        &mut self,
        client: &mut LocalClient,
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
            let (pending, candidate) = match outcome {
                AgentLaunchOutcome::Pending {
                    reservation: pending,
                    candidate_session_id,
                } if same_reservation(reservation, &pending) => (pending, candidate_session_id),
                other => return Err(refusal(other)),
            };
            self.last_observed = Some(pending.clone());
            let process = self.sample(client, terminal, wake)?;
            if process.epoch.to_text() != reservation.binding.host_epoch.as_str()
                || process.info.terminal_id.to_text()
                    != reservation.binding.host_terminal_id.as_str()
                || terminal.presentation.as_str() != reservation.binding.presentation_id.as_str()
            {
                return Err(Failure::Start(StartFailure::BindingChanged));
            }
            let runtime = agent_start_runtime::interpret(&process.info);
            match runtime.foreground.selection {
                agent_start_runtime::Selection::Ambiguous => {
                    return Err(Failure::Start(StartFailure::RuntimeAmbiguous));
                }
                agent_start_runtime::Selection::AgentChain
                    if runtime.foreground.provider != Some(self.request.provider.as_str()) =>
                {
                    return Err(Failure::Start(StartFailure::ProviderMismatch));
                }
                agent_start_runtime::Selection::Unavailable => {
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
                    && runtime.foreground.provider == Some(self.request.provider.as_str())
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

fn idle_shell(sample: &TerminalProcessInfo) -> Result<&TerminalProcess, Failure> {
    let refuse = || Failure::Start(StartFailure::TerminalNotIdle);
    if sample.processes.len() >= orchbee_terminal_protocol::MAXIMUM_PROCESS_SAMPLE_ROWS {
        return Err(refuse());
    }
    let interpreted = agent_start_runtime::interpret(sample);
    let root = interpreted
        .processes
        .iter()
        .find(|p| p.facts.pid == sample.root_pid)
        .ok_or_else(refuse)?;
    let image = root.facts.image.as_deref().ok_or_else(refuse)?;
    let shell = crate::agent_provider::basename(image);
    if !matches!(shell.as_str(), "cmd" | "powershell" | "pwsh") {
        return Err(refuse());
    }
    let args = root.argv.as_deref().ok_or_else(refuse)?;
    if args
        .first()
        .is_none_or(|first| crate::agent_provider::basename(first) != shell)
    {
        return Err(refuse());
    }
    let switches = &args[1..];
    // The worker's fixed initialization finishes before the interactive prompt. Arbitrary
    // command bodies and noninteractive launches still cannot establish an idle shell.
    let runtime_initialization = shell != "cmd"
        && switches.last().is_some_and(|script| {
            script == &orchbee_local_runtime::windows_interactive_powershell_initialization()
        });
    let flags = if switches.len() >= 3
        && runtime_initialization
        && switches[switches.len() - 2].eq_ignore_ascii_case("-Command")
        && switches[..switches.len() - 2]
            .iter()
            .any(|flag| flag.eq_ignore_ascii_case("-NoExit"))
    {
        &switches[..switches.len() - 2]
    } else {
        switches
    };
    for arg in flags {
        let arg = arg.to_ascii_lowercase();
        if if shell == "cmd" {
            !matches!(
                arg.as_str(),
                "/d" | "/q" | "/a" | "/u" | "/v:on" | "/v:off" | "/e:on" | "/e:off"
            )
        } else {
            !matches!(arg.as_str(), "-nologo" | "-noprofile" | "-noexit")
        } {
            return Err(refuse());
        }
    }
    if interpreted.processes.iter().any(|p| {
        p.facts.pid != sample.root_pid
            && p.facts.image.as_deref().is_none_or(|image| {
                !matches!(
                    crate::agent_provider::basename(image).as_str(),
                    "conhost" | "openconsole"
                )
            })
    }) {
        return Err(refuse());
    }
    sample
        .processes
        .iter()
        .find(|p| p.pid == sample.root_pid)
        .ok_or_else(refuse)
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
