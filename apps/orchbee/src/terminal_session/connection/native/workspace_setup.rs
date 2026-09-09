use super::{
    NativeExecutor, Result, SetupEntryExpiry, SetupOwner, SetupTarget, closed, event, snapshot,
};
use crate::local_client::workbench::workspace_setup::{
    WorkbenchSetupNotSubmitted, WorkbenchSetupSubmission,
};
use crate::terminal_session::state::workspace_setup::{
    SetupOperation, SetupResult, SetupWorkerEvidence,
};
use crate::terminal_session::{
    mailbox::{CommandLease, MutationResult, UiReply},
    state::{CarrierFailure, RefreshDecision},
};
use orchbee_workbench_protocol::{ProtocolVersion, SnapshotReadIntent};
impl NativeExecutor {
    pub(super) fn run_workspace_setup(
        &mut self,
        lease: &CommandLease,
        operation: SetupOperation,
    ) -> Result<()> {
        if self
            .mailbox
            .mark_setup_in_flight(&operation)
            .map_err(closed)?
            != MutationResult::Applied
            || self.pending_setup.is_some()
        {
            return Err(CarrierFailure::Protocol);
        }
        self.pending_setup = Some(SetupWorkerEvidence::Unknown(operation.clone()));
        let cutoff = match self.effective_lease_cutoff(lease, operation.deadlines.forward_cutoff())
        {
            Ok(cutoff) => cutoff,
            Err(failure) => {
                self.pending_setup = Some(SetupWorkerEvidence::NotApplied(operation.clone()));
                self.mailbox.retain_setup_not_applied(&operation);
                return Err(failure);
            }
        };
        let outcome = match self
            .client
            .submit_workbench_setup(operation.request.clone(), cutoff)
        {
            WorkbenchSetupSubmission::DefinitelyNotSubmitted(reason) => {
                self.pending_setup = Some(SetupWorkerEvidence::NotApplied(operation.clone()));
                if !self.mailbox.retain_setup_not_applied(&operation) {
                    return Err(CarrierFailure::Protocol);
                }
                if matches!(reason, WorkbenchSetupNotSubmitted::SessionUnavailable)
                    || self.client.generation().is_none()
                {
                    return Err(CarrierFailure::Transport);
                }
                SetupResult::NotApplied
            }
            WorkbenchSetupSubmission::DefinitePrecommit(_) => {
                self.pending_setup = Some(SetupWorkerEvidence::NotApplied(operation.clone()));
                if !self.mailbox.retain_setup_not_applied(&operation) {
                    return Err(CarrierFailure::Protocol);
                }
                SetupResult::NotApplied
            }
            WorkbenchSetupSubmission::OutcomeUnknown { request_id, .. } => {
                return Err(if request_id == operation.command.request_id {
                    CarrierFailure::OutcomeUnknown
                } else {
                    CarrierFailure::Protocol
                });
            }
            WorkbenchSetupSubmission::Committed(commit) => {
                let known = operation
                    .known_commit(commit)
                    .ok_or(CarrierFailure::AuthorityDivergence)?;
                self.pending_setup = Some(SetupWorkerEvidence::Known(known.clone()));
                if !self.mailbox.retain_setup_known(&known) {
                    return Err(CarrierFailure::Protocol);
                }
                self.ensure_before(cutoff)?;
                let request_id = self
                    .mailbox
                    .allocate_workbench_request_id()
                    .map_err(closed)?;
                let snapshot_result = self
                    .client
                    .submit_workbench_snapshot_read_before(
                        SnapshotReadIntent {
                            protocol_version: ProtocolVersion,
                            request_id,
                        },
                        cutoff,
                    )
                    .map_err(snapshot)?;
                SetupResult::Committed {
                    known: Box::new(known),
                    outcome: Box::new(snapshot_result.outcome),
                }
            }
        };
        let wait = self
            .mailbox
            .publish_setup_result_before(operation.clone(), outcome, &self.clock)
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply_before(&wait, operation.deadlines.total(), &self.clock)
            .map_err(event)?
            .ok_or(CarrierFailure::Protocol)?;
        let UiReply::Refresh(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        let result = match settlement.decision() {
            RefreshDecision::Continue | RefreshDecision::SettledWithoutAttachment => Ok(()),
            RefreshDecision::Cleanup(cleanup) => {
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            RefreshDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
            RefreshDecision::AttachRecovered(target) => self
                .setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_| ()),
        };
        if result.is_ok() {
            self.pending_setup = None;
        }
        result
    }
}
