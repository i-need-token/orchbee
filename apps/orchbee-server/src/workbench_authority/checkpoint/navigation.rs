//! Strict retained navigation without physical Worker authority or historical shapes.
use orchbee_workbench_protocol::{
    Directory, Navigation, Project, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchProjectId, WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId, Workspace,
};
use serde::{Deserialize, Serialize};

use super::WorkbenchCheckpointError;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredNavigation {
    projects: Vec<Project>,
    project_order: Vec<WorkbenchProjectId>,
    directories: Vec<StoredDirectory>,
    workspaces: Vec<Workspace>,
    workspace_order: Vec<WorkbenchWorkspaceId>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredDirectory {
    directory_id: WorkbenchDirectoryId,
    path: WorkbenchDirectoryPath,
}

impl StoredDirectory {
    fn restore(&self, worker: &WorkerId) -> Directory {
        Directory {
            directory_id: self.directory_id.clone(),
            path: self.path.clone(),
            worker_id: worker.clone(),
        }
    }
}

impl StoredNavigation {
    pub(super) fn capture(
        mut navigation: Navigation,
        worker: &WorkerId,
    ) -> Result<Self, WorkbenchCheckpointError> {
        if navigation.workspace_order.is_empty() {
            navigation.workspace_order = navigation
                .workspaces
                .iter()
                .map(|workspace| workspace.workspace_id.clone())
                .collect();
        }
        let directories = navigation
            .directories
            .into_iter()
            .map(|directory| {
                if directory.worker_id != *worker {
                    return Err(WorkbenchCheckpointError::ForeignWorker);
                }
                Ok(StoredDirectory {
                    directory_id: directory.directory_id,
                    path: directory.path,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            projects: navigation.projects,
            project_order: navigation.project_order,
            directories,
            workspaces: navigation.workspaces,
            workspace_order: navigation.workspace_order,
        })
    }

    pub(super) fn restore(
        &self,
        worker: &WorkerId,
    ) -> Result<Navigation, WorkbenchCheckpointError> {
        if self.workspace_order.len() != self.workspaces.len() {
            return Err(WorkbenchCheckpointError::InvalidDocument);
        }
        Ok(Navigation {
            project_order: self.project_order.clone(),
            projects: self.projects.clone(),
            directories: self
                .directories
                .iter()
                .map(|directory| directory.restore(worker))
                .collect(),
            workspaces: self.workspaces.clone(),
            workspace_order: self.workspace_order.clone(),
        })
    }
}
