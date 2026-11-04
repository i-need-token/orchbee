//! Where the Server says the focused Panel's terminal stands, read under the ordinary refresh lease.
use super::{
    CarrierFailure, ClientExecutor, CommandLease, EventDeadline, MutationResult, Result,
    WorkbenchSnapshotOutcome, WorkerEvent, closed, event,
};
use crate::workbench_state::worktree_launch::Preparation;
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
    pub(super) fn prepare_worktree_source(
        &mut self,
        p: &Preparation,
        outcome: &WorkbenchSnapshotOutcome,
    ) -> Result<Option<WorktreeSource>> {
        let WorkbenchSnapshotOutcome::Available { snapshot } = outcome else {
            return Ok(None);
        };
        if !p
            .origin
            .cache
            .project_snapshot(snapshot)
            .is_ok_and(|projected| p.origin.source_current_in(&projected))
        {
            return Ok(None);
        }
        let crate::workbench_projection::ClientContextLocation::Workspace { workspace_id, .. } =
            p.origin.cache.location()
        else {
            return Ok(None);
        };
        // The terminal the focused Panel shows, as a person who just changed directory there
        // expects; a Workspace without a focused Panel reads its first terminal.
        let panel = p.origin.member.as_ref().map(|member| {
            orchbee_workbench_protocol::CurrentDirectoryPanel {
                panel_id: member.panel.clone(),
                presentation_id: member.presentation.clone(),
            }
        });
        let observed = self.read_current_directory(
            p.origin.cache.session_id(),
            workspace_id,
            panel,
            p.deadlines.forward_cutoff(),
        )?;
        Ok(p.origin.source(observed))
    }
}
