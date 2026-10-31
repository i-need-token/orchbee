//! Complete canonical Workspace metadata and local Git checkpoints. No paint-time I/O.
use crate::local_client::LocalClient;
use crate::local_client::cancellation::Cancellation;
use crate::machine_observation::{check, request_id, snapshot};
use crate::workbench_projection::{ClientContextLocation, ClientContextTarget};
use orchbee_workbench_protocol::{
    GitDivergence, MAXIMUM_WORKSPACE_METADATA_READ_ROWS, MetadataTokens, ProtocolVersion,
    SessionId, Snapshot, WorkbenchProjectId, WorkbenchRevision, WorkbenchWorkspaceId,
    WorkspaceGitFailure, WorkspaceGitOutcome, WorkspaceGitRequest, WorkspaceMetadataAction,
    WorkspaceMetadataOutcome, WorkspaceMetadataRequest, WorkspaceMetadataRow,
};
use std::{collections::BTreeMap, sync::Arc, time::Instant};
const MAXIMUM_ROWS: usize = 4096;
const MAXIMUM_BYTES: usize = 8 * 1024 * 1024;
mod navigation_projection;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Git {
    pub(crate) branch: Option<String>,
    pub(crate) divergence: Option<GitDivergence>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) target: ClientContextTarget,
    /// Fenced with this checkpoint, independent of the stable terminal target.
    pub(crate) project_id: Option<WorkbenchProjectId>,
    pub(crate) label: Option<String>,
    pub(crate) tokens: MetadataTokens,
    /// Canonical directory identity fences reuse of a last known Git observation.
    pub(crate) directory: Option<orchbee_workbench_protocol::Directory>,
    pub(crate) git: Option<Git>,
    pub(crate) git_stale: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectSummary {
    pub(crate) project_id: WorkbenchProjectId,
    pub(crate) label: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    pub(crate) session_id: SessionId,
    pub(crate) revision: WorkbenchRevision,
    pub(crate) metadata_revision: WorkbenchRevision,
    pub(crate) git_requested: bool,
    pub(crate) rows: Arc<[Row]>,
    pub(crate) projects: Arc<[ProjectSummary]>,
}
pub(crate) type Observation = crate::machine_observation::Observation<Checkpoint>;
pub type Collector = crate::machine_observation::Collector<Checkpoint>;

impl Checkpoint {
    /// Missing reads retain facts only for the same exact Workspace and canonical source.
    pub(crate) fn retaining_git(&self, previous: &Self) -> Self {
        let mut checkpoint = self.clone();
        if self.session_id != previous.session_id
            || self
                .rows
                .iter()
                .all(|row| row.git.is_some() || row.directory.is_none())
        {
            return checkpoint;
        }
        let mut rows = self.rows.to_vec();
        for row in &mut rows {
            if row.git.is_none()
                && row.directory.is_some()
                && let Some(known) = previous
                    .rows
                    .iter()
                    .find(|known| known.target == row.target && known.directory == row.directory)
            {
                row.git.clone_from(&known.git);
                row.git_stale = row.git.is_some();
            }
        }
        checkpoint.rows = rows.into();
        checkpoint
    }
}
struct Reader<'a> {
    client: &'a mut LocalClient,
    before: Snapshot,
    deadline: Instant,
    cancel: Cancellation<'a>,
    bytes: usize,
    metadata_revision: Option<WorkbenchRevision>,
}
impl Reader<'_> {
    fn metadata(&mut self, ids: &[WorkbenchWorkspaceId]) -> Result<Vec<WorkspaceMetadataRow>, ()> {
        check(self.deadline, self.cancel)?;
        let reply = self
            .client
            .workspace_metadata(
                WorkspaceMetadataRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request_id()?,
                    session_id: self.before.session_id.clone(),
                    action: WorkspaceMetadataAction::Read {
                        workspace_ids: ids.to_vec(),
                    },
                },
                self.deadline,
                self.cancel.wake(),
            )
            .map_err(|_| ())?;
        let WorkspaceMetadataOutcome::Read {
            workbench_revision,
            metadata_revision,
            workspaces,
        } = reply.outcome
        else {
            return Err(());
        };
        if workbench_revision != self.before.revision
            || self
                .metadata_revision
                .is_some_and(|held| held != metadata_revision)
            || workspaces.len() != ids.len()
            || workspaces
                .iter()
                .zip(ids)
                .any(|(row, id)| &row.workspace_id != id)
        {
            return Err(());
        }
        self.metadata_revision = Some(metadata_revision);
        self.count(&workspaces)?;
        Ok(workspaces)
    }
    fn count<T: serde::Serialize>(&mut self, value: &T) -> Result<(), ()> {
        self.bytes = self
            .bytes
            .checked_add(serde_json::to_vec(value).map_err(|_| ())?.len())
            .ok_or(())?;
        if self.bytes > MAXIMUM_BYTES {
            Err(())
        } else {
            Ok(())
        }
    }
    fn git(&mut self, id: &WorkbenchWorkspaceId) -> Result<Option<Git>, ()> {
        check(self.deadline, self.cancel)?;
        let reply = self
            .client
            .workspace_git(
                WorkspaceGitRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request_id()?,
                    session_id: self.before.session_id.clone(),
                    workspace_id: id.clone(),
                },
                self.deadline,
                self.cancel.wake(),
            )
            .map_err(|_| ())?;
        self.count(&reply)?;
        match reply.outcome {
            WorkspaceGitOutcome::Observed {
                workbench_revision,
                branch,
                divergence,
            } if workbench_revision == self.before.revision => Ok(Some(Git { branch, divergence })),
            WorkspaceGitOutcome::Failed {
                reason:
                    WorkspaceGitFailure::Unavailable
                    | WorkspaceGitFailure::Timeout
                    | WorkspaceGitFailure::Malformed,
            } => Ok(None),
            WorkspaceGitOutcome::Observed { .. } | WorkspaceGitOutcome::Failed { .. } => Err(()),
        }
    }
    fn read(&mut self, git_requested: bool) -> Result<Arc<Checkpoint>, ()> {
        let ordered: Vec<_> = self
            .before
            .navigation
            .ordered_workspaces()
            .map_err(|_| ())?
            .into_iter()
            .map(|(_, workspace)| workspace.clone())
            .collect();
        if ordered.len() > MAXIMUM_ROWS {
            return Err(());
        }
        let ids: Vec<_> = ordered
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect();
        let mut metadata = BTreeMap::new();
        for chunk in ids.chunks(MAXIMUM_WORKSPACE_METADATA_READ_ROWS) {
            for row in self.metadata(chunk)? {
                if metadata.insert(row.workspace_id, row.tokens).is_some() {
                    return Err(());
                }
            }
        }
        let mut rows = Vec::new();
        for workspace in ordered {
            let directory = self
                .before
                .navigation
                .workspace_directory(&workspace.workspace_id)
                .cloned();
            // The Server observes each Workspace's first terminal directory before its canonical
            // directory, so Workspaces sharing a canonical directory still read separately.
            let git = if git_requested {
                self.git(&workspace.workspace_id)?
            } else {
                None
            };
            rows.push(Row {
                target: ClientContextTarget {
                    session_id: self.before.session_id.clone(),
                    location: ClientContextLocation::Workspace {
                        workspace_id: workspace.workspace_id.clone(),
                        terminal_context_id: workspace.terminal_context_id,
                    },
                },
                project_id: workspace.project_id,
                label: Some(workspace.label),
                tokens: metadata.remove(&workspace.workspace_id).ok_or(())?,
                directory,
                git,
                git_stale: false,
            });
        }
        // An empty read fences expiration/report changes after every Workspace and Git batch.
        if !self.metadata(&[])?.is_empty() || !metadata.is_empty() {
            return Err(());
        }
        let after = snapshot(self.client, self.deadline, self.cancel)?;
        if after != self.before {
            return Err(());
        }
        check(self.deadline, self.cancel)?;
        Ok(Arc::new(Checkpoint {
            session_id: self.before.session_id.clone(),
            revision: self.before.revision,
            metadata_revision: self.metadata_revision.ok_or(())?,
            git_requested,
            rows: rows.into(),
            projects: self
                .before
                .navigation
                .ordered_projects()
                .map_err(|_| ())?
                .into_iter()
                .map(|project| ProjectSummary {
                    project_id: project.project_id.clone(),
                    label: project.label.clone(),
                })
                .collect(),
        }))
    }
}
fn read(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: Cancellation<'_>,
    git_requested: bool,
) -> Result<Arc<Checkpoint>, ()> {
    let before = snapshot(client, deadline, cancel)?;
    let bytes = serde_json::to_vec(&before).map_err(|_| ())?.len();
    Reader {
        client,
        before,
        deadline,
        cancel,
        bytes,
        metadata_revision: None,
    }
    .read(git_requested)
}
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn read_with_git(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<Arc<Checkpoint>, ()> {
    read(client, deadline, cancel, true)
}
#[expect(
    clippy::result_unit_err,
    reason = "application-private boundary whose callers only branch on failure"
)]
pub fn read_metadata(
    client: &mut LocalClient,
    deadline: Instant,
    cancel: Cancellation<'_>,
) -> Result<Arc<Checkpoint>, ()> {
    read(client, deadline, cancel, false)
}
#[cfg(test)]
mod tests;
