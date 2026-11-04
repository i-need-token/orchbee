//! Reads where a Workspace is working: the directory its first terminal, or the terminal a named
//! Panel shows, is in when the request arrives, otherwise the Workspace's own directory. The read
//! changes no Workbench state.
use crate::{
    MAXIMUM_DIRECTORY_PATH_BYTES, ProtocolVersion, SessionId, TerminalPresentationId,
    WorkbenchDirectoryPath, WorkbenchPanelId, WorkbenchRequestId, WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};

/// A longest directory with every byte escaped to six JSON bytes, plus room for its identity.
pub const MAXIMUM_CURRENT_DIRECTORY_BYTES: usize = MAXIMUM_DIRECTORY_PATH_BYTES * 6 + 2048;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentDirectoryRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    /// Absent, the read is about the Workspace's first terminal.
    #[serde(
        default,
        deserialize_with = "present_panel",
        skip_serializing_if = "Option::is_none"
    )]
    pub panel: Option<CurrentDirectoryPanel>,
}

/// A Panel of the Workspace and the Presentation the asking Client shows in it, which is a child
/// terminal while one is displayed. Which Presentation a Panel shows is the Client's choice, so
/// the Client names it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentDirectoryPanel {
    pub panel_id: WorkbenchPanelId,
    pub presentation_id: TerminalPresentationId,
}

fn present_panel<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<CurrentDirectoryPanel>, D::Error> {
    CurrentDirectoryPanel::deserialize(d).map(Some)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CurrentDirectoryFailure {
    Denied,
    NotFound,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CurrentDirectoryOutcome {
    Observed { directory: WorkbenchDirectoryPath },
    Failed { reason: CurrentDirectoryFailure },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentDirectoryResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    #[serde(
        default,
        deserialize_with = "present_panel",
        skip_serializing_if = "Option::is_none"
    )]
    pub panel: Option<CurrentDirectoryPanel>,
    pub outcome: CurrentDirectoryOutcome,
}

impl CurrentDirectoryResult {
    pub fn answers(&self, request: &CurrentDirectoryRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.workspace_id == request.workspace_id
            && self.panel == request.panel
    }
}
