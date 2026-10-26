//! Exact location-label targets and receipt validation; the setup owner retains submission authority.
use super::super::{PaneKey, PaneRef, WorkbenchClientState};
use super::{
    NativeContextLocation, NativeContextTabs, NativeLocationLabelStatus, NativeLocationLabelTarget,
    NativeSetupStatus, SetupOperation, WorkbenchAction, WorkbenchSetupCommit,
};
use orchbee_workbench_protocol::{Project, WorkbenchProjectId, WorkbenchWorkspaceId};
use std::time::Instant;

/// The exact owner a rename addresses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LabelOwner {
    Project(WorkbenchProjectId),
    Workspace(WorkbenchWorkspaceId),
}

/// Text from a rename refused as stale, offered once to the next rename of the same
/// owner so a concurrent change never discards what the user typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RetainedLabel {
    owner: LabelOwner,
    label: String,
}

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
    pub(in crate::workbench_client) fn capture(
        state: &WorkbenchClientState,
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

    pub(super) fn matches(&self, state: &WorkbenchClientState) -> bool {
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

    pub(in crate::workbench_client) fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl WorkbenchClientState {
    pub(in crate::workbench_client) fn project_rename_available(&self) -> bool {
        !self.setup_blocks_structure() && self.setup_authority_available() && self.invariant_holds()
    }

    pub(in crate::workbench_client) fn settle_stale_project_rename(&mut self) -> bool {
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

/// The current location's rename owner and its present label.
pub(super) fn location_label_owner(
    cache: &NativeContextTabs,
    target: NativeLocationLabelTarget,
) -> Option<(LabelOwner, String)> {
    let label = location_label(cache, target)?.to_owned();
    let NativeContextLocation::Workspace { workspace_id, .. } = cache.location() else {
        return None;
    };
    let owner = match target {
        NativeLocationLabelTarget::Project => LabelOwner::Project(
            cache
                .canonical_snapshot()
                .navigation
                .workspace(workspace_id)?
                .project_id
                .clone()?,
        ),
        NativeLocationLabelTarget::Workspace => LabelOwner::Workspace(workspace_id.clone()),
    };
    Some((owner, label))
}

impl WorkbenchClientState {
    /// Takes retained rename text only for the same owner; another owner leaves it.
    pub(super) fn take_retained_label(&mut self, owner: &LabelOwner) -> Option<String> {
        if self
            .setup
            .retained_label
            .as_ref()
            .is_some_and(|retained| &retained.owner == owner)
        {
            self.setup
                .retained_label
                .take()
                .map(|retained| retained.label)
        } else {
            None
        }
    }
}

pub(super) fn location_label(
    cache: &NativeContextTabs,
    target: NativeLocationLabelTarget,
) -> Option<&str> {
    let NativeContextLocation::Workspace {
        workspace_id,
        terminal_context_id,
    } = cache.location()
    else {
        return None;
    };
    let navigation = &cache.canonical_snapshot().navigation;
    let workspace = navigation.workspace(workspace_id)?;
    if &workspace.terminal_context_id != terminal_context_id {
        return None;
    }
    Some(match target {
        NativeLocationLabelTarget::Project => {
            &navigation.project(workspace.project_id.as_ref()?)?.label
        }
        NativeLocationLabelTarget::Workspace => &workspace.label,
    })
}
impl SetupOperation {
    pub(super) fn retained_label(&self) -> Option<RetainedLabel> {
        let (owner, label) = match &self.command.action {
            WorkbenchAction::ProjectRename { project_id, label } => {
                (LabelOwner::Project(project_id.clone()), label)
            }
            WorkbenchAction::WorkspaceRename {
                workspace_id,
                label,
            } => (LabelOwner::Workspace(workspace_id.clone()), label),
            // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
            _ => return None,
        };
        Some(RetainedLabel {
            owner,
            label: label.as_str().to_owned(),
        })
    }
    pub(super) fn location_label_target(&self) -> Option<NativeLocationLabelTarget> {
        match self.command.action {
            WorkbenchAction::ProjectRename { .. } => Some(NativeLocationLabelTarget::Project),
            WorkbenchAction::WorkspaceRename { .. } => Some(NativeLocationLabelTarget::Workspace),
            // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
            _ => None,
        }
    }
    pub(super) fn status(
        &self,
        label_status: NativeLocationLabelStatus,
        other: NativeSetupStatus,
    ) -> NativeSetupStatus {
        if matches!(self.command.action, WorkbenchAction::TabsConfigure { .. }) {
            return match other {
                NativeSetupStatus::NotApplied => NativeSetupStatus::TabsNotApplied,
                NativeSetupStatus::CommittedNeedRefresh => {
                    NativeSetupStatus::TabsCommittedNeedRefresh
                }
                NativeSetupStatus::OutcomeUnknown => NativeSetupStatus::TabsOutcomeUnknown,
                status => status,
            };
        }
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
            workspace_id: expected_workspace,
            ..
        } = self.baseline.location()
        else {
            return None;
        };
        let valid = match &self.command.action {
            WorkbenchAction::WorkspaceRename { workspace_id, .. } => {
                workspace_id == expected_workspace
            }
            // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
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
                    workspace_id,
                    label,
                },
                WorkbenchSetupCommit::WorkspaceRenamed {
                    workspace_id: returned_workspace,
                    unchanged,
                    ..
                },
            ) if workspace_id == returned_workspace => (label, *unchanged),
            // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
            _ => return None,
        };
        (unchanged == (self.original_location_label()? == label.as_str())).then_some(unchanged)
    }
}
