//! Reads whether closing one Workspace would ask first, by the evidence a conditional Workspace
//! Close reads, and which of its terminals keep it from closing unasked. The read closes, reserves
//! and retains nothing; a Close admitted later reads its own evidence again.
use crate::{
    ContractError, ProtocolVersion, SessionId, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchTabId, WorkbenchWorkspaceId,
    validation::MAXIMUM_PANEL_NODES_PER_CONTEXT,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Exact encoded bound of a `workbench.workspace-idle.result` document: the envelope at its
/// longest spelling plus `MAXIMUM_PANEL_NODES_PER_CONTEXT` members, each at its longest reason. A
/// test builds that document and compares its length with this constant.
pub const MAXIMUM_ENCODED_WORKSPACE_IDLE_RESULT_BYTES: usize = 659_699;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceIdleRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
}

/// Why one terminal keeps its Workspace from closing unasked.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceIdleReason {
    /// Its shell reported a running command, or a program other than the shell holds its console.
    Running,
    /// Its evidence could not be read in time, or did not tell.
    Unknown,
}

/// One terminal of the Workspace that keeps it from closing unasked, as the exact
/// `(Tab, Panel, Presentation)` member a Close of the Workspace would capture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceIdleMember {
    pub tab_id: WorkbenchTabId,
    pub panel_id: WorkbenchPanelId,
    pub presentation_id: TerminalPresentationId,
    pub reason: WorkspaceIdleReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceIdleFailure {
    Denied,
    /// The Session has no Workspace with that identity.
    NotFound,
    /// The Session's Workbench is unavailable.
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceIdleOutcome {
    /// Every terminal has ended, never started, or shows nothing beyond an idle shell: closing the
    /// Workspace would not ask.
    Idle {},
    /// Closing the Workspace would ask; `members` are the terminals that keep it from closing
    /// unasked, in the order a Close captures them.
    Busy {
        members: Vec<WorkspaceIdleMember>,
    },
    Failed {
        reason: WorkspaceIdleFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceIdleResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub outcome: WorkspaceIdleOutcome,
}

impl WorkspaceIdleResult {
    /// Whether this result correlates with `request`.
    pub fn answers(&self, request: &WorkspaceIdleRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.workspace_id == request.workspace_id
    }

    /// A busy outcome names one to `MAXIMUM_PANEL_NODES_PER_CONTEXT` distinct Presentations.
    pub fn validate(&self) -> Result<(), ContractError> {
        let WorkspaceIdleOutcome::Busy { members } = &self.outcome else {
            return Ok(());
        };
        let mut seen = HashSet::with_capacity(members.len());
        if members.is_empty()
            || members.len() > MAXIMUM_PANEL_NODES_PER_CONTEXT
            || !members
                .iter()
                .all(|member| seen.insert(&member.presentation_id))
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}
