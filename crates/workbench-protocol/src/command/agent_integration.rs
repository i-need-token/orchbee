//! Defines Client start, poll, and cancellation of the Server-authorized Agent integration job on
//! the selected Worker. Operation requests and reports use the Agent integration contract; this
//! module neither inspects nor changes provider configuration.

use crate::{
    ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision, WorkerId,
};
use orchbee_agent_integration::{IntegrationOperation, IntegrationReport, IntegrationRequest};
use serde::{Deserialize, Serialize};

/// Encoded result bound: one contract report plus the job envelope.
pub const MAXIMUM_ENCODED_AGENT_INTEGRATION_RESULT_BYTES: usize =
    orchbee_agent_integration::MAXIMUM_INTEGRATION_REPORT_BYTES + 4 * 1024;
/// Encoded request bound: one contract request plus the job envelope.
pub const MAXIMUM_ENCODED_AGENT_INTEGRATION_REQUEST_BYTES: usize =
    orchbee_agent_integration::MAXIMUM_INTEGRATION_REQUEST_BYTES + 2 * 1024;

/// The Worker generation and job a poll or cancel is fenced to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentIntegrationJob {
    pub worker_id: WorkerId,
    pub generation: WorkbenchRevision,
    /// 32 lowercase hexadecimal digits.
    pub job_id: String,
}

impl AgentIntegrationJob {
    fn validate(&self) -> Result<(), ContractError> {
        if self.generation.value() == 0
            || self.job_id.len() != 32
            || !self
                .job_id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentIntegrationAction {
    Start { request: IntegrationRequest },
    Poll { job: AgentIntegrationJob },
    Cancel { job: AgentIntegrationJob },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentIntegrationRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub action: AgentIntegrationAction,
}

impl AgentIntegrationAction {
    /// Starting `enable` or `disable` changes node-local provider configuration.
    #[must_use]
    pub fn mutating(&self) -> bool {
        matches!(
            self,
            Self::Start {
                request: IntegrationRequest {
                    operation: IntegrationOperation::Enable { .. }
                        | IntegrationOperation::Disable { .. },
                    ..
                }
            }
        )
    }
}

impl AgentIntegrationRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.action {
            AgentIntegrationAction::Start { request } => request
                .validate()
                .map_err(|_| ContractError::InvalidDocument),
            AgentIntegrationAction::Poll { job } | AgentIntegrationAction::Cancel { job } => {
                job.validate()
            }
        }
    }
}

/// Why no job outcome is available. `Busy` is a definite pre-effect refusal; `WorkerChanged`
/// means the fenced Worker generation is gone, so the job's effects are unknown.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentIntegrationFailure {
    Denied,
    SessionMismatch,
    InvalidRequest,
    WorkerUnavailable,
    WorkerChanged,
    Busy,
    JobUnknown,
    Abandoned,
    Malformed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentIntegrationOutcome {
    /// The Worker admitted the operation; poll `job` for its report.
    Accepted {
        job: AgentIntegrationJob,
    },
    Pending {
        job: AgentIntegrationJob,
    },
    Completed {
        job: AgentIntegrationJob,
        report: Box<IntegrationReport>,
    },
    /// Cooperative cancellation is recorded; the final report stays observable through `Poll`.
    CancelRegistered {
        job: AgentIntegrationJob,
    },
    /// The job had already settled; poll for its report.
    AlreadyTerminal {
        job: AgentIntegrationJob,
    },
    Failed {
        reason: AgentIntegrationFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentIntegrationResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub outcome: AgentIntegrationOutcome,
}

impl AgentIntegrationResult {
    /// A job outcome answers only its own action, and a poll or cancel answers only its job.
    pub fn answers(&self, request: &AgentIntegrationRequest) -> bool {
        if self.protocol_version != request.protocol_version
            || self.request_id != request.request_id
            || self.session_id != request.session_id
        {
            return false;
        }
        match (&request.action, &self.outcome) {
            (_, AgentIntegrationOutcome::Failed { .. })
            | (AgentIntegrationAction::Start { .. }, AgentIntegrationOutcome::Accepted { .. }) => {
                true
            }
            (
                AgentIntegrationAction::Poll { job },
                AgentIntegrationOutcome::Pending { job: answered }
                | AgentIntegrationOutcome::Completed { job: answered, .. },
            )
            | (
                AgentIntegrationAction::Cancel { job },
                AgentIntegrationOutcome::CancelRegistered { job: answered }
                | AgentIntegrationOutcome::AlreadyTerminal { job: answered },
            ) => job == answered,
            _ => false,
        }
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            AgentIntegrationOutcome::Accepted { job }
            | AgentIntegrationOutcome::Pending { job }
            | AgentIntegrationOutcome::Completed { job, .. }
            | AgentIntegrationOutcome::CancelRegistered { job }
            | AgentIntegrationOutcome::AlreadyTerminal { job } => job.validate(),
            AgentIntegrationOutcome::Failed { .. } => Ok(()),
        }
    }
}
