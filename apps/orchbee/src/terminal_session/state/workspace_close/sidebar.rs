//! Sidebar Delete retains exact source evidence and asks the Server for conditional admission.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceDeleteRequest {
    baseline: NativeContextTabs,
    desired: Option<PaneKey>,
    workspace: orchbee_workbench_protocol::Workspace,
    deadline: Instant,
}

impl WorkspaceDeleteRequest {
    pub(in crate::terminal_session) fn capture(
        state: &NativeSessionState,
        workspace: &WorkbenchWorkspaceId,
        deadline: Instant,
    ) -> Option<Self> {
        state.close_scope_available().then_some(())?;
        Some(Self {
            baseline: state.cache.clone(),
            desired: state.desired.clone(),
            workspace: state
                .cache
                .canonical_snapshot()
                .navigation
                .workspace(workspace)?
                .clone(),
            deadline,
        })
    }
    pub(in crate::terminal_session) fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl NativeSessionState {
    pub(in crate::terminal_session) fn plan_sidebar_workspace_delete(
        &self,
        request: &WorkspaceDeleteRequest,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<WorkspaceCloseOperation> {
        if generation == 0
            || request.baseline != self.cache
            || request.desired != self.desired
            || self
                .cache
                .canonical_snapshot()
                .navigation
                .workspace(&request.workspace.workspace_id)
                != Some(&request.workspace)
            || !self.close_scope_available()
        {
            return None;
        }
        Some(WorkspaceCloseOperation {
            conditional: true,
            destructive: true,
            target: WorkspaceCloseTarget::Workspace {
                workspace_id: request.workspace.workspace_id.clone(),
                terminal_context_id: request.workspace.terminal_context_id.clone(),
            },
            request_id,
            generation,
            deadlines: deadlines.bounded_by(request.deadline),
            baseline: self.cache.clone(),
            desired: self.desired.clone(),
            label: request.workspace.label.to_string(),
        })
    }

    pub(super) fn sidebar_workspace_delete_current(
        &self,
        operation: &WorkspaceCloseOperation,
    ) -> bool {
        operation.conditional
            && operation.destructive
            && operation.generation != 0
            && self.close_scope_available()
            && operation.baseline == self.cache
            && operation.desired == self.desired
            && operation.target.matches_snapshot(&self.cache)
            && matches!(operation.target, WorkspaceCloseTarget::Workspace { .. })
    }

    pub(in crate::terminal_session) fn review_sidebar_workspace_delete(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        if !matches!(evidence.result, WorkspaceCloseResult::ConfirmationRequired)
            || !evidence.operation.conditional
            || evidence.operation.baseline != self.cache
            || evidence.operation.desired != self.desired
            || !self.close_scope_available()
        {
            return false;
        }
        if self
            .open_workspace_scope_close_confirmation(
                evidence.operation.target.clone(),
                evidence.operation.label.clone(),
            )
            .is_none()
        {
            return false;
        }
        if let Some(prompt) = &mut self.workspace_close.confirmation {
            prompt.destructive = true;
        }
        true
    }
}
