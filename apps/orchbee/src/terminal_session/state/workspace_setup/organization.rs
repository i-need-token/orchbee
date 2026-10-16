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
pub(in crate::terminal_session) enum OrganizationChoice {
    Review,
    NewProject,
    SetProject(Option<orchbee_workbench_protocol::WorkbenchProjectId>),
    DeleteIfIdle,
    ReviewDelete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOrganizationRequest {
    pub(super) target: OrganizationTarget,
    pub(super) choice: OrganizationChoice,
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
            choice: OrganizationChoice::Review,
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

    pub(in crate::terminal_session) fn with_choice(
        mut self,
        choice: OrganizationChoice,
    ) -> Option<Self> {
        let valid = match (&self.target, &choice) {
            (OrganizationTarget::Workspace(workspace), OrganizationChoice::NewProject) => {
                workspace.project_id.is_none()
            }
            (OrganizationTarget::Workspace(workspace), OrganizationChoice::SetProject(project)) => {
                project.as_ref().is_some_and(|id| {
                    self.source
                        .canonical_snapshot()
                        .navigation
                        .project(id)
                        .is_some()
                }) || project.is_none() && workspace.project_id.is_some()
            }
            (
                OrganizationTarget::Project(_),
                OrganizationChoice::DeleteIfIdle | OrganizationChoice::ReviewDelete,
            ) => true,
            (_, OrganizationChoice::Review) => true,
            _ => false,
        };
        if !valid {
            return None;
        }
        self.choice = choice;
        Some(self)
    }

    pub(in crate::terminal_session) fn direct(&self) -> bool {
        matches!(
            self.choice,
            OrganizationChoice::SetProject(_) | OrganizationChoice::DeleteIfIdle
        )
    }

    pub(in crate::terminal_session) fn reviewed_delete(&self) -> bool {
        self.choice == OrganizationChoice::ReviewDelete
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
    pub(in crate::terminal_session) fn plan_sidebar_organization(
        &self,
        request: &WorkspaceOrganizationRequest,
        request_id: super::WorkbenchRequestId,
        generation: u64,
        deadlines: super::OperationDeadlines,
    ) -> Option<SetupOperation> {
        if generation == 0
            || !request.matches(self)
            || !self.setup_authority_available()
            || self.setup_blocks_structure()
            || !self.invariant_holds()
        {
            return None;
        }
        let action = match (&request.target, &request.choice) {
            (
                OrganizationTarget::Workspace(workspace),
                OrganizationChoice::SetProject(project_id),
            ) => WorkbenchAction::WorkspaceSetProject {
                workspace_id: workspace.workspace_id.clone(),
                project_id: project_id.clone(),
            },
            (OrganizationTarget::Project(project), OrganizationChoice::DeleteIfIdle) => {
                WorkbenchAction::ProjectDissolveIfIdle {
                    project_id: project.project_id.clone(),
                }
            }
            _ => return None,
        };
        let command = super::WorkbenchCommand {
            protocol_version: super::ProtocolVersion,
            request_id,
            site_id: self.cache.site_id().clone(),
            expected_revision: self.cache.revision(),
            action,
        };
        Some(SetupOperation {
            quick: None,
            generation,
            request: super::WorkbenchSetupRequest::try_from(command.clone()).ok()?,
            command,
            baseline: self.cache.clone(),
            deadlines,
        })
    }

    pub(in crate::terminal_session) fn review_project_delete(
        &mut self,
        operation: &SetupOperation,
        deadline: Instant,
    ) -> Option<super::NativeSetupToken> {
        let WorkbenchAction::ProjectDissolveIfIdle { project_id } = &operation.command.action
        else {
            return None;
        };
        if self.cache != operation.baseline {
            return None;
        }
        let project = self
            .cache
            .canonical_snapshot()
            .navigation
            .project(project_id)?;
        let request = WorkspaceOrganizationRequest::project(self, project, deadline)?
            .with_choice(OrganizationChoice::ReviewDelete)?;
        self.open_setup_draft(super::NativeSetupIntent::Organize(Box::new(request)))
    }

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
