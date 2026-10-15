//! Versioned retained navigation. Historical ownership is checked before normalization.
use orchbee_workbench_protocol::{
    Directory, MAXIMUM_DIRECTORIES_PER_PROJECT, MAXIMUM_WORKSPACES_PER_PROJECT, Navigation,
    Project, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchProjectId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceId, WorkerId, Workspace,
};
use serde::{Deserialize, Serialize};

use super::WorkbenchCheckpointError;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum StoredNavigation {
    Current(CurrentNavigation),
    Historical(HistoricalNavigation),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentNavigation {
    standalone_terminal_context_id: WorkbenchTerminalContextId,
    projects: Vec<Project>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    project_order: Vec<WorkbenchProjectId>,
    directories: Vec<StoredDirectory>,
    workspaces: Vec<Workspace>,
    workspace_order: Vec<WorkbenchWorkspaceId>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct HistoricalNavigation {
    standalone_terminal_context_id: WorkbenchTerminalContextId,
    projects: Vec<HistoricalProject>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    workspace_order: Vec<WorkbenchWorkspaceId>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoricalProject {
    project_id: WorkbenchProjectId,
    label: String,
    directories: Vec<StoredDirectory>,
    workspaces: Vec<HistoricalWorkspace>,
}

// Keep this DTO separate from the runtime Workspace: versions 1–5 never stored projectId.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistoricalWorkspace {
    label: String,
    #[serde(deserialize_with = "required_nullable")]
    primary_directory_id: Option<WorkbenchDirectoryId>,
    terminal_context_id: WorkbenchTerminalContextId,
    workspace_id: WorkbenchWorkspaceId,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
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
        Ok(Self::Current(CurrentNavigation {
            standalone_terminal_context_id: navigation.standalone_terminal_context_id,
            projects: navigation.projects,
            project_order: navigation.project_order,
            directories,
            workspaces: navigation.workspaces,
            workspace_order: navigation.workspace_order,
        }))
    }

    pub(super) fn restore(
        &self,
        version: u32,
        worker: &WorkerId,
    ) -> Result<Navigation, WorkbenchCheckpointError> {
        match (self, version) {
            (Self::Current(stored), 6 | 7)
                if stored.workspace_order.len() == stored.workspaces.len()
                    && (version == 7 || stored.project_order.is_empty()) =>
            {
                Ok(Navigation {
                    project_order: stored.project_order.clone(),
                    standalone_terminal_context_id: stored.standalone_terminal_context_id.clone(),
                    projects: stored.projects.clone(),
                    directories: stored
                        .directories
                        .iter()
                        .map(|d| d.restore(worker))
                        .collect(),
                    workspaces: stored.workspaces.clone(),
                    workspace_order: stored.workspace_order.clone(),
                })
            }
            (Self::Historical(stored), 1..=5) => stored.restore(worker),
            _ => Err(WorkbenchCheckpointError::InvalidDocument),
        }
    }
}

impl HistoricalNavigation {
    fn restore(&self, worker: &WorkerId) -> Result<Navigation, WorkbenchCheckpointError> {
        let mut navigation = Navigation {
            project_order: Vec::new(),
            standalone_terminal_context_id: self.standalone_terminal_context_id.clone(),
            projects: Vec::new(),
            directories: Vec::new(),
            workspaces: Vec::new(),
            workspace_order: self.workspace_order.clone(),
        };
        for project in &self.projects {
            if project.directories.len() > MAXIMUM_DIRECTORIES_PER_PROJECT
                || project.workspaces.len() > MAXIMUM_WORKSPACES_PER_PROJECT
                || !project
                    .directories
                    .windows(2)
                    .all(|w| w[0].directory_id < w[1].directory_id)
                || !project
                    .workspaces
                    .windows(2)
                    .all(|w| w[0].workspace_id < w[1].workspace_id)
                || project.workspaces.iter().any(|workspace| {
                    workspace.primary_directory_id.as_ref().is_some_and(|id| {
                        !project
                            .directories
                            .iter()
                            .any(|directory| directory.directory_id == *id)
                    })
                })
            {
                return Err(WorkbenchCheckpointError::InvalidDocument);
            }
            navigation.projects.push(Project {
                project_id: project.project_id.clone(),
                label: project.label.clone(),
            });
            navigation
                .directories
                .extend(project.directories.iter().map(|d| d.restore(worker)));
            for workspace in &project.workspaces {
                navigation.workspaces.push(Workspace {
                    project_id: Some(project.project_id.clone()),
                    workspace_id: workspace.workspace_id.clone(),
                    label: workspace.label.clone(),
                    primary_directory_id: workspace.primary_directory_id.clone(),
                    terminal_context_id: workspace.terminal_context_id.clone(),
                });
            }
        }
        // An omitted historical permutation meant Project traversal, not global identity order.
        if navigation.workspace_order.is_empty() {
            navigation.workspace_order = navigation
                .workspaces
                .iter()
                .map(|w| w.workspace_id.clone())
                .collect();
        }
        navigation
            .directories
            .sort_by(|a, b| a.directory_id.cmp(&b.directory_id));
        navigation
            .workspaces
            .sort_by(|a, b| a.workspace_id.cmp(&b.workspace_id));
        Ok(navigation)
    }
}
