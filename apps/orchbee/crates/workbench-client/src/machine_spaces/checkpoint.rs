//! Settled Workspace observation values: canonical Workspace metadata rows, their last known
//! Git facts, and the checkpoint that fences them. Readers produce them; Client state holds them.
use crate::client_context::ClientContextTarget;
use orchbee_workbench_protocol::{
    GitDivergence, MetadataTokens, SessionId, WorkbenchProjectId, WorkbenchRevision,
};
use std::sync::Arc;

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
    /// The first terminal's observed directory while it differs from `directory`.
    pub(crate) terminal_directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
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
pub(crate) type Observation = crate::machine_observation::outcome::Observation<Checkpoint>;

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
