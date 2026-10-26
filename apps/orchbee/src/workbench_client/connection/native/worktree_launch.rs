//! Canonical Workspace directory or exact primary root cwd under the ordinary refresh lease.
use super::{
    CarrierFailure, ClientExecutor, CommandLease, EventDeadline, MutationResult, Result,
    WorkbenchSnapshotOutcome, WorkerEvent, closed, event,
};
use crate::workbench_client::state::worktree_launch::Preparation;
use orchbee_workbench_protocol::WorktreeSource;
impl ClientExecutor {
    pub(super) fn run_worktree_preparation(
        &mut self,
        lease: &CommandLease,
        marker: &crate::workbench_client::mailbox::RefreshMarker,
        p: Preparation,
    ) -> Result<()> {
        if self
            .mailbox
            .mark_refresh_in_flight(p.generation)
            .map_err(closed)?
            != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        let request = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        let outcome = self.read_workbench_before(request, p.deadlines.forward_cutoff())?;
        let source = self.prepare_worktree_source(&p, &outcome)?;
        let wait = self
            .mailbox
            .publish_event_before(
                WorkerEvent::WorktreePrepared {
                    preparation: Box::new(p.clone()),
                    source,
                    outcome: Box::new(outcome),
                },
                EventDeadline::PlacementOnly(p.deadlines.forward_cutoff()),
                &self.clock,
            )
            .map_err(event)?;
        self.finish_refresh(lease, marker, &wait)
    }
    fn prepare_worktree_source(
        &mut self,
        p: &Preparation,
        outcome: &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorktreeSource>> {
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Ok(None);
        };
        if p.origin.cache.project_snapshot(snapshot).ok().as_ref() != Some(&p.origin.cache) {
            return Ok(None);
        }
        if p.origin.cache.directory().is_some() {
            let crate::workbench_client::workbench_projection::ClientContextLocation::Workspace {
                workspace_id,
                ..
            } = p.origin.cache.location()
            else {
                return Err(CarrierFailure::Protocol);
            };
            return Ok(Some(WorktreeSource::Workspace {
                workspace_id: workspace_id.clone(),
            }));
        }
        let Some(binding) = &p.primary else {
            return Ok(None);
        };
        match self.read_source_root(
            &p.origin.cache,
            p.primary_pane.as_ref(),
            p.deadlines,
            binding,
        )? {
            super::source_directory::RootDirectory::Available(Some(path)) => {
                Ok(Some(WorktreeSource::Directory { path }))
            }
            _ => Ok(None),
        }
    }
}
#[cfg(test)]
mod tests;
