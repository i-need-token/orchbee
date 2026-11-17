//! Reads whether the Session's terminals are served, the Server is replacing a lost Worker, or it
//! gave up. The read changes no Workbench state; a Client opens it as a State stream, whose first
//! frame is the current state and which sends one frame per later revision.
use crate::{ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalRuntimeRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalRuntimeState {
    /// A Worker serves the terminals.
    Serving,
    /// The Server is replacing a lost Worker; programs keep running and input is refused.
    Reconnecting,
    /// The Server stopped replacing the Worker until the Session stops; programs keep running but
    /// no Client can reach them.
    GivenUp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchTerminalRuntime {
    /// Starts at 1 and advances on each change of the state.
    pub revision: WorkbenchRevision,
    pub state: TerminalRuntimeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalRuntimeFailure {
    Denied,
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
pub enum TerminalRuntimeOutcome {
    Current { runtime: WorkbenchTerminalRuntime },
    Failed { reason: TerminalRuntimeFailure },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalRuntimeResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub outcome: TerminalRuntimeOutcome,
}

impl TerminalRuntimeResult {
    pub fn answers(&self, request: &TerminalRuntimeRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
    }

    pub fn validate(&self) -> Result<(), crate::ContractError> {
        match &self.outcome {
            TerminalRuntimeOutcome::Current { runtime } if runtime.revision.value() == 0 => {
                Err(crate::ContractError::InvalidDocument)
            }
            TerminalRuntimeOutcome::Current { .. } | TerminalRuntimeOutcome::Failed { .. } => {
                Ok(())
            }
        }
    }
}
