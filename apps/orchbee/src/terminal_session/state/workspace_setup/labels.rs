use super::{
    NativeContextLocation, NativeContextTabs, NativeLocationLabelStatus, NativeLocationLabelTarget,
    NativeSetupStatus, SetupOperation, WorkbenchAction, WorkbenchSetupCommit,
};

pub(super) fn location_label(
    cache: &NativeContextTabs,
    target: NativeLocationLabelTarget,
) -> Option<&str> {
    let NativeContextLocation::Workspace {
        project_id,
        workspace_id,
        terminal_context_id,
    } = cache.location()
    else {
        return None;
    };
    let project = cache
        .projects()
        .iter()
        .find(|project| &project.project_id == project_id)?;
    let workspace = project.workspaces.iter().find(|workspace| {
        &workspace.workspace_id == workspace_id
            && &workspace.terminal_context_id == terminal_context_id
    })?;
    Some(match target {
        NativeLocationLabelTarget::Project => &project.label,
        NativeLocationLabelTarget::Workspace => &workspace.label,
    })
}
impl SetupOperation {
    pub(super) fn location_label_target(&self) -> Option<NativeLocationLabelTarget> {
        match self.command.action {
            WorkbenchAction::ProjectRename { .. } => Some(NativeLocationLabelTarget::Project),
            WorkbenchAction::WorkspaceRename { .. } => Some(NativeLocationLabelTarget::Workspace),
            _ => None,
        }
    }
    pub(super) fn status(
        &self,
        label_status: NativeLocationLabelStatus,
        other: NativeSetupStatus,
    ) -> NativeSetupStatus {
        self.location_label_target()
            .map_or(other, |target| NativeSetupStatus::LocationLabel {
                target,
                status: label_status,
            })
    }
    pub(super) fn original_location_label(&self) -> Option<&str> {
        let target = self.location_label_target()?;
        let NativeContextLocation::Workspace {
            project_id: expected_project,
            workspace_id: expected_workspace,
            ..
        } = self.baseline.location()
        else {
            return None;
        };
        let valid = match &self.command.action {
            WorkbenchAction::ProjectRename { project_id, .. } => project_id == expected_project,
            WorkbenchAction::WorkspaceRename {
                project_id,
                workspace_id,
                ..
            } => project_id == expected_project && workspace_id == expected_workspace,
            _ => false,
        };
        valid
            .then(|| location_label(&self.baseline, target))
            .flatten()
    }
    /// Returns the Server's no-op bit only after checking the captured owner and original text.
    pub(super) fn validate_label_receipt(&self, commit: &WorkbenchSetupCommit) -> Option<bool> {
        let (label, unchanged) = match (&self.command.action, commit) {
            (
                WorkbenchAction::ProjectRename { project_id, label },
                WorkbenchSetupCommit::ProjectRenamed {
                    project_id: returned,
                    unchanged,
                    ..
                },
            ) if project_id == returned => (label, *unchanged),
            (
                WorkbenchAction::WorkspaceRename {
                    project_id,
                    workspace_id,
                    label,
                },
                WorkbenchSetupCommit::WorkspaceRenamed {
                    project_id: returned_project,
                    workspace_id: returned_workspace,
                    unchanged,
                    ..
                },
            ) if project_id == returned_project && workspace_id == returned_workspace => {
                (label, *unchanged)
            }
            _ => return None,
        };
        (unchanged == (self.original_location_label()? == label.as_str())).then_some(unchanged)
    }
}
