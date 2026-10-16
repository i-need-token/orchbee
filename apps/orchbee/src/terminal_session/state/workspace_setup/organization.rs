//! Optional grouping changes preserve the initiating terminal and its immutable resource identity.
use super::super::{PaneKey, PaneRef};
use super::{
    NativeContextTabs, NativeSessionState, SetupOperation, WorkbenchAction, WorkbenchSetupCommit,
};
use orchbee_workbench_protocol::{Project, WorkbenchWorkspaceId, Workspace};
use std::time::Instant;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OrganizationTarget {
    Workspace(Workspace),
    Project(Project),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOrganizationRequest {
    pub(super) target: OrganizationTarget,
    source: NativeContextTabs,
    desired: Option<PaneKey>,
    focus: Option<PaneRef>,
    deadline: Instant,
}

impl WorkspaceOrganizationRequest {
    pub(in crate::terminal_session) fn workspace(
        state: &NativeSessionState,
        id: &WorkbenchWorkspaceId,
        deadline: Instant,
    ) -> Option<Self> {
        let workspace = state
            .cache()
            .canonical_snapshot()
            .navigation
            .workspace(id)?
            .clone();
        Self::capture(state, OrganizationTarget::Workspace(workspace), deadline)
    }

    pub(in crate::terminal_session) fn project(
        state: &NativeSessionState,
        project: &Project,
        deadline: Instant,
    ) -> Option<Self> {
        Self::capture(
            state,
            OrganizationTarget::Project(project.clone()),
            deadline,
        )
    }

    fn capture(
        state: &NativeSessionState,
        target: OrganizationTarget,
        deadline: Instant,
    ) -> Option<Self> {
        let request = Self {
            target,
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
            && match &self.target {
                OrganizationTarget::Workspace(workspace) => {
                    state
                        .cache()
                        .canonical_snapshot()
                        .navigation
                        .workspace(&workspace.workspace_id)
                        == Some(workspace)
                }
                OrganizationTarget::Project(project) => {
                    state
                        .cache()
                        .canonical_snapshot()
                        .navigation
                        .project(&project.project_id)
                        == Some(project)
                }
            }
    }

    pub(in crate::terminal_session) fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl SetupOperation {
    pub(super) fn organization_receipt(&self, commit: &WorkbenchSetupCommit) -> Option<bool> {
        let navigation = &self.baseline.canonical_snapshot().navigation;
        match (&self.command.action, commit) {
            (
                WorkbenchAction::WorkspaceSetProject {
                    workspace_id,
                    project_id,
                },
                WorkbenchSetupCommit::WorkspaceProjectChanged {
                    workspace_id: workspace,
                    project_id: project,
                    created: false,
                    unchanged,
                    ..
                },
            ) => {
                let before = navigation.workspace(workspace_id)?;
                (workspace_id == workspace
                    && project_id == project
                    && *unchanged == (&before.project_id == project_id))
                    .then_some(*unchanged)
            }
            (
                WorkbenchAction::WorkspaceCreateProject { workspace_id, .. },
                WorkbenchSetupCommit::WorkspaceProjectChanged {
                    workspace_id: workspace,
                    project_id: Some(project),
                    created: true,
                    unchanged: false,
                    ..
                },
            ) => {
                navigation.workspace(workspace_id)?;
                (workspace_id == workspace && navigation.project(project).is_none())
                    .then_some(false)
            }
            (
                WorkbenchAction::ProjectDissolve { project_id }
                | WorkbenchAction::ProjectDissolveIfIdle { project_id },
                WorkbenchSetupCommit::ProjectDissolved {
                    project_id: project,
                    ..
                },
            ) => {
                navigation.project(project_id)?;
                (project_id == project).then_some(false)
            }
            _ => None,
        }
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn arm_project_dissolve(
        &mut self,
        token: super::NativeSetupToken,
        deadlines: super::OperationDeadlines,
    ) -> bool {
        let Some(draft) = self.setup.draft.as_mut().filter(|draft| {
            draft.token == token && draft.stage == super::NativeSetupStage::DissolveProject
        }) else {
            return false;
        };
        draft.continuation = Some(deadlines);
        true
    }
}
