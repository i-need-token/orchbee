//! Read-only Workspace file and Git facts for the right bar: one directory listing, one file's
//! content from the working tree, the index or `HEAD`, or the Workspace's change list. Paths are
//! Workspace-relative and `/`-separated; the Server resolves the Workspace's own directory, so a
//! client never names an absolute path.
use crate::{ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchWorkspaceId};
use serde::{Deserialize, Serialize};

/// The encoded request stays small: one relative path and a closed operation.
pub const MAXIMUM_WORKSPACE_FILES_REQUEST_BYTES: usize = 16 * 1024;
/// The encoded result fits the general Workbench response budget.
pub const MAXIMUM_WORKSPACE_FILES_RESULT_BYTES: usize = 8 * 1024 * 1024;
/// Workspace-relative path bytes.
pub const MAXIMUM_WORKSPACE_FILE_PATH_BYTES: usize = 4096;
/// One directory entry's name bytes.
pub const MAXIMUM_WORKSPACE_FILE_NAME_BYTES: usize = 1024;
/// Entries one listing or change list carries before it reports truncation.
pub const MAXIMUM_WORKSPACE_FILE_ENTRIES: usize = 5000;
/// Raw text bytes one file read returns; larger files answer `too-large`.
pub const MAXIMUM_WORKSPACE_FILE_TEXT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFileSource {
    Worktree,
    Index,
    Head,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFilesOperation {
    ListDirectory {
        path: String,
    },
    ReadFile {
        path: String,
        source: WorkspaceFileSource,
    },
    GitStatus {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceFilesRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub operation: WorkspaceFilesOperation,
}
impl WorkspaceFilesRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        self.operation.validate()
    }
}
impl WorkspaceFilesOperation {
    pub fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::ListDirectory { path } if path.is_empty() || is_relative_path(path) => Ok(()),
            Self::ReadFile { path, .. } if is_relative_path(path) => Ok(()),
            Self::GitStatus {} => Ok(()),
            Self::ListDirectory { .. } | Self::ReadFile { .. } => {
                Err(ContractError::InvalidDocument)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceEntryKind {
    File,
    Directory,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceDirectoryEntry {
    pub name: String,
    /// What the entry resolves to; a symbolic link reports its target's kind.
    pub entry_kind: WorkspaceEntryKind,
    pub symbolic_link: bool,
    /// Whether the Workspace's Git ignore rules exclude the entry.
    pub ignored: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFileContent {
    Text {
        text: String,
    },
    Binary {},
    TooLarge {},
    /// The source has no such file: an untracked file in `HEAD`, a deleted file in the tree.
    Absent {},
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceGitStatusCode {
    Unmodified,
    Modified,
    TypeChanged,
    Added,
    Deleted,
    Renamed,
    Copied,
    Unmerged,
    Untracked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceGitStatusEntry {
    pub path: String,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub original_path: Option<String>,
    pub index: WorkspaceGitStatusCode,
    pub worktree: WorkspaceGitStatusCode,
    pub conflicted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFilesFailure {
    Denied,
    Unavailable,
    NotFound,
    NotRepository,
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
pub enum WorkspaceFilesOutcome {
    Directory {
        entries: Vec<WorkspaceDirectoryEntry>,
        truncated: bool,
    },
    File {
        byte_length: u64,
        content: WorkspaceFileContent,
    },
    GitStatus {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        branch: Option<String>,
        entries: Vec<WorkspaceGitStatusEntry>,
        truncated: bool,
    },
    Failed {
        reason: WorkspaceFilesFailure,
    },
}
impl WorkspaceFilesOutcome {
    /// The invariants serde cannot express; a producer checks them before it answers.
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match self {
            Self::Directory { entries, .. } => {
                entries.len() <= MAXIMUM_WORKSPACE_FILE_ENTRIES
                    && entries.iter().all(|entry| is_entry_name(&entry.name))
            }
            Self::File {
                byte_length,
                content,
            } => match content {
                WorkspaceFileContent::Text { text } => {
                    text.len() <= MAXIMUM_WORKSPACE_FILE_TEXT_BYTES
                        && u64::try_from(text.len()).ok() == Some(*byte_length)
                }
                WorkspaceFileContent::Absent {} => *byte_length == 0,
                WorkspaceFileContent::Binary {} | WorkspaceFileContent::TooLarge {} => true,
            },
            Self::GitStatus {
                branch, entries, ..
            } => {
                branch.as_deref().is_none_or(is_branch)
                    && entries.len() <= MAXIMUM_WORKSPACE_FILE_ENTRIES
                    && entries.iter().all(|entry| {
                        is_relative_path(&entry.path)
                            && entry.original_path.as_deref().is_none_or(is_relative_path)
                    })
            }
            Self::Failed { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    /// Whether this outcome is a possible answer to `operation`.
    pub fn answers(&self, operation: &WorkspaceFilesOperation) -> bool {
        matches!(
            (self, operation),
            (Self::Failed { .. }, _)
                | (
                    Self::Directory { .. },
                    WorkspaceFilesOperation::ListDirectory { .. }
                )
                | (Self::File { .. }, WorkspaceFilesOperation::ReadFile { .. })
                | (
                    Self::GitStatus { .. },
                    WorkspaceFilesOperation::GitStatus {}
                )
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceFilesResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    pub workspace_id: WorkbenchWorkspaceId,
    pub outcome: WorkspaceFilesOutcome,
}
impl WorkspaceFilesResult {
    pub fn answers(&self, request: &WorkspaceFilesRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.session_id == request.session_id
            && self.workspace_id == request.workspace_id
            && self.outcome.answers(&request.operation)
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        self.outcome.validate()
    }
}

/// A Workspace-relative path: non-empty `/`-separated names, none of them `.` or `..`, no
/// leading or trailing separator, no control character and no backslash.
pub fn is_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAXIMUM_WORKSPACE_FILE_PATH_BYTES
        && path.split('/').all(is_entry_name)
}

fn is_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAXIMUM_WORKSPACE_FILE_NAME_BYTES
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
}

fn is_branch(branch: &str) -> bool {
    !branch.is_empty() && branch.len() <= 1024 && !branch.chars().any(char::is_control)
}
