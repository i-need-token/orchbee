//! Canonical source membership and the post-Git Workspace association join.
use super::{WorkbenchAuthority, WorktreeSourceTarget};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryPath, WorkerId, WorktreeEntry, WorktreeEntryText,
    WorktreeListUnavailableReason, WorktreeQueryEntry, WorktreeQueryFailure as Failure,
    WorktreeSource, WorktreeWorkspaceAssociation,
};

impl WorkbenchAuthority {
    pub fn worktree_query_target(
        &self,
        source: &WorktreeSource,
        worker: &WorkerId,
    ) -> Result<WorktreeSourceTarget, Failure> {
        if self.retired {
            return Err(Failure::Unavailable {
                reason: WorktreeListUnavailableReason::ServerBusy,
            });
        }
        let (workspace_id, stands_in) = match source {
            WorktreeSource::Directory { path } => {
                return Ok(WorktreeSourceTarget {
                    directory_id: None,
                    path: path.clone(),
                    worker_id: worker.clone(),
                });
            }
            WorktreeSource::Workspace { workspace_id } => (workspace_id, None),
            WorktreeSource::WorkspaceDirectory { workspace_id, path } => (workspace_id, Some(path)),
        };
        let workspace = self
            .snapshot()
            .navigation
            .workspace(workspace_id)
            .ok_or(Failure::NotFound {})?;
        if self.open_close_fences_context(&workspace.terminal_context_id) {
            return Err(Failure::Unavailable {
                reason: WorktreeListUnavailableReason::ServerBusy,
            });
        }
        if let Some(path) = stands_in {
            return self
                .workspace_directory_source(workspace_id, path, worker)
                .ok_or(Failure::Unavailable {
                    reason: WorktreeListUnavailableReason::WorkerUnavailable,
                });
        }
        let directory = self
            .snapshot()
            .navigation
            .workspace_directory(workspace_id)
            .ok_or(Failure::NotFound {})?;
        if directory.worker_id != *worker {
            return Err(Failure::Unavailable {
                reason: WorktreeListUnavailableReason::WorkerUnavailable,
            });
        }
        Ok(WorktreeSourceTarget {
            directory_id: Some(directory.directory_id.clone()),
            path: directory.path.clone(),
            worker_id: worker.clone(),
        })
    }

    /// Called under the same gate as the final source membership recheck.
    pub fn worktree_query_entries(
        &self,
        worker: &WorkerId,
        source: &WorktreeSource,
        source_checkout: &WorkbenchDirectoryPath,
        entries: Vec<WorktreeEntry>,
    ) -> Result<Vec<WorktreeQueryEntry>, Failure> {
        let source_workspace = match source {
            WorktreeSource::Workspace { workspace_id }
            | WorktreeSource::WorkspaceDirectory { workspace_id, .. } => Some(workspace_id),
            WorktreeSource::Directory { .. } => None,
        };
        entries
            .into_iter()
            .enumerate()
            .map(|(index, checkout)| {
                let WorktreeEntryText::Utf8(path) = &checkout.path else {
                    return Err(Failure::MalformedListing {});
                };
                let path = WorkbenchDirectoryPath::parse(path)
                    .map_err(|_| Failure::MalformedListing {})?;
                let known = self.checkout_workspaces(&path, worker);
                let workspaces = self
                    .snapshot()
                    .navigation
                    .workspaces
                    .iter()
                    .filter_map(|workspace| {
                        let explicit_source = source_workspace == Some(&workspace.workspace_id)
                            && orchbee_workbench_protocol::same_directory_spelling(
                                path.as_str(),
                                source_checkout.as_str(),
                            );
                        (explicit_source || known.contains(&workspace.workspace_id)).then(|| {
                            WorktreeWorkspaceAssociation {
                                project_id: workspace.project_id.clone(),
                                workspace_id: workspace.workspace_id.clone(),
                                label: orchbee_workbench_protocol::WorkbenchLabel::parse(
                                    &workspace.label,
                                )
                                .expect("validated Workspace label"),
                            }
                        })
                    })
                    .collect();
                Ok(WorktreeQueryEntry {
                    checkout,
                    linked: index != 0,
                    workspaces,
                })
            })
            .collect()
    }
}
