use super::*;
use crate::local_client::workbench::label_rename::{
    WorkbenchLabelRenameNotSubmitted, WorkbenchLabelRenameSubmission,
};
use crate::terminal_session::{
    mailbox::MutationResult,
    state::tab_rename::{RenameOperation, RenameResult, RenameWorkerEvidence},
};

impl NativeExecutor {
    pub(super) fn run_rename(
        &mut self,
        lease: &CommandLease,
        operation: RenameOperation,
    ) -> Result<()> {
        if self
            .mailbox
            .mark_rename_in_flight(&operation)
            .map_err(closed)?
            != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        let cutoff = self.effective_lease_cutoff(lease, operation.deadline())?;
        if self.pending_rename.is_some() {
            return Err(CarrierFailure::Protocol);
        }
        self.pending_rename = Some(RenameWorkerEvidence::Unknown(operation.clone()));
        let outcome = match self
            .client
            .submit_workbench_label_rename(operation.request().clone(), cutoff)
        {
            WorkbenchLabelRenameSubmission::DefinitelyNotSubmitted(reason) => {
                self.pending_rename = None;
                if matches!(reason, WorkbenchLabelRenameNotSubmitted::SessionUnavailable)
                    || self.client.generation().is_none()
                {
                    return Err(CarrierFailure::Transport);
                }
                RenameResult::NotSubmitted
            }
            WorkbenchLabelRenameSubmission::DefinitePrecommit(result) => {
                self.pending_rename = None;
                RenameResult::Precommit(result)
            }
            WorkbenchLabelRenameSubmission::Unchanged { revision } => {
                if !matches!(
                    operation
                        .prepare(RenameResult::Unchanged(revision))
                        .result(),
                    crate::terminal_session::state::tab_rename::RenamePreparation::Unchanged
                ) {
                    return Err(CarrierFailure::AuthorityDivergence);
                }
                self.pending_rename = None;
                RenameResult::Unchanged(revision)
            }
            WorkbenchLabelRenameSubmission::OutcomeUnknown { request_id, .. } => {
                return Err(if &request_id == operation.request_id() {
                    CarrierFailure::OutcomeUnknown
                } else {
                    CarrierFailure::Protocol
                });
            }
            WorkbenchLabelRenameSubmission::Committed { revision } => {
                let known = operation
                    .known_commit(revision)
                    .ok_or(CarrierFailure::AuthorityDivergence)?;
                self.pending_rename = Some(RenameWorkerEvidence::Known(known));
                self.ensure_before(cutoff)?;
                let request_id = self
                    .mailbox
                    .allocate_workbench_request_id()
                    .map_err(closed)?;
                let outcome = self.read_workbench_before(request_id, cutoff)?;
                RenameResult::Committed { revision, outcome }
            }
        };
        let wait = self
            .mailbox
            .publish_rename_result_before(operation.clone(), outcome, cutoff, &self.clock)
            .map_err(event)?;
        let UiReply::Rename(settlement) = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?
        else {
            return Err(CarrierFailure::Protocol);
        };
        if settlement.request_id() != operation.request_id() {
            return Err(CarrierFailure::Protocol);
        }
        let result = match settlement.decision() {
            RefreshDecision::Continue | RefreshDecision::SettledWithoutAttachment => Ok(()),
            RefreshDecision::AttachRecovered(target) => self
                .setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_| ()),
            RefreshDecision::Cleanup(cleanup) => {
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            RefreshDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
        };
        if result.is_ok() {
            self.pending_rename = None;
        }
        result
    }
}
