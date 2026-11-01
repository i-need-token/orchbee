//! Workspaces follow the directory sampled from their first terminal: they record it and an
//! automatic Workspace takes its name.
use super::SessionCoordinator;
use orchbee_server::same_directory;
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, Snapshot, WorkbenchDirectoryPath, WorkbenchWorkspaceId,
    Workspace,
};
use std::collections::BTreeMap;

/// The last directory considered for each Workspace's first terminal. It is process-local, so a
/// restart considers each restored terminal's first sample afresh.
pub(super) type FollowedDirectories = BTreeMap<WorkbenchWorkspaceId, WorkbenchDirectoryPath>;

/// The primary binding of the first Tab's leading Panel, the Workspace's first terminal.
pub(super) fn first_terminal<'a>(
    snapshot: &'a Snapshot,
    workspace: &Workspace,
) -> Option<&'a PresentationBinding> {
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)?;
    let mut node = &context.tabs.first()?.root;
    loop {
        match node {
            PanelNode::Panel {
                primary_presentation,
                ..
            } => break primary_presentation.binding.as_ref(),
            PanelNode::Split { children, .. } => node = &children.first()?.node,
        }
    }
}

impl SessionCoordinator {
    /// Called with every committed sample's directory under the Session gate. The first
    /// directory seen for a Workspace's first terminal, and each later different one, including
    /// one from a replacement first terminal, is recorded and names an automatic Workspace.
    /// Repeating a directory does nothing, so another Workspace's later rename never reshuffles
    /// this one's suffix.
    pub(super) fn follow_first_terminal_directory(
        &mut self,
        binding: &PresentationBinding,
        directory: &WorkbenchDirectoryPath,
    ) {
        let Some(workbench) = self.workbench.as_mut() else {
            return;
        };
        let snapshot = workbench.snapshot();
        self.followed_directories
            .retain(|workspace_id, _| snapshot.navigation.workspace(workspace_id).is_some());
        let Some(workspace_id) = snapshot
            .navigation
            .workspaces
            .iter()
            .find(|workspace| first_terminal(snapshot, workspace) == Some(binding))
            .map(|workspace| workspace.workspace_id.clone())
        else {
            return;
        };
        if self
            .followed_directories
            .insert(workspace_id.clone(), directory.clone())
            .is_none_or(|previous| !same_directory(previous.as_str(), directory.as_str()))
        {
            workbench.follow_workspace_directory(&workspace_id, directory);
        }
    }
}
