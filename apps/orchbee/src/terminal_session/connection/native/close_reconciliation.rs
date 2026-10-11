use super::{
    CarrierFailure, CommandLease, MutationResult, NativeExecutor, RefreshDecision, Result,
    SetupEntryExpiry, SetupOwner, SetupTarget, UiReply, closed, event,
};
use crate::terminal_session::state::CloseReconciliationMarker;

impl NativeExecutor {
    pub(super) fn run_close_reconciliation(
        &mut self,
        lease: &CommandLease,
        marker: &CloseReconciliationMarker,
    ) -> Result<()> {
        if self
            .mailbox
            .mark_close_reconciliation_in_flight(marker)
            .map_err(closed)?
            != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        let cutoff = self.effective_lease_cutoff(lease, marker.deadline())?;
        self.ensure_before(cutoff)?;
        let outcome = self.read_workbench_before(marker.snapshot_request_id().clone(), cutoff)?;
        let wait = self
            .mailbox
            .publish_close_reconciliation_before(marker.clone(), outcome, cutoff, &self.clock)
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?;
        let UiReply::CloseReconciliation(reply) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        if reply.marker() != marker {
            return Err(CarrierFailure::Protocol);
        }
        match reply.settlement().decision() {
            RefreshDecision::Cleanup(cleanup) => {
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            RefreshDecision::Continue => Ok(()),
            RefreshDecision::SettledWithoutAttachment => {
                self.status_only = false;
                Ok(())
            }
            RefreshDecision::AttachRecovered(target) => {
                self.status_only = false;
                self.setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_| ())
            }
            RefreshDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
        }
    }
}
