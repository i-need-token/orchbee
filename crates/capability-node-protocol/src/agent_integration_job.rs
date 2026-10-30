//! Defines generation-fenced start, poll, and cancellation actions and outcomes for the one
//! Worker-owned Agent integration job. Operation requests and reports are opaque bounded
//! documents of the Agent integration contract; this crate neither parses nor executes them.

use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Matches the Agent integration contract's encoded request bound.
pub const MAXIMUM_AGENT_INTEGRATION_REQUEST_BYTES: usize = 8 * 1024;
/// Matches the Agent integration contract's encoded report bound.
pub const MAXIMUM_AGENT_INTEGRATION_REPORT_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentIntegrationJobAction {
    Start { request: Vec<u8> },
    Poll { job_id: OperationId },
    Cancel { job_id: OperationId },
}

impl AgentIntegrationJobAction {
    pub fn start(request: Vec<u8>) -> Result<Self, ProtocolError> {
        if request.is_empty() || request.len() > MAXIMUM_AGENT_INTEGRATION_REQUEST_BYTES {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self::Start { request })
    }

    #[must_use]
    pub fn admits(&self, outcome: &AgentIntegrationJobOutcome) -> bool {
        match self {
            Self::Start { .. } => matches!(outcome, AgentIntegrationJobOutcome::Start(_)),
            Self::Poll { .. } => matches!(outcome, AgentIntegrationJobOutcome::Poll(_)),
            Self::Cancel { .. } => matches!(outcome, AgentIntegrationJobOutcome::Cancel(_)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIntegrationJobRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub action: AgentIntegrationJobAction,
}

/// `Busy` is a definite pre-effect refusal: another integration job still owns the node writer.
/// `InvalidRequest` means the operation document failed the contract before any effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntegrationJobStartOutcome {
    Accepted,
    Busy,
    InvalidRequest,
    SpawnThreadFailed,
    Dead,
    RejectedFencing,
}

/// `Completed` carries the encoded report. `Abandoned` means the job ended without a report the
/// contract can carry, so its effects are unknown; `Unknown` names a job the owner no longer
/// retains.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentIntegrationJobPollOutcome {
    Pending,
    Completed { report: Vec<u8> },
    Abandoned,
    Unknown,
    RejectedFencing,
}

impl AgentIntegrationJobPollOutcome {
    pub fn completed(report: Vec<u8>) -> Result<Self, ProtocolError> {
        if report.is_empty() || report.len() > MAXIMUM_AGENT_INTEGRATION_REPORT_BYTES {
            return Err(ProtocolError::PayloadTooLarge {
                actual: report.len(),
                maximum: MAXIMUM_AGENT_INTEGRATION_REPORT_BYTES,
            });
        }
        Ok(Self::Completed { report })
    }
}

/// `Registered` records cooperative cancellation; the job's final report stays observable
/// through `Poll`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIntegrationJobCancelOutcome {
    Registered,
    AlreadyTerminal,
    Unknown,
    RejectedFencing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentIntegrationJobOutcome {
    Start(AgentIntegrationJobStartOutcome),
    Poll(AgentIntegrationJobPollOutcome),
    Cancel(AgentIntegrationJobCancelOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentIntegrationJobResult {
    pub operation_id: OperationId,
    pub job_id: OperationId,
    pub outcome: AgentIntegrationJobOutcome,
}
