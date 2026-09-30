//! Read-only local branch/upstream facts; neither topology nor a remote fetch.
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};
pub const MAXIMUM_WORKSPACE_GIT_BYTES: usize = 8192;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitDivergence {
    pub ahead: u32,
    pub behind: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGitRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub workspace_id: WorkbenchWorkspaceId,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGitFailure {
    Denied,
    Unavailable,
    NotFound,
    SourceChanged,
    Timeout,
    Malformed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceGitOutcome {
    Observed {
        workbench_revision: WorkbenchRevision,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        branch: Option<String>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        divergence: Option<GitDivergence>,
    },
    Failed {
        reason: WorkspaceGitFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGitResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub outcome: WorkspaceGitOutcome,
}
impl WorkspaceGitResult {
    pub fn answers(&self, request: &WorkspaceGitRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.site_id == request.site_id
            && self.workspace_id == request.workspace_id
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkspaceGitOutcome::Observed {
            branch, divergence, ..
        } = &self.outcome
            && (branch
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 1024 || v.chars().any(char::is_control))
                || (branch.is_none() && divergence.is_some()))
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
