//! Publishes the label an automatic Workspace takes from its first terminal's directory.
use super::WorkbenchAuthority;
use orchbee_workbench_protocol::{
    MAXIMUM_LABEL_LENGTH, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProtocolDocument,
    WorkbenchRevision, WorkbenchWorkspaceId, serialize_document,
};

impl WorkbenchAuthority {
    /// Advances the projection with one automatic Workspace's new label: the directory's final
    /// name, suffixed ` (2)`, ` (3)` and onward like generated names while another Workspace uses
    /// it. The caller holds the Session gate and decides that the directory changed. A fixed
    /// label, a root directory, an unchanged label or an exhausted projection budget commits
    /// nothing and is never retried here.
    pub fn follow_workspace_directory(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
        directory: &WorkbenchDirectoryPath,
    ) {
        if self.retired {
            return;
        }
        let Some(name) = std::path::Path::new(directory.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
        else {
            return;
        };
        let mut candidate = self.projection.snapshot().clone();
        let occupied = candidate
            .navigation
            .workspaces
            .iter()
            .filter(|workspace| &workspace.workspace_id != workspace_id)
            .map(|workspace| workspace.label.as_str())
            .collect::<std::collections::HashSet<_>>();
        // There are at most N occupied names, so one of these N+1 candidates is free.
        let Some(label) = (1..=occupied.len() + 1)
            .map(|ordinal| {
                let suffix = if ordinal == 1 {
                    String::new()
                } else {
                    format!(" ({ordinal})")
                };
                let mut label = name
                    .chars()
                    .take(MAXIMUM_LABEL_LENGTH - suffix.chars().count())
                    .collect::<String>();
                label.push_str(&suffix);
                label
            })
            .find(|label| !occupied.contains(label.as_str()))
        else {
            return;
        };
        if WorkbenchLabel::parse_workspace(&label).is_err() {
            return;
        }
        let Some(workspace) = candidate
            .navigation
            .workspaces
            .iter_mut()
            .find(|workspace| &workspace.workspace_id == workspace_id)
        else {
            return;
        };
        if !workspace.automatic_label || workspace.label == label {
            return;
        }
        workspace.label = label;
        let Some(revision) = candidate.revision.value().checked_add(1) else {
            return;
        };
        candidate.revision = WorkbenchRevision::new(revision);
        let Ok(encoded) = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate))
        else {
            return;
        };
        let Ok(prepared) = self.projection.prepare_advance(&encoded) else {
            return;
        };
        self.projection.commit_prepared(prepared);
    }
}
