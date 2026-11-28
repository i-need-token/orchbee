//! Captures one Workspace's terminals for the read-only idle check: the members a Workspace Close
//! would capture, each with its runtime as a conditional Close resolves it. It admits, reserves
//! and retains nothing, so the read leaves no ledger entry and no lifecycle fence.

use orchbee_server::WorkbenchTerminalTab;
use orchbee_workbench_protocol::{WorkbenchWorkspaceId, WorkspaceIdleFailure};

use super::{IdleCloseMember, SessionCoordinator};
use crate::terminal_coordinator::WorkerPlacement;

/// One member a Workspace Close would capture, and its runtime, or `None` when this owner cannot
/// resolve it, which a conditional Close answers by asking.
pub(crate) struct IdleCheckMember {
    pub(crate) tab: WorkbenchTerminalTab,
    pub(crate) runtime: Option<IdleCloseMember>,
}

impl SessionCoordinator {
    pub(crate) fn capture_workspace_idle(
        &self,
        workspace_id: &WorkbenchWorkspaceId,
        serving: &[WorkerPlacement],
    ) -> Result<Vec<IdleCheckMember>, WorkspaceIdleFailure> {
        let workbench = self
            .workbench
            .as_ref()
            .ok_or(WorkspaceIdleFailure::Unavailable)?;
        let workspace = workbench
            .snapshot()
            .navigation
            .workspace(workspace_id)
            .ok_or(WorkspaceIdleFailure::NotFound)?;
        let membership = workbench
            .presentation_membership(&workspace.terminal_context_id)
            .map_err(|_invariant| WorkspaceIdleFailure::Unavailable)?;
        let session_id = workbench.session_id();
        Ok(membership
            .members()
            .iter()
            .map(|tab| IdleCheckMember {
                tab: tab.clone(),
                runtime: Self::idle_close_member(
                    &self.terminals,
                    session_id,
                    tab.binding(),
                    serving,
                )
                .ok(),
            })
            .collect())
    }
}
