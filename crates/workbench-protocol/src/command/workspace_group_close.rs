//! Bounded retained Workspace group closure; child lifecycle truth remains owned by Close.
use super::json_fields::deserialize_nullable;
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchLabel, WorkbenchProjectId, WorkbenchRequestId,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseStatus, WorkbenchWorkspaceId,
};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeSet;

/// Independent aggregate response bound, checked before a frame body is allocated.
pub const MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES: usize = 512 * 1024;
/// Maximum frozen Workspace scopes in one group operation.
pub const MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS: usize = 128;
/// Explicit attempts never wrap or silently replay.
pub const MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS: u32 = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceGroupCloseAction {
    Start {
        operation_id: WorkbenchRequestId,
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
    },
    Read {
        operation_id: WorkbenchRequestId,
    },
    Resume {
        operation_id: WorkbenchRequestId,
        #[serde(deserialize_with = "deserialize_attempt")]
        expected_attempt: u32,
    },
}
impl WorkspaceGroupCloseAction {
    /// Stable operation identity, independent of this exchange.
    pub fn operation_id(&self) -> &WorkbenchRequestId {
        match self {
            Self::Start { operation_id, .. }
            | Self::Read { operation_id }
            | Self::Resume { operation_id, .. } => operation_id,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceGroupCloseRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: WorkspaceGroupCloseAction,
}
impl WorkbenchWorkspaceGroupCloseRequest {
    /// Rejects attempts outside the retained generation space.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkspaceGroupCloseAction::Resume {
            expected_attempt, ..
        } = self.action
        {
            validate_attempt(expected_attempt)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGroupCloseChild {
    pub request_id: WorkbenchRequestId,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub observation: Option<WorkspaceGroupCloseObservation>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGroupCloseObservation {
    pub attempt_generation: WorkbenchTabCloseAttemptGeneration,
    pub status: WorkbenchTabCloseStatus,
}
impl WorkspaceGroupCloseChild {
    /// Closure is known only through an observed child tombstone.
    pub fn is_closed(&self) -> bool {
        self.observation.as_ref().is_some_and(|observation| {
            matches!(observation.status, WorkbenchTabCloseStatus::Closed { .. })
        })
    }
    /// An unobserved accepted Close cannot be replayed or declared resumable.
    pub fn can_observe_or_resume(&self) -> bool {
        self.observation.as_ref().is_some_and(|observation| {
            !matches!(
                observation.status,
                WorkbenchTabCloseStatus::OutcomeUnknown { .. }
                    | WorkbenchTabCloseStatus::CleanupBlocked {
                        resumable: false,
                        ..
                    }
                    | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        resumable: false,
                        ..
                    }
            )
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGroupCloseMember {
    pub project_id: WorkbenchProjectId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub label: WorkbenchLabel,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub close: Option<WorkspaceGroupCloseChild>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGroupCloseFailure {
    SourceChanged,
    WorkerUnavailable,
    ChildAdmission,
    ChildBlocked,
    OutcomeUnknown,
    Deadline,
    SiteStopping,
    Internal,
}
impl WorkspaceGroupCloseFailure {
    /// Eligibility is additionally constrained by the observed child lifecycle status.
    pub fn permits_resume(self) -> bool {
        matches!(
            self,
            Self::WorkerUnavailable | Self::ChildAdmission | Self::ChildBlocked | Self::Deadline
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkspaceGroupCloseStatus {
    Pending {},
    Closed {},
    Failed {
        reason: WorkspaceGroupCloseFailure,
        resumable: bool,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGroupCloseUnavailable {
    Unsupported,
    ServiceUnavailable,
    Busy,
    Capacity,
    SourceUnavailable,
    AmbiguousMembership,
    WorkerUnavailable,
    SourceChanged,
    NotResumable,
    StaleAttempt,
    AttemptExhausted,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceGroupCloseOutcome {
    Available {
        project_id: WorkbenchProjectId,
        workspace_id: WorkbenchWorkspaceId,
        #[serde(deserialize_with = "deserialize_attempt")]
        attempt: u32,
        status: WorkspaceGroupCloseStatus,
        members: Vec<WorkspaceGroupCloseMember>,
    },
    NotFound {},
    Conflict {},
    Denied {},
    Unavailable {
        reason: WorkspaceGroupCloseUnavailable,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceGroupCloseResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub operation_id: WorkbenchRequestId,
    pub outcome: WorkspaceGroupCloseOutcome,
}
impl WorkbenchWorkspaceGroupCloseResult {
    /// Enforces bounded unique membership, source-last ordering and truthful aggregate completion.
    pub fn validate(&self) -> Result<(), ContractError> {
        let WorkspaceGroupCloseOutcome::Available {
            project_id,
            workspace_id,
            attempt,
            status,
            members,
        } = &self.outcome
        else {
            return Ok(());
        };
        validate_attempt(*attempt)?;
        if members.is_empty()
            || members.len() > MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS
            || !members.last().is_some_and(|member| {
                member.project_id == *project_id && member.workspace_id == *workspace_id
            })
        {
            return Err(ContractError::InvalidDocument);
        }
        let mut workspaces = BTreeSet::new();
        let mut requests = BTreeSet::new();
        let mut blocked = false;
        for member in members {
            if !workspaces.insert(&member.workspace_id) {
                return Err(ContractError::InvalidDocument);
            }
            if let Some(child) = &member.close {
                if blocked
                    || child.request_id == self.operation_id
                    || !requests.insert(&child.request_id)
                {
                    return Err(ContractError::InvalidDocument);
                }
                blocked = !child.is_closed();
            } else {
                blocked = true;
            }
        }
        if matches!(status, WorkspaceGroupCloseStatus::Closed {}) && blocked {
            return Err(ContractError::InvalidDocument);
        }
        if let WorkspaceGroupCloseStatus::Failed {
            reason,
            resumable: true,
        } = status
            && (!reason.permits_resume()
                || members.iter().any(|member| {
                    member
                        .close
                        .as_ref()
                        .is_some_and(|child| !child.can_observe_or_resume())
                }))
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
fn validate_attempt(attempt: u32) -> Result<(), ContractError> {
    if (1..=MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS).contains(&attempt) {
        Ok(())
    } else {
        Err(ContractError::InvalidDocument)
    }
}
fn deserialize_attempt<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if value.is_finite()
        && value.fract() == 0.0
        && value >= 1.0
        && value <= f64::from(MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS)
    {
        Ok(value as u32)
    } else {
        Err(de::Error::custom("invalid Workspace group Close attempt"))
    }
}
