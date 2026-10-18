//! Typed Agent startup shared by the CLI adapter and native Client surfaces.
//!
//! The workflow owns the reserve -> one input submission -> canonical readiness
//! sequence. Callers inject an exact Panel, one absolute deadline, and an
//! optional cancellation wake. Presentation, process-wide signals, and surface
//! generations remain with their callers.

use std::time::Instant;

use orchbee_agent_observation_protocol::{
    AgentLaunchReservation, AgentName, AgentProviderId, AgentSessionId, ObapRequestId,
};
use serde_json::Value;

use crate::cli::{Failure, FailureKind};

mod command;
mod workflow;

/// One already-validated startup intent. Panel selection is deliberately not
/// part of this value: the caller supplies the exact target resolved for the
/// admitted Snapshot.
#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub(crate) name: AgentName,
    pub(crate) provider: AgentProviderId,
    pub(crate) args: Box<[String]>,
}

impl Request {
    pub(crate) fn new(name: AgentName, provider: AgentProviderId, args: Vec<String>) -> Self {
        Self {
            name,
            provider,
            args: args.into_boxed_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReservationState {
    None,
    Pending,
    Unknown,
    Released,
    Completed,
}

impl ReservationState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pending => "pending",
            Self::Unknown => "unknown",
            Self::Released => "released",
            Self::Completed => "completed",
        }
    }
}

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

/// Typed terminal result. Every independently known mutation/input fact is
/// retained before either the CLI or the TUI projects it.
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
        .then(|| workflow::cause(&failure));
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

    pub(crate) const fn failure_kind(&self) -> Option<FailureKind> {
        match &self.verdict {
            Verdict::Started { .. } => None,
            Verdict::Failed { failure, .. } => Some(failure.kind),
        }
    }

    pub(crate) const fn completed(&self) -> bool {
        matches!(self.verdict, Verdict::Started { .. })
    }

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
            Some(workflow::cause(&failure))
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

pub(crate) fn run_local(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    resolve_panel: impl FnOnce(
        &orchbee_workbench_protocol::Snapshot,
    ) -> Result<crate::invocation::Target, Failure>,
) -> Report {
    workflow::run_local(request, started, deadline, wake, resolve_panel)
}

pub(crate) fn run_on_origin(
    request: Request,
    started: Instant,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    origin: &crate::local_client::LocalClientOrigin,
    clock: crate::local_client::LocalOperationClock,
    panel: orchbee_workbench_protocol::WorkbenchPanelId,
) -> Report {
    workflow::run_on_origin(request, started, deadline, wake, origin, clock, panel)
}

pub(crate) fn failure(kind: FailureKind, reason: &str) -> Failure {
    workflow::error(kind, reason)
}

pub(crate) fn command_valid(request: &Request) -> bool {
    command::launch(request.provider.as_str(), &request.args).is_some()
}
