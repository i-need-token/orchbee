//! Canonical membership fence for ephemeral visual layers.
use super::SessionCoordinator;
use orchbee_workbench_protocol::{PaneGraphicsFailure as Failure, SessionId, Snapshot};
impl SessionCoordinator {
    pub(crate) fn graphics_owner(
        &self,
    ) -> Option<std::sync::Arc<std::sync::Mutex<orchbee_server::PaneGraphicsAuthority>>> {
        Some(self.workbench.as_ref()?.graphics_owner())
    }
    pub(crate) fn graphics_snapshot(&self, session: &SessionId) -> Result<Snapshot, Failure> {
        let workbench = self.workbench.as_ref().ok_or(Failure::Unavailable)?;
        if workbench.session_id() != session {
            return Err(Failure::SessionMismatch);
        }
        self.terminal_agent_bindings()
            .map_err(|_| Failure::Unavailable)?;
        Ok(workbench.snapshot().clone())
    }
}
