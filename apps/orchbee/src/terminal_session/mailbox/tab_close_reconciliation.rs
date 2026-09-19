use super::*;

pub(super) struct CloseReconciliationFlight {
    serial: u64,
    marker: CloseReconciliationMarker,
    stage: PendingRefreshStage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct CloseReconciliationSettlement {
    marker: CloseReconciliationMarker,
    settlement: RefreshSettlement,
}

impl CloseReconciliationSettlement {
    pub(in crate::terminal_session) fn new(
        marker: CloseReconciliationMarker,
        decision: RefreshDecision,
        desired: Option<PaneKey>,
    ) -> Self {
        Self {
            marker,
            settlement: RefreshSettlement::new(decision, desired),
        }
    }
    pub(in crate::terminal_session) const fn marker(&self) -> &CloseReconciliationMarker {
        &self.marker
    }
    pub(in crate::terminal_session) const fn settlement(&self) -> &RefreshSettlement {
        &self.settlement
    }
}

impl MailboxState {
    pub(super) fn exact_close_reconciliation(&self, correlation: &UiCorrelation) -> bool {
        self.close_reconciliation.as_ref().is_some_and(|flight| {
            flight.stage == PendingRefreshStage::AwaitingUi
                && matches!(correlation, UiCorrelation::CloseReconciliation(key, snapshot)
                    if key == flight.marker.origin().key() && snapshot == flight.marker.snapshot_request_id())
        })
    }

    pub(super) fn place_close_reconciliation(
        &mut self,
        marker: &CloseReconciliationMarker,
        deadline: EventDeadline,
        anchor: Option<u64>,
    ) -> bool {
        let Some(flight) = &mut self.close_reconciliation else {
            return false;
        };
        if flight.marker != *marker
            || flight.stage != PendingRefreshStage::InFlight
            || anchor != Some(flight.serial)
            || !matches!(deadline, EventDeadline::PlacementOnly(cutoff) if cutoff <= marker.deadline())
        {
            return false;
        }
        flight.stage = PendingRefreshStage::AwaitingUi;
        true
    }

    pub(super) fn settle_close_reconciliation(
        &mut self,
        correlation: &UiCorrelation,
        reply: &CloseReconciliationSettlement,
    ) -> bool {
        if !self.exact_close_reconciliation(correlation)
            || self
                .close_reconciliation
                .as_ref()
                .is_none_or(|flight| flight.marker != reply.marker)
            || self.desired.key() != reply.settlement.desired()
        {
            return false;
        }
        if let RefreshDecision::AttachRecovered(target) = reply.settlement.decision()
            && Some(target.selected().key()) != reply.settlement.desired()
        {
            return false;
        }
        if let RefreshDecision::Cleanup(cleanup) = reply.settlement.decision() {
            if cleanup.close_marker() != Some(&reply.marker) {
                return false;
            }
        } else {
            self.close_reconciliation = None;
        }
        true
    }
}

impl CommandMailbox {
    pub(in crate::terminal_session) fn take_close_acknowledgement(
        &self,
    ) -> Option<CloseLeaseAcknowledgement> {
        self.lock().close_acknowledgement.take()
    }
    pub(in crate::terminal_session) fn try_enqueue_close_reconciliation_for(
        &self,
        canonical: &mut super::super::state::NativeSessionState,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<CloseReconciliationMarker, CloseAdmissionError<super::super::state::CloseStateError>>
    {
        self.try_enqueue_close_reconciliation_using(canonical, deadline, clock)
    }

    pub(super) fn try_enqueue_close_reconciliation_using(
        &self,
        canonical: &mut super::super::state::NativeSessionState,
        deadline: Instant,
        clock: &dyn MailboxClock,
    ) -> Result<CloseReconciliationMarker, CloseAdmissionError<super::super::state::CloseStateError>>
    {
        let (mut state, _) = self
            .lock_before(deadline, clock)
            .ok_or(CloseAdmissionError::Deadline)?;
        Self::ensure_open(&state).map_err(CloseAdmissionError::Closed)?;
        if state.desired.awaiting_bootstrap()
            || state.close_flight.is_some()
            || state.close_reconciliation.is_some()
            || state.structural_mutation_pending.is_some()
            || state.refresh_flight.is_some()
            || state
                .queue
                .iter()
                .any(|record| record.command.barrier().is_some())
        {
            return Err(CloseAdmissionError::Busy);
        }
        if state.queue.len() == COMMAND_CAPACITY {
            return Err(CloseAdmissionError::Full);
        }
        let (cursor, request_id) = workbench_identity_candidate(state.identity_cursor)
            .ok_or(CloseAdmissionError::IdentityExhausted)?;
        let serial = state
            .next_command_serial
            .checked_add(1)
            .ok_or(CloseAdmissionError::IdentityExhausted)?;
        let marker = canonical
            .plan_close_reconciliation(request_id, deadline)
            .map_err(CloseAdmissionError::Build)?;
        canonical
            .begin_close_reconciliation(&marker)
            .map_err(CloseAdmissionError::Build)?;
        state.identity_cursor = cursor;
        state.next_command_serial = serial;
        state.close_reconciliation = Some(CloseReconciliationFlight {
            serial,
            marker: marker.clone(),
            stage: PendingRefreshStage::Queued,
        });
        state.queue.push_back(QueuedRecord {
            serial,
            command: QueuedCommand::ReconcileClose(marker.clone()),
            retry_at: None,
        });
        self.changed.notify_all();
        Ok(marker)
    }

    pub(in crate::terminal_session) fn mark_close_reconciliation_in_flight(
        &self,
        marker: &CloseReconciliationMarker,
    ) -> Result<MutationResult, MailboxClosed> {
        let mut state = self.lock();
        Self::ensure_open(&state)?;
        let serial = state.active_lease.map(|lease| lease.serial);
        match &mut state.close_reconciliation {
            Some(flight)
                if flight.marker == *marker
                    && serial == Some(flight.serial)
                    && flight.stage == PendingRefreshStage::Queued =>
            {
                flight.stage = PendingRefreshStage::InFlight;
                Ok(MutationResult::Applied)
            }
            _ => Ok(MutationResult::Stale),
        }
    }

    pub(in crate::terminal_session) fn publish_close_reconciliation_before(
        &self,
        marker: CloseReconciliationMarker,
        outcome: WorkbenchSnapshotOutcome,
        deadline: Instant,
        clock: &LocalOperationClock,
    ) -> Result<UiWaitKey, EventError> {
        self.publish_event_before(
            WorkerEvent::CloseReconciliationSnapshot { marker, outcome },
            EventDeadline::PlacementOnly(deadline),
            clock,
        )
    }
}
