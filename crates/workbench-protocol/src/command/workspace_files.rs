//! Workspace file and Git facts for the right bar: one directory listing, one chunk of a file's
//! content from the working tree, the index or `HEAD`, or the Workspace's change list; or, as a
//! State stream, how far what the right bar shows has changed since. One operation writes: it
//! saves a working-tree file only while the file is still the version the client read. Paths are
//! Workspace-relative and `/`-separated; the Server resolves the Workspace's own directory, so a
//! client never names an absolute path.
use crate::{
    ContractError, ProtocolVersion, SessionId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchWorkspaceId,
};
use serde::{Deserialize, Serialize};

/// The encoded request stays small: one relative path and a closed operation, or the paths a
/// watch follows.
pub const MAXIMUM_WORKSPACE_FILES_REQUEST_BYTES: usize = 64 * 1024;
/// An encoded `write-file` request: the largest editable text, room for its JSON escapes, and the
/// identity around it. A client checks the encoded size before it asks.
pub const MAXIMUM_WORKSPACE_FILES_WRITE_REQUEST_BYTES: usize = 24 * 1024 * 1024;
/// The encoded result fits the general Workbench response budget.
pub const MAXIMUM_WORKSPACE_FILES_RESULT_BYTES: usize = 8 * 1024 * 1024;
/// Workspace-relative path bytes.
pub const MAXIMUM_WORKSPACE_FILE_PATH_BYTES: usize = 4096;
/// One directory entry's name bytes.
pub const MAXIMUM_WORKSPACE_FILE_NAME_BYTES: usize = 1024;
/// Entries one listing or change list carries before it reports truncation.
pub const MAXIMUM_WORKSPACE_FILE_ENTRIES: usize = 5000;
/// Content bytes one file read returns; a client reads a larger file chunk by chunk.
pub const WORKSPACE_FILE_CHUNK_BYTES: usize = 256 * 1024;
/// The largest working-tree file that opens as text at all; larger ones answer `too-large`.
pub const MAXIMUM_WORKSPACE_FILE_OPEN_BYTES: u64 = 50 * 1024 * 1024;
/// The largest file that carries a content hash in its version, opens for editing, or reads from
/// the index or `HEAD` for a diff.
pub const MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES: u64 = 10 * 1024 * 1024;
/// The largest file that reads as bytes, such as an image to show.
pub const MAXIMUM_WORKSPACE_FILE_BYTES_READ: u64 = 10 * 1024 * 1024;
/// Directories one watch names: the ones the right bar shows expanded.
pub const MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES: usize = 256;
/// Files one watch names: the ones open from the right bar.
pub const MAXIMUM_WORKSPACE_FILES_WATCHED_FILES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFileSource {
    Worktree,
    Index,
    Head,
}

/// How a read returns content: text that must be UTF-8, or bytes in base64, such as an image.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFileEncoding {
    Text,
    Bytes,
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
    /// One chunk of a file from `offset`, a byte position a previous chunk named.
    ReadFile {
        path: String,
        source: WorkspaceFileSource,
        offset: u64,
        encoding: WorkspaceFileEncoding,
    },
    GitStatus {},
    /// Follows changes as a State stream: each frame carries the Workspace's revisions, and the
    /// Server sends a new frame when one moves. `directories` are the ones shown, the empty path
    /// naming the Workspace itself, and `files` the ones open in the working tree. A platform that
    /// watches recursively also sees changes beneath directories nobody shows, which move only
    /// `repository`.
    Watch {
        directories: Vec<String>,
        files: Vec<String>,
    },
    /// Replaces a working-tree file's whole content with `text`, but only while the file is still
    /// the one the client read: `expected` is that read's SHA-256, or null for a file that must
    /// not exist yet. Any other file answers `conflict` and is left as it was.
    WriteFile {
        path: String,
        text: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        expected: Option<String>,
    },
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
            Self::Watch { directories, files }
                if directories.len() <= MAXIMUM_WORKSPACE_FILES_WATCHED_DIRECTORIES
                    && files.len() <= MAXIMUM_WORKSPACE_FILES_WATCHED_FILES
                    && directories
                        .iter()
                        .all(|path| path.is_empty() || is_relative_path(path))
                    && files.iter().all(|path| is_relative_path(path)) =>
            {
                Ok(())
            }
            Self::WriteFile {
                path,
                text,
                expected,
            } if is_relative_path(path)
                && text.len() as u64 <= MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES
                && expected.as_deref().is_none_or(is_sha256) =>
            {
                Ok(())
            }
            Self::ListDirectory { .. }
            | Self::ReadFile { .. }
            | Self::Watch { .. }
            | Self::WriteFile { .. } => Err(ContractError::InvalidDocument),
        }
    }
    /// Whether the operation changes the Workspace, which needs more than reading permission.
    #[must_use]
    pub fn writes(&self) -> bool {
        matches!(self, Self::WriteFile { .. })
    }
    /// The encoded size a request carrying this operation may reach.
    #[must_use]
    pub fn request_byte_limit(&self) -> usize {
        if self.writes() {
            MAXIMUM_WORKSPACE_FILES_WRITE_REQUEST_BYTES
        } else {
            MAXIMUM_WORKSPACE_FILES_REQUEST_BYTES
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
    /// UTF-8 text from `offset`, ending on a character boundary; `next_offset` names where the
    /// next chunk starts, or is null at the end of the file.
    Text {
        offset: u64,
        text: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        next_offset: Option<u64>,
    },
    /// Bytes from `offset` in standard padded base64.
    Bytes {
        offset: u64,
        base64: String,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        next_offset: Option<u64>,
    },
    Binary {},
    TooLarge {},
    /// The source has no such file: an untracked file in `HEAD`, a deleted file in the tree.
    Absent {},
}

/// What a working-tree file was when it was read, so a later write can refuse to replace a newer
/// one and a chunked read can notice the file changing under it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceFileVersion {
    /// Modification time in nanoseconds since the Unix epoch, in decimal; `0` when unknown.
    pub modified: String,
    /// Lowercase hexadecimal SHA-256 of the whole file; only the first chunk of a file within
    /// [`MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES`] carries it.
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub sha256: Option<String>,
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

/// Whether a watch follows changes. A client that reads `unavailable` refreshes only when asked.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceFilesWatchState {
    /// The Server has asked to watch and has no answer yet.
    Starting,
    Watching,
    /// The platform refused, or the Workspace holds more than the platform can watch.
    Unavailable,
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
        /// The whole file's size, whichever chunk this is.
        byte_length: u64,
        /// Present for a working-tree file that exists.
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        version: Option<WorkspaceFileVersion>,
        content: WorkspaceFileContent,
    },
    GitStatus {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        branch: Option<String>,
        entries: Vec<WorkspaceGitStatusEntry>,
        truncated: bool,
    },
    /// A watch's state. `files` moves when an entry of a shown directory, or an open file,
    /// changes; `repository` moves when anything in the working tree changes, or the repository's
    /// index, `HEAD` or references do. Both start again whenever the Server starts following the
    /// Workspace, so only a value that differs from the last frame means anything.
    Watching {
        files: WorkbenchRevision,
        repository: WorkbenchRevision,
        state: WorkspaceFilesWatchState,
    },
    /// The file now holds the written text; `version` is what a later write expects, and always
    /// carries its SHA-256.
    Written {
        version: WorkspaceFileVersion,
    },
    /// The file is no longer the version the write expected, so nothing was written; `version` is
    /// what it is now, or null when no file is there.
    Conflict {
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        version: Option<WorkspaceFileVersion>,
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
                version,
                content,
            } => {
                version.as_ref().is_none_or(is_version)
                    && match content {
                        WorkspaceFileContent::Text {
                            offset,
                            text,
                            next_offset,
                        } => is_chunk(*byte_length, *offset, text.len(), *next_offset),
                        WorkspaceFileContent::Bytes {
                            offset,
                            base64,
                            next_offset,
                        } => base64_length(base64).is_some_and(|length| {
                            is_chunk(*byte_length, *offset, length, *next_offset)
                        }),
                        WorkspaceFileContent::Absent {} => *byte_length == 0 && version.is_none(),
                        WorkspaceFileContent::Binary {} | WorkspaceFileContent::TooLarge {} => true,
                    }
            }
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
            Self::Written { version } => is_version(version) && version.sha256.is_some(),
            Self::Conflict { version } => version.as_ref().is_none_or(is_version),
            Self::Watching { .. } | Self::Failed { .. } => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    /// Whether this outcome is a possible answer to `operation`.
    pub fn answers(&self, operation: &WorkspaceFilesOperation) -> bool {
        if let (
            Self::File { content, .. },
            WorkspaceFilesOperation::ReadFile {
                offset, encoding, ..
            },
        ) = (self, operation)
        {
            return match content {
                WorkspaceFileContent::Text { offset: from, .. } => {
                    *encoding == WorkspaceFileEncoding::Text && from == offset
                }
                WorkspaceFileContent::Bytes { offset: from, .. } => {
                    *encoding == WorkspaceFileEncoding::Bytes && from == offset
                }
                WorkspaceFileContent::Binary {}
                | WorkspaceFileContent::TooLarge {}
                | WorkspaceFileContent::Absent {} => true,
            };
        }
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
                | (Self::Watching { .. }, WorkspaceFilesOperation::Watch { .. })
                | (
                    Self::Written { .. } | Self::Conflict { .. },
                    WorkspaceFilesOperation::WriteFile { .. }
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

/// A chunk of `length` bytes at `offset` within a file of `byte_length`: at most one chunk long,
/// and either the end of the file or followed by the next chunk exactly where it stops.
fn is_chunk(byte_length: u64, offset: u64, length: usize, next_offset: Option<u64>) -> bool {
    let Ok(length) = u64::try_from(length) else {
        return false;
    };
    let Some(end) = offset.checked_add(length) else {
        return false;
    };
    length <= WORKSPACE_FILE_CHUNK_BYTES as u64
        && end <= byte_length
        && match next_offset {
            None => end == byte_length,
            Some(next) => next == end && length > 0 && next < byte_length,
        }
}

/// The byte count standard padded base64 decodes to, or none when it is not that encoding.
fn base64_length(encoded: &str) -> Option<usize> {
    let bytes = encoded.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    let body = &bytes[..bytes.len() - padding];
    if padding > 2
        || !body
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/')
    {
        return None;
    }
    Some(bytes.len() / 4 * 3 - padding)
}

fn is_version(version: &WorkspaceFileVersion) -> bool {
    !version.modified.is_empty()
        && version.modified.len() <= 39
        && version.modified.bytes().all(|byte| byte.is_ascii_digit())
        && version.sha256.as_deref().is_none_or(is_sha256)
}

/// Lowercase hexadecimal SHA-256.
fn is_sha256(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_branch(branch: &str) -> bool {
    !branch.is_empty() && branch.len() <= 1024 && !branch.chars().any(char::is_control)
}
