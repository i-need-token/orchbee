//! One Close uses the existing Refresh lease, with evidence retained before publication.
use super::{
    COMMAND_CAPACITY, CommandMailbox, PendingRefreshStage, QueuedCommand, QueuedRecord,
    RefreshFlight, RefreshMarker, workbench_identity_candidate,
};
use crate::local_client::LocalOperationClock;
use crate::workbench_state::workspace_close::{
    WorkspaceCloseEvidence, WorkspaceCloseOperation, WorkspaceCloseResult, WorkspaceCloseToken,
};
use crate::workbench_state::{OperationDeadlines, WorkbenchClientState};

pub(super) struct WorkspaceCloseFlight {
    serial: u64,
    evidence: WorkspaceCloseEvidence,
}
impl CommandMailbox {
    pub(in crate::workbench_client) fn try_enqueue_workspace_close_for(
        &self,
        canonical: &mut WorkbenchClientState,
        token: WorkspaceCloseToken,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> bool {
        self.enqueue_workspace_close(
            canonical,
            deadlines,
            clock,
            |state, request_id, generation| {
                state.plan_workspace_close(token, request_id, generation, deadlines)
            },
        )
    }
    pub(in crate::workbench_client) fn try_enqueue_sidebar_workspace_delete_for(
        &self,
        canonical: &mut WorkbenchClientState,
        request: &crate::workbench_state::workspace_close::WorkspaceDeleteRequest,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> bool {
        let deadlines = deadlines.bounded_by(request.deadline());
        self.enqueue_workspace_close(
            canonical,
            deadlines,
            clock,
            |state, request_id, generation| {
                state.plan_sidebar_workspace_delete(request, request_id, generation, deadlines)
            },
        )
    }
    fn enqueue_workspace_close(
        &self,
        canonical: &mut WorkbenchClientState,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
        plan: impl FnOnce(
            &WorkbenchClientState,
            orchbee_workbench_protocol::WorkbenchRequestId,
            u64,
        ) -> Option<WorkspaceCloseOperation>,
    ) -> bool {
        let Some((mut state, _)) = self.lock_before(deadlines.forward_cutoff(), clock) else {
            return false;
        };
        if Self::ensure_open(&state).is_err()
            || state.desired.awaiting_bootstrap()
            || state.desired.key() != canonical.desired()
            || state.refresh_flight.is_some()
            || state.structural_mutation_pending.is_some()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state.queue.len() == COMMAND_CAPACITY
            || state
                .queue
                .iter()
                .any(|entry| entry.command.barrier().is_some())
        {
            return false;
        }
        let Some((cursor, request_id)) = workbench_identity_candidate(state.identity_cursor) else {
            return false;
        };
        let Some(serial) = state.next_command_serial.checked_add(1) else {
            return false;
        };
        let Some(generation) = state.refresh_generation.checked_add(1) else {
            return false;
        };
        let Some(operation) = plan(canonical, request_id, generation) else {
            return false;
        };
        let mut marker = RefreshMarker::new(generation, deadlines.total());
        marker.context_deadlines = Some(deadlines);
        marker.purpose = super::RefreshPurpose::WorkspaceClose(Box::new(operation.clone()));
        let flight = WorkspaceCloseFlight {
            serial,
            evidence: WorkspaceCloseEvidence {
                operation: operation.clone(),
                result: WorkspaceCloseResult::NotSubmitted,
            },
        };
        if clock.now() >= deadlines.forward_cutoff() || !canonical.begin_workspace_close(operation)
        {
            return false;
        }
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.refresh_generation = generation;
        state.refresh_flight = Some(RefreshFlight {
            generation,
            stage: PendingRefreshStage::Queued,
        });
        state.workspace_close_flight = Some(flight);
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Refresh(marker),
            retry_at: None,
        });
        self.changed.notify_all();
        true
    }
    pub(in crate::workbench_client) fn retain_workspace_close_result(
        &self,
        operation: &WorkspaceCloseOperation,
        result: WorkspaceCloseResult,
    ) -> bool {
        let mut state = self.lock();
        let serial = state.active_lease.map(|lease| lease.serial);
        let Some(flight) = state.workspace_close_flight.as_mut() else {
            return false;
        };
        if flight.serial != serial.unwrap_or(0)
            || flight.evidence.operation != *operation
            || !operation.admits(&result)
        {
            return false;
        }
        if !crate::workbench_state::workspace_close::retains_close_result(
            &flight.evidence.result,
            &result,
        ) {
            return false;
        }
        flight.evidence.result = result;
        true
    }
    pub(in crate::workbench_client) fn workspace_close_worker_loss(
        &self,
    ) -> Option<WorkspaceCloseEvidence> {
        self.lock()
            .workspace_close_flight
            .as_ref()
            .map(|flight| flight.evidence.clone())
    }
    pub(super) fn workspace_close_event_matches(
        state: &super::MailboxState,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        state.workspace_close_flight.as_ref().is_some_and(|flight| {
            flight.evidence == *evidence
                && state.active_lease.map(|lease| lease.serial) == Some(flight.serial)
        })
    }
}
