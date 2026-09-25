//! Windows runtime workflow. One absolute budget covers admission, reservation,
//! the single send, observation and best-effort name release.

use super::*;
use crate::cli::{
    caller_context::PanelTarget,
    output, processes, query, send,
    source::{ArmedSignals, Finish, SignalWatch},
};
use crate::invocation::Target;
use crate::local_client::{
    LocalClient,
    agent::{AgentExchangeError, AgentExchangeFailure},
};
use orchbee_agent_observation_protocol::{
    AgentHostEpoch, AgentHostTerminalId, AgentLaunchOutcome, AgentLaunchRequest,
    AgentLaunchReservation, AgentLifecycle, AgentObservationFreshness, AgentPresentationId,
    AgentProviderId, AgentSafeInteger, AgentSemanticStatus, AgentSessionId, ObapRequestId,
};
use orchbee_local_ipc::LocalIo;
use orchbee_terminal_protocol::{TerminalProcess, TerminalProcessInfo};
use orchbee_workbench_protocol::Snapshot;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-start";
const READ_BUDGET: Duration = Duration::from_secs(5);
const SETTLE: Duration = Duration::from_millis(3000);

#[cfg(test)]
mod tests;

struct Workflow<'a> {
    command: &'a AgentStartCommand,
    started: Instant,
    deadline: Instant,
    reservation: Option<ObapRequestId>,
    last_observed: Option<AgentLaunchReservation>,
    state: &'static str,
    submission: Option<Value>,
    submission_status: Option<u8>,
    last_read_failure: Option<Value>,
}

pub(super) fn run(command: &AgentStartCommand, format: OutputFormat, locale: Locale) -> Output {
    let start = Instant::now();
    let mut workflow = Workflow {
        command,
        started: start,
        deadline: start + Duration::from_millis(u64::from(command.timeout_ms)),
        reservation: None,
        last_observed: None,
        state: "none",
        submission: None,
        submission_status: None,
        last_read_failure: None,
    };
    let Some(text) = super::command::launch(&command.provider, &command.args) else {
        return workflow.failure(
            error(FailureKind::InvalidUsage, "command-too-long"),
            format,
            locale,
        );
    };
    let armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => {
            return workflow.failure(
                error(FailureKind::InternalFailure, "signal-registration-failed"),
                format,
                locale,
            );
        }
    };
    let mut result = workflow.span(&text, armed.poll_fd());
    let finalization = match armed.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization, .. }) => {
            if result
                .as_ref()
                .is_err_and(|failure| failure.kind != FailureKind::OutcomeUnknown)
            {
                result = Err(error(FailureKind::Cancelled, "cancelled"));
            }
            Some(finalization)
        }
        Err(_) => {
            if workflow.state != "completed"
                && !result
                    .as_ref()
                    .is_err_and(|failure| failure.kind == FailureKind::OutcomeUnknown)
            {
                result = Err(error(
                    FailureKind::InternalFailure,
                    "signal-finalization-failed",
                ));
            }
            None
        }
    };
    let mut answer = match result {
        Ok(session) => workflow.success(&session, format, locale),
        Err(failure) => workflow.failure(failure, format, locale),
    };
    answer.finalization = finalization;
    answer
}

impl Workflow<'_> {
    fn step(&self) -> Result<Instant, Failure> {
        let now = Instant::now();
        if now >= self.deadline {
            return Err(error(FailureKind::Timeout, "start-deadline"));
        }
        Ok(self.deadline.min(now + READ_BUDGET))
    }

    fn span(&mut self, text: &str, wake: Option<LocalIo<'_>>) -> Result<AgentSessionId, Failure> {
        let prepared =
            PanelTarget::prepare(self.command.panel.as_ref(), |name| std::env::var_os(name))?;
        let (mut client, _, snapshot) = query::snapshot_session_until(self.step()?)?;
        let panel = prepared.resolve(&snapshot)?;
        let terminal = query::primary_terminal(&snapshot, &panel)?;
        let answer = self.launch_on_session(&mut client, &snapshot, &terminal, text, wake);
        if answer.is_err() && self.state == "pending" {
            self.release(&mut client);
        }
        drop(client);
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
        let reservation_id = query::obap_request_id()?;
        self.reservation = Some(reservation_id.clone());
        // Once a mutating exchange starts, only its correlated reply can settle
        // this knowledge. A lost reply is never followed by another reserve.
        self.state = "unknown";
        let outcome = exchange(
            client,
            AgentLaunchRequest::Reserve {
                request_id: reservation_id.clone(),
                name: self.command.name.clone(),
                provider_id: AgentProviderId::parse(&self.command.provider)
                    .map_err(|_| invalid())?,
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
                    && reservation.name == self.command.name
                    && reservation.provider_id.as_str() == self.command.provider
                    && reservation.binding.presentation_id == presentation
                    && reservation.binding.host_epoch == epoch
                    && reservation.binding.host_terminal_id == physical =>
            {
                reservation
            }
            outcome @ (AgentLaunchOutcome::Rejected { .. }
            | AgentLaunchOutcome::Unavailable { .. }) => {
                self.state = "none";
                return Err(refusal(outcome));
            }
            _ => return Err(error(FailureKind::OutcomeUnknown, "reply-not-confirmed")),
        };
        self.state = "pending";
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
        let receipt = match send::submit_prompt_on_session(
            client,
            snapshot,
            terminal,
            text,
            powershell,
            self.step()?,
        ) {
            Ok(receipt) => receipt,
            Err(failure) => {
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
                    request_id: query::obap_request_id()?,
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
            let runtime = processes::interpret(&process.info);
            match runtime.foreground.selection {
                processes::Selection::Ambiguous => {
                    return Err(error(FailureKind::Rejected, "agent-runtime-ambiguous"));
                }
                processes::Selection::AgentChain
                    if runtime.foreground.provider != Some(self.command.provider.as_str()) =>
                {
                    return Err(error(FailureKind::Rejected, "agent-provider-mismatch"));
                }
                processes::Selection::Unavailable => {
                    return Err(error(FailureKind::Rejected, "agent-exited"));
                }
                _ => {}
            }
            if submitted.elapsed() >= SETTLE && idle_shell(&process.info).is_ok() {
                return Err(error(FailureKind::Rejected, "agent-exited"));
            }
            if let Some(session_id) = candidate {
                let observed =
                    query::read_agent_observation(client, &session_id, self.step()?, wake)
                        .map_err(|failure| self.read_failure(failure))?;
                let detail = &observed.session;
                let pinned = detail.snapshot.provider_id.as_str() == self.command.provider
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
                if !detail.snapshot.pending_requests.is_empty()
                    || matches!(detail.status, AgentSemanticStatus::NeedsAttention { .. })
                {
                    return Err(error(FailureKind::Rejected, "agent-blocked"));
                }
                let ready = detail.snapshot.observation.freshness
                    == AgentObservationFreshness::Fresh
                    && matches!(
                        detail.status,
                        AgentSemanticStatus::Idle { .. }
                            | AgentSemanticStatus::Succeeded { .. }
                            | AgentSemanticStatus::Failed { .. }
                            | AgentSemanticStatus::Canceled { .. }
                            | AgentSemanticStatus::Interrupted { .. }
                    );
                if ready
                    && runtime.foreground.provider == Some(self.command.provider.as_str())
                    && submitted.elapsed() >= SETTLE
                    && pending.settle_remaining_ms.value() == 0
                {
                    interrupted(wake)?;
                    self.state = "unknown";
                    let outcome = exchange(
                        client,
                        AgentLaunchRequest::Complete {
                            request_id: query::obap_request_id()?,
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
                            && name == self.command.name
                            && confirmed == session_id =>
                        {
                            self.state = "completed";
                            return Ok(session_id);
                        }
                        AgentLaunchOutcome::Rejected {
                            reason:
                                orchbee_agent_observation_protocol::AgentLaunchRejectionReason::NotReady,
                        } => self.state = "pending",
                        other @ (AgentLaunchOutcome::Rejected { .. }
                        | AgentLaunchOutcome::Unavailable { .. }) => {
                            self.state = "pending";
                            return Err(refusal(other));
                        }
                        _ => return Err(error(FailureKind::OutcomeUnknown, "reply-not-confirmed")),
                    }
                }
            }
            let delay = Duration::from_millis(100)
                .min(self.deadline.saturating_duration_since(Instant::now()));
            if query::wait_backoff(wake, delay) {
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
        let Ok(request_id) = query::obap_request_id() else {
            return;
        };
        self.state = "unknown";
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
            }) if confirmed == reservation_id => self.state = "released",
            Ok(AgentLaunchOutcome::Rejected {
                reason:
                    orchbee_agent_observation_protocol::AgentLaunchRejectionReason::ReservationNotFound,
            }) => self.state = "released",
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
            let result = query::read_terminal_processes(client, self.step()?, snapshot, terminal);
            match result {
                Err(failure) if transient_sample(&failure) => {
                    self.last_read_failure = Some(cause(&failure));
                    let delay = Duration::from_millis(50)
                        .min(self.deadline.saturating_duration_since(Instant::now()));
                    if query::wait_backoff(wake, delay) {
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

    fn elapsed(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn failure(&self, mut failure: Failure, format: OutputFormat, locale: Locale) -> Output {
        let upstream = if failure
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
            .filter(|details| details.get("stage").and_then(Value::as_str) == Some("agent-start"))
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("startup-failed")
            .to_owned();
        failure.details = Some(serde_json::Map::from_iter([
            ("stage".into(), json!("agent-start")),
            ("reason".into(), json!(reason)),
            ("name".into(), json!(self.command.name)),
            ("providerId".into(), json!(self.command.provider)),
            ("reservationId".into(), json!(self.reservation)),
            ("reservationState".into(), json!(self.state)),
            ("lastObservedReservation".into(), json!(self.last_observed)),
            ("submission".into(), json!(self.submission)),
            ("elapsedMs".into(), json!(self.elapsed())),
            ("cause".into(), json!(upstream)),
        ]));
        failure.source = None;
        failure.version = 1;
        failure.failure_schema = Some(FAILURE_SCHEMA);
        let submission_status = (reason == "input-not-accepted")
            .then_some(self.submission_status)
            .flatten();
        let mut answer = failure_output(failure, Some(format), locale);
        if let Some(status) = submission_status {
            answer.status = status;
        }
        answer
    }

    fn success(&self, session: &AgentSessionId, format: OutputFormat, locale: Locale) -> Output {
        let document = json!({"schema":"orchbee.agent.start", "version":2, "result":{
            "kind":"started", "interactiveReady":true, "name":self.command.name, "providerId":self.command.provider,
            "sessionId":session, "reservationId":self.reservation, "elapsedMs":self.elapsed(), "submission":self.submission,
        }});
        let bytes = match format {
            OutputFormat::Json => output::encode_document(&document),
            OutputFormat::Text => output::encode_text(
                &locale.cli_agent_started(self.command.name.as_str(), session.as_str()),
            ),
        };
        match bytes {
            Ok(stdout) => Output {
                status: 0,
                stdout,
                stderr: String::new(),
                finalization: None,
            },
            Err(_) => self.failure(
                error(FailureKind::OutputTooLarge, "output-too-large"),
                format,
                locale,
            ),
        }
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
    let interpreted = processes::interpret(sample);
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
    for arg in &args[1..] {
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
    if query::wait_backoff(wake, Duration::from_millis(1)) {
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

fn cause(failure: &Failure) -> Value {
    let field = |key| {
        failure
            .details
            .as_ref()
            .and_then(|details| details.get(key))
            .and_then(Value::as_str)
    };
    json!({"kind":failure.kind.as_str(), "stage":field("stage"), "operation":field("operation"), "reason":field("reason"), "requestId":failure.source.as_ref().and_then(|source| source.get("requestId")).and_then(Value::as_str)})
}
fn error(kind: FailureKind, reason: &str) -> Failure {
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
