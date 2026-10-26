//! Exact directory authority for a menu target, independent of unrelated navigation rows.
use super::{ClientContextLocation, ClientContextTabs};
use orchbee_workbench_protocol::{Directory, Workspace};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum MenuContextSource {
    Workspace {
        workspace: Workspace,
        directory: Option<Directory>,
    },
}

impl ClientContextTabs {
    pub(in crate::workbench_client) fn menu_context_source(
        &self,
        location: &ClientContextLocation,
    ) -> Option<MenuContextSource> {
        let ClientContextLocation::Workspace {
            workspace_id,
            terminal_context_id,
        } = location
        else {
            return None;
        };
        let workspace = self
            .canonical_snapshot()
            .navigation
            .workspaces
            .iter()
            .find(|w| {
                &w.workspace_id == workspace_id && &w.terminal_context_id == terminal_context_id
            })?;
        let directory = match &workspace.primary_directory_id {
            Some(id) => Some(self.canonical_snapshot().navigation.directory(id)?.clone()),
            None => None,
        };
        Some(MenuContextSource::Workspace {
            workspace: workspace.clone(),
            directory,
        })
    }
}
