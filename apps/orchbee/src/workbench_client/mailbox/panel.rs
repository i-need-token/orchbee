//! Owns bounded Panel-operation queue admission, structural barriers, exact in-flight markers,
//! and correlated result settlement. It performs no protocol I/O and does not apply canonical
//! topology or UI membership itself.

use super::{
    BarrierFence, BarrierKind, COMMAND_CAPACITY, CommandMailbox, EventDeadline, EventError,
    Instant, LocalOperationClock, MailboxClock, MailboxClosed, MailboxState, MembershipResultKey,
    MoveAdmissionError, MutationResult, PaneKey, PendingRefreshStage, QueuedCommand, QueuedRecord,
    StructuralMutationFlight, UiCorrelation, UiWaitKey, WorkbenchRequestId, WorkerEvent,
    workbench_identity_candidate,
};
use crate::workbench_client::state::{PanelDecision, PanelOperation, PanelResult};

pub(super) struct PanelFlight {
    pub(super) serial: u64,
    pub(super) operation: PanelOperation,
    pub(super) stage: PendingRefreshStage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct PanelMarker {
    operation: PanelOperation,
}

impl PanelMarker {
    pub(in crate::workbench_client) const fn new(operation: PanelOperation) -> Self {
        Self { operation }
    }
    pub(in crate::workbench_client) const fn operation(&self) -> &PanelOperation {
        &self.operation
    }
    pub(super) fn barrier(&self) -> BarrierFence {
        BarrierFence {
            kind: BarrierKind::Panel,
            forward_cutoff: self.operation.deadline(),
            total: self.operation.deadline(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::workbench_client) struct PanelSettlement {
    decision: PanelDecision,
    desired: Option<PaneKey>,
}

impl PanelSettlement {
    pub(in crate::workbench_client) fn new(
        decision: PanelDecision,
        desired: impl Into<Option<PaneKey>>,
    ) -> Self {
        Self {
            decision,
            desired: desired.into(),
        }
    }
    pub(in crate::workbench_client) const fn decision(&self) -> &PanelDecision {
        &self.decision
    }
    pub(in crate::workbench_client) const fn desired(&self) -> Option<&PaneKey> {
        self.desired.as_ref()
    }
}

impl CommandMailbox {
    pub(in crate::workbench_client) fn try_enqueue_panel_for<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        clock: &LocalOperationClock,
        build: impl FnOnce(&Canonical, WorkbenchRequestId) -> Result<PanelOperation, BuildError>,
        commit: impl FnOnce(&mut Canonical, &PanelOperation) -> Result<(), BuildError>,
    ) -> Result<PanelOperation, MoveAdmissionError<BuildError>> {
        self.try_enqueue_panel_for_using(canonical, deadline, clock, build, commit)
    }

    pub(super) fn try_enqueue_panel_for_using<Canonical, BuildError>(
        &self,
        canonical: &mut Canonical,
        deadline: Instant,
        clock: &dyn MailboxClock,
        build: impl FnOnce(&Canonical, WorkbenchRequestId) -> Result<PanelOperation, BuildError>,
        commit: impl FnOnce(&mut Canonical, &PanelOperation) -> Result<(), BuildError>,
    ) -> Result<PanelOperation, MoveAdmissionError<BuildError>> {
        let (mut state, _) = self
            .lock_before(deadline, clock)
            .ok_or(MoveAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(MoveAdmissionError::Closed)?;
        if state.desired.key().is_none()
            || state.structural_mutation_pending.is_some()
            || (state.close_flight.is_some() || state.close_reconciliation.is_some())
            || state.refresh_flight.is_some()
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
        let operation = build(canonical, request_id.clone()).map_err(MoveAdmissionError::Build)?;
        if operation.request_id() != &request_id
            || operation.deadline() != deadline
            || state.desired.key().map(PaneKey::anchor) != Some(operation.target())
            || operation
                .baseline()
                .tab_index(operation.target().tab_id())
                .is_none()
            || operation
                .baseline()
                .tabs()
                .get(
                    operation
                        .baseline()
                        .tab_index(operation.target().tab_id())
                        .unwrap_or(0),
                )
                .is_none_or(|tab| tab.panel_index(operation.panel_id()).is_none())
        {
            return Err(MoveAdmissionError::InvalidTarget);
        }
        let flight = Box::new(PanelFlight {
            serial,
            operation: operation.clone(),
            stage: PendingRefreshStage::Queued,
        });
        commit(canonical, &operation).map_err(MoveAdmissionError::Build)?;
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.structural_mutation_pending = Some(StructuralMutationFlight::Panel(flight));
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::Panel(PanelMarker::new(operation.clone())),
            retry_at: None,
        });
        self.changed.notify_all();
        Ok(operation)
    }

    pub(in crate::workbench_client) fn mark_panel_in_flight(
        &self,
        operation: &PanelOperation,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let active_serial = state.active_lease.map(|lease| lease.serial);
        match state.structural_mutation_pending.as_mut() {
            Some(StructuralMutationFlight::Panel(flight))
                if flight.operation == *operation
                    && flight.stage == PendingRefreshStage::Queued
                    && active_serial == Some(flight.serial) =>
            {
                flight.stage = PendingRefreshStage::InFlight;
                self.changed.notify_all();
                Ok(MutationResult::Applied)
            }
            _ => Ok(MutationResult::Stale),
        }
    }

    pub(in crate::workbench_client) fn publish_panel_result_before(
        &self,
        operation: PanelOperation,
        outcome: PanelResult,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before(
            WorkerEvent::PanelResult { operation, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }
}

impl MailboxState {
    pub(super) fn place_panel_result(
        &mut self,
        operation: &PanelOperation,
        deadline: EventDeadline,
        anchor: Option<u64>,
    ) -> bool {
        let Some(StructuralMutationFlight::Panel(flight)) =
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
        true
    }

    pub(super) fn panel_reply_current(&self, correlation: &UiCorrelation) -> bool {
        let (request_id, old) = match correlation {
            UiCorrelation::Panel(id) => (id, None),
            UiCorrelation::Membership(MembershipResultKey::Panel(id), old) => (id, Some(old)),
            _ => return true,
        };
        matches!(&self.structural_mutation_pending, Some(StructuralMutationFlight::Panel(flight)) if flight.operation.request_id() == request_id
            && flight.stage == PendingRefreshStage::AwaitingUi
            && self.active_lease.map(|lease| lease.serial) == Some(flight.serial)
            && self.event_anchor_serial == Some(flight.serial)
            && self.desired.key().map(PaneKey::anchor) == Some(flight.operation.target())
            && old.is_none_or(|old| old == flight.operation.target()))
    }
}
