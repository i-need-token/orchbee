//! Strict retained navigation without physical Worker authority or historical shapes.
use std::collections::BTreeMap;

use orchbee_workbench_protocol::{
    Directory, Navigation, Project, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchProjectId, WorkbenchWorkspaceId, WorkerId, Workspace,
};
use serde::{Deserialize, Serialize};

use super::super::{identity, project_identity_above};
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

    /// The checkpoint step to random Project identities: each Project takes a fresh one, rising in
    /// the registry's identity order so that order and an implicit display order stay as they were,
    /// and the Project order and every member Workspace follow it.
    pub(super) fn give_projects_random_identities(
        &mut self,
        seed: &[u8],
    ) -> Result<(), WorkbenchCheckpointError> {
        let mut given = BTreeMap::new();
        let mut last = 0;
        for (index, project) in (1..).zip(&mut self.projects) {
            last = project_identity_above(last, seed, index)
                .map_err(|_failure| WorkbenchCheckpointError::InvalidIdentity)?;
            let fresh = WorkbenchProjectId::parse(&identity(last))
                .map_err(|_error| WorkbenchCheckpointError::InvalidIdentity)?;
            given.insert(project.project_id.clone(), fresh.clone());
            project.project_id = fresh;
        }
        let follow = |project_id: &mut WorkbenchProjectId| {
            given
                .get(project_id)
                .cloned()
                .map(|fresh| *project_id = fresh)
                .ok_or(WorkbenchCheckpointError::InvalidDocument)
        };
        self.project_order.iter_mut().try_for_each(follow)?;
        self.workspaces
            .iter_mut()
            .filter_map(|workspace| workspace.project_id.as_mut())
            .try_for_each(follow)
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
