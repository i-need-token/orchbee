//! CLI startup presentation and signal-boundary overrides, separate from domain effect facts.
use super::{Failure, FailureKind};
use crate::agent_start::{Request, ReservationState};
use orchbee_agent_observation_protocol::{
    AgentLaunchReservation, AgentName, AgentProviderId, AgentSessionId, ObapRequestId,
};
use serde_json::{Value, json};
use std::time::Instant;

/// Presentation of startup completion or failure, including the CLI reason and optional read cause.
#[derive(Debug)]
pub(crate) enum Verdict {
    Started {
        session: AgentSessionId,
    },
    Failed {
        failure: Failure,
        reason: String,
        cause: Option<Value>,
    },
}

/// CLI presentation of the independently retained domain facts, with process-local overrides.
#[derive(Debug)]
pub(crate) struct Report {
    pub(crate) name: AgentName,
    pub(crate) provider: AgentProviderId,
    pub(crate) reservation: Option<ObapRequestId>,
    pub(crate) last_observed: Option<AgentLaunchReservation>,
    pub(crate) reservation_state: ReservationState,
    pub(crate) submission: Option<Value>,
    pub(crate) submission_status: Option<u8>,
    pub(crate) elapsed_ms: u64,
    pub(crate) verdict: Verdict,
}

impl Report {
    /// Present a CLI preflight failure with no reservation or submission and the original start time.
    pub(crate) fn preflight(request: Request, started: Instant, failure: Failure) -> Self {
        let reason = failure
            .details
            .as_ref()
            .filter(|details| details.get("stage").and_then(Value::as_str) == Some("agent-start"))
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("startup-failed")
            .to_owned();
        let cause = (failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(Value::as_str)
            != Some("agent-start"))
        .then(|| cause(&failure));
        Self {
            name: request.name,
            provider: request.provider,
            reservation: None,
            last_observed: None,
            reservation_state: ReservationState::None,
            submission: None,
            submission_status: None,
            elapsed_ms: elapsed_ms(started),
            verdict: Verdict::Failed {
                failure,
                reason,
                cause,
            },
        }
    }

    /// Expose the current presentation failure for signal-finalization arbitration.
    pub(crate) const fn failure_kind(&self) -> Option<FailureKind> {
        match &self.verdict {
            Verdict::Started { .. } => None,
            Verdict::Failed { failure, .. } => Some(failure.kind),
        }
    }

    /// Report confirmed startup completion; accepted input alone does not satisfy this condition.
    pub(crate) const fn completed(&self) -> bool {
        matches!(self.verdict, Verdict::Started { .. })
    }

    /// Apply a CLI signal-boundary failure without replacing the recorded submission or reservation.
    /// Preserve a previous read cause only when it belonged to an Agent startup failure.
    pub(crate) fn replace_failure(&mut self, failure: Failure) {
        let read_cause = match &self.verdict {
            Verdict::Failed {
                failure: previous,
                cause,
                ..
            } if previous
                .details
                .as_ref()
                .and_then(|details| details.get("stage"))
                .and_then(Value::as_str)
                == Some("agent-start") =>
            {
                cause.clone()
            }
            Verdict::Started { .. } | Verdict::Failed { .. } => None,
        };
        let reason = failure
            .details
            .as_ref()
            .filter(|details| details.get("stage").and_then(Value::as_str) == Some("agent-start"))
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .unwrap_or("startup-failed")
            .to_owned();
        let cause = if failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(Value::as_str)
            == Some("agent-start")
        {
            read_cause
        } else {
            Some(cause(&failure))
        };
        self.verdict = Verdict::Failed {
            failure,
            reason,
            cause,
        };
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Project the existing failure's diagnostic identity without embedding its complete document.
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

/// Project typed startup facts through the existing query/send formatters before CLI overrides.
/// A later workflow failure cannot replace the submission envelope or its independent status.
impl From<crate::agent_start::Report> for Report {
    fn from(report: crate::agent_start::Report) -> Self {
        use crate::agent_start::{Submission, Verdict as DomainVerdict};
        let (submission, submission_status) = match report.submission {
            Some(Submission::Sent(receipt)) => {
                let (document, status) = super::super::send::startup_receipt(&receipt);
                (Some(document), Some(status))
            }
            Some(Submission::Failed(error)) => {
                let failure = super::super::send::startup_failure(&error);
                let mut document = json!({"schema":failure.failure_schema.unwrap_or("orchbee.cli.failure"), "version":1, "failure":{"kind":failure.kind.as_str()}});
                if let Some(source) = failure.source {
                    document["source"] = source;
                }
                if let Some(details) = failure.details {
                    document["failure"]["details"] = json!(details);
                }
                (Some(document), None)
            }
            None => (None, None),
        };
        let verdict = match report.verdict {
            DomainVerdict::Started { session } => Verdict::Started { session },
            DomainVerdict::Failed {
                failure,
                read_cause,
            } => {
                let failure = domain_failure(failure);
                let startup = failure.details.as_ref().is_some_and(|details| {
                    details.get("stage").and_then(Value::as_str) == Some("agent-start")
                });
                let reason = failure
                    .details
                    .as_ref()
                    .filter(|_| startup)
                    .and_then(|details| details.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("startup-failed")
                    .to_owned();
                let cause = if startup {
                    read_cause.map(|error| cause(&domain_failure(*error)))
                } else {
                    Some(cause(&failure))
                };
                Verdict::Failed {
                    failure,
                    reason,
                    cause,
                }
            }
        };
        Self {
            name: report.name,
            provider: report.provider,
            reservation: report.reservation,
            last_observed: report.last_observed,
            reservation_state: report.reservation_state,
            submission,
            submission_status,
            elapsed_ms: report.elapsed_ms,
            verdict,
        }
    }
}

/// Translate operation-specific domain evidence into the existing CLI failure schema and reasons.
/// This presentation boundary performs no recovery, observation or input submission.
pub(super) fn domain_failure(error: crate::agent_start::Failure) -> Failure {
    use crate::agent_runtime::startup::{binding::PrimaryError, send::InputFailure};
    use crate::agent_start::{ExchangeFailure, Failure as DomainFailure, StartFailure};
    let start = match error {
        DomainFailure::Unsupported => {
            return Failure::new(FailureKind::ServiceUnavailable).details(
                super::super::stage_reason_details("platform", "unsupported-platform"),
            );
        }
        DomainFailure::Identity => return Failure::new(FailureKind::InternalFailure),
        DomainFailure::Primary(PrimaryError::InvalidBinding) => {
            return Failure::new(FailureKind::InternalFailure);
        }
        DomainFailure::Primary(PrimaryError::Unavailable(panel)) => {
            return Failure::new(FailureKind::NotFound).details(super::super::target_details(
                crate::invocation::Resource::Panel,
                panel.as_str(),
            ));
        }
        DomainFailure::Admission(error) => {
            return super::super::query::classify_startup_admission(*error);
        }
        DomainFailure::Process(error) => return super::super::query::classify_process(*error),
        DomainFailure::Observation(error) => {
            return super::super::query::classify_observation(*error);
        }
        DomainFailure::Send(error) => return super::super::send::startup_failure(&error),
        DomainFailure::Start(start) => start,
    };
    let (kind, reason) = match start {
        StartFailure::Deadline => (FailureKind::Timeout, "start-deadline"),
        StartFailure::Cancelled => (FailureKind::Cancelled, "cancelled"),
        StartFailure::CommandTooLong => (FailureKind::InvalidUsage, "command-too-long"),
        StartFailure::TerminalNotIdle => (FailureKind::Rejected, "terminal-not-idle"),
        StartFailure::BindingChanged => (FailureKind::Rejected, "binding-changed"),
        StartFailure::RuntimeAmbiguous => (FailureKind::Rejected, "agent-runtime-ambiguous"),
        StartFailure::ProviderMismatch => (FailureKind::Rejected, "agent-provider-mismatch"),
        StartFailure::AgentExited => (FailureKind::Rejected, "agent-exited"),
        StartFailure::AgentBlocked => (FailureKind::Rejected, "agent-blocked"),
        StartFailure::ReplyNotConfirmed => (FailureKind::OutcomeUnknown, "reply-not-confirmed"),
        StartFailure::InvalidProtocol => (FailureKind::ProtocolInvalid, "launch-protocol-invalid"),
        StartFailure::RuntimeUnavailable => {
            (FailureKind::ServiceUnavailable, "runtime-unavailable")
        }
        StartFailure::Rejected(reason) => {
            use orchbee_agent_observation_protocol::AgentLaunchRejectionReason;
            let kind = if matches!(
                reason,
                AgentLaunchRejectionReason::ReadDenied | AgentLaunchRejectionReason::ManageDenied
            ) {
                FailureKind::Denied
            } else {
                FailureKind::Rejected
            };
            return super::failure(
                kind,
                serde_json::to_value(reason)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .as_deref()
                    .unwrap_or("launch-refused"),
            );
        }
        StartFailure::Exchange(reason) => (
            match reason {
                ExchangeFailure::Timeout => FailureKind::Timeout,
                ExchangeFailure::Transport => FailureKind::TransportLoss,
                ExchangeFailure::Internal => FailureKind::InternalFailure,
            },
            "launch-exchange-failed",
        ),
        StartFailure::InputNotAccepted(reason) => (
            match reason {
                InputFailure::Unknown => FailureKind::OutcomeUnknown,
                InputFailure::Unavailable => FailureKind::ServiceUnavailable,
                InputFailure::Transport => FailureKind::TransportLoss,
                InputFailure::Rejected => FailureKind::Rejected,
            },
            "input-not-accepted",
        ),
    };
    super::failure(kind, reason)
}
