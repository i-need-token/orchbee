//! Retained close identity lookup, independent of surviving layout membership.
use crate::{
    ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabCloseStatus, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloseOperationRead {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub operation_id: WorkbenchRequestId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CloseOperationTarget {
    Workspace {
        workspace_id: WorkbenchWorkspaceId,
    },
    Tab {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
    Panel {
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CloseOperationOutcome {
    NotFound {},
    Denied {},
    Unavailable {},
    Available {
        target: CloseOperationTarget,
        generation: WorkbenchTabCloseAttemptGeneration,
        status: WorkbenchTabCloseStatus,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloseOperationResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub operation_id: WorkbenchRequestId,
    pub outcome: CloseOperationOutcome,
}

impl CloseOperationResult {
    #[must_use]
    pub fn answers(&self, request: &CloseOperationRead) -> bool {
        self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.operation_id == request.operation_id
    }
}
