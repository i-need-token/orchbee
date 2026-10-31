//! Sidebar Delete retains exact source evidence and asks the Server for conditional admission.
use super::{
    ClientContextTabs, ClientNotice, ClientPhase, Instant, OperationDeadlines, PaneKey,
    WorkbenchClientState, WorkbenchRequestId, WorkbenchWorkspaceId, WorkspaceCloseEvidence,
    WorkspaceCloseOperation, WorkspaceCloseResult, WorkspaceCloseTarget,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceDeleteRequest {
    baseline: ClientContextTabs,
    desired: Option<PaneKey>,
    workspace: orchbee_workbench_protocol::Workspace,
    deadline: Instant,
    recheck: bool,
}

impl WorkspaceDeleteRequest {
    pub(crate) fn capture(
        state: &WorkbenchClientState,
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
            recheck: false,
        })
    }
    /// The single automatic follow-up after the target changed under a confirmation reply.
    pub(crate) fn recheck(
        state: &WorkbenchClientState,
        workspace: &WorkbenchWorkspaceId,
        deadline: Instant,
    ) -> Option<Self> {
        Self::capture(state, workspace, deadline).map(|request| Self {
            recheck: true,
            ..request
        })
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    /// The same request over a newer projection when its target facts are unchanged, so an
    /// unrelated publication between capture and admission never refuses it.
    pub(crate) fn rebased(&self, state: &WorkbenchClientState) -> Option<Self> {
        if self.baseline == state.cache {
            return Some(self.clone());
        }
        if self.desired != state.desired
            || !target_unchanged(&self.baseline, &state.cache, &self.workspace.workspace_id)
        {
            return None;
        }
        Some(Self {
            baseline: state.cache.clone(),
            ..self.clone()
        })
    }
}

/// The next step after a conditional Close reply. No variant discards a confirmation silently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarDeleteReview {
    /// The reply asks for no confirmation, or its own refresh still owns settlement.
    NotApplicable,
    /// The destructive confirmation is open over the current snapshot.
    Confirm,
    /// The target changed while the request was in flight; ask the Server once more.
    Recheck(WorkbenchWorkspaceId),
    /// The target changed again or cannot be reviewed now; a notice asks the user to retry.
    Changed,
}

/// The facts a conditional reply speaks for: the addressed Workspace and its whole terminal Context.
fn target_facts<'a>(
    cache: &'a ClientContextTabs,
    workspace_id: &WorkbenchWorkspaceId,
) -> Option<(
    &'a orchbee_workbench_protocol::Workspace,
    &'a orchbee_workbench_protocol::TerminalContext,
)> {
    let snapshot = cache.canonical_snapshot();
    let workspace = snapshot.navigation.workspace(workspace_id)?;
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == workspace.terminal_context_id)?;
    Some((workspace, context))
}

/// Unrelated snapshot changes keep the reply valid; a change to the target itself does not.
pub(super) fn target_unchanged(
    before: &ClientContextTabs,
    after: &ClientContextTabs,
    workspace_id: &WorkbenchWorkspaceId,
) -> bool {
    before.session_id() == after.session_id()
        && target_facts(before, workspace_id)
            .is_some_and(|facts| target_facts(after, workspace_id) == Some(facts))
}

impl WorkbenchClientState {
    pub(crate) fn plan_sidebar_workspace_delete(
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
            recheck: request.recheck,
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

    /// Reviews a settled `confirmation-required` reply against the snapshot installed after it.
    /// Unrelated drift still opens the confirmation from the current snapshot; target drift asks
    /// once more, because a conditional request has no effect before confirmation.
    pub(crate) fn review_sidebar_workspace_delete(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> SidebarDeleteReview {
        let WorkspaceCloseTarget::Workspace { workspace_id, .. } = &evidence.operation.target
        else {
            return SidebarDeleteReview::NotApplicable;
        };
        if !matches!(evidence.result, WorkspaceCloseResult::ConfirmationRequired)
            || !evidence.operation.conditional
            || self.pending_refresh_generation() == Some(evidence.operation.generation)
            || matches!(self.phase, ClientPhase::Retired(_))
        {
            return SidebarDeleteReview::NotApplicable;
        }
        let available = self.close_scope_available();
        if available && target_unchanged(&evidence.operation.baseline, &self.cache, workspace_id) {
            if self
                .open_workspace_scope_close_confirmation(
                    evidence.operation.target.clone(),
                    evidence.operation.label.clone(),
                )
                .is_some()
            {
                if let Some(prompt) = &mut self.workspace_close.confirmation {
                    prompt.destructive = true;
                }
                return SidebarDeleteReview::Confirm;
            }
        } else if available
            && !evidence.operation.recheck
            && target_facts(&self.cache, workspace_id).is_some()
        {
            return SidebarDeleteReview::Recheck(workspace_id.clone());
        }
        self.settle_sidebar_workspace_delete_changed();
        SidebarDeleteReview::Changed
    }

    /// Ends a reply that cannot become a confirmation or a retry with a visible retry notice.
    pub(crate) fn settle_sidebar_workspace_delete_changed(&mut self) {
        if self
            .workspace_close
            .evidence
            .as_ref()
            .is_some_and(|evidence| {
                matches!(
                    evidence.result,
                    WorkspaceCloseResult::ConfirmationRequired | WorkspaceCloseResult::Stale
                )
            })
        {
            self.workspace_close.evidence = None;
        }
        self.set_notice(ClientNotice::WorkspaceChanged);
    }
}
