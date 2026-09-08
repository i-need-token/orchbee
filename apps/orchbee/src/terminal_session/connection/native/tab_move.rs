use super::{NativeExecutor, Result, closed, event, snapshot};
use crate::{
    local_client::workbench::{
        WorkbenchTabMoveNotSubmitted, WorkbenchTabMoveOutcomeUnknownReason,
        WorkbenchTabMovePrecommit, WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
    },
    terminal_session::{
        mailbox::{CommandLease, MutationResult, UiReply},
        state::{CarrierFailure, MoveDecision, MoveOperation, MoveResult, PendingMoveCommit},
    },
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SnapshotReadIntent, WorkbenchAction, WorkbenchCommand,
    WorkbenchRejectionReason, WorkbenchSnapshotOutcome,
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
                terminal_context_id: baseline.terminal_context_id().clone(),
                tab_id: operation.target().tab_id().clone(),
                direction: operation.direction(),
            },
        })
        .map_err(|_command| CarrierFailure::Protocol)?;
        let result = match self.client.submit_workbench_tab_move(request, cutoff) {
            WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(reason) => {
                if matches!(reason, WorkbenchTabMoveNotSubmitted::SessionUnavailable)
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
        if settlement.desired() != operation.target() {
            return Err(CarrierFailure::Protocol);
        }
        if self
            .pending_move
            .as_ref()
            .is_some_and(|pending| pending.operation() != &operation)
        {
            return Err(CarrierFailure::Protocol);
        }
        match settlement.decision() {
            MoveDecision::Continue => {
                self.pending_move = None;
                Ok(())
            }
            MoveDecision::Retire => Err(CarrierFailure::AuthorityDivergence),
        }
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
        self.client
            .submit_workbench_snapshot_read_before(
                SnapshotReadIntent {
                    protocol_version: ProtocolVersion,
                    request_id,
                },
                cutoff,
            )
            .map(|snapshot| snapshot.outcome)
            .map_err(snapshot)
    }
}

fn move_unknown(
    _request_id: orchbee_workbench_protocol::WorkbenchRequestId,
    _reason: WorkbenchTabMoveOutcomeUnknownReason,
) -> CarrierFailure {
    CarrierFailure::OutcomeUnknown
}
