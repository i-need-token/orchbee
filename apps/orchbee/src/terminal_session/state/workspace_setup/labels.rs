//! Exact location-label targets and receipt validation; the setup owner retains submission authority.
use super::super::{NativeSessionState, PaneKey, PaneRef};
use super::{
    NativeContextLocation, NativeContextTabs, NativeLocationLabelStatus, NativeLocationLabelTarget,
    NativeSetupStatus, SetupOperation, WorkbenchAction, WorkbenchSetupCommit,
};
use orchbee_workbench_protocol::{Project, WorkbenchProjectId};
use std::time::Instant;

/// One drawn Project rename handoff, preserving the independent initiating context and focus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectRenameRequest {
    project: Project,
    source: NativeContextTabs,
    desired: Option<PaneKey>,
    focus: Option<PaneRef>,
    deadline: Instant,
}

impl ProjectRenameRequest {
    pub(in crate::terminal_session) fn capture(
        state: &NativeSessionState,
        project: &Project,
        deadline: Instant,
    ) -> Option<Self> {
        let request = Self {
            project: project.clone(),
            source: state.cache().clone(),
            desired: state.desired().cloned(),
            focus: state.focus_pane_ref(),
            deadline,
        };
        request.matches(state).then_some(request)
    }

    pub(super) fn matches(&self, state: &NativeSessionState) -> bool {
        self.source == *state.cache()
            && self.desired.as_ref() == state.desired()
            && self.focus == state.focus_pane_ref()
            && state
                .cache()
                .projects()
                .iter()
                .find(|p| p.project_id == self.project.project_id)
                == Some(&self.project)
    }

    pub(super) fn project_id(&self) -> &WorkbenchProjectId {
        &self.project.project_id
    }

    pub(super) fn label(&self) -> &str {
        &self.project.label
    }

    pub(in crate::terminal_session) fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn project_rename_available(&self) -> bool {
        !self.setup_blocks_structure() && self.setup_authority_available() && self.invariant_holds()
    }

    pub(in crate::terminal_session) fn settle_stale_project_rename(&mut self) -> bool {
        let stale = self.setup.draft.as_ref().is_some_and(|draft| {
            draft
                .project_rename
                .as_ref()
                .is_some_and(|request| !request.matches(self))
        });
        if stale {
            self.setup.draft = None;
            self.setup.status = Some(NativeSetupStatus::LocationLabel {
                target: NativeLocationLabelTarget::Project,
                status: NativeLocationLabelStatus::NotApplied,
            });
        }
        stale
    }
}

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
        if let WorkbenchAction::ProjectRename { project_id, .. } = &self.command.action {
            return self
                .baseline
                .projects()
                .iter()
                .find(|project| &project.project_id == project_id)
                .map(|project| project.label.as_str());
        }
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
