//! Defines bounded worktree-list reads and outcomes with validated lossless path-text
//! representations and association facts. It does not invoke Git, canonicalize filesystem
//! identity, or mutate Workbench directory membership.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{
    ContractError, ProtocolVersion, SessionId, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchRequestId, WorkbenchRevision,
};

/// The dedicated encoded bound for this response variant, enforced on both
/// parse and serialize beneath the outer frame cap — deliberately not the
/// 16 KiB intent class and not the snapshot class.
pub const MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorktreeListRead {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub directory_id: WorkbenchDirectoryId,
}

/// One observed Git worktree entry. The `worktreePath`/`worktreePathBytesHex`
/// and `branch`/`branchBytesHex` pairs are semantic-exclusive: a non-UTF-8
/// value is only ever published in its hex form, never lossy-decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeEntry {
    pub path: WorktreeEntryText,
    /// 40- or 64-digit lowercase hex; `None` exactly when `bare`.
    pub head: Option<String>,
    /// Ref verbatim (`refs/heads/…`) or raw bytes; `None` when bare or
    /// detached.
    pub branch: Option<WorktreeEntryText>,
    pub bare: bool,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeEntryText {
    Utf8(String),
    Bytes(Vec<u8>),
}

impl WorktreeEntry {
    /// Structural invariants the Git parser must already satisfy; a violation
    /// is malformed Git data, not a partial entry.
    pub fn validate(&self) -> Result<(), ContractError> {
        let path_ok = match &self.path {
            WorktreeEntryText::Utf8(path) => {
                crate::directory::is_absolute_path(path.as_bytes()) && !path.contains('\0')
            }
            WorktreeEntryText::Bytes(bytes) => {
                crate::directory::is_absolute_path(bytes) && !bytes.contains(&0)
            }
        };
        if !path_ok {
            return Err(ContractError::InvalidDocument);
        }
        match &self.head {
            Some(head) => {
                let valid = matches!(head.len(), 40 | 64)
                    && head
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
                if !valid {
                    return Err(ContractError::InvalidDocument);
                }
            }
            None => {
                if !self.bare {
                    return Err(ContractError::InvalidDocument);
                }
            }
        }
        if self.bare && (self.head.is_some() || self.branch.is_some() || self.detached) {
            return Err(ContractError::InvalidDocument);
        }
        if self.detached && self.branch.is_some() {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeListStage {
    Probe,
    List,
}

/// Which bound a `too-large` outcome reports; the CLI must never guess which
/// limit failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeListTooLargeOrigin {
    GitProbe,
    GitList,
    WorkerResult,
    WorkbenchResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreeListUnavailableReason {
    ServerBusy,
    WorkerUnavailable,
    JobDead,
    JobUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchWorktreeListOutcome {
    Found {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
        workbench_revision: WorkbenchRevision,
        directory_id: WorkbenchDirectoryId,
        /// The registered workbench directory path — distinct from the
        /// observed Git `worktreePath` values inside `entries`.
        path: WorkbenchDirectoryPath,
        entries: Vec<WorktreeEntry>,
    },
    NotFound {},
    Denied {},
    Unavailable {
        reason: WorktreeListUnavailableReason,
    },
    GitFailed {
        stage: WorktreeListStage,
        #[serde(default, deserialize_with = "super::json_fields::deserialize_i32")]
        status: Option<i32>,
    },
    SpawnUnavailable {
        stage: WorktreeListStage,
    },
    SpawnFailed {
        stage: WorktreeListStage,
    },
    TooLarge {
        origin: WorktreeListTooLargeOrigin,
    },
    Timeout {},
    OwnerIo {
        stage: Option<WorktreeListStage>,
    },
    TransportLoss {},
    MalformedListing {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorktreeListResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchWorktreeListOutcome,
}

impl WorkbenchWorktreeListResult {
    /// Entry structural validation is part of the encoded contract; the
    /// caller pairs this with the explicit variant byte cap.
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkbenchWorktreeListOutcome::Found { entries, .. } = &self.outcome {
            for entry in entries {
                entry.validate()?;
            }
        }
        Ok(())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble"));
        encoded.push(char::from_digit(u32::from(byte & 0x0f), 16).expect("nibble"));
    }
    encoded
}

fn hex_decode(encoded: &str) -> Result<Vec<u8>, ContractError> {
    if !encoded.len().is_multiple_of(2)
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ContractError::InvalidDocument);
    }
    Ok(encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            (pair[0] as char).to_digit(16).expect("hex") as u8 * 16
                + (pair[1] as char).to_digit(16).expect("hex") as u8
        })
        .collect())
}

impl Serialize for WorktreeEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match &self.path {
            WorktreeEntryText::Utf8(path) => map.serialize_entry("worktreePath", path)?,
            WorktreeEntryText::Bytes(bytes) => {
                map.serialize_entry("worktreePathBytesHex", &hex_encode(bytes))?;
            }
        }
        map.serialize_entry("head", &self.head)?;
        match &self.branch {
            Some(WorktreeEntryText::Utf8(branch)) => {
                map.serialize_entry("branch", branch)?;
            }
            Some(WorktreeEntryText::Bytes(bytes)) => {
                map.serialize_entry("branchBytesHex", &hex_encode(bytes))?;
            }
            None => map.serialize_entry("branch", &Option::<String>::None)?,
        }
        map.serialize_entry("bare", &self.bare)?;
        map.serialize_entry("detached", &self.detached)?;
        map.serialize_entry("locked", &self.locked)?;
        map.serialize_entry("prunable", &self.prunable)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for WorktreeEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Raw {
            #[serde(default, rename = "worktreePath")]
            worktree_path: Option<String>,
            #[serde(default)]
            worktree_path_bytes_hex: Option<String>,
            head: Option<String>,
            #[serde(default)]
            branch: Option<String>,
            #[serde(default)]
            branch_bytes_hex: Option<String>,
            bare: bool,
            detached: bool,
            locked: bool,
            prunable: bool,
        }
        let raw = Raw::deserialize(deserializer)?;
        let invalid = || de::Error::custom("invalid worktree entry");
        let path = match (raw.worktree_path, raw.worktree_path_bytes_hex) {
            (Some(path), None) => WorktreeEntryText::Utf8(path),
            (None, Some(hex)) => WorktreeEntryText::Bytes(hex_decode(&hex).map_err(|_| invalid())?),
            _ => return Err(invalid()),
        };
        let branch = match (raw.branch, raw.branch_bytes_hex) {
            (Some(branch), None) => Some(WorktreeEntryText::Utf8(branch)),
            (None, Some(hex)) => Some(WorktreeEntryText::Bytes(
                hex_decode(&hex).map_err(|_| invalid())?,
            )),
            (None, None) => None,
            (Some(_), Some(_)) => return Err(invalid()),
        };
        let entry = Self {
            path,
            head: raw.head,
            branch,
            bare: raw.bare,
            detached: raw.detached,
            locked: raw.locked,
            prunable: raw.prunable,
        };
        entry.validate().map_err(|_| invalid())?;
        Ok(entry)
    }
}
