//! Independent editor intent contains selectors, never executable or document authority.
use crate::{
    ClientPopupSource, ContractError, ProtocolVersion, SiteId, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchTerminalGeometry,
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_TERMINAL_EDITOR_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScrollbackEditorSpec {
    pub source: ClientPopupSource,
}
impl ScrollbackEditorSpec {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.source.member.validate()?;
        if self.source.member.binding().is_none() {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorPopupCapture {
    pub attachment_id: WorkbenchRequestId,
    pub generation: WorkbenchRevision,
    pub revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalEditorAction {
    Open {
        popup_id: WorkbenchRequestId,
        spec: Box<ScrollbackEditorSpec>,
        capture: EditorPopupCapture,
        geometry: WorkbenchTerminalGeometry,
    },
    Read {
        popup_id: WorkbenchRequestId,
    },
    Close {
        popup_id: WorkbenchRequestId,
    },
}
impl TerminalEditorAction {
    #[must_use]
    pub const fn popup_id(&self) -> &WorkbenchRequestId {
        match self {
            Self::Open { popup_id, .. } | Self::Read { popup_id } | Self::Close { popup_id } => {
                popup_id
            }
        }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if let Self::Open { spec, capture, .. } = self {
            spec.validate()?;
            if capture.generation.value() == 0 {
                return Err(ContractError::InvalidDocument);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalEditorRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: TerminalEditorAction,
}
impl TerminalEditorRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.action.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalEditorFailure {
    Denied,
    InvalidIntent,
    SiteMismatch,
    Unavailable,
    Busy,
    Conflict,
    NotFound,
    StaleSource,
    EditorUnavailable,
    PhysicalUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalEditorOutcome {
    Opened {
        popup_id: WorkbenchRequestId,
        title: String,
    },
    State {
        popup_id: WorkbenchRequestId,
        settled: bool,
    },
    Closing {
        popup_id: WorkbenchRequestId,
    },
    Closed {
        popup_id: WorkbenchRequestId,
    },
    Unknown {
        popup_id: WorkbenchRequestId,
    },
    Failed {
        reason: TerminalEditorFailure,
    },
}
impl TerminalEditorOutcome {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let Self::Opened { title, .. } = self
            && (title.is_empty() || title.len() > 4096 || title.contains('\0'))
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TerminalEditorResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: TerminalEditorOutcome,
}
impl TerminalEditorResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.outcome.validate()
    }
    #[must_use]
    pub fn answers(&self, request: &TerminalEditorRequest) -> bool {
        if self.validate().is_err()
            || self.request_id != request.request_id
            || self.site_id != request.site_id
        {
            return false;
        }
        let id = match &self.outcome {
            TerminalEditorOutcome::Opened { popup_id, .. }
                if matches!(request.action, TerminalEditorAction::Open { .. }) =>
            {
                popup_id
            }
            TerminalEditorOutcome::State { popup_id, .. }
                if matches!(request.action, TerminalEditorAction::Read { .. }) =>
            {
                popup_id
            }
            TerminalEditorOutcome::Closing { popup_id }
            | TerminalEditorOutcome::Closed { popup_id }
                if !matches!(request.action, TerminalEditorAction::Open { .. }) =>
            {
                popup_id
            }
            TerminalEditorOutcome::Unknown { popup_id } => popup_id,
            TerminalEditorOutcome::Failed { .. } => return true,
            _ => return false,
        };
        id == request.action.popup_id()
    }
}

#[cfg(test)]
mod tests;
