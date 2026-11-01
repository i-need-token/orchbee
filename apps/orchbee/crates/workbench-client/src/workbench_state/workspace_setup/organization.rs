//! Optional grouping changes preserve the initiating terminal and its immutable resource identity.
use super::super::{PaneKey, PaneRef};
use super::{
    ClientContextTabs, SetupOperation, WorkbenchAction, WorkbenchClientState, WorkbenchSetupCommit,
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
pub(crate) enum OrganizationChoice {
    Review,
    NewProject,
    SetProject(Option<orchbee_workbench_protocol::WorkbenchProjectId>),
    /// Dissolving a Project keeps every Workspace, so it runs at once without a confirmation.
    Dissolve,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceOrganizationRequest {
    pub(super) target: OrganizationTarget,
    pub(super) choice: OrganizationChoice,
    source: ClientContextTabs,
    desired: Option<PaneKey>,
    focus: Option<PaneRef>,
    deadline: Instant,
}

impl WorkspaceOrganizationRequest {
    pub(crate) fn workspace(
        state: &WorkbenchClientState,
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

    pub(crate) fn project(
        state: &WorkbenchClientState,
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
        state: &WorkbenchClientState,
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

    pub(super) fn matches(&self, state: &WorkbenchClientState) -> bool {
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

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn with_choice(mut self, choice: OrganizationChoice) -> Option<Self> {
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
            (OrganizationTarget::Project(_), OrganizationChoice::Dissolve) => true,
            (OrganizationTarget::Workspace(_), OrganizationChoice::Review) => true,
            _ => false,
        };
        if !valid {
            return None;
        }
        self.choice = choice;
        Some(self)
    }

    pub(crate) fn direct(&self) -> bool {
        matches!(
            self.choice,
            OrganizationChoice::SetProject(_) | OrganizationChoice::Dissolve
        )
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
            // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
            _ => None,
        }
    }
}

impl WorkbenchClientState {
    pub(crate) fn plan_sidebar_organization(
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
            (OrganizationTarget::Project(project), OrganizationChoice::Dissolve) => {
                WorkbenchAction::ProjectDissolve {
                    project_id: project.project_id.clone(),
                }
            }
            _ => return None,
        };
        let command = super::WorkbenchCommand {
            protocol_version: super::ProtocolVersion,
            request_id,
            session_id: self.cache.session_id().clone(),
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
}
