//! Carries bounded Workspace closure observations and generation-fenced resumes with explicit
//! per-member status and counts. It validates the aggregate contract without executing child
//! closes or inferring physical cleanup from membership removal.

use serde::{Deserialize, Serialize};

use crate::{
    ContractError, ProtocolVersion, SessionId, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
    WorkbenchTabClosePhase, WorkbenchTabClosePublicationReason,
    WorkbenchTabCloseResumeRejectionReason, WorkbenchTabCloseStatus,
    WorkbenchTabCloseUnavailableReason, WorkbenchTabCloseUnknownPhase,
    WorkbenchTabCloseUnknownReason, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWorkspaceId, validation::MAXIMUM_PANEL_NODES_PER_CONTEXT,
};

/// Exact encoded bound of a `workbench.workspace-close.result` document whose
/// `available` outcome carries the protocol's maximum captured membership.
/// Derived from the fixed envelope plus `MAXIMUM_PANEL_NODES_PER_CONTEXT`
/// member entries at their longest legal status spelling; a unit test
/// constructs that exact document and asserts equality with this constant.
pub const MAXIMUM_ENCODED_WORKSPACE_CLOSE_READ_RESULT_BYTES: usize = 1_000_007;

/// Per-member lifecycle truth in a Workspace Close read: the member's own
/// terminal disposition only — the close revision belongs solely to the
/// scope-level `status`, never to a member entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkbenchWorkspaceCloseMemberStatus {
    Closing {
        phase: WorkbenchTabClosePhase,
    },
    CleanupBlocked {
        reason: WorkbenchTabCloseBlockedReason,
        resumable: bool,
    },
    OutcomeUnknown {
        phase: WorkbenchTabCloseUnknownPhase,
        reason: WorkbenchTabCloseUnknownReason,
    },
    DisposedAwaitingPublication {
        reason: WorkbenchTabClosePublicationReason,
        resumable: bool,
    },
}

/// One captured leaf member of a Workspace Close, reported in the frozen
/// admission order: the exact `(Tab, Panel, Presentation)` triple the
/// publication re-proves, plus its current lifecycle truth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceCloseMember {
    pub tab_id: WorkbenchTabId,
    pub panel_id: WorkbenchPanelId,
    pub presentation_id: TerminalPresentationId,
    pub status: WorkbenchWorkspaceCloseMemberStatus,
}

/// Aggregate member-status histogram accompanying `members` so a reader can
/// budget work without scanning the full list. Counts describe the same
/// response's member entries; each field is a `u32` count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceCloseCounts {
    pub closing: u32,
    pub cleanup_blocked: u32,
    pub outcome_unknown: u32,
    pub disposed_awaiting_publication: u32,
}

/// Read the retained Workspace Close operation for the named
/// stable Workspace scope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceCloseRead {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchWorkspaceCloseReadOutcome {
    /// No retained Workspace Close covers the named scope — including a scope
    /// that never saw one. A completed Close keeps its tombstone, so a scope
    /// removed by publication still answers `available`.
    NotClosing {},
    Denied {},
    Unavailable {
        reason: WorkbenchTabCloseUnavailableReason,
    },
    Available {
        close_request_id: WorkbenchRequestId,
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
        attempt_generation: WorkbenchTabCloseAttemptGeneration,
        status: WorkbenchTabCloseStatus,
        counts: WorkbenchWorkspaceCloseCounts,
        members: Vec<WorkbenchWorkspaceCloseMember>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceCloseReadResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchWorkspaceCloseReadOutcome,
}

impl WorkbenchWorkspaceCloseReadResult {
    /// Wire-level member bound: the captured membership can never exceed the
    /// protocol's per-context leaf Panel limit.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkbenchWorkspaceCloseReadOutcome::Available { members, .. } = &self.outcome
            && members.len() > MAXIMUM_PANEL_NODES_PER_CONTEXT
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

/// Request one fresh lifecycle attempt window for a resumable retained
/// Workspace Close. `close_request_id` names the operation; the generation
/// fence makes a stale copy a settled `stale` answer, never a second attempt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceCloseResume {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub close_request_id: WorkbenchRequestId,
    pub expected_attempt_generation: WorkbenchTabCloseAttemptGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchWorkspaceCloseResumeOutcome {
    Resumed {
        attempt_generation: WorkbenchTabCloseAttemptGeneration,
    },
    Stale {
        attempt_generation: WorkbenchTabCloseAttemptGeneration,
    },
    Denied {},
    Unavailable {
        reason: WorkbenchTabCloseUnavailableReason,
    },
    Rejected {
        reason: WorkbenchTabCloseResumeRejectionReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorkspaceCloseResumeResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub close_request_id: WorkbenchRequestId,
    pub outcome: WorkbenchWorkspaceCloseResumeOutcome,
}
