//! Identity-based resource lookup, independent of mutable Project organization.
use crate::{
    Directory, Navigation, Project, WorkbenchDirectoryId, WorkbenchLabel, WorkbenchProjectId,
    WorkbenchWorkspaceId, Workspace,
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
