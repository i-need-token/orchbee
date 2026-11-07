//! Joins an earlier layout with the committed Workbench under the Session gate: it lists how
//! each kept layout compares with what is open, grafts the missing Workspaces and Tabs back, and
//! names the restore targets of the reopened Presentations. It probes no directory and launches
//! nothing; the caller decides each target outside the gate and reserves through restore's path.

use super::SessionCoordinator;
use orchbee_server::{
    LayoutReopenError, ReopenedLayout, WorkbenchCheckpoint, WorkbenchRestoreTarget,
};
use orchbee_workbench_protocol::{LayoutHistoryEntry, SessionId, WorkerId};
use std::io;

impl SessionCoordinator {
    /// Summaries of every kept layout against the committed Workbench, and its Session.
    pub(crate) fn layout_history_summaries(
        &self,
        history: &crate::layout_history::LayoutHistory,
    ) -> io::Result<(SessionId, Vec<LayoutHistoryEntry>)> {
        let workbench = self
            .workbench
            .as_ref()
            .ok_or_else(|| io::Error::other("Workbench unavailable"))?;
        Ok((
            workbench.session_id().clone(),
            history.summaries(&workbench.open_layout())?,
        ))
    }

    /// Commits the missing part of `entry` and returns what it opened with the restore target of
    /// each reopened Presentation, in launch order.
    pub(crate) fn reopen_layout(
        &mut self,
        entry: &WorkbenchCheckpoint,
        owner: &WorkerId,
    ) -> Result<(ReopenedLayout, Vec<WorkbenchRestoreTarget>), LayoutReopenError> {
        let workbench = self
            .workbench
            .as_mut()
            .ok_or(LayoutReopenError::Unavailable)?;
        let reopened = workbench.reopen_layout(entry, owner)?;
        let mut targets = Vec::with_capacity(reopened.presentations.len());
        for presentation in &reopened.presentations {
            if let Some(target) = workbench
                .restoration_target(owner, presentation)
                .map_err(|_| LayoutReopenError::Unavailable)?
            {
                targets.push(target);
            }
        }
        Ok((reopened, targets))
    }

    /// Holds a reopened target whose directory is unavailable, as restore holds one. A target
    /// that changed since the reopen is left as it is.
    pub(crate) fn hold_reopened_terminal(
        &mut self,
        target: &WorkbenchRestoreTarget,
        directory: orchbee_workbench_protocol::WorkbenchDirectoryPath,
    ) {
        if let Some(workbench) = self.workbench.as_mut() {
            let _changed = workbench.hold_restored_terminal(target, directory, false);
        }
    }
}
