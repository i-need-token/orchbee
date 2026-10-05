//! Exact directory authority for a menu target, independent of unrelated navigation rows.
use super::{NativeContextLocation, NativeContextTabs};
use orchbee_workbench_protocol::{Directory, Workspace};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum NativeMenuContextSource {
    Standalone,
    Workspace {
        workspace: Workspace,
        directory: Option<Directory>,
    },
}

impl NativeContextTabs {
    pub(in crate::terminal_session) fn menu_context_source(
        &self,
        location: &NativeContextLocation,
    ) -> Option<NativeMenuContextSource> {
        let NativeContextLocation::Workspace {
            project_id,
            workspace_id,
            terminal_context_id,
        } = location
        else {
            return (location == self.location()).then_some(NativeMenuContextSource::Standalone);
        };
        let project = self
            .projects()
            .iter()
            .find(|p| &p.project_id == project_id)?;
        let workspace = project.workspaces.iter().find(|w| {
            &w.workspace_id == workspace_id && &w.terminal_context_id == terminal_context_id
        })?;
        let directory = match &workspace.primary_directory_id {
            Some(id) => Some(
                project
                    .directories
                    .iter()
                    .find(|d| &d.directory_id == id)?
                    .clone(),
            ),
            None => None,
        };
        Some(NativeMenuContextSource::Workspace {
            workspace: workspace.clone(),
            directory,
        })
    }
}
