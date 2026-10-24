//! Windows runtime workflow. One absolute budget covers admission, reservation,
//! the single send, observation and best-effort name release.

use super::{DeliveryDisposition, Report, Request, ReservationState, Verdict, elapsed_ms};
use crate::cli::{Failure, FailureKind, agent_start_runtime};
use crate::invocation::Target;
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
use orchbee_workbench_protocol::Snapshot;
use serde_json::{Value, json};
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
    submission: Option<Value>,
    submission_status: Option<u8>,
    delivery: DeliveryDisposition,
    last_read_failure: Option<Value>,
}

pub(super) fn run_local(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
    resolve_panel: impl FnOnce(&Snapshot) -> Result<Target, Failure>,
) -> Report {
    let admitted = step_deadline(deadline)
        .and_then(|step| agent_start_runtime::stream_snapshot(step, wake))
        .and_then(|(client, snapshot)| {
            let panel = resolve_panel(&snapshot)?;
            Ok((client, snapshot, panel))
        });
    match settle_cancellation(admitted, wake) {
        Ok((client, snapshot, panel)) => run_admitted(
            &request, started, deadline, wake, client, snapshot, panel, None,
        ),
        Err(failure) => Report::preflight(request, started, failure),
    }
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
    let admitted = step_deadline(deadline)
        .and_then(|step| agent_start_runtime::snapshot_from_origin(origin, clock, step, wake));
    match settle_cancellation(admitted, wake) {
        Ok((client, snapshot)) => {
            let panel = Target::Id(panel.as_str().to_owned());
            run_admitted(
                &request,
                started,
                deadline,
                wake,
                client,
                snapshot,
                panel,
                Some(progress),
            )
        }
        Err(failure) => Report::preflight(request, started, failure),
    }
}

fn step_deadline(deadline: Instant) -> Result<Instant, Failure> {
    let now = Instant::now();
    if now >= deadline {
        return Err(error(FailureKind::Timeout, "start-deadline"));
    }
    Ok(deadline.min(now + READ_BUDGET))
}

#[allow(clippy::too_many_arguments)]
fn run_admitted(
    request: &Request,
    started: Instant,
    deadline: Instant,
    wake: Option<LocalIo<'_>>,
    mut client: LocalClient,
    snapshot: Snapshot,
    panel: Target,
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
        submission_status: None,
        delivery: DeliveryDisposition::NotApplied,
        last_read_failure: None,
    };
    let result = match super::command::launch(request.provider.as_str(), &request.args) {
        Some(text) => workflow.span(&mut client, &snapshot, &panel, &text, wake),
        None => Err(error(FailureKind::InvalidUsage, "command-too-long")),
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
            if failure.kind != FailureKind::OutcomeUnknown
                && agent_start_runtime::wait_backoff(wake, Duration::from_millis(1)) =>
        {
            Err(error(FailureKind::Cancelled, "cancelled"))
        }
        result => result,
    }
}

impl Workflow<'_> {
    fn step(&self) -> Result<Instant, Failure> {
        let now = Instant::now();
        if now >= self.deadline {
            return Err(error(FailureKind::Timeout, "start-deadline"));
        }
        Ok(self.deadline.min(now + READ_BUDGET))
    }

    fn span(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        panel: &Target,
        text: &str,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentSessionId, Failure> {
        let terminal = agent_start_runtime::primary_terminal(snapshot, panel)?;
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
        terminal: &Target,
        text: &str,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentSessionId, Failure> {
        let (source, sample) = self.sample(client, snapshot, terminal, wake)?;
        let shell = idle_shell(&sample.info)?;
        let before = shell.clone();
        let epoch = AgentHostEpoch::parse(source["hostEpoch"].as_str().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let physical = AgentHostTerminalId::parse(&sample.info.terminal_id.to_text())
            .map_err(|_| invalid())?;
        let presentation =
            AgentPresentationId::parse(&sample.terminal.presentation_id).map_err(|_| invalid())?;
        let ttl = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if ttl <= 3000 {
            return Err(error(FailureKind::Timeout, "start-deadline"));
        }
        interrupted(wake)?;
        let reservation_id = agent_start_runtime::obap_request_id()?;
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
            _ => return Err(error(FailureKind::OutcomeUnknown, "reply-not-confirmed")),
        };
        self.state = ReservationState::Pending;
        self.last_observed = Some(reservation.clone());
        let (latest_source, latest) = self.sample(client, snapshot, terminal, wake)?;
        let latest_shell = idle_shell(&latest.info)?;
        if latest_source["hostEpoch"] != source["hostEpoch"]
            || latest.info.terminal_id != sample.info.terminal_id
            || latest.terminal.presentation_id != sample.terminal.presentation_id
            || latest_shell.pid != before.pid
            || latest_shell.creation_time != before.creation_time
            || latest_shell.image != before.image
            || latest_shell.command_line != before.command_line
        {
            return Err(error(FailureKind::Rejected, "binding-changed"));
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
        let receipt = match agent_start_runtime::submit_prompt_on_session(
            client,
            snapshot,
            terminal,
            text,
            powershell,
            self.step()?,
            wake,
        ) {
            Ok(receipt) => receipt,
            Err(failure) => {
                self.delivery = if failure.kind == FailureKind::OutcomeUnknown {
                    DeliveryDisposition::Unknown
                } else {
                    DeliveryDisposition::NotApplied
                };
                let mut document = json!({"schema":failure.failure_schema.unwrap_or("orchbee.cli.failure"), "version":failure.version, "failure":{"kind":failure.kind.as_str()}});
                if let Some(source) = &failure.source {
                    document["source"] = source.clone();
                }
                if let Some(details) = &failure.details {
                    document["failure"]["details"] = json!(details);
                }
                self.submission = Some(document);
                return Err(failure);
            }
        };
        let submitted = Instant::now();
        let status = receipt.exit_status();
        self.delivery = delivery_disposition(receipt.verdict());
        // Readiness and cleanup remain separate from delivery. In particular,
        // status 5 is overloaded: its typed verdict decides whether input was
        // accepted, never the numeric process status alone.
        self.submission_status = Some(status);
        self.submission = Some(receipt.document());
        if !receipt.accepted() {
            let kind = match status {
                8 => FailureKind::OutcomeUnknown,
                4 => FailureKind::ServiceUnavailable,
                5 => FailureKind::TransportLoss,
                _ => FailureKind::Rejected,
            };
            return Err(error(kind, "input-not-accepted"));
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
        terminal: &Target,
        reservation: &AgentLaunchReservation,
        submitted: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<AgentSessionId, Failure> {
        loop {
            interrupted(wake)?;
            let outcome = exchange(
                client,
                AgentLaunchRequest::Inspect {
                    request_id: agent_start_runtime::obap_request_id()?,
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
            let (source, process) = self.sample(client, snapshot, terminal, wake)?;
            if source["hostEpoch"].as_str() != Some(reservation.binding.host_epoch.as_str())
                || process.info.terminal_id.to_text()
                    != reservation.binding.host_terminal_id.as_str()
                || process.terminal.presentation_id != reservation.binding.presentation_id.as_str()
            {
                return Err(error(FailureKind::Rejected, "binding-changed"));
            }
            let runtime = agent_start_runtime::interpret(&process.info);
            match runtime.foreground.selection {
                agent_start_runtime::Selection::Ambiguous => {
                    return Err(error(FailureKind::Rejected, "agent-runtime-ambiguous"));
                }
                agent_start_runtime::Selection::AgentChain
                    if runtime.foreground.provider != Some(self.request.provider.as_str()) =>
                {
                    return Err(error(FailureKind::Rejected, "agent-provider-mismatch"));
                }
                agent_start_runtime::Selection::Unavailable => {
                    return Err(error(FailureKind::Rejected, "agent-exited"));
                }
                _ => {}
            }
            if submitted.elapsed() >= SETTLE && idle_shell(&process.info).is_ok() {
                return Err(error(FailureKind::Rejected, "agent-exited"));
            }
            if let Some(session_id) = candidate {
                let observed = agent_start_runtime::read_agent_observation(
                    client,
                    &session_id,
                    self.step()?,
                    wake,
                )
                .map_err(|failure| self.read_failure(failure))?;
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
                    return Err(error(FailureKind::Rejected, "binding-changed"));
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
                    return Err(error(FailureKind::Rejected, "agent-blocked"));
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
                            request_id: agent_start_runtime::obap_request_id()?,
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
                        _ => return Err(error(FailureKind::OutcomeUnknown, "reply-not-confirmed")),
                    }
                }
            }
            let delay = Duration::from_millis(100)
                .min(self.deadline.saturating_duration_since(Instant::now()));
            if agent_start_runtime::wait_backoff(wake, delay) {
                return Err(error(FailureKind::Cancelled, "cancelled"));
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
        let Ok(request_id) = agent_start_runtime::obap_request_id() else {
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
            _ => {}
        }
    }

    fn sample(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        terminal: &Target,
        wake: Option<LocalIo<'_>>,
    ) -> Result<(Value, crate::cli::results::PanelProcessInfoDoc), Failure> {
        loop {
            interrupted(wake)?;
            let result = agent_start_runtime::read_terminal_processes_cancellable(
                client,
                self.step()?,
                snapshot,
                terminal,
                wake,
            );
            match result {
                Err(failure) if transient_sample(&failure) => {
                    self.last_read_failure = Some(cause(&failure));
                    let delay = Duration::from_millis(50)
                        .min(self.deadline.saturating_duration_since(Instant::now()));
                    if agent_start_runtime::wait_backoff(wake, delay) {
                        return Err(error(FailureKind::Cancelled, "cancelled"));
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
        if Instant::now() >= self.deadline
            && matches!(
                failure.kind,
                FailureKind::TransportLoss | FailureKind::Timeout
            )
        {
            self.last_read_failure = Some(cause(&failure));
            error(FailureKind::Timeout, "start-deadline")
        } else {
            failure
        }
    }

    fn finish(self, result: Result<AgentSessionId, Failure>) -> Report {
        let verdict = match result {
            Ok(session) => Verdict::Started { session },
            Err(failure) => {
                let cause = if failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(Value::as_str)
                    == Some("agent-start")
                {
                    self.last_read_failure.clone()
                } else {
                    Some(cause(&failure))
                };
                let reason = failure
                    .details
                    .as_ref()
                    .filter(|details| {
                        details.get("stage").and_then(Value::as_str) == Some("agent-start")
                    })
                    .and_then(|details| details.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("startup-failed")
                    .to_owned();
                Verdict::Failed {
                    failure,
                    reason,
                    cause,
                }
            }
        };
        Report {
            name: self.request.name.clone(),
            provider: self.request.provider.clone(),
            reservation: self.reservation,
            last_observed: self.last_observed,
            reservation_state: self.state,
            submission: self.submission,
            submission_status: self.submission_status,
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
    let refuse = || error(FailureKind::Rejected, "terminal-not-idle");
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
            script == &orchbee_local_site_runtime::windows_interactive_powershell_initialization()
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
        Err(error(FailureKind::Cancelled, "cancelled"))
    } else {
        Ok(())
    }
}
fn invalid() -> Failure {
    error(FailureKind::ProtocolInvalid, "launch-protocol-invalid")
}

fn transient_sample(failure: &Failure) -> bool {
    failure.failure_schema == Some("orchbee.cli.failure.process-info")
        && failure.kind == FailureKind::ServiceUnavailable
        && failure
            .details
            .as_ref()
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .is_some_and(|reason| {
                matches!(
                    reason,
                    "process-sample-changed" | "process-sample-unavailable"
                )
            })
}

pub(super) fn cause(failure: &Failure) -> Value {
    let field = |key| {
        failure
            .details
            .as_ref()
            .and_then(|details| details.get(key))
            .and_then(Value::as_str)
    };
    json!({"kind":failure.kind.as_str(), "stage":field("stage"), "operation":field("operation"), "reason":field("reason"), "requestId":failure.source.as_ref().and_then(|source| source.get("requestId")).and_then(Value::as_str)})
}
pub(super) fn error(kind: FailureKind, reason: &str) -> Failure {
    Failure::new(kind).details(crate::cli::stage_reason_details("agent-start", reason))
}
fn refusal(outcome: AgentLaunchOutcome) -> Failure {
    match outcome {
        AgentLaunchOutcome::Rejected { reason } => {
            let kind = if matches!(
                reason,
                orchbee_agent_observation_protocol::AgentLaunchRejectionReason::ReadDenied
                    | orchbee_agent_observation_protocol::AgentLaunchRejectionReason::ManageDenied
            ) {
                FailureKind::Denied
            } else {
                FailureKind::Rejected
            };
            error(
                kind,
                serde_json::to_value(reason)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .as_deref()
                    .unwrap_or("launch-refused"),
            )
        }
        AgentLaunchOutcome::Unavailable { .. } => {
            error(FailureKind::ServiceUnavailable, "runtime-unavailable")
        }
        _ => invalid(),
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
                error(FailureKind::OutcomeUnknown, "reply-not-confirmed")
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
            | AgentExchangeError::RequestNotSubmitted(io) => error(
                if io.kind() == std::io::ErrorKind::TimedOut {
                    FailureKind::Timeout
                } else {
                    FailureKind::TransportLoss
                },
                "launch-exchange-failed",
            ),
            AgentExchangeError::Failed { .. } => invalid(),
            _ => error(FailureKind::InternalFailure, "launch-exchange-failed"),
        })
}
