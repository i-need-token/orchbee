//! Keeps captured menu authority with Workbench while exposing only its plain view intent to
//! the Client reducer. A decided action can consume the matching capture once on the same call.

use crate::client_view::ClientAction;
use crate::workbench_client::{
    ProjectQuickWorkspaceRequest, ProjectRenameRequest, WorkspaceDeleteRequest,
    WorkspaceOrganizationRequest,
};

/// An input action carries either a plain view intent or an opaque Workbench-owned capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InputAction {
    View(ClientAction),
    Captured(CapturedClientAction),
}

/// Immutable source, baseline, focus and deadline evidence captured by the owning menu.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapturedClientAction(CapturedRequest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) enum CapturedRequest {
    WorkspaceAtDirectory(Box<ProjectQuickWorkspaceRequest>),
    ProjectQuickWorkspace(Box<ProjectQuickWorkspaceRequest>),
    SidebarWorkspaceDelete(Box<WorkspaceDeleteRequest>),
    TargetedProjectRename(Box<ProjectRenameRequest>),
    WorkspaceOrganization(Box<WorkspaceOrganizationRequest>),
}

impl InputAction {
    /// The view reducer sees the semantic intent without receiving captured authority.
    pub(crate) fn intent(&self) -> ClientAction {
        match self {
            Self::View(action) => action.clone(),
            Self::Captured(captured) => captured.intent(),
        }
    }

    /// Separates the view intent from evidence that must remain on the current dispatch stack.
    pub(crate) fn into_parts(self) -> (ClientAction, Option<CapturedClientAction>) {
        match self {
            Self::View(action) => (action, None),
            Self::Captured(captured) => (captured.intent(), Some(captured)),
        }
    }

    /// A refused or rewritten view decision cannot execute an unrelated captured command.
    pub(crate) fn for_decision(
        action: ClientAction,
        captured: Option<CapturedClientAction>,
    ) -> Self {
        match captured {
            Some(captured) if captured.intent() == action => Self::Captured(captured),
            _ => Self::View(action),
        }
    }

    pub(in crate::workbench_client) fn workspace_at_directory(
        request: Box<ProjectQuickWorkspaceRequest>,
    ) -> Self {
        Self::Captured(CapturedClientAction(CapturedRequest::WorkspaceAtDirectory(
            request,
        )))
    }

    pub(in crate::workbench_client) fn project_quick_workspace(
        request: Box<ProjectQuickWorkspaceRequest>,
    ) -> Self {
        Self::Captured(CapturedClientAction(
            CapturedRequest::ProjectQuickWorkspace(request),
        ))
    }

    pub(in crate::workbench_client) fn sidebar_workspace_delete(
        request: Box<WorkspaceDeleteRequest>,
    ) -> Self {
        Self::Captured(CapturedClientAction(
            CapturedRequest::SidebarWorkspaceDelete(request),
        ))
    }

    pub(in crate::workbench_client) fn targeted_project_rename(
        request: Box<ProjectRenameRequest>,
    ) -> Self {
        Self::Captured(CapturedClientAction(
            CapturedRequest::TargetedProjectRename(request),
        ))
    }

    pub(in crate::workbench_client) fn workspace_organization(
        request: Box<WorkspaceOrganizationRequest>,
    ) -> Self {
        Self::Captured(CapturedClientAction(
            CapturedRequest::WorkspaceOrganization(request),
        ))
    }
}

impl From<ClientAction> for InputAction {
    fn from(action: ClientAction) -> Self {
        Self::View(action)
    }
}

impl CapturedClientAction {
    fn intent(&self) -> ClientAction {
        match &self.0 {
            CapturedRequest::WorkspaceAtDirectory(_) => ClientAction::RequestWorkspaceAtDirectory,
            CapturedRequest::ProjectQuickWorkspace(_) => ClientAction::RequestProjectQuickWorkspace,
            CapturedRequest::SidebarWorkspaceDelete(_) => {
                ClientAction::RequestSidebarWorkspaceDelete
            }
            CapturedRequest::TargetedProjectRename(_) => ClientAction::RequestTargetedProjectRename,
            CapturedRequest::WorkspaceOrganization(_) => ClientAction::RequestWorkspaceOrganization,
        }
    }

    pub(in crate::workbench_client) fn into_request(self) -> CapturedRequest {
        self.0
    }
}
