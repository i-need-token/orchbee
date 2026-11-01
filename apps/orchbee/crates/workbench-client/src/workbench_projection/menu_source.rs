//! Exact directory authority for a menu target, independent of unrelated navigation rows.
use super::{ClientContextLocation, ClientContextTabs};
use orchbee_workbench_protocol::{Directory, Workspace};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MenuContextSource {
    Workspace {
        workspace: Workspace,
        directory: Option<Directory>,
    },
}

/// `workspace` without what its first terminal's directory moves on its own: the observed directory
/// and an automatic label. Comparing this, a `cd` is not another Workspace.
pub(crate) fn settled_workspace(workspace: &Workspace) -> Workspace {
    let mut settled = workspace.clone();
    settled.terminal_directory = None;
    if settled.automatic_label {
        settled.label.clear();
    }
    settled
}

impl ClientContextTabs {
    pub(crate) fn menu_context_source(
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
            workspace: settled_workspace(workspace),
            directory,
        })
    }
}
