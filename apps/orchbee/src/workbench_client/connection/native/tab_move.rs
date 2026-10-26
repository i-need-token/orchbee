use super::{NativeExecutor, Result, SetupEntryExpiry, SetupOwner, SetupTarget, closed, event};
use crate::{
    local_client::workbench::{
        WorkbenchTabMoveNotSubmitted, WorkbenchTabMoveOutcomeUnknownReason,
        WorkbenchTabMovePrecommit, WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
    },
    workbench_client::{
        mailbox::{CommandLease, MutationResult, UiReply},
        state::{
            CarrierFailure, MoveDecision, MoveOperation, MoveResult, PaneKey, PendingMoveCommit,
        },
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchRejectionReason,
    WorkbenchSnapshotOutcome,
};

impl NativeExecutor {
    pub(super) fn run_move(
        &mut self,
        lease: &CommandLease,
        operation: MoveOperation,
    ) -> Result<()> {
        if self
            .mailbox
            .mark_move_in_flight(&operation)
            .map_err(closed)?
            != MutationResult::Applied
        {
            return Err(CarrierFailure::Protocol);
        }
        let cutoff = self.effective_lease_cutoff(lease, operation.deadline())?;
        let baseline = operation.baseline();
        let request = WorkbenchTabMoveRequest::try_from(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: operation.request_id().clone(),
            site_id: baseline.site_id().clone(),
            expected_revision: baseline.revision(),
            action: WorkbenchAction::TabMove {
                terminal_context_id: baseline
                    .terminal_context_id()
                    .ok_or(CarrierFailure::Protocol)?
                    .clone(),
                tab_id: operation.target().tab_id().clone(),
                direction: operation.direction(),
            },
        })
        .map_err(|_command| CarrierFailure::Protocol)?;
        let result = match self.client.submit_workbench_tab_move(request, cutoff) {
            WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(reason) => {
                if matches!(reason, WorkbenchTabMoveNotSubmitted::ConnectionUnavailable)
                    || self.client.generation().is_none()
                {
                    return Err(CarrierFailure::Transport);
                }
                MoveResult::NotSubmitted
            }
            WorkbenchTabMoveSubmission::DefinitePrecommit(
                WorkbenchTabMovePrecommit::Rejected {
                    revision,
                    reason: WorkbenchRejectionReason::TabUnavailable,
                },
            ) => MoveResult::TabUnavailable {
                revision,
                outcome: self.read_move_snapshot(cutoff)?,
            },
            WorkbenchTabMoveSubmission::DefinitePrecommit(outcome) => {
                MoveResult::Precommit(outcome)
            }
            WorkbenchTabMoveSubmission::Unchanged { revision } => MoveResult::Unchanged(revision),
            WorkbenchTabMoveSubmission::OutcomeUnknown { request_id, reason } => {
                // The adapter retires every ambiguous handoff; no new identity can replay it.
                return Err(move_unknown(request_id, reason));
            }
            WorkbenchTabMoveSubmission::Committed(commit) => {
                if self.pending_move.is_some() || self.pending_create.is_some() {
                    return Err(CarrierFailure::Protocol);
                }
                self.pending_move = Some(PendingMoveCommit::new(operation.clone(), commit.clone()));
                // Keep the commit worker-owned through Snapshot failure, timed deposit and join.
                MoveResult::Committed {
                    commit,
                    outcome: self.read_move_snapshot(cutoff)?,
                }
            }
        };
        let wait = self
            .mailbox
            .publish_move_result_before(operation.clone(), result, cutoff, &self.clock)
            .map_err(event)?;
        let reply = self
            .mailbox
            .await_ui_reply(&wait)
            .map_err(closed)?
            .ok_or(CarrierFailure::Protocol)?;
        let UiReply::Move(settlement) = reply else {
            return Err(CarrierFailure::Protocol);
        };
        if self
            .pending_move
            .as_ref()
            .is_some_and(|pending| pending.operation() != &operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        let result = match settlement.decision() {
            MoveDecision::Continue => {
                if settlement.desired().map(PaneKey::anchor) != Some(operation.target()) {
                    return Err(CarrierFailure::Protocol);
                }
                Ok(())
            }
            MoveDecision::Cleanup(cleanup) => {
                if cleanup.old() != operation.target()
                    || cleanup.result()
                        != &crate::workbench_client::state::MembershipResultKey::Move(
                            operation.request_id().clone(),
                        )
                    || settlement.desired().map(PaneKey::anchor) != Some(operation.target())
                {
                    return Err(CarrierFailure::Protocol);
                }
                self.run_membership_cleanup(lease, cleanup.clone())
            }
            MoveDecision::AttachRecovered(target) => {
                if settlement.desired() != Some(target.selected().key())
                    || !self.attachments.is_empty()
                {
                    return Err(CarrierFailure::Protocol);
                }
                self.setup_target(
                    SetupTarget::operation(target.clone())?,
                    SetupOwner::Lease(lease),
                    SetupEntryExpiry::Retire,
                )
                .map(|_| ())
            }
            MoveDecision::SettledWithoutAttachment if self.attachments.is_empty() => Ok(()),
            MoveDecision::SettledWithoutAttachment => Err(CarrierFailure::Protocol),
            MoveDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
        };
        if result.is_ok() {
            self.pending_move = None;
        }
        result
    }

    fn read_move_snapshot(
        &mut self,
        cutoff: std::time::Instant,
    ) -> Result<WorkbenchSnapshotOutcome> {
        self.ensure_before(cutoff)?;
        let request_id = self
            .mailbox
            .allocate_workbench_request_id()
            .map_err(closed)?;
        self.read_workbench_before(request_id, cutoff)
    }
}

fn move_unknown(
    _request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    _reason: WorkbenchTabMoveOutcomeUnknownReason,
) -> CarrierFailure {
    CarrierFailure::OutcomeUnknown
}
