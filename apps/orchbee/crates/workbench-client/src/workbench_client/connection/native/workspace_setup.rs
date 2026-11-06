//! Submits a captured Workspace setup operation and preserves known commit or pre-submission
//! evidence through the canonical snapshot handoff. It does not own the draft or install the
//! resulting Client membership.

use super::{ClientExecutor, Result, SetupEntryExpiry, SetupOwner, SetupTarget, closed, event};
use crate::local_client::workbench::workspace_setup::{
    WorkbenchSetupNotSubmitted, WorkbenchSetupPrecommit, WorkbenchSetupSubmission,
};
use crate::workbench_client::mailbox::{CommandLease, MutationResult, UiReply};
use crate::workbench_state::workspace_setup::{SetupOperation, SetupResult, SetupWorkerEvidence};
use crate::workbench_state::{CarrierFailure, RefreshDecision};

impl ClientExecutor {
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
                if matches!(reason, WorkbenchSetupNotSubmitted::ConnectionUnavailable)
                    || self.client.generation().is_none()
                {
                    return Err(CarrierFailure::Transport);
                }
                SetupResult::NotApplied
            }
            WorkbenchSetupSubmission::DefinitePrecommit(reason) => {
                self.pending_setup = Some(SetupWorkerEvidence::NotApplied(operation.clone()));
                if !self.mailbox.retain_setup_not_applied(&operation) {
                    return Err(CarrierFailure::Protocol);
                }
                if matches!(
                    operation.command.action,
                    orchbee_workbench_protocol::WorkbenchAction::ProjectDissolveIfIdle { .. }
                ) && matches!(&reason, WorkbenchSetupPrecommit::Rejected {
                        revision, reason: orchbee_workbench_protocol::WorkbenchRejectionReason::ConfirmationRequired,
                    } if revision == &operation.command.expected_revision)
                {
                    if !self
                        .mailbox
                        .retain_setup_confirmation_required(&operation, &reason)
                    {
                        return Err(CarrierFailure::Protocol);
                    }
                    SetupResult::ConfirmationRequired
                } else if matches!(&reason, WorkbenchSetupPrecommit::Rejected { revision, reason }
                    if *reason == orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionMismatch
                        || revision != &operation.command.expected_revision)
                {
                    SetupResult::Stale
                } else if matches!(&reason, WorkbenchSetupPrecommit::Rejected {
                    reason: orchbee_workbench_protocol::WorkbenchRejectionReason::LaunchRequestTooLarge,
                    ..
                }) {
                    SetupResult::LaunchRequestTooLarge
                } else {
                    SetupResult::NotApplied
                }
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
                if let crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit::WorkspaceLaunched(commit) = &known.commit
                    && !matches!(
                        commit.terminal_outcome,
                        Some(orchbee_workbench_protocol::WorkbenchTerminalOutcome::Started { .. })
                    )
                {
                    return Err(if matches!(
                        commit.terminal_outcome,
                        Some(orchbee_workbench_protocol::WorkbenchTerminalOutcome::OutcomeUnknown { .. })
                    ) {
                        CarrierFailure::OutcomeUnknown
                    } else {
                        CarrierFailure::Transport
                    });
                }
                self.ensure_before(cutoff)?;
                let request_id = self
                    .mailbox
                    .allocate_workbench_request_id()
                    .map_err(closed)?;
                let outcome = self.read_workbench_before(request_id, cutoff)?;
                SetupResult::Committed {
                    known: Box::new(known),
                    outcome: Box::new(outcome),
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
