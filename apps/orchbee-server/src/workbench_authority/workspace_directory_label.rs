//! Publishes what a Workspace takes from its first terminal's directory: the observed directory
//! itself and, for an automatic Workspace, its label.
use super::WorkbenchAuthority;
use orchbee_workbench_protocol::{
    MAXIMUM_LABEL_LENGTH, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchProtocolDocument,
    WorkbenchRevision, WorkbenchWorkspaceId, serialize_document,
};

/// Directory reports may differ only in ASCII case or a trailing separator between sources.
pub fn same_directory(left: &str, right: &str) -> bool {
    without_trailing_separator(left).eq_ignore_ascii_case(without_trailing_separator(right))
}

/// A process reports its directory with a trailing separator; a drive root keeps its own.
fn without_trailing_separator(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        path
    } else {
        trimmed
    }
}

impl WorkbenchAuthority {
    /// Advances the projection with one Workspace's first terminal directory. The directory is
    /// recorded only while it differs from the primary Directory, so a sample matching it at
    /// creation records nothing. An automatic Workspace also takes the directory's final name,
    /// suffixed ` (2)`, ` (3)` and onward like generated names while another Workspace uses it; a
    /// root directory or invalid name leaves the label. The caller holds the Session gate and
    /// decides that the directory changed. No change or an exhausted projection budget commits
    /// nothing, and nothing is retried here.
    pub fn follow_workspace_directory(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
        directory: &WorkbenchDirectoryPath,
    ) {
        if self.retired {
            return;
        }
        let Ok(directory) =
            WorkbenchDirectoryPath::parse(without_trailing_separator(directory.as_str()))
        else {
            return;
        };
        let mut candidate = self.projection.snapshot().clone();
        let observed = (!candidate
            .navigation
            .workspace_directory(workspace_id)
            .is_some_and(|primary| same_directory(primary.path.as_str(), directory.as_str())))
        .then(|| directory.clone());
        let label = Self::directory_label(&candidate, workspace_id, &directory);
        let Some(workspace) = candidate
            .navigation
            .workspaces
            .iter_mut()
            .find(|workspace| &workspace.workspace_id == workspace_id)
        else {
            return;
        };
        let mut changed = workspace.terminal_directory != observed;
        workspace.terminal_directory = observed;
        if workspace.automatic_label
            && let Some(label) = label
            && workspace.label != label
        {
            workspace.label = label;
            changed = true;
        }
        if !changed {
            return;
        }
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

    /// The directory's final name, first free among its suffixed forms, when it is a valid
    /// Workspace label.
    fn directory_label(
        snapshot: &orchbee_workbench_protocol::Snapshot,
        workspace_id: &WorkbenchWorkspaceId,
        directory: &WorkbenchDirectoryPath,
    ) -> Option<String> {
        let name = std::path::Path::new(directory.as_str())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())?;
        let occupied = snapshot
            .navigation
            .workspaces
            .iter()
            .filter(|workspace| &workspace.workspace_id != workspace_id)
            .map(|workspace| workspace.label.as_str())
            .collect::<std::collections::HashSet<_>>();
        // There are at most N occupied names, so one of these N+1 candidates is free.
        (1..=occupied.len() + 1)
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
            .filter(|label| WorkbenchLabel::parse_workspace(label).is_ok())
    }
}
