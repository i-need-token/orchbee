//! Identity-based resource lookup, independent of mutable Project organization.
use crate::{
    Directory, Navigation, Project, WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchLabel,
    WorkbenchProjectId, WorkbenchWorkspaceId, WorkerId, Workspace, same_directory_spelling,
};
use serde::{Deserialize, Serialize};

/// Organization committed with a new Workspace and its first terminal structure.
/// `Alongside` resolves the source's current group at commit, never a stale UI label.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspacePlacement {
    Ungrouped {},
    Project {
        project_id: WorkbenchProjectId,
    },
    NewProject {
        label: WorkbenchLabel,
    },
    Alongside {
        workspace_id: WorkbenchWorkspaceId,
        project_label: WorkbenchLabel,
    },
}

impl Navigation {
    /// Look up an optional grouping without granting resource or lifecycle authority.
    #[must_use]
    pub fn project(&self, project_id: &WorkbenchProjectId) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| &project.project_id == project_id)
    }

    /// Workspace identity remains stable when its Project changes or is dissolved.
    #[must_use]
    pub fn workspace(&self, workspace_id: &WorkbenchWorkspaceId) -> Option<&Workspace> {
        self.workspaces
            .iter()
            .find(|workspace| &workspace.workspace_id == workspace_id)
    }

    /// Directories belong to the Session and can be referenced across organizational groups.
    #[must_use]
    pub fn directory(&self, directory_id: &WorkbenchDirectoryId) -> Option<&Directory> {
        self.directories
            .iter()
            .find(|directory| &directory.directory_id == directory_id)
    }

    /// Resolve a Workspace's physical directory without consulting its Project.
    #[must_use]
    pub fn workspace_directory(&self, workspace_id: &WorkbenchWorkspaceId) -> Option<&Directory> {
        self.directory(
            self.workspace(workspace_id)?
                .primary_directory_id
                .as_ref()?,
        )
    }

    /// Where a Workspace stands: its first terminal's directory while that differs from its
    /// primary Directory, otherwise the primary Directory's path. `None` without a primary
    /// Directory, where no terminal can start.
    #[must_use]
    pub fn workspace_current_path(
        &self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Option<&WorkbenchDirectoryPath> {
        let workspace = self.workspace(workspace_id)?;
        let primary = self.directory(workspace.primary_directory_id.as_ref()?)?;
        Some(
            workspace
                .terminal_directory
                .as_ref()
                .unwrap_or(&primary.path),
        )
    }

    /// A Project's folders: each member Workspace's current path in display order, one entry per
    /// directory spelling, with the Worker of that Workspace's primary Directory. Empty when the
    /// display order is invalid.
    #[must_use]
    pub fn project_folders(
        &self,
        project_id: &WorkbenchProjectId,
    ) -> Vec<(&WorkerId, &WorkbenchDirectoryPath)> {
        let Ok(ordered) = self.ordered_workspaces() else {
            return Vec::new();
        };
        let mut folders: Vec<(&WorkerId, &WorkbenchDirectoryPath)> = Vec::new();
        for (_, workspace) in ordered
            .into_iter()
            .filter(|(project, _)| project.is_some_and(|project| &project.project_id == project_id))
        {
            let (Some(directory), Some(path)) = (
                self.workspace_directory(&workspace.workspace_id),
                self.workspace_current_path(&workspace.workspace_id),
            ) else {
                continue;
            };
            if !folders
                .iter()
                .any(|(_, folder)| same_directory_spelling(folder.as_str(), path.as_str()))
            {
                folders.push((&directory.worker_id, path));
            }
        }
        folders
    }

    /// Iterate canonical members of a Project, or all ungrouped Workspaces for `None`.
    pub fn workspaces_in_project<'a>(
        &'a self,
        project_id: Option<&'a WorkbenchProjectId>,
    ) -> impl Iterator<Item = &'a Workspace> {
        self.workspaces
            .iter()
            .filter(move |workspace| workspace.project_id.as_ref() == project_id)
    }
}

#[cfg(test)]
mod tests;
