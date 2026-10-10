//! Read-only source discovery, with canonical Workspace associations observed after Git.
use super::json_fields::{deserialize_i32, deserialize_nullable};
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchProjectId, WorkbenchRequestId, WorkbenchRevision, WorkbenchWorkspaceId, WorkerId,
    WorktreeEntry, WorktreeEntryText, WorktreeListStage, WorktreeListTooLargeOrigin,
    WorktreeListUnavailableReason, WorktreeSource,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorktreeQuery {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub source: WorktreeSource,
    pub trust_repository: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorktreeQuerySource {
    pub requested_path: WorkbenchDirectoryPath,
    pub repository_root: WorkbenchDirectoryPath,
    pub common_directory: WorkbenchDirectoryPath,
    pub checkout_path: WorkbenchDirectoryPath,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub workspace_id: Option<WorkbenchWorkspaceId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorktreeWorkspaceAssociation {
    #[serde(deserialize_with = "deserialize_nullable")]
    pub project_id: Option<WorkbenchProjectId>,
    pub workspace_id: WorkbenchWorkspaceId,
    pub label: WorkbenchLabel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorktreeQueryEntry {
    pub checkout: WorktreeEntry,
    pub linked: bool,
    pub workspaces: Vec<WorktreeWorkspaceAssociation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorktreeQueryFailure {
    NotFound {},
    Denied {},
    Unsupported {},
    SourceChanged {},
    Unavailable {
        reason: WorktreeListUnavailableReason,
    },
    GitFailed {
        stage: WorktreeListStage,
        #[serde(deserialize_with = "deserialize_i32")]
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
        #[serde(deserialize_with = "deserialize_nullable")]
        stage: Option<WorktreeListStage>,
    },
    TransportLoss {},
    MalformedListing {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkbenchWorktreeQueryOutcome {
    Found {
        site_id: SiteId,
        workbench_revision: WorkbenchRevision,
        worker_id: WorkerId,
        source: WorktreeQuerySource,
        entries: Vec<WorktreeQueryEntry>,
    },
    Failed {
        reason: WorktreeQueryFailure,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchWorktreeQueryResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: WorkbenchWorktreeQueryOutcome,
}

impl WorkbenchWorktreeQueryResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        let WorkbenchWorktreeQueryOutcome::Found {
            source, entries, ..
        } = &self.outcome
        else {
            return Ok(());
        };
        let invalid = ContractError::InvalidDocument;
        let Some(first) = entries.first() else {
            return Err(invalid);
        };
        if first.checkout.path
            != WorktreeEntryText::Utf8(source.repository_root.as_str().to_owned())
        {
            return Err(invalid);
        }
        let mut paths = BTreeSet::new();
        let mut workspaces = BTreeSet::new();
        let mut source_found = false;
        for (index, entry) in entries.iter().enumerate() {
            entry.checkout.validate()?;
            if !entry.checkout.bare && entry.checkout.detached == entry.checkout.branch.is_some() {
                return Err(invalid);
            }
            if let Some(branch) = &entry.checkout.branch {
                let WorktreeEntryText::Utf8(branch) = branch else {
                    return Err(invalid);
                };
                if !branch.starts_with("refs/heads/") || branch.len() == 11 || branch.contains('\0')
                {
                    return Err(invalid);
                }
            }
            // Source discovery is a Windows UTF-8 drive-path contract. The
            // legacy list continues to preserve arbitrary path bytes separately.
            let WorktreeEntryText::Utf8(path) = &entry.checkout.path else {
                return Err(invalid);
            };
            WorkbenchDirectoryPath::parse(path).map_err(|_| ContractError::InvalidDocument)?;
            if !paths.insert(path)
                || entry.linked != (index != 0)
                || (index != 0 && entry.checkout.bare)
            {
                return Err(invalid);
            }
            if path == source.checkout_path.as_str() {
                if entry.checkout.bare
                    || source.workspace_id.as_ref().is_some_and(|id| {
                        !entry
                            .workspaces
                            .iter()
                            .any(|workspace| &workspace.workspace_id == id)
                    })
                {
                    return Err(invalid);
                }
                source_found = true;
            }
            for workspace in &entry.workspaces {
                if !workspaces.insert(&workspace.workspace_id) {
                    return Err(invalid);
                }
            }
        }
        if !source_found {
            return Err(invalid);
        }
        Ok(())
    }
}
