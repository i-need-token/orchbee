//! One refresh barrier and an evidence sidecar for the explicit control gesture.
use super::{
    COMMAND_CAPACITY, CommandMailbox, PendingRefreshStage, QueuedCommand, QueuedRecord,
    RefreshFlight, RefreshMarker, RefreshPurpose, workbench_identity_candidate,
};
use crate::{
    local_client::LocalOperationClock,
    workbench_client::state::{
        OperationDeadlines, WorkbenchClientState,
        control_takeover::{TakeoverEvidence, TakeoverOperation, TakeoverResult},
    },
};
use orchbee_client_server_protocol::RequestId;

pub(super) struct TakeoverFlight {
    serial: u64,
    evidence: TakeoverEvidence,
}
impl CommandMailbox {
    pub(in crate::workbench_client) fn try_enqueue_takeover_for(
        &self,
        canonical: &mut WorkbenchClientState,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
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
        let Ok(request_id) = RequestId::parse(request_id.as_str()) else {
            return false;
        };
        let Some(serial) = state.next_command_serial.checked_add(1) else {
            return false;
        };
        let Some(generation) = state.refresh_generation.checked_add(1) else {
            return false;
        };
        let Some(operation) = canonical.plan_takeover(request_id, generation, deadlines) else {
            return false;
        };
        if clock.now() >= deadlines.forward_cutoff() || !canonical.begin_takeover(operation.clone())
        {
            return false;
        }
        let mut marker = RefreshMarker::new(generation, deadlines.total());
        marker.context_deadlines = Some(deadlines);
        marker.purpose = RefreshPurpose::Takeover(Box::new(operation.clone()));
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.refresh_generation = generation;
        state.refresh_flight = Some(RefreshFlight {
            generation,
            stage: PendingRefreshStage::Queued,
        });
        state.takeover_flight = Some(TakeoverFlight {
            serial,
            evidence: TakeoverEvidence {
                operation,
                result: TakeoverResult::NotSubmitted,
            },
        });
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Refresh(marker),
            retry_at: None,
        });
        self.changed.notify_all();
        true
    }
    pub(in crate::workbench_client) fn retain_takeover_result(
        &self,
        operation: &TakeoverOperation,
        result: TakeoverResult,
    ) -> bool {
        let mut state = self.lock();
        let serial = state.active_lease.map(|lease| lease.serial);
        let Some(flight) = state.takeover_flight.as_mut() else {
            return false;
        };
        if serial != Some(flight.serial)
            || flight.evidence.operation != *operation
            || !result.retains(&flight.evidence.result)
        {
            return false;
        }
        flight.evidence.result = result;
        true
    }
    pub(in crate::workbench_client) fn takeover_worker_loss(&self) -> Option<TakeoverEvidence> {
        self.lock()
            .takeover_flight
            .as_ref()
            .map(|flight| flight.evidence.clone())
    }
    pub(super) fn takeover_event_matches(
        state: &super::MailboxState,
        evidence: &TakeoverEvidence,
    ) -> bool {
        state.takeover_flight.as_ref().is_some_and(|flight| {
            flight.evidence == *evidence
                && state.active_lease.map(|lease| lease.serial) == Some(flight.serial)
        })
    }
}
