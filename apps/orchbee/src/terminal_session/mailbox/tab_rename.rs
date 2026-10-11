use super::{
    BarrierFence, BarrierKind, COMMAND_CAPACITY, CommandMailbox, EventDeadline, EventError,
    Instant, LocalOperationClock, MailboxClock, MailboxClosed, MailboxState, MembershipResultKey,
    MoveAdmissionError, MutationResult, PaneKey, PendingRefreshStage, QueuedCommand, QueuedRecord,
    RefreshDecision, StructuralMutationFlight, UiCorrelation, UiWaitKey, WorkbenchRequestId,
    WorkerEvent, workbench_identity_candidate,
};
use crate::terminal_session::state::{
    NativeSessionState,
    rename_draft::RenameDraftToken,
    tab_rename::{RenameOperation, RenameResult},
};

pub(super) struct RenameFlight {
    pub(super) serial: u64,
    pub(super) operation: RenameOperation,
    pub(super) stage: PendingRefreshStage,
    result: Option<RenameResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameMarker(RenameOperation);
impl RenameMarker {
    pub(in crate::terminal_session) fn operation(&self) -> &RenameOperation {
        &self.0
    }
    pub(super) fn barrier(&self) -> BarrierFence {
        BarrierFence {
            kind: BarrierKind::Rename,
            forward_cutoff: self.0.deadline(),
            total: self.0.deadline(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct RenameSettlement {
    request_id: WorkbenchRequestId,
    decision: RefreshDecision,
    desired: Option<PaneKey>,
}
impl RenameSettlement {
    pub(in crate::terminal_session) fn new(
        request_id: WorkbenchRequestId,
        decision: RefreshDecision,
        desired: Option<PaneKey>,
    ) -> Self {
        Self {
            request_id,
            decision,
            desired,
        }
    }
    pub(in crate::terminal_session) fn request_id(&self) -> &WorkbenchRequestId {
        &self.request_id
    }
    pub(in crate::terminal_session) fn decision(&self) -> &RefreshDecision {
        &self.decision
    }
    pub(in crate::terminal_session) fn desired(&self) -> Option<&PaneKey> {
        self.desired.as_ref()
    }
}

impl CommandMailbox {
    pub(in crate::terminal_session) fn open_rename_draft_for(
        &self,
        canonical: &mut NativeSessionState,
    ) -> Option<RenameDraftToken> {
        let state = self.lock();
        if Self::ensure_open(&state).is_err()
            || state.desired.awaiting_bootstrap()
            || state.desired.key().is_none()
            || state.desired.key() != canonical.desired()
            || state.structural_mutation_pending.is_some()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state.refresh_flight.is_some()
            || canonical.close_status_only()
            || state
                .queue
                .iter()
                .any(|record| record.command.barrier().is_some())
        {
            return None;
        }
        canonical.open_rename_draft()
    }
    pub(in crate::terminal_session) fn try_enqueue_rename_for(
        &self,
        canonical: &mut NativeSessionState,
        token: RenameDraftToken,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<RenameOperation, MoveAdmissionError<()>> {
        self.try_enqueue_rename_for_using(canonical, token, deadline, clock)
    }
    pub(super) fn try_enqueue_rename_for_using(
        &self,
        canonical: &mut NativeSessionState,
        token: RenameDraftToken,
        deadline: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<RenameOperation, MoveAdmissionError<()>> {
        let (mut state, _) = self
            .lock_before(deadline, clock)
            .ok_or(MoveAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(MoveAdmissionError::Closed)?;
        if state.desired.awaiting_bootstrap()
            || state.desired.key().is_none()
            || state.structural_mutation_pending.is_some()
            || state.close_flight.is_some()
            || state.refresh_flight.is_some()
            || state.close_reconciliation.is_some()
            || canonical.close_status_only()
            || state
                .queue
                .iter()
                .any(|record| record.command.barrier().is_some())
        {
            return Err(MoveAdmissionError::Busy);
        }
        if state.queue.len() == COMMAND_CAPACITY {
            return Err(MoveAdmissionError::Full);
        }
        let (cursor, request_id) = workbench_identity_candidate(state.identity_cursor)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let serial = state
            .next_command_serial
            .checked_add(1)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let operation = canonical
            .plan_rename(token, request_id, deadline)
            .ok_or(MoveAdmissionError::Build(()))?;
        if state.desired.key().map(PaneKey::anchor) != Some(operation.target()) {
            return Err(MoveAdmissionError::InvalidTarget);
        }
        let flight = Box::new(RenameFlight {
            serial,
            operation: operation.clone(),
            stage: PendingRefreshStage::Queued,
            result: None,
        });
        let marker = RenameMarker(operation.clone());
        if !canonical.begin_rename(operation.clone()) {
            return Err(MoveAdmissionError::Build(()));
        }
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.structural_mutation_pending = Some(StructuralMutationFlight::Rename(flight));
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Rename(marker),
            retry_at: None,
        });
        self.changed.notify_all();
        Ok(operation)
    }

    pub(in crate::terminal_session) fn mark_rename_in_flight(
        &self,
        operation: &RenameOperation,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let active = state.active_lease.map(|lease| lease.serial);
        match state.structural_mutation_pending.as_mut() {
            Some(StructuralMutationFlight::Rename(flight))
                if &flight.operation == operation
                    && flight.stage == PendingRefreshStage::Queued
                    && active == Some(flight.serial) =>
            {
                flight.stage = PendingRefreshStage::InFlight;
                Ok(MutationResult::Applied)
            }
            _ => Ok(MutationResult::Stale),
        }
    }

    pub(in crate::terminal_session) fn publish_rename_result_before(
        &self,
        operation: RenameOperation,
        outcome: RenameResult,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before(
            WorkerEvent::RenameResult { operation, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }
}

impl MailboxState {
    pub(super) fn place_rename_result(
        &mut self,
        operation: &RenameOperation,
        outcome: &RenameResult,
        deadline: EventDeadline,
        anchor: Option<u64>,
    ) -> bool {
        let Some(StructuralMutationFlight::Rename(flight)) =
            self.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if &flight.operation != operation
            || flight.stage != PendingRefreshStage::InFlight
            || anchor != Some(flight.serial)
            || deadline != EventDeadline::PlacementOnly(operation.deadline())
        {
            return false;
        }
        flight.stage = PendingRefreshStage::AwaitingUi;
        flight.result = Some(outcome.clone());
        true
    }
    pub(super) fn rename_reply_current(&self, correlation: &UiCorrelation) -> bool {
        let (request_id, old) = match correlation {
            UiCorrelation::Rename(id) => (id, None),
            UiCorrelation::Membership(MembershipResultKey::Rename(id), old) => (id, Some(old)),
            _ => return true,
        };
        matches!(&self.structural_mutation_pending, Some(StructuralMutationFlight::Rename(flight)) if flight.operation.request_id() == request_id
            && flight.stage == PendingRefreshStage::AwaitingUi
            && self.active_lease.map(|lease| lease.serial) == Some(flight.serial)
            && self.event_anchor_serial == Some(flight.serial)
            && self.desired.key().map(PaneKey::anchor) == Some(flight.operation.target())
            && old.is_none_or(|old| old == flight.operation.target()))
    }
}

impl CommandMailbox {
    pub(in crate::terminal_session) fn rename_worker_loss(
        &self,
    ) -> Option<crate::terminal_session::state::tab_rename::RenameWorkerEvidence> {
        use crate::terminal_session::state::tab_rename::RenameWorkerEvidence;
        let state = self.lock();
        let Some(StructuralMutationFlight::Rename(flight)) = &state.structural_mutation_pending
        else {
            return None;
        };
        if flight.stage == PendingRefreshStage::Queued {
            return None;
        }
        match &flight.result {
            Some(RenameResult::Committed { revision, .. }) => flight
                .operation
                .known_commit(*revision)
                .map(RenameWorkerEvidence::Known)
                .or_else(|| Some(RenameWorkerEvidence::Unknown(flight.operation.clone()))),
            Some(
                RenameResult::NotSubmitted
                | RenameResult::Precommit(_)
                | RenameResult::Unchanged(_),
            ) => None,
            None => Some(RenameWorkerEvidence::Unknown(flight.operation.clone())),
        }
    }
}
