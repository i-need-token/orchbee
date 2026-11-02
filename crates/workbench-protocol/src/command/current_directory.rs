//! Reads where a Workspace is working: the directory its first terminal is in when the request
//! arrives, otherwise the Workspace's own directory. The read changes no Workbench state.
use crate::{
    MAXIMUM_DIRECTORY_PATH_BYTES, ProtocolVersion, SessionId, WorkbenchDirectoryPath,
    WorkbenchRequestId, WorkbenchWorkspaceId,
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
    pub outcome: CurrentDirectoryOutcome,
}

impl CurrentDirectoryResult {
    pub fn answers(&self, request: &CurrentDirectoryRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.workspace_id == request.workspace_id
    }
}
