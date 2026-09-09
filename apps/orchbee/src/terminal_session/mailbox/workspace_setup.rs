use super::{
    AttachmentKey, BarrierFence, BarrierKind, COMMAND_CAPACITY, CommandMailbox, EventDeadline,
    EventError, MailboxClock, MailboxClosed, MailboxState, MembershipResultKey, MoveAdmissionError,
    MutationResult, QueuedCommand, QueuedRecord, RefreshFlight, StructuralMutationFlight,
    UiCorrelation, UiWaitKey, WorkerEvent, workbench_identity_candidate,
};
use crate::local_client::LocalOperationClock;
use crate::terminal_session::state::{
    NativeSessionState,
    workspace_setup::{
        NativeSetupIntent, NativeSetupToken, SetupOperation, SetupResult, SetupWorkerEvidence,
    },
};
use crate::terminal_session::state::{OperationDeadlines, PendingRefreshStage};

pub(super) struct SetupFlight {
    serial: u64,
    desired: Option<AttachmentKey>,
    operation: SetupOperation,
    stage: PendingRefreshStage,
    not_applied: bool,
    known: Option<crate::terminal_session::state::workspace_setup::SetupKnownCommit>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct WorkspaceSetupMarker(
    pub(in crate::terminal_session) SetupOperation,
);
impl WorkspaceSetupMarker {
    pub(super) fn barrier(&self) -> BarrierFence {
        BarrierFence::from_deadlines(BarrierKind::WorkspaceSetup, self.0.deadlines)
    }
}
impl CommandMailbox {
    pub(in crate::terminal_session) fn open_setup_draft_for(
        &self,
        canonical: &mut NativeSessionState,
        intent: NativeSetupIntent,
    ) -> Option<NativeSetupToken> {
        let state = self.lock();
        if Self::ensure_open(&state).is_err()
            || state.desired.awaiting_bootstrap()
            || state.desired.key() != canonical.desired()
            || state.structural_mutation_pending.is_some()
            || state.refresh_flight.is_some()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state
                .queue
                .iter()
                .any(|entry| entry.command.barrier().is_some())
        {
            return None;
        }
        canonical.open_setup_draft(intent)
    }
    pub(in crate::terminal_session) fn try_enqueue_setup_for(
        &self,
        canonical: &mut NativeSessionState,
        token: NativeSetupToken,
        deadlines: OperationDeadlines,
        clock: &LocalOperationClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        self.try_enqueue_setup_for_using(canonical, token, deadlines, clock)
    }
    pub(super) fn try_enqueue_setup_for_using(
        &self,
        canonical: &mut NativeSessionState,
        token: NativeSetupToken,
        deadlines: OperationDeadlines,
        clock: &dyn MailboxClock,
    ) -> Result<(), MoveAdmissionError<()>> {
        let (mut state, _) = self
            .lock_before(deadlines.forward_cutoff(), clock)
            .ok_or(MoveAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(MoveAdmissionError::Closed)?;
        if state.desired.awaiting_bootstrap()
            || state.desired.key() != canonical.desired()
            || state.structural_mutation_pending.is_some()
            || state.refresh_flight.is_some()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state
                .queue
                .iter()
                .any(|entry| entry.command.barrier().is_some())
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
        let generation = state
            .refresh_generation
            .checked_add(1)
            .ok_or(MoveAdmissionError::IdentityExhausted)?;
        let operation = canonical
            .plan_setup(token, request_id, generation, deadlines)
            .ok_or(MoveAdmissionError::Build(()))?;
        let flight = Box::new(SetupFlight {
            serial,
            desired: canonical.desired().cloned(),
            operation: operation.clone(),
            stage: PendingRefreshStage::Queued,
            not_applied: false,
            known: None,
        });
        let marker = WorkspaceSetupMarker(operation.clone());
        if clock.now() >= deadlines.forward_cutoff() {
            return Err(MoveAdmissionError::Deadline);
        }
        canonical.begin_setup(operation);
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.refresh_generation = generation;
        state.refresh_flight = Some(RefreshFlight {
            generation,
            stage: PendingRefreshStage::Queued,
        });
        state.structural_mutation_pending = Some(StructuralMutationFlight::Setup(flight));
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::WorkspaceSetup(marker),
        });
        self.changed.notify_all();
        Ok(())
    }
    pub(in crate::terminal_session) fn mark_setup_in_flight(
        &self,
        operation: &SetupOperation,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let serial = state.active_lease.map(|lease| lease.serial);
        let valid = matches!(&state.structural_mutation_pending, Some(StructuralMutationFlight::Setup(flight)) if flight.operation == *operation && flight.stage == PendingRefreshStage::Queued && serial == Some(flight.serial));
        if !valid {
            return Ok(MutationResult::Stale);
        }
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return Ok(MutationResult::Stale);
        };
        flight.stage = PendingRefreshStage::InFlight;
        state.refresh_flight = Some(RefreshFlight {
            generation: operation.generation,
            stage: PendingRefreshStage::InFlight,
        });
        Ok(MutationResult::Applied)
    }
    pub(in crate::terminal_session) fn publish_setup_result_before(
        &self,
        operation: SetupOperation,
        outcome: SetupResult,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        let deadline = operation.deadlines.forward_cutoff();
        self.publish_event_before(
            WorkerEvent::WorkspaceSetupResult { operation, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }
    pub(in crate::terminal_session) fn retain_setup_not_applied(
        &self,
        operation: &SetupOperation,
    ) -> bool {
        let mut state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.operation != *operation
            || flight.stage != PendingRefreshStage::InFlight
            || flight.known.is_some()
        {
            return false;
        }
        flight.not_applied = true;
        true
    }
    pub(in crate::terminal_session) fn retain_setup_known(
        &self,
        known: &crate::terminal_session::state::workspace_setup::SetupKnownCommit,
    ) -> bool {
        let prepared = known.clone();
        let mut state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) =
            state.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.not_applied
            || flight.operation != known.operation
            || flight.stage != PendingRefreshStage::InFlight
            || flight.known.as_ref().is_some_and(|prior| prior != known)
        {
            return false;
        }
        flight.known = Some(prepared);
        true
    }
    pub(in crate::terminal_session) fn setup_worker_loss(&self) -> Option<SetupWorkerEvidence> {
        let state = self.lock();
        let Some(StructuralMutationFlight::Setup(flight)) = &state.structural_mutation_pending
        else {
            return None;
        };
        if let Some(known) = &flight.known {
            return Some(SetupWorkerEvidence::Known(known.clone()));
        }
        if flight.stage == PendingRefreshStage::Queued {
            return Some(SetupWorkerEvidence::NotApplied(flight.operation.clone()));
        }
        Some(if flight.not_applied {
            SetupWorkerEvidence::NotApplied(flight.operation.clone())
        } else {
            SetupWorkerEvidence::Unknown(flight.operation.clone())
        })
    }
}
impl MailboxState {
    pub(super) fn setup_reply_current(&self, correlation: &UiCorrelation) -> bool {
        let Some(StructuralMutationFlight::Setup(flight)) = &self.structural_mutation_pending
        else {
            return true;
        };
        let generation = match correlation {
            UiCorrelation::Refresh(generation)
            | UiCorrelation::Membership(MembershipResultKey::Refresh(generation), _) => *generation,
            _ => return true,
        };
        generation == flight.operation.generation
            && flight.stage == PendingRefreshStage::AwaitingUi
            && self.active_lease.map(|lease| lease.serial) == Some(flight.serial)
            && self.event_anchor_serial == Some(flight.serial)
            && self.desired.key() == flight.desired.as_ref()
    }
    pub(super) fn place_setup_result(
        &mut self,
        operation: &SetupOperation,
        outcome: &SetupResult,
        deadline: EventDeadline,
        anchor: Option<u64>,
    ) -> bool {
        let Some(StructuralMutationFlight::Setup(flight)) =
            self.structural_mutation_pending.as_mut()
        else {
            return false;
        };
        if flight.operation != *operation
            || flight.stage != PendingRefreshStage::InFlight
            || anchor != Some(flight.serial)
            || deadline != EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff())
        {
            return false;
        }
        let valid_result = match outcome {
            SetupResult::Committed { known, .. } => flight.known.as_ref() == Some(known.as_ref()),
            SetupResult::NotApplied => flight.not_applied && flight.known.is_none(),
        };
        if !valid_result {
            return false;
        }
        flight.stage = PendingRefreshStage::AwaitingUi;
        self.refresh_flight = Some(RefreshFlight {
            generation: operation.generation,
            stage: PendingRefreshStage::AwaitingUi,
        });
        true
    }
}
