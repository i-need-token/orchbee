//! Automatic Workspace labels follow the directory sampled from each Workspace's first terminal.
use super::SessionCoordinator;
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, Snapshot, WorkbenchDirectoryPath, WorkbenchWorkspaceId,
    Workspace,
};
use std::collections::BTreeMap;

/// The last directory considered for each Workspace's first terminal. It is process-local, so a
/// restart re-seeds it from the restored terminal without renaming anything.
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

/// Directory reports may differ only in ASCII case or a trailing separator between sources.
fn same_directory(left: &WorkbenchDirectoryPath, right: &WorkbenchDirectoryPath) -> bool {
    let trim = |path: &WorkbenchDirectoryPath| {
        let path = path.as_str();
        let trimmed = path.trim_end_matches(['\\', '/']);
        if trimmed.is_empty() || trimmed.ends_with(':') {
            path.to_owned()
        } else {
            trimmed.to_owned()
        }
    };
    trim(left).eq_ignore_ascii_case(&trim(right))
}

impl SessionCoordinator {
    /// Called with every committed sample's directory under the Session gate. The first
    /// directory seen for a Workspace's first terminal only seeds the record, so creation and
    /// restart keep the committed label; a later different directory, including one from a
    /// replacement first terminal, renames an automatic Workspace.
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
        if let Some(previous) = self
            .followed_directories
            .insert(workspace_id.clone(), directory.clone())
            && !same_directory(&previous, directory)
        {
            workbench.follow_workspace_directory(&workspace_id, directory);
        }
    }
}
